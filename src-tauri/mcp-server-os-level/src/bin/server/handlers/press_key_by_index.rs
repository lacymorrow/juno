use axum::{
    extract::{Json, State},
    http::StatusCode,
    response::Json as JsonResponse,
};
use serde_json::json;
use std::sync::Arc;
use tracing::{debug, error};

use crate::server::handlers::utils::refresh_elements_and_attributes_after_action;
use crate::server::types::{
    AppState, ListElementsAndAttributesResponse, PressKeyByIndexRequest,
    PressKeyByIndexResponse, PressKeyByIndexWithElementsResponse,
};

pub async fn press_key_by_index_handler(
    State(state): State<Arc<AppState>>,
    Json(request): Json<PressKeyByIndexRequest>,
) -> Result<
    JsonResponse<PressKeyByIndexWithElementsResponse>,
    (StatusCode, JsonResponse<serde_json::Value>),
> {
    debug!(
        "pressing key combination by index: element_index={}, key_combo={}",
        request.element_index, request.key_combo
    );

    let cache_guard = state.element_cache.lock().await;
    if let Some(cache) = cache_guard.as_ref() {
        if cache.timestamp.elapsed() < std::time::Duration::from_secs(30) {
            if request.element_index < cache.elements.len() {
                let element_to_press = cache.elements[request.element_index].clone();
                let app_name_from_cache = cache.app_name.clone(); // Clone the String from cache
                drop(cache_guard); // Release the lock before async operations

                debug!(
                    "pressing key '{}' on element at index {} from cache",
                    request.key_combo, request.element_index
                );
                match element_to_press.press_key(&request.key_combo) {
                    Ok(_) => {
                        debug!("press key successful");
                        // Refresh elements after the action
                        let refreshed_elements = refresh_elements_and_attributes_with_cache(
                            state.clone(),
                            app_name_from_cache, // Use cloned String
                            500,
                        )
                        .await;

                        return Ok(JsonResponse(PressKeyByIndexWithElementsResponse {
                            press_key: PressKeyByIndexResponse {
                                success: true,
                                message: format!(
                                    "Pressed key '{}' on element at index {}",
                                    request.key_combo, request.element_index
                                ),
                            },
                            elements: refreshed_elements,
                        }));
                    }
                    Err(e) => {
                        error!(
                            "failed to press key '{}' on element at index {}: {}",
                            request.key_combo, request.element_index, e
                        );
                        return Err((
                            StatusCode::INTERNAL_SERVER_ERROR,
                            JsonResponse(json!({
                                "error": format!("Failed to press key: {}", e)
                            })),
                        ));
                    }
                }
            } else {
                // Index out of bounds for the cached elements
                error!(
                    "element index {} out of bounds for cached elements (count: {}), app: {}",
                    request.element_index,
                    cache.elements.len(),
                    cache.app_name
                );
                return Err((
                    StatusCode::BAD_REQUEST,
                    JsonResponse(json!({
                        "error": format!("Element index {} out of bounds for cached elements of app '{}'. Try listing elements again.", request.element_index, cache.app_name)
                    })),
                ));
            }
        } else {
            debug!("cache miss or expired");
            drop(cache_guard);
        }
    } else {
        debug!("element cache is empty");
        drop(cache_guard);
    }

    // Cache miss logic...
    error!("cannot perform press key by index without a valid element cache. list elements first.");
    Err((
        StatusCode::PRECONDITION_FAILED,
        JsonResponse(json!({
            "error": "Element cache miss or invalid. Please list elements for the target application first."
        })),
    ))
}

// Helper function
async fn refresh_elements_and_attributes_with_cache(
    state: Arc<AppState>,
    app_name: String, // Takes String
    wait_ms: u64,
) -> Option<ListElementsAndAttributesResponse> {
    tokio::time::sleep(tokio::time::Duration::from_millis(wait_ms)).await;
    refresh_elements_and_attributes_after_action(state, app_name, 500).await // Pass String
}

// Helper function to convert key combo to AppleScript format
fn convert_key_combo_to_applescript(key_combo: &str) -> String {
    // Split the key combo by "+" to handle modifiers
    let parts: Vec<&str> = key_combo.split('+').collect();

    // Last part is usually the main key
    let main_key = parts.last().unwrap_or(&"").trim();

    // Check for modifiers
    let has_command = parts
        .iter()
        .any(|p| p.trim().eq_ignore_ascii_case("command") || p.trim().eq_ignore_ascii_case("cmd"));
    let has_shift = parts.iter().any(|p| p.trim().eq_ignore_ascii_case("shift"));
    let has_option = parts
        .iter()
        .any(|p| p.trim().eq_ignore_ascii_case("option") || p.trim().eq_ignore_ascii_case("alt"));
    let has_control = parts
        .iter()
        .any(|p| p.trim().eq_ignore_ascii_case("control") || p.trim().eq_ignore_ascii_case("ctrl"));

    // For special keys like Return, Tab, etc.
    let special_key_mapping = match main_key.to_lowercase().as_str() {
        "return" | "enter" => "return",
        "tab" => "tab",
        "escape" | "esc" => "escape",
        "backspace" | "delete" => "delete",
        "space" => "space",
        "down" | "downarrow" => "down arrow",
        "up" | "uparrow" => "up arrow",
        "left" | "leftarrow" => "left arrow",
        "right" | "rightarrow" => "right arrow",
        _ => main_key, // use as is for regular keys
    };

    // Build the AppleScript
    let mut script = String::from("tell application \"System Events\" to ");

    // For simple one-character keys
    if special_key_mapping.len() == 1 && !has_command && !has_shift && !has_option && !has_control {
        script.push_str(&format!("keystroke \"{}\"", special_key_mapping));
    } else {
        // For key combinations or special keys
        script.push_str("key code ");

        // Map the key to AppleScript key code or use the name for special keys
        match special_key_mapping {
            "return" => script.push_str("36"),
            "tab" => script.push_str("48"),
            "escape" => script.push_str("53"),
            "delete" => script.push_str("51"),
            "space" => script.push_str("49"),
            "down arrow" => script.push_str("125"),
            "up arrow" => script.push_str("126"),
            "left arrow" => script.push_str("123"),
            "right arrow" => script.push_str("124"),
            _ => {
                // For single character keys
                if special_key_mapping.len() == 1 {
                    // Get ASCII value
                    let c = special_key_mapping.chars().next().unwrap();
                    // This is a simplification - a proper implementation would map characters to key codes
                    // For letters, lowercase ASCII - 'a' + 0 would work
                    if c.is_ascii_lowercase() {
                        script.push_str(&format!("{}", (c as u8 - b'a') + 0));
                    } else if c.is_ascii_uppercase() {
                        script.push_str(&format!("{}", (c as u8 - b'A') + 0));
                    } else {
                        // This is a placeholder - you'd need a full mapping for all characters
                        script.push_str(&format!("\"{}\"", c));
                    }
                } else {
                    // For anything else, default to keystroke
                    script = format!(
                        "tell application \"System Events\" to keystroke \"{}\"",
                        special_key_mapping
                    );
                }
            }
        }

        // Add modifiers
        if has_command || has_shift || has_option || has_control {
            script.push_str(" using {");
            let mut modifiers = Vec::new();
            if has_command {
                modifiers.push("command down");
            }
            if has_shift {
                modifiers.push("shift down");
            }
            if has_option {
                modifiers.push("option down");
            }
            if has_control {
                modifiers.push("control down");
            }
            script.push_str(&modifiers.join(", "));
            script.push_str("}");
        }
    }

    debug!("generated applescript: {}", script);
    script
}
