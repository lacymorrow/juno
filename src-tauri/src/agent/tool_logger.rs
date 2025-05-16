use serde::Serialize;
use serde_json::{Value};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Manager, Emitter};
use tracing::{info, warn, error};
use futures::FutureExt;

/// Type for tool usage events sent to the frontend
#[derive(Serialize, Clone)]
pub struct ToolUsageEntry {
    timestamp: u64,
    tool: String,
    inputs: Value,
    result: Option<Value>,
    success: bool,
    screenshot_base64: Option<String>, // Optional screenshot data
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
    // Record the start time
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    // Log tool invocation
    info!("Tool execution started: {}", tool_name);

    // Execute the tool
    let result = executor(inputs.clone());

    // Determine if execution was successful
    let success = result.is_ok();

    // If this is a screenshot tool, we want to include the screenshot in the event
    let screenshot_base64 = if tool_name == "capture_screenshot" || tool_name == "screenshot" {
        match &result {
            Ok(output) => {
                if let Some(base64) = output.as_str() {
                    info!("Screenshot captured successfully. Including in event.");
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

    // Create the tool usage entry
    let entry = ToolUsageEntry {
        timestamp,
        tool: tool_name.to_string(),
        inputs,
        result: result.as_ref().ok().cloned(),
        success,
        screenshot_base64,
    };

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

// Add a function to log async tool execution
pub async fn log_async_tool_execution<F>(
    app_handle: &AppHandle,
    tool_name: &str,
    input: Value,
    execution_future: F,
) -> Result<Value, String>
where
    F: std::future::Future<Output = Result<Value, String>> + Send,
{
    // Log tool call request
    log_tool_call_request(
        app_handle,
        tool_name,
        input.clone(), // Clone input for logging
        Some(format!("Attempting to execute tool: {}", tool_name)),
    );

    let result = std::panic::AssertUnwindSafe(execution_future)
        .catch_unwind()
        .await;

    match result {
        Ok(Ok(output)) => {
            let mut final_screenshot_base64: Option<String> = None;
            let mut final_tool_output = output.clone(); // Clone original output to potentially modify

            // Check for screenshot tool names
            if tool_name == "capture_screenshot" || tool_name == "capture_element_screenshot" || tool_name == "browser_screenshot" {
                if let Some(s_val) = output.as_str() {
                    final_screenshot_base64 = Some(s_val.to_string());
                    // Set a generic success message for the main tool_output as the screenshot is now separate
                    final_tool_output = serde_json::json!({ "status": "success", "message": "Screenshot captured and available in screenshot_base64 field." });
                } else if let Some(obj) = output.as_object() {
                    // Handle cases where the output might be an object containing the base64 string, e.g., {"base64": "..."}
                    // This was seen in the browser_controller.rs screenshot tool
                    if let Some(b64_val) = obj.get("base64").and_then(|v| v.as_str()) {
                        final_screenshot_base64 = Some(b64_val.to_string());
                        final_tool_output = serde_json::json!({ "status": "success", "message": "Screenshot extracted from tool output." });
                    }
                }
            }

            // Determine success based on the tool's output if available
            let execution_success = output.as_object()
                .and_then(|obj| obj.get("success"))
                .and_then(|val| val.as_bool())
                .unwrap_or(true); // Default to true if "success" field is not present or not a bool

            let content_message = if execution_success {
                format!("Tool {} executed successfully.", tool_name)
            } else {
                // Try to get an error message from the tool output if it failed
                let error_detail = output.as_object()
                    .and_then(|obj| obj.get("error").or_else(|| obj.get("stderr")))
                    .and_then(|val| val.as_str())
                    .map(|s| format!(": {}", s.trim().replace("\n", " "))) // also replace newlines for cleaner log
                    .unwrap_or_default();
                format!("Tool {} reported failure{}", tool_name, error_detail)
            };

            // Log tool call result
            log_tool_call_result(
                app_handle,
                tool_name,
                final_tool_output, // Use the (potentially modified) tool output
                execution_success, // Use the determined success status
                Some(content_message),
                final_screenshot_base64, // Pass the extracted screenshot data
            );
            Ok(output) // Return the original, unmodified output from the tool execution
        }
        Ok(Err(e)) => {
            // Log tool call failure
            log_tool_call_result(
                app_handle,
                tool_name,
                serde_json::json!({ "error": e }), // Log error as JSON
                false,
                Some(format!("Tool {} failed: {}", tool_name, e)),
                None,
            );
            Err(e)
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
            // Log tool call panic as failure
            log_tool_call_result(
                app_handle,
                tool_name,
                serde_json::json!({ "panic": err_msg }),
                false,
                Some(err_msg.clone()),
                None,
            );
            Err(err_msg)
        }
    }
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
}

#[derive(Clone, Debug, Serialize)]
struct ToolCallResultPayload {
    tool_name: String,
    tool_output: Value, // Keep as Value
    success: bool,
    content: Option<String>, // Optional descriptive content
    screenshot_base64: Option<String>, // Optional screenshot from the tool
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
        }),
    };
    emit_agent_event(app_handle, event);
}

// Example usage for emitting a screenshot event (if not part of a tool result):
pub fn log_screenshot(app_handle: &AppHandle, screenshot_base64: String, content: Option<String>) {
    let event = AgentEvent {
        event_type: "screenshot".to_string(),
        payload: AgentEventPayload::Screenshot(ScreenshotPayload {
            screenshot_base64,
            content,
        }),
    };
    emit_agent_event(app_handle, event);
}

// Function to log a generic content message (can be used by system, or for other status updates)
pub fn log_generic_content(app_handle: &AppHandle, content_text: &str) {
    let event = AgentEvent {
        event_type: "generic_content".to_string(), // Or another specific type if needed
        payload: AgentEventPayload::GenericContent(GenericContentPayload {
            content: content_text.to_string(),
        }),
    };
    emit_agent_event(app_handle, event);
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
