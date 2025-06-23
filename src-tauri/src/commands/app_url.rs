// Commands related to opening applications and URLs

use tauri::{AppHandle, State};
use crate::commands::debug_utils::{should_enable_debug, log_debug_operation, send_debug_notification, time_operation};
use crate::state::AppState;
use tracing::{info, error};

// =============================================================================
// CONSOLIDATED PRODUCTION COMMANDS WITH DEBUG FEATURES
// =============================================================================

#[tauri::command]
pub(crate) async fn open_application(
    app: AppHandle,
    app_name: String,
    state: State<'_, AppState>,
    debug_mode: Option<bool>
) -> Result<(), String> {
    let debug = should_enable_debug(debug_mode, &state);

    if debug {
        log_debug_operation("open_application", &format!("Opening application: {}", app_name));
    }

    let start_time = std::time::Instant::now();
    let desktop = state.get_desktop()?;

    match desktop.open_application(&app_name) {
        Ok(_) => {
            if debug {
                time_operation(start_time);
                log_debug_operation("open_application", &format!("Successfully opened application: {}", app_name));
                send_debug_notification(&app, "Application", &format!("Opened application: {}", app_name))?;
            }
            Ok(())
        }
        Err(e) => {
            let error_msg = format!("Failed to open application '{}': {}", app_name, e);
            if debug {
                log_debug_operation("open_application", &format!("Error: {}", error_msg));
            }
            Err(error_msg)
        }
    }
}

#[tauri::command]
pub(crate) async fn open_url(
    app: AppHandle,
    url: String,
    state: State<'_, AppState>,
    debug_mode: Option<bool>
) -> Result<(), String> {
    let debug = should_enable_debug(debug_mode, &state);

    if debug {
        log_debug_operation("open_url", &format!("Opening URL: {}", url));
    }

    let start_time = std::time::Instant::now();
    let desktop = state.get_desktop()?;

    match desktop.open_url(&url, None) {
        Ok(_) => {
            if debug {
                time_operation(start_time);
                log_debug_operation("open_url", &format!("Successfully opened URL: {}", url));
                send_debug_notification(&app, "URL", &format!("Opened URL: {}", url))?;
            }
            Ok(())
        }
        Err(e) => {
            let error_msg = format!("Failed to open URL '{}': {}", url, e);
            if debug {
                log_debug_operation("open_url", &format!("Error: {}", error_msg));
            }
            Err(error_msg)
        }
    }
}

// =============================================================================
// BACKWARD COMPATIBILITY WRAPPERS
// =============================================================================

#[tauri::command]
pub(crate) async fn dev_open_application(app_name: String, state: State<'_, AppState>, app: AppHandle) -> Result<(), String> {
    open_application(app, app_name, state, Some(true)).await
}

#[tauri::command]
pub(crate) async fn dev_open_url(url: String, state: State<'_, AppState>, app: AppHandle) -> Result<(), String> {
    open_url(app, url, state, Some(true)).await
}

// =============================================================================
// EXISTING IMPLEMENTATIONS (TO BE REMOVED AFTER MIGRATION)
// =============================================================================

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
