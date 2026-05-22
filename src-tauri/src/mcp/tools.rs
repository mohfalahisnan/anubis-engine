use serde::Serialize;
use serde_json::{json, Value};

use crate::embedder::local;
use crate::engine::{indexer, state::AppState};
use crate::mcp::protocol::{CallToolResult, ListToolsResult, Tool, ToolContent};
use crate::query::hybrid::{run_query, QueryOpts};
use crate::store::{chunks, db, graph_store};

pub fn list_tools() -> ListToolsResult {
    ListToolsResult {
        tools: vec![
            tool(
                "anubis_search",
                "Hybrid search over the Anubis local knowledge index using semantic vectors, BM25, entities, and graph expansion.",
                json!({
                    "type": "object",
                    "properties": {
                        "q": { "type": "string", "description": "Search query." },
                        "limit": { "type": "integer", "minimum": 1, "maximum": 50, "default": 10 },
                        "depth": { "type": "integer", "minimum": 0, "maximum": 3, "default": 1 }
                    },
                    "required": ["q"]
                }),
            ),
            tool(
                "anubis_index_file",
                "Index or re-index one local file into Anubis.",
                json!({
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "Absolute path to a supported file." }
                    },
                    "required": ["path"]
                }),
            ),
            tool(
                "anubis_index_folder",
                "Index supported files under one local folder into Anubis.",
                json!({
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "Absolute path to a folder." }
                    },
                    "required": ["path"]
                }),
            ),
            tool(
                "anubis_get_index_stats",
                "Return document, chunk, entity, and graph counts for the local Anubis index.",
                json!({ "type": "object", "properties": {} }),
            ),
            tool(
                "anubis_list_documents",
                "List documents currently known to the Anubis index.",
                json!({ "type": "object", "properties": {} }),
            ),
            tool(
                "anubis_get_doc_chunks",
                "Return all chunks for one indexed document.",
                json!({
                    "type": "object",
                    "properties": {
                        "doc_id": { "type": "string", "description": "Document id." }
                    },
                    "required": ["doc_id"]
                }),
            ),
            tool(
                "anubis_get_chunk_neighbors",
                "Return graph neighbors for one chunk.",
                json!({
                    "type": "object",
                    "properties": {
                        "chunk_id": { "type": "string", "description": "Chunk id." },
                        "depth": { "type": "integer", "minimum": 1, "maximum": 3, "default": 1 }
                    },
                    "required": ["chunk_id"]
                }),
            ),
            tool(
                "anubis_get_graph_overview",
                "Return a capped graph overview for visualization or exploration.",
                json!({
                    "type": "object",
                    "properties": {
                        "limit": { "type": "integer", "minimum": 1, "maximum": 1000, "default": 250 }
                    }
                }),
            ),
            tool(
                "anubis_get_graph_neighborhood",
                "Return graph nodes and edges around one chunk.",
                json!({
                    "type": "object",
                    "properties": {
                        "chunk_id": { "type": "string", "description": "Chunk id." },
                        "depth": { "type": "integer", "minimum": 1, "maximum": 3, "default": 2 },
                        "limit": { "type": "integer", "minimum": 1, "maximum": 1000, "default": 160 }
                    },
                    "required": ["chunk_id"]
                }),
            ),
        ],
    }
}

pub async fn call_tool(state: &AppState, name: &str, arguments: Value) -> CallToolResult {
    match dispatch(state, name, arguments).await {
        Ok(structured) => tool_result(structured),
        Err(message) => error_result(&message),
    }
}

