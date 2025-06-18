use axum::{
    extract::{Json, State},
    http::StatusCode,
    response::Json as JsonResponse,
};
use serde_json;
use std::sync::Arc;
use tracing::{error, info};
use core_graphics::event::{CGEventSource, CGEventSourceStateID, CGEvent, CGEventType, CGEventTapLocation, CGKeyCode};
use core_graphics::geometry::CGPoint;

use crate::server::types::{
    AppState, ElementCache, InputAction, InputControlRequest, InputControlResponse,
    InputControlWithElementsResponse,
};
use crate::server::handlers::utils::refresh_elements_and_attributes_after_action;
use crate::platforms::macos::interaction::{get_key_code, parse_key_combination};

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
            info!("pressing key: {}", key);

            // Try to parse as key combination first (e.g., "cmd+c", "shift+tab")
            let result = if key.contains('+') {
                // Use centralized key combination parsing
                match parse_key_combination(&key) {
                    Ok((key_code, flags)) => {
                        let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
                            .map_err(|_| "Failed to create event source")?;

                        let key_down = CGEvent::new_keyboard_event(source.clone(), key_code as CGKeyCode, true)
                            .map_err(|_| "Failed to create key down event")?;
                        if !flags.is_empty() {
                            key_down.set_flags(flags);
                        }
                        key_down.post(CGEventTapLocation::HID);

                        std::thread::sleep(std::time::Duration::from_millis(50));

                        let key_up = CGEvent::new_keyboard_event(source, key_code as CGKeyCode, false)
                            .map_err(|_| "Failed to create key up event")?;
                        if !flags.is_empty() {
                            key_up.set_flags(flags);
                        }
                        key_up.post(CGEventTapLocation::HID);

                        Ok(())
                    }
                    Err(e) => Err(format!("Invalid key combination '{}': {}", key, e)),
                }
            } else {
                // Use centralized single key parsing
                match get_key_code(&key) {
                    Ok(key_code) => {
                        let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
                            .map_err(|_| "Failed to create event source")?;

                        let key_down = CGEvent::new_keyboard_event(source.clone(), key_code as CGKeyCode, true)
                            .map_err(|_| "Failed to create key down event")?;
                        key_down.post(CGEventTapLocation::HID);

                        std::thread::sleep(std::time::Duration::from_millis(50));

                        let key_up = CGEvent::new_keyboard_event(source, key_code as CGKeyCode, false)
                            .map_err(|_| "Failed to create key up event")?;
                        key_up.post(CGEventTapLocation::HID);

                        Ok(())
                    }
                    Err(e) => Err(format!("Invalid key '{}': {}", key, e)),
                }
            };

            if let Err(error_msg) = result {
                error!("failed to press key: {}", error_msg);
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    JsonResponse(
                        serde_json::json!({"error": error_msg}),
                    ),
                ));
            }
        }
        InputAction::MouseMove { x, y } => {
            info!("moving mouse to ({}, {})", x, y);

            let location = CGPoint::new(x as f64, y as f64);
            let source = match CGEventSource::new(CGEventSourceStateID::HIDSystemState) {
                Ok(source) => source,
                Err(_) => {
                    error!("failed to create event source for mouse move");
                    return Err((
                        StatusCode::INTERNAL_SERVER_ERROR,
                        JsonResponse(
                            serde_json::json!({"error": "Failed to create event source for mouse move"}),
                        ),
                    ));
                }
            };

            let mouse_event = match CGEvent::new_mouse_event(
                source,
                CGEventType::MouseMoved,
                location,
                core_graphics::event::CGMouseButton::Left, // Ignored for mouse move
            ) {
                Ok(event) => event,
                Err(_) => {
                    error!("failed to create mouse move event");
                    return Err((
                        StatusCode::INTERNAL_SERVER_ERROR,
                        JsonResponse(
                            serde_json::json!({"error": "Failed to create mouse move event"}),
                        ),
                    ));
                }
            };

            mouse_event.post(CGEventTapLocation::HID);
        }
        InputAction::MouseClick(button) => {
            info!("clicking mouse button: {}", button);

            // Get current mouse position for click
            let current_location = unsafe {
                let event_source = CGEventSource::new(CGEventSourceStateID::HIDSystemState).unwrap();
                let mouse_event = CGEvent::new_mouse_event(
                    event_source,
                    CGEventType::MouseMoved,
                    CGPoint::new(0.0, 0.0),
                    core_graphics::event::CGMouseButton::Left,
                ).unwrap();
                mouse_event.location()
            };

            let (cg_button, down_event_type, up_event_type) = match button.as_str() {
                "left" => (
                    core_graphics::event::CGMouseButton::Left,
                    CGEventType::LeftMouseDown,
                    CGEventType::LeftMouseUp,
                ),
                "right" => (
                    core_graphics::event::CGMouseButton::Right,
                    CGEventType::RightMouseDown,
                    CGEventType::RightMouseUp,
                ),
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

            let source = match CGEventSource::new(CGEventSourceStateID::HIDSystemState) {
                Ok(source) => source,
                Err(_) => {
                    error!("failed to create event source for mouse click");
                    return Err((
                        StatusCode::INTERNAL_SERVER_ERROR,
                        JsonResponse(
                            serde_json::json!({"error": "Failed to create event source for mouse click"}),
                        ),
                    ));
                }
            };

            // Mouse down
            let down_event = match CGEvent::new_mouse_event(source.clone(), down_event_type, current_location, cg_button) {
                Ok(event) => event,
                Err(_) => {
                    error!("failed to create mouse down event");
                    return Err((
                        StatusCode::INTERNAL_SERVER_ERROR,
                        JsonResponse(
                            serde_json::json!({"error": "Failed to create mouse down event"}),
                        ),
                    ));
                }
            };
            down_event.post(CGEventTapLocation::HID);

            std::thread::sleep(std::time::Duration::from_millis(50));

            // Mouse up
            let up_event = match CGEvent::new_mouse_event(source, up_event_type, current_location, cg_button) {
                Ok(event) => event,
                Err(_) => {
                    error!("failed to create mouse up event");
                    return Err((
                        StatusCode::INTERNAL_SERVER_ERROR,
                        JsonResponse(
                            serde_json::json!({"error": "Failed to create mouse up event"}),
                        ),
                    ));
                }
            };
            up_event.post(CGEventTapLocation::HID);
        }
        InputAction::WriteText(text) => {
            info!("writing text: {}", text);

            let source = match CGEventSource::new(CGEventSourceStateID::HIDSystemState) {
                Ok(source) => source,
                Err(_) => {
                    error!("failed to create event source for text writing");
                    return Err((
                        StatusCode::INTERNAL_SERVER_ERROR,
                        JsonResponse(
                            serde_json::json!({"error": "Failed to create event source for text writing"}),
                        ),
                    ));
                }
            };

            // Type each character in the text
            for character in text.chars() {
                // Create keyboard event for each character
                let event = match CGEvent::new_unicode_keyboard_event(source.clone(), &[character as u16], false) {
                    Ok(event) => event,
                    Err(_) => {
                        error!("failed to create keyboard event for character: {}", character);
                        return Err((
                            StatusCode::INTERNAL_SERVER_ERROR,
                            JsonResponse(
                                serde_json::json!({"error": format!("Failed to create keyboard event for character: {}", character)}),
                            ),
                        ));
                    }
                };

                event.post(CGEventTapLocation::HID);

                // Small delay between characters for better reliability
                std::thread::sleep(std::time::Duration::from_millis(10));
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
