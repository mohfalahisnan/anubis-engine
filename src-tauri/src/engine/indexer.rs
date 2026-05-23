use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;

use chrono::Utc;
use tauri::{AppHandle, Emitter};
use walkdir::WalkDir;

use crate::{
    chunker::sliding,
    embedder::local,
    entities,
    graph::builder,
    parser,
    store::{
        chunks, db, entities as entity_store, fts,
        graph_store::{self},
        vectors,
    },
    types::{IndexProgress, IndexStage, IndexStatus},
    EngineError,
};

use super::{preprocess, state::AppState};

#[derive(Debug, Default)]
struct IndexRunReport {
    errors: Vec<String>,
    cancelled: bool,
    done: usize,
}

pub async fn index_folder(
    path: &str,
    state: &AppState,
    app: Option<AppHandle>,
) -> Result<(), EngineError> {
    let mut indexing = state.indexing.lock().await;
    if *indexing {
        return Err(EngineError::AlreadyIndexing);
    }
    state.cancel_token.store(false, Ordering::Relaxed);
    *indexing = true;
    drop(indexing);

    let result = index_folder_inner(path, state, app).await;

    let mut indexing = state.indexing.lock().await;
    *indexing = false;
    result
}

pub async fn index_file(path: &str, state: &AppState) -> Result<(), EngineError> {
    state.cancel_token.store(false, Ordering::Relaxed);
    let _ = run_index_paths(&[PathBuf::from(path)], state, None).await?;
    Ok(())
}

async fn index_folder_inner(
    path: &str,
    state: &AppState,
    app: Option<AppHandle>,
) -> Result<(), EngineError> {
    let paths = collect_supported_files(Path::new(path));
    let preprocess_report = preprocess::run_preprocessing(&paths, state, app.clone()).await?;
    if preprocess_report.cancelled {
        emit_progress(
            state,
            &app,
            paths.len(),
            0,
            "",
            IndexStatus::Cancelled,
            preprocess_report
                .failed
                .iter()
                .map(|(path, error)| format!("{}: {}", path.display(), error))
                .collect(),
            None,
        );
        return Ok(());
    }

    let failed_preprocess: HashSet<PathBuf> = preprocess_report
        .failed
        .iter()
        .map(|(path, _)| path.clone())
        .collect();
    let indexable_paths: Vec<PathBuf> = paths
        .into_iter()
        .filter(|path| !failed_preprocess.contains(path))
        .collect();

    let report = run_index_paths(&indexable_paths, state, app.clone()).await?;
    let status = if report.cancelled {
        IndexStatus::Cancelled
    } else if report.errors.is_empty() {
        IndexStatus::Done
    } else {
        IndexStatus::Error
    };
    emit_progress(
        state,
        &app,
        indexable_paths.len(),
        report.done,
        "",
        status,
        report.errors,
        None,
    );
    Ok(())
}

async fn run_index_paths(
    paths: &[PathBuf],
    state: &AppState,
    app: Option<AppHandle>,
) -> Result<IndexRunReport, EngineError> {
    let mut errors = Vec::new();

    for (index, path) in paths.iter().enumerate() {
        if state.cancel_token.load(Ordering::Relaxed) {
            emit_progress(
                state,
                &app,
                paths.len(),
                index,
                "",
                IndexStatus::Cancelled,
                errors.clone(),
                None,
            );
            return Ok(IndexRunReport {
                errors,
                cancelled: true,
                done: index,
            });
        }

        let current = path
            .file_name()
            .and_then(|name| name.to_str())
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| path.to_string_lossy().into_owned());
        emit_progress(
            state,
            &app,
            paths.len(),
            index,
            &current,
            IndexStatus::Running,
            errors.clone(),
            None,
        );

        match index_one(path, state, &app, paths.len(), index, &current, &errors).await {
            Ok(true) => {}
            Ok(false) => {
                return Ok(IndexRunReport {
                    errors,
                    cancelled: true,
                    done: index,
                });
            }
            Err(error) => {
                tracing::error!("failed to index {}: {}", path.display(), error);
                errors.push(format!("{}: {}", path.display(), error));
            }
        }
    }

    Ok(IndexRunReport {
        errors,
        cancelled: false,
        done: paths.len(),
    })
}

