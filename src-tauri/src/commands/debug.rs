use computer_use_ai_sdk::platforms::macos::debug_mouse;
use tauri::{AppHandle, State};
use tauri::Manager;

#[tauri::command]
pub async fn debug_displays() -> Result<String, String> {
    debug_mouse::debug_display_info();
    Ok("Display info logged to console - check the logs".to_string())
}

#[tauri::command]
pub async fn debug_cursor() -> Result<String, String> {
    let (x, y) = debug_mouse::debug_cursor_position();
    Ok(format!("Cursor position: ({}, {})", x, y))
}

#[tauri::command]
pub async fn debug_point(x: f64, y: f64) -> Result<String, String> {
    debug_mouse::debug_point_display(x, y);
    Ok(format!("Point ({}, {}) display info logged to console", x, y))
}

#[tauri::command]
pub async fn debug_click(x: f64, y: f64) -> Result<String, String> {
    debug_mouse::debug_click_test(x, y);
    Ok(format!("Click test at ({}, {}) completed - check logs", x, y))
}

pub fn register_debug_commands(app: &AppHandle) {
    app.manage(());
}
