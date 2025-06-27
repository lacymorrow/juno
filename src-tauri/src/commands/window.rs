// Commands related to window management (list, info, focus, scroll)

use crate::state::AppState;
use computer_use_ai_sdk::{AutomationError, UIElement};
use tauri::{AppHandle, State};
use serde::Serialize;
use serde_json;
use tracing::{info, error};
use super::send_dev_tool_notification; // Use helper from parent module

#[derive(Serialize)]
struct WindowInfo {
    id: String,
    title: String,
    // Add other fields as needed from UIElementAttributes if available
    // e.g., pid: Option<i32>,
    // bounds: Option<(i32, i32, i32, i32)>,
}

// Helper function moved here as it's only used by window commands
fn find_window_by_id(state: &State<'_, AppState>, window_id: &str) -> Result<Option<UIElement>, String> {
    let desktop = &state.desktop;
    match desktop.list_windows() {
        Ok(windows) => {
            // 1) Try exact ID match first
            for window in &windows {
                if let Some(id) = window.id() {
                    if id == window_id {
                        return Ok(Some(window.clone()));
                    }
                }
            }

            // 2) Fallback: if the provided "window_id" is numeric, treat it as the index within the window list
            if let Ok(index) = window_id.parse::<usize>() {
                if index < windows.len() {
                    return Ok(Some(windows[index].clone()));
                }
            }

            Ok(None) // Not found by ID or index
        }
        Err(e) => Err(format!("Failed to list windows: {}", e)),
    }
}

// CONSOLIDATED: dev_scroll_window removed - use scroll_window production function

// CONSOLIDATED: dev_get_window_list removed - use get_window_list production function


// CONSOLIDATED: dev_get_window_info removed - use get_window_info production function

// CONSOLIDATED: dev_focus_window removed - use focus_window production function

// --- PRODUCTION WINDOW FUNCTIONS WITH DEBUG CAPABILITIES ---
// These functions replace the dev_ prefixed functions by incorporating debug features conditionally

