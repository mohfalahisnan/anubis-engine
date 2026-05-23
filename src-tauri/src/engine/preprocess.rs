//! Preprocessing pre-pass.
//!
//! Heavy IO-bound or model-bound work (Whisper transcription, image OCR)
//! used to run inline inside the relevant `parser::*::parse` function,
//! which had three painful consequences:
//!
//!   1. The user could not tell whether a long stall was transcription,
//!      embedding, or a deadlock — every file just showed its name and
//!      "Indexing" forever.
//!   2. Failures mid-batch (a corrupt video, an empty audio track) were
//!      handled per-file but the user had no UI signal until the indexer
//!      finished walking everything.
//!   3. Re-indexing required re-doing the heavy work even when the cached
//!      sidecar (`<stem>.anubis.txt`) was perfectly fresh.
//!
//! This module runs preprocessing as a distinct first stage of
//! `index_folder`. The indexer's parsers then read the sidecar instead of
//! re-running the heavy step. Failures mark the document as `status='error'`
//! and skip it in the index pass, so one bad file doesn't poison the batch.

use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;

use chrono::Utc;
use tauri::{AppHandle, Emitter};

use crate::{
    engine::{settings::transcription_enabled, sidecar, state::AppState},
    parser,
    store::db,
    transcription::engine::transcribe_file,
    types::{DocFormat, IndexStatus, PreprocessKind, PreprocessProgress, PreprocessStage},
    EngineError,
};

/// Outcome of running the pre-pass over a batch of paths.
#[derive(Default, Debug, Clone)]
pub struct PreprocessReport {
    /// Files that completed preprocessing in this run (newly written sidecar).
    pub ok: Vec<PathBuf>,
    /// Files whose sidecar was already up to date — no work performed.
    pub skipped_fresh: Vec<PathBuf>,
    /// Files that errored. The string is the human-readable reason; these
    /// files are also recorded as `status='error'` documents in the DB so
    /// the UI can surface them.
    pub failed: Vec<(PathBuf, String)>,
    /// True iff the user invoked `cancel_indexing` partway through.
    pub cancelled: bool,
}

/// Decide whether a file needs the pre-pass at all, and what kind of work.
/// Files that don't need preprocessing (text, JSON, PDF text-extraction,
/// docx, csv, xlsx, markdown) return `None` and skip Stage B entirely.
///
/// `Pdf` is reserved for future scanned-PDF page-image OCR — the framework
/// supports plugging it in but today the PDF parser uses `lopdf` text
/// extraction inline, which is fast enough that no pre-pass is needed.
pub fn needs_preprocessing(path: &Path) -> Option<PreprocessKind> {
    match parser::format_from_path(path) {
        DocFormat::Video => Some(PreprocessKind::Video),
        DocFormat::Audio => Some(PreprocessKind::Audio),
        DocFormat::Image => Some(PreprocessKind::Image),
        _ => None,
    }
}

