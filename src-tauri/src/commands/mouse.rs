// Commands related to mouse actions (clicks, movement, position)

use crate::commands::debug_utils::{
    log_debug_operation, send_debug_notification, should_enable_debug, validators, DebugConfig,
};
use crate::constants::mouse::movement;
use crate::constants::{events, timeouts};
use crate::state::AppState;
use tauri::{AppHandle, Emitter, Manager, State};
use tracing::{error, info};
// Import constants to replace magic numbers

// Helper function to perform smooth mouse movement with cursor highlighting
pub async fn smooth_mouse_move(
    app: &AppHandle,
    state: &State<'_, AppState>,
    target_x: f64,
    target_y: f64,
    duration_ms: Option<u64>,
) -> Result<(), String> {
    let duration = duration_ms.unwrap_or(movement::DEFAULT_MOVEMENT_DURATION_MS);

    // Get current cursor position
    let current_pos = match state.desktop.cursor_position() {
        Ok(pos) => pos,
        Err(e) => {
            error!("Failed to get current cursor position: {}", e);
            // If we can't get current position, just move directly
            return match state.desktop.mouse_move(target_x, target_y) {
                Ok(_) => Ok(()),
                Err(e) => Err(format!("Failed to move mouse: {}", e)),
            };
        }
    };

    let start_x = current_pos.0;
    let start_y = current_pos.1;

    // Calculate distance and check if smooth movement is needed
    let distance = ((target_x - start_x).powi(2) + (target_y - start_y).powi(2)).sqrt();

    // If distance is too small, just move directly
    if distance < movement::MIN_MOVEMENT_DISTANCE {
        return match state.desktop.mouse_move(target_x, target_y) {
            Ok(_) => Ok(()),
            Err(e) => Err(format!("Failed to move mouse: {}", e)),
        };
    }

    // Start cursor highlighting
    if let Err(e) = app.emit(events::ui::UI_CURSOR_HIGHLIGHT_START, (start_x, start_y)) {
        error!("Failed to emit cursor highlight start: {}", e);
    }

    // Calculate number of frames needed
    let total_frames = (duration / movement::SMOOTH_MOVEMENT_FRAME_TIME_MS).max(1);

    // Perform smooth movement with ease-out curve
    for frame in 0..=total_frames {
        let progress = frame as f64 / total_frames as f64;

        // Ease-out curve: 1 - (1 - t)^3
        let eased_progress = 1.0 - (1.0 - progress).powi(3);

        let current_x = start_x + (target_x - start_x) * eased_progress;
        let current_y = start_y + (target_y - start_y) * eased_progress;

        // Move mouse to current position
        if let Err(e) = state.desktop.mouse_move(current_x, current_y) {
            error!("Failed to move mouse during smooth movement: {}", e);
            // Continue with the movement even if one frame fails
        }

        // Emit cursor highlight move event
        if let Err(e) = app.emit(events::ui::UI_CURSOR_HIGHLIGHT_MOVE, (current_x, current_y)) {
            error!("Failed to emit cursor highlight move: {}", e);
        }

        // Wait for next frame (except on last frame)
        if frame < total_frames {
            tokio::time::sleep(tokio::time::Duration::from_millis(
                movement::SMOOTH_MOVEMENT_FRAME_TIME_MS,
            ))
            .await;
        }
    }

    // Stop cursor highlighting
    if let Err(e) = app.emit(events::ui::UI_CURSOR_HIGHLIGHT_STOP, (target_x, target_y)) {
        error!("Failed to emit cursor highlight stop: {}", e);
    }

    Ok(())
}

// Helper function to create a visual indicator for mouse clicks
fn create_click_visualization(app: &AppHandle, x: f64, y: f64, color: &str) -> Result<(), String> {
    // Send an event to the frontend to display a visual indicator
    app.emit(events::ui::CLICK_VISUALIZATION, (x, y, color))
        .map_err(|e| format!("Failed to emit click visualization event: {}", e))?;
    Ok(())
}

// Helper function to ensure the main window has focus for mouse operations
async fn ensure_main_window_focus(app: &AppHandle) -> Result<(), String> {
    if let Some(main_window) = app.get_webview_window("main") {
        if let Err(e) = main_window.set_focus() {
            error!("Failed to focus main window before mouse operation: {}", e);
            // Don't fail the operation, just log the warning
        }
        // Small delay to ensure focus is established
        tokio::time::sleep(tokio::time::Duration::from_millis(
            timeouts::MOUSE_MICRO_DELAY_MS,
        ))
        .await;
    }
    Ok(())
}

// --- PRODUCTION WINDOW RELATIVE CLICK FUNCTIONS WITH DEBUG CAPABILITIES ---

