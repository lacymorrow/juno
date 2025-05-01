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
                     capture_screenshot_command(app_handle)
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

    // Removed state_clone capture
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

    // Removed state_clone capture
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

    // Add new computer use tools based on the Anthropic documentation
    register_additional_computer_use_tools(provider, app_handle.clone()).await;

    info!("Desktop tool registration completed with core tools and additional computer use tools.");
}

// Register additional computer use tools for Anthropic's specs
async fn register_additional_computer_use_tools(
    provider: &mut LocalToolProvider,
    app_handle: AppHandle,
) {
    // TODO: Register more advanced computer use tools
}
