// Commands related to opening applications and URLs

use tauri::State;
use crate::state::AppState;
use tracing::{info, error};

// ============================================================================
// MACOS HELPERS: RUNNING APP DETECTION
// ============================================================================

/// NSString UTF-8 encoding constant (NSUTF8StringEncoding = 4).
#[cfg(target_os = "macos")]
const NS_UTF8_STRING_ENCODING: usize = 4;

/// Returns the PID of the named app if it is currently running, otherwise None.
/// Uses NSWorkspace.runningApplications for reliable detection (background apps included).
#[cfg(target_os = "macos")]
fn find_running_app_pid(app_name: &str) -> Option<i32> {
    use objc::{class, msg_send, sel, sel_impl};
    unsafe {
        let workspace_class = class!(NSWorkspace);
        let shared: *mut objc::runtime::Object = msg_send![workspace_class, sharedWorkspace];
        if shared.is_null() {
            return None;
        }
        let apps: *mut objc::runtime::Object = msg_send![shared, runningApplications];
        if apps.is_null() {
            return None;
        }
        let count: usize = msg_send![apps, count];
        for i in 0..count {
            let app: *mut objc::runtime::Object = msg_send![apps, objectAtIndex: i];
            if app.is_null() {
                continue;
            }
            let name_obj: *mut objc::runtime::Object = msg_send![app, localizedName];
            if name_obj.is_null() {
                continue;
            }
            let bytes: *const std::os::raw::c_char =
                msg_send![name_obj, UTF8String];
            let len: usize =
                msg_send![name_obj, lengthOfBytesUsingEncoding: NS_UTF8_STRING_ENCODING];
            if bytes.is_null() || len == 0 {
                continue;
            }
            let bytes_slice = std::slice::from_raw_parts(bytes as *const u8, len);
            if let Ok(found_name) = std::str::from_utf8(bytes_slice) {
                if found_name.to_lowercase() == app_name.to_lowercase() {
                    let pid: i32 = msg_send![app, processIdentifier];
                    return Some(pid);
                }
            }
        }
        None
    }
}

#[cfg(not(target_os = "macos"))]
fn find_running_app_pid(_app_name: &str) -> Option<i32> {
    None
}

/// Returns true if the named app is the current frontmost application.
#[cfg(target_os = "macos")]
fn is_app_frontmost(app_name: &str) -> bool {
    use objc::{class, msg_send, sel, sel_impl};
    unsafe {
        let workspace_class = class!(NSWorkspace);
        let shared: *mut objc::runtime::Object = msg_send![workspace_class, sharedWorkspace];
        if shared.is_null() {
            return false;
        }
        let frontmost: *mut objc::runtime::Object = msg_send![shared, frontmostApplication];
        if frontmost.is_null() {
            return false;
        }
        let name_obj: *mut objc::runtime::Object = msg_send![frontmost, localizedName];
        if name_obj.is_null() {
            return false;
        }
        let bytes: *const std::os::raw::c_char =
            msg_send![name_obj, UTF8String];
        let len: usize =
            msg_send![name_obj, lengthOfBytesUsingEncoding: NS_UTF8_STRING_ENCODING];
        if bytes.is_null() || len == 0 {
            return false;
        }
        let bytes_slice = std::slice::from_raw_parts(bytes as *const u8, len);
        std::str::from_utf8(bytes_slice)
            .map(|name| name.to_lowercase() == app_name.to_lowercase())
            .unwrap_or(false)
    }
}

#[cfg(not(target_os = "macos"))]
fn is_app_frontmost(_app_name: &str) -> bool {
    false
}

// ============================================================================
// PRODUCTION APP FUNCTIONS WITH UNIFIED DEBUG SYSTEM
// ============================================================================

/// Production function to open an application with optional debug features.
/// Smart logic: if the app is already running, focus it instead of relaunching.
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

    // Check if the app is already running — prefer focus over relaunch.
    if let Some(_pid) = find_running_app_pid(&app_name) {
        if is_app_frontmost(&app_name) {
            if debug_config.log_operations {
                info!("[APP] App '{}' already running and is frontmost, no action needed", app_name);
            }
            debug_op.complete(Some(&app_handle), true);
            return Ok(());
        }

        if debug_config.log_operations {
            info!("[APP] App '{}' already running, focusing instead of launching", app_name);
        }

        // `open -a <app>` on macOS activates a running app without relaunching it,
        // skipping the 10-attempt retry loop that desktop.open_application() would use.
        let focus_status = std::process::Command::new("open")
            .args(["-a", &app_name])
            .status();

        return match focus_status {
            Ok(s) if s.success() => {
                if debug_config.log_operations {
                    info!("[APP] Successfully focused application: {}", app_name);
                }
                if debug_config.send_notifications {
                    send_debug_notification(
                        &app_handle,
                        "Focus Application",
                        &format!("Focused already-running application: {}", app_name),
                    )?;
                }
                debug_op.complete(Some(&app_handle), true);
                Ok(())
            }
            Ok(s) => {
                let err_msg = format!(
                    "Failed to focus already-running '{}': exit code {:?}",
                    app_name,
                    s.code()
                );
                if debug_config.log_operations {
                    error!("[APP] {}", err_msg);
                }
                debug_op.complete(Some(&app_handle), false);
                Err(err_msg)
            }
            Err(e) => {
                let err_msg = format!("Failed to execute focus command for '{}': {}", app_name, e);
                if debug_config.log_operations {
                    error!("[APP] {}", err_msg);
                }
                debug_op.complete(Some(&app_handle), false);
                Err(err_msg)
            }
        };
    }

    // App is not running — use the full launch path.
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

/// Returns the visible window titles for a specific running application.
///
/// Useful for agents that want to know "is Gmail open in Chrome?" before
/// deciding whether to navigate or open a new window.
/// Uses CGWindowListCopyWindowInfo (~1 ms) filtered by app name.
#[tauri::command]
pub async fn get_app_windows(app_name: String) -> Result<Vec<String>, String> {
    if app_name.is_empty() {
        return Err("app_name cannot be empty".to_string());
    }

    #[cfg(target_os = "macos")]
    {
        use computer_use_ai_sdk::platforms::macos::display::list_visible_windows;

        let name = app_name.clone();
        let windows = tokio::task::spawn_blocking(move || list_visible_windows())
            .await
            .map_err(|e| format!("get_app_windows task panicked: {}", e))?
            .map_err(|e| format!("Failed to list windows: {}", e))?;

        let titles: Vec<String> = windows
            .into_iter()
            .filter(|w| w.app_name.to_lowercase() == name.to_lowercase())
            .filter_map(|w| w.window_title)
            .collect();

        Ok(titles)
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = app_name;
        Ok(vec![])
    }
}