#[cfg(target_os = "macos")]
#[tauri::command]
#[allow(dead_code)] // Used via Tauri frontend
pub(crate) async fn window_relative_click(
    app: AppHandle,
    state: State<'_, AppState>,
    window_id: String,
    x: f64,
    y: f64,
    click_type: Option<String>,
    modifier: Option<String>,
) -> Result<(), String> {
    let debug_enabled = should_enable_debug(false, &state);
    let debug_config = if debug_enabled {
        DebugConfig::development_mode()
    } else {
        DebugConfig::production_mode()
    };

    use computer_use_ai_sdk::platforms::macos::element::MacOSUIElement;

    log_debug_operation(
        "window_relative_click",
        &format!(
            "Window relative click: window_id={}, x={}, y={}, click_type={:?}, modifier={:?}",
            window_id, x, y, click_type, modifier
        ),
        &debug_config,
    );
    info!(
        "Window relative click: window_id={}, x={}, y={}, click_type={:?}, modifier={:?}",
        window_id, x, y, click_type, modifier
    );

    // Find the window by ID
    let desktop = state.get_desktop()?;
    let windows = desktop
        .list_windows()
        .map_err(|e| format!("Failed to list windows: {}", e))?;

    let target_window = windows
        .into_iter()
        .find(|window| window.id().is_some_and(|id| id == window_id))
        .ok_or_else(|| format!("Window with ID '{}' not found", window_id))?;

    // Downcast to MacOSUIElement
    let _macos_element = target_window
        .as_any()
        .downcast_ref::<MacOSUIElement>()
        .ok_or_else(|| "Failed to downcast window element to MacOSUIElement".to_string())?;

    // Convert window-relative coordinates to global coordinates
    let (window_x, window_y, _width, _height) = target_window
        .bounds()
        .map_err(|e| format!("Failed to get window bounds: {}", e))?;
    let global_x = window_x + x;
    let global_y = window_y + y;

    info!(
        "Converted window coordinates ({}, {}) to global coordinates ({}, {})",
        x, y, global_x, global_y
    );

    // Perform the click using existing functionality
    let result = match click_type.as_deref().unwrap_or("left") {
        "left" => left_click(app.clone(), state, global_x, global_y, modifier.clone()).await,
        "right" => right_click(app.clone(), state, global_x, global_y, modifier.clone()).await,
        "double" => double_click(app.clone(), state, global_x, global_y, modifier.clone()).await,
        "middle" => middle_click(app.clone(), state, global_x, global_y, modifier.clone()).await,
        "triple" => triple_click(app.clone(), state, global_x, global_y, modifier.clone()).await,
        unknown => Err(format!("Unsupported click type: {}", unknown)),
    };

    // Send debug notification if enabled
    if debug_config.send_notifications && result.is_ok() {
        let _ = send_debug_notification(
            &app,
            "Window Relative Click",
            &format!(
                "Clicked at window ({}, {}) -> global ({}, {})",
                x, y, global_x, global_y
            ),
        );
    }

    result
}

#[cfg(not(target_os = "macos"))]
#[tauri::command]
pub(crate) async fn window_relative_click(
    _app: AppHandle,
    _state: State<'_, AppState>,
    _window_id: String,
    _x: f64,
    _y: f64,
    _click_type: Option<String>,
    _modifier: Option<String>,
) -> Result<(), String> {
    Err("Window relative click is only supported on macOS currently.".to_string())
}

