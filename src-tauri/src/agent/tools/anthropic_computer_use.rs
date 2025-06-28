//! Official Anthropic Computer Use tools for desktop screen interaction.
//! Implements the complete Anthropic Computer Use API specification.

use crate::agent::implementations::tool_provider::LocalToolProvider;
use crate::agent::core::ToolDefinition;
use crate::state::AppState;
use crate::utils::permission_validator::{validate_permission, RequiredPermission};
use crate::utils::coordinates;
// Import mouse commands to restore proper functionality
use crate::commands::mouse::{
    left_click, right_click, middle_click, double_click, triple_click,
    left_click_drag, mouse_move as mouse_move_command,
    left_mouse_down, left_mouse_up
};
use serde_json::{json, Value};
use tauri::Manager;
use tracing::{info, warn, error};
use std::fs;
use std::path::{Path, PathBuf};

// --- Security and Validation Helpers ---

/// Security configuration for text editor operations
struct SecurityConfig {
    max_file_size: usize,
    allowed_extensions: Vec<&'static str>,
    allow_absolute_paths: bool,
}

impl SecurityConfig {
    fn default() -> Self {
        Self {
            max_file_size: 10 * 1024 * 1024, // 10MB in production
            allowed_extensions: vec![
                "txt", "md", "rs", "js", "ts", "py", "java", "c", "cpp", "h", "hpp",
                "css", "html", "xml", "json", "yaml", "yml", "toml", "cfg", "ini",
                "sh", "bat", "ps1", "sql", "go", "rb", "php", "swift", "kt", "scala"
            ],
            allow_absolute_paths: false,
        }
    }

    fn development_mode() -> Self {
        Self {
            max_file_size: 50 * 1024 * 1024, // 50MB in development
            allowed_extensions: vec![
                "txt", "md", "rs", "js", "ts", "py", "java", "c", "cpp", "h", "hpp",
                "css", "html", "xml", "json", "yaml", "yml", "toml", "cfg", "ini",
                "sh", "bat", "ps1", "sql", "go", "rb", "php", "swift", "kt", "scala",
                "log", "out", "err", "tmp"
            ],
            allow_absolute_paths: true,
        }
    }
}

/// Validates file path for security concerns
fn validate_file_path(path: &str, config: &SecurityConfig) -> Result<PathBuf, String> {
    // Check for path traversal attempts
    if path.contains("../") || path.contains("..\\") {
        return Err("Path traversal not allowed".to_string());
    }

    // Check for home directory access (unless allowed)
    if path.starts_with("~/") && !config.allow_absolute_paths {
        return Err("Home directory access not allowed".to_string());
    }

    let path_buf = PathBuf::from(path);

    // Validate file extension if it's a file
    if let Some(extension) = path_buf.extension() {
        let ext_str = extension.to_string_lossy().to_lowercase();
        if !config.allowed_extensions.contains(&ext_str.as_str()) {
            return Err(format!("File extension '{}' not allowed", ext_str));
        }
    }

    Ok(path_buf)
}

/// Validates file size against security limits
fn validate_file_size(path: &Path, config: &SecurityConfig) -> Result<(), String> {
    match fs::metadata(path) {
        Ok(metadata) => {
            let size = metadata.len() as usize;
            if size > config.max_file_size {
                return Err(format!("File size {} bytes exceeds limit of {} bytes",
                    size, config.max_file_size));
            }
            Ok(())
        }
        Err(_) => Ok(()), // File doesn't exist yet, that's fine
    }
}

/// Adds line numbers to file content for display
fn add_line_numbers(content: &str) -> String {
    content
        .lines()
        .enumerate()
        .map(|(i, line)| format!("{}: {}", i + 1, line))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Extracts specific line range from content
fn extract_line_range(content: &str, start_line: usize, end_line: Option<usize>) -> Result<String, String> {
    let lines: Vec<&str> = content.lines().collect();
    let total_lines = lines.len();

    if start_line == 0 {
        return Err("Line numbers are 1-indexed, start_line cannot be 0".to_string());
    }

    let start_idx = start_line - 1; // Convert to 0-indexed
    if start_idx >= total_lines {
        return Err(format!("Start line {} exceeds file length of {} lines", start_line, total_lines));
    }

    let end_idx = match end_line {
        Some(end) if end == 0 => return Err("Line numbers are 1-indexed, end_line cannot be 0".to_string()),
        Some(end) => {
            let end_idx = end;
            if end_idx > total_lines {
                return Err(format!("End line {} exceeds file length of {} lines", end_idx, total_lines));
            }
            end_idx
        }
        None => total_lines, // None means end of file
    };

    if start_idx >= end_idx {
        return Err("Start line must be less than end line".to_string());
    }

    let selected_lines = &lines[start_idx..end_idx];
    let numbered_content = selected_lines
        .iter()
        .enumerate()
        .map(|(i, line)| format!("{}: {}", start_idx + i + 1, line))
        .collect::<Vec<_>>()
        .join("\n");

    Ok(numbered_content)
}

/// Preserves original line ending style when writing files
fn detect_line_ending(content: &str) -> &'static str {
    if content.contains("\r\n") {
        "\r\n"
    } else if content.contains('\n') {
        "\n"
    } else {
        "\n" // Default to LF for new files
    }
}

