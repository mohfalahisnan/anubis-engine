use std::{
    io::{self, BufRead, Write},
    path::PathBuf,
};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::{
    embedder::local,
    engine::{indexer, state::AppState},
    query::hybrid::{run_query, QueryOpts},
    store::{chunks, db, graph_store},
};

const PROTOCOL_VERSION: &str = "2025-06-18";
const SERVER_NAME: &str = "anubis-engine";
const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Deserialize)]
struct JsonRpcRequest {
    id: Option<Value>,
    method: String,
    #[serde(default)]
    params: Value,
}

#[derive(Debug, Serialize)]
struct JsonRpcResponse {
    jsonrpc: &'static str,
    id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize)]
struct JsonRpcError {
    code: i64,
    message: String,
}

pub fn run_stdio() -> Result<(), String> {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_writer(io::stderr)
        .try_init();

    let db_path = configured_db_path()?;
    let fts_path = configured_fts_path(&db_path);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| error.to_string())?;
    let state = AppState::new(&db_path, &fts_path).map_err(|error| error.to_string())?;

    let stdin = io::stdin();
    let mut stdout = io::stdout();
    for line in stdin.lock().lines() {
        let line = line.map_err(|error| error.to_string())?;
        if line.trim().is_empty() {
            continue;
        }

        let request = match serde_json::from_str::<JsonRpcRequest>(&line) {
            Ok(request) => request,
            Err(error) => {
                write_response(
                    &mut stdout,
                    error_response(Value::Null, -32700, format!("Parse error: {error}")),
                )?;
                continue;
            }
        };

        let Some(id) = request.id.clone() else {
            continue;
        };

        let response = match handle_request(&runtime, &state, request) {
            Ok(result) => success_response(id, result),
            Err((code, message)) => error_response(id, code, message),
        };
        write_response(&mut stdout, response)?;
    }

    Ok(())
}

fn handle_request(
    runtime: &tokio::runtime::Runtime,
    state: &AppState,
    request: JsonRpcRequest,
) -> Result<Value, (i64, String)> {
    match request.method.as_str() {
        "initialize" => Ok(json!({
            "protocolVersion": PROTOCOL_VERSION,
            "capabilities": {
                "tools": {
                    "listChanged": false
                }
            },
            "serverInfo": {
                "name": SERVER_NAME,
                "version": SERVER_VERSION
            }
        })),
        "tools/list" => Ok(json!({ "tools": tool_definitions() })),
        "tools/call" => {
            let name = request
                .params
                .get("name")
                .and_then(Value::as_str)
                .ok_or_else(|| (-32602, "tools/call requires params.name".to_string()))?;
            let arguments = request
                .params
                .get("arguments")
                .cloned()
                .unwrap_or_else(|| json!({}));
            runtime.block_on(call_tool(state, name, arguments))
        }
        "ping" => Ok(json!({})),
        _ => Err((-32601, format!("Method not found: {}", request.method))),
    }
}

