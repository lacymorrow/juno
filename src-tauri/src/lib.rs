#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use clap::Parser;
use computer_use_ai_sdk::{Desktop, ToolDefinition, ToolInputSchema, ToolParameter};
use computer_use_ai_sdk::AutomationError;
use dotenvy::dotenv;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::env;
use std::sync::Arc;
use image::{GenericImageView, ImageFormat};
use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use std::io::Cursor;
use tracing_subscriber::{fmt, EnvFilter};
use tracing::{debug, error, info, warn};
use tauri_plugin_notification::NotificationExt;
use tauri::{AppHandle, Manager, State};
use tauri::menu::{Menu, PredefinedMenuItem, MenuItemKind};
use tauri::tray::{TrayIconEvent, MouseButton, MouseButtonState};
use tauri::{WindowEvent};
use tauri::menu::MenuItemBuilder;
use tauri::image::Image;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use std::collections::HashMap;
use std::process::Command;
use std::time::Duration;
use wait_timeout::ChildExt;

#[cfg(target_os = "macos")]
use computer_use_ai_sdk::platforms::macos::utils as macos_utils;

#[cfg(target_os = "macos")]
use computer_use_ai_sdk::platforms::macos::element::MacOSUIElement;

#[cfg(target_os = "macos")]
use computer_use_ai_sdk::platforms::macos::element::get_focused_element_ns_workspace;

mod tts;

// Added for selector parsing
use computer_use_ai_sdk::Selector;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Run the NSWorkspace-based test for focused element
    #[arg(long)]
    test_focused_element_ns: bool,

    /// Check if the process has accessibility permissions
    #[arg(long)]
    check_accessibility: bool,

    // Add other test flags here, e.g.:
    // #[arg(long)]
    // test_list_apps: bool,

    // #[arg(long)]
    // test_screenshot: bool,
}

#[allow(dead_code)] // Allow dead code for potentially unused fields
pub(crate) struct AppState {
    desktop: Arc<Desktop>,
    // State for text_editor_undo_edit
    #[allow(dead_code)] // Temporarily allow, seems used by call_tool
    last_edited_file: Mutex<Option<PathBuf>>,
    #[allow(dead_code)] // Temporarily allow, seems used by call_tool
    previous_content: Mutex<Option<Option<String>>>, // Option<Option<String>>: None=no undo, Some(None)=last was create, Some(Some(content))=last was edit
}

#[derive(Serialize, Clone)]
struct AnthropicMessage {
    role: String,
    content: Value,
}

#[derive(Deserialize, Debug, Clone, Serialize)]
struct AnthropicContentBlock {
    #[serde(rename = "type")]
    type_: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    input: Option<Value>,
}

#[derive(Serialize)]
struct ToolResultBlock {
    #[serde(rename = "type")]
    type_: String, // Always "tool_result"
    tool_use_id: String,
    content: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    is_error: Option<bool>,
}

#[derive(Serialize)]
struct SubmitQueryResult {
    text: String,
    audio_base64: Option<String>,
}

// Tool function for find and replace in a file (Moved before submit_query)
fn str_replace_editor(file_path: String, find_text: String, replace_text: String) -> Result<String, String> {
    info!(file_path = %file_path, find = %find_text, "Attempting str_replace_editor");

    // Read the file content
    let content = match fs::read_to_string(&file_path) {
        Ok(c) => c,
        Err(e) => {
            let err_msg = format!("Failed to read file '{}': {}", file_path, e);
            error!(error = %err_msg, "str_replace_editor failed");
            return Err(err_msg);
        }
    };

    // Perform the replacement
    let new_content = content.replace(&find_text, &replace_text);

    // Write the new content back to the file
    match fs::write(&file_path, new_content) {
        Ok(_) => {
            let success_msg = format!("Successfully updated file '{}'", file_path);
            info!(success_msg);
            Ok(success_msg)
        }
        Err(e) => {
            let err_msg = format!("Failed to write file '{}': {}", file_path, e);
            error!(error = %err_msg, "str_replace_editor failed");
            Err(err_msg)
        }
    }
}

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[cfg(target_os = "macos")]
#[tauri::command]
async fn capture_screenshot_command(app: tauri::AppHandle) -> Result<String, String> {
    match macos_utils::capture_and_encode_screenshot() {
        Ok(base64_string) => {
            // Send notification on success
            app.notification()
                .builder()
                .title("Screenshot")
                .body("Screenshot captured successfully.")
                .show()
                .map_err(|e| format!("Failed to send notification: {}", e))?;
            Ok(base64_string)
        }
        Err(e) => Err(format!("Failed to capture screenshot: {}", e)),
    }
}

#[cfg(not(target_os = "macos"))]
#[tauri::command]
async fn capture_screenshot_command(_app: tauri::AppHandle) -> Result<String, String> {
    Err("Screenshot capture is only supported on macOS currently.".to_string())
}

#[tauri::command]
async fn list_apps(state: tauri::State<'_, AppState>) -> Result<Vec<String>, String> {
    match state.desktop.applications() {
        Ok(apps) => {
            let app_names = apps
                .into_iter()
                .map(|app| {
                    app.attributes()
                        .label
                        .unwrap_or_else(|| "Unknown Label".to_string())
                })
                .collect();
            Ok(app_names)
        }
        Err(e) => Err(format!("Failed to get applications: {}", e)),
    }
}

#[tauri::command]
fn check_server_status(state: tauri::State<'_, AppState>) -> bool {
    let _ = state.desktop;
    true
}

#[tauri::command]
async fn dev_get_focused_element_info(app: tauri::AppHandle, _state: tauri::State<'_, AppState>) -> Result<String, String> {
    println!("[DEV_TOOL] Attempting to get focused element info using NSWorkspace...");

    #[cfg(target_os = "macos")]
    let result = get_focused_element_ns_workspace(false, true);

    #[cfg(not(target_os = "macos"))]
    let result: Result<computer_use_ai_sdk::UIElement, AutomationError> = Err(AutomationError::UnsupportedPlatform);

    match result {
        Ok(element) => {
            println!("[DEV_TOOL] get_focused_element_info (NSWorkspace) succeeded.");
            // Send notification on success
             app.notification()
                .builder()
                .title("Focus Info")
                .body("Focused element info retrieved.")
                .show()
                .map_err(|e| format!("Failed to send notification: {}", e))?;

            let attrs = element.attributes();
            serde_json::to_string_pretty(&attrs).map_err(|e| {
                let err_msg = format!("Failed to serialize element info result: {}", e);
                println!("[DEV_TOOL] Error: {}", err_msg);
                err_msg
            })
        }
        Err(e) => {
            let err_msg = format!("Failed to call get_focused_element_info (NSWorkspace): {}", e);
            println!("[DEV_TOOL] Error: {}", err_msg);
            Err(err_msg)
        }
    }
}

#[cfg(target_os = "macos")]
#[tauri::command]
async fn capture_element_screenshot_command(
    app: AppHandle,
    _state: State<'_, AppState>,
) -> Result<String, String> {
    println!("[DEV_TOOL] Capturing focused element screenshot using NSWorkspace method...");

    let focused_element = match get_focused_element_ns_workspace(false, true) {
        Ok(el) => el,
        Err(e) => {
            let err_msg = format!("Failed to get focused element (NSWorkspace): {}", e);
            println!("[DEV_TOOL] Error: {}", err_msg);
            return Err(err_msg);
        }
    };

    let macos_element = match focused_element.as_any().downcast_ref::<MacOSUIElement>() {
        Some(el) => el,
        None => {
            let err_msg = "Focused element is not a MacOSUIElement".to_string();
            println!("[DEV_TOOL] Error: {}", err_msg);
            return Err(err_msg);
        }
    };

    match macos_utils::capture_element_screenshot(macos_element) {
        Ok(base64_string) => {
            println!("[DEV_TOOL] Element screenshot captured successfully.");
            // Send notification on success
            app.notification()
                .builder()
                .title("Element Screenshot")
                .body("Focused element screenshot captured.")
                .show()
                .map_err(|e| format!("Failed to send notification: {}", e))?;
            Ok(base64_string)
        },
        Err(e) => {
            match e {
                AutomationError::ZeroElementDimensions { role, label, x, y, width, height } => {
                    let user_friendly_err_msg = format!(
                        "Error: The focused element ('{}', Label: '{}') reported zero or negative dimensions ({}, {}, {}, {}) and could not be captured.",
                        role,
                        label,
                        x, y, width, height
                    );
                    println!("[DEV_TOOL] Error: {}", user_friendly_err_msg);
                    Err(user_friendly_err_msg)
                }
                _ => {
                    let err_msg = format!("Failed to capture element screenshot: {}", e);
                    println!("[DEV_TOOL] Error: {}", err_msg);
                    Err(err_msg)
                }
            }
        }
    }
}

#[cfg(not(target_os = "macos"))]
#[tauri::command]
async fn capture_element_screenshot_command(
    _app: tauri::AppHandle,
    _state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    Err("Element screenshot capture is only supported on macOS currently.".to_string())
}

