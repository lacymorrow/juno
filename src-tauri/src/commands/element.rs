// Commands related to UI element interaction (focus, info, click, find, screenshots)

use crate::state::AppState;
use computer_use_ai_sdk::{AutomationError, Selector};
use tauri::{AppHandle, State};
use serde_json;
use super::send_dev_tool_notification; // Use helper from parent module

#[cfg(target_os = "macos")]
use computer_use_ai_sdk::platforms::macos::utils as macos_utils;
#[cfg(target_os = "macos")]
use computer_use_ai_sdk::platforms::macos::element::{get_focused_element_ns_workspace, MacOSUIElement};


#[tauri::command]
pub(crate) async fn dev_get_focused_element_info(app: tauri::AppHandle, _state: tauri::State<'_, AppState>) -> Result<String, String> {
    println!("[DEV_TOOL] Attempting to get focused element info using NSWorkspace...");

    #[cfg(target_os = "macos")]
    let result = get_focused_element_ns_workspace(false, true);

    #[cfg(not(target_os = "macos"))]
    let result: Result<computer_use_ai_sdk::UIElement, AutomationError> = Err(AutomationError::UnsupportedPlatform("macOS specific functionality not available on this platform".to_string()));

    match result {
        Ok(element) => {
            println!("[DEV_TOOL] get_focused_element_info (NSWorkspace) succeeded.");
            // Send notification on success
             send_dev_tool_notification(&app, "Focus Info", "Focused element info retrieved.")?;

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
pub(crate) async fn dev_click_focused_element(
    app: AppHandle,
    state: State<'_, AppState>
) -> Result<(), String> {
    println!("[DEV_TOOL] Attempting to click focused element...");

    #[cfg(target_os = "macos")]
    {
        // Get the focused element first
        let desktop = &state.desktop;
        let focused_element = match desktop.focused_element() {
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
        Err(AutomationError::UnsupportedPlatform("macOS specific functionality not available on this platform".to_string()).to_string())
    }
}

// New command to find element by selector
#[tauri::command]
pub(crate) async fn dev_find_element_by_selector(
    selector_str: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    println!("[DEV_TOOL] Finding element by selector: {}", selector_str);
    let selector: Selector = selector_str.as_str().into(); // Use From<&str> for Selector

    let desktop = &state.desktop;
    match desktop.locator(selector) {
        Ok(locator) => {
            match locator.first() {
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
        Err(e) => {
            let err_msg = format!("Error creating locator for selector '{}': {}", selector_str, e);
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

    let desktop = &state.desktop;
    match desktop.locator(selector) {
        Ok(locator) => {
            match locator.first() {
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
                    let err_msg = format!("Error finding element for selector '{}': {}", selector_str, e);
                    println!("[DEV_TOOL] Error: {}", err_msg);
                    Err(err_msg)
                }
            }
        }
        Err(e) => {
            let err_msg = format!(
                "Error creating locator for selector '{}': {}",
                selector_str,
                e
            );
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
        let desktop = &state.desktop;
        match desktop.focused_element() {
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
