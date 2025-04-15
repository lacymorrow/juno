use axum::{
    extract::{Json, State},
    http::StatusCode,
    response::Json as JsonResponse,
};
use serde_json;
use std::process::Command;
use std::sync::Arc;
use tracing::{error, info};

use crate::server::types::{
    AppState, ElementCache, InputAction, InputControlRequest, InputControlResponse,
    InputControlWithElementsResponse,
};

use crate::server::handlers::utils::refresh_elements_and_attributes_after_action;

// Define the handler for input control
pub async fn input_control_handler(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<InputControlRequest>,
) -> Result<
    JsonResponse<InputControlWithElementsResponse>,
    (StatusCode, JsonResponse<serde_json::Value>),
> {
    info!("input control handler {:?}", payload);

    // Execute appropriate input action
    match payload.action {
        InputAction::KeyPress(key) => {
            // Add key name to key code mapping
            let key_code = match key.as_str() {
                "Tab" => "48",    // Tab key code
                "Return" => "36", // Enter/Return key code
                "Space" => "49",  // Space key code
                "Escape" => "53", // Escape key code
                // Add more key mappings as needed
                _ => key.as_str(), // Use as-is if it's already a number
            };

            let script = format!(
                "tell application \"System Events\" to key code {}",
                key_code
            );
            info!("executing key press script: {}", script);
            if let Err(e) = Command::new("osascript").arg("-e").arg(script).output() {
                error!("failed to press key: {}", e);
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    JsonResponse(
                        serde_json::json!({"error": format!("failed to press key: {}", e)}),
                    ),
                ));
            }
        }
        InputAction::MouseMove { x, y } => {
            // Implement mouse move
            let script = format!(
                "tell application \"System Events\" to set mouse position to {{{}, {}}}",
                x, y
            );
            if let Err(e) = Command::new("osascript").arg("-e").arg(script).output() {
                error!("failed to move mouse: {}", e);
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    JsonResponse(
                        serde_json::json!({"error": format!("failed to move mouse: {}", e)}),
                    ),
                ));
            }
        }
        InputAction::MouseClick(button) => {
            // Implement mouse click
            let button_num = match button.as_str() {
                "left" => 1,
                "right" => 2,
                _ => {
                    error!("unsupported mouse button: {}", button);
                    return Err((
                        StatusCode::BAD_REQUEST,
                        JsonResponse(
                            serde_json::json!({"error": format!("unsupported mouse button: {}", button)}),
                        ),
                    ));
                }
            };

            let script = format!(
                "tell application \"System Events\" to click button {}",
                button_num
            );
            if let Err(e) = Command::new("osascript").arg("-e").arg(script).output() {
                error!("failed to click mouse: {}", e);
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    JsonResponse(
                        serde_json::json!({"error": format!("failed to click mouse: {}", e)}),
                    ),
                ));
            }
        }
        InputAction::WriteText(text) => {
            // Implement text writing
            let script = format!(
                "tell application \"System Events\" to keystroke \"{}\"",
                text
            );
            if let Err(e) = Command::new("osascript").arg("-e").arg(script).output() {
                error!("failed to write text: {}", e);
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    JsonResponse(
                        serde_json::json!({"error": format!("failed to write text: {}", e)}),
                    ),
                ));
            }
        }
    }

    // Get elements from cache to find the active application
    let cache_guard = state.element_cache.lock().await;
    let app_name_to_use = match cache_guard.as_ref() {
        // Correctly destructure ElementCache
        Some(ElementCache { app_name, .. }) => Some(app_name.clone()),
        None => None,
    };
    drop(cache_guard); // Release lock

    // Refresh elements if we have an app name
    let elements_response = if let Some(app_name) = app_name_to_use {
        info!("refreshing elements for app: {}", app_name);
        refresh_elements_and_attributes_after_action(state.clone(), app_name, 500).await
    } else {
        info!("no active app found in cache, skipping element refresh");
        None
    };

    // Return combined response
    Ok(JsonResponse(InputControlWithElementsResponse {
        input: InputControlResponse { success: true },
        elements: elements_response,
    }))
}
