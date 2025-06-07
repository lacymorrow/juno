use crate::agent::implementations::tool_provider::LocalToolProvider;
use crate::agent::structs::ToolDefinition;
use crate::state::AppState;
use crate::commands;
use tauri::{State, Manager};
use serde_json::{Value, json};
use tracing::info;
<<<<<<< HEAD
// use std::fs; // Unused
// use std::process::Command; // Unused
// use std::io::Write; // Unused
// use std::sync::Arc; // Unused
use crate::commands::window; // Add window for scroll command
use std::sync::Arc;

// Removed unused imports: capture_screenshot_command, dev_get_clipboard, dev_set_clipboard
// use crate::{
//     capture_screenshot_command,
//     dev_get_clipboard,
//     dev_set_clipboard,
// };

// Ensure all necessary command modules are imported - keep even if some are unused for now
// as they might be needed by the stubbed function later.

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
                     window::dev_scroll_window(app.clone(), state_manager, args.direction, args.amount as f64, None, None)
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

    // --- Press Key Tool --- (Corresponds to 'key' in Anthropic spec)
    #[derive(serde::Deserialize)]
    struct PressKeyInput { key: String, modifier: Option<String> }

    let press_key_def = ToolDefinition {
        name: "press_key".to_string(),
        description: "Presses a single key or key combination (e.g., 'a', 'Return', 'cmd+c'). See xdotool key names.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "key": { "type": "string", "description": "The key or key combination to press (e.g., 'a', 'Return', 'alt+Tab', 'ctrl+s')." },
                "modifier": { "type": "string", "enum": ["shift", "ctrl", "alt", "cmd"], "description": "Optional modifier key to hold during the press." }
            },
            "required": ["key"]
        }),
    };

    let app_handle_clone = app_handle.clone();
    let press_key_exec = move |input: Value| {
        let app = app_handle_clone.clone();
        async move {
            let state_manager = app.state::<AppState>();
            let args = serde_json::from_value::<PressKeyInput>(input)
                .map_err(|e| format!("Failed to parse press_key input: {}", e))?;

            let inner_result = tokio::task::block_in_place(|| {
                let rt = tokio::runtime::Handle::current();
                rt.block_on(async {
                    commands::keyboard::dev_press_key(args.key, args.modifier, state_manager)
                        .await
                })
            });
            inner_result.map_err(|e| format!("Error pressing key: {}", e))?;
            Ok(json!({"success": true}))
        }
    };
    provider.register_async_tool(press_key_def, press_key_exec).await;
    info!("Registered tool: press_key");


    // --- Hold Key Tool ---
    #[derive(serde::Deserialize)]
    struct HoldKeyInput { key: String, duration_ms: Option<u64> } // Match engine spec

    let hold_key_def = ToolDefinition {
        name: "hold_key".to_string(),
        description: "Holds down a key for a specified duration (or until released).".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "key": { "type": "string", "description": "Key to hold down (e.g., 'Shift', 'Control')." },
                "duration_ms": { "type": ["integer", "null"], "description": "Optional duration in milliseconds to hold the key. If null/omitted, key is held until 'release_key' is called." }
            },
            "required": ["key"]
        }),
    };

    let app_handle_clone = app_handle.clone(); // Keep pattern even if app not directly used
    let hold_key_exec = move |input: Value| {
        let app = app_handle_clone.clone(); // Not strictly needed here
        async move {
            let state_manager = app.state::<AppState>();
            let args = serde_json::from_value::<HoldKeyInput>(input)
                .map_err(|e| format!("Failed to parse hold_key input: {}", e))?;

            // Directly call the engine's hold_key method
            match state_manager.desktop.hold_key(&args.key, args.duration_ms) {
                Ok(_) => Ok(json!({"success": true})),
                Err(e) => Err(format!("Error holding key '{}': {}", args.key, e)),
            }
        }
    };
    provider.register_async_tool(hold_key_def, hold_key_exec).await;
    info!("Registered tool: hold_key");

    // --- Release Key Tool ---
    #[derive(serde::Deserialize)]
    struct ReleaseKeyInput { key: String } // Match engine spec

    let release_key_def = ToolDefinition {
        name: "release_key".to_string(),
        description: "Releases a previously held key.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "key": { "type": "string", "description": "Key to release (e.g., 'Shift', 'Control')." }
            },
            "required": ["key"]
        }),
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


    // --- Left Mouse Down Tool ---
    #[derive(serde::Deserialize)]
    struct MousePositionInput { x: f64, y: f64 } // Re-use existing struct

    let left_mouse_down_def = ToolDefinition {
        name: "left_mouse_down".to_string(),
        description: "Presses the left mouse button down at the specified coordinates (screen coordinates).".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "x": { "type": "number", "description": "Screen X coordinate." },
                "y": { "type": "number", "description": "Screen Y coordinate." }
            },
            "required": ["x", "y"]
        }),
    };
    let app_handle_clone = app_handle.clone();
    let left_mouse_down_exec = move |input: Value| {
        let app = app_handle_clone.clone();
        async move {
            let state_manager = app.state::<AppState>();
            let args = serde_json::from_value::<MousePositionInput>(input)
                .map_err(|e| format!("Failed to parse mouse position input for down: {}", e))?;
            let inner_result = tokio::task::block_in_place(|| {
                let rt = tokio::runtime::Handle::current();
                rt.block_on(async {
                    commands::mouse::dev_left_mouse_down(app.clone(), state_manager, args.x, args.y).await
                })
            });
            inner_result.map_err(|e| format!("Error pressing left mouse down: {}", e))?;
            Ok(json!({"success": true}))
        }
    };
    provider.register_async_tool(left_mouse_down_def, left_mouse_down_exec).await;
    info!("Registered tool: left_mouse_down");

    // --- Left Mouse Up Tool ---
    let left_mouse_up_def = ToolDefinition {
        name: "left_mouse_up".to_string(),
        description: "Releases the left mouse button at the specified coordinates (screen coordinates).".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "x": { "type": "number", "description": "Screen X coordinate." },
                "y": { "type": "number", "description": "Screen Y coordinate." }
            },
            "required": ["x", "y"]
        }),
    };
    let app_handle_clone = app_handle.clone();
    let left_mouse_up_exec = move |input: Value| {
        let app = app_handle_clone.clone();
        async move {
            let state_manager = app.state::<AppState>();
            let args = serde_json::from_value::<MousePositionInput>(input)
                .map_err(|e| format!("Failed to parse mouse position input for up: {}", e))?;
            let inner_result = tokio::task::block_in_place(|| {
                let rt = tokio::runtime::Handle::current();
                rt.block_on(async {
                    commands::mouse::dev_left_mouse_up(app.clone(), state_manager, args.x, args.y).await
                })
            });
            inner_result.map_err(|e| format!("Error releasing left mouse up: {}", e))?;
            Ok(json!({"success": true}))
        }
    };
    provider.register_async_tool(left_mouse_up_def, left_mouse_up_exec).await;
    info!("Registered tool: left_mouse_up");

    // --- Triple Click Tool ---
    let triple_click_def = ToolDefinition {
        name: "triple_click".to_string(),
        description: "Performs a triple left click at the specified coordinates (screen coordinates).".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "x": { "type": "number", "description": "Screen X coordinate." },
                "y": { "type": "number", "description": "Screen Y coordinate." },
                 "modifier": { "type": "string", "enum": ["shift", "ctrl", "alt", "cmd"], "description": "Optional modifier key." }
            },
            "required": ["x", "y"]
        }),
    };
    #[derive(serde::Deserialize)]
    struct ClickInput { x: f64, y: f64, modifier: Option<String> } // Re-use for triple click

    let app_handle_clone = app_handle.clone();
    let triple_click_exec = move |input: Value| {
        let app = app_handle_clone.clone();
        async move {
            let state_manager = app.state::<AppState>();
            let args = serde_json::from_value::<ClickInput>(input)
                .map_err(|e| format!("Failed to parse triple_click input: {}", e))?;

            let inner_result = tokio::task::block_in_place(|| {
                let rt = tokio::runtime::Handle::current();
                rt.block_on(async {
                    commands::mouse::dev_triple_click(app.clone(), state_manager, args.x, args.y, args.modifier).await
                })
            });
            inner_result.map_err(|e| format!("Error triple clicking: {}", e))?;
            Ok(json!({"success": true}))
        }
    };
    provider.register_async_tool(triple_click_def, triple_click_exec).await;
    info!("Registered tool: triple_click");

    // TODO: Add other tools as needed, e.g., window management?

    Ok(())
}
=======
>>>>>>> main