/// Generate a descriptive tool name based on the computer action
fn get_descriptive_tool_name(action: &str, input: &Value) -> String {
    match action {
        "screenshot" => "computer/screenshot".to_string(),
        "cursor_position" => "computer/get_cursor_position".to_string(),
        "mouse_move" => {
            if let Some(coord) = input["coordinate"].as_array() {
                format!("computer/move_to({}, {})",
                    coord[0].as_f64().unwrap_or(0.0) as i32,
                    coord[1].as_f64().unwrap_or(0.0) as i32)
            } else {
                "computer/mouse_move".to_string()
            }
        },
        "left_click" => {
            if let Some(coord) = input["coordinate"].as_array() {
                format!("computer/click({}, {})",
                    coord[0].as_f64().unwrap_or(0.0) as i32,
                    coord[1].as_f64().unwrap_or(0.0) as i32)
            } else {
                "computer/left_click".to_string()
            }
        },
        "right_click" => {
            if let Some(coord) = input["coordinate"].as_array() {
                format!("computer/right_click({}, {})",
                    coord[0].as_f64().unwrap_or(0.0) as i32,
                    coord[1].as_f64().unwrap_or(0.0) as i32)
            } else {
                "computer/right_click".to_string()
            }
        },
        "middle_click" => {
            if let Some(coord) = input["coordinate"].as_array() {
                format!("computer/middle_click({}, {})",
                    coord[0].as_f64().unwrap_or(0.0) as i32,
                    coord[1].as_f64().unwrap_or(0.0) as i32)
            } else {
                "computer/middle_click".to_string()
            }
        },
        "double_click" => {
            if let Some(coord) = input["coordinate"].as_array() {
                format!("computer/double_click({}, {})",
                    coord[0].as_f64().unwrap_or(0.0) as i32,
                    coord[1].as_f64().unwrap_or(0.0) as i32)
            } else {
                "computer/double_click".to_string()
            }
        },
        "triple_click" => {
            if let Some(coord) = input["coordinate"].as_array() {
                format!("computer/triple_click({}, {})",
                    coord[0].as_f64().unwrap_or(0.0) as i32,
                    coord[1].as_f64().unwrap_or(0.0) as i32)
            } else {
                "computer/triple_click".to_string()
            }
        },
        "left_click_drag" => {
            if let Some(start) = input["start_coordinate"].as_array() {
                if let Some(end) = input["coordinate"].as_array() {
                    format!("computer/drag({},{} → {},{})",
                        start[0].as_f64().unwrap_or(0.0) as i32,
                        start[1].as_f64().unwrap_or(0.0) as i32,
                        end[0].as_f64().unwrap_or(0.0) as i32,
                        end[1].as_f64().unwrap_or(0.0) as i32)
                } else {
                    "computer/left_click_drag".to_string()
                }
            } else {
                "computer/left_click_drag".to_string()
            }
        },
        "left_mouse_down" => {
            if let Some(coord) = input["coordinate"].as_array() {
                format!("computer/mouse_down({}, {})",
                    coord[0].as_f64().unwrap_or(0.0) as i32,
                    coord[1].as_f64().unwrap_or(0.0) as i32)
            } else {
                "computer/left_mouse_down".to_string()
            }
        },
        "left_mouse_up" => {
            if let Some(coord) = input["coordinate"].as_array() {
                format!("computer/mouse_up({}, {})",
                    coord[0].as_f64().unwrap_or(0.0) as i32,
                    coord[1].as_f64().unwrap_or(0.0) as i32)
            } else {
                "computer/left_mouse_up".to_string()
            }
        },
        "scroll" => {
            let direction = input["scroll_direction"].as_str().unwrap_or("up");
            let amount = input["scroll_amount"].as_i64().unwrap_or(3);
            if let Some(coord) = input["coordinate"].as_array() {
                format!("computer/scroll_{}({},{} × {})",
                    direction,
                    coord[0].as_f64().unwrap_or(0.0) as i32,
                    coord[1].as_f64().unwrap_or(0.0) as i32,
                    amount)
            } else {
                format!("computer/scroll_{} × {}", direction, amount)
            }
        },
        "type" => {
            let text = input["text"].as_str().unwrap_or("");
            if text.len() > 30 {
                format!("computer/type(\"{}...\")", &text[..27])
            } else {
                format!("computer/type(\"{}\")", text)
            }
        },
        "key" => {
            let key = input["text"].as_str().unwrap_or("");
            format!("computer/press_key({})", key)
        },
        "hold_key" => {
            let key = input["text"].as_str().unwrap_or("");
            let duration = input["duration"].as_u64().unwrap_or(1000);
            format!("computer/hold_key({}, {}ms)", key, duration)
        },
        "wait" => {
            let duration = input["duration"].as_u64().unwrap_or(1);
            format!("computer/wait({}s)", duration)
        },
        _ => format!("computer/{}", action),
    }
}

// --- Main computer tool execution function ---