#[tauri::command]
async fn submit_query(
    query: String,
    state: tauri::State<'_, AppState>,
) -> Result<SubmitQueryResult, String> {
    println!("Received query in submit_query: {}", query);

    let api_key = std::env::var("ANTHROPIC_API_KEY")
        .map_err(|_| "ANTHROPIC_API_KEY not configured.".to_string())?;

    let desktop_arc = state.desktop.clone();
    let http_client = Client::new();
    let mut conversation_history: Vec<AnthropicMessage> = Vec::new();
    let mut final_response_text = String::new();
    const MAX_ITERATIONS: u32 = 10;

    conversation_history.push(AnthropicMessage {
        role: "user".to_string(),
        content: Value::String(query.clone()),
    });

    let available_tools = list_tools(&desktop_arc); // Call the local list_tools function

    for iteration in 0..MAX_ITERATIONS {
        println!("Agent Iteration: {}", iteration + 1);

        #[derive(Serialize)]
        struct AnthropicThinkingBudget {
            #[serde(rename = "type")]
            type_: String,
            budget_tokens: u32,
        }

        #[derive(Serialize)]
        struct AnthropicRequest<'a> {
            model: &'a str,
            max_tokens: u32,
            messages: Vec<AnthropicMessage>,
            tools: Vec<ToolDefinition>,
            #[serde(skip_serializing_if = "Option::is_none")]
            system: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none")]
            thinking: Option<AnthropicThinkingBudget>,
        }

        let max_output_tokens = 1024;
        let thinking_budget = 4000;
        let total_max_tokens = max_output_tokens + thinking_budget;

        let request_payload = AnthropicRequest {
            model: "claude-3-7-sonnet-20250219", // Use Claude 3.5 Sonnet
            max_tokens: total_max_tokens,
            messages: conversation_history.clone(),
            tools: available_tools.clone(),
            system: Some("You are an AI assistant that can use tools to interact with the user's computer desktop environment. Use the provided tools to fulfill the user's request. Respond with the final result or status.".to_string()),
            thinking: None, // Commented out for now
        };

        let response = http_client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &api_key)
            .header("anthropic-version", "2023-06-01")
            .header("anthropic-beta", "computer-use-2025-01-24")
            .header("content-type", "application/json")
            .json(&request_payload)
            .send()
            .await;

        let response = match response {
            Ok(res) => res,
            Err(e) => {
                let err_msg = format!("HTTP request to Anthropic failed: {}", e);
                error!("Error: {}", err_msg);
                return Err(err_msg);
            }
        };

        if !response.status().is_success() {
            let status = response.status();
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "Failed to read error body".to_string());
            let err_msg = format!("Anthropic API error: {} - {}", status, body);
            error!("Error: {}", err_msg);
            return Err(err_msg);
        }

        // New Anthropic Response structure matching API
        #[derive(Deserialize, Debug)]
        struct AnthropicUsage {
            #[allow(dead_code)] // Allow dead code for potentially unused fields
            input_tokens: u32,
            #[allow(dead_code)] // Allow dead code for potentially unused fields
            output_tokens: u32,
        }
        #[derive(Deserialize, Debug)]
        struct AnthropicResponse { // Shadowing the previous struct is fine here
            content: Vec<AnthropicContentBlock>,
            stop_reason: String,
            #[allow(dead_code)] // Allow dead code for potentially unused fields
            usage: AnthropicUsage,
        }

        let anthropic_response: AnthropicResponse = match response.json().await {
            Ok(res) => res,
            Err(e) => {
                let err_msg = format!("Failed to parse Anthropic JSON response: {}", e);
                error!("Error: {}", err_msg);
                return Err(err_msg);
            }
        };

        debug!("Anthropic Raw Response: {:?}", anthropic_response);

        let filtered_content: Vec<AnthropicContentBlock> = anthropic_response
            .content
            .clone()
            .into_iter()
            .filter(|block| block.type_ != "thinking")
            .collect();

        let assistant_content_value = match serde_json::to_value(filtered_content) {
            Ok(v) => v,
            Err(e) => {
                let err_msg = format!("Failed to serialize assistant content: {}", e);
                error!("Error: {}", err_msg);
                return Err(err_msg);
            }
        };
        conversation_history.push(AnthropicMessage {
            role: "assistant".to_string(),
            content: assistant_content_value,
        });

        let mut tool_results: Vec<ToolResultBlock> = Vec::new();
        let mut has_tool_calls = false;

        for block in &anthropic_response.content {
            match block.type_.as_str() {
                "text" => {
                    if let Some(text) = &block.text {
                        final_response_text.push_str(text);
                        final_response_text.push('\n');
                    }
                }
                "tool_use" => {
                    has_tool_calls = true;
                    if let (Some(id), Some(name), Some(input)) =
                        (&block.id, &block.name, &block.input)
                    {
                        info!("Executing tool: {} with input: {:?}", name, input);

                        let tool_result = match name.as_str() {
                            "openApplication" => {
                                let app_name = input.get("application_name").and_then(|v| v.as_str()).map(|s| s.to_string());
                                if let Some(name) = app_name {
                                    desktop_arc.open_application(&name)
                                        .map(|_| json!({ "status": format!("Successfully opened {}", name) }))
                                } else {
                                    Err(AutomationError::InvalidArgument("Missing required parameter: application_name".to_string()))
                                }
                            }
                            "str_replace_editor" => {
                                let file_path = input.get("file_path").and_then(|v| v.as_str()).map(String::from);
                                let find_text = input.get("find").and_then(|v| v.as_str()).map(String::from);
                                let replace_text = input.get("replace").and_then(|v| v.as_str()).map(String::from);

                                match (file_path, find_text, replace_text) {
                                    (Some(fp), Some(find), Some(replace)) => {
                                        str_replace_editor(fp, find, replace)
                                            .map(|msg| json!({ "status": msg }))
                                            .map_err(|e| AutomationError::Internal(e))
                                    }
                                    _ => Err(AutomationError::InvalidArgument(
                                        "Missing required parameters: file_path, find, replace".to_string()
                                    )),
                                }
                            }
                            "dev_get_clipboard_content" => desktop_arc.get_clipboard_content()
                                .map(|content| json!({ "content": content }))
                                .map_err(|e| AutomationError::Internal(format!("Clipboard error: {}", e))),
                            "dev_set_clipboard_content" => {
                                let content = input.get("content").and_then(|v| v.as_str());
                                if let Some(content) = content {
                                    desktop_arc.set_clipboard_content(content)
                                        .map(|_| json!({ "status": "Clipboard set successfully" }))
                                } else {
                                    Err(AutomationError::InvalidArgument("Missing required parameter: content".to_string()))
                                }
                            }
                            _ => desktop_arc.call_tool(name, input.clone()),
                        };

                        let (result_content_value, is_error) = match tool_result {
                            Ok(result_value) => {
                                info!("Tool '{}' success: {:?}", name, result_value);
                                if name == "captureScreenshot" {
                                    if let Some(base64_data) = result_value
                                        .get("screenshot_base64")
                                        .and_then(|v| v.as_str())
                                    {
                                        let resized_base64_data = match BASE64_STANDARD.decode(base64_data) {
                                            Ok(image_bytes) => {
                                                match image::load_from_memory(&image_bytes) {
                                                    Ok(img) => {
                                                        let (width, height) = img.dimensions();
                                                        let max_dim = 1024.0;
                                                        let scale = if width > height {
                                                            max_dim / width as f32
                                                        } else {
                                                            max_dim / height as f32
                                                        };

                                                        let new_width = (width as f32 * scale).round() as u32;
                                                        let new_height = (height as f32 * scale).round() as u32;

                                                        let resized_img = if scale < 1.0 {
                                                             img.resize_exact(new_width, new_height, image::imageops::FilterType::Lanczos3)
                                                        } else {
                                                             img
                                                        };

                                                        let mut png_bytes = Vec::new();
                                                        match resized_img.write_to(&mut Cursor::new(&mut png_bytes), ImageFormat::Png) {
                                                            Ok(_) => BASE64_STANDARD.encode(&png_bytes),
                                                            Err(e) => {
                                                                let err_msg = format!("Failed to encode resized image to PNG: {}", e);
                                                                error!("{}", err_msg);
                                                                base64_data.to_string()
                                                            }
                                                        }
                                                    }
                                                    Err(e) => {
                                                        let err_msg = format!("Failed to load image from screenshot bytes: {}", e);
                                                        error!("{}", err_msg);
                                                        base64_data.to_string()
                                                    }
                                                }
                                            }
                                            Err(e) => {
                                                let err_msg = format!("Failed to decode base64 screenshot data: {}", e);
                                                error!("{}", err_msg);
                                                base64_data.to_string()
                                            }
                                        };

                                        let image_block = json!({
                                            "type": "image",
                                            "source": {
                                                "type": "base64",
                                                "media_type": "image/png",
                                                "data": resized_base64_data
                                            }
                                        });
                                        (Value::Array(vec![image_block]), false)
                                    } else {
                                        let error_msg = format!("Tool '{}' succeeded but returned unexpected data: {:?}", name, result_value);
                                        error!("{}", error_msg);
                                        (json!([{ "type": "text", "text": error_msg }]), true)
                                    }
                                } else {
                                    let result_str = serde_json::to_string(&result_value)
                                        .unwrap_or_else(|e| {
                                            format!("Failed to serialize tool result: {}", e)
                                        });
                                    (json!([{ "type": "text", "text": result_str }]), false)
                                }
                            }
                            Err(e) => {
                                println!("Tool execution error: {}", e);
                                let error_str = serde_json::to_string(&serde_json::json!({
                                    "error": format!("Tool execution failed: {}", e)
                                }))
                                .unwrap_or_default();
                                error!("Tool '{}' failed: {}", name, e);
                                (json!([{ "type": "text", "text": error_str }]), true)
                            }
                        };

                        tool_results.push(ToolResultBlock {
                            type_: "tool_result".to_string(),
                            tool_use_id: id.clone(),
                            content: result_content_value,
                            is_error: Some(is_error),
                        });
                    } else {
                        let warn_msg =
                            format!("Warning: Received incomplete tool_use block: {:?}", block);
                        warn!("{}", warn_msg);
                    }
                }
                _ => {
                    let warn_msg = format!("Warning: Unknown content block type: {}", block.type_);
                    warn!("{}", warn_msg);
                }
            }
        }

        if has_tool_calls {
            let tool_results_value = match serde_json::to_value(tool_results) {
                Ok(v) => v,
                Err(e) => {
                    let err_msg = format!("Failed to serialize tool results: {}", e);
                    error!("Error: {}", err_msg);
                    return Err(err_msg);
                }
            };

            conversation_history.push(AnthropicMessage {
                role: "user".to_string(),
                content: tool_results_value,
            });
        } else {
            if anthropic_response.stop_reason == "end_turn"
                || anthropic_response.stop_reason == "stop_sequence"
            {
                info!(
                    "Agent loop finished. Stop reason: {}",
                    anthropic_response.stop_reason
                );
                break;
            } else {
                let warn_msg = format!(
                    "Warning: Loop continued without tool calls but stop reason was: {}",
                    anthropic_response.stop_reason
                );
                warn!("{}", warn_msg);
            }
        }

        if iteration == MAX_ITERATIONS - 1 {
            let warn_msg = "Warning: Max iterations reached without final answer.".to_string();
            warn!("{}", warn_msg);
            final_response_text.push_str("\n[Agent reached maximum iterations]");
        }
    }

    let final_text = final_response_text.trim().to_string();
    info!("Final agent text response: {}", final_text);

    let audio_result = tts::elevenlabs::invoke_elevenlabs_tts(final_text.clone(), state).await;

    let audio_base64 = match audio_result {
        Ok(base64) => {
            info!("TTS successful, including audio in response.");
            Some(base64)
        }
        Err(e) => {
            error!("TTS failed: {}. Returning response without audio.", e);
            None
        }
    };

    Ok(SubmitQueryResult {
        text: final_text,
        audio_base64,
    })
}

#[tauri::command]
async fn get_logs(_state: tauri::State<'_, AppState>) -> Result<Vec<String>, String> {
    Ok(vec!["Log viewing is deprecated. Logs are now output to the terminal using the tracing library.".to_string()])
}

fn send_dev_tool_notification(app: &tauri::AppHandle, title: &str, body: &str) -> Result<(), String> {
    app.notification()
        .builder()
        .title(title)
        .body(body)
        .show()
        .map_err(|e| format!("Failed to send notification: {}", e))
}

