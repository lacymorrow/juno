use crate::state::AppState;
use computer_use_ai_sdk::{Desktop, AutomationError};
use serde_json::{json, Value};
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, State};
use tauri_plugin_notification::NotificationExt;
use tracing::{error, info, warn};
use wait_timeout::ChildExt;
use shlex;

#[cfg(target_os = "macos")]
use computer_use_ai_sdk::{
    platforms::macos::element::MacOSUIElement,
    platforms::macos::utils as macos_utils,
};

// Import helpers from the sibling module
use super::helpers::*;

// --- Helper Function for Holding Keys ---

/// Holds specified modifier keys, runs a closure, and releases the keys.
/// Returns the closure's result or an error if key holding/releasing fails.
fn hold_keys_and_run<F, T>(
    desktop: &Arc<Desktop>,
    keys: &[String],
    func: F,
) -> Result<T, Value>
where
    F: FnOnce() -> Result<T, AutomationError>, // Closure returns SDK result
{
    // Hold keys
    let mut held_keys = Vec::<String>::new(); // Track successfully held keys
    for key in keys {
        if let Err(e) = desktop.hold_key(key) {
            // Release already held keys in reverse order before returning error
            for held_key in held_keys.iter().rev() {
                desktop.release_key(held_key).ok(); // Ignore release error during cleanup
            }
            return Err(json!({ "error": format!("Failed to hold modifier key '{}': {}", key, e) }));
        }
        held_keys.push(key.clone()); // Add to held keys list
    }

    // Run the function
    let func_result = func().map_err(|e| json!({ "error": e.to_string() })); // Convert SDK error to JSON error

    // Release keys (attempt regardless of func_result)
    let mut release_errors = Vec::new();
    for key in keys.iter().rev() { // Release in reverse order
        if let Err(e) = desktop.release_key(key) {
            release_errors.push(format!("Failed to release key '{}': {}", key, e));
        }
    }

    // Combine results
    match (func_result, release_errors.is_empty()) {
        (Ok(result), true) => Ok(result), // Success
        (Ok(_), false) => Err(json!({ "error": format!("Action succeeded, but failed to release modifiers: {}", release_errors.join(", ")) })),
        (Err(func_err), true) => Err(func_err), // Function failed, release succeeded
        (Err(func_err), false) => Err(json!({ // Both failed
            "error": format!("Action failed: {}. Also failed to release modifiers: {}",
                func_err.get("error").and_then(|v| v.as_str()).unwrap_or("Unknown action error"),
                release_errors.join(", ")
            )
        })),
    }
}

// --- Tool Call Dispatcher ---

