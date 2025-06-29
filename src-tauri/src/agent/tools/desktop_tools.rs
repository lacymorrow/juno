//! # Desktop Tools Module
//!
//! Cross-platform desktop automation tools for computer use agents.
//! Provides comprehensive desktop interaction capabilities including mouse control,
//! keyboard input, screen capture, UI element interaction, and clipboard operations.
//!
//! ## Core Capabilities:
//! - Screen capture and element screenshots
//! - Mouse control (click, drag, move, up/down)
//! - Keyboard input (typing, key presses, modifiers)
//! - UI element accessibility and interaction
//! - Clipboard operations (get/set)
//! - Window scrolling and focus management
//!
//! ## Platform Support:
//! - macOS: Full support via computer_use_ai_sdk
//! - Other platforms: Limited support, some features may not be available
//!
//! ## Usage
//! Used by: Anthropic Computer Use agents, desktop automation workflows, UI testing
//! Registration: Called via `register_desktop_tools()` and `setup_tools()` during agent setup

use crate::agent::implementations::tool_provider::LocalToolProvider;
use crate::agent::core::ToolDefinition;
use crate::state::AppState;
use crate::commands;
use crate::utils::permission_validator::{validate_permission, RequiredPermission};
use tauri::{State, Manager};
use serde_json::{Value, json};
use tracing::{info, warn};
use crate::commands::window; // Add window for scroll command
use std::sync::Arc;

use tokio;
use std::time::Duration;


// Removed unused imports: capture_screenshot_command, dev_get_clipboard, dev_set_clipboard
// use crate::{
//     capture_screenshot_command,
//     dev_get_clipboard,
//     dev_set_clipboard,
// };

// Ensure all necessary command modules are imported - keep even if some are unused for now
// as they might be needed by the stubbed function later.

