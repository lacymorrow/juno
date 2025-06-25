// Commands related to opening applications and URLs

use tauri::State;
use crate::state::AppState;
use tracing::{info, error};

// ============================================================================
// PRODUCTION APP FUNCTIONS WITH UNIFIED DEBUG SYSTEM
// ============================================================================

/// Production function to open an application with optional debug features
#[tauri::command]
pub async fn open_application(
    app_handle: tauri::AppHandle,
    state: State<'_, AppState>,
    app_name: String,
    debug_mode: Option<bool>,
) -> Result<(), String> {
    use crate::commands::debug_utils::{DebugConfig, DebugOperation, should_enable_debug, validators::non_empty_text, send_debug_notification};

    let debug_config = if should_enable_debug(debug_mode.unwrap_or(false), &state) {
        DebugConfig::development_mode()
    } else {
        DebugConfig::production_mode()
    };

    let debug_op = DebugOperation::start("open_application", debug_config.clone());

        // Debug validation
    if debug_config.validate_inputs {
        if let Err(e) = non_empty_text(&app_name) {
            let err_msg = format!("Invalid app name: {}", e);
            if debug_config.send_notifications {
                send_debug_notification(&app_handle, "Open Application Error", &err_msg)?;
            }
            debug_op.complete(Some(&app_handle), false);
            return Err(err_msg);
        }
    }

    if debug_config.log_operations {
        info!("[APP] Opening application: {}", app_name);
    }

    let desktop = state.get_desktop()?;
    match desktop.open_application(&app_name) {
        Ok(_) => {
            if debug_config.log_operations {
                info!("[APP] Successfully opened application: {}", app_name);
            }
            if debug_config.send_notifications {
                send_debug_notification(
                    &app_handle,
                    "Open Application",
                    &format!("Opened application: {}", app_name),
                )?;
            }
            debug_op.complete(Some(&app_handle), true);
            Ok(())
        }
        Err(e) => {
            let error_msg = format!("Failed to open application '{}': {}", app_name, e);
            if debug_config.log_operations {
                error!("[APP] Error: {}", error_msg);
            }
            if debug_config.send_notifications {
                send_debug_notification(&app_handle, "Open Application Error", &error_msg)?;
            }
            debug_op.complete(Some(&app_handle), false);
            Err(error_msg)
        }
    }
}

/// Production function to open a URL with optional debug features
#[tauri::command]
pub async fn open_url(
    app_handle: tauri::AppHandle,
    state: State<'_, AppState>,
    url: String,
    debug_mode: Option<bool>,
) -> Result<(), String> {
    use crate::commands::debug_utils::{DebugConfig, DebugOperation, should_enable_debug, validators::non_empty_text, send_debug_notification};

    let debug_config = if should_enable_debug(debug_mode.unwrap_or(false), &state) {
        DebugConfig::development_mode()
    } else {
        DebugConfig::production_mode()
    };

    let debug_op = DebugOperation::start("open_url", debug_config.clone());

        // Debug validation
    if debug_config.validate_inputs {
        if let Err(e) = non_empty_text(&url) {
            let err_msg = format!("Invalid URL: {}", e);
            if debug_config.send_notifications {
                send_debug_notification(&app_handle, "Open URL Error", &err_msg)?;
            }
            debug_op.complete(Some(&app_handle), false);
            return Err(err_msg);
        }

        // Basic URL validation
        if !url.starts_with("http://") && !url.starts_with("https://") && !url.starts_with("file://") && !url.starts_with("ftp://") {
            let err_msg = "URL must start with a valid protocol (http://, https://, file://, or ftp://)".to_string();
            if debug_config.send_notifications {
                send_debug_notification(&app_handle, "Open URL Error", &err_msg)?;
            }
            debug_op.complete(Some(&app_handle), false);
            return Err(err_msg);
        }
    }

    if debug_config.log_operations {
        info!("[APP] Opening URL: {}", url);
    }

    let desktop = state.get_desktop()?;
    match desktop.open_url(&url, None) {
        Ok(_) => {
            if debug_config.log_operations {
                info!("[APP] Successfully opened URL: {}", url);
            }
            if debug_config.send_notifications {
                send_debug_notification(
                    &app_handle,
                    "Open URL",
                    &format!("Opened URL: {}", url),
                )?;
            }
            debug_op.complete(Some(&app_handle), true);
            Ok(())
        }
        Err(e) => {
            let error_msg = format!("Failed to open URL '{}': {}", url, e);
            if debug_config.log_operations {
                error!("[APP] Error: {}", error_msg);
            }
            if debug_config.send_notifications {
                send_debug_notification(&app_handle, "Open URL Error", &error_msg)?;
            }
            debug_op.complete(Some(&app_handle), false);
            Err(error_msg)
        }
    }
}