#[cfg(target_os = "macos")]
#[tauri::command]
#[allow(dead_code)] // Used via Tauri frontend
pub(crate) async fn focused_window_relative_click(
    app: AppHandle,
    state: State<'_, AppState>,
    x: f64,
    y: f64,
    click_type: Option<String>,
    modifier: Option<String>,
) -> Result<(), String> {
    let debug_enabled = should_enable_debug(false, &state);
    let debug_config = if debug_enabled {
        DebugConfig::development_mode()
    } else {
        DebugConfig::production_mode()
    };

    use computer_use_ai_sdk::platforms::macos::element::MacOSUIElement;

    log_debug_operation(
        "focused_window_relative_click",
        &format!(
            "Focused window relative click: x={}, y={}, click_type={:?}, modifier={:?}",
            x, y, click_type, modifier
        ),
        &debug_config,
    );
    info!(
        "Focused window relative click: x={}, y={}, click_type={:?}, modifier={:?}",
        x, y, click_type, modifier
    );

    let desktop = state.get_desktop()?;

    // Get the focused element first
    let focused_element = desktop
        .focused_element()
        .map_err(|e| format!("Failed to get focused element: {}", e))?;

    // Check if the focused element is a window, if not try to get its window
    let window_element = {
        let attrs = focused_element.attributes();
        if attrs.role == "AXWindow" {
            focused_element
        } else {
            // Try to traverse up to find the window
            let mut current = focused_element;
            loop {
                match current.parent() {
                    Ok(Some(parent)) => {
                        let parent_attrs = parent.attributes();
                        if parent_attrs.role == "AXWindow" {
                            current = parent;
                            break;
                        }
                        current = parent;
                    }
                    Ok(None) => {
                        return Err("No window found in element hierarchy".to_string());
                    }
                    Err(e) => {
                        return Err(format!("Error traversing element hierarchy: {}", e));
                    }
                }
            }
            current
        }
    };

    // Downcast to MacOSUIElement
    let _macos_element = window_element
        .as_any()
        .downcast_ref::<MacOSUIElement>()
        .ok_or_else(|| "Failed to downcast window element to MacOSUIElement".to_string())?;

    // Convert window-relative coordinates to global coordinates
    let (window_x, window_y, _width, _height) = window_element
        .bounds()
        .map_err(|e| format!("Failed to get window bounds: {}", e))?;
    let global_x = window_x + x;
    let global_y = window_y + y;

    info!(
        "Converted focused window coordinates ({}, {}) to global coordinates ({}, {})",
        x, y, global_x, global_y
    );

    // Perform the click using existing functionality
    let result = match click_type.as_deref().unwrap_or("left") {
        "left" => left_click(app.clone(), state, global_x, global_y, modifier.clone()).await,
        "right" => right_click(app.clone(), state, global_x, global_y, modifier.clone()).await,
        "double" => double_click(app.clone(), state, global_x, global_y, modifier.clone()).await,
        "middle" => middle_click(app.clone(), state, global_x, global_y, modifier.clone()).await,
        "triple" => triple_click(app.clone(), state, global_x, global_y, modifier.clone()).await,
        unknown => Err(format!("Unsupported click type: {}", unknown)),
    };

    // Send debug notification if enabled
    if debug_config.send_notifications && result.is_ok() {
        let _ = send_debug_notification(
            &app,
            "Focused Window Relative Click",
            &format!(
                "Clicked at focused window ({}, {}) -> global ({}, {})",
                x, y, global_x, global_y
            ),
        );
    }

    result
}

#[cfg(not(target_os = "macos"))]
#[tauri::command]
pub(crate) async fn focused_window_relative_click(
    _app: AppHandle,
    _state: State<'_, AppState>,
    _x: f64,
    _y: f64,
    _click_type: Option<String>,
    _modifier: Option<String>,
) -> Result<(), String> {
    Err("Focused window relative click is only supported on macOS currently.".to_string())
}

// --- PRODUCTION MOUSE FUNCTIONS WITH DEBUG CAPABILITIES ---
// These functions replace the dev_ prefixed functions by incorporating debug features conditionally

#[tauri::command]
pub(crate) async fn left_click(
    app: AppHandle,
    state: State<'_, AppState>,
    x: f64,
    y: f64,
    modifier: Option<String>,
) -> Result<(), String> {
    let debug_enabled = should_enable_debug(false, &state);
    let debug_config = if debug_enabled {
        DebugConfig::development_mode()
    } else {
        DebugConfig::production_mode()
    };

    // Debug validation
    if debug_config.validate_inputs {
        validators::valid_coordinates(x, y)?;
    }

    log_debug_operation(
        "left_click",
        &format!("Clicking at ({}, {}) with modifier: {:?}", x, y, modifier),
        &debug_config,
    );
    info!(
        "Executing left_click at screen coordinates ({}, {}) Modifier: {:?}",
        x, y, modifier
    );

    // Ensure main window has focus before performing mouse action
    ensure_main_window_focus(&app).await?;

    create_click_visualization(&app, x, y, "#FF0000")?; // Red for left click

    match state.desktop.left_click(x, y, modifier.as_deref()) {
        Ok(_) => {
            info!("Successfully performed left click at ({}, {})", x, y);

            // Send debug notification if enabled
            if debug_config.send_notifications {
                let _ = send_debug_notification(
                    &app,
                    "Left Click",
                    &format!("Clicked at ({}, {})", x, y),
                );
            }

            Ok(())
        }
        Err(e) => {
            let error_msg = format!("Failed to perform left click: {}", e);
            error!("{}", error_msg);
            Err(error_msg)
        }
    }
}

