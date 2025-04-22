// Commands related to keyboard actions (typing, pressing keys)

use crate::state::AppState;
use tauri::{AppHandle, State};
use tracing::{info};
use super::send_dev_tool_notification; // Use helper from parent module

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
    let result = Err(computer_use_ai_sdk::AutomationError::UnsupportedPlatform);

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

        // Coerce single lowercase letters to uppercase for the SDK
        let key_to_press = if key.len() == 1 {
            let char = key.chars().next().unwrap();
            if char.is_ascii_lowercase() {
                println!("[DEV_TOOL] Coercing lowercase key '{}' to uppercase '{}'", char, char.to_ascii_uppercase());
                char.to_ascii_uppercase().to_string()
            } else {
                key // Use original if not lowercase
            }
        } else {
            key // Use original if not single char
        };

        // Press key on the element using the potentially coerced key
        match focused_element.press_key(&key_to_press) {
             Ok(_) => {
                println!("[DEV_TOOL] press_key succeeded for: {}", key_to_press); // Log the key actually pressed
                send_dev_tool_notification(&app, "Press Key", &format!("Pressed key(s): {}", key_to_press))?; // Send notification
                Ok(())
            }
            Err(e) => {
                let err_msg = format!("Failed to call press_key for '{}': {}", key_to_press, e); // Log the key actually pressed
                println!("[DEV_TOOL] Error: {}", err_msg);
                Err(err_msg)
            }
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
         Err(computer_use_ai_sdk::AutomationError::UnsupportedPlatform.to_string())
    }
}

#[tauri::command]
pub(crate) async fn dev_global_type_text(text: String, state: tauri::State<'_, AppState>) -> Result<(), String> {
    info!("Executing dev_global_type_text with text: {}", text);
    state.desktop.type_text(&text)
        .map_err(|e| format!("Error typing global text: {}", e))
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
