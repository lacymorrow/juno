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

/// Specialized agent for desktop automation and native application interactions
pub struct DesktopAgent {
    status: RwLock<DesktopAgentStatus>,
    app_handle: tauri::AppHandle,
}

#[derive(Debug, Clone)]
struct DesktopAgentStatus {
    is_available: bool,
    current_tasks: usize,
    total_completed: usize,
    successful_tasks: usize,
    total_execution_time: Duration,
}

impl DesktopAgent {
    /// Create a new desktop agent
    pub fn new(app_handle: tauri::AppHandle) -> Result<Self, AgentError> {
        Ok(Self {
            status: RwLock::new(DesktopAgentStatus {
                is_available: true,
                current_tasks: 0,
                total_completed: 0,
                successful_tasks: 0,
                total_execution_time: Duration::new(0, 0),
            }),
            app_handle,
        })
    }

    /// Execute a desktop-related tool call
    async fn execute_desktop_tool(&self, tool_call: &ToolCall) -> Result<ToolResult, AgentError> {
        let state = self.app_handle.state::<AppState>();

        match tool_call.name.as_str() {
            "dev_left_click" | "desktop_click" => {
                let x = tool_call
                    .input
                    .get("x")
                    .and_then(|v| v.as_f64())
                    .ok_or_else(|| {
                        AgentError::InputError("Missing or invalid 'x' coordinate".to_string())
                    })?;
                let y = tool_call
                    .input
                    .get("y")
                    .and_then(|v| v.as_f64())
                    .ok_or_else(|| {
                        AgentError::InputError("Missing or invalid 'y' coordinate".to_string())
                    })?;
                let modifier = tool_call
                    .input
                    .get("modifier")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());

                let result =
                    commands::mouse::dev_left_click(self.app_handle.clone(), state, x, y, modifier)
                        .await;

                match result {
                    Ok(_) => Ok(ToolResult {
                        call_id: tool_call.id.clone(),
                        output: serde_json::json!({"success": true, "action": "left_click", "coordinates": [x, y]}),
                    }),
                    Err(e) => Err(AgentError::ToolError(e)),
                }
            }
            "dev_type_text" | "dev_global_type_text" | "desktop_type" => {
                let text = tool_call
                    .input
                    .get("text")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        AgentError::InputError("Missing or invalid 'text' parameter".to_string())
                    })?;

                let result =
                    commands::dev::dev_type_text(text.to_string(), self.app_handle.clone(), state)
                        .await;

