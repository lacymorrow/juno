// Core/Miscellaneous commands (screenshots, app list, clipboard, wait)

use crate::state::AppState;
// Removed unused: use tauri::{AppHandle, State};
use tracing::{info};
use super::send_dev_tool_notification; // Use helper from parent module
use crate::agent::providers::factory::{BrainFactory, ProviderInfo};
use crate::utils::coordinates;

#[cfg(target_os = "macos")]
use computer_use_ai_sdk::platforms::macos::utils as macos_utils;
#[cfg(target_os = "macos")]
use tauri::AppHandle; // AppHandle needed for macos capture_screenshot_command
#[cfg(not(target_os = "macos"))]
use tauri::AppHandle as DummyAppHandle; // Alias for non-macos signature consistency
use tauri::State; // State is needed for several commands


#[cfg(target_os = "macos")]
#[tauri::command]
pub(crate) async fn capture_screenshot_command(app: AppHandle) -> Result<String, String> {
    match macos_utils::capture_screenshot_with_context() {
        Ok(screenshot_context) => {
            // Update coordinate transformation system with display context
            coordinates::update_display_context(
                screenshot_context.display_bounds.width as u32,
                screenshot_context.display_bounds.height as u32,
                screenshot_context.display_bounds.width as u32, // No scaling applied by default
                screenshot_context.display_bounds.height as u32,
                1.0, // No scaling factor by default
                screenshot_context.display_bounds.origin_x,
                screenshot_context.display_bounds.origin_y,
                screenshot_context.display_bounds.display_id,
                screenshot_context.is_primary_display,
            );

            info!("Screenshot captured with display context: origin=({}, {}), size={}x{}, display_id={}, primary={}",
                  screenshot_context.display_bounds.origin_x,
                  screenshot_context.display_bounds.origin_y,
                  screenshot_context.display_bounds.width,
                  screenshot_context.display_bounds.height,
                  screenshot_context.display_bounds.display_id,
                  screenshot_context.is_primary_display);

            // Send notification on success
            send_dev_tool_notification(&app, "Screenshot", 
                &format!("Screenshot captured from display {} at ({}, {})", 
                        screenshot_context.display_bounds.display_id,
                        screenshot_context.display_bounds.origin_x,
                        screenshot_context.display_bounds.origin_y))?;
            
            Ok(screenshot_context.base64_image)
        }
        Err(e) => Err(format!("Failed to capture screenshot: {}", e)),
    }
}

#[cfg(not(target_os = "macos"))]
#[tauri::command]
pub(crate) async fn capture_screenshot_command(_app: DummyAppHandle) -> Result<String, String> { // Use alias
    Err("Screenshot capture is only supported on macOS currently.".to_string())
}

#[tauri::command]
pub(crate) async fn list_apps(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    let desktop = state.get_desktop()?;
    match desktop.applications() {
        Ok(apps) => {
            let app_names = apps
                .into_iter()
                .map(|app| {
                    app.attributes()
                        .label
                        .unwrap_or_else(|| "Unknown Label".to_string())
                })
                .collect();
            Ok(app_names)
        }
        Err(e) => Err(format!("Failed to get applications: {}", e)),
    }
}

#[tauri::command]
pub(crate) fn check_server_status(state: State<'_, AppState>) -> bool {
    state.is_desktop_available()
}

#[tauri::command]
pub(crate) async fn dev_wait(duration_sec: f64, state: State<'_, AppState>) -> Result<(), String> {
    let duration_ms = (duration_sec * 1000.0).max(0.0) as u64; // Convert seconds to ms, ensure non-negative
    info!("Executing dev_wait for {} seconds ({} ms)", duration_sec, duration_ms);
    let desktop = state.get_desktop()?;
    desktop.wait(duration_ms)
        .map_err(|e| format!("Error during wait: {}", e))
}

#[tauri::command]
pub(crate) async fn dev_get_clipboard(state: State<'_, AppState>) -> Result<String, String> {
    info!("Executing dev_get_clipboard");
    let desktop = state.get_desktop()?;
    desktop.get_clipboard_content()
        .map_err(|e| format!("Error getting clipboard content: {}", e))
}

#[tauri::command]
pub(crate) async fn dev_set_clipboard(content: String, state: State<'_, AppState>) -> Result<(), String> {
    info!("Executing dev_set_clipboard {}", content);
    let desktop = state.get_desktop()?;
    desktop.set_clipboard_content(&content)
        .map_err(|e| format!("Error setting clipboard content: {}", e))
}

