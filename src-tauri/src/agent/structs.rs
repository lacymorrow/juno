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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_agent_error_display() {
        let error = AgentError::LlmError("Connection failed".to_string());
        assert_eq!(error.to_string(), "LLM communication error: Connection failed");

        let error = AgentError::MaxStepsReached;
        assert_eq!(error.to_string(), "Maximum steps reached");

        let error = AgentError::Terminated;
        assert_eq!(error.to_string(), "Agent terminated");
    }

    #[test]
    fn test_agent_error_equality() {
        let error1 = AgentError::LlmError("test".to_string());
        let error2 = AgentError::LlmError("test".to_string());
        let error3 = AgentError::LlmError("different".to_string());

        assert_eq!(error1, error2);
        assert_ne!(error1, error3);
        assert_ne!(error1, AgentError::MaxStepsReached);
    }

    #[test]
    fn test_role_serialization() {
        let role = Role::User;
        let serialized = serde_json::to_string(&role).unwrap();
        assert_eq!(serialized, "\"User\"");

        let deserialized: Role = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized, Role::User);
    }

    #[test]
    fn test_message_creation() {
        let message = Message {
            role: Role::User,
            content: "Hello".to_string(),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        };

        assert_eq!(message.role, Role::User);
        assert_eq!(message.content, "Hello");
        assert!(message.tool_calls.is_none());
    }

    #[test]
    fn test_message_with_tool_calls() {
        let tool_call = ToolCall {
            id: "call_123".to_string(),
            name: "get_weather".to_string(),
            input: json!({"location": "New York"}),
        };

        let message = Message {
            role: Role::Assistant,
            content: "I'll check the weather for you".to_string(),
            tool_calls: Some(vec![tool_call.clone()]),
            tool_call_id: None,
            name: None,
        };

        assert_eq!(message.tool_calls.as_ref().unwrap().len(), 1);
        assert_eq!(message.tool_calls.as_ref().unwrap()[0], tool_call);
    }

    #[test]
    fn test_tool_call_serialization() {
        let tool_call = ToolCall {
            id: "call_123".to_string(),
            name: "test_tool".to_string(),
            input: json!({"param1": "value1", "param2": 42}),
        };

        let serialized = serde_json::to_string(&tool_call).unwrap();
        let deserialized: ToolCall = serde_json::from_str(&serialized).unwrap();

        assert_eq!(deserialized, tool_call);
        assert_eq!(deserialized.input["param1"], "value1");
        assert_eq!(deserialized.input["param2"], 42);
    }

    #[test]
    fn test_tool_result_creation() {
        let result = ToolResult {
            call_id: "call_123".to_string(),
            output: json!({"success": true, "data": "test result"}),
        };

        assert_eq!(result.call_id, "call_123");
        assert_eq!(result.output["success"], true);
        assert_eq!(result.output["data"], "test result");
    }

    #[test]
    fn test_tool_definition_validation() {
        let definition = ToolDefinition {
            name: "test_tool".to_string(),
            description: "A test tool for validation".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "param1": {"type": "string"},
                    "param2": {"type": "number"}
                },
                "required": ["param1"]
            }),
        };

        assert_eq!(definition.name, "test_tool");
        assert!(definition.input_schema["properties"].is_object());
        assert!(definition.input_schema["required"].is_array());
    }

    #[test]
    fn test_agent_state_transitions() {
        let state = AgentState::Idle;
        assert_eq!(state, AgentState::Idle);

        let state = AgentState::Failed("Test error".to_string());
        match state {
            AgentState::Failed(msg) => assert_eq!(msg, "Test error"),
            _ => panic!("Expected Failed state"),
        }
    }

    #[test]
    fn test_agent_state_serialization() {
        let state = AgentState::Thinking;
        let serialized = serde_json::to_string(&state).unwrap();
        let deserialized: AgentState = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized, AgentState::Thinking);

        let failed_state = AgentState::Failed("Error message".to_string());
        let serialized = serde_json::to_string(&failed_state).unwrap();
        let deserialized: AgentState = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized, failed_state);
    }

    #[test]
    fn test_agent_action_types() {
        let action = AgentAction::Think;
        assert_eq!(action, AgentAction::Think);

        let tool_call = ToolCall {
            id: "call_123".to_string(),
            name: "test_tool".to_string(),
            input: json!({}),
        };
        let action = AgentAction::ExecuteTool(vec![tool_call.clone()]);
        match action {
            AgentAction::ExecuteTool(calls) => {
                assert_eq!(calls.len(), 1);
                assert_eq!(calls[0], tool_call);
            }
            _ => panic!("Expected ExecuteTool action"),
        }

        let action = AgentAction::RespondToUser("Hello".to_string());
        match action {
            AgentAction::RespondToUser(msg) => assert_eq!(msg, "Hello"),
            _ => panic!("Expected RespondToUser action"),
        }

        let action = AgentAction::Finish("Done".to_string());
        match action {
            AgentAction::Finish(msg) => assert_eq!(msg, "Done"),
            _ => panic!("Expected Finish action"),
        }

        let error = AgentError::MaxStepsReached;
        let action = AgentAction::Error(error.clone());
        match action {
            AgentAction::Error(e) => assert_eq!(e, error),
            _ => panic!("Expected Error action"),
        }
    }

    #[test]
    fn test_message_serialization_skips_none_fields() {
        let message = Message {
            role: Role::User,
            content: "Test".to_string(),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        };

        let serialized = serde_json::to_string(&message).unwrap();
        let json_value: serde_json::Value = serde_json::from_str(&serialized).unwrap();

        // None fields should not be present in serialized JSON
        assert!(!json_value.as_object().unwrap().contains_key("tool_calls"));
        assert!(!json_value.as_object().unwrap().contains_key("tool_call_id"));
        assert!(!json_value.as_object().unwrap().contains_key("name"));

        // But required fields should be present
        assert!(json_value.as_object().unwrap().contains_key("role"));
        assert!(json_value.as_object().unwrap().contains_key("content"));
    }

    #[test]
    fn test_tool_result_message() {
        let message = Message {
            role: Role::Tool,
            content: "Function executed successfully".to_string(),
            tool_calls: None,
            tool_call_id: Some("call_123".to_string()),
            name: Some("get_weather".to_string()),
        };

        assert_eq!(message.role, Role::Tool);
        assert_eq!(message.tool_call_id.as_ref().unwrap(), "call_123");
        assert_eq!(message.name.as_ref().unwrap(), "get_weather");
    }
}