/// Registers additional computer use tools beyond the basic desktop tools.
///
/// This function provides advanced desktop automation tools including scrolling,
/// waiting, key control, and mouse operations. These tools extend the basic
/// desktop capabilities with more sophisticated interaction patterns.
///
/// Used by: Advanced computer use workflows, complex automation scenarios
///
/// # Arguments
/// * `provider` - Mutable reference to LocalToolProvider for tool registration
/// * `app_handle` - Tauri app handle for state access and command execution
///
/// # Returns
/// `Result<(), String>` - Success or error message
///
/// # Tools Registered
/// - `scroll`: Window/element scrolling with direction and amount
/// - `wait`: Pause execution for specified duration
/// - `press_key`: Single key or key combination presses
/// - `hold_key`: Hold key down for duration or until released
/// - `release_key`: Release previously held keys
/// - `left_mouse_down`: Press left mouse button down at coordinates
/// - `left_mouse_up`: Release left mouse button at coordinates
/// - `triple_click`: Perform triple-click at coordinates
// Stub function to resolve compilation error
async fn register_additional_computer_use_tools(
    provider: &mut LocalToolProvider,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    info!("Registering additional computer use tools...");

    // --- Scroll Tool ---
    #[derive(serde::Deserialize)]
    struct ScrollInput {
        // Define coordinates if needed, or use window scroll
        direction: String,
        amount: i32, // Keep as i32? Docs use f64, engine uses f64, command uses f64. Let's use f64.
        // Optional coordinates - if not provided, scroll focused window/element?
        // For now, let's align with the existing registration using dev_scroll_window
        // which doesn't seem to take coordinates directly in the agent tool registration
        // but might internally. Let's omit x,y for now based on existing registration.
        // x: Option<f64>,
        // y: Option<f64>,
    }

    let scroll_def = ToolDefinition {
        name: "scroll".to_string(),
        description: "Scrolls the currently active window/element. Requires accessibility permissions.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "direction": { "type": "string", "enum": ["up", "down", "left", "right"], "description": "Direction to scroll." },
                "amount": { "type": "number", "description": "Amount to scroll (e.g., pixels or lines depending on context)." },
                // "x": { "type": "number", "description": "Optional X coordinate for targeted scroll." },
                // "y": { "type": "number", "description": "Optional Y coordinate for targeted scroll." },
            },
            "required": ["direction", "amount"]
        }),
        api_type: None,
        beta_flag: None,
    };

    let app_handle_clone = app_handle.clone();
    let scroll_exec = move |input: Value| {
        let app = app_handle_clone.clone();
         async move {
             let state_manager = app.state::<AppState>();
             let args = serde_json::from_value::<ScrollInput>(input)
                 .map_err(|e| format!("Failed to parse scroll input: {}", e))?;

             // Note: dev_scroll_window signature might differ slightly from engine's scroll_at_position
             // It takes Optional x,y. We pass None for now.
             let inner_result = tokio::task::block_in_place(|| {
                 let rt = tokio::runtime::Handle::current();
                 rt.block_on(async {
                     // Use the window command as seen elsewhere in this file
                     window::scroll_window(args.direction, args.amount as f64, None, None, app.clone(), state_manager)
                         .await
                 })
             });

             inner_result.map_err(|e| format!("Error scrolling: {}", e))?;
             Ok(json!({"success": true}))
         }
    };
    provider.register_async_tool(scroll_def, scroll_exec).await;
    info!("Registered tool: scroll");

    // --- Wait Tool ---
    #[derive(serde::Deserialize)]
    struct WaitInput { duration_ms: u64 } // Match engine spec

    let wait_def = ToolDefinition {
        name: "wait".to_string(),
        description: "Pauses execution for a specified duration.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "duration_ms": { "type": "integer", "description": "Duration to wait in milliseconds." }
            },
            "required": ["duration_ms"]
        }),
        api_type: None,
        beta_flag: None,
    };

    let app_handle_clone = app_handle.clone();
    let wait_exec = move |input: Value| {
         let app = app_handle_clone.clone(); // Not strictly needed for wait, but keep pattern
         async move {
            let state_manager = app.state::<AppState>(); // Not strictly needed for wait
             let args = serde_json::from_value::<WaitInput>(input)
                 .map_err(|e| format!("Failed to parse wait input: {}", e))?;

             // Directly call the engine's wait method
             match state_manager.desktop.wait(args.duration_ms) {
                 Ok(_) => Ok(json!({"success": true})),
                 Err(e) => Err(format!("Error waiting: {}", e)),
             }
         }
    };
    provider.register_async_tool(wait_def, wait_exec).await;
    info!("Registered tool: wait");

    // NOTE: Keyboard tools (press_key, hold_key) have been REMOVED to eliminate redundancy
    // Use the official Anthropic Computer Use API via the 'computer' tool instead:
    // - computer -> {"action": "key", "text": "Return"}
    // - computer -> {"action": "hold_key", "text": "shift", "duration": 2000}
    // - computer -> {"action": "type", "text": "hello world"}
    //
    // This ensures compliance with the official Anthropic specification and eliminates
    // tool confusion where agents had multiple ways to perform the same keyboard operations.

    // --- Release Key Tool ---
    // NOTE: This tool is KEPT as it provides functionality not available in the official Anthropic API
    // Use this for releasing keys that were held indefinitely (without duration)
    #[derive(serde::Deserialize)]
    struct ReleaseKeyInput { key: String } // Match engine spec

    let release_key_def = ToolDefinition {
        name: "release_key".to_string(),
        description: "Releases a previously held key. Use this for keys held indefinitely (without duration). For timed key holds, use computer tool with hold_key action and duration parameter.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "key": { "type": "string", "description": "Key to release (e.g., 'Shift', 'Control')." }
            },
            "required": ["key"]
        }),
        api_type: None,
        beta_flag: None,
    };

    let app_handle_clone = app_handle.clone(); // Keep pattern
    let release_key_exec = move |input: Value| {
        let app = app_handle_clone.clone(); // Not strictly needed here
        async move {
            let state_manager = app.state::<AppState>();
            let args = serde_json::from_value::<ReleaseKeyInput>(input)
                .map_err(|e| format!("Failed to parse release_key input: {}", e))?;

            // Directly call the engine's release_key method
            match state_manager.desktop.release_key(&args.key) {
                 Ok(_) => Ok(json!({"success": true})),
                 Err(e) => Err(format!("Error releasing key '{}': {}", args.key, e)),
            }
        }
    };
    provider.register_async_tool(release_key_def, release_key_exec).await;
    info!("Registered tool: release_key");


    // REMOVED: 11 redundant mouse tools - Use computer tool with official Anthropic Computer Use API instead
    // The following tools have been consolidated into the computer tool:
    // - dev_left_click, desktop_click, left_click → computer tool with action: "click"
    // - dev_right_click, right_click → computer tool with action: "right_click"
    // - dev_middle_click, middle_click → computer tool with action: "middle_click"
    // - dev_double_click, double_click → computer tool with action: "double_click"
    // - dev_triple_click, triple_click → computer tool with action: "triple_click"
    // - dev_left_click_drag, left_click_drag → computer tool with action: "drag"
    // - dev_left_mouse_down, left_mouse_down → computer tool with action: "drag" (start)
    // - dev_left_mouse_up, left_mouse_up → computer tool with action: "drag" (complete)
    // - mouse_move → computer tool with action: "click" (movement automatic)
    //
    // This eliminates 11 redundant tools and ~400 lines of duplicate code for 100% API compliance.
    // Agents should use the computer tool for ALL mouse operations.

    // TODO: Add other tools as needed, e.g., window management?

    Ok(())
}

