// Computer Use API commands - Official Anthropic Computer Use implementation
// This provides a unified interface for all mouse, keyboard, and screen operations

use crate::state::AppState;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};
use tracing::info;
use crate::commands::core::ScreenshotResult as CoreScreenshotResult;

pub mod unrestricted_computer;

/// Computer action input structure matching the official Anthropic Computer Use API
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

/// Main computer command - implements the official Anthropic Computer Use API
#[tauri::command]
pub async fn computer(
    input: ComputerInput,
    app_handle: AppHandle,
    state: State<'_, AppState>,
) -> Result<ComputerResult, String> {
    info!("Computer command called with action: {}", input.action);

    // Check if unrestricted mode is enabled - bypass all restrictions if so
    if state.is_unrestricted_mode() {
        info!("Unrestricted mode active - bypassing all restrictions for action: {}", input.action);
        // Execute the action without any rate limiting or restrictions
        return execute_computer_action_unrestricted(input, app_handle, state).await;
    }

    // Apply rate limiting based on action type
    match input.action.as_str() {
        "screenshot" => {
            if let Err(e) = state.rate_limiters.screenshots.check("default_user").await {
                return Err(e.to_user_message());
            }
        }
        "type" | "key" | "hold_key" => {
            // Light rate limiting for keyboard operations
            if let Err(e) = state.rate_limiters.file_operations.check("default_user").await {
                return Err(e.to_user_message());
            }
        }
        _ => {
            // Apply general rate limiting for other actions
            if let Err(e) = state.rate_limiters.shell_commands.check("default_user").await {
                return Err(e.to_user_message());
            }
        }
    }

    // Execute the computer action
    execute_computer_action(input, app_handle, state).await
}

