// Commands related to UI element interaction (focus, info, click, find, screenshots)

use crate::state::AppState;
use crate::commands::debug_utils::{should_enable_debug, log_debug_operation, send_debug_notification, time_operation};
use computer_use_ai_sdk::{AutomationError, Selector};
use tauri::{AppHandle, State};
use serde_json;
use super::send_dev_tool_notification; // Use helper from parent module

#[cfg(target_os = "macos")]
use computer_use_ai_sdk::platforms::macos::utils as macos_utils;
#[cfg(target_os = "macos")]
use computer_use_ai_sdk::platforms::macos::element::{get_focused_element_ns_workspace, MacOSUIElement};

// =============================================================================
// CONSOLIDATED PRODUCTION COMMANDS WITH DEBUG FEATURES
// =============================================================================

#[tauri::command]
pub(crate) async fn get_focused_element_info(
    app: AppHandle,
    state: State<'_, AppState>,
    debug_mode: Option<bool>
) -> Result<String, String> {
    let debug = should_enable_debug(&state, debug_mode);

    if debug {
        log_debug_operation("get_focused_element_info", "Getting focused element info using NSWorkspace");
    }

    let start_time = std::time::Instant::now();

    #[cfg(target_os = "macos")]
    let result = get_focused_element_ns_workspace(false, true);

    #[cfg(not(target_os = "macos"))]
    let result: Result<computer_use_ai_sdk::UIElement, AutomationError> = Err(AutomationError::UnsupportedPlatform("macOS specific functionality not available on this platform".to_string()));

    match result {
        Ok(element) => {
            let attrs = element.attributes();
            let serialized = serde_json::to_string_pretty(&attrs).map_err(|e| {
                format!("Failed to serialize element info result: {}", e)
            })?;

            if debug {
                time_operation("get_focused_element_info", start_time);
                send_debug_notification(&app, "Focus Info", "Focused element info retrieved.")?;
            }

            Ok(serialized)
        }
        Err(e) => {
            let err_msg = format!("Failed to get focused element info: {}", e);
            if debug {
                log_debug_operation("get_focused_element_info", &format!("Error: {}", err_msg));
            }
            Err(err_msg)
        }
    }
}

#[tauri::command]
pub(crate) async fn click_focused_element(
    app: AppHandle,
    state: State<'_, AppState>,
    debug_mode: Option<bool>
) -> Result<(), String> {
    let debug = should_enable_debug(&state, debug_mode);

    if debug {
        log_debug_operation("click_focused_element", "Attempting to click focused element");
    }

    let start_time = std::time::Instant::now();

    #[cfg(target_os = "macos")]
    {
        let desktop = &state.desktop;
        let focused_element = desktop.focused_element().map_err(|e| {
            format!("Failed to get focused element for click: {}", e)
        })?;

        focused_element.click().map_err(|e| {
            format!("Failed to click focused element: {}", e)
        })?;

        if debug {
            time_operation("click_focused_element", start_time);
            send_debug_notification(&app, "Click", "Clicked focused element.")?;
        }

        Ok(())
    }

    #[cfg(not(target_os = "macos"))]
    {
        Err("Click focused element is only supported on macOS currently.".to_string())
    }
}

#[tauri::command]
pub(crate) async fn find_element_by_selector(
    selector_str: String,
    state: State<'_, AppState>,
    debug_mode: Option<bool>
) -> Result<String, String> {
    let debug = should_enable_debug(&state, debug_mode);

    if debug {
        log_debug_operation("find_element_by_selector", &format!("Finding element by selector: {}", selector_str));
    }

    let start_time = std::time::Instant::now();
    let selector: Selector = selector_str.as_str().into();

    let desktop = &state.desktop;
    let locator = desktop.locator(selector).map_err(|e| {
        format!("Error creating locator for selector '{}': {}", selector_str, e)
    })?;

    match locator.first() {
        Ok(Some(element)) => {
            let attrs = element.attributes();
            let serialized = serde_json::to_string_pretty(&attrs).map_err(|e| {
                format!("Failed to serialize found element attributes: {}", e)
            })?;

            if debug {
                time_operation("find_element_by_selector", start_time);
                log_debug_operation("find_element_by_selector", &format!("Found element: {:?}", attrs));
            }

            Ok(serialized)
        }
        Ok(None) => {
            let err_msg = format!("Element not found for selector: {}", selector_str);
            if debug {
                log_debug_operation("find_element_by_selector", &err_msg);
            }
            Err(err_msg)
        }
        Err(e) => {
            let err_msg = format!("Error finding element for selector '{}': {}", selector_str, e);
            if debug {
                log_debug_operation("find_element_by_selector", &format!("Error: {}", err_msg));
            }
            Err(err_msg)
        }
    }
}

