#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use clap::Parser;
use computer_use_ai_sdk::{Desktop, ToolDefinition};
use dotenvy::dotenv;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::env;
use std::sync::Arc;
use image::{GenericImageView, ImageFormat};
use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use std::io::Cursor;
use computer_use_ai_sdk::AutomationError;

// Correct V2 Imports
use tauri::menu::{MenuBuilder, MenuItemBuilder, PredefinedMenuItem, MenuItemKind};
use tauri::tray::{TrayIconBuilder, TrayIconEvent, MouseButton, MouseButtonState};
use tauri::{Manager, WindowEvent};

// Only include macos specific imports when targeting macos
#[cfg(target_os = "macos")]
use computer_use_ai_sdk::platforms::macos::utils as macos_utils;

// Need access to the specific element type
#[cfg(target_os = "macos")]
use computer_use_ai_sdk::platforms::macos::element::MacOSUIElement;

// Import the new function
#[cfg(target_os = "macos")]
use computer_use_ai_sdk::platforms::macos::element::get_focused_element_ns_workspace;

// Declare the tts module
mod tts;

// --- CLI Argument Parsing ---

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

// Define a struct to hold the application state
pub(crate) struct AppState {
    desktop: Arc<Desktop>,
}

// --- Anthropic API Structures ---

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

// Structure for tool results (sent back to Anthropic)
#[derive(Serialize)]
struct ToolResultBlock {
    #[serde(rename = "type")]
    type_: String, // Always "tool_result"
    tool_use_id: String,
    content: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    is_error: Option<bool>,
}

// Structure for the combined result of submit_query
#[derive(Serialize)]
struct SubmitQueryResult {
    text: String,
    audio_base64: Option<String>,
}

// --- End Anthropic API Structures ---

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

// Command to capture a screenshot (macOS only for now)
#[cfg(target_os = "macos")]
#[tauri::command]
async fn capture_screenshot_command() -> Result<String, String> {
    match macos_utils::capture_and_encode_screenshot() {
        Ok(base64_string) => Ok(base64_string),
        Err(e) => Err(format!("Failed to capture screenshot: {}", e)),
    }
}

// Stub command for non-macos platforms
#[cfg(not(target_os = "macos"))]
#[tauri::command]
async fn capture_screenshot_command() -> Result<String, String> {
    Err("Screenshot capture is only supported on macOS currently.".to_string())
}

// New command to list applications using the managed state
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

// Command to check if the backend (Desktop instance) initialized
#[tauri::command]
fn check_server_status(state: tauri::State<'_, AppState>) -> bool {
    let _ = state.desktop;
    true
}

