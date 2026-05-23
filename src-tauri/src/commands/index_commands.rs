use std::sync::atomic::Ordering;

use tauri::{AppHandle, State};

use crate::commands::workdir_commands::ensure_path_inside_workdir;
use crate::commands::workdir_state;
use crate::engine::state::EngineHandle;
use crate::store::{chunks, db, fts};

#[tauri::command]
pub async fn index_folder(
    workdir: String,
    path: String,
    state: State<'_, EngineHandle>,
    app: AppHandle,
) -> Result<(), String> {
    ensure_path_inside_workdir(&workdir, &path)?;
    let (_workdir_id, engine) = workdir_state(&state, &workdir).await?;
    crate::engine::indexer::index_folder(&path, &engine, Some(app))
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn index_file(
    workdir: String,
    path: String,
    state: State<'_, EngineHandle>,
) -> Result<(), String> {
    ensure_path_inside_workdir(&workdir, &path)?;
    let (_workdir_id, engine) = workdir_state(&state, &workdir).await?;
    crate::engine::indexer::index_file(&path, &engine)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn cancel_indexing(
    workdir: String,
    state: State<'_, EngineHandle>,
) -> Result<(), String> {
    let (_workdir_id, engine) = workdir_state(&state, &workdir).await?;
    engine.cancel_token.store(true, Ordering::Relaxed);
    Ok(())
}

#[tauri::command]
pub async fn remove_document(
    workdir: String,
    doc_id: String,
    state: State<'_, EngineHandle>,
) -> Result<(), String> {
    let (_workdir_id, engine) = workdir_state(&state, &workdir).await?;
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
pub async fn reset_index(
    workdir: String,
    state: State<'_, EngineHandle>,
) -> Result<(), String> {
    let (_workdir_id, engine) = workdir_state(&state, &workdir).await?;
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