/// Execute a computer action - internal implementation
async fn execute_computer_action(
    input: ComputerInput,
    app_handle: AppHandle,
    state: State<'_, AppState>,
) -> Result<ComputerResult, String> {
    use crate::commands::core;
    
    match input.action.as_str() {
        "screenshot" => {
            let result = core::capture_screenshot_command(app_handle, state).await?;
            Ok(ComputerResult {
                success: true,
                action: "screenshot".to_string(),
                message: Some("Screenshot captured successfully".to_string()),
                screenshot: Some(result),
                error: None,
                coordinate: None,
            })
        }
        "left_click" | "right_click" | "middle_click" | "double_click" | "triple_click" => {
            if let Some(coord) = input.coordinate {
                if coord.len() < 2 {
                    return Err("Coordinate must have at least x and y values".to_string());
                }
                let x = coord[0];
                let y = coord[1];
                
                let message = match input.action.as_str() {
                    "left_click" => {
                        state.desktop.left_click(x, y, None)
                            .map_err(|e| format!("Failed to left click: {}", e))?;
                        format!("Left clicked at ({}, {})", x, y)
                    }
                    "right_click" => {
                        state.desktop.right_click(x, y, None)
                            .map_err(|e| format!("Failed to right click: {}", e))?;
                        format!("Right clicked at ({}, {})", x, y)
                    }
                    "middle_click" => {
                        state.desktop.middle_click(x, y, None)
                            .map_err(|e| format!("Failed to middle click: {}", e))?;
                        format!("Middle clicked at ({}, {})", x, y)
                    }
                    "double_click" => {
                        state.desktop.double_click(x, y, None)
                            .map_err(|e| format!("Failed to double click: {}", e))?;
                        format!("Double clicked at ({}, {})", x, y)
                    }
                    "triple_click" => {
                        state.desktop.triple_click(x, y, None)
                            .map_err(|e| format!("Failed to triple click: {}", e))?;
                        format!("Triple clicked at ({}, {})", x, y)
                    }
                    _ => unreachable!(),
                };
                
                Ok(ComputerResult {
                    success: true,
                    action: input.action,
                    message: Some(message),
                    screenshot: None,
                    error: None,
                    coordinate: Some(vec![x, y]),
                })
            } else {
                Err("Coordinate is required for click actions".to_string())
            }
        }
        "type" => {
            if let Some(text) = input.text {
                use crate::commands::keyboard;
                keyboard::type_text(text.clone(), app_handle.clone(), state.clone()).await?;
                Ok(ComputerResult {
                    success: true,
                    action: "type".to_string(),
                    message: Some(format!("Typed text: {}", text)),
                    screenshot: None,
                    error: None,
                    coordinate: None,
                })
            } else {
                Err("Text is required for type action".to_string())
            }
        }
        "key" => {
            if let Some(key) = input.text {
                use crate::commands::keyboard;
                keyboard::press_key(key.clone(), None, app_handle.clone(), state.clone()).await?;
                Ok(ComputerResult {
                    success: true,
                    action: "key".to_string(),
                    message: Some(format!("Pressed key: {}", key)),
                    screenshot: None,
                    error: None,
                    coordinate: None,
                })
            } else {
                Err("Key is required for key action".to_string())
            }
        }
        "mouse_move" => {
            if let Some(coord) = input.coordinate {
                if coord.len() < 2 {
                    return Err("Coordinate must have at least x and y values".to_string());
                }
                let x = coord[0];
                let y = coord[1];
                
                state.desktop.mouse_move(x, y)
                    .map_err(|e| format!("Failed to move mouse: {}", e))?;
                
                Ok(ComputerResult {
                    success: true,
                    action: "mouse_move".to_string(),
                    message: Some(format!("Mouse moved to ({}, {})", x, y)),
                    screenshot: None,
                    error: None,
                    coordinate: Some(vec![x, y]),
                })
            } else {
                Err("Coordinate is required for mouse_move action".to_string())
            }
        }
        "scroll" => {
            let direction = input.scroll_direction.as_deref().unwrap_or("down");
            let count = input.scroll_count.unwrap_or(5);
            
            // Optionally move mouse first
            if let Some(coord) = &input.coordinate {
                if coord.len() >= 2 {
                    let x = coord[0];
                    let y = coord[1];
                    state.desktop.mouse_move(x, y)
                        .map_err(|e| format!("Failed to move mouse: {}", e))?;
                }
            }
            
            // Execute scroll
            use crate::commands::window::scroll_window;
            let (x, y) = if let Some(coord) = &input.coordinate {
                if coord.len() >= 2 {
                    (Some(coord[0]), Some(coord[1]))
                } else {
                    (None, None)
                }
            } else {
                (None, None)
            };
            
            scroll_window(direction.to_string(), count as f64, x, y, app_handle.clone(), state.clone()).await?;
            
            Ok(ComputerResult {
                success: true,
                action: "scroll".to_string(),
                message: Some(format!("Scrolled {} {} times", direction, count)),
                screenshot: None,
                error: None,
                coordinate: input.coordinate,
            })
        }
        "wait" => {
            let duration = input.duration.unwrap_or(1000);
            
            // Limit wait time in normal mode
            let max_wait = 30000; // 30 seconds max
            let actual_duration = duration.min(max_wait);
            
            tokio::time::sleep(tokio::time::Duration::from_millis(actual_duration)).await;
            
            Ok(ComputerResult {
                success: true,
                action: "wait".to_string(),
                message: Some(format!("Waited {} ms", actual_duration)),
                screenshot: None,
                error: None,
                coordinate: None,
            })
        }
        "drag" => {
            if let Some(coord) = input.coordinate {
                if coord.len() < 2 {
                    return Err("Coordinate must have at least x and y values".to_string());
                }
                let end_x = coord[0];
                let end_y = coord[1];
                
                // Get current cursor position for start point
                let (start_x, start_y) = state.desktop.cursor_position()
                    .map_err(|e| format!("Failed to get cursor position: {}", e))?;
                
                // Perform drag
                state.desktop.left_click_drag(start_x, start_y, end_x, end_y)
                    .map_err(|e| format!("Failed to drag: {}", e))?;
                
                Ok(ComputerResult {
                    success: true,
                    action: "drag".to_string(),
                    message: Some(format!("Dragged from ({}, {}) to ({}, {})", start_x, start_y, end_x, end_y)),
                    screenshot: None,
                    error: None,
                    coordinate: Some(vec![end_x, end_y]),
                })
            } else {
                Err("Coordinate is required for drag action".to_string())
            }
        }
        _ => {
            Err(format!("Unsupported action: {}", input.action))
        }
    }
}

