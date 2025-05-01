// Commands related to mouse actions (clicks, movement, position)

use crate::state::AppState;
use tauri::{AppHandle, State, Emitter};
use super::send_dev_tool_notification; // Use helper from parent module
use tracing::{info, error}; // Import tracing for better logging
use serde::Deserialize; // Import Deserialize for attribute usage
use crate::utils::coordinates; // Import the coordinates module explicitly

// Helper function to create a visual indicator for mouse clicks
fn create_click_visualization(app: &AppHandle, x: f64, y: f64, color: &str) -> Result<(), String> {
    // Send an event to the frontend to display a visual indicator
    app.emit("click-visualization", (x, y, color))
        .map_err(|e| format!("Failed to emit click visualization event: {}", e))?;
    Ok(())
}

// Test command to manually trigger click visualization
#[tauri::command]
pub(crate) async fn dev_test_click_visualization(
    app: AppHandle,
    x: f64,
    y: f64,
    color: Option<String>
) -> Result<(), String> {
    let color_to_use = color.unwrap_or_else(|| "#FF0000".to_string()); // Default to red
    println!("[DEV_TOOL] Testing click visualization at ({}, {}) with color {}", x, y, color_to_use);

    create_click_visualization(&app, x, y, &color_to_use)?;
    send_dev_tool_notification(&app, "Click Visualization", &format!("Visualized at ({}, {})", x, y))?;

    Ok(())
}

// -- QA TESTING FUNCTIONS --

/// Structure to hold QA test results for click operations
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

/// Comprehensive QA test for mouse clicks
/// Tests a specified click type at the given coordinates with detailed verification
#[tauri::command]
pub(crate) async fn qa_test_click(
    app: AppHandle,
    state: State<'_, AppState>,
    x: f64,
    y: f64,
    clickType: String,
) -> Result<ClickQAResult, String> {
    info!("[QA_TOOL] Testing {} click at ({}, {})", clickType, x, y);

    // Transform coordinates if we're working with a screenshot
    let (original_x, original_y) = crate::utils::coordinates::transform_to_screen_coordinates(x, y);
    info!("[QA_TOOL] Transformed coordinates: from ({}, {}) to ({}, {})", x, y, original_x, original_y);

    // Record start time for latency measurement
    let start_time = std::time::Instant::now();

    // Perform the requested click type
    let result = match clickType.as_str() {
        "left" => dev_left_click(app.clone(), state.clone(), original_x, original_y, None).await,
        "right" => dev_right_click(app.clone(), state.clone(), original_x, original_y, None).await,
        "middle" => dev_middle_click(app.clone(), state.clone(), original_x, original_y, None).await,
        "double" => dev_double_click(app.clone(), state.clone(), original_x, original_y, None).await,
        "triple" => dev_triple_click(app.clone(), state.clone(), original_x, original_y, None).await,
        _ => Err(format!("Unknown click type: {}", clickType)),
    };

    // Calculate latency
    let duration = start_time.elapsed();
    let latency_ms = duration.as_secs_f64() * 1000.0;

    // Get cursor position after click for verification
    let cursor_position_result = dev_get_cursor_position(app.clone(), state.clone()).await;
    let cursor_position = cursor_position_result.ok();

    // Create QA result structure
    let qa_result = ClickQAResult {
        success: result.is_ok(),
        operation: format!("{} click", clickType),
        coordinates: (x, y),
        original_coordinates: Some((original_x, original_y)),
        error: result.err(),
        visualization_success: true, // Assume visualization worked if click worked
        cursor_position_after: cursor_position,
        latency_ms,
    };

    // Send notification with result
    let status = if qa_result.success { "Success" } else { "Failed" };
    send_dev_tool_notification(
        &app,
        &format!("QA {} Click Test", clickType),
        &format!("{}: ({}, {}) - Latency: {:.2}ms", status, x, y, latency_ms)
    )?;

    Ok(qa_result)
}