/// Execute computer tool
pub async fn execute_computer_tool(
    app_handle: &tauri::AppHandle,
    input: Value,
) -> Result<Value, String> {
    let action = input["action"].as_str()
        .ok_or_else(|| "Missing 'action' parameter".to_string())?;

    let state_manager = app_handle.state::<AppState>();

    // Generate descriptive tool name for better logging
    let descriptive_tool_name = get_descriptive_tool_name(action, &input);

    // Enhanced logging with descriptive tool name and action details
    info!("🖥️ Computer Use: {} → {}", descriptive_tool_name, action);

    // Log enhanced tool call request with descriptive name
    crate::agent::tool_logger::log_enhanced_tool_call_request(
        app_handle,
        &descriptive_tool_name,
        input.clone(),
        Some(format!("Executing computer action: {}", action)),
        Some(&*state_manager),
    ).await;

    // Execute action
    let execution_start = std::time::Instant::now();
    let result = match action {
        "screenshot" => {
            // Validate screen recording permission
            validate_permission(
                app_handle,
                RequiredPermission::ScreenRecording,
                "computer (screenshot)"
            ).await.map_err(|e| format!("Permission validation failed: {}", e))?;

            let screenshot_path = crate::commands::core::capture_screenshot_command(
                app_handle.clone(),
            ).await.map_err(|e| format!("Screenshot failed: {}", e))?;

            Ok(json!({
                "base64_image": screenshot_path
            }))
        }
        "left_click" | "right_click" | "middle_click" | "double_click" | "triple_click" |
        "left_click_drag" | "mouse_move" | "left_mouse_down" | "left_mouse_up" => {
            // Validate accessibility permission for mouse operations
            validate_permission(
                app_handle,
                RequiredPermission::Accessibility,
                &format!("computer ({})", action)
            ).await.map_err(|e| format!("Permission validation failed: {}", e))?;

            match action {
                "left_click" => {
                    let coordinate = input["coordinate"].as_array()
                        .ok_or_else(|| "Missing 'coordinate' parameter".to_string())?;
                    let x = coordinate.get(0).and_then(|v| v.as_f64())
                        .ok_or_else(|| "Invalid x coordinate".to_string())?;
                    let y = coordinate.get(1).and_then(|v| v.as_f64())
                        .ok_or_else(|| "Invalid y coordinate".to_string())?;

                    // Transform coordinates from scaled screenshot to screen coordinates
                    let (screen_x, screen_y) = coordinates::transform_to_screen_coordinates(x, y);

                    // Use proper mouse command which includes focus, visualization, debug logging, and validation
                    left_click(app_handle.clone(), state_manager, screen_x, screen_y, None).await
                        .map_err(|e| format!("Left click failed: {}", e))?;

                    Ok(json!({
                        "success": true
                    }))
                }
                "right_click" => {
                    let coordinate = input["coordinate"].as_array()
                        .ok_or_else(|| "Missing 'coordinate' parameter".to_string())?;
                    let x = coordinate.get(0).and_then(|v| v.as_f64())
                        .ok_or_else(|| "Invalid x coordinate".to_string())?;
                    let y = coordinate.get(1).and_then(|v| v.as_f64())
                        .ok_or_else(|| "Invalid y coordinate".to_string())?;

                    // Transform coordinates from scaled screenshot to screen coordinates
                    let (screen_x, screen_y) = coordinates::transform_to_screen_coordinates(x, y);

                    // Use proper mouse command which includes focus, visualization, debug logging, and validation
                    right_click(app_handle.clone(), state_manager, screen_x, screen_y, None).await
                        .map_err(|e| format!("Right click failed: {}", e))?;

                    Ok(json!({
                        "success": true
                    }))
                }
                "middle_click" => {
                    let coordinate = input["coordinate"].as_array()
                        .ok_or_else(|| "Missing 'coordinate' parameter".to_string())?;
                    let x = coordinate.get(0).and_then(|v| v.as_f64())
                        .ok_or_else(|| "Invalid x coordinate".to_string())?;
                    let y = coordinate.get(1).and_then(|v| v.as_f64())
                        .ok_or_else(|| "Invalid y coordinate".to_string())?;

                    // Transform coordinates from scaled screenshot to screen coordinates
                    let (screen_x, screen_y) = coordinates::transform_to_screen_coordinates(x, y);

                    // Use proper mouse command which includes focus, visualization, debug logging, and validation
                    middle_click(app_handle.clone(), state_manager, screen_x, screen_y, None).await
                        .map_err(|e| format!("Middle click failed: {}", e))?;

                    Ok(json!({
                        "success": true
                    }))
                }
                "double_click" => {
                    let coordinate = input["coordinate"].as_array()
                        .ok_or_else(|| "Missing 'coordinate' parameter".to_string())?;
                    let x = coordinate.get(0).and_then(|v| v.as_f64())
                        .ok_or_else(|| "Invalid x coordinate".to_string())?;
                    let y = coordinate.get(1).and_then(|v| v.as_f64())
                        .ok_or_else(|| "Invalid y coordinate".to_string())?;

                    // Transform coordinates from scaled screenshot to screen coordinates
                    let (screen_x, screen_y) = coordinates::transform_to_screen_coordinates(x, y);

                    // Use proper mouse command which includes focus, visualization, debug logging, and validation
                    double_click(app_handle.clone(), state_manager, screen_x, screen_y, None).await
                        .map_err(|e| format!("Double click failed: {}", e))?;

                    Ok(json!({
                        "success": true
                    }))
                }
                "triple_click" => {
                    let coordinate = input["coordinate"].as_array()
                        .ok_or_else(|| "Missing 'coordinate' parameter".to_string())?;
                    let x = coordinate.get(0).and_then(|v| v.as_f64())
                        .ok_or_else(|| "Invalid x coordinate".to_string())?;
                    let y = coordinate.get(1).and_then(|v| v.as_f64())
                        .ok_or_else(|| "Invalid y coordinate".to_string())?;

                    // Transform coordinates from scaled screenshot to screen coordinates
                    let (screen_x, screen_y) = coordinates::transform_to_screen_coordinates(x, y);

                    // Use proper mouse command which includes focus, visualization, debug logging, and validation
                    triple_click(app_handle.clone(), state_manager, screen_x, screen_y, None).await
                        .map_err(|e| format!("Triple click failed: {}", e))?;

                    Ok(json!({
                        "success": true
                    }))
                }
                "left_click_drag" => {
                    // Support multiple parameter formats for backward compatibility
                    let (start_x, start_y, end_x, end_y) = if let Some(start_coordinate) = input["start_coordinate"].as_array() {
                        // Check if using start_coordinate + end_coordinate format (most common)
                        if let Some(end_coordinate) = input["end_coordinate"].as_array() {
                            // Format: start_coordinate + end_coordinate
                            let start_x = start_coordinate.get(0).and_then(|v| v.as_f64())
                                .ok_or_else(|| "Invalid start x coordinate".to_string())?;
                            let start_y = start_coordinate.get(1).and_then(|v| v.as_f64())
                                .ok_or_else(|| "Invalid start y coordinate".to_string())?;
                            let end_x = end_coordinate.get(0).and_then(|v| v.as_f64())
                                .ok_or_else(|| "Invalid end x coordinate".to_string())?;
                            let end_y = end_coordinate.get(1).and_then(|v| v.as_f64())
                                .ok_or_else(|| "Invalid end y coordinate".to_string())?;

                            (start_x, start_y, end_x, end_y)
                        } else if let Some(coordinate) = input["coordinate"].as_array() {
                            // Legacy format: start_coordinate + coordinate (end)
                            let start_x = start_coordinate.get(0).and_then(|v| v.as_f64())
                                .ok_or_else(|| "Invalid start x coordinate".to_string())?;
                            let start_y = start_coordinate.get(1).and_then(|v| v.as_f64())
                                .ok_or_else(|| "Invalid start y coordinate".to_string())?;
                            let end_x = coordinate.get(0).and_then(|v| v.as_f64())
                                .ok_or_else(|| "Invalid end x coordinate".to_string())?;
                            let end_y = coordinate.get(1).and_then(|v| v.as_f64())
                                .ok_or_else(|| "Invalid end y coordinate".to_string())?;

                            (start_x, start_y, end_x, end_y)
                        } else {
                            return Err("Missing end coordinate parameter for drag operation. Use 'end_coordinate' or 'coordinate' with 'start_coordinate'".to_string());
                        }
                    } else if let Some(coordinate) = input["coordinate"].as_array() {
                        // Format: coordinate (start) + end_coordinate
                        let end_coordinate = input["end_coordinate"].as_array()
                            .ok_or_else(|| "Missing 'end_coordinate' parameter for drag operation".to_string())?;

                        let start_x = coordinate.get(0).and_then(|v| v.as_f64())
                            .ok_or_else(|| "Invalid start x coordinate".to_string())?;
                        let start_y = coordinate.get(1).and_then(|v| v.as_f64())
                            .ok_or_else(|| "Invalid start y coordinate".to_string())?;
                        let end_x = end_coordinate.get(0).and_then(|v| v.as_f64())
                            .ok_or_else(|| "Invalid end x coordinate".to_string())?;
                        let end_y = end_coordinate.get(1).and_then(|v| v.as_f64())
                            .ok_or_else(|| "Invalid end y coordinate".to_string())?;

                        (start_x, start_y, end_x, end_y)
                    } else {
                        return Err("Missing coordinate parameters for drag operation. Use 'start_coordinate' + 'end_coordinate', 'start_coordinate' + 'coordinate', or 'coordinate' + 'end_coordinate'".to_string());
                    };

                    // Transform coordinates from scaled screenshot to screen coordinates
                    let (screen_start_x, screen_start_y) = coordinates::transform_to_screen_coordinates(start_x, start_y);
                    let (screen_end_x, screen_end_y) = coordinates::transform_to_screen_coordinates(end_x, end_y);

                    // Use proper mouse command which includes focus, visualization, debug logging, and validation
                    left_click_drag(app_handle.clone(), state_manager, screen_start_x, screen_start_y, screen_end_x, screen_end_y).await
                        .map_err(|e| format!("Left click drag failed: {}", e))?;

                    Ok(json!({
                        "success": true
                    }))
                }
                "mouse_move" => {
                    let coordinate = input["coordinate"].as_array()
                        .ok_or_else(|| "Missing 'coordinate' parameter".to_string())?;
                    let x = coordinate.get(0).and_then(|v| v.as_f64())
                        .ok_or_else(|| "Invalid x coordinate".to_string())?;
                    let y = coordinate.get(1).and_then(|v| v.as_f64())
                        .ok_or_else(|| "Invalid y coordinate".to_string())?;

                    // Transform coordinates from scaled screenshot to screen coordinates
                    let (screen_x, screen_y) = coordinates::transform_to_screen_coordinates(x, y);

                    // Use proper mouse command which includes debug logging and validation
                    mouse_move_command(app_handle.clone(), state_manager, screen_x, screen_y).await
                        .map_err(|e| format!("Mouse move failed: {}", e))?;

                    Ok(json!({
                        "success": true
                    }))
                }
                "left_mouse_down" => {
                    let coordinate = input["coordinate"].as_array()
                        .ok_or_else(|| "Missing 'coordinate' parameter".to_string())?;
                    let x = coordinate.get(0).and_then(|v| v.as_f64())
                        .ok_or_else(|| "Invalid x coordinate".to_string())?;
                    let y = coordinate.get(1).and_then(|v| v.as_f64())
                        .ok_or_else(|| "Invalid y coordinate".to_string())?;

                    // Transform coordinates from scaled screenshot to screen coordinates
                    let (screen_x, screen_y) = coordinates::transform_to_screen_coordinates(x, y);

                    // Use proper mouse command which includes debug logging and validation
                    left_mouse_down(app_handle.clone(), state_manager, screen_x, screen_y).await
                        .map_err(|e| format!("Left mouse down failed: {}", e))?;

                    Ok(json!({
                        "success": true
                    }))
                }
                "left_mouse_up" => {
                    let coordinate = input["coordinate"].as_array()
                        .ok_or_else(|| "Missing 'coordinate' parameter".to_string())?;
                    let x = coordinate.get(0).and_then(|v| v.as_f64())
                        .ok_or_else(|| "Invalid x coordinate".to_string())?;
                    let y = coordinate.get(1).and_then(|v| v.as_f64())
                        .ok_or_else(|| "Invalid y coordinate".to_string())?;

                    // Transform coordinates from scaled screenshot to screen coordinates
                    let (screen_x, screen_y) = coordinates::transform_to_screen_coordinates(x, y);

                    // Use proper mouse command to ensure main window focus, click visualization, debug logging, etc.
                    left_mouse_up(app_handle.clone(), state_manager, screen_x, screen_y).await
                        .map_err(|e| format!("Left mouse up failed: {}", e))?;

                    Ok(json!({
                        "success": true
                    }))
                }
                _ => unreachable!("Mouse action already matched in outer pattern")
            }
        }
        "key" | "hold_key" | "type" => {
            // Validate accessibility permission for keyboard operations
            validate_permission(
                app_handle,
                RequiredPermission::Accessibility,
                &format!("computer ({})", action)
            ).await.map_err(|e| format!("Permission validation failed: {}", e))?;

            match action {
                "key" => {
                    // Support both 'key' and 'text' parameters for backward compatibility
                    let key = input["key"].as_str()
                        .or_else(|| input["text"].as_str()) // Backward compatibility
                        .ok_or_else(|| "Missing 'key' or 'text' parameter".to_string())?;

                    crate::commands::keyboard::press_key(
                        key.to_string(),
                        None, // modifier
                        app_handle.clone(),
                        state_manager,
                    ).await.map_err(|e| format!("Key press failed: {}", e))?;

                    Ok(json!({
                        "success": true
                    }))
                }
                "hold_key" => {
                    // Support both 'key' and 'text' parameters for backward compatibility
                    let key = input["key"].as_str()
                        .or_else(|| input["text"].as_str()) // Backward compatibility
                        .ok_or_else(|| "Missing 'key' or 'text' parameter".to_string())?;

                    // Support both 'duration_ms' and 'duration' parameters for backward compatibility
                    let duration_ms = input["duration_ms"].as_u64()
                        .or_else(|| input["duration"].as_u64()) // Backward compatibility
                        .ok_or_else(|| "Missing 'duration_ms' or 'duration' parameter".to_string())?;

                    crate::commands::keyboard::hold_key(
                        key.to_string(),
                        Some(duration_ms),
                        app_handle.clone(),
                        state_manager,
                    ).await.map_err(|e| format!("Hold key failed: {}", e))?;

                    Ok(json!({
                        "success": true
                    }))
                }
                "type" => {
                    let text = input["text"].as_str()
                        .ok_or_else(|| "Missing 'text' parameter".to_string())?;

                    crate::commands::keyboard::type_text(
                        text.to_string(),
                        app_handle.clone(),
                        state_manager,
                    ).await.map_err(|e| format!("Type text failed: {}", e))?;

                    Ok(json!({
                        "success": true
                    }))
                }
                _ => unreachable!("Keyboard action already matched in outer pattern")
            }
        }
        "scroll" => {
            // Validate accessibility permission for scroll operations
            validate_permission(
                app_handle,
                RequiredPermission::Accessibility,
                "computer (scroll)"
            ).await.map_err(|e| format!("Permission validation failed: {}", e))?;

            let coordinate = input["coordinate"].as_array()
                .ok_or_else(|| "Missing 'coordinate' parameter".to_string())?;
            let x = coordinate.get(0).and_then(|v| v.as_f64())
                .ok_or_else(|| "Invalid x coordinate".to_string())?;
            let y = coordinate.get(1).and_then(|v| v.as_f64())
                .ok_or_else(|| "Invalid y coordinate".to_string())?;

            let scroll_direction = input["scroll_direction"].as_str()
                .ok_or_else(|| "Missing 'scroll_direction' parameter".to_string())?;
            let scroll_clicks = input["scroll_clicks"].as_u64().unwrap_or(3);

            // Transform coordinates from scaled screenshot to screen coordinates
            let (screen_x, screen_y) = coordinates::transform_to_screen_coordinates(x, y);

            crate::commands::window::scroll_window(
                scroll_direction.to_string(),
                scroll_clicks as f64,
                Some(screen_x),
                Some(screen_y),
                app_handle.clone(),
                state_manager,
            ).await.map_err(|e| format!("Scroll failed: {}", e))?;

            Ok(json!({
                "success": true
            }))
        }
        "cursor_position" => {
            // No permission validation needed for cursor position query
            let (x, y) = crate::commands::mouse::get_cursor_position(
                app_handle.clone(),
                state_manager,
            ).await.map_err(|e| format!("Get cursor position failed: {}", e))?;

            Ok(json!({
                "coordinate": [x, y]
            }))
        }
        "wait" => {
            // No permission validation needed for wait operation
            // Support both 'seconds' and 'duration' parameters for backward compatibility
            let seconds = input["seconds"].as_f64()
                .or_else(|| input["duration"].as_f64()) // Backward compatibility
                .ok_or_else(|| "Missing 'seconds' or 'duration' parameter".to_string())?;

            crate::commands::core::wait(
                seconds,
                app_handle.clone(),
                state_manager,
            ).await.map_err(|e| format!("Wait failed: {}", e))?;

            Ok(json!({
                "success": true
            }))
        }
        _ => Err(format!("Unknown action: {}", action)),
    };

    // Calculate execution time
    let execution_time_ms = execution_start.elapsed().as_millis() as u64;

    // Get screenshot from result if applicable
    let screenshot_base64 = if action == "screenshot" {
        match &result {
            Ok(output) => output.as_str().map(|s| s.to_string()),
            Err(_) => None,
        }
    } else {
        None
    };

    // Enhanced result logging with descriptive name and execution time
    let success = result.is_ok();
    let result_content = if success {
        Some(format!("✅ {} completed successfully in {}ms", descriptive_tool_name, execution_time_ms))
    } else {
        Some(format!("❌ {} failed", descriptive_tool_name))
    };

    crate::agent::tool_logger::log_enhanced_tool_call_result_with_inputs(
        app_handle,
        &descriptive_tool_name,
        Some(input.clone()),
        result.as_ref().unwrap_or(&json!({})).clone(),
        success,
        result_content,
        screenshot_base64,
        Some(execution_time_ms),
        Some(&*app_handle.state::<AppState>()),
    ).await;

    result
}