                match result {
                    Ok(_) => Ok(ToolResult {
                        call_id: tool_call.id.clone(),
                        output: serde_json::json!({"success": true, "action": "type_text", "text": text}),
                    }),
                    Err(e) => Err(AgentError::ToolError(e)),
                }
            }
            "dev_press_key" => {
                let key = tool_call
                    .input
                    .get("key")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        AgentError::InputError("Missing or invalid 'key' parameter".to_string())
                    })?;
                let modifier = tool_call
                    .input
                    .get("modifier")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());

                let result = commands::dev::dev_press_key(
                    key.to_string(),
                    modifier,
                    self.app_handle.clone(),
                    state,
                )
                .await;

                match result {
                    Ok(_) => Ok(ToolResult {
                        call_id: tool_call.id.clone(),
                        output: serde_json::json!({"success": true, "action": "press_key", "key": key}),
                    }),
                    Err(e) => Err(AgentError::ToolError(e)),
                }
            }
            "dev_open_application" | "desktop_open_app" => {
                let app_name = tool_call
                    .input
                    .get("app_name")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        AgentError::InputError(
                            "Missing or invalid 'app_name' parameter".to_string(),
                        )
                    })?;

                let result =
                    commands::app_url::dev_open_application(app_name.to_string(), state, self.app_handle.clone()).await;

                match result {
                    Ok(_) => Ok(ToolResult {
                        call_id: tool_call.id.clone(),
                        output: serde_json::json!({"success": true, "action": "open_application", "app_name": app_name}),
                    }),
                    Err(e) => Err(AgentError::ToolError(e)),
                }
            }
            "dev_focus_window" | "desktop_focus_window" => {
                let window_id = tool_call
                    .input
                    .get("window_id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        AgentError::InputError(
                            "Missing or invalid 'window_id' parameter".to_string(),
                        )
                    })?;

                let result = commands::window::dev_focus_window(
                    self.app_handle.clone(),
                    state,
                    window_id.to_string(),
                )
                .await;

                match result {
                    Ok(_) => Ok(ToolResult {
                        call_id: tool_call.id.clone(),
                        output: serde_json::json!({"success": true, "action": "focus_window", "window_id": window_id}),
                    }),
                    Err(e) => Err(AgentError::ToolError(e)),
                }
            }
            "dev_scroll_window" | "desktop_scroll" => {
                let direction = tool_call
                    .input
                    .get("direction")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        AgentError::InputError(
                            "Missing or invalid 'direction' parameter".to_string(),
                        )
                    })?;
                let scroll_amount = tool_call
                    .input
                    .get("scroll_amount")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(3.0);
                let x = tool_call.input.get("x").and_then(|v| v.as_f64());
                let y = tool_call.input.get("y").and_then(|v| v.as_f64());

                let result = commands::window::dev_scroll_window(
                    self.app_handle.clone(),
                    state,
                    direction.to_string(),
                    scroll_amount,
                    x,
                    y,
                )
                .await;

                match result {
                    Ok(_) => Ok(ToolResult {
                        call_id: tool_call.id.clone(),
                        output: serde_json::json!({"success": true, "action": "scroll", "direction": direction, "scroll_amount": scroll_amount}),
                    }),
                    Err(e) => Err(AgentError::ToolError(e)),
                }
            }
            "capture_screenshot_command" | "desktop_screenshot" => {
                let result =
                    commands::core::capture_screenshot_command(self.app_handle.clone()).await;

                match result {
                    Ok(screenshot_data) => Ok(ToolResult {
                        call_id: tool_call.id.clone(),
                        output: serde_json::json!({"success": true, "action": "screenshot", "data": screenshot_data}),
                    }),
                    Err(e) => Err(AgentError::ToolError(e)),
                }
            }
            "dev_get_clipboard" => {
                let result = commands::core::dev_get_clipboard(state).await;

                match result {
                    Ok(clipboard_content) => Ok(ToolResult {
                        call_id: tool_call.id.clone(),
                        output: serde_json::json!({"success": true, "action": "get_clipboard", "content": clipboard_content}),
                    }),
                    Err(e) => Err(AgentError::ToolError(e)),
                }
            }
            "dev_set_clipboard" => {
                let content = tool_call
                    .input
                    .get("content")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        AgentError::InputError("Missing or invalid 'content' parameter".to_string())
                    })?;

                let result = commands::core::dev_set_clipboard(content.to_string(), state).await;

                match result {
                    Ok(_) => Ok(ToolResult {
                        call_id: tool_call.id.clone(),
                        output: serde_json::json!({"success": true, "action": "set_clipboard", "content": content}),
                    }),
                    Err(e) => Err(AgentError::ToolError(e)),
                }
            }
            "dev_right_click" => {
                let x = tool_call
                    .input
                    .get("x")
                    .and_then(|v| v.as_f64())
                    .ok_or_else(|| {
                        AgentError::InputError("Missing or invalid 'x' coordinate".to_string())
                    })?;
                let y = tool_call
                    .input
                    .get("y")
                    .and_then(|v| v.as_f64())
                    .ok_or_else(|| {
                        AgentError::InputError("Missing or invalid 'y' coordinate".to_string())
                    })?;
                let modifier = tool_call
                    .input
                    .get("modifier")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());

                let result = commands::mouse::dev_right_click(
                    self.app_handle.clone(),
                    state,
                    x,
                    y,
                    modifier,
                )
                .await;

                match result {
                    Ok(_) => Ok(ToolResult {
                        call_id: tool_call.id.clone(),
                        output: serde_json::json!({"success": true, "action": "right_click", "coordinates": [x, y]}),
                    }),
                    Err(e) => Err(AgentError::ToolError(e)),
                }
            }
            "dev_double_click" => {
                let x = tool_call
                    .input
                    .get("x")
                    .and_then(|v| v.as_f64())
                    .ok_or_else(|| {
                        AgentError::InputError("Missing or invalid 'x' coordinate".to_string())
                    })?;
                let y = tool_call
                    .input
                    .get("y")
                    .and_then(|v| v.as_f64())
                    .ok_or_else(|| {
                        AgentError::InputError("Missing or invalid 'y' coordinate".to_string())
                    })?;
                let modifier = tool_call
                    .input
                    .get("modifier")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());

                let result = commands::mouse::dev_double_click(
                    self.app_handle.clone(),
                    state,
                    x,
                    y,
                    modifier,
                )
                .await;

                match result {
                    Ok(_) => Ok(ToolResult {
                        call_id: tool_call.id.clone(),
                        output: serde_json::json!({"success": true, "action": "double_click", "coordinates": [x, y]}),
                    }),
                    Err(e) => Err(AgentError::ToolError(e)),
                }
            }
            "dev_get_window_list" => {
                let result =
                    commands::window::dev_get_window_list(self.app_handle.clone(), state).await;

                match result {
                    Ok(windows) => Ok(ToolResult {
                        call_id: tool_call.id.clone(),
                        output: serde_json::json!({"success": true, "action": "get_window_list", "windows": windows}),
                    }),
                    Err(e) => Err(AgentError::ToolError(e)),
                }
            }
            "dev_find_element_by_selector" => {
                let selector = tool_call
                    .input
                    .get("selector")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        AgentError::InputError(
                            "Missing or invalid 'selector' parameter".to_string(),
                        )
                    })?;

                let result =
                    commands::element::dev_find_element_by_selector(selector.to_string(), state)
                        .await;

                match result {
                    Ok(element_info) => Ok(ToolResult {
                        call_id: tool_call.id.clone(),
                        output: serde_json::json!({"success": true, "action": "find_element", "selector": selector, "element": element_info}),
                    }),
                    Err(e) => Err(AgentError::ToolError(e)),
                }
            }
            _ => Err(AgentError::ToolNotFound(tool_call.name.clone())),
        }
    }
}

