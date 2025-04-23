use crate::agent::implementations::tool_provider::LocalToolProvider;
use crate::agent::structs::ToolDefinition;
use crate::state::AppState;
use crate::commands;
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

        let rt = tokio::runtime::Runtime::new().map_err(|e| format!("Failed to create runtime: {}", e))?;
        let result = rt.block_on(async {
            let app_handle_for_async = app_handle.clone();
            commands::element::dev_get_focused_element_info(app_handle_for_async, managed_state)
                .await
                .map_err(|e| format!("Error getting focused element: {}", e))
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

        let rt = tokio::runtime::Runtime::new().map_err(|e| format!("Failed to create runtime: {}", e))?;
        let result = rt.block_on(async {
            let app_handle_for_async = app_handle.clone();
            capture_screenshot_command(app_handle_for_async)
                .await
                .map_err(|e| format!("Error capturing screenshot: {}", e))
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

        let rt = tokio::runtime::Runtime::new().map_err(|e| format!("Failed to create runtime: {}", e))?;
        let result = rt.block_on(async {
            let app_handle_for_async = app_handle.clone();
            commands::element::capture_element_screenshot_command(app_handle_for_async, managed_state)
                .await
                .map_err(|e| format!("Error capturing element screenshot: {}", e))
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

        let rt = tokio::runtime::Runtime::new().map_err(|e| format!("Failed to create runtime: {}", e))?;
        let result = rt.block_on(async {
            let app_handle_for_async = app_handle.clone();
            commands::keyboard::dev_type_text(app_handle_for_async, managed_state, args.text)
                .await
                .map_err(|e| format!("Error typing text: {}", e))
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

        let rt = tokio::runtime::Runtime::new().map_err(|e| format!("Failed to create runtime: {}", e))?;
        let result = rt.block_on(async {
            dev_get_clipboard(managed_state)
                .await
                .map_err(|e| format!("Error getting clipboard: {}", e))
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

        let rt = tokio::runtime::Runtime::new().map_err(|e| format!("Failed to create runtime: {}", e))?;
        let result = rt.block_on(async {
            dev_set_clipboard(args.text, managed_state)
                .await
                .map_err(|e| format!("Error setting clipboard: {}", e))
        })?;

        Ok(json!(result))
    };
    provider.register_tool(set_clipboard_def, set_clipboard_exec).await;
    info!("Registered tool: set_clipboard");

    info!("Desktop tool registration completed with core tools. Additional tools to be implemented as needed.");
}
