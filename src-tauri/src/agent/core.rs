use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

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
    /// Images the person attached to this turn, as data URLs.
    ///
    /// They live on the message rather than travelling beside the query,
    /// because "here is a screenshot" and then "now look at this other thing"
    /// only works if the picture is still in the conversation two turns later.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub images: Option<Vec<String>>,
}

impl Message {
    /// A plain message with no tool calls, name, or attachments.
    pub fn new(role: Role, content: impl Into<String>) -> Self {
        Self {
            role,
            content: content.into(),
            tool_calls: None,
            tool_call_id: None,
            name: None,
            images: None,
        }
    }

    /// A message from the person, carrying whatever they attached to it.
    pub fn from_user(content: impl Into<String>, images: Option<Vec<String>>) -> Self {
        Self {
            images: images.filter(|i| !i.is_empty()),
            ..Self::new(Role::User, content)
        }
    }
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
    /// API type identifier for tool versioning (e.g., "computer_20250124", "bash_20250124")
    /// This ensures compatibility with specific API versions and enables version-based tool selection
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_type: Option<String>,
    /// Beta flag identifier for tools requiring beta access (e.g., "computer-use-2025-01-24")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub beta_flag: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentState {
    Idle,           // Waiting to start
    Thinking,       // Processing, deciding next step (e.g., calling LLM)
    Executing,      // Running a tool
    Responding,     // Preparing final response
    Finished,       // Completed successfully
    Failed(String), // Encountered an error
    Paused,         // Temporarily stopped, can be resumed
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

// The traits that used to sit here were a second, poorer declaration of the
// four in agent/traits.rs, which every implementor and caller actually imports.
// traits.rs is a strict superset: it also has clean_orphaned_tool_calls and its
// siblings, execute_batch_tools, supports_streaming, decide_next_action_streaming
// and the whole StreamingAgentBrain trait. Nothing resolved through the copies
// here, so they were deleted rather than merged.

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // These came from agent/structs.rs, a duplicate of this file that nothing
    // imported. It carried the only tests these types had; this file had none.
    // The types were identical apart from variants and fields this one has and
    // that one lacked, so the tests moved here rather than going in the bin
    // with their copy.

    #[test]
    fn an_error_reads_as_a_sentence() {
        assert_eq!(
            AgentError::LlmError("Connection failed".to_string()).to_string(),
            "LLM communication error: Connection failed"
        );
        assert_eq!(
            AgentError::MaxStepsReached.to_string(),
            "Maximum steps reached"
        );
        assert_eq!(AgentError::Terminated.to_string(), "Agent terminated");
    }

    #[test]
    fn errors_compare_on_their_contents() {
        let one = AgentError::LlmError("test".to_string());
        assert_eq!(one, AgentError::LlmError("test".to_string()));
        assert_ne!(one, AgentError::LlmError("different".to_string()));
        assert_ne!(one, AgentError::MaxStepsReached);
    }

    #[test]
    fn a_role_survives_a_round_trip() {
        let serialized = serde_json::to_string(&Role::User).expect("serialize");
        assert_eq!(serialized, "\"User\"");
        let back: Role = serde_json::from_str(&serialized).expect("deserialize");
        assert_eq!(back, Role::User);
    }

    #[test]
    fn a_plain_message_carries_nothing_extra() {
        let message = Message::new(Role::User, "Hello");
        assert_eq!(message.role, Role::User);
        assert_eq!(message.content, "Hello");
        assert!(message.tool_calls.is_none());
        assert!(message.images.is_none());
    }

    #[test]
    fn a_user_message_keeps_what_was_attached_to_it() {
        let with = Message::from_user(
            "look at this",
            Some(vec!["data:image/png;base64,AAA".into()]),
        );
        assert_eq!(with.images.as_deref().map(<[String]>::len), Some(1));
        // An empty list is the same as nothing attached, so the provider is
        // never handed a turn with an empty image array.
        let without = Message::from_user("hello", Some(vec![]));
        assert!(without.images.is_none());
        assert!(Message::from_user("hello", None).images.is_none());
    }

    #[test]
    fn a_message_can_carry_tool_calls() {
        let tool_call = ToolCall {
            id: "call_123".to_string(),
            name: "get_weather".to_string(),
            input: json!({ "location": "New York" }),
        };
        let mut message = Message::new(Role::Assistant, "I'll check the weather for you");
        message.tool_calls = Some(vec![tool_call.clone()]);

        let calls = message.tool_calls.as_ref().expect("tool calls");
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0], tool_call);
    }

    #[test]
    fn a_tool_call_survives_a_round_trip() {
        let tool_call = ToolCall {
            id: "call_123".to_string(),
            name: "test_tool".to_string(),
            input: json!({ "param1": "value1", "param2": 42 }),
        };
        let serialized = serde_json::to_string(&tool_call).expect("serialize");
        let back: ToolCall = serde_json::from_str(&serialized).expect("deserialize");

        assert_eq!(back, tool_call);
        assert_eq!(back.input["param1"], "value1");
        assert_eq!(back.input["param2"], 42);
    }

    #[test]
    fn a_tool_result_keeps_its_call_id_and_output() {
        let result = ToolResult {
            call_id: "call_123".to_string(),
            output: json!({ "success": true, "data": "test result" }),
        };
        assert_eq!(result.call_id, "call_123");
        assert_eq!(result.output["success"], true);
        assert_eq!(result.output["data"], "test result");
    }

    #[test]
    fn a_tool_definition_holds_its_schema() {
        let definition = ToolDefinition {
            name: "test_tool".to_string(),
            description: "A test tool for validation".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "param1": { "type": "string" },
                    "param2": { "type": "number" }
                },
                "required": ["param1"]
            }),
            api_type: None,
            beta_flag: None,
        };
        assert_eq!(definition.name, "test_tool");
        assert!(definition.input_schema["properties"].is_object());
        assert!(definition.input_schema["required"].is_array());
    }

    #[test]
    fn a_failed_state_carries_its_reason() {
        assert_eq!(AgentState::Idle, AgentState::Idle);
        match AgentState::Failed("Test error".to_string()) {
            AgentState::Failed(message) => assert_eq!(message, "Test error"),
            other => panic!("Expected Failed state, got {:?}", other),
        }
    }

    #[test]
    fn a_state_survives_a_round_trip() {
        let serialized = serde_json::to_string(&AgentState::Thinking).expect("serialize");
        let back: AgentState = serde_json::from_str(&serialized).expect("deserialize");
        assert_eq!(back, AgentState::Thinking);

        let failed = AgentState::Failed("Error message".to_string());
        let serialized = serde_json::to_string(&failed).expect("serialize");
        let back: AgentState = serde_json::from_str(&serialized).expect("deserialize");
        assert_eq!(back, failed);
    }
}
