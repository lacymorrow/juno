use std::{collections::HashMap, sync::Arc, time::Instant};
use computer_use_ai_sdk::UIElement;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use serde_json::Value;

// ================ Types ================

#[derive(Debug, Deserialize, Serialize)]
pub struct ElementSelector {
    pub app_name: String,
    pub window_name: Option<String>,
    pub locator: String,
    pub index: Option<usize>,
    pub text: Option<String>,
    pub label: Option<String>,
    pub description: Option<String>,
    pub element_id: Option<String>,
    pub use_background_apps: Option<bool>,
    pub activate_app: Option<bool>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct FindElementsRequest {
    pub selector: ElementSelector,
    pub max_results: Option<usize>,
    pub max_depth: Option<usize>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ClickElementRequest {
    pub selector: ElementSelector,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TypeTextRequest {
    pub selector: ElementSelector,
    pub text: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PressKeyRequest {
    pub selector: ElementSelector,
    pub key_combo: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct GetTextRequest {
    pub app_name: String,
    pub window_name: Option<String>,
    pub max_depth: Option<usize>,
    pub use_background_apps: Option<bool>,
    pub activate_app: Option<bool>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ElementPosition {
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ElementSize {
    pub width: i32,
    pub height: i32,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ElementInfo {
    pub id: Option<String>,
    pub role: String,
    pub label: Option<String>,
    pub description: Option<String>,
    pub text: Option<String>,
    pub position: Option<ElementPosition>,
    pub size: Option<ElementSize>,
    pub properties: serde_json::Value,
}

#[derive(Debug, Serialize)]
pub struct FindElementsResponse {
    pub data: Vec<ElementInfo>,
}

#[derive(Debug, Serialize)]
pub struct ActionResponse {
    pub success: bool,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct GetTextResponse {
    pub success: bool,
    pub text: String,
}

// App state
pub struct AppState {
    pub element_cache: Arc<Mutex<Option<(Vec<UIElement>, Instant, String)>>>,
}

// MCP-specific types
#[derive(Debug, Deserialize, Serialize)]
pub struct MCPRequest {
    pub jsonrpc: String,
    pub id: Value,
    pub method: String,
    pub params: Option<Value>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct MCPResponse {
    pub jsonrpc: String,
    pub id: Value,
    pub result: Value,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct MCPErrorResponse {
    pub jsonrpc: String,
    pub id: Value,
    pub error: MCPError,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct MCPError {
    pub code: i32,
    pub message: String,
    pub data: Option<Value>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct InitializeParams {
    pub capabilities: ClientCapabilities,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ClientCapabilities {
    // MCP client capabilities
    pub tools: Option<ToolClientCapabilities>,
    pub resources: Option<ResourceClientCapabilities>,
    // Add other capabilities as needed
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ToolClientCapabilities {
    pub execution: bool,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ResourceClientCapabilities {
    // Resource capabilities
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ServerCapabilities {
    pub tools: Option<ToolServerCapabilities>,
    pub resources: Option<ResourceServerCapabilities>,
    // Add other capabilities as needed
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ToolServerCapabilities {
    pub functions: Vec<ToolFunctionDefinition>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ResourceServerCapabilities {
    // Resource capabilities
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ToolFunctionDefinition {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value, // JSON Schema
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ExecuteToolFunctionParams {
    pub function: String,
    pub arguments: Value,
}

// Types for scrolling
#[derive(Debug, Deserialize, Serialize)]
pub struct ScrollElementRequest {
    pub selector: Option<ElementSelector>,
    pub coordinates: Option<ElementPosition>,
    pub direction: String,
    pub amount: f64,
}

// Types for opening applications
#[derive(Deserialize, Serialize)]
pub struct OpenApplicationRequest {
    pub app_name: String,
}

#[derive(Serialize)]
pub struct OpenApplicationResponse {
    pub success: bool,
    pub message: String,
}

// Types for opening URLs
#[derive(Deserialize, Serialize)]
pub struct OpenUrlRequest {
    pub url: String,
    pub browser: Option<String>,
}

#[derive(Serialize)]
pub struct OpenUrlResponse {
    pub success: bool,
    pub message: String,
}

// Types for interactable elements
#[derive(Debug, Deserialize, Serialize)]
pub struct ListInteractableElementsRequest {
    pub app_name: String,
    pub max_elements: Option<usize>,
    pub use_background_apps: Option<bool>,
    pub activate_app: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct InteractableElement {
    pub index: usize,
    pub role: String,
    pub interactability: String, // "definite", "sometimes", "none"
    pub text: String,
    pub position: Option<ElementPosition>,
    pub size: Option<ElementSize>,
    pub element_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ElementCacheInfo {
    pub cache_id: String,
    pub timestamp: String,
    pub expires_at: String,
    pub element_count: usize,
    pub ttl_seconds: u64,
}

// Remove old ElementStats and add new ElementStatistics struct
#[derive(serde::Serialize, Debug)]
pub struct ElementStatistics {
    pub count: usize,
    pub excluded_count: usize,
    pub excluded_non_interactable: usize,
    pub excluded_no_text: usize,
    pub with_text_count: usize,
    pub without_text_count: usize,
    pub top_roles: HashMap<String, u32>,
    pub properties: HashMap<String, u32>,
}

#[derive(serde::Serialize, Debug)]
pub struct ListElementsAndAttributesResponse {
    pub elements: Vec<serde_json::Value>,
    pub cache_info: ElementCacheInfo,
    pub stats: ElementStatistics,
    pub processing_time_seconds: String,
}

// Types for index-based operations
#[derive(Debug, Deserialize, Serialize)]
pub struct ClickByIndexRequest {
    pub element_index: usize,
}

#[derive(Debug, Serialize)]
pub struct ClickByIndexResponse {
    pub success: bool,
    pub message: String,
    pub elements: Option<ListElementsAndAttributesResponse>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TypeByIndexRequest {
    pub element_index: usize,
    pub text: String,
}

#[derive(Debug, Serialize)]
pub struct TypeByIndexResponse {
    pub success: bool,
    pub message: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PressKeyByIndexRequest {
    pub element_index: usize,
    pub key_combo: String,
}

#[derive(Debug, Serialize)]
pub struct PressKeyByIndexResponse {
    pub success: bool,
    pub message: String,
}

// Types for input control
#[derive(Debug, Deserialize)]
pub struct InputControlRequest {
    pub action: InputAction,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum InputAction {
    KeyPress(String),
    MouseMove { x: i32, y: i32 },
    MouseClick(String),
    WriteText(String),
}

#[derive(Serialize)]
pub struct InputControlResponse {
    pub success: bool,
}

// Combined response types
#[derive(Serialize)]
pub struct InputControlWithElementsResponse {
    pub input: InputControlResponse,
    pub elements: Option<ListElementsAndAttributesResponse>,
}

#[derive(Serialize)]
pub struct ClickByIndexWithElementsResponse {
    pub click: ClickByIndexResponse,
    pub elements: Option<ListElementsAndAttributesResponse>,
}

#[derive(Serialize)]
pub struct TypeByIndexWithElementsResponse {
    pub type_action: TypeByIndexResponse,
    pub elements: Option<ListElementsAndAttributesResponse>,
}

#[derive(Debug, Serialize)]
pub struct PressKeyByIndexWithElementsResponse {
    pub press_key: PressKeyByIndexResponse,
    pub elements: Option<ListElementsAndAttributesResponse>,
}

#[derive(Serialize)]
pub struct OpenApplicationWithElementsResponse {
    pub application: OpenApplicationResponse,
    pub elements: Option<ListElementsAndAttributesResponse>,
}

#[derive(Serialize)]
pub struct OpenUrlWithElementsResponse {
    pub url: OpenUrlResponse,
    pub elements: Option<ListElementsAndAttributesResponse>,
}
