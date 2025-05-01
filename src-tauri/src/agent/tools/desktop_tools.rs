use crate::agent::implementations::tool_provider::LocalToolProvider;
use crate::agent::structs::ToolDefinition;
use crate::state::AppState;
use crate::commands;
// use crate::utils::coordinates; // Keep commented if still unused
use tauri::{State, Manager};
use serde_json::{Value, json};
use serde::Deserialize; // Import Deserialize
use tracing::{info, log}; // Added log for error logging

// Input struct for the consolidated 'computer' tool, based on Anthropic computer_20250124 schema
#[derive(Deserialize, Debug)]
struct ComputerActionInput {
    action: String, // Enum would be safer but string matches schema examples directly
    coordinate: Option<Vec<f64>>, // [x, y]
    duration: Option<f64>, // Changed from integer in schema example to f64 to match dev_wait and allow fractional seconds
    scroll_amount: Option<i32>, // Changed from integer to i32 to match ScrollInput
    scroll_direction: Option<String>,
    start_coordinate: Option<Vec<f64>>, // [x, y]
    text: Option<String>,
    // Anthropic schema uses 'key'/'hold_key'/'type' actions with 'text' param.
    // Our commands map:
    // action=key -> dev_press_key(text)
    // action=hold_key -> dev_hold_key(text, duration)
    // action=type -> dev_type_text(text)
    // action=release_key -> dev_release_key(text) - Added this action for completeness
}

// Removed stub function register_additional_computer_use_tools

