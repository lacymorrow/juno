use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use crate::agent::core::{AgentError, ToolCall};

/// Enum defining the different types of specialized agents
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AgentType {
    Browser,
    Desktop,
    System,
    Orchestrator,
}

/// Priority levels for task execution
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum TaskPriority {
    Low,
    Normal,
    High,
    Critical,
}

/// Represents a task to be executed by a specialized agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub description: String,
    pub tool_calls: Vec<ToolCall>,
    pub agent_type: AgentType,
    pub priority: TaskPriority,
    pub dependencies: Vec<String>,
    pub timeout: Option<Duration>,
    pub metadata: serde_json::Value,
}

/// Result of task execution by an agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResult {
    pub task_id: String,
    pub success: bool,
    pub output: serde_json::Value,
    pub error: Option<String>,
    pub execution_time: Duration,
    pub agent_type: AgentType,
    pub metadata: serde_json::Value,
}

impl TaskResult {
    /// Format the output for user-friendly display
    /// This properly handles different JSON value types without showing raw JSON syntax
    pub fn format_output(&self) -> String {
        format_task_output(&self.output)
    }
}

/// Helper function to format a serde_json::Value for user-friendly display
///
/// This function addresses the issue where `.to_string()` on serde_json::Value
/// causes JSON strings to appear with escaped quotes and objects/arrays to show
/// raw JSON syntax, making output less readable.
pub fn format_task_output(value: &serde_json::Value) -> String {
    match value {
        // For JSON strings, extract the string content directly (no quotes)
        serde_json::Value::String(s) => s.clone(),

        // For numbers and booleans, display them cleanly
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::Bool(b) => b.to_string(),

        // For null values, provide a meaningful message
        serde_json::Value::Null => "No output".to_string(),

        // For objects, provide a summary rather than raw JSON
        serde_json::Value::Object(obj) => {
            if obj.is_empty() {
                "Empty result".to_string()
            } else if obj.len() == 1 {
                if let Some((key, val)) = obj.iter().next() {
                    // For single-key objects, try to extract meaningful content
                    match val {
                        serde_json::Value::String(s) => s.clone(),
                        _ => format!("{}: {}", key, format_task_output(val))
                    }
                } else {
                    "Result available".to_string()
                }
            } else {
                format!("Result with {} fields", obj.len())
            }
        },

        // For arrays, provide a summary rather than raw JSON
        serde_json::Value::Array(arr) => {
            if arr.is_empty() {
                "Empty list".to_string()
            } else if arr.len() == 1 {
                // For single-item arrays, try to extract the content
                format_task_output(&arr[0])
            } else {
                format!("List with {} items", arr.len())
            }
        }
    }
}

/// Capability description for an agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentCapability {
    pub name: String,
    pub description: String,
    pub tool_patterns: Vec<String>, // Tool name patterns this capability handles
    pub confidence: f32, // 0.0 to 1.0, how confident this agent is with this capability
}

/// Status information about an agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentStatus {
    pub agent_type: AgentType,
    pub is_available: bool,
    pub current_tasks: usize,
    pub total_completed: usize,
    pub success_rate: f32,
    pub average_execution_time: Duration,
    pub capabilities: Vec<AgentCapability>,
}

/// Trait that all specialized agents must implement
#[async_trait]
pub trait SpecializedAgent: Send + Sync {
    /// Get the type of this agent
    fn agent_type(&self) -> AgentType;

    /// Get the capabilities this agent can handle
    fn get_capabilities(&self) -> Vec<AgentCapability>;

    /// Check if this agent can handle a specific task
    async fn can_handle_task(&self, task: &Task) -> bool;

    /// Execute a task and return the result
    async fn handle_task(&self, task: Task) -> Result<TaskResult, AgentError>;

    /// Get current status of the agent
    async fn get_status(&self) -> AgentStatus;

    /// Initialize the agent (called once at startup)
    async fn initialize(&mut self) -> Result<(), AgentError> {
        Ok(())
    }

    /// Shutdown the agent gracefully
    async fn shutdown(&mut self) -> Result<(), AgentError> {
        Ok(())
    }

    /// Get the confidence level for handling a specific tool call (0.0 to 1.0)
    fn get_confidence_for_tool(&self, tool_name: &str) -> f32 {
        let capabilities = self.get_capabilities();
        for capability in capabilities {
            for pattern in &capability.tool_patterns {
                if tool_name.contains(pattern) {
                    return capability.confidence;
                }
            }
        }
        0.0
    }

    /// Check if the agent is currently available to take on new tasks
    async fn is_available(&self) -> bool {
        true // Default implementation - can be overridden
    }
}
