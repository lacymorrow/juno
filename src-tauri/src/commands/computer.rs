//! # Computer Use API Commands
//! 
//! Purpose: Official implementation of Anthropic's Computer Use API specification
//! providing a unified interface for all mouse, keyboard, and screen operations.
//! 
//! ## Architecture Overview
//! This module implements the complete Computer Use API as a single Tauri command
//! that dispatches to specific handlers based on the action type. All operations
//! follow the official Anthropic specification for compatibility.
//! 
//! ## Supported Actions
//! - **Screen**: screenshot, cursor_position
//! - **Mouse**: click, right_click, middle_click, double_click, triple_click, move, drag
//! - **Keyboard**: type, key, hold_key
//! - **Scroll**: scroll (vertical/horizontal)
//! - **Utility**: wait
//! 
//! ## Coordinate System
//! - Uses absolute screen coordinates (not window-relative)
//! - Automatically scales for high-DPI displays
//! - Validates bounds before execution
//! 
//! ## Related Files
//! - `utils/coordinates.rs` - Coordinate scaling and validation
//! - `commands/core.rs` - Screenshot implementation
//! - `commands/clicks.rs` - Mouse click implementations
//! - `commands/keyboard.rs` - Keyboard input handling
//! 
//! ## Design Decisions
//! - Single entry point for all computer operations (reduces API surface)
//! - Coordinate validation prevents out-of-bounds errors
//! - All operations return consistent ComputerResult structure
//! - Screenshots included in response for visual feedback

use crate::state::AppState;
use crate::utils::coordinates;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};
use tracing::{error, info};
use crate::commands::core::ScreenshotResult as CoreScreenshotResult;

/// Computer action input structure matching the official Anthropic Computer Use API.
/// 
/// # Field Usage by Action Type
/// - `screenshot`: No additional fields required
/// - `click/move`: Requires `coordinate` [x, y]
/// - `drag`: Requires `coordinate` for end position (starts at cursor)
/// - `scroll`: Requires `scroll_count` and optionally `scroll_direction`
/// - `type`: Requires `text` to type
/// - `key`: Requires `text` with key combination (e.g., "cmd+c")
/// - `wait`: Optionally uses `duration` (default 1000ms)
/// 
/// # Coordinate Format
/// Coordinates are absolute screen positions as [x, y] floats.
/// The system automatically handles DPI scaling.
#[derive(Debug, Deserialize)]
pub struct ComputerInput {
    pub action: String,
    pub coordinate: Option<Vec<f64>>,
    // Note: Following official Anthropic Computer Use specification
    // Drag operations start from current cursor position and end at 'coordinate'
    pub text: Option<String>,
    #[serde(rename = "scrollCount")]
    pub scroll_count: Option<i32>,
    #[serde(rename = "scrollDirection")]
    pub scroll_direction: Option<String>,
    pub duration: Option<u64>,
}

/// Computer action result structure
#[derive(Debug, Serialize)]
pub struct ComputerResult {
    pub success: bool,
    pub action: String,
    pub message: Option<String>,
    #[serde(flatten)]
    pub screenshot: Option<CoreScreenshotResult>,
    pub error: Option<String>,
    pub coordinate: Option<Vec<f64>>,
}

