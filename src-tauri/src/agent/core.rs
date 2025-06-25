use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

use crate::state::CancelReceiver;

// --- Structs ---

#[derive(Error, Debug, Clone, PartialEq, Serialize, Deserialize)]
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
    #[error("Invalid output: {0}")]
    InvalidOutput(String),
    #[error("Invalid input: {0}")]
    InvalidInput(String),
    #[error("Tool unavailable: {0}")]
    ToolUnavailable(String),
    #[error("Tool not found: {0}")]
    ToolNotFound(String),
    #[error("Tool disabled: {0}")]
    ToolDisabled(String),
    #[error("Agent terminated")]
    Terminated,
    #[error("Permission denied: {0}")]
    PermissionDenied(String),
    #[error("Unknown error: {0}")]
    Unknown(String),
    #[error("General error: {0}")]
    Other(String),
    // New variants for simplified error recovery
    #[error("Timeout: {0}")]
    Timeout(String),
    #[error("Network error: {0}")]
    NetworkError(String),
    #[error("Validation error: {0}")]
    ValidationError(String),
    #[error("Resource busy: {0}")]
    ResourceBusy(String),
}

impl From<&str> for AgentError {
    fn from(error: &str) -> Self {
        AgentError::Unknown(error.to_string())
    }
}

impl From<String> for AgentError {
    fn from(error: String) -> Self {
        AgentError::Unknown(error)
    }
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
    Idle,       // Waiting to start
    Thinking,   // Processing, deciding next step (e.g., calling LLM)
    Executing,  // Running a tool
    Responding, // Preparing final response
    Finished,   // Completed successfully
    Failed(String), // Encountered an error
    Paused,     // Temporarily stopped, can be resumed
}

// Represents the action the agent decided to take next
#[derive(Debug, Clone, PartialEq)]
pub enum AgentAction {
    ExecuteTool(Vec<ToolCall>),
    RespondToUser(String), // Added this for potential streaming/intermediate responses
    Finish(String),        // Finish with a final message
    Error(AgentError),
    Think, // Continue the thinking loop if more work needed (e.g., after tool execution)
}

// --- Traits ---

/// Manages the agent's memory (conversation history).
#[async_trait]
pub trait MemoryManager: Send + Sync {
    /// Adds a message to the memory.
    async fn add_message(&mut self, message: Message) -> Result<(), AgentError>;

    /// Retrieves all messages from memory.
    async fn get_messages(&self) -> Result<Vec<Message>, AgentError>;

    /// Retrieves the last N messages.
    async fn get_last_n_messages(&self, n: usize) -> Result<Vec<Message>, AgentError>;

    /// Clears the agent's memory.
    async fn clear_memory(&mut self) -> Result<(), AgentError>;

    // Potential future additions:
    // async fn summarize_memory(&self) -> Result<String, AgentError>;
    // async fn prune_memory(&mut self, max_tokens: usize) -> Result<(), AgentError>;
}

/// Provides and executes tools available to the agent.
#[async_trait]
pub trait ToolProvider: Send + Sync {
    /// Lists all tools currently available.
    async fn list_tools(&self) -> Result<Vec<ToolDefinition>, AgentError>;

    /// Executes a specific tool call.
    async fn execute_tool(&self, tool_call: ToolCall) -> Result<ToolResult, AgentError>;
}

/// Represents the agent's "brain" - responsible for deciding the next action.
#[async_trait]
pub trait AgentBrain: Send + Sync {
    /// Takes the current memory and available tools, returns the next action.
    async fn decide_next_action(
        &self,
        messages: &[Message],
        available_tools: &[ToolDefinition],
    ) -> Result<AgentAction, AgentError>;
}

/// Defines the main runnable interface for an agent.
/// This ties together the brain, memory, and tools.
#[async_trait]
pub trait AgentRunnable: Send + Sync {
    /// Runs the agent loop with an initial prompt.
    async fn run(
        &mut self,
        initial_prompt: String,
        cancel_rx: CancelReceiver,
    ) -> Result<String, AgentError>;

    /// Executes a single step of the agent loop.
    async fn step(&mut self, cancel_rx: CancelReceiver) -> Result<AgentAction, AgentError>;

    // Maybe add methods for pausing, resuming, stopping?
    // async fn pause(&mut self) -> Result<(), AgentError>;
    // async fn resume(&mut self) -> Result<(), AgentError>;
    // async fn stop(&mut self) -> Result<(), AgentError>;
}
