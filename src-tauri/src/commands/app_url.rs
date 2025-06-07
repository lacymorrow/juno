// Commands related to opening applications and URLs

use tauri::State;
use crate::state::AppState;
use tracing::{info, error};

#[tauri::command]
pub(crate) async fn dev_open_application(app_name: String, state: State<'_, AppState>) -> Result<(), String> {
    info!("Executing dev_open_application for: {}", app_name);
    let desktop = state.get_desktop()?;
    match desktop.open_application(&app_name) {
        Ok(_) => {
            info!("Successfully opened application: {}", app_name);
            Ok(())
        }
        Err(e) => {
            let error_msg = format!("Failed to open application '{}': {}", app_name, e);
            error!("{}", error_msg);
            Err(error_msg)
        }
    }
}

#[tauri::command]
pub(crate) async fn dev_open_url(url: String, state: State<'_, AppState>) -> Result<(), String> {
    info!("Executing dev_open_url for: {}", url);
    let desktop = state.get_desktop()?;
    match desktop.open_url(&url, None) {
        Ok(_) => {
            info!("Successfully opened URL: {}", url);
            Ok(())
        }
        Err(e) => {
            let error_msg = format!("Failed to open URL '{}': {}", url, e);
            error!("{}", error_msg);
            Err(error_msg)
        }
    }
}
