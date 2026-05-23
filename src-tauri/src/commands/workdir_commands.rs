use tauri::State;

use crate::commands::registry_or_error;
use crate::engine::state::EngineHandle;
use crate::engine::workdir::WorkdirInfo;
use crate::store::db;

#[tauri::command]
pub async fn list_workdirs(
    state: State<'_, EngineHandle>,
) -> Result<Vec<WorkdirInfo>, String> {
    let registry = registry_or_error(&state)?;
    let mut infos = registry.list().map_err(|error| error.to_string())?;

    // Best-effort doc_count by opening already-loaded states. We avoid
    // forcing a lazy load just for counts — unloaded workdirs return None
    // and the UI can show a "—" placeholder until the user clicks in.
    let loaded = registry.loaded_states().await;
    for info in infos.iter_mut() {
        let cached = loaded.iter().find_map(|(id, state)| {
            if id.as_str() == info.id {
                Some(state.clone())
            } else {
                None
            }
        });
        if let Some(state_arc) = cached {
            let db = state_arc.db.lock().await;
            info.doc_count = db::list_documents(&db).ok().map(|docs| docs.len() as i64);
        }
    }
    Ok(infos)
}

#[tauri::command]
pub async fn delete_workdir(
    workdir: String,
    state: State<'_, EngineHandle>,
) -> Result<(), String> {
    let registry = registry_or_error(&state)?;
    registry
        .delete(&workdir)
        .await
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    #[test]
    fn placeholder_compiles() {
        assert_eq!(2 + 2, 4);
    }
}