// Tool call dispatcher (Corrected Error Handling and Return Type)
#[allow(dead_code)] // Allow dead code for helper potentially used by submit_query
pub(crate) async fn call_tool(
    desktop: &Arc<Desktop>,
    app_handle: &AppHandle,
    tool_name: &str,
    input: &Value,
    state: &State<'_, AppState>, // Correctly include state here in the definition
) -> Result<Value, Value> { // Returns Result<SuccessJson, ErrorJson>
    // Use debug formatting for potentially complex input Value
    info!(tool_name = %tool_name, input = ?input, "Calling tool");

    // Wrap the core logic in an async block
    let result = async {
        match tool_name {
            "get_focused_element_info" => {
                match desktop.focused_element() { // Changed from get_focused_element
                    Ok(element) => {
                        let attrs = element.attributes();
                        serde_json::to_value(&attrs).map_err(|e| json!({ "error": format!("Failed to serialize element info: {}", e) }))
                    },
                    Err(e) => Err(json!({ "error": format!("Failed to get focused element: {}", e) })),
                }
            }
            "click_focused_element" => {
                match desktop.focused_element() { // Changed from get_focused_element
                    Ok(element) => {
                        match element.click() {
                            Ok(_) => Ok(json!({ "success": true, "message": "Clicked focused element." })),
                            Err(e) => Err(json!({ "error": format!("Failed to click focused element: {}", e) })),
                        }
                    },
                    Err(e) => Err(json!({ "error": format!("Failed to get focused element for clicking: {}", e) })),
                }
            }
            "type_text" => {
                match get_string_param(input, "text") {
                    Ok(text) => match desktop.type_text(&text) {
                        Ok(_) => Ok(json!({ "success": true, "message": "Text typed." })),
                        Err(e) => Err(json!({ "error": format!("Failed to type text: {}", e) })),
                    },
                    Err(e) => Err(e), // Propagate param parsing error
                }
            }
            "press_key" => {
                match (get_string_param(input, "key"), get_optional_string_param(input, "modifier")) {
                    (Ok(key), Ok(modifier)) => {
                        // Always use desktop.press_key for consistency
                        info!(key = %key, ?modifier, "Using press_key");
                        match desktop.press_key(&key, modifier.as_deref()) {
                            Ok(_) => Ok(json!({ "success": true, "message": format!("Key '{}' pressed{}.", key, modifier.map(|m| format!(" with modifier '{}'", m)).unwrap_or_default()) })),
                            Err(e) => Err(json!({ "error": format!("Failed to press key: {}", e) })),
                        }
                    }
                    (Err(e), _) | (_, Err(e)) => Err(e), // Propagate param parsing error
                }
            }
            "open_application" => {
                match get_string_param(input, "app_name") {
                    Ok(app_name) => match desktop.open_application(&app_name) {
                        Ok(_) => Ok(json!({ "success": true, "message": format!("Application '{}' opened.", app_name) })),
                        Err(e) => Err(json!({ "error": format!("Failed to open application: {}", e) })),
                    },
                    Err(e) => Err(e),
                }
            }
            "open_url" => {
                match get_string_param(input, "url") {
                    Ok(url) => match desktop.open_url(&url, None) {
                        Ok(_) => Ok(json!({ "success": true, "message": format!("URL '{}' opened.", url) })),
                        Err(e) => Err(json!({ "error": format!("Failed to open URL: {}", e) })),
                    },
                    Err(e) => Err(e),
                }
            }
            "scroll_window" => { // Maps to scroll_at_current_position
                let direction = get_string_param(input, "direction")?;
                let amount = get_f64_param(input, "amount")?;
                let modifier_keys = get_optional_modifier_keys(input)?;

                if let Some(keys) = modifier_keys {
                    if !keys.is_empty() {
                        info!(%direction, %amount, ?keys, "Executing scroll_window with modifiers");
                        hold_keys_and_run(desktop, &keys, || {
                            desktop.scroll_at_current_position(&direction, amount)
                        })
                            .map(|_| json!({ "success": true, "message": format!("Scrolled {} by {} with modifiers {:?}.", direction, amount, keys) }))
                    } else {
                        // Empty modifier list, normal scroll
                        info!(%direction, %amount, "Executing scroll_window (empty modifiers list)");
                        match desktop.scroll_at_current_position(&direction, amount) {
                            Ok(_) => Ok(json!({ "success": true, "message": format!("Scrolled {} by {}.", direction, amount) })),
                            Err(e) => Err(json!({ "error": format!("Failed to scroll window: {}", e) })),
                        }
                    }
                } else {
                    // No modifiers, normal scroll
                    info!(%direction, %amount, "Executing scroll_window");
                    match desktop.scroll_at_current_position(&direction, amount) {
                        Ok(_) => Ok(json!({ "success": true, "message": format!("Scrolled {} by {}.", direction, amount) })),
                        Err(e) => Err(json!({ "error": format!("Failed to scroll window: {}", e) })),
                    }
                }
            }
            "capture_screenshot" => {
                #[cfg(target_os = "macos")]
                {
                    match macos_utils::capture_and_encode_screenshot() {
                        Ok(base64_string) => {
                            app_handle.notification().builder().title("Screenshot").body("Screenshot captured.").show().ok();
                            // Return the raw base64 string for processing in submit_query
                            Ok(json!({ "success": true, "screenshot_base64": base64_string }))
                        },
                        Err(e) => Err(json!({ "error": format!("Failed to capture screenshot: {}", e) })),
                    }
                }
                #[cfg(not(target_os = "macos"))]
                {
                    Err(json!({ "error": "Screenshot capture is only supported on macOS currently." }))
                }
            }
            "capture_element_screenshot" => {
                #[cfg(target_os = "macos")]
                {
                    match desktop.focused_element() { // Changed from get_focused_element
                        Ok(focused_element) => {
                            if let Some(macos_element) = focused_element.as_any().downcast_ref::<MacOSUIElement>() {
                                match macos_utils::capture_element_screenshot(macos_element) {
                                    Ok(base64_string) => {
                                        app_handle.notification().builder().title("Element Screenshot").body("Focused element screenshot captured.").show().ok();
                                        // Return the raw base64 string for processing in submit_query
                                        Ok(json!({ "success": true, "screenshot_base64": base64_string }))
                                    },
                                    Err(e) => Err(json!({ "error": format!("Failed to capture element screenshot: {}", e) })),
                                }
                            } else {
                                Err(json!({ "error": "Focused element is not a MacOSUIElement" }))
                            }
                        },
                        Err(e) => Err(json!({ "error": format!("Failed to get focused element for screenshot: {}", e) })),
                    }
                }
                #[cfg(not(target_os = "macos"))]
                {
                    Err(json!({ "error": "Element screenshot capture is only supported on macOS currently." }))
                }
            }
            // --- Added Tool Handlers ---
            "wait" => {
                match get_u64_param(input, "duration_ms") {
                    Ok(duration_ms) => match desktop.wait(duration_ms) {
                        Ok(_) => Ok(json!({ "success": true, "message": format!("Waited for {} ms.", duration_ms) })),
                        Err(e) => Err(json!({ "error": format!("Wait failed: {}", e) })),
                    },
                    Err(e) => Err(e),
                }
            }
            "cursor_position" => {
                match desktop.cursor_position() {
                    Ok((x, y)) => Ok(json!({ "success": true, "x": x, "y": y })),
                    Err(e) => Err(json!({ "error": format!("Failed to get cursor position: {}", e) })),
                }
            }
            "mouse_move" => {
                match (get_f64_param(input, "x"), get_f64_param(input, "y")) {
                    (Ok(x), Ok(y)) => match desktop.mouse_move(x, y) {
                        Ok(_) => Ok(json!({ "success": true, "message": format!("Mouse moved to ({}, {}).", x, y) })),
                        Err(e) => Err(json!({ "error": format!("Failed to move mouse: {}", e) })),
                    },
                    (Err(e), _) | (_, Err(e)) => Err(e),
                }
            }
            "left_mouse_down" => {
                match (get_f64_param(input, "x"), get_f64_param(input, "y")) {
                    (Ok(x), Ok(y)) => match desktop.left_mouse_down(x, y) {
                        Ok(_) => Ok(json!({ "success": true, "message": "Left mouse button pressed down." })),
                        Err(e) => Err(json!({ "error": format!("Failed to press left mouse button down: {}", e) })),
                    },
                    (Err(e), _) | (_, Err(e)) => Err(e),
                }
            }
            "left_mouse_up" => {
                match (get_f64_param(input, "x"), get_f64_param(input, "y")) {
                    (Ok(x), Ok(y)) => match desktop.left_mouse_up(x, y) {
                        Ok(_) => Ok(json!({ "success": true, "message": "Left mouse button released." })),
                        Err(e) => Err(json!({ "error": format!("Failed to release left mouse button: {}", e) })),
                    },
                    (Err(e), _) | (_, Err(e)) => Err(e),
                }
            }
            "left_click" => {
                let x = get_f64_param(input, "x")?;
                let y = get_f64_param(input, "y")?;
                // Extract optional modifier keys
                let modifier_keys_val = input.get("modifier_keys");
                let modifier_keys: Option<Vec<String>> = modifier_keys_val
                    .and_then(|v| v.as_array())
                    .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect());

                if let Some(keys) = modifier_keys {
                    if !keys.is_empty() {
                        info!(x = %x, y = %y, ?keys, "Executing left click with modifiers");
                        // Hold keys
                        for key in &keys {
                            if let Err(e) = desktop.hold_key(key) {
                                // Attempt to release any already held keys before returning error
                                for held_key in keys.iter().take_while(|&k| k != key) {
                                    desktop.release_key(held_key).ok(); // Ignore release error during cleanup
                                }
                                return Err(json!({ "error": format!("Failed to hold modifier key '{}': {}", key, e) }));
                            }
                        }

                        // Perform click
                        let click_result = desktop.left_click(x, y);

                        // Release keys (attempt regardless of click result)
                        let mut release_errors = Vec::new();
                        for key in keys.iter().rev() { // Release in reverse order
                            if let Err(e) = desktop.release_key(key) {
                                release_errors.push(format!("Failed to release key '{}': {}", key, e));
                            }
                        }

                        // Handle results
                        match click_result {
                            Ok(_) if release_errors.is_empty() => Ok(json!({ "success": true, "message": format!("Left clicked at ({}, {}) with modifiers {:?}.", x, y, keys) })),
                            Ok(_) => Err(json!({ "error": format!("Click succeeded, but failed to release modifiers: {}", release_errors.join(", ")) })),
                            Err(e) if release_errors.is_empty() => Err(json!({ "error": format!("Click failed: {}. Modifiers released.", e) })),
                            Err(e) => Err(json!({ "error": format!("Click failed: {}. Also failed to release modifiers: {}", e, release_errors.join(", ")) })),
                        }
                    } else {
                        // Modifiers array provided but empty, treat as normal click
                        info!(x = %x, y = %y, "Executing left click (empty modifiers list)");
                        match desktop.left_click(x, y) {
                            Ok(_) => Ok(json!({ "success": true, "message": format!("Left clicked at ({}, {}).", x, y) })),
                            Err(e) => Err(json!({ "error": format!("Failed to perform left click: {}", e) })),
                        }
                    }
                } else {
                    // No modifiers provided, perform normal click
                    info!(x = %x, y = %y, "Executing left click");
                    match desktop.left_click(x, y) {
                        Ok(_) => Ok(json!({ "success": true, "message": format!("Left clicked at ({}, {}).", x, y) })),
                        Err(e) => Err(json!({ "error": format!("Failed to perform left click: {}", e) })),
                    }
                }
            }
            "right_click" => {
                let x = get_f64_param(input, "x")?;
                let y = get_f64_param(input, "y")?;
                let modifier_keys = get_optional_modifier_keys(input)?;

                if let Some(keys) = modifier_keys {
                    if !keys.is_empty() {
                        info!(x = %x, y = %y, ?keys, "Executing right click with modifiers");
                        hold_keys_and_run(desktop, &keys, || desktop.right_click(x, y))
                            .map(|_| json!({ "success": true, "message": format!("Right clicked at ({}, {}) with modifiers {:?}.", x, y, keys) }))
                    } else {
                        // Empty modifier list, normal click
                        info!(x = %x, y = %y, "Executing right click (empty modifiers list)");
                        match desktop.right_click(x, y) {
                            Ok(_) => Ok(json!({ "success": true, "message": format!("Right clicked at ({}, {}).", x, y) })),
                            Err(e) => Err(json!({ "error": format!("Failed to perform right click: {}", e) })),
                        }
                    }
                } else {
                    // No modifiers, normal click
                    info!(x = %x, y = %y, "Executing right click");
                    match desktop.right_click(x, y) {
                        Ok(_) => Ok(json!({ "success": true, "message": format!("Right clicked at ({}, {}).", x, y) })),
                        Err(e) => Err(json!({ "error": format!("Failed to perform right click: {}", e) })),
                    }
                }
            }
            "middle_click" => {
                let x = get_f64_param(input, "x")?;
                let y = get_f64_param(input, "y")?;
                let modifier_keys = get_optional_modifier_keys(input)?;

                if let Some(keys) = modifier_keys {
                    if !keys.is_empty() {
                        info!(x = %x, y = %y, ?keys, "Executing middle click with modifiers");
                        hold_keys_and_run(desktop, &keys, || desktop.middle_click(x, y))
                            .map(|_| json!({ "success": true, "message": format!("Middle clicked at ({}, {}) with modifiers {:?}.", x, y, keys) }))
                    } else {
                        // Empty modifier list, normal click
                        info!(x = %x, y = %y, "Executing middle click (empty modifiers list)");
                        match desktop.middle_click(x, y) {
                            Ok(_) => Ok(json!({ "success": true, "message": format!("Middle clicked at ({}, {}).", x, y) })),
                            Err(e) => Err(json!({ "error": format!("Failed to perform middle click: {}", e) })),
                        }
                    }
                } else {
                    // No modifiers, normal click
                    info!(x = %x, y = %y, "Executing middle click");
                    match desktop.middle_click(x, y) {
                        Ok(_) => Ok(json!({ "success": true, "message": format!("Middle clicked at ({}, {}).", x, y) })),
                        Err(e) => Err(json!({ "error": format!("Failed to perform middle click: {}", e) })),
                    }
                }
            }
            "double_click" => {
                let x = get_f64_param(input, "x")?;
                let y = get_f64_param(input, "y")?;
                let modifier_keys = get_optional_modifier_keys(input)?;

                if let Some(keys) = modifier_keys {
                    if !keys.is_empty() {
                        info!(x = %x, y = %y, ?keys, "Executing double click with modifiers");
                        hold_keys_and_run(desktop, &keys, || desktop.double_click(x, y))
                            .map(|_| json!({ "success": true, "message": format!("Double clicked at ({}, {}) with modifiers {:?}.", x, y, keys) }))
                    } else {
                        // Empty modifier list, normal click
                        info!(x = %x, y = %y, "Executing double click (empty modifiers list)");
                        match desktop.double_click(x, y) {
                            Ok(_) => Ok(json!({ "success": true, "message": format!("Double clicked at ({}, {}).", x, y) })),
                            Err(e) => Err(json!({ "error": format!("Failed to perform double click: {}", e) })),
                        }
                    }
                } else {
                    // No modifiers, normal click
                    info!(x = %x, y = %y, "Executing double click");
                    match desktop.double_click(x, y) {
                        Ok(_) => Ok(json!({ "success": true, "message": format!("Double clicked at ({}, {}).", x, y) })),
                        Err(e) => Err(json!({ "error": format!("Failed to perform double click: {}", e) })),
                    }
                }
            }
            "triple_click" => {
                let x = get_f64_param(input, "x")?;
                let y = get_f64_param(input, "y")?;
                let modifier_keys = get_optional_modifier_keys(input)?;

                if let Some(keys) = modifier_keys {
                    if !keys.is_empty() {
                        info!(x = %x, y = %y, ?keys, "Executing triple click with modifiers");
                        hold_keys_and_run(desktop, &keys, || desktop.triple_click(x, y))
                            .map(|_| json!({ "success": true, "message": format!("Triple clicked at ({}, {}) with modifiers {:?}.", x, y, keys) }))
                    } else {
                        // Empty modifier list, normal click
                        info!(x = %x, y = %y, "Executing triple click (empty modifiers list)");
                        match desktop.triple_click(x, y) {
                            Ok(_) => Ok(json!({ "success": true, "message": format!("Triple clicked at ({}, {}).", x, y) })),
                            Err(e) => Err(json!({ "error": format!("Failed to perform triple click: {}", e) })),
                        }
                    }
                } else {
                    // No modifiers, normal click
                    info!(x = %x, y = %y, "Executing triple click");
                    match desktop.triple_click(x, y) {
                        Ok(_) => Ok(json!({ "success": true, "message": format!("Triple clicked at ({}, {}).", x, y) })),
                        Err(e) => Err(json!({ "error": format!("Failed to perform triple click: {}", e) })),
                    }
                }
            }
            "left_click_drag" => {
                match (
                    get_f64_param(input, "start_x"),
                    get_f64_param(input, "start_y"),
                    get_f64_param(input, "end_x"),
                    get_f64_param(input, "end_y"),
                ) {
                    (Ok(start_x), Ok(start_y), Ok(end_x), Ok(end_y)) => {
                        info!(start_x=%start_x, start_y=%start_y, end_x=%end_x, end_y=%end_y, "Executing left click drag");
                        // Assuming desktop.left_click_drag exists or will be added
                        match desktop.left_click_drag(start_x, start_y, end_x, end_y) {
                            Ok(_) => Ok(json!({ "success": true, "message": format!("Left click drag from ({}, {}) to ({}, {}).", start_x, start_y, end_x, end_y) })),
                            Err(e) => Err(json!({ "error": format!("Failed to perform left click drag: {}", e) })),
                        }
                    }
                    (Err(e), _, _, _) | (_, Err(e), _, _) | (_, _, Err(e), _) | (_, _, _, Err(e)) => Err(e), // Propagate the first parsing error
                }
            }
            "scroll_at_position" => { // Assuming Desktop has this method wrapping the engine call
                let x = get_f64_param(input, "x")?;
                let y = get_f64_param(input, "y")?;
                let direction = get_string_param(input, "direction")?;
                let amount = get_f64_param(input, "amount")?;
                let modifier_keys = get_optional_modifier_keys(input)?;

                if let Some(keys) = modifier_keys {
                    if !keys.is_empty() {
                        info!(%x, %y, %direction, %amount, ?keys, "Executing scroll_at_position with modifiers");
                        hold_keys_and_run(desktop, &keys, || {
                            desktop.scroll_at_position(x, y, &direction, amount)
                        })
                            .map(|_| json!({ "success": true, "message": format!("Scrolled {} by {} at ({}, {}) with modifiers {:?}.", direction, amount, x, y, keys) }))
                    } else {
                        // Empty modifier list, normal scroll
                        info!(%x, %y, %direction, %amount, "Executing scroll_at_position (empty modifiers list)");
                        match desktop.scroll_at_position(x, y, &direction, amount) {
                            Ok(_) => Ok(json!({ "success": true, "message": format!("Scrolled {} by {} at ({}, {}).", direction, amount, x, y) })),
                            Err(e) => Err(json!({ "error": format!("Failed to scroll at position: {}", e) })),
                        }
                    }
                } else {
                    // No modifiers, normal scroll
                    info!(%x, %y, %direction, %amount, "Executing scroll_at_position");
                    match desktop.scroll_at_position(x, y, &direction, amount) {
                        Ok(_) => Ok(json!({ "success": true, "message": format!("Scrolled {} by {} at ({}, {}).", direction, amount, x, y) })),
                        Err(e) => Err(json!({ "error": format!("Failed to scroll at position: {}", e) })),
                    }
                }
            }
            "hold_key" => {
                let key = get_string_param(input, "key")?;
                let duration_ms_opt = get_optional_u64_param(input, "duration_ms")?;

                // Currently, desktop.hold_key doesn't accept duration.
                // This will need modification in the SDK (mcp-server-os-level).
                // For now, we parse it but don't use it, logging a warning.
                // TODO: Update SDK hold_key to accept Option<u64> and implement timed hold.
                match duration_ms_opt {
                    Some(duration) => {
                        warn!(key=%key, duration=%duration, "hold_key with duration is not yet fully implemented in SDK. Holding indefinitely.");
                        // Placeholder: Call the existing hold_key for now
                        match desktop.hold_key(&key) {
                            Ok(_) => Ok(json!({ "success": true, "message": format!("Holding key '{}' (duration ignored, held indefinitely).", key) })),
                            Err(e) => Err(json!({ "error": format!("Failed to hold key: {}", e) })),
                        }
                        // Correct SDK call would be something like:
                        // match desktop.hold_key(&key, Some(duration)) { ... }
                    }
                    None => {
                        // Call the existing hold_key (which holds indefinitely)
                        match desktop.hold_key(&key) {
                            Ok(_) => Ok(json!({ "success": true, "message": format!("Holding key '{}' indefinitely.", key) })),
                            Err(e) => Err(json!({ "error": format!("Failed to hold key: {}", e) })),
                        }
                    }
                }
            }
            "release_key" => {
                match get_string_param(input, "key") {
                    Ok(key) => match desktop.release_key(&key) {
                        Ok(_) => Ok(json!({ "success": true, "message": format!("Released key '{}'.", key) })),
                        Err(e) => Err(json!({ "error": format!("Failed to release key: {}", e) })),
                    },
                    Err(e) => Err(e),
                }
            }
            // --- Text Editor Handlers ---
            "text_editor_view" => {
                let file_path = get_string_param(input, "file_path")?;
                // Get optional line numbers (use our helper for optional u64)
                let start_line_opt = get_optional_u64_param(input, "start_line")?;
                let end_line_opt = get_optional_u64_param(input, "end_line")?;
                let path = PathBuf::from(&file_path);

                // --- Check if Path is Directory ---
                match fs::metadata(&path) {
                    Ok(metadata) => {
                        if metadata.is_dir() {
                            // --- Handle Directory Listing ---
                            if start_line_opt.is_some() || end_line_opt.is_some() {
                                return Err(json!({ "error": "Line range parameters (start_line, end_line) are not allowed when viewing a directory." }));
                            }

                            info!(path = %file_path, "Listing directory contents");
                            // Construct the find command: find <path> -maxdepth 2 -not -path '*/.*'
                            // Using sh -c to handle potential special characters in the path
                            let find_command = format!("find {} -maxdepth 2 -not -path '*/.*'", shlex::quote(&file_path));
                            match Command::new("sh").arg("-c").arg(&find_command).output() {
                                Ok(output) => {
                                    if output.status.success() {
                                        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                                        let result_message = format!(
                                            "Directory listing for '{}' (up to 2 levels deep, excluding hidden):
{}",
                                            file_path,
                                            stdout
                                        );
                                        Ok(json!({ "success": true, "content": result_message }))
                                    } else {
                                        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                                        Err(json!({ "error": format!("Failed to list directory '{}': Command failed: {}", file_path, stderr) }))
                                    }
                                }
                                Err(e) => Err(json!({ "error": format!("Failed to execute find command for directory '{}': {}", file_path, e) }))
                            }
                            // --- End Handle Directory Listing ---
                        } else {
                            // --- Handle File Viewing (Existing Logic) ---
                            match (start_line_opt, end_line_opt) {
                                (Some(start), Some(end)) => {
                                    // Both start and end provided: Read range
                                    info!(path = %file_path, start=start, end=end, "Reading file range");
                                    if start == 0 || end == 0 || start > end {
                                        return Err(json!({ "error": format!("Invalid line range: start ({}) must be >= 1 and <= end ({}).", start, end) }));
                                    }
                                    match fs::read_to_string(&file_path) {
                                        Ok(content) => {
                                            let lines: Vec<&str> = content.lines().collect();
                                            let total_lines = lines.len();
                                            // Adjust to 0-based index, clamp to bounds
                                            let start_idx = (start - 1).clamp(0, total_lines as u64) as usize;
                                            let end_idx = (end).clamp(0, total_lines as u64) as usize; // end is exclusive for slice

                                            if start_idx >= end_idx {
                                                // Handle case where start is beyond end after clamping (e.g., start > total_lines)
                                                Ok(json!({ "success": true, "content": "", "message": "Specified range is empty or invalid for this file." }))
                                            } else {
                                                let range_content = lines[start_idx..end_idx].join("\n");
                                                Ok(json!({ "success": true, "content": range_content }))
                                            }
                                        }
                                        Err(e) => Err(json!({ "error": format!("Failed to read file '{}': {}", file_path, e) })),
                                    }
                                }
                                (None, None) => {
                                    // Neither provided: Read whole file
                                    info!(path = %file_path, "Reading whole file");
                                    match fs::read_to_string(&file_path) {
                                        Ok(content) => Ok(json!({ "success": true, "content": content })),
                                        Err(e) => Err(json!({ "error": format!("Failed to read file '{}': {}", file_path, e) })),
                                    }
                                }
                                _ => {
                                    // Only one provided: Invalid combination
                                    Err(json!({ "error": "Both start_line and end_line must be provided together, or neither." }))
                                }
                            }
                            // --- End Handle File Viewing ---
                        }
                    }
                    Err(e) => {
                        // Handle cases where metadata cannot be retrieved (e.g., path doesn't exist)
                        Err(json!({ "error": format!("Failed to access path '{}': {}", file_path, e) }))
                    }
                }
                // --- End Check if Path is Directory ---
            }
            "text_editor_create" => {
                match (get_string_param(input, "file_path"), get_string_param(input, "content")) {
                    (Ok(file_path), Ok(content)) => {
                        // --- Undo State Update ---
                        let path = PathBuf::from(file_path.clone());
                        crate::state::update_undo_state(state, path, None); // Use crate::state::
                        // --- End Undo State Update ---
                        match fs::write(&file_path, content) {
                            Ok(_) => Ok(json!({ "success": true, "message": format!("File '{}' created/overwritten.", file_path) })),
                            Err(e) => Err(json!({ "error": format!("Failed to write file '{}': {}", file_path, e) })),
                        }
                    },
                    (Err(e), _) | (_, Err(e)) => Err(e),
                }
            }
            "text_editor_insert" => {
                match (
                    get_string_param(input, "file_path"),
                    get_string_param(input, "text_to_insert"),
                    get_i64_param(input, "line_number")
                ) {
                    (Ok(file_path), Ok(text_to_insert), Ok(line_number)) => {
                        let line_usize = line_number as usize;
                        // --- Undo State Update ---
                        let path = PathBuf::from(file_path.clone());
                        // Read current content *before* modification
                        let current_content = match fs::read_to_string(&path) {
                            Ok(content) => Some(content),
                            Err(e) => {
                                // If the file doesn't exist, it's an error for insert, but technically the previous state is "doesn't exist"
                                warn!(error = %e, file_path = %file_path, "File not found for insert, proceeding but undo will delete.");
                                None
                            }
                        };
                        crate::state::update_undo_state(state, path.clone(), current_content); // Use crate::state::
                        // --- End Undo State Update ---
                        match fs::read_to_string(&file_path) {
                            Ok(content) => {
                                let mut lines: Vec<String> = content.lines().map(String::from).collect();
                                if line_usize == 0 || line_usize > lines.len() + 1 { // Allow inserting at the end (len + 1)
                                    lines.push(text_to_insert);
                                } else {
                                    lines.insert(line_usize - 1, text_to_insert);
                                }
                                let new_content = lines.join("\n");
                                match fs::write(&file_path, new_content) {
                                    Ok(_) => Ok(json!({ "success": true, "message": format!("Inserted text into '{}' at line {}.", file_path, line_usize) })),
                                    Err(e) => Err(json!({ "error": format!("Failed to write updated file '{}': {}", file_path, e) })),
                                }
                            }
                            Err(e) if e.kind() == std::io::ErrorKind::NotFound => { // File doesn't exist, create it
                                match fs::write(&file_path, text_to_insert) {
                                    Ok(_) => Ok(json!({ "success": true, "message": format!("Created file '{}' with inserted text.", file_path) })),
                                    Err(write_err) => Err(json!({ "error": format!("Failed to create file '{}' for insert: {}", file_path, write_err) })),
                                }
                            },
                            Err(e) => Err(json!({ "error": format!("Failed to read file '{}' for insert: {}", file_path, e) })),
                        }
                    }
                    (Err(e), _, _) | (_, Err(e), _) | (_, _, Err(e)) => Err(e),
                }
            }
            "text_editor_str_replace" => {
                match (
                    get_string_param(input, "file_path"),
                    get_string_param(input, "find_text"),
                    get_string_param(input, "replace_text")
                ) {
                    (Ok(file_path), Ok(find_text), Ok(replace_text)) => {
                        // --- Undo State Update ---
                        let path = PathBuf::from(file_path.clone());
                        let current_content = match fs::read_to_string(&path) {
                            Ok(content) => Some(content),
                            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None, // File doesn't exist yet, treat as create
                            Err(e) => return Err(json!({ "status": "error", "message": format!("Failed to read file '{}' before replace: {}", file_path, e) })),
                        };
                        crate::state::update_undo_state(state, path.clone(), current_content); // Use crate::state::
                        // --- End Undo State Update ---

                        match str_replace_editor(file_path.clone(), find_text, replace_text) {
                            Ok(msg) => Ok(json!({ "success": true, "message": msg })),
                            Err(e) => Err(json!({ "error": format!("Failed to replace text in file '{}': {}", file_path, e) })),
                        }
                    }
                    (Err(e), _, _) | (_, Err(e), _) | (_, _, Err(e)) => Err(e),
                }
            }
            "text_editor_undo_edit" => {
                let file_path_param = get_string_param(input, "file_path")?; // Get param for logging/confirmation if needed

                let mut last_edited_path_guard = state.last_edited_file.lock().unwrap();
                let mut previous_content_guard = state.previous_content.lock().unwrap();

                if let Some(path_to_undo) = last_edited_path_guard.take() {
                    // Verify param matches state if desired, though state is source of truth
                    if PathBuf::from(&file_path_param) != path_to_undo {
                        warn!(param_path=%file_path_param, state_path=?path_to_undo, "Undo called with path mismatch, using state path.");
                    }

                    if let Some(maybe_content) = previous_content_guard.take() {
                        match maybe_content {
                            Some(content) => {
                                // Last action was an edit, restore content
                                match fs::write(&path_to_undo, content) {
                                    Ok(_) => Ok(json!({ "status": "success", "message": format!("Undo successful for '{}'.", path_to_undo.display()) })),
                                    Err(e) => Err(json!({ "status": "error", "message": format!("Failed to write previous content during undo for '{}': {}", path_to_undo.display(), e) })),
                                }
                            }
                            None => {
                                // Last action was create, delete the file
                                match fs::remove_file(&path_to_undo) {
                                    Ok(_) => Ok(json!({ "status": "success", "message": format!("Undo successful for '{}' (file deleted).", path_to_undo.display()) })),
                                    // If it already doesn't exist, that's okay for undoing a create
                                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                                        Ok(json!({ "status": "success", "message": format!("Undo successful for '{}' (file was already deleted).", path_to_undo.display()) }))
                                    }
                                    Err(e) => Err(json!({ "status": "error", "message": format!("Failed to delete file during undo for '{}': {}", path_to_undo.display(), e) })),
                                }
                            }
                        }
                    } else {
                        // Should not happen if last_edited_path was Some, indicates state inconsistency
                        error!("Undo state inconsistency: last_edited_file was Some, but previous_content was None.");
                        Err(json!({ "status": "error", "message": "Internal error: Undo state inconsistent." }))
                    }

                } else {
                    Err(json!({ "status": "error", "message": "Nothing to undo." }))
                }
            }
            // --- Bash Handler ---
            "bash" => {
                match (
                    get_string_param(input, "command"),
                    get_optional_u64_param(input, "timeout_seconds"),
                    get_optional_bool_param(input, "restart"),
                ) {
                    (Ok(command), Ok(timeout_seconds), Ok(restart_opt)) => {
                        let restart = restart_opt.unwrap_or(true); // Default to true (fresh shell) if omitted
                        info!(
                            command = %command,
                            timeout = ?timeout_seconds,
                            restart = restart,
                            "Executing bash command"
                        );

                        // Handle restart parameter logic
                        if !restart {
                            // If restart is explicitly false, warn about lack of persistent state
                            warn!(
                                "Bash 'restart: false' requested, but persistent shell state is not currently supported. Command will run in a fresh shell instance."
                            );
                            // Continue execution as if restart=true, because that's the current behavior
                        } // If restart is true or omitted, current behavior is correct (fresh shell)

                        // TODO: Implement proper shell state management for 'restart: false' if needed in the future.

                        let mut cmd = Command::new("sh"); // Using sh -c for broader compatibility
                        cmd.arg("-c");
                        cmd.arg(&command);

                        // Basic timeout handling with std::process, more robust handling might need wait-timeout crate
                        let timeout_duration = timeout_seconds.map(Duration::from_secs);

                        // Spawn the child process
                        match cmd.spawn() {
                            Ok(mut child) => {
                                let status_result = if let Some(duration) = timeout_duration {
                                    match child.wait_timeout(duration) {
                                        Ok(Some(status)) => Ok(status),
                                        Ok(None) => { // Timeout occurred
                                            warn!(command = %command, "Command timed out, killing process...");
                                            child.kill().map_err(|e| format!("Failed to kill timed out process: {}", e))?;
                                            // Return an error indicating timeout explicitly
                                            Err("Command timed out".to_string())
                                        }
                                        Err(e) => Err(format!(
                                            "Failed to wait for child process with timeout: {}",
                                            e
                                        )),
                                    }
                                } else {
                                    child.wait().map_err(|e| {
                                        format!("Failed to wait for child process: {}", e)
                                    })
                                };

                                match status_result {
                                    Ok(status) => {
                                        // Attempt to get stdout/stderr - Note: This requires capturing output
                                        // Modify cmd.spawn() to cmd.output() if synchronous capture is acceptable
                                        // Or use async process handling if needed.
                                        // For now, stick to exit code and timeout status.
                                        let stdout_content = "(stdout not captured)".to_string(); // Placeholder
                                        let stderr_content = "(stderr not captured)".to_string(); // Placeholder

                                        Ok(json!({
                                            "success": status.success(),
                                            "stdout": stdout_content,
                                            "stderr": stderr_content,
                                            "exit_code": status.code(),
                                            "timed_out": false // Explicitly false if finished normally
                                        }))
                                    }
                                    Err(e) => {
                                        // Check if the error is our specific timeout message
                                        if e == "Command timed out" {
                                            Err(json!({
                                                "error": e,
                                                "timed_out": true,
                                                "stdout": "(stdout not captured on timeout)",
                                                "stderr": "(stderr not captured on timeout)"
                                            }))
                                        } else {
                                            Err(json!({ "error": e }))
                                        }
                                    }
                                }
                            }
                            Err(e) => Err(json!({
                                "error": format!("Failed to spawn command '{}': {}", command, e)
                            })),
                        }
                    }
                    (Err(e), _, _) | (_, Err(e), _) | (_, _, Err(e)) => {
                        Err(json!({ "error": format!("Failed to parse command arguments: {}", e) }))
                    } // Corrected structure
                }
            }
            // --- Standard Tool Implementations ---
            "read_file" => { // Reverted to use std::fs
                match get_string_param(input, "path") {
                    Ok(path) => {
                        info!(path = %path, "Reading file");
                        match fs::read_to_string(&path) {
                            Ok(content) => Ok(json!({ "success": true, "content": content })),
                            Err(e) => {
                                error!(path = %path, error = %e, "Failed to read file");
                                Err(json!({ "error": format!("Failed to read file '{}': {}", path, e) }))
                            }
                        }
                    }
                    Err(e) => Err(e),
                }
            }
            "write_file" => { // Reverted to use std::fs
                match (get_string_param(input, "path"), get_string_param(input, "content")) {
                    (Ok(path), Ok(content)) => {
                        info!(path = %path, content_length = content.len(), "Writing file");
                        match fs::write(&path, &content) {
                            Ok(_) => Ok(json!({ "success": true, "message": "File written." })),
                            Err(e) => {
                                error!(path = %path, error = %e, "Failed to write file");
                                Err(json!({ "error": format!("Failed to write file '{}': {}", path, e) }))
                            }
                        }
                    }
                    (Err(e), _) | (_, Err(e)) => Err(e),
                }
            }
            "get_element_by_description" => {
                let description = get_string_param(input, "description")?;
                info!(description = %description, "Executing get_element_by_description");

                // Use the locator API provided by the Desktop object
                let selector = computer_use_ai_sdk::Selector::Description(description.clone()); // Use the Selector from the SDK
                match desktop.locator(selector).first() {
                    Ok(Some(element)) => {
                        let attrs = element.attributes(); // Get attributes
                        match serde_json::to_value(&attrs) {
                            Ok(element_info) => Ok(json!({ "success": true, "element": element_info })),
                            Err(e) => Err(json!({ "error": format!("Failed to serialize found element info: {}", e) }))
                        }
                    },
                    Ok(None) => {
                         Err(json!({ "error": format!("Element with description '{}' not found.", description) }))
                    }
                    Err(e) => {
                        Err(json!({ "error": format!("Failed to find element by description '{}': {}", description, e) }))
                    },
                }
            }
            "get_element_tree" => {
                info!("Executing get_element_tree on focused element");
                // Get the focused element first
                match desktop.focused_element() {
                    Ok(focused_element) => {
                        // Call get_tree on the focused element
                        match focused_element.get_tree() { // get_tree is on UIElement
                            Ok(tree_info) => {
                                match serde_json::to_value(&tree_info) { // Serialize the result
                                    Ok(json_tree) => Ok(json!({ "success": true, "tree": json_tree })),
                                    Err(e) => Err(json!({ "error": format!("Failed to serialize element tree: {}", e) }))
                                }
                            },
                            Err(e) => Err(json!({ "error": format!("Failed to get element tree from focused element: {}", e) }))
                        }
                    },
                    Err(e) => Err(json!({ "error": format!("Failed to get focused element to retrieve tree: {}", e) }))
                }
            }
            "get_clipboard_content" => {
                match desktop.get_clipboard_content() {
                    Ok(content) => Ok(json!({ "success": true, "content": content })),
                    Err(e) => Err(json!({ "error": format!("Failed to get clipboard content: {}", e) })),
                }
            }
            "set_clipboard_content" => {
                match get_string_param(input, "content") {
                    Ok(content) => match desktop.set_clipboard_content(&content) {
                        Ok(_) => Ok(json!({ "success": true, "message": "Clipboard content set." })),
                        Err(e) => Err(json!({ "error": format!("Failed to set clipboard content: {}", e) })),
                    },
                    Err(e) => Err(e),
                }
            }
            // --- Newly Added Tool Handlers (Placeholders/Implementations) ---
            "get_browser_info" => {
                #[cfg(target_os = "macos")]
                {
                    info!("Executing get_browser_info for Safari");
                    // AppleScript to get URL and Title from the front Safari window's current tab
                    let script = r#"
                        tell application "Safari"
                            if it is running then
                                try
                                    set currentURL to URL of current tab of front window
                                    set currentTitle to name of current tab of front window
                                    return "{\"url\":\"" & currentURL & "\", \"title\":\"" & currentTitle & "\"}"
                                on error errMsg number errorNumber
                                    # Return error details as JSON string
                                    return "{\"error\": \"Failed to get Safari info: " & errMsg & " (" & (errorNumber as string) & ")\"}"
                                end try
                            else
                                return "{\"error\": \"Safari is not running\"}"
                            end if
                        end tell
                    "#;

                    match Command::new("osascript").arg("-e").arg(script).output() {
                        Ok(output) => {
                            let result_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
                            if output.status.success() {
                                // Attempt to parse the JSON string returned by AppleScript
                                match serde_json::from_str::<Value>(&result_str) {
                                    Ok(json_value) => {
                                        // Check if the parsed JSON contains an error key (from the script itself)
                                        if json_value.get("error").is_some() {
                                            warn!(script_error = %result_str, "get_browser_info AppleScript reported an error");
                                            Err(json_value) // Return the error JSON from the script
                                        } else {
                                            Ok(json!({ "success": true, "browser_info": json_value }))
                                        }
                                    },
                                    Err(parse_err) => {
                                        error!(output = %result_str, error = %parse_err, "Failed to parse AppleScript output as JSON");
                                        Err(json!({ "error": format!("Failed to parse browser info from script: {}", parse_err), "raw_output": result_str }))
                                    }
                                }
                            } else {
                                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                                warn!(stdout = %result_str, stderr = %stderr, "osascript execution failed for get_browser_info");
                                // Attempt to parse stdout as potential error JSON from script, otherwise use stderr
                                match serde_json::from_str::<Value>(&result_str) {
                                    Ok(json_value) if json_value.get("error").is_some() => Err(json_value),
                                    _ => Err(json!({ "error": format!("osascript failed: {}", stderr), "stdout": result_str }))
                                }
                            }
                        }
                        Err(e) => {
                            error!(error = %e, "Failed to run osascript command for get_browser_info");
                            Err(json!({ "error": format!("Failed to execute osascript for browser info: {}", e) }))
                        }
                    }
                }
                #[cfg(not(target_os = "macos"))]
                {
                    Err(json!({ "error": "get_browser_info is only implemented for macOS (Safari) currently." }))
                }
            }
            "run_applescript" => {
                #[cfg(target_os = "macos")]
                {
                    match get_string_param(input, "script") {
                        Ok(script) => {
                            info!(script = %script, "Executing AppleScript");
                            match Command::new("osascript").arg("-e").arg(&script).output() {
                                Ok(output) => {
                                    if output.status.success() {
                                        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                                        Ok(json!({ "success": true, "result": stdout }))
                                    } else {
                                        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                                        warn!(stderr = %stderr, "AppleScript execution failed");
                                        Err(json!({ "error": format!("AppleScript execution failed: {}", stderr) }))
                                    }
                                }
                                Err(e) => {
                                    error!(error = %e, "Failed to run osascript command");
                                    Err(json!({ "error": format!("Failed to execute osascript: {}", e) }))
                                }
                            }
                        }
                        Err(e) => Err(e),
                    }
                }
                #[cfg(not(target_os = "macos"))]
                {
                    Err(json!({ "error": "run_applescript is only available on macOS." }))
                }
            }
            "get_screen_text" => {
                // This requires an OCR engine integration.
                Err(json!({ "error": "Tool 'get_screen_text' not implemented yet." }))
            }
            // --- Unknown Tool ---
            _ => Err(json!({ "error": format!("Unknown tool name: {}", tool_name) })),
        }
    };

    result.await
}
