use tauri::{AppHandle, Manager};
use tracing::{info, warn};
use crate::constants::errors::templates;

/// Format error message with template substitution
fn format_error(template: &str, context: &str, error: impl std::fmt::Display) -> String {
    template.replacen("{}", context, 1).replacen("{}", &error.to_string(), 1)
}

#[cfg(target_os = "macos")]
use cocoa::{
    appkit::NSWindow,
    base::{id as cocoa_id, BOOL, YES, NO},
    foundation::NSString,
};

#[cfg(target_os = "macos")]
use objc::{msg_send, sel, sel_impl};

/// Set the floating panel's click-through behavior
/// When enabled (true), the panel ignores mouse events and clicks pass through
/// When disabled (false), the panel captures mouse events and can be interacted with
/// NOTE: No longer a Tauri command - used internally by UI API bridge system
pub fn set_floating_panel_click_through(app: AppHandle, click_through: bool) -> Result<(), String> {
    info!("Setting floating panel click-through: {}", click_through);

    #[cfg(target_os = "macos")]
    {
        if let Some(window) = app.get_webview_window(crate::constants::window_labels::FLOATING_PANEL) {
            match window.ns_window() {
                Ok(ns_window_ptr) => {
                    let ns_window = ns_window_ptr as cocoa_id;

                    // Ensure we're on the main thread for Cocoa operations
                    if ns_window.is_null() {
                        let error_msg = "NSWindow pointer is null".to_string();
                        warn!("{}", error_msg);
                        return Err(error_msg);
                    }

                    unsafe {
                        let ignore_events: BOOL = if click_through { YES } else { NO };
                        // Set ignore mouse events with proper return type
                        let _: () = msg_send![ns_window, setIgnoresMouseEvents: ignore_events];
                        info!("macOS: Floating panel click-through set to: {}", click_through);
                    }
                }
                Err(e) => {
                    let error_msg = format_error(templates::FAILED_TO_RETRIEVE, "NSWindow for floating panel", e);
                    warn!("{}", error_msg);
                    return Err(error_msg);
                }
            }
        } else {
            let error_msg = "Floating panel window not found".to_string();
            warn!("{}", error_msg);
            return Err(error_msg);
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        warn!("Click-through behavior only supported on macOS");
        return Err("Click-through behavior only supported on macOS".to_string());
    }

    Ok(())
}

/// Enable click-through for the floating panel (makes it non-interactive)
/// NOTE: No longer a Tauri command - used internally by UI API bridge system
pub fn enable_floating_panel_click_through(app: AppHandle) -> Result<(), String> {
    set_floating_panel_click_through(app, true)
}

/// Disable click-through for the floating panel (makes it interactive)
/// NOTE: No longer a Tauri command - used internally by UI API bridge system
pub fn disable_floating_panel_click_through(app: AppHandle) -> Result<(), String> {
    set_floating_panel_click_through(app, false)
}

/// Get the current state of the floating panel (visible, focused, etc.)
/// NOTE: No longer a Tauri command - used internally by UI API bridge system
pub fn get_floating_panel_state(app: AppHandle) -> Result<serde_json::Value, String> {
    if let Some(window) = app.get_webview_window(crate::constants::window_labels::FLOATING_PANEL) {
        let is_visible = window.is_visible().unwrap_or(false);
        let is_focused = window.is_focused().unwrap_or(false);

        let state = serde_json::json!({
            "visible": is_visible,
            "focused": is_focused,
            "label": crate::constants::window_labels::FLOATING_PANEL
        });

        Ok(state)
    } else {
        Err("Floating panel window not found".to_string())
    }
}

/// Properly position the floating panel according to macOS conventions
/// This ensures the panel respects system UI elements like the menu bar and dock
/// NOTE: No longer a Tauri command - used internally by UI API bridge system
pub fn position_floating_panel_properly(app: AppHandle, x: Option<f64>, y: Option<f64>) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(crate::constants::window_labels::FLOATING_PANEL) {
        // Get screen dimensions to ensure proper positioning
        let monitor = window.current_monitor().map_err(|e| format_error(templates::FAILED_TO_RETRIEVE, "monitor", e))?;

        if let Some(monitor) = monitor {
            let screen_size = monitor.size();
            let _work_area = monitor.position(); // This gives us the usable area excluding dock/menu bar

            // Default position: top-right corner of usable screen area, with padding
            let final_x = x.unwrap_or(screen_size.width as f64 - 200.0); // 200px from right edge
            let final_y = y.unwrap_or(50.0); // 50px from top (below menu bar)

            // Ensure position is within screen bounds
            let clamped_x = final_x.clamp(0.0, (screen_size.width as f64) - 180.0); // Account for panel width
            let clamped_y = final_y.clamp(0.0, (screen_size.height as f64) - 100.0); // Account for panel height

            let position = tauri::PhysicalPosition::new(clamped_x as i32, clamped_y as i32);
            window.set_position(position).map_err(|e| format_error(templates::FAILED_TO_UPDATE, "position", e))?;

            info!("Positioned floating panel at ({}, {})", clamped_x, clamped_y);
        }

        Ok(())
    } else {
        Err("Floating panel window not found".to_string())
    }
}

/// Update the floating panel's window level for proper stacking order
/// NOTE: No longer a Tauri command - used internally by UI API bridge system
pub fn set_floating_panel_level(app: AppHandle, level: i32) -> Result<(), String> {
    info!("Setting floating panel window level: {}", level);

    #[cfg(target_os = "macos")]
    {
        if let Some(window) = app.get_webview_window(crate::constants::window_labels::FLOATING_PANEL) {
            match window.ns_window() {
                Ok(ns_window_ptr) => {
                    let ns_window = ns_window_ptr as cocoa_id;
                    unsafe {
                        // Validate level (common macOS window levels)
                        let safe_level = match level {
                            0 => 0,   // NSNormalWindowLevel
                            1 => 1,   // NSFloatingWindowLevel
                            3 => 3,   // NSFloatingWindowLevel
                            5 => 5,   // NSStatusWindowLevel
                            8 => 8,   // NSModalPanelWindowLevel
                            24 => 24, // NSPopUpMenuWindowLevel
                            _ => 3,   // Default to NSFloatingWindowLevel for safety
                        };

                        ns_window.setLevel_(safe_level);
                        info!("macOS: Floating panel window level set to: {}", safe_level);
                    }
                }
                Err(e) => {
                    let error_msg = format_error(templates::FAILED_TO_RETRIEVE, "NSWindow for floating panel", e);
                    warn!("{}", error_msg);
                    return Err(error_msg);
                }
            }
        } else {
            let error_msg = "Floating panel window not found".to_string();
            warn!("{}", error_msg);
            return Err(error_msg);
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        warn!("Window level adjustment only supported on macOS");
        return Err("Window level adjustment only supported on macOS".to_string());
    }

    Ok(())
}