/// Get a list of available AI providers
#[tauri::command]
pub async fn list_ai_providers() -> Result<Vec<ProviderInfo>, String> {
    Ok(BrainFactory::list_providers())
}

/// Set the active AI provider
#[tauri::command]
pub async fn set_ai_provider(provider_id: String) -> Result<(), String> {
    // Set environment variable for the current process
    std::env::set_var("AI_PROVIDER", provider_id.clone());

    // For a real implementation, you would want to persist this setting
    // to a config file or database so it's remembered across app restarts

    tracing::info!("Set AI provider to: {}", provider_id);
    Ok(())
}

#[cfg(target_os = "macos")]
#[tauri::command]
pub(crate) async fn get_display_info(app: AppHandle) -> Result<serde_json::Value, String> {
    match macos_utils::capture_screenshot_with_context() {
        Ok(screenshot_context) => {
            let display_info = serde_json::json!({
                "display_bounds": {
                    "origin_x": screenshot_context.display_bounds.origin_x,
                    "origin_y": screenshot_context.display_bounds.origin_y,
                    "width": screenshot_context.display_bounds.width,
                    "height": screenshot_context.display_bounds.height,
                    "display_id": screenshot_context.display_bounds.display_id
                },
                "cursor_position": {
                    "x": screenshot_context.cursor_position.0,
                    "y": screenshot_context.cursor_position.1
                },
                "is_primary_display": screenshot_context.is_primary_display,
                "current_coordinate_context": coordinates::get_display_context()
            });
            
            send_dev_tool_notification(&app, "Display Info", 
                &format!("Display {}: {}x{} at ({}, {})", 
                        screenshot_context.display_bounds.display_id,
                        screenshot_context.display_bounds.width,
                        screenshot_context.display_bounds.height,
                        screenshot_context.display_bounds.origin_x,
                        screenshot_context.display_bounds.origin_y))?;
            
            Ok(display_info)
        }
        Err(e) => Err(format!("Failed to get display info: {}", e)),
    }
}

#[cfg(not(target_os = "macos"))]
#[tauri::command]
pub(crate) async fn get_display_info(_app: DummyAppHandle) -> Result<serde_json::Value, String> {
    Err("Display info is only supported on macOS currently.".to_string())
}

#[cfg(target_os = "macos")]
#[tauri::command]
pub(crate) async fn test_coordinate_transformation(
    app: AppHandle, 
    screenshot_x: f64, 
    screenshot_y: f64
) -> Result<serde_json::Value, String> {
    let (global_x, global_y) = coordinates::transform_to_screen_coordinates(screenshot_x, screenshot_y);
    let (back_to_screenshot_x, back_to_screenshot_y) = coordinates::transform_to_scaled_coordinates(global_x, global_y);
    
    let context = coordinates::get_display_context();
    
    let result = serde_json::json!({
        "input_screenshot_coords": { "x": screenshot_x, "y": screenshot_y },
        "transformed_global_coords": { "x": global_x, "y": global_y },
        "roundtrip_screenshot_coords": { "x": back_to_screenshot_x, "y": back_to_screenshot_y },
        "transformation_error": {
            "x": (screenshot_x - back_to_screenshot_x).abs(),
            "y": (screenshot_y - back_to_screenshot_y).abs()
        },
        "display_context": context,
        "is_accurate": (screenshot_x - back_to_screenshot_x).abs() < 1.0 && (screenshot_y - back_to_screenshot_y).abs() < 1.0
    });
    
    send_dev_tool_notification(&app, "Coordinate Test", 
        &format!("Screenshot ({}, {}) → Global ({}, {}) | Error: ({:.2}, {:.2})", 
                screenshot_x, screenshot_y, global_x, global_y,
                (screenshot_x - back_to_screenshot_x).abs(),
                (screenshot_y - back_to_screenshot_y).abs()))?;
    
    Ok(result)
}

#[cfg(not(target_os = "macos"))]
#[tauri::command]
pub(crate) async fn test_coordinate_transformation(
    _app: DummyAppHandle, 
    _screenshot_x: f64, 
    _screenshot_y: f64
) -> Result<serde_json::Value, String> {
    Err("Coordinate transformation testing is only supported on macOS currently.".to_string())
}
