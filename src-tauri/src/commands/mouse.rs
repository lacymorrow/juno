// Commands related to mouse actions (clicks, movement, position)

use crate::state::AppState;
use tauri::{AppHandle, State};
use super::send_dev_tool_notification; // Use helper from parent module


#[tauri::command]
pub(crate) async fn dev_triple_click(
    app: AppHandle,
    state: State<'_, AppState>,
    x: f64,
    y: f64
) -> Result<(), String> {
    println!("[DEV_TOOL] Attempting to triple click at ({}, {})...", x, y);

    #[cfg(target_os = "macos")]
    let result = state.desktop.triple_click(x, y);

    #[cfg(not(target_os = "macos"))]
    let result = Err(computer_use_ai_sdk::AutomationError::UnsupportedPlatform);

    match result {
        Ok(_) => {
            println!("[DEV_TOOL] triple_click succeeded.");
             send_dev_tool_notification(&app, "Triple Click", &format!("Clicked at ({}, {})", x, y))?;
            Ok(())
        }
        Err(e) => {
            let err_msg = format!("Failed to call triple_click: {}", e);
            println!("[DEV_TOOL] Error: {}", err_msg);
            Err(err_msg)
        }
    }
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
) -> Result<(), String> {
    println!("[DEV_TOOL] Attempting left click at ({}, {})...", x, y);

    #[cfg(target_os = "macos")]
    let result = state.desktop.left_click(x, y);

    #[cfg(not(target_os = "macos"))]
    let result = Err(computer_use_ai_sdk::AutomationError::UnsupportedPlatform);

    match result {
        Ok(_) => {
            println!("[DEV_TOOL] left_click at ({}, {}) succeeded.", x, y);
            send_dev_tool_notification(&app, "Mouse Action", &format!("Left clicked at ({}, {})", x, y))?;
            Ok(())
        }
        Err(e) => {
            let err_msg = format!("Failed to call left_click at ({}, {}): {}", x, y, e);
            println!("[DEV_TOOL] Error: {}", err_msg);
            Err(err_msg)
        }
    }
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
pub(crate) async fn dev_right_click(
    app: AppHandle,
    state: State<'_, AppState>,
    x: f64,
    y: f64,
) -> Result<(), String> {
    println!("[DEV_TOOL] Attempting right click at ({}, {})...", x, y);

    #[cfg(target_os = "macos")]
    let result = state.desktop.right_click(x, y);

    #[cfg(not(target_os = "macos"))]
    let result = Err(computer_use_ai_sdk::AutomationError::UnsupportedPlatform);

    match result {
        Ok(_) => {
            println!("[DEV_TOOL] right_click at ({}, {}) succeeded.", x, y);
            send_dev_tool_notification(&app, "Mouse Action", &format!("Right clicked at ({}, {})", x, y))?;
            Ok(())
        }
        Err(e) => {
            let err_msg = format!("Failed to call right_click at ({}, {}): {}", x, y, e);
            println!("[DEV_TOOL] Error: {}", err_msg);
            Err(err_msg)
        }
    }
}

#[tauri::command]
pub(crate) async fn dev_middle_click(
    app: AppHandle,
    state: State<'_, AppState>,
    x: f64,
    y: f64,
) -> Result<(), String> {
    println!("[DEV_TOOL] Attempting middle click at ({}, {})...", x, y);

    #[cfg(target_os = "macos")]
    let result = state.desktop.middle_click(x, y);

    #[cfg(not(target_os = "macos"))]
    let result = Err(computer_use_ai_sdk::AutomationError::UnsupportedPlatform);

    match result {
        Ok(_) => {
            println!("[DEV_TOOL] middle_click at ({}, {}) succeeded.", x, y);
            send_dev_tool_notification(&app, "Mouse Action", &format!("Middle clicked at ({}, {})", x, y))?;
            Ok(())
        }
        Err(e) => {
            let err_msg = format!("Failed to call middle_click at ({}, {}): {}", x, y, e);
            println!("[DEV_TOOL] Error: {}", err_msg);
            Err(err_msg)
        }
    }
}

#[tauri::command]
pub(crate) async fn dev_double_click(
    app: AppHandle,
    state: State<'_, AppState>,
    x: f64,
    y: f64,
) -> Result<(), String> {
    println!("[DEV_TOOL] Attempting double click at ({}, {})...", x, y);

    #[cfg(target_os = "macos")]
    let result = state.desktop.double_click(x, y);

    #[cfg(not(target_os = "macos"))]
    let result = Err(computer_use_ai_sdk::AutomationError::UnsupportedPlatform);

    match result {
        Ok(_) => {
            println!("[DEV_TOOL] double_click at ({}, {}) succeeded.", x, y);
            send_dev_tool_notification(&app, "Mouse Action", &format!("Double clicked at ({}, {})", x, y))?;
            Ok(())
        }
        Err(e) => {
            let err_msg = format!("Failed to call double_click at ({}, {}): {}", x, y, e);
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
