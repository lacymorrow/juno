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
use crate::state::AppState;

/// Specialized agent for browser automation and web interactions
pub struct BrowserAgent {
    status: RwLock<BrowserAgentStatus>,
    app_handle: tauri::AppHandle,
}

#[derive(Debug, Clone)]
struct BrowserAgentStatus {
    is_available: bool,
    current_tasks: usize,
    total_completed: usize,
    successful_tasks: usize,
    total_execution_time: Duration,
}

impl BrowserAgent {
    /// Create a new browser agent
    pub fn new(app_handle: tauri::AppHandle) -> Result<Self, AgentError> {
        Ok(Self {
            status: RwLock::new(BrowserAgentStatus {
                is_available: true,
                current_tasks: 0,
                total_completed: 0,
                successful_tasks: 0,
                total_execution_time: Duration::new(0, 0),
            }),
            app_handle,
        })
    }

    /// Execute a browser-related tool call
    async fn execute_browser_tool(&self, tool_call: &ToolCall) -> Result<ToolResult, AgentError> {
        let state = self.app_handle.state::<AppState>();

        // Get or initialize browser controller
        let browser_controller = state.get_or_init_browser_controller().await.map_err(|e| {
            AgentError::ToolError(format!("Failed to initialize browser controller: {}", e))
        })?;

        match tool_call.name.as_str() {
            "browser_navigate" => {
                browser_controller.navigate(&tool_call.input).await
            }
            "browser_click" | "browser_type" | "browser_interact" => {
                browser_controller.interact(&tool_call.input).await
            }
            "browser_screenshot" => {
                browser_controller.screenshot(&tool_call.input).await
            }
            "browser_extract_content" => {
                browser_controller.extract_content(&tool_call.input).await
            }
            "browser_get_current_url" => {
                browser_controller.get_current_url(&tool_call.input).await
            }
            _ => Err(AgentError::ToolNotFound(tool_call.name.clone())),
        }
    }
}

#[async_trait]
impl SpecializedAgent for BrowserAgent {
    fn agent_type(&self) -> AgentType {
        AgentType::Browser
    }

    fn get_capabilities(&self) -> Vec<AgentCapability> {
        vec![
            AgentCapability {
                name: "Web Navigation".to_string(),
                description: "Navigate to URLs, handle browser navigation".to_string(),
                tool_patterns: vec![
                    "browser_navigate".to_string(),
                    "navigate".to_string(),
                    "url".to_string(),
                ],
                confidence: 0.95,
            },
            AgentCapability {
                name: "Element Interaction".to_string(),
                description: "Click elements, type text, interact with web elements".to_string(),
                tool_patterns: vec![
                    "browser_click".to_string(),
                    "browser_type".to_string(),
                    "click".to_string(),
                    "type".to_string(),
                ],
                confidence: 0.90,
            },
            AgentCapability {
                name: "Content Extraction".to_string(),
                description: "Extract text, data, and content from web pages".to_string(),
                tool_patterns: vec![
                    "browser_extract".to_string(),
                    "extract".to_string(),
                    "content".to_string(),
                ],
                confidence: 0.85,
            },
            AgentCapability {
                name: "Web Screenshots".to_string(),
                description: "Capture screenshots of web pages and elements".to_string(),
                tool_patterns: vec!["browser_screenshot".to_string(), "screenshot".to_string()],
                confidence: 0.90,
            },
            AgentCapability {
                name: "Form Automation".to_string(),
                description: "Fill forms, submit data, handle web forms".to_string(),
                tool_patterns: vec![
                    "browser_form".to_string(),
                    "form".to_string(),
                    "submit".to_string(),
                ],
                confidence: 0.80,
            },
        ]
    }

    async fn can_handle_task(&self, task: &Task) -> bool {
        // Use ToolMappingService instead of string matching
        for tool_call in &task.tool_calls {
            if ToolMappingService::is_tool_in_category(&tool_call.name, &ToolCategory::Browser) {
                return true;
            }
        }

        // Use ToolMappingService for task description analysis
        let mapping_agent_type = ToolMappingService::analyze_user_intent(&task.description);
        matches!(
            mapping_agent_type,
            crate::agent::tools::tool_mapping::AgentType::BrowserExpert
        )
    }

    async fn handle_task(&self, task: Task) -> Result<TaskResult, AgentError> {
        let start_time = Instant::now();

        // Update status - task started
        {
            let mut status = self.status.write().await;
            status.current_tasks += 1;
            status.is_available = status.current_tasks < 3; // Allow up to 3 concurrent tasks
        }

        let mut results = Vec::new();
        let mut has_error = false;
        let mut error_message = None;

        // Execute all tool calls in the task
        for tool_call in &task.tool_calls {
            match self.execute_browser_tool(tool_call).await {
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
            agent_type: AgentType::Browser,
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

impl BrowserAgent {
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