// Function to register all desktop tools with the tool provider
pub async fn register_desktop_tools(
    provider: &mut LocalToolProvider,
    _state: State<'_, AppState>,
    app_handle: tauri::AppHandle,
) {
    info!("Registering desktop tools...");

    // --- Element Tools ---

    // get_focused_element_info
    let get_focused_def = ToolDefinition {
        name: "get_focused_element_info".to_string(),
        description: "Get accessibility information about the currently focused UI element in the active desktop application.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {},
            "required": []
        }),
    };

    let app_handle_clone = app_handle.clone();
    let get_focused_exec = move |_input: Value| {
        let app = app_handle_clone.clone();
        async move {
            let state_manager = app.state::<AppState>();
            let result = tokio::task::block_in_place(|| {
                let rt = tokio::runtime::Handle::current();
                rt.block_on(async {
                    commands::element::dev_get_focused_element_info(app.clone(), state_manager)
                        .await
                })
            }).map_err(|e| format!("Error getting focused element: {}", e))?;
            Ok(json!(result))
        }
    };
    provider.register_async_tool(get_focused_def, get_focused_exec).await;
    info!("Registered tool: get_focused_element_info");

    // capture_screenshot
    let capture_screenshot_def = ToolDefinition {
        name: "capture_screenshot".to_string(),
        description: "Captures a screenshot of the entire desktop screen.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {},
            "required": []
        }),
    };

    let app_handle_clone = app_handle.clone();
    let capture_screenshot_exec = move |_input: Value| {
        let app_handle = app_handle_clone.clone(); // Clone for this specific async move block
         async move {
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

    // capture_element_screenshot
    let capture_element_screenshot_def = ToolDefinition {
        name: "capture_element_screenshot".to_string(),
        description: "Captures a screenshot of the currently focused UI element on the desktop.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {},
            "required": []
        }),
    };

    let app_handle_clone = app_handle.clone();
    let capture_element_screenshot_exec = move |_input: Value| {
        let app = app_handle_clone.clone();
        async move {
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

    // type_text
    #[derive(serde::Deserialize)]
    #[allow(dead_code)] // Allow unused fields for now
    struct TypeTextArgs { text: String, delay: Option<f64> }

    let type_text_def = ToolDefinition {
        name: "type_text".to_string(),
        description: "Types the given text into the active desktop application, optionally with a delay between characters.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "text": { "type": "string" },
                "delay": { "type": "number", "description": "Delay in seconds between keystrokes" }
            },
            "required": ["text"]
        }),
    };

    let app_handle_clone = app_handle.clone();
    let type_text_exec = move |input: Value| {
        let app = app_handle_clone.clone();
        async move {
            let state_manager = app.state::<AppState>();
            let args = serde_json::from_value::<TypeTextArgs>(input)
                .map_err(|e| format!("Failed to parse type_text input: {}", e))?;

            let inner_result = tokio::task::block_in_place(|| {
                let rt = tokio::runtime::Handle::current();
                rt.block_on(async {
                    commands::keyboard::dev_type_text(args.text, state_manager)
                        .await
                })
            });
            inner_result.map_err(|e| format!("Error typing text: {}", e))?;
            Ok(json!({"success": true}))
        }
    };
    provider.register_async_tool(type_text_def, type_text_exec).await;
    info!("Registered tool: type_text");

    // Get Clipboard Tool
    let get_clipboard_def = ToolDefinition {
        name: "get_clipboard".to_string(),
        description: "Get the current text contents of the operating system clipboard.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {},
            "required": []
        }),
    };

    let app_handle_clone = app_handle.clone();
    let get_clipboard_exec = move |_args: Value| {
        let app = app_handle_clone.clone();
        async move {
            let state_manager = app.state::<AppState>();
            match commands::core::dev_get_clipboard(state_manager).await {
                Ok(content) => Ok(json!({ "content": content })),
                Err(e) => Err(format!("Error getting clipboard content: {}", e))
            }
        }
    };

    provider.register_async_tool(get_clipboard_def, get_clipboard_exec).await;
    info!("Registered tool: get_clipboard");

    // Set Clipboard Tool
    #[derive(serde::Deserialize)]
    struct SetClipboardContentInput { content: String }

    let set_clipboard_def = ToolDefinition {
        name: "set_clipboard_content".to_string(),
        description: "Sets the operating system clipboard content to the provided text.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "content": { "type": "string" }
            },
            "required": ["content"]
        }),
    };

    let app_handle_clone = app_handle.clone();
    let set_clipboard_exec = move |input: Value| {
        let app = app_handle_clone.clone();
        async move {
            let state_manager = app.state::<AppState>();
            let args = serde_json::from_value::<SetClipboardContentInput>(input)
                .map_err(|e| format!("Failed to parse set_clipboard_content input: {}", e))?;

            let inner_result = tokio::task::block_in_place(|| {
                let rt = tokio::runtime::Handle::current();
                rt.block_on(async {
                    commands::core::dev_set_clipboard(args.content, state_manager)
                        .await
                })
            });
            inner_result.map_err(|e| format!("Error setting clipboard content: {}", e))?;
            Ok(json!({"success": true}))
        }
    };
    provider.register_async_tool(set_clipboard_def, set_clipboard_exec).await;
    info!("Registered tool: set_clipboard_content");

    // Desktop click tool
    #[derive(serde::Deserialize)]
    #[allow(dead_code)] // Allow unused fields for now
    struct DesktopClickArgs {
        x: f64,
        y: f64,
        click_type: Option<String>,
        modifier: Option<String>,
    }

    let desktop_click_def = ToolDefinition {
        name: "desktop_click".to_string(),
        description: "Performs a mouse click at specified desktop coordinates with optional click type and modifier key.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "x": { "type": "number", "description": "The x coordinate to click at." },
                "y": { "type": "number", "description": "The y coordinate to click at." },
                "click_type": { "type": "string", "description": "Type of click: 'left', 'right', 'double', or 'triple'." },
                "modifier": { "type": "string", "description": "Modifier key: 'cmd', 'shift', 'alt', or 'ctrl'." }
            },
            "required": ["x", "y"]
        }),
    };

    let app_handle_clone = app_handle.clone();
    let desktop_click_exec = move |input: Value| {
        let app = app_handle_clone.clone();
        async move {
            let state_manager = app.state::<AppState>();
            let args = serde_json::from_value::<DesktopClickArgs>(input)
                .map_err(|e| format!("Failed to parse desktop click input: {}", e))?;

            let inner_result = tokio::task::block_in_place(|| {
                let rt = tokio::runtime::Handle::current();
                rt.block_on(async {
                    commands::mouse::dev_left_click(app.clone(), state_manager, args.x, args.y, args.modifier)
                        .await
                })
            });
            inner_result.map_err(|e| format!("Error clicking on desktop: {}", e))?;
            Ok(json!({"success": true}))
        }
    };
    provider.register_async_tool(desktop_click_def, desktop_click_exec).await;
    info!("Registered tool: desktop_click");
