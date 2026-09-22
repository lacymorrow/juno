//! # Timer Tools Module
//!
//! Advanced scheduling and monitoring tools for delayed agent execution and system monitoring.
//! Enables agents to set timers, monitor screen changes, file system events, and application states.
//!
//! ## Core Capabilities:
//! - Simple time-based delays with context restoration
//! - Screen region monitoring for visual changes
//! - File system monitoring (creation, modification, deletion)
//! - Application state monitoring (launch, focus, termination)
//!
//! ## Usage
//! Used by: Game automation, long-running tasks, system monitoring, waiting for external events
//! Registration: Called via `register_timer_tools()` during agent setup

use crate::agent::implementations::tool_provider::LocalToolProvider;
use crate::constants::{agent, error_messages, events};
use crate::state::AppState;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager};
use tokio::fs;
use tokio::sync::Mutex;
use tokio::time::sleep;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

#[cfg(target_os = "macos")]
use computer_use_ai_sdk::platforms::macos::utils as macos_utils;

// Timer state management

/// Represents a scheduled timer task with associated context and configuration.
///
/// Used by: Timer management system, monitoring tools, agent scheduling
/// Contains all information needed to restore agent state when timer expires.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TimerTask {
    /// Unique identifier for this timer task
    pub id: String,
    /// Unix timestamp in seconds when timer should trigger
    pub trigger_time: u64,
    /// JSON context to restore when timer triggers (game state, conversation history, etc.)
    pub context: Value,
    /// Human-readable description of what this timer is for
    pub description: String,
    /// Unix timestamp when timer was created
    pub created_at: u64,
    /// Type and configuration of the timer (simple, screen monitor, etc.)
    pub timer_type: TimerType,
}

/// Defines the different types of timers and their specific configurations.
///
/// Used by: Timer task creation, monitoring system dispatch, timer execution logic
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum TimerType {
    /// Simple time-based delay timer
    Simple,
    /// Screen monitoring with change detection
    ScreenMonitor {
        /// Optional screen region to monitor (full screen if None)
        region: Option<ScreenRegion>,
        /// Percentage change threshold to trigger (0.0-1.0)
        threshold: f32,
        /// How often to check for changes in seconds
        check_interval_seconds: u64,
    },
    /// File system monitoring for various file events
    FileMonitor {
        /// Path to the file to monitor
        file_path: String,
        /// Type of file event to watch for
        monitor_type: FileMonitorType,
    },
    /// Application state monitoring
    ApplicationMonitor {
        /// Name of the application to monitor
        app_name: String,
        /// State change to watch for
        monitor_state: AppMonitorState,
    },
}

/// Defines a rectangular screen region for monitoring.
///
/// Used by: Screen monitoring timers, visual change detection
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ScreenRegion {
    /// X coordinate of top-left corner
    pub x: f64,
    /// Y coordinate of top-left corner
    pub y: f64,
    /// Width of the region
    pub width: f64,
    /// Height of the region
    pub height: f64,
}

/// Types of file system events that can be monitored.
///
/// Used by: File monitoring timers, filesystem change detection
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum FileMonitorType {
    /// File was created
    Created,
    /// File content was modified
    Modified,
    /// File was deleted
    Deleted,
    /// File size changed
    SizeChanged,
}

/// Application state changes that can be monitored.
///
/// Used by: Application monitoring timers, app state tracking
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum AppMonitorState {
    /// Application was launched
    Launched,
    /// Application was terminated
    Terminated,
    /// Application gained focus
    BecameFocused,
    /// Application lost focus
    LostFocus,
}

/// Enhanced timer manager with monitoring capabilities.
///
/// Manages active timers and their associated monitoring tasks.
/// Provides thread-safe access to timer state and cancellation.
///
/// Used by: Agent system for timer lifecycle management, monitoring coordination
#[derive(Debug, Default, Clone)]
pub struct TimerManager {
    /// Map of active timer tasks by ID
    pub active_timers: Arc<Mutex<HashMap<String, TimerTask>>>,
    /// Map of monitoring task handles by timer ID
    pub monitoring_tasks: Arc<Mutex<HashMap<String, tauri::async_runtime::JoinHandle<()>>>>,
}

