use tauri::State;

use crate::commands::{registry_or_error, workdir_state};
use crate::engine::state::EngineHandle;
use crate::engine::workdir::{self, WorkdirInfo};
use crate::store::db;

/// Resolve a folder path into a workdir, creating its storage directory
/// on disk and returning canonical path metadata. The frontend calls this
/// right after the user picks a folder so all subsequent invocations use
/// the canonical form (e.g. `\\?\D:\foo` on Windows) rather than the
/// raw OS dialog string — keeping `localStorage`, `activeWorkdir`, and
/// `list_workdirs` consistent.
#[tauri::command]
pub async fn register_workdir(
    workdir: String,
    state: State<'_, EngineHandle>,
) -> Result<WorkdirInfo, String> {
    let (id, _) = workdir_state(&state, &workdir).await?;
    let registry = registry_or_error(&state)?;
    let infos = registry.list().map_err(|error| error.to_string())?;
    infos
        .into_iter()
        .find(|info| info.id == id.as_str())
        .ok_or_else(|| "workdir registered but meta.json missing".to_string())
}

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

/// Ensure `path` is the same folder as the workdir or a descendant of it.
/// Both inputs are canonicalised; returns a user-facing error string if
/// `path` is outside the workdir or cannot be resolved.
pub fn ensure_path_inside_workdir(workdir: &str, path: &str) -> Result<(), String> {
    let (workdir_canonical, _) = workdir::resolve(workdir).map_err(|error| error.to_string())?;
    let raw = std::path::Path::new(path);
    if !raw.exists() {
        return Err(format!("Path not found: {path}"));
    }
    let path_canonical = std::fs::canonicalize(raw)
        .map_err(|error| format!("Cannot canonicalise path {path}: {error}"))?;
    if !workdir::is_inside(&path_canonical, &workdir_canonical) {
        return Err(format!(
            "Path {} is outside the active workdir {}",
            path_canonical.display(),
            workdir_canonical.display()
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn placeholder_compiles() {
        assert_eq!(2 + 2, 4);
    }
}
