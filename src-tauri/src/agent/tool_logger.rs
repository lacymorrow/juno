use crate::agent::structs::AgentError;
use crate::state::AppState;
use chrono::{DateTime, Local};
use futures::FutureExt;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::Mutex as TokioMutex;
use tracing::{debug, error, info, warn};

/// Type for tool usage events sent to the frontend
#[derive(Serialize, Clone)]
pub struct ToolUsageEntry {
    timestamp: u64,
    tool: String,
    inputs: Value,
    result: Option<Value>,
    success: bool,
    screenshot_base64: Option<String>, // Optional screenshot data
    show_timestamp: bool, // New field to control timestamp display
    formatted_time: Option<String>, // Pre-formatted time string for consistent display
}

/// Configuration for timestamp grouping (similar to Slack/Apple Messages)
struct TimestampGroupingConfig {
    /// Show timestamp if this many minutes have passed since last entry
    time_threshold_minutes: i64,
    /// Show timestamp if this many events have occurred since last timestamp
    event_threshold: usize,
    /// Always show timestamp on first entry of a session
    show_first_timestamp: bool,
    /// Time format for display (12h or 24h)
    use_24h_format: bool,
}

impl Default for TimestampGroupingConfig {
    fn default() -> Self {
        Self {
            time_threshold_minutes: 5, // Show timestamp every 5 minutes like Slack
            event_threshold: 10,       // Or every 10 events, whichever comes first
            show_first_timestamp: true,
            use_24h_format: false,     // Default to 12h format (3:45 PM vs 15:45)
        }
    }
}

/// Helper function to determine if timestamp should be shown
fn should_show_timestamp(
    current_timestamp: u64,
    last_timestamp_shown: Option<u64>,
    events_since_last_timestamp: usize,
    config: &TimestampGroupingConfig,
) -> bool {
    // Always show first timestamp
    if last_timestamp_shown.is_none() && config.show_first_timestamp {
        return true;
    }

    // Check event threshold
    if events_since_last_timestamp >= config.event_threshold {
        return true;
    }

    // Check time threshold
    if let Some(last_ts) = last_timestamp_shown {
        let time_diff_minutes = (current_timestamp.saturating_sub(last_ts)) / (1000 * 60);
        if time_diff_minutes >= config.time_threshold_minutes as u64 {
            return true;
        }
    }

    false
}

/// Format timestamp for display based on configuration
fn format_timestamp(timestamp_ms: u64, use_24h: bool) -> String {
    let timestamp_secs = timestamp_ms / 1000;
    let dt = match DateTime::from_timestamp(timestamp_secs as i64, 0) {
        Some(utc_dt) => utc_dt.with_timezone(&Local),
        None => return "Invalid time".to_string(),
    };

    if use_24h {
        dt.format("%H:%M").to_string()
    } else {
        dt.format("%l:%M %p").to_string().trim().to_string()
    }
}

/// Enhanced tool usage entry creation with timestamp grouping
fn create_tool_usage_entry(
    timestamp: u64,
    tool: String,
    inputs: Value,
    result: Option<Value>,
    success: bool,
    screenshot_base64: Option<String>,
    last_timestamp_shown: Option<u64>,
    events_since_last_timestamp: usize,
) -> ToolUsageEntry {
    let config = TimestampGroupingConfig::default();
    let show_timestamp = should_show_timestamp(
        timestamp,
        last_timestamp_shown,
        events_since_last_timestamp,
        &config,
    );

    let formatted_time = if show_timestamp {
        Some(format_timestamp(timestamp, config.use_24h_format))
    } else {
        None
    };

    ToolUsageEntry {
        timestamp,
        tool,
        inputs,
        result,
        success,
        screenshot_base64,
        show_timestamp,
        formatted_time,
    }
}