/// Main computer command - implements the official Anthropic Computer Use API.
/// 
/// # Purpose
/// Single entry point for all computer automation operations. Dispatches
/// to appropriate handlers based on action type and ensures consistent
/// error handling and response format.
/// 
/// # Arguments
/// * `input` - Action specification with type and parameters
/// * `app_handle` - Tauri app handle for system operations
/// * `state` - Application state for coordinate scaling
/// 
/// # Returns
/// * `Ok(ComputerResult)` - Always returns Ok with success/error in result
/// * `Err(String)` - Only for catastrophic failures
/// 
/// # Error Handling
/// Errors are captured in ComputerResult.error field rather than
/// propagated as Err to ensure consistent API responses.
/// 
/// # Example Usage
/// ```json
/// {
///   "action": "click",
///   "coordinate": [500, 300]
/// }
/// ```
#[tauri::command]
pub async fn computer(
    input: ComputerInput,
    app_handle: AppHandle,
    state: State<'_, AppState>,
) -> Result<ComputerResult, String> {
    info!("Computer command called with action: {}", input.action);

    let result = match input.action.as_str() {
        "screenshot" => handle_screenshot(&app_handle).await,
        "click" => handle_click(&input, &app_handle, state).await,
        "right_click" => handle_right_click(&input, &app_handle, state).await,
        "middle_click" => handle_middle_click(&input, &app_handle, state).await,
        "double_click" => handle_double_click(&input, &app_handle, state).await,
        "triple_click" => handle_triple_click(&input, &app_handle, state).await,
        "left_click_drag" => handle_drag(&input, &app_handle, state).await,
        "move" => handle_move(&input, &app_handle, state).await,
        "scroll" => handle_scroll(&input, &app_handle, state).await,
        "type" => handle_type(&input, &app_handle, state).await,
        "key" => handle_key(&input, &app_handle, state).await,
        "hold_key" => handle_hold_key(&input, &app_handle, state).await,
        "wait" => handle_wait(&input).await,
        "cursor_position" => handle_cursor_position(&app_handle, state).await,
        _ => {
            let error_msg = format!("Unknown computer action: {}", input.action);
            error!("{}", error_msg);
            Err(error_msg)
        }
    };

    match result {
        Ok(mut computer_result) => {
            info!("Computer action '{}' completed successfully", input.action);
            computer_result.success = true;
            computer_result.action = input.action.clone();
            Ok(computer_result)
        }
        Err(e) => {
            error!("Computer action '{}' failed: {}", input.action, e);
            Ok(ComputerResult {
                success: false,
                action: input.action.clone(),
                message: None,
                screenshot: None,
                error: Some(e),
                coordinate: None,
            })
        }
    }
}

// ===== ACTION HANDLERS =====
// Each handler implements a specific computer action following
// the Anthropic Computer Use specification. Handlers are responsible
// for parameter validation and delegating to appropriate subsystems.

async fn handle_screenshot(app_handle: &AppHandle) -> Result<ComputerResult, String> {
    let screenshot_result = crate::commands::core::capture_screenshot_command(app_handle.clone())
        .await
        .map_err(|e| format!("Screenshot failed: {}", e))?;

    Ok(ComputerResult {
        success: true,
        action: "screenshot".to_string(),
        message: Some("Screenshot captured successfully".to_string()),
        screenshot: Some(screenshot_result),
        error: None,
        coordinate: None,
    })
}

async fn handle_click(
    input: &ComputerInput,
    app_handle: &AppHandle,
    state: State<'_, AppState>,
) -> Result<ComputerResult, String> {
    let coordinates = input.coordinate.as_ref()
        .ok_or("Click action requires coordinate parameter")?;

    if coordinates.len() != 2 {
        return Err("Coordinate must be an array of [x, y]".to_string());
    }

    let x = coordinates[0];
    let y = coordinates[1];

    crate::commands::mouse::left_click(app_handle.clone(), state, x, y, None)
        .await
        .map_err(|e| format!("Click failed: {}", e))?;

    Ok(ComputerResult {
        success: true,
        action: "click".to_string(),
        message: Some(format!("Clicked at ({}, {})", x, y)),
        screenshot: None,
        error: None,
        coordinate: None,
    })
}

async fn handle_right_click(
    input: &ComputerInput,
    app_handle: &AppHandle,
    state: State<'_, AppState>,
) -> Result<ComputerResult, String> {
    let coordinates = input.coordinate.as_ref()
        .ok_or("Right click action requires coordinate parameter")?;

    if coordinates.len() != 2 {
        return Err("Coordinate must be an array of [x, y]".to_string());
    }

    let x = coordinates[0];
    let y = coordinates[1];

    crate::commands::mouse::right_click(app_handle.clone(), state, x, y, None)
        .await
        .map_err(|e| format!("Right click failed: {}", e))?;

    Ok(ComputerResult {
        success: true,
        action: "right_click".to_string(),
        message: Some(format!("Right clicked at ({}, {})", x, y)),
        screenshot: None,
        error: None,
        coordinate: None,
    })
}

async fn handle_middle_click(
    input: &ComputerInput,
    app_handle: &AppHandle,
    state: State<'_, AppState>,
) -> Result<ComputerResult, String> {
    let coordinates = input.coordinate.as_ref()
        .ok_or("Middle click action requires coordinate parameter")?;

    if coordinates.len() != 2 {
        return Err("Coordinate must be an array of [x, y]".to_string());
    }

    let x = coordinates[0];
    let y = coordinates[1];

    crate::commands::mouse::middle_click(app_handle.clone(), state, x, y, None)
        .await
        .map_err(|e| format!("Middle click failed: {}", e))?;

    Ok(ComputerResult {
        success: true,
        action: "middle_click".to_string(),
        message: Some(format!("Middle clicked at ({}, {})", x, y)),
        screenshot: None,
        error: None,
        coordinate: None,
    })
}