/// Run the pre-pass over the given paths in order. Sequential — Whisper
/// and OCR are already multi-core internally; running multiple at once
/// thrashes. Emits `preprocess-progress` events between files. On
/// failure, records the file as `status='error'` in the DB and continues
/// with the next one. Respects `state.cancel_token`.
pub async fn run_preprocessing(
    paths: &[PathBuf],
    state: &AppState,
    app: Option<AppHandle>,
) -> Result<PreprocessReport, EngineError> {
    let mut report = PreprocessReport::default();

    // Filter to only files that actually need work, before we count.
    let work: Vec<(PathBuf, PreprocessKind)> = paths
        .iter()
        .filter_map(|p| needs_preprocessing(p).map(|kind| (p.clone(), kind)))
        .collect();
    let total = work.len();
    if total == 0 {
        // Nothing to do — emit a single Done event so the UI knows the
        // pre-pass finished and Stage C is about to start.
        emit(
            &app,
            PreprocessProgress {
                total: 0,
                done: 0,
                current: String::new(),
                kind: None,
                stage: None,
                status: IndexStatus::Done,
                errors: Vec::new(),
            },
        );
        return Ok(report);
    }

    emit(
        &app,
        PreprocessProgress {
            total,
            done: 0,
            current: String::new(),
            kind: None,
            stage: None,
            status: IndexStatus::Running,
            errors: Vec::new(),
        },
    );

    for (index, (path, kind)) in work.iter().enumerate() {
        if state.cancel_token.load(Ordering::Relaxed) {
            report.cancelled = true;
            emit(
                &app,
                PreprocessProgress {
                    total,
                    done: index,
                    current: String::new(),
                    kind: None,
                    stage: None,
                    status: IndexStatus::Cancelled,
                    errors: collect_errors(&report),
                },
            );
            return Ok(report);
        }

        let current = filename_of(path);

        if sidecar::is_fresh(path) {
            report.skipped_fresh.push(path.clone());
            emit(
                &app,
                PreprocessProgress {
                    total,
                    done: index + 1,
                    current: current.clone(),
                    kind: Some(*kind),
                    stage: Some(PreprocessStage::CachedSkipped),
                    status: IndexStatus::Running,
                    errors: collect_errors(&report),
                },
            );
            continue;
        }

        // Emit a "starting this file" event so the UI updates BEFORE the
        // slow step rather than only after it finishes.
        emit(
            &app,
            PreprocessProgress {
                total,
                done: index,
                current: current.clone(),
                kind: Some(*kind),
                stage: Some(stage_for(*kind)),
                status: IndexStatus::Running,
                errors: collect_errors(&report),
            },
        );

        // Skip Whisper entirely when the user has the toggle off.
        // (`transcribe_file` would also refuse but doing it here keeps the
        // pre-pass honest about why a video has no sidecar.)
        if matches!(kind, PreprocessKind::Video | PreprocessKind::Audio) && !transcription_enabled()
        {
            report.skipped_fresh.push(path.clone());
            emit(
                &app,
                PreprocessProgress {
                    total,
                    done: index + 1,
                    current: current.clone(),
                    kind: Some(*kind),
                    stage: Some(PreprocessStage::CachedSkipped),
                    status: IndexStatus::Running,
                    errors: collect_errors(&report),
                },
            );
            continue;
        }

        match preprocess_one(path, *kind) {
            Ok(()) => report.ok.push(path.clone()),
            Err(error) => {
                let msg = error.to_string();
                tracing::error!("preprocessing {} failed: {}", path.display(), msg);
                // Record an error document so the UI surfaces the failure
                // and excludes the file from Stage C.
                if let Err(record_err) = record_error_doc(state, path, &msg).await {
                    tracing::warn!(
                        "failed to record error doc for {}: {}",
                        path.display(),
                        record_err
                    );
                }
                report.failed.push((path.clone(), msg));
            }
        }

        emit(
            &app,
            PreprocessProgress {
                total,
                done: index + 1,
                current: current.clone(),
                kind: Some(*kind),
                stage: Some(stage_for(*kind)),
                status: IndexStatus::Running,
                errors: collect_errors(&report),
            },
        );
    }

    let final_status = if report.failed.is_empty() {
        IndexStatus::Done
    } else {
        // Non-empty failed list is NOT a fatal error — the indexer pass
        // will still run on the surviving files. We surface this as a
        // "done-with-errors" by including the errors but Done status.
        IndexStatus::Done
    };
    emit(
        &app,
        PreprocessProgress {
            total,
            done: total,
            current: String::new(),
            kind: None,
            stage: None,
            status: final_status,
            errors: collect_errors(&report),
        },
    );
    Ok(report)
}