// New command for developer tool: Get focused element info
#[tauri::command]
async fn dev_get_focused_element_info(_state: tauri::State<'_, AppState>) -> Result<String, String> {
    println!("[DEV_TOOL] Attempting to get focused element info using NSWorkspace...");

    #[cfg(target_os = "macos")]
    let result = get_focused_element_ns_workspace(false, true);

    #[cfg(not(target_os = "macos"))]
    let result: Result<computer_use_ai_sdk::UIElement, AutomationError> = Err(AutomationError::UnsupportedPlatform);

    match result {
        Ok(element) => {
            println!("[DEV_TOOL] get_focused_element_info (NSWorkspace) succeeded.");
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

// New command for capturing element screenshot (macOS only for now)
#[cfg(target_os = "macos")]
#[tauri::command]
async fn capture_element_screenshot_command(
    _state: tauri::State<'_, AppState>,
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
             Ok(base64_string)
        },
        Err(e) => {
            match e {
                AutomationError::ZeroElementDimensions { role, label, x, y, width, height } => {
                    let user_friendly_err_msg = format!(
                        "Error: The focused element ('{}', Label: '{}') reported zero or negative dimensions ({}, {}, {}, {}) and could not be captured.",
                        role, label, x, y, width, height // Use label directly as it's a String
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

// Stub command for non-macos platforms
#[cfg(not(target_os = "macos"))]
#[tauri::command]
async fn capture_element_screenshot_command(
    _state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    Err("Element screenshot capture is only supported on macOS currently.".to_string())
}

// Updated command to handle user queries with agent loop and TTS
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

    let available_tools = desktop_arc.list_tools();

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
                println!("Error: {}", err_msg);
                desktop_arc.log("error", err_msg.clone());
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
            println!("Error: {}", err_msg);
            desktop_arc.log("error", err_msg.clone());
            return Err(err_msg);
        }

        // New Anthropic Response structure matching API
        #[derive(Deserialize, Debug)]
        struct AnthropicUsage {
            input_tokens: u32,
            output_tokens: u32,
        }
        #[derive(Deserialize, Debug)]
        struct AnthropicResponse { // Shadowing the previous struct is fine here
            content: Vec<AnthropicContentBlock>,
            stop_reason: String,
            usage: AnthropicUsage,
        }

        let anthropic_response: AnthropicResponse = match response.json().await {
            Ok(res) => res,
            Err(e) => {
                let err_msg = format!("Failed to parse Anthropic JSON response: {}", e);
                println!("Error: {}", err_msg);
                desktop_arc.log("error", err_msg.clone());
                return Err(err_msg);
            }
        };

        println!("Anthropic Raw Response: {:?}", anthropic_response);
        desktop_arc.log(
            "debug",
            format!("Anthropic Raw Response: {:?}", anthropic_response),
        );

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
                println!("Error: {}", err_msg);
                desktop_arc.log("error", err_msg.clone());
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
                        println!("Executing tool: {} with input: {:?}", name, input);
                        desktop_arc.log(
                            "info",
                            format!("Executing tool: {} with input: {:?}", name, input),
                        );

                        let tool_result = desktop_arc.call_tool(name, input.clone());

                        let (result_content_value, is_error) = match tool_result {
                            Ok(result_value) => {
                                desktop_arc.log(
                                    "info",
                                    format!("Tool '{}' success: {:?}", name, result_value),
                                );
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
                                                             desktop_arc.log("info", format!("Resizing screenshot from {}x{} to {}x{}", width, height, new_width, new_height));
                                                             img.resize_exact(new_width, new_height, image::imageops::FilterType::Lanczos3)
                                                        } else {
                                                             desktop_arc.log("info", format!("Screenshot dimensions {}x{} are within limits, not resizing.", width, height));
                                                             img
                                                        };

                                                        let mut png_bytes = Vec::new();
                                                        match resized_img.write_to(&mut Cursor::new(&mut png_bytes), ImageFormat::Png) {
                                                            Ok(_) => BASE64_STANDARD.encode(&png_bytes),
                                                            Err(e) => {
                                                                let err_msg = format!("Failed to encode resized image to PNG: {}", e);
                                                                desktop_arc.log("error", err_msg.clone());
                                                                base64_data.to_string()
                                                            }
                                                        }
                                                    }
                                                    Err(e) => {
                                                        let err_msg = format!("Failed to load image from screenshot bytes: {}", e);
                                                        desktop_arc.log("error", err_msg.clone());
                                                        base64_data.to_string()
                                                    }
                                                }
                                            }
                                            Err(e) => {
                                                let err_msg = format!("Failed to decode base64 screenshot data: {}", e);
                                                desktop_arc.log("error", err_msg.clone());
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
                                        desktop_arc.log("error", error_msg.clone());
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
                                desktop_arc.log("error", format!("Tool '{}' failed: {}", name, e));
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
                        println!("{}", warn_msg);
                        desktop_arc.log("warn", warn_msg);
                    }
                }
                _ => {
                    let warn_msg = format!("Warning: Unknown content block type: {}", block.type_);
                    println!("{}", warn_msg);
                    desktop_arc.log("warn", warn_msg);
                }
            }
        }

        if has_tool_calls {
            let tool_results_value = match serde_json::to_value(tool_results) {
                Ok(v) => v,
                Err(e) => {
                    let err_msg = format!("Failed to serialize tool results: {}", e);
                    println!("Error: {}", err_msg);
                    desktop_arc.log("error", err_msg.clone());
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
                println!(
                    "Agent loop finished. Stop reason: {}",
                    anthropic_response.stop_reason
                );
                desktop_arc.log(
                    "info",
                    format!(
                        "Agent loop finished. Stop reason: {}",
                        anthropic_response.stop_reason
                    ),
                );
                break;
            } else {
                let warn_msg = format!(
                    "Warning: Loop continued without tool calls but stop reason was: {}",
                    anthropic_response.stop_reason
                );
                println!("{}", warn_msg);
                desktop_arc.log("warn", warn_msg);
            }
        }

        if iteration == MAX_ITERATIONS - 1 {
            let warn_msg = "Warning: Max iterations reached without final answer.".to_string();
            println!("{}", warn_msg);
            desktop_arc.log("warn", warn_msg);
            final_response_text.push_str("\n[Agent reached maximum iterations]");
        }
    }

    let final_text = final_response_text.trim().to_string();
    desktop_arc.log("info", format!("Final agent text response: {}", final_text));

    let audio_result = tts::elevenlabs::invoke_elevenlabs_tts(final_text.clone(), state).await;

    let audio_base64 = match audio_result {
        Ok(base64) => {
            desktop_arc.log(
                "info",
                "TTS successful, including audio in response.".to_string(),
            );
            Some(base64)
        }
        Err(e) => {
            desktop_arc.log(
                "error",
                format!("TTS failed: {}. Returning response without audio.", e),
            );
            None
        }
    };

    Ok(SubmitQueryResult {
        text: final_text,
        audio_base64,
    })
}