// Function to register all desktop tools with the tool provider
pub async fn register_desktop_tools(
    provider: &mut LocalToolProvider,
    _state: State<'_, AppState>, // Keep state as it might be needed by underlying commands
    app_handle: tauri::AppHandle,
) {
    info!("Registering desktop tools...");

    // --- Element Tools (Keep separate for now) ---

    // get_focused_element_info
    let get_focused_def = ToolDefinition {
        name: "get_focused_element_info".to_string(),
        description: "Get accessibility information about the currently focused UI element.".to_string(),
        input_schema: json!({ "type": "object", "properties": {}, "required": [] }),
    };
    let app_handle_clone = app_handle.clone();
    let get_focused_exec = move |_input: Value| {
        let app = app_handle_clone.clone();
        async move {
            let state_manager = app.state::<AppState>();
            // Removed blocking call, assume commands handle their own blocking if needed
            let result = commands::element::dev_get_focused_element_info(app.clone(), state_manager).await
                         .map_err(|e| format!("Error getting focused element: {}", e))?;
            // Result is already stringified JSON
            Ok(serde_json::from_str(&result)
                .map_err(|e| format!("Failed to parse element info JSON: {}", e))?)
        }
    };
    provider.register_async_tool(get_focused_def, get_focused_exec).await;
    info!("Registered tool: get_focused_element_info");

     // capture_element_screenshot
    let capture_element_screenshot_def = ToolDefinition {
        name: "capture_element_screenshot".to_string(),
        description: "Captures a screenshot of the currently focused UI element.".to_string(),
        input_schema: json!({ "type": "object", "properties": {}, "required": [] }),
    };
    let app_handle_clone = app_handle.clone();
    let capture_element_screenshot_exec = move |_input: Value| {
        let app = app_handle_clone.clone();
        async move {
            let state_manager = app.state::<AppState>();
            let result = commands::element::capture_element_screenshot_command(app.clone(), state_manager).await
                         .map_err(|e| format!("Error capturing element screenshot: {}", e))?;
            // Result is base64 string
            Ok(json!(result))
        }
    };
    provider.register_async_tool(capture_element_screenshot_def, capture_element_screenshot_exec).await;
    info!("Registered tool: capture_element_screenshot");


    // --- Clipboard Tools (Keep separate for now) ---

    // get_clipboard
    let get_clipboard_def = ToolDefinition {
        name: "get_clipboard".to_string(),
        description: "Get the current text contents of the operating system clipboard.".to_string(),
        input_schema: json!({ "type": "object", "properties": {}, "required": [] }),
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
    #[derive(Deserialize)]
    struct SetClipboardContentInput { content: String }
    let set_clipboard_def = ToolDefinition {
        name: "set_clipboard_content".to_string(),
        description: "Sets the operating system clipboard content to the provided text.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": { "content": { "type": "string" } },
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
            commands::core::dev_set_clipboard(args.content, state_manager).await
                 .map_err(|e| format!("Error setting clipboard content: {}", e))?;
            Ok(json!({"success": true}))
        }
    };
    provider.register_async_tool(set_clipboard_def, set_clipboard_exec).await;
    info!("Registered tool: set_clipboard_content");


    // --- Consolidated Computer Tool ---

    let computer_tool_def = ToolDefinition {
        // Use the Anthropic name for the tool
        name: "computer".to_string(),
        // Use description similar to Anthropic spec
        description: "Use a mouse and keyboard to interact with a computer GUI, and take screenshots.".to_string(),
        // Define the input schema based on ComputerActionInput and Anthropic spec
        input_schema: json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": [
                        "key", "hold_key", "release_key", "type", // Keyboard
                        "cursor_position", "mouse_move", "left_mouse_down", "left_mouse_up", // Mouse
                        "left_click", "left_click_drag", "right_click", "middle_click",
                        "double_click", "triple_click",
                        "scroll", // Scroll
                        "wait", // Other
                        "screenshot"
                    ],
                    "description": "The action to perform."
                },
                "coordinate": {
                    "type": "array",
                    "items": { "type": "number" },
                    "minItems": 2,
                    "maxItems": 2,
                    "description": "(x, y) coordinate for mouse actions like move, clicks, scroll, drag (end)."
                },
                "duration": {
                    "type": "number", // Use number to allow fractional seconds
                    "description": "Duration in seconds for 'hold_key' and 'wait' actions."
                },
                "scroll_amount": {
                    "type": "integer",
                    "description": "Number of scroll wheel 'clicks' for 'scroll' action."
                },
                "scroll_direction": {
                    "type": "string",
                    "enum": ["up", "down", "left", "right"],
                    "description": "Direction for 'scroll' action."
                },
                "start_coordinate": {
                    "type": "array",
                    "items": { "type": "number" },
                    "minItems": 2,
                    "maxItems": 2,
                    "description": "(x, y) starting coordinate for 'left_click_drag' action."
                },
                "text": {
                    "type": "string",
                    "description": "Text for 'type', 'key', 'hold_key', 'release_key'. Can optionally be used by click/scroll actions as modifier."
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
            let args = match serde_json::from_value::<ComputerActionInput>(input.clone()) {
                 Ok(args) => args,
                 Err(e) => return Err(format!("Failed to parse computer tool input: {}. Input: {}", e, input)),
             };

            // Macro to simplify getting required parameters
            macro_rules! get_required {
                ($param:expr, $action:expr, $type:expr) => {
                    $param.ok_or_else(|| format!("Missing required parameter '{}' for action '{}'", $type, $action))?
                };
            }

            // Macro to simplify getting coordinate parameters
            macro_rules! get_coord {
                ($param:expr, $action:expr, $name:expr) => {
                    {
                        let vec = get_required!($param, $action, $name)?;
                        if vec.len() != 2 {
                             return Err(format!("Parameter '{}' must be an array of [x, y] for action '{}'", $name, $action));
                        }
                        (vec[0], vec[1])
                    }
                };
             }

            // Dispatch based on action
            match args.action.as_str() {
                // Keyboard Actions
                "key" => {
                    let key_text = get_required!(args.text, "key", "text")?;
                    commands::keyboard::dev_press_key(app.clone(), state_manager, key_text).await?;
                    Ok(json!({"success": true}))
                }
                "hold_key" => {
                    let key_text = get_required!(args.text, "hold_key", "text")?;
                    // Duration from schema is f64 (seconds), command expects Option<u64> (ms)
                    let duration_ms = args.duration.map(|d| (d * 1000.0).max(0.0) as u64);
                    commands::keyboard::dev_hold_key(key_text, duration_ms, state_manager).await?;
                     Ok(json!({"success": true}))
                }
                "release_key" => {
                    let key_text = get_required!(args.text, "release_key", "text")?;
                    commands::keyboard::dev_release_key(key_text, state_manager).await?;
                     Ok(json!({"success": true}))
                }
                 "type" => {
                    let text_to_type = get_required!(args.text, "type", "text")?;
                    commands::keyboard::dev_type_text(app.clone(), state_manager, text_to_type).await?;
                    Ok(json!({"success": true}))
                }

                // Mouse Actions
                "cursor_position" => {
                    let (x, y) = commands::mouse::dev_get_cursor_position(app.clone(), state_manager).await?;
                    Ok(json!({ "x": x, "y": y }))
                }
                "mouse_move" => {
                    let (x, y) = get_coord!(args.coordinate, "mouse_move", "coordinate")?;
                    commands::mouse::dev_mouse_move(app.clone(), state_manager, x, y).await?;
                    Ok(json!({"success": true}))
                }
                 "left_mouse_down" => {
                    // Schema doesn't require coordinate for down/up, but our commands do. Use current pos? Or require coord?
                    // Let's require coordinate for now for consistency with click/drag.
                    let (x, y) = get_coord!(args.coordinate, "left_mouse_down", "coordinate")?;
                    commands::mouse::dev_left_mouse_down(app.clone(), state_manager, x, y).await?;
                     Ok(json!({"success": true}))
                 }
                 "left_mouse_up" => {
                    let (x, y) = get_coord!(args.coordinate, "left_mouse_up", "coordinate")?;
                    commands::mouse::dev_left_mouse_up(app.clone(), state_manager, x, y).await?;
                     Ok(json!({"success": true}))
                 }
                "left_click" => {
                    let (x, y) = get_coord!(args.coordinate, "left_click", "coordinate")?;
                    // Use args.text as optional modifier key
                    commands::mouse::dev_left_click(app.clone(), state_manager, x, y, args.text).await?;
                    Ok(json!({"success": true}))
                 }
                 "left_click_drag" => {
                    let (start_x, start_y) = get_coord!(args.start_coordinate, "left_click_drag", "start_coordinate")?;
                    let (end_x, end_y) = get_coord!(args.coordinate, "left_click_drag", "coordinate")?;
                    commands::mouse::dev_left_click_drag(app.clone(), state_manager, start_x, start_y, end_x, end_y).await?;
                    Ok(json!({"success": true}))
                 }
                "right_click" => {
                    let (x, y) = get_coord!(args.coordinate, "right_click", "coordinate")?;
                    commands::mouse::dev_right_click(app.clone(), state_manager, x, y, args.text).await?;
                    Ok(json!({"success": true}))
                 }
                "middle_click" => {
                    let (x, y) = get_coord!(args.coordinate, "middle_click", "coordinate")?;
                    commands::mouse::dev_middle_click(app.clone(), state_manager, x, y, args.text).await?;
                    Ok(json!({"success": true}))
                 }
                 "double_click" => {
                    let (x, y) = get_coord!(args.coordinate, "double_click", "coordinate")?;
                    commands::mouse::dev_double_click(app.clone(), state_manager, x, y, args.text).await?;
                    Ok(json!({"success": true}))
                 }
                 "triple_click" => {
                    let (x, y) = get_coord!(args.coordinate, "triple_click", "coordinate")?;
                    commands::mouse::dev_triple_click(app.clone(), state_manager, x, y, args.text).await?;
                    Ok(json!({"success": true}))
                 }

                // Scroll Action
                "scroll" => {
                    // Use coordinate if provided, otherwise scroll at current position (or should we require coordinate?)
                    // Anthropic schema requires coordinate for scroll. Let's follow that.
                    let (x, y) = get_coord!(args.coordinate, "scroll", "coordinate")?;
                    let direction = get_required!(args.scroll_direction, "scroll", "scroll_direction")?;
                    let amount = get_required!(args.scroll_amount, "scroll", "scroll_amount")?;
                    // Use args.text as optional modifier key - dev_scroll_window doesn't support modifier, only position.
                    // Pass position (x, y) to dev_scroll_window
                    commands::window::dev_scroll_window(app.clone(), state_manager, direction, amount as f64, Some(x), Some(y)).await?;
                    Ok(json!({"success": true}))
                }

                // Other Actions
                 "wait" => {
                    let duration_sec = get_required!(args.duration, "wait", "duration")?;
                    commands::core::dev_wait(duration_sec, state_manager).await?;
                    Ok(json!({"success": true}))
                 }
                 "screenshot" => {
                    let base64_string = crate::capture_screenshot_command(app.clone()).await?;
                    Ok(json!({ "screenshot": base64_string })) // Return base64 string directly
                 }

                // Unknown action
                _ => Err(format!("Unknown computer action: {}", args.action)),
            }
        }
    };
    provider.register_async_tool(computer_tool_def, computer_tool_exec).await;
    info!("Registered consolidated tool: computer");

    // Remove registrations for individual tools that are now part of 'computer'
    // - capture_screenshot
    // - type_text
    // - desktop_click (combined left, right, double)
    // - scroll
    // - triple_click
    // - wait
    // - hold_key
    // - release_key
    // - mouse_move
    // - left_mouse_down
    // - left_mouse_up
    // - left_click
    // - right_click
    // - middle_click
    // - double_click
    // - left_click_drag
    // - cursor_position

    // Note: Element and Clipboard tools remain registered separately above.

    info!("Desktop tool registration completed.");
}

