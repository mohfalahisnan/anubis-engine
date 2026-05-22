pub mod protocol;
pub mod tools;

use std::io::{self, BufRead, Write};
use serde_json::{json, Value};
use tokio::runtime::Runtime;

use crate::engine::state::AppState;
use protocol::{JsonRpcRequest, JsonRpcResponse, JsonRpcError, InitializeResult, ServerInfo};

fn get_db_path() -> Result<std::path::PathBuf, String> {
    if let Some(path) = std::env::var_os("ANUBIS_DB_PATH") {
        return Ok(std::path::PathBuf::from(path));
    }
    Ok(default_app_data_dir()?.join("anubis.db"))
}

fn get_fts_path(db_path: &std::path::Path) -> std::path::PathBuf {
    if let Some(path) = std::env::var_os("ANUBIS_FTS_PATH") {
        return std::path::PathBuf::from(path);
    }
    db_path
        .parent()
        .map(|parent| parent.join("fts_index"))
        .unwrap_or_else(|| std::path::PathBuf::from("fts_index"))
}

fn default_app_data_dir() -> Result<std::path::PathBuf, String> {
    #[cfg(target_os = "windows")]
    {
        // Tauri's app_data_dir on Windows uses ROAMING (%APPDATA%), not LOCAL.
        // The MCP server must match so it sees the same database the UI writes to.
        let appdata = std::env::var("APPDATA")
            .map_err(|_| "APPDATA not set".to_string())?;
        Ok(std::path::PathBuf::from(appdata).join("com.anubis-os.app"))
    }
    #[cfg(target_os = "macos")]
    {
        let home = std::env::var("HOME").map_err(|_| "HOME not set".to_string())?;
        Ok(std::path::PathBuf::from(home).join("Library/Application Support/com.anubis-os.app"))
    }
    #[cfg(target_os = "linux")]
    {
        let data_home = std::env::var("XDG_DATA_HOME").unwrap_or_else(|_| {
            let home = std::env::var("HOME").unwrap_or_default();
            format!("{}/.local/share", home)
        });
        Ok(std::path::PathBuf::from(data_home).join("com.anubis-os.app"))
    }
}

pub fn run_stdio() -> Result<(), Box<dyn std::error::Error>> {
    let rt = Runtime::new()?;
    rt.block_on(async {
        run_async().await
    })
}

async fn run_async() -> Result<(), Box<dyn std::error::Error>> {
    let db_path = get_db_path()?;
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let fts_path = get_fts_path(&db_path);

    let state = AppState::new(&db_path, &fts_path)
        .map_err(|error| Box::<dyn std::error::Error>::from(error.to_string()))?;
    
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    let mut reader = stdin.lock();

    let mut line = String::new();
    while reader.read_line(&mut line)? > 0 {
        let req_str = line.trim();
        if req_str.is_empty() {
            line.clear();
            continue;
        }

        match serde_json::from_str::<JsonRpcRequest>(req_str) {
            Ok(req) => {
                if let Some(res) = handle_request(&state, req).await {
                    let out = serde_json::to_string(&res)?;
                    writeln!(stdout, "{}", out)?;
                    stdout.flush()?;
                }
            }
            Err(e) => {
                let err_res = JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: Value::Null,
                    result: None,
                    error: Some(JsonRpcError {
                        code: -32700,
                        message: format!("Parse error: {}", e),
                        data: None,
                    }),
                };
                let out = serde_json::to_string(&err_res)?;
                writeln!(stdout, "{}", out)?;
                stdout.flush()?;
            }
        }
        line.clear();
    }

    Ok(())
}

async fn handle_request(state: &AppState, req: JsonRpcRequest) -> Option<JsonRpcResponse> {
    let id = req.id.clone().unwrap_or(Value::Null);
    // Ignore notifications (requests without an ID)
    if id.is_null() && req.method != "notifications/initialized" {
        return None;
    }

    match req.method.as_str() {
        "initialize" => {
            Some(JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id,
                result: Some(json!(InitializeResult {
                    protocolVersion: "2025-06-18".to_string(),
                    capabilities: json!({
                        "tools": {
                            "listChanged": false
                        }
                    }),
                    serverInfo: ServerInfo {
                        name: "anubis-engine".to_string(),
                        version: env!("CARGO_PKG_VERSION").to_string(),
                    }
                })),
                error: None,
            })
        }
        "notifications/initialized" => {
            // Nothing to reply for notifications
            None
        }
        "ping" => Some(JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(json!({})),
            error: None,
        }),
        "tools/list" => {
            let tools = tools::list_tools();
            Some(JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id,
                result: Some(serde_json::to_value(tools).unwrap()),
                error: None,
            })
        }
        "tools/call" => {
            if let Some(params) = req.params {
                if let Some(name) = params.get("name").and_then(|v| v.as_str()) {
                    let args = params.get("arguments").cloned().unwrap_or(json!({}));
                    let result = tools::call_tool(state, name, args).await;
                    return Some(JsonRpcResponse {
                        jsonrpc: "2.0".to_string(),
                        id,
                        result: Some(serde_json::to_value(result).unwrap()),
                        error: None,
                    });
                }
            }
            Some(JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id,
                result: None,
                error: Some(JsonRpcError {
                    code: -32602,
                    message: "Invalid params for tools/call".to_string(),
                    data: None,
                })
            })
        }
        _ => {
            // Method not found
            if !id.is_null() {
                Some(JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id,
                    result: None,
                    error: Some(JsonRpcError {
                        code: -32601,
                        message: format!("Method not found: {}", req.method),
                        data: None,
                    }),
                })
            } else {
                None
            }
        }
    }
}
