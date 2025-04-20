// Declare the submodules
mod definitions;
mod dispatch;
mod helpers;

// Re-export the public interface
pub use definitions::list_tools;

// Import necessary items for handle_tool_call
use crate::state::AppState;
use computer_use_ai_sdk::Desktop;
use serde_json::{json, Value};
use std::sync::Arc;
use tauri::{AppHandle, State};
use tracing::{error, info};

// Import the internal call_tool function
use dispatch::call_tool;

// --- Tool Call Wrapper ---

/// Wrapper function to integrate call_tool result into Anthropic flow
#[allow(dead_code)] // Allow dead code for helper potentially used by submit_query
pub async fn handle_tool_call(
    desktop: &Arc<Desktop>,
    app_handle: &AppHandle,
    tool_name: &str,
    input: &Value,
    state: &State<'_, AppState>, // Added state parameter
) -> Value { // Returns the JSON expected by Anthropic (either success or error content)
    match call_tool(desktop, app_handle, tool_name, input, state).await { // Pass state
        Ok(success_json) => {
            info!(tool_name = %tool_name, output = ?success_json, "Tool call succeeded");
            success_json
        }
        Err(error_json) => {
            error!(tool_name = %tool_name, error = ?error_json, "Tool call failed");
            // Ensure the error JSON has an "error" field for consistency
            if error_json.get("error").is_some() {
                error_json
            } else {
                json!({ "error": "An unexpected error occurred", "details": error_json })
            }
        }
    }
}

// Conditionally compile the tests module
#[cfg(test)]
mod tests;
