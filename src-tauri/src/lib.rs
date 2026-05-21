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
pub mod types;

use tauri::Manager;

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
    #[error("Index already running")]
    AlreadyIndexing,
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
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
            let db_path = app_data.join("anubis.db");
            let fts_path = app_data.join("fts_index");
            let state = engine::state::AppState::new(&db_path, &fts_path)
                .map_err(|error| Box::<dyn std::error::Error>::from(error.to_string()))?;
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::index_commands::index_folder,
            commands::index_commands::index_file,
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
        ])
        .run(tauri::generate_context!())
}
