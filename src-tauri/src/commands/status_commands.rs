use serde_json::json;
use tauri::State;

use crate::{
    commands::engine_or_error,
    engine::{settings as engine_settings, state::EngineHandle},
    store::db,
};

#[tauri::command]
pub async fn get_index_stats(state: State<'_, EngineHandle>) -> Result<serde_json::Value, String> {
    let engine = engine_or_error(&state)?;
    let db = engine.db.lock().await;
    db::get_index_stats(&db).map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn list_documents(
    state: State<'_, EngineHandle>,
) -> Result<Vec<serde_json::Value>, String> {
    let engine = engine_or_error(&state)?;
    let db = engine.db.lock().await;
    db::list_documents(&db).map_err(|error| error.to_string())
}

/// Whether the engine has finished its (possibly slow) first-run setup —
/// model downloads, DB migration, FTS reconcile. Frontend polls this after
/// the splash screen disappears or uses the `model-download` events to know
/// the same thing.
#[tauri::command]
pub async fn engine_ready(state: State<'_, EngineHandle>) -> Result<bool, String> {
    Ok(state.get().is_some())
}

/// Returns the user-tunable runtime settings the UI exposes. Today that's
/// just the transcription toggle, but the shape leaves room for more
/// switches (per-source crawl options, OCR enable/disable, etc.) without
/// having to add a new command for each one.
#[tauri::command]
pub async fn get_settings(_state: State<'_, EngineHandle>) -> Result<serde_json::Value, String> {
    Ok(json!({
        "transcription_enabled": engine_settings::transcription_enabled(),
    }))
}

#[tauri::command]
pub async fn set_transcription_enabled(
    enabled: bool,
    state: State<'_, EngineHandle>,
) -> Result<bool, String> {
    let engine = engine_or_error(&state)?;
    let db = engine.db.lock().await;
    engine_settings::persist(&db, enabled).map_err(|error| error.to_string())?;
    Ok(enabled)
}

#[cfg(test)]
mod tests {
    #[test]
    fn placeholder_compiles() {
        assert_eq!(2 + 2, 4);
    }
}
