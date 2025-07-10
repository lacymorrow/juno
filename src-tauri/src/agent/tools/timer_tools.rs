//! # Timer Tools Module - Simplified
//!
//! Simple timer functionality for AI agents. Trust the agent to use timers appropriately.
//! Provides essential delay capabilities with context restoration for long-running tasks.

use crate::agent::implementations::tool_provider::LocalToolProvider;
use crate::agent::core::ToolDefinition;
use crate::state::AppState;
use crate::constants::agent::tool_names;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter};
use tokio::sync::Mutex;
use uuid::Uuid;
use tracing::{info, error};

/// Simple timer task with context restoration
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TimerTask {
    pub id: String,
    pub trigger_time: u64,
    pub context: Value,
    pub description: String,
    pub created_at: u64,
}

/// Simple timer manager
pub struct TimerManager {
    tasks: Arc<Mutex<HashMap<String, TimerTask>>>,
}

impl TimerManager {
    pub fn new() -> Self {
        Self {
            tasks: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Add a simple timer
    pub async fn add_timer(&self, task: TimerTask) {
        let mut tasks = self.tasks.lock().await;
        tasks.insert(task.id.clone(), task);
    }

    /// Cancel a timer
    pub async fn cancel_timer(&self, timer_id: &str) -> bool {
        let mut tasks = self.tasks.lock().await;
        tasks.remove(timer_id).is_some()
    }

    /// List active timers
    pub async fn list_timers(&self) -> Vec<TimerTask> {
        let tasks = self.tasks.lock().await;
        tasks.values().cloned().collect()
    }

    /// Check for expired timers
    pub async fn check_expired(&self, app_handle: &AppHandle) {
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let mut tasks = self.tasks.lock().await;
        let expired: Vec<_> = tasks.iter()
            .filter(|(_, task)| task.trigger_time <= current_time)
            .map(|(id, task)| (id.clone(), task.clone()))
            .collect();

        for (id, task) in expired {
            tasks.remove(&id);

            // Emit timer expired event
            if let Err(e) = app_handle.emit("timer-expired", &task) {
                error!("Failed to emit timer expired event: {}", e);
            }

            info!("Timer expired: {} - {}", task.id, task.description);
        }
    }
}

/// Register simple timer tools
pub async fn register_timer_tools(
    tool_provider: &mut LocalToolProvider,
    _app_state: &AppState,
) {
    let timer_manager = Arc::new(TimerManager::new());

    // Note: Timer manager stored locally - AppState integration can be added later if needed

    // set_timer tool
    let set_timer_def = ToolDefinition {
        name: "set_timer".to_string(),
        description: "Set a simple timer with context restoration. Trust the agent to use appropriate delays.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "delay_seconds": {
                    "type": "number",
                    "description": "Number of seconds to wait"
                },
                "description": {
                    "type": "string",
                    "description": "What this timer is for"
                },
                "context": {
                    "type": "object",
                    "description": "Context to restore when timer triggers"
                }
            },
            "required": ["delay_seconds", "description"]
        }),
        api_type: None,
        beta_flag: None,
    };

    let set_timer_executor = {
        let timer_manager = timer_manager.clone();
        move |input: Value| {
            let timer_manager = timer_manager.clone();
            async move {
                let delay_seconds = input["delay_seconds"].as_f64()
                    .ok_or("Missing delay_seconds")?;
                let description = input["description"].as_str()
                    .ok_or("Missing description")?;
                let context = input.get("context").cloned()
                    .unwrap_or(json!({}));

                let timer_id = Uuid::new_v4().to_string();
                let current_time = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs();

                let task = TimerTask {
                    id: timer_id.clone(),
                    trigger_time: current_time + delay_seconds as u64,
                    context,
                    description: description.to_string(),
                    created_at: current_time,
                };

                timer_manager.add_timer(task).await;

                Ok(json!({
                    "success": true,
                    "timer_id": timer_id,
                    "message": format!("Timer set for {} seconds", delay_seconds)
                }))
            }
        }
    };

    tool_provider.register_async_tool(set_timer_def, set_timer_executor).await;

    // cancel_timer tool
    let cancel_timer_def = ToolDefinition {
        name: tool_names::CANCEL_TIMER.to_string(),
        description: "Cancel an active timer by ID".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "timer_id": {
                    "type": "string",
                    "description": "ID of timer to cancel"
                }
            },
            "required": ["timer_id"]
        }),
        api_type: None,
        beta_flag: None,
    };

    let cancel_timer_executor = {
        let timer_manager = timer_manager.clone();
        move |input: Value| {
            let timer_manager = timer_manager.clone();
            async move {
                let timer_id = input["timer_id"].as_str()
                    .ok_or("Missing timer_id")?;

                let cancelled = timer_manager.cancel_timer(timer_id).await;

                Ok(json!({
                    "success": cancelled,
                    "message": if cancelled {
                        "Timer cancelled"
                    } else {
                        "Timer not found"
                    }
                }))
            }
        }
    };

    tool_provider.register_async_tool(cancel_timer_def, cancel_timer_executor).await;

    // list_timers tool
    let list_timers_def = ToolDefinition {
        name: "list_timers".to_string(),
        description: "List all active timers".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {}
        }),
        api_type: None,
        beta_flag: None,
    };

    let list_timers_executor = {
        let timer_manager = timer_manager.clone();
        move |_input: Value| {
            let timer_manager = timer_manager.clone();
            async move {
                let timers = timer_manager.list_timers().await;

                Ok(json!({
                    "success": true,
                    "timers": timers,
                    "count": timers.len()
                }))
            }
        }
    };

    tool_provider.register_async_tool(list_timers_def, list_timers_executor).await;

    // Note: Timer background task can be added later when AppState integration is complete

    info!("Registered 3 simple timer tools");
}
