#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use computer_use_ai_sdk::{Desktop, ToolDefinition}; // Add ToolDefinition
use dotenvy::dotenv; // Added for .env loading
use reqwest::Client; // Add reqwest client
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::env;
use std::sync::Arc;

// Only include macos specific imports when targeting macos
#[cfg(target_os = "macos")]
use computer_use_ai_sdk::platforms::macos::utils as macos_utils;

// Declare the tts module
mod tts;

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
    content: String, // JSON string representation of the tool result
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
    println!("[DEV_TOOL] Attempting to call get_focused_element_info tool...");
    let result = state
        .desktop
        .call_tool("get_focused_element_info", Value::Null);

    match result {
        Ok(result_value) => {
            println!("[DEV_TOOL] get_focused_element_info tool succeeded.");
            // Serialize the resulting Value to JSON string
            serde_json::to_string(&result_value).map_err(|e| {
                let err_msg = format!("Failed to serialize element info result: {}", e);
                println!("[DEV_TOOL] Error: {}", err_msg);
                err_msg
            })
        }
        Err(e) => {
            let err_msg = format!("Failed to call get_focused_element_info tool: {}", e);
            println!("[DEV_TOOL] Error: {}", err_msg);
            Err(err_msg)
        }
    }
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
        }

        let request_payload = AnthropicRequest {
            model: "claude-3-haiku-20240307", // Or another suitable model
            max_tokens: 1024,
            messages: conversation_history.clone(), // Clone history for this request
            tools: available_tools.clone(),
            system: Some("You are an AI assistant that can use tools to interact with the user's computer desktop environment. Use the provided tools to fulfill the user's request. Respond with the final result or status.".to_string()),
        };

        // Send request to Anthropic
        let response = http_client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &api_key)
            .header("anthropic-version", "2023-06-01")
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

        // Add assistant's response (content blocks) to history
        let assistant_content_value = match serde_json::to_value(anthropic_response.content.clone())
        {
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

                        let (content_str, is_error) = match tool_result {
                            Ok(result_value) => {
                                let result_str = serde_json::to_string(&result_value)
                                    .unwrap_or_else(|e| format!("{{\"error\": \"Failed to serialize tool result: {}\"}}", e));
                                desktop_arc.log(
                                    "info",
                                    format!("Tool '{}' success: {}", name, result_str),
                                );
                                (result_str, false)
                            }
                            Err(e) => {
                                println!("Tool execution error: {}", e);
                                let error_str = serde_json::to_string(&serde_json::json!({
                                    "error": format!("Tool execution failed: {}", e)
                                }))
                                .unwrap_or_default();
                                desktop_arc.log("error", format!("Tool '{}' failed: {}", name, e));
                                (error_str, true)
                            }
                        };

                        tool_results.push(ToolResultBlock {
                            type_: "tool_result".to_string(),
                            tool_use_id: id.clone(),
                            content: content_str,
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize tracing subscriber
    // Default level is INFO, set RUST_LOG=debug or RUST_LOG=trace for more detailed logs
    tracing_subscriber::fmt::init();

    // Load environment variables from .env file
    dotenv().ok();

    // Initialize the Desktop automation engine
    // Use default settings for now (use_background_apps=false, activate_app=true)
    let desktop_instance =
        Desktop::new(false, true).expect("Failed to initialize Desktop Automation Engine");
    let desktop_arc = Arc::new(desktop_instance); // Wrap in Arc for shared ownership

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
            capture_screenshot_command,   // Add the new command here
            dev_get_focused_element_info  // Add the new command here
        ]) // Add the new command
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