<<<<<<< HEAD

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

    // mouse_move
    let mouse_move_def = ToolDefinition {
        name: "mouse_move".to_string(),
        description: "Move the mouse cursor to the specified coordinates.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "x": { "type": "number", "description": "The x coordinate to move to." },
                "y": { "type": "number", "description": "The y coordinate to move to." }
            },
            "required": ["x", "y"]
        }),
    };
    let app_handle_clone = app_handle.clone();
    let mouse_move_exec = move |input: Value| {
        let app = app_handle_clone.clone();
        async move {
            let state_manager = app.state::<AppState>();
            let args = serde_json::from_value::<MousePositionInput>(input)
                .map_err(|e| format!("Failed to parse mouse position input: {}", e))?;

            // info! message from HEAD
            info!("Mouse move at ({}, {}) - no transformation applied", args.x, args.y);

            let inner_result = tokio::task::block_in_place(|| {
                let rt = tokio::runtime::Handle::current();
                rt.block_on(async {
                    commands::mouse::dev_mouse_move(app.clone(), state_manager, args.x, args.y).await
                })
            });
            inner_result.map_err(|e| format!("Error moving mouse: {}", e))?;
            Ok(json!({"success": true}))
        }
    };
    provider.register_async_tool(mouse_move_def, mouse_move_exec).await;
    info!("Registered tool: mouse_move");

    // left_click
    let left_click_def = ToolDefinition {
        name: "left_click".to_string(),
        description: "Perform a left click at the specified coordinates.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "x": { "type": "number", "description": "The x coordinate." },
                "y": { "type": "number", "description": "The y coordinate." }
            },
            "required": ["x", "y"]
        }),
    };
    let app_handle_clone = app_handle.clone();
    let left_click_exec = move |input: Value| {
        let app = app_handle_clone.clone();
        async move {
            let state_manager = app.state::<AppState>();
            let args = serde_json::from_value::<MousePositionInput>(input)
                .map_err(|e| format!("Failed to parse mouse position input: {}", e))?;

            // info! message from HEAD
            info!("Left click at ({}, {}) - no transformation applied", args.x, args.y);

            let inner_result = tokio::task::block_in_place(|| {
                let rt = tokio::runtime::Handle::current();
                rt.block_on(async {
                    commands::mouse::dev_left_click(app.clone(), state_manager, args.x, args.y, None).await
                })
            });
            inner_result.map_err(|e| format!("Error left clicking: {}", e))?;
            Ok(json!({"success": true}))
        }
    };
    provider.register_async_tool(left_click_def, left_click_exec).await;
    info!("Registered tool: left_click");

    // right_click
    let right_click_def = ToolDefinition {
        name: "right_click".to_string(),
        description: "Perform a right click at the specified coordinates.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "x": { "type": "number", "description": "The x coordinate." },
                "y": { "type": "number", "description": "The y coordinate." }
            },
            "required": ["x", "y"]
        }),
    };
    let app_handle_clone = app_handle.clone();
    let right_click_exec = move |input: Value| {
        let app = app_handle_clone.clone();
        async move {
            let state_manager = app.state::<AppState>();
            let args = serde_json::from_value::<MousePositionInput>(input)
                .map_err(|e| format!("Failed to parse mouse position input: {}", e))?;

            // info! message from HEAD
            info!("Right click at ({}, {}) - no transformation applied", args.x, args.y);

            let inner_result = tokio::task::block_in_place(|| {
                let rt = tokio::runtime::Handle::current();
                rt.block_on(async {
                    commands::mouse::dev_right_click(app.clone(), state_manager, args.x, args.y, None).await
                })
            });
            inner_result.map_err(|e| format!("Error right clicking: {}", e))?;
            Ok(json!({"success": true}))
        }
    };
    provider.register_async_tool(right_click_def, right_click_exec).await;
    info!("Registered tool: right_click");

    // middle_click
    let middle_click_def = ToolDefinition {
        name: "middle_click".to_string(),
        description: "Perform a middle click at the specified coordinates.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "x": { "type": "number", "description": "The x coordinate." },
                "y": { "type": "number", "description": "The y coordinate." }
            },
            "required": ["x", "y"]
        }),
    };
    let app_handle_clone = app_handle.clone();
    let middle_click_exec = move |input: Value| {
        let app = app_handle_clone.clone();
        async move {
            let state_manager = app.state::<AppState>();
            let args = serde_json::from_value::<MousePositionInput>(input)
                .map_err(|e| format!("Failed to parse mouse position input: {}", e))?;

            // info! message from HEAD
            info!("Middle click at ({}, {}) - no transformation applied", args.x, args.y);

            let inner_result = tokio::task::block_in_place(|| {
                let rt = tokio::runtime::Handle::current();
                rt.block_on(async {
                    commands::mouse::dev_middle_click(app.clone(), state_manager, args.x, args.y, None).await
                })
            });
            inner_result.map_err(|e| format!("Error middle clicking: {}", e))?;
            Ok(json!({"success": true}))
        }
    };
    provider.register_async_tool(middle_click_def, middle_click_exec).await;
    info!("Registered tool: middle_click");

    // double_click
    let double_click_def = ToolDefinition {
        name: "double_click".to_string(),
        description: "Perform a double click at the specified coordinates.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "x": { "type": "number", "description": "The x coordinate." },
                "y": { "type": "number", "description": "The y coordinate." }
            },
            "required": ["x", "y"]
        }),
    };
    let app_handle_clone = app_handle.clone();
    let double_click_exec = move |input: Value| {
        let app = app_handle_clone.clone();
        async move {
            let state_manager = app.state::<AppState>();
            let args = serde_json::from_value::<MousePositionInput>(input)
                .map_err(|e| format!("Failed to parse mouse position input: {}", e))?;

            // info! message from HEAD
            info!("Double click at ({}, {}) - no transformation applied", args.x, args.y);

            let inner_result = tokio::task::block_in_place(|| {
                let rt = tokio::runtime::Handle::current();
                rt.block_on(async {
                    commands::mouse::dev_double_click(app.clone(), state_manager, args.x, args.y, None).await
                })
            });
            inner_result.map_err(|e| format!("Error double clicking: {}", e))?;
            Ok(json!({"success": true}))
        }
    };
    provider.register_async_tool(double_click_def, double_click_exec).await;
    info!("Registered tool: double_click");

    // left_click_drag
    let left_click_drag_def = ToolDefinition {
        name: "left_click_drag".to_string(),
        description: "Perform a drag operation with the left mouse button from start coordinates to end coordinates.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "start_x": { "type": "number", "description": "The starting x coordinate." },
                "start_y": { "type": "number", "description": "The starting y coordinate." },
                "end_x": { "type": "number", "description": "The ending x coordinate." },
                "end_y": { "type": "number", "description": "The ending y coordinate." }
            },
            "required": ["start_x", "start_y", "end_x", "end_y"]
        }),
    };
    let app_handle_clone = app_handle.clone();
    let left_click_drag_exec = move |input: Value| {
        let app = app_handle_clone.clone();
        async move {
            let state_manager = app.state::<AppState>();
            let args = serde_json::from_value::<DragInput>(input)
                .map_err(|e| format!("Failed to parse drag input: {}", e))?;

            // info! message from HEAD
            info!("Click and drag from ({}, {}) to ({}, {}) - no transformation applied",
                args.start_x, args.start_y, args.end_x, args.end_y);

            let inner_result = tokio::task::block_in_place(|| {
                let rt = tokio::runtime::Handle::current();
                rt.block_on(async {
                    commands::mouse::dev_left_click_drag(
                        app.clone(),
                        state_manager,
                        args.start_x,
                        args.start_y,
                        args.end_x,
                        args.end_y
                    ).await
                })
            });
            inner_result.map_err(|e| format!("Error performing click and drag: {}", e))?;
            Ok(json!({"success": true}))
        }
    };
    provider.register_async_tool(left_click_drag_def, left_click_drag_exec).await;
    info!("Registered tool: left_click_drag");

    // cursor_position
    let cursor_position_def = ToolDefinition {
        name: "cursor_position".to_string(),
        description: "Get the current cursor position.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {},
            "required": []
        }),
    };
    let app_handle_clone = app_handle.clone();
    let cursor_position_exec = move |_input: Value| {
        let app = app_handle_clone.clone();
        async move {
            let state_manager = app.state::<AppState>();
            let result = tokio::task::block_in_place(|| {
                let rt = tokio::runtime::Handle::current();
                rt.block_on(async {
                    commands::mouse::dev_get_cursor_position(app.clone(), state_manager).await
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

    // window_management
    let window_list_def = ToolDefinition {
        name: "list_windows".to_string(),
        description: "Get a list of all open windows with their IDs, titles, and applications. Useful for targeting specific windows for screenshots or clicks.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {},
            "required": []
        }),
    };

    let app_handle_clone = app_handle.clone();
    let window_list_exec = move |_input: Value| {
        let app = app_handle_clone.clone();
        async move {
            let state_manager = app.state::<AppState>();
            match crate::commands::window::dev_get_window_list(app.clone(), state_manager).await {
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
    };

    let app_handle_clone = app_handle.clone();
    let window_info_exec = move |input: Value| {
        let app = app_handle_clone.clone();
        async move {
            let state_manager = app.state::<AppState>();
            let window_id = input["window_id"]
                .as_str()
                .ok_or_else(|| "Missing or invalid 'window_id' parameter".to_string())?;

            match crate::commands::window::dev_get_window_info(app.clone(), state_manager, window_id.to_string()).await {
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

    info!("Desktop tool registration completed.");
=======
>>>>>>> main
}

// Function to set up tools (wrapper for register_desktop_tools for backwards compatibility)
pub async fn setup_tools(
    provider: &mut LocalToolProvider,
    state: State<'_, AppState>,
    app_handle: tauri::AppHandle,
<<<<<<< HEAD
) -> Arc<tokio::sync::Mutex<LocalToolProvider>> {
    // Set up MCP manager in the tool provider
    let mcp_manager = state.get_mcp_manager().await;
    provider.set_mcp_manager(mcp_manager);

    // Register basic desktop tools
    register_desktop_tools(provider, state.clone(), app_handle.clone()).await;

    // Initialize MCP servers and refresh tools if needed
    if let Err(e) = state.initialize_mcp_servers().await {
        log::warn!("Failed to initialize MCP servers: {}", e);
    } else {
        log::info!("MCP servers initialized successfully");

        // Refresh MCP tools to include them in the provider
        if let Err(e) = provider.refresh_mcp_tools().await {
            log::warn!("Failed to refresh MCP tools: {}", e);
        } else {
            log::debug!("MCP tools refreshed and available in tool provider");
        }
    }

    // Create the Arc<Mutex<>> wrapper for the provider
    let provider_arc = std::sync::Arc::new(tokio::sync::Mutex::new(provider.clone()));

    // Register this tool provider with the AppState for future MCP refresh notifications
    state.register_tool_provider(provider_arc.clone());
    log::debug!("Tool provider registered with AppState for MCP refresh notifications");

    // Return the Arc so the caller can use the same instance that's registered
    provider_arc
=======
) {
    register_desktop_tools(provider, state, app_handle).await;
>>>>>>> main
}