// Command to get logs from the backend buffer
#[tauri::command]
async fn get_logs(state: tauri::State<'_, AppState>) -> Result<Vec<String>, String> {
    let logs = state.desktop.get_logs();
    let formatted_logs = logs
        .into_iter()
        .map(|log| format!("[{}] {}", log.level, log.message))
        .collect();
    Ok(formatted_logs)
}

// --- Dev Tool Commands ---

#[tauri::command]
async fn dev_click_focused_element(
    _state: tauri::State<'_, AppState>
) -> Result<String, String> {
    #[cfg(target_os = "macos")]
    {
        println!("[DEV_TOOL] Attempting to click focused element...");

        let element = match get_focused_element_ns_workspace(false, true) {
            Ok(el) => el,
            Err(e) => {
                let err_msg = format!("[DEV_TOOL] Failed to get focused element for click: {}", e);
                println!("{}", err_msg);
                return Err(err_msg);
            }
        };

        let attrs = element.attributes();
        println!("[DEV_TOOL] Clicking element: Role={}, Label={:?}, Desc={:?}",
                 attrs.role, attrs.label, attrs.description);

        match element.click() {
            Ok(_) => {
                let success_msg = "[DEV_TOOL] Click focused element succeeded.".to_string();
                println!("{}", success_msg);
                Ok(success_msg)
            }
            Err(e) => {
                let err_msg = format!("[DEV_TOOL] Failed to click focused element: {}", e);
                println!("{}", err_msg);
                Err(err_msg)
            }
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        Err("Click focused element is only supported on macOS.".to_string())
    }
}

#[tauri::command]
async fn dev_type_text(
    _state: tauri::State<'_, AppState>,
    text: String
) -> Result<String, String> {
    #[cfg(target_os = "macos")]
    {
        println!("[DEV_TOOL] Attempting to type text: '{}'", text);
        let err_msg = format!("[DEV_TOOL] Typing text ('{}') not implemented yet for macOS Desktop.", text);
        println!("{}", err_msg);
        Err(err_msg)
        // TODO: Implement actual typing logic, potentially using desktop.type_text(...)
        // match state.desktop.type_text(&text) {
        //     Ok(_) => {
        //         let success_msg = format!("[DEV_TOOL] Typed text '{}' successfully.", text);
        //         println!("{}", success_msg);
        //         Ok(success_msg)
        //     }
        //     Err(e) => {
        //         let err_msg = format!("[DEV_TOOL] Failed to type text '{}': {}", text, e);
        //         println!("{}", err_msg);
        //         Err(err_msg)
        //     }
        // }
        // TODO: Check if desktop.type_text exists and handles errors correctly.
        // let err_msg = format!("[DEV_TOOL] Typing text ('{}') not implemented yet for macOS Desktop.", text);
        // println!("{}", err_msg);
        // Err(err_msg)
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err("Type text is only supported on macOS currently.".to_string())
    }
}

#[tauri::command]
async fn dev_press_key(
    _state: tauri::State<'_, AppState>,
    key: String
) -> Result<String, String> {
    #[cfg(target_os = "macos")]
    {
        println!("[DEV_TOOL] Attempting to press key: '{}'", key);
        let err_msg = format!("[DEV_TOOL] Pressing key ('{}') not implemented yet for macOS Desktop.", key);
        println!("{}", err_msg);
        Err(err_msg)
        // TODO: Implement actual key press logic, potentially using desktop.press_key(...)
        // match state.desktop.press_key(&key) {
        //      Ok(_) => {
        //         let success_msg = format!("[DEV_TOOL] Pressed key '{}' successfully.", key);
        //         println!("{}", success_msg);
        //         Ok(success_msg)
        //     }
        //     Err(e) => {
        //         let err_msg = format!("[DEV_TOOL] Failed to press key '{}': {}", key, e);
        //         println!("{}", err_msg);
        //         Err(err_msg)
        //     }
        // }
        // TODO: Check if desktop.press_key exists and handles errors correctly.
        // let err_msg = format!("[DEV_TOOL] Pressing key ('{}') not implemented yet for macOS Desktop.", key);
        // println!("{}", err_msg);
        // Err(err_msg)
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err("Press key is only supported on macOS currently.".to_string())
    }
}

#[tauri::command]
async fn dev_open_application(state: tauri::State<'_, AppState>, app_name: String) -> Result<String, String> {
    println!("[DEV_TOOL] Attempting to open application: '{}'", app_name);
    match state.desktop.open_application(&app_name) {
         Ok(_) => {
            let success_msg = format!("[DEV_TOOL] Opened application '{}' successfully.", app_name);
            println!("{}", success_msg);
            Ok(success_msg)
        }
        Err(e) => {
            let err_msg = format!("[DEV_TOOL] Failed to open application '{}': {}", app_name, e);
            println!("{}", err_msg);
            Err(err_msg)
        }
    }
}

#[tauri::command]
async fn dev_open_url(state: tauri::State<'_, AppState>, url: String) -> Result<String, String> {
    println!("[DEV_TOOL] Attempting to open URL: '{}'", url);
    if !url.starts_with("http://") && !url.starts_with("https://") {
        let err_msg = "[DEV_TOOL] Invalid URL format. Must start with http:// or https://".to_string();
        println!("{}", err_msg);
        return Err(err_msg);
    }

    match state.desktop.open_url(&url, None) {
         Ok(_) => {
            let success_msg = format!("[DEV_TOOL] Opened URL '{}' successfully.", url);
            println!("{}", success_msg);
            Ok(success_msg)
        }
        Err(e) => {
            let err_msg = format!("[DEV_TOOL] Failed to open URL '{}': {}", url, e);
            println!("{}", err_msg);
            Err(err_msg)
        }
    }
}

#[tauri::command]
async fn dev_scroll_window(
    _state: tauri::State<'_, AppState>,
    direction: String,
    _amount_str: Option<String>
) -> Result<String, String> {
    #[cfg(target_os = "macos")]
    {
        println!("[DEV_TOOL] Attempting to scroll window: '{}'", direction);
        let err_msg = format!("[DEV_TOOL] Scrolling window ('{}') not implemented yet for macOS Desktop.", direction);
        println!("{}", err_msg);
        Err(err_msg)
        // TODO: Implement actual scroll logic, potentially using desktop.scroll(...)
        // TODO: Parse amount_str if needed by the SDK's scroll method
        // match state.desktop.scroll_window(&direction, None) { // Assuming scroll_window takes direction and optional amount
        //      Ok(_) => {
        //         let success_msg = format!("[DEV_TOOL] Scrolled window '{}' successfully.", direction);
        //         println!("{}", success_msg);
        //         Ok(success_msg)
        //     }
        //     Err(e) => {
        //         let err_msg = format!("[DEV_TOOL] Failed to scroll window '{}': {}", direction, e);
        //         println!("{}", err_msg);
        //         Err(err_msg)
        //     }
        // }
        // TODO: Check if desktop.scroll_window exists, handles amount, and manages errors.
        // let err_msg = format!("[DEV_TOOL] Scrolling window ('{}') not implemented yet for macOS Desktop.", direction);
        // println!("{}", err_msg);
        // Err(err_msg)
    }

    #[cfg(not(target_os = "macos"))]
    {
        Err("Scroll window is only supported on macOS currently.".to_string())
    }
}

// --- End Dev Tool Commands ---

// Helper function to run the focused element test
#[cfg(target_os = "macos")]
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

// New helper function for NSWorkspace method
#[cfg(target_os = "macos")]
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


#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt::init();
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

    tauri::Builder::default()
        .manage(AppState { desktop: desktop_arc })
        .setup(|app| {
            let app_handle = app.handle().clone();

            // --- System Tray Setup ---
            let quit_item = MenuItemBuilder::with_id("quit", "Quit DotDot").build(&app_handle)?;
            let toggle_item = MenuItemBuilder::with_id("toggle_panel", "Show Panel").build(&app_handle)?;
            let separator = PredefinedMenuItem::separator(&app_handle)?;
            let tray_menu = MenuBuilder::new(&app_handle)
                .items(&[&toggle_item, &separator, &quit_item])
                .build()?;

            let _tray_icon = TrayIconBuilder::new()
                .menu(&tray_menu)
                .tooltip("DotDot AI Agent")
                .on_tray_icon_event(move |tray, event| {
                    let app = tray.app_handle();
                    let menu_handle = app.menu().unwrap();
                    match event {
                        TrayIconEvent::Click { button, button_state, .. } => {
                            if button == MouseButton::Left && button_state == MouseButtonState::Up {
                                 if let Some(window) = app.get_webview_window("main") {
                                    let is_visible = window.is_visible().unwrap_or(false);
                                    if let Some(MenuItemKind::MenuItem(item)) = menu_handle.get("toggle_panel") {
                                        if is_visible {
                                            window.hide().unwrap(); item.set_text("Show Panel").unwrap();
                                        } else {
                                            window.show().unwrap(); window.set_focus().unwrap(); item.set_text("Hide Panel").unwrap();
                                        }
                                    }
                                } else { println!("Error: Main window not found for click toggle."); }
                             }
                        }
                        _ => { /*println!("Unhandled TrayIconEvent: {:?}", event);*/ }
                    }
                 })
                .on_menu_event(move |app, event| {
                     match event.id.0.as_str() {
                         "quit" => { app.exit(0); }
                         "toggle_panel" => {
                             if let Some(window) = app.get_webview_window("main") {
                                let is_visible = window.is_visible().unwrap_or(false);
                                if let Some(MenuItemKind::MenuItem(item)) = app.menu().unwrap().get(&event.id) {
                                    if is_visible {
                                        window.hide().unwrap(); item.set_text("Show Panel").unwrap();
                                    } else {
                                        window.show().unwrap(); window.set_focus().unwrap(); item.set_text("Hide Panel").unwrap();
                                    }
                                }
                             } else { println!("Error: Main window not found for menu toggle."); }
                         }
                         _ => {}
                     }
                })
                .build(&app_handle)?;
            // --- End System Tray Setup ---

            // --- Window Event Handling ---
            let main_window = app.get_webview_window("main")
               .ok_or_else(|| "Fatal: Main window not found during setup".to_string())?;

            let window_event_handle = app.handle().clone();
            main_window.on_window_event(move |event| {
                match event {
                    WindowEvent::CloseRequested { api, .. } => {
                        api.prevent_close();
                        let window = window_event_handle.get_webview_window("main").unwrap();
                        window.hide().unwrap();
                        if let Some(MenuItemKind::MenuItem(item)) = window_event_handle.menu().unwrap().get("toggle_panel") {
                            item.set_text("Show Panel").unwrap();
                        }
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
        })
        .plugin(tauri_plugin_opener::init())
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
            dev_scroll_window
        ])
        .build(tauri::generate_context!())
        .expect("Error building Tauri application")
        .run(|_app_handle, event| match event {
            tauri::RunEvent::ExitRequested { api, .. } => {
                 // Keep app running on manual exit request (e.g., Cmd+Q)
                 // The tray icon's Quit item handles graceful exit via app.exit(0)
                 println!("Exit requested via UI/shortcut, preventing default exit.");
                 api.prevent_exit();
            }
            _ => {}
        });
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