/// Execute bash tool
pub async fn execute_bash_tool(
    app_handle: &tauri::AppHandle,
    input: Value,
) -> Result<Value, String> {
    let command = input["command"].as_str()
        .ok_or_else(|| "Missing 'command' parameter".to_string())?;

    let state_manager = app_handle.state::<AppState>();

    // Use the bash command execution
    let result = crate::commands::shell::bash_command(
        app_handle.clone(),
        state_manager,
        command.to_string(),
        None, // timeout_seconds
        None, // restart
        None, // debug_mode
    ).await.map_err(|e| format!("Bash command failed: {}", e))?;

    // Log the raw result for debugging
    info!("Raw bash_command result: {}", result);

    // The bash_command function returns a JSON string containing stdout, stderr, exit_code, etc.
    // We need to parse this JSON to extract the specific fields we want to return
    let result_json: Value = serde_json::from_str(&result)
        .map_err(|e| {
            // If JSON parsing fails, provide detailed error information
            error!("Failed to parse bash_command result as JSON. Error: {}, Raw result: '{}'", e, result);
            format!("Failed to parse bash command result as JSON: '{}'. Raw result was: '{}'", e, result)
        })?;

    // Extract stdout and exit_code from the parsed JSON with better error handling
    let stdout = result_json.get("stdout")
        .and_then(|v| v.as_str())
        .unwrap_or_else(|| {
            warn!("Missing or invalid 'stdout' field in bash command result: {}", result_json);
            ""
        });

    let exit_code = result_json.get("exit_code")
        .and_then(|v| v.as_i64())
        .unwrap_or_else(|| {
            warn!("Missing or invalid 'exit_code' field in bash command result: {}", result_json);
            -1
        });

    // Also extract stderr for completeness (even though we don't return it in the final result)
    let stderr = result_json.get("stderr")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let success = result_json.get("success")
        .and_then(|v| v.as_bool())
        .unwrap_or(exit_code == 0);

    info!("Parsed bash result - stdout: '{}', stderr: '{}', exit_code: {}, success: {}",
          stdout, stderr, exit_code, success);

    // Return the result in the format expected by the Anthropic Computer Use API
    Ok(json!({
        "output": stdout,
        "exit_code": exit_code
    }))
}