/// Registers core desktop tools with the tool provider.
///
/// This function provides fundamental desktop automation capabilities including
/// UI element interaction, screen capture, text input, clipboard operations,
/// and mouse control. These are the essential tools for desktop automation.
///
/// Used by: Agent initialization, desktop automation workflows, UI testing
///
/// # Arguments
/// * `provider` - Mutable reference to LocalToolProvider for tool registration
/// * `_state` - App state (currently unused but kept for interface consistency)
/// * `app_handle` - Tauri app handle for state access and command execution
///
/// # Tools Registered
/// - `get_focused_element_info`: Get accessibility info for focused UI element
/// - `capture_screenshot`: Take full desktop screenshot
/// - `capture_element_screenshot`: Screenshot of focused element
/// - `type_text`: Type text into active application
/// - `get_clipboard`: Get current clipboard text content
/// - `set_clipboard`: Set clipboard text content
/// - `desktop_click`: Click at screen coordinates with modifiers
/// - `mouse_position`: Get current mouse cursor position
/// - `mouse_drag`: Drag from start to end coordinates
// Function to register all desktop tools with the tool provider
pub async fn register_desktop_tools(
    provider: &mut LocalToolProvider,
    _state: State<'_, AppState>,
    app_handle: tauri::AppHandle,
) {
    info!("Registering desktop tools...");

    // --- Element Tools ---

    // Tool for getting accessibility information about the currently focused UI element.
    // Used by: UI automation, accessibility testing, element interaction workflows
    // get_focused_element_info
    let get_focused_def = ToolDefinition {
        name: "get_focused_element_info".to_string(),
        description: "Get accessibility information about the currently focused UI element in the active desktop application.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {},
            "required": []
        }),
        api_type: None,
        beta_flag: None,
    };

    let app_handle_clone = app_handle.clone();
    let get_focused_exec = move |_input: Value| {
        let app = app_handle_clone.clone();
        async move {
            // Validate accessibility permission before accessing UI elements
            if let Err(e) = validate_permission(&app, RequiredPermission::Accessibility, "get_focused_element_info").await {
                return Err(e.to_string());
            }

            let state_manager = app.state::<AppState>();
            let result = tokio::task::block_in_place(|| {
                let rt = tokio::runtime::Handle::current();
                rt.block_on(async {
                    commands::element::get_focused_element_info(app.clone(), state_manager)
                        .await
                })
            }).map_err(|e| format!("Error getting focused element: {}", e))?;
            Ok(json!(result))
        }
    };
    provider.register_async_tool(get_focused_def, get_focused_exec).await;
    info!("Registered tool: get_focused_element_info");

    // Tool for capturing full desktop screenshots.
    // Used by: Computer use agents, visual analysis, UI state documentation
    // capture_screenshot
    let capture_screenshot_def = ToolDefinition {
        name: "capture_screenshot".to_string(),
        description: "Captures a screenshot of the entire desktop screen.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {},
            "required": []
        }),
        api_type: None,
        beta_flag: None,
    };

    let app_handle_clone = app_handle.clone();
    let capture_screenshot_exec = move |_input: Value| {
        let app_handle = app_handle_clone.clone(); // Clone for this specific async move block
         async move {
            // Validate screen recording permission before taking screenshot
            if let Err(e) = validate_permission(&app_handle, RequiredPermission::ScreenRecording, "capture_screenshot").await {
                return Err(e.to_string());
            }

            let block_result: Result<String, String> = tokio::task::block_in_place(|| {
                let rt = tokio::runtime::Handle::current();
                rt.block_on(async {
                    crate::capture_screenshot_command(app_handle.clone()).await // Clone app_handle for the inner async block
                })
            });

            // Handle error from capture_screenshot_command (and map its format if desired)
            let base64_string: String =
                block_result.map_err(|e| format!("Error from screenshot command: {}", e))?;

            Ok(Value::String(base64_string)) // Return as Value::String
         }
    };
    provider.register_async_tool(capture_screenshot_def, capture_screenshot_exec).await;
    info!("Registered tool: capture_screenshot");

    // Tool for capturing screenshots of specific UI elements.
    // Used by: Element-focused automation, accessibility testing, targeted analysis
    // capture_element_screenshot
    let capture_element_screenshot_def = ToolDefinition {
        name: "capture_element_screenshot".to_string(),
        description: "Captures a screenshot of the currently focused UI element on the desktop.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {},
            "required": []
        }),
        api_type: None,
        beta_flag: None,
    };

    let app_handle_clone = app_handle.clone();
    let capture_element_screenshot_exec = move |_input: Value| {
        let app = app_handle_clone.clone();
        async move {
            // Validate accessibility permission before capturing element screenshot
            if let Err(e) = validate_permission(&app, RequiredPermission::Accessibility, "capture_element_screenshot").await {
                return Err(e.to_string());
            }

            let state_manager = app.state::<AppState>();
            let result = tokio::task::block_in_place(|| {
                let rt = tokio::runtime::Handle::current();
                rt.block_on(async {
                    commands::element::capture_element_screenshot_command(app.clone(), state_manager)
                        .await
                })
            }).map_err(|e| format!("Error capturing element screenshot: {}", e))?;
            Ok(json!(result))
        }
    };
    provider.register_async_tool(capture_element_screenshot_def, capture_element_screenshot_exec).await;
    info!("Registered tool: capture_element_screenshot");

    // REMOVED: type_text tool - Use computer tool with action: "type" instead
    // This tool has been consolidated into the official Anthropic Computer Use API.
    // Use: {"name": "computer", "input": {"action": "type", "text": "your text here"}}
    // This eliminates redundancy and ensures 100% compliance with the official specification.

    // Tool for getting current clipboard text content.
    // Used by: Data extraction, clipboard monitoring, text analysis workflows
    // Get Clipboard Tool
    let get_clipboard_def = ToolDefinition {
        name: "get_clipboard".to_string(),
        description: "Get the current text contents of the operating system clipboard.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {},
            "required": []
        }),
        api_type: None,
        beta_flag: None,
    };

    let app_handle_clone = app_handle.clone();
    let get_clipboard_exec = move |_args: Value| {
        let app = app_handle_clone.clone();
        async move {
            let state_manager = app.state::<AppState>();
            match commands::core::get_clipboard(app.clone(), state_manager).await {
                Ok(content) => Ok(json!({ "content": content })),
                Err(e) => Err(format!("Error getting clipboard content: {}", e))
            }
        }
    };

    provider.register_async_tool(get_clipboard_def, get_clipboard_exec).await;
    info!("Registered tool: get_clipboard");

    // Tool for setting clipboard text content.
    // Used by: Data injection, automated copying, content sharing workflows
    // Set Clipboard Tool
    let set_clipboard_def = ToolDefinition {
        name: "set_clipboard".to_string(),
        description: "Set the contents of the operating system clipboard to the specified text.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "content": {
                    "type": "string",
                    "description": "The text content to set in the clipboard"
                }
            },
            "required": ["content"]
        }),
        api_type: None,
        beta_flag: None,
    };

    #[derive(serde::Deserialize)]
    struct SetClipboardContentInput { content: String }

    let app_handle_clone = app_handle.clone();
    let set_clipboard_exec = move |input: Value| {
        let app = app_handle_clone.clone();
        async move {
            let state_manager = app.state::<AppState>();
            let args = serde_json::from_value::<SetClipboardContentInput>(input)
                .map_err(|e| format!("Failed to parse set_clipboard input: {}", e))?;

            let inner_result = tokio::task::block_in_place(|| {
                let rt = tokio::runtime::Handle::current();
                rt.block_on(async {
                    commands::core::set_clipboard(args.content, app.clone(), state_manager)
                        .await
                })
            });
            inner_result.map_err(|e| format!("Error setting clipboard content: {}", e))?;
            Ok(json!({"success": true}))
        }
    };
    provider.register_async_tool(set_clipboard_def, set_clipboard_exec).await;
    info!("Registered tool: set_clipboard");

    // Tool for performing mouse clicks at specified desktop coordinates.
    // Used by: Computer use agents, UI automation, element interaction workflows
    // Desktop click tool
    #[derive(serde::Deserialize)]
    #[allow(dead_code)] // Allow unused fields for now
    struct DesktopClickArgs {
        x: f64,
        y: f64,
        click_type: Option<String>,
        modifier: Option<String>,
    }

    // REMOVED: desktop_click tool - Use computer tool with action: "click" instead
    // This tool has been consolidated into the official Anthropic Computer Use API.
    // Use: {"name": "computer", "input": {"action": "click", "coordinate": [x, y]}}
    // For modifier support: Use computer tool with appropriate key combinations.
    // This eliminates redundancy and ensures 100% compliance with the official specification.

    // Add new computer use tools based on the Anthropic documentation
    // Handle the result of the registration
    if let Err(e) = register_additional_computer_use_tools(provider, app_handle.clone()).await {
        log::error!("Failed to register additional computer use tools: {}", e);
        // Depending on requirements, might want to panic or return an error here
    }

    // Define common input structs from tools2
    #[derive(serde::Deserialize)]
    struct MousePositionInput {
        x: f64,
        y: f64,
    }

    #[derive(serde::Deserialize)]
    struct DragInput {
        start_x: f64,
        start_y: f64,
        end_x: f64,
        end_y: f64,
    }

    // Note: scroll tool is already registered in register_additional_computer_use_tools

    // Note: triple_click tool is already registered in register_additional_computer_use_tools

    // Note: hold_key and release_key tools are already registered in register_additional_computer_use_tools

    // Note: left_mouse_down and left_mouse_up tools are already registered in register_additional_computer_use_tools

    // REMOVED: 11 redundant mouse tools - Use computer tool with official Anthropic Computer Use API instead
    // All mouse operations now use the computer tool with appropriate actions:
    // - Standard clicks: action: "click", "right_click", "middle_click"
    // - Special clicks: action: "double_click", "triple_click"
    // - Drag operations: action: "drag" with start/end coordinates
    // - Mouse movement: automatic with any click action
    //
    // This eliminates 11 redundant tools and ~400 lines of duplicate code for 100% API compliance.
    // Agents should use the computer tool for ALL mouse operations.

    // Tool for getting current mouse cursor position.
    // Used by: Position tracking, relative movement calculations, cursor state queries
    // cursor_position
    let cursor_position_def = ToolDefinition {
        name: "cursor_position".to_string(),
        description: "Get the current cursor position.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {},
            "required": []
        }),
        api_type: None,
        beta_flag: None,
    };
    let app_handle_clone = app_handle.clone();
    let cursor_position_exec = move |_input: Value| {
        let app = app_handle_clone.clone();
        async move {
            // Validate accessibility permission before getting cursor position
            if let Err(e) = validate_permission(&app, RequiredPermission::Accessibility, "cursor_position").await {
                return Err(e.to_string());
            }

            let state_manager = app.state::<AppState>();
            let result = tokio::task::block_in_place(|| {
                let rt = tokio::runtime::Handle::current();
                rt.block_on(async {
                    commands::mouse::get_cursor_position(app.clone(), state_manager).await
                })
            });
            let (x, y) = result.map_err(|e| format!("Error getting cursor position: {}", e))?;

            // info! message from HEAD
            info!("Cursor position: returning screen coordinates ({}, {}) directly (no scaling applied)", x, y);

            Ok(json!({ "x": x, "y": y }))
        }
    };
    provider.register_async_tool(cursor_position_def, cursor_position_exec).await;
    info!("Registered tool: cursor_position");

    // Tool for listing all open windows in the system.
    // Used by: Window discovery, application targeting, desktop state analysis
    // window_management
    let window_list_def = ToolDefinition {
        name: "list_windows".to_string(),
        description: "Get a list of all open windows with their IDs, titles, and applications. Useful for targeting specific windows for screenshots or clicks.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {},
            "required": []
        }),
        api_type: None,
        beta_flag: None,
    };

    let app_handle_clone = app_handle.clone();
    let window_list_exec = move |_input: Value| {
        let app = app_handle_clone.clone();
        async move {
            let state_manager = app.state::<AppState>();
            match crate::commands::window::get_window_list(app.clone(), state_manager).await {
                Ok(window_list_json) => {
                    // Try to parse the JSON to ensure it's valid, then return it
                    match serde_json::from_str::<Value>(&window_list_json) {
                        Ok(parsed) => Ok(parsed),
                        Err(e) => Err(format!("Failed to parse window list JSON: {}", e))
                    }
                },
                Err(e) => Err(format!("Failed to get window list: {}", e))
            }
        }
    };
    provider.register_async_tool(window_list_def, window_list_exec).await;
    info!("Registered tool: list_windows");

    // Tool for getting detailed information about a specific window.
    // Used by: Window analysis, specific window targeting, window state queries
    let window_info_def = ToolDefinition {
        name: "get_window_info".to_string(),
        description: "Get detailed information about a specific window by its ID.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "window_id": {
                    "type": "string",
                    "description": "The ID of the window to get information about"
                }
            },
            "required": ["window_id"]
        }),
        api_type: None,
        beta_flag: None,
    };

    let app_handle_clone = app_handle.clone();
    let window_info_exec = move |input: Value| {
        let app = app_handle_clone.clone();
        async move {
            let state_manager = app.state::<AppState>();
            let window_id = input["window_id"]
                .as_str()
                .ok_or_else(|| "Missing or invalid 'window_id' parameter".to_string())?;

            match crate::commands::window::get_window_info(window_id.to_string(), app.clone(), state_manager).await {
                Ok(window_info_json) => {
                    // Try to parse the JSON to ensure it's valid, then return it
                    match serde_json::from_str::<Value>(&window_info_json) {
                        Ok(parsed) => Ok(parsed),
                        Err(e) => Err(format!("Failed to parse window info JSON: {}", e))
                    }
                },
                Err(e) => Err(format!("Failed to get window info: {}", e))
            }
        }
    };
    provider.register_async_tool(window_info_def, window_info_exec).await;
    info!("Registered tool: get_window_info");

    // === COMPOUND TOOLS ===
    // These tools combine multiple basic operations for common workflows

    // Compound tool for executing shell commands and capturing output.
    // Used by: Development workflows, system administration, automated testing
    #[derive(serde::Deserialize)]
    struct ExecuteCommandArgs {
        command: String,
        timeout_seconds: Option<u64>,
        working_directory: Option<String>,
    }

    let execute_command_def = ToolDefinition {
        name: "execute_command".to_string(),
        description: "Execute a shell command and return the output, error, and exit code. Combines command execution with result capture.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The shell command to execute"
                },
                "timeout_seconds": {
                    "type": "number",
                    "description": "Maximum time to wait for command completion (default: 30 seconds)"
                },
                "working_directory": {
                    "type": "string",
                    "description": "Directory to execute the command in (optional)"
                }
            },
            "required": ["command"]
        }),
        api_type: None,
        beta_flag: None,
    };

    let app_handle_clone = app_handle.clone();
    let execute_command_exec = move |input: Value| {
        let app = app_handle_clone.clone();
        async move {
            let state_manager = app.state::<AppState>();
            let args = serde_json::from_value::<ExecuteCommandArgs>(input)
                .map_err(|e| format!("Failed to parse execute_command input: {}", e))?;

            // Use existing bash_command implementation
            let result = commands::shell::bash_command(
                app.clone(),
                state_manager,
                args.command.clone(),
                args.timeout_seconds,
                None, // restart parameter
                Some(true), // Enable debug mode for agent usage
            ).await;

            match result {
                Ok(bash_result) => {
                    let output = bash_result.get_output();
                    info!("Command executed successfully: {}", args.command);
                    Ok(json!({
                        "success": true,
                        "command": args.command,
                        "output": output,
                        "exit_code": 0,
                        "is_restart": bash_result.is_restart()
                    }))
                }
                Err(e) => {
                    warn!("Command execution failed: {}", e);
                    Ok(json!({
                        "success": false,
                        "command": args.command,
                        "error": e.to_string(),
                        "exit_code": 1
                    }))
                }
            }
        }
    };
    provider.register_async_tool(execute_command_def, execute_command_exec).await;
    info!("Registered compound tool: execute_command");

    // Compound tool for opening a file and typing content into it.
    // Used by: File editing workflows, content creation, automated document generation
    #[derive(serde::Deserialize)]
    struct OpenFileAndTypeArgs {
        file_path: String,
        content: String,
        append: Option<bool>,
    }

    let open_file_and_type_def = ToolDefinition {
        name: "open_file_and_type".to_string(),
        description: "Open a file in the default editor and type content into it. Combines file opening with text input.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "Path to the file to open and edit"
                },
                "content": {
                    "type": "string",
                    "description": "Text content to type into the file"
                },
                "append": {
                    "type": "boolean",
                    "description": "Whether to append to the file (true) or overwrite (false, default)"
                }
            },
            "required": ["file_path", "content"]
        }),
        api_type: None,
        beta_flag: None,
    };

    let app_handle_clone = app_handle.clone();
    let open_file_and_type_exec = move |input: Value| {
        let app = app_handle_clone.clone();
        async move {
            let state_manager = app.state::<AppState>();
            let args = serde_json::from_value::<OpenFileAndTypeArgs>(input)
                .map_err(|e| format!("Failed to parse open_file_and_type input: {}", e))?;

            // Step 0: Validate accessibility permission before any keyboard operations
            if let Err(e) = validate_permission(&app, RequiredPermission::Accessibility, "open_file_and_type").await {
                return Err(format!("Accessibility permission required for open_file_and_type: {}", e));
            }

            // Step 1: Check if file path is accessible and create directory if needed
            let file_path = std::path::Path::new(&args.file_path);
            if let Some(parent_dir) = file_path.parent() {
                if !parent_dir.exists() {
                    if let Err(e) = std::fs::create_dir_all(parent_dir) {
                        return Err(format!("Failed to create directory '{}': {}", parent_dir.display(), e));
                    }
                }
            }

            // Step 2: Attempt direct file creation if it doesn't exist
            if !file_path.exists() {
                match std::fs::write(&args.file_path, "") {
                    Ok(_) => {
                        info!("Created empty file: {}", args.file_path);
                    }
                    Err(e) => {
                        warn!("Failed to create file directly, will try opening with default app: {}", e);
                    }
                }
            }

            // Step 3: Open file with default application (with timeout)
            let open_command = format!("open '{}'", args.file_path);
            let open_result = commands::shell::bash_command(
                app.clone(),
                state_manager,
                open_command,
                Some(15), // Increased timeout for opening (15 seconds)
                None,
                Some(true), // Enable debug mode for agent usage
            ).await;

            if let Err(e) = open_result {
                // Fallback: Try to write content directly to file
                warn!("Failed to open file with default app, attempting direct write: {}", e);

                let content_to_write = if args.append.unwrap_or(false) {
                    // Read existing content and append
                    match std::fs::read_to_string(&args.file_path) {
                        Ok(existing) => format!("{}\n{}", existing, args.content),
                        Err(_) => args.content.clone(),
                    }
                } else {
                    args.content.clone()
                };

                match std::fs::write(&args.file_path, content_to_write) {
                    Ok(_) => {
                        info!("Successfully wrote content directly to file: {}", args.file_path);
                        return Ok(json!({
                            "success": true,
                            "file_path": args.file_path,
                            "content_length": args.content.len(),
                            "operation": if args.append.unwrap_or(false) { "append" } else { "write" },
                            "method": "direct_write",
                            "message": "File written directly (app opening failed)"
                        }));
                    }
                    Err(e) => {
                        return Err(format!("Failed to open file '{}' and direct write also failed: {}", args.file_path, e));
                    }
                }
            }

            // Step 4: Wait for application to launch (with adaptive timing)
            info!("Waiting for application to launch...");
            tokio::time::sleep(std::time::Duration::from_millis(3000)).await;

            // Step 5: Type the content with timeout using tokio::time::timeout
            let state_manager = app.state::<AppState>();
            let typing_timeout = Duration::from_secs(30); // 30 second timeout for typing

            let typing_result = tokio::time::timeout(
                typing_timeout,
                commands::keyboard::type_text(args.content.clone(), app.clone(), state_manager)
            ).await;

            match typing_result {
                Ok(Ok(_)) => {
                    info!("Successfully opened file and typed content: {}", args.file_path);
                    Ok(json!({
                        "success": true,
                        "file_path": args.file_path,
                        "content_length": args.content.len(),
                        "operation": if args.append.unwrap_or(false) { "append" } else { "write" },
                        "method": "app_and_type"
                    }))
                }
                Ok(Err(e)) => {
                    // Typing failed, try fallback direct write
                    warn!("Typing failed, attempting direct write fallback: {}", e);

                    let content_to_write = if args.append.unwrap_or(false) {
                        match std::fs::read_to_string(&args.file_path) {
                            Ok(existing) => format!("{}\n{}", existing, args.content),
                            Err(_) => args.content.clone(),
                        }
                    } else {
                        args.content.clone()
                    };

                    match std::fs::write(&args.file_path, content_to_write) {
                        Ok(_) => {
                            info!("Fallback: Successfully wrote content directly to file: {}", args.file_path);
                            Ok(json!({
                                "success": true,
                                "file_path": args.file_path,
                                "content_length": args.content.len(),
                                "operation": if args.append.unwrap_or(false) { "append" } else { "write" },
                                "method": "direct_write_fallback",
                                "message": "Used direct write after typing failed"
                            }))
                        }
                        Err(write_err) => {
                            Err(format!("Typing failed and direct write fallback also failed. Typing error: {}. Write error: {}", e, write_err))
                        }
                    }
                }
                Err(_) => {
                    // Timeout occurred
                    warn!("Typing operation timed out after {} seconds, attempting direct write fallback", typing_timeout.as_secs());

                    let content_to_write = if args.append.unwrap_or(false) {
                        match std::fs::read_to_string(&args.file_path) {
                            Ok(existing) => format!("{}\n{}", existing, args.content),
                            Err(_) => args.content.clone(),
                        }
                    } else {
                        args.content.clone()
                    };

                    match std::fs::write(&args.file_path, content_to_write) {
                        Ok(_) => {
                            info!("Timeout fallback: Successfully wrote content directly to file: {}", args.file_path);
                            Ok(json!({
                                "success": true,
                                "file_path": args.file_path,
                                "content_length": args.content.len(),
                                "operation": if args.append.unwrap_or(false) { "append" } else { "write" },
                                "method": "direct_write_timeout_fallback",
                                "message": "Used direct write after typing timed out"
                            }))
                        }
                        Err(write_err) => {
                            Err(format!("Typing timed out after {} seconds and direct write fallback also failed: {}", typing_timeout.as_secs(), write_err))
                        }
                    }
                }
            }
        }
    };
    provider.register_async_tool(open_file_and_type_def, open_file_and_type_exec).await;
    info!("Registered compound tool: open_file_and_type");

    // Compound tool for saving and closing the current file.
    // Used by: File editing workflows, automated saving, document completion
    let save_and_close_file_def = ToolDefinition {
        name: "save_and_close_file".to_string(),
        description: "Save the current file and close the editor. Uses keyboard shortcuts Cmd+S and Cmd+W.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {},
            "required": []
        }),
        api_type: None,
        beta_flag: None,
    };

    let app_handle_clone = app_handle.clone();
    let save_and_close_file_exec = move |_input: Value| {
        let app = app_handle_clone.clone();
        async move {
            // Validate accessibility permission before keyboard operations
            if let Err(e) = validate_permission(&app, RequiredPermission::Accessibility, "save_and_close_file").await {
                return Err(e.to_string());
            }

            let state_manager = app.state::<AppState>();

            // Step 1: Save file with Cmd+S
            let save_result = crate::commands::keyboard::press_key(
                "s".to_string(),
                Some("cmd".to_string()),
                app.clone(),
                state_manager,
            ).await;

            if let Err(e) = save_result {
                return Err(format!("Failed to save file: {}", e));
            }

            // Step 2: Wait a moment for save to complete
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;

            // Step 3: Close file with Cmd+W
            let state_manager = app.state::<AppState>();
            let close_result = crate::commands::keyboard::press_key(
                "w".to_string(),
                Some("cmd".to_string()),
                app.clone(),
                state_manager,
            ).await;

            if let Err(e) = close_result {
                warn!("Failed to close file: {}", e);
                return Err(format!("Failed to close file: {}", e));
            }

            Ok(json!({
                "success": true,
                "message": "File saved and closed successfully"
            }))
        }
    };
    provider.register_async_tool(save_and_close_file_def, save_and_close_file_exec).await;
    info!("Registered compound tool: save_and_close_file");

    // Compound tool for copying text to clipboard and pasting at cursor.
    // Used by: Text manipulation workflows, content transfer, automated copy-paste operations
    #[derive(serde::Deserialize)]
    struct CopyAndPasteArgs {
        text: String,
        clear_selection: Option<bool>,
    }

    let copy_to_clipboard_and_paste_def = ToolDefinition {
        name: "copy_to_clipboard_and_paste".to_string(),
        description: "Copy text to the clipboard and immediately paste it at the current cursor position.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "text": {
                    "type": "string",
                    "description": "Text to copy to clipboard and paste"
                },
                "clear_selection": {
                    "type": "boolean",
                    "description": "Whether to clear current selection before pasting (default: false)"
                }
            },
            "required": ["text"]
        }),
        api_type: None,
        beta_flag: None,
    };

    let app_handle_clone = app_handle.clone();
    let copy_to_clipboard_and_paste_exec = move |input: Value| {
        let app = app_handle_clone.clone();
        async move {
            // Validate accessibility permission before keyboard operations
            if let Err(e) = validate_permission(&app, RequiredPermission::Accessibility, "copy_to_clipboard_and_paste").await {
                return Err(e.to_string());
            }

            let args = serde_json::from_value::<CopyAndPasteArgs>(input)
                .map_err(|e| format!("Failed to parse copy_and_paste input: {}", e))?;

            // Step 1: Set clipboard content
            let state_manager = app.state::<AppState>();
            let clipboard_result = commands::core::set_clipboard(args.text.clone(), app.clone(), state_manager).await;

            if let Err(e) = clipboard_result {
                return Err(format!("Failed to set clipboard: {}", e));
            }

            // Step 2: Clear selection if requested
            if args.clear_selection.unwrap_or(false) {
                // Press Escape to clear selection
                let state_manager = app.state::<AppState>();
                let _ = crate::commands::keyboard::press_key(
                    "Escape".to_string(),
                    None,
                    app.clone(),
                    state_manager,
                ).await;

                // Brief pause
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }

            // Step 3: Paste with Cmd+V
            let state_manager = app.state::<AppState>();
            let paste_result = crate::commands::keyboard::press_key(
                "v".to_string(),
                Some("cmd".to_string()),
                app.clone(),
                state_manager,
            ).await;

            match paste_result {
                Ok(_) => {
                    info!("Successfully copied to clipboard and pasted text ({} chars)", args.text.len());
                    Ok(json!({
                        "success": true,
                        "text_length": args.text.len(),
                        "operations": ["copy_to_clipboard", "paste"],
                        "cleared_selection": args.clear_selection.unwrap_or(false)
                    }))
                }
                Err(e) => {
                    Err(format!("Failed to paste from clipboard: {}", e))
                }
            }
        }
    };
    provider.register_async_tool(copy_to_clipboard_and_paste_def, copy_to_clipboard_and_paste_exec).await;
    info!("Registered compound tool: copy_to_clipboard_and_paste");

    info!("All compound tools registered successfully.");

    info!("Desktop tool registration completed.");
}

