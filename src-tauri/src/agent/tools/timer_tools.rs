use crate::agent::implementations::tool_provider::LocalToolProvider;
use crate::state::AppState;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::Mutex;
use tokio::time::sleep;
use uuid::Uuid;

// Timer state management
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TimerTask {
    pub id: String,
    pub trigger_time: u64, // Unix timestamp in seconds
    pub context: Value,    // JSON context to restore when timer triggers
    pub description: String,
    pub created_at: u64,
}

#[derive(Debug, Default, Clone)]
pub struct TimerManager {
    pub active_timers: Arc<Mutex<HashMap<String, TimerTask>>>,
}

impl TimerManager {
    pub fn new() -> Self {
        Self {
            active_timers: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn add_timer(&self, timer: TimerTask) {
        let mut timers = self.active_timers.lock().await;
        timers.insert(timer.id.clone(), timer);
    }

    pub async fn remove_timer(&self, timer_id: &str) -> Option<TimerTask> {
        let mut timers = self.active_timers.lock().await;
        timers.remove(timer_id)
    }

    pub async fn get_timer(&self, timer_id: &str) -> Option<TimerTask> {
        let timers = self.active_timers.lock().await;
        timers.get(timer_id).cloned()
    }

    pub async fn list_active_timers(&self) -> Vec<TimerTask> {
        let timers = self.active_timers.lock().await;
        timers.values().cloned().collect()
    }

    pub async fn get_expired_timers(&self) -> Vec<TimerTask> {
        let timers = self.active_timers.lock().await;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        timers
            .values()
            .filter(|timer| timer.trigger_time <= now)
            .cloned()
            .collect()
    }
}

// Tool implementations
mod timer_tools_impl {
    use super::*;
    use crate::agent::structs::ToolDefinition;

    pub fn set_timer_definition() -> ToolDefinition {
        ToolDefinition {
            name: "set_timer".to_string(),
            description: "Sets a timer that will restart the agent after a specified delay. Useful for long-running tasks like games where the agent needs to wait for external events or take breaks. The agent will be restarted with the saved context when the timer expires.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "delay_seconds": {
                        "type": "number",
                        "description": "Number of seconds to wait before restarting the agent",
                        "minimum": 1
                    },
                    "context": {
                        "type": "object",
                        "description": "Context data to restore when the timer expires (game state, conversation history, etc.)",
                        "additionalProperties": true
                    },
                    "description": {
                        "type": "string",
                        "description": "Human-readable description of what this timer is for"
                    }
                },
                "required": ["delay_seconds", "context", "description"]
            }),
        }
    }

    pub async fn set_timer_exec(
        input: Value,
        app_handle: AppHandle,
    ) -> Result<Value, String> {
        let delay_seconds = input["delay_seconds"]
            .as_f64()
            .ok_or_else(|| "Missing or invalid 'delay_seconds' parameter".to_string())? as u64;

        let context = input["context"]
            .as_object()
            .ok_or_else(|| "Missing or invalid 'context' parameter".to_string())?
            .clone();

        let description = input["description"]
            .as_str()
            .ok_or_else(|| "Missing or invalid 'description' parameter".to_string())?
            .to_string();

        let timer_id = Uuid::new_v4().to_string();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let trigger_time = now + delay_seconds;

        let timer_task = TimerTask {
            id: timer_id.clone(),
            trigger_time,
            context: Value::Object(context),
            description,
            created_at: now,
        };

        // Get or create timer manager from app state
        let state = app_handle.state::<AppState>();
        let timer_manager = state.get::<TimerManager>()
            .unwrap_or_else(|| {
                let manager = Arc::new(TimerManager::new());
                state.insert(manager.clone());
                manager
            });

        timer_manager.add_timer(timer_task.clone()).await;

        // Start the timer task
        let app_handle_clone = app_handle.clone();
        let timer_manager_clone = timer_manager.clone();
        let timer_id_clone = timer_id.clone(); // Clone for use in async task
        tokio::spawn(async move {
            sleep(Duration::from_secs(delay_seconds)).await;

            // Check if timer is still active (might have been cancelled)
            if let Some(expired_timer) = timer_manager_clone.remove_timer(&timer_id_clone).await {
                log::info!("Timer {} expired, triggering agent restart with context", timer_id_clone);

                // Emit event to frontend to restart agent with context
                if let Err(e) = app_handle_clone.emit("timer-expired", &expired_timer) {
                    log::error!("Failed to emit timer-expired event: {}", e);
                }
            }
        });

        Ok(json!({
            "success": true,
            "timer_id": timer_id,
            "trigger_time": trigger_time,
            "message": format!("Timer set for {} seconds from now", delay_seconds)
        }))
    }

    pub fn cancel_timer_definition() -> ToolDefinition {
        ToolDefinition {
            name: "cancel_timer".to_string(),
            description: "Cancels a previously set timer by its ID. Useful if conditions change and the agent no longer needs to restart.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "timer_id": {
                        "type": "string",
                        "description": "The ID of the timer to cancel"
                    }
                },
                "required": ["timer_id"]
            }),
        }
    }

    pub async fn cancel_timer_exec(
        input: Value,
        app_handle: AppHandle,
    ) -> Result<Value, String> {
        let timer_id = input["timer_id"]
            .as_str()
            .ok_or_else(|| "Missing or invalid 'timer_id' parameter".to_string())?;

        let state = app_handle.state::<AppState>();
        let timer_manager = state.get::<TimerManager>()
            .ok_or_else(|| "Timer manager not initialized".to_string())?;

        if let Some(cancelled_timer) = timer_manager.remove_timer(timer_id).await {
            Ok(json!({
                "success": true,
                "message": format!("Timer {} cancelled", timer_id),
                "cancelled_timer": {
                    "id": cancelled_timer.id,
                    "description": cancelled_timer.description,
                    "trigger_time": cancelled_timer.trigger_time
                }
            }))
        } else {
            Ok(json!({
                "success": false,
                "message": format!("Timer {} not found or already expired", timer_id)
            }))
        }
    }

    pub fn list_timers_definition() -> ToolDefinition {
        ToolDefinition {
            name: "list_timers".to_string(),
            description: "Lists all active timers that are currently scheduled. Useful for checking what timers are running.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false
            }),
        }
    }

    pub async fn list_timers_exec(
        _input: Value,
        app_handle: AppHandle,
    ) -> Result<Value, String> {
        let state = app_handle.state::<AppState>();
        let timer_manager = state.get::<TimerManager>()
            .ok_or_else(|| "Timer manager not initialized".to_string())?;

        let active_timers = timer_manager.list_active_timers().await;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let timer_info: Vec<Value> = active_timers
            .iter()
            .map(|timer| {
                let time_remaining = if timer.trigger_time > now {
                    timer.trigger_time - now
                } else {
                    0
                };

                json!({
                    "id": timer.id,
                    "description": timer.description,
                    "trigger_time": timer.trigger_time,
                    "time_remaining_seconds": time_remaining,
                    "created_at": timer.created_at
                })
            })
            .collect();

        Ok(json!({
            "success": true,
            "active_timers": timer_info,
            "count": active_timers.len()
        }))
    }

    pub fn check_expired_timers_definition() -> ToolDefinition {
        ToolDefinition {
            name: "check_expired_timers".to_string(),
            description: "Checks for any expired timers and returns their contexts. This is useful during agent startup to see if the agent should resume a previous task.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false
            }),
        }
    }

    pub async fn check_expired_timers_exec(
        _input: Value,
        app_handle: AppHandle,
    ) -> Result<Value, String> {
        let state = app_handle.state::<AppState>();
        let timer_manager = state.get::<TimerManager>()
            .ok_or_else(|| "Timer manager not initialized".to_string())?;

        let expired_timers = timer_manager.get_expired_timers().await;

        // Remove expired timers from active list
        for timer in &expired_timers {
            timer_manager.remove_timer(&timer.id).await;
        }

        let expired_info: Vec<Value> = expired_timers
            .iter()
            .map(|timer| {
                json!({
                    "id": timer.id,
                    "description": timer.description,
                    "context": timer.context,
                    "trigger_time": timer.trigger_time,
                    "created_at": timer.created_at
                })
            })
            .collect();

        Ok(json!({
            "success": true,
            "expired_timers": expired_info,
            "count": expired_timers.len(),
            "message": if expired_timers.is_empty() {
                "No expired timers found"
            } else {
                "Found expired timers with context to resume"
            }
        }))
    }
}