#[tauri::command]
pub(crate) async fn scroll_window(
    direction: String,
    scroll_amount: f64,
    x: Option<f64>,
    y: Option<f64>,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    use crate::commands::debug_utils::{DebugConfig, should_enable_debug, log_debug_operation, send_debug_notification, validators};

    let debug_enabled = should_enable_debug(false, &state);
    let debug_config = if debug_enabled { DebugConfig::development_mode() } else { DebugConfig::production_mode() };

    // Debug validation
    if debug_config.validate_inputs {
        let valid_directions = ["up", "down", "left", "right"];
        if !valid_directions.contains(&direction.as_str()) {
            return Err(format!(
                "Invalid scroll direction: '{}'. Must be one of: {:?}",
                direction, valid_directions
            ));
        }

        if scroll_amount <= 0.0 {
            return Err("Scroll amount must be greater than 0".to_string());
        }

        if let (Some(px), Some(py)) = (x, y) {
            validators::valid_coordinates(px, py)?;
        }
    }

    let operation_desc = match (x, y) {
        (Some(px), Some(py)) => format!("scroll {} by {} units at ({}, {})", direction, scroll_amount, px, py),
        _ => format!("scroll {} by {} units at current position", direction, scroll_amount),
    };

    log_debug_operation("scroll_window", &operation_desc, &debug_config);
    info!("Executing scroll_window: {}", operation_desc);

    let result: Result<(), AutomationError>;
    let action_desc: String;

    #[cfg(target_os = "macos")]
    {
        match (x, y) {
            (Some(px), Some(py)) => {
                let desktop = &state.desktop;
                result = desktop.scroll_at_position(px, py, &direction, scroll_amount).map_err(|e| AutomationError::Internal(e));
                action_desc = format!("Scrolled {} by {} at ({}, {})", direction, scroll_amount, px, py);
            }
            _ => {
                let desktop = &state.desktop;
                result = desktop.scroll_at_current_position(&direction, scroll_amount).map_err(|e| AutomationError::Internal(e));
                action_desc = format!("Scrolled {} by {} at current position", direction, scroll_amount);
            }
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        result = Err(AutomationError::UnsupportedPlatform("macOS specific functionality not available on this platform".to_string()));
        action_desc = "Scroll (Unsupported Platform)".to_string();
    }

    match result {
        Ok(_) => {
            info!("Successfully executed scroll_window: {}", action_desc);

            // Send debug notification if enabled
            if debug_config.send_notifications {
                let _ = send_debug_notification(&app, "Scroll Window", &action_desc);
            }

            Ok(())
        }
        Err(e) => {
            let error_msg = format!("Failed to perform scroll action ({}): {}", action_desc, e);
            error!("{}", error_msg);
            Err(error_msg)
        }
    }
}

#[tauri::command]
pub(crate) async fn get_window_list(app: AppHandle, state: State<'_, AppState>) -> Result<String, String> {
    use crate::commands::debug_utils::{DebugConfig, should_enable_debug, log_debug_operation, send_debug_notification};

    let debug_enabled = should_enable_debug(false, &state);
    let debug_config = if debug_enabled { DebugConfig::development_mode() } else { DebugConfig::production_mode() };

    log_debug_operation("get_window_list", "Getting list of all windows", &debug_config);
    info!("Executing get_window_list");

    let desktop = &state.desktop;
    match desktop.list_windows() {
        Ok(windows) => {
            info!("Successfully retrieved window list. Found {} windows.", windows.len());

            let mut window_infos: Vec<WindowInfo> = Vec::new();
            for win in windows {
                let attrs = win.attributes();
                let title = attrs.label.unwrap_or_else(|| "Untitled Window".to_string());

                match win.id() {
                    Some(id) => {
                        window_infos.push(WindowInfo { id, title });
                    }
                    None => {
                        window_infos.push(WindowInfo { id: "<no_id>".to_string(), title });
                    }
                }
            }

            match serde_json::to_string_pretty(&window_infos) {
                Ok(json_string) => {
                    // Send debug notification if enabled
                    if debug_config.send_notifications {
                        let _ = send_debug_notification(&app, "Window List", &format!("Retrieved {} windows", window_infos.len()));
                    }

                    Ok(json_string)
                }
                Err(e) => {
                    let error_msg = format!("Failed to serialize window list: {}", e);
                    error!("{}", error_msg);
                    Err(error_msg)
                }
            }
        }
        Err(e) => {
            let error_msg = format!("Failed to call desktop.windows(): {}", e);
            error!("{}", error_msg);
            Err(error_msg)
        }
    }
}

#[tauri::command]
pub(crate) async fn get_window_info(window_id: String, app: AppHandle, state: State<'_, AppState>) -> Result<String, String> {
    use crate::commands::debug_utils::{DebugConfig, should_enable_debug, log_debug_operation, send_debug_notification, validators};

    let debug_enabled = should_enable_debug(false, &state);
    let debug_config = if debug_enabled { DebugConfig::development_mode() } else { DebugConfig::production_mode() };

    // Debug validation
    if debug_config.validate_inputs {
        validators::non_empty_text(&window_id)?;
    }

    log_debug_operation("get_window_info", &format!("Getting info for window ID/index: {}", window_id), &debug_config);
    info!("Executing get_window_info for window: {}", window_id);

    match find_window_by_id(&state, &window_id) {
        Ok(Some(window)) => {
            info!("Found target window: {}", window_id);

            let attrs_result = window.get_all_attributes();
            let attrs_to_serialize = match attrs_result {
                Ok(all_attrs) => all_attrs,
                Err(_) => window.attributes()
            };

            match serde_json::to_string_pretty(&attrs_to_serialize) {
                Ok(json_string) => {
                    // Send debug notification if enabled
                    if debug_config.send_notifications {
                        let _ = send_debug_notification(&app, "Window Info", &format!("Retrieved info for window: {}", window_id));
                    }

                    Ok(json_string)
                }
                Err(e) => {
                    let error_msg = format!("Failed to serialize window attributes: {}", e);
                    error!("{}", error_msg);
                    Err(error_msg)
                }
            }
        }
        Ok(None) => {
            let error_msg = format!("Window with ID or index '{}' not found.", window_id);
            error!("{}", error_msg);
            Err(error_msg)
        }
        Err(e) => {
            error!("Error while searching for window: {}", e);
            Err(e)
        }
    }
}

#[tauri::command]
pub(crate) async fn focus_window(window_id: String, app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    use crate::commands::debug_utils::{DebugConfig, should_enable_debug, log_debug_operation, send_debug_notification, validators};

    let debug_enabled = should_enable_debug(false, &state);
    let debug_config = if debug_enabled { DebugConfig::development_mode() } else { DebugConfig::production_mode() };

    // Debug validation
    if debug_config.validate_inputs {
        validators::non_empty_text(&window_id)?;
    }

    log_debug_operation("focus_window", &format!("Focusing window ID: {}", window_id), &debug_config);
    info!("Executing focus_window for window: {}", window_id);

    match find_window_by_id(&state, &window_id) {
        Ok(Some(window)) => {
            match window.focus() {
                Ok(_) => {
                    info!("Successfully focused window: {}", window_id);

                    // Send debug notification if enabled
                    if debug_config.send_notifications {
                        let _ = send_debug_notification(&app, "Focus Window", &format!("Focused window: {}", window_id));
                    }

                    Ok(())
                }
                Err(e) => {
                    let error_msg = format!("Failed to focus window '{}': {}", window_id, e);
                    error!("{}", error_msg);
                    Err(error_msg)
                }
            }
        }
        Ok(None) => {
            let error_msg = format!("Window with ID '{}' not found for focusing.", window_id);
            error!("{}", error_msg);
            Err(error_msg)
        }
        Err(e) => {
            error!("Error finding window for focus: {}", e);
            Err(e)
        }
    }
}

#[tauri::command]
pub(crate) async fn resize_window(
    window_id: String,
    width: i32,
    height: i32,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    use crate::commands::debug_utils::{DebugConfig, should_enable_debug, log_debug_operation, send_debug_notification, validators};

    let debug_enabled = should_enable_debug(false, &state);
    let debug_config = if debug_enabled { DebugConfig::development_mode() } else { DebugConfig::production_mode() };

    // Debug validation
    if debug_config.validate_inputs {
        validators::non_empty_text(&window_id)?;
        if width <= 0 {
            return Err("Width must be greater than 0".to_string());
        }
        if height <= 0 {
            return Err("Height must be greater than 0".to_string());
        }
    }

    log_debug_operation("resize_window", &format!("Resizing window ID: {} to {}x{}", window_id, width, height), &debug_config);
    info!("Executing resize_window for window: {} to size {}x{}", window_id, width, height);

    match find_window_by_id(&state, &window_id) {
        Ok(Some(window)) => {
            // Check if this is a window element
            let role = window.attributes().role;
            if role != "window" && role != "AXWindow" {
                let error_msg = format!("Element with ID '{}' is not a window (role: {})", window_id, role);
                error!("{}", error_msg);
                return Err(error_msg);
            }

            // For macOS, use the desktop's accessibility APIs to resize the window
            #[cfg(target_os = "macos")]
            {
                // First focus the window, then resize it
                match window.focus() {
                    Ok(_) => {
                        info!("Successfully focused window '{}'", window_id);

                        // Small delay to ensure focus has taken effect - use async sleep
                        tokio::time::sleep(tokio::time::Duration::from_millis(crate::constants::mouse::delays::WINDOW_FOCUS_DELAY_MS)).await;

                        // Now resize the focused window using the desktop's engine
                        let desktop = state.get_desktop()?;

                        match desktop.engine().resize_window(width as f64, height as f64) {
                            Ok(_) => {
                                info!("Successfully resized window '{}' to size {}x{}", window_id, width, height);

                                // Send debug notification if enabled
                                if debug_config.send_notifications {
                                    let _ = send_debug_notification(&app, "Resize Window", &format!("Resized window '{}' to {}x{}", window_id, width, height));
                                }

                                Ok(())
                            }
                            Err(e) => {
                                let error_msg = format!("Failed to resize window '{}': {}", window_id, e);
                                error!("{}", error_msg);
                                Err(error_msg)
                            }
                        }
                    }
                    Err(e) => {
                        let error_msg = format!("Failed to focus window '{}' before resizing: {}", window_id, e);
                        error!("{}", error_msg);
                        Err(error_msg)
                    }
                }
            }

            #[cfg(not(target_os = "macos"))]
            {
                let error_msg = "Window resizing is only supported on macOS".to_string();
                error!("{}", error_msg);
                Err(error_msg)
            }
        }
        Ok(None) => {
            let error_msg = format!("Window with ID '{}' not found for resizing.", window_id);
            error!("{}", error_msg);
            Err(error_msg)
        }
        Err(e) => {
            error!("Error finding window for resize: {}", e);
            Err(e)
        }
    }
}

#[tauri::command]
pub(crate) async fn move_window(
    window_id: String,
    x: i32,
    y: i32,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    use crate::commands::debug_utils::{DebugConfig, should_enable_debug, log_debug_operation, send_debug_notification, validators};

    let debug_enabled = should_enable_debug(false, &state);
    let debug_config = if debug_enabled { DebugConfig::development_mode() } else { DebugConfig::production_mode() };

    // Debug validation
    if debug_config.validate_inputs {
        validators::non_empty_text(&window_id)?;
    }

    log_debug_operation("move_window", &format!("Moving window ID: {} to ({}, {})", window_id, x, y), &debug_config);
    info!("Executing move_window for window: {} to position ({}, {})", window_id, x, y);

    match find_window_by_id(&state, &window_id) {
        Ok(Some(window)) => {
            // Check if this is a window element
            let role = window.attributes().role;
            if role != "window" && role != "AXWindow" {
                let error_msg = format!("Element with ID '{}' is not a window (role: {})", window_id, role);
                error!("{}", error_msg);
                return Err(error_msg);
            }

            // For macOS, use the desktop's accessibility APIs to move the window
            #[cfg(target_os = "macos")]
            {
                // First focus the window, then move it
                match window.focus() {
                    Ok(_) => {
                        info!("Successfully focused window '{}'", window_id);

                        // Small delay to ensure focus has taken effect - use async sleep
                        tokio::time::sleep(tokio::time::Duration::from_millis(crate::constants::mouse::delays::WINDOW_FOCUS_DELAY_MS)).await;

                        // Now move the focused window using the desktop's engine
                        let desktop = state.get_desktop()?;

                        match desktop.engine().move_window(x as f64, y as f64) {
                            Ok(_) => {
                                info!("Successfully moved window '{}' to position ({}, {})", window_id, x, y);

                                // Send debug notification if enabled
                                if debug_config.send_notifications {
                                    let _ = send_debug_notification(&app, "Move Window", &format!("Moved window '{}' to ({}, {})", window_id, x, y));
                                }

                                Ok(())
                            }
                            Err(e) => {
                                let error_msg = format!("Failed to move window '{}': {}", window_id, e);
                                error!("{}", error_msg);
                                Err(error_msg)
                            }
                        }
                    }
                    Err(e) => {
                        let error_msg = format!("Failed to focus window '{}' before moving: {}", window_id, e);
                        error!("{}", error_msg);
                        Err(error_msg)
                    }
                }
            }

            #[cfg(not(target_os = "macos"))]
            {
                let error_msg = "Window moving is only supported on macOS".to_string();
                error!("{}", error_msg);
                Err(error_msg)
            }
        }
        Ok(None) => {
            let error_msg = format!("Window with ID '{}' not found for moving.", window_id);
            error!("{}", error_msg);
            Err(error_msg)
        }
        Err(e) => {
            error!("Error finding window for move: {}", e);
            Err(e)
        }
    }
}

#[tauri::command]
pub(crate) async fn close_window(
    window_id: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    use crate::commands::debug_utils::{DebugConfig, should_enable_debug, log_debug_operation, send_debug_notification, validators};

    let debug_enabled = should_enable_debug(false, &state);
    let debug_config = if debug_enabled { DebugConfig::development_mode() } else { DebugConfig::production_mode() };

    // Debug validation
    if debug_config.validate_inputs {
        validators::non_empty_text(&window_id)?;
    }

    log_debug_operation("close_window", &format!("Closing window ID: {}", window_id), &debug_config);
    info!("Executing close_window for window: {}", window_id);

    match find_window_by_id(&state, &window_id) {
        Ok(Some(window)) => {
            // Check if this is a window element
            let role = window.attributes().role;
            if role != "window" && role != "AXWindow" {
                let error_msg = format!("Element with ID '{}' is not a window (role: {})", window_id, role);
                error!("{}", error_msg);
                return Err(error_msg);
            }

            // For macOS, use the desktop's accessibility APIs to close the window
            #[cfg(target_os = "macos")]
            {
                // First focus the window, then close it
                match window.focus() {
                    Ok(_) => {
                        info!("Successfully focused window '{}'", window_id);

                        // Small delay to ensure focus has taken effect - use async sleep
                        tokio::time::sleep(tokio::time::Duration::from_millis(crate::constants::mouse::delays::WINDOW_FOCUS_DELAY_MS)).await;

                        // Now close the focused window using the desktop's engine
                        let desktop = state.get_desktop()?;

                        match desktop.engine().close_window() {
                            Ok(_) => {
                                info!("Successfully closed window '{}'", window_id);

                                // Send debug notification if enabled
                                if debug_config.send_notifications {
                                    let _ = send_debug_notification(&app, "Close Window", &format!("Closed window '{}'", window_id));
                                }

                                Ok(())
                            }
                            Err(e) => {
                                let error_msg = format!("Failed to close window '{}': {}", window_id, e);
                                error!("{}", error_msg);
                                Err(error_msg)
                            }
                        }
                    }
                    Err(e) => {
                        let error_msg = format!("Failed to focus window '{}' before closing: {}", window_id, e);
                        error!("{}", error_msg);
                        Err(error_msg)
                    }
                }
            }

            #[cfg(not(target_os = "macos"))]
            {
                let error_msg = "Window closing is only supported on macOS".to_string();
                error!("{}", error_msg);
                Err(error_msg)
            }
        }
        Ok(None) => {
            let error_msg = format!("Window with ID '{}' not found for closing.", window_id);
            error!("{}", error_msg);
            Err(error_msg)
        }
        Err(e) => {
            error!("Error finding window for close: {}", e);
            Err(e)
        }
    }
}
