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
    key: String,
    modifier: Option<String>
) -> Result<(), String> {
    println!("[DEV_TOOL] Attempting to press key sequence: {} with modifier: {:?}", key, modifier);

    #[cfg(target_os = "macos")]
    {
        // Directly call the engine's press_key which handles modifiers
        match state.desktop.press_key(&key, modifier.as_deref()) {
            Ok(_) => {
                println!("[DEV_TOOL] press_key succeeded for: {} with modifier: {:?}", key, modifier);
                send_dev_tool_notification(&app, "Press Key", &format!("Pressed key(s): {} Modifier: {:?}", key, modifier))?;
                Ok(())
            }
            Err(e) => {
                let err_msg = format!("Failed to call press_key for '{}' with modifier {:?}: {}", key, modifier, e);
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
pub(crate) async fn dev_hold_key(
    key: String,
    duration_ms: Option<u64>,
    state: tauri::State<'_, AppState>
) -> Result<(), String> {
    info!("Executing dev_hold_key with key: {}, duration: {:?}ms", key, duration_ms);
    state.desktop.hold_key(&key, duration_ms)
        .map_err(|e| format!("Error holding key '{}': {}", key, e))
}

#[tauri::command]
pub(crate) async fn dev_release_key(key: String, state: tauri::State<'_, AppState>) -> Result<(), String> {
    info!("Executing dev_release_key with key: {}", key);
    state.desktop.release_key(&key)
        .map_err(|e| format!("Error releasing key '{}': {}", key, e))
}
