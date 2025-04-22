// Commands related to opening applications and URLs

use crate::state::AppState;
use tauri::{AppHandle, State};
use super::send_dev_tool_notification; // Use helper from parent module

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
