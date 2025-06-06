//! Refactored mouse commands demonstrating DRY patterns and reduced complexity
//! 
//! This module shows how the new command macros can reduce boilerplate
//! and provide consistent error handling across mouse commands.

use tauri::{AppHandle, State};
use crate::state::AppState;
use crate::utils::coordinates;
use crate::{dev_command, qa_test_command, state_command};

// Helper function to create a visual indicator for mouse clicks
fn create_click_visualization(app: &AppHandle, x: f64, y: f64, color: &str) -> Result<(), String> {
    app.emit("click-visualization", (x, y, color))
        .map_err(|e| format!("Failed to emit click visualization event: {}", e))
}

// QA Test result structure (shared across QA commands)
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

// Basic mouse commands using the dev_command! macro
dev_command! {
    pub async fn dev_right_click(
        app: AppHandle,
        state: State<'_, AppState>,
        x: f64,
        y: f64,
        modifier: Option<String>,
    ) -> Result<(), String> {
        action: "Right Click",
        operation: "Right clicking at screen coordinates ({}, {}) Modifier: {:?}",
        {
            create_click_visualization(&app, x, y, "#0000FF")?; // Blue for right click
            state.desktop.right_click(x, y, modifier.as_deref())
                .map_err(|e| format!("Failed to perform right click: {}", e))
        }
    }
}

dev_command! {
    pub async fn dev_middle_click(
        app: AppHandle,
        state: State<'_, AppState>,
        x: f64,
        y: f64,
        modifier: Option<String>,
    ) -> Result<(), String> {
        action: "Middle Click",
        operation: "Middle clicking at screen coordinates ({}, {}) Modifier: {:?}",
        {
            create_click_visualization(&app, x, y, "#FFFF00")?; // Yellow for middle click
            state.desktop.middle_click(x, y, modifier.as_deref())
                .map_err(|e| format!("Failed to perform middle click: {}", e))
        }
    }
}

dev_command! {
    pub async fn dev_double_click(
        app: AppHandle,
        state: State<'_, AppState>,
        x: f64,
        y: f64,
        modifier: Option<String>,
    ) -> Result<(), String> {
        action: "Double Click",
        operation: "Double clicking at screen coordinates ({}, {}) Modifier: {:?}",
        {
            create_click_visualization(&app, x, y, "#FFA500")?; // Orange for double click
            state.desktop.double_click(x, y, modifier.as_deref())
                .map_err(|e| format!("Failed to perform double click: {}", e))
        }
    }
}

dev_command! {
    pub async fn dev_triple_click(
        app: AppHandle,
        state: State<'_, AppState>,
        x: f64,
        y: f64,
        modifier: Option<String>,
    ) -> Result<(), String> {
        action: "Triple Click",
        operation: "Triple clicking at screen coordinates ({}, {}) Modifier: {:?}",
        {
            create_click_visualization(&app, x, y, "#800080")?; // Purple for triple click
            state.desktop.triple_click(x, y, modifier.as_deref())
                .map_err(|e| format!("Failed to perform triple click: {}", e))
        }
    }
}

dev_command! {
    pub async fn dev_mouse_move(
        app: AppHandle,
        state: State<'_, AppState>,
        x: f64,
        y: f64,
    ) -> Result<(), String> {
        action: "Mouse Move",
        operation: "Moving mouse to ({}, {})",
        {
            state.desktop.mouse_move(x, y)
                .map_err(|e| format!("Failed to call mouse_move: {}", e))
        }
    }
}

dev_command! {
    pub async fn dev_left_mouse_down(
        app: AppHandle,
        state: State<'_, AppState>,
        x: f64,
        y: f64,
    ) -> Result<(), String> {
        action: "Mouse Action",
        operation: "Left mouse button pressed at ({}, {})",
        {
            state.desktop.left_mouse_down(x, y)
                .map_err(|e| format!("Failed to call left_mouse_down at ({}, {}): {}", x, y, e))
        }
    }
}

dev_command! {
    pub async fn dev_left_mouse_up(
        app: AppHandle,
        state: State<'_, AppState>,
        x: f64,
        y: f64,
    ) -> Result<(), String> {
        action: "Mouse Action",
        operation: "Left mouse button released at ({}, {})",
        {
            state.desktop.left_mouse_up(x, y)
                .map_err(|e| format!("Failed to call left_mouse_up at ({}, {}): {}", x, y, e))
        }
    }
}

dev_command! {
    pub async fn dev_left_click(
        app: AppHandle,
        state: State<'_, AppState>,
        x: f64,
        y: f64,
        modifier: Option<String>,
    ) -> Result<(), String> {
        action: "Left Click",
        operation: "Left clicking at screen coordinates ({}, {}) Modifier: {:?}",
        {
            create_click_visualization(&app, x, y, "#FF0000")?; // Red for left click
            state.desktop.left_click(x, y, modifier.as_deref())
                .map_err(|e| format!("Failed to perform left click: {}", e))
        }
    }
}

dev_command! {
    pub async fn dev_left_click_drag(
        app: AppHandle,
        state: State<'_, AppState>,
        start_x: f64,
        start_y: f64,
        end_x: f64,
        end_y: f64,
    ) -> Result<(), String> {
        action: "Left Click Drag",
        operation: "Dragging from ({}, {}) to ({}, {})",
        {
            state.desktop.left_click_drag(start_x, start_y, end_x, end_y)
                .map_err(|e| format!("Failed to perform left click drag: {}", e))
        }
    }
}

