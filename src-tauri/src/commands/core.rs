// Core/Miscellaneous commands (screenshots, app list, clipboard, wait)

use crate::state::AppState;
// Removed unused: use tauri::{AppHandle, State};
use tracing::{info};
use super::send_dev_tool_notification; // Use helper from parent module

#[cfg(target_os = "macos")]
use computer_use_ai_sdk::platforms::macos::utils as macos_utils;
#[cfg(target_os = "macos")]
use tauri::AppHandle; // AppHandle needed for macos capture_screenshot_command
#[cfg(not(target_os = "macos"))]
use tauri::AppHandle as DummyAppHandle; // Alias for non-macos signature consistency
use tauri::State; // State is needed for several commands


#[cfg(target_os = "macos")]
#[tauri::command]
pub(crate) async fn capture_screenshot_command(app: AppHandle) -> Result<String, String> {
    match macos_utils::capture_and_encode_screenshot() {
        Ok(base64_string) => {
            // Send notification on success
            send_dev_tool_notification(&app, "Screenshot", "Screenshot captured successfully.")?;
            Ok(base64_string)
        }
        Err(e) => Err(format!("Failed to capture screenshot: {}", e)),
    }
}

#[cfg(not(target_os = "macos"))]
#[tauri::command]
pub(crate) async fn capture_screenshot_command(_app: DummyAppHandle) -> Result<String, String> { // Use alias
    Err("Screenshot capture is only supported on macOS currently.".to_string())
}

#[tauri::command]
pub(crate) async fn list_apps(state: State<'_, AppState>) -> Result<Vec<String>, String> {
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
pub(crate) fn check_server_status(state: State<'_, AppState>) -> bool {
    let _ = state.desktop;
    true
}

#[tauri::command]
pub(crate) async fn dev_wait(duration_sec: f64, state: State<'_, AppState>) -> Result<(), String> {
    let duration_ms = (duration_sec * 1000.0).max(0.0) as u64; // Convert seconds to ms, ensure non-negative
    info!("Executing dev_wait for {} seconds ({} ms)", duration_sec, duration_ms);
    state.desktop.wait(duration_ms)
        .map_err(|e| format!("Error during wait: {}", e))
}

#[tauri::command]
pub(crate) async fn dev_get_clipboard(state: State<'_, AppState>) -> Result<String, String> {
    info!("Executing dev_get_clipboard");
    state.desktop.get_clipboard_content()
        .map_err(|e| format!("Error getting clipboard content: {}", e))
}

#[tauri::command]
pub(crate) async fn dev_set_clipboard(content: String, state: State<'_, AppState>) -> Result<(), String> {
    info!("Executing dev_set_clipboard {}", content);
    state.desktop.set_clipboard_content(&content)
        .map_err(|e| format!("Error setting clipboard content: {}", e))
}