async fn dispatch(state: &AppState, name: &str, arguments: Value) -> Result<Value, String> {
    match name {
        "anubis_search" => {
            let q = string_arg(&arguments, "q")?;
            let limit = usize_arg(&arguments, "limit")?.unwrap_or(10).min(50);
            let depth = usize_arg(&arguments, "depth")?.unwrap_or(1).min(3);
            let query_embedding = {
                let mut embedder = state.embedder.lock().await;
                local::embed_query(&mut embedder, &q).map_err(|e| e.to_string())?
            };
            let db = state.db.lock().await;
            let fts = state.fts.lock().await;
            let results = run_query(&db, &fts, &q, &query_embedding, QueryOpts { limit, depth })
                .map_err(|e| e.to_string())?;
            to_json(results)
        }
        "anubis_index_file" => {
            let path = string_arg(&arguments, "path")?;
            indexer::index_file(&path, state)
                .await
                .map_err(|e| e.to_string())?;
            Ok(json!({ "indexed": path }))
        }
        "anubis_index_folder" => {
            let path = string_arg(&arguments, "path")?;
            indexer::index_folder(&path, state, None)
                .await
                .map_err(|e| e.to_string())?;
            Ok(json!({ "indexed_folder": path }))
        }
        "anubis_get_index_stats" => {
            let db = state.db.lock().await;
            db::get_index_stats(&db).map_err(|e| e.to_string())
        }
        "anubis_list_documents" => {
            let db = state.db.lock().await;
            let docs = db::list_documents(&db).map_err(|e| e.to_string())?;
            // Wrap array in an object so MCP clients that expect `record`
            // for structuredContent don't reject it.
            Ok(json!({ "documents": docs }))
        }
        "anubis_get_doc_chunks" => {
            let doc_id = string_arg(&arguments, "doc_id")?;
            let db = state.db.lock().await;
            let doc_chunks = chunks::get_doc_chunks(&db, &doc_id).map_err(|e| e.to_string())?;
            Ok(json!({ "chunks": doc_chunks }))
        }
        "anubis_get_chunk_neighbors" => {
            let chunk_id = string_arg(&arguments, "chunk_id")?;
            let depth = usize_arg(&arguments, "depth")?.unwrap_or(1).min(3);
            let db = state.db.lock().await;
            let neighbors = graph_store::chunk_neighbors(&db, &chunk_id, depth * 50)
                .map_err(|e| e.to_string())?;
            Ok(json!({ "neighbors": neighbors }))
        }
        "anubis_get_graph_overview" => {
            let limit = usize_arg(&arguments, "limit")?.unwrap_or(250).min(1000);
            let db = state.db.lock().await;
            let overview = graph_store::graph_overview(&db, limit).map_err(|e| e.to_string())?;
            to_json(overview)
        }
        "anubis_get_graph_neighborhood" => {
            let chunk_id = string_arg(&arguments, "chunk_id")?;
            let depth = usize_arg(&arguments, "depth")?.unwrap_or(2).min(3);
            let limit = usize_arg(&arguments, "limit")?.unwrap_or(160).min(1000);
            let db = state.db.lock().await;
            let overview = graph_store::graph_neighborhood(&db, &chunk_id, depth, limit)
                .map_err(|e| e.to_string())?;
            to_json(overview)
        }
        _ => Err(format!("Unknown tool: {name}")),
    }
}

fn tool(name: &str, description: &str, input_schema: Value) -> Tool {
    Tool {
        name: name.to_string(),
        description: description.to_string(),
        inputSchema: input_schema,
    }
}

fn tool_result(structured: Value) -> CallToolResult {
    let text =
        serde_json::to_string_pretty(&structured).unwrap_or_else(|_| structured.to_string());
    CallToolResult {
        content: vec![ToolContent::Text { text }],
        structuredContent: Some(structured),
        isError: false,
    }
}

fn error_result(message: &str) -> CallToolResult {
    CallToolResult {
        content: vec![ToolContent::Text {
            text: message.to_string(),
        }],
        structuredContent: None,
        isError: true,
    }
}

fn to_json<T: Serialize>(value: T) -> Result<Value, String> {
    serde_json::to_value(value).map_err(|e| e.to_string())
}

fn string_arg(arguments: &Value, name: &str) -> Result<String, String> {
    arguments
        .get(name)
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| format!("Missing or invalid string argument: {name}"))
}

fn usize_arg(arguments: &Value, name: &str) -> Result<Option<usize>, String> {
    let Some(value) = arguments.get(name) else {
        return Ok(None);
    };
    let Some(number) = value.as_u64() else {
        return Err(format!("Invalid integer argument: {name}"));
    };
    usize::try_from(number)
        .map(Some)
        .map_err(|error| format!("Invalid integer argument {name}: {error}"))
}

#[cfg(test)]
mod tests {
    use super::list_tools;

    #[test]
    fn lists_all_nine_tools() {
        let result = list_tools();
        let names: Vec<&str> = result.tools.iter().map(|t| t.name.as_str()).collect();
        for expected in [
            "anubis_search",
            "anubis_index_file",
            "anubis_index_folder",
            "anubis_get_index_stats",
            "anubis_list_documents",
            "anubis_get_doc_chunks",
            "anubis_get_chunk_neighbors",
            "anubis_get_graph_overview",
            "anubis_get_graph_neighborhood",
        ] {
            assert!(
                names.contains(&expected),
                "missing tool {expected}; got {names:?}"
            );
        }
    }
}