#[tauri::command]
pub(crate) async fn right_click(
    app: AppHandle,
    state: State<'_, AppState>,
    x: f64,
    y: f64,
    modifier: Option<String>,
) -> Result<(), String> {
    let debug_enabled = should_enable_debug(false, &state);
    let debug_config = if debug_enabled {
        DebugConfig::development_mode()
    } else {
        DebugConfig::production_mode()
    };

    // Debug validation
    if debug_config.validate_inputs {
        validators::valid_coordinates(x, y)?;
    }

    log_debug_operation(
        "right_click",
        &format!(
            "Right clicking at ({}, {}) with modifier: {:?}",
            x, y, modifier
        ),
        &debug_config,
    );
    info!(
        "Executing right_click at screen coordinates ({}, {}) Modifier: {:?}",
        x, y, modifier
    );

    create_click_visualization(&app, x, y, "#0000FF")?; // Blue for right click

    match state.desktop.right_click(x, y, modifier.as_deref()) {
        Ok(_) => {
            info!("Successfully performed right click at ({}, {})", x, y);

            // Send debug notification if enabled
            if debug_config.send_notifications {
                let _ = send_debug_notification(
                    &app,
                    "Right Click",
                    &format!("Right clicked at ({}, {})", x, y),
                );
            }

            Ok(())
        }
        Err(e) => {
            let error_msg = format!("Failed to perform right click: {}", e);
            error!("{}", error_msg);
            Err(error_msg)
        }
    }
}

#[tauri::command]
pub(crate) async fn mouse_move(
    app: AppHandle,
    state: State<'_, AppState>,
    x: f64,
    y: f64,
) -> Result<(), String> {
    let debug_enabled = should_enable_debug(false, &state);
    let debug_config = if debug_enabled {
        DebugConfig::development_mode()
    } else {
        DebugConfig::production_mode()
    };

    // Debug validation
    if debug_config.validate_inputs {
        validators::valid_coordinates(x, y)?;
    }

    // Check if smooth mouse movement is enabled (default: true)
    let use_smooth_movement = {
        // Try to get from centralized settings first
        if let Ok(settings_manager) = crate::settings::manager::SettingsManager::new(app.clone()) {
            if let Ok(tool_settings) = settings_manager.get_tool_settings().await {
                tool_settings.smooth_mouse_movement
            } else {
                // Fallback to runtime state
                state.get_smooth_mouse_movement().unwrap_or(true)
            }
        } else {
            // Fallback to runtime state
            state.get_smooth_mouse_movement().unwrap_or(true)
        }
    };

    if use_smooth_movement {
        log_debug_operation(
            "mouse_move",
            &format!("Moving mouse smoothly to ({}, {})", x, y),
            &debug_config,
        );
        info!("Executing smooth mouse_move to ({}, {})", x, y);

        // Use smooth mouse movement for better user experience
        match smooth_mouse_move(&app, &state, x, y, None).await {
            Ok(_) => {
                info!("Successfully moved mouse smoothly to ({}, {})", x, y);

                // Send debug notification if enabled
                if debug_config.send_notifications {
                    let _ = send_debug_notification(
                        &app,
                        "Mouse Move",
                        &format!("Moved mouse smoothly to ({}, {})", x, y),
                    );
                }

                Ok(())
            }
            Err(e) => {
                let error_msg = format!("Failed to move mouse smoothly: {}", e);
                error!("{}", error_msg);
                Err(error_msg)
            }
        }
    } else {
        log_debug_operation(
            "mouse_move",
            &format!("Moving mouse immediately to ({}, {})", x, y),
            &debug_config,
        );
        info!("Executing immediate mouse_move to ({}, {})", x, y);

        // Use immediate mouse movement for performance
        match state.desktop.mouse_move(x, y) {
            Ok(_) => {
                info!("Successfully moved mouse immediately to ({}, {})", x, y);

                // Send debug notification if enabled
                if debug_config.send_notifications {
                    let _ = send_debug_notification(
                        &app,
                        "Mouse Move",
                        &format!("Moved mouse immediately to ({}, {})", x, y),
                    );
                }

                Ok(())
            }
            Err(e) => {
                let error_msg = format!("Failed to move mouse immediately: {}", e);
                error!("{}", error_msg);
                Err(error_msg)
            }
        }
    }
}