/// Execute str_replace_based_edit_tool
pub async fn execute_str_replace_tool(
    _app_handle: &tauri::AppHandle,
    input: Value,
) -> Result<Value, String> {
    let command = input["command"].as_str()
        .ok_or_else(|| "Missing 'command' parameter".to_string())?;

    let path = input["path"].as_str()
        .ok_or_else(|| "Missing 'path' parameter".to_string())?;

    // Get security config based on debug mode
    let config = if cfg!(debug_assertions) {
        SecurityConfig::development_mode()
    } else {
        SecurityConfig::default()
    };

    match command {
        "view" => {
            // Validate file path
            let file_path = validate_file_path(path, &config)?;
            validate_file_size(&file_path, &config)?;

            // Handle view_range if provided
            if let (Some(start), end) = (
                input["view_range"].as_array().and_then(|arr| arr.get(0)).and_then(|v| v.as_u64()),
                input["view_range"].as_array().and_then(|arr| arr.get(1)).and_then(|v| v.as_u64())
            ) {
                let start_line = start as usize;
                let end_line = end.map(|e| e as usize);

                // Read file content
                let content = fs::read_to_string(&file_path)
                    .map_err(|e| format!("Failed to read file '{}': {}", path, e))?;

                let range_content = extract_line_range(&content, start_line, end_line)?;

                Ok(json!({
                    "content": range_content,
                    "view_range": [start_line, end_line.unwrap_or(content.lines().count())]
                }))
            } else {
                // Read entire file
                let content = fs::read_to_string(&file_path)
                    .map_err(|e| format!("Failed to read file '{}': {}", path, e))?;

                let numbered_content = add_line_numbers(&content);

                Ok(json!({
                    "content": numbered_content
                }))
            }
        }
        "str_replace" => {
            let old_str = input["old_str"].as_str()
                .ok_or_else(|| "Missing 'old_str' parameter".to_string())?;
            let new_str = input["new_str"].as_str()
                .ok_or_else(|| "Missing 'new_str' parameter".to_string())?;

            // Validate file path
            let file_path = validate_file_path(path, &config)?;
            validate_file_size(&file_path, &config)?;

            // Read file content
            let content = fs::read_to_string(&file_path)
                .map_err(|e| format!("Failed to read file '{}': {}", path, e))?;

            // Check if old_str exists in file
            if !content.contains(old_str) {
                return Err(format!("String '{}' not found in file '{}'", old_str, path));
            }

            // Detect original line ending style
            let original_line_ending = detect_line_ending(&content);

            // Normalize the replacement text to match the original file's line ending style
            let normalized_new_str = if original_line_ending == "\r\n" {
                // If original uses CRLF, normalize replacement text to use CRLF
                new_str.replace("\r\n", "\n").replace('\n', "\r\n")
            } else {
                // If original uses LF, normalize replacement text to use LF
                new_str.replace("\r\n", "\n")
            };

            // Perform replacement with normalized replacement text
            let new_content = content.replace(old_str, &normalized_new_str);

            // Write back to file
            fs::write(&file_path, &new_content)
                .map_err(|e| format!("Failed to write file '{}': {}", path, e))?;

            Ok(json!({
                "success": true,
                "message": format!("Successfully replaced text in '{}'", path)
            }))
        }
        "create" => {
            let file_content = input["file_text"].as_str()
                .ok_or_else(|| "Missing 'file_text' parameter".to_string())?;

            // Validate file path
            let file_path = validate_file_path(path, &config)?;

            // Check if file already exists
            if file_path.exists() {
                return Err(format!("File '{}' already exists", path));
            }

            // Create parent directories if they don't exist
            if let Some(parent) = file_path.parent() {
                fs::create_dir_all(parent)
                    .map_err(|e| format!("Failed to create directories for '{}': {}", path, e))?;
            }

            // Write file
            fs::write(&file_path, file_content)
                .map_err(|e| format!("Failed to create file '{}': {}", path, e))?;

            Ok(json!({
                "success": true,
                "message": format!("Successfully created file '{}'", path)
            }))
        }
        _ => Err(format!("Unknown str_replace_based_edit_tool command: {}", command)),
    }
}

