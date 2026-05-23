pub mod index_commands;
pub mod query_commands;
pub mod status_commands;
pub mod workdir_commands;

use std::sync::Arc;

use tauri::State;

use crate::engine::registry::WorkdirRegistry;
use crate::engine::state::{AppState, EngineHandle};
use crate::engine::workdir::WorkdirId;

/// Get the workdir registry from the Tauri-managed handle, or return a
/// user-facing error if first-run setup is still in flight.
pub fn registry_or_error<'a>(
    state: &'a State<'_, EngineHandle>,
) -> Result<&'a Arc<WorkdirRegistry>, String> {
    state
        .get()
        .ok_or_else(|| "Engine still initialising — please wait for setup to finish.".to_string())
}

/// Resolve `workdir` into a cached or lazily-built [`AppState`]. Returns
/// the `WorkdirId` alongside the state so commands can include it in event
/// payloads.
pub async fn workdir_state(
    state: &State<'_, EngineHandle>,
    workdir: &str,
) -> Result<(WorkdirId, Arc<AppState>), String> {
    let registry = registry_or_error(state)?;
    registry
        .get_or_load(workdir)
        .await
        .map_err(|error| error.to_string())
}
