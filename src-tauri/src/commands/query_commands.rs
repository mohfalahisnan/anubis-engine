use tauri::State;

use crate::{
    commands::workdir_state,
    embedder::local,
    engine::state::EngineHandle,
    query::hybrid::{run_query, QueryOpts},
    store::{chunks, graph_store, graph_store::GraphNeighbor, graph_store::GraphOverview},
    types::{Chunk, QueryResult},
};

#[tauri::command]
pub async fn query(
    workdir: String,
    q: String,
    limit: Option<usize>,
    depth: Option<usize>,
    state: State<'_, EngineHandle>,
) -> Result<Vec<QueryResult>, String> {
    let (_workdir_id, engine) = workdir_state(&state, &workdir).await?;
    let limit = limit.unwrap_or(10);
    let depth = depth.unwrap_or(1).min(3);

    let query_embedding = {
        let mut embedder = engine.embedder.lock().await;
        local::embed_query(&mut embedder, &q).map_err(|e| e.to_string())?
    };

    let db = engine.db.lock().await;
    let fts = engine.fts.lock().await;
    run_query(&db, &fts, &q, &query_embedding, QueryOpts { limit, depth })
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn get_chunk_neighbors(
    workdir: String,
    chunk_id: String,
    depth: Option<usize>,
    state: State<'_, EngineHandle>,
) -> Result<Vec<GraphNeighbor>, String> {
    let (_workdir_id, engine) = workdir_state(&state, &workdir).await?;
    let db = engine.db.lock().await;
    graph_store::chunk_neighbors(&db, &chunk_id, depth.unwrap_or(1) * 50)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn get_graph_overview(
    workdir: String,
    limit: Option<usize>,
    state: State<'_, EngineHandle>,
) -> Result<GraphOverview, String> {
    let (_workdir_id, engine) = workdir_state(&state, &workdir).await?;
    let db = engine.db.lock().await;
    graph_store::graph_overview(&db, limit.unwrap_or(250)).map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn get_graph_neighborhood(
    workdir: String,
    chunk_id: String,
    depth: Option<usize>,
    limit: Option<usize>,
    state: State<'_, EngineHandle>,
) -> Result<GraphOverview, String> {
    let (_workdir_id, engine) = workdir_state(&state, &workdir).await?;
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
    workdir: String,
    chunk_ids: Vec<String>,
    depth: Option<usize>,
    limit: Option<usize>,
    state: State<'_, EngineHandle>,
) -> Result<GraphOverview, String> {
    let (_workdir_id, engine) = workdir_state(&state, &workdir).await?;
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
    workdir: String,
    doc_id: String,
    state: State<'_, EngineHandle>,
) -> Result<Vec<Chunk>, String> {
    let (_workdir_id, engine) = workdir_state(&state, &workdir).await?;
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
