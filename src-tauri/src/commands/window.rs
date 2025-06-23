// Commands related to window management (list, info, focus, scroll)

use crate::state::AppState;
use computer_use_ai_sdk::{AutomationError, UIElement};
use tauri::{AppHandle, State};
use serde::Serialize;
use serde_json;
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

#[tauri::command]
pub async fn scroll_window(
    app: AppHandle,
    state: State<'_, AppState>,
    direction: String,
    scroll_amount: f64,
    x: Option<f64>,
    y: Option<f64>,
    debug_mode: Option<bool>,
) -> Result<(), String> {
    use crate::commands::debug_utils::{should_enable_debug, log_debug_operation, send_debug_notification, time_operation};

    let debug = should_enable_debug(debug_mode, &state);
    let start_time = std::time::Instant::now();

    // Validate direction
    let valid_directions = ["up", "down", "left", "right"];
    if !valid_directions.contains(&direction.as_str()) {
        let err_msg = format!(
            "Invalid scroll direction: '{}'. Must be one of: {:?}",
            direction, valid_directions
        );
        if debug {
            log_debug_operation("scroll_window", &format!("Error: {}", err_msg));
        }
        return Err(err_msg);
    }

    let result: Result<(), AutomationError>;
    let action_desc: String;

    #[cfg(target_os = "macos")]
    {
        match (x, y) {
            (Some(px), Some(py)) => {
                if debug {
                    log_debug_operation("scroll_window", &format!("Attempting to scroll {} by {} units at position ({}, {})", direction, scroll_amount, px, py));
                }
                let desktop = &state.desktop;
                result = desktop.scroll_at_position(px, py, &direction, scroll_amount).map_err(|e| AutomationError::Internal(e));
                action_desc = format!("Scrolled {} by {} at ({}, {})", direction, scroll_amount, px, py);
            }
            _ => {
                if debug {
                    log_debug_operation("scroll_window", &format!("Attempting to scroll {} by {} units at current position", direction, scroll_amount));
                }
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

    if debug {
        let duration = time_operation(start_time);
        match &result {
            Ok(_) => {
                log_debug_operation("scroll_window", &format!("{} succeeded in {:.2}ms", action_desc, duration));
                send_debug_notification(&app, "Scroll", &format!("{} ({:.2}ms)", action_desc, duration))?;
            }
            Err(e) => {
                let err_msg = format!("Failed to perform scroll action ({}): {}", action_desc, e);
                log_debug_operation("scroll_window", &format!("Error: {}", err_msg));
            }
        }
    }

    result.map_err(|e| format!("Failed to perform scroll action ({}): {}", action_desc, e))
}

#[tauri::command]
pub async fn get_window_list(
    app: AppHandle,
    state: State<'_, AppState>,
    debug_mode: Option<bool>,
) -> Result<String, String> {
    use crate::commands::debug_utils::{should_enable_debug, log_debug_operation, send_debug_notification, time_operation};

    let debug = should_enable_debug(debug_mode, &state);
    let start_time = std::time::Instant::now();

    if debug {
        log_debug_operation("get_window_list", "Attempting to get window list");
    }

    let desktop = &state.desktop;
    match desktop.list_windows() {
        Ok(windows) => {
            if debug {
                log_debug_operation("get_window_list", &format!("Found {} windows", windows.len()));
            }

            let mut window_infos: Vec<WindowInfo> = Vec::new();
            for win in windows {
                let attrs = win.attributes();
                let title = attrs.label.unwrap_or_else(|| "Untitled Window".to_string());
                match win.id() {
                    Some(id) => {
                        window_infos.push(WindowInfo { id, title });
                    }
                    None => {
                        if debug {
                            log_debug_operation("get_window_list", &format!("Window found with no ID. Title: {}", title));
                        }
                        window_infos.push(WindowInfo { id: "<no_id>".to_string(), title });
                    }
                }
            }

            match serde_json::to_string_pretty(&window_infos) {
                Ok(json_string) => {
                    if debug {
                        let duration = time_operation(start_time);
                        log_debug_operation("get_window_list", &format!("Retrieved window list in {:.2}ms", duration));
                        send_debug_notification(&app, "Window List", &format!("Retrieved {} windows ({:.2}ms)", window_infos.len(), duration))?;
                    }
                    Ok(json_string)
                }
                Err(e) => {
                    let err_msg = format!("Failed to serialize window list: {}", e);
                    if debug {
                        log_debug_operation("get_window_list", &format!("Error: {}", err_msg));
                    }
                    Err(err_msg)
                }
            }
        }
        Err(e) => {
            let err_msg = format!("Failed to call desktop.windows(): {}", e);
            if debug {
                log_debug_operation("get_window_list", &format!("Error: {}", err_msg));
            }
            Err(err_msg)
        }
    }
}

#[tauri::command]
pub async fn get_window_info(
    app: AppHandle,
    state: State<'_, AppState>,
    window_id: String,
    debug_mode: Option<bool>,
) -> Result<String, String> {
    use crate::commands::debug_utils::{should_enable_debug, log_debug_operation, send_debug_notification, time_operation};

    let debug = should_enable_debug(debug_mode, &state);
    let start_time = std::time::Instant::now();

    if debug {
        log_debug_operation("get_window_info", &format!("Getting info for window ID/index: {}", window_id));
    }

    match find_window_by_id(&state, &window_id) {
        Ok(Some(window)) => {
            if debug {
                log_debug_operation("get_window_info", "Found target window");
            }

            let attrs_result = window.get_all_attributes();
            let attrs_to_serialize = match attrs_result {
                Ok(all_attrs) => {
                    if debug {
                        log_debug_operation("get_window_info", "Using get_all_attributes result");
                    }
                    all_attrs
                }
                Err(e) => {
                    if debug {
                        log_debug_operation("get_window_info", &format!("get_all_attributes failed ({}), falling back to basic attributes", e));
                    }
                    window.attributes()
                }
            };

            match serde_json::to_string_pretty(&attrs_to_serialize) {
                Ok(json_string) => {
                    if debug {
                        let duration = time_operation(start_time);
                        log_debug_operation("get_window_info", &format!("Retrieved window info in {:.2}ms", duration));
                        send_debug_notification(&app, "Window Info", &format!("Retrieved window info ({:.2}ms)", duration))?;
                    }
                    Ok(json_string)
                }
                Err(e) => {
                    let err_msg = format!("Failed to serialize window attributes: {}", e);
                    if debug {
                        log_debug_operation("get_window_info", &format!("Error: {}", err_msg));
                    }
                    Err(err_msg)
                }
            }
        }
        Ok(None) => {
            let err_msg = format!("Window with ID or index '{}' not found.", window_id);
            if debug {
                log_debug_operation("get_window_info", &format!("Info: {}", err_msg));
            }
            Err(err_msg)
        }
        Err(e) => {
            if debug {
                log_debug_operation("get_window_info", &format!("Error while searching for window: {}", e));
            }
            Err(e)
        }
    }
}

#[tauri::command]
pub async fn focus_window(
    app: AppHandle,
    state: State<'_, AppState>,
    window_id: String,
    debug_mode: Option<bool>,
) -> Result<(), String> {
    use crate::commands::debug_utils::{should_enable_debug, log_debug_operation, send_debug_notification, time_operation};

    let debug = should_enable_debug(debug_mode, &state);
    let start_time = std::time::Instant::now();

    if debug {
        log_debug_operation("focus_window", &format!("Focusing window ID: {}", window_id));
    }

    match find_window_by_id(&state, &window_id) {
        Ok(Some(window)) => {
            match window.focus() {
                Ok(_) => {
                    if debug {
                        let duration = time_operation(start_time);
                        log_debug_operation("focus_window", &format!("Focus window succeeded in {:.2}ms", duration));
                        send_debug_notification(&app, "Focus Window", &format!("Window focused ({:.2}ms)", duration))?;
                    }
                    Ok(())
                }
                Err(e) => {
                    let err_msg = format!("Failed to focus window '{}': {}", window_id, e);
                    if debug {
                        log_debug_operation("focus_window", &format!("Error: {}", err_msg));
                    }
                    Err(err_msg)
                }
            }
        }
        Ok(None) => {
            let err_msg = format!("Window with ID '{}' not found for focusing.", window_id);
            if debug {
                log_debug_operation("focus_window", &format!("Info: {}", err_msg));
            }
            Err(err_msg)
        }
        Err(e) => {
            if debug {
                log_debug_operation("focus_window", &format!("Error finding window for focus: {}", e));
            }
            Err(e)
        }
    }
}

// --- BACKWARD COMPATIBILITY WRAPPERS ---

#[tauri::command]
pub(crate) async fn dev_scroll_window_compat(
    app: AppHandle,
    state: State<'_, AppState>,
    direction: String,
    scroll_amount: f64,
    x: Option<f64>,
    y: Option<f64>,
) -> Result<(), String> {
    scroll_window(app, state, direction, scroll_amount, x, y, Some(true)).await
}

#[tauri::command]
pub(crate) async fn dev_get_window_list_compat(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<String, String> {
    get_window_list(app, state, Some(true)).await
}

#[tauri::command]
pub(crate) async fn dev_get_window_info_compat(
    app: AppHandle,
    state: State<'_, AppState>,
    window_id: String,
) -> Result<String, String> {
    get_window_info(app, state, window_id, Some(true)).await
}

#[tauri::command]
pub(crate) async fn dev_focus_window_compat(
    app: AppHandle,
    state: State<'_, AppState>,
    window_id: String,
) -> Result<(), String> {
    focus_window(app, state, window_id, Some(true)).await
}

// --- DEV TOOL COMMANDS (Keep legacy versions with dev tool specific features) ---

#[tauri::command]
pub(crate) async fn dev_scroll_window(
    app: AppHandle,
    state: State<'_, AppState>,
    direction: String,      // "up", "down", "left", "right"
    scroll_amount: f64, // Number of units/clicks (changed back to f64 for SDK)
    x: Option<f64>,     // Optional x coordinate
    y: Option<f64>,     // Optional y coordinate
) -> Result<(), String> {
    // Validate direction
    let valid_directions = ["up", "down", "left", "right"];
    if !valid_directions.contains(&direction.as_str()) {
        let err_msg = format!(
            "Invalid scroll direction: '{}'. Must be one of: {:?}",
            direction, valid_directions
        );
        println!("[DEV_TOOL] Error: {}", err_msg);
        return Err(err_msg);
    }

    let result: Result<(), AutomationError>;
    let action_desc: String; // Declare without initializing

    #[cfg(target_os = "macos")]
    {
        match (x, y) {
            (Some(px), Some(py)) => {
                println!(
                    "[DEV_TOOL] Attempting to scroll {} by {} units at position ({}, {})...",
                    direction, scroll_amount, px, py
                );
                let desktop = &state.desktop;
                result = desktop.scroll_at_position(px, py, &direction, scroll_amount).map_err(|e| AutomationError::Internal(e));
                action_desc = format!( // Assign here
                    "Scrolled {} by {} at ({}, {})",
                    direction, scroll_amount, px, py
                );
            }
            _ => {
                println!(
                    "[DEV_TOOL] Attempting to scroll {} by {} units at current position...",
                    direction, scroll_amount
                );
                let desktop = &state.desktop;
                result = desktop.scroll_at_current_position(&direction, scroll_amount).map_err(|e| AutomationError::Internal(e));
                action_desc = format!( // Assign here
                    "Scrolled {} by {} at current position",
                    direction, scroll_amount
                );
            }
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        result = Err(AutomationError::UnsupportedPlatform("macOS specific functionality not available on this platform".to_string()));
        action_desc = "Scroll (Unsupported Platform)".to_string(); // Assign here
    }

    match result {
        Ok(_) => {
            println!("[DEV_TOOL] {} succeeded.", action_desc); // Now action_desc is definitely assigned
            send_dev_tool_notification(&app, "Scroll", &action_desc)?;
            Ok(())
        }
        Err(e) => {
            let err_msg = format!("Failed to perform scroll action ({}): {}", action_desc, e); // Now action_desc is definitely assigned
            println!("[DEV_TOOL] Error: {}", err_msg);
            Err(err_msg)
        }
    }
}

#[tauri::command]
pub(crate) async fn dev_get_window_list(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<String, String> {
    println!("[DEV_TOOL] Attempting to get window list...");

    let desktop = &state.desktop;
    match desktop.list_windows() {
        Ok(windows) => {
            println!("[DEV_TOOL] dev_get_window_list succeeded. Found {} windows.", windows.len());
            // Use a for loop for clearer error handling
            let mut window_infos: Vec<WindowInfo> = Vec::new();
            for win in windows {
                let attrs = win.attributes();
                let title = attrs.label.unwrap_or_else(|| "Untitled Window".to_string());
                // Handle the Option<String> returned by win.id()
                match win.id() { // Match the Option<String>
                    Some(id) => {
                        window_infos.push(WindowInfo { id, title });
                    }
                    None => {
                        println!("[DEV_TOOL] Window found with no ID (using placeholder). Title: {}", title);
                        window_infos.push(WindowInfo { id: "<no_id>".to_string(), title });
                        // If this case represents an error internally handled by the SDK,
                        // we might want to log differently or skip, but for now, treat it as 'no ID'.
                    }
                }
            }

            match serde_json::to_string_pretty(&window_infos) {
                Ok(json_string) => {
                     send_dev_tool_notification(&app, "Window List", "Retrieved window list.")?;
                    Ok(json_string)
                }
                Err(e) => {
                    let err_msg = format!("Failed to serialize window list: {}", e);
                    println!("[DEV_TOOL] Error: {}", err_msg);
                    Err(err_msg)
                }
            }
        }
        Err(e) => {
            let err_msg = format!("Failed to call desktop.windows(): {}", e);
            println!("[DEV_TOOL] Error: {}", err_msg);
            Err(err_msg)
        }
    }
}


#[tauri::command]
pub(crate) async fn dev_get_window_info(
    app: AppHandle,
    state: State<'_, AppState>,
    window_id: String,
) -> Result<String, String> {
    println!("[DEV_TOOL] Getting info for window ID/index: {}", window_id);

    match find_window_by_id(&state, &window_id) {
        Ok(Some(window)) => {
            println!("[DEV_TOOL] Found target window.");

            // Attempt to get detailed attributes first
            let attrs_result = window.get_all_attributes();
            let attrs_to_serialize = match attrs_result {
                Ok(all_attrs) => {
                    println!("[DEV_TOOL] Using get_all_attributes result.");
                    all_attrs
                }
                Err(e) => {
                    println!("[DEV_TOOL] get_all_attributes failed ({}), falling back to basic attributes.", e);
                    window.attributes()
                }
            };

            match serde_json::to_string_pretty(&attrs_to_serialize) {
                Ok(json_string) => {
                    send_dev_tool_notification(&app, "Window Info", "Retrieved window info.")?;
                    Ok(json_string)
                }
                Err(e) => {
                    let err_msg = format!("Failed to serialize window attributes: {}", e);
                    println!("[DEV_TOOL] Error: {}", err_msg);
                    Err(err_msg)
                }
            }
        }
        Ok(None) => {
            let err_msg = format!("Window with ID or index '{}' not found.", window_id);
            println!("[DEV_TOOL] Info: {}", err_msg);
            Err(err_msg)
        }
        Err(e) => {
            println!("[DEV_TOOL] Error while searching for window: {}", e);
            Err(e)
        }
    }
}

#[tauri::command]
pub(crate) async fn dev_focus_window(
    app: AppHandle,
    state: State<'_, AppState>,
    window_id: String,
) -> Result<(), String> {
    println!("[DEV_TOOL] Focusing window ID: {}", window_id);

    match find_window_by_id(&state, &window_id) {
        Ok(Some(window)) => {
            match window.focus() {
                Ok(_) => {
                    println!("[DEV_TOOL] Focus window succeeded.");
                    send_dev_tool_notification(&app, "Focus Window", "Window focused.")?;
                    Ok(())
                }
                Err(e) => {
                    let err_msg = format!("Failed to focus window '{}': {}", window_id, e);
                    println!("[DEV_TOOL] Error: {}", err_msg);
                    Err(err_msg)
                }
            }
        }
        Ok(None) => {
            let err_msg = format!("Window with ID '{}' not found for focusing.", window_id);
            println!("[DEV_TOOL] Info: {}", err_msg);
            Err(err_msg)
        }
        Err(e) => {
            // Error message already includes context from find_window_by_id
            println!("[DEV_TOOL] Error finding window for focus: {}", e);
            Err(e)
        }
    }
}