async fn call_tool(state: &AppState, name: &str, arguments: Value) -> Result<Value, (i64, String)> {
    match name {
        "anubis_search" => {
            let q = string_arg(&arguments, "q")?;
            let limit = usize_arg(&arguments, "limit")?.unwrap_or(10).min(50);
            let depth = usize_arg(&arguments, "depth")?.unwrap_or(1).min(3);
            let query_embedding = {
                let mut embedder = state.embedder.lock().await;
                local::embed_query(&mut embedder, &q).map_err(tool_error)?
            };
            let db = state.db.lock().await;
            let fts = state.fts.lock().await;
            let results = run_query(&db, &fts, &q, &query_embedding, QueryOpts { limit, depth })
                .map_err(tool_error)?;
            tool_json(results)
        }
        "anubis_index_file" => {
            let path = string_arg(&arguments, "path")?;
            indexer::index_file(&path, state)
                .await
                .map_err(tool_error)?;
            tool_json(json!({ "indexed": path }))
        }
        "anubis_index_folder" => {
            let path = string_arg(&arguments, "path")?;
            indexer::index_folder(&path, state, None)
                .await
                .map_err(tool_error)?;
            tool_json(json!({ "indexed_folder": path }))
        }
        "anubis_get_index_stats" => {
            let db = state.db.lock().await;
            let stats = db::get_index_stats(&db).map_err(tool_error)?;
            tool_json(stats)
        }
        "anubis_list_documents" => {
            let db = state.db.lock().await;
            let documents = db::list_documents(&db).map_err(tool_error)?;
            tool_json(documents)
        }
        "anubis_get_doc_chunks" => {
            let doc_id = string_arg(&arguments, "doc_id")?;
            let db = state.db.lock().await;
            let doc_chunks = chunks::get_doc_chunks(&db, &doc_id).map_err(tool_error)?;
            tool_json(doc_chunks)
        }
        "anubis_get_chunk_neighbors" => {
            let chunk_id = string_arg(&arguments, "chunk_id")?;
            let depth = usize_arg(&arguments, "depth")?.unwrap_or(1).min(3);
            let db = state.db.lock().await;
            let neighbors =
                graph_store::chunk_neighbors(&db, &chunk_id, depth * 50).map_err(tool_error)?;
            tool_json(neighbors)
        }
        "anubis_get_graph_overview" => {
            let limit = usize_arg(&arguments, "limit")?.unwrap_or(250).min(1000);
            let db = state.db.lock().await;
            let overview = graph_store::graph_overview(&db, limit).map_err(tool_error)?;
            tool_json(overview)
        }
        "anubis_get_graph_neighborhood" => {
            let chunk_id = string_arg(&arguments, "chunk_id")?;
            let depth = usize_arg(&arguments, "depth")?.unwrap_or(2).min(3);
            let limit = usize_arg(&arguments, "limit")?.unwrap_or(160).min(1000);
            let db = state.db.lock().await;
            let overview = graph_store::graph_neighborhood(&db, &chunk_id, depth, limit)
                .map_err(tool_error)?;
            tool_json(overview)
        }
        _ => Err((-32602, format!("Unknown tool: {name}"))),
    }
}

fn tool_definitions() -> Value {
    json!([
        {
            "name": "anubis_search",
            "description": "Hybrid search over the Anubis local knowledge index using semantic vectors, BM25, entities, and graph expansion.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "q": { "type": "string", "description": "Search query." },
                    "limit": { "type": "integer", "minimum": 1, "maximum": 50, "default": 10 },
                    "depth": { "type": "integer", "minimum": 0, "maximum": 3, "default": 1 }
                },
                "required": ["q"]
            },
            "annotations": { "readOnlyHint": true }
        },
        {
            "name": "anubis_index_file",
            "description": "Index or re-index one local file into Anubis.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Absolute path to a supported file." }
                },
                "required": ["path"]
            },
            "annotations": { "readOnlyHint": false, "idempotentHint": true }
        },
        {
            "name": "anubis_index_folder",
            "description": "Index supported files under one local folder into Anubis.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Absolute path to a folder." }
                },
                "required": ["path"]
            },
            "annotations": { "readOnlyHint": false, "idempotentHint": true }
        },
        {
            "name": "anubis_get_index_stats",
            "description": "Return document, chunk, entity, and graph counts for the local Anubis index.",
            "inputSchema": { "type": "object", "properties": {} },
            "annotations": { "readOnlyHint": true }
        },
        {
            "name": "anubis_list_documents",
            "description": "List documents currently known to the Anubis index.",
            "inputSchema": { "type": "object", "properties": {} },
            "annotations": { "readOnlyHint": true }
        },
        {
            "name": "anubis_get_doc_chunks",
            "description": "Return all chunks for one indexed document.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "doc_id": { "type": "string", "description": "Document id." }
                },
                "required": ["doc_id"]
            },
            "annotations": { "readOnlyHint": true }
        },
        {
            "name": "anubis_get_chunk_neighbors",
            "description": "Return graph neighbors for one chunk.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "chunk_id": { "type": "string", "description": "Chunk id." },
                    "depth": { "type": "integer", "minimum": 1, "maximum": 3, "default": 1 }
                },
                "required": ["chunk_id"]
            },
            "annotations": { "readOnlyHint": true }
        },
        {
            "name": "anubis_get_graph_overview",
            "description": "Return a capped graph overview for visualization or exploration.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "limit": { "type": "integer", "minimum": 1, "maximum": 1000, "default": 250 }
                }
            },
            "annotations": { "readOnlyHint": true }
        },
        {
            "name": "anubis_get_graph_neighborhood",
            "description": "Return graph nodes and edges around one chunk.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "chunk_id": { "type": "string", "description": "Chunk id." },
                    "depth": { "type": "integer", "minimum": 1, "maximum": 3, "default": 2 },
                    "limit": { "type": "integer", "minimum": 1, "maximum": 1000, "default": 160 }
                },
                "required": ["chunk_id"]
            },
            "annotations": { "readOnlyHint": true }
        }
    ])
}