async fn handle_double_click(
    input: &ComputerInput,
    app_handle: &AppHandle,
    state: State<'_, AppState>,
) -> Result<ComputerResult, String> {
    let coordinates = input.coordinate.as_ref()
        .ok_or("Double click action requires coordinate parameter")?;

    if coordinates.len() != 2 {
        return Err("Coordinate must be an array of [x, y]".to_string());
    }

    let x = coordinates[0];
    let y = coordinates[1];

    crate::commands::mouse::double_click(app_handle.clone(), state, x, y, None)
        .await
        .map_err(|e| format!("Double click failed: {}", e))?;

    Ok(ComputerResult {
        success: true,
        action: "double_click".to_string(),
        message: Some(format!("Double clicked at ({}, {})", x, y)),
        screenshot: None,
        error: None,
        coordinate: None,
    })
}

async fn handle_triple_click(
    input: &ComputerInput,
    app_handle: &AppHandle,
    state: State<'_, AppState>,
) -> Result<ComputerResult, String> {
    let coordinates = input.coordinate.as_ref()
        .ok_or("Triple click action requires coordinate parameter")?;

    if coordinates.len() != 2 {
        return Err("Coordinate must be an array of [x, y]".to_string());
    }

    let x = coordinates[0];
    let y = coordinates[1];

    crate::commands::mouse::triple_click(app_handle.clone(), state, x, y, None)
        .await
        .map_err(|e| format!("Triple click failed: {}", e))?;

    Ok(ComputerResult {
        success: true,
        action: "triple_click".to_string(),
        message: Some(format!("Triple clicked at ({}, {})", x, y)),
        screenshot: None,
        error: None,
        coordinate: None,
    })
}

async fn handle_drag(
    input: &ComputerInput,
    app_handle: &AppHandle,
    state: State<'_, AppState>,
) -> Result<ComputerResult, String> {
    // Following official Anthropic Computer Use specification:
    // Drag starts from current cursor position and ends at 'coordinate'
    let end_coords = input.coordinate.as_ref()
        .ok_or("Drag action requires coordinate parameter (end position)")?;

    if end_coords.len() != 2 {
        return Err("Coordinate must be an array of [x, y]".to_string());
    }

    // Get current cursor position as start point (returns screen coordinates)
    let (start_x, start_y) = crate::commands::mouse::get_cursor_position(app_handle.clone(), state.clone())
        .await
        .map_err(|e| format!("Failed to get cursor position: {}", e))?;

    let end_x = end_coords[0];
    let end_y = end_coords[1];

    // Transform end coordinates from screenshot space to screen space to match start coordinates
    let (screen_end_x, screen_end_y) = coordinates::transform_to_screen_coordinates(end_x, end_y);

    crate::commands::mouse::left_click_drag(
        app_handle.clone(),
        state.clone(),
        start_x,
        start_y,
        screen_end_x,
        screen_end_y,
    )
    .await
    .map_err(|e| format!("Drag failed: {}", e))?;

    Ok(ComputerResult {
        success: true,
        action: "left_click_drag".to_string(),
        message: Some(format!("Dragged from cursor position ({:.1}, {:.1}) to screen coordinates ({:.1}, {:.1})", start_x, start_y, screen_end_x, screen_end_y)),
        screenshot: None,
        error: None,
        coordinate: None,
    })
}

async fn handle_move(
    input: &ComputerInput,
    app_handle: &AppHandle,
    state: State<'_, AppState>,
) -> Result<ComputerResult, String> {
    let coordinates = input.coordinate.as_ref()
        .ok_or("Move action requires coordinate parameter")?;

    if coordinates.len() != 2 {
        return Err("Coordinate must be an array of [x, y]".to_string());
    }

    let x = coordinates[0];
    let y = coordinates[1];

    crate::commands::mouse::mouse_move(app_handle.clone(), state, x, y)
        .await
        .map_err(|e| format!("Mouse move failed: {}", e))?;

    Ok(ComputerResult {
        success: true,
        action: "move".to_string(),
        message: Some(format!("Moved mouse to ({}, {})", x, y)),
        screenshot: None,
        error: None,
        coordinate: None,
    })
}

