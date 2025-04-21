use crate::state::AppState;
// use computer_use_ai_sdk::{AutomationError, Desktop, Selector, UIElement}; // Remove unused
use computer_use_ai_sdk::{Selector};
use computer_use_ai_sdk::AutomationError;
use tauri::{AppHandle, State}; // Remove unused Manager
use tauri_plugin_notification::NotificationExt;
use tracing::{info}; // Remove unused error
// use computer_use_ai_sdk::UIElementAttributes; // Removed unused import

#[cfg(target_os = "macos")]
use computer_use_ai_sdk::platforms::macos::utils as macos_utils;
#[cfg(target_os = "macos")]
use computer_use_ai_sdk::platforms::macos::element::{get_focused_element_ns_workspace, MacOSUIElement};

// Helper function (consider moving to utils.rs later)
fn send_dev_tool_notification(app: &tauri::AppHandle, title: &str, body: &str) -> Result<(), String> {
    app.notification()
        .builder()
        .title(title)
        .body(body)
        .show()
        .map_err(|e| format!("Failed to send notification: {}", e))
}

#[cfg(target_os = "macos")]
#[tauri::command]
pub(crate) async fn capture_screenshot_command(app: tauri::AppHandle) -> Result<String, String> {
    match macos_utils::capture_and_encode_screenshot() {
        Ok(base64_string) => {
            // Send notification on success
            app.notification()
                .builder()
                .title("Screenshot")
                .body("Screenshot captured successfully.")
                .show()
                .map_err(|e| format!("Failed to send notification: {}", e))?;
            Ok(base64_string)
        }
        Err(e) => Err(format!("Failed to capture screenshot: {}", e)),
    }
}

#[cfg(not(target_os = "macos"))]
#[tauri::command]
pub(crate) async fn capture_screenshot_command(_app: tauri::AppHandle) -> Result<String, String> {
    Err("Screenshot capture is only supported on macOS currently.".to_string())
}

#[tauri::command]
pub(crate) async fn list_apps(state: tauri::State<'_, AppState>) -> Result<Vec<String>, String> {
    match state.desktop.applications() {
        Ok(apps) => {
            let app_names = apps
                .into_iter()
                .map(|app| {
                    app.attributes()
                        .label
                        .unwrap_or_else(|| "Unknown Label".to_string())
                })
                .collect();
            Ok(app_names)
        }
        Err(e) => Err(format!("Failed to get applications: {}", e)),
    }
}

#[tauri::command]
pub(crate) fn check_server_status(state: tauri::State<'_, AppState>) -> bool {
    let _ = state.desktop;
    true
}

#[tauri::command]
pub(crate) async fn dev_get_focused_element_info(app: tauri::AppHandle, _state: tauri::State<'_, AppState>) -> Result<String, String> {
    println!("[DEV_TOOL] Attempting to get focused element info using NSWorkspace...");

    #[cfg(target_os = "macos")]
    let result = get_focused_element_ns_workspace(false, true);

    #[cfg(not(target_os = "macos"))]
    let result: Result<computer_use_ai_sdk::UIElement, AutomationError> = Err(AutomationError::UnsupportedPlatform);

    match result {
        Ok(element) => {
            println!("[DEV_TOOL] get_focused_element_info (NSWorkspace) succeeded.");
            // Send notification on success
             app.notification()
                .builder()
                .title("Focus Info")
                .body("Focused element info retrieved.")
                .show()
                .map_err(|e| format!("Failed to send notification: {}", e))?;

            let attrs = element.attributes();
            serde_json::to_string_pretty(&attrs).map_err(|e| {
                let err_msg = format!("Failed to serialize element info result: {}", e);
                println!("[DEV_TOOL] Error: {}", err_msg);
                err_msg
            })
        }
        Err(e) => {
            let err_msg = format!("Failed to call get_focused_element_info (NSWorkspace): {}", e);
            println!("[DEV_TOOL] Error: {}", err_msg);
            Err(err_msg)
        }
    }
}