/// Wraps a tool execution with logging and event emission
pub async fn log_tool_execution<F>(
    app_handle: &AppHandle,
    tool_name: &str,
    inputs: Value,
    executor: F,
) -> Result<Value, String>
where
    F: FnOnce(Value) -> Result<Value, String>,
{
    use crate::state::AppState;

    // Record the start time
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    // Log tool invocation with enhanced formatting using new log formatter
    crate::utils::log_formatter::log_tool_start(tool_name, None);

    // Execute the tool
    let result = executor(inputs.clone());

    // Determine if execution was successful
    let success = result.is_ok();

    // Enhanced success/error logging
    if success {
        info!("✅ Tool '{}' completed successfully", tool_name);
    } else if let Err(ref e) = result {
        warn!("❌ Tool '{}' failed: {}", tool_name, e);
    }

    // If this is a screenshot tool, we want to include the screenshot in the event
    let screenshot_base64 = if tool_name == "capture_screenshot" || tool_name == "screenshot" {
        match &result {
            Ok(output) => {
                if let Some(base64) = output.as_str() {
                    info!("📸 Screenshot captured successfully. Including in event.");
                    Some(base64.to_string())
                } else {
                    warn!("Screenshot tool returned non-string result");
                    None
                }
            },
            Err(e) => {
                warn!("Screenshot capture failed: {}", e);
                None
            },
        }
    } else {
        None
    };

    // Get timestamp tracking state from AppState
    let (last_timestamp_shown, events_since_last) = if let Some(state) = app_handle.try_state::<AppState>() {
        let tracker = state.timestamp_tracker.lock().unwrap();
        (tracker.last_timestamp_shown, tracker.events_since_last_timestamp)
    } else {
        warn!("AppState not available for timestamp tracking");
        (None, 0)
    };

    // Create the enhanced tool usage entry with proper timestamp logic
    let entry = create_tool_usage_entry(
        timestamp,
        tool_name.to_string(),
        inputs,
        result.as_ref().ok().cloned(),
        success,
        screenshot_base64,
        last_timestamp_shown,
        events_since_last,
    );

    // Update the timestamp tracker in AppState
    if let Some(state) = app_handle.try_state::<AppState>() {
        let mut tracker = state.timestamp_tracker.lock().unwrap();
        tracker.record_event(timestamp, entry.show_timestamp);
    }

    // Emit the event to the frontend
    if let Some(window) = app_handle.get_window("main") {
        if let Err(e) = window.emit("tool-usage", entry) {
            warn!("Failed to emit tool-usage event: {}", e);
        }
    } else {
        warn!("Main window not found, cannot emit tool-usage event");
    }

    // Return the original result
    result
}

