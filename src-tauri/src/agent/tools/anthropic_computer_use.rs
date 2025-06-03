use crate::agent::structs::ToolDefinition;
use crate::agent::implementations::tool_provider::LocalToolProvider;
use crate::state::AppState;
use serde_json::{json, Value};
use tauri::Manager;
use tracing::info;

// Computer Use Tool for Anthropic Claude
// Based on official specification: https://docs.anthropic.com/en/docs/agents-and-tools/computer-use

/// Register the official Anthropic Computer Use tools with exact API specification
pub async fn register_anthropic_computer_use_tools(
    provider: &mut LocalToolProvider,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    info!("Registering Anthropic Computer Use tools...");

    // Computer Tool (computer_20250124) - Enhanced version for Claude 4 & Sonnet 3.7
    let computer_tool_def = ToolDefinition {
        name: "computer".to_string(),
        description: "Use a mouse and keyboard to interact with a computer, and take screenshots.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": [
                        "key",
                        "hold_key",
                        "type",
                        "cursor_position",
                        "mouse_move",
                        "left_mouse_down",
                        "left_mouse_up",
                        "left_click",
                        "left_click_drag",
                        "right_click",
                        "middle_click",
                        "double_click",
                        "triple_click",
                        "scroll",
                        "wait",
                        "screenshot"
                    ],
                    "description": "The action to perform."
                },
                "coordinate": {
                    "type": "array",
                    "description": "(x, y): The x (pixels from the left edge) and y (pixels from the top edge) coordinates to move the mouse to. Required only by action=mouse_move and action=left_click_drag.",
                    "items": {"type": "number"}
                },
                "start_coordinate": {
                    "type": "array",
                    "description": "(x, y): The x (pixels from the left edge) and y (pixels from the top edge) coordinates to start the drag from. Required only by action=left_click_drag.",
                    "items": {"type": "number"}
                },
                "text": {
                    "type": "string",
                    "description": "Required only by action=type, action=key, and action=hold_key. Can also be used by click or scroll actions to hold down keys while clicking or scrolling."
                },
                "duration": {
                    "type": "integer",
                    "description": "The duration to hold the key down for. Required only by action=hold_key and action=wait."
                },
                "scroll_direction": {
                    "type": "string",
                    "enum": ["up", "down", "left", "right"],
                    "description": "The direction to scroll the screen. Required only by action=scroll."
                },
                "scroll_amount": {
                    "type": "integer",
                    "description": "The number of 'clicks' to scroll. Required only by action=scroll."
                }
            },
            "required": ["action"]
        }),
    };

    let app_handle_clone = app_handle.clone();
    let computer_tool_exec = move |input: Value| {
        let app = app_handle_clone.clone();
        async move {
            let state_manager = app.state::<AppState>();

            let action = input["action"]
                .as_str()
                .ok_or_else(|| "Missing or invalid 'action' parameter".to_string())?;

            match action {
                "screenshot" => {
                    let screenshot_result = tokio::task::block_in_place(|| {
                        let rt = tokio::runtime::Handle::current();
                        rt.block_on(async {
                            crate::capture_screenshot_command(app.clone()).await
                        })
                    });

                    match screenshot_result {
                        Ok(base64_image) => Ok(json!({
                            "type": "image",
                            "data": base64_image,
                            "format": "png"
                        })),
                        Err(e) => Err(format!("Screenshot failed: {}", e))
                    }
                },
                "cursor_position" => {
                    match state_manager.desktop.cursor_position() {
                        Ok((x, y)) => Ok(json!([x, y])),
                        Err(e) => Err(format!("Failed to get cursor position: {}", e))
                    }
                },
                "mouse_move" => {
                    let coordinate = input["coordinate"]
                        .as_array()
                        .ok_or_else(|| "Missing or invalid 'coordinate' parameter".to_string())?;
                    if coordinate.len() != 2 {
                        return Err("coordinate must be an array of [x, y]".to_string());
                    }
                    let x = coordinate[0].as_f64().ok_or("Invalid x coordinate")?;
                    let y = coordinate[1].as_f64().ok_or("Invalid y coordinate")?;

                    state_manager.desktop.mouse_move(x, y)
                        .map_err(|e| format!("Mouse move failed: {}", e))?;
                    Ok(json!({"success": true}))
                },
                "left_mouse_down" => {
                    let coordinate = input["coordinate"]
                        .as_array()
                        .ok_or_else(|| "Missing or invalid 'coordinate' parameter".to_string())?;
                    if coordinate.len() != 2 {
                        return Err("coordinate must be an array of [x, y]".to_string());
                    }
                    let x = coordinate[0].as_f64().ok_or("Invalid x coordinate")?;
                    let y = coordinate[1].as_f64().ok_or("Invalid y coordinate")?;

                    state_manager.desktop.left_mouse_down(x, y)
                        .map_err(|e| format!("Left mouse down failed: {}", e))?;
                    Ok(json!({"success": true}))
                },
                "left_mouse_up" => {
                    let coordinate = input["coordinate"]
                        .as_array()
                        .ok_or_else(|| "Missing or invalid 'coordinate' parameter".to_string())?;
                    if coordinate.len() != 2 {
                        return Err("coordinate must be an array of [x, y]".to_string());
                    }
                    let x = coordinate[0].as_f64().ok_or("Invalid x coordinate")?;
                    let y = coordinate[1].as_f64().ok_or("Invalid y coordinate")?;

                    state_manager.desktop.left_mouse_up(x, y)
                        .map_err(|e| format!("Left mouse up failed: {}", e))?;
                    Ok(json!({"success": true}))
                },
                "left_click" => {
                    let coordinate = input["coordinate"]
                        .as_array()
                        .ok_or_else(|| "Missing or invalid 'coordinate' parameter".to_string())?;
                    if coordinate.len() != 2 {
                        return Err("coordinate must be an array of [x, y]".to_string());
                    }
                    let x = coordinate[0].as_f64().ok_or("Invalid x coordinate")?;
                    let y = coordinate[1].as_f64().ok_or("Invalid y coordinate")?;
                    let modifiers = input["text"].as_str(); // Optional modifier keys

                    state_manager.desktop.left_click(x, y, modifiers)
                        .map_err(|e| format!("Left click failed: {}", e))?;
                    Ok(json!({"success": true}))
                },
                "right_click" => {
                    let coordinate = input["coordinate"]
                        .as_array()
                        .ok_or_else(|| "Missing or invalid 'coordinate' parameter".to_string())?;
                    if coordinate.len() != 2 {
                        return Err("coordinate must be an array of [x, y]".to_string());
                    }
                    let x = coordinate[0].as_f64().ok_or("Invalid x coordinate")?;
                    let y = coordinate[1].as_f64().ok_or("Invalid y coordinate")?;
                    let modifiers = input["text"].as_str(); // Optional modifier keys

                    state_manager.desktop.right_click(x, y, modifiers)
                        .map_err(|e| format!("Right click failed: {}", e))?;
                    Ok(json!({"success": true}))
                },
                "middle_click" => {
                    let coordinate = input["coordinate"]
                        .as_array()
                        .ok_or_else(|| "Missing or invalid 'coordinate' parameter".to_string())?;
                    if coordinate.len() != 2 {
                        return Err("coordinate must be an array of [x, y]".to_string());
                    }
                    let x = coordinate[0].as_f64().ok_or("Invalid x coordinate")?;
                    let y = coordinate[1].as_f64().ok_or("Invalid y coordinate")?;
                    let modifiers = input["text"].as_str(); // Optional modifier keys

                    state_manager.desktop.middle_click(x, y, modifiers)
                        .map_err(|e| format!("Middle click failed: {}", e))?;
                    Ok(json!({"success": true}))
                },
                "double_click" => {
                    let coordinate = input["coordinate"]
                        .as_array()
                        .ok_or_else(|| "Missing or invalid 'coordinate' parameter".to_string())?;
                    if coordinate.len() != 2 {
                        return Err("coordinate must be an array of [x, y]".to_string());
                    }
                    let x = coordinate[0].as_f64().ok_or("Invalid x coordinate")?;
                    let y = coordinate[1].as_f64().ok_or("Invalid y coordinate")?;
                    let modifiers = input["text"].as_str(); // Optional modifier keys

                    state_manager.desktop.double_click(x, y, modifiers)
                        .map_err(|e| format!("Double click failed: {}", e))?;
                    Ok(json!({"success": true}))
                },
                "triple_click" => {
                    let coordinate = input["coordinate"]
                        .as_array()
                        .ok_or_else(|| "Missing or invalid 'coordinate' parameter".to_string())?;
                    if coordinate.len() != 2 {
                        return Err("coordinate must be an array of [x, y]".to_string());
                    }
                    let x = coordinate[0].as_f64().ok_or("Invalid x coordinate")?;
                    let y = coordinate[1].as_f64().ok_or("Invalid y coordinate")?;
                    let modifiers = input["text"].as_str(); // Optional modifier keys

                    state_manager.desktop.triple_click(x, y, modifiers)
                        .map_err(|e| format!("Triple click failed: {}", e))?;
                    Ok(json!({"success": true}))
                },
                "left_click_drag" => {
                    let start_coord = input["start_coordinate"]
                        .as_array()
                        .ok_or_else(|| "Missing or invalid 'start_coordinate' parameter".to_string())?;
                    let end_coord = input["coordinate"]
                        .as_array()
                        .ok_or_else(|| "Missing or invalid 'coordinate' parameter".to_string())?;

                    if start_coord.len() != 2 || end_coord.len() != 2 {
                        return Err("coordinates must be arrays of [x, y]".to_string());
                    }

                    let start_x = start_coord[0].as_f64().ok_or("Invalid start_x coordinate")?;
                    let start_y = start_coord[1].as_f64().ok_or("Invalid start_y coordinate")?;
                    let end_x = end_coord[0].as_f64().ok_or("Invalid end_x coordinate")?;
                    let end_y = end_coord[1].as_f64().ok_or("Invalid end_y coordinate")?;

                    state_manager.desktop.left_click_drag(start_x, start_y, end_x, end_y)
                        .map_err(|e| format!("Left click drag failed: {}", e))?;
                    Ok(json!({"success": true}))
                },
                "scroll" => {
                    let coordinate = input["coordinate"]
                        .as_array()
                        .ok_or_else(|| "Missing or invalid 'coordinate' parameter".to_string())?;
                    if coordinate.len() != 2 {
                        return Err("coordinate must be an array of [x, y]".to_string());
                    }
                    let x = coordinate[0].as_f64().ok_or("Invalid x coordinate")?;
                    let y = coordinate[1].as_f64().ok_or("Invalid y coordinate")?;

                    let direction = input["scroll_direction"]
                        .as_str()
                        .ok_or_else(|| "Missing or invalid 'scroll_direction' parameter".to_string())?;
                    let amount = input["scroll_amount"]
                        .as_i64()
                        .ok_or_else(|| "Missing or invalid 'scroll_amount' parameter".to_string())? as f64;

                    state_manager.desktop.scroll_at_position(x, y, direction, amount)
                        .map_err(|e| format!("Scroll failed: {}", e))?;
                    Ok(json!({"success": true}))
                },
                "type" => {
                    let text = input["text"]
                        .as_str()
                        .ok_or_else(|| "Missing or invalid 'text' parameter".to_string())?;

                    state_manager.desktop.type_text(text)
                        .map_err(|e| format!("Type text failed: {}", e))?;
                    Ok(json!({"success": true}))
                },
                "key" => {
                    let key_combo = input["text"]
                        .as_str()
                        .ok_or_else(|| "Missing or invalid 'text' parameter".to_string())?;

                    state_manager.desktop.press_key(key_combo, None)
                        .map_err(|e| format!("Key press failed: {}", e))?;
                    Ok(json!({"success": true}))
                },
                "hold_key" => {
                    let key = input["text"]
                        .as_str()
                        .ok_or_else(|| "Missing or invalid 'text' parameter".to_string())?;
                    let duration = input["duration"]
                        .as_u64()
                        .ok_or_else(|| "Missing or invalid 'duration' parameter".to_string())?;

                    state_manager.desktop.hold_key(key, Some(duration))
                        .map_err(|e| format!("Hold key failed: {}", e))?;
                    Ok(json!({"success": true}))
                },
                "wait" => {
                    let duration_ms = input["duration"]
                        .as_u64()
                        .ok_or_else(|| "Missing or invalid 'duration' parameter".to_string())? * 1000; // Convert seconds to milliseconds

                    state_manager.desktop.wait(duration_ms)
                        .map_err(|e| format!("Wait failed: {}", e))?;
                    Ok(json!({"success": true}))
                },
                _ => Err(format!("Unknown action: {}", action))
            }
        }
    };

    provider.register_async_tool(computer_tool_def, computer_tool_exec).await;
    info!("Registered tool: computer (Anthropic Computer Use)");

    // Text Editor Tool (text_editor_20250429) - Claude 4 version without undo_edit
    let text_editor_tool_def = ToolDefinition {
        name: "str_replace_based_edit_tool".to_string(),
        description: "Custom editing tool for viewing, creating and editing files".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "enum": ["view", "create", "str_replace", "insert"],
                    "description": "The commands to run. Allowed options are: view, create, str_replace, insert."
                },
                "path": {
                    "type": "string",
                    "description": "Absolute path to file or directory, e.g. /repo/file.py or /repo."
                },
                "file_text": {
                    "type": "string",
                    "description": "Required parameter of create command, with the content of the file to be created."
                },
                "old_str": {
                    "type": "string",
                    "description": "Required parameter of str_replace command containing the string in path to replace."
                },
                "new_str": {
                    "type": "string",
                    "description": "Optional parameter of str_replace command containing the new string (if not given, no string will be added). Required parameter of insert command containing the string to insert."
                },
                "insert_line": {
                    "type": "integer",
                    "description": "Required parameter of insert command. The new_str will be inserted AFTER the line insert_line of path."
                },
                "view_range": {
                    "type": "array",
                    "items": {"type": "integer"},
                    "description": "Optional parameter of view command when path points to a file. If none is given, the full file is shown. If provided, the file will be shown in the indicated line number range, e.g. [11, 12] will show lines 11 and 12. Indexing at 1 to start. Setting [start_line, -1] shows all lines from start_line to the end of the file."
                }
            },
            "required": ["command", "path"]
        }),
    };

    let app_handle_clone = app_handle.clone();
    let text_editor_exec = move |input: Value| {
        let _app = app_handle_clone.clone();
        async move {
            let command = input["command"]
                .as_str()
                .ok_or_else(|| "Missing or invalid 'command' parameter".to_string())?;
            let path = input["path"]
                .as_str()
                .ok_or_else(|| "Missing or invalid 'path' parameter".to_string())?;

            match command {
                "view" => {
                    match std::fs::read_to_string(path) {
                        Ok(content) => {
                            if let Some(view_range) = input["view_range"].as_array() {
                                if view_range.len() == 2 {
                                    let start = view_range[0].as_i64().unwrap_or(1) as usize;
                                    let end = view_range[1].as_i64().unwrap_or(-1);

                                    let lines: Vec<&str> = content.lines().collect();
                                    let end_line = if end == -1 { lines.len() } else { end as usize };

                                    let start_idx = if start > 0 { start - 1 } else { 0 };
                                    let end_idx = std::cmp::min(end_line, lines.len());

                                    if start_idx < lines.len() {
                                        let selected_lines = &lines[start_idx..end_idx];
                                        let result = selected_lines.iter()
                                            .enumerate()
                                            .map(|(i, line)| format!("{:4}: {}", start_idx + i + 1, line))
                                            .collect::<Vec<_>>()
                                            .join("\n");
                                        Ok(json!(result))
                                    } else {
                                        Ok(json!(""))
                                    }
                                } else {
                                    Ok(json!(content))
                                }
                            } else {
                                // Show with line numbers
                                let numbered_content = content.lines()
                                    .enumerate()
                                    .map(|(i, line)| format!("{:4}: {}", i + 1, line))
                                    .collect::<Vec<_>>()
                                    .join("\n");
                                Ok(json!(numbered_content))
                            }
                        },
                        Err(e) => {
                            if std::path::Path::new(path).is_dir() {
                                // List directory contents
                                match std::fs::read_dir(path) {
                                    Ok(entries) => {
                                        let mut items = Vec::new();
                                        for entry in entries {
                                            if let Ok(entry) = entry {
                                                let name = entry.file_name().to_string_lossy().to_string();
                                                let is_dir = entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false);
                                                items.push(if is_dir { format!("{}/", name) } else { name });
                                            }
                                        }
                                        items.sort();
                                        Ok(json!(items.join("\n")))
                                    },
                                    Err(e) => Err(format!("Failed to list directory: {}", e))
                                }
                            } else {
                                Err(format!("Failed to read file: {}", e))
                            }
                        }
                    }
                },
                "create" => {
                    let file_text = input["file_text"]
                        .as_str()
                        .ok_or_else(|| "Missing or invalid 'file_text' parameter".to_string())?;

                    if std::path::Path::new(path).exists() {
                        return Err(format!("File already exists: {}", path));
                    }

                    // Create parent directories if they don't exist
                    if let Some(parent) = std::path::Path::new(path).parent() {
                        std::fs::create_dir_all(parent)
                            .map_err(|e| format!("Failed to create parent directories: {}", e))?;
                    }

                    std::fs::write(path, file_text)
                        .map_err(|e| format!("Failed to create file: {}", e))?;

                    Ok(json!(format!("File created successfully: {}", path)))
                },
                "str_replace" => {
                    let old_str = input["old_str"]
                        .as_str()
                        .ok_or_else(|| "Missing or invalid 'old_str' parameter".to_string())?;
                    let new_str = input["new_str"].as_str().unwrap_or("");

                    let content = std::fs::read_to_string(path)
                        .map_err(|e| format!("Failed to read file: {}", e))?;

                    if content.matches(old_str).count() != 1 {
                        return Err(format!("old_str must match exactly one occurrence in the file. Found {} matches.", content.matches(old_str).count()));
                    }

                    let new_content = content.replace(old_str, new_str);
                    std::fs::write(path, new_content)
                        .map_err(|e| format!("Failed to write file: {}", e))?;

                    Ok(json!(format!("String replacement completed in: {}", path)))
                },
                "insert" => {
                    let new_str = input["new_str"]
                        .as_str()
                        .ok_or_else(|| "Missing or invalid 'new_str' parameter".to_string())?;
                    let insert_line = input["insert_line"]
                        .as_i64()
                        .ok_or_else(|| "Missing or invalid 'insert_line' parameter".to_string())? as usize;

                    let content = std::fs::read_to_string(path)
                        .map_err(|e| format!("Failed to read file: {}", e))?;

                    let lines: Vec<&str> = content.lines().collect();

                    if insert_line > lines.len() {
                        return Err(format!("insert_line {} is beyond file length {}", insert_line, lines.len()));
                    }

                    // Insert after the specified line (1-indexed)
                    let insert_pos = insert_line; // insert_line is 1-indexed, so line 1 means insert at index 1 (after line 0)

                    let mut new_lines = Vec::new();
                    new_lines.extend_from_slice(&lines[..insert_pos]);
                    new_lines.push(new_str);
                    new_lines.extend_from_slice(&lines[insert_pos..]);

                    let new_content = new_lines.join("\n");
                    std::fs::write(path, new_content)
                        .map_err(|e| format!("Failed to write file: {}", e))?;

                    Ok(json!(format!("Text inserted at line {} in: {}", insert_line, path)))
                },
                _ => Err(format!("Unknown command: {}", command))
            }
        }
    };

    provider.register_async_tool(text_editor_tool_def, text_editor_exec).await;
    info!("Registered tool: str_replace_based_edit_tool (Anthropic Text Editor)");

    // Bash Tool (bash_20250124) - Enhanced bash tool
    let bash_tool_def = ToolDefinition {
        name: "bash".to_string(),
        description: "Run commands in a bash shell".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The bash command to run. Required unless the tool is being restarted."
                },
                "restart": {
                    "type": "boolean",
                    "description": "Specifying true will restart this tool. Otherwise, leave this unspecified."
                }
            }
        }),
    };

    let bash_exec = move |input: Value| {
        async move {
            if let Some(restart) = input["restart"].as_bool() {
                if restart {
                    return Ok(json!({"status": "restarted", "message": "Bash environment restarted"}));
                }
            }

            let command = input["command"]
                .as_str()
                .ok_or_else(|| "Missing or invalid 'command' parameter".to_string())?;

            info!("Executing bash command: {}", command);

            let output = std::process::Command::new("bash")
                .arg("-c")
                .arg(command)
                .output()
                .map_err(|e| format!("Failed to execute command: {}", e))?;

            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            let exit_code = output.status.code();

            let result = if stdout.is_empty() && stderr.is_empty() {
                "[No output]".to_string()
            } else if !stdout.is_empty() && stderr.is_empty() {
                stdout
            } else if stdout.is_empty() && !stderr.is_empty() {
                format!("STDERR:\n{}", stderr)
            } else {
                format!("STDOUT:\n{}\nSTDERR:\n{}", stdout, stderr)
            };

            Ok(json!({
                "output": result,
                "exit_code": exit_code,
                "success": output.status.success()
            }))
        }
    };

    provider.register_async_tool(bash_tool_def, bash_exec).await;
    info!("Registered tool: bash (Anthropic Bash Tool)");

    Ok(())
}
