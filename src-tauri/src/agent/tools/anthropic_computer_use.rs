//! Official Anthropic Computer Use tools for desktop screen interaction.
//! Implements the complete Anthropic Computer Use API specification.

use crate::agent::implementations::tool_provider::LocalToolProvider;
use crate::agent::core::ToolDefinition;
use crate::state::AppState;
use crate::utils::permission_validator::{validate_permission, RequiredPermission};
use crate::utils::coordinates;
use serde_json::{json, Value};
use std::sync::Arc;
use tauri::Manager;
use tracing::info;

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
            let screenshot_x = coord[0].as_f64().ok_or("Invalid x")?;
            let screenshot_y = coord[1].as_f64().ok_or("Invalid y")?;

            // Transform screenshot coordinates to screen coordinates
            let (screen_x, screen_y) = coordinates::transform_to_screen_coordinates(screenshot_x, screenshot_y);

            crate::commands::mouse::mouse_move(app_handle.clone(), app_handle.state(), screen_x, screen_y).await
                .map(|_| json!({"success": true, "action": "mouse_move", "coordinate": [screenshot_x, screenshot_y], "screen_coordinate": [screen_x, screen_y]}))
                .map_err(|e| format!("Mouse move failed: {}", e))
        }
        "left_click" | "right_click" | "middle_click" | "double_click" | "triple_click" => {
            let coord = input["coordinate"].as_array().ok_or("Missing coordinate")?;
            let screenshot_x = coord[0].as_f64().ok_or("Invalid x")?;
            let screenshot_y = coord[1].as_f64().ok_or("Invalid y")?;

            // Transform screenshot coordinates to screen coordinates
            let (screen_x, screen_y) = coordinates::transform_to_screen_coordinates(screenshot_x, screenshot_y);

            let result = match action {
                "left_click" => crate::commands::mouse::left_click(app_handle.clone(), app_handle.state(), screen_x, screen_y, None).await,
                "right_click" => crate::commands::mouse::right_click(app_handle.clone(), app_handle.state(), screen_x, screen_y, None).await,
                "middle_click" => crate::commands::mouse::middle_click(app_handle.clone(), app_handle.state(), screen_x, screen_y, None).await,
                "double_click" => crate::commands::mouse::double_click(app_handle.clone(), app_handle.state(), screen_x, screen_y, None).await,
                "triple_click" => crate::commands::mouse::triple_click(app_handle.clone(), app_handle.state(), screen_x, screen_y, None).await,
                _ => return Err(format!("Unsupported click action: {}", action)),
            };

            result.map(|_| json!({"success": true, "action": action, "coordinate": [screenshot_x, screenshot_y], "screen_coordinate": [screen_x, screen_y]}))
                  .map_err(|e| format!("{} failed: {}", action, e))
        }
        "left_click_drag" => {
            let start = input["start_coordinate"].as_array().ok_or("Missing start_coordinate")?;
            let end = input["coordinate"].as_array().ok_or("Missing coordinate")?;
            let start_screenshot_x = start[0].as_f64().ok_or("Invalid start x")?;
            let start_screenshot_y = start[1].as_f64().ok_or("Invalid start y")?;
            let end_screenshot_x = end[0].as_f64().ok_or("Invalid end x")?;
            let end_screenshot_y = end[1].as_f64().ok_or("Invalid end y")?;

            // Transform screenshot coordinates to screen coordinates
            let (start_screen_x, start_screen_y) = coordinates::transform_to_screen_coordinates(start_screenshot_x, start_screenshot_y);
            let (end_screen_x, end_screen_y) = coordinates::transform_to_screen_coordinates(end_screenshot_x, end_screenshot_y);

            crate::commands::mouse::left_click_drag(app_handle.clone(), app_handle.state(), start_screen_x, start_screen_y, end_screen_x, end_screen_y).await
                .map(|_| json!({"success": true, "action": "left_click_drag", "start_coordinate": [start_screenshot_x, start_screenshot_y], "end_coordinate": [end_screenshot_x, end_screenshot_y], "start_screen_coordinate": [start_screen_x, start_screen_y], "end_screen_coordinate": [end_screen_x, end_screen_y]}))
                .map_err(|e| format!("Drag failed: {}", e))
        }
        "left_mouse_down" | "left_mouse_up" => {
            let coord = input["coordinate"].as_array().ok_or("Missing coordinate")?;
            let screenshot_x = coord[0].as_f64().ok_or("Invalid x")?;
            let screenshot_y = coord[1].as_f64().ok_or("Invalid y")?;

            // Transform screenshot coordinates to screen coordinates
            let (screen_x, screen_y) = coordinates::transform_to_screen_coordinates(screenshot_x, screenshot_y);

            let result = match action {
                "left_mouse_down" => crate::commands::mouse::left_mouse_down(app_handle.clone(), app_handle.state(), screen_x, screen_y).await,
                "left_mouse_up" => crate::commands::mouse::left_mouse_up(app_handle.clone(), app_handle.state(), screen_x, screen_y).await,
                _ => return Err(format!("Unsupported mouse action: {}", action)),
            };

            result.map(|_| json!({"success": true, "action": action, "coordinate": [screenshot_x, screenshot_y], "screen_coordinate": [screen_x, screen_y]}))
                  .map_err(|e| format!("{} failed: {}", action, e))
        }
        "scroll" => {
            let coord = input["coordinate"].as_array().ok_or("Missing coordinate")?;
            let direction = input["scroll_direction"].as_str().ok_or("Missing scroll_direction")?;
            let amount = input["scroll_amount"].as_i64().unwrap_or(3) as f64;
            let screenshot_x = coord[0].as_f64().ok_or("Invalid x")?;
            let screenshot_y = coord[1].as_f64().ok_or("Invalid y")?;

            // Transform screenshot coordinates to screen coordinates
            let (screen_x, screen_y) = coordinates::transform_to_screen_coordinates(screenshot_x, screenshot_y);

            // Use the correct function signature: scroll_window(direction, amount, x, y, app_handle, state)
            crate::commands::window::scroll_window(direction.to_string(), amount, Some(screen_x), Some(screen_y), app_handle.clone(), app_handle.state()).await
                .map(|_| json!({"success": true, "action": "scroll", "coordinate": [screenshot_x, screenshot_y], "screen_coordinate": [screen_x, screen_y], "direction": direction, "amount": amount}))
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

/// Execute bash command tool
pub async fn execute_bash_tool(
    app_handle: &tauri::AppHandle,
    tool_call: &crate::agent::core::ToolCall,
) -> Result<Value, String> {
    let input = &tool_call.input;
    let command = input["command"].as_str()
        .ok_or_else(|| "Missing 'command' parameter".to_string())?;

    let timeout = input["timeout"].as_u64();
    let working_dir = input["working_dir"].as_str().map(|s| s.to_string());

    // Execute bash command using existing implementation
    crate::commands::shell::bash_command(
        app_handle.clone(),
        app_handle.state(),
        command.to_string(),
        timeout,
        None, // restart
        Some(false), // debug_mode
    )
    .await
    .map(|output| json!({"success": true, "output": output}))
    .map_err(|e| format!("Bash command failed: {}", e))
}

/// Execute str_replace_based_edit_tool
pub async fn execute_str_replace_tool(
    app_handle: &tauri::AppHandle,
    tool_call: &crate::agent::core::ToolCall,
) -> Result<Value, String> {
    let input = &tool_call.input;
    let command = input["command"].as_str()
        .ok_or_else(|| "Missing 'command' parameter".to_string())?;

    match command {
        "view" => {
            let path = input["path"].as_str()
                .ok_or_else(|| "Missing 'path' parameter for view command".to_string())?;

            crate::commands::text_editor::text_editor_view(path.to_string())
                .await
                .map(|content| json!({"success": true, "content": content}))
                .map_err(|e| format!("View failed: {}", e))
        }
        "create" => {
            let path = input["path"].as_str()
                .ok_or_else(|| "Missing 'path' parameter for create command".to_string())?;
            let file_text = input["file_text"].as_str()
                .ok_or_else(|| "Missing 'file_text' parameter for create command".to_string())?;

            crate::commands::text_editor::text_editor_create(path.to_string(), file_text.to_string(), app_handle.state(), app_handle.clone())
                .await
                .map(|_| json!({"success": true, "message": "File created successfully"}))
                .map_err(|e| format!("Create failed: {}", e))
        }
        "str_replace" => {
            let path = input["path"].as_str()
                .ok_or_else(|| "Missing 'path' parameter for str_replace command".to_string())?;
            let old_str = input["old_str"].as_str()
                .ok_or_else(|| "Missing 'old_str' parameter for str_replace command".to_string())?;
            let new_str = input["new_str"].as_str().unwrap_or("");

            crate::commands::text_editor::text_editor_str_replace(path.to_string(), old_str.to_string(), new_str.to_string(), app_handle.state(), app_handle.clone())
                .await
                .map(|_| json!({"success": true, "message": "String replacement completed"}))
                .map_err(|e| format!("String replace failed: {}", e))
        }
        _ => Err(format!("Unknown str_replace_based_edit_tool command: {}", command)),
    }
}

/// Register all official Anthropic Computer Use tools with the tool provider
pub async fn register_anthropic_computer_use_tools(
    provider: &mut LocalToolProvider,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    // === COMPUTER TOOL ===
    let computer_tool_def = ToolDefinition {
        name: "computer".to_string(),
        description: "Use a computer like a human to interact with applications, take screenshots, click, type, and navigate. This is the official Anthropic Computer Use tool.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["screenshot", "left_click", "right_click", "middle_click", "double_click", "triple_click", "left_click_drag", "mouse_move", "left_mouse_down", "left_mouse_up", "type", "key", "hold_key", "scroll", "wait", "cursor_position"],
                    "description": "The action to perform."
                },
                "coordinate": {
                    "type": "array",
                    "items": {"type": "number"},
                    "description": "Array of [x, y] coordinates for click, mouse actions, and end position of drag actions."
                },
                "start_coordinate": {
                    "type": "array",
                    "items": {"type": "number"},
                    "description": "Array of [x, y] start coordinates for drag actions."
                },
                "text": {
                    "type": "string",
                    "description": "Text to type or key combination to press."
                },
                "scroll_direction": {
                    "type": "string",
                    "enum": ["up", "down", "left", "right"],
                    "description": "Direction to scroll."
                },
                "scroll_amount": {
                    "type": "number",
                    "description": "Amount to scroll (default: 3)."
                },
                "duration": {
                    "type": "number",
                    "description": "Duration in milliseconds for wait or hold_key actions."
                }
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
                // FIX: Call the execution function directly instead of recursively calling execute_computer_tool
                // This eliminates the infinite recursion bug
                execute_computer_tool(&app, &tool_call).await
            }
        }
    };

    provider.register_async_tool(computer_tool_def, computer_tool_exec).await;

    // === BASH TOOL ===
    let bash_tool_def = ToolDefinition {
        name: "bash".to_string(),
        description: "Execute bash commands in the terminal. Use this for system operations, file management, and running scripts.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The bash command to execute."
                },
                "timeout": {
                    "type": "integer",
                    "description": "Optional timeout in seconds (default: 30)."
                },
                "working_dir": {
                    "type": "string",
                    "description": "Optional working directory for the command."
                }
            },
            "required": ["command"]
        }),
    };

    let app_handle_clone = Arc::new(app_handle.clone());
    let bash_tool_exec = {
        let app_handle_clone = app_handle_clone.clone();
        move |input: Value| {
            let app = (*app_handle_clone).clone();
            async move {
                let tool_call = crate::agent::core::ToolCall {
                    id: "bash_tool".to_string(),
                    name: "bash".to_string(),
                    input,
                };
                execute_bash_tool(&app, &tool_call).await
            }
        }
    };

    provider.register_async_tool(bash_tool_def, bash_tool_exec).await;

    // === STR_REPLACE_BASED_EDIT_TOOL ===
    let str_replace_tool_def = ToolDefinition {
        name: "str_replace_based_edit_tool".to_string(),
        description: "A tool for viewing, creating and editing files based on string replacement. Use this for precise text editing operations.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "enum": ["view", "create", "str_replace"],
                    "description": "The command to execute: view (read file), create (create new file), str_replace (replace string in file)."
                },
                "path": {
                    "type": "string",
                    "description": "The file path for the operation."
                },
                "file_text": {
                    "type": "string",
                    "description": "The complete text content for create command."
                },
                "old_str": {
                    "type": "string",
                    "description": "The string to find and replace (for str_replace command)."
                },
                "new_str": {
                    "type": "string",
                    "description": "The replacement string (for str_replace command)."
                }
            },
            "required": ["command", "path"]
        }),
    };

    let app_handle_clone = Arc::new(app_handle.clone());
    let str_replace_tool_exec = {
        let app_handle_clone = app_handle_clone.clone();
        move |input: Value| {
            let app = (*app_handle_clone).clone();
            async move {
                let tool_call = crate::agent::core::ToolCall {
                    id: "str_replace_tool".to_string(),
                    name: "str_replace_based_edit_tool".to_string(),
                    input,
                };
                execute_str_replace_tool(&app, &tool_call).await
            }
        }
    };

    provider.register_async_tool(str_replace_tool_def, str_replace_tool_exec).await;

    info!("Successfully registered complete Anthropic Computer Use tools: computer, bash, str_replace_based_edit_tool");
    Ok(())
}