#[tauri::command]
pub(crate) async fn click_element_by_selector(
    app: AppHandle,
    selector_str: String,
    state: State<'_, AppState>,
    debug_mode: Option<bool>
) -> Result<(), String> {
    let debug = should_enable_debug(&state, debug_mode);

    if debug {
        log_debug_operation("click_element_by_selector", &format!("Clicking element by selector: {}", selector_str));
    }

    let start_time = std::time::Instant::now();
    let selector: Selector = selector_str.as_str().into();

    let desktop = &state.desktop;
    let locator = desktop.locator(selector).map_err(|e| {
        format!("Error creating locator for selector '{}': {}", selector_str, e)
    })?;

    match locator.first() {
        Ok(Some(element)) => {
            element.click().map_err(|e| {
                format!("Failed to click element found by selector '{}': {}", selector_str, e)
            })?;

            if debug {
                time_operation("click_element_by_selector", start_time);
                let click_msg = format!("Clicked element matching: {}", selector_str);
                send_debug_notification(&app, "Click Element", &click_msg)?;
            }

            Ok(())
        }
        Ok(None) => {
            let err_msg = format!("Element not found for click selector: {}", selector_str);
            if debug {
                log_debug_operation("click_element_by_selector", &err_msg);
            }
            Err(err_msg)
        }
        Err(e) => {
            let err_msg = format!("Error finding element for selector '{}': {}", selector_str, e);
            if debug {
                log_debug_operation("click_element_by_selector", &format!("Error: {}", err_msg));
            }
            Err(err_msg)
        }
    }
}

#[tauri::command]
pub(crate) async fn get_selected_text(
    app: AppHandle,
    state: State<'_, AppState>,
    debug_mode: Option<bool>
) -> Result<String, String> {
    let debug = should_enable_debug(&state, debug_mode);

    if debug {
        log_debug_operation("get_selected_text", "Getting selected text from focused element");
    }

    let start_time = std::time::Instant::now();

    #[cfg(target_os = "macos")]
    {
        let desktop = &state.desktop;
        let element = desktop.focused_element().map_err(|e| {
            format!("Failed to get focused element for selected text: {}", e)
        })?;

        let attrs = element.attributes();
        let selected_text = attrs.value.unwrap_or_else(|| "".to_string());

        if debug {
            time_operation("get_selected_text", start_time);
            log_debug_operation("get_selected_text", &format!("Retrieved text: '{}'", selected_text));
            send_debug_notification(&app, "Selected Text", "Retrieved selected text.")?;
        }

        Ok(selected_text)
    }

    #[cfg(not(target_os = "macos"))]
    {
        Err("Get selected text is only supported on macOS currently.".to_string())
    }
}

// =============================================================================
// BACKWARD COMPATIBILITY WRAPPERS
// =============================================================================

#[tauri::command]
pub(crate) async fn dev_get_focused_element_info(app: AppHandle, state: State<'_, AppState>) -> Result<String, String> {
    get_focused_element_info(app, state, Some(true)).await
}

#[tauri::command]
pub(crate) async fn dev_click_focused_element(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    click_focused_element(app, state, Some(true)).await
}

#[tauri::command]
pub(crate) async fn dev_find_element_by_selector(selector_str: String, state: State<'_, AppState>) -> Result<String, String> {
    find_element_by_selector(selector_str, state, Some(true)).await
}

#[tauri::command]
pub(crate) async fn dev_click_element_by_selector(app: AppHandle, selector_str: String, state: State<'_, AppState>) -> Result<(), String> {
    click_element_by_selector(app, selector_str, state, Some(true)).await
}

#[tauri::command]
pub(crate) async fn dev_get_selected_text(app: AppHandle, state: State<'_, AppState>) -> Result<String, String> {
    get_selected_text(app, state, Some(true)).await
}

// =============================================================================
// EXISTING IMPLEMENTATIONS (TO BE REMOVED AFTER MIGRATION)
// =============================================================================

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
            send_dev_tool_notification(&app, "Element Screenshot", "Focused element screenshot captured.")?;
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