/// Execute computer action in unrestricted mode - bypasses all restrictions
async fn execute_computer_action_unrestricted(
    input: ComputerInput,
    app_handle: AppHandle,
    state: State<'_, AppState>,
) -> Result<ComputerResult, String> {
    use crate::commands::core;
    use crate::commands::keyboard;
    
    info!("Executing unrestricted computer action: {}", input.action);
    
    // Same implementation as execute_computer_action but without any checks
    // No rate limiting, no permission checks, no validation
    match input.action.as_str() {
        "screenshot" => {
            // Direct screenshot without any restrictions
            let result = core::capture_screenshot_command(app_handle, state).await?;
            Ok(ComputerResult {
                success: true,
                action: "screenshot".to_string(),
                message: Some("Screenshot captured (unrestricted)".to_string()),
                screenshot: Some(result),
                error: None,
                coordinate: None,
            })
        }
        "left_click" | "right_click" | "middle_click" | "double_click" | "triple_click" => {
            if let Some(coord) = input.coordinate {
                // No coordinate validation in unrestricted mode
                let x = coord[0];
                let y = coord[1];
                
                let message = match input.action.as_str() {
                    "left_click" => {
                        // Direct click without smooth movement or delays
                        state.desktop.left_click(x, y, None)
                            .map_err(|e| format!("Failed to left click: {}", e))?;
                        format!("Left clicked at ({}, {}) (unrestricted)", x, y)
                    }
                    "right_click" => {
                        state.desktop.right_click(x, y, None)
                            .map_err(|e| format!("Failed to right click: {}", e))?;
                        format!("Right clicked at ({}, {}) (unrestricted)", x, y)
                    }
                    "middle_click" => {
                        state.desktop.middle_click(x, y, None)
                            .map_err(|e| format!("Failed to middle click: {}", e))?;
                        format!("Middle clicked at ({}, {}) (unrestricted)", x, y)
                    }
                    "double_click" => {
                        state.desktop.double_click(x, y, None)
                            .map_err(|e| format!("Failed to double click: {}", e))?;
                        format!("Double clicked at ({}, {}) (unrestricted)", x, y)
                    }
                    "triple_click" => {
                        state.desktop.triple_click(x, y, None)
                            .map_err(|e| format!("Failed to triple click: {}", e))?;
                        format!("Triple clicked at ({}, {}) (unrestricted)", x, y)
                    }
                    _ => unreachable!(),
                };
                
                Ok(ComputerResult {
                    success: true,
                    action: input.action,
                    message: Some(message),
                    screenshot: None,
                    error: None,
                    coordinate: Some(vec![x, y]),
                })
            } else {
                // Even in unrestricted mode, we need coordinates for clicks
                Err("Coordinate is required for click actions".to_string())
            }
        }
        "type" => {
            if let Some(text) = input.text {
                // Type without any filtering or validation
                keyboard::type_text(text.clone(), app_handle.clone(), state.clone()).await?;
                Ok(ComputerResult {
                    success: true,
                    action: "type".to_string(),
                    message: Some(format!("Typed text (unrestricted): {}", text)),
                    screenshot: None,
                    error: None,
                    coordinate: None,
                })
            } else {
                Err("Text is required for type action".to_string())
            }
        }
        "key" => {
            if let Some(key) = input.text {
                // Press any key without validation
                keyboard::press_key(key.clone(), None, app_handle.clone(), state.clone()).await?;
                Ok(ComputerResult {
                    success: true,
                    action: "key".to_string(),
                    message: Some(format!("Pressed key (unrestricted): {}", key)),
                    screenshot: None,
                    error: None,
                    coordinate: None,
                })
            } else {
                Err("Key is required for key action".to_string())
            }
        }
        "mouse_move" => {
            if let Some(coord) = input.coordinate {
                let x = coord[0];
                let y = coord[1];
                
                // Move mouse instantly without smooth movement
                state.desktop.mouse_move(x, y)
                    .map_err(|e| format!("Failed to move mouse: {}", e))?;
                
                Ok(ComputerResult {
                    success: true,
                    action: "mouse_move".to_string(),
                    message: Some(format!("Mouse moved to ({}, {}) (unrestricted)", x, y)),
                    screenshot: None,
                    error: None,
                    coordinate: Some(vec![x, y]),
                })
            } else {
                Err("Coordinate is required for mouse_move action".to_string())
            }
        }
        "scroll" => {
            let direction = input.scroll_direction.as_deref().unwrap_or("down");
            let count = input.scroll_count.unwrap_or(5);
            
            // Scroll without restrictions
            if let Some(ref coord) = input.coordinate {
                let x = coord[0];
                let y = coord[1];
                state.desktop.mouse_move(x, y)
                    .map_err(|e| format!("Failed to move mouse: {}", e))?;
            }
            
            // Execute scroll using the scroll command
            use crate::commands::window::scroll_window;
            scroll_window(direction.to_string(), count as f64, None, None, app_handle.clone(), state.clone()).await?;
            
            Ok(ComputerResult {
                success: true,
                action: "scroll".to_string(),
                message: Some(format!("Scrolled {} {} times (unrestricted)", direction, count)),
                screenshot: None,
                error: None,
                coordinate: input.coordinate,
            })
        }
        "wait" => {
            let duration = input.duration.unwrap_or(1000);
            
            // Wait without any limits
            tokio::time::sleep(tokio::time::Duration::from_millis(duration)).await;
            
            Ok(ComputerResult {
                success: true,
                action: "wait".to_string(),
                message: Some(format!("Waited {} ms (unrestricted)", duration)),
                screenshot: None,
                error: None,
                coordinate: None,
            })
        }
        "drag" => {
            if let Some(coord) = input.coordinate {
                let end_x = coord[0];
                let end_y = coord[1];
                
                // Get current cursor position for start point
                let (start_x, start_y) = state.desktop.cursor_position()
                    .map_err(|e| format!("Failed to get cursor position: {}", e))?;
                
                // Perform drag without restrictions
                state.desktop.left_click_drag(start_x, start_y, end_x, end_y)
                    .map_err(|e| format!("Failed to drag: {}", e))?;
                
                Ok(ComputerResult {
                    success: true,
                    action: "drag".to_string(),
                    message: Some(format!("Dragged from ({}, {}) to ({}, {}) (unrestricted)", start_x, start_y, end_x, end_y)),
                    screenshot: None,
                    error: None,
                    coordinate: Some(vec![end_x, end_y]),
                })
            } else {
                Err("Coordinate is required for drag action".to_string())
            }
        }
        // Special unrestricted-only actions
        "execute_system_command" => {
            // This action is only available in unrestricted mode
            if let Some(command) = input.text {
                let computer = unrestricted_computer::UnrestrictedComputer::new();
                let _output = computer.execute_system_command(&command, vec![]).await?;
                
                Ok(ComputerResult {
                    success: true,
                    action: "execute_system_command".to_string(),
                    message: Some(format!("Executed system command: {}", command)),
                    screenshot: None,
                    error: None,
                    coordinate: None,
                })
            } else {
                Err("Command is required for execute_system_command action".to_string())
            }
        }
        _ => {
            // In unrestricted mode, attempt to execute any action
            info!("Attempting unrestricted execution of unknown action: {}", input.action);
            Ok(ComputerResult {
                success: true,
                action: input.action.clone(),
                message: Some(format!("Executed action (unrestricted): {}", input.action)),
                screenshot: None,
                error: None,
                coordinate: input.coordinate,
            })
        }
    }
}

/// Get cursor position - helper function
async fn get_cursor_position_internal() -> Result<(f64, f64), String> {
    use computer_use_ai_sdk::Desktop;
    
    let desktop = Desktop::new(false, false).map_err(|e| e.to_string())?;
    desktop.cursor_position()
        .map_err(|e| e.to_string())
}