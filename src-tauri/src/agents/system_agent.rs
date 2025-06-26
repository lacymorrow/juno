use async_trait::async_trait;
use serde_json::Value;
use std::time::{Duration, Instant};
use tauri::Manager;
use tokio::sync::RwLock;

use super::base_agent::{
    AgentCapability, AgentStatus, AgentType, SpecializedAgent, Task, TaskResult,
};
use crate::agent::core::{AgentError, ToolCall, ToolResult};
use crate::agent::tools::{ToolCategory, ToolMappingService};
use crate::commands;
use crate::state::AppState;

/// Specialized agent for system operations, shell commands, and file management
pub struct SystemAgent {
    status: RwLock<SystemAgentStatus>,
    app_handle: tauri::AppHandle,
}

#[derive(Debug, Clone)]
struct SystemAgentStatus {
    is_available: bool,
    current_tasks: usize,
    total_completed: usize,
    successful_tasks: usize,
    total_execution_time: Duration,
}

impl SystemAgent {
    /// Create a new system agent
    pub fn new(app_handle: tauri::AppHandle) -> Result<Self, AgentError> {
        Ok(Self {
            status: RwLock::new(SystemAgentStatus {
                is_available: true,
                current_tasks: 0,
                total_completed: 0,
                successful_tasks: 0,
                total_execution_time: Duration::new(0, 0),
            }),
            app_handle,
        })
    }

