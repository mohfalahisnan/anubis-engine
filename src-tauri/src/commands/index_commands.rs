use tauri::{AppHandle, State};

use crate::engine::state::AppState;
use crate::store::{chunks, db, fts};

#[tauri::command]
pub async fn index_folder(
    path: String,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<(), String> {
    crate::engine::indexer::index_folder(&path, state.inner(), Some(app))
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn index_file(path: String, state: State<'_, AppState>) -> Result<(), String> {
    crate::engine::indexer::index_file(&path, state.inner())
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn remove_document(doc_id: String, state: State<'_, AppState>) -> Result<(), String> {
    let chunk_ids = {
        let db = state.db.lock().await;
        chunks::get_doc_chunks(&db, &doc_id)
            .map_err(|error| error.to_string())?
            .into_iter()
            .map(|chunk| chunk.id)
            .collect::<Vec<_>>()
    };

    {
        let db = state.db.lock().await;
        db::delete_document(&db, &doc_id).map_err(|error| error.to_string())?;
    }

    let fts = state.fts.lock().await;
    fts::delete_chunks(&fts, &chunk_ids).map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn reset_index(state: State<'_, AppState>) -> Result<(), String> {
    {
        let db = state.db.lock().await;
        db::reset_index(&db).map_err(|error| error.to_string())?;
    }

    let fts = state.fts.lock().await;
    fts::clear(&fts).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    #[test]
    fn placeholder_compiles() {
        assert_eq!(2 + 2, 4);
    }
}
