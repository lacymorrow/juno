#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use clap::Parser; // <-- Add clap parser
use computer_use_ai_sdk::{Desktop, ToolDefinition}; // Add ToolDefinition
use dotenvy::dotenv; // Added for .env loading
use reqwest::Client; // Add reqwest client
use serde::{Deserialize, Serialize};
use serde_json::{json, Value}; // Import the json! macro
use std::env;
use std::sync::Arc;
use image::{GenericImageView, ImageFormat}; // Added for image processing
use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _}; // Added for base64
use std::io::Cursor; // Added for in-memory image handling
use computer_use_ai_sdk::AutomationError; // <-- Import AutomationError

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
// Make AppState visible within the crate
pub(crate) struct AppState {
    desktop: Arc<Desktop>,
}

// --- Anthropic API Structures ---

#[derive(Serialize, Clone)] // Add Clone
struct AnthropicMessage {
    role: String,
    content: Value, // Can be string or array of content blocks
}

#[derive(Deserialize, Debug, Clone, Serialize)] // Add Serialize
struct AnthropicContentBlock {
    #[serde(rename = "type")]
    type_: String, // "text" or "tool_use"
    // For text blocks
    #[serde(skip_serializing_if = "Option::is_none")]
    text: Option<String>,
    // For tool_use blocks
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    input: Option<Value>,
}

#[derive(Deserialize, Debug)]
struct AnthropicResponse {
    completion: String,
    stop_reason: String,
    model: String,
    stop: Option<String>,
    log_id: String,
    exception: Option<String>,
    // Add other fields as needed
}

// Structure for tool results (sent back to Anthropic)
#[derive(Serialize)]
struct ToolResultBlock {
    #[serde(rename = "type")]
    type_: String, // Always "tool_result"
    tool_use_id: String,
    content: Value, // Can be string (old way, potentially phase out) or array of content blocks (text/image)
    #[serde(skip_serializing_if = "Option::is_none")]
    is_error: Option<bool>,
}

// Structure for the combined result of submit_query
#[derive(Serialize)] // Serialize to send back to frontend
struct SubmitQueryResult {
    text: String,
    audio_base64: Option<String>, // Option because TTS might fail
}

// --- End Anthropic API Structures ---

// --- REMOVED Replicate API Structures ---

// --- REMOVED ElevenLabs API Structures ---

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

// Command to capture a screenshot (macOS only for now)
#[cfg(target_os = "macos")]
#[tauri::command]
async fn capture_screenshot_command() -> Result<String, String> {
    // Call the utility function that already handles capture and encoding
    match macos_utils::capture_and_encode_screenshot() {
        Ok(base64_string) => Ok(base64_string),
        Err(e) => Err(format!("Failed to capture screenshot: {}", e)),
    }
}

// Stub command for non-macos platforms to prevent compile errors
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
                    // Use attributes().label instead of title()
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
    // For now, just confirm the state exists.
    // We could add more checks here later if needed.
    let _ = state.desktop; // Access it to ensure it's valid
    true // Assume connected if we reached here
}

