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

use crate::types::{AppState, TypeByIndexRequest, TypeByIndexResponse, ListElementsAndAttributesResponse};
use crate::refresh_elements_and_attributes_after_action;

// Response type that combines both results
#[derive(Serialize)]
pub struct TypeByIndexWithElementsResponse {
    pub type_action: TypeByIndexResponse,
    pub elements: Option<ListElementsAndAttributesResponse>,
}

pub async fn type_by_index_handler(
    State(state): State<Arc<AppState>>,
    Json(request): Json<TypeByIndexRequest>,
) -> Result<JsonResponse<TypeByIndexWithElementsResponse>, (StatusCode, JsonResponse<serde_json::Value>)> {
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
            // Use element_index directly
            if request.element_index < elements.len() {
                let element = &elements[request.element_index];
                
                // Step 1: Try inputControl first
                debug!("attempting to type text '{}' using inputControl (AppleScript)", request.text);

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

                // Click the element first to ensure it has focus
                if let Err(e) = element.click() {
                    debug!("failed to click element before typing: {}", e);
                    // Continue anyway
                }

                // Small delay to ensure element is focused
                std::thread::sleep(std::time::Duration::from_millis(100));

                // Use inputControl for text input using System Events
                use std::process::Command;

                // Escape any quotes in the text to avoid breaking the AppleScript
                let escaped_text = request.text.replace("\"", "\\\"");
                let script = format!("tell application \"System Events\" to keystroke \"{}\"", escaped_text);

                let input_control_success = match Command::new("osascript").arg("-e").arg(script).output() {
                    Ok(_) => {
                        debug!("successfully typed text '{}' using inputControl", request.text);
                        true
                    },
                    Err(e) => {
                        debug!("failed to type text using inputControl: {} - falling back to AXValue", e);
                        false
                    }
                };

                // Step 2: If inputControl failed, try AXValue as fallback
                if !input_control_success {
                    debug!("falling back to AXValue for typing");
                    match element.type_text(&request.text) {
                        Ok(_) => {
                            debug!("successfully typed text '{}' into element with role: {} using AXValue", 
                                  request.text, element.role());
                            
                            // Add a small delay to ensure UI updates
                            std::thread::sleep(std::time::Duration::from_millis(100));
                            
                            // Verify text was actually set by reading it back
                            let verification = match element.text(1) {
                                Ok(actual_text) => {
                                    let contains_text = actual_text.contains(&request.text);
                                    if contains_text {
                                        debug!("verified text was set correctly: '{}'", actual_text);
                                        true
                                    } else {
                                        debug!("verification failed: expected '{}' but got '{}'", 
                                              request.text, actual_text);
                                        false
                                    }
                                },
                                Err(e) => {
                                    debug!("failed to verify text: {}", e);
                                    false
                                }
                            };
                            
                            if !verification {
                                error!("failed to verify text was set with AXValue after inputControl failure");
                                return Err((
                                    StatusCode::INTERNAL_SERVER_ERROR,
                                    JsonResponse(json!({
                                        "error": "failed to type text using both inputControl and AXValue methods"
                                    })),
                                ));
                            }
                        },
                        Err(e) => {
                            error!("failed to type text into element with AXValue: {}", e);
                            return Err((
                                StatusCode::INTERNAL_SERVER_ERROR,
                                JsonResponse(json!({
                                    "error": format!("failed to type text using both inputControl and AXValue methods: {}", e)
                                })),
                            ));
                        }
                    }
                }

                // Create the success response based on which method worked
                let method_used = if input_control_success { "AppleScript" } else { "AXValue" };
                let type_response = TypeByIndexResponse {
                    success: true,
                    message: format!(
                        "successfully typed text into element with role: {} (using {} method)",
                        element.role(), method_used
                    ),
                };
                
                // Get refreshed elements using the helper function
                let elements_response = refresh_elements_and_attributes_after_action(state, app_name.clone(), 500).await;
                
                // Return combined response
                Ok(JsonResponse(TypeByIndexWithElementsResponse {
                    type_action: type_response,
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