fn tool_json(value: impl Serialize) -> Result<Value, (i64, String)> {
    let structured = serde_json::to_value(value).map_err(|error| (-32603, error.to_string()))?;
    let text =
        serde_json::to_string_pretty(&structured).map_err(|error| (-32603, error.to_string()))?;
    Ok(json!({
        "content": [
            {
                "type": "text",
                "text": text
            }
        ],
        "structuredContent": structured,
        "isError": false
    }))
}

fn string_arg(arguments: &Value, name: &str) -> Result<String, (i64, String)> {
    arguments
        .get(name)
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            (
                -32602,
                format!("Missing or invalid string argument: {name}"),
            )
        })
}

fn usize_arg(arguments: &Value, name: &str) -> Result<Option<usize>, (i64, String)> {
    let Some(value) = arguments.get(name) else {
        return Ok(None);
    };
    let Some(number) = value.as_u64() else {
        return Err((-32602, format!("Invalid integer argument: {name}")));
    };
    usize::try_from(number)
        .map(Some)
        .map_err(|error| (-32602, format!("Invalid integer argument {name}: {error}")))
}

fn success_response(id: Value, result: Value) -> JsonRpcResponse {
    JsonRpcResponse {
        jsonrpc: "2.0",
        id,
        result: Some(result),
        error: None,
    }
}

fn error_response(id: Value, code: i64, message: String) -> JsonRpcResponse {
    JsonRpcResponse {
        jsonrpc: "2.0",
        id,
        result: None,
        error: Some(JsonRpcError { code, message }),
    }
}

fn write_response(stdout: &mut io::Stdout, response: JsonRpcResponse) -> Result<(), String> {
    let line = serde_json::to_string(&response).map_err(|error| error.to_string())?;
    stdout
        .write_all(line.as_bytes())
        .and_then(|_| stdout.write_all(b"\n"))
        .and_then(|_| stdout.flush())
        .map_err(|error| error.to_string())
}

fn tool_error(error: impl std::fmt::Display) -> (i64, String) {
    (-32603, error.to_string())
}

fn configured_db_path() -> Result<PathBuf, String> {
    if let Some(path) = std::env::var_os("ANUBIS_DB_PATH") {
        return Ok(PathBuf::from(path));
    }

    Ok(default_app_data_dir()?.join("anubis.db"))
}

fn configured_fts_path(db_path: &std::path::Path) -> PathBuf {
    if let Some(path) = std::env::var_os("ANUBIS_FTS_PATH") {
        return PathBuf::from(path);
    }

    db_path
        .parent()
        .map(|parent| parent.join("fts_index"))
        .unwrap_or_else(|| PathBuf::from("fts_index"))
}

fn default_app_data_dir() -> Result<PathBuf, String> {
    if cfg!(target_os = "windows") {
        let Some(appdata) = std::env::var_os("APPDATA") else {
            return Err("ANUBIS_DB_PATH is required when APPDATA is unset".to_string());
        };
        return Ok(PathBuf::from(appdata).join("com.anubis-os.app"));
    }

    if cfg!(target_os = "macos") {
        let Some(home) = std::env::var_os("HOME") else {
            return Err("ANUBIS_DB_PATH is required when HOME is unset".to_string());
        };
        return Ok(PathBuf::from(home)
            .join("Library")
            .join("Application Support")
            .join("com.anubis-os.app"));
    }

    if let Some(data_home) = std::env::var_os("XDG_DATA_HOME") {
        return Ok(PathBuf::from(data_home).join("com.anubis-os.app"));
    }

    let Some(home) = std::env::var_os("HOME") else {
        return Err("ANUBIS_DB_PATH is required when HOME is unset".to_string());
    };
    Ok(PathBuf::from(home)
        .join(".local")
        .join("share")
        .join("com.anubis-os.app"))
}

#[cfg(test)]
mod tests {
    use super::tool_definitions;

    #[test]
    fn lists_anubis_tools() {
        let tools_value = tool_definitions();
        let tools = tools_value.as_array().expect("tools array");
        assert!(tools.iter().any(|tool| tool["name"] == "anubis_search"));
    }
}
