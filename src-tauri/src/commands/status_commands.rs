use tauri::State;

use crate::{engine::state::AppState, store::db};

#[tauri::command]
pub async fn get_index_stats(
    state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    let db = state.db.lock().await;
    db::get_index_stats(&db).map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn list_documents(
    state: State<'_, AppState>,
) -> Result<Vec<serde_json::Value>, String> {
    let db = state.db.lock().await;
    db::list_documents(&db).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    #[test]
    fn placeholder_compiles() {
        assert_eq!(2 + 2, 4);
    }
}
