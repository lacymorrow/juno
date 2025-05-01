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
        description: "Get information about the currently focused UI element.".to_string(),
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
        description: "Captures a screenshot of the entire screen.".to_string(),
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
        description: "Captures a screenshot of the currently focused UI element.".to_string(),
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
        description: "Types the given text, optionally with a delay between characters.".to_string(),
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
        description: "Get the current contents of the clipboard.".to_string(),
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
        description: "Sets the system clipboard content.".to_string(),
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
                    commands::core::dev_set_clipboard(args.content, state_manager).await
                })
            });

            inner_result.map_err(|e| format!("Error setting clipboard content: {}", e))?;
            Ok(json!({"success": true}))
        }
    };
    provider.register_async_tool(set_clipboard_def, set_clipboard_exec).await;
    info!("Registered tool: set_clipboard_content");

    // --- Add missing tools from tools2 branch, adapted for async --- //

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

    // scroll
    #[derive(serde::Deserialize)]
    struct ScrollInput {
        x: f64,
        y: f64,
        direction: String,
        amount: i32, // Keep as i32 as in tools2
    }
    let scroll_def = ToolDefinition {
        name: "scroll".to_string(),
        description: "Scroll the screen in a specified direction by a specified amount at given coordinates.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "x": { "type": "number", "description": "The x coordinate to scroll at." },
                "y": { "type": "number", "description": "The y coordinate to scroll at." },
                "direction": { "type": "string", "description": "The direction to scroll: 'up', 'down', 'left', or 'right'." },
                "amount": { "type": "integer", "description": "The number of scroll wheel clicks." }
            },
            "required": ["x", "y", "direction", "amount"]
        }),
    };
    let app_handle_clone = app_handle.clone();
    let scroll_exec = move |input: Value| {
        let app = app_handle_clone.clone();
        async move {
            let state_manager = app.state::<AppState>();
            let args = serde_json::from_value::<ScrollInput>(input)
                .map_err(|e| format!("Failed to parse scroll input: {}", e))?;
            let inner_result = tokio::task::block_in_place(|| {
                let rt = tokio::runtime::Handle::current();
                rt.block_on(async {
                    commands::mouse::dev_scroll(app.clone(), state_manager, args.x, args.y, args.direction, args.amount as f64).await
                })
            });
            inner_result.map_err(|e| format!("Error scrolling: {}", e))?;
            Ok(json!({"success": true}))
        }
    };
    provider.register_async_tool(scroll_def, scroll_exec).await;
    info!("Registered tool: scroll");

    // triple_click
    let triple_click_def = ToolDefinition {
        name: "triple_click".to_string(),
        description: "Triple-click at specified coordinates.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "x": { "type": "number", "description": "The x coordinate to click at." },
                "y": { "type": "number", "description": "The y coordinate to click at." }
            },
            "required": ["x", "y"]
        }),
    };
    let app_handle_clone = app_handle.clone();
    let triple_click_exec = move |input: Value| {
        let app = app_handle_clone.clone();
        async move {
            let state_manager = app.state::<AppState>();
            let args = serde_json::from_value::<MousePositionInput>(input)
                .map_err(|e| format!("Failed to parse triple click input: {}", e))?;
            let inner_result = tokio::task::block_in_place(|| {
                let rt = tokio::runtime::Handle::current();
                rt.block_on(async {
                    commands::mouse::dev_triple_click(app.clone(), state_manager, args.x, args.y).await
                })
            });
            inner_result.map_err(|e| format!("Error triple clicking: {}", e))?;
            Ok(json!({"success": true}))
        }
    };
    provider.register_async_tool(triple_click_def, triple_click_exec).await;
    info!("Registered tool: triple_click");

    // wait
    #[derive(serde::Deserialize)]
    struct WaitInput { duration: f64 }
    let wait_def = ToolDefinition {
        name: "wait".to_string(),
        description: "Wait for a specified duration in seconds.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "duration": { "type": "number", "description": "The duration to wait in seconds." }
            },
            "required": ["duration"]
        }),
    };
    let app_handle_clone = app_handle.clone();
    let wait_exec = move |input: Value| {
        let app = app_handle_clone.clone();
        async move {
            let state_manager = app.state::<AppState>();
            let args = serde_json::from_value::<WaitInput>(input)
                .map_err(|e| format!("Failed to parse wait input: {}", e))?;
            let inner_result = tokio::task::block_in_place(|| {
                let rt = tokio::runtime::Handle::current();
                rt.block_on(async {
                    commands::core::dev_wait(args.duration, state_manager).await
                })
            });
            inner_result.map_err(|e| format!("Error during wait: {}", e))?;
            Ok(json!({"success": true}))
        }
    };
    provider.register_async_tool(wait_def, wait_exec).await;
    info!("Registered tool: wait");

    // hold_key (Separate Hold)
    #[derive(serde::Deserialize)]
    struct KeyInput { key: String }
    let hold_key_def = ToolDefinition {
        name: "hold_key".to_string(),
        description: "Presses and holds a specific key (e.g., 'Shift', 'Cmd'). Use 'release_key' to release it.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "key": { "type": "string", "description": "The key to hold (e.g., 'Shift', 'Cmd', 'A')." }
            },
            "required": ["key"]
        }),
    };
    let app_handle_clone = app_handle.clone();
    let hold_key_exec = move |input: Value| {
        let app = app_handle_clone.clone();
        async move {
            let state_manager = app.state::<AppState>();
            let args = serde_json::from_value::<KeyInput>(input)
                .map_err(|e| format!("Failed to parse hold key input: {}", e))?;
            let inner_result = tokio::task::block_in_place(|| {
                let rt = tokio::runtime::Handle::current();
                rt.block_on(async {
                    commands::keyboard::dev_hold_key(args.key, state_manager).await
                })
            });
            inner_result.map_err(|e| format!("Error holding key: {}", e))?;
            Ok(json!({"success": true}))
        }
    };
    provider.register_async_tool(hold_key_def, hold_key_exec).await;
    info!("Registered tool: hold_key");

    // release_key (Separate Release)
    let release_key_def = ToolDefinition {
        name: "release_key".to_string(),
        description: "Releases a previously held key.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "key": { "type": "string", "description": "The key to release (e.g., 'Shift', 'Cmd', 'A')." }
            },
            "required": ["key"]
        }),
    };
    let app_handle_clone = app_handle.clone();
    let release_key_exec = move |input: Value| {
        let app = app_handle_clone.clone();
        async move {
            let state_manager = app.state::<AppState>();
            let args = serde_json::from_value::<KeyInput>(input)
                .map_err(|e| format!("Failed to parse release key input: {}", e))?;
            let inner_result = tokio::task::block_in_place(|| {
                let rt = tokio::runtime::Handle::current();
                rt.block_on(async {
                    commands::keyboard::dev_release_key(args.key, state_manager).await
                })
            });
            inner_result.map_err(|e| format!("Error releasing key: {}", e))?;
            Ok(json!({"success": true}))
        }
    };
    provider.register_async_tool(release_key_def, release_key_exec).await;
    info!("Registered tool: release_key");

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

    // left_mouse_down
    let left_mouse_down_def = ToolDefinition {
        name: "left_mouse_down".to_string(),
        description: "Press the left mouse button down at the specified coordinates.".to_string(),
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
    let left_mouse_down_exec = move |input: Value| {
        let app = app_handle_clone.clone();
        async move {
            let state_manager = app.state::<AppState>();
            let args = serde_json::from_value::<MousePositionInput>(input)
                .map_err(|e| format!("Failed to parse mouse position input: {}", e))?;
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

    // left_mouse_up
    let left_mouse_up_def = ToolDefinition {
        name: "left_mouse_up".to_string(),
        description: "Release the left mouse button at the specified coordinates.".to_string(),
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
    let left_mouse_up_exec = move |input: Value| {
        let app = app_handle_clone.clone();
        async move {
            let state_manager = app.state::<AppState>();
            let args = serde_json::from_value::<MousePositionInput>(input)
                .map_err(|e| format!("Failed to parse mouse position input: {}", e))?;
            let inner_result = tokio::task::block_in_place(|| {
                let rt = tokio::runtime::Handle::current();
                rt.block_on(async {
                    commands::mouse::dev_left_mouse_up(app.clone(), state_manager, args.x, args.y).await
                })
            });
            inner_result.map_err(|e| format!("Error releasing left mouse: {}", e))?;
            Ok(json!({"success": true}))
        }
    };
    provider.register_async_tool(left_mouse_up_def, left_mouse_up_exec).await;
    info!("Registered tool: left_mouse_up");

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
            let inner_result = tokio::task::block_in_place(|| {
                let rt = tokio::runtime::Handle::current();
                rt.block_on(async {
                    commands::mouse::dev_left_click(app.clone(), state_manager, args.x, args.y).await
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
            let inner_result = tokio::task::block_in_place(|| {
                let rt = tokio::runtime::Handle::current();
                rt.block_on(async {
                    commands::mouse::dev_right_click(app.clone(), state_manager, args.x, args.y).await
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
            let inner_result = tokio::task::block_in_place(|| {
                let rt = tokio::runtime::Handle::current();
                rt.block_on(async {
                    commands::mouse::dev_middle_click(app.clone(), state_manager, args.x, args.y).await
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
            let inner_result = tokio::task::block_in_place(|| {
                let rt = tokio::runtime::Handle::current();
                rt.block_on(async {
                    commands::mouse::dev_double_click(app.clone(), state_manager, args.x, args.y).await
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
            Ok(json!({ "x": x, "y": y }))
        }
    };
    provider.register_async_tool(cursor_position_def, cursor_position_exec).await;
    info!("Registered tool: cursor_position");

    info!("Desktop tool registration completed.");
}