#[cfg(target_os = "macos")]
#[tauri::command]
pub(crate) async fn capture_element_screenshot_command(
    app: AppHandle,
    _state: State<'_, AppState>,
) -> Result<String, String> {
    println!("[DEV_TOOL] Capturing focused element screenshot using NSWorkspace method...");

    let focused_element = match get_focused_element_ns_workspace(false, true) {
        Ok(el) => el,
        Err(e) => {
            let err_msg = format!("Failed to get focused element (NSWorkspace): {}", e);
            println!("[DEV_TOOL] Error: {}", err_msg);
            return Err(err_msg);
        }
    };

    let macos_element = match focused_element.as_any().downcast_ref::<MacOSUIElement>() {
        Some(el) => el,
        None => {
            let err_msg = "Focused element is not a MacOSUIElement".to_string();
            println!("[DEV_TOOL] Error: {}", err_msg);
            return Err(err_msg);
        }
    };

    match macos_utils::capture_element_screenshot(macos_element) {
        Ok(base64_string) => {
            println!("[DEV_TOOL] Element screenshot captured successfully.");
            // Send notification on success
            app.notification()
                .builder()
                .title("Element Screenshot")
                .body("Focused element screenshot captured.")
                .show()
                .map_err(|e| format!("Failed to send notification: {}", e))?;
            Ok(base64_string)
        },
        Err(e) => {
            match e {
                AutomationError::ZeroElementDimensions { role, label, x, y, width, height } => {
                    let user_friendly_err_msg = format!(
                        "Error: The focused element ('{}', Label: '{}') reported zero or negative dimensions ({}, {}, {}, {}) and could not be captured.",
                        role,
                        label,
                        x, y, width, height
                    );
                    println!("[DEV_TOOL] Error: {}", user_friendly_err_msg);
                    Err(user_friendly_err_msg)
                }
                _ => {
                    let err_msg = format!("Failed to capture element screenshot: {}", e);
                    println!("[DEV_TOOL] Error: {}", err_msg);
                    Err(err_msg)
                }
            }
        }
    }
}

#[cfg(not(target_os = "macos"))]
#[tauri::command]
pub(crate) async fn capture_element_screenshot_command(
    _app: tauri::AppHandle,
    _state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    Err("Element screenshot capture is only supported on macOS currently.".to_string())
}

