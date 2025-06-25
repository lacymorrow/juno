//! Official Anthropic Computer Use tools for desktop screen interaction.
//! Streamlined implementation with minimal token overhead.

use crate::agent::implementations::tool_provider::LocalToolProvider;
use crate::agent::core::ToolDefinition;
use crate::state::AppState;
use crate::utils::permission_validator::{validate_permission, RequiredPermission};
use serde_json::{json, Value};
use std::sync::Arc;
use tauri::Manager;
use tracing::{info, warn};

/// Main computer tool execution function
pub async fn execute_computer_tool(
    app_handle: &tauri::AppHandle,
    tool_call: &crate::agent::core::ToolCall,
) -> Result<Value, String> {
    let state_manager = app_handle.state::<AppState>();
    let input = &tool_call.input;

    let action = input["action"].as_str()
        .ok_or_else(|| "Missing 'action' parameter".to_string())?;

    // Permission validation
    match action {
        "screenshot" => validate_permission(app_handle, RequiredPermission::ScreenRecording, "computer/screenshot").await.map_err(|e| e.to_string())?,
        "key" | "hold_key" | "type" | "left_click" | "right_click" | "middle_click" | "double_click" | "triple_click" | "left_click_drag" | "mouse_move" | "left_mouse_down" | "left_mouse_up" | "scroll" | "cursor_position" => {
            validate_permission(app_handle, RequiredPermission::Accessibility, &format!("computer/{}", action)).await.map_err(|e| e.to_string())?;
        }
        "wait" => {}
        _ => return Err(format!("Unknown action: {}", action)),
    }

    // Execute action
    match action {
        "screenshot" => {
            crate::commands::core::capture_screenshot_command(app_handle.clone()).await
                .map(|base64| json!(base64))
        },
        "cursor_position" => {
            match state_manager.desktop.cursor_position() {
                Ok((x, y)) => Ok(json!([x, y])),
                Err(e) => Err(format!("Cursor position failed: {}", e)),
            }
        }
        "mouse_move" => {
            let coord = input["coordinate"].as_array().ok_or("Missing coordinate")?;
            let x = coord[0].as_f64().ok_or("Invalid x")?;
            let y = coord[1].as_f64().ok_or("Invalid y")?;
            crate::commands::mouse::mouse_move(app_handle.clone(), app_handle.state(), x, y).await
                .map(|_| json!({"success": true, "action": "mouse_move", "coordinate": [x, y]}))
                .map_err(|e| format!("Mouse move failed: {}", e))
        }
        "left_click" | "right_click" | "middle_click" | "double_click" | "triple_click" => {
            let coord = input["coordinate"].as_array().ok_or("Missing coordinate")?;
            let x = coord[0].as_f64().ok_or("Invalid x")?;
            let y = coord[1].as_f64().ok_or("Invalid y")?;

            let result = match action {
                "left_click" => crate::commands::mouse::left_click(app_handle.clone(), app_handle.state(), x, y, None).await,
                "right_click" => crate::commands::mouse::right_click(app_handle.clone(), app_handle.state(), x, y, None).await,
                "middle_click" => crate::commands::mouse::middle_click(app_handle.clone(), app_handle.state(), x, y, None).await,
                "double_click" => crate::commands::mouse::double_click(app_handle.clone(), app_handle.state(), x, y, None).await,
                "triple_click" => crate::commands::mouse::triple_click(app_handle.clone(), app_handle.state(), x, y, None).await,
                _ => unreachable!(),
            };

            result.map(|_| json!({"success": true, "action": action, "coordinate": [x, y]}))
                  .map_err(|e| format!("{} failed: {}", action, e))
        }
        "left_click_drag" => {
            let start = input["start_coordinate"].as_array().ok_or("Missing start_coordinate")?;
            let end = input["coordinate"].as_array().ok_or("Missing coordinate")?;
            let sx = start[0].as_f64().ok_or("Invalid start x")?;
            let sy = start[1].as_f64().ok_or("Invalid start y")?;
            let ex = end[0].as_f64().ok_or("Invalid end x")?;
            let ey = end[1].as_f64().ok_or("Invalid end y")?;

            crate::commands::mouse::left_click_drag(app_handle.clone(), app_handle.state(), sx, sy, ex, ey).await
                .map(|_| json!({"success": true, "action": "left_click_drag", "start_coordinate": [sx, sy], "end_coordinate": [ex, ey]}))
                .map_err(|e| format!("Drag failed: {}", e))
        }
        "left_mouse_down" | "left_mouse_up" => {
            let coord = input["coordinate"].as_array().ok_or("Missing coordinate")?;
            let x = coord[0].as_f64().ok_or("Invalid x")?;
            let y = coord[1].as_f64().ok_or("Invalid y")?;

            let result = match action {
                "left_mouse_down" => crate::commands::mouse::left_mouse_down(app_handle.clone(), app_handle.state(), x, y).await,
                "left_mouse_up" => crate::commands::mouse::left_mouse_up(app_handle.clone(), app_handle.state(), x, y).await,
                _ => unreachable!(),
            };

            result.map(|_| json!({"success": true, "action": action, "coordinate": [x, y]}))
                  .map_err(|e| format!("{} failed: {}", action, e))
        }
        "scroll" => {
            let coord = input["coordinate"].as_array().ok_or("Missing coordinate")?;
            let direction = input["scroll_direction"].as_str().ok_or("Missing scroll_direction")?;
            let amount = input["scroll_amount"].as_i64().unwrap_or(3) as f64;
            let x = coord[0].as_f64().ok_or("Invalid x")?;
            let y = coord[1].as_f64().ok_or("Invalid y")?;

            crate::commands::window::scroll_window(direction.to_string(), amount, Some(x), Some(y), app_handle.clone(), app_handle.state()).await
                .map(|_| json!({"success": true, "action": "scroll", "coordinate": [x, y], "direction": direction, "amount": amount}))
                .map_err(|e| format!("Scroll failed: {}", e))
        }
        "type" => {
            let text = input["text"].as_str().ok_or("Missing text")?;
            crate::commands::keyboard::global_type_text(text.to_string(), app_handle.clone(), app_handle.state()).await
                .map(|_| json!({"success": true, "action": "type", "text": text}))
                .map_err(|e| format!("Type failed: {}", e))
        }
        "key" => {
            let key = input["text"].as_str().ok_or("Missing text for key")?;
            crate::commands::keyboard::press_key(key.to_string(), None, app_handle.clone(), app_handle.state()).await
                .map(|_| json!({"success": true, "action": "key", "key": key}))
                .map_err(|e| format!("Key failed: {}", e))
        }
        "hold_key" => {
            let key = input["text"].as_str().ok_or("Missing text for hold_key")?;
            let duration = input["duration"].as_u64().unwrap_or(1000); // Default to 1000ms
            crate::commands::keyboard::hold_key(key.to_string(), Some(duration), app_handle.clone(), app_handle.state()).await
                .map(|_| json!({"success": true, "action": "hold_key", "key": key, "duration": duration}))
                .map_err(|e| format!("Hold key failed: {}", e))
        }
        "wait" => {
            let duration = input["duration"].as_u64().unwrap_or(1);
            tokio::time::sleep(std::time::Duration::from_secs(duration)).await;
            Ok(json!({"success": true, "action": "wait", "duration": duration}))
        }
        _ => Err(format!("Unknown action: {}", action)),
    }
}

