use serde::Serialize;
use serde_json::{Value};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Manager, Emitter};
use tracing::{info, warn};

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