impl TimerManager {
    /// Creates a new empty timer manager.
    ///
    /// Used by: Agent initialization, app state setup
    ///
    /// # Returns
    /// New `TimerManager` instance with empty timer collections
    pub fn new() -> Self {
        Self {
            active_timers: Arc::new(Mutex::new(HashMap::new())),
            monitoring_tasks: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Adds a timer to the active timers collection.
    ///
    /// Used by: Timer creation functions, scheduler setup
    ///
    /// # Arguments
    /// * `timer` - The TimerTask to add to active collection
    pub async fn add_timer(&self, timer: TimerTask) {
        let mut timers = self.active_timers.lock().await;
        timers.insert(timer.id.clone(), timer);
    }

    /// Removes and returns a timer from the active collection.
    ///
    /// Also cancels any associated monitoring task.
    /// Used by: Timer expiration, timer cancellation, cleanup
    ///
    /// # Arguments
    /// * `timer_id` - ID of the timer to remove
    ///
    /// # Returns
    /// The removed `TimerTask` if it existed, None otherwise
    pub async fn remove_timer(&self, timer_id: &str) -> Option<TimerTask> {
        let mut timers = self.active_timers.lock().await;
        let timer = timers.remove(timer_id);

        // Cancel monitoring task if exists
        let mut monitoring_tasks = self.monitoring_tasks.lock().await;
        if let Some(task_handle) = monitoring_tasks.remove(timer_id) {
            task_handle.abort();
            debug!("Cancelled monitoring task for timer: {}", timer_id);
        }

        timer
    }

    /// Adds a monitoring task handle for cleanup management.
    ///
    /// Used by: Monitoring timer creation, background task tracking
    ///
    /// # Arguments
    /// * `timer_id` - ID of the timer this task belongs to
    /// * `task_handle` - Handle to the background monitoring task
    pub async fn add_monitoring_task(
        &self,
        timer_id: String,
        task_handle: tauri::async_runtime::JoinHandle<()>,
    ) {
        let mut monitoring_tasks = self.monitoring_tasks.lock().await;
        monitoring_tasks.insert(timer_id, task_handle);
    }

    /// Retrieves a timer by ID without removing it.
    ///
    /// Used by: Timer status checking, monitoring loops
    ///
    /// # Arguments
    /// * `timer_id` - ID of the timer to retrieve
    ///
    /// # Returns
    /// Cloned `TimerTask` if found, None otherwise
    pub async fn get_timer(&self, timer_id: &str) -> Option<TimerTask> {
        let timers = self.active_timers.lock().await;
        timers.get(timer_id).cloned()
    }

    /// Returns a list of all currently active timers.
    ///
    /// Used by: Timer listing tool, status reporting, debugging
    ///
    /// # Returns
    /// Vector of all active `TimerTask` instances
    pub async fn list_active_timers(&self) -> Vec<TimerTask> {
        let timers = self.active_timers.lock().await;
        timers.values().cloned().collect()
    }

    /// Gets all expired timers from the active timer collection.
    ///
    /// Used by: Timer expiration checking, cleanup processes
    ///
    /// # Returns
    /// Vector of expired `TimerTask` instances
    pub async fn get_expired_timers(&self) -> Vec<TimerTask> {
        let timers = self.active_timers.lock().await;

        // Get current time, or return empty vector if system time error
        let now = match SystemTime::now().duration_since(UNIX_EPOCH) {
            Ok(duration) => duration.as_secs(),
            Err(e) => {
                warn!("System time error in get_expired_timers: {}", e);
                return Vec::new(); // Return empty vector if we can't get current time
            }
        };

        timers
            .values()
            .filter(|timer| {
                match timer.timer_type {
                    TimerType::Simple => timer.trigger_time <= now,
                    _ => false, // Monitoring timers don't expire by time
                }
            })
            .cloned()
            .collect()
    }
}

// Tool implementations
mod timer_tools_impl {
    use super::*;
    use crate::agent::core::ToolDefinition;

    /// Upper bound, in seconds, on any schedule an agent can ask for (30 days).
    ///
    /// The tool schemas declare `"minimum": 1` and no maximum, and a JSON
    /// Schema `minimum` is advisory — nothing on the request path enforced it.
    /// Two silent failures followed from that:
    ///
    /// - `now + delay_seconds` overflowed. Rust's float-to-int casts saturate,
    ///   so `delay_seconds: 1e30` became `u64::MAX` and the addition panicked
    ///   in a debug build or wrapped in a release build — a wrap producing a
    ///   `trigger_time` in the past, so the timer fired at once.
    /// - `check_interval_seconds: 0` reached `tokio::time::interval`, which
    ///   panics on a zero period and killed the monitoring task outright.
    ///
    /// Thirty days is far longer than any plausible agent timer and leaves no
    /// room for either.
    pub(super) const MAX_SCHEDULE_SECONDS: u64 = 30 * 24 * 60 * 60;

    /// Read a count of **seconds** from an agent-supplied `input[key]`.
    ///
    /// Returns `None` only when the key is absent or is not a number, so the
    /// caller can distinguish "not asked for" from "asked for badly".
    ///
    /// Fractional JSON numbers are accepted and rounded. These sites used
    /// `as_u64()`, which returns `None` for `2.5` — so a model that asked for
    /// a 2.5-second interval silently got the default instead, with nothing
    /// said about it. That is the same class of defect as a wrong unit: the
    /// request was ignored and the log looked fine.
    ///
    /// The result is clamped into `1..=MAX_SCHEDULE_SECONDS`. Saturating casts
    /// put negatives and NaN on `0` and absurd values on `u64::MAX`; the clamp
    /// catches both ends, which is what keeps `now.saturating_add(..)` inside
    /// a sane range and keeps a zero period away from `tokio::time::interval`.
    pub(super) fn schedule_seconds(input: &Value, key: &str) -> Option<u64> {
        let raw = input.get(key)?.as_f64()?;
        Some((raw.round() as u64).clamp(1, MAX_SCHEDULE_SECONDS))
    }

    /// Creates the tool definition for the `set_timer` tool.
    ///
    /// Used by: Tool registration system, agent tool discovery
    /// Creates schema for simple time-based delay timers.
    ///
    /// # Returns
    /// `ToolDefinition` for setting simple delay timers with context
    pub fn set_timer_definition() -> ToolDefinition {
        ToolDefinition {
            name: agent::tool_names::SET_TIMER.to_string(),
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
    pub async fn set_timer_exec(input: Value, app_handle: AppHandle) -> Result<Value, String> {
        let delay_seconds = schedule_seconds(&input, "delay_seconds").ok_or_else(|| {
            error_messages::tool_errors::MISSING_DELAY_SECONDS_PARAMETER.to_string()
        })?;

        let context = input["context"]
            .as_object()
            .ok_or_else(|| error_messages::tool_errors::MISSING_CONTEXT_PARAMETER.to_string())?
            .clone();

        let description = input["description"]
            .as_str()
            .ok_or_else(|| error_messages::tool_errors::MISSING_DESCRIPTION_PARAMETER.to_string())?
            .to_string();

        let timer_id = Uuid::new_v4().to_string();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| {
                error_messages::format_strings::SYSTEM_TIME_ERROR.replace("{}", &e.to_string())
            })?
            .as_secs();
        let trigger_time = now.saturating_add(delay_seconds);

        let timer_task = TimerTask {
            id: timer_id.clone(),
            trigger_time,
            context: Value::Object(context),
            description,
            created_at: now,
            timer_type: TimerType::Simple,
        };

        // Get or create timer manager from app state
        let state = app_handle.state::<AppState>();
        let timer_manager = state.get::<TimerManager>().unwrap_or_else(|| {
            let manager = Arc::new(TimerManager::new());
            let _ = state.insert(manager.clone());
            manager
        });

        let _ = timer_manager.add_timer(timer_task.clone()).await;

        // Start the timer task
        let app_handle_clone = app_handle.clone();
        let timer_manager_clone = timer_manager.clone();
        let timer_id_clone = timer_id.clone();
        tauri::async_runtime::spawn(async move {
            sleep(Duration::from_secs(delay_seconds)).await;

            // Check if timer is still active (might have been cancelled)
            if let Some(expired_timer) = timer_manager_clone.remove_timer(&timer_id_clone).await {
                info!(
                    "{}",
                    error_messages::format_strings::TIMER_EXPIRED_TRIGGERING_RESTART
                        .replace("{}", &timer_id_clone)
                );

                // Emit event to frontend to restart agent with context
                if let Err(e) = app_handle_clone.emit(events::timer::EXPIRED, &expired_timer) {
                    error!(
                        "{}",
                        error_messages::format_strings::FAILED_TO_EMIT_TIMER_EXPIRED_EVENT
                            .replace("{}", &e.to_string())
                    );
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

    /// Creates the tool definition for the `set_screen_monitor` tool.
    ///
    /// Used by: Tool registration system for screen monitoring capabilities
    /// Enables monitoring of screen regions for visual changes.
    ///
    /// # Returns
    /// `ToolDefinition` for screen change monitoring with region and threshold options
    pub fn set_screen_monitor_definition() -> ToolDefinition {
        ToolDefinition {
            name: agent::tool_names::SET_SCREEN_MONITOR.to_string(),
            description: "Sets up screen monitoring that will restart the agent when significant changes are detected in a specified screen region. Useful for monitoring game states, chat applications, or waiting for UI changes. The agent will be restarted when the screen content changes beyond the threshold.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "context": {
                        "type": "object",
                        "description": "Context data to restore when screen changes are detected",
                        "additionalProperties": true
                    },
                    "description": {
                        "type": "string",
                        "description": "Human-readable description of what this monitor is watching for"
                    },
                    "region": {
                        "type": "object",
                        "description": "Screen region to monitor (optional - monitors full screen if not specified)",
                        "properties": {
                            "x": {"type": "number", "description": "X coordinate of top-left corner"},
                            "y": {"type": "number", "description": "Y coordinate of top-left corner"},
                            "width": {"type": "number", "description": "Width of region"},
                            "height": {"type": "number", "description": "Height of region"}
                        }
                    },
                    "threshold": {
                        "type": "number",
                        "description": "Percentage change threshold to trigger (0.0-1.0, default 0.1 = 10%)",
                        "minimum": 0.0,
                        "maximum": 1.0
                    },
                    "check_interval_seconds": {
                        "type": "number",
                        "description": "How often to check for changes in seconds (default 2)",
                        "minimum": 1
                    },
                    "max_duration_seconds": {
                        "type": "number",
                        "description": "Maximum monitoring duration in seconds (optional)",
                        "minimum": 1
                    }
                },
                "required": ["context", "description"]
            }),
            api_type: None,
            beta_flag: None,
        }
    }

    /// Executes the `set_screen_monitor` tool operation (macOS only).
    ///
    /// Sets up continuous screen monitoring that compares screenshots
    /// to detect visual changes and trigger agent restart.
    ///
    /// Used by: Game automation, UI state monitoring, visual change detection
    ///
    /// # Arguments
    /// * `input` - JSON with monitoring configuration
    /// * `app_handle` - Tauri app handle for event emission
    ///
    /// # Returns
    /// Success response with monitor details or error message
    #[cfg(target_os = "macos")]
    pub async fn set_screen_monitor_exec(
        input: Value,
        app_handle: AppHandle,
    ) -> Result<Value, String> {
        let context = input["context"]
            .as_object()
            .ok_or_else(|| error_messages::tool_errors::MISSING_CONTEXT_PARAMETER.to_string())?
            .clone();

        let description = input["description"]
            .as_str()
            .ok_or_else(|| error_messages::tool_errors::MISSING_DESCRIPTION_PARAMETER.to_string())?
            .to_string();

        let region = input["region"].as_object().map(|r| ScreenRegion {
            x: r["x"].as_f64().unwrap_or(0.0),
            y: r["y"].as_f64().unwrap_or(0.0),
            width: r["width"].as_f64().unwrap_or(1920.0),
            height: r["height"].as_f64().unwrap_or(1080.0),
        });

        let threshold = input["threshold"].as_f64().unwrap_or(0.1) as f32;
        let check_interval_seconds =
            schedule_seconds(&input, "check_interval_seconds").unwrap_or(2);
        let max_duration_seconds = schedule_seconds(&input, "max_duration_seconds");

        let timer_id = Uuid::new_v4().to_string();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| {
                error_messages::format_strings::SYSTEM_TIME_ERROR.replace("{}", &e.to_string())
            })?
            .as_secs();

        let timer_task = TimerTask {
            id: timer_id.clone(),
            trigger_time: max_duration_seconds
                .map(|d| now.saturating_add(d))
                .unwrap_or(u64::MAX),
            context: Value::Object(context),
            description: description.clone(),
            created_at: now,
            timer_type: TimerType::ScreenMonitor {
                region,
                threshold,
                check_interval_seconds,
            },
        };

        // Get or create timer manager from app state
        let state = app_handle.state::<AppState>();
        let timer_manager = state.get::<TimerManager>().unwrap_or_else(|| {
            let manager = Arc::new(TimerManager::new());
            let _ = state.insert(manager.clone());
            manager
        });

        let _ = timer_manager.add_timer(timer_task.clone()).await;

        // Take initial screenshot for comparison
        let initial_screenshot = macos_utils::capture_and_encode_screenshot()
            .map_err(|e| format!("Failed to capture initial screenshot: {}", e))?;

        // Start the monitoring task
        let app_handle_clone = app_handle.clone();
        let timer_manager_clone = timer_manager.clone();
        let timer_id_clone = timer_id.clone();
        let _description_clone = description.clone();

        let monitoring_task = tauri::async_runtime::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(check_interval_seconds));
            let mut previous_screenshot = initial_screenshot;
            // Monotonic. This value only ever measures elapsed time, so it
            // must not come from the wall clock: `SystemTime` here meant a
            // backward step made `now - start_time` underflow (a panic in a
            // debug build, a wrap in a release build that read as a huge
            // elapsed time and stopped the monitor at once).
            let monitor_started_at = Instant::now();

            loop {
                interval.tick().await;

                // Check if we've exceeded max duration
                if let Some(max_duration_seconds) = max_duration_seconds {
                    let elapsed_seconds = monitor_started_at.elapsed().as_secs();
                    if elapsed_seconds >= max_duration_seconds {
                        info!(
                            "Screen monitor {} reached max duration, stopping",
                            timer_id_clone
                        );
                        timer_manager_clone.remove_timer(&timer_id_clone).await;
                        break;
                    }
                }

                // Check if timer is still active
                if timer_manager_clone
                    .get_timer(&timer_id_clone)
                    .await
                    .is_none()
                {
                    debug!("Screen monitor {} was cancelled, stopping", timer_id_clone);
                    break;
                }

                // Capture new screenshot
                match macos_utils::capture_and_encode_screenshot() {
                    Ok(current_screenshot) => {
                        // Simple comparison - in a real implementation, you might want
                        // to decode and compare the actual image data
                        let change_detected = current_screenshot != previous_screenshot;

                        if change_detected {
                            info!(
                                "Screen change detected in monitor {}, triggering agent restart",
                                timer_id_clone
                            );

                            if let Some(expired_timer) =
                                timer_manager_clone.remove_timer(&timer_id_clone).await
                            {
                                // Emit event to frontend to restart agent with context
                                if let Err(e) =
                                    app_handle_clone.emit(events::timer::EXPIRED, &expired_timer)
                                {
                                    error!("{}", error_messages::format_strings::FAILED_TO_EMIT_TIMER_EXPIRED_EVENT.replace("{}", &e.to_string()));
                                }
                            }
                            break;
                        }

                        previous_screenshot = current_screenshot;
                    }
                    Err(e) => {
                        error!(
                            "Failed to capture screenshot for monitor {}: {}",
                            timer_id_clone, e
                        );
                        // Continue monitoring despite screenshot errors
                    }
                }
            }
        });

        timer_manager
            .add_monitoring_task(timer_id.clone(), monitoring_task)
            .await;

        Ok(json!({
            "success": true,
            "timer_id": timer_id,
            "message": format!("Screen monitor set up: {}", description),
            "check_interval_seconds": check_interval_seconds,
            "threshold": threshold
        }))
    }

