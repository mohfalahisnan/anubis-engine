use serde_json::json;
use tauri::State;

use crate::{
    commands::{registry_or_error, workdir_state},
    engine::{settings as engine_settings, state::EngineHandle},
    store::db,
};

#[tauri::command]
pub async fn get_index_stats(
    workdir: String,
    state: State<'_, EngineHandle>,
) -> Result<serde_json::Value, String> {
    let (_workdir_id, engine) = workdir_state(&state, &workdir).await?;
    let db = engine.db.lock().await;
    db::get_index_stats(&db).map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn list_documents(
    workdir: String,
    state: State<'_, EngineHandle>,
) -> Result<Vec<serde_json::Value>, String> {
    let (_workdir_id, engine) = workdir_state(&state, &workdir).await?;
    let db = engine.db.lock().await;
    db::list_documents(&db).map_err(|error| error.to_string())
}

/// Whether the engine has finished its first-run setup (embedder download
/// + registry construction). Workdir-agnostic — the registry is built once
/// per process; individual workdirs are loaded lazily on first use.
#[tauri::command]
pub async fn engine_ready(state: State<'_, EngineHandle>) -> Result<bool, String> {
    Ok(registry_or_error(&state).is_ok())
}

#[tauri::command]
pub async fn get_settings(_state: State<'_, EngineHandle>) -> Result<serde_json::Value, String> {
    Ok(json!({
        "transcription_enabled": engine_settings::transcription_enabled(),
    }))
}

/// Toggle transcription. Persisted into every currently-loaded workdir's
/// DB (the setting is a process-global, not per-workdir, switch — we mirror
/// it to every DB so a future reload picks it up regardless of which workdir
/// is opened first). Also updates the in-memory flag immediately.
#[tauri::command]
pub async fn set_transcription_enabled(
    enabled: bool,
    state: State<'_, EngineHandle>,
) -> Result<bool, String> {
    let registry = registry_or_error(&state)?;
    engine_settings::set_transcription_enabled(enabled);
    let loaded = {
        let states = registry.loaded_states().await;
        states.values().cloned().collect::<Vec<_>>()
    };
    for engine in loaded {
        let db = engine.db.lock().await;
        engine_settings::persist(&db, enabled).map_err(|error| error.to_string())?;
    }
    Ok(enabled)
}

#[cfg(test)]
mod tests {
    #[test]
    fn placeholder_compiles() {
        assert_eq!(2 + 2, 4);
    }
}