// Add a function to log async tool execution with proper timestamp grouping
pub async fn log_async_tool_execution<F>(
    app_handle: &AppHandle,
    tool_name: &str,
    input: Value,
    execution_future: F,
) -> Result<Value, String>
where
    F: std::future::Future<Output = Result<Value, String>> + Send,
{
    use crate::state::AppState;

    // Record the start time
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    // Log tool invocation with enhanced formatting using new log formatter
    crate::utils::log_formatter::log_tool_start(tool_name, None);

    let result = std::panic::AssertUnwindSafe(execution_future)
        .catch_unwind()
        .await;

    // Determine if execution was successful and extract results
    let (success, final_result, screenshot_base64) = match result {
        Ok(Ok(output)) => {
            let mut final_screenshot_base64: Option<String> = None;

            // Check for screenshot tool names
            if tool_name == "capture_screenshot" || tool_name == "capture_element_screenshot" || tool_name == "browser_screenshot" {
                if let Some(s_val) = output.as_str() {
                    final_screenshot_base64 = Some(s_val.to_string());
                    info!("📸 Screenshot captured successfully. Including in event.");
                } else if let Some(obj) = output.as_object() {
                    // Handle cases where the output might be an object containing the base64 string
                    if let Some(b64_val) = obj.get("base64").and_then(|v| v.as_str()) {
                        final_screenshot_base64 = Some(b64_val.to_string());
                        info!("📸 Screenshot extracted from tool output.");
                    }
                }
            }

            info!("✅ Tool '{}' completed successfully", tool_name);
            (true, Ok(output.clone()), final_screenshot_base64)
        }
        Ok(Err(e)) => {
            warn!("❌ Tool '{}' failed: {}", tool_name, e);
            (false, Err(e.clone()), None)
        }
        Err(panic_payload) => {
            let err_msg = if let Some(s) = panic_payload.downcast_ref::<String>() {
                format!("Tool {} panicked: {}", tool_name, s)
            } else if let Some(s) = panic_payload.downcast_ref::<&str>() {
                format!("Tool {} panicked: {}", tool_name, s)
            } else {
                format!("Tool {} panicked with unknown type.", tool_name)
            };
            error!("{}", err_msg);
            (false, Err(err_msg.clone()), None)
        }
    };

    // Get timestamp tracking state from AppState
    let (last_timestamp_shown, events_since_last) = if let Some(state) = app_handle.try_state::<AppState>() {
        let tracker = state.timestamp_tracker.lock().unwrap();
        (tracker.last_timestamp_shown, tracker.events_since_last_timestamp)
    } else {
        warn!("AppState not available for timestamp tracking");
        (None, 0)
    };

    // Create the enhanced tool usage entry with proper timestamp logic
    let entry = create_tool_usage_entry(
        timestamp,
        tool_name.to_string(),
        input,
        final_result.as_ref().ok().cloned(),
        success,
        screenshot_base64,
        last_timestamp_shown,
        events_since_last,
    );

    // Update the timestamp tracker in AppState
    if let Some(state) = app_handle.try_state::<AppState>() {
        let mut tracker = state.timestamp_tracker.lock().unwrap();
        tracker.record_event(timestamp, entry.show_timestamp);
    }

    // Emit the event to the frontend
    if let Some(window) = app_handle.get_window("main") {
        if let Err(e) = window.emit("tool-usage", entry) {
            warn!("Failed to emit tool-usage event: {}", e);
        }
    } else {
        warn!("Main window not found, cannot emit tool-usage event");
    }

    // Return the original result
    final_result
}

// NEW: Define the structure for our generic agent event
#[derive(Clone, Debug, Serialize)]
struct AgentEvent {
    #[serde(rename = "type")]
    event_type: String, // "thinking", "tool_call_request", "tool_call_result", "screenshot"
    payload: AgentEventPayload,
}

#[derive(Clone, Debug, Serialize)]
#[serde(untagged)] // Allows payload to be one of the variants without a type field in payload itself
enum AgentEventPayload {
    Thinking(ThinkingPayload),
    ToolCallRequest(ToolCallRequestPayload),
    ToolCallResult(ToolCallResultPayload),
    Screenshot(ScreenshotPayload),
    GenericContent(GenericContentPayload),
}

#[derive(Clone, Debug, Serialize)]
struct ThinkingPayload {
    content: String,
}

#[derive(Clone, Debug, Serialize)]
struct ToolCallRequestPayload {
    tool_name: String,
    tool_args: Value, // Keep as Value for flexibility
    content: Option<String>, // Optional descriptive content
    // NEW: Dynamic tool metadata for intelligent notifications
    tool_category: Option<String>, // Tool category for dynamic icon/message selection
    tool_description: Option<String>, // Tool description for context
    notification_level: String, // "silent", "minimal", "standard", "detailed"
    estimated_duration: Option<String>, // "instant", "short", "medium", "long"
}

#[derive(Clone, Debug, Serialize)]
struct ToolCallResultPayload {
    tool_name: String,
    tool_output: Value, // Keep as Value
    success: bool,
    content: Option<String>, // Optional descriptive content
    screenshot_base64: Option<String>, // Optional screenshot from the tool
    // NEW: Additional result metadata
    tool_category: Option<String>, // Tool category for consistent handling
    execution_time_ms: Option<u64>, // Actual execution time for performance tracking
    notification_level: String, // Match the request level
}

#[derive(Clone, Debug, Serialize)]
struct ScreenshotPayload {
    screenshot_base64: String,
    content: Option<String>, // Optional descriptive content like "AI captured a screenshot"
}