    /// Executes the `set_screen_monitor` tool operation (non-macOS platforms).
    ///
    /// Returns an error indicating screen monitoring is only supported on macOS.
    ///
    /// # Arguments
    /// * `_input` - Unused input (screen monitoring not supported)
    /// * `_app_handle` - Unused app handle
    ///
    /// # Returns
    /// Error message indicating platform limitation
    #[cfg(not(target_os = "macos"))]
    pub async fn set_screen_monitor_exec(
        _input: Value,
        _app_handle: AppHandle,
    ) -> Result<Value, String> {
        Err(error_messages::tool_errors::SCREEN_MONITORING_MACOS_ONLY.to_string())
    }

    /// Creates the tool definition for the `set_file_monitor` tool.
    ///
    /// Used by: Tool registration system for file system monitoring
    /// Enables monitoring of file creation, modification, deletion, and size changes.
    ///
    /// # Returns
    /// `ToolDefinition` for file system event monitoring
    pub fn set_file_monitor_definition() -> ToolDefinition {
        ToolDefinition {
            name: agent::tool_names::SET_FILE_MONITOR.to_string(),
            description: "Sets up file system monitoring that will restart the agent when specified file events occur. Useful for monitoring downloads, log files, or waiting for file creation/modification. The agent will be restarted when the monitored file event occurs.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "file_path": {
                        "type": "string",
                        "description": "Path to the file to monitor"
                    },
                    "monitor_type": {
                        "type": "string",
                        "enum": ["created", "modified", "deleted", "size_changed"],
                        "description": "Type of file event to monitor for"
                    },
                    "context": {
                        "type": "object",
                        "description": "Context data to restore when file event occurs",
                        "additionalProperties": true
                    },
                    "description": {
                        "type": "string",
                        "description": "Human-readable description of what this monitor is watching for"
                    },
                    "check_interval_seconds": {
                        "type": "number",
                        "description": "How often to check for file changes in seconds (default 5)",
                        "minimum": 1
                    },
                    "max_duration_seconds": {
                        "type": "number",
                        "description": "Maximum monitoring duration in seconds (optional)",
                        "minimum": 1
                    }
                },
                "required": ["file_path", "monitor_type", "context", "description"]
            }),
            api_type: None,
            beta_flag: None,
        }
    }

    /// Executes the `set_file_monitor` tool operation.
    ///
    /// Sets up continuous monitoring of a file for specified events
    /// (creation, modification, deletion, size changes).
    ///
    /// Used by: Download monitoring, log file watching, build process tracking
    ///
    /// # Arguments
    /// * `input` - JSON with file path, monitor type, and configuration
    /// * `app_handle` - Tauri app handle for event emission
    ///
    /// # Returns
    /// Success response with monitor details or error message
    pub async fn set_file_monitor_exec(
        input: Value,
        app_handle: AppHandle,
    ) -> Result<Value, String> {
        let file_path = input["file_path"]
            .as_str()
            .ok_or_else(|| "Missing or invalid 'file_path' parameter".to_string())?
            .to_string();

        let monitor_type_str = input["monitor_type"]
            .as_str()
            .ok_or_else(|| "Missing or invalid 'monitor_type' parameter".to_string())?;

        let monitor_type = match monitor_type_str {
            "created" => FileMonitorType::Created,
            "modified" => FileMonitorType::Modified,
            "deleted" => FileMonitorType::Deleted,
            "size_changed" => FileMonitorType::SizeChanged,
            _ => return Err(error_messages::tool_errors::INVALID_MONITOR_TYPE.to_string()),
        };

        let context = input["context"]
            .as_object()
            .ok_or_else(|| error_messages::tool_errors::MISSING_CONTEXT_PARAMETER.to_string())?
            .clone();

        let description = input["description"]
            .as_str()
            .ok_or_else(|| error_messages::tool_errors::MISSING_DESCRIPTION_PARAMETER.to_string())?
            .to_string();

        let check_interval_seconds =
            schedule_seconds(&input, "check_interval_seconds").unwrap_or(5);
        let max_duration_seconds = schedule_seconds(&input, "max_duration_seconds");

        let timer_id = Uuid::new_v4().to_string();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| {
                error_messages::format_strings::SYSTEM_TIME_ERROR.replace("{}", &e.to_string())
            })?
            .as_secs();

        let timer_task = TimerTask {
            id: timer_id.clone(),
            trigger_time: max_duration_seconds
                .map(|d| now.saturating_add(d))
                .unwrap_or(u64::MAX),
            context: Value::Object(context),
            description: description.clone(),
            created_at: now,
            timer_type: TimerType::FileMonitor {
                file_path: file_path.clone(),
                monitor_type: monitor_type.clone(),
            },
        };

        // Get or create timer manager from app state
        let state = app_handle.state::<AppState>();
        let timer_manager = state.get::<TimerManager>().unwrap_or_else(|| {
            let manager = Arc::new(TimerManager::new());
            let _ = state.insert(manager.clone());
            manager
        });

        let _ = timer_manager.add_timer(timer_task.clone()).await;

        // Start the monitoring task
        let app_handle_clone = app_handle.clone();
        let timer_manager_clone = timer_manager.clone();
        let timer_id_clone = timer_id.clone();
        let _description_clone = description.clone();
        let file_path_for_async = file_path.clone();

        // Get initial file state
        let path = PathBuf::from(&file_path_for_async);
        let initial_exists = path.exists();
        let initial_size = if initial_exists {
            fs::metadata(&path).await.map(|m| m.len()).unwrap_or(0)
        } else {
            0
        };

        let monitoring_task = tauri::async_runtime::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(check_interval_seconds));
            let mut last_exists = initial_exists;
            let mut last_size = initial_size;
            // Wall clock, deliberately: this one is compared against a file's
            // modification time further down, which is also wall clock.
            let started_at_unix_seconds = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            // Monotonic, for measuring elapsed time only. Keeping the two
            // apart is the point: the wall clock answers "when", the
            // monotonic clock answers "how long since", and a clock step must
            // not be able to underflow the second question.
            let monitor_started_at = Instant::now();

            loop {
                interval.tick().await;

                // Check if we've exceeded max duration
                if let Some(max_duration_seconds) = max_duration_seconds {
                    let elapsed_seconds = monitor_started_at.elapsed().as_secs();
                    if elapsed_seconds >= max_duration_seconds {
                        info!(
                            "File monitor {} reached max duration, stopping",
                            timer_id_clone
                        );
                        timer_manager_clone.remove_timer(&timer_id_clone).await;
                        break;
                    }
                }

                // Check if timer is still active
                if timer_manager_clone
                    .get_timer(&timer_id_clone)
                    .await
                    .is_none()
                {
                    debug!("File monitor {} was cancelled, stopping", timer_id_clone);
                    break;
                }

                // Check file state
                let current_exists = path.exists();
                let current_size = if current_exists {
                    fs::metadata(&path).await.map(|m| m.len()).unwrap_or(0)
                } else {
                    0
                };

                let event_detected = match monitor_type {
                    FileMonitorType::Created => !last_exists && current_exists,
                    FileMonitorType::Deleted => last_exists && !current_exists,
                    FileMonitorType::Modified => {
                        if !current_exists {
                            false
                        } else {
                            // Check modification time
                            match fs::metadata(&path).await {
                                Ok(metadata) => match metadata.modified() {
                                    Ok(modified_time) => {
                                        let started_at_sys = UNIX_EPOCH
                                            + Duration::from_secs(started_at_unix_seconds);
                                        modified_time
                                            .duration_since(started_at_sys)
                                            .unwrap_or(Duration::ZERO)
                                            < Duration::from_secs(
                                                check_interval_seconds.saturating_add(1),
                                            )
                                    }
                                    Err(_) => false,
                                },
                                Err(_) => false,
                            }
                        }
                    }
                    FileMonitorType::SizeChanged => current_exists && current_size != last_size,
                };

                if event_detected {
                    info!(
                        "File event detected in monitor {}: {:?} for {}",
                        timer_id_clone, monitor_type, file_path_for_async
                    );

                    if let Some(expired_timer) =
                        timer_manager_clone.remove_timer(&timer_id_clone).await
                    {
                        // Emit event to frontend to restart agent with context
                        if let Err(e) =
                            app_handle_clone.emit(events::timer::EXPIRED, &expired_timer)
                        {
                            error!(
                                "{}",
                                error_messages::format_strings::FAILED_TO_EMIT_TIMER_EXPIRED_EVENT
                                    .replace("{}", &e.to_string())
                            );
                        }
                    }
                    break;
                }

                last_exists = current_exists;
                last_size = current_size;
            }
        });

        timer_manager
            .add_monitoring_task(timer_id.clone(), monitoring_task)
            .await;

        Ok(json!({
            "success": true,
            "timer_id": timer_id,
            "message": format!("File monitor set up: {}", description),
            "file_path": file_path,
            "monitor_type": monitor_type_str,
            "check_interval_seconds": check_interval_seconds
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
            name: agent::tool_names::CANCEL_TIMER.to_string(),
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
    pub async fn cancel_timer_exec(input: Value, app_handle: AppHandle) -> Result<Value, String> {
        let timer_id = input["timer_id"]
            .as_str()
            .ok_or_else(|| "Missing or invalid 'timer_id' parameter".to_string())?;

        let state = app_handle.state::<AppState>();
        let timer_manager = state.get::<TimerManager>().ok_or_else(|| {
            error_messages::tool_errors::TIMER_MANAGER_NOT_INITIALIZED.to_string()
        })?;

        if let Some(cancelled_timer) = timer_manager.remove_timer(timer_id).await {
            Ok(json!({
                "success": true,
                "message": format!("Timer {} cancelled", timer_id),
                "cancelled_timer": {
                    "id": cancelled_timer.id,
                    "description": cancelled_timer.description,
                    "trigger_time": cancelled_timer.trigger_time,
                    "timer_type": cancelled_timer.timer_type
                }
            }))
        } else {
            Ok(json!({
                "success": false,
                "message": format!("Timer {} not found or already expired", timer_id)
            }))
        }
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
            name: agent::tool_names::LIST_TIMERS.to_string(),
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
    pub async fn list_timers_exec(_input: Value, app_handle: AppHandle) -> Result<Value, String> {
        let state = app_handle.state::<AppState>();
        let timer_manager = state.get::<TimerManager>().ok_or_else(|| {
            error_messages::tool_errors::TIMER_MANAGER_NOT_INITIALIZED.to_string()
        })?;

        let active_timers = timer_manager.list_active_timers().await;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| {
                error_messages::format_strings::SYSTEM_TIME_ERROR.replace("{}", &e.to_string())
            })?
            .as_secs();

        let timer_info: Vec<Value> = active_timers
            .iter()
            .map(|timer| {
                #[allow(clippy::if_same_then_else)]
                let time_remaining = if timer.trigger_time > now && timer.trigger_time != u64::MAX {
                    timer.trigger_time - now
                } else if timer.trigger_time == u64::MAX {
                    0 // Monitoring timers
                } else {
                    0
                };

                json!({
                    "id": timer.id,
                    "description": timer.description,
                    "trigger_time": timer.trigger_time,
                    "time_remaining_seconds": time_remaining,
                    "created_at": timer.created_at,
                    "timer_type": timer.timer_type
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
            name: agent::tool_names::CHECK_EXPIRED_TIMERS.to_string(),
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
        let timer_manager = state.get::<TimerManager>().ok_or_else(|| {
            error_messages::tool_errors::TIMER_MANAGER_NOT_INITIALIZED.to_string()
        })?;

        let expired_timers = timer_manager.get_expired_timers().await;

        // Remove expired timers from active list
        for timer in &expired_timers {
            let _ = timer_manager.remove_timer(&timer.id).await;
        }

        let expired_info: Vec<Value> = expired_timers
            .iter()
            .map(|timer| {
                json!({
                    "id": timer.id,
                    "description": timer.description,
                    "context": timer.context,
                    "trigger_time": timer.trigger_time,
                    "created_at": timer.created_at,
                    "timer_type": timer.timer_type
                })
            })
            .collect();

        Ok(json!({
            "success": true,
            "expired_timers": expired_info,
            "count": expired_timers.len(),
            "message": if expired_timers.is_empty() {
                error_messages::tool_errors::NO_EXPIRED_TIMERS_FOUND
            } else {
                error_messages::tool_errors::FOUND_EXPIRED_TIMERS_WITH_CONTEXT
            }
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
/// - `set_screen_monitor`: Screen change monitoring (macOS only)
/// - `set_file_monitor`: File system event monitoring
/// - `cancel_timer`: Timer cancellation by ID
/// - `list_timers`: List all active timers with status
/// - `check_expired_timers`: Check for expired timers needing context restoration
pub async fn register_timer_tools(provider: &mut LocalToolProvider, app_handle: AppHandle) {
    // set_timer
    let set_timer_def = timer_tools_impl::set_timer_definition();
    let app_handle_clone1 = app_handle.clone();
    let set_timer_exec = move |input| {
        let handle = app_handle_clone1.clone();
        async move { timer_tools_impl::set_timer_exec(input, handle).await }
    };
    provider
        .register_async_tool(set_timer_def, set_timer_exec)
        .await;

    // set_screen_monitor
    let set_screen_monitor_def = timer_tools_impl::set_screen_monitor_definition();
    let app_handle_clone2 = app_handle.clone();
    let set_screen_monitor_exec = move |input| {
        let handle = app_handle_clone2.clone();
        async move { timer_tools_impl::set_screen_monitor_exec(input, handle).await }
    };
    provider
        .register_async_tool(set_screen_monitor_def, set_screen_monitor_exec)
        .await;

    // set_file_monitor
    let set_file_monitor_def = timer_tools_impl::set_file_monitor_definition();
    let app_handle_clone3 = app_handle.clone();
    let set_file_monitor_exec = move |input| {
        let handle = app_handle_clone3.clone();
        async move { timer_tools_impl::set_file_monitor_exec(input, handle).await }
    };
    provider
        .register_async_tool(set_file_monitor_def, set_file_monitor_exec)
        .await;

    // cancel_timer
    let cancel_timer_def = timer_tools_impl::cancel_timer_definition();
    let app_handle_clone4 = app_handle.clone();
    let cancel_timer_exec = move |input| {
        let handle = app_handle_clone4.clone();
        async move { timer_tools_impl::cancel_timer_exec(input, handle).await }
    };
    provider
        .register_async_tool(cancel_timer_def, cancel_timer_exec)
        .await;

    // list_timers
    let list_timers_def = timer_tools_impl::list_timers_definition();
    let app_handle_clone5 = app_handle.clone();
    let list_timers_exec = move |input| {
        let handle = app_handle_clone5.clone();
        async move { timer_tools_impl::list_timers_exec(input, handle).await }
    };
    provider
        .register_async_tool(list_timers_def, list_timers_exec)
        .await;

    // check_expired_timers
    let check_expired_def = timer_tools_impl::check_expired_timers_definition();
    let app_handle_clone6 = app_handle.clone();
    let check_expired_exec = move |input| {
        let handle = app_handle_clone6.clone();
        async move { timer_tools_impl::check_expired_timers_exec(input, handle).await }
    };
    provider
        .register_async_tool(check_expired_def, check_expired_exec)
        .await;

    info!("Registered enhanced timer tools: set_timer, set_screen_monitor, set_file_monitor, cancel_timer, list_timers, check_expired_timers");
}

/// Every number of seconds these tools take comes from a model, so the reader
/// of `schedule_seconds` is an adversary by default: fractional, negative,
/// zero, absurd and non-numeric all have to land somewhere safe.
#[cfg(test)]
mod schedule_seconds_tests {
    use super::timer_tools_impl::{schedule_seconds, MAX_SCHEDULE_SECONDS};
    use serde_json::json;

    #[test]
    fn a_whole_number_of_seconds_passes_through() {
        assert_eq!(schedule_seconds(&json!({ "s": 30 }), "s"), Some(30));
    }

    #[test]
    fn fractional_seconds_are_rounded_not_discarded() {
        // `as_u64()` returned None for these, so the caller silently fell back
        // to its default and the model's request vanished without a word.
        assert_eq!(schedule_seconds(&json!({ "s": 2.4 }), "s"), Some(2));
        assert_eq!(schedule_seconds(&json!({ "s": 2.6 }), "s"), Some(3));
    }

    #[test]
    fn zero_becomes_one_second() {
        // A zero period panics `tokio::time::interval`, which killed the
        // monitoring task outright.
        assert_eq!(schedule_seconds(&json!({ "s": 0 }), "s"), Some(1));
        assert_eq!(schedule_seconds(&json!({ "s": 0.2 }), "s"), Some(1));
    }

    #[test]
    fn negative_values_become_one_second() {
        assert_eq!(schedule_seconds(&json!({ "s": -5 }), "s"), Some(1));
        assert_eq!(schedule_seconds(&json!({ "s": -1e30 }), "s"), Some(1));
    }

    #[test]
    fn absurd_values_are_capped_rather_than_overflowing_the_deadline() {
        // The saturating float cast puts these on u64::MAX; without the clamp
        // `now + seconds` overflows, panicking in debug and wrapping in
        // release to a deadline in the past.
        assert_eq!(
            schedule_seconds(&json!({ "s": 1e30 }), "s"),
            Some(MAX_SCHEDULE_SECONDS)
        );
        assert_eq!(
            schedule_seconds(&json!({ "s": u64::MAX }), "s"),
            Some(MAX_SCHEDULE_SECONDS)
        );
        assert_eq!(
            schedule_seconds(&json!({ "s": MAX_SCHEDULE_SECONDS + 1 }), "s"),
            Some(MAX_SCHEDULE_SECONDS)
        );
    }

    #[test]
    fn nan_lands_at_the_floor_rather_than_panicking() {
        // serde_json cannot hold NaN, so this arrives as a very large finite
        // number or not at all; the guard is that nothing here can panic.
        assert_eq!(
            schedule_seconds(&json!({ "s": f64::MAX }), "s"),
            Some(MAX_SCHEDULE_SECONDS)
        );
    }

    #[test]
    fn a_missing_or_non_numeric_key_is_none_not_a_default() {
        // `None` has to stay distinguishable from a clamped value, because the
        // caller uses it to tell "not asked for" from "asked for badly".
        assert_eq!(schedule_seconds(&json!({}), "s"), None);
        assert_eq!(schedule_seconds(&json!({ "s": "30" }), "s"), None);
        assert_eq!(schedule_seconds(&json!({ "s": null }), "s"), None);
    }

    #[test]
    fn the_cap_is_expressed_in_the_unit_it_claims() {
        // Thirty days, stated in seconds. If this ever disagrees with the doc
        // comment, that is the `wait` bug again.
        assert_eq!(MAX_SCHEDULE_SECONDS, 2_592_000);
    }
}
