use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

#[derive(Error, Debug, Clone, PartialEq)]
pub enum AgentError {
    #[error("LLM communication error: {0}")]
    LlmError(String),
    #[error("Tool execution error: {0}")]
    ToolError(String),
    #[error("Memory error: {0}")]
    MemoryError(String),
    #[error("Configuration error: {0}")]
    ConfigurationError(String),
    #[error("Invalid state transition: {0}")]
    StateError(String),
    #[error("Maximum steps reached")]
    MaxStepsReached,
    #[error("Agent loop error: {0}")]
    LoopError(String),
    #[error("Input validation error: {0}")]
    InputError(String),
    #[error("Output processing error: {0}")]
    OutputError(String),
    #[error("Tool not found: {0}")]
    ToolNotFound(String),
    #[error("Agent terminated")]
    Terminated,
    #[error("Unknown error: {0}")]
    Unknown(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Role {
    User,
    Assistant,
    Tool,
    System,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Message {
    pub role: Role,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>, // Used for Tool Result messages
    // Optional name field, as seen in some APIs
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>, // Often used for the tool name in Tool Role messages
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolCall {
    pub id: String,
    // pub tool_type: String, // Often fixed like "function", maybe omit if not needed
    pub name: String, // The name of the tool to call
    pub input: Value, // Arguments for the tool (JSON object)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolResult {
    pub call_id: String, // Reference back to the ToolCall id
    pub output: Value,   // The result from the tool (JSON value)
    // Consider adding success/failure status
    // pub success: bool,
}

// Basic definition for a tool known by the agent
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: Value, // JSON schema for the 'input' field in ToolCall
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentState {
    Idle,      // Waiting to start
    Thinking,  // Processing, deciding next step (e.g., calling LLM)
    Executing, // Running a tool
    Responding, // Preparing final response
    Finished,  // Completed successfully
    Failed(String), // Encountered an error
    Paused,    // Temporarily stopped, can be resumed
}

// Represents the action the agent decided to take next
#[derive(Debug, Clone, PartialEq)]
pub enum AgentAction {
    ExecuteTool(Vec<ToolCall>),
    RespondToUser(String),
    Finish(String), // Finish with a final message
    Error(AgentError),
    Think,         // Continue the thinking loop if more work needed
}
