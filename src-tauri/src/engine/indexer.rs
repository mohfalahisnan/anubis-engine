use std::path::{Path, PathBuf};

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
        chunks, db, entities as entity_store,
        graph_store::{self},
        vectors,
    },
    types::{IndexProgress, IndexStatus},
    EngineError,
};

use super::state::AppState;

pub async fn index_folder(
    path: &str,
    state: &AppState,
    app: Option<AppHandle>,
) -> Result<(), EngineError> {
    let mut indexing = state.indexing.lock().await;
    if *indexing {
        return Err(EngineError::AlreadyIndexing);
    }
    *indexing = true;
    drop(indexing);

    let result = index_folder_inner(path, state, app).await;

    let mut indexing = state.indexing.lock().await;
    *indexing = false;
    result
}

pub async fn index_file(path: &str, state: &AppState) -> Result<(), EngineError> {
    index_paths(&[PathBuf::from(path)], state, None).await
}

async fn index_folder_inner(
    path: &str,
    state: &AppState,
    app: Option<AppHandle>,
) -> Result<(), EngineError> {
    let paths = collect_supported_files(Path::new(path));
    emit_progress(&app, paths.len(), 0, "", IndexStatus::Running, Vec::new());
    index_paths(&paths, state, app.clone()).await?;
    emit_progress(
        &app,
        paths.len(),
        paths.len(),
        "",
        IndexStatus::Done,
        Vec::new(),
    );
    Ok(())
}

async fn index_paths(
    paths: &[PathBuf],
    state: &AppState,
    app: Option<AppHandle>,
) -> Result<(), EngineError> {
    let mut errors = Vec::new();

    for (index, path) in paths.iter().enumerate() {
        let current = path
            .file_name()
            .and_then(|name| name.to_str())
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| path.to_string_lossy().into_owned());
        emit_progress(
            &app,
            paths.len(),
            index,
            &current,
            IndexStatus::Running,
            errors.clone(),
        );

        if let Err(error) = index_one(path, state).await {
            tracing::error!("failed to index {}: {}", path.display(), error);
            errors.push(format!("{}: {}", path.display(), error));
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        emit_progress(
            &app,
            paths.len(),
            paths.len(),
            "",
            IndexStatus::Error,
            errors,
        );
        Ok(())
    }
}

async fn index_one(path: &Path, state: &AppState) -> Result<(), EngineError> {
    let parsed = parser::parse(path)?;
    let existing = {
        let db = state.db.lock().await;
        db::get_document_by_path(&db, &parsed.path)?
    };

    if existing
        .as_ref()
        .map(|doc| doc.hash == parsed.metadata.hash && doc.status == "indexed")
        .unwrap_or(false)
    {
        tracing::info!("skipping unchanged file {}", parsed.path);
        return Ok(());
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
    };

    let chunks_for_doc = sliding::chunk_document(&parsed);
    let texts: Vec<String> = chunks_for_doc
        .iter()
        .map(|chunk| chunk.content.clone())
        .collect();
    // Real dense embeddings (fastembed AllMiniLML6V2) — same space the query
    // path uses. Falls back to the deterministic embedder ONLY on hard errors
    // so a single broken file doesn't poison the entire index.
    let embeddings = {
        let mut embedder = state.embedder.lock().await;
        match local::embed_batch(&mut embedder, &texts) {
            Ok(vectors) => vectors,
            Err(error) => {
                tracing::warn!(
                    "fastembed failed on {}: {} — falling back to deterministic",
                    parsed.path,
                    error
                );
                local::deterministic_embed_batch(&texts)
            }
        }
    };
    let entity_hits = entities::extract_from_chunks(&chunks_for_doc);

    // Snapshot existing vectors from other docs (for cross-doc edges) BEFORE
    // we write the new doc's chunks/vectors. Done under one db lock.
    let mut all_edges = Vec::new();
    {
        let mut db = state.db.lock().await;

        let existing_vectors = vectors::vectors_excluding_doc(&db, &parsed.doc_id)?;

        db::upsert_document(&db, &doc)?;
        chunks::replace_doc_chunks(&mut db, &parsed.doc_id, &chunks_for_doc)?;
        for (chunk, embedding) in chunks_for_doc.iter().zip(embeddings.iter()) {
            vectors::upsert_vector(&db, &chunk.id, embedding)?;
        }

        // Persist entities; then derive shared_entity edges from the DB.
        entity_store::insert_entities(&db, &entity_hits)?;

        let semantic_edges = builder::build_edges(&chunks_for_doc, &embeddings, &existing_vectors);
        let shared_edges = entity_store::build_shared_entity_edges(&db, &parsed.doc_id)?;

        all_edges.extend(semantic_edges);
        all_edges.extend(shared_edges);

        graph_store::upsert_edges(&mut db, &all_edges)?;
        db::mark_document_status(&db, &parsed.doc_id, "indexed", None)?;
    }

    {
        let fts = state.fts.lock().await;
        crate::store::fts::replace_chunks(&fts, &chunks_for_doc)
            .map_err(|error| EngineError::Embed(error.to_string()))?;
    }

    tracing::info!(
        "indexed {} ({} chunks, {} edges, {} entities)",
        parsed.path,
        chunks_for_doc.len(),
        all_edges.len(),
        entity_hits.len(),
    );
    Ok(())
}

fn collect_supported_files(root: &Path) -> Vec<PathBuf> {
    WalkDir::new(root)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
        .map(|entry| entry.into_path())
        .filter(|path| is_supported(path))
        .collect()
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
                | "png"
                | "jpg"
                | "jpeg"
                | "webp"
                | "tiff"
                | "mp4"
                | "mov"
                | "avi"
        )
    )
}

fn emit_progress(
    app: &Option<AppHandle>,
    total: usize,
    done: usize,
    current: &str,
    status: IndexStatus,
    errors: Vec<String>,
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