#[tauri::command]
pub(crate) async fn middle_click(
    app: AppHandle,
    state: State<'_, AppState>,
    x: f64,
    y: f64,
    modifier: Option<String>,
) -> Result<(), String> {
    let debug_enabled = should_enable_debug(false, &state);
    let debug_config = if debug_enabled {
        DebugConfig::development_mode()
    } else {
        DebugConfig::production_mode()
    };

    // Debug validation
    if debug_config.validate_inputs {
        validators::valid_coordinates(x, y)?;
    }

    log_debug_operation(
        "middle_click",
        &format!(
            "Middle clicking at ({}, {}) with modifier: {:?}",
            x, y, modifier
        ),
        &debug_config,
    );
    info!(
        "Executing middle_click at screen coordinates ({}, {}) Modifier: {:?}",
        x, y, modifier
    );

    // Ensure main window has focus before performing mouse action
    ensure_main_window_focus(&app).await?;

    create_click_visualization(&app, x, y, "#FFFF00")?; // Yellow for middle click

    match state.desktop.middle_click(x, y, modifier.as_deref()) {
        Ok(_) => {
            info!("Successfully performed middle click at ({}, {})", x, y);

            // Send debug notification if enabled
            if debug_config.send_notifications {
                let _ = send_debug_notification(
                    &app,
                    "Middle Click",
                    &format!("Middle clicked at ({}, {})", x, y),
                );
            }

            Ok(())
        }
        Err(e) => {
            let error_msg = format!("Failed to perform middle click: {}", e);
            error!("{}", error_msg);
            Err(error_msg)
        }
    }
}

#[tauri::command]
pub(crate) async fn double_click(
    app: AppHandle,
    state: State<'_, AppState>,
    x: f64,
    y: f64,
    modifier: Option<String>,
) -> Result<(), String> {
    let debug_enabled = should_enable_debug(false, &state);
    let debug_config = if debug_enabled {
        DebugConfig::development_mode()
    } else {
        DebugConfig::production_mode()
    };

    // Debug validation
    if debug_config.validate_inputs {
        validators::valid_coordinates(x, y)?;
    }

    log_debug_operation(
        "double_click",
        &format!(
            "Double clicking at ({}, {}) with modifier: {:?}",
            x, y, modifier
        ),
        &debug_config,
    );
    info!(
        "Executing double_click at screen coordinates ({}, {}) Modifier: {:?}",
        x, y, modifier
    );

    // Ensure main window has focus before performing mouse action
    ensure_main_window_focus(&app).await?;

    create_click_visualization(&app, x, y, "#FFA500")?; // Orange for double click

    match state.desktop.double_click(x, y, modifier.as_deref()) {
        Ok(_) => {
            info!("Successfully performed double click at ({}, {})", x, y);

            // Send debug notification if enabled
            if debug_config.send_notifications {
                let _ = send_debug_notification(
                    &app,
                    "Double Click",
                    &format!("Double clicked at ({}, {})", x, y),
                );
            }

            Ok(())
        }
        Err(e) => {
            let error_msg = format!("Failed to perform double click: {}", e);
            error!("{}", error_msg);
            Err(error_msg)
        }
    }
}

#[tauri::command]
pub(crate) async fn triple_click(
    app: AppHandle,
    state: State<'_, AppState>,
    x: f64,
    y: f64,
    modifier: Option<String>,
) -> Result<(), String> {
    let debug_enabled = should_enable_debug(false, &state);
    let debug_config = if debug_enabled {
        DebugConfig::development_mode()
    } else {
        DebugConfig::production_mode()
    };

    // Debug validation
    if debug_config.validate_inputs {
        validators::valid_coordinates(x, y)?;
    }

    log_debug_operation(
        "triple_click",
        &format!(
            "Triple clicking at ({}, {}) with modifier: {:?}",
            x, y, modifier
        ),
        &debug_config,
    );
    info!(
        "Executing triple_click at screen coordinates ({}, {}) Modifier: {:?}",
        x, y, modifier
    );

    // Ensure main window has focus before performing mouse action
    ensure_main_window_focus(&app).await?;

    create_click_visualization(&app, x, y, "#800080")?; // Purple for triple click

    match state.desktop.triple_click(x, y, modifier.as_deref()) {
        Ok(_) => {
            info!("Successfully performed triple click at ({}, {})", x, y);

            // Send debug notification if enabled
            if debug_config.send_notifications {
                let _ = send_debug_notification(
                    &app,
                    "Triple Click",
                    &format!("Triple clicked at ({}, {})", x, y),
                );
            }

            Ok(())
        }
        Err(e) => {
            let error_msg = format!("Failed to perform triple click: {}", e);
            error!("{}", error_msg);
            Err(error_msg)
        }
    }
}