#[derive(Clone, Debug, Serialize)]
struct GenericContentPayload {
    content: String,
}

// NEW: Streaming event payloads
#[derive(Clone, Debug, Serialize)]
struct StreamingTextPayload {
    chunk: String,
    message_id: Option<String>, // Optional message ID to track which response this belongs to
}

#[derive(Clone, Debug, Serialize)]
struct StreamStartPayload {
    message_id: String,
}

#[derive(Clone, Debug, Serialize)]
struct StreamEndPayload {
    message_id: String,
    complete_text: String,
}

// Emit an event when a tool is used
// This function would be called from your tool execution logic
fn emit_agent_event(app_handle: &AppHandle, event: AgentEvent) {
    info!("Emitting agent-event: {:?}", event);
    if let Err(e) = app_handle.emit("agent-event", event) {
        warn!("Failed to emit agent-event: {}", e);
    }
}

// Example usage for emitting a thinking event:
pub fn log_thinking(app_handle: &AppHandle, thought: &str) {
    let event = AgentEvent {
        event_type: "thinking".to_string(),
        payload: AgentEventPayload::Thinking(ThinkingPayload {
            content: thought.to_string(),
        }),
    };
    emit_agent_event(app_handle, event);
}

// Example usage for emitting a tool call request:
pub fn log_tool_call_request(app_handle: &AppHandle, tool_name: &str, tool_args: Value, content: Option<String>) {
    let event = AgentEvent {
        event_type: "tool_call_request".to_string(),
        payload: AgentEventPayload::ToolCallRequest(ToolCallRequestPayload {
            tool_name: tool_name.to_string(),
            tool_args,
            content,
            tool_category: None,
            tool_description: None,
            notification_level: "standard".to_string(),
            estimated_duration: None,
        }),
    };
    emit_agent_event(app_handle, event);
}

// NEW: Enhanced tool call request logging with dynamic metadata
pub async fn log_enhanced_tool_call_request(
    app_handle: &AppHandle,
    tool_name: &str,
    tool_args: Value,
    content: Option<String>,
    app_state: Option<&crate::state::AppState>
) {
    let mut tool_metadata = ToolMetadata::determine_for_tool(tool_name, app_state).await;

    let event = AgentEvent {
        event_type: "tool_call_request".to_string(),
        payload: AgentEventPayload::ToolCallRequest(ToolCallRequestPayload {
            tool_name: tool_name.to_string(),
            tool_args,
            content: content.or_else(|| tool_metadata.generate_start_message()),
            tool_category: Some(tool_metadata.category),
            tool_description: tool_metadata.description,
            notification_level: tool_metadata.notification_level,
            estimated_duration: tool_metadata.estimated_duration,
        }),
    };
    emit_agent_event(app_handle, event);
}

// Example usage for emitting a tool call result:
pub fn log_tool_call_result(
    app_handle: &AppHandle,
    tool_name: &str,
    tool_output: Value,
    success: bool,
    content: Option<String>,
    screenshot_base64: Option<String>,
) {
    let event = AgentEvent {
        event_type: "tool_call_result".to_string(),
        payload: AgentEventPayload::ToolCallResult(ToolCallResultPayload {
            tool_name: tool_name.to_string(),
            tool_output,
            success,
            content,
            screenshot_base64,
            tool_category: None,
            execution_time_ms: None,
            notification_level: "standard".to_string(),
        }),
    };
    emit_agent_event(app_handle, event);
}

// NEW: Enhanced tool call result logging with metadata and timing
pub async fn log_enhanced_tool_call_result(
    app_handle: &AppHandle,
    tool_name: &str,
    tool_output: Value,
    success: bool,
    content: Option<String>,
    screenshot_base64: Option<String>,
    execution_time_ms: Option<u64>,
    app_state: Option<&crate::state::AppState>
) {
    let mut tool_metadata = ToolMetadata::determine_for_tool(tool_name, app_state).await;

    let event = AgentEvent {
        event_type: "tool_call_result".to_string(),
        payload: AgentEventPayload::ToolCallResult(ToolCallResultPayload {
            tool_name: tool_name.to_string(),
            tool_output,
            success,
            content: content.or_else(|| tool_metadata.generate_result_message(success, execution_time_ms)),
            screenshot_base64,
            tool_category: Some(tool_metadata.category),
            execution_time_ms,
            notification_level: tool_metadata.notification_level,
        }),
    };
    emit_agent_event(app_handle, event);
}

