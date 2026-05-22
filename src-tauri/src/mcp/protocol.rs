use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Serialize, Deserialize, Debug)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: Option<Value>,
    pub method: String,
    #[serde(default)]
    pub params: Option<Value>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct InitializeResult {
    pub protocolVersion: String,
    pub capabilities: Value,
    pub serverInfo: ServerInfo,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ServerInfo {
    pub name: String,
    pub version: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Tool {
    pub name: String,
    pub description: String,
    pub inputSchema: Value,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ListToolsResult {
    pub tools: Vec<Tool>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct CallToolResult {
    pub content: Vec<ToolContent>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub structuredContent: Option<Value>,
    #[serde(default)]
    pub isError: bool,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "type")]
pub enum ToolContent {
    #[serde(rename = "text")]
    Text { text: String },
}