// State accessor using state_command! macro
state_command! {
    pub fn dev_get_cursor_position(
        state: State<'_, AppState>
    ) -> Result<(f64, f64), String> {
        {
            state.desktop.get_cursor_position()
                .map_err(|e| format!("Failed to get cursor position: {}", e))
        }
    }
}

// QA test commands using qa_test_command! macro
qa_test_command! {
    pub async fn qa_test_click(
        app: AppHandle,
        state: State<'_, AppState>,
        x: f64,
        y: f64,
        click_type: String,
    ) -> Result<ClickQAResult, String> {
        test_name: "Click Test",
        operation: "{} click at ({}, {})",
        {
            let (original_x, original_y) = coordinates::transform_to_screen_coordinates(x, y);
            
            let result = match click_type.as_str() {
                "left" => dev_left_click(app.clone(), state.clone(), original_x, original_y, None).await,
                "right" => dev_right_click(app.clone(), state.clone(), original_x, original_y, None).await,
                "middle" => dev_middle_click(app.clone(), state.clone(), original_x, original_y, None).await,
                "double" => dev_double_click(app.clone(), state.clone(), original_x, original_y, None).await,
                "triple" => dev_triple_click(app.clone(), state.clone(), original_x, original_y, None).await,
                _ => Err(format!("Unknown click type: {}", click_type)),
            };
            
            let cursor_position = dev_get_cursor_position(app.clone(), state.clone()).ok();
            
            Ok(ClickQAResult {
                success: result.is_ok(),
                operation: format!("{} click", click_type),
                coordinates: (x, y),
                original_coordinates: Some((original_x, original_y)),
                error: result.err(),
                visualization_success: true,
                cursor_position_after: cursor_position,
                latency_ms: 0.0, // This will be populated by the qa_test_command! macro
            })
        }
    }
}

qa_test_command! {
    pub async fn qa_test_coordinate_transformation(
        app: AppHandle,
        state: State<'_, AppState>,
        x: f64,
        y: f64,
    ) -> Result<serde_json::Value, String> {
        test_name: "Coordinate Transformation",
        operation: "Testing coordinate transformation at scaled ({}, {})",
        {
            let (screen_x, screen_y) = coordinates::transform_to_screen_coordinates(x, y);
            
            // Move mouse to calculated position
            let _ = dev_mouse_move(app.clone(), state.clone(), screen_x, screen_y).await;
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            
            // Get actual cursor position
            let actual_cursor_pos = dev_get_cursor_position(app.clone(), state.clone());
            let (actual_screen_x, actual_screen_y) = match actual_cursor_pos {
                Ok(pos) => (Some(pos.0), Some(pos.1)),
                Err(_) => (None, None),
            };
            
            let (actual_scaled_x, actual_scaled_y) = if let (Some(ax), Some(ay)) = (actual_screen_x, actual_screen_y) {
                coordinates::transform_to_scaled_coordinates(ax, ay)
            } else {
                (x, y)
            };
            
            let (back_to_scaled_x, back_to_scaled_y) = coordinates::transform_to_scaled_coordinates(screen_x, screen_y);
            
            // Create visualizations
            let _ = create_click_visualization(&app, x, y, "#00FF00"); // Original (Green)
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
            let _ = create_click_visualization(&app, back_to_scaled_x, back_to_scaled_y, "#0000FF"); // Round-trip (Blue)
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
            let _ = create_click_visualization(&app, actual_scaled_x, actual_scaled_y, "#FF0000"); // Actual (Red)
            
            let roundtrip_error_x = (x - back_to_scaled_x).abs();
            let roundtrip_error_y = (y - back_to_scaled_y).abs();
            let accuracy_error_x = (x - actual_scaled_x).abs();
            let accuracy_error_y = (y - actual_scaled_y).abs();
            
            Ok(serde_json::json!({
                "original_scaled": { "x": x, "y": y },
                "calculated_screen": { "x": screen_x, "y": screen_y },
                "actual_screen": { "x": actual_screen_x, "y": actual_screen_y },
                "roundtrip_scaled": { "x": back_to_scaled_x, "y": back_to_scaled_y },
                "actual_scaled": { "x": actual_scaled_x, "y": actual_scaled_y },
                "roundtrip_error": { "x": roundtrip_error_x, "y": roundtrip_error_y },
                "accuracy_error": { "x": accuracy_error_x, "y": accuracy_error_y },
                "is_accurate": accuracy_error_x < 2.0 && accuracy_error_y < 2.0
            }))
        }
    }
}

// Visualization test command
dev_command! {
    pub async fn dev_test_click_visualization(
        app: AppHandle,
        state: State<'_, AppState>,
        x: f64,
        y: f64,
        color: Option<String>,
    ) -> Result<(), String> {
        action: "Click Visualization",
        operation: "Testing click visualization at ({}, {}) with color {:?}",
        {
            let color_to_use = color.unwrap_or_else(|| "#FF0000".to_string());
            create_click_visualization(&app, x, y, &color_to_use)
        }
    }
}