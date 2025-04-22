use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Role {
    User,
    Assistant,
    Tool,
    System, // Added for potential system prompts
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: String,
    // Optional fields for tool calls/results, mirroring Anthropic's structure
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    // Optional name field, sometimes used by LLMs
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    #[serde(rename = "type")] // Match common API structures
    pub tool_type: String, // Typically "tool_use" or similar
    pub input: serde_json::Value, // Arguments for the tool
}

// Note: Manus competitive analysis mentions ToolResult with (id, observation).
// Anthropic's API typically puts the result in a 'tool' role message content.
// We'll stick to the Message structure for now, using the 'Tool' role
// and the 'tool_call_id' to link results back to calls.
// If a separate ToolResult struct becomes necessary, we can add it later.