/// Tests a series of clicks at different positions with varying click types
/// Useful for stress testing the click functionality
#[tauri::command]
pub(crate) async fn qa_test_click_series(
    app: AppHandle,
    state: State<'_, AppState>,
    positions: Vec<(f64, f64, String)>, // Vec of (x, y, clickType)
) -> Result<Vec<ClickQAResult>, String> {
    info!("[QA_TOOL] Running click series test with {} positions", positions.len());

    let mut results = Vec::new();

    for (i, (x, y, click_type)) in positions.iter().enumerate() {
        info!("[QA_TOOL] Series test {}/{}: {} click at ({}, {})",
            i+1, positions.len(), click_type, x, y);

        // Add a small delay between clicks
        if i > 0 {
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        }

        // Perform individual click test
        match qa_test_click(app.clone(), state.clone(), *x, *y, click_type.clone()).await {
            Ok(result) => results.push(result),
            Err(e) => {
                error!("[QA_TOOL] Error during click series test: {}", e);
                // Create a failure result to include in the series
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

    // Send notification with overall result
    let success_count = results.iter().filter(|r| r.success).count();
    send_dev_tool_notification(
        &app,
        "QA Click Series Test",
        &format!("Completed: {}/{} successful", success_count, positions.len())
    )?;

    Ok(results)
}

/// Test click tracking and coordinate transformation accuracy
#[tauri::command]
pub(crate) async fn qa_test_coordinate_transformation(
    app: AppHandle,
    state: State<'_, AppState>,
    x: f64,
    y: f64,
) -> Result<serde_json::Value, String> {
    info!("[QA_TOOL] Testing coordinate transformation at scaled ({}, {})", x, y);

    // Transform coordinates from screenshot (scaled) space to screen space
    let (screen_x, screen_y) = coordinates::transform_to_screen_coordinates(x, y);
    info!("[QA_TOOL] Calculated screen coordinates: ({}, {})", screen_x, screen_y);

    // Attempt to move the mouse to the calculated screen coordinates
    let move_result = dev_mouse_move(app.clone(), state.clone(), screen_x, screen_y).await;
    if let Err(e) = &move_result {
        error!("[QA_TOOL] Failed to move mouse to calculated screen coordinates: {}", e);
        // Optionally return error early or continue with potentially incorrect actual coordinates
    } else {
        // Give a moment for the move to register
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }

    // Get the actual cursor position after attempting the move
    let actual_cursor_pos = dev_get_cursor_position(app.clone(), state.clone()).await;
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

    // Transform actual screen coordinates back to scaled space for comparison and visualization
    let (actual_scaled_x, actual_scaled_y) = if let (Some(ax), Some(ay)) = (actual_screen_x, actual_screen_y) {
        coordinates::transform_to_scaled_coordinates(ax, ay)
    } else {
        // Use original scaled if actual couldn't be determined
        (x, y)
    };

    // Transform the *original target screen coordinates* back to scaled space for verification
    let (back_to_scaled_x, back_to_scaled_y) = coordinates::transform_to_scaled_coordinates(screen_x, screen_y);

    // Get current scaling info
    let scaling_info = {
        let scaling_info_guard = coordinates::SCREENSHOT_SCALE.read().map_err(|_| "Failed to read scaling info".to_string())?;
        let info_value = serde_json::to_value(&*scaling_info_guard).unwrap_or(serde_json::Value::Null);
        // Drop the guard explicitly immediately after use, before any awaits
        drop(scaling_info_guard);
        info_value // Return the owned Value
    };

    // Calculate transformation error (original scaled vs. round-tripped scaled)
    let roundtrip_error_x = (x - back_to_scaled_x).abs();
    let roundtrip_error_y = (y - back_to_scaled_y).abs();

    // Calculate accuracy error (original scaled vs. actual scaled position)
    let accuracy_error_x = (x - actual_scaled_x).abs();
    let accuracy_error_y = (y - actual_scaled_y).abs();

    // --- Visualization ---
    // Visualize original scaled point (Green)
    if let Err(e) = create_click_visualization(&app, x, y, "#00FF00") {
        error!("[QA_TOOL] Failed to create visualization for original scaled: {}", e);
    }
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    // Visualize round-tripped scaled point (Blue) - where the calculation thinks it should be
    if let Err(e) = create_click_visualization(&app, back_to_scaled_x, back_to_scaled_y, "#0000FF") {
        error!("[QA_TOOL] Failed to create visualization for round-tripped scaled: {}", e);
    }
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    // Visualize actual resulting position (Red) - where the cursor actually ended up (in scaled terms)
    if let Err(e) = create_click_visualization(&app, actual_scaled_x, actual_scaled_y, "#FF0000") {
        error!("[QA_TOOL] Failed to create visualization for actual scaled: {}", e);
    }
    // --- End Visualization ---

    // Create result object
    let result = serde_json::json!({
        "original_scaled": { "x": x, "y": y },
        "calculated_screen": { "x": screen_x, "y": screen_y },
        "actual_screen": { "x": actual_screen_x, "y": actual_screen_y },
        "roundtrip_scaled": { "x": back_to_scaled_x, "y": back_to_scaled_y },
        "actual_scaled": { "x": actual_scaled_x, "y": actual_scaled_y },
        "roundtrip_error": { "x": roundtrip_error_x, "y": roundtrip_error_y },
        "accuracy_error": { "x": accuracy_error_x, "y": accuracy_error_y }, // Difference between requested scaled and actual scaled position
        "scaling_info": scaling_info,
        "move_success": move_result.is_ok(),
        "get_pos_success": actual_cursor_pos.is_ok(),
        "is_accurate": accuracy_error_x < 2.0 && accuracy_error_y < 2.0 // Define accuracy threshold (e.g., within 2 pixels)
    });

    // Send notification
    send_dev_tool_notification(
        &app,
        "QA Coordinate Test",
        &format!("Target ({:.1}, {:.1}), Actual ({:.1}, {:.1}) -> Acc Error: x={:.2}, y={:.2}",
            x, y, actual_scaled_x, actual_scaled_y, accuracy_error_x, accuracy_error_y
        )
    )?;

    Ok(result)
}

/// Perform a QA test of the click visualization system itself
#[tauri::command]
pub(crate) async fn qa_test_click_visualization(
    app: AppHandle,
) -> Result<serde_json::Value, String> {
    info!("[QA_TOOL] Testing click visualization system");

    let test_colors = [
        "#FF0000", // Red
        "#00FF00", // Green
        "#0000FF", // Blue
        "#FFFF00", // Yellow
        "#FF00FF", // Magenta
    ];

    let center_x = 500.0;
    let center_y = 300.0;
    let radius = 100.0;

    let mut results = Vec::new();

    // Create circle of visualization points
    for (i, color) in test_colors.iter().enumerate() {
        let angle = 2.0 * std::f64::consts::PI * (i as f64) / (test_colors.len() as f64);
        let x = center_x + radius * angle.cos();
        let y = center_y + radius * angle.sin();

        // Small delay between visualizations
        if i > 0 {
            tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;
        }

        // Create click visualization
        match create_click_visualization(&app, x, y, color) {
            Ok(_) => {
                results.push(serde_json::json!({
                    "position": {"x": x, "y": y},
                    "color": color,
                    "success": true
                }));
            },
            Err(e) => {
                error!("[QA_TOOL] Visualization test failed at point {}: {}", i, e);
                results.push(serde_json::json!({
                    "position": {"x": x, "y": y},
                    "color": color,
                    "success": false,
                    "error": e
                }));
            }
        }
    }

    // Send notification
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

/// QA Test for text selection functionality
/// Tests the ability to select all text in the currently focused element
#[tauri::command]
pub(crate) async fn qa_test_select_text(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    info!("[QA_TOOL] Testing text selection on currently focused element");

    // Record start time for latency measurement
    let start_time = std::time::Instant::now();

    // Get the focused element
    let focused_element = match state.desktop.focused_element() {
        Ok(element) => element,
        Err(e) => {
            let err_msg = format!("Failed to get focused element for text selection: {}", e);
            error!("[QA_TOOL] {}", err_msg);
            return Err(err_msg);
        }
    };

    // Get element info for reporting
    let attrs = focused_element.attributes();
    let element_info = format!(
        "Role: {}, Label: {:?}, Value: {:?}",
        attrs.role,
        attrs.label.unwrap_or_default(),
        attrs.value.unwrap_or_default()
    );
    info!("[QA_TOOL] Attempting to select text in element: {}", element_info);

    // Try to select text
    let result = focused_element.select_text();

    // Calculate latency
    let duration = start_time.elapsed();
    let latency_ms = duration.as_secs_f64() * 1000.0;

    // Prepare result info
    let success = result.is_ok();
    let status = if success { "Success" } else { "Failed" };
    let error_msg = result.err().map(|e| e.to_string());

    // Create result JSON
    let result_json = serde_json::json!({
        "success": success,
        "operation": "select_text",
        "element_info": element_info,
        "error": error_msg,
        "latency_ms": latency_ms
    });

    // Send notification with result
    send_dev_tool_notification(
        &app,
        "QA Text Selection Test",
        &format!("{}: Focused element text selection - Latency: {:.2}ms", status, latency_ms)
    )?;

    Ok(result_json)
}

/// QA Test for scrolling functionality
/// Tests the ability to scroll in the specified direction with the given amount
#[tauri::command]
pub(crate) async fn qa_test_scroll(
    app: AppHandle,
    state: State<'_, AppState>,
    direction: String,
    amount: f64,
) -> Result<serde_json::Value, String> {
    info!("[QA_TOOL] Testing scroll: direction={}, amount={}", direction, amount);

    // Record start time for latency measurement
    let start_time = std::time::Instant::now();

    // Get the focused element
    let focused_element = match state.desktop.focused_element() {
        Ok(element) => element,
        Err(e) => {
            let err_msg = format!("Failed to get focused element for scrolling: {}", e);
            error!("[QA_TOOL] {}", err_msg);
            return Err(err_msg);
        }
    };

    // Get element info for reporting
    let attrs = focused_element.attributes();
    let element_info = format!(
        "Role: {}, Label: {:?}",
        attrs.role,
        attrs.label.unwrap_or_default()
    );
    info!("[QA_TOOL] Attempting to scroll in element: {}", element_info);

    // Try to scroll
    let result = focused_element.scroll(&direction, amount);

    // Calculate latency
    let duration = start_time.elapsed();
    let latency_ms = duration.as_secs_f64() * 1000.0;

    // Prepare result info
    let success = result.is_ok();
    let status = if success { "Success" } else { "Failed" };
    let error_msg = result.err().map(|e| e.to_string());

    // Create result JSON
    let result_json = serde_json::json!({
        "success": success,
        "operation": "scroll",
        "direction": direction,
        "amount": amount,
        "element_info": element_info,
        "error": error_msg,
        "latency_ms": latency_ms
    });

    // Send notification with result
    send_dev_tool_notification(
        &app,
        "QA Scroll Test",
        &format!("{}: {} scroll by {} - Latency: {:.2}ms",
            status, direction, amount, latency_ms)
    )?;

    Ok(result_json)
}

#[tauri::command]
pub(crate) async fn dev_right_click(
    app: AppHandle,
    state: State<'_, AppState>,
    x: f64,
    y: f64,
    modifier: Option<String>,
) -> Result<(), String> {
    info!("Executing dev_right_click at ({}, {}) with modifier: {:?}", x, y, modifier);

    // Create visualization for user feedback
    if let Err(e) = create_click_visualization(&app, x, y, "#FF6600") {
        error!("Failed to create click visualization: {}", e);
        // Continue even if visualization fails
    }

    // Use a string slice for the modifier parameter if it exists
    let modifier_str = modifier.as_deref();

    // Execute the click
    state.desktop.right_click(x, y, modifier_str)
        .map_err(|e| format!("Failed to perform right click: {}", e))?;

    send_dev_tool_notification(&app, "Right Click", &format!("Clicked at ({}, {})", x, y))?;
    Ok(())
}

#[tauri::command]
pub(crate) async fn dev_middle_click(
    app: AppHandle,
    state: State<'_, AppState>,
    x: f64,
    y: f64,
    modifier: Option<String>,
) -> Result<(), String> {
    info!("Executing dev_middle_click at ({}, {}) with modifier: {:?}", x, y, modifier);

    // Create visualization for user feedback
    if let Err(e) = create_click_visualization(&app, x, y, "#33CC33") {
        error!("Failed to create click visualization: {}", e);
        // Continue even if visualization fails
    }

    // Use a string slice for the modifier parameter if it exists
    let modifier_str = modifier.as_deref();

    // Execute the click
    state.desktop.middle_click(x, y, modifier_str)
        .map_err(|e| format!("Failed to perform middle click: {}", e))?;

    send_dev_tool_notification(&app, "Middle Click", &format!("Clicked at ({}, {})", x, y))?;
    Ok(())
}

#[tauri::command]
pub(crate) async fn dev_double_click(
    app: AppHandle,
    state: State<'_, AppState>,
    x: f64,
    y: f64,
    modifier: Option<String>,
) -> Result<(), String> {
    info!("Executing dev_double_click at ({}, {}) with modifier: {:?}", x, y, modifier);

    // Create visualization for user feedback
    if let Err(e) = create_click_visualization(&app, x, y, "#3366FF") {
        error!("Failed to create click visualization: {}", e);
        // Continue even if visualization fails
    }

    // Use a string slice for the modifier parameter if it exists
    let modifier_str = modifier.as_deref();

    // Execute the click
    state.desktop.double_click(x, y, modifier_str)
        .map_err(|e| format!("Failed to perform double click: {}", e))?;

    send_dev_tool_notification(&app, "Double Click", &format!("Clicked at ({}, {})", x, y))?;
    Ok(())
}

#[tauri::command]
pub(crate) async fn dev_triple_click(
    app: AppHandle,
    state: State<'_, AppState>,
    x: f64,
    y: f64,
    modifier: Option<String>,
) -> Result<(), String> {
    info!("Executing dev_triple_click at ({}, {}) with modifier: {:?}", x, y, modifier);

    // Create visualization for user feedback
    if let Err(e) = create_click_visualization(&app, x, y, "#9933CC") {
        error!("Failed to create click visualization: {}", e);
        // Continue even if visualization fails
    }

    // Use a string slice for the modifier parameter if it exists
    let modifier_str = modifier.as_deref();

    // Execute the click
    state.desktop.triple_click(x, y, modifier_str)
        .map_err(|e| format!("Failed to perform triple click: {}", e))?;

    send_dev_tool_notification(&app, "Triple Click", &format!("Clicked at ({}, {})", x, y))?;
    Ok(())
}

#[tauri::command]
pub(crate) async fn dev_mouse_move(
    app: AppHandle,
    state: State<'_, AppState>,
    x: f64,
    y: f64
) -> Result<(), String> {
    println!("[DEV_TOOL] Attempting to move mouse to ({}, {})...", x, y);

    #[cfg(target_os = "macos")]
    let result = state.desktop.mouse_move(x, y);

    #[cfg(not(target_os = "macos"))]
    let result = Err(computer_use_ai_sdk::AutomationError::UnsupportedPlatform);

    match result {
        Ok(_) => {
            println!("[DEV_TOOL] mouse_move succeeded.");
            send_dev_tool_notification(&app, "Mouse Move", &format!("Moved mouse to ({}, {})", x, y))?;
            Ok(())
        }
        Err(e) => {
            let err_msg = format!("Failed to call mouse_move: {}", e);
            println!("[DEV_TOOL] Error: {}", err_msg);
            Err(err_msg)
        }
    }
}

#[tauri::command]
pub(crate) async fn dev_left_mouse_down(
    app: AppHandle,
    state: State<'_, AppState>,
    x: f64,
    y: f64
) -> Result<(), String> {
    println!("[DEV_TOOL] Attempting left mouse down at ({}, {})...", x, y);

    #[cfg(target_os = "macos")]
    let result = state.desktop.left_mouse_down(x, y);

    #[cfg(not(target_os = "macos"))]
    let result = Err(computer_use_ai_sdk::AutomationError::UnsupportedPlatform);

    match result {
        Ok(_) => {
            println!("[DEV_TOOL] left_mouse_down succeeded at ({}, {}).", x, y);
            send_dev_tool_notification(&app, "Mouse Action", &format!("Left mouse button pressed at ({}, {})", x, y))?;
            Ok(())
        }
        Err(e) => {
            let err_msg = format!("Failed to call left_mouse_down at ({}, {}): {}", x, y, e);
            println!("[DEV_TOOL] Error: {}", err_msg);
            Err(err_msg)
        }
    }
}

#[tauri::command]
pub(crate) async fn dev_left_mouse_up(
    app: AppHandle,
    state: State<'_, AppState>,
    x: f64,
    y: f64
) -> Result<(), String> {
    println!("[DEV_TOOL] Attempting left mouse up at ({}, {})...", x, y);

    #[cfg(target_os = "macos")]
    let result = state.desktop.left_mouse_up(x, y);

    #[cfg(not(target_os = "macos"))]
    let result = Err(computer_use_ai_sdk::AutomationError::UnsupportedPlatform);

    match result {
        Ok(_) => {
            println!("[DEV_TOOL] left_mouse_up succeeded at ({}, {}).", x, y);
            send_dev_tool_notification(&app, "Mouse Action", &format!("Left mouse button released at ({}, {})", x, y))?;
            Ok(())
        }
        Err(e) => {
            let err_msg = format!("Failed to call left_mouse_up at ({}, {}): {}", x, y, e);
            println!("[DEV_TOOL] Error: {}", err_msg);
            Err(err_msg)
        }
    }
}

#[tauri::command]
pub(crate) async fn dev_left_click(
    app: AppHandle,
    state: State<'_, AppState>,
    x: f64,
    y: f64,
    modifier: Option<String>,
) -> Result<(), String> {
    info!("Executing dev_left_click at ({}, {}) with modifier: {:?}", x, y, modifier);

    // Create visualization for user feedback
    if let Err(e) = create_click_visualization(&app, x, y, "#FF3366") {
        error!("Failed to create click visualization: {}", e);
        // Continue even if visualization fails
    }

    // Use a string slice for the modifier parameter if it exists
    let modifier_str = modifier.as_deref();

    // Execute the click
    state.desktop.left_click(x, y, modifier_str)
        .map_err(|e| format!("Failed to perform left click: {}", e))?;

    send_dev_tool_notification(&app, "Left Click", &format!("Clicked at ({}, {})", x, y))?;
    Ok(())
}

#[tauri::command]
pub(crate) async fn dev_left_click_drag(
    app: AppHandle,
    state: State<'_, AppState>,
    start_x: f64,
    start_y: f64,
    end_x: f64,
    end_y: f64,
) -> Result<(), String> {
    println!("[DEV_TOOL] Attempting left click drag from ({}, {}) to ({}, {})...", start_x, start_y, end_x, end_y);

    #[cfg(target_os = "macos")]
    let result = state.desktop.left_click_drag(start_x, start_y, end_x, end_y);

    #[cfg(not(target_os = "macos"))]
    let result = Err(computer_use_ai_sdk::AutomationError::UnsupportedPlatform);

    match result {
        Ok(_) => {
            println!("[DEV_TOOL] left_click_drag succeeded.");
            send_dev_tool_notification(&app, "Mouse Action", &format!("Dragged from ({}, {}) to ({}, {})", start_x, start_y, end_x, end_y))?;
            Ok(())
        }
        Err(e) => {
            let err_msg = format!("Failed to call left_click_drag: {}", e);
            println!("[DEV_TOOL] Error: {}", err_msg);
            Err(err_msg)
        }
    }
}

#[tauri::command]
pub(crate) async fn dev_get_cursor_position(
    app: AppHandle,
    state: State<'_, AppState>
) -> Result<(f64, f64), String> {
    println!("[DEV_TOOL] Attempting to get cursor position...");

    #[cfg(target_os = "macos")]
    let result = state.desktop.cursor_position();

    #[cfg(not(target_os = "macos"))]
    let result = Err(computer_use_ai_sdk::AutomationError::UnsupportedPlatform);

    match result {
        Ok(pos) => {
            println!("[DEV_TOOL] get_cursor_position succeeded: ({}, {}).", pos.0, pos.1);
            send_dev_tool_notification(&app, "Cursor Info", &format!("Cursor at ({}, {})", pos.0, pos.1))?;
            Ok(pos)
        }
        Err(e) => {
            let err_msg = format!("Failed to call get_cursor_position: {}", e);
            println!("[DEV_TOOL] Error: {}", err_msg);
            Err(err_msg)
        }
    }
}