async fn handle_scroll(
    input: &ComputerInput,
    app_handle: &AppHandle,
    state: State<'_, AppState>,
) -> Result<ComputerResult, String> {
    let coordinates = input.coordinate.as_ref()
        .ok_or("Scroll action requires coordinate parameter")?;

    if coordinates.len() != 2 {
        return Err("Coordinate must be an array of [x, y]".to_string());
    }

    let x = coordinates[0];
    let y = coordinates[1];
    let scroll_count = input.scroll_count.unwrap_or(3);
    let direction = input.scroll_direction.as_deref().unwrap_or("down");

    // Scroll amount is always positive - direction is handled by the scroll_window function
    let scroll_amount = scroll_count as f64;

    // Use the scroll function from window.rs with correct signature
    crate::commands::window::scroll_window(
        direction.to_string(),
        scroll_amount,
        Some(x),
        Some(y),
        app_handle.clone(),
        state,
    )
    .await
    .map_err(|e| format!("Scroll failed: {}", e))?;

    Ok(ComputerResult {
        success: true,
        action: "scroll".to_string(),
        message: Some(format!("Scrolled {} {} times at ({}, {})", direction, scroll_count, x, y)),
        screenshot: None,
        error: None,
        coordinate: None,
    })
}

async fn handle_type(
    input: &ComputerInput,
    app_handle: &AppHandle,
    state: State<'_, AppState>,
) -> Result<ComputerResult, String> {
    let text = input.text.as_ref()
        .ok_or("Type action requires text parameter")?;

    crate::commands::keyboard::global_type_text(text.clone(), app_handle.clone(), state)
        .await
        .map_err(|e| format!("Type failed: {}", e))?;

    Ok(ComputerResult {
        success: true,
        action: "type".to_string(),
        message: Some(format!("Typed text: {}", text)),
        screenshot: None,
        error: None,
        coordinate: None,
    })
}

async fn handle_key(
    input: &ComputerInput,
    app_handle: &AppHandle,
    state: State<'_, AppState>,
) -> Result<ComputerResult, String> {
    let key = input.text.as_ref()
        .ok_or("Key action requires text parameter")?;

    crate::commands::keyboard::press_key(key.clone(), None, app_handle.clone(), state)
        .await
        .map_err(|e| format!("Key press failed: {}", e))?;

    Ok(ComputerResult {
        success: true,
        action: "key".to_string(),
        message: Some(format!("Pressed key: {}", key)),
        screenshot: None,
        error: None,
        coordinate: None,
    })
}

async fn handle_hold_key(
    input: &ComputerInput,
    app_handle: &AppHandle,
    state: State<'_, AppState>,
) -> Result<ComputerResult, String> {
    let key = input.text.as_ref()
        .ok_or("Hold key action requires text parameter")?;

    let duration = input.duration.unwrap_or(1000);

    crate::commands::keyboard::hold_key(key.clone(), Some(duration), app_handle.clone(), state)
        .await
        .map_err(|e| format!("Hold key failed: {}", e))?;

    Ok(ComputerResult {
        success: true,
        action: "hold_key".to_string(),
        message: Some(format!("Held key {} for {}ms", key, duration)),
        screenshot: None,
        error: None,
        coordinate: None,
    })
}

async fn handle_wait(input: &ComputerInput) -> Result<ComputerResult, String> {
    let duration = input.duration.unwrap_or(1000);

    tokio::time::sleep(tokio::time::Duration::from_millis(duration)).await;

    Ok(ComputerResult {
        success: true,
        action: "wait".to_string(),
        message: Some(format!("Waited for {}ms", duration)),
        screenshot: None,
        error: None,
        coordinate: None,
    })
}

async fn handle_cursor_position(
    app_handle: &AppHandle,
    state: State<'_, AppState>,
) -> Result<ComputerResult, String> {
    let (x, y) = crate::commands::mouse::get_cursor_position(app_handle.clone(), state)
        .await
        .map_err(|e| format!("Get cursor position failed: {}", e))?;

    Ok(ComputerResult {
        success: true,
        action: "cursor_position".to_string(),
        message: Some(format!("Cursor position: ({}, {})", x, y)),
        screenshot: None,
        error: None,
        coordinate: Some(vec![x, y]),
    })
}
