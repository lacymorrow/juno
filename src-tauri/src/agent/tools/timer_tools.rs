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

// Tool implementations
mod timer_tools_impl {
    use super::*;
    use crate::agent::core::ToolDefinition;

    /// Creates the tool definition for the `set_timer` tool.
    ///
    /// Used by: Tool registration system, agent tool discovery
    /// Creates schema for simple time-based delay timers.
    ///
    /// # Returns
    /// `ToolDefinition` for setting simple delay timers with context
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
            api_type: None,
            beta_flag: None,
        }
    }

    /// Executes the `set_timer` tool operation.
    ///
    /// Creates a simple delay timer that will emit a timer-expired event
    /// to restart the agent with saved context after the delay.
    ///
    /// Used by: Game automation, long-running processes, scheduled tasks
    ///
    /// # Arguments
    /// * `input` - JSON with delay_seconds, context, and description
    /// * `app_handle` - Tauri app handle for event emission
    ///
    /// # Returns
    /// Success response with timer details or error message
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
            .map_err(|e| format!("System time error: {}", e))?
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
        let timer_manager = state.get_timer_manager().await;

        timer_manager.add_timer(timer_task.clone()).await;

        // Start the timer task
        let app_handle_clone = app_handle.clone();
        let timer_manager_clone = timer_manager.clone();
        let timer_id_clone = timer_id.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(delay_seconds)).await;

            // Check if timer is still active (might have been cancelled)
            if timer_manager_clone.cancel_timer(&timer_id_clone).await {
                info!("Timer {} expired, triggering agent restart with context", timer_id_clone);

                // Emit event to frontend to restart agent with context
                if let Err(e) = app_handle_clone.emit("timer-expired", &timer_task) {
                    error!("Failed to emit timer-expired event: {}", e);
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

    /// Creates the tool definition for the `cancel_timer` tool.
    ///
    /// Used by: Tool registration system for timer cancellation capabilities
    /// Allows agents to cancel previously set timers when conditions change.
    ///
    /// # Returns
    /// `ToolDefinition` for cancelling active timers by ID
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
            api_type: None,
            beta_flag: None,
        }
    }

    /// Executes the `cancel_timer` tool operation.
    ///
    /// Cancels an active timer by removing it from the manager and stopping
    /// any associated monitoring tasks.
    ///
    /// Used by: Cleanup processes, condition changes, manual timer cancellation
    ///
    /// # Arguments
    /// * `input` - JSON with timer_id to cancel
    /// * `app_handle` - Tauri app handle for state access
    ///
    /// # Returns
    /// Success/failure response with cancellation details
    pub async fn cancel_timer_exec(
        input: Value,
        app_handle: AppHandle,
    ) -> Result<Value, String> {
        let timer_id = input["timer_id"]
            .as_str()
            .ok_or_else(|| "Missing or invalid 'timer_id' parameter".to_string())?;

        let state = app_handle.state::<AppState>();
        let timer_manager = state.get_timer_manager().await;

        let cancelled = timer_manager.cancel_timer(timer_id).await;

        Ok(json!({
            "success": cancelled,
            "message": if cancelled {
                format!("Timer {} cancelled", timer_id)
            } else {
                format!("Timer {} not found or already expired", timer_id)
            }
        }))
    }

    /// Creates the tool definition for the `list_timers` tool.
    ///
    /// Used by: Tool registration system for timer status inspection
    /// Enables agents to view all currently active timers and their details.
    ///
    /// # Returns
    /// `ToolDefinition` for listing all active timers
    pub fn list_timers_definition() -> ToolDefinition {
        ToolDefinition {
            name: "list_timers".to_string(),
            description: "Lists all active timers that are currently scheduled. Useful for checking what timers are running.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false
            }),
            api_type: None,
            beta_flag: None,
        }
    }

    /// Executes the `list_timers` tool operation.
    ///
    /// Returns a comprehensive list of all active timers with their configurations,
    /// remaining time, and current status.
    ///
    /// Used by: Status reporting, debugging, timer management interfaces
    ///
    /// # Arguments
    /// * `_input` - Unused (no parameters required)
    /// * `app_handle` - Tauri app handle for state access
    ///
    /// # Returns
    /// JSON array of all active timers with details and time remaining
    pub async fn list_timers_exec(
        _input: Value,
        app_handle: AppHandle,
    ) -> Result<Value, String> {
        let state = app_handle.state::<AppState>();
        let timer_manager = state.get_timer_manager().await;

        let active_timers = timer_manager.list_timers().await;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| format!("System time error: {}", e))?
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

    /// Creates the tool definition for the `check_expired_timers` tool.
    ///
    /// Used by: Tool registration system for expired timer checking
    /// Critical for agent startup to detect if previous timers have expired
    /// and need context restoration.
    ///
    /// # Returns
    /// `ToolDefinition` for checking and retrieving expired timer contexts
    pub fn check_expired_timers_definition() -> ToolDefinition {
        ToolDefinition {
            name: "check_expired_timers".to_string(),
            description: "Checks for any expired timers and returns their contexts. This is useful during agent startup to see if the agent should resume a previous task.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false
            }),
            api_type: None,
            beta_flag: None,
        }
    }

    /// Executes the `check_expired_timers` tool operation.
    ///
    /// Scans for expired timers and returns their contexts for agent resumption.
    /// Automatically removes expired timers from the active collection.
    ///
    /// Used by: Agent startup, context restoration, expired timer cleanup
    ///
    /// # Arguments
    /// * `_input` - Unused (no parameters required)
    /// * `app_handle` - Tauri app handle for state access
    ///
    /// # Returns
    /// JSON with expired timer details and contexts for restoration
    pub async fn check_expired_timers_exec(
        _input: Value,
        app_handle: AppHandle,
    ) -> Result<Value, String> {
        let state = app_handle.state::<AppState>();
        let timer_manager = state.get_timer_manager().await;

        timer_manager.check_expired(&app_handle).await;

        Ok(json!({
            "success": true,
            "message": "Checked for expired timers"
        }))
    }
}

/// Registers all timer tools with the provider for agent task scheduling and resumption.
///
/// This is the main registration function that makes all timer capabilities available
/// to agents. Includes simple timers, monitoring timers, and timer management tools.
///
/// Used by: Agent initialization in `anthropic.rs`, tool provider setup
///
/// # Arguments
/// * `provider` - Mutable reference to LocalToolProvider for tool registration
/// * `app_handle` - Tauri app handle for state access and event emission
///
/// # Tools Registered
/// - `set_timer`: Simple delay timers with context restoration
/// - `cancel_timer`: Timer cancellation by ID
/// - `list_timers`: List all active timers with status
/// - `check_expired_timers`: Check for expired timers needing context restoration
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