// New command for developer tool: Get focused element info
#[tauri::command]
async fn dev_get_focused_element_info(state: tauri::State<'_, AppState>) -> Result<String, String> {
    println!("[DEV_TOOL] Attempting to get focused element info using NSWorkspace...");

    // Use the new function directly
    #[cfg(target_os = "macos")]
    let result = get_focused_element_ns_workspace(false, true); // Assuming default values

    // Stub for non-macOS
    #[cfg(not(target_os = "macos"))]
    let result: Result<computer_use_ai_sdk::UIElement, AutomationError> = Err(AutomationError::UnsupportedPlatform);

    match result {
        Ok(element) => {
            println!("[DEV_TOOL] get_focused_element_info (NSWorkspace) succeeded.");
            // Get attributes and serialize
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
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    println!("[DEV_TOOL] Capturing focused element screenshot using NSWorkspace method...");

    // 1. Get the focused element using the new function
    let focused_element = match get_focused_element_ns_workspace(false, true) { // Assuming defaults
        Ok(el) => el,
        Err(e) => {
            let err_msg = format!("Failed to get focused element (NSWorkspace): {}", e);
            println!("[DEV_TOOL] Error: {}", err_msg);
            return Err(err_msg);
        }
    };

    // 2. Downcast the UIElement trait object to the concrete MacOSUIElement type
    //    We need the concrete type to pass to the utility function.
    let macos_element = match focused_element.as_any().downcast_ref::<MacOSUIElement>() {
        Some(el) => el,
        None => {
            let err_msg = "Focused element is not a MacOSUIElement".to_string();
            println!("[DEV_TOOL] Error: {}", err_msg);
            return Err(err_msg);
        }
    };

    // 3. Call the utility function from macos_utils
    match macos_utils::capture_element_screenshot(macos_element) {
        Ok(base64_string) => {
             println!("[DEV_TOOL] Element screenshot captured successfully.");
             Ok(base64_string)
        },
        Err(e) => {
            // Match on the specific error variant
            match e {
                AutomationError::ZeroElementDimensions { role, label, x, y, width, height } => {
                    let user_friendly_err_msg = format!(
                        "Error: The focused element ('{}', Label: '{}') reported zero or negative dimensions ({}, {}, {}, {}) and could not be captured.",
                        role, label, x, y, width, height
                    );
                    println!("[DEV_TOOL] Error: {}", user_friendly_err_msg);
                    Err(user_friendly_err_msg)
                }
                _ => {
                    // Handle other errors normally
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

    // Add initial user query
    conversation_history.push(AnthropicMessage {
        role: "user".to_string(),
        content: Value::String(query.clone()),
    });

    // Get available tools
    let available_tools = desktop_arc.list_tools();

    for iteration in 0..MAX_ITERATIONS {
        println!("Agent Iteration: {}", iteration + 1);

        #[derive(Serialize)]
        struct AnthropicThinkingBudget {
            #[serde(rename = "type")]
            type_: String,
            budget_tokens: u32,
        }

        // Prepare request payload
        // Need to define AnthropicRequest struct here or import it if moved
        #[derive(Serialize)]
        struct AnthropicRequest<'a> {
            model: &'a str,
            max_tokens: u32,
            messages: Vec<AnthropicMessage>,
            tools: Vec<ToolDefinition>,
            #[serde(skip_serializing_if = "Option::is_none")]
            system: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none")] // Add this for thinking
            thinking: Option<AnthropicThinkingBudget>, // Add thinking parameter
        }

        let max_output_tokens = 1024; // Desired max output tokens for the final answer
        let thinking_budget = 4000; // Recommended thinking budget
        let total_max_tokens = max_output_tokens + thinking_budget; // Total max tokens including thinking

        let request_payload = AnthropicRequest {
            model: "claude-3-7-sonnet-20250219", // Use Claude 3.5 Sonnet
            max_tokens: total_max_tokens, // Set total max tokens
            messages: conversation_history.clone(), // Clone history for this request
            tools: available_tools.clone(),
            system: Some("You are an AI assistant that can use tools to interact with the user's computer desktop environment. Use the provided tools to fulfill the user's request. Respond with the final result or status.".to_string()),
            // thinking: Some(AnthropicThinkingBudget {
            //     type_: "enabled".to_string(),
            //     budget_tokens: thinking_budget,
            // }), // Enable thinking
            thinking: None, // Commented out: Some(AnthropicThinkingBudget { type_: "enabled".to_string(), budget_tokens: thinking_budget, }),

        };

        // Send request to Anthropic
        let response = http_client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &api_key)
            .header("anthropic-version", "2023-06-01") // Revert to standard API version
            .header("anthropic-beta", "computer-use-2025-01-24") // Beta flag for 3.7 Sonnet tools & thinking
            .header("content-type", "application/json")
            .json(&request_payload)
            .send()
            .await;

        let response = match response {
            Ok(res) => res,
            Err(e) => {
                let err_msg = format!("HTTP request to Anthropic failed: {}", e);
                println!("Error: {}", err_msg);
                // Log error using desktop instance
                desktop_arc.log("error", err_msg.clone());
                return Err(err_msg);
            }
        };

        // Check response status
        if !response.status().is_success() {
            let status = response.status();
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "Failed to read error body".to_string());
            let err_msg = format!("Anthropic API error: {} - {}", status, body);
            println!("Error: {}", err_msg);
            // Log error using desktop instance
            desktop_arc.log("error", err_msg.clone());
            return Err(err_msg);
        }

        // Parse the successful response
        // Need to redefine AnthropicResponse struct locally or import if moved
        #[derive(Deserialize, Debug)]
        struct AnthropicUsage {
            input_tokens: u32,
            output_tokens: u32,
        }
        #[derive(Deserialize, Debug)]
        struct AnthropicResponse {
            content: Vec<AnthropicContentBlock>,
            stop_reason: String, // e.g., "end_turn", "tool_use"
            usage: AnthropicUsage,
        }

        let anthropic_response: AnthropicResponse = match response.json().await {
            Ok(res) => res,
            Err(e) => {
                let err_msg = format!("Failed to parse Anthropic JSON response: {}", e);
                println!("Error: {}", err_msg);
                // Log error using desktop instance
                desktop_arc.log("error", err_msg.clone());
                return Err(err_msg);
            }
        };

        println!("Anthropic Raw Response: {:?}", anthropic_response);
        desktop_arc.log(
            "debug",
            format!("Anthropic Raw Response: {:?}", anthropic_response),
        ); // Log raw response

        // Filter out thinking blocks before adding to history
        let filtered_content: Vec<AnthropicContentBlock> = anthropic_response
            .content
            .clone()
            .into_iter()
            .filter(|block| block.type_ != "thinking")
            .collect();

        // Add assistant's response (filtered content blocks) to history
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

        let mut tool_results: Vec<ToolResultBlock> = Vec::new(); // Use ToolResultBlock defined earlier
        let mut has_tool_calls = false;

        // Process content blocks for text and tool calls
        for block in &anthropic_response.content {
            match block.type_.as_str() {
                "text" => {
                    if let Some(text) = &block.text {
                        final_response_text.push_str(text);
                        final_response_text.push('\n'); // Add newline for readability
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

                        // Call the tool on the Desktop instance
                        let tool_result = desktop_arc.call_tool(name, input.clone());

                        let (result_content_value, is_error) = match tool_result {
                            Ok(result_value) => {
                                desktop_arc.log(
                                    "info",
                                    format!("Tool '{}' success: {:?}", name, result_value),
                                );
                                // Special handling for screenshot results
                                if name == "captureScreenshot" {
                                    if let Some(base64_data) = result_value
                                        .get("screenshot_base64")
                                        .and_then(|v| v.as_str())
                                    {
                                        // Decode base64
                                        let resized_base64_data = match BASE64_STANDARD.decode(base64_data) {
                                            Ok(image_bytes) => {
                                                // Load image
                                                match image::load_from_memory(&image_bytes) {
                                                    Ok(img) => {
                                                        // Resize (e.g., max 1024px width/height, maintaining aspect ratio)
                                                        let (width, height) = img.dimensions();
                                                        let max_dim = 1024.0;
                                                        let scale = if width > height {
                                                            max_dim / width as f32
                                                        } else {
                                                            max_dim / height as f32
                                                        };

                                                        let new_width = (width as f32 * scale).round() as u32;
                                                        let new_height = (height as f32 * scale).round() as u32;

                                                        // Only resize if scale is less than 1 (i.e., image is larger than target)
                                                        let resized_img = if scale < 1.0 {
                                                             desktop_arc.log("info", format!("Resizing screenshot from {}x{} to {}x{}", width, height, new_width, new_height));
                                                             img.resize_exact(new_width, new_height, image::imageops::FilterType::Lanczos3)
                                                        } else {
                                                             desktop_arc.log("info", format!("Screenshot dimensions {}x{} are within limits, not resizing.", width, height));
                                                             img // Return original if already small enough
                                                        };

                                                        // Encode resized image back to PNG bytes
                                                        let mut png_bytes = Vec::new();
                                                        match resized_img.write_to(&mut Cursor::new(&mut png_bytes), ImageFormat::Png) {
                                                            Ok(_) => {
                                                                // Encode bytes back to base64
                                                                BASE64_STANDARD.encode(&png_bytes)
                                                            }
                                                            Err(e) => {
                                                                let err_msg = format!("Failed to encode resized image to PNG: {}", e);
                                                                desktop_arc.log("error", err_msg.clone());
                                                                // Fallback or handle error - maybe send original?
                                                                // For now, log error and send original base64
                                                                base64_data.to_string()
                                                            }
                                                        }
                                                    }
                                                    Err(e) => {
                                                        let err_msg = format!("Failed to load image from screenshot bytes: {}", e);
                                                        desktop_arc.log("error", err_msg.clone());
                                                        // Fallback or handle error
                                                        base64_data.to_string()
                                                    }
                                                }
                                            }
                                            Err(e) => {
                                                let err_msg = format!("Failed to decode base64 screenshot data: {}", e);
                                                desktop_arc.log("error", err_msg.clone());
                                                // Fallback or handle error
                                                base64_data.to_string()
                                            }
                                        };

                                        let image_block = json!({
                                            "type": "image",
                                            "source": {
                                                "type": "base64",
                                                "media_type": "image/png",
                                                "data": resized_base64_data // Use resized data
                                            }
                                        });
                                        // Per API spec, tool result content should be an ARRAY of blocks
                                        (Value::Array(vec![image_block]), false)
                                    } else {
                                        // Screenshot tool succeeded but didn't return expected data?
                                        let error_msg = format!("Tool '{}' succeeded but returned unexpected data: {:?}", name, result_value);
                                        desktop_arc.log("error", error_msg.clone());
                                        (json!([{ "type": "text", "text": error_msg }]), true)
                                        // Send error text back
                                    }
                                } else {
                                    // For other tools, wrap the result in a text block within an array
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
                                // Send error back as a text block in an array
                                (json!([{ "type": "text", "text": error_str }]), true)
                            }
                        };

                        tool_results.push(ToolResultBlock {
                            type_: "tool_result".to_string(),
                            tool_use_id: id.clone(),
                            content: result_content_value, // Use the Value directly
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

        // If tools were called, add results to history and continue loop
        if has_tool_calls {
            // Convert tool results into a Value (array of blocks)
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
                role: "user".to_string(), // Per Anthropic spec, tool results come from 'user' role
                content: tool_results_value,
            });
        } else {
            // No tool calls, check stop reason
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
                break; // Exit loop, we have the final response
            } else {
                let warn_msg = format!(
                    "Warning: Loop continued without tool calls but stop reason was: {}",
                    anthropic_response.stop_reason
                );
                println!("{}", warn_msg);
                desktop_arc.log("warn", warn_msg);
                // Potentially break here too, or handle specific stop reasons
            }
        }

        // Safety break if loop didn't finish naturally
        if iteration == MAX_ITERATIONS - 1 {
            let warn_msg = "Warning: Max iterations reached without final answer.".to_string();
            println!("{}", warn_msg);
            desktop_arc.log("warn", warn_msg);
            final_response_text.push_str("\n[Agent reached maximum iterations]");
        }
    }

    let final_text = final_response_text.trim().to_string();
    desktop_arc.log("info", format!("Final agent text response: {}", final_text));

    // Attempt ElevenLabs TTS
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
            None // Return None if TTS fails
        }
    };

    // Return the combined result
    Ok(SubmitQueryResult {
        text: final_text,
        audio_base64,
    })
}

// Command to get logs from the backend buffer
#[tauri::command]
async fn get_logs(state: tauri::State<'_, AppState>) -> Result<Vec<String>, String> {
    // Retrieve logs from the Desktop instance's log buffer
    let logs = state.desktop.get_logs();

    // Format logs as strings [level] message
    let formatted_logs = logs
        .into_iter()
        .map(|log| format!("[{}] {}", log.level, log.message))
        .collect();

    Ok(formatted_logs)
}

// --- REMOVED Replicate TTS Command Function ---

// --- REMOVED ElevenLabs TTS Command Function ---

// Helper function to run the focused element test
#[cfg(target_os = "macos")] // Keep macOS specific
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
#[cfg(target_os = "macos")] // Keep macOS specific
fn run_test_focused_element_ns() -> Result<(), String> {
    // We need to call the function directly as it's not part of the Desktop struct yet.
    // Import the function:
    use computer_use_ai_sdk::platforms::macos::element::get_focused_element_ns_workspace;

    println!("--- Running Test: Get Focused Element (NSWorkspace Method) ---");
    // Assuming default values for use_background_apps and activate_app
    // TODO: Potentially make these configurable via CLI flags as well?
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
    // Initialize tracing subscriber
    // Default level is INFO, set RUST_LOG=debug or RUST_LOG=trace for more detailed logs
    tracing_subscriber::fmt::init();

    // Load environment variables from .env file
    dotenv().ok();

    // Parse CLI arguments
    let cli = Cli::parse();

    // Initialize the Desktop automation engine
    let desktop_instance_result = Desktop::new(false, true); // Initialize here
    let desktop_instance = match desktop_instance_result {
        Ok(instance) => instance,
        Err(e) => {
            eprintln!("FATAL: Failed to initialize Desktop Automation Engine: {}", e);
            // Attempt to log if possible, though logging might not be fully set up
            // Consider using a simple logger here if tracing fails early
            tracing::error!("Failed to initialize Desktop Automation Engine: {}", e);
            std::process::exit(1); // Exit if core engine fails
        }
    };

    // Handle test flags first
    let mut ran_test = false;
    let mut test_result: Result<(), String> = Ok(());

    if cli.test_focused_element_ns { // Check the new flag
        #[cfg(target_os = "macos")]
        {
            // Call the new test function
            test_result = run_test_focused_element_ns();
            ran_test = true;
        }
        #[cfg(not(target_os = "macos"))]
        {
            eprintln!("Error: --test-focused-element-ns is only supported on macOS.");
            test_result = Err("Unsupported platform".to_string());
            ran_test = true;
        }
    }

    if cli.check_accessibility {
        #[cfg(target_os = "macos")]
        {
            test_result = run_check_accessibility();
            ran_test = true;
        }
        #[cfg(not(target_os = "macos"))]
        {
             eprintln!("Warning: --check-accessibility is a macOS-specific check.");
            // Allow check on other platforms, but it won't do anything
             println!("--- Running Test: Check Accessibility Permissions ---");
             println!("Check is only relevant on macOS. Skipping.");
             println!("--- Test Check Accessibility: Skipped ---");
             ran_test = true;
        }
    }

    // If a test was run, print the result and exit
    if ran_test {
        match test_result {
            Ok(_) => std::process::exit(0), // Exit with success code
            Err(_) => std::process::exit(1), // Exit with error code
        }
    }

    // If no test flags were provided, run the Tauri application
    println!("No test flags detected, launching Tauri application...");
    let desktop_arc = Arc::new(desktop_instance); // Wrap in Arc for Tauri state

    tauri::Builder::default()
        .manage(AppState {
            desktop: desktop_arc,
        }) // Add the Desktop instance to Tauri's managed state
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            greet,
            list_apps,
            check_server_status,
            submit_query,
            get_logs,
            // Use paths from the new module
            tts::replicate::invoke_replicate_tts,
            tts::elevenlabs::invoke_elevenlabs_tts,
            capture_screenshot_command,
            dev_get_focused_element_info,
            capture_element_screenshot_command
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

// Unit tests module
#[cfg(test)]
mod tests {
    use super::*; // Import items from the parent module (lib.rs)

    // Example test function (can be removed later)
    fn add(a: i32, b: i32) -> i32 {
        a + b
    }

    #[test]
    fn test_simple_addition() {
        assert_eq!(add(2, 2), 4, "Check basic addition");
    }

    #[test]
    fn test_focused_element_info_placeholder() {
        // This is a placeholder. Testing the actual focused element logic
        // typically requires an integration test or complex mocking of OS APIs.
        // For now, we just assert true to ensure the test runner picks it up.
        assert!(true, "Placeholder test for focused element concept");
        // In a real scenario, you might try to initialize the engine
        // and call the relevant function, asserting it doesn't panic,
        // but verifying the *correct* focused element is hard in isolation.
    }

    // Add more tests here...
}