/// Register all Anthropic Computer Use tools with the provider
pub async fn register_anthropic_computer_use_tools(
    provider: &mut LocalToolProvider,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    info!("Registering official Anthropic Computer Use tools...");

    // Computer tool - main screen interaction tool
    let computer_tool = ToolDefinition {
        name: "computer".to_string(),
        description: "Use a computer to complete tasks. This tool gives you access to interact with any desktop application using the mouse and keyboard, take screenshots, and perform various system operations.

The computer tool accepts these actions:
- screenshot: Take a screenshot of the current screen
- left_click: Click at coordinates with left mouse button
- right_click: Click at coordinates with right mouse button
- middle_click: Click at coordinates with middle mouse button
- double_click: Double-click at coordinates
- triple_click: Triple-click at coordinates
- left_click_drag: Drag from start coordinates to end coordinates
- mouse_move: Move mouse to coordinates
- left_mouse_down: Press and hold left mouse button at coordinates
- left_mouse_up: Release left mouse button at coordinates
- key: Press a key (supports modifiers like 'cmd+c', 'ctrl+v', etc.)
- hold_key: Hold a key for specified duration in milliseconds
- type: Type text at current cursor position
- scroll: Scroll at coordinates in specified direction
- cursor_position: Get current mouse cursor position
- wait: Wait for specified number of seconds

Coordinates are provided as [x, y] arrays and are automatically transformed from screenshot coordinates to screen coordinates.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "description": "The action to perform",
                    "enum": ["screenshot", "left_click", "right_click", "middle_click", "double_click", "triple_click", "left_click_drag", "mouse_move", "left_mouse_down", "left_mouse_up", "key", "hold_key", "type", "scroll", "cursor_position", "wait"]
                },
                "coordinate": {
                    "type": "array",
                    "description": "The [x, y] coordinate for mouse actions. For drag operations, this can be either start coordinate (with end_coordinate) or end coordinate (with start_coordinate)",
                    "items": {"type": "number"}
                },
                "start_coordinate": {
                    "type": "array",
                    "description": "The start [x, y] coordinate for drag actions (backward compatibility)",
                    "items": {"type": "number"}
                },
                "end_coordinate": {
                    "type": "array",
                    "description": "The end [x, y] coordinate for drag actions",
                    "items": {"type": "number"}
                },
                "key": {
                    "type": "string",
                    "description": "The key to press (supports modifiers like 'cmd+c'). Preferred parameter name."
                },
                "text": {
                    "type": "string",
                    "description": "Text to type, or key to press (backward compatibility for key action)"
                },
                "duration_ms": {
                    "type": "number",
                    "description": "Duration in milliseconds for hold_key action. Preferred parameter name."
                },
                "duration": {
                    "type": "number",
                    "description": "Duration in milliseconds for hold_key action, or seconds for wait action (backward compatibility)"
                },
                "scroll_direction": {
                    "type": "string",
                    "description": "Direction to scroll: 'up', 'down', 'left', 'right'"
                },
                "scroll_clicks": {
                    "type": "number",
                    "description": "Number of scroll clicks (default: 3)"
                },
                "seconds": {
                    "type": "number",
                    "description": "Number of seconds to wait. Preferred parameter name."
                }
            },
            "required": ["action"]
        }),
    };

    // Bash tool - command execution
    let bash_tool = ToolDefinition {
        name: "bash".to_string(),
        description: "Execute bash commands on the system. Use this tool to run shell commands, scripts, and interact with the command line.

The tool accepts a 'command' parameter with the bash command to execute.
Returns the command output and exit code.

Example usage:
- List files: {\"command\": \"ls -la\"}
- Check system info: {\"command\": \"uname -a\"}
- Run scripts: {\"command\": \"./script.sh\"}".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The bash command to execute"
                }
            },
            "required": ["command"]
        }),
    };

    // String replacement based edit tool
    let str_replace_tool = ToolDefinition {
        name: "str_replace_based_edit_tool".to_string(),
        description: "Edit files using string replacement operations. This tool provides safe file editing capabilities with security validation.

Supports these commands:
- view: Read file content with optional line range
- str_replace: Replace exact string matches in files
- create: Create new files with specified content

The tool includes security features:
- Path traversal protection
- File extension validation
- File size limits
- Safe file operations

Example usage:
- View file: {\"command\": \"view\", \"path\": \"file.txt\"}
- View range: {\"command\": \"view\", \"path\": \"file.txt\", \"view_range\": [1, 10]}
- Replace text: {\"command\": \"str_replace\", \"path\": \"file.txt\", \"old_str\": \"old text\", \"new_str\": \"new text\"}
- Create file: {\"command\": \"create\", \"path\": \"new_file.txt\", \"file_text\": \"content\"}".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The operation to perform",
                    "enum": ["view", "str_replace", "create"]
                },
                "path": {
                    "type": "string",
                    "description": "Path to the file"
                },
                "view_range": {
                    "type": "array",
                    "description": "Optional [start_line, end_line] for view command",
                    "items": {"type": "number"}
                },
                "old_str": {
                    "type": "string",
                    "description": "String to replace (for str_replace command)"
                },
                "new_str": {
                    "type": "string",
                    "description": "Replacement string (for str_replace command)"
                },
                "file_text": {
                    "type": "string",
                    "description": "Content for new file (for create command)"
                }
            },
            "required": ["command", "path"]
        }),
    };

    // Register all tools with the provider using the correct method
    provider.register_async_tool(computer_tool, {
        let handle = app_handle.clone();
        move |input: Value| {
            let handle = handle.clone();
            async move {
                execute_computer_tool(&handle, input).await
            }
        }
    }).await;

    provider.register_async_tool(bash_tool, {
        let handle = app_handle.clone();
        move |input: Value| {
            let handle = handle.clone();
            async move {
                execute_bash_tool(&handle, input).await
            }
        }
    }).await;

    provider.register_async_tool(str_replace_tool, {
        let handle = app_handle.clone();
        move |input: Value| {
            let handle = handle.clone();
            async move {
                execute_str_replace_tool(&handle, input).await
            }
        }
    }).await;

    info!("Successfully registered 3 official Anthropic Computer Use tools");
    Ok(())
}

