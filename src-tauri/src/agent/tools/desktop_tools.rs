use crate::agent::implementations::tool_provider::LocalToolProvider;
use crate::agent::structs::ToolDefinition;
use crate::state::AppState;
use crate::commands;
use crate::utils::coordinates; // Import the coordinates module
use tauri::{AppHandle, State, Manager};
use serde_json::{Value, json};
use tracing::info;

// Import missing functions that are registered at the crate root
use crate::{
    capture_screenshot_command,
    dev_get_clipboard,
    dev_set_clipboard,
};

// Function to register all desktop tools with the tool provider
pub async fn register_desktop_tools(
    provider: &mut LocalToolProvider,
    app_handle: AppHandle,
    _state: State<'_, AppState>, // Not using the passed state directly
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
    let get_focused_exec = move |_input: Value| -> Result<Value, String> {
        let app_handle = app_handle_clone.clone();
        let managed_state = app_handle.state::<AppState>();

        let result = tokio::task::block_in_place(|| {
            let rt = tokio::runtime::Handle::current();
            rt.block_on(async {
                let app_handle_for_async = app_handle.clone();
                commands::element::dev_get_focused_element_info(app_handle_for_async, managed_state)
                    .await
                    .map_err(|e| format!("Error getting focused element: {}", e))
            })
        })?;

        Ok(json!(result))
    };
    provider.register_tool(get_focused_def, get_focused_exec).await;
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
    let capture_screenshot_exec = move |_input: Value| -> Result<Value, String> {
        let app_handle = app_handle_clone.clone();

        let result = tokio::task::block_in_place(|| {
            let rt = tokio::runtime::Handle::current();
            rt.block_on(async {
                let app_handle_for_async = app_handle.clone();
                capture_screenshot_command(app_handle_for_async)
                    .await
                    .map_err(|e| format!("Error capturing screenshot: {}", e))
            })
        })?;

        Ok(json!(result))
    };
    provider.register_tool(capture_screenshot_def, capture_screenshot_exec).await;
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
    let capture_element_screenshot_exec = move |_input: Value| -> Result<Value, String> {
        let app_handle = app_handle_clone.clone();
        let managed_state = app_handle.state::<AppState>();

        let result = tokio::task::block_in_place(|| {
            let rt = tokio::runtime::Handle::current();
            rt.block_on(async {
                let app_handle_for_async = app_handle.clone();
                commands::element::capture_element_screenshot_command(app_handle_for_async, managed_state)
                    .await
                    .map_err(|e| format!("Error capturing element screenshot: {}", e))
            })
        })?;

        Ok(json!(result))
    };
    provider.register_tool(capture_element_screenshot_def, capture_element_screenshot_exec).await;
    info!("Registered tool: capture_element_screenshot");

    // type_text
    #[derive(serde::Deserialize)]
    struct TypeTextInput { text: String }

    let type_text_def = ToolDefinition {
        name: "type_text".to_string(),
        description: "Types the given text into the currently focused element.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "text": { "type": "string", "description": "The text to type." }
            },
            "required": ["text"]
        }),
    };

    let app_handle_clone = app_handle.clone();
    let type_text_exec = move |input: Value| -> Result<Value, String> {
        let app_handle = app_handle_clone.clone();
        let managed_state = app_handle.state::<AppState>();

        let args = serde_json::from_value::<TypeTextInput>(input)
            .map_err(|e| format!("Failed to parse type_text input: {}", e))?;

        let result = tokio::task::block_in_place(|| {
            let rt = tokio::runtime::Handle::current();
            rt.block_on(async {
                let app_handle_for_async = app_handle.clone();
                commands::keyboard::dev_type_text(app_handle_for_async, managed_state, args.text)
                    .await
                    .map_err(|e| format!("Error typing text: {}", e))
            })
        })?;

        Ok(json!(result))
    };
    provider.register_tool(type_text_def, type_text_exec).await;
    info!("Registered tool: type_text");

    // get_clipboard
    let get_clipboard_def = ToolDefinition {
        name: "get_clipboard".to_string(),
        description: "Gets the current content of the clipboard.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {},
            "required": []
        }),
    };

    let app_handle_clone = app_handle.clone();
    let get_clipboard_exec = move |_input: Value| -> Result<Value, String> {
        let app_handle = app_handle_clone.clone();
        let managed_state = app_handle.state::<AppState>();

        let result = tokio::task::block_in_place(|| {
            let rt = tokio::runtime::Handle::current();
            rt.block_on(async {
                dev_get_clipboard(managed_state)
                    .await
                    .map_err(|e| format!("Error getting clipboard: {}", e))
            })
        })?;

        Ok(json!(result))
    };
    provider.register_tool(get_clipboard_def, get_clipboard_exec).await;
    info!("Registered tool: get_clipboard");

    // set_clipboard
    #[derive(serde::Deserialize)]
    struct SetClipboardInput {
        text: String
    }

    let set_clipboard_def = ToolDefinition {
        name: "set_clipboard".to_string(),
        description: "Sets the content of the clipboard.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "text": { "type": "string", "description": "The text to place on the clipboard." }
            },
            "required": ["text"]
        }),
    };

    let app_handle_clone = app_handle.clone();
    let set_clipboard_exec = move |input: Value| -> Result<Value, String> {
        let app_handle = app_handle_clone.clone();
        let managed_state = app_handle.state::<AppState>();

        let args = serde_json::from_value::<SetClipboardInput>(input)
            .map_err(|e| format!("Failed to parse set_clipboard input: {}", e))?;

        let result = tokio::task::block_in_place(|| {
            let rt = tokio::runtime::Handle::current();
            rt.block_on(async {
                dev_set_clipboard(args.text, managed_state)
                    .await
                    .map_err(|e| format!("Error setting clipboard: {}", e))
            })
        })?;

        Ok(json!(result))
    };
    provider.register_tool(set_clipboard_def, set_clipboard_exec).await;
    info!("Registered tool: set_clipboard");

    // Add new computer use tools based on the Anthropic documentation
    register_additional_computer_use_tools(provider, app_handle.clone()).await;

    info!("Desktop tool registration completed with core tools and additional computer use tools.");
}

