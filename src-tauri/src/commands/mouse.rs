// Commands related to mouse actions (clicks, movement, position)

use tauri::{AppHandle, State, Emitter, Manager};
use crate::state::AppState;
use crate::commands::debug_utils::{DebugConfig, should_enable_debug, log_debug_operation, send_debug_notification, validators};
use tracing::{info, error};
use crate::constants::{timeouts, events};
use crate::constants::mouse::movement;



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
            tokio::time::sleep(tokio::time::Duration::from_millis(movement::SMOOTH_MOVEMENT_FRAME_TIME_MS)).await;
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
        tokio::time::sleep(tokio::time::Duration::from_millis(timeouts::MOUSE_MICRO_DELAY_MS)).await;
    }
    Ok(())
}






// --- PRODUCTION MOUSE FUNCTIONS WITH DEBUG CAPABILITIES ---

#[tauri::command]
pub(crate) async fn left_click(
    app: AppHandle,
    state: State<'_, AppState>,
    x: f64,
    y: f64,
    modifier: Option<String>,
) -> Result<(), String> {
    let debug_enabled = should_enable_debug(false, &state);
    let debug_config = if debug_enabled { DebugConfig::development_mode() } else { DebugConfig::production_mode() };

    // Debug validation
    if debug_config.validate_inputs {
        validators::valid_coordinates(x, y)?;
    }

    log_debug_operation("left_click", &format!("Clicking at ({}, {}) with modifier: {:?}", x, y, modifier), &debug_config);
    info!("Executing left_click at screen coordinates ({}, {}) Modifier: {:?}", x, y, modifier);

    // Ensure main window has focus before performing mouse action
    ensure_main_window_focus(&app).await?;

    create_click_visualization(&app, x, y, "#FF0000")?; // Red for left click

    match state.desktop.left_click(x, y, modifier.as_deref()) {
        Ok(_) => {
            info!("Successfully performed left click at ({}, {})", x, y);

            // Send debug notification if enabled
            if debug_config.send_notifications {
                let _ = send_debug_notification(&app, "Left Click", &format!("Clicked at ({}, {})", x, y));
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
    let debug_config = if debug_enabled { DebugConfig::development_mode() } else { DebugConfig::production_mode() };

    // Debug validation
    if debug_config.validate_inputs {
        validators::valid_coordinates(x, y)?;
    }

    log_debug_operation("right_click", &format!("Right clicking at ({}, {}) with modifier: {:?}", x, y, modifier), &debug_config);
    info!("Executing right_click at screen coordinates ({}, {}) Modifier: {:?}", x, y, modifier);

    create_click_visualization(&app, x, y, "#0000FF")?; // Blue for right click

    match state.desktop.right_click(x, y, modifier.as_deref()) {
        Ok(_) => {
            info!("Successfully performed right click at ({}, {})", x, y);

            // Send debug notification if enabled
            if debug_config.send_notifications {
                let _ = send_debug_notification(&app, "Right Click", &format!("Right clicked at ({}, {})", x, y));
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
    y: f64
) -> Result<(), String> {
    let debug_enabled = should_enable_debug(false, &state);
    let debug_config = if debug_enabled { DebugConfig::development_mode() } else { DebugConfig::production_mode() };

    // Debug validation
    if debug_config.validate_inputs {
        validators::valid_coordinates(x, y)?;
    }

    // Check if smooth mouse movement is enabled
    let use_smooth_movement = state.get_smooth_mouse_movement().unwrap_or(false);

    if use_smooth_movement {
        log_debug_operation("mouse_move", &format!("Moving mouse smoothly to ({}, {})", x, y), &debug_config);
        info!("Executing smooth mouse_move to ({}, {})", x, y);

        // Use smooth mouse movement for better user experience
        match smooth_mouse_move(&app, &state, x, y, None).await {
            Ok(_) => {
                info!("Successfully moved mouse smoothly to ({}, {})", x, y);

                // Send debug notification if enabled
                if debug_config.send_notifications {
                    let _ = send_debug_notification(&app, "Mouse Move", &format!("Moved mouse smoothly to ({}, {})", x, y));
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
        log_debug_operation("mouse_move", &format!("Moving mouse immediately to ({}, {})", x, y), &debug_config);
        info!("Executing immediate mouse_move to ({}, {})", x, y);

        // Use immediate mouse movement for performance
        match state.desktop.mouse_move(x, y) {
            Ok(_) => {
                info!("Successfully moved mouse immediately to ({}, {})", x, y);

                // Send debug notification if enabled
                if debug_config.send_notifications {
                    let _ = send_debug_notification(&app, "Mouse Move", &format!("Moved mouse immediately to ({}, {})", x, y));
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
    let debug_config = if debug_enabled { DebugConfig::development_mode() } else { DebugConfig::production_mode() };

    // Debug validation
    if debug_config.validate_inputs {
        validators::valid_coordinates(x, y)?;
    }

    log_debug_operation("middle_click", &format!("Middle clicking at ({}, {}) with modifier: {:?}", x, y, modifier), &debug_config);
    info!("Executing middle_click at screen coordinates ({}, {}) Modifier: {:?}", x, y, modifier);

    // Ensure main window has focus before performing mouse action
    ensure_main_window_focus(&app).await?;

    create_click_visualization(&app, x, y, "#FFFF00")?; // Yellow for middle click

    match state.desktop.middle_click(x, y, modifier.as_deref()) {
        Ok(_) => {
            info!("Successfully performed middle click at ({}, {})", x, y);

            // Send debug notification if enabled
            if debug_config.send_notifications {
                let _ = send_debug_notification(&app, "Middle Click", &format!("Middle clicked at ({}, {})", x, y));
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
    let debug_config = if debug_enabled { DebugConfig::development_mode() } else { DebugConfig::production_mode() };

    // Debug validation
    if debug_config.validate_inputs {
        validators::valid_coordinates(x, y)?;
    }

    log_debug_operation("double_click", &format!("Double clicking at ({}, {}) with modifier: {:?}", x, y, modifier), &debug_config);
    info!("Executing double_click at screen coordinates ({}, {}) Modifier: {:?}", x, y, modifier);

    // Ensure main window has focus before performing mouse action
    ensure_main_window_focus(&app).await?;

    create_click_visualization(&app, x, y, "#FFA500")?; // Orange for double click

    match state.desktop.double_click(x, y, modifier.as_deref()) {
        Ok(_) => {
            info!("Successfully performed double click at ({}, {})", x, y);

            // Send debug notification if enabled
            if debug_config.send_notifications {
                let _ = send_debug_notification(&app, "Double Click", &format!("Double clicked at ({}, {})", x, y));
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
    let debug_config = if debug_enabled { DebugConfig::development_mode() } else { DebugConfig::production_mode() };

    // Debug validation
    if debug_config.validate_inputs {
        validators::valid_coordinates(x, y)?;
    }

    log_debug_operation("triple_click", &format!("Triple clicking at ({}, {}) with modifier: {:?}", x, y, modifier), &debug_config);
    info!("Executing triple_click at screen coordinates ({}, {}) Modifier: {:?}", x, y, modifier);

    // Ensure main window has focus before performing mouse action
    ensure_main_window_focus(&app).await?;

    create_click_visualization(&app, x, y, "#800080")?; // Purple for triple click

    match state.desktop.triple_click(x, y, modifier.as_deref()) {
        Ok(_) => {
            info!("Successfully performed triple click at ({}, {})", x, y);

            // Send debug notification if enabled
            if debug_config.send_notifications {
                let _ = send_debug_notification(&app, "Triple Click", &format!("Triple clicked at ({}, {})", x, y));
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
    y: Option<f64>
) -> Result<(), String> {
    let debug_enabled = should_enable_debug(false, &state);
    let debug_config = if debug_enabled { DebugConfig::development_mode() } else { DebugConfig::production_mode() };

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
            let cursor_pos = state.desktop.cursor_position()
                .map_err(|e| format!("Failed to get current cursor position for left_mouse_down: {}", e))?;
            if debug_config.validate_inputs {
                validators::valid_coordinates(x_val, cursor_pos.1)?;
            }
            (x_val, cursor_pos.1)
        }
        (None, Some(y_val)) => {
            // Only y provided, get current x
            let cursor_pos = state.desktop.cursor_position()
                .map_err(|e| format!("Failed to get current cursor position for left_mouse_down: {}", e))?;
            if debug_config.validate_inputs {
                validators::valid_coordinates(cursor_pos.0, y_val)?;
            }
            (cursor_pos.0, y_val)
        }
        (None, None) => {
            // Neither coordinate provided, use current cursor position
            let cursor_pos = state.desktop.cursor_position()
                .map_err(|e| format!("Failed to get current cursor position for left_mouse_down: {}", e))?;
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
    log_debug_operation("left_mouse_down", &format!("Left mouse down at ({}, {}) {}", target_x, target_y, coord_info), &debug_config);
    info!("Executing left_mouse_down at ({}, {}) {}", target_x, target_y, coord_info);

    match state.desktop.left_mouse_down(target_x, target_y) {
        Ok(_) => {
            info!("Successfully performed left mouse down at ({}, {})", target_x, target_y);

            // Send debug notification if enabled
            if debug_config.send_notifications {
                let _ = send_debug_notification(&app, "Mouse Action", &format!("Left mouse button pressed at ({}, {}) [{}]", target_x, target_y, coord_info));
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
    y: Option<f64>
) -> Result<(), String> {
    let debug_enabled = should_enable_debug(false, &state);
    let debug_config = if debug_enabled { DebugConfig::development_mode() } else { DebugConfig::production_mode() };

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
            let cursor_pos = state.desktop.cursor_position()
                .map_err(|e| format!("Failed to get current cursor position for left_mouse_up: {}", e))?;
            if debug_config.validate_inputs {
                validators::valid_coordinates(x_val, cursor_pos.1)?;
            }
            (x_val, cursor_pos.1)
        }
        (None, Some(y_val)) => {
            // Only y provided, get current x
            let cursor_pos = state.desktop.cursor_position()
                .map_err(|e| format!("Failed to get current cursor position for left_mouse_up: {}", e))?;
            if debug_config.validate_inputs {
                validators::valid_coordinates(cursor_pos.0, y_val)?;
            }
            (cursor_pos.0, y_val)
        }
        (None, None) => {
            // Neither coordinate provided, use current cursor position
            let cursor_pos = state.desktop.cursor_position()
                .map_err(|e| format!("Failed to get current cursor position for left_mouse_up: {}", e))?;
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
    log_debug_operation("left_mouse_up", &format!("Left mouse up at ({}, {}) {}", target_x, target_y, coord_info), &debug_config);
    info!("Executing left_mouse_up at ({}, {}) {}", target_x, target_y, coord_info);

    match state.desktop.left_mouse_up(target_x, target_y) {
        Ok(_) => {
            info!("Successfully performed left mouse up at ({}, {})", target_x, target_y);

            // Send debug notification if enabled
            if debug_config.send_notifications {
                let _ = send_debug_notification(&app, "Mouse Action", &format!("Left mouse button released at ({}, {}) [{}]", target_x, target_y, coord_info));
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
    let debug_config = if debug_enabled { DebugConfig::development_mode() } else { DebugConfig::production_mode() };

    // Debug validation
    if debug_config.validate_inputs {
        validators::valid_coordinates(start_x, start_y)?;
        validators::valid_coordinates(end_x, end_y)?;
    }

    log_debug_operation("left_click_drag", &format!("Left click drag from ({}, {}) to ({}, {})", start_x, start_y, end_x, end_y), &debug_config);
    info!("Executing left_click_drag from ({}, {}) to ({}, {})", start_x, start_y, end_x, end_y);

    // Note: No need to pre-position cursor - left_click_drag handles its own positioning
    match state.desktop.left_click_drag(start_x, start_y, end_x, end_y) {
        Ok(_) => {
            info!("Successfully performed left click drag from ({}, {}) to ({}, {})", start_x, start_y, end_x, end_y);

            // Send debug notification if enabled
            if debug_config.send_notifications {
                let _ = send_debug_notification(&app, "Mouse Action", &format!("Dragged from ({}, {}) to ({}, {})", start_x, start_y, end_x, end_y));
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
    state: State<'_, AppState>
) -> Result<(f64, f64), String> {
    let debug_enabled = should_enable_debug(false, &state);
    let debug_config = if debug_enabled { DebugConfig::development_mode() } else { DebugConfig::production_mode() };

    log_debug_operation("get_cursor_position", "Getting cursor position", &debug_config);
    info!("Executing get_cursor_position");

    match state.desktop.cursor_position() {
        Ok(pos) => {
            info!("Successfully retrieved cursor position: ({}, {})", pos.0, pos.1);

            // Send debug notification if enabled
            if debug_config.send_notifications {
                let _ = send_debug_notification(&app, "Cursor Info", &format!("Cursor at ({}, {})", pos.0, pos.1));
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
    state: State<'_, AppState>
) -> Result<bool, String> {
    state.get_smooth_mouse_movement()
}

#[tauri::command]
pub(crate) async fn set_smooth_mouse_movement_setting(
    state: State<'_, AppState>,
    enabled: bool
) -> Result<(), String> {
    state.set_smooth_mouse_movement(enabled)
}

// Window-relative click functions removed due to missing DesktopWrapper functionality
// TODO: Implement window bounds retrieval methods in DesktopWrapper to support:
// - window_relative_click(window_id, relative_x, relative_y, modifier)
// - focused_window_relative_click(relative_x, relative_y, modifier)
// These functions require get_window_bounds() and get_focused_window_bounds() methods