/// Tool metadata for dynamic notification generation
#[derive(Debug, Clone)]
struct ToolMetadata {
    category: String,
    description: Option<String>,
    notification_level: String,
    estimated_duration: Option<String>,
    icon: String,
    action_verb: String,
}

impl ToolMetadata {
    /// Determine tool metadata dynamically based on tool name and configuration
    async fn determine_for_tool(tool_name: &str, app_state: Option<&crate::state::AppState>) -> Self {
        // Try to get more detailed info from tool configuration if available
        if let Some(app_state) = app_state {
            let config_manager = app_state.get_tool_config_manager().await;
            let config_guard = config_manager.lock().await;
            if let Some(tool_config) = config_guard.get_tool_config(tool_name) {
                return Self::from_tool_config(&tool_config);
            }
        }

        // Fallback to pattern-based detection
        Self::from_tool_name_patterns(tool_name)
    }

    /// Create metadata from tool configuration
    fn from_tool_config(config: &crate::agent::tools::ToolConfig) -> Self {
        use crate::agent::tools::ToolCategory;

        let (icon, action_verb, notification_level, estimated_duration) = match config.category {
            ToolCategory::AnthropicComputerUse => {
                match config.name.as_str() {
                    "screenshot" => ("📸", "Taking screenshot", "standard", Some("instant")),
                    "click" => ("👆", "Clicking", "minimal", Some("instant")),
                    "type" => ("⌨️", "Typing", "minimal", Some("short")),
                    "key" => ("🔤", "Pressing keys", "minimal", Some("instant")),
                    "scroll" => ("📜", "Scrolling", "minimal", Some("instant")),
                    "drag" => ("🖱️", "Dragging", "minimal", Some("short")),
                    "move" => ("↗️", "Moving cursor", "silent", Some("instant")),
                    _ => ("🖥️", "Interacting with screen", "standard", Some("short"))
                }
            },
            ToolCategory::Desktop => ("🖥️", "Controlling desktop", "standard", Some("short")),
            ToolCategory::Browser => ("🌐", "Browser action", "standard", Some("medium")),
            ToolCategory::Timer => ("⏰", "Managing timer", "standard", Some("instant")),
            ToolCategory::Basic => {
                if config.name.contains("file") {
                    ("📁", "File operation", "standard", Some("short"))
                } else if config.name.contains("command") || config.name.contains("shell") {
                    ("⚡", "Running command", "standard", Some("medium"))
                } else {
                    ("🔧", "Basic operation", "standard", Some("short"))
                }
            },
            ToolCategory::MCP => ("🔌", "External tool", "standard", Some("medium")),
        };

        Self {
            category: format!("{:?}", config.category),
            description: config.description.clone(),
            notification_level: notification_level.to_string(),
            estimated_duration: estimated_duration.map(|s| s.to_string()),
            icon: icon.to_string(),
            action_verb: action_verb.to_string(),
        }
    }

