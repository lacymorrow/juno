use chrono::{DateTime, Local};
use futures::FutureExt;
use serde::Serialize;
use serde_json::Value;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager};
use tracing::{error, info, warn};
use crate::constants::events;

/// Type for tool usage events sent to the frontend
#[derive(Serialize, Clone)]
pub struct ToolUsageEntry {
    timestamp: u64,
    tool: String,
    inputs: Value,
    result: Option<Value>,
    success: bool,
    screenshot_base64: Option<String>, // Optional screenshot data
    show_timestamp: bool,              // New field to control timestamp display
    formatted_time: Option<String>,    // Pre-formatted time string for consistent display
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
            use_24h_format: false, // Default to 12h format (3:45 PM vs 15:45)
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
#[allow(clippy::too_many_arguments)]
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

    // Determine if execution was successful - handle Anthropic Computer Use API error format
    let success = match &result {
        Ok(output) => {
            // Check if this is an Anthropic error response (computer tools)
            !crate::agent::tools::anthropic_computer_use::is_anthropic_error_response(output)
        }
        Err(_) => false, // Traditional error format
    };

    // Enhanced success/error logging
    if success {
        info!("✅ Tool '{}' completed successfully", tool_name);
    } else {
        match &result {
            Ok(output) => {
                // Anthropic error format
                if let Some(error_msg) = crate::agent::tools::anthropic_computer_use::extract_anthropic_error_message(output) {
                    warn!("❌ Tool '{}' failed: {}", tool_name, error_msg);
                } else {
                    warn!("❌ Tool '{}' failed with unknown Anthropic error format", tool_name);
                }
            }
            Err(e) => {
                // Traditional error format
                warn!("❌ Tool '{}' failed: {}", tool_name, e);
            }
        }
    }

    // If this is a screenshot tool, we want to include the screenshot in the event
    // But only if the operation was successful
    let screenshot_base64 = if success && (tool_name == "capture_screenshot" || tool_name == "screenshot") {
        match &result {
            Ok(output) => {
                if let Some(base64) = output.as_str() {
                    info!("📸 Screenshot captured successfully. Including in event.");
                    Some(base64.to_string())
                } else {
                    warn!("Screenshot tool returned non-string result");
                    None
                }
            }
            Err(e) => {
                warn!("Screenshot capture failed: {}", e);
                None
            }
        }
    } else {
        None
    };

    // Get timestamp tracking state from AppState
    let (last_timestamp_shown, events_since_last) =
        if let Some(state) = app_handle.try_state::<AppState>() {
            match state.timestamp_tracker.lock() {
                Ok(tracker) => (
                    tracker.last_timestamp_shown,
                    tracker.events_since_last_timestamp,
                ),
                Err(e) => {
                    warn!("Failed to acquire timestamp tracker lock: {}", e);
                    (None, 0) // Safe fallback
                }
            }
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
        match state.timestamp_tracker.lock() {
            Ok(mut tracker) => {
                tracker.record_event(timestamp, entry.show_timestamp);
            }
            Err(e) => {
                warn!("Failed to update timestamp tracker: {}", e);
                // Continue without updating - not critical for operation
            }
        }
    }

