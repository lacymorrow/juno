use std::sync::Arc;
use axum::{
    extract::{Json, State},
    http::StatusCode,
    response::Json as JsonResponse,
};
use serde::Serialize;
use serde_json::json;
use tracing::{debug, error};
use computer_use_ai_sdk::Desktop;

use crate::types::{AppState, PressKeyByIndexRequest, PressKeyByIndexResponse, ListElementsAndAttributesResponse};
use crate::refresh_elements_and_attributes_after_action;

// Response type that combines both results
#[derive(Debug, Serialize)]
pub struct PressKeyByIndexWithElementsResponse {
    pub press_key: PressKeyByIndexResponse,
    pub elements: Option<ListElementsAndAttributesResponse>,
}

pub async fn press_key_by_index_handler(
    State(state): State<Arc<AppState>>,
    Json(request): Json<PressKeyByIndexRequest>,
) -> Result<JsonResponse<PressKeyByIndexWithElementsResponse>, (StatusCode, JsonResponse<serde_json::Value>)> {
    debug!("pressing key combination by index: element_index={}, key_combo={}", 
        request.element_index, request.key_combo);

    // Get elements from cache
    let elements_opt = {
        let cache = state.element_cache.lock().await;
        cache.clone()
    };

    // Check if cache exists
    if elements_opt.is_none() {
        return Err((
            StatusCode::BAD_REQUEST,
            JsonResponse(json!({
                "error": "no element cache found - you must call listInteractableElementsByIndex first to index the elements before using by-index operations"
            })),
        ));
    }

    match elements_opt {
        Some((elements, timestamp, app_name)) if timestamp.elapsed() < std::time::Duration::from_secs(30) => {
            // Activate the app first
            debug!("activating app: {}", app_name);
            let desktop = match Desktop::new(false, true) {
                Ok(d) => d,
                Err(e) => {
                    error!("failed to initialize desktop automation: {}", e);
                    return Err((
                        StatusCode::INTERNAL_SERVER_ERROR,
                        JsonResponse(json!({
                            "error": format!("failed to initialize desktop automation: {}", e)
                        })),
                    ));
                }
            };

            // Get and activate the application
            let _ = match desktop.application(&app_name) {
                Ok(app) => app,
                Err(e) => {
                    error!("application not found: {}", e);
                    return Err((
                        StatusCode::NOT_FOUND,
                        JsonResponse(json!({
                            "error": format!("application not found: {}", e)
                        })),
                    ));
                }
            };

            // Use element_index directly
            if request.element_index < elements.len() {
                let element = &elements[request.element_index];
                
                // Step 1: Try to click the element first to focus it
                if let Err(e) = element.click() {
                    debug!("failed to click element before key press: {}", e);
                    // Continue anyway
                }
                
                // Small delay to ensure element is focused
                std::thread::sleep(std::time::Duration::from_millis(100));
                
                // Step 2: Try inputControl first (AppleScript)
                debug!("attempting to press key '{}' using inputControl (AppleScript)", request.key_combo);
                
                use std::process::Command;
                
                // Convert key combo to AppleScript format
                let key_script = convert_key_combo_to_applescript(&request.key_combo);
                
                let input_control_success = match Command::new("osascript").arg("-e").arg(key_script).output() {
                    Ok(_) => {
                        debug!("successfully pressed key '{}' using inputControl", request.key_combo);
                        true
                    },
                    Err(e) => {
                        debug!("failed to press key using inputControl: {} - falling back to accessibility API", e);
                        false
                    }
                };
                
                // Step 3: If inputControl failed, use accessibility API as fallback
                if !input_control_success {
                    debug!("falling back to accessibility API for key press");
                    match element.press_key(&request.key_combo) {
                        Ok(_) => {
                            debug!("successfully pressed key '{}' using accessibility API", request.key_combo);
                        },
                        Err(e) => {
                            error!("failed to press key on element with accessibility API: {}", e);
                            return Err((
                                StatusCode::INTERNAL_SERVER_ERROR,
                                JsonResponse(json!({
                                    "error": format!("failed to press key using both inputControl and accessibility API: {}", e)
                                })),
                            ));
                        }
                    }
                }
                
                // Create the success response based on which method worked
                let method_used = if input_control_success { "AppleScript" } else { "Accessibility API" };
                let press_key_response = PressKeyByIndexResponse {
                    success: true,
                    message: format!(
                        "successfully pressed key combination '{}' on element with role: {} (using {} method)",
                        request.key_combo,
                        element.role(),
                        method_used
                    ),
                };
                
                // Get refreshed elements using the helper function
                let elements_response = refresh_elements_and_attributes_after_action(state, app_name.clone(), 500).await;
                
                // Return combined response
                Ok(JsonResponse(PressKeyByIndexWithElementsResponse {
                    press_key: press_key_response,
                    elements: elements_response,
                }))
            } else {
                error!(
                    "element index out of bounds: {} (max: {})",
                    request.element_index,
                    elements.len() - 1
                );
                Err((
                    StatusCode::BAD_REQUEST,
                    JsonResponse(json!({
                        "error": format!("element index out of bounds: {} (max: {})",
                                       request.element_index, elements.len() - 1)
                    })),
                ))
            }
        }
        Some(_) => {
            // Cache entry expired
            Err((
                StatusCode::BAD_REQUEST,
                JsonResponse(json!({
                    "error": "cache entry expired, please list elements again"
                })),
            ))
        }
        None => {
            // Cache miss
            Err((
                StatusCode::NOT_FOUND,
                JsonResponse(json!({
                    "error": "no cache entry found, please list elements again"
                })),
            ))
        }
    }
}

// Helper function to convert key combo to AppleScript format
fn convert_key_combo_to_applescript(key_combo: &str) -> String {
    // Split the key combo by "+" to handle modifiers
    let parts: Vec<&str> = key_combo.split('+').collect();
    
    // Last part is usually the main key
    let main_key = parts.last().unwrap_or(&"").trim();
    
    // Check for modifiers
    let has_command = parts.iter().any(|p| p.trim().eq_ignore_ascii_case("command") || p.trim().eq_ignore_ascii_case("cmd"));
    let has_shift = parts.iter().any(|p| p.trim().eq_ignore_ascii_case("shift"));
    let has_option = parts.iter().any(|p| p.trim().eq_ignore_ascii_case("option") || p.trim().eq_ignore_ascii_case("alt")); 
    let has_control = parts.iter().any(|p| p.trim().eq_ignore_ascii_case("control") || p.trim().eq_ignore_ascii_case("ctrl"));
    
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
        _ => main_key,  // use as is for regular keys
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
                    script = format!("tell application \"System Events\" to keystroke \"{}\"", special_key_mapping);
                }
            }
        }
        
        // Add modifiers
        if has_command || has_shift || has_option || has_control {
            script.push_str(" using {");
            let mut modifiers = Vec::new();
            if has_command { modifiers.push("command down"); }
            if has_shift { modifiers.push("shift down"); }
            if has_option { modifiers.push("option down"); }
            if has_control { modifiers.push("control down"); }
            script.push_str(&modifiers.join(", "));
            script.push_str("}");
        }
    }
    
    debug!("generated applescript: {}", script);
    script
}