#[tauri::command]
pub(crate) async fn left_mouse_down(
    app: AppHandle,
    state: State<'_, AppState>,
    x: Option<f64>,
    y: Option<f64>,
) -> Result<(), String> {
    let debug_enabled = should_enable_debug(false, &state);
    let debug_config = if debug_enabled {
        DebugConfig::development_mode()
    } else {
        DebugConfig::production_mode()
    };

    // Get coordinates - use provided coordinates or current cursor position
    let (target_x, target_y) = match (x, y) {
        (Some(x_val), Some(y_val)) => {
            // Both coordinates provided
            if debug_config.validate_inputs {
                validators::valid_coordinates(x_val, y_val)?;
            }
            (x_val, y_val)
        }
        (Some(x_val), None) => {
            // Only x provided, get current y
            let cursor_pos = state.desktop.cursor_position().map_err(|e| {
                format!(
                    "Failed to get current cursor position for left_mouse_down: {}",
                    e
                )
            })?;
            if debug_config.validate_inputs {
                validators::valid_coordinates(x_val, cursor_pos.1)?;
            }
            (x_val, cursor_pos.1)
        }
        (None, Some(y_val)) => {
            // Only y provided, get current x
            let cursor_pos = state.desktop.cursor_position().map_err(|e| {
                format!(
                    "Failed to get current cursor position for left_mouse_down: {}",
                    e
                )
            })?;
            if debug_config.validate_inputs {
                validators::valid_coordinates(cursor_pos.0, y_val)?;
            }
            (cursor_pos.0, y_val)
        }
        (None, None) => {
            // Neither coordinate provided, use current cursor position
            let cursor_pos = state.desktop.cursor_position().map_err(|e| {
                format!(
                    "Failed to get current cursor position for left_mouse_down: {}",
                    e
                )
            })?;
            if debug_config.validate_inputs {
                validators::valid_coordinates(cursor_pos.0, cursor_pos.1)?;
            }
            cursor_pos
        }
    };

    let coord_info = match (x, y) {
        (Some(_), Some(_)) => "(both coordinates explicit)",
        (Some(_), None) => "(x explicit, y from cursor)",
        (None, Some(_)) => "(x from cursor, y explicit)",
        (None, None) => "(both coordinates from cursor)",
    };
    log_debug_operation(
        "left_mouse_down",
        &format!(
            "Left mouse down at ({}, {}) {}",
            target_x, target_y, coord_info
        ),
        &debug_config,
    );
    info!(
        "Executing left_mouse_down at ({}, {}) {}",
        target_x, target_y, coord_info
    );

    match state.desktop.left_mouse_down(target_x, target_y) {
        Ok(_) => {
            info!(
                "Successfully performed left mouse down at ({}, {})",
                target_x, target_y
            );

            // Send debug notification if enabled
            if debug_config.send_notifications {
                let _ = send_debug_notification(
                    &app,
                    "Mouse Action",
                    &format!(
                        "Left mouse button pressed at ({}, {}) [{}]",
                        target_x, target_y, coord_info
                    ),
                );
            }

            Ok(())
        }
        Err(e) => {
            let error_msg = format!("Failed to perform left mouse down: {}", e);
            error!("{}", error_msg);
            Err(error_msg)
        }
    }
}

#[tauri::command]
pub(crate) async fn left_mouse_up(
    app: AppHandle,
    state: State<'_, AppState>,
    x: Option<f64>,
    y: Option<f64>,
) -> Result<(), String> {
    let debug_enabled = should_enable_debug(false, &state);
    let debug_config = if debug_enabled {
        DebugConfig::development_mode()
    } else {
        DebugConfig::production_mode()
    };

    // Get coordinates - use provided coordinates or current cursor position
    let (target_x, target_y) = match (x, y) {
        (Some(x_val), Some(y_val)) => {
            // Both coordinates provided
            if debug_config.validate_inputs {
                validators::valid_coordinates(x_val, y_val)?;
            }
            (x_val, y_val)
        }
        (Some(x_val), None) => {
            // Only x provided, get current y
            let cursor_pos = state.desktop.cursor_position().map_err(|e| {
                format!(
                    "Failed to get current cursor position for left_mouse_up: {}",
                    e
                )
            })?;
            if debug_config.validate_inputs {
                validators::valid_coordinates(x_val, cursor_pos.1)?;
            }
            (x_val, cursor_pos.1)
        }
        (None, Some(y_val)) => {
            // Only y provided, get current x
            let cursor_pos = state.desktop.cursor_position().map_err(|e| {
                format!(
                    "Failed to get current cursor position for left_mouse_up: {}",
                    e
                )
            })?;
            if debug_config.validate_inputs {
                validators::valid_coordinates(cursor_pos.0, y_val)?;
            }
            (cursor_pos.0, y_val)
        }
        (None, None) => {
            // Neither coordinate provided, use current cursor position
            let cursor_pos = state.desktop.cursor_position().map_err(|e| {
                format!(
                    "Failed to get current cursor position for left_mouse_up: {}",
                    e
                )
            })?;
            if debug_config.validate_inputs {
                validators::valid_coordinates(cursor_pos.0, cursor_pos.1)?;
            }
            cursor_pos
        }
    };

    let coord_info = match (x, y) {
        (Some(_), Some(_)) => "(both coordinates explicit)",
        (Some(_), None) => "(x explicit, y from cursor)",
        (None, Some(_)) => "(x from cursor, y explicit)",
        (None, None) => "(both coordinates from cursor)",
    };
    log_debug_operation(
        "left_mouse_up",
        &format!(
            "Left mouse up at ({}, {}) {}",
            target_x, target_y, coord_info
        ),
        &debug_config,
    );
    info!(
        "Executing left_mouse_up at ({}, {}) {}",
        target_x, target_y, coord_info
    );

    match state.desktop.left_mouse_up(target_x, target_y) {
        Ok(_) => {
            info!(
                "Successfully performed left mouse up at ({}, {})",
                target_x, target_y
            );

            // Send debug notification if enabled
            if debug_config.send_notifications {
                let _ = send_debug_notification(
                    &app,
                    "Mouse Action",
                    &format!(
                        "Left mouse button released at ({}, {}) [{}]",
                        target_x, target_y, coord_info
                    ),
                );
            }

            Ok(())
        }
        Err(e) => {
            let error_msg = format!("Failed to perform left mouse up: {}", e);
            error!("{}", error_msg);
            Err(error_msg)
        }
    }
}