// Register additional computer use tools for Anthropic's specs
async fn register_additional_computer_use_tools(
    provider: &mut LocalToolProvider,
    app_handle: AppHandle
) {
    // scroll
    #[derive(serde::Deserialize)]
    struct ScrollInput {
        x: f64,
        y: f64,
        direction: String,
        amount: i32,
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
    let scroll_exec = move |input: Value| -> Result<Value, String> {
        let app_handle = app_handle_clone.clone();
        let managed_state = app_handle.state::<AppState>();

        let args = serde_json::from_value::<ScrollInput>(input)
            .map_err(|e| format!("Failed to parse scroll input: {}", e))?;

        // Use a blocking task to handle the async operation
        let _result = tokio::task::block_in_place(|| {
            let rt = tokio::runtime::Handle::current();
            rt.block_on(async {
                #[cfg(target_os = "macos")]
                {
                    // Convert direction to expected format
                    let scroll_direction = match args.direction.to_lowercase().as_str() {
                        "up" => "up",
                        "down" => "down",
                        "left" => "left",
                        "right" => "right",
                        _ => return Err("Invalid scroll direction. Use 'up', 'down', 'left', or 'right'.".to_string())
                    };

                    // Call the desktop scroll function
                    managed_state.desktop.scroll_at_position(args.x, args.y, scroll_direction, args.amount as f64)
                        .map_err(|e| format!("Error scrolling: {}", e))
                }

                #[cfg(not(target_os = "macos"))]
                {
                    Err("Scrolling is only supported on macOS currently.".to_string())
                }
            })
        })?;

        Ok(json!({"success": true}))
    };
    provider.register_tool(scroll_def, scroll_exec).await;
    info!("Registered tool: scroll");

    // triple_click
    #[derive(serde::Deserialize)]
    struct TripleClickInput {
        x: f64,
        y: f64,
    }

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
    let triple_click_exec = move |input: Value| -> Result<Value, String> {
        let app_handle = app_handle_clone.clone();
        let managed_state = app_handle.state::<AppState>();

        let args = serde_json::from_value::<TripleClickInput>(input)
            .map_err(|e| format!("Failed to parse triple click input: {}", e))?;

        // Transform coordinates from scaled to original
        let (original_x, original_y) = coordinates::transform_to_screen_coordinates(args.x, args.y);
        info!("Triple click: transforming from ({}, {}) to ({}, {})", args.x, args.y, original_x, original_y);

        // Use a blocking task to handle the async operation
        let _result = tokio::task::block_in_place(|| {
            let rt = tokio::runtime::Handle::current();
            rt.block_on(async {
                let app_handle_for_async = app_handle.clone();
                commands::mouse::dev_triple_click(app_handle_for_async, managed_state, original_x, original_y)
                    .await
                    .map_err(|e| format!("Error triple clicking: {}", e))
            })
        })?;

        Ok(json!({"success": true}))
    };
    provider.register_tool(triple_click_def, triple_click_exec).await;
    info!("Registered tool: triple_click");

    // wait
    #[derive(serde::Deserialize)]
    struct WaitInput {
        duration: f64,
    }

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
    let wait_exec = move |input: Value| -> Result<Value, String> {
        let managed_state = app_handle_clone.state::<AppState>();

        let args = serde_json::from_value::<WaitInput>(input)
            .map_err(|e| format!("Failed to parse wait input: {}", e))?;

        // Use a blocking task to handle the async operation
        let _result = tokio::task::block_in_place(|| {
            let rt = tokio::runtime::Handle::current();
            rt.block_on(async {
                commands::core::dev_wait(args.duration, managed_state)
                    .await
                    .map_err(|e| format!("Error during wait: {}", e))
            })
        })?;

        Ok(json!({"success": true}))
    };
    provider.register_tool(wait_def, wait_exec).await;
    info!("Registered tool: wait");

    // hold_key
    #[derive(serde::Deserialize)]
    struct HoldKeyInput {
        key: String,
        duration: Option<f64>,
    }

    let hold_key_def = ToolDefinition {
        name: "hold_key".to_string(),
        description: "Hold down a key or key-combination for a specified duration.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "key": { "type": "string", "description": "The key to hold down." },
                "duration": { "type": "number", "description": "The duration to hold the key in seconds." }
            },
            "required": ["key"]
        }),
    };

    let app_handle_clone = app_handle.clone();
    let hold_key_exec = move |input: Value| -> Result<Value, String> {
        let app_handle = app_handle_clone.clone();
        let managed_state = app_handle.state::<AppState>();

        let args = serde_json::from_value::<HoldKeyInput>(input)
            .map_err(|e| format!("Failed to parse hold key input: {}", e))?;

        // Default duration to 1 second if not provided
        let duration = args.duration.unwrap_or(1.0);

        // Use a blocking task to handle the async operation
        let _result = tokio::task::block_in_place(|| {
            let rt = tokio::runtime::Handle::current();
            rt.block_on(async {
                // Hold key
                commands::keyboard::dev_hold_key(args.key.clone(), managed_state.clone())
                    .await
                    .map_err(|e| format!("Error holding key: {}", e))?;

                // Wait for specified duration
                commands::core::dev_wait(duration, managed_state.clone())
                    .await
                    .map_err(|e| format!("Error during wait after hold key: {}", e))?;

                // Release key
                commands::keyboard::dev_release_key(args.key, managed_state)
                    .await
                    .map_err(|e| format!("Error releasing key: {}", e))
            })
        })?;

        Ok(json!({"success": true}))
    };
    provider.register_tool(hold_key_def, hold_key_exec).await;
    info!("Registered tool: hold_key");

    // Define common input structs
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
    let mouse_move_exec = move |input: Value| -> Result<Value, String> {
        let app_handle = app_handle_clone.clone();
        let managed_state = app_handle.state::<AppState>();

        let args = serde_json::from_value::<MousePositionInput>(input)
            .map_err(|e| format!("Failed to parse mouse position input: {}", e))?;

        // Transform coordinates from scaled to original
        let (original_x, original_y) = coordinates::transform_to_screen_coordinates(args.x, args.y);
        info!("Mouse move: transforming from ({}, {}) to ({}, {})", args.x, args.y, original_x, original_y);

        // Use a blocking task to handle the async operation
        let _result = tokio::task::block_in_place(|| {
            let rt = tokio::runtime::Handle::current();
            rt.block_on(async {
                let app_handle_for_async = app_handle.clone();
                commands::mouse::dev_mouse_move(app_handle_for_async, managed_state, original_x, original_y)
                    .await
                    .map_err(|e| format!("Error moving mouse: {}", e))
            })
        })?;

        Ok(json!({"success": true}))
    };
    provider.register_tool(mouse_move_def, mouse_move_exec).await;
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
    let left_mouse_down_exec = move |input: Value| -> Result<Value, String> {
        let app_handle = app_handle_clone.clone();
        let managed_state = app_handle.state::<AppState>();

        let args = serde_json::from_value::<MousePositionInput>(input)
            .map_err(|e| format!("Failed to parse mouse position input: {}", e))?;

        // Transform coordinates from scaled to original
        let (original_x, original_y) = coordinates::transform_to_screen_coordinates(args.x, args.y);
        info!("Left mouse down: transforming from ({}, {}) to ({}, {})", args.x, args.y, original_x, original_y);

        // Use a blocking task to handle the async operation
        let _result = tokio::task::block_in_place(|| {
            let rt = tokio::runtime::Handle::current();
            rt.block_on(async {
                let app_handle_for_async = app_handle.clone();
                commands::mouse::dev_left_mouse_down(app_handle_for_async, managed_state, original_x, original_y)
                    .await
                    .map_err(|e| format!("Error pressing left mouse down: {}", e))
            })
        })?;

        Ok(json!({"success": true}))
    };
    provider.register_tool(left_mouse_down_def, left_mouse_down_exec).await;
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
    let left_mouse_up_exec = move |input: Value| -> Result<Value, String> {
        let app_handle = app_handle_clone.clone();
        let managed_state = app_handle.state::<AppState>();

        let args = serde_json::from_value::<MousePositionInput>(input)
            .map_err(|e| format!("Failed to parse mouse position input: {}", e))?;

        // Transform coordinates from scaled to original
        let (original_x, original_y) = coordinates::transform_to_screen_coordinates(args.x, args.y);
        info!("Left mouse up: transforming from ({}, {}) to ({}, {})", args.x, args.y, original_x, original_y);

        // Use a blocking task to handle the async operation
        let _result = tokio::task::block_in_place(|| {
            let rt = tokio::runtime::Handle::current();
            rt.block_on(async {
                let app_handle_for_async = app_handle.clone();
                commands::mouse::dev_left_mouse_up(app_handle_for_async, managed_state, original_x, original_y)
                    .await
                    .map_err(|e| format!("Error releasing left mouse: {}", e))
            })
        })?;

        Ok(json!({"success": true}))
    };
    provider.register_tool(left_mouse_up_def, left_mouse_up_exec).await;
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
    let left_click_exec = move |input: Value| -> Result<Value, String> {
        let app_handle = app_handle_clone.clone();
        let managed_state = app_handle.state::<AppState>();

        let args = serde_json::from_value::<MousePositionInput>(input)
            .map_err(|e| format!("Failed to parse mouse position input: {}", e))?;

        // Transform coordinates from scaled to original
        let (original_x, original_y) = coordinates::transform_to_screen_coordinates(args.x, args.y);
        info!("Left click: transforming from ({}, {}) to ({}, {})", args.x, args.y, original_x, original_y);

        // Use a blocking task to handle the async operation
        let _result = tokio::task::block_in_place(|| {
            let rt = tokio::runtime::Handle::current();
            rt.block_on(async {
                let app_handle_for_async = app_handle.clone();
                commands::mouse::dev_left_click(app_handle_for_async, managed_state, original_x, original_y)
                    .await
                    .map_err(|e| format!("Error left clicking: {}", e))
            })
        })?;

        Ok(json!({"success": true}))
    };
    provider.register_tool(left_click_def, left_click_exec).await;
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
    let right_click_exec = move |input: Value| -> Result<Value, String> {
        let app_handle = app_handle_clone.clone();
        let managed_state = app_handle.state::<AppState>();

        let args = serde_json::from_value::<MousePositionInput>(input)
            .map_err(|e| format!("Failed to parse mouse position input: {}", e))?;

        // Transform coordinates from scaled to original
        let (original_x, original_y) = coordinates::transform_to_screen_coordinates(args.x, args.y);
        info!("Right click: transforming from ({}, {}) to ({}, {})", args.x, args.y, original_x, original_y);

        // Use a blocking task to handle the async operation
        let _result = tokio::task::block_in_place(|| {
            let rt = tokio::runtime::Handle::current();
            rt.block_on(async {
                let app_handle_for_async = app_handle.clone();
                commands::mouse::dev_right_click(app_handle_for_async, managed_state, original_x, original_y)
                    .await
                    .map_err(|e| format!("Error right clicking: {}", e))
            })
        })?;

        Ok(json!({"success": true}))
    };
    provider.register_tool(right_click_def, right_click_exec).await;
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
    let middle_click_exec = move |input: Value| -> Result<Value, String> {
        let app_handle = app_handle_clone.clone();
        let managed_state = app_handle.state::<AppState>();

        let args = serde_json::from_value::<MousePositionInput>(input)
            .map_err(|e| format!("Failed to parse mouse position input: {}", e))?;

        // Transform coordinates from scaled to original
        let (original_x, original_y) = coordinates::transform_to_screen_coordinates(args.x, args.y);
        info!("Middle click: transforming from ({}, {}) to ({}, {})", args.x, args.y, original_x, original_y);

        // Use a blocking task to handle the async operation
        let _result = tokio::task::block_in_place(|| {
            let rt = tokio::runtime::Handle::current();
            rt.block_on(async {
                let app_handle_for_async = app_handle.clone();
                commands::mouse::dev_middle_click(app_handle_for_async, managed_state, original_x, original_y)
                    .await
                    .map_err(|e| format!("Error middle clicking: {}", e))
            })
        })?;

        Ok(json!({"success": true}))
    };
    provider.register_tool(middle_click_def, middle_click_exec).await;
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
    let double_click_exec = move |input: Value| -> Result<Value, String> {
        let app_handle = app_handle_clone.clone();
        let managed_state = app_handle.state::<AppState>();

        let args = serde_json::from_value::<MousePositionInput>(input)
            .map_err(|e| format!("Failed to parse mouse position input: {}", e))?;

        // Transform coordinates from scaled to original
        let (original_x, original_y) = coordinates::transform_to_screen_coordinates(args.x, args.y);
        info!("Double click: transforming from ({}, {}) to ({}, {})", args.x, args.y, original_x, original_y);

        // Use a blocking task to handle the async operation
        let _result = tokio::task::block_in_place(|| {
            let rt = tokio::runtime::Handle::current();
            rt.block_on(async {
                let app_handle_for_async = app_handle.clone();
                commands::mouse::dev_double_click(app_handle_for_async, managed_state, original_x, original_y)
                    .await
                    .map_err(|e| format!("Error double clicking: {}", e))
            })
        })?;

        Ok(json!({"success": true}))
    };
    provider.register_tool(double_click_def, double_click_exec).await;
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
    let left_click_drag_exec = move |input: Value| -> Result<Value, String> {
        let app_handle = app_handle_clone.clone();
        let managed_state = app_handle.state::<AppState>();

        let args = serde_json::from_value::<DragInput>(input)
            .map_err(|e| format!("Failed to parse drag input: {}", e))?;

        // Transform coordinates from scaled to original
        let (original_start_x, original_start_y) = coordinates::transform_to_screen_coordinates(args.start_x, args.start_y);
        let (original_end_x, original_end_y) = coordinates::transform_to_screen_coordinates(args.end_x, args.end_y);

        info!("Click and drag: transforming from ({}, {}) -> ({}, {}) to ({}, {}) -> ({}, {})",
            args.start_x, args.start_y, args.end_x, args.end_y,
            original_start_x, original_start_y, original_end_x, original_end_y);

        // Use a blocking task to handle the async operation
        let _result = tokio::task::block_in_place(|| {
            let rt = tokio::runtime::Handle::current();
            rt.block_on(async {
                let app_handle_for_async = app_handle.clone();
                commands::mouse::dev_left_click_drag(
                    app_handle_for_async,
                    managed_state,
                    original_start_x,
                    original_start_y,
                    original_end_x,
                    original_end_y
                )
                .await
                .map_err(|e| format!("Error performing click and drag: {}", e))
            })
        })?;

        Ok(json!({"success": true}))
    };
    provider.register_tool(left_click_drag_def, left_click_drag_exec).await;
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
    let cursor_position_exec = move |_input: Value| -> Result<Value, String> {
        let app_handle = app_handle_clone.clone();
        let managed_state = app_handle.state::<AppState>();

        // Use a blocking task to handle the async operation
        let result = tokio::task::block_in_place(|| {
            let rt = tokio::runtime::Handle::current();
            rt.block_on(async {
                let app_handle_for_async = app_handle.clone();
                commands::mouse::dev_get_cursor_position(app_handle_for_async, managed_state)
                    .await
                    .map_err(|e| format!("Error getting cursor position: {}", e))
            })
        })?;

        // Transform the cursor coordinates from screen space to scaled space so they match the screenshot
        let (x, y) = result;
        let (scaled_x, scaled_y) = coordinates::transform_to_scaled_coordinates(x, y);

        info!("Cursor position: transforming from screen ({}, {}) to scaled ({}, {})", x, y, scaled_x, scaled_y);

        Ok(json!({
            "x": scaled_x,
            "y": scaled_y
        }))
    };
    provider.register_tool(cursor_position_def, cursor_position_exec).await;
    info!("Registered tool: cursor_position");
}