    /// Fallback pattern-based detection for tools not in configuration
    fn from_tool_name_patterns(tool_name: &str) -> Self {
        let (icon, action_verb, category, notification_level, estimated_duration) = match tool_name {
            // Screenshot tools - always highly visible
            name if name.contains("screenshot") => ("📸", "Taking screenshot", "Screenshot", "standard", Some("instant")),

            // Mouse and click actions - minimal notifications
            name if name.contains("click") => ("👆", "Clicking", "Mouse", "minimal", Some("instant")),
            name if name.contains("drag") => ("🖱️", "Dragging", "Mouse", "minimal", Some("short")),
            name if name.contains("move") && name.contains("mouse") => ("↗️", "Moving cursor", "Mouse", "silent", Some("instant")),

            // Keyboard actions - minimal notifications
            name if name.contains("type") => ("⌨️", "Typing", "Keyboard", "minimal", Some("short")),
            name if name.contains("key") => ("🔤", "Pressing keys", "Keyboard", "minimal", Some("instant")),

            // File operations - standard notifications
            name if name.contains("file") && name.contains("read") => ("📖", "Reading file", "File", "standard", Some("short")),
            name if name.contains("file") && (name.contains("write") || name.contains("save")) => ("💾", "Writing file", "File", "standard", Some("short")),
            name if name.contains("file") => ("📁", "File operation", "File", "standard", Some("short")),

            // Command execution - detailed notifications
            name if name.contains("command") || name.contains("shell") || name.contains("terminal") => ("⚡", "Running command", "Command", "detailed", Some("medium")),

            // Browser actions
            name if name.contains("browser") || name.contains("navigate") => ("🌐", "Browser action", "Browser", "standard", Some("medium")),

            // Desktop automation
            name if name.contains("desktop") || name.contains("application") => ("🖥️", "Desktop action", "Desktop", "standard", Some("short")),

            // Timer and scheduling
            name if name.contains("timer") => ("⏰", "Timer action", "Timer", "standard", Some("instant")),

            // MCP tools
            name if name.contains("mcp") => ("🔌", "External tool", "MCP", "standard", Some("medium")),

            // Default fallback
            _ => ("🔧", "Tool execution", "General", "standard", Some("short")),
        };

        Self {
            category: category.to_string(),
            description: None,
            notification_level: notification_level.to_string(),
            estimated_duration: estimated_duration.map(|s| s.to_string()),
            icon: icon.to_string(),
            action_verb: action_verb.to_string(),
        }
    }

    /// Generate a start message for notifications
    fn generate_start_message(&self) -> Option<String> {
        match self.notification_level.as_str() {
            "silent" => None,
            "minimal" => Some(format!("{} {}", self.icon, self.action_verb)),
            "standard" => Some(format!("{} {}...", self.icon, self.action_verb)),
            "detailed" => Some(format!("{} {} {}", self.icon, self.action_verb,
                self.description.as_deref().unwrap_or("in progress"))),
            _ => Some(format!("{} {}", self.icon, self.action_verb)),
        }
    }

    /// Generate a result message for notifications
    fn generate_result_message(&self, success: bool, execution_time_ms: Option<u64>) -> Option<String> {
        match self.notification_level.as_str() {
            "silent" => None,
            "minimal" => {
                if success {
                    Some(format!("{} ✅", self.icon))
                } else {
                    Some(format!("{} ❌", self.icon))
                }
            },
            "standard" => {
                let status = if success { "completed" } else { "failed" };
                Some(format!("{} {} {}", self.icon, self.action_verb, status))
            },
            "detailed" => {
                let status = if success { "completed" } else { "failed" };
                let timing = execution_time_ms
                    .map(|ms| format!(" ({}ms)", ms))
                    .unwrap_or_default();
                Some(format!("{} {} {}{}", self.icon, self.action_verb, status, timing))
            },
            _ => {
                let status = if success { "✅" } else { "❌" };
                Some(format!("{} {}", self.icon, status))
            }
        }
    }
}

// OLD ToolUsageEntry - We might adapt this or replace its usage
// #[derive(Clone, Debug, Serialize)]
// pub struct ToolUsageEntry {
//     pub tool: String,
//     pub input: Value,
//     pub output: Option<Value>,
//     pub success: bool,
//     pub error: Option<String>,
//     pub screenshot_base64: Option<String>, // Field for base64 encoded screenshot
//     pub timestamp: String,                 // ISO 8601 timestamp
// }