#[tauri::command]
async fn dev_click_focused_element(
    app: AppHandle,
    state: State<'_, AppState>
) -> Result<(), String> {
    println!("[DEV_TOOL] Attempting to click focused element...");

    #[cfg(target_os = "macos")]
    {
        // Get the focused element first
        let focused_element = match state.desktop.focused_element() {
            Ok(el) => el,
            Err(e) => {
                let err_msg = format!("Failed to get focused element for click: {}", e);
                println!("[DEV_TOOL] Error: {}", err_msg);
                return Err(err_msg);
            }
        };

        // Now click the element
        match focused_element.click() {
             Ok(_) => {
                println!("[DEV_TOOL] click_focused_element succeeded.");
                send_dev_tool_notification(&app, "Click", "Clicked focused element.")?;
                Ok(())
            }
             Err(e) => {
                 let err_msg = format!("Failed to call click_focused_element: {}", e);
                println!("[DEV_TOOL] Error: {}", err_msg);
                Err(err_msg)
            }
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        Err(AutomationError::UnsupportedPlatform.to_string())
    }
}

#[tauri::command]
async fn dev_type_text(
    app: AppHandle,
    state: State<'_, AppState>,
    text: String
) -> Result<(), String> {
    println!("[DEV_TOOL] Attempting to type text: {}", text);

    #[cfg(target_os = "macos")]
    let result = state.desktop.type_text(&text);

    #[cfg(not(target_os = "macos"))]
    let result = Err(AutomationError::UnsupportedPlatform);

    match result {
        Ok(_) => {
            println!("[DEV_TOOL] type_text succeeded.");
             send_dev_tool_notification(&app, "Type Text", &format!("Typed: \"{}\"", text))?;
            Ok(())
        }
        Err(e) => {
            let err_msg = format!("Failed to call type_text: {}", e);
            println!("[DEV_TOOL] Error: {}", err_msg);
            Err(err_msg)
        }
    }
}

#[tauri::command]
async fn dev_press_key(
    app: AppHandle,
    state: State<'_, AppState>,
    key: String
) -> Result<(), String> {
    println!("[DEV_TOOL] Attempting to press key sequence: {}", key);

    #[cfg(target_os = "macos")]
    {
         // Get the focused element first
        let focused_element = match state.desktop.focused_element() {
            Ok(el) => el,
            Err(e) => {
                let err_msg = format!("Failed to get focused element for key press: {}", e);
                println!("[DEV_TOOL] Error: {}", err_msg);
                return Err(err_msg);
            }
        };

        // Press key on the element
        match focused_element.press_key(&key) {
             Ok(_) => {
                println!("[DEV_TOOL] press_key succeeded for: {}", key);
                send_dev_tool_notification(&app, "Press Key", &format!("Pressed key(s): {}", key))?; // Send notification
                Ok(())
            }
            Err(e) => {
                let err_msg = format!("Failed to call press_key for '{}': {}", key, e);
                println!("[DEV_TOOL] Error: {}", err_msg);
                Err(err_msg)
            }
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
         Err(AutomationError::UnsupportedPlatform.to_string())
    }
}

#[tauri::command]
async fn dev_open_application(app: tauri::AppHandle, state: tauri::State<'_, AppState>, app_name: String) -> Result<(), String> {
    println!("[DEV_TOOL] Attempting to open application: {}", app_name);
    match state.desktop.open_application(&app_name) {
        Ok(_) => {
            println!("[DEV_TOOL] open_application succeeded for: {}", app_name);
            send_dev_tool_notification(&app, "Open App", &format!("Opened application: {}", app_name))?;
            Ok(())
        }
        Err(e) => {
            let err_msg = format!("Failed to open application '{}': {}", app_name, e);
            println!("[DEV_TOOL] Error: {}", err_msg);
            Err(err_msg)
        }
    }
}

#[tauri::command]
async fn dev_open_url(app: AppHandle, state: State<'_, AppState>, url: String) -> Result<(), String> {
    println!("[DEV_TOOL] Attempting to open URL: {}", url);
    match state.desktop.open_url(&url, None) {
        Ok(_) => {
            println!("[DEV_TOOL] open_url succeeded for: {}", url);
            send_dev_tool_notification(&app, "Open URL", &format!("Opened URL: {}", url))?;
            Ok(())
        }
        Err(e) => {
            let err_msg = format!("Failed to open URL '{}': {}", url, e);
            println!("[DEV_TOOL] Error: {}", err_msg);
            Err(err_msg)
        }
    }
}

#[tauri::command]
async fn dev_scroll_window(
    app: AppHandle,
    state: State<'_, AppState>,
    direction: String,
    amount_str: Option<String>
) -> Result<(), String> {
    println!("[DEV_TOOL] Attempting to scroll window {}...", direction);

    // Validate direction string and determine the effective direction for the SDK call
    let lower_direction = direction.to_lowercase();
    #[cfg(target_os = "macos")]
    let effective_direction = match lower_direction.as_str() {
        "up" | "down" | "left" | "right" => lower_direction.as_str(), // Use direction directly
        _ => return Err(format!("Invalid scroll direction: '{}'. Must be 'up', 'down', 'left', or 'right'.", direction)),
    };

    #[cfg(not(target_os = "macos"))]
    let effective_direction = match lower_direction.as_str() {
         "up" => "up",
         "down" => "down",
        _ => return Err(format!("Invalid scroll direction: {}. Must be 'up' or 'down'.", direction)),
    };

    // Parse amount, default to a reasonable value (e.g., 3.0 units)
    let amount: f64 = match amount_str {
        Some(s) => match s.parse::<f64>() {
            Ok(num) => num,
            Err(_) => return Err(format!("Invalid scroll amount: '{}'. Must be a number.", s)),
        },
        None => 3.0, // Default scroll amount
    };

    #[cfg(target_os = "macos")]
    // Use the engine's scroll_at_current_position method with the inverted direction
    let result = state.desktop.engine().scroll_at_current_position(effective_direction, amount);

    #[cfg(not(target_os = "macos"))]
    let result = Err(AutomationError::UnsupportedPlatform); // Keep original behavior for non-macOS

    match result {
        Ok(_) => {
            println!("[DEV_TOOL] scroll_window {} (effective: {}) succeeded.", direction, effective_direction);
            let scroll_msg = format!("Scrolled window {} by {}", direction, amount);
            send_dev_tool_notification(&app, "Scroll", &scroll_msg)?;
            Ok(())
        }
        Err(e) => {
            let err_msg = format!("Failed to scroll window {} (effective: {}): {}", direction, effective_direction, e);
            println!("[DEV_TOOL] Error: {}", err_msg);
            Err(err_msg)
        }
    }
}

#[cfg(target_os = "macos")]
#[allow(dead_code)] // Allow dead code as this is a test/debug function
fn run_test_focused_element(desktop: &Desktop) -> Result<(), String> {
    println!("--- Running Test: Get Focused Element (Original Method) ---");
    match desktop.focused_element() {
        Ok(element) => {
            let attrs = element.attributes();
            println!("Focused Element Found:");
            println!("  Role: {}", attrs.role);
            println!("  Label: {:?}", attrs.label);
            println!("  Value: {:?}", attrs.value);
            println!("  Description: {:?}", attrs.description);
            println!("  Properties:");
            for (key, value) in attrs.properties {
                println!("    {}: {:?}", key, value);
            }
             if let Ok((x, y, w, h)) = element.bounds() {
                println!("  Bounds: x={}, y={}, width={}, height={}", x, y, w, h);
            } else {
                println!("  Bounds: Failed to retrieve");
            }
            println!("--- Test Focused Element (Original Method): Success ---");
            Ok(())
        }
        Err(e) => {
            let err_msg = format!("Failed to get focused element: {}", e);
            eprintln!("Error: {}", err_msg);
            println!("--- Test Focused Element (Original Method): Failed ---");
            Err(err_msg)
        }
    }
}

#[cfg(target_os = "macos")]
#[allow(dead_code)] // Allow dead code as this is a test/debug function
fn run_test_focused_element_ns() -> Result<(), String> {
    use computer_use_ai_sdk::platforms::macos::element::get_focused_element_ns_workspace;

    println!("--- Running Test: Get Focused Element (NSWorkspace Method) ---");
    match get_focused_element_ns_workspace(false, true) {
        Ok(element) => {
            let attrs = element.attributes();
            println!("Focused Element Found:");
            println!("  Role: {}", attrs.role);
            println!("  Label: {:?}", attrs.label);
            println!("  Value: {:?}", attrs.value);
            println!("  Description: {:?}", attrs.description);
            println!("  Properties:");
            for (key, value) in attrs.properties {
                println!("    {}: {:?}", key, value);
            }
             if let Ok((x, y, w, h)) = element.bounds() {
                println!("  Bounds: x={}, y={}, width={}, height={}", x, y, w, h);
            } else {
                println!("  Bounds: Failed to retrieve");
            }
            println!("--- Test Focused Element (NSWorkspace Method): Success ---");
            Ok(())
        }
        Err(e) => {
            let err_msg = format!("Failed to get focused element via NSWorkspace: {}", e);
            eprintln!("Error: {}", err_msg);
            println!("--- Test Focused Element (NSWorkspace Method): Failed ---");
            Err(err_msg)
        }
    }
}

#[cfg(target_os = "macos")]
#[allow(dead_code)] // Allow dead code as this is a test/debug function
fn run_check_accessibility() -> Result<(), String> {
    println!("--- Running Test: Check Accessibility Permissions ---");
    match computer_use_ai_sdk::platforms::macos::permissions::check_accessibility_permissions(true)
    {
        Ok(granted) => {
            println!("Accessibility permissions granted: {}", granted);
            if !granted {
                println!("Please grant accessibility permissions in System Settings.");
            }
            println!("--- Test Check Accessibility: Success ---");
            Ok(())
        }
        Err(e) => {
            let err_msg = format!("Failed to check accessibility permissions: {}", e);
            eprintln!("Error: {}", err_msg);
            println!("--- Test Check Accessibility: Failed ---");
            Err(err_msg)
        }
    }
}

// Helper function to update undo state
#[allow(dead_code)] // Temporarily allow, seems used by call_tool
fn update_undo_state(state: &AppState, file_path: PathBuf, previous_content: Option<String>) {
    *state.last_edited_file.lock().unwrap() = Some(file_path);
    *state.previous_content.lock().unwrap() = Some(previous_content);
}

#[tauri::command]
async fn dev_global_type_text(text: String, state: tauri::State<'_, AppState>) -> Result<(), String> {
    info!("Executing dev_global_type_text with text: {}", text);
    state.desktop.type_text(&text)
        .map_err(|e| format!("Error typing global text: {}", e))
}

#[tauri::command]
async fn dev_get_clipboard(state: tauri::State<'_, AppState>) -> Result<String, String> {
    info!("Executing dev_get_clipboard");
    state.desktop.get_clipboard_content()
        .map_err(|e| format!("Error getting clipboard content: {}", e))
}

#[tauri::command]
async fn dev_set_clipboard(content: String, state: tauri::State<'_, AppState>) -> Result<(), String> {
    info!("Executing dev_set_clipboard {}", content);
    state.desktop.set_clipboard_content(&content)
        .map_err(|e| format!("Error setting clipboard content: {}", e))
}

#[tauri::command]
async fn dev_hold_key(key: String, state: tauri::State<'_, AppState>) -> Result<(), String> {
    info!("Executing dev_hold_key with key: {}", key);
    state.desktop.hold_key(&key)
        .map_err(|e| format!("Error holding key '{}': {}", key, e))
}

#[tauri::command]
async fn dev_release_key(key: String, state: tauri::State<'_, AppState>) -> Result<(), String> {
    info!("Executing dev_release_key with key: {}", key);
    state.desktop.release_key(&key)
        .map_err(|e| format!("Error releasing key '{}': {}", key, e))
}

#[tauri::command]
async fn dev_wait(duration_ms: u64, state: tauri::State<'_, AppState>) -> Result<(), String> {
    info!("Executing dev_wait for {} ms", duration_ms);
    state.desktop.wait(duration_ms)
        .map_err(|e| format!("Error during wait: {}", e))
}

// New command to find element by selector
#[tauri::command]
async fn dev_find_element_by_selector(
    selector_str: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    println!("[DEV_TOOL] Finding element by selector: {}", selector_str);
    let selector: Selector = selector_str.as_str().into(); // Use From<&str> for Selector

    match state.desktop.locator(selector).first() {
        Ok(Some(element)) => {
            println!("[DEV_TOOL] Found element: {:?}", element.attributes());
            let attrs = element.attributes();
            serde_json::to_string_pretty(&attrs).map_err(|e| {
                let err_msg = format!("Failed to serialize found element attributes: {}", e);
                println!("[DEV_TOOL] Error: {}", err_msg);
                err_msg
            })
        }
        Ok(None) => {
            let err_msg = format!("Element not found for selector: {}", selector_str);
            println!("[DEV_TOOL] Info: {}", err_msg);
            Err(err_msg)
        }
        Err(e) => {
            let err_msg = format!("Error finding element for selector '{}': {}", selector_str, e);
            println!("[DEV_TOOL] Error: {}", err_msg);
            Err(err_msg)
        }
    }
}

// New command to click an element found by selector
#[tauri::command]
async fn dev_click_element_by_selector(
    app: AppHandle,
    selector_str: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    println!("[DEV_TOOL] Clicking element by selector: {}", selector_str);
    let selector: Selector = selector_str.as_str().into();

    match state.desktop.locator(selector).first() {
        Ok(Some(element)) => {
            println!("[DEV_TOOL] Found element, attempting click...");
            match element.click() {
                Ok(click_result) => {
                    println!("[DEV_TOOL] Click successful: {:?}", click_result);
                     let click_msg = format!("Clicked element matching: {}", selector_str);
                     send_dev_tool_notification(&app, "Click Element", &click_msg)?;
                    Ok(())
                }
                Err(e) => {
                    let err_msg = format!("Failed to click element found by selector '{}': {}", selector_str, e);
                    println!("[DEV_TOOL] Error: {}", err_msg);
                    Err(err_msg)
                }
            }
        }
        Ok(None) => {
            let err_msg = format!("Element not found for click selector: {}", selector_str);
            println!("[DEV_TOOL] Info: {}", err_msg);
            Err(err_msg)
        }
        Err(e) => {
            let err_msg = format!(
                "Error finding element before click for selector '{}': {}",
                selector_str,
                e
            );
            println!("[DEV_TOOL] Error: {}", err_msg);
            Err(err_msg)
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Explicitly initialize tracing with INFO level by default
    // tracing_subscriber::fmt::init(); // Remove this line
    fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("info".parse().unwrap()))
        .init();
    dotenv().ok();
    let cli = Cli::parse();

    let desktop_instance_result = Desktop::new(false, true);
    let desktop_instance = match desktop_instance_result {
        Ok(instance) => instance,
        Err(e) => {
            eprintln!("FATAL: Failed to initialize Desktop Automation Engine: {}", e);
            tracing::error!("Failed to initialize Desktop Automation Engine: {}", e);
            std::process::exit(1);
        }
    };

    // Handle test flags
    let mut ran_test = false;
    let mut test_result: Result<(), String> = Ok(());
    if cli.test_focused_element_ns {
        #[cfg(target_os = "macos")] { test_result = run_test_focused_element_ns(); ran_test = true; }
        #[cfg(not(target_os = "macos"))] { eprintln!("Error: --test-focused-element-ns is only supported on macOS."); test_result = Err("Unsupported platform".to_string()); ran_test = true; }
    }
    if cli.check_accessibility {
        #[cfg(target_os = "macos")] { test_result = run_check_accessibility(); ran_test = true; }
        #[cfg(not(target_os = "macos"))] { println!("Warning: --check-accessibility is macOS-specific. Skipping check."); ran_test = true; }
    }
    if ran_test {
        match test_result { Ok(_) => std::process::exit(0), Err(_) => std::process::exit(1), }
    }

    println!("No test flags detected, launching Tauri application...");
    let desktop_arc = Arc::new(desktop_instance);

    // Create the AppState
    let app_state = AppState {
        desktop: desktop_arc.clone(),
        last_edited_file: Mutex::new(None), // Initialize undo state
        previous_content: Mutex::new(None), // Initialize undo state
    };

    // --- Tauri Application Builder ---
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .manage(app_state) // Manage the AppState
        .invoke_handler(tauri::generate_handler![
            greet,
            list_apps,
            check_server_status,
            submit_query,
            get_logs,
            tts::replicate::invoke_replicate_tts, // Assuming these are correct
            tts::elevenlabs::invoke_elevenlabs_tts, // Assuming these are correct
            capture_screenshot_command,
            dev_get_focused_element_info,
            capture_element_screenshot_command,
            dev_click_focused_element,
            dev_type_text,
            dev_press_key,
            dev_open_application,
            dev_open_url,
            dev_scroll_window,
            dev_global_type_text,
            dev_get_clipboard,
            dev_set_clipboard,
            dev_hold_key,
            dev_release_key,
            dev_wait,
            dev_find_element_by_selector,
            dev_click_element_by_selector,
        ])
        .on_menu_event(|app, event| { // Attach menu event handler directly
            let window = app.get_webview_window("main").unwrap();
            match event.id.as_ref() {
                "quit" => {
                    println!("[Menu] Quit requested.");
                    app.exit(0);
                }
                "toggle" => { // Keep toggle for floating bar if needed elsewhere, or remove if only tray controls it
                    println!("[Menu] Toggle floating bar requested.");
                    if let Some(window) = app.get_webview_window("floating-bar") {
                        match window.is_visible() {
                            Ok(true) => window.hide().unwrap(),
                            Ok(false) => {
                                window.show().unwrap();
                                window.set_focus().unwrap();
                            },
                            Err(e) => eprintln!("[Menu Error] checking floating bar visibility: {}", e),
                        }
                    } else {
                         eprintln!("[Menu Error] Floating bar window not found for toggle.");
                    }
                }
                "toggle_panel" => {
                    println!("[Menu] Toggle panel requested.");
                    let main_window_visible = window.is_visible().unwrap_or(false);
                    if main_window_visible {
                        window.hide().unwrap();
                        // Optionally update menu item text - requires mutable access or rebuilding menu
                        if let Some(MenuItemKind::MenuItem(item)) = app.menu().unwrap().get("toggle_panel") {
                            item.set_text("Show Panel").unwrap();
                        }
                    } else {
                        window.show().unwrap();
                        window.set_focus().unwrap();
                         // Optionally update menu item text
                        if let Some(MenuItemKind::MenuItem(item)) = app.menu().unwrap().get("toggle_panel") {
                            item.set_text("Hide Panel").unwrap();
                        }
                    }
                }
                _ => {
                     println!("[Menu] Unhandled event: {:?}", event.id);
                }
            }
        })
        .on_tray_icon_event(|tray, event| { // Attach tray event handler directly
            // Use if let for specific event types
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                .. // Ignore other fields like position, rect
            } = event
            {
                println!("[Tray] Left click detected.");
                let app = tray.app_handle();
                // Toggle the floating bar window on left click
                if let Some(window) = app.get_webview_window("floating-bar") {
                    match window.is_visible() {
                        Ok(true) => window.hide().unwrap(),
                        Ok(false) => {
                            window.show().unwrap();
                            window.set_focus().unwrap();
                            println!("[Tray] Floating bar shown and focused.");
                        },
                        Err(e) => eprintln!("[Tray Error] checking floating bar visibility: {}", e),
                    }
                } else {
                     eprintln!("[Tray Error] Floating bar window not found on left click.");
                }
            }
            // No longer handle RightClick here
            // else if let TrayIconEvent::RightClick { ... } = event { ... }
            // Optionally handle other events here
            // else {
            //     println!("[Tray] Unhandled event: {:?}", event);
            // }
        })
        .setup(|app| {
            // --- Tray Icon Setup ---
            let app_handle = app.handle().clone();

            // 1. Build the menu
            let toggle_panel_item = MenuItemBuilder::new("Show Panel") // Start with Show Panel
                .id("toggle_panel")
                .build(&app_handle)
                .expect("Failed to build toggle_panel item");
            let quit_item = PredefinedMenuItem::quit(&app_handle, Some("Quit dotdot"))
                .expect("Failed to build quit item");

            let menu = Menu::with_items(&app_handle, &[
                &toggle_panel_item,
                &quit_item,
            ]).expect("Failed to build tray menu");

            // 2. Build the TrayIcon
            // Load icon bytes
            let icon_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../src/assets/tray-Template.png");
            let icon_bytes = std::fs::read(&icon_path).expect("Failed to read icon file");
            let icon = Image::from_bytes(&icon_bytes).expect("Failed to create image from bytes");

            let _tray = tauri::tray::TrayIconBuilder::new() // Use qualified path
                .menu(&menu)
                .icon(icon)
                .icon_as_template(true)
                .tooltip("dotdot") // Add tooltip
                // Use show_menu_on_left_click instead of deprecated menu_on_left_click
                .show_menu_on_left_click(false)
                // .on_menu_event(...) // Optional specific handler - relying on global one
                // Pass handle as reference
                .build(&app_handle)
                .expect("Failed to build tray icon");
            // --- End Tray Icon Setup ---

            // --- Original Setup Code ---
            // Ensure the main window exists before proceeding with event handling setup
            let main_window = app.get_webview_window("main")
               .ok_or_else(|| "Fatal: Main window not found during setup".to_string())?;

            let window_event_handle = app.handle().clone();
            main_window.on_window_event(move |event| {
                match event {
                    WindowEvent::CloseRequested { api, .. } => {
                        api.prevent_close();
                        let window = window_event_handle.get_webview_window("main").unwrap();
                        window.hide().unwrap();
                        tracing::info!("[INFO] Main window hidden via close request.");
                        // Update menu item text when window is closed via 'X'
                        // This needs access to the menu item handle, which is tricky here.
                        // Consider updating the text *when the menu is built* instead,
                        // or fetching the handle via app_handle.tray_by_id(&id)?.get_item(&item_id)
                        // if tray id is known/stored.
                        // For simplicity, removing this update attempt for now.
                        // if let Some(MenuItemKind::MenuItem(item)) = window_event_handle.menu().unwrap().get("toggle_panel") {
                        //     item.set_text("Show Panel").unwrap();
                        //     tracing::info!("[INFO] Toggle panel menu item text set to 'Show Panel' due to close request.");
                        // }
                    }
                    _ => {}
                }
            });
            // --- End Window Event Handling ---

            // Check for floating bar window
            if let Some(_floating_bar) = app.get_webview_window("floating-bar") {
                println!("Floating bar window found.");
            } else {
                eprintln!("Warning: Floating bar window not found during setup.");
            }

            Ok(())
        });

    // Run the application
    builder
        .run(tauri::generate_context!()) // Use the context generated by tauri-build
        .expect("error while running tauri application");
}

// Unit tests module
#[cfg(test)]
mod tests {
    use super::*;

    fn add(a: i32, b: i32) -> i32 {
        a + b
    }

    #[test]
    fn test_simple_addition() {
        assert_eq!(add(2, 2), 4, "Check basic addition");
    }

    #[test]
    fn test_focused_element_info_placeholder() {
        assert!(true, "Placeholder test for focused element concept");
    }
}

#[allow(unused_variables)] // desktop parameter is not used currently
fn list_tools(desktop: &Arc<Desktop>) -> Vec<ToolDefinition> {
    // Keep existing tools and add new ones
    let tools = vec![
        // --- Existing Tools (Corrected Construction) ---
        ToolDefinition {
            name: "get_focused_element_info".to_string(),
            description: "Get information about the currently focused UI element.".to_string(),
            input_schema: ToolInputSchema {
                type_: "object".to_string(),
                properties: HashMap::new(), // No properties
                required: Vec::new(),       // No required fields
            },
        },
        ToolDefinition {
            name: "click_focused_element".to_string(),
            description: "Clicks the center of the currently focused UI element.".to_string(),
            input_schema: ToolInputSchema {
                type_: "object".to_string(),
                properties: HashMap::new(),
                required: Vec::new(),
            },
        },
        ToolDefinition {
            name: "type_text".to_string(),
            description: "Types the given text into the currently focused element.".to_string(),
            input_schema: ToolInputSchema {
                type_: "object".to_string(),
                properties: {
                    let mut props = HashMap::new();
                    props.insert("text".to_string(), ToolParameter { type_: "string".to_string(), description: "The text to type.".to_string() });
                    props
                },
                required: vec!["text".to_string()],
            },
        },
        ToolDefinition {
            name: "press_key".to_string(),
            description: "Presses a single key, optionally with a modifier.".to_string(),
            input_schema: ToolInputSchema {
                type_: "object".to_string(),
                properties: {
                    let mut props = HashMap::new();
                    props.insert("key".to_string(), ToolParameter { type_: "string".to_string(), description: "The key to press (e.g., 'a', 'Enter').".to_string() });
                    props.insert("modifier".to_string(), ToolParameter { type_: "string".to_string(), description: "Optional modifier key (e.g., 'cmd', 'ctrl').".to_string() }); // Add enum validation if needed
                    props
                },
                required: vec!["key".to_string()],
            },
        },
        ToolDefinition {
            name: "open_application".to_string(),
            description: "Opens an application by its name.".to_string(),
            input_schema: ToolInputSchema {
                type_: "object".to_string(),
                properties: {
                    let mut props = HashMap::new();
                    props.insert("app_name".to_string(), ToolParameter { type_: "string".to_string(), description: "The name of the application to open.".to_string() });
                    props
                },
                required: vec!["app_name".to_string()],
            },
        },
        ToolDefinition {
            name: "open_url".to_string(),
            description: "Opens a URL in the default web browser.".to_string(),
            input_schema: ToolInputSchema {
                type_: "object".to_string(),
                properties: {
                    let mut props = HashMap::new();
                    props.insert("url".to_string(), ToolParameter { type_: "string".to_string(), description: "The URL to open.".to_string() });
                    props
                },
                required: vec!["url".to_string()],
            },
        },
        ToolDefinition {
            name: "scroll_window".to_string(),
            description: "Scrolls the currently active window or element.".to_string(),
            input_schema: ToolInputSchema {
                type_: "object".to_string(),
                properties: {
                    let mut props = HashMap::new();
                    props.insert("direction".to_string(), ToolParameter { type_: "string".to_string(), description: "Direction (up, down, left, right).".to_string() });
                    props.insert("amount".to_string(), ToolParameter { type_: "number".to_string(), description: "Amount to scroll.".to_string() });
                    props
                },
                required: vec!["direction".to_string(), "amount".to_string()],
            },
        },
        ToolDefinition {
            name: "capture_screenshot".to_string(),
            description: "Captures a screenshot of the entire screen.".to_string(),
            input_schema: ToolInputSchema {
                type_: "object".to_string(),
                properties: HashMap::new(),
                required: Vec::new(),
            },
        },
        ToolDefinition {
            name: "capture_element_screenshot".to_string(),
            description: "Captures a screenshot of the currently focused UI element.".to_string(),
            input_schema: ToolInputSchema {
                type_: "object".to_string(),
                properties: HashMap::new(),
                required: Vec::new(),
            },
        },
        // --- Added Tools (Corrected Construction) ---
        ToolDefinition {
            name: "wait".to_string(),
            description: "Pauses execution for a specified duration.".to_string(),
            input_schema: ToolInputSchema {
                type_: "object".to_string(),
                properties: {
                    let mut props = HashMap::new();
                    props.insert("duration_ms".to_string(), ToolParameter { type_: "integer".to_string(), description: "Wait duration in milliseconds.".to_string() });
                    props
                },
                required: vec!["duration_ms".to_string()],
            },
        },
        ToolDefinition {
            name: "cursor_position".to_string(),
            description: "Gets the current mouse cursor position.".to_string(),
            input_schema: ToolInputSchema {
                type_: "object".to_string(),
                properties: HashMap::new(),
                required: Vec::new(),
            },
        },
        ToolDefinition {
            name: "mouse_move".to_string(),
            description: "Moves the mouse cursor to specified coordinates.".to_string(),
            input_schema: ToolInputSchema {
                type_: "object".to_string(),
                properties: {
                    let mut props = HashMap::new();
                    props.insert("x".to_string(), ToolParameter { type_: "number".to_string(), description: "Target X coordinate.".to_string() });
                    props.insert("y".to_string(), ToolParameter { type_: "number".to_string(), description: "Target Y coordinate.".to_string() });
                    props
                },
                required: vec!["x".to_string(), "y".to_string()],
            },
        },
        ToolDefinition {
            name: "left_mouse_down".to_string(),
            description: "Presses and holds the left mouse button at coordinates.".to_string(),
            input_schema: ToolInputSchema {
                type_: "object".to_string(),
                properties: {
                     let mut props = HashMap::new();
                    props.insert("x".to_string(), ToolParameter { type_: "number".to_string(), description: "X coordinate.".to_string() });
                    props.insert("y".to_string(), ToolParameter { type_: "number".to_string(), description: "Y coordinate.".to_string() });
                    props
                },
                required: vec!["x".to_string(), "y".to_string()],
            },
        },
        ToolDefinition {
            name: "left_mouse_up".to_string(),
            description: "Releases the left mouse button at coordinates.".to_string(),
            input_schema: ToolInputSchema {
                type_: "object".to_string(),
                properties: {
                     let mut props = HashMap::new();
                    props.insert("x".to_string(), ToolParameter { type_: "number".to_string(), description: "X coordinate.".to_string() });
                    props.insert("y".to_string(), ToolParameter { type_: "number".to_string(), description: "Y coordinate.".to_string() });
                    props
                },
                required: vec!["x".to_string(), "y".to_string()],
            },
        },
         ToolDefinition {
            name: "left_click".to_string(),
            description: "Performs a left mouse click at coordinates.".to_string(),
             input_schema: ToolInputSchema {
                type_: "object".to_string(),
                properties: {
                     let mut props = HashMap::new();
                    props.insert("x".to_string(), ToolParameter { type_: "number".to_string(), description: "X coordinate.".to_string() });
                    props.insert("y".to_string(), ToolParameter { type_: "number".to_string(), description: "Y coordinate.".to_string() });
                    props
                },
                required: vec!["x".to_string(), "y".to_string()],
            },
        },
        ToolDefinition {
            name: "right_click".to_string(),
            description: "Performs a right mouse click at coordinates.".to_string(),
            input_schema: ToolInputSchema {
                type_: "object".to_string(),
                properties: {
                     let mut props = HashMap::new();
                    props.insert("x".to_string(), ToolParameter { type_: "number".to_string(), description: "X coordinate.".to_string() });
                    props.insert("y".to_string(), ToolParameter { type_: "number".to_string(), description: "Y coordinate.".to_string() });
                    props
                },
                required: vec!["x".to_string(), "y".to_string()],
            },
        },
        ToolDefinition {
            name: "middle_click".to_string(),
            description: "Performs a middle mouse click at coordinates.".to_string(),
            input_schema: ToolInputSchema {
                type_: "object".to_string(),
                properties: {
                     let mut props = HashMap::new();
                    props.insert("x".to_string(), ToolParameter { type_: "number".to_string(), description: "X coordinate.".to_string() });
                    props.insert("y".to_string(), ToolParameter { type_: "number".to_string(), description: "Y coordinate.".to_string() });
                    props
                },
                required: vec!["x".to_string(), "y".to_string()],
            },
        },
        ToolDefinition {
            name: "double_click".to_string(),
            description: "Performs a double left mouse click at coordinates.".to_string(),
            input_schema: ToolInputSchema {
                type_: "object".to_string(),
                properties: {
                     let mut props = HashMap::new();
                    props.insert("x".to_string(), ToolParameter { type_: "number".to_string(), description: "X coordinate.".to_string() });
                    props.insert("y".to_string(), ToolParameter { type_: "number".to_string(), description: "Y coordinate.".to_string() });
                    props
                },
                required: vec!["x".to_string(), "y".to_string()],
            },
        },
        ToolDefinition {
            name: "triple_click".to_string(),
            description: "Performs a triple left mouse click at coordinates.".to_string(),
            input_schema: ToolInputSchema {
                type_: "object".to_string(),
                properties: {
                     let mut props = HashMap::new();
                    props.insert("x".to_string(), ToolParameter { type_: "number".to_string(), description: "X coordinate.".to_string() });
                    props.insert("y".to_string(), ToolParameter { type_: "number".to_string(), description: "Y coordinate.".to_string() });
                    props
                },
                required: vec!["x".to_string(), "y".to_string()],
            },
        },
         ToolDefinition {
            name: "left_click_drag".to_string(),
            description: "Drags the mouse with the left button held down.".to_string(),
             input_schema: ToolInputSchema {
                type_: "object".to_string(),
                properties: {
                    let mut props = HashMap::new();
                    props.insert("start_x".to_string(), ToolParameter { type_: "number".to_string(), description: "Starting X coordinate.".to_string() });
                    props.insert("start_y".to_string(), ToolParameter { type_: "number".to_string(), description: "Starting Y coordinate.".to_string() });
                    props.insert("end_x".to_string(), ToolParameter { type_: "number".to_string(), description: "Ending X coordinate.".to_string() });
                    props.insert("end_y".to_string(), ToolParameter { type_: "number".to_string(), description: "Ending Y coordinate.".to_string() });
                    props
                },
                required: vec!["start_x".to_string(), "start_y".to_string(), "end_x".to_string(), "end_y".to_string()],
            },
        },
        ToolDefinition {
            name: "scroll_at_position".to_string(),
            description: "Scrolls the view at a specific coordinate.".to_string(),
            input_schema: ToolInputSchema {
                type_: "object".to_string(),
                properties: {
                    let mut props = HashMap::new();
                    props.insert("x".to_string(), ToolParameter { type_: "number".to_string(), description: "X coordinate to scroll at.".to_string() });
                    props.insert("y".to_string(), ToolParameter { type_: "number".to_string(), description: "Y coordinate to scroll at.".to_string() });
                    props.insert("direction".to_string(), ToolParameter { type_: "string".to_string(), description: "Direction (up, down, left, right).".to_string() });
                    props.insert("amount".to_string(), ToolParameter { type_: "number".to_string(), description: "Amount to scroll.".to_string() });
                    props
                },
                required: vec!["x".to_string(), "y".to_string(), "direction".to_string(), "amount".to_string()],
            },
        },
        ToolDefinition {
            name: "hold_key".to_string(),
            description: "Presses and holds a modifier key.".to_string(),
             input_schema: ToolInputSchema {
                type_: "object".to_string(),
                properties: {
                    let mut props = HashMap::new();
                    props.insert("key".to_string(), ToolParameter { type_: "string".to_string(), description: "Modifier key to hold (cmd, ctrl, alt, shift).".to_string() });
                    props
                },
                required: vec!["key".to_string()],
            },
        },
        ToolDefinition {
            name: "release_key".to_string(),
            description: "Releases a previously held modifier key.".to_string(),
            input_schema: ToolInputSchema {
                type_: "object".to_string(),
                properties: {
                    let mut props = HashMap::new();
                    props.insert("key".to_string(), ToolParameter { type_: "string".to_string(), description: "Modifier key to release.".to_string() });
                    props
                },
                required: vec!["key".to_string()],
            },
        },
        ToolDefinition {
            name: "get_clipboard_content".to_string(),
            description: "Gets the current system clipboard content.".to_string(),
            input_schema: ToolInputSchema {
                type_: "object".to_string(),
                properties: HashMap::new(),
                required: Vec::new(),
            },
        },
        ToolDefinition {
            name: "set_clipboard_content".to_string(),
            description: "Sets the system clipboard content.".to_string(),
            input_schema: ToolInputSchema {
                type_: "object".to_string(),
                properties: {
                     let mut props = HashMap::new();
                    props.insert("content".to_string(), ToolParameter { type_: "string".to_string(), description: "Text content to set.".to_string() });
                    props
                },
                required: vec!["content".to_string()],
            },
        },
        // --- Text Editor Tools ---
        ToolDefinition {
            name: "text_editor_view".to_string(),
            description: "Reads and returns the content of a text file.".to_string(),
            input_schema: ToolInputSchema {
                type_: "object".to_string(),
                properties: {
                     let mut props = HashMap::new();
                    props.insert("file_path".to_string(), ToolParameter { type_: "string".to_string(), description: "Absolute path to the file.".to_string() });
                    props
                },
                required: vec!["file_path".to_string()],
            },
        },
        ToolDefinition {
            name: "text_editor_create".to_string(),
            description: "Creates/overwrites a text file with given content.".to_string(),
            input_schema: ToolInputSchema {
                type_: "object".to_string(),
                properties: {
                     let mut props = HashMap::new();
                    props.insert("file_path".to_string(), ToolParameter { type_: "string".to_string(), description: "Absolute path for the file.".to_string() });
                    props.insert("content".to_string(), ToolParameter { type_: "string".to_string(), description: "Initial content.".to_string() });
                    props
                },
                required: vec!["file_path".to_string(), "content".to_string()],
            },
        },
        ToolDefinition {
            name: "text_editor_insert".to_string(),
            description: "Inserts text into a file at a specific line number.".to_string(),
            input_schema: ToolInputSchema {
                type_: "object".to_string(),
                properties: {
                    let mut props = HashMap::new();
                    props.insert("file_path".to_string(), ToolParameter { type_: "string".to_string(), description: "Absolute path to the file.".to_string() });
                    props.insert("line_number".to_string(), ToolParameter { type_: "integer".to_string(), description: "1-based line number to insert at.".to_string() });
                    props.insert("text_to_insert".to_string(), ToolParameter { type_: "string".to_string(), description: "Text to insert.".to_string() });
                    props
                },
                required: vec!["file_path".to_string(), "line_number".to_string(), "text_to_insert".to_string()],
            },
        },
        ToolDefinition {
            name: "text_editor_str_replace".to_string(),
            description: "Replaces all occurrences of a string in a file.".to_string(),
            input_schema: ToolInputSchema {
                type_: "object".to_string(),
                properties: {
                     let mut props = HashMap::new();
                    props.insert("file_path".to_string(), ToolParameter { type_: "string".to_string(), description: "Absolute path to the file.".to_string() });
                    props.insert("find_text".to_string(), ToolParameter { type_: "string".to_string(), description: "Text to find.".to_string() });
                    props.insert("replace_text".to_string(), ToolParameter { type_: "string".to_string(), description: "Replacement text.".to_string() });
                    props
                },
                required: vec!["file_path".to_string(), "find_text".to_string(), "replace_text".to_string()],
            },
        },
        // text_editor_undo_edit (Corrected definition)
         ToolDefinition {
             name: "text_editor_undo_edit".to_string(),
             description: "Undoes the last text editing operation (create, insert, replace).".to_string(),
             input_schema: ToolInputSchema {
                 type_: "object".to_string(),
                 properties: {
                     let mut props = HashMap::new();
                     props.insert("file_path".to_string(), ToolParameter { type_: "string".to_string(), description: "The path to the file for which the last edit should be undone (used for confirmation).".to_string() });
                     props
                 },
                 required: vec!["file_path".to_string()],
             },
         },
        // --- Bash Tool ---
        ToolDefinition {
            name: "bash".to_string(),
            description: "Executes a shell command.".to_string(),
             input_schema: ToolInputSchema {
                type_: "object".to_string(),
                properties: {
                    let mut props = HashMap::new();
                    props.insert("command".to_string(), ToolParameter { type_: "string".to_string(), description: "Command line to execute.".to_string() });
                    props.insert("timeout_seconds".to_string(), ToolParameter { type_: "integer".to_string(), description: "Optional timeout (not implemented).".to_string() });
                    props
                },
                required: vec!["command".to_string()],
            },
        },
    ];

    // Add platform-specific tools or modify existing ones if needed
    #[cfg(target_os = "macos")]
    {
        // Example: Add macOS specific tool if any
    }

    tools
}

// Helper to extract string param or return error JSON
#[allow(dead_code)] // Allow dead code for helper potentially used by call_tool
fn get_string_param(input: &Value, key: &str) -> Result<String, Value> {
    input[key]
        .as_str()
        .map(String::from)
        .ok_or_else(|| json!({"error": format!("Missing or invalid string parameter: {}", key)}))
}

// Helper to extract optional string param (Corrected)
#[allow(dead_code)] // Allow dead code for helper potentially used by call_tool
fn get_optional_string_param(input: &Value, key: &str) -> Result<Option<String>, Value> {
    match input.get(key) {
        Some(value) => {
            if value.is_null() {
                Ok(None) // Treat null as None
            } else {
                value.as_str()
                     .map(|s| Ok(Some(s.to_string())))
                     .unwrap_or_else(|| Err(json!({"error": format!("Invalid optional string parameter type: {}", key)})))
            }
        }
        None => Ok(None), // Key not present is Ok(None)
    }
}


// Helper to extract f64 param or return error JSON
#[allow(dead_code)] // Allow dead code for helper potentially used by call_tool
fn get_f64_param(input: &Value, key: &str) -> Result<f64, Value> {
    input[key]
        .as_f64()
        .ok_or_else(|| json!({"error": format!("Missing or invalid number parameter: {}", key)}))
}

// Helper to extract u64 param or return error JSON
#[allow(dead_code)] // Allow dead code for helper potentially used by call_tool
fn get_u64_param(input: &Value, key: &str) -> Result<u64, Value> {
    input[key]
        .as_u64()
        .ok_or_else(|| json!({"error": format!("Missing or invalid integer parameter: {}", key)}))
}

// Helper to extract i64 param or return error JSON
#[allow(dead_code)] // Allow dead code for helper potentially used by call_tool
fn get_i64_param(input: &Value, key: &str) -> Result<i64, Value> {
    input[key]
        .as_i64()
        .ok_or_else(|| json!({"error": format!("Missing or invalid integer parameter: {}", key)}))
}

// Helper function to get an optional u64 parameter from JSON
#[allow(dead_code)] // Allow dead code for helper potentially used by call_tool
fn get_optional_u64_param(input: &Value, key: &str) -> Result<Option<u64>, Value> {
    match input.get(key) {
        Some(value) => {
            if value.is_null() {
                Ok(None) // Treat null as None
            } else if let Some(num) = value.as_u64() {
                Ok(Some(num))
            } else {
                // Use value.to_string() or describe the type in the error message
                Err(json!({ "error": format!("Invalid type for parameter '{}': expected u64 or null, got type {}", key, value.to_string()) }))
            }
        }
        None => Ok(None), // Key not present
    }
}

// Tool call dispatcher (Corrected Error Handling and Return Type)
#[allow(dead_code)] // Allow dead code for helper potentially used by submit_query
async fn call_tool(
    desktop: &Arc<Desktop>,
    app_handle: &AppHandle,
    tool_name: &str,
    input: &Value,
    state: &State<'_, AppState>, // Correctly include state here in the definition
) -> Result<Value, Value> { // Returns Result<SuccessJson, ErrorJson>
    info!(tool_name = %tool_name, input = ?input, "Calling tool");

    // Use match to handle errors from param helpers
    match tool_name {
        "get_focused_element_info" => {
             match desktop.focused_element() { // Changed from get_focused_element
                Ok(element) => {
                    let attrs = element.attributes();
                    serde_json::to_value(&attrs).map_err(|e| json!({"error": format!("Failed to serialize element info: {}", e)}))
                },
                Err(e) => Err(json!({"error": format!("Failed to get focused element: {}", e)})),
            }
        }
        "click_focused_element" => {
            match desktop.focused_element() { // Changed from get_focused_element
                Ok(element) => {
                    match element.click() {
                         Ok(_) => Ok(json!({"success": true, "message": "Clicked focused element."})),
                         Err(e) => Err(json!({"error": format!("Failed to click focused element: {}", e)})),
                    }
                },
                Err(e) => Err(json!({"error": format!("Failed to get focused element for clicking: {}", e)})),
            }
        }
        "type_text" => {
            match get_string_param(input, "text") {
                Ok(text) => match desktop.type_text(&text) {
                    Ok(_) => Ok(json!({"success": true, "message": "Text typed."})),
                    Err(e) => Err(json!({"error": format!("Failed to type text: {}", e)})),
                },
                Err(e) => Err(e), // Propagate param parsing error
            }
        }
        "press_key" => {
             match (get_string_param(input, "key"), get_optional_string_param(input, "modifier")) {
                (Ok(key), Ok(modifier)) => {
                     match desktop.press_key(&key, modifier.as_deref()) {
                         Ok(_) => Ok(json!({"success": true, "message": format!("Key '{}' pressed.", key)})),
                         Err(e) => Err(json!({"error": format!("Failed to press key: {}", e)})),
                     }
                 }
                 (Err(e), _) | (_, Err(e)) => Err(e), // Propagate param parsing error
             }
        }
        "open_application" => {
             match get_string_param(input, "app_name") {
                 Ok(app_name) => match desktop.open_application(&app_name) {
                     Ok(_) => Ok(json!({"success": true, "message": format!("Application '{}' opened.", app_name)})),
                     Err(e) => Err(json!({"error": format!("Failed to open application: {}", e)})),
                 },
                 Err(e) => Err(e),
             }
        }
        "open_url" => {
             match get_string_param(input, "url") {
                 Ok(url) => match desktop.open_url(&url, None) {
                     Ok(_) => Ok(json!({"success": true, "message": format!("URL '{}' opened.", url)})),
                     Err(e) => Err(json!({"error": format!("Failed to open URL: {}", e)})),
                 },
                 Err(e) => Err(e),
             }
        }
        "scroll_window" => { // Maps to scroll_at_current_position
            match (get_string_param(input, "direction"), get_f64_param(input, "amount")) {
                 (Ok(direction), Ok(amount)) => match desktop.scroll_at_current_position(&direction, amount) {
                     Ok(_) => Ok(json!({"success": true, "message": format!("Scrolled {} by {}.", direction, amount)})),
                     Err(e) => Err(json!({"error": format!("Failed to scroll window: {}", e)})),
                 },
                 (Err(e), _) | (_, Err(e)) => Err(e),
            }
        }
        "capture_screenshot" => {
            #[cfg(target_os = "macos")]
            {
                match macos_utils::capture_and_encode_screenshot() {
                    Ok(base64_string) => {
                        app_handle.notification().builder().title("Screenshot").body("Screenshot captured.").show().ok();
                        Ok(json!({"success": true, "image_base64": base64_string}))
                    },
                    Err(e) => Err(json!({"error": format!("Failed to capture screenshot: {}", e)})),
                }
            }
            #[cfg(not(target_os = "macos"))]
            {
                 Err(json!({"error": "Screenshot capture is only supported on macOS currently."}))
            }
        }
         "capture_element_screenshot" => {
            #[cfg(target_os = "macos")]
            {
                 match desktop.focused_element() { // Changed from get_focused_element
                     Ok(focused_element) => {
                        if let Some(macos_element) = focused_element.as_any().downcast_ref::<MacOSUIElement>() {
                             match macos_utils::capture_element_screenshot(macos_element) {
                                Ok(base64_string) => {
                                     app_handle.notification().builder().title("Element Screenshot").body("Focused element screenshot captured.").show().ok();
                                     Ok(json!({"success": true, "image_base64": base64_string}))
                                },
                                Err(e) => Err(json!({"error": format!("Failed to capture element screenshot: {}", e)})),
                             }
                        } else {
                            Err(json!({"error": "Focused element is not a MacOSUIElement"}))
                        }
                    },
                    Err(e) => Err(json!({"error": format!("Failed to get focused element for screenshot: {}", e)})),
                }
            }
             #[cfg(not(target_os = "macos"))]
            {
                 Err(json!({"error": "Element screenshot capture is only supported on macOS currently."}))
            }
        }
        // --- Added Tool Handlers ---
        "wait" => {
             match get_u64_param(input, "duration_ms") {
                 Ok(duration_ms) => match desktop.wait(duration_ms) {
                     Ok(_) => Ok(json!({"success": true, "message": format!("Waited for {} ms.", duration_ms)})),
                     Err(e) => Err(json!({"error": format!("Wait failed: {}", e)})),
                 },
                 Err(e) => Err(e),
            }
        }
        "cursor_position" => {
            match desktop.cursor_position() {
                Ok((x, y)) => Ok(json!({"success": true, "x": x, "y": y})),
                Err(e) => Err(json!({"error": format!("Failed to get cursor position: {}", e)})),
            }
        }
        "mouse_move" => {
            match (get_f64_param(input, "x"), get_f64_param(input, "y")) {
                (Ok(x), Ok(y)) => match desktop.mouse_move(x, y) {
                    Ok(_) => Ok(json!({"success": true, "message": format!("Mouse moved to ({}, {}).", x, y)})),
                    Err(e) => Err(json!({"error": format!("Failed to move mouse: {}", e)})),
                },
                (Err(e), _) | (_, Err(e)) => Err(e),
            }
        }
         "left_mouse_down" => {
             match (get_f64_param(input, "x"), get_f64_param(input, "y")) {
                (Ok(x), Ok(y)) => match desktop.left_mouse_down(x, y) {
                    Ok(_) => Ok(json!({"success": true, "message": "Left mouse button pressed down."})),
                    Err(e) => Err(json!({"error": format!("Failed to press left mouse button down: {}", e)})),
                },
                (Err(e), _) | (_, Err(e)) => Err(e),
            }
        }
        "left_mouse_up" => {
            match (get_f64_param(input, "x"), get_f64_param(input, "y")) {
                (Ok(x), Ok(y)) => match desktop.left_mouse_up(x, y) {
                     Ok(_) => Ok(json!({"success": true, "message": "Left mouse button released."})),
                     Err(e) => Err(json!({"error": format!("Failed to release left mouse button: {}", e)})),
                 },
                 (Err(e), _) | (_, Err(e)) => Err(e),
            }
        }
        "left_click" => {
             match (get_f64_param(input, "x"), get_f64_param(input, "y")) {
                (Ok(x), Ok(y)) => match desktop.left_click(x, y) {
                    Ok(_) => Ok(json!({"success": true, "message": format!("Left clicked at ({}, {}).", x, y)})),
                    Err(e) => Err(json!({"error": format!("Failed to perform left click: {}", e)})),
                },
                (Err(e), _) | (_, Err(e)) => Err(e),
            }
        }
        "right_click" => {
             match (get_f64_param(input, "x"), get_f64_param(input, "y")) {
                 (Ok(x), Ok(y)) => match desktop.right_click(x, y) {
                     Ok(_) => Ok(json!({"success": true, "message": format!("Right clicked at ({}, {}).", x, y)})),
                     Err(e) => Err(json!({"error": format!("Failed to perform right click: {}", e)})),
                 },
                 (Err(e), _) | (_, Err(e)) => Err(e),
            }
        }
        "middle_click" => {
             match (get_f64_param(input, "x"), get_f64_param(input, "y")) {
                 (Ok(x), Ok(y)) => match desktop.middle_click(x, y) {
                     Ok(_) => Ok(json!({"success": true, "message": format!("Middle clicked at ({}, {}).", x, y)})),
                     Err(e) => Err(json!({"error": format!("Failed to perform middle click: {}", e)})),
                 },
                 (Err(e), _) | (_, Err(e)) => Err(e),
             }
        }
        "double_click" => {
            match (get_f64_param(input, "x"), get_f64_param(input, "y")) {
                 (Ok(x), Ok(y)) => match desktop.double_click(x, y) {
                     Ok(_) => Ok(json!({"success": true, "message": format!("Double clicked at ({}, {}).", x, y)})),
                     Err(e) => Err(json!({"error": format!("Failed to perform double click: {}", e)})),
                 },
                 (Err(e), _) | (_, Err(e)) => Err(e),
            }
        }
        "triple_click" => {
             match (get_f64_param(input, "x"), get_f64_param(input, "y")) {
                 (Ok(x), Ok(y)) => match desktop.triple_click(x, y) {
                     Ok(_) => Ok(json!({"success": true, "message": format!("Triple clicked at ({}, {}).", x, y)})),
                     Err(e) => Err(json!({"error": format!("Failed to perform triple click: {}", e)})),
                 },
                 (Err(e), _) | (_, Err(e)) => Err(e),
            }
        }
         "left_click_drag" => {
            match (
                get_f64_param(input, "start_x"),
                get_f64_param(input, "start_y"),
                get_f64_param(input, "end_x"),
                get_f64_param(input, "end_y")
            ) {
                (Ok(start_x), Ok(start_y), Ok(end_x), Ok(end_y)) => {
                    match desktop.left_click_drag(start_x, start_y, end_x, end_y) {
                         Ok(_) => Ok(json!({"success": true, "message": format!("Dragged from ({}, {}) to ({}, {}).", start_x, start_y, end_x, end_y)})),
                         Err(e) => Err(json!({"error": format!("Failed to perform drag: {}", e)})),
                     }
                }
                (Err(e), _, _, _) | (_, Err(e), _, _) | (_, _, Err(e), _) | (_, _, _, Err(e)) => Err(e),
            }
        }
        "scroll_at_position" => { // Assuming Desktop has this method wrapping the engine call
              match (
                get_f64_param(input, "x"),
                get_f64_param(input, "y"),
                get_string_param(input, "direction"),
                get_f64_param(input, "amount")
              ) {
                 (Ok(x), Ok(y), Ok(direction), Ok(amount)) => {
                     match desktop.scroll_at_position(x, y, &direction, amount) { // Verify this method exists on Desktop
                         Ok(_) => Ok(json!({"success": true, "message": format!("Scrolled {} by {} at ({}, {}).", direction, amount, x, y)})),
                         Err(e) => Err(json!({"error": format!("Failed to scroll at position: {}", e)})),
                     }
                 }
                 (Err(e), _, _, _) | (_, Err(e), _, _) | (_, _, Err(e), _) | (_, _, _, Err(e)) => Err(e),
             }
        }
         "hold_key" => {
            match get_string_param(input, "key") {
                 Ok(key) => match desktop.hold_key(&key) {
                    Ok(_) => Ok(json!({"success": true, "message": format!("Holding key '{}'.", key)})),
                    Err(e) => Err(json!({"error": format!("Failed to hold key: {}", e)})),
                },
                Err(e) => Err(e),
            }
        }
        "release_key" => {
             match get_string_param(input, "key") {
                 Ok(key) => match desktop.release_key(&key) {
                    Ok(_) => Ok(json!({"success": true, "message": format!("Released key '{}'.", key)})),
                    Err(e) => Err(json!({"error": format!("Failed to release key: {}", e)})),
                },
                Err(e) => Err(e),
            }
        }
        "get_clipboard_content" => {
             match desktop.get_clipboard_content() {
                Ok(content) => Ok(json!({"success": true, "content": content})),
                Err(e) => Err(json!({"error": format!("Failed to get clipboard content: {}", e)})),
            }
        }
        "set_clipboard_content" => {
             match get_string_param(input, "content") {
                Ok(content) => match desktop.set_clipboard_content(&content) {
                    Ok(_) => Ok(json!({"success": true, "message": "Clipboard content set."})),
                    Err(e) => Err(json!({"error": format!("Failed to set clipboard content: {}", e)})),
                },
                Err(e) => Err(e),
            }
        }
        // --- Text Editor Handlers ---
        "text_editor_view" => {
             match get_string_param(input, "file_path") {
                 Ok(file_path) => match fs::read_to_string(&file_path) {
                     Ok(content) => Ok(json!({"success": true, "content": content})),
                     Err(e) => Err(json!({"error": format!("Failed to read file '{}': {}", file_path, e)})),
                 },
                 Err(e) => Err(e),
             }
        }
        "text_editor_create" => {
             match (get_string_param(input, "file_path"), get_string_param(input, "content")) {
                 (Ok(file_path), Ok(content)) => {
                    // --- Undo State Update ---
                    let path = PathBuf::from(file_path.clone());
                    update_undo_state(state, path, None); // Last action was create
                    // --- End Undo State Update ---
                    match fs::write(&file_path, content) {
                        Ok(_) => Ok(json!({"success": true, "message": format!("File '{}' created/overwritten.", file_path)})),
                        Err(e) => Err(json!({"error": format!("Failed to write file '{}': {}", file_path, e)})),
                    }
                 },
                 (Err(e), _) | (_, Err(e)) => Err(e),
             }
        }
        "text_editor_insert" => {
              match (
                 get_string_param(input, "file_path"),
                 get_string_param(input, "text_to_insert"),
                 get_i64_param(input, "line_number")
              ) {
                 (Ok(file_path), Ok(text_to_insert), Ok(line_number)) => {
                    let line_usize = line_number as usize;
                    // --- Undo State Update ---
                    let path = PathBuf::from(file_path.clone());
                    // Read current content *before* modification
                    let current_content = match fs::read_to_string(&path) {
                         Ok(content) => Some(content),
                         Err(e) => {
                              // If the file doesn't exist, it's an error for insert, but technically the previous state is "doesn't exist"
                              warn!(error = %e, file_path = %file_path, "File not found for insert, proceeding but undo will delete.");
                              None
                         }
                    };
                    update_undo_state(state, path.clone(), current_content);
                     // --- End Undo State Update ---
                    match fs::read_to_string(&file_path) {
                        Ok(content) => {
                            let mut lines: Vec<String> = content.lines().map(String::from).collect();
                             if line_usize == 0 || line_usize > lines.len() {
                                lines.push(text_to_insert);
                            } else {
                                lines.insert(line_usize - 1, text_to_insert);
                            }
                            let new_content = lines.join("\n");
                            match fs::write(&file_path, new_content) {
                                Ok(_) => Ok(json!({"success": true, "message": format!("Inserted text into '{}' at line {}.", file_path, line_usize)})),
                                Err(e) => Err(json!({"error": format!("Failed to write updated file '{}': {}", file_path, e)})),
                            }
                        }
                        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                            match fs::write(&file_path, text_to_insert) {
                                Ok(_) => Ok(json!({"success": true, "message": format!("Created file '{}' with inserted text.", file_path)})),
                                Err(write_err) => Err(json!({"error": format!("Failed to create file '{}' for insert: {}", file_path, write_err)})),
                            }
                        },
                        Err(e) => Err(json!({"error": format!("Failed to read file '{}' for insert: {}", file_path, e)})),
                    }
                 }
                 (Err(e), _, _) | (_, Err(e), _) | (_, _, Err(e)) => Err(e),
              }
        }
         "text_editor_str_replace" => {
             match (
                get_string_param(input, "file_path"),
                get_string_param(input, "find_text"),
                get_string_param(input, "replace_text")
             ) {
                 (Ok(file_path), Ok(find_text), Ok(replace_text)) => {
                    // --- Undo State Update ---
                    let path = PathBuf::from(file_path.clone());
                    let current_content = match fs::read_to_string(&path) {
                        Ok(content) => Some(content),
                        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None, // File doesn't exist yet, treat as create
                        Err(e) => return Err(json!({ "status": "error", "message": format!("Failed to read file '{}' before replace: {}", file_path, e) })),
                    };
                    update_undo_state(state, path.clone(), current_content);
                    // --- End Undo State Update ---

                    match str_replace_editor(file_path.clone(), find_text, replace_text) {
                         Ok(msg) => Ok(json!({"success": true, "message": msg})),
                         Err(e) => Err(json!({"error": format!("Failed to replace text in file '{}': {}", file_path, e)})),
                    }
                 }
                  (Err(e), _, _) | (_, Err(e), _) | (_, _, Err(e)) => Err(e),
            }
        }
        "text_editor_undo_edit" => {
            let file_path_param = get_string_param(input, "file_path")?; // Get param for logging/confirmation if needed

            let mut last_edited_path_guard = state.last_edited_file.lock().unwrap();
            let mut previous_content_guard = state.previous_content.lock().unwrap();

            if let Some(path_to_undo) = last_edited_path_guard.take() {
                 // Verify param matches state if desired, though state is source of truth
                if PathBuf::from(&file_path_param) != path_to_undo {
                    warn!(param_path=%file_path_param, state_path=?path_to_undo, "Undo called with path mismatch, using state path.");
                 }

                if let Some(maybe_content) = previous_content_guard.take() {
                    match maybe_content {
                        Some(content) => {
                            // Last action was an edit, restore content
                            match fs::write(&path_to_undo, content) {
                                Ok(_) => Ok(json!({ "status": "success", "message": format!("Undo successful for '{}'.", path_to_undo.display()) })),
                                Err(e) => Err(json!({ "status": "error", "message": format!("Failed to write previous content during undo for '{}': {}", path_to_undo.display(), e) })),
                            }
                        }
                        None => {
                            // Last action was create, delete the file
                            match fs::remove_file(&path_to_undo) {
                                Ok(_) => Ok(json!({ "status": "success", "message": format!("Undo successful for '{}' (file deleted).", path_to_undo.display()) })),
                                // If it already doesn't exist, that's okay for undoing a create
                                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                                     Ok(json!({ "status": "success", "message": format!("Undo successful for '{}' (file was already deleted).", path_to_undo.display()) }))
                                }
                                Err(e) => Err(json!({ "status": "error", "message": format!("Failed to delete file during undo for '{}': {}", path_to_undo.display(), e) })),
                            }
                        }
                    }
                } else {
                     // Should not happen if last_edited_path was Some, indicates state inconsistency
                     error!("Undo state inconsistency: last_edited_file was Some, but previous_content was None.");
                     Err(json!({ "status": "error", "message": "Internal error: Undo state inconsistent." }))
                 }

            } else {
                Err(json!({ "status": "error", "message": "Nothing to undo." }))
            }
        }
        // Comment out the problematic computer_screenshot arm
        /*
        "computer_screenshot" => {
            // ... existing code ...
            // This arm currently returns () instead of Result<Value, Value>,
            // causing E0308. Commenting out until fixed or removed.
            // placeholder to satisfy type checker if uncommented
            // Err(json!({ "status": "error", "message": "Screenshot within call_tool not implemented" }))
        }
        */
        // --- Bash Handler ---
        "bash" => {
            let command_str = get_string_param(input, "command")?;
            let timeout_seconds_opt = get_optional_u64_param(input, "timeout_seconds");
            let timeout = match timeout_seconds_opt {
                Ok(Some(secs)) => Some(Duration::from_secs(secs)),
                Ok(None) => None, // No timeout specified
                Err(e) => return Err(e), // Error parsing timeout
            };

            info!(command = %command_str, ?timeout, "Executing bash command");
            #[cfg(target_os = "macos")] let shell = "/bin/zsh";
            #[cfg(target_os = "windows")] let shell = "cmd";
            #[cfg(target_os = "linux")] let shell = "/bin/bash";
            #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))] let shell = "sh";

            let mut cmd = Command::new(shell);
            cmd.arg("-c").arg(&command_str);

            match cmd.spawn() { // Spawn the process
                Ok(mut child) => {
                    let status_result = match timeout {
                        Some(duration) => child.wait_timeout(duration),
                        None => child.wait().map(Some), // Wait indefinitely if no timeout
                    };

                    match status_result {
                        Ok(Some(status)) => { // Process finished or killed by timeout
                            // Attempt to get output even if killed (might be partial)
                            let output_result = child.wait_with_output();
                            match output_result {
                                Ok(output) => {
                                     let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                                     let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                                     let exit_code = output.status.code();
                                     let timed_out = !status.success() && exit_code.is_none(); // Infer timeout if killed
                                     info!(stdout = %stdout, stderr = %stderr, exit_code = ?exit_code, timed_out = timed_out, "Bash command finished (or timed out)");
                                     Ok(json!({
                                        "success": output.status.success(),
                                        "stdout": stdout,
                                        "stderr": stderr,
                                        "exit_code": exit_code,
                                        "timed_out": timed_out
                                     }))
                                }
                                Err(e) => {
                                    // This might happen if the process was forcefully killed and output couldn't be retrieved
                                    error!(error = %e, command = %command_str, "Failed to get output after command finished/timed out");
                                    Err(json!({"error": format!("Failed to get output for command '{}' after execution: {}", command_str, e), "timed_out": true})) // Assume timeout if we can't get output
                                }
                            }
                        }
                        Ok(None) => { // Timeout occurred
                            info!(command = %command_str, "Bash command timed out");
                            // Attempt to kill the process if it timed out
                            let _ = child.kill(); // Ignore kill errors, best effort
                            let _ = child.wait(); // Ensure it's reaped
                            Err(json!({
                                "error": "Command execution timed out".to_string(),
                                "stdout": "",
                                "stderr": "",
                                "exit_code": null, // No exit code if timed out
                                "timed_out": true
                            }))
                        }
                        Err(e) => { // Error waiting for the process
                            error!(error = %e, command = %command_str, "Error waiting for bash command");
                            Err(json!({"error": format!("Error waiting for command '{}': {}", command_str, e)}))
                        }
                    }
                }
                Err(e) => { // Error spawning the process
                    error!(error = %e, command = %command_str, "Failed to spawn bash command");
                    Err(json!({"error": format!("Failed to spawn command '{}': {}", command_str, e)}))
                }
            }
        }
        // --- Default Case ---
        _ => Err(json!({"error": format!("Unknown tool name: {}", tool_name)})),
    }
}

// Wrapper function to integrate call_tool result into Anthropic flow
#[allow(dead_code)] // Allow dead code for helper potentially used by submit_query
async fn handle_tool_call(
    desktop: &Arc<Desktop>,
    app_handle: &AppHandle,
    tool_name: &str,
    input: &Value,
    state: &State<'_, AppState>, // Added state parameter
) -> Value { // Returns the JSON expected by Anthropic (either success or error content)
    match call_tool(desktop, app_handle, tool_name, input, state).await { // Pass state
        Ok(success_json) => {
            info!(tool_name = %tool_name, output = ?success_json, "Tool call succeeded");
            success_json
        }
        Err(error_json) => {
            error!(tool_name = %tool_name, error = ?error_json, "Tool call failed");
            // Ensure the error JSON has an "error" field for consistency
            if error_json.get("error").is_some() {
                error_json
            } else {
                json!({"error": "An unexpected error occurred", "details": error_json})
            }
        }
    }
}
