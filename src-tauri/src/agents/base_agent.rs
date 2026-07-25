use crate::agent::core::{AgentError, ToolCall};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::time::Duration;

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
    /// Parallel-session id for roster attribution (LAC-3073). Travels on the
    /// task — never on the long-lived shared agent instance — so concurrent
    /// orchestrated runs cannot leak identity into each other. `None` for
    /// callers outside the session registry (queued/benchmark/legacy paths).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
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
                        _ => format!("{}: {}", key, format_task_output(val)),
                    }
                } else {
                    "Result available".to_string()
                }
            } else {
                format!("Result with {} fields", obj.len())
            }
        }

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
    pub confidence: f32,            // 0.0 to 1.0, how confident this agent is with this capability
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    fn task_with_session(id: &str, session_id: Option<&str>) -> Task {
        Task {
            id: id.to_string(),
            description: "test task".to_string(),
            tool_calls: vec![],
            agent_type: AgentType::Desktop,
            priority: TaskPriority::Normal,
            dependencies: vec![],
            timeout: None,
            metadata: serde_json::Value::Null,
            session_id: session_id.map(|s| s.to_string()),
        }
    }

    /// Tasks serialized before LAC-3073 (or built by the frontend without a
    /// session) must still deserialize — `session_id` defaults to `None`.
    #[test]
    fn task_without_session_id_deserializes_to_none() {
        let json = serde_json::json!({
            "id": "t1",
            "description": "legacy task",
            "tool_calls": [],
            "agent_type": "Desktop",
            "priority": "Normal",
            "dependencies": [],
            "timeout": null,
            "metadata": {}
        });
        let task: Task = serde_json::from_value(json).expect("legacy task deserializes");
        assert_eq!(task.session_id, None);
    }

    /// Mock agent that mirrors the shared-instance shape of DesktopAgent:
    /// one Arc'd instance handles tasks from many concurrent runs. It records
    /// which session id each handle_task invocation observed.
    struct RecordingAgent {
        seen: Arc<Mutex<Vec<(String, Option<String>)>>>,
    }

    #[async_trait]
    impl SpecializedAgent for RecordingAgent {
        fn agent_type(&self) -> AgentType {
            AgentType::Desktop
        }

        fn get_capabilities(&self) -> Vec<AgentCapability> {
            vec![]
        }

        async fn can_handle_task(&self, _task: &Task) -> bool {
            true
        }

        async fn handle_task(&self, task: Task) -> Result<TaskResult, AgentError> {
            // Yield so concurrent invocations interleave — a session id
            // stored on the shared instance (the LAC-3073 anti-pattern)
            // would be observed by the wrong task here.
            tokio::task::yield_now().await;
            self.seen
                .lock()
                .await
                .push((task.id.clone(), task.session_id.clone()));
            Ok(TaskResult {
                task_id: task.id,
                success: true,
                output: serde_json::Value::Null,
                error: None,
                execution_time: Duration::from_millis(0),
                agent_type: AgentType::Desktop,
                metadata: serde_json::Value::Null,
            })
        }

        async fn get_status(&self) -> AgentStatus {
            AgentStatus {
                agent_type: AgentType::Desktop,
                is_available: true,
                current_tasks: 0,
                total_completed: 0,
                success_rate: 1.0,
                average_execution_time: Duration::from_millis(0),
                capabilities: vec![],
            }
        }
    }

    /// Regression test for LAC-3073 per-task attribution: concurrent tasks
    /// routed through ONE shared agent instance must each carry their own
    /// session id — identity travels on the `Task`, not the agent.
    #[tokio::test]
    async fn concurrent_tasks_keep_their_own_session_ids() {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let agent: Arc<dyn SpecializedAgent> = Arc::new(RecordingAgent { seen: seen.clone() });

        let mut handles = Vec::new();
        for i in 0..8 {
            let agent = agent.clone();
            let task = task_with_session(&format!("task-{i}"), Some(&format!("session-{i}")));
            handles.push(tokio::spawn(async move { agent.handle_task(task).await }));
        }
        for handle in handles {
            let result = handle.await.expect("join ok").expect("task ok");
            assert!(result.success);
        }

        let seen = seen.lock().await;
        assert_eq!(seen.len(), 8);
        for (task_id, session_id) in seen.iter() {
            let index = task_id
                .strip_prefix("task-")
                .expect("task id shape")
                .to_string();
            assert_eq!(
                session_id.as_deref(),
                Some(format!("session-{index}").as_str()),
                "task {task_id} observed a session id from another run"
            );
        }
    }
}
