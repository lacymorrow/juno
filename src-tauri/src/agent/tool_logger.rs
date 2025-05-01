use serde::Serialize;
use serde_json::{Value};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Manager, Emitter};
use tracing::{info, warn};
use crate::agent::implementations::tool_provider::AsyncToolFn;
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
pub async fn log_async_tool_execution<'a, Fut>(
    app_handle: &tauri::AppHandle,
    tool_name: &str,
    input: serde_json::Value,
    executor: Fut, // Accept the future directly
) -> Result<serde_json::Value, String>
where
    Fut: std::future::Future<Output = Result<serde_json::Value, String>> + Send + 'a,
{
    // Emit start event
    if let Err(e) = app_handle.emit("tool-execution-start", (tool_name, &input)) {
        log::warn!("Failed to emit tool-execution-start event: {}", e);
    }

    // Execute the future and capture the result
    let result = std::panic::AssertUnwindSafe(executor).catch_unwind().await;

    // Process the result and emit end/error event
    match result {
        Ok(Ok(output)) => {
            if let Err(e) = app_handle.emit("tool-execution-end", (tool_name, &output)) {
                log::warn!("Failed to emit tool-execution-end event: {}", e);
            }
            Ok(output)
        }
        Ok(Err(error_msg)) => {
             let error_val = serde_json::json!({ "error": error_msg });
             if let Err(e) = app_handle.emit("tool-execution-error", (tool_name, &error_val)) {
                log::warn!("Failed to emit tool-execution-error event: {}", e);
            }
            Err(error_msg)
        }
        Err(panic_payload) => {
            let error_msg = if let Some(s) = panic_payload.downcast_ref::<&str>() {
                format!("Tool execution panicked: {}", s)
            } else if let Some(s) = panic_payload.downcast_ref::<String>() {
                format!("Tool execution panicked: {}", s)
            } else {
                "Tool execution panicked with unknown payload".to_string()
            };
            log::error!("Tool '{}' panicked: {}", tool_name, error_msg);
            let error_val = serde_json::json!({ "error": &error_msg });
             if let Err(e) = app_handle.emit("tool-execution-error", (tool_name, &error_val)) {
                log::warn!("Failed to emit tool-execution-error event after panic: {}", e);
            }
            Err(error_msg)
        }
    }
}