    // Emit the event to the frontend
    if let Some(window) = app_handle.get_window("main") {
        if let Err(e) = window.emit(events::tools::USAGE, entry) {
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
            // Check if this is an Anthropic error response (computer tools)
            let is_anthropic_error = crate::agent::tools::anthropic_computer_use::is_anthropic_error_response(&output);
            let is_success = !is_anthropic_error;

            let mut final_screenshot_base64: Option<String> = None;

            // Extract screenshot only if operation was successful
            if is_success && (tool_name == "capture_screenshot"
                || tool_name == "capture_element_screenshot"
                || tool_name == "browser_screenshot"
                || tool_name == "computer"
            ) {
                if let Some(s_val) = output.as_str() {
                    final_screenshot_base64 = Some(s_val.to_string());
                    info!("📸 Screenshot captured successfully. Including in event.");
                } else if let Some(obj) = output.as_object() {
                    // Handle cases where the output might be an object containing the base64 string
                    // Check multiple possible field names for screenshot data
                    if let Some(b64_val) = obj.get("base64_image").and_then(|v| v.as_str()) {
                        final_screenshot_base64 = Some(b64_val.to_string());
                        info!("📸 Screenshot extracted from tool output (base64_image field).");
                    } else if let Some(b64_val) = obj.get("base64").and_then(|v| v.as_str()) {
                        final_screenshot_base64 = Some(b64_val.to_string());
                        info!("📸 Screenshot extracted from tool output (base64 field).");
                    } else if let Some(b64_val) = obj.get("data").and_then(|v| v.as_str()) {
                        final_screenshot_base64 = Some(b64_val.to_string());
                        info!("📸 Screenshot extracted from tool output (data field).");
                    }
                }
            }

            if is_success {
                info!("✅ Tool '{}' completed successfully", tool_name);
                (true, Ok(output.clone()), final_screenshot_base64)
            } else {
                let error_msg = crate::agent::tools::anthropic_computer_use::extract_anthropic_error_message(&output)
                    .unwrap_or_else(|| "Unknown Anthropic error".to_string());
                warn!("❌ Tool '{}' failed: {}", tool_name, error_msg);
                (false, Err(error_msg), None)
            }
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
    let (last_timestamp_shown, events_since_last) =
        if let Some(state) = app_handle.try_state::<AppState>() {
            match state.timestamp_tracker.lock() {
                Ok(tracker) => (
                    tracker.last_timestamp_shown,
                    tracker.events_since_last_timestamp,
                ),
                Err(e) => {
                    warn!("Failed to acquire timestamp tracker lock: {}", e);
                    (None, 0) // Safe fallback
                }
            }
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
        match state.timestamp_tracker.lock() {
            Ok(mut tracker) => {
                tracker.record_event(timestamp, entry.show_timestamp);
            }
            Err(e) => {
                warn!("Failed to update timestamp tracker: {}", e);
                // Continue without updating - not critical for operation
            }
        }
    }

    // Emit the event to the frontend
    if let Some(window) = app_handle.get_window("main") {
        if let Err(e) = window.emit(events::tools::USAGE, entry) {
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

#[allow(dead_code)]
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
    tool_args: Value,        // Keep as Value for flexibility
    content: Option<String>, // Optional descriptive content
    // NEW: Dynamic tool metadata for intelligent notifications
    tool_category: Option<String>, // Tool category for dynamic icon/message selection
    tool_description: Option<String>, // Tool description for context
    notification_level: String,    // "silent", "minimal", "standard", "detailed"
    estimated_duration: Option<String>, // "instant", "short", "medium", "long"
}

#[derive(Clone, Debug, Serialize)]
struct ToolCallResultPayload {
    tool_name: String,
    tool_output: Value, // Keep as Value
    success: bool,
    content: Option<String>,           // Optional descriptive content
    screenshot_base64: Option<String>, // Optional screenshot from the tool
    // NEW: Additional result metadata
    tool_category: Option<String>, // Tool category for consistent handling
    execution_time_ms: Option<u64>, // Actual execution time for performance tracking
    notification_level: String,    // Match the request level
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
#[allow(dead_code)]
struct StreamingTextPayload {
    chunk: String,
    message_id: Option<String>, // Optional message ID to track which response this belongs to
}

#[derive(Clone, Debug, Serialize)]
#[allow(dead_code)]
struct StreamStartPayload {
    message_id: String,
}

#[derive(Clone, Debug, Serialize)]
#[allow(dead_code)]
struct StreamEndPayload {
    message_id: String,
    complete_text: String,
}

fn emit_agent_event(app_handle: &AppHandle, event: AgentEvent) {
    // Use sanitized version for logging to prevent base64 console spam
    info!(
        "Emitting agent-event: {:?}",
        sanitize_event_for_logging(&event)
    );
    if let Err(e) = app_handle.emit(events::agent::EVENT, event) {
        warn!("Failed to emit agent-event: {}", e);
    }
}

/// Sanitize agent events for logging by removing or truncating base64 data
fn sanitize_event_for_logging(event: &AgentEvent) -> AgentEvent {
    let sanitized_payload = match &event.payload {
        AgentEventPayload::ToolCallResult(payload) => {
            let sanitized_screenshot = payload.screenshot_base64.as_ref().map(|base64| {
                // Truncate base64 data for logging
                if base64.len() > 100 {
                    format!(
                        "{}...[BASE64_SCREENSHOT_TRUNCATED_{}bytes]",
                        &base64[..std::cmp::min(50, base64.len())],
                        base64.len()
                    )
                } else {
                    base64.clone()
                }
            });

            let sanitized_tool_output = sanitize_value_for_logging(&payload.tool_output);

            AgentEventPayload::ToolCallResult(ToolCallResultPayload {
                tool_name: payload.tool_name.clone(),
                tool_output: sanitized_tool_output,
                success: payload.success,
                content: payload.content.clone(),
                screenshot_base64: sanitized_screenshot,
                tool_category: payload.tool_category.clone(),
                execution_time_ms: payload.execution_time_ms,
                notification_level: payload.notification_level.clone(),
            })
        }
        AgentEventPayload::Screenshot(payload) => {
            let sanitized_base64 = if payload.screenshot_base64.len() > 100 {
                format!(
                    "{}...[BASE64_SCREENSHOT_TRUNCATED_{}bytes]",
                    &payload.screenshot_base64
                        [..std::cmp::min(50, payload.screenshot_base64.len())],
                    payload.screenshot_base64.len()
                )
            } else {
                payload.screenshot_base64.clone()
            };

            AgentEventPayload::Screenshot(ScreenshotPayload {
                screenshot_base64: sanitized_base64,
                content: payload.content.clone(),
            })
        }
        AgentEventPayload::ToolCallRequest(payload) => {
            let sanitized_tool_args = sanitize_value_for_logging(&payload.tool_args);

            AgentEventPayload::ToolCallRequest(ToolCallRequestPayload {
                tool_name: payload.tool_name.clone(),
                tool_args: sanitized_tool_args,
                content: payload.content.clone(),
                tool_category: payload.tool_category.clone(),
                tool_description: payload.tool_description.clone(),
                notification_level: payload.notification_level.clone(),
                estimated_duration: payload.estimated_duration.clone(),
            })
        }
        // Other payload types don't contain base64 data, so clone as-is
        other => other.clone(),
    };

    AgentEvent {
        event_type: event.event_type.clone(),
        payload: sanitized_payload,
    }
}

/// Sanitize serde_json::Value for logging by removing or truncating base64 data
fn sanitize_value_for_logging(value: &Value) -> Value {
    match value {
        Value::String(s) => {
            // Check if this looks like base64 data (long string with base64 characters)
            if s.len() > 100
                && s.chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '/' || c == '=')
            {
                // Truncate base64 data and add indication it was truncated
                Value::String(format!(
                    "{}...[BASE64_DATA_TRUNCATED_{}bytes]",
                    &s[..std::cmp::min(50, s.len())],
                    s.len()
                ))
            } else {
                Value::String(s.clone())
            }
        }
        Value::Object(obj) => {
            let mut sanitized = serde_json::Map::new();
            for (key, val) in obj {
                sanitized.insert(key.clone(), sanitize_value_for_logging(val));
            }
            Value::Object(sanitized)
        }
        Value::Array(arr) => {
            let sanitized: Vec<_> = arr.iter().map(sanitize_value_for_logging).collect();
            Value::Array(sanitized)
        }
        _ => value.clone(),
    }
}

// Example usage for emitting a thinking event (non-streaming, batch):
pub fn log_thinking(app_handle: &AppHandle, thought: &str) {
    let event = AgentEvent {
        event_type: "thinking".to_string(),
        payload: AgentEventPayload::Thinking(ThinkingPayload {
            content: thought.to_string(),
        }),
    };
    emit_agent_event(app_handle, event);
}

/// Emit thinking stream start event - creates a new streaming thinking message
pub fn emit_thinking_start(app_handle: &AppHandle, message_id: String) {
    let event_data = serde_json::json!({
        "message_id": message_id
    });

    if let Err(e) = app_handle.emit(crate::constants::events::streaming::THINKING_START, event_data) {
        warn!("Failed to emit thinking-start event: {}", e);
    }
}

/// Emit thinking stream chunk - appends to the current streaming thinking message
pub fn emit_thinking_chunk(app_handle: &AppHandle, chunk: String, message_id: Option<String>) {
    let event_data = serde_json::json!({
        "chunk": chunk,
        "message_id": message_id
    });

    if let Err(e) = app_handle.emit(crate::constants::events::streaming::THINKING_STREAM, event_data) {
        warn!("Failed to emit thinking-stream event: {}", e);
    }
}

/// Emit thinking stream end event - finalizes the streaming thinking message
pub fn emit_thinking_end(app_handle: &AppHandle, message_id: String, complete_text: String) {
    let event_data = serde_json::json!({
        "message_id": message_id,
        "complete_text": complete_text
    });

    if let Err(e) = app_handle.emit(crate::constants::events::streaming::THINKING_END, event_data) {
        warn!("Failed to emit thinking-end event: {}", e);
    }
}

// Example usage for emitting a tool call request:
pub fn log_tool_call_request(
    app_handle: &AppHandle,
    tool_name: &str,
    tool_args: Value,
    content: Option<String>,
) {
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
    app_state: Option<&crate::state::AppState>,
) {
    let tool_metadata =
        ToolMetadata::determine_for_tool_with_inputs(tool_name, Some(tool_args.clone()), app_state)
            .await;

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
#[allow(clippy::too_many_arguments)]
pub async fn log_enhanced_tool_call_result(
    app_handle: &AppHandle,
    tool_name: &str,
    tool_output: Value,
    success: bool,
    content: Option<String>,
    screenshot_base64: Option<String>,
    execution_time_ms: Option<u64>,
    app_state: Option<&crate::state::AppState>,
) {
    let tool_metadata = ToolMetadata::determine_for_tool(tool_name, app_state).await;

    let event = AgentEvent {
        event_type: "tool_call_result".to_string(),
        payload: AgentEventPayload::ToolCallResult(ToolCallResultPayload {
            tool_name: tool_name.to_string(),
            tool_output,
            success,
            content: content
                .or_else(|| tool_metadata.generate_result_message(success, execution_time_ms)),
            screenshot_base64,
            tool_category: Some(tool_metadata.category),
            execution_time_ms,
            notification_level: tool_metadata.notification_level,
        }),
    };
    emit_agent_event(app_handle, event);
}

// NEW: Enhanced tool call result logging with original inputs for better details
#[allow(clippy::too_many_arguments)]
pub async fn log_enhanced_tool_call_result_with_inputs(
    app_handle: &AppHandle,
    tool_name: &str,
    tool_inputs: Option<Value>,
    tool_output: Value,
    success: bool,
    content: Option<String>,
    screenshot_base64: Option<String>,
    execution_time_ms: Option<u64>,
    app_state: Option<&crate::state::AppState>,
) {
    // Use the original inputs to get detailed metadata for result messages
    let tool_metadata = if let Some(inputs) = tool_inputs {
        ToolMetadata::determine_for_tool_with_inputs(tool_name, Some(inputs), app_state).await
    } else {
        ToolMetadata::determine_for_tool(tool_name, app_state).await
    };

    let event = AgentEvent {
        event_type: "tool_call_result".to_string(),
        payload: AgentEventPayload::ToolCallResult(ToolCallResultPayload {
            tool_name: tool_name.to_string(),
            tool_output,
            success,
            content: content
                .or_else(|| tool_metadata.generate_result_message(success, execution_time_ms)),
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
    // NEW: Store actual tool inputs for detailed messaging
    tool_inputs: Option<Value>,
}

impl ToolMetadata {
    fn normalized_category_from_action(action_verb: &str) -> &'static str {
        let lower_action = action_verb.to_ascii_lowercase();
        if lower_action.contains("typing") || lower_action.contains("pressing key") || lower_action.contains("pressing keys") {
            "Keyboard"
        } else if lower_action.contains("running command") {
            "Command"
        } else if lower_action.contains("file") {
            "File"
        } else if lower_action.contains("browser") {
            "Browser"
        } else if lower_action.contains("timer") {
            "Timer"
        } else if lower_action.contains("desktop") {
            "Desktop"
        } else if lower_action.contains("screen") || lower_action.contains("screenshot") || lower_action.contains("click") || lower_action.contains("scroll") || lower_action.contains("drag") || lower_action.contains("cursor") {
            "Computer Use"
        } else {
            "General"
        }
    }

    /// Determine tool metadata dynamically based on tool name and configuration
    async fn determine_for_tool(
        tool_name: &str,
        app_state: Option<&crate::state::AppState>,
    ) -> Self {
        // Try to get more detailed info from tool configuration if available
        if let Some(app_state) = app_state {
            let config_manager = app_state.get_tool_config_manager().await;
            let config_guard = config_manager.lock().await;
            if let Some(tool_config) = config_guard.get_tool_config(tool_name) {
                return Self::from_tool_config(&tool_config);
            }
        }

        // Tool metadata from name patterns (fallback categorization)
        Self::from_tool_name_patterns(tool_name)
    }

    /// Determine tool metadata with inputs for enhanced detail extraction
    async fn determine_for_tool_with_inputs(
        tool_name: &str,
        tool_inputs: Option<Value>,
        app_state: Option<&crate::state::AppState>,
    ) -> Self {
        let mut metadata = Self::determine_for_tool(tool_name, app_state).await;
        metadata.tool_inputs = tool_inputs;
        metadata
    }

    /// Create tool metadata from a tool configuration
    /// This uses the proper tool configuration system when available
    fn from_tool_config(config: &crate::agent::tools::ToolConfig) -> Self {
        use crate::agent::tools::ToolCategory;

        let (icon, action_verb, notification_level, estimated_duration) = match config.category {
            ToolCategory::AnthropicComputerUse => {
                match config.name.as_str() {
                    "screenshot" => ("📸", "Taking screenshot", "standard", Some("instant")),
                    "click" => ("👆", "Clicking", "minimal", Some("instant")),
                    "type" => ("⌨️", "Typing", "minimal", Some("short")),
                    "key" => ("🔤", "Pressing keys", "standard", Some("instant")), // Changed to standard for key details
                    "scroll" => ("📜", "Scrolling", "minimal", Some("instant")),
                    "drag" => ("🖱️", "Dragging", "minimal", Some("short")),
                    "move" => ("↗️", "Moving cursor", "silent", Some("instant")),
                    _ => ("🖥️", "Interacting with screen", "standard", Some("short")),
                }
            }
            ToolCategory::Desktop => ("🖥️", "Controlling desktop", "standard", Some("short")),
            ToolCategory::Browser => ("🌐", "Browser action", "standard", Some("medium")),
            ToolCategory::Timer => ("⏰", "Managing timer", "standard", Some("instant")),
            ToolCategory::Basic => {
                if config.name.contains("file") {
                    ("📁", "File operation", "standard", Some("short"))
                } else if config.name.contains("command")
                    || config.name.contains("shell")
                    || config.name.contains("bash")
                    || config.name.contains("terminal")
                {
                    ("⚡", "Running command", "detailed", Some("medium")) // Changed to detailed for command details
                } else {
                    ("🔧", "Basic operation", "standard", Some("short"))
                }
            }
            ToolCategory::MCP => ("🔌", "External tool", "standard", Some("medium")),
        };

        let normalized_category = Self::normalized_category_from_action(action_verb);

        Self {
            category: normalized_category.to_string(),
            description: config.description.clone(),
            notification_level: notification_level.to_string(),
            estimated_duration: estimated_duration.map(|s| s.to_string()),
            icon: icon.to_string(),
            action_verb: action_verb.to_string(),
            tool_inputs: None,
        }
    }

    /// Create tool metadata from pattern matching on tool name
    fn from_tool_name_patterns(tool_name: &str) -> Self {
        // Quick exit for empty tool names
        if tool_name.is_empty() {
            return Self {
                category: "General".to_string(),
                description: None,
                notification_level: "standard".to_string(),
                estimated_duration: Some("short".to_string()),
                icon: "🔧".to_string(),
                action_verb: "Tool execution".to_string(),
                tool_inputs: None,
            };
        }

        // Pattern-based categorization for tools without configuration
        let (icon, action_verb, category, notification_level, estimated_duration) = match tool_name
        {
            // Computer use tools - specific patterns for Anthropic Computer Use
            name if name.starts_with("computer/screenshot") => (
                "📸",
                "Taking screenshot",
                "Computer Use",
                "standard",
                Some("instant"),
            ),
            name if name.starts_with("computer/click(") => (
                "👆",
                "Clicking",
                "Computer Use",
                "minimal",
                Some("instant"),
            ),
            name if name.starts_with("computer/right_click(") => (
                "🖱️",
                "Right-clicking",
                "Computer Use",
                "minimal",
                Some("instant"),
            ),
            name if name.starts_with("computer/middle_click(") => (
                "🖱️",
                "Middle-clicking",
                "Computer Use",
                "minimal",
                Some("instant"),
            ),
            name if name.starts_with("computer/double_click(") => (
                "👆",
                "Double-clicking",
                "Computer Use",
                "minimal",
                Some("instant"),
            ),
            name if name.starts_with("computer/triple_click(") => (
                "👆",
                "Triple-clicking",
                "Computer Use",
                "minimal",
                Some("instant"),
            ),
            name if name.starts_with("computer/drag(") => (
                "🖱️",
                "Dragging",
                "Computer Use",
                "minimal",
                Some("short"),
            ),
            name if name.starts_with("computer/move_to(") => (
                "↗️",
                "Moving cursor",
                "Computer Use",
                "silent",
                Some("instant"),
            ),
            name if name.starts_with("computer/mouse_down(") => (
                "🖱️",
                "Mouse down",
                "Computer Use",
                "silent",
                Some("instant"),
            ),
            name if name.starts_with("computer/mouse_up(") => (
                "🖱️",
                "Mouse up",
                "Computer Use",
                "silent",
                Some("instant"),
            ),
            name if name.starts_with("computer/scroll_") => (
                "📜",
                "Scrolling",
                "Computer Use",
                "minimal",
                Some("instant"),
            ),
            name if name.starts_with("computer/type(") => (
                "⌨️",
                "Typing",
                "Computer Use",
                "standard",
                Some("short"),
            ),
            name if name.starts_with("computer/press_key(") => (
                "🔤",
                "Pressing key",
                "Computer Use",
                "standard",
                Some("instant"),
            ),
            name if name.starts_with("computer/hold_key(") => (
                "🔤",
                "Holding key",
                "Computer Use",
                "standard",
                Some("short"),
            ),
            name if name.starts_with("computer/wait(") => (
                "⏳",
                "Waiting",
                "Computer Use",
                "minimal",
                Some("medium"),
            ),
            name if name.starts_with("computer/get_cursor_position") => (
                "📍",
                "Getting cursor position",
                "Computer Use",
                "silent",
                Some("instant"),
            ),
            name if name.starts_with("computer/") => (
                "🖥️",
                "Computer action",
                "Computer Use",
                "standard",
                Some("short"),
            ),

            // General screenshot tools - always highly visible
            name if name.contains("screenshot") => (
                "📸",
                "Taking screenshot",
                "Screenshot",
                "standard",
                Some("instant"),
            ),

            // Mouse and click actions - minimal notifications
            name if name.contains("click") => {
                ("👆", "Clicking", "Mouse", "minimal", Some("instant"))
            }
            name if name.contains("drag") => ("🖱️", "Dragging", "Mouse", "minimal", Some("short")),
            name if name.contains("move") && name.contains("mouse") => {
                ("↗️", "Moving cursor", "Mouse", "silent", Some("instant"))
            }

            // Keyboard actions - standard notifications for better visibility of key details
            name if name.contains("type") => {
                ("⌨️", "Typing", "Keyboard", "standard", Some("short"))
            } // Changed to standard
            name if name.contains("key") || name.contains("press") => (
                "🔤",
                "Pressing keys",
                "Keyboard",
                "standard",
                Some("instant"),
            ), // Changed to standard

            // File operations - standard notifications
            name if name.contains("file") && name.contains("read") => {
                ("📖", "Reading file", "File", "standard", Some("short"))
            }
            name if name.contains("file") && (name.contains("write") || name.contains("save")) => {
                ("💾", "Writing file", "File", "standard", Some("short"))
            }
            name if name.contains("file") => {
                ("📁", "File operation", "File", "standard", Some("short"))
            }

            // Command execution - detailed notifications to show full commands
            name if name.contains("command")
                || name.contains("shell")
                || name.contains("terminal")
                || name.contains("bash")
                || name.contains("exec")
                || name.contains("run") =>
            {
                (
                    "⚡",
                    "Running command",
                    "Command",
                    "detailed",
                    Some("medium"),
                )
            }

            // Browser actions
            name if name.contains("browser") || name.contains("navigate") => (
                "🌐",
                "Browser action",
                "Browser",
                "standard",
                Some("medium"),
            ),

            // Desktop automation
            name if name.contains("desktop") || name.contains("application") => {
                ("🖥️", "Desktop action", "Desktop", "standard", Some("short"))
            }

            // Timer and scheduling
            name if name.contains("timer") => {
                ("⏰", "Timer action", "Timer", "standard", Some("instant"))
            }

            // MCP tools
            name if name.contains("mcp") => {
                ("🔌", "External tool", "MCP", "standard", Some("medium"))
            }

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
            tool_inputs: None,
        }
    }

    /// Extract key details from tool inputs for display
    fn extract_key_details(&self) -> Option<String> {
        let inputs = self.tool_inputs.as_ref()?;

        // Extract key information
        if let Some(key) = inputs.get("key").and_then(|v| v.as_str()) {
            let modifier = inputs
                .get("modifier")
                .and_then(|v| v.as_str())
                .map(|m| format!("{}+", m))
                .unwrap_or_default();
            return Some(format!("{}{}", modifier, key));
        }

        None
    }

    /// Extract command details from tool inputs for display
    fn extract_command_details(&self) -> Option<String> {
        let inputs = self.tool_inputs.as_ref()?;

        // Extract command information
        if let Some(command) = inputs.get("command").and_then(|v| v.as_str()) {
            // Truncate very long commands for display
            if command.len() > 100 {
                return Some(format!("{}...", &command[..97]));
            }
            return Some(command.to_string());
        }

        None
    }

    /// Extract text details from tool inputs for display
    fn extract_text_details(&self) -> Option<String> {
        let inputs = self.tool_inputs.as_ref()?;

        // Extract text information
        if let Some(text) = inputs.get("text").and_then(|v| v.as_str()) {
            // Truncate very long text for display
            if text.len() > 50 {
                return Some(format!("\"{}...\"", &text[..47]));
            }
            return Some(format!("\"{}\"", text));
        }

        None
    }

    /// Extract file path details from tool inputs for display
    fn extract_file_details(&self) -> Option<String> {
        let inputs = self.tool_inputs.as_ref()?;

        // Extract file path information
        if let Some(path) = inputs.get("path").and_then(|v| v.as_str()) {
            // Show just the filename for brevity
            if let Some(filename) = std::path::Path::new(path).file_name() {
                if let Some(filename_str) = filename.to_str() {
                    return Some(filename_str.to_string());
                }
            }
            // Fallback to showing truncated path
            if path.len() > 30 {
                return Some(format!("...{}", &path[path.len() - 27..]));
            }
            return Some(path.to_string());
        }

        None
    }

    /// Generate a start message for notifications with enhanced details
    fn generate_start_message(&self) -> Option<String> {
        match self.notification_level.as_str() {
            "silent" => None,
            "minimal" => Some(format!("{} {}", self.icon, self.action_verb)),
            "standard" => {
                // Include specific details for standard level
                let mut message = format!("{} {}", self.icon, self.action_verb);

                // Add specific details based on available inputs (no brittle category matching)
                if self.action_verb.to_ascii_lowercase().contains("typing") {
                    if let Some(text_details) = self.extract_text_details() {
                        message = format!("{} {}", message, text_details);
                    }
                }
                if let Some(key_details) = self.extract_key_details() {
                    message = format!("{} {}", message, key_details);
                }
                if self.action_verb.to_ascii_lowercase().contains("running command") {
                    if let Some(command_details) = self.extract_command_details() {
                        message = format!("{}: {}", message, command_details);
                    }
                }
                if let Some(file_details) = self.extract_file_details() {
                    message = format!("{} {}", message, file_details);
                }

                Some(format!("{}...", message))
            }
            "detailed" => {
                let mut message = format!("{} {}", self.icon, self.action_verb);

                // Add comprehensive details for detailed level
                if self.action_verb.to_ascii_lowercase().contains("running command") {
                    if let Some(command_details) = self.extract_command_details() {
                        message = format!("{}: {}", message, command_details);
                    }
                }
                if let Some(key_details) = self.extract_key_details() {
                    message = format!("{} {}", message, key_details);
                }
                if self.action_verb.to_ascii_lowercase().contains("typing") {
                    if let Some(text_details) = self.extract_text_details() {
                        message = format!("{} {}", message, text_details);
                    }
                }
                if let Some(file_details) = self.extract_file_details() {
                    message = format!("{} {}", message, file_details);
                }

                Some(format!(
                    "{} {}",
                    message,
                    self.description.as_deref().unwrap_or("in progress")
                ))
            }
            _ => Some(format!("{} {}", self.icon, self.action_verb)),
        }
    }

    /// Generate a result message for notifications with enhanced details
    fn generate_result_message(
        &self,
        success: bool,
        execution_time_ms: Option<u64>,
    ) -> Option<String> {
        match self.notification_level.as_str() {
            "silent" => None,
            "minimal" => {
                if success {
                    Some(format!("{} ✅", self.icon))
                } else {
                    Some(format!("{} ❌", self.icon))
                }
            }
            "standard" => {
                let status = if success { "completed" } else { "failed" };
                let mut message = format!("{} {}", self.icon, self.action_verb);

                // Add specific details for context
                if let Some(key_details) = self.extract_key_details() {
                    message = format!("{} {}", message, key_details);
                }
                if self.action_verb.to_ascii_lowercase().contains("typing") {
                    if let Some(text_details) = self.extract_text_details() {
                        message = format!("{} {}", message, text_details);
                    }
                }
                if let Some(file_details) = self.extract_file_details() {
                    message = format!("{} {}", message, file_details);
                }

                Some(format!("{} {}", message, status))
            }
            "detailed" => {
                let status = if success { "completed" } else { "failed" };
                let timing = execution_time_ms
                    .map(|ms| format!(" ({}ms)", ms))
                    .unwrap_or_default();

                let mut message = format!("{} {}", self.icon, self.action_verb);

                // Add comprehensive details
                if self.action_verb.to_ascii_lowercase().contains("running command") {
                    if let Some(command_details) = self.extract_command_details() {
                        message = format!("{}: {}", message, command_details);
                    }
                }
                if let Some(key_details) = self.extract_key_details() {
                    message = format!("{} {}", message, key_details);
                }
                if self.action_verb.to_ascii_lowercase().contains("typing") {
                    if let Some(text_details) = self.extract_text_details() {
                        message = format!("{} {}", message, text_details);
                    }
                }
                if let Some(file_details) = self.extract_file_details() {
                    message = format!("{} {}", message, file_details);
                }

                Some(format!("{} {}{}", message, status, timing))
            }
            _ => {
                let status = if success { "✅" } else { "❌" };
                Some(format!("{} {}", self.icon, status))
            }
        }
    }
}

// ===== REMOVED: Large block of deprecated legacy code =====
// This section contained old ToolUsageEntry and logging functions that have been
// replaced by the modern agent-event system. Removed for performance and clarity.

pub fn emit_stream_start(app_handle: &AppHandle, message_id: String) {
    let event_data = serde_json::json!({
        "message_id": message_id
    });

    if let Err(e) = app_handle.emit(
        crate::constants::events::streaming::STREAM_START,
        event_data,
    ) {
        warn!("Failed to emit agent-stream-start event: {}", e);
    }
}

pub fn emit_streaming_text_chunk(
    app_handle: &AppHandle,
    text: String,
    message_id: Option<String>,
    tts_content: Option<String>,
) {
    let event_data = serde_json::json!({
        "chunk": text,
        "message_id": message_id,
        "tts_content": tts_content, // Include TTS content for decorative display
        "metadata": {
            "has_spoken_content": tts_content.is_some(),
            "spoken_text": tts_content.clone()
        }
    });

    if let Err(e) = app_handle.emit(crate::constants::events::streaming::TEXT_STREAM, event_data) {
        warn!("Failed to emit agent-text-stream event: {}", e);
    }

    // If TTS content is provided, emit it for immediate processing
    if let Some(tts_text) = tts_content {
        process_tts_content_immediately(app_handle.clone(), tts_text);
    }
}

/// Process TTS content immediately with proper escape key management
/// ARCHITECTURAL DESIGN: invoke_tts properly handles audio completion tracking:
/// - Concurrency control (prevents overlapping TTS)
/// - Escape key registration during entire audio lifecycle
/// - Enhanced audio playback with completion validation
/// - Proper cleanup only after audio actually finishes
pub fn process_tts_content_immediately(app_handle: AppHandle, tts_content: String) {
    info!("Processing TTS content immediately: '{}'", tts_content);

    // CRITICAL FIX: Use a single background task to prevent audio overlap
    // invoke_tts now properly waits for actual audio completion before cleanup
    tokio::spawn(async move {
        // Get the app state for TTS invocation
        let app_state = match app_handle.try_state::<crate::state::AppState>() {
            Some(state) => state,
            None => {
                warn!("AppState not available for TTS processing, skipping");
                return;
            }
        };

        info!("Starting TTS processing with enhanced completion tracking...");

        // ARCHITECTURAL DESIGN: invoke_tts now properly handles:
        // - Text filtering and validation
        // - Concurrency prevention with mutex
        // - Escape key management throughout audio lifecycle
        // - Enhanced audio generation and playback tracking
        // - Proper cleanup only after actual audio completion
        match crate::tts::invoke_tts(tts_content, app_state, app_handle.clone()).await {
            Ok(status_result) => {
                info!("TTS processing completed with status: {}", status_result);

                // Handle the actual status results from invoke_tts
                match status_result.as_str() {
                    "TTS_COMPLETED" => {
                        info!("✅ TTS audio played successfully");
                    }
                    "TTS_ALREADY_PLAYING" => {
                        info!("🔊 TTS already playing, skipped to prevent overlap");
                    }
                    "TTS_DISABLED_BY_SETTING" => {
                        info!("🔇 TTS is disabled by user setting");
                    }
                    "TTS_CONTENT_FILTERED" => {
                        info!("🧹 TTS content was filtered out (code/unwanted content)");
                    }
                    "TTS_STOPPED_BY_USER" => {
                        info!("⏹️ TTS was stopped by user (escape key)");
                    }
                    "TTS_SOUND_DISABLED" => {
                        info!("🔇 Sound is disabled in settings");
                    }
                    _ => {
                        // Unexpected status - this shouldn't happen with the current architecture
                        warn!("Unexpected TTS status: '{}'. This may indicate an architectural mismatch.",
                              status_result.chars().take(50).collect::<String>());
                    }
                }
            }
            Err(e) => {
                warn!("❌ TTS processing failed: {}. Continuing without audio.", e);
            }
        }
    });

    // CRITICAL: Return immediately so agent execution continues
    // invoke_tts now properly handles escape key management and audio completion
    info!("TTS processing started in background with enhanced completion tracking...");
}



pub fn emit_stream_end(app_handle: &AppHandle, message_id: String, complete_text: String) {
    let is_jsx = crate::anthropic::is_jsx_content(&complete_text);
    let event_data = serde_json::json!({
        "message_id": message_id,
        "complete_text": complete_text,
        "is_jsx": is_jsx
    });

    if let Err(e) = app_handle.emit(crate::constants::events::streaming::STREAM_END, event_data) {
        warn!("Failed to emit agent-stream-end event: {}", e);
    }
}

pub fn emit_stream_end_with_state(
    app_handle: &AppHandle,
    message_id: String,
    complete_text: String,
    agent_state: String,
) {
    let is_jsx = crate::anthropic::is_jsx_content(&complete_text);
    let event_data = serde_json::json!({
        "message_id": message_id,
        "complete_text": complete_text,
        "agent_state": agent_state,
        "is_jsx": is_jsx
    });

    if let Err(e) = app_handle.emit(crate::constants::events::streaming::STREAM_END, event_data) {
        warn!("Failed to emit agent-stream-end event: {}", e);
    }
}
