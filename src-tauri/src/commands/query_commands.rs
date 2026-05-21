use tauri::State;

use crate::{
    embedder::local,
    engine::state::AppState,
    store::{chunks, graph_store},
    types::{Chunk, QueryResult},
};

#[tauri::command]
pub async fn query(
    q: String,
    limit: Option<usize>,
    state: State<'_, AppState>,
) -> Result<Vec<QueryResult>, String> {
    let embedding = local::deterministic_embedding(&q);
    let db = state.db.lock().await;
    crate::query::hybrid::query_with_embedding(&db, &embedding, limit.unwrap_or(10))
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn get_chunk_neighbors(
    chunk_id: String,
    depth: Option<usize>,
    state: State<'_, AppState>,
) -> Result<Vec<QueryResult>, String> {
    let db = state.db.lock().await;
    graph_store::chunk_neighbors(&db, &chunk_id, depth.unwrap_or(1) * 50)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn get_doc_chunks(
    doc_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<Chunk>, String> {
    let db = state.db.lock().await;
    chunks::get_doc_chunks(&db, &doc_id).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    #[test]
    fn placeholder_compiles() {
        assert_eq!(2 + 2, 4);
    }
}
