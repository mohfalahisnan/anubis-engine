pub mod index_commands;
pub mod query_commands;
pub mod status_commands;

use tauri::State;

use crate::engine::state::{AppState, EngineHandle};

/// Resolve the active [`AppState`] from the Tauri-managed handle. Returns a
/// user-facing error string while first-run setup is still in flight (model
/// downloads, etc.) so the UI can show a friendly message instead of a Rust
/// panic message.
pub fn engine_or_error<'a>(state: &'a State<'_, EngineHandle>) -> Result<&'a AppState, String> {
    state
        .get()
        .ok_or_else(|| "Engine still initialising — please wait for setup to finish.".to_string())
}