/// Sets up the complete tool provider with desktop tools and MCP integration.
///
/// This is the main setup function that initializes all desktop automation capabilities
/// and integrates with Model Context Protocol (MCP) servers for extensibility.
/// It serves as a wrapper for register_desktop_tools with additional MCP setup.
///
/// Used by: Agent initialization, main tool provider setup in application startup
///
/// # Arguments
/// * `provider` - Mutable reference to LocalToolProvider for tool registration
/// * `state` - App state containing MCP manager and configuration
/// * `app_handle` - Tauri app handle for state access and command execution
///
/// # Returns
/// `Arc<Mutex<LocalToolProvider>>` - Thread-safe shared tool provider instance
///
/// # Features
/// - Registers all desktop automation tools
/// - Initializes MCP server connections
/// - Sets up MCP tool refresh capabilities
/// - Returns shared provider instance for multi-threaded access
// Function to set up tools (wrapper for register_desktop_tools for backwards compatibility)
pub async fn setup_tools(
    provider: &mut LocalToolProvider,
    state: State<'_, AppState>,
    app_handle: tauri::AppHandle,
) -> Arc<tokio::sync::Mutex<LocalToolProvider>> {
    // Set up MCP manager in the tool provider (lightweight operation)
    let mcp_manager = state.get_mcp_manager().await;
    provider.set_mcp_manager(mcp_manager);

    // Register basic desktop tools (core functionality)
    register_desktop_tools(provider, state.clone(), app_handle.clone()).await;

    // MCP servers are initialized at app startup via state_management.rs to avoid
    // repeated initialization on every agent creation. Here we only refresh from cache.

    // Refresh MCP tools from cache (fast operation - no network calls or server startup)
    if let Err(e) = provider.refresh_mcp_tools().await {
        log::warn!("Failed to refresh MCP tools from cache: {}", e);
    } else {
        log::debug!("MCP tools refreshed from cache for tool provider");
    }

    // Create the Arc<Mutex<>> wrapper for the provider
    let provider_arc = std::sync::Arc::new(tokio::sync::Mutex::new(provider.clone()));

    // Register this tool provider with the AppState for future MCP refresh notifications
    // Using Weak reference to prevent Arc cycles
    if let Err(e) = state.register_tool_provider(provider_arc.clone()).await {
        log::warn!("Failed to register tool provider: {}", e);
    } else {
        log::debug!("Tool provider registered with AppState for MCP refresh notifications");
    }

    // Return the Arc so the caller can use the same instance that's registered
    provider_arc
}