#[tauri::command]
pub(crate) async fn left_click_drag(
    app: AppHandle,
    state: State<'_, AppState>,
    start_x: f64,
    start_y: f64,
    end_x: f64,
    end_y: f64,
) -> Result<(), String> {
    let debug_enabled = should_enable_debug(false, &state);
    let debug_config = if debug_enabled {
        DebugConfig::development_mode()
    } else {
        DebugConfig::production_mode()
    };

    // Debug validation
    if debug_config.validate_inputs {
        validators::valid_coordinates(start_x, start_y)?;
        validators::valid_coordinates(end_x, end_y)?;
    }

    log_debug_operation(
        "left_click_drag",
        &format!(
            "Left click drag from ({}, {}) to ({}, {})",
            start_x, start_y, end_x, end_y
        ),
        &debug_config,
    );
    info!(
        "Executing left_click_drag from ({}, {}) to ({}, {})",
        start_x, start_y, end_x, end_y
    );

    // Note: No need to pre-position cursor - left_click_drag handles its own positioning
    match state
        .desktop
        .left_click_drag(start_x, start_y, end_x, end_y)
    {
        Ok(_) => {
            info!(
                "Successfully performed left click drag from ({}, {}) to ({}, {})",
                start_x, start_y, end_x, end_y
            );

            // Send debug notification if enabled
            if debug_config.send_notifications {
                let _ = send_debug_notification(
                    &app,
                    "Mouse Action",
                    &format!(
                        "Dragged from ({}, {}) to ({}, {})",
                        start_x, start_y, end_x, end_y
                    ),
                );
            }

            Ok(())
        }
        Err(e) => {
            let error_msg = format!("Failed to perform left click drag: {}", e);
            error!("{}", error_msg);
            Err(error_msg)
        }
    }
}