#[async_trait]
impl SpecializedAgent for DesktopAgent {
    fn agent_type(&self) -> AgentType {
        AgentType::Desktop
    }

    fn get_capabilities(&self) -> Vec<AgentCapability> {
        vec![
            AgentCapability {
                name: "Mouse Control".to_string(),
                description: "Control mouse movements, clicks, and interactions".to_string(),
                tool_patterns: vec![
                    "click".to_string(),
                    "mouse".to_string(),
                    "dev_click".to_string(),
                    "dev_mouse".to_string(),
                ],
                confidence: 0.95,
            },
            AgentCapability {
                name: "Keyboard Control".to_string(),
                description: "Type text, press keys, handle keyboard interactions".to_string(),
                tool_patterns: vec![
                    "type".to_string(),
                    "key".to_string(),
                    "dev_type".to_string(),
                    "dev_key".to_string(),
                ],
                confidence: 0.95,
            },
            AgentCapability {
                name: "Application Management".to_string(),
                description: "Open, focus, and manage native applications".to_string(),
                tool_patterns: vec![
                    "app".to_string(),
                    "application".to_string(),
                    "dev_open".to_string(),
                ],
                confidence: 0.90,
            },
            AgentCapability {
                name: "Window Management".to_string(),
                description: "Focus windows, get window information, manage window state"
                    .to_string(),
                tool_patterns: vec![
                    "window".to_string(),
                    "focus".to_string(),
                    "dev_window".to_string(),
                ],
                confidence: 0.85,
            },
            AgentCapability {
                name: "Screen Interaction".to_string(),
                description: "Take screenshots, scroll, interact with screen elements".to_string(),
                tool_patterns: vec![
                    "screenshot".to_string(),
                    "scroll".to_string(),
                    "element".to_string(),
                ],
                confidence: 0.85,
            },
            AgentCapability {
                name: "Clipboard Operations".to_string(),
                description: "Read from and write to system clipboard".to_string(),
                tool_patterns: vec!["clipboard".to_string(), "dev_clipboard".to_string()],
                confidence: 0.90,
            },
        ]
    }

    async fn can_handle_task(&self, task: &Task) -> bool {
        // Use ToolMappingService instead of string matching
        for tool_call in &task.tool_calls {
            if ToolMappingService::is_tool_in_category(&tool_call.name, &ToolCategory::Desktop)
                || ToolMappingService::is_tool_in_category(
                    &tool_call.name,
                    &ToolCategory::AnthropicComputerUse,
                )
            {
                return true;
            }
        }

        // Use ToolMappingService for task description analysis
        let mapping_agent_type = ToolMappingService::analyze_user_intent(&task.description);
        matches!(
            mapping_agent_type,
            crate::agent::tools::tool_mapping::AgentType::DesktopExpert
        )
    }

    async fn handle_task(&self, task: Task) -> Result<TaskResult, AgentError> {
        let start_time = Instant::now();

        // Update status - task started
        {
            let mut status = self.status.write().await;
            status.current_tasks += 1;
            status.is_available = status.current_tasks < 5; // Allow up to 5 concurrent tasks
        }

        let mut results = Vec::new();
        let mut has_error = false;
        let mut error_message = None;

        // Execute all tool calls in the task
        for tool_call in &task.tool_calls {
            match self.execute_desktop_tool(tool_call).await {
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
            agent_type: AgentType::Desktop,
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

impl DesktopAgent {
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
