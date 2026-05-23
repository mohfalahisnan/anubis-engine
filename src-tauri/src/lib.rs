pub mod chunker;
pub mod commands;
pub mod embedder;
pub mod engine;
pub mod entities;
pub mod graph;
pub mod mcp;
pub mod ocr;
pub mod parser;
pub mod query;
pub mod store;
pub mod transcription;
pub mod types;

use tauri::Manager;

use crate::engine::events;
use crate::engine::state::new_engine_handle;

#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error("Database error: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("Parse error for {path}: {msg}")]
    Parse { path: String, msg: String },
    #[error("Embed error: {0}")]
    Embed(String),
    #[error("OCR error: {0}")]
    Ocr(String),
    #[error("Transcription error: {0}")]
    Transcribe(String),
    /// Soft signal — file has no audio stream so there's nothing to
    /// transcribe. Callers can treat this as "indexed with empty text"
    /// rather than a hard failure.
    #[error("No audio track in {0}")]
    NoAudioTrack(String),
    #[error("Index already running")]
    AlreadyIndexing,
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Workdir error: {0}")]
    Workdir(#[from] crate::engine::workdir::WorkdirError),
}

pub fn run() {
    if let Err(error) = try_run() {
        tracing::error!("failed to run Anubis OS: {}", error);
        std::process::exit(1);
    }
}

fn try_run() -> tauri::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let app_data = app.path().app_data_dir()?;
            std::fs::create_dir_all(&app_data)?;
            let workdirs_root = app_data.join("workdirs");
            std::fs::create_dir_all(&workdirs_root)?;

            // Register the AppHandle BEFORE spawning init so model-download
            // events are wired up before the first download tick.
            events::set_app_handle(app.handle().clone());

            // Hand the frontend an empty handle right away so the window can
            // paint and listen for setup events. The heavy bits (fastembed
            // model download) run on a worker thread; per-workdir AppStates
            // are then constructed lazily on first use.
            let engine = new_engine_handle();
            app.manage(engine.clone());

            let models_dir = app_data.clone();
            std::thread::spawn(move || {
                events::emit_starting("engine", "Engine bootstrap");
                let embedder = match crate::engine::state::bootstrap_shared_engines(&models_dir) {
                    Ok(handle) => handle,
                    Err(error) => {
                        tracing::error!("engine bootstrap failed: {error}");
                        events::emit_error("engine", "Engine bootstrap", error.to_string());
                        return;
                    }
                };
                let registry = std::sync::Arc::new(
                    crate::engine::registry::WorkdirRegistry::new(workdirs_root, embedder),
                );
                if engine.set(registry).is_err() {
                    tracing::warn!("engine handle already initialised");
                }
                events::emit_ready("engine", "Engine bootstrap");
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::index_commands::index_folder,
            commands::index_commands::index_file,
            commands::index_commands::cancel_indexing,
            commands::index_commands::remove_document,
            commands::index_commands::reset_index,
            commands::query_commands::query,
            commands::query_commands::get_chunk_neighbors,
            commands::query_commands::get_graph_overview,
            commands::query_commands::get_graph_neighborhood,
            commands::query_commands::get_search_neighborhood,
            commands::query_commands::get_doc_chunks,
            commands::status_commands::get_index_stats,
            commands::status_commands::list_documents,
            commands::status_commands::engine_ready,
            commands::status_commands::get_settings,
            commands::status_commands::set_transcription_enabled,
        ])
        .run(tauri::generate_context!())
}