/// Register Anthropic Computer Use tools with streamlined schema
pub async fn register_anthropic_computer_use_tools(
    provider: &mut LocalToolProvider,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    info!("Registering streamlined Anthropic Computer Use tools...");

    let computer_tool_def = ToolDefinition {
        name: "computer".to_string(),
        description: "Use mouse and keyboard to interact with computer, and take screenshots.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["key", "hold_key", "type", "cursor_position", "mouse_move", "left_mouse_down", "left_mouse_up", "left_click", "left_click_drag", "right_click", "middle_click", "double_click", "triple_click", "scroll", "wait", "screenshot"],
                    "description": "The action to perform."
                },
                "coordinate": {"type": "array", "description": "(x, y) coordinates.", "items": {"type": "number"}},
                "start_coordinate": {"type": "array", "description": "(x, y) start for drag.", "items": {"type": "number"}},
                "text": {"type": "string", "description": "Text for type/key actions."},
                "duration": {"type": "integer", "description": "Duration in seconds."},
                "scroll_direction": {"type": "string", "enum": ["up", "down", "left", "right"], "description": "Scroll direction."},
                "scroll_amount": {"type": "integer", "description": "Scroll amount."}
            },
            "required": ["action"]
        }),
    };

    let app_handle_clone = Arc::new(app_handle.clone());
    let computer_tool_exec = {
        let app_handle_clone = app_handle_clone.clone();
        move |input: Value| {
            let app = (*app_handle_clone).clone();
            async move {
                let tool_call = crate::agent::core::ToolCall {
                    id: "computer_tool".to_string(),
                    name: "computer".to_string(),
                    input,
                };
                execute_computer_tool(&app, &tool_call).await
            }
        }
    };

    provider.register_async_tool(computer_tool_def, computer_tool_exec).await;

    info!("Successfully registered streamlined Computer Use tools");
    Ok(())
}

