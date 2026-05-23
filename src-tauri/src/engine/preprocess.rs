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
    engine::{settings::transcription_enabled, state::AppState},
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

/// Return true when `<stem>.anubis.txt` already exists next to `source`
/// (or in `ANUBIS_TRANSCRIPT_DIR` if set) and its mtime is `>=` the
/// source's. Same cache policy as `parser::video::read_fresh_sidecar` and
/// `parser::image::read_fresh_sidecar` — kept consistent so the pre-pass
/// and the indexer always agree on what counts as "already done".
pub fn sidecar_fresh(source: &Path) -> bool {
    let Some(stem) = source.file_stem().and_then(|s| s.to_str()) else {
        return false;
    };
    let sidecar_dir = sidecar_dir_for(source);
    let sidecar = sidecar_dir.join(format!("{stem}.anubis.txt"));
    let (Ok(src_meta), Ok(car_meta)) = (std::fs::metadata(source), std::fs::metadata(&sidecar))
    else {
        return false;
    };
    match (src_meta.modified(), car_meta.modified()) {
        (Ok(src_mtime), Ok(car_mtime)) => car_mtime >= src_mtime,
        _ => false,
    }
}

fn sidecar_dir_for(source: &Path) -> PathBuf {
    if let Ok(env_dir) = std::env::var("ANUBIS_TRANSCRIPT_DIR") {
        if !env_dir.trim().is_empty() {
            return PathBuf::from(env_dir);
        }
    }
    source
        .parent()
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
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

        if sidecar_fresh(path) {
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
                    write_sidecar_atomic(path, "").map_err(|error| EngineError::Parse {
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
            write_sidecar_atomic(path, &text).map_err(|error| EngineError::Parse {
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

/// Write `<stem>.anubis.txt` atomically: write to `.tmp`, then rename. A
/// crash mid-write never leaves a half-written sidecar that would later be
/// served as if it were a valid cache.
fn write_sidecar_atomic(source: &Path, text: &str) -> std::io::Result<()> {
    let stem = source
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| std::io::Error::other("source has no file stem"))?;
    let dir = sidecar_dir_for(source);
    std::fs::create_dir_all(&dir)?;
    let final_path = dir.join(format!("{stem}.anubis.txt"));
    let tmp_path = dir.join(format!("{stem}.anubis.txt.tmp"));
    std::fs::write(&tmp_path, text)?;
    std::fs::rename(&tmp_path, &final_path)?;
    Ok(())
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
    use super::{needs_preprocessing, sidecar_fresh};
    use crate::types::PreprocessKind;
    use std::time::{Duration, SystemTime};

    fn write_with_mtime(path: &std::path::Path, contents: &str, mtime: SystemTime) {
        std::fs::write(path, contents).expect("write fixture");
        let file = std::fs::File::options()
            .write(true)
            .open(path)
            .expect("open for mtime");
        file.set_modified(mtime).expect("set mtime");
    }

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

    #[test]
    fn sidecar_fresh_is_true_when_sidecar_at_least_as_new() {
        let tmp = std::env::temp_dir().join(format!("anubis-pp-fresh-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&tmp).unwrap();
        let source = tmp.join("clip.mp4");
        let sidecar = tmp.join("clip.anubis.txt");

        let base = SystemTime::now() - Duration::from_secs(10);
        write_with_mtime(&source, "fake mp4", base);
        write_with_mtime(&sidecar, "hello", base + Duration::from_secs(5));

        assert!(sidecar_fresh(&source));

        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn sidecar_fresh_is_false_when_source_is_newer() {
        let tmp = std::env::temp_dir().join(format!("anubis-pp-stale-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&tmp).unwrap();
        let source = tmp.join("clip.mp4");
        let sidecar = tmp.join("clip.anubis.txt");

        let base = SystemTime::now() - Duration::from_secs(10);
        write_with_mtime(&sidecar, "stale", base);
        write_with_mtime(&source, "fresh", base + Duration::from_secs(5));

        assert!(!sidecar_fresh(&source));

        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn sidecar_fresh_is_false_when_sidecar_missing() {
        let tmp = std::env::temp_dir().join(format!("anubis-pp-missing-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&tmp).unwrap();
        let source = tmp.join("clip.mp4");
        write_with_mtime(&source, "fake", SystemTime::now());

        assert!(!sidecar_fresh(&source));

        std::fs::remove_dir_all(&tmp).ok();
    }
}
