use serde_json::json;

use crate::engine::state::AppState;
use crate::mcp::protocol::{CallToolResult, ListToolsResult, Tool, ToolContent};
use crate::query::hybrid::{run_query, QueryOpts};
use crate::embedder::local;
use crate::store::db;

pub fn list_tools() -> ListToolsResult {
    ListToolsResult {
        tools: vec![
            Tool {
                name: "anubis_query".to_string(),
                description: "Query the Anubis knowledge engine using semantic search.".to_string(),
                inputSchema: json!({
                    "type": "object",
                    "properties": {
                        "query": {
                            "type": "string",
                            "description": "The search query"
                        },
                        "limit": {
                            "type": "number",
                            "description": "Maximum number of results to return (default 10)"
                        }
                    },
                    "required": ["query"]
                }),
            },
            Tool {
                name: "anubis_list_documents".to_string(),
                description: "List all indexed documents in the Anubis knowledge engine.".to_string(),
                inputSchema: json!({
                    "type": "object",
                    "properties": {}
                }),
            },
            Tool {
                name: "anubis_index_file".to_string(),
                description: "Index a specific file into the Anubis knowledge engine.".to_string(),
                inputSchema: json!({
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Absolute path to the file to index"
                        }
                    },
                    "required": ["path"]
                }),
            }
        ]
    }
}

pub async fn call_tool(state: &AppState, name: &str, params: serde_json::Value) -> CallToolResult {
    match name {
        "anubis_query" => {
            let query = params.get("query").and_then(|v| v.as_str()).unwrap_or("");
            let limit = params.get("limit").and_then(|v| v.as_u64()).unwrap_or(10) as usize;
            
            let query_embedding = {
                let mut embedder = state.embedder.lock().await;
                match local::embed_query(&mut embedder, query) {
                    Ok(emb) => emb,
                    Err(e) => return error_result(&format!("Failed to embed query: {}", e)),
                }
            };
            
            let db_lock = state.db.lock().await;
            let fts_lock = state.fts.lock().await;
            
            match run_query(&db_lock, &fts_lock, query, &query_embedding, QueryOpts { limit, depth: 1 }) {
                Ok(results) => {
                    let text = serde_json::to_string_pretty(&results).unwrap_or_else(|_| "[]".to_string());
                    CallToolResult {
                        content: vec![ToolContent::Text { text }],
                        isError: false,
                    }
                }
                Err(e) => error_result(&format!("Query failed: {}", e)),
            }
        }
        "anubis_list_documents" => {
            let db_lock = state.db.lock().await;
            match db::list_documents(&db_lock) {
                Ok(docs) => {
                    let text = serde_json::to_string_pretty(&docs).unwrap_or_else(|_| "[]".to_string());
                    CallToolResult {
                        content: vec![ToolContent::Text { text }],
                        isError: false,
                    }
                }
                Err(e) => error_result(&format!("Failed to list documents: {}", e)),
            }
        }
        "anubis_index_file" => {
            let path = match params.get("path").and_then(|v| v.as_str()) {
                Some(p) => p,
                None => return error_result("Missing or invalid 'path' parameter"),
            };
            
            match crate::engine::indexer::index_file(path, state).await {
                Ok(_) => CallToolResult {
                    content: vec![ToolContent::Text { text: format!("Successfully indexed file: {}", path) }],
                    isError: false,
                },
                Err(e) => error_result(&format!("Failed to index file: {}", e)),
            }
        }
        _ => error_result(&format!("Unknown tool: {}", name)),
    }
}

fn error_result(msg: &str) -> CallToolResult {
    CallToolResult {
        content: vec![ToolContent::Text { text: msg.to_string() }],
        isError: true,
    }
}
