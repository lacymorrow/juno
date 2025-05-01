use crate::agent::implementations::tool_provider::LocalToolProvider;
use crate::agent::structs::ToolDefinition;
use crate::state::AppState;
use crate::commands;
use crate::utils::coordinates; // Import the coordinates module
use tauri::{AppHandle, State, Manager};
use serde_json::{Value, json};
use tracing::info;
use std::fs;
use std::process::Command;
use std::io::Write; // Import the Write trait
use std::sync::Arc;

// Import missing functions that are registered at the crate root
use crate::{
    capture_screenshot_command,
    dev_get_clipboard,
    dev_set_clipboard,
};

// Ensure all necessary command modules are imported
use crate::commands::{core, element, keyboard, mouse};

// Function to register all desktop tools with the tool provider
pub async fn register_desktop_tools(
    provider: &mut LocalToolProvider,
    app_handle: tauri::AppHandle,
    state: State<'_, AppState>,
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
        let app_handle = app_handle_clone.clone();
         async move {
             let result = tokio::task::block_in_place(|| {
                 let rt = tokio::runtime::Handle::current();
                 rt.block_on(async {
                     crate::capture_screenshot_command(app_handle)
                        .await
                 })
             }).map_err(|e| format!("Error capturing screenshot: {}", e))?;
            Ok(json!(result))
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
                    commands::keyboard::dev_type_text(app.clone(), state_manager, args.text)
                        .await
                })
            });
            inner_result.map_err(|e| format!("Error typing text: {}", e))?;
            Ok(json!({"success": true}))
        }
    };
    provider.register_async_tool(type_text_def, type_text_exec).await;
    info!("Registered tool: type_text");

    // --- Register Get Clipboard Tool ---
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

    // set_clipboard_content
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
            let args = serde_json::from_value::<SetClipboardContentInput>(input)
                .map_err(|e| format!("Failed to parse set_clipboard_content input: {}", e))?;
            let state_manager = app.state::<AppState>();
            match commands::core::dev_set_clipboard(state_manager, args.content).await {
                Ok(_) => Ok(json!({"success": true})),
                Err(e) => Err(format!("Error setting clipboard content: {}", e))
            }
        }
    };
    provider.register_async_tool(set_clipboard_def, set_clipboard_exec).await;
    info!("Registered tool: set_clipboard_content");

    // --- Mouse Tools ---

    // Define input structs for mouse actions
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

    #[derive(serde::Deserialize)]
    struct ScrollInput {
        x: f64,
        y: f64,
        direction: String,
        amount: i32, // Keep as i32 as in tools2
    }

    // mouse_move
    let mouse_move_def = ToolDefinition {
        name: "mouse_move".to_string(),
        description: "Moves the mouse cursor to the specified screen coordinates (x, y).".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "x": { "type": "number", "description": "The horizontal coordinate." },
                "y": { "type": "number", "description": "The vertical coordinate." }
            },
            "required": ["x", "y"]
        }),
    };
    let mouse_move_exec = move |input: Value| {
        let app_handle_clone = app_handle.clone(); // Clone app_handle for this closure
        async move {
            let args = serde_json::from_value::<MousePositionInput>(input)
                .map_err(|e| format!("Failed to parse mouse_move input: {}", e))?;
            let state_manager = app_handle_clone.state::<AppState>(); // Get state inside closure
            let screen_coords = coordinates::window_to_screen_coords(app_handle_clone.clone(), args.x, args.y)?;
            match commands::mouse::dev_mouse_move(state_manager, screen_coords.x as i32, screen_coords.y as i32).await {
                Ok(_) => Ok(json!({ "success": true })),
                Err(e) => Err(format!("Error moving mouse: {}", e)),
            }
        }
    };
    provider.register_async_tool(mouse_move_def, mouse_move_exec).await;
    info!("Registered tool: mouse_move");

    // mouse_click
    #[derive(serde::Deserialize)]
    struct MouseClickInput { button: String, click_type: String } // Added click_type
    let mouse_click_def = ToolDefinition {
        name: "mouse_click".to_string(),
        description: "Performs a mouse click (left, right, middle, double, triple) at the current cursor position.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "button": { "type": "string", "enum": ["left", "right", "middle"] },
                 "click_type": { "type": "string", "enum": ["single", "double", "triple"], "default": "single" }
            },
            "required": ["button"]
        }),
    };
    let mouse_click_exec = move |input: Value| {
        let app_handle_clone = app_handle.clone();
        async move {
            let args = serde_json::from_value::<MouseClickInput>(input)
                .map_err(|e| format!("Failed to parse mouse_click input: {}", e))?;
             let state_manager = app_handle_clone.state::<AppState>(); // Get state inside closure
            match commands::mouse::dev_mouse_click(state_manager, args.button, Some(args.click_type)).await { // Pass click_type
                Ok(_) => Ok(json!({ "success": true })),
                Err(e) => Err(format!("Error performing mouse click: {}", e)),
            }
        }
    };
    provider.register_async_tool(mouse_click_def, mouse_click_exec).await;
    info!("Registered tool: mouse_click");

    // mouse_drag
    let mouse_drag_def = ToolDefinition {
        name: "mouse_drag".to_string(),
        description: "Performs a mouse drag operation from start coordinates to end coordinates.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "start_x": { "type": "number" },
                "start_y": { "type": "number" },
                "end_x": { "type": "number" },
                "end_y": { "type": "number" }
            },
            "required": ["start_x", "start_y", "end_x", "end_y"]
        }),
    };
    let mouse_drag_exec = move |input: Value| {
        let app_handle_clone = app_handle.clone();
        async move {
            let args = serde_json::from_value::<DragInput>(input)
                .map_err(|e| format!("Failed to parse mouse_drag input: {}", e))?;
            let state_manager = app_handle_clone.state::<AppState>();
            // Convert window coordinates to screen coordinates for both start and end points
            let start_screen_coords = coordinates::window_to_screen_coords(app_handle_clone.clone(), args.start_x, args.start_y)?;
            let end_screen_coords = coordinates::window_to_screen_coords(app_handle_clone.clone(), args.end_x, args.end_y)?;
            match commands::mouse::dev_mouse_drag(
                state_manager,
                start_screen_coords.x as i32, start_screen_coords.y as i32,
                end_screen_coords.x as i32, end_screen_coords.y as i32,
                "left".to_string() // Assuming left button drag for now
            ).await {
                Ok(_) => Ok(json!({ "success": true })),
                Err(e) => Err(format!("Error performing mouse drag: {}", e)),
            }
        }
    };
    provider.register_async_tool(mouse_drag_def, mouse_drag_exec).await;
    info!("Registered tool: mouse_drag");

    // scroll_window (assuming scroll happens at current mouse position or center of window)
    // Note: `scroll_window` command in Rust seems to take direction (up/down) not coordinates/amount. Adapting tool.
    #[derive(serde::Deserialize)]
    struct ScrollWindowInput { direction: String }
    let scroll_window_def = ToolDefinition {
        name: "scroll_window".to_string(),
        description: "Scrolls the currently focused window up or down.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "direction": { "type": "string", "enum": ["up", "down"] }
            },
            "required": ["direction"]
        }),
    };
    let scroll_window_exec = move |input: Value| {
        let app_handle_clone = app_handle.clone();
        async move {
            let args = serde_json::from_value::<ScrollWindowInput>(input)
                .map_err(|e| format!("Failed to parse scroll_window input: {}", e))?;
            let state_manager = app_handle_clone.state::<AppState>();
            match commands::mouse::dev_scroll_window(state_manager, args.direction).await {
                Ok(_) => Ok(json!({ "success": true })),
                Err(e) => Err(format!("Error scrolling window: {}", e)),
            }
        }
    };
    provider.register_async_tool(scroll_window_def, scroll_window_exec).await;
    info!("Registered tool: scroll_window");

    // --- Other Desktop Tools ---

    // wait
    #[derive(serde::Deserialize)]
    struct WaitInput { duration: f64 } // duration in seconds

    let wait_def = ToolDefinition {
        name: "wait".to_string(),
        description: "Pauses execution for a specified duration.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "duration": { "type": "number", "description": "Duration to wait in seconds." }
            },
            "required": ["duration"]
        }),
    };

    let wait_exec = move |input: Value| {
        async move {
            let args = serde_json::from_value::<WaitInput>(input)
                .map_err(|e| format!("Failed to parse wait input: {}", e))?;
            let duration_ms = (args.duration * 1000.0) as u64; // Convert seconds to milliseconds
             match commands::core::dev_wait(duration_ms).await {
                 Ok(_) => Ok(json!({"success": true})),
                 Err(e) => Err(format!("Error waiting: {}", e))
             }
        }
    };
    provider.register_async_tool(wait_def, wait_exec).await;
    info!("Registered tool: wait");

    // --- Keyboard Tools ---

    // press_key
    #[derive(serde::Deserialize)]
    struct KeyInput { key: String }

    let press_key_def = ToolDefinition {
        name: "press_key".to_string(),
        description: "Simulates pressing a specific key or key combination (e.g., 'a', 'Enter', 'Cmd+S').".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "key": { "type": "string", "description": "The key or combination to press." }
            },
            "required": ["key"]
        }),
    };

    let press_key_exec = move |input: Value| {
        let app_handle_clone = app_handle.clone();
        async move {
            let args = serde_json::from_value::<KeyInput>(input)
                .map_err(|e| format!("Failed to parse press_key input: {}", e))?;
            let state_manager = app_handle_clone.state::<AppState>();
            match commands::keyboard::dev_press_key(state_manager, args.key).await {
                Ok(_) => Ok(json!({ "success": true })),
                Err(e) => Err(format!("Error pressing key: {}", e)),
            }
        }
    };
    provider.register_async_tool(press_key_def, press_key_exec).await;
    info!("Registered tool: press_key");

    // hold_key
     let hold_key_def = ToolDefinition {
        name: "hold_key".to_string(),
        description: "Simulates holding down a modifier key (e.g., 'Shift', 'Cmd', 'Ctrl', 'Alt').".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "key": { "type": "string", "description": "The modifier key to hold." }
            },
            "required": ["key"]
        }),
    };

    let hold_key_exec = move |input: Value| {
        let app_handle_clone = app_handle.clone();
        async move {
            let args = serde_json::from_value::<KeyInput>(input)
                .map_err(|e| format!("Failed to parse hold_key input: {}", e))?;
            let state_manager = app_handle_clone.state::<AppState>();
            match commands::keyboard::dev_hold_key(state_manager, args.key).await {
                Ok(_) => Ok(json!({ "success": true })),
                Err(e) => Err(format!("Error holding key: {}", e)),
            }
        }
    };
    provider.register_async_tool(hold_key_def, hold_key_exec).await;
    info!("Registered tool: hold_key");

    // release_key
    let release_key_def = ToolDefinition {
        name: "release_key".to_string(),
        description: "Simulates releasing a previously held modifier key.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "key": { "type": "string", "description": "The modifier key to release." }
            },
            "required": ["key"]
        }),
    };

    let release_key_exec = move |input: Value| {
        let app_handle_clone = app_handle.clone();
        async move {
            let args = serde_json::from_value::<KeyInput>(input)
                .map_err(|e| format!("Failed to parse release_key input: {}", e))?;
             let state_manager = app_handle_clone.state::<AppState>();
            match commands::keyboard::dev_release_key(state_manager, args.key).await {
                Ok(_) => Ok(json!({ "success": true })),
                Err(e) => Err(format!("Error releasing key: {}", e)),
            }
        }
    };
    provider.register_async_tool(release_key_def, release_key_exec).await;
    info!("Registered tool: release_key");

    // get_selected_text
    let get_selected_text_def = ToolDefinition {
        name: "get_selected_text".to_string(),
        description: "Retrieves the currently selected text in the active application.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {},
            "required": []
        }),
    };

    let get_selected_text_exec = move |_input: Value| {
         let app_handle_clone = app_handle.clone(); // Clone app_handle
        async move {
            let state_manager = app_handle_clone.state::<AppState>(); // Get state inside closure
             match commands::element::dev_get_selected_text(state_manager).await {
                 Ok(text) => Ok(json!({ "selected_text": text })),
                 Err(e) => Err(format!("Error getting selected text: {}", e)),
             }
        }
    };
    provider.register_async_tool(get_selected_text_def, get_selected_text_exec).await;
    info!("Registered tool: get_selected_text");

    // --- Window Management Tools ---

    // get_window_list
    let get_window_list_def = ToolDefinition {
        name: "get_window_list".to_string(),
        description: "Retrieves a list of currently open windows with their IDs, titles, and application names.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {},
            "required": []
        }),
    };

    let get_window_list_exec = move |_input: Value| {
        let app_handle_clone = app_handle.clone();
        async move {
             let state_manager = app_handle_clone.state::<AppState>(); // Get state inside closure
            match commands::window::dev_get_window_list(state_manager).await {
                Ok(list) => Ok(json!(list)), // Assuming list is already serializable
                Err(e) => Err(format!("Error getting window list: {}", e)),
            }
        }
    };
    provider.register_async_tool(get_window_list_def, get_window_list_exec).await;
    info!("Registered tool: get_window_list");

    // get_window_info
    #[derive(serde::Deserialize)]
    struct WindowIdInput { window_id: String }

    let get_window_info_def = ToolDefinition {
        name: "get_window_info".to_string(),
        description: "Retrieves detailed information about a specific window using its ID (e.g., position, size, title).".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "window_id": { "type": "string", "description": "The unique identifier of the window." }
            },
            "required": ["window_id"]
        }),
    };

    let get_window_info_exec = move |input: Value| {
        let app_handle_clone = app_handle.clone();
        async move {
            let args = serde_json::from_value::<WindowIdInput>(input)
                .map_err(|e| format!("Failed to parse get_window_info input: {}", e))?;
            let state_manager = app_handle_clone.state::<AppState>();
            match commands::window::dev_get_window_info(state_manager, args.window_id).await {
                Ok(info) => Ok(json!(info)), // Assuming info is already serializable
                Err(e) => Err(format!("Error getting window info: {}", e)),
            }
        }
    };
    provider.register_async_tool(get_window_info_def, get_window_info_exec).await;
    info!("Registered tool: get_window_info");

    // focus_window
    let focus_window_def = ToolDefinition {
        name: "focus_window".to_string(),
        description: "Brings a specific window to the foreground and makes it active.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "window_id": { "type": "string", "description": "The unique identifier of the window to focus." }
            },
            "required": ["window_id"]
        }),
    };

    let focus_window_exec = move |input: Value| {
        let app_handle_clone = app_handle.clone();
        async move {
            let args = serde_json::from_value::<WindowIdInput>(input)
                .map_err(|e| format!("Failed to parse focus_window input: {}", e))?;
            let state_manager = app_handle_clone.state::<AppState>();
             match commands::window::dev_focus_window(state_manager, args.window_id).await {
                 Ok(_) => Ok(json!({ "success": true })),
                 Err(e) => Err(format!("Error focusing window: {}", e)),
             }
        }
    };
    provider.register_async_tool(focus_window_def, focus_window_exec).await;
    info!("Registered tool: focus_window");

    // resize_window
    #[derive(serde::Deserialize)]
    struct ResizeWindowInput { window_id: String, width: i32, height: i32 }

    let resize_window_def = ToolDefinition {
        name: "resize_window".to_string(),
        description: "Resizes a specific window to the given width and height.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "window_id": { "type": "string", "description": "The ID of the window to resize." },
                "width": { "type": "integer", "description": "The desired width in pixels." },
                "height": { "type": "integer", "description": "The desired height in pixels." }
            },
            "required": ["window_id", "width", "height"]
        }),
    };

    let resize_window_exec = move |input: Value| {
        let app_handle_clone = app_handle.clone();
        async move {
            let args = serde_json::from_value::<ResizeWindowInput>(input)
                .map_err(|e| format!("Failed to parse resize_window input: {}", e))?;
            let state_manager = app_handle_clone.state::<AppState>();
            match commands::window::dev_resize_window(state_manager, args.window_id, args.width, args.height).await {
                Ok(_) => Ok(json!({ "success": true })),
                Err(e) => Err(format!("Error resizing window: {}", e)),
            }
        }
    };
    provider.register_async_tool(resize_window_def, resize_window_exec).await;
    info!("Registered tool: resize_window");

     // move_window
    #[derive(serde::Deserialize)]
    struct MoveWindowInput { window_id: String, x: i32, y: i32 }

    let move_window_def = ToolDefinition {
        name: "move_window".to_string(),
        description: "Moves a specific window to the given screen coordinates (top-left corner).".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "window_id": { "type": "string", "description": "The ID of the window to move." },
                "x": { "type": "integer", "description": "The desired X coordinate." },
                "y": { "type": "integer", "description": "The desired Y coordinate." }
            },
            "required": ["window_id", "x", "y"]
        }),
    };

    let move_window_exec = move |input: Value| {
        let app_handle_clone = app_handle.clone();
        async move {
            let args = serde_json::from_value::<MoveWindowInput>(input)
                .map_err(|e| format!("Failed to parse move_window input: {}", e))?;
            let state_manager = app_handle_clone.state::<AppState>();
             match commands::window::dev_move_window(state_manager, args.window_id, args.x, args.y).await {
                 Ok(_) => Ok(json!({ "success": true })),
                 Err(e) => Err(format!("Error moving window: {}", e)),
             }
        }
    };
    provider.register_async_tool(move_window_def, move_window_exec).await;
    info!("Registered tool: move_window");

    // close_window
    let close_window_def = ToolDefinition {
        name: "close_window".to_string(),
        description: "Closes a specific window using its ID.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "window_id": { "type": "string", "description": "The ID of the window to close." }
            },
            "required": ["window_id"]
        }),
    };

    let close_window_exec = move |input: Value| {
         let app_handle_clone = app_handle.clone();
        async move {
            let args = serde_json::from_value::<WindowIdInput>(input)
                .map_err(|e| format!("Failed to parse close_window input: {}", e))?;
             let state_manager = app_handle_clone.state::<AppState>();
            match commands::window::dev_close_window(state_manager, args.window_id).await {
                Ok(_) => Ok(json!({ "success": true })),
                Err(e) => Err(format!("Error closing window: {}", e)),
            }
        }
    };
    provider.register_async_tool(close_window_def, close_window_exec).await;
    info!("Registered tool: close_window");


    // --- File System Tools ---

    // list_files
     #[derive(serde::Deserialize)]
     struct PathInput { path: String }

     let list_files_def = ToolDefinition {
         name: "list_files".to_string(),
         description: "Lists the files and directories within a specified path.".to_string(),
         input_schema: json!({
             "type": "object",
             "properties": {
                 "path": { "type": "string", "description": "The directory path to list." }
             },
             "required": ["path"]
         }),
     };

     let list_files_exec = move |input: Value| {
         async move {
             let args = serde_json::from_value::<PathInput>(input)
                 .map_err(|e| format!("Failed to parse list_files input: {}", e))?;
             // Use the imported `dev_list_files` function directly
             match commands::filesystem::dev_list_files(args.path).await {
                 Ok(files) => Ok(json!(files)),
                 Err(e) => Err(format!("Error listing files: {}", e)),
             }
         }
     };
     provider.register_async_tool(list_files_def, list_files_exec).await;
     info!("Registered tool: list_files");

     // get_file_content
     let get_file_content_def = ToolDefinition {
         name: "get_file_content".to_string(),
         description: "Reads and returns the content of a specified file as a string.".to_string(),
         input_schema: json!({
             "type": "object",
             "properties": {
                 "path": { "type": "string", "description": "The path to the file to read." }
             },
             "required": ["path"]
         }),
     };

     let get_file_content_exec = move |input: Value| {
         async move {
             let args = serde_json::from_value::<PathInput>(input)
                 .map_err(|e| format!("Failed to parse get_file_content input: {}", e))?;
             // Use the imported `dev_get_file_content` function directly
             match commands::filesystem::dev_get_file_content(args.path).await {
                 Ok(content) => Ok(json!({ "content": content })),
                 Err(e) => Err(format!("Error getting file content: {}", e)),
             }
         }
     };
     provider.register_async_tool(get_file_content_def, get_file_content_exec).await;
     info!("Registered tool: get_file_content");

     // set_file_content
     #[derive(serde::Deserialize)]
     struct SetFileInput { path: String, content: String }

     let set_file_content_def = ToolDefinition {
         name: "set_file_content".to_string(),
         description: "Writes the provided string content to a specified file, overwriting it if it exists.".to_string(),
         input_schema: json!({
             "type": "object",
             "properties": {
                 "path": { "type": "string", "description": "The path to the file to write." },
                 "content": { "type": "string", "description": "The content to write to the file." }
             },
             "required": ["path", "content"]
         }),
     };

     let set_file_content_exec = move |input: Value| {
         async move {
             let args = serde_json::from_value::<SetFileInput>(input)
                 .map_err(|e| format!("Failed to parse set_file_content input: {}", e))?;
             // Use the imported `dev_set_file_content` function directly
             match commands::filesystem::dev_set_file_content(args.path, args.content).await {
                 Ok(_) => Ok(json!({ "success": true })),
                 Err(e) => Err(format!("Error setting file content: {}", e)),
             }
         }
     };
     provider.register_async_tool(set_file_content_def, set_file_content_exec).await;
     info!("Registered tool: set_file_content");

    info!("Finished registering desktop tools.");
}
