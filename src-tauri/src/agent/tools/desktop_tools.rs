use crate::agent::implementations::tool_provider::LocalToolProvider;
use crate::agent::structs::ToolDefinition;
use crate::state::AppState;
use crate::commands;
use tauri::{State, Manager};
use serde_json::{Value, json};
use tracing::info;
use crate::commands::window;

}

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
                    commands::core::dev_set_clipboard(args.content, state_manager).await
                })
            });

            inner_result.map_err(|e| format!("Error setting clipboard content: {}", e))?;
            Ok(json!({"success": true}))
        }
    };
    provider.register_async_tool(set_clipboard_def, set_clipboard_exec).await;
    info!("Registered tool: set_clipboard_content");

    // Desktop Click Tool
    #[derive(serde::Deserialize)]
    struct DesktopClickArgs {
        x: f64,
        y: f64,
        click_type: Option<String>,
        modifier: Option<String>,
    }

    let desktop_click_def = ToolDefinition {
        name: "desktop_click".to_string(),
        description: "Performs a mouse click (left, right, double) at the specified coordinates on the desktop screen. Coordinates should typically be obtained from 'get_focused_element_info' or a screenshot analysis.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "x": { "type": "number", "description": "X-coordinate for the click." },
                "y": { "type": "number", "description": "Y-coordinate for the click." },
                "click_type": { "type": "string", "enum": ["left", "right", "double"], "description": "Type of click (defaults to left)." },
                "modifier": { "type": "string", "enum": ["shift", "ctrl", "alt", "cmd"], "description": "Optional modifier key to hold during the click." }
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
                .map_err(|e| format!("Failed to parse desktop_click input: {}", e))?;

            let x = args.x;
            let y = args.y;
            let modifier = args.modifier;

            let click_result = match args.click_type.as_deref().unwrap_or("left") {
                "left" => commands::mouse::dev_left_click(app.clone(), state_manager, x, y, modifier).await,
                "right" => commands::mouse::dev_right_click(app.clone(), state_manager, x, y, modifier).await,
                "double" => commands::mouse::dev_double_click(app.clone(), state_manager, x, y, modifier).await,
                unknown => Err(format!("Unsupported click type: {}", unknown)),
            };

            click_result.map_err(|e| format!("Error performing desktop click: {}", e))?;
            Ok(json!({"success": true}))
        }
    };
    provider.register_async_tool(desktop_click_def, desktop_click_exec).await;
    info!("Registered tool: desktop_click");

    info!("Desktop tool registration completed.");
}

pub async fn setup_tools(
    provider: &mut LocalToolProvider,
    state: State<'_, AppState>,
    app_handle: tauri::AppHandle,
) {
    register_desktop_tools(provider, state, app_handle.clone()).await;
}
