use tauri::State;

use crate::{
    commands::engine_or_error,
    embedder::local,
    engine::state::EngineHandle,
    query::hybrid::{run_query, QueryOpts},
    store::{chunks, graph_store, graph_store::GraphOverview},
    types::{Chunk, QueryResult},
};

#[tauri::command]
pub async fn query(
    q: String,
    limit: Option<usize>,
    depth: Option<usize>,
    state: State<'_, EngineHandle>,
) -> Result<Vec<QueryResult>, String> {
    let engine = engine_or_error(&state)?;
    let limit = limit.unwrap_or(10);
    let depth = depth.unwrap_or(1).min(3);

    // Real fastembed dense embedding for the query — same model as indexing.
    let query_embedding = {
        let mut embedder = engine.embedder.lock().await;
        local::embed_query(&mut embedder, &q).map_err(|e| e.to_string())?
    };

    let db = engine.db.lock().await;
    let fts = engine.fts.lock().await;
    run_query(
        &db,
        &fts,
        &q,
        &query_embedding,
        QueryOpts { limit, depth },
    )
    .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn get_chunk_neighbors(
    chunk_id: String,
    depth: Option<usize>,
    state: State<'_, EngineHandle>,
) -> Result<Vec<QueryResult>, String> {
    let engine = engine_or_error(&state)?;
    let db = engine.db.lock().await;
    graph_store::chunk_neighbors(&db, &chunk_id, depth.unwrap_or(1) * 50)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn get_graph_overview(
    limit: Option<usize>,
    state: State<'_, EngineHandle>,
) -> Result<GraphOverview, String> {
    let engine = engine_or_error(&state)?;
    let db = engine.db.lock().await;
    graph_store::graph_overview(&db, limit.unwrap_or(250)).map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn get_graph_neighborhood(
    chunk_id: String,
    depth: Option<usize>,
    limit: Option<usize>,
    state: State<'_, EngineHandle>,
) -> Result<GraphOverview, String> {
    let engine = engine_or_error(&state)?;
    let db = engine.db.lock().await;
    graph_store::graph_neighborhood(
        &db,
        &chunk_id,
        depth.unwrap_or(2).min(3),
        limit.unwrap_or(160),
    )
    .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn get_search_neighborhood(
    chunk_ids: Vec<String>,
    depth: Option<usize>,
    limit: Option<usize>,
    state: State<'_, EngineHandle>,
) -> Result<GraphOverview, String> {
    let engine = engine_or_error(&state)?;
    let db = engine.db.lock().await;
    graph_store::graph_search_neighborhood(
        &db,
        &chunk_ids,
        depth.unwrap_or(1).min(3),
        limit.unwrap_or(200),
    )
    .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn get_doc_chunks(
    doc_id: String,
    state: State<'_, EngineHandle>,
) -> Result<Vec<Chunk>, String> {
    let engine = engine_or_error(&state)?;
    let db = engine.db.lock().await;
    chunks::get_doc_chunks(&db, &doc_id).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    #[test]
    fn placeholder_compiles() {
        assert_eq!(2 + 2, 4);
    }
}