pub async fn setup_tools(
    provider: &mut LocalToolProvider,
    state: State<'_, AppState>,
    app_handle: tauri::AppHandle,
) {
    // Register the consolidated and remaining separate tools
    register_desktop_tools(provider, state, app_handle.clone()).await;

    // Removed call to the stub function register_additional_computer_use_tools
}

// Helper function for extracting coordinate pair
// fn get_coordinate(coord_opt: Option<&Value>, action: &str, param_name: &str) -> Result<(f64, f64), String> {
//     let coord_val = coord_opt.ok_or_else(|| format!("Missing required parameter '{}' for action '{}'", param_name, action))?;
//     let coord_arr = coord_val.as_array().ok_or_else(|| format!("Parameter '{}' must be an array for action '{}'", param_name, action))?;
//     if coord_arr.len() != 2 {
//         return Err(format!("Parameter '{}' must be an array of [x, y] for action '{}'", param_name, action));
//     }
//     let x = coord_arr[0].as_f64().ok_or_else(|| format!("Invalid x value in '{}' for action '{}'", param_name, action))?;
//     let y = coord_arr[1].as_f64().ok_or_else(|| format!("Invalid y value in '{}' for action '{}'", param_name, action))?;
//     Ok((x, y))
// }

// Helper function for extracting required string
// fn get_required_string(text_opt: Option<&Value>, action: &str, param_name: &str) -> Result<String, String> {
//     let text_val = text_opt.ok_or_else(|| format!("Missing required parameter '{}' for action '{}'", param_name, action))?;
//     text_val.as_str().map(String::from).ok_or_else(|| format!("Parameter '{}' must be a string for action '{}'", param_name, action))
// }

// Helper function for extracting required number (f64)
// fn get_required_f64(num_opt: Option<&Value>, action: &str, param_name: &str) -> Result<f64, String> {
//    let num_val = num_opt.ok_or_else(|| format!("Missing required parameter '{}' for action '{}'", param_name, action))?;
//    num_val.as_f64().ok_or_else(|| format!("Parameter '{}' must be a number for action '{}'", param_name, action))
//}

// Helper function for extracting required integer (i32)
// fn get_required_i32(num_opt: Option<&Value>, action: &str, param_name: &str) -> Result<i32, String> {
//     let num_val = num_opt.ok_or_else(|| format!("Missing required parameter '{}' for action '{}'", param_name, action))?;
//     num_val.as_i64().map(|v| v as i32).ok_or_else(|| format!("Parameter '{}' must be an integer for action '{}'", param_name, action))
// }
