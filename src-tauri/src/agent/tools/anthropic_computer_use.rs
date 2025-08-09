//! Official Anthropic Computer Use tools for desktop screen interaction.
//! Implements the complete Anthropic Computer Use API specification.

use crate::agent::implementations::tool_provider::LocalToolProvider;
use crate::agent::core::{ToolDefinition, AgentError};
use crate::state::AppState;
use crate::utils::permission_validator::{validate_permission, RequiredPermission};
use crate::utils::coordinates;
// Removed unused import - BashResult is handled differently now
// Keep the tool versioning from errors branch (enhanced functionality)
use super::tool_versioning::{ToolVersionManager, ToolVersionConfig};
// Keep the mouse command imports from main branch (proper command usage)
use crate::commands::mouse::{
    left_click, right_click, middle_click
};
use serde_json::{json, Value};
use tauri::Manager;
use tracing::{info, warn};
use std::fs;
use std::path::{Path, PathBuf};
use crate::utils::coordinate_validation::{
    validate_coordinate_parameter,
    validate_coordinate_pair,
    CoordinateValidationError
};

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
            } else if let Some(end) = input["end_coordinate"].as_array() {
                if let Some(start) = input["coordinate"].as_array() {
                    format!("computer/drag({},{} → {},{})",
                        start[0].as_f64().unwrap_or(0.0) as i32,
                        start[1].as_f64().unwrap_or(0.0) as i32,
                        end[0].as_f64().unwrap_or(0.0) as i32,
                        end[1].as_f64().unwrap_or(0.0) as i32)
                } else {
                    "computer/left_click_drag".to_string()
                }
            } else if let Some(coord) = input["coordinate"].as_array() {
                format!("computer/drag(cursor → {},{})",
                    coord[0].as_f64().unwrap_or(0.0) as i32,
                    coord[1].as_f64().unwrap_or(0.0) as i32)
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

impl From<CoordinateValidationError> for String {
    fn from(error: CoordinateValidationError) -> String {
        error.to_string()
    }
}

/// Helper function to check if a JSON Value contains an Anthropic Computer Use API error
/// Returns true if the value contains { "is_error": true, ... }
pub fn is_anthropic_error_response(value: &Value) -> bool {
    value.get("is_error")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
}

/// Helper function to extract error message from Anthropic error response
/// Returns the error message if this is an error response, None otherwise
pub fn extract_anthropic_error_message(value: &Value) -> Option<String> {
    if is_anthropic_error_response(value) {
        value.get("error")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    } else {
        None
    }
}

/// Convert error messages to Anthropic Computer Use API compliant format
/// According to Anthropic's specification, errors should be returned as successful JSON responses
/// with is_error: true and error: "message" instead of using Rust's Err() pattern
fn create_anthropic_error_response(error_message: String) -> Value {
    json!({
        "is_error": true,
        "error": error_message
    })
}

/// Helper macro to convert Result<T, E> to proper Anthropic format
/// This ensures all tools follow the same error handling pattern
/// Works with any error type that can be converted to String
macro_rules! handle_anthropic_result {
    ($result:expr) => {
        match $result {
            Ok(value) => value,
            Err(error_msg) => return Ok(create_anthropic_error_response(error_msg.to_string())),
        }
    };
}

// --- Main computer tool execution function ---

/// Execute computer tool
pub async fn execute_computer_tool(
    app_handle: &tauri::AppHandle,
    input: Value,
) -> Result<Value, String> {
    let action = match input["action"].as_str() {
        Some(action) => action,
        None => return Ok(create_anthropic_error_response("Missing 'action' parameter".to_string())),
    };

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
            handle_anthropic_result!(validate_permission(
                app_handle,
                RequiredPermission::ScreenRecording,
                "computer (screenshot)"
            ).await.map_err(|e: AgentError| format!("Permission validation failed: {}", e)));

            let screenshot_result = handle_anthropic_result!(crate::commands::core::capture_screenshot_command(
                app_handle.clone(),
                state_manager.clone()
            ).await.map_err(|e| format!("Screenshot failed: {}", e)));

            // The result is a struct with the screenshot data and dimensions.
            // Serialize it to a JSON value to return to the agent.
            match serde_json::to_value(screenshot_result) {
                Ok(value) => Ok::<Value, String>(value),
                Err(e) => Ok::<Value, String>(create_anthropic_error_response(format!("Failed to serialize screenshot data: {}", e)))
            }
        }
        "left_click" | "right_click" | "middle_click" |
        "mouse_move" | "left_mouse_down" | "left_mouse_up" => {
            // Validate accessibility permission for mouse operations
            handle_anthropic_result!(validate_permission(
                app_handle,
                RequiredPermission::Accessibility,
                &format!("computer ({})", action)
            ).await.map_err(|e: AgentError| format!("Permission validation failed: {}", e)));

            match action {
                "left_click" => {
                    // Strict coordinate validation per Anthropic Computer Use API specification
                    let coordinate = handle_anthropic_result!(validate_coordinate_parameter(&input, "coordinate"));
                    let (x, y) = coordinate.to_f64();

                    // Transform coordinates from scaled screenshot to screen coordinates
                    let (screen_x, screen_y) = coordinates::transform_to_screen_coordinates(x, y);

                    // Use proper mouse command which includes focus, visualization, debug logging, and validation
                    handle_anthropic_result!(left_click(app_handle.clone(), state_manager, screen_x, screen_y, None).await
                        .map_err(|e| format!("Left click failed: {}", e)));

                    Ok(json!({
                        "success": true
                    }))
                }
                "right_click" => {
                    // Strict coordinate validation per Anthropic Computer Use API specification
                    let coordinate = handle_anthropic_result!(validate_coordinate_parameter(&input, "coordinate"));
                    let (x, y) = coordinate.to_f64();

                    // Transform coordinates from scaled screenshot to screen coordinates
                    let (screen_x, screen_y) = coordinates::transform_to_screen_coordinates(x, y);

                    // Use proper mouse command which includes focus, visualization, debug logging, and validation
                    handle_anthropic_result!(right_click(app_handle.clone(), state_manager, screen_x, screen_y, None).await
                        .map_err(|e| format!("Right click failed: {}", e)));

                    Ok(json!({
                        "success": true
                    }))
                }
                "middle_click" => {
                    // Strict coordinate validation per Anthropic Computer Use API specification
                    let coordinate = handle_anthropic_result!(validate_coordinate_parameter(&input, "coordinate"));
                    let (x, y) = coordinate.to_f64();

                    // Transform coordinates from scaled screenshot to screen coordinates
                    let (screen_x, screen_y) = coordinates::transform_to_screen_coordinates(x, y);

                    // Use proper mouse command which includes focus, visualization, debug logging, and validation
                    handle_anthropic_result!(middle_click(app_handle.clone(), state_manager, screen_x, screen_y, None).await
                        .map_err(|e| format!("Middle click failed: {}", e)));

                    Ok(json!({
                        "success": true
                    }))
                }
                
                "mouse_move" => {
                    // Strict coordinate validation per Anthropic Computer Use API specification
                    let coordinate = handle_anthropic_result!(validate_coordinate_parameter(&input, "coordinate"));
                    let (x, y) = coordinate.to_f64();

                    // Transform coordinates from scaled screenshot to screen coordinates
                    let (screen_x, screen_y) = coordinates::transform_to_screen_coordinates(x, y);

                    // Use proper mouse command which includes debug logging and validation
                    handle_anthropic_result!(crate::commands::mouse::mouse_move(app_handle.clone(), state_manager, screen_x, screen_y).await
                        .map_err(|e| format!("Mouse move failed: {}", e)));

                    Ok(json!({
                        "success": true
                    }))
                }
                "left_mouse_down" => {
                    // Strict coordinate validation per Anthropic Computer Use API specification
                    let coordinate = handle_anthropic_result!(validate_coordinate_parameter(&input, "coordinate"));
                    let (x, y) = coordinate.to_f64();

                    // Transform coordinates from scaled screenshot to screen coordinates
                    let (screen_x, screen_y) = coordinates::transform_to_screen_coordinates(x, y);

                    // Use proper mouse command which includes debug logging and validation
                    handle_anthropic_result!(crate::commands::mouse::left_mouse_down(app_handle.clone(), state_manager, Some(screen_x), Some(screen_y)).await
                        .map_err(|e| format!("Left mouse down failed: {}", e)));

                    Ok(json!({
                        "success": true
                    }))
                }
                "left_mouse_up" => {
                    // Strict coordinate validation per Anthropic Computer Use API specification
                    let coordinate = handle_anthropic_result!(validate_coordinate_parameter(&input, "coordinate"));
                    let (x, y) = coordinate.to_f64();

                    // Transform coordinates from scaled screenshot to screen coordinates
                    let (screen_x, screen_y) = coordinates::transform_to_screen_coordinates(x, y);

                    // Use proper mouse command to ensure main window focus, click visualization, debug logging, etc.
                    handle_anthropic_result!(crate::commands::mouse::left_mouse_up(app_handle.clone(), state_manager, Some(screen_x), Some(screen_y)).await
                        .map_err(|e| format!("Left mouse up failed: {}", e)));

                    Ok(json!({
                        "success": true
                    }))
                }
                _ => unreachable!("Mouse action already matched in outer pattern")
            }
        }
        "key" | "hold_key" | "type" => {
            // Validate accessibility permission for keyboard operations
            handle_anthropic_result!(validate_permission(
                app_handle,
                RequiredPermission::Accessibility,
                &format!("computer ({})", action)
            ).await.map_err(|e: AgentError| format!("Permission validation failed: {}", e)));

            match action {
                "key" => {
                    // Support both 'key' and 'text' parameters for backward compatibility
                    let key = match input["key"].as_str().or_else(|| input["text"].as_str()) {
                        Some(key) => key,
                        None => return Ok(create_anthropic_error_response("Missing 'key' or 'text' parameter".to_string())),
                    };

                    handle_anthropic_result!(crate::commands::keyboard::press_key(
                        key.to_string(),
                        None, // modifier
                        app_handle.clone(),
                        state_manager,
                    ).await.map_err(|e| format!("Key press failed: {}", e)));

                    Ok(json!({
                        "success": true
                    }))
                }
                "hold_key" => {
                    // Support both 'key' and 'text' parameters for backward compatibility
                    let key = match input["key"].as_str().or_else(|| input["text"].as_str()) {
                        Some(key) => key,
                        None => return Ok(create_anthropic_error_response("Missing 'key' or 'text' parameter".to_string())),
                    };

                    // Support both 'duration_ms' and 'duration' parameters for backward compatibility
                    let duration_ms = match input["duration_ms"].as_u64().or_else(|| input["duration"].as_u64()) {
                        Some(duration) => duration,
                        None => return Ok(create_anthropic_error_response("Missing 'duration_ms' or 'duration' parameter".to_string())),
                    };

                    handle_anthropic_result!(crate::commands::keyboard::hold_key(
                        key.to_string(),
                        Some(duration_ms),
                        app_handle.clone(),
                        state_manager,
                    ).await.map_err(|e| format!("Hold key failed: {}", e)));

                    Ok(json!({
                        "success": true
                    }))
                }
                "type" => {
                    let text = match input["text"].as_str() {
                        Some(text) => text,
                        None => return Ok(create_anthropic_error_response("Missing 'text' parameter".to_string())),
                    };

                    handle_anthropic_result!(crate::commands::keyboard::type_text(
                        text.to_string(),
                        app_handle.clone(),
                        state_manager,
                    ).await.map_err(|e| format!("Type text failed: {}", e)));

                    Ok(json!({
                        "success": true
                    }))
                }
                _ => unreachable!("Keyboard action already matched in outer pattern")
            }
        }
        "scroll" => {
            // Validate accessibility permission for scroll operations
            handle_anthropic_result!(validate_permission(
                app_handle,
                RequiredPermission::Accessibility,
                "computer (scroll)"
            ).await.map_err(|e: AgentError| format!("Permission validation failed: {}", e)));

            // Strict coordinate validation per Anthropic Computer Use API specification
            let coordinate = handle_anthropic_result!(validate_coordinate_parameter(&input, "coordinate"));
            let (x, y) = coordinate.to_f64();

            let scroll_direction = match input["scroll_direction"].as_str() {
                Some(direction) => direction,
                None => return Ok(create_anthropic_error_response("Missing 'scroll_direction' parameter".to_string())),
            };
            let scroll_amount = input["scroll_amount"].as_u64().unwrap_or(3);

            // Transform coordinates from scaled screenshot to screen coordinates
            let (screen_x, screen_y) = coordinates::transform_to_screen_coordinates(x, y);

            handle_anthropic_result!(crate::commands::window::scroll_window(
                scroll_direction.to_string(),
                scroll_amount as f64,
                Some(screen_x),
                Some(screen_y),
                app_handle.clone(),
                state_manager,
            ).await.map_err(|e| format!("Scroll failed: {}", e)));

            Ok(json!({
                "success": true
            }))
        }
        "cursor_position" => {
            // No permission validation needed for cursor position query
            let (x, y) = handle_anthropic_result!(crate::commands::mouse::get_cursor_position(
                app_handle.clone(),
                state_manager,
            ).await.map_err(|e| format!("Get cursor position failed: {}", e)));

            Ok(json!({
                "coordinate": [x, y]
            }))
        }
        "wait" => {
            // No permission validation needed for wait operation
            // Support both 'seconds' and 'duration' parameters for backward compatibility
            let seconds = match input["seconds"].as_f64().or_else(|| input["duration"].as_f64()) {
                Some(seconds) => seconds,
                None => return Ok(create_anthropic_error_response("Missing 'seconds' or 'duration' parameter".to_string())),
            };

            handle_anthropic_result!(crate::commands::core::wait(
                seconds,
                app_handle.clone(),
                state_manager.clone(),
            ).await.map_err(|e| format!("Wait failed: {}", e)));

            Ok(json!({
                "success": true
            }))
        }
        _ => Ok(create_anthropic_error_response(format!("Unknown action: {}", action))),
    };

    // Calculate execution time
    let execution_time_ms = execution_start.elapsed().as_millis() as u64;

    // Determine if execution was successful - handle Anthropic Computer Use API error format
    let success = match &result {
        Ok(output) => !is_anthropic_error_response(output),
        Err(_) => false,
    };

    // Get screenshot from result if applicable AND operation was successful
    let screenshot_base64 = if success && action == "screenshot" {
        match &result {
            Ok(output) => output.as_str().map(|s| s.to_string()),
            Err(_) => None,
        }
    } else {
        None
    };

    // Enhanced result logging with descriptive name and execution time
    let result_content = if success {
        Some(format!("✅ {} completed successfully in {}ms", descriptive_tool_name, execution_time_ms))
    } else {
        let error_msg = match &result {
            Ok(output) => extract_anthropic_error_message(output).unwrap_or_else(|| "Unknown error".to_string()),
            Err(e) => e.clone(),
        };
        Some(format!("❌ {} failed: {}", descriptive_tool_name, error_msg))
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

/// Execute bash tool - Anthropic Computer Use API compliant
pub async fn execute_bash_tool(
    app_handle: &tauri::AppHandle,
    input: Value,
) -> Result<Value, String> {
    let command = match input["command"].as_str() {
        Some(command) => command,
        None => return Ok(create_anthropic_error_response("Missing 'command' parameter".to_string())),
    };

    // Handle restart parameter if provided (Anthropic Computer Use API requirement)
    let restart = input["restart"].as_bool().unwrap_or(false);

    let state_manager = app_handle.state::<AppState>();

    // Use the Anthropic-compliant bash command execution - NO STRING COMPARISONS
    let result = handle_anthropic_result!(crate::commands::shell::bash_command(
        app_handle.clone(),
        state_manager,
        command.to_string(),
        None, // timeout_seconds
        Some(restart), // restart parameter
        None, // debug_mode
    ).await.map_err(|e| format!("Bash command failed: {}", e)));

    // Log the result for debugging
    info!("Anthropic compliant bash result: {:?}", result);

    // Handle structured result - NO STRING COMPARISONS NEEDED
    match result {
        crate::commands::shell::BashResult::Restarted => {
            // Tool was restarted - return official Anthropic message
            Ok(json!({
                "output": "tool has been restarted."
            }))
        }
        crate::commands::shell::BashResult::Output(output) => {
            // Regular output
            Ok(json!({
                "output": output
            }))
        }
        crate::commands::shell::BashResult::CommandResult { output, success } => {
            // Command execution result with exit code information
            let exit_code = if success { 0 } else { 1 };
            Ok(json!({
                "output": output,
                "exit_code": exit_code
            }))
        }
    }
}

/// Execute str_replace_based_edit_tool
pub async fn execute_str_replace_tool(
    _app_handle: &tauri::AppHandle,
    input: Value,
) -> Result<Value, String> {
    let command = match input["command"].as_str() {
        Some(command) => command,
        None => return Ok(create_anthropic_error_response("Missing 'command' parameter".to_string())),
    };

    let path = match input["path"].as_str() {
        Some(path) => path,
        None => return Ok(create_anthropic_error_response("Missing 'path' parameter".to_string())),
    };

    // Get security config based on debug mode
    let config = if cfg!(debug_assertions) {
        SecurityConfig::development_mode()
    } else {
        SecurityConfig::default()
    };

    match command {
        "view" => {
            // Validate file path
            let file_path = handle_anthropic_result!(validate_file_path(path, &config));
            handle_anthropic_result!(validate_file_size(&file_path, &config));

            // Handle view_range if provided
            if let (Some(start), end) = (
                input["view_range"].as_array().and_then(|arr| arr.get(0)).and_then(|v| v.as_u64()),
                input["view_range"].as_array().and_then(|arr| arr.get(1)).and_then(|v| v.as_u64())
            ) {
                let start_line = start as usize;
                let end_line = end.map(|e| e as usize);

                // Read file content
                let content = handle_anthropic_result!(fs::read_to_string(&file_path)
                    .map_err(|e| format!("Failed to read file '{}': {}", path, e)));

                let range_content = handle_anthropic_result!(extract_line_range(&content, start_line, end_line));

                Ok(json!({
                    "content": range_content,
                    "view_range": [start_line, end_line.unwrap_or(content.lines().count())]
                }))
            } else {
                // Read entire file
                let content = handle_anthropic_result!(fs::read_to_string(&file_path)
                    .map_err(|e| format!("Failed to read file '{}': {}", path, e)));

                let numbered_content = add_line_numbers(&content);

                Ok(json!({
                    "content": numbered_content
                }))
            }
        }
        "str_replace" => {
            let old_str = match input["old_str"].as_str() {
                Some(old_str) => old_str,
                None => return Ok(create_anthropic_error_response("Missing 'old_str' parameter".to_string())),
            };
            let new_str = match input["new_str"].as_str() {
                Some(new_str) => new_str,
                None => return Ok(create_anthropic_error_response("Missing 'new_str' parameter".to_string())),
            };

            // Validate file path
            let file_path = handle_anthropic_result!(validate_file_path(path, &config));
            handle_anthropic_result!(validate_file_size(&file_path, &config));

            // Read file content
            let content = handle_anthropic_result!(fs::read_to_string(&file_path)
                .map_err(|e| format!("Failed to read file '{}': {}", path, e)));

            // Check if old_str exists in file
            if !content.contains(old_str) {
                return Ok(create_anthropic_error_response(format!("String '{}' not found in file '{}'", old_str, path)));
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
            handle_anthropic_result!(fs::write(&file_path, &new_content)
                .map_err(|e| format!("Failed to write file '{}': {}", path, e)));

            Ok(json!({
                "success": true,
                "message": format!("Successfully replaced text in '{}'", path)
            }))
        }
        "create" => {
            let file_content = match input["file_text"].as_str() {
                Some(file_content) => file_content,
                None => return Ok(create_anthropic_error_response("Missing 'file_text' parameter".to_string())),
            };

            // Validate file path
            let file_path = handle_anthropic_result!(validate_file_path(path, &config));

            // Check if file already exists
            if file_path.exists() {
                return Ok(create_anthropic_error_response(format!("File '{}' already exists", path)));
            }

            // Create parent directories if they don't exist
            if let Some(parent) = file_path.parent() {
                handle_anthropic_result!(fs::create_dir_all(parent)
                    .map_err(|e| format!("Failed to create directories for '{}': {}", path, e)));
            }

            // Write file
            handle_anthropic_result!(fs::write(&file_path, file_content)
                .map_err(|e| format!("Failed to create file '{}': {}", path, e)));

            Ok(json!({
                "success": true,
                "message": format!("Successfully created file '{}'", path)
            }))
        }
        _ => Ok(create_anthropic_error_response(format!("Unknown str_replace_based_edit_tool command: {}", command))),
    }
}

/// Register all Anthropic Computer Use tools with the provider
/// Create versioned Anthropic Computer Use tools based on API version
///
/// This function creates tools with proper API types and versioning to ensure
/// compliance with the official Anthropic Computer Use specification
pub fn create_versioned_tools(version_config: Option<ToolVersionConfig>) -> Vec<ToolDefinition> {
    let manager = if let Some(config) = version_config {
        ToolVersionManager::with_config(config)
    } else {
        ToolVersionManager::new()
    };

    let mut tools = Vec::new();

    // Computer tool - main screen interaction tool (Official Anthropic Computer Use API)
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
        api_type: None, // Will be set by version manager
        beta_flag: None, // Will be set by version manager
        input_schema: json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "description": "The action to perform",
        "enum": ["screenshot", "left_click", "right_click", "middle_click", "mouse_move", "left_mouse_down", "left_mouse_up", "key", "hold_key", "type", "scroll", "cursor_position", "wait"]
                },
                "coordinate": {
                    "type": "array",
                    "description": "The [x, y] coordinate for mouse actions. For drag operations, this is the end coordinate (drag starts from current cursor position)",
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
                "scroll_amount": {
                    "type": "number",
                    "description": "Amount to scroll (default: 3)"
                },
                "seconds": {
                    "type": "number",
                    "description": "Number of seconds to wait. Preferred parameter name."
                }
            },
            "required": ["action"]
        }),
    };

    // Bash tool - command execution (Official Anthropic Computer Use API)
    let bash_tool = ToolDefinition {
        name: "bash".to_string(),
        description: "Execute bash commands on the system. Use this tool to run shell commands, scripts, and interact with the command line.

The tool accepts a 'command' parameter with the bash command to execute.
Returns the command output and exit code.

Example usage:
- List files: {\"command\": \"ls -la\"}
- Check system info: {\"command\": \"uname -a\"}
- Run scripts: {\"command\": \"./script.sh\"}".to_string(),
        api_type: None, // Will be set by version manager
        beta_flag: None, // Will be set by version manager
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

    // String replacement based edit tool (Official Anthropic Computer Use API)
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
        api_type: None, // Will be set by version manager
        beta_flag: None, // Will be set by version manager
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

    // Apply versioning to all tools
    tools.push(manager.apply_versioning(computer_tool));
    tools.push(manager.apply_versioning(bash_tool));
    tools.push(manager.apply_versioning(str_replace_tool));

    tools
}

pub async fn register_anthropic_computer_use_tools(
    provider: &mut LocalToolProvider,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    register_anthropic_computer_use_tools_with_version(provider, app_handle, None).await
}

/// Register Anthropic Computer Use tools with specific API version
pub async fn register_anthropic_computer_use_tools_with_version(
    provider: &mut LocalToolProvider,
    app_handle: tauri::AppHandle,
    version_config: Option<ToolVersionConfig>,
) -> Result<(), String> {
    let version_info = version_config
        .as_ref()
        .map(|c| format!("{:?}", c.current_version))
        .unwrap_or_else(|| "latest".to_string());

    info!("Registering official Anthropic Computer Use tools (API version: {})...", version_info);

    // Create versioned tools
    let versioned_tools = create_versioned_tools(version_config);
    let tool_count = versioned_tools.len();

    for tool in versioned_tools {
        match tool.name.as_str() {
            "computer" => {
                provider.register_async_tool(tool, {
                    let handle = app_handle.clone();
                    move |input: Value| {
                        let handle = handle.clone();
                        async move {
                            execute_computer_tool(&handle, input).await
                        }
                    }
                }).await;
            }
            "bash" => {
                provider.register_async_tool(tool, {
                    let handle = app_handle.clone();
                    move |input: Value| {
                        let handle = handle.clone();
                        async move {
                            execute_bash_tool(&handle, input).await
                        }
                    }
                }).await;
            }
            "str_replace_based_edit_tool" => {
                provider.register_async_tool(tool, {
                    let handle = app_handle.clone();
                    move |input: Value| {
                        let handle = handle.clone();
                        async move {
                            execute_str_replace_tool(&handle, input).await
                        }
                    }
                }).await;
            }
            _ => {
                warn!("Unknown tool name in versioned tools: {}", tool.name);
            }
        }
    }

    info!("Successfully registered {} official Anthropic Computer Use tools", tool_count);
    Ok(())
}

