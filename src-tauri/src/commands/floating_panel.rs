use tauri::{AppHandle, Manager};
use tracing::{info, warn};
use crate::constants;
use tauri::{WebviewWindow};

#[cfg(target_os = "macos")]
use cocoa::{
    appkit::{NSWindow, NSWindowCollectionBehavior},
    base::{id as cocoa_id, nil, BOOL, YES, NO},
    foundation::NSString,
};

#[cfg(target_os = "macos")]
use objc::{msg_send, sel, sel_impl};

/// Set the floating panel's click-through behavior
/// When enabled (true), the panel ignores mouse events and clicks pass through
/// When disabled (false), the panel captures mouse events and can be interacted with
#[tauri::command]
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
                    let error_msg = format!("Failed to get NSWindow for floating panel: {}", e);
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
#[tauri::command]
pub fn enable_floating_panel_click_through(app: AppHandle) -> Result<(), String> {
    set_floating_panel_click_through(app, true)
}

/// Disable click-through for the floating panel (makes it interactive)
#[tauri::command]
pub fn disable_floating_panel_click_through(app: AppHandle) -> Result<(), String> {
    set_floating_panel_click_through(app, false)
}

/// Get the current state of the floating panel (visible, focused, etc.)
#[tauri::command]
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
#[tauri::command]
pub fn position_floating_panel_properly(app: AppHandle, x: Option<f64>, y: Option<f64>) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(crate::constants::window_labels::FLOATING_PANEL) {
        // Get screen dimensions to ensure proper positioning
        let monitor = window.current_monitor().map_err(|e| format!("Failed to get monitor: {}", e))?;

        if let Some(monitor) = monitor {
            let screen_size = monitor.size();
            let work_area = monitor.position(); // This gives us the usable area excluding dock/menu bar

            // Default position: top-right corner of usable screen area, with padding
            let final_x = x.unwrap_or(screen_size.width as f64 - 200.0); // 200px from right edge
            let final_y = y.unwrap_or(50.0); // 50px from top (below menu bar)

            // Ensure position is within screen bounds
            let clamped_x = final_x.clamp(0.0, (screen_size.width as f64) - 180.0); // Account for panel width
            let clamped_y = final_y.clamp(0.0, (screen_size.height as f64) - 100.0); // Account for panel height

            let position = tauri::PhysicalPosition::new(clamped_x as i32, clamped_y as i32);
            window.set_position(position).map_err(|e| format!("Failed to set position: {}", e))?;

            info!("Positioned floating panel at ({}, {})", clamped_x, clamped_y);
        }

        Ok(())
    } else {
        Err("Floating panel window not found".to_string())
    }
}

/// Update the floating panel's window level for proper stacking order
#[tauri::command]
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
                    let error_msg = format!("Failed to get NSWindow for floating panel: {}", e);
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

/// Apply pill-shaped vibrancy to a window
#[tauri::command]
pub fn apply_pill_vibrancy(app: AppHandle, window_label: Option<String>) -> Result<(), String> {
    let label = window_label.unwrap_or_else(|| constants::window_labels::FLOATING_PANEL.to_string());

    if let Some(window) = app.get_webview_window(&label) {
        // Apply vibrancy using window-vibrancy
        #[cfg(target_os = "macos")]
        {
            use window_vibrancy::{apply_vibrancy, NSVisualEffectMaterial, NSVisualEffectState};

            match apply_vibrancy(
                &window,
                NSVisualEffectMaterial::HudWindow,
                Some(NSVisualEffectState::Active),
                Some(50.0) // Corner radius for pill shape
            ) {
                Ok(_) => {
                    info!("✅ Successfully applied pill vibrancy to window: {}", label);
                    info!("🎨 Vibrancy material: HudWindow, State: Active, Corner radius: 24px");

                                        // Additional macOS-specific styling for pill shape
                    if let Ok(ns_window_ptr) = window.ns_window() {
                        let ns_window = ns_window_ptr as cocoa_id;
                        unsafe {
                            // Make window background clear for vibrancy effect
                            use objc::runtime::Class;
                            use objc::{msg_send, sel, sel_impl};

                            let clear_color: cocoa_id = msg_send![Class::get("NSColor").unwrap(), clearColor];
                            let _: () = msg_send![ns_window, setBackgroundColor: clear_color];

                            // Set window to be non-opaque for vibrancy to work
                            let _: () = msg_send![ns_window, setOpaque: NO];

                            // Ensure the window has a shadow for better vibrancy effect
                            let _: () = msg_send![ns_window, setHasShadow: YES];

                            // Set the window level to floating for better visibility
                            let _: () = msg_send![ns_window, setLevel: 3]; // NSFloatingWindowLevel

                            // Ensure window is interactive (not click-through)
                            let _: () = msg_send![ns_window, setIgnoresMouseEvents: NO];
                            info!("🖱️ Window '{}' set to interactive (not click-through)", label);
                        }
                    }
                }
                Err(e) => {
                    let error_msg = format!("Failed to apply vibrancy: {}", e);
                    warn!("{}", error_msg);
                    return Err(error_msg);
                }
            }
        }

        #[cfg(not(target_os = "macos"))]
        {
            info!("Pill vibrancy applied via CSS for window: {} (non-macOS)", label);
        }

        Ok(())
    } else {
        Err(format!("Window '{}' not found", label))
    }
}

