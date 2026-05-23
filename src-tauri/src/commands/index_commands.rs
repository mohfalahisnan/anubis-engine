use std::sync::atomic::Ordering;
use tauri::{AppHandle, State};

use crate::commands::engine_or_error;
use crate::engine::state::EngineHandle;
use crate::store::{chunks, db, fts};

#[tauri::command]
pub async fn index_folder(
    path: String,
    state: State<'_, EngineHandle>,
    app: AppHandle,
) -> Result<(), String> {
    let engine = engine_or_error(&state)?;
    crate::engine::indexer::index_folder(&path, engine, Some(app))
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn index_file(path: String, state: State<'_, EngineHandle>) -> Result<(), String> {
    let engine = engine_or_error(&state)?;
    crate::engine::indexer::index_file(&path, engine)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn cancel_indexing(state: State<'_, EngineHandle>) -> Result<(), String> {
    let engine = engine_or_error(&state)?;
    engine.cancel_token.store(true, Ordering::Relaxed);
    Ok(())
}

#[tauri::command]
pub async fn remove_document(doc_id: String, state: State<'_, EngineHandle>) -> Result<(), String> {
    let engine = engine_or_error(&state)?;
    let chunk_ids = {
        let db = engine.db.lock().await;
        chunks::get_doc_chunks(&db, &doc_id)
            .map_err(|error| error.to_string())?
            .into_iter()
            .map(|chunk| chunk.id)
            .collect::<Vec<_>>()
    };

    {
        let fts = engine.fts.lock().await;
        fts::delete_chunks(&fts, &chunk_ids).map_err(|error| error.to_string())?;
    }

    {
        let db = engine.db.lock().await;
        db::delete_document(&db, &doc_id).map_err(|error| error.to_string())
    }
}

#[tauri::command]
pub async fn reset_index(state: State<'_, EngineHandle>) -> Result<(), String> {
    let engine = engine_or_error(&state)?;
    {
        let fts = engine.fts.lock().await;
        fts::clear(&fts).map_err(|error| error.to_string())?;
    }

    {
        let db = engine.db.lock().await;
        db::reset_index(&db).map_err(|error| error.to_string())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn placeholder_compiles() {
        assert_eq!(2 + 2, 4);
    }
}