    /// Execute a system-related tool call
    async fn execute_system_tool(&self, tool_call: &ToolCall) -> Result<ToolResult, AgentError> {
        let state = self.app_handle.state::<AppState>();

        match tool_call.name.as_str() {
            "bash_command" | "system_exec" => {
                let command = tool_call
                    .input
                    .get("command")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        AgentError::InputError("Missing or invalid 'command' parameter".to_string())
                    })?;
                let timeout_seconds = tool_call
                    .input
                    .get("timeout_seconds")
                    .and_then(|v| v.as_u64());
                let restart = tool_call.input.get("restart").and_then(|v| v.as_bool());

                let result = commands::shell::bash_command(
                    self.app_handle.clone(),
                    state,
                    command.to_string(),
                    timeout_seconds,
                    restart,
                    Some(true), // Enable debug mode for agent usage
                )
                .await;

                match result {
                    Ok(output) => Ok(ToolResult {
                        call_id: tool_call.id.clone(),
                        output: serde_json::json!({
                            "success": true,
                            "output": output,
                            "command": command
                        }),
                    }),
                    Err(e) => Err(AgentError::ToolError(e)),
                }
            }
            "list_files" | "system_list_files" => {
                let path = tool_call
                    .input
                    .get("path")
                    .and_then(|v| v.as_str())
                    .unwrap_or(".");

                let result = commands::filesystem::list_files(
                    self.app_handle.clone(),
                    state,
                    path.to_string(),
                    Some(true), // Enable debug mode for agent usage
                )
                .await;

                match result {
                    Ok(files_json) => Ok(ToolResult {
                        call_id: tool_call.id.clone(),
                        output: serde_json::json!({
                            "success": true,
                            "files": files_json,
                            "path": path
                        }),
                    }),
                    Err(e) => Err(AgentError::ToolError(e)),
                }
            }
            "get_file_content" | "system_read_file" => {
                let file_path = tool_call
                    .input
                    .get("file_path")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        AgentError::InputError(
                            "Missing or invalid 'file_path' parameter".to_string(),
                        )
                    })?;

                let result = commands::filesystem::get_file_content(
                    self.app_handle.clone(),
                    state,
                    file_path.to_string(),
                    Some(true), // Enable debug mode for agent usage
                )
                .await;

                match result {
                    Ok(content) => Ok(ToolResult {
                        call_id: tool_call.id.clone(),
                        output: serde_json::json!({
                            "success": true,
                            "content": content,
                            "file_path": file_path
                        }),
                    }),
                    Err(e) => Err(AgentError::ToolError(e)),
                }
            }
            "set_file_content" | "system_write_file" => {
                let file_path = tool_call
                    .input
                    .get("file_path")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        AgentError::InputError(
                            "Missing or invalid 'file_path' parameter".to_string(),
                        )
                    })?;
                let content = tool_call
                    .input
                    .get("content")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        AgentError::InputError("Missing or invalid 'content' parameter".to_string())
                    })?;

                let result = commands::filesystem::set_file_content(
                    self.app_handle.clone(),
                    state,
                    file_path.to_string(),
                    content.to_string(),
                    Some(true), // Enable debug mode for agent usage
                )
                .await;

                match result {
                    Ok(_) => Ok(ToolResult {
                        call_id: tool_call.id.clone(),
                        output: serde_json::json!({
                            "success": true,
                            "file_path": file_path,
                            "bytes_written": content.len()
                        }),
                    }),
                    Err(e) => Err(AgentError::ToolError(e)),
                }
            }
            "text_editor_view" => {
                let file_path = tool_call
                    .input
                    .get("file_path")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        AgentError::InputError(
                            "Missing or invalid 'file_path' parameter".to_string(),
                        )
                    })?;

                let result =
                    commands::text_editor::text_editor_view(file_path.to_string()).await;

                match result {
                    Ok(content) => Ok(ToolResult {
                        call_id: tool_call.id.clone(),
                        output: serde_json::json!({
                            "success": true,
                            "action": "view",
                            "file_path": file_path,
                            "content": content
                        }),
                    }),
                    Err(e) => Err(AgentError::ToolError(e)),
                }
            }
            "text_editor_create" => {
                let file_path = tool_call
                    .input
                    .get("file_path")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        AgentError::InputError(
                            "Missing or invalid 'file_path' parameter".to_string(),
                        )
                    })?;
                let file_text = tool_call
                    .input
                    .get("file_text")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                let result = commands::text_editor::text_editor_create(
                    file_path.to_string(),
                    file_text.to_string(),
                    state,
                    self.app_handle.clone(),
                )
                .await;

                match result {
                    Ok(_) => Ok(ToolResult {
                        call_id: tool_call.id.clone(),
                        output: serde_json::json!({
                            "success": true,
                            "action": "create",
                            "file_path": file_path
                        }),
                    }),
                    Err(e) => Err(AgentError::ToolError(e)),
                }
            }
            "text_editor_str_replace" => {
                let file_path = tool_call
                    .input
                    .get("file_path")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        AgentError::InputError(
                            "Missing or invalid 'file_path' parameter".to_string(),
                        )
                    })?;
                let old_str = tool_call
                    .input
                    .get("old_str")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        AgentError::InputError("Missing or invalid 'old_str' parameter".to_string())
                    })?;
                let new_str = tool_call
                    .input
                    .get("new_str")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        AgentError::InputError("Missing or invalid 'new_str' parameter".to_string())
                    })?;

                let result = commands::text_editor::text_editor_str_replace(
                    file_path.to_string(),
                    old_str.to_string(),
                    new_str.to_string(),
                    state,
                    self.app_handle.clone(),
                )
                .await;

                match result {
                    Ok(_) => Ok(ToolResult {
                        call_id: tool_call.id.clone(),
                        output: serde_json::json!({
                            "success": true,
                            "action": "str_replace",
                            "file_path": file_path
                        }),
                    }),
                    Err(e) => Err(AgentError::ToolError(e)),
                }
            }
            _ => Err(AgentError::ToolNotFound(tool_call.name.clone())),
        }
    }
}

#[async_trait]
impl SpecializedAgent for SystemAgent {
    fn agent_type(&self) -> AgentType {
        AgentType::System
    }

    fn get_capabilities(&self) -> Vec<AgentCapability> {
        vec![
            AgentCapability {
                name: "Shell Command Execution".to_string(),
                description: "Execute shell commands and handle command output".to_string(),
                tool_patterns: vec![
                    "bash".to_string(),
                    "command".to_string(),
                    "exec".to_string(),
                ],
                confidence: 0.95,
            },
            AgentCapability {
                name: "File Operations".to_string(),
                description: "Read, write, create, and manage files and directories".to_string(),
                tool_patterns: vec![
                    "file".to_string(),
                    "read".to_string(),
                    "write".to_string(),
                ],
                confidence: 0.90,
            },
            AgentCapability {
                name: "Directory Management".to_string(),
                description: "List directories, navigate file systems, manage folder structures"
                    .to_string(),
                tool_patterns: vec![
                    "list".to_string(),
                    "directory".to_string(),
                ],
                confidence: 0.85,
            },
            AgentCapability {
                name: "Process Management".to_string(),
                description:
                    "Monitor processes, get system information, manage running applications"
                        .to_string(),
                tool_patterns: vec![
                    "process".to_string(),
                    "system".to_string(),
                    "info".to_string(),
                ],
                confidence: 0.80,
            },
            AgentCapability {
                name: "Text Editor Operations".to_string(),
                description: "Create, edit, and modify text files programmatically".to_string(),
                tool_patterns: vec![
                    "editor".to_string(),
                    "text".to_string(),
                ],
                confidence: 0.85,
            },
        ]
    }