#[tauri::command]
pub(crate) async fn dev_click_focused_element(
    app: AppHandle,
    state: State<'_, AppState>
) -> Result<(), String> {
    println!("[DEV_TOOL] Attempting to click focused element...");

    #[cfg(target_os = "macos")]
    {
        // Get the focused element first
        let focused_element = match state.desktop.focused_element() {
            Ok(el) => el,
            Err(e) => {
                let err_msg = format!("Failed to get focused element for click: {}", e);
                println!("[DEV_TOOL] Error: {}", err_msg);
                return Err(err_msg);
            }
        };

        // Now click the element
        match focused_element.click() {
             Ok(_) => {
                println!("[DEV_TOOL] click_focused_element succeeded.");
                send_dev_tool_notification(&app, "Click", "Clicked focused element.")?;
                Ok(())
            }
             Err(e) => {
                 let err_msg = format!("Failed to call click_focused_element: {}", e);
                println!("[DEV_TOOL] Error: {}", err_msg);
                Err(err_msg)
            }
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        Err(AutomationError::UnsupportedPlatform.to_string())
    }
}

#[tauri::command]
pub(crate) async fn dev_type_text(
    app: AppHandle,
    state: State<'_, AppState>,
    text: String
) -> Result<(), String> {
    println!("[DEV_TOOL] Attempting to type text: {}", text);

    #[cfg(target_os = "macos")]
    let result = state.desktop.type_text(&text);

    #[cfg(not(target_os = "macos"))]
    let result = Err(AutomationError::UnsupportedPlatform);

    match result {
        Ok(_) => {
            println!("[DEV_TOOL] type_text succeeded.");
             send_dev_tool_notification(&app, "Type Text", &format!("Typed: \"{}\"", text))?;
            Ok(())
        }
        Err(e) => {
            let err_msg = format!("Failed to call type_text: {}", e);
            println!("[DEV_TOOL] Error: {}", err_msg);
            Err(err_msg)
        }
    }
}

#[tauri::command]
pub(crate) async fn dev_press_key(
    app: AppHandle,
    state: State<'_, AppState>,
    key: String
) -> Result<(), String> {
    println!("[DEV_TOOL] Attempting to press key sequence: {}", key);

    #[cfg(target_os = "macos")]
    {
         // Get the focused element first
        let focused_element = match state.desktop.focused_element() {
            Ok(el) => el,
            Err(e) => {
                let err_msg = format!("Failed to get focused element for key press: {}", e);
                println!("[DEV_TOOL] Error: {}", err_msg);
                return Err(err_msg);
            }
        };

        // Press key on the element
        match focused_element.press_key(&key) {
             Ok(_) => {
                println!("[DEV_TOOL] press_key succeeded for: {}", key);
                send_dev_tool_notification(&app, "Press Key", &format!("Pressed key(s): {}", key))?; // Send notification
                Ok(())
            }
            Err(e) => {
                let err_msg = format!("Failed to call press_key for '{}': {}", key, e);
                println!("[DEV_TOOL] Error: {}", err_msg);
                Err(err_msg)
            }
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
         Err(AutomationError::UnsupportedPlatform.to_string())
    }
}

#[tauri::command]
pub(crate) async fn dev_open_application(app: tauri::AppHandle, state: tauri::State<'_, AppState>, app_name: String) -> Result<(), String> {
    println!("[DEV_TOOL] Attempting to open application: {}", app_name);
    match state.desktop.open_application(&app_name) {
        Ok(_) => {
            println!("[DEV_TOOL] open_application succeeded for: {}", app_name);
            send_dev_tool_notification(&app, "Open App", &format!("Opened application: {}", app_name))?;
            Ok(())
        }
        Err(e) => {
            let err_msg = format!("Failed to open application '{}': {}", app_name, e);
            println!("[DEV_TOOL] Error: {}", err_msg);
            Err(err_msg)
        }
    }
}

#[tauri::command]
pub(crate) async fn dev_open_url(app: AppHandle, state: State<'_, AppState>, url: String) -> Result<(), String> {
    println!("[DEV_TOOL] Attempting to open URL: {}", url);
    match state.desktop.open_url(&url, None) {
        Ok(_) => {
            println!("[DEV_TOOL] open_url succeeded for: {}", url);
            send_dev_tool_notification(&app, "Open URL", &format!("Opened URL: {}", url))?;
            Ok(())
        }
        Err(e) => {
            let err_msg = format!("Failed to open URL '{}': {}", url, e);
            println!("[DEV_TOOL] Error: {}", err_msg);
            Err(err_msg)
        }
    }
}

#[tauri::command]
pub(crate) async fn dev_scroll_window(
    app: AppHandle,
    state: State<'_, AppState>,
    direction: String,      // "up", "down", "left", "right"
    scroll_amount: f64, // Number of units/clicks (changed back to f64 for SDK)
) -> Result<(), String> {
    println!(
        "[DEV_TOOL] Attempting to scroll {} by {} units at current position...",
        direction,
        scroll_amount
    );

    // Validate direction
    let valid_directions = ["up", "down", "left", "right"];
    if !valid_directions.contains(&direction.as_str()) {
        let err_msg = format!("Invalid scroll direction: '{}'. Must be one of: {:?}", direction, valid_directions);
        println!("[DEV_TOOL] Error: {}", err_msg);
        return Err(err_msg);
    }

    #[cfg(target_os = "macos")]
    let result = state.desktop.scroll_at_current_position(&direction, scroll_amount);

    #[cfg(not(target_os = "macos"))]
    let result = Err(AutomationError::UnsupportedPlatform);

    match result {
        Ok(_) => {
            println!("[DEV_TOOL] scroll_at_current_position {} by {} succeeded.", direction, scroll_amount);
            send_dev_tool_notification(
                &app,
                "Scroll",
                &format!("Scrolled {} by {} at current position", direction, scroll_amount),
            )?;
            Ok(())
        }
        Err(e) => {
            let err_msg = format!("Failed to call scroll_at_current_position: {}", e);
            println!("[DEV_TOOL] Error: {}", err_msg);
            Err(err_msg)
        }
    }
}


#[tauri::command]
pub(crate) async fn dev_global_type_text(text: String, state: tauri::State<'_, AppState>) -> Result<(), String> {
    info!("Executing dev_global_type_text with text: {}", text);
    state.desktop.type_text(&text)
        .map_err(|e| format!("Error typing global text: {}", e))
}

#[tauri::command]
pub(crate) async fn dev_get_clipboard(state: tauri::State<'_, AppState>) -> Result<String, String> {
    info!("Executing dev_get_clipboard");
    state.desktop.get_clipboard_content()
        .map_err(|e| format!("Error getting clipboard content: {}", e))
}

#[tauri::command]
pub(crate) async fn dev_set_clipboard(content: String, state: tauri::State<'_, AppState>) -> Result<(), String> {
    info!("Executing dev_set_clipboard {}", content);
    state.desktop.set_clipboard_content(&content)
        .map_err(|e| format!("Error setting clipboard content: {}", e))
}

#[tauri::command]
pub(crate) async fn dev_hold_key(key: String, state: tauri::State<'_, AppState>) -> Result<(), String> {
    info!("Executing dev_hold_key with key: {}", key);
    state.desktop.hold_key(&key)
        .map_err(|e| format!("Error holding key '{}': {}", key, e))
}

#[tauri::command]
pub(crate) async fn dev_release_key(key: String, state: tauri::State<'_, AppState>) -> Result<(), String> {
    info!("Executing dev_release_key with key: {}", key);
    state.desktop.release_key(&key)
        .map_err(|e| format!("Error releasing key '{}': {}", key, e))
}

#[tauri::command]
pub(crate) async fn dev_wait(duration_ms: u64, state: tauri::State<'_, AppState>) -> Result<(), String> {
    info!("Executing dev_wait for {} ms", duration_ms);
    state.desktop.wait(duration_ms)
        .map_err(|e| format!("Error during wait: {}", e))
}

// New command to find element by selector
#[tauri::command]
pub(crate) async fn dev_find_element_by_selector(
    selector_str: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    println!("[DEV_TOOL] Finding element by selector: {}", selector_str);
    let selector: Selector = selector_str.as_str().into(); // Use From<&str> for Selector

    match state.desktop.locator(selector).first() {
        Ok(Some(element)) => {
            println!("[DEV_TOOL] Found element: {:?}", element.attributes());
            let attrs = element.attributes();
            serde_json::to_string_pretty(&attrs).map_err(|e| {
                let err_msg = format!("Failed to serialize found element attributes: {}", e);
                println!("[DEV_TOOL] Error: {}", err_msg);
                err_msg
            })
        }
        Ok(None) => {
            let err_msg = format!("Element not found for selector: {}", selector_str);
            println!("[DEV_TOOL] Info: {}", err_msg);
            Err(err_msg)
        }
        Err(e) => {
            let err_msg = format!("Error finding element for selector '{}': {}", selector_str, e);
            println!("[DEV_TOOL] Error: {}", err_msg);
            Err(err_msg)
        }
    }
}

// New command to click an element found by selector
#[tauri::command]
pub(crate) async fn dev_click_element_by_selector(
    app: AppHandle,
    selector_str: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    println!("[DEV_TOOL] Clicking element by selector: {}", selector_str);
    let selector: Selector = selector_str.as_str().into();

    match state.desktop.locator(selector).first() {
        Ok(Some(element)) => {
            println!("[DEV_TOOL] Found element, attempting click...");
            match element.click() {
                Ok(click_result) => {
                    println!("[DEV_TOOL] Click successful: {:?}", click_result);
                     let click_msg = format!("Clicked element matching: {}", selector_str);
                     send_dev_tool_notification(&app, "Click Element", &click_msg)?;
                    Ok(())
                }
                Err(e) => {
                    let err_msg = format!("Failed to click element found by selector '{}': {}", selector_str, e);
                    println!("[DEV_TOOL] Error: {}", err_msg);
                    Err(err_msg)
                }
            }
        }
        Ok(None) => {
            let err_msg = format!("Element not found for click selector: {}", selector_str);
            println!("[DEV_TOOL] Info: {}", err_msg);
            Err(err_msg)
        }
        Err(e) => {
            let err_msg = format!(
                "Error finding element before click for selector '{}': {}",
                selector_str,
                e
            );
            println!("[DEV_TOOL] Error: {}", err_msg);
            Err(err_msg)
        }
    }
}

#[derive(serde::Serialize)]
struct WindowInfo {
    id: String,
    title: String,
    // Add other fields as needed from UIElementAttributes if available
    // e.g., pid: Option<i32>,
    // bounds: Option<(i32, i32, i32, i32)>,
}

#[tauri::command]
pub(crate) async fn dev_get_window_list(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<String, String> {
    println!("[DEV_TOOL] Attempting to get window list...");

    match state.desktop.engine().list_windows() {
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
pub(crate) async fn dev_get_selected_text(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<String, String> {
    println!("[DEV_TOOL] Attempting to get selected text...");

    #[cfg(target_os = "macos")]
    {
        match state.desktop.focused_element() {
            Ok(element) => {
                let attrs = element.attributes();
                let selected_text = attrs.value.unwrap_or_else(|| "".to_string()); // Get value, default to empty string
                println!("[DEV_TOOL] get_selected_text succeeded. Text: '{}'", selected_text);
                send_dev_tool_notification(&app, "Selected Text", "Retrieved selected text.")?;
                Ok(selected_text)
            }
            Err(e) => {
                let err_msg = format!("Failed to get focused element for selected text: {}", e);
                println!("[DEV_TOOL] Error: {}", err_msg);
                Err(err_msg)
            }
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        Err("Get selected text is only supported on macOS currently.".to_string())
    }
}

#[tauri::command]
pub(crate) async fn dev_get_window_info(
    app: AppHandle,
    state: State<'_, AppState>,
    window_id: String,
) -> Result<String, String> {
    println!("[DEV_TOOL] Getting info for window ID: {}", window_id);

    match state.desktop.engine().list_windows() {
        Ok(windows) => {
            println!("[DEV_TOOL] Found {} windows to search.", windows.len());
            for window in windows {
                if let Some(id) = window.id() {
                    if id == window_id {
                        println!("[DEV_TOOL] Found matching window.");
                        // Attempt to get potentially more detailed attributes
                        let attrs_result = window.get_all_attributes(); // Check if this provides more info

                        let attrs_to_serialize = match attrs_result {
                             Ok(all_attrs) => {
                                println!("[DEV_TOOL] Using get_all_attributes result.");
                                all_attrs // Use detailed attributes if successful
                             },
                             Err(e) => {
                                println!("[DEV_TOOL] get_all_attributes failed ({}), falling back to basic attributes.", e);
                                window.attributes() // Fallback to basic attributes
                             }
                        };

                        match serde_json::to_string_pretty(&attrs_to_serialize) {
                            Ok(json_string) => {
                                send_dev_tool_notification(&app, "Window Info", "Retrieved window info.")?;
                                return Ok(json_string);
                            }
                            Err(e) => {
                                let err_msg = format!("Failed to serialize window attributes: {}", e);
                                println!("[DEV_TOOL] Error: {}", err_msg);
                                return Err(err_msg);
                            }
                        }
                    }
                }
            }
            // If loop finishes without finding the window
            let err_msg = format!("Window with ID '{}' not found.", window_id);
            println!("[DEV_TOOL] Info: {}", err_msg);
            Err(err_msg)
        }
        Err(e) => {
            let err_msg = format!("Failed to list windows while getting info: {}", e);
            println!("[DEV_TOOL] Error: {}", err_msg);
            Err(err_msg)
        }
    }
}

// Helper function to find a window by ID
// TODO: Consider moving this to a more central location or caching results if performance becomes an issue.
fn find_window_by_id(state: &State<'_, AppState>, window_id: &str) -> Result<Option<computer_use_ai_sdk::UIElement>, String> {
    match state.desktop.engine().list_windows() {
        Ok(windows) => {
            for window in windows {
                if let Some(id) = window.id() {
                    if id == window_id {
                        return Ok(Some(window));
                    }
                }
            }
            Ok(None) // Not found
        }
        Err(e) => Err(format!("Failed to list windows: {}", e)),
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

#[tauri::command]
pub(crate) async fn dev_triple_click(
    app: AppHandle,
    state: State<'_, AppState>,
    x: f64,
    y: f64
) -> Result<(), String> {
    println!("[DEV_TOOL] Attempting to triple click at ({}, {})...", x, y);

    #[cfg(target_os = "macos")]
    let result = state.desktop.triple_click(x, y);

    #[cfg(not(target_os = "macos"))]
    let result = Err(AutomationError::UnsupportedPlatform);

    match result {
        Ok(_) => {
            println!("[DEV_TOOL] triple_click succeeded.");
             send_dev_tool_notification(&app, "Triple Click", &format!("Clicked at ({}, {})", x, y))?;
            Ok(())
        }
        Err(e) => {
            let err_msg = format!("Failed to call triple_click: {}", e);
            println!("[DEV_TOOL] Error: {}", err_msg);
            Err(err_msg)
        }
    }
}

#[tauri::command]
pub(crate) async fn dev_mouse_move(
    app: AppHandle,
    state: State<'_, AppState>,
    x: f64,
    y: f64
) -> Result<(), String> {
    println!("[DEV_TOOL] Attempting to move mouse to ({}, {})...", x, y);

    #[cfg(target_os = "macos")]
    let result = state.desktop.mouse_move(x, y);

    #[cfg(not(target_os = "macos"))]
    let result = Err(AutomationError::UnsupportedPlatform);

    match result {
        Ok(_) => {
            println!("[DEV_TOOL] mouse_move succeeded.");
            send_dev_tool_notification(&app, "Mouse Move", &format!("Moved mouse to ({}, {})", x, y))?;
            Ok(())
        }
        Err(e) => {
            let err_msg = format!("Failed to call mouse_move: {}", e);
            println!("[DEV_TOOL] Error: {}", err_msg);
            Err(err_msg)
        }
    }
}

#[tauri::command]
pub(crate) async fn dev_left_mouse_down(
    app: AppHandle,
    state: State<'_, AppState>,
    x: f64,
    y: f64
) -> Result<(), String> {
    println!("[DEV_TOOL] Attempting left mouse down at ({}, {})...", x, y);

    #[cfg(target_os = "macos")]
    let result = state.desktop.left_mouse_down(x, y);

    #[cfg(not(target_os = "macos"))]
    let result = Err(AutomationError::UnsupportedPlatform);

    match result {
        Ok(_) => {
            println!("[DEV_TOOL] left_mouse_down succeeded at ({}, {}).", x, y);
            send_dev_tool_notification(&app, "Mouse Action", &format!("Left mouse button pressed at ({}, {})", x, y))?;
            Ok(())
        }
        Err(e) => {
            let err_msg = format!("Failed to call left_mouse_down at ({}, {}): {}", x, y, e);
            println!("[DEV_TOOL] Error: {}", err_msg);
            Err(err_msg)
        }
    }
}

#[tauri::command]
pub(crate) async fn dev_left_mouse_up(
    app: AppHandle,
    state: State<'_, AppState>,
    x: f64,
    y: f64
) -> Result<(), String> {
    println!("[DEV_TOOL] Attempting left mouse up at ({}, {})...", x, y);

    #[cfg(target_os = "macos")]
    let result = state.desktop.left_mouse_up(x, y);

    #[cfg(not(target_os = "macos"))]
    let result = Err(AutomationError::UnsupportedPlatform);

    match result {
        Ok(_) => {
            println!("[DEV_TOOL] left_mouse_up succeeded at ({}, {}).", x, y);
            send_dev_tool_notification(&app, "Mouse Action", &format!("Left mouse button released at ({}, {})", x, y))?;
            Ok(())
        }
        Err(e) => {
            let err_msg = format!("Failed to call left_mouse_up at ({}, {}): {}", x, y, e);
            println!("[DEV_TOOL] Error: {}", err_msg);
            Err(err_msg)
        }
    }
}

#[tauri::command]
pub(crate) async fn dev_left_click(
    app: AppHandle,
    state: State<'_, AppState>,
    x: f64,
    y: f64,
) -> Result<(), String> {
    println!("[DEV_TOOL] Attempting left click at ({}, {})...", x, y);

    #[cfg(target_os = "macos")]
    let result = state.desktop.left_click(x, y);

    #[cfg(not(target_os = "macos"))]
    let result = Err(AutomationError::UnsupportedPlatform);

    match result {
        Ok(_) => {
            println!("[DEV_TOOL] left_click at ({}, {}) succeeded.", x, y);
            send_dev_tool_notification(&app, "Mouse Action", &format!("Left clicked at ({}, {})", x, y))?;
            Ok(())
        }
        Err(e) => {
            let err_msg = format!("Failed to call left_click at ({}, {}): {}", x, y, e);
            println!("[DEV_TOOL] Error: {}", err_msg);
            Err(err_msg)
        }
    }
}

#[tauri::command]
pub(crate) async fn dev_left_click_drag(
    app: AppHandle,
    state: State<'_, AppState>,
    start_x: f64,
    start_y: f64,
    end_x: f64,
    end_y: f64,
) -> Result<(), String> {
    println!("[DEV_TOOL] Attempting left click drag from ({}, {}) to ({}, {})...", start_x, start_y, end_x, end_y);

    #[cfg(target_os = "macos")]
    let result = state.desktop.left_click_drag(start_x, start_y, end_x, end_y);

    #[cfg(not(target_os = "macos"))]
    let result = Err(AutomationError::UnsupportedPlatform);

    match result {
        Ok(_) => {
            println!("[DEV_TOOL] left_click_drag succeeded.");
            send_dev_tool_notification(&app, "Mouse Action", &format!("Dragged from ({}, {}) to ({}, {})", start_x, start_y, end_x, end_y))?;
            Ok(())
        }
        Err(e) => {
            let err_msg = format!("Failed to call left_click_drag: {}", e);
            println!("[DEV_TOOL] Error: {}", err_msg);
            Err(err_msg)
        }
    }
}

#[tauri::command]
pub(crate) async fn dev_right_click(
    app: AppHandle,
    state: State<'_, AppState>,
    x: f64,
    y: f64,
) -> Result<(), String> {
    println!("[DEV_TOOL] Attempting right click at ({}, {})...", x, y);

    #[cfg(target_os = "macos")]
    let result = state.desktop.right_click(x, y);

    #[cfg(not(target_os = "macos"))]
    let result = Err(AutomationError::UnsupportedPlatform);

    match result {
        Ok(_) => {
            println!("[DEV_TOOL] right_click at ({}, {}) succeeded.", x, y);
            send_dev_tool_notification(&app, "Mouse Action", &format!("Right clicked at ({}, {})", x, y))?;
            Ok(())
        }
        Err(e) => {
            let err_msg = format!("Failed to call right_click at ({}, {}): {}", x, y, e);
            println!("[DEV_TOOL] Error: {}", err_msg);
            Err(err_msg)
        }
    }
}

#[tauri::command]
pub(crate) async fn dev_middle_click(
    app: AppHandle,
    state: State<'_, AppState>,
    x: f64,
    y: f64,
) -> Result<(), String> {
    println!("[DEV_TOOL] Attempting middle click at ({}, {})...", x, y);

    #[cfg(target_os = "macos")]
    let result = state.desktop.middle_click(x, y);

    #[cfg(not(target_os = "macos"))]
    let result = Err(AutomationError::UnsupportedPlatform);

    match result {
        Ok(_) => {
            println!("[DEV_TOOL] middle_click at ({}, {}) succeeded.", x, y);
            send_dev_tool_notification(&app, "Mouse Action", &format!("Middle clicked at ({}, {})", x, y))?;
            Ok(())
        }
        Err(e) => {
            let err_msg = format!("Failed to call middle_click at ({}, {}): {}", x, y, e);
            println!("[DEV_TOOL] Error: {}", err_msg);
            Err(err_msg)
        }
    }
}

#[tauri::command]
pub(crate) async fn dev_double_click(
    app: AppHandle,
    state: State<'_, AppState>,
    x: f64,
    y: f64,
) -> Result<(), String> {
    println!("[DEV_TOOL] Attempting double click at ({}, {})...", x, y);

    #[cfg(target_os = "macos")]
    let result = state.desktop.double_click(x, y);

    #[cfg(not(target_os = "macos"))]
    let result = Err(AutomationError::UnsupportedPlatform);

    match result {
        Ok(_) => {
            println!("[DEV_TOOL] double_click at ({}, {}) succeeded.", x, y);
            send_dev_tool_notification(&app, "Mouse Action", &format!("Double clicked at ({}, {})", x, y))?;
            Ok(())
        }
        Err(e) => {
            let err_msg = format!("Failed to call double_click at ({}, {}): {}", x, y, e);
            println!("[DEV_TOOL] Error: {}", err_msg);
            Err(err_msg)
        }
    }
}

#[tauri::command]
pub(crate) async fn dev_get_cursor_position(
    app: AppHandle,
    state: State<'_, AppState>
) -> Result<(f64, f64), String> {
    println!("[DEV_TOOL] Attempting to get cursor position...");

    #[cfg(target_os = "macos")]
    let result = state.desktop.cursor_position();

    #[cfg(not(target_os = "macos"))]
    let result = Err(AutomationError::UnsupportedPlatform);

    match result {
        Ok(pos) => {
            println!("[DEV_TOOL] get_cursor_position succeeded: ({}, {}).", pos.0, pos.1);
            send_dev_tool_notification(&app, "Cursor Info", &format!("Cursor at ({}, {})", pos.0, pos.1))?;
            Ok(pos)
        }
        Err(e) => {
            let err_msg = format!("Failed to call get_cursor_position: {}", e);
            println!("[DEV_TOOL] Error: {}", err_msg);
            Err(err_msg)
        }
    }
}
