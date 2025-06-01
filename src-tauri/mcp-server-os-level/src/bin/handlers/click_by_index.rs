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

use crate::types::{AppState, ClickByIndexRequest, ClickByIndexResponse, ListElementsAndAttributesResponse};
use crate::refresh_elements_and_attributes_after_action;

// Response type that combines both click result and elements
#[derive(Serialize)]
pub struct ClickByIndexWithElementsResponse {
    pub click: ClickByIndexResponse,
    pub elements: Option<ListElementsAndAttributesResponse>,
}

pub async fn click_by_index_handler(
    State(state): State<Arc<AppState>>,
    Json(request): Json<ClickByIndexRequest>,
) -> Result<JsonResponse<ClickByIndexWithElementsResponse>, (StatusCode, JsonResponse<serde_json::Value>)> {
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
                
                // Step 1: Try inputControl first (AppleScript) if bounds are available
                let bounds = element.bounds();
                let input_control_success = if let Ok((x, y, width, height)) = bounds {
                    debug!("attempting to click element at position [{}, {}] using inputControl", 
                          x + width/2.0, y + height/2.0);
                    
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
                    
                    // Calculate center of element
                    let center_x = x + width/2.0;
                    let center_y = y + height/2.0;
                    
                    use std::process::Command;
                    
                    // Use AppleScript to click at position
                    let script = format!(
                        "tell application \"System Events\" to click at {{round {}, round {}}}",
                        center_x, center_y
                    );
                    
                    match Command::new("osascript").arg("-e").arg(script).output() {
                        Ok(_) => {
                            debug!("successfully clicked element using inputControl at [{}, {}]",
                                  center_x, center_y);
                            true
                        },
                        Err(e) => {
                            debug!("failed to click using inputControl: {} - falling back to accessibility API", e);
                            false
                        }
                    }
                } else {
                    debug!("could not get element bounds - skipping inputControl approach");
                    false
                };
                
                // Step 2: If inputControl failed, use accessibility API as fallback
                if !input_control_success {
                    debug!("using accessibility API for clicking");
                    match element.click() {
                        Ok(_) => {
                            debug!("successfully clicked element using accessibility API");
                        },
                        Err(e) => {
                            error!("failed to click element with accessibility API: {}", e);
                            return Err((
                                StatusCode::INTERNAL_SERVER_ERROR,
                                JsonResponse(json!({
                                    "error": format!("failed to click element using both inputControl and accessibility API: {}", e)
                                })),
                            ));
                        }
                    }
                }
                
                // Create the success response based on which method worked
                let method_used = if input_control_success { "AppleScript" } else { "Accessibility API" };
                let click_response = ClickByIndexResponse {
                    success: true,
                    message: format!(
                        "successfully clicked element with role: {} (using {} method)",
                        element.role(), method_used
                    ),
                    elements: None,  // add the missing field
                };
                
                // Get refreshed elements using the helper function
                let elements_response = refresh_elements_and_attributes_after_action(state, app_name.clone(), 500).await;
                
                // Return combined response
                Ok(JsonResponse(ClickByIndexWithElementsResponse {
                    click: click_response,
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