    async fn can_handle_task(&self, task: &Task) -> bool {
        // Use ToolMappingService instead of string matching
        for tool_call in &task.tool_calls {
            if ToolMappingService::is_tool_in_category(&tool_call.name, &ToolCategory::Basic) {
                return true;
            }
        }

        // Use ToolMappingService for task description analysis
        let mapping_agent_type = ToolMappingService::analyze_user_intent(&task.description);
        matches!(
            mapping_agent_type,
            crate::agent::tools::tool_mapping::AgentType::CodingExpert
        )
    }

    async fn handle_task(&self, task: Task) -> Result<TaskResult, AgentError> {
        let start_time = Instant::now();

        // Update status - task started
        {
            let mut status = self.status.write().await;
            status.current_tasks += 1;
            status.is_available = status.current_tasks < 10; // Allow up to 10 concurrent tasks
        }

        let mut results = Vec::new();
        let mut has_error = false;
        let mut error_message = None;

        // Execute all tool calls in the task
        for tool_call in &task.tool_calls {
            match self.execute_system_tool(tool_call).await {
                Ok(result) => results.push(result),
                Err(e) => {
                    has_error = true;
                    error_message = Some(e.to_string());
                    break;
                }
            }
        }

        let execution_time = start_time.elapsed();

        // Update status - task completed
        {
            let mut status = self.status.write().await;
            status.current_tasks -= 1;
            status.total_completed += 1;
            if !has_error {
                status.successful_tasks += 1;
            }
            status.total_execution_time += execution_time;
            status.is_available = true;
        }

        let output = if results.is_empty() {
            Value::Null
        } else {
            Value::Array(
                results
                    .into_iter()
                    .map(|r| serde_json::to_value(r).unwrap_or(Value::Null))
                    .collect(),
            )
        };

        Ok(TaskResult {
            task_id: task.id.clone(),
            success: !has_error,
            output,
            error: error_message,
            execution_time,
            agent_type: AgentType::System,
            metadata: serde_json::json!({
                "tool_calls_executed": task.tool_calls.len(),
                "agent_confidence": self.get_confidence_for_task(&task)
            }),
        })
    }

    async fn get_status(&self) -> AgentStatus {
        let status = self.status.read().await;
        let success_rate = if status.total_completed > 0 {
            status.successful_tasks as f32 / status.total_completed as f32
        } else {
            0.0
        };

        let average_execution_time = if status.total_completed > 0 {
            status.total_execution_time / status.total_completed as u32
        } else {
            Duration::new(0, 0)
        };

        AgentStatus {
            agent_type: self.agent_type(),
            is_available: status.is_available,
            current_tasks: status.current_tasks,
            total_completed: status.total_completed,
            success_rate,
            average_execution_time,
            capabilities: self.get_capabilities(),
        }
    }

    async fn is_available(&self) -> bool {
        let status = self.status.read().await;
        status.is_available
    }
}

impl SystemAgent {
    /// Get confidence for handling a specific task
    fn get_confidence_for_task(&self, task: &Task) -> f32 {
        let mut total_confidence = 0.0;
        let mut tool_count = 0;

        for tool_call in &task.tool_calls {
            total_confidence += self.get_confidence_for_tool(&tool_call.name);
            tool_count += 1;
        }

        if tool_count > 0 {
            total_confidence / tool_count as f32
        } else {
            0.0
        }
    }

    /// Get confidence for handling a specific tool call (0.0 to 1.0)
    pub fn get_confidence_for_tool(&self, tool_name: &str) -> f32 {
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
}
