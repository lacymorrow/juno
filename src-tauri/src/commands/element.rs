//! Commands related to UI element interaction (focus, info, click, find, screenshots)
//! Consolidated to production functions with conditional debug features

use crate::state::AppState;
use computer_use_ai_sdk::{AutomationError, Selector};
use tauri::{AppHandle, State};
use serde_json;
use tracing::{info, error};
use super::send_dev_tool_notification; // Use helper from parent module

#[cfg(target_os = "macos")]
use computer_use_ai_sdk::platforms::macos::utils as macos_utils;
#[cfg(target_os = "macos")]
use computer_use_ai_sdk::platforms::macos::element::{get_focused_element_ns_workspace, MacOSUIElement};




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

#[tauri::command]
pub(crate) async fn get_focused_element_info(app: AppHandle, state: State<'_, AppState>) -> Result<String, String> {
    use crate::commands::debug_utils::{DebugConfig, should_enable_debug, log_debug_operation, send_debug_notification};

    let debug_enabled = should_enable_debug(false, &state);
    let debug_config = if debug_enabled { DebugConfig::development_mode() } else { DebugConfig::production_mode() };

    log_debug_operation("get_focused_element_info", "Getting focused element info using NSWorkspace", &debug_config);
    info!("Executing get_focused_element_info");

    #[cfg(target_os = "macos")]
    let result = get_focused_element_ns_workspace(false, true);

    #[cfg(not(target_os = "macos"))]
    let result: Result<computer_use_ai_sdk::UIElement, AutomationError> = Err(AutomationError::UnsupportedPlatform("macOS specific functionality not available on this platform".to_string()));

    match result {
        Ok(element) => {
            info!("Successfully retrieved focused element info");

            let attrs = element.attributes();
            match serde_json::to_string_pretty(&attrs) {
                Ok(json_string) => {
                    // Send debug notification if enabled
                    if debug_config.send_notifications {
                        let _ = send_debug_notification(&app, "Focus Info", "Focused element info retrieved");
                    }

                    Ok(json_string)
                }
                Err(e) => {
                    let error_msg = format!("Failed to serialize element info result: {}", e);
                    error!("{}", error_msg);
                    Err(error_msg)
                }
            }
        }
        Err(e) => {
            let error_msg = format!("Failed to call get_focused_element_info (NSWorkspace): {}", e);
            error!("{}", error_msg);
            Err(error_msg)
        }
    }
}