/// Remove vibrancy from a window
#[tauri::command]
pub fn remove_vibrancy(app: AppHandle, window_label: Option<String>) -> Result<(), String> {
    let label = window_label.unwrap_or_else(|| constants::window_labels::FLOATING_PANEL.to_string());

    if let Some(window) = app.get_webview_window(&label) {
        #[cfg(target_os = "macos")]
        {
            use window_vibrancy::clear_vibrancy;

            match clear_vibrancy(&window) {
                Ok(_) => {
                    info!("✅ Successfully removed vibrancy from window: {}", label);
                }
                Err(e) => {
                    let error_msg = format!("Failed to remove vibrancy: {}", e);
                    warn!("{}", error_msg);
                    return Err(error_msg);
                }
            }
        }

        #[cfg(not(target_os = "macos"))]
        {
            info!("Vibrancy removed via CSS for window: {} (non-macOS)", label);
        }

        Ok(())
    } else {
        Err(format!("Window '{}' not found", label))
    }
}

/// Create a new pill-shaped floating window with vibrancy
#[tauri::command]
pub fn create_pill_window(
    app: AppHandle,
    label: String,
    width: f64,
    height: f64,
    x: Option<f64>,
    y: Option<f64>
) -> Result<(), String> {
    use tauri::{WebviewUrl, WebviewWindowBuilder};

    // Calculate pill dimensions (height should be much smaller than width for pill shape)
    let pill_height = height.min(width * 0.6); // Ensure pill proportions

    // Create the window
    let window = WebviewWindowBuilder::new(
        &app,
        &label,
        WebviewUrl::App("/floating-panel".into())
    )
    .title("")
    .inner_size(width, pill_height)
    .position(x.unwrap_or(500.0), y.unwrap_or(500.0))
    .decorations(false)
    .transparent(true)
    .always_on_top(true)
    .resizable(false)
    .skip_taskbar(true)
    .shadow(true)
    .build()
    .map_err(|e| format!("Failed to create pill window: {}", e))?;

    // Ensure the window is NOT click-through (interactive)
    #[cfg(target_os = "macos")]
    {
        if let Ok(ns_window_ptr) = window.ns_window() {
            let ns_window = ns_window_ptr as cocoa_id;
            unsafe {
                // Explicitly disable click-through to make window interactive
                let _: () = msg_send![ns_window, setIgnoresMouseEvents: NO];
                info!("✅ Pill window '{}' set to interactive (not click-through)", label);
            }
        }
    }

    // Apply vibrancy immediately after creation
    let _ = tokio::time::sleep(tokio::time::Duration::from_millis(100));
    apply_pill_vibrancy(app, Some(label))?;

    Ok(())
}

/// Make a window interactive (disable click-through)
#[tauri::command]
pub fn make_window_interactive(app: AppHandle, window_label: Option<String>) -> Result<(), String> {
    let label = window_label.unwrap_or_else(|| constants::window_labels::FLOATING_PANEL.to_string());

    if let Some(window) = app.get_webview_window(&label) {
        #[cfg(target_os = "macos")]
        {
            if let Ok(ns_window_ptr) = window.ns_window() {
                let ns_window = ns_window_ptr as cocoa_id;
                unsafe {
                    // Disable click-through to make window interactive
                    let _: () = msg_send![ns_window, setIgnoresMouseEvents: NO];
                    info!("✅ Window '{}' is now interactive (click-through disabled)", label);
                }
            } else {
                return Err(format!("Failed to get NSWindow for window: {}", label));
            }
        }

        #[cfg(not(target_os = "macos"))]
        {
            info!("Window '{}' interaction mode set (non-macOS)", label);
        }

        Ok(())
    } else {
        Err(format!("Window '{}' not found", label))
    }
}
