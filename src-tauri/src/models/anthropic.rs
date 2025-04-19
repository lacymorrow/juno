use serde::{Deserialize, Serialize};
use serde_json::Value;

// --- Anthropic API Structures ---
#[derive(Serialize, Clone)]
pub struct AnthropicMessage {
    pub role: String,
    pub content: Value, // Can be string or array of content blocks
}

#[derive(Deserialize, Debug, Clone, Serialize)]
pub struct AnthropicContentBlock {
    #[serde(rename = "type")]
    pub type_: String, // "text" or "tool_use"
    // For text blocks
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    // For tool_use blocks
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input: Option<Value>,
}

#[derive(Deserialize, Debug)]
pub struct AnthropicResponse {
    pub completion: String,
    pub stop_reason: String,
    pub model: String,
    pub stop: Option<String>,
    pub log_id: String,
    pub exception: Option<String>,
}

// Structure for tool results (sent back to Anthropic)
#[derive(Serialize)]
pub struct ToolResultBlock {
    #[serde(rename = "type")]
    pub type_: String, // Always "tool_result"
    pub tool_use_id: String,
    pub content: Value, // Can be string (old way, potentially phase out) or array of content blocks (text/image)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_error: Option<bool>,
}

// Structure for the combined result of submit_query
#[derive(Serialize)]
pub struct SubmitQueryResult {
    pub text: String,
    pub audio_base64: Option<String>, // Option because TTS might fail
}

// For the API request
#[derive(Serialize)]
pub struct AnthropicThinkingBudget {
    #[serde(rename = "type")]
    pub type_: String,
    pub budget_tokens: u32,
}

#[derive(Serialize)]
pub struct AnthropicRequest<'a> {
    pub model: &'a str,
    pub max_tokens: u32,
    pub messages: Vec<AnthropicMessage>,
    pub tools: Vec<computer_use_ai_sdk::ToolDefinition>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking: Option<AnthropicThinkingBudget>,
}

#[derive(Deserialize, Debug)]
pub struct AnthropicUsage {
    pub input_tokens: u32,
    pub output_tokens: u32,
}

#[derive(Deserialize, Debug)]
pub struct AnthropicResponseWithContent {
    pub content: Vec<AnthropicContentBlock>,
    pub stop_reason: String, // e.g., "end_turn", "tool_use"
    pub usage: AnthropicUsage,
}