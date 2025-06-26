// Commands related to mouse actions (clicks, movement, position)

use tauri::{AppHandle, State, Emitter, Manager};
use crate::state::AppState;
use crate::commands::debug_utils::{DebugConfig, should_enable_debug, log_debug_operation, send_debug_notification, validators};
use tracing::{info, error};
use crate::utils::coordinates;
use crate::constants::{timeouts, events};
use super::send_dev_tool_notification;

// Smooth mouse movement configuration
const SMOOTH_MOVEMENT_FPS: u64 = 60; // 60 FPS for smooth movement
const SMOOTH_MOVEMENT_FRAME_TIME_MS: u64 = 1000 / SMOOTH_MOVEMENT_FPS; // ~16.67ms per frame
const DEFAULT_MOVEMENT_DURATION_MS: u64 = 300; // Default movement duration
const MIN_MOVEMENT_DISTANCE: f64 = 5.0; // Minimum distance to trigger smooth movement

// Helper function to perform smooth mouse movement with cursor highlighting
async fn smooth_mouse_move(
    app: &AppHandle,
    state: &State<'_, AppState>,
    target_x: f64,
    target_y: f64,
    duration_ms: Option<u64>,
) -> Result<(), String> {
    let duration = duration_ms.unwrap_or(DEFAULT_MOVEMENT_DURATION_MS);

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
    if distance < MIN_MOVEMENT_DISTANCE {
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
    let total_frames = (duration / SMOOTH_MOVEMENT_FRAME_TIME_MS).max(1);

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
            tokio::time::sleep(tokio::time::Duration::from_millis(SMOOTH_MOVEMENT_FRAME_TIME_MS)).await;
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

// CONSOLIDATED: dev_test_click_visualization removed - use test_click_visualization production function

// Test command to manually trigger click visualization with production function
#[tauri::command]
pub(crate) async fn test_click_visualization(
    app: AppHandle,
    state: State<'_, AppState>,
    x: f64,
    y: f64,
    color: Option<String>
) -> Result<(), String> {
    let debug_enabled = should_enable_debug(false, &state);
    let debug_config = if debug_enabled { DebugConfig::development_mode() } else { DebugConfig::production_mode() };

    let color_to_use = color.unwrap_or_else(|| "#FF0000".to_string()); // Default to red

    log_debug_operation("test_click_visualization", &format!("Testing click visualization at ({}, {}) with color {}", x, y, color_to_use), &debug_config);
    info!("Testing click visualization at ({}, {}) with color {}", x, y, color_to_use);

    create_click_visualization(&app, x, y, &color_to_use)?;

    // Send debug notification if enabled
    if debug_config.send_notifications {
        let _ = send_debug_notification(&app, "Click Visualization", &format!("Visualized at ({}, {})", x, y));
    }

    Ok(())
}

// --- QA TESTING FUNCTIONS (Keep from main) ---
#[derive(serde::Serialize)]
pub struct ClickQAResult {
    success: bool,
    operation: String,
    coordinates: (f64, f64),
    original_coordinates: Option<(f64, f64)>,
    error: Option<String>,
    visualization_success: bool,
    cursor_position_after: Option<(f64, f64)>,
    latency_ms: f64,
}

#[tauri::command]
#[allow(dead_code)] // Called via Tauri from frontend QA tools
pub(crate) async fn qa_test_click(
    app: AppHandle,
    state: State<'_, AppState>,
    x: f64,
    y: f64,
    click_type: String,
) -> Result<ClickQAResult, String> {
    info!("[QA_TOOL] Testing {} click at ({}, {})", click_type, x, y);
    let (original_x, original_y) = crate::utils::coordinates::transform_to_screen_coordinates(x, y);
    info!("[QA_TOOL] Transformed coordinates: from ({}, {}) to ({}, {})", x, y, original_x, original_y);
    let start_time = std::time::Instant::now();
    let result = match click_type.as_str() {
        "left" => left_click(app.clone(), state.clone(), original_x, original_y, None).await,
        "right" => right_click(app.clone(), state.clone(), original_x, original_y, None).await,
        "middle" => middle_click(app.clone(), state.clone(), original_x, original_y, None).await,
        "double" => double_click(app.clone(), state.clone(), original_x, original_y, None).await,
        "triple" => triple_click(app.clone(), state.clone(), original_x, original_y, None).await,
        _ => Err(format!("Unknown click type: {}", click_type)),
    };
    let duration = start_time.elapsed();
    let latency_ms = duration.as_secs_f64() * 1000.0;
    let cursor_position_result = get_cursor_position(app.clone(), state.clone()).await;
    let cursor_position = cursor_position_result.ok();
    let qa_result = ClickQAResult {
        success: result.is_ok(),
        operation: format!("{} click", click_type),
        coordinates: (x, y),
        original_coordinates: Some((original_x, original_y)),
        error: result.err(),
        visualization_success: true,
        cursor_position_after: cursor_position,
        latency_ms,
    };
    let status = if qa_result.success { "Success" } else { "Failed" };
    send_dev_tool_notification(
        &app,
        &format!("QA {} Click Test", click_type),
        &format!("{}: ({}, {}) - Latency: {:.2}ms", status, x, y, latency_ms)
    )?;
    Ok(qa_result)
}

#[tauri::command]
#[allow(dead_code)] // Called via Tauri from frontend QA tools
pub(crate) async fn qa_test_click_series(
    app: AppHandle,
    state: State<'_, AppState>,
    positions: Vec<(f64, f64, String)>,
) -> Result<Vec<ClickQAResult>, String> {
    info!("[QA_TOOL] Running click series test with {} positions", positions.len());
    let mut results = Vec::new();
    for (i, (x, y, click_type)) in positions.iter().enumerate() {
        info!("[QA_TOOL] Series test {}/{}: {} click at ({}, {})",
            i+1, positions.len(), click_type, x, y);
        if i > 0 {
            tokio::time::sleep(tokio::time::Duration::from_millis(timeouts::DOUBLE_CLICK_DELAY_MS)).await;
        }
        match qa_test_click(app.clone(), state.clone(), *x, *y, click_type.clone()).await {
            Ok(result) => results.push(result),
            Err(e) => {
                error!("[QA_TOOL] Error during click series test: {}", e);
                results.push(ClickQAResult {
                    success: false,
                    operation: format!("{} click", click_type),
                    coordinates: (*x, *y),
                    original_coordinates: None,
                    error: Some(e.clone()),
                    visualization_success: false,
                    cursor_position_after: None,
                    latency_ms: 0.0,
                });
            }
        }
    }
    let success_count = results.iter().filter(|r| r.success).count();
    send_dev_tool_notification(
        &app,
        "QA Click Series Test",
        &format!("Completed: {}/{} successful", success_count, positions.len())
    )?;
    Ok(results)
}

#[tauri::command]
#[allow(dead_code)] // Called via Tauri from frontend QA tools
pub(crate) async fn qa_test_coordinate_transformation(
    app: AppHandle,
    state: State<'_, AppState>,
    x: f64,
    y: f64,
) -> Result<serde_json::Value, String> {
    info!("[QA_TOOL] Testing coordinate transformation at scaled ({}, {})", x, y);
    let (screen_x, screen_y) = coordinates::transform_to_screen_coordinates(x, y);
    info!("[QA_TOOL] Calculated screen coordinates: ({}, {})", screen_x, screen_y);
            let move_result = mouse_move(app.clone(), state.clone(), screen_x, screen_y).await;
    if let Err(e) = &move_result {
        error!("[QA_TOOL] Failed to move mouse to calculated screen coordinates: {}", e);
    } else {
        tokio::time::sleep(tokio::time::Duration::from_millis(timeouts::MOUSE_ACTION_DELAY_MS)).await;
    }
    let actual_cursor_pos = get_cursor_position(app.clone(), state.clone()).await;
    let (actual_screen_x, actual_screen_y) = match actual_cursor_pos {
        Ok(pos) => {
            info!("[QA_TOOL] Actual cursor screen coordinates: ({}, {})", pos.0, pos.1);
            (Some(pos.0), Some(pos.1))
        }
        Err(ref e) => {
            error!("[QA_TOOL] Failed to get actual cursor position: {}", e);
            (None, None)
        }
    };
    let (actual_scaled_x, actual_scaled_y) = if let (Some(ax), Some(ay)) = (actual_screen_x, actual_screen_y) {
        coordinates::transform_to_scaled_coordinates(ax, ay)
    } else {
        (x, y)
    };
    let (back_to_scaled_x, back_to_scaled_y) = coordinates::transform_to_scaled_coordinates(screen_x, screen_y);
    let scaling_info = {
        let scaling_info_guard = coordinates::SCREENSHOT_SCALE.read().map_err(|_| "Failed to read scaling info".to_string())?;
        let info_value = serde_json::to_value(&*scaling_info_guard).unwrap_or(serde_json::Value::Null);
        drop(scaling_info_guard);
        info_value
    };
    let roundtrip_error_x = (x - back_to_scaled_x).abs();
    let roundtrip_error_y = (y - back_to_scaled_y).abs();
    let accuracy_error_x = (x - actual_scaled_x).abs();
    let accuracy_error_y = (y - actual_scaled_y).abs();
    if let Err(e) = create_click_visualization(&app, x, y, "#00FF00") { // Original Scaled (Green)
        error!("[QA_TOOL] Failed to create visualization for original scaled: {}", e);
    }
    tokio::time::sleep(tokio::time::Duration::from_millis(timeouts::MOUSE_CLICK_DELAY_MS)).await;
    if let Err(e) = create_click_visualization(&app, back_to_scaled_x, back_to_scaled_y, "#0000FF") { // Round-tripped Scaled (Blue)
        error!("[QA_TOOL] Failed to create visualization for round-tripped scaled: {}", e);
    }
    tokio::time::sleep(tokio::time::Duration::from_millis(timeouts::MOUSE_CLICK_DELAY_MS)).await;
    if let Err(e) = create_click_visualization(&app, actual_scaled_x, actual_scaled_y, "#FF0000") { // Actual Scaled (Red)
        error!("[QA_TOOL] Failed to create visualization for actual scaled: {}", e);
    }
    let result = serde_json::json!({
        "original_scaled": { "x": x, "y": y },
        "calculated_screen": { "x": screen_x, "y": screen_y },
        "actual_screen": { "x": actual_screen_x, "y": actual_screen_y },
        "roundtrip_scaled": { "x": back_to_scaled_x, "y": back_to_scaled_y },
        "actual_scaled": { "x": actual_scaled_x, "y": actual_scaled_y },
        "roundtrip_error": { "x": roundtrip_error_x, "y": roundtrip_error_y },
        "accuracy_error": { "x": accuracy_error_x, "y": accuracy_error_y },
        "scaling_info": scaling_info,
        "move_success": move_result.is_ok(),
        "get_pos_success": actual_cursor_pos.is_ok(),
        "is_accurate": accuracy_error_x < 2.0 && accuracy_error_y < 2.0
    });
    send_dev_tool_notification(
        &app,
        "QA Coordinate Test",
        &format!("Target ({:.1}, {:.1}), Actual ({:.1}, {:.1}) -> Acc Error: x={:.2}, y={:.2}",
            x, y, actual_scaled_x, actual_scaled_y, accuracy_error_x, accuracy_error_y
        )
    )?;
    Ok(result)
}

#[tauri::command]
#[allow(dead_code)] // Called via Tauri from frontend QA tools
pub(crate) async fn qa_test_click_visualization(
    app: AppHandle,
) -> Result<serde_json::Value, String> {
    info!("[QA_TOOL] Testing click visualization system");
    let test_colors = ["#FF0000", "#00FF00", "#0000FF", "#FFFF00", "#FF00FF"];
    let center_x = 500.0;
    let center_y = 300.0;
    let radius = 100.0;
    let mut results = Vec::new();
    for (i, color) in test_colors.iter().enumerate() {
        let angle = 2.0 * std::f64::consts::PI * (i as f64) / (test_colors.len() as f64);
        let x = center_x + radius * angle.cos();
        let y = center_y + radius * angle.sin();
        if i > 0 {
            tokio::time::sleep(tokio::time::Duration::from_millis(timeouts::MOUSE_SEQUENCE_DELAY_MS)).await;
        }
        match create_click_visualization(&app, x, y, color) {
            Ok(_) => results.push(serde_json::json!({"position": {"x": x, "y": y}, "color": color, "success": true})),
            Err(e) => {
                error!("[QA_TOOL] Visualization test failed at point {}: {}", i, e);
                results.push(serde_json::json!({"position": {"x": x, "y": y}, "color": color, "success": false, "error": e}));
            }
        }
    }
    let success_count = results.iter().filter(|r| r["success"].as_bool().unwrap_or(false)).count();
    send_dev_tool_notification(
        &app,
        "QA Visualization Test",
        &format!("Completed: {}/{} visualization points", success_count, test_colors.len())
    )?;
    Ok(serde_json::json!({
        "test": "click_visualization",
        "results": results,
        "success_rate": (success_count as f32) / (test_colors.len() as f32)
    }))
}

#[tauri::command]
#[allow(dead_code)] // Called via Tauri from frontend QA tools
pub(crate) async fn qa_test_select_text(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    info!("[QA_TOOL] Testing text selection on currently focused element");
    let start_time = std::time::Instant::now();
    let focused_element = match state.desktop.focused_element() {
        Ok(element) => element,
        Err(e) => {
            let err_msg = format!("Failed to get focused element for text selection: {}", e);
            error!("[QA_TOOL] {}", err_msg);
            return Err(err_msg);
        }
    };
    let attrs = focused_element.attributes();
    let element_info = format!(
        "Role: {}, Label: {:?}, Value: {:?}",
        attrs.role,
        attrs.label.unwrap_or_default(),
        attrs.value.unwrap_or_default()
    );
    info!("[QA_TOOL] Attempting to select text in element: {}", element_info);
    let result = focused_element.select_text();
    let duration = start_time.elapsed();
    let latency_ms = duration.as_secs_f64() * 1000.0;
    let success = result.is_ok();
    let status = if success { "Success" } else { "Failed" };
    let error_msg = result.err().map(|e| e.to_string());
    let result_json = serde_json::json!({
        "success": success,
        "operation": "select_text",
        "element_info": element_info,
        "error": error_msg,
        "latency_ms": latency_ms
    });
    send_dev_tool_notification(
        &app,
        "QA Text Selection Test",
        &format!("{}: Focused element text selection - Latency: {:.2}ms", status, latency_ms)
    )?;
    Ok(result_json)
}

#[tauri::command]
#[allow(dead_code)] // Called via Tauri from frontend QA tools
pub(crate) async fn qa_test_scroll(
    app: AppHandle,
    state: State<'_, AppState>,
    direction: String,
    amount: f64,
) -> Result<serde_json::Value, String> {
    info!("[QA_TOOL] Testing scroll: direction={}, amount={}", direction, amount);
    let start_time = std::time::Instant::now();
    let focused_element = match state.desktop.focused_element() {
        Ok(element) => element,
        Err(e) => {
            let err_msg = format!("Failed to get focused element for scrolling: {}", e);
            error!("[QA_TOOL] {}", err_msg);
            return Err(err_msg);
        }
    };
    let attrs = focused_element.attributes();
    let element_info = format!(
        "Role: {}, Label: {:?}",
        attrs.role,
        attrs.label.unwrap_or_default()
    );
    info!("[QA_TOOL] Attempting to scroll in element: {}", element_info);
    let result = focused_element.scroll(&direction, amount);
    let duration = start_time.elapsed();
    let latency_ms = duration.as_secs_f64() * 1000.0;
    let success = result.is_ok();
    let status = if success { "Success" } else { "Failed" };
    let error_msg = result.err().map(|e| e.to_string());
    let result_json = serde_json::json!({
        "success": success,
        "operation": "scroll",
        "direction": direction,
        "amount": amount,
        "element_info": element_info,
        "error": error_msg,
        "latency_ms": latency_ms
    });
    send_dev_tool_notification(
        &app,
        "QA Scroll Test",
        &format!("{}: {} scroll by {} - Latency: {:.2}ms",
            status, direction, amount, latency_ms)
    )?;
    Ok(result_json)
}

// CONSOLIDATED: dev_window_relative_click removed - use window_relative_click production function

// --- PRODUCTION WINDOW RELATIVE CLICK FUNCTIONS WITH DEBUG CAPABILITIES ---

#[cfg(target_os = "macos")]
#[tauri::command]
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
    let debug_config = if debug_enabled { DebugConfig::development_mode() } else { DebugConfig::production_mode() };

    use computer_use_ai_sdk::platforms::macos::element::MacOSUIElement;

    log_debug_operation("window_relative_click",
        &format!("Window relative click: window_id={}, x={}, y={}, click_type={:?}, modifier={:?}",
            window_id, x, y, click_type, modifier), &debug_config);
    info!(
        "Window relative click: window_id={}, x={}, y={}, click_type={:?}, modifier={:?}",
        window_id, x, y, click_type, modifier
    );

    // Find the window by ID
    let desktop = state.get_desktop()?;
    let windows = desktop.list_windows().map_err(|e| format!("Failed to list windows: {}", e))?;

    let target_window = windows
        .into_iter()
        .find(|window| {
            window.id().map_or(false, |id| id == window_id)
        })
        .ok_or_else(|| format!("Window with ID '{}' not found", window_id))?;

    // Downcast to MacOSUIElement
    let _macos_element = target_window
        .as_any()
        .downcast_ref::<MacOSUIElement>()
        .ok_or_else(|| "Failed to downcast window element to MacOSUIElement".to_string())?;

    // Convert window-relative coordinates to global coordinates
    let (window_x, window_y, _width, _height) = target_window.bounds()
        .map_err(|e| format!("Failed to get window bounds: {}", e))?;
    let global_x = window_x + x;
    let global_y = window_y + y;

    info!("Converted window coordinates ({}, {}) to global coordinates ({}, {})", x, y, global_x, global_y);

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
        let _ = send_debug_notification(&app, "Window Relative Click",
            &format!("Clicked at window ({}, {}) -> global ({}, {})", x, y, global_x, global_y));
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

// CONSOLIDATED: dev_focused_window_relative_click removed - use focused_window_relative_click production function

#[cfg(target_os = "macos")]
#[tauri::command]
pub(crate) async fn focused_window_relative_click(
    app: AppHandle,
    state: State<'_, AppState>,
    x: f64,
    y: f64,
    click_type: Option<String>,
    modifier: Option<String>,
) -> Result<(), String> {
    let debug_enabled = should_enable_debug(false, &state);
    let debug_config = if debug_enabled { DebugConfig::development_mode() } else { DebugConfig::production_mode() };

    use computer_use_ai_sdk::platforms::macos::element::MacOSUIElement;

    log_debug_operation("focused_window_relative_click",
        &format!("Focused window relative click: x={}, y={}, click_type={:?}, modifier={:?}",
            x, y, click_type, modifier), &debug_config);
    info!(
        "Focused window relative click: x={}, y={}, click_type={:?}, modifier={:?}",
        x, y, click_type, modifier
    );

    let desktop = state.get_desktop()?;

    // Get the focused element first
    let focused_element = desktop.focused_element()
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
    let (window_x, window_y, _width, _height) = window_element.bounds()
        .map_err(|e| format!("Failed to get window bounds: {}", e))?;
    let global_x = window_x + x;
    let global_y = window_y + y;

    info!("Converted focused window coordinates ({}, {}) to global coordinates ({}, {})", x, y, global_x, global_y);

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
        let _ = send_debug_notification(&app, "Focused Window Relative Click",
            &format!("Clicked at focused window ({}, {}) -> global ({}, {})", x, y, global_x, global_y));
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

    log_debug_operation("mouse_move", &format!("Moving mouse to ({}, {})", x, y), &debug_config);
    info!("Executing mouse_move to ({}, {})", x, y);

    match state.desktop.mouse_move(x, y) {
        Ok(_) => {
            info!("Successfully moved mouse to ({}, {})", x, y);

            // Send debug notification if enabled
            if debug_config.send_notifications {
                let _ = send_debug_notification(&app, "Mouse Move", &format!("Moved mouse to ({}, {})", x, y));
            }

            Ok(())
        }
        Err(e) => {
            let error_msg = format!("Failed to move mouse: {}", e);
            error!("{}", error_msg);
            Err(error_msg)
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
    x: f64,
    y: f64
) -> Result<(), String> {
    let debug_enabled = should_enable_debug(false, &state);
    let debug_config = if debug_enabled { DebugConfig::development_mode() } else { DebugConfig::production_mode() };

    // Debug validation
    if debug_config.validate_inputs {
        validators::valid_coordinates(x, y)?;
    }

    log_debug_operation("left_mouse_down", &format!("Left mouse down at ({}, {})", x, y), &debug_config);
    info!("Executing left_mouse_down at ({}, {})", x, y);

    match state.desktop.left_mouse_down(x, y) {
        Ok(_) => {
            info!("Successfully performed left mouse down at ({}, {})", x, y);

            // Send debug notification if enabled
            if debug_config.send_notifications {
                let _ = send_debug_notification(&app, "Mouse Action", &format!("Left mouse button pressed at ({}, {})", x, y));
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
    x: f64,
    y: f64
) -> Result<(), String> {
    let debug_enabled = should_enable_debug(false, &state);
    let debug_config = if debug_enabled { DebugConfig::development_mode() } else { DebugConfig::production_mode() };

    // Debug validation
    if debug_config.validate_inputs {
        validators::valid_coordinates(x, y)?;
    }

    log_debug_operation("left_mouse_up", &format!("Left mouse up at ({}, {})", x, y), &debug_config);
    info!("Executing left_mouse_up at ({}, {})", x, y);

    match state.desktop.left_mouse_up(x, y) {
        Ok(_) => {
            info!("Successfully performed left mouse up at ({}, {})", x, y);

            // Send debug notification if enabled
            if debug_config.send_notifications {
                let _ = send_debug_notification(&app, "Mouse Action", &format!("Left mouse button released at ({}, {})", x, y));
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