async fn index_one(
    path: &Path,
    state: &AppState,
    app: &Option<AppHandle>,
    total: usize,
    done: usize,
    current: &str,
    errors: &[String],
) -> Result<bool, EngineError> {
    if check_cancelled(state, app, total, done, current, errors) {
        return Ok(false);
    }
    emit_progress(
        state,
        app,
        total,
        done,
        current,
        IndexStatus::Running,
        errors.to_vec(),
        Some(IndexStage::Parsing),
    );

    // Pre-flight: read just the file metadata so even if parsing fails (e.g.
    // a long transcription that errors halfway), the user still sees the
    // document in the index with status='error' instead of having it silently
    // disappear from the list.
    let preflight_meta = parser::metadata_for_path(path)?;
    let preflight_path = path.to_string_lossy().into_owned();
    let preflight_format = parser::format_from_path(path);
    let preflight_class = parser::doc_class_from_path(path);

    let existing_for_id = {
        let db = state.db.lock().await;
        db::get_document_by_path(&db, &preflight_path)?
    };

    let mut parsed = match parser::parse(path) {
        Ok(parsed) => parsed,
        Err(error) => {
            // Record the failure so the UI can show it and the user can re-try
            // by re-indexing. Without this branch the file would never show up
            // in the documents list.
            let error_doc = db::DocumentRecord {
                id: existing_for_id
                    .as_ref()
                    .map(|d| d.id.clone())
                    .unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
                path: preflight_path.clone(),
                filename: preflight_meta.filename.clone(),
                format: preflight_format,
                size_bytes: preflight_meta.size_bytes,
                hash: preflight_meta.hash.clone(),
                indexed_at: Utc::now().to_rfc3339(),
                status: "error".to_string(),
                error_msg: Some(error.to_string()),
                doc_class: preflight_class,
            };
            let db = state.db.lock().await;
            db::upsert_document(&db, &error_doc)?;
            tracing::error!(
                "parse failed for {}: {} (recorded as error doc)",
                preflight_path,
                error
            );
            return Err(error);
        }
    };

    if check_cancelled(state, app, total, done, current, errors) {
        return Ok(false);
    }
    emit_progress(
        state,
        app,
        total,
        done,
        current,
        IndexStatus::Running,
        errors.to_vec(),
        Some(IndexStage::Embedding),
    );

    let existing = existing_for_id;
    if let Some(existing_doc) = &existing {
        parsed.doc_id = existing_doc.id.clone();
    }

    if existing
        .as_ref()
        .map(|doc| doc.hash == parsed.metadata.hash && doc.status == "indexed")
        .unwrap_or(false)
    {
        tracing::info!("skipping unchanged file {}", parsed.path);
        return Ok(true);
    }

    let doc = db::DocumentRecord {
        id: parsed.doc_id.clone(),
        path: parsed.path.clone(),
        filename: parsed.metadata.filename.clone(),
        format: parsed.format.clone(),
        size_bytes: parsed.metadata.size_bytes,
        hash: parsed.metadata.hash.clone(),
        indexed_at: Utc::now().to_rfc3339(),
        status: "pending".to_string(),
        error_msg: None,
        doc_class: parsed.doc_class,
    };

    let chunks_for_doc = sliding::chunk_document(&parsed);
    let old_chunk_ids = if existing.is_some() {
        let db = state.db.lock().await;
        chunks::get_doc_chunks(&db, &parsed.doc_id)?
            .into_iter()
            .map(|chunk| chunk.id)
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    let texts: Vec<String> = chunks_for_doc
        .iter()
        .map(|chunk| chunk.content.clone())
        .collect();
    let embeddings = if texts.is_empty() {
        Vec::new()
    } else {
        let mut embedder = state.embedder.lock().await;
        match local::embed_batch_with_retry(&mut embedder, &texts) {
            Ok(vectors) => vectors,
            Err(error) => {
                let error_msg = format!("fastembed failed: {}", error);
                tracing::error!(
                    "failed to embed {}; marking document error: {}",
                    parsed.path,
                    error_msg
                );
                mark_document_embedding_error(state, &doc, &old_chunk_ids, &error_msg).await?;
                return Err(EngineError::Embed(error_msg));
            }
        }
    };
    if embeddings.len() != chunks_for_doc.len() {
        let error_msg = format!(
            "embedding count mismatch: got {}, expected {}",
            embeddings.len(),
            chunks_for_doc.len()
        );
        tracing::error!(
            "failed to embed {}; marking document error: {}",
            parsed.path,
            error_msg
        );
        mark_document_embedding_error(state, &doc, &old_chunk_ids, &error_msg).await?;
        return Err(EngineError::Embed(error_msg));
    }
    let entity_hits = entities::extract_from_chunks(&chunks_for_doc);

    if check_cancelled(state, app, total, done, current, errors) {
        return Ok(false);
    }
    emit_progress(
        state,
        app,
        total,
        done,
        current,
        IndexStatus::Running,
        errors.to_vec(),
        Some(IndexStage::Writing),
    );

    // Snapshot existing vectors from other docs (for cross-doc edges) BEFORE
    // we write the new doc's chunks/vectors. Done under one db lock.
    let mut all_edges = Vec::new();
    {
        let mut db = state.db.lock().await;

        let existing_vectors = vectors::vectors_excluding_doc(&db, &parsed.doc_id)?;

        db::upsert_document(&db, &doc)?;
        chunks::replace_doc_chunks(&mut db, &parsed.doc_id, &chunks_for_doc)?;

        let vector_items: Vec<(&str, &[f32])> = chunks_for_doc
            .iter()
            .zip(embeddings.iter())
            .map(|(chunk, embedding)| (chunk.id.as_str(), embedding.as_slice()))
            .collect();
        vectors::upsert_vectors_batch(&mut db, &vector_items)?;

        // Persist entities; then derive shared_entity edges from the DB.
        entity_store::insert_entities(&mut db, &entity_hits)?;

        let semantic_edges = builder::build_edges(&chunks_for_doc, &embeddings, &existing_vectors);
        let shared_anchor_edges = entity_store::build_shared_anchor_edges(&db, &parsed.doc_id)?;
        let shared_entity_edges = entity_store::build_shared_entity_edges(&db, &parsed.doc_id)?;
        let manifest_overlap_edges =
            entity_store::build_manifest_overlap_edges(&db, &parsed.doc_id)?;

        all_edges.extend(semantic_edges);
        all_edges.extend(shared_anchor_edges);
        all_edges.extend(shared_entity_edges);
        all_edges.extend(manifest_overlap_edges);

        graph_store::upsert_edges(&mut db, &all_edges)?;
    }

    emit_progress(
        state,
        app,
        total,
        done,
        current,
        IndexStatus::Running,
        errors.to_vec(),
        Some(IndexStage::Linking),
    );

    {
        let fts = state.fts.lock().await;
        fts::delete_chunks(&fts, &old_chunk_ids)
            .map_err(|error| EngineError::Embed(error.to_string()))?;
        fts::replace_chunks(&fts, &chunks_for_doc)
            .map_err(|error| EngineError::Embed(error.to_string()))?;
    }

    {
        let db = state.db.lock().await;
        db::mark_document_status(&db, &parsed.doc_id, "indexed", None)?;
    }

    tracing::info!(
        "indexed {} ({} chunks, {} edges, {} entities)",
        parsed.path,
        chunks_for_doc.len(),
        all_edges.len(),
        entity_hits.len(),
    );
    Ok(true)
}

fn check_cancelled(
    state: &AppState,
    app: &Option<AppHandle>,
    total: usize,
    done: usize,
    current: &str,
    errors: &[String],
) -> bool {
    if state.cancel_token.load(Ordering::Relaxed) {
        emit_progress(
            state,
            app,
            total,
            done,
            current,
            IndexStatus::Cancelled,
            errors.to_vec(),
            None,
        );
        true
    } else {
        false
    }
}

async fn mark_document_embedding_error(
    state: &AppState,
    doc: &db::DocumentRecord,
    old_chunk_ids: &[String],
    error_msg: &str,
) -> Result<(), EngineError> {
    {
        let fts = state.fts.lock().await;
        fts::delete_chunks(&fts, old_chunk_ids)?;
    }

    let mut error_doc = doc.clone();
    error_doc.status = "error".to_string();
    error_doc.error_msg = Some(error_msg.to_string());

    let mut db = state.db.lock().await;
    db::upsert_document(&db, &error_doc)?;
    chunks::replace_doc_chunks(&mut db, &doc.id, &[])?;
    Ok(())
}

fn collect_supported_files(root: &Path) -> Vec<PathBuf> {
    WalkDir::new(root)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
        .map(|entry| entry.into_path())
        .filter(|path| is_supported(path) && !is_engine_output(path))
        .collect()
}

/// Skip files we wrote ourselves (transcript sidecars + extracted audio).
/// We mark them with a `.anubis.` infix so we can recognise them next time
/// the user re-indexes the folder — otherwise the extracted WAV would be
/// re-fed to ffmpeg in a loop ("Output same as Input #0").
fn is_engine_output(path: &Path) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .map(|name| name.to_ascii_lowercase().contains(".anubis."))
        .unwrap_or(false)
}

fn is_supported(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|extension| extension.to_str())
            .map(|extension| extension.to_ascii_lowercase())
            .as_deref(),
        Some(
            "md" | "txt"
                | "pdf"
                | "docx"
                | "xlsx"
                | "csv"
                | "json"
                | "png"
                | "jpg"
                | "jpeg"
                | "webp"
                | "tiff"
                | "mp4"
                | "mov"
                | "avi"
                | "mkv"
                | "webm"
                | "wmv"
                | "mp3"
                | "wav"
                | "m4a"
                | "flac"
                | "ogg"
                | "opus"
        )
    )
}

fn emit_progress(
    state: &AppState,
    app: &Option<AppHandle>,
    total: usize,
    done: usize,
    current: &str,
    status: IndexStatus,
    errors: Vec<String>,
    stage: Option<IndexStage>,
) {
    if let Some(app) = app {
        if let Err(error) = app.emit(
            "index-progress",
            IndexProgress {
                total,
                done,
                current: current.to_string(),
                status,
                errors,
                stage,
                workdir_id: state.workdir_id_str(),
            },
        ) {
            tracing::warn!("failed to emit index progress: {}", error);
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn placeholder_compiles() {
        assert_eq!(2 + 2, 4);
    }
}