/// Registers timer tools with the provider for agent task scheduling and resumption.
pub async fn register_timer_tools(
    provider: &mut LocalToolProvider,
    app_handle: AppHandle,
) {
    // set_timer
    let set_timer_def = timer_tools_impl::set_timer_definition();
    let app_handle_clone1 = app_handle.clone();
    let set_timer_exec = move |input| {
        let handle = app_handle_clone1.clone();
        async move {
            timer_tools_impl::set_timer_exec(input, handle).await
        }
    };
    provider.register_async_tool(set_timer_def, set_timer_exec).await;

    // cancel_timer
    let cancel_timer_def = timer_tools_impl::cancel_timer_definition();
    let app_handle_clone2 = app_handle.clone();
    let cancel_timer_exec = move |input| {
        let handle = app_handle_clone2.clone();
        async move {
            timer_tools_impl::cancel_timer_exec(input, handle).await
        }
    };
    provider.register_async_tool(cancel_timer_def, cancel_timer_exec).await;

    // list_timers
    let list_timers_def = timer_tools_impl::list_timers_definition();
    let app_handle_clone3 = app_handle.clone();
    let list_timers_exec = move |input| {
        let handle = app_handle_clone3.clone();
        async move {
            timer_tools_impl::list_timers_exec(input, handle).await
        }
    };
    provider.register_async_tool(list_timers_def, list_timers_exec).await;

    // check_expired_timers
    let check_expired_def = timer_tools_impl::check_expired_timers_definition();
    let app_handle_clone4 = app_handle.clone();
    let check_expired_exec = move |input| {
        let handle = app_handle_clone4.clone();
        async move {
            timer_tools_impl::check_expired_timers_exec(input, handle).await
        }
    };
    provider.register_async_tool(check_expired_def, check_expired_exec).await;

    log::info!("Registered timer tools: set_timer, cancel_timer, list_timers, check_expired_timers");
}