#[tauri::command]
pub(crate) async fn click_focused_element(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    use crate::commands::debug_utils::{DebugConfig, should_enable_debug, log_debug_operation, send_debug_notification};

    let debug_enabled = should_enable_debug(false, &state);
    let debug_config = if debug_enabled { DebugConfig::development_mode() } else { DebugConfig::production_mode() };

    log_debug_operation("click_focused_element", "Clicking focused element", &debug_config);
    info!("Executing click_focused_element");

    #[cfg(target_os = "macos")]
    {
        let desktop = &state.desktop;
        let focused_element = match desktop.focused_element() {
            Ok(el) => el,
            Err(e) => {
                let error_msg = format!("Failed to get focused element for click: {}", e);
                error!("{}", error_msg);
                return Err(error_msg);
            }
        };

        match focused_element.click() {
            Ok(_) => {
                info!("Successfully clicked focused element");

                // Send debug notification if enabled
                if debug_config.send_notifications {
                    let _ = send_debug_notification(&app, "Click", "Clicked focused element");
                }

                Ok(())
            }
            Err(e) => {
                let error_msg = format!("Failed to call click_focused_element: {}", e);
                error!("{}", error_msg);
                Err(error_msg)
            }
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        Err(AutomationError::UnsupportedPlatform("macOS specific functionality not available on this platform".to_string()).to_string())
    }
}

#[tauri::command]
pub(crate) async fn find_element_by_selector(selector_str: String, state: State<'_, AppState>) -> Result<String, String> {
    use crate::commands::debug_utils::{DebugConfig, should_enable_debug, log_debug_operation, validators};

    let debug_enabled = should_enable_debug(false, &state);
    let debug_config = if debug_enabled { DebugConfig::development_mode() } else { DebugConfig::production_mode() };

    // Debug validation
    if debug_config.validate_inputs {
        validators::non_empty_text(&selector_str)?;
    }

    log_debug_operation("find_element_by_selector", &format!("Finding element by selector: {}", selector_str), &debug_config);
    info!("Executing find_element_by_selector for selector: {}", selector_str);

    let selector: Selector = selector_str.as_str().into();
    let desktop = &state.desktop;

    match desktop.locator(selector) {
        Ok(locator) => {
            match locator.first() {
                Ok(Some(element)) => {
                    info!("Found element for selector: {}", selector_str);

                    let attrs = element.attributes();
                    match serde_json::to_string_pretty(&attrs) {
                        Ok(json_string) => Ok(json_string),
                        Err(e) => {
                            let error_msg = format!("Failed to serialize found element attributes: {}", e);
                            error!("{}", error_msg);
                            Err(error_msg)
                        }
                    }
                }
                Ok(None) => {
                    let error_msg = format!("Element not found for selector: {}", selector_str);
                    error!("{}", error_msg);
                    Err(error_msg)
                }
                Err(e) => {
                    let error_msg = format!("Error finding element for selector '{}': {}", selector_str, e);
                    error!("{}", error_msg);
                    Err(error_msg)
                }
            }
        }
        Err(e) => {
            let error_msg = format!("Error creating locator for selector '{}': {}", selector_str, e);
            error!("{}", error_msg);
            Err(error_msg)
        }
    }
}

#[tauri::command]
pub(crate) async fn click_element_by_selector(selector_str: String, app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    use crate::commands::debug_utils::{DebugConfig, should_enable_debug, log_debug_operation, send_debug_notification, validators};

    let debug_enabled = should_enable_debug(false, &state);
    let debug_config = if debug_enabled { DebugConfig::development_mode() } else { DebugConfig::production_mode() };

    // Debug validation
    if debug_config.validate_inputs {
        validators::non_empty_text(&selector_str)?;
    }

    log_debug_operation("click_element_by_selector", &format!("Clicking element by selector: {}", selector_str), &debug_config);
    info!("Executing click_element_by_selector for selector: {}", selector_str);

    let selector: Selector = selector_str.as_str().into();
    let desktop = &state.desktop;

    match desktop.locator(selector) {
        Ok(locator) => {
            match locator.first() {
                Ok(Some(element)) => {
                    info!("Found element, attempting click for selector: {}", selector_str);

                    match element.click() {
                        Ok(_) => {
                            info!("Successfully clicked element matching selector: {}", selector_str);

                            // Send debug notification if enabled
                            if debug_config.send_notifications {
                                let _ = send_debug_notification(&app, "Click Element", &format!("Clicked element matching: {}", selector_str));
                            }

                            Ok(())
                        }
                        Err(e) => {
                            let error_msg = format!("Failed to click element found by selector '{}': {}", selector_str, e);
                            error!("{}", error_msg);
                            Err(error_msg)
                        }
                    }
                }
                Ok(None) => {
                    let error_msg = format!("Element not found for click selector: {}", selector_str);
                    error!("{}", error_msg);
                    Err(error_msg)
                }
                Err(e) => {
                    let error_msg = format!("Error finding element for selector '{}': {}", selector_str, e);
                    error!("{}", error_msg);
                    Err(error_msg)
                }
            }
        }
        Err(e) => {
            let error_msg = format!("Error creating locator for selector '{}': {}", selector_str, e);
            error!("{}", error_msg);
            Err(error_msg)
        }
    }
}

#[tauri::command]
pub(crate) async fn get_selected_text(app: AppHandle, state: State<'_, AppState>) -> Result<String, String> {
    use crate::commands::debug_utils::{DebugConfig, should_enable_debug, log_debug_operation, send_debug_notification};

    let debug_enabled = should_enable_debug(false, &state);
    let debug_config = if debug_enabled { DebugConfig::development_mode() } else { DebugConfig::production_mode() };

    log_debug_operation("get_selected_text", "Getting selected text from focused element", &debug_config);
    info!("Executing get_selected_text");

    #[cfg(target_os = "macos")]
    {
        let desktop = &state.desktop;
        match desktop.focused_element() {
            Ok(element) => {
                let attrs = element.attributes();
                let selected_text = attrs.value.unwrap_or_else(|| "".to_string());

                info!("Successfully retrieved selected text (length: {})", selected_text.len());

                // Send debug notification if enabled
                if debug_config.send_notifications {
                    let preview = if selected_text.len() > 50 {
                        format!("{}...", &selected_text[..50])
                    } else {
                        selected_text.clone()
                    };
                    let _ = send_debug_notification(&app, "Selected Text", &format!("Retrieved: {}", preview));
                }

                Ok(selected_text)
            }
            Err(e) => {
                let error_msg = format!("Failed to get focused element for selected text: {}", e);
                error!("{}", error_msg);
                Err(error_msg)
            }
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        Err("Get selected text is only supported on macOS currently.".to_string())
    }
}