fn preprocess_one(path: &Path, kind: PreprocessKind) -> Result<(), EngineError> {
    match kind {
        PreprocessKind::Video | PreprocessKind::Audio => {
            // `transcribe_file` already writes `<stem>.anubis.txt` (and
            // optional `.anubis.wav`) atomically. Empty transcript is a
            // valid outcome (no usable speech) — record an empty sidecar
            // so the next reindex skips re-running Whisper.
            if let Err(error) = transcribe_file(path) {
                if matches!(error, EngineError::NoAudioTrack(_)) {
                    sidecar::write_atomic(path, "").map_err(|error| EngineError::Parse {
                        path: path.to_string_lossy().into_owned(),
                        msg: format!("write empty transcript sidecar: {error}"),
                    })?;
                } else {
                    return Err(error);
                }
            }
            Ok(())
        }
        PreprocessKind::Image => {
            let bytes = std::fs::read(path).map_err(EngineError::Io)?;
            let text = crate::ocr::engine::run(&bytes)?;
            sidecar::write_atomic(path, &text).map_err(|error| EngineError::Parse {
                path: path.to_string_lossy().into_owned(),
                msg: format!("write OCR sidecar: {error}"),
            })?;
            Ok(())
        }
        PreprocessKind::Pdf => {
            // Reserved — the framework supports it but no scanned-PDF
            // rasterizer is wired today. Treat as a no-op so the indexer
            // falls through to the existing lopdf text-extraction path.
            Ok(())
        }
    }
}

fn stage_for(kind: PreprocessKind) -> PreprocessStage {
    match kind {
        PreprocessKind::Video | PreprocessKind::Audio => PreprocessStage::Transcribing,
        PreprocessKind::Image | PreprocessKind::Pdf => PreprocessStage::Ocr,
    }
}

fn filename_of(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| path.to_string_lossy().into_owned())
}

fn collect_errors(report: &PreprocessReport) -> Vec<String> {
    report
        .failed
        .iter()
        .map(|(path, msg)| format!("{}: {}", path.display(), msg))
        .collect()
}

async fn record_error_doc(
    state: &AppState,
    path: &Path,
    error_msg: &str,
) -> Result<(), EngineError> {
    let meta = parser::metadata_for_path(path)?;
    let preflight_path = path.to_string_lossy().into_owned();
    let format = parser::format_from_path(path);
    let doc_class = parser::doc_class_from_path(path);

    let db = state.db.lock().await;
    let existing = db::get_document_by_path(&db, &preflight_path)?;
    let doc = db::DocumentRecord {
        id: existing
            .as_ref()
            .map(|d| d.id.clone())
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
        path: preflight_path,
        filename: meta.filename,
        format,
        size_bytes: meta.size_bytes,
        hash: meta.hash,
        indexed_at: Utc::now().to_rfc3339(),
        status: "error".to_string(),
        error_msg: Some(format!("preprocess: {error_msg}")),
        doc_class,
    };
    db::upsert_document(&db, &doc)?;
    Ok(())
}

fn emit(app: &Option<AppHandle>, progress: PreprocessProgress) {
    if let Some(app) = app {
        if let Err(error) = app.emit("preprocess-progress", progress) {
            tracing::warn!("failed to emit preprocess progress: {}", error);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::needs_preprocessing;
    use crate::types::PreprocessKind;

    #[test]
    fn classifies_paths_by_needed_preprocessing() {
        let cases: &[(&str, Option<PreprocessKind>)] = &[
            ("a.mp4", Some(PreprocessKind::Video)),
            ("a.MOV", Some(PreprocessKind::Video)),
            ("a.mp3", Some(PreprocessKind::Audio)),
            ("a.png", Some(PreprocessKind::Image)),
            ("a.JPG", Some(PreprocessKind::Image)),
            ("a.pdf", None),
            ("a.md", None),
            ("a.json", None),
            ("a.csv", None),
        ];
        for (name, expected) in cases {
            assert_eq!(
                needs_preprocessing(std::path::Path::new(name)),
                *expected,
                "{name}",
            );
        }
    }

    // Sidecar freshness / write-atomic tests live in `engine::sidecar`'s
    // test module now that the policy is owned there.
}
