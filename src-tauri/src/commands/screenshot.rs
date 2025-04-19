use crate::AppState;
use computer_use_ai_sdk::AutomationError;

// Command to capture a screenshot (macOS only for now)
#[cfg(target_os = "macos")]
#[tauri::command]
pub async fn capture_screenshot_command() -> Result<String, String> {
    // Call the utility function that already handles capture and encoding
    match computer_use_ai_sdk::platforms::macos::utils::capture_and_encode_screenshot() {
        Ok(base64_string) => Ok(base64_string),
        Err(e) => Err(format!("Failed to capture screenshot: {}", e)),
    }
}

// Stub command for non-macos platforms to prevent compile errors
#[cfg(not(target_os = "macos"))]
#[tauri::command]
pub async fn capture_screenshot_command() -> Result<String, String> {
    Err("Screenshot capture is only supported on macOS currently.".to_string())
}

// New command for developer tool: Get focused element info
#[tauri::command]
pub async fn dev_get_focused_element_info(state: tauri::State<'_, AppState>) -> Result<String, String> {
    println!("[DEV_TOOL] Attempting to get focused element info using NSWorkspace...");

    // Use the new function directly
    #[cfg(target_os = "macos")]
    let result = computer_use_ai_sdk::platforms::macos::element::get_focused_element_ns_workspace(false, true); // Assuming default values

    // Stub for non-macOS
    #[cfg(not(target_os = "macos"))]
    let result: Result<computer_use_ai_sdk::UIElement, AutomationError> = Err(AutomationError::UnsupportedPlatform);

    match result {
        Ok(element) => {
            println!("[DEV_TOOL] get_focused_element_info (NSWorkspace) succeeded.");
            // Get attributes and serialize
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

// New command for capturing element screenshot (macOS only for now)
#[cfg(target_os = "macos")]
#[tauri::command]
pub async fn capture_element_screenshot_command(
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    use computer_use_ai_sdk::platforms::macos::element::MacOSUIElement;
    use computer_use_ai_sdk::platforms::macos::utils;
    use computer_use_ai_sdk::platforms::macos::element::get_focused_element_ns_workspace;
    
    println!("[DEV_TOOL] Capturing focused element screenshot using NSWorkspace method...");

    // 1. Get the focused element using the new function
    let focused_element = match get_focused_element_ns_workspace(false, true) { // Assuming defaults
        Ok(el) => el,
        Err(e) => {
            let err_msg = format!("Failed to get focused element (NSWorkspace): {}", e);
            println!("[DEV_TOOL] Error: {}", err_msg);
            return Err(err_msg);
        }
    };

    // 2. Downcast the UIElement trait object to the concrete MacOSUIElement type
    //    We need the concrete type to pass to the utility function.
    let macos_element = match focused_element.as_any().downcast_ref::<MacOSUIElement>() {
        Some(el) => el,
        None => {
            let err_msg = "Focused element is not a MacOSUIElement".to_string();
            println!("[DEV_TOOL] Error: {}", err_msg);
            return Err(err_msg);
        }
    };

    // 3. Call the utility function from macos_utils
    match utils::capture_element_screenshot(macos_element) {
        Ok(base64_string) => {
             println!("[DEV_TOOL] Element screenshot captured successfully.");
             Ok(base64_string)
        },
        Err(e) => {
            // Match on the specific error variant
            match e {
                AutomationError::ZeroElementDimensions { role, label, x, y, width, height } => {
                    let user_friendly_err_msg = format!(
                        "Error: The focused element ('{}', Label: '{}') reported zero or negative dimensions ({}, {}, {}, {}) and could not be captured.",
                        role, label, x, y, width, height
                    );
                    println!("[DEV_TOOL] Error: {}", user_friendly_err_msg);
                    Err(user_friendly_err_msg)
                }
                _ => {
                    // Handle other errors normally
                    let err_msg = format!("Failed to capture element screenshot: {}", e);
                    println!("[DEV_TOOL] Error: {}", err_msg);
                    Err(err_msg)
                }
            }
        }
    }
}

// Stub command for non-macos platforms
#[cfg(not(target_os = "macos"))]
#[tauri::command]
pub async fn capture_element_screenshot_command(
    _state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    Err("Element screenshot capture is only supported on macOS currently.".to_string())
}