#[tauri::command]
pub(crate) async fn get_cursor_position(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(f64, f64), String> {
    let debug_enabled = should_enable_debug(false, &state);
    let debug_config = if debug_enabled {
        DebugConfig::development_mode()
    } else {
        DebugConfig::production_mode()
    };

    log_debug_operation(
        "get_cursor_position",
        "Getting cursor position",
        &debug_config,
    );
    info!("Executing get_cursor_position");

    match state.desktop.cursor_position() {
        Ok(pos) => {
            info!(
                "Successfully retrieved cursor position: ({}, {})",
                pos.0, pos.1
            );

            // Send debug notification if enabled
            if debug_config.send_notifications {
                let _ = send_debug_notification(
                    &app,
                    "Cursor Info",
                    &format!("Cursor at ({}, {})", pos.0, pos.1),
                );
            }

            Ok(pos)
        }
        Err(e) => {
            let error_msg = format!("Failed to get cursor position: {}", e);
            error!("{}", error_msg);
            Err(error_msg)
        }
    }
}

#[tauri::command]
pub(crate) async fn get_smooth_mouse_movement_setting(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<bool, String> {
    // Try to get from centralized settings first
    if let Ok(settings_manager) = crate::settings::manager::SettingsManager::new(app.clone()) {
        if let Ok(tool_settings) = settings_manager.get_tool_settings().await {
            // Update runtime state to match centralized settings
            let _ = state.set_smooth_mouse_movement(tool_settings.smooth_mouse_movement);
            return Ok(tool_settings.smooth_mouse_movement);
        }
    }

    // Fallback to runtime state
    state.get_smooth_mouse_movement()
}

#[tauri::command]
pub(crate) async fn set_smooth_mouse_movement_setting(
    app: AppHandle,
    state: State<'_, AppState>,
    enabled: bool,
) -> Result<(), String> {
    // Update runtime state first
    state.set_smooth_mouse_movement(enabled)?;

    // Update centralized settings
    if let Ok(settings_manager) = crate::settings::manager::SettingsManager::new(app.clone()) {
        if let Ok(mut tool_settings) = settings_manager.get_tool_settings().await {
            tool_settings.smooth_mouse_movement = enabled;
            if let Err(e) = settings_manager.set_tool_settings(&tool_settings).await {
                tracing::warn!("Failed to persist smooth mouse movement setting: {}", e);
                // Don't fail the operation if persistence fails
            }
        }
    }

    Ok(())
}

// === Big Cursor Settings ===

#[tauri::command]
pub(crate) async fn get_big_cursor_enabled(
    settings_manager: State<'_, crate::settings::manager::SettingsManager>,
) -> Result<bool, String> {
    let agent_settings = settings_manager.get_agent_settings().await?;
    Ok(agent_settings.big_cursor_enabled)
}

#[tauri::command]
pub(crate) async fn set_big_cursor_enabled(
    settings_manager: State<'_, crate::settings::manager::SettingsManager>,
    enabled: bool,
) -> Result<(), String> {
    let mut agent_settings = settings_manager.get_agent_settings().await?;
    agent_settings.big_cursor_enabled = enabled;
    settings_manager.set_agent_settings(&agent_settings).await?;

    if !enabled {
        crate::cursor_scale::force_restore_cursor_scale();
    }

    info!(
        "Big cursor {}",
        if enabled { "enabled" } else { "disabled" }
    );
    Ok(())
}

#[tauri::command]
pub(crate) async fn get_big_cursor_scale(
    settings_manager: State<'_, crate::settings::manager::SettingsManager>,
) -> Result<f32, String> {
    let agent_settings = settings_manager.get_agent_settings().await?;
    Ok(agent_settings.big_cursor_scale)
}

#[tauri::command]
pub(crate) async fn set_big_cursor_scale(
    settings_manager: State<'_, crate::settings::manager::SettingsManager>,
    scale: f32,
) -> Result<(), String> {
    use crate::constants::settings::validation;
    if !(validation::MIN_BIG_CURSOR_SCALE..=validation::MAX_BIG_CURSOR_SCALE).contains(&scale) {
        return Err(format!(
            "Cursor scale must be between {} and {}",
            validation::MIN_BIG_CURSOR_SCALE,
            validation::MAX_BIG_CURSOR_SCALE
        ));
    }

    let mut agent_settings = settings_manager.get_agent_settings().await?;
    agent_settings.big_cursor_scale = scale;
    settings_manager.set_agent_settings(&agent_settings).await?;

    if crate::cursor_scale::is_cursor_scaled() {
        crate::cursor_scale::update_active_scale(scale as f64);
    }

    info!("Big cursor scale set to {:.1}x", scale);
    Ok(())
}

// === Companion Mode Settings ===

#[tauri::command]
pub(crate) async fn get_companion_mode(
    settings_manager: State<'_, crate::settings::manager::SettingsManager>,
) -> Result<bool, String> {
    let agent_settings = settings_manager.get_agent_settings().await?;
    Ok(agent_settings.companion_mode)
}

#[tauri::command]
pub(crate) async fn set_companion_mode(
    settings_manager: State<'_, crate::settings::manager::SettingsManager>,
    enabled: bool,
) -> Result<(), String> {
    let mut agent_settings = settings_manager.get_agent_settings().await?;
    agent_settings.companion_mode = enabled;
    settings_manager.set_agent_settings(&agent_settings).await?;
    info!(
        "Companion mode {}",
        if enabled { "enabled" } else { "disabled" }
    );
    Ok(())
}

// Window-relative click functions removed due to missing DesktopWrapper functionality
// TODO: Implement window bounds retrieval methods in DesktopWrapper to support:
// - window_relative_click(window_id, relative_x, relative_y, modifier)
// - focused_window_relative_click(relative_x, relative_y, modifier)
// These functions require get_window_bounds() and get_focused_window_bounds() methods

#[tauri::command]
pub(crate) fn test_cursor_scale(scale: f64) -> Result<(), String> {
    info!("[CursorScale] Test: setting cursor scale to {:.1}x", scale);
    crate::cursor_scale::write_cursor_size_preview(scale);
    Ok(())
}

#[tauri::command]
pub(crate) fn test_cursor_restore() -> Result<(), String> {
    info!("[CursorScale] User-initiated cursor reset to default");
    crate::cursor_scale::reset_cursor_to_default();
    Ok(())
}

#[tauri::command]
pub(crate) fn get_system_cursor_size() -> Result<f64, String> {
    Ok(crate::cursor_scale::get_system_cursor_size())
}