// Log to a specific window (e.g., a dev tools panel)
// pub fn log_tool_usage_to_window(window: &Window, entry: &ToolUsageEntry) {
//     info!(target: "tool_events", "Logging to window: {:?}, Tool: {}, Success: {}", window.label(), entry.tool, entry.success);
//     // Emit the event to the specific window
//     if let Err(e) = window.emit("tool-usage", entry) {
//         warn!("Failed to emit tool-usage event: {}", e);
//     }
// }

// Log to all windows or globally via AppHandle
// pub fn log_tool_usage_global(app_handle: &AppHandle, entry: &ToolUsageEntry) {
//     info!(target: "tool_events", "Logging globally: Tool: {}, Success: {}", entry.tool, entry.success);
//     // Emit the event globally. This will reach all listeners.
//     // If you only want it in the main chat, you might need to target the main window specifically.
//     // However, for the dev panel, global might be okay, or it could listen to "agent-event" too.

//     // For now, let's assume "tool-usage" is still used by DevToolsPanel or similar.
//     // If DevToolsPanel is updated to listen to "agent-event", this can also change.
//     if let Err(e) = app_handle.emit("tool-usage", entry) {
//         warn!("Failed to emit consolidated tool-usage event: {}", e);
//     }

//     // Example of how you might adapt to the new system if DevToolsPanel also listens to "agent-event"
//     // let agent_event = AgentEvent {
//     //     event_type: if entry.success { "tool_call_result".to_string() } else { "tool_call_error".to_string() }, // Simplified
//     //     payload: AgentEventPayload::ToolCallResult(ToolCallResultPayload { // Or a specific error payload
//     //         tool_name: entry.tool.clone(),
//     //         tool_output: entry.output.clone().unwrap_or(Value::Null),
//     //         success: entry.success,
//     //         content: entry.error.clone(), // Or some other content
//     //         screenshot_base64: entry.screenshot_base64.clone(),
//     //     })
//     // };
//     // emit_agent_event(app_handle, agent_event);
// }

// This function is called by tools to log their usage.
// It now needs to decide whether to log to a specific window or globally.
// For simplicity, let's assume it logs globally using the AppHandle.
// pub fn log_tool_usage(
//     app_handle: &AppHandle, // Use AppHandle for global logging
//     tool_name: &str,
//     input: &Value,
//     output: Option<&Value>,
//     success: bool,
//     error_message: Option<&str>,
//     screenshot_base64: Option<String>, // Added screenshot data
// ) {
//     let entry = ToolUsageEntry::new(
//         tool_name.to_string(),
//         input.clone(),
//         output.cloned(),
//         success,
//         error_message.map(String::from),
//         screenshot_base64, // Pass screenshot data
//     );

//     // Log globally
//     log_tool_usage_global(app_handle, &entry);

//     // If you have a specific dev tools window and want to log there too:
//     // if let Some(dev_window) = app_handle.get_window("devtools") {
//     //     log_tool_usage_to_window(&dev_window, &entry);
//     // } else {
//     //     warn!("DevTools window not found, cannot log tool usage event to it.");
//     // }
// }

pub fn emit_stream_start(app_handle: &AppHandle, message_id: String) {
    let event = AgentEvent {
        event_type: "stream_start".to_string(),
        payload: AgentEventPayload::GenericContent(GenericContentPayload {
            content: format!("Stream started with message ID: {}", message_id),
        }),
    };
    emit_agent_event(app_handle, event);
}

pub fn emit_streaming_text_chunk(app_handle: &AppHandle, text: String, message_id: Option<String>) {
    let event = AgentEvent {
        event_type: "streaming_text_chunk".to_string(),
        payload: AgentEventPayload::GenericContent(GenericContentPayload {
            content: text,
        }),
    };
    emit_agent_event(app_handle, event);
}

pub fn emit_stream_end(app_handle: &AppHandle, message_id: String) {
    let event = AgentEvent {
        event_type: "stream_end".to_string(),
        payload: AgentEventPayload::GenericContent(GenericContentPayload {
            content: format!("Stream ended with message ID: {}", message_id),
        }),
    };
    emit_agent_event(app_handle, event);
}
