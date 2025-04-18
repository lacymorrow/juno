use crate::state::AppState;
use crate::tools::{list_tools, handle_tool_call}; // Import from tools module
use crate::tts;
// use computer_use_ai_sdk::{Desktop}; // Remove unused
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use image::{GenericImageView, ImageFormat};
use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use std::io::Cursor;
use tracing::{debug, error, info, warn};
use tauri::State;
use futures::future; // Add futures import

// --- Anthropic API Structs ---

#[derive(Serialize, Clone)]
pub(crate) struct AnthropicMessage {
    pub(crate) role: String,
    pub(crate) content: Value,
}

#[derive(Deserialize, Debug, Clone, Serialize)]
pub(crate) struct AnthropicContentBlock {
    #[serde(rename = "type")]
    pub(crate) type_: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) input: Option<Value>,
}

#[derive(Serialize)]
pub(crate) struct ToolResultBlock {
    #[serde(rename = "type")]
    pub(crate) type_: String, // Always "tool_result"
    pub(crate) tool_use_id: String,
    pub(crate) content: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) is_error: Option<bool>,
}

#[derive(Serialize)]
pub(crate) struct SubmitQueryResult {
    pub(crate) text: String,
    pub(crate) audio_base64: Option<String>,
}


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
    tools: Vec<computer_use_ai_sdk::ToolDefinition>, // Use full path
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    thinking: Option<AnthropicThinkingBudget>,
}

#[derive(Deserialize, Debug)]
struct AnthropicUsage {
    #[allow(dead_code)] // Allow dead code for potentially unused fields
    input_tokens: u32,
    #[allow(dead_code)] // Allow dead code for potentially unused fields
    output_tokens: u32,
}

#[derive(Deserialize, Debug)]
struct AnthropicResponse {
    content: Vec<AnthropicContentBlock>,
    stop_reason: String,
    #[allow(dead_code)] // Allow dead code for potentially unused fields
    usage: AnthropicUsage,
}

// --- Submit Query Function ---

#[tauri::command]
pub async fn submit_query(
    query: String,
    state: State<'_, AppState>,
    app_handle: tauri::AppHandle, // Pass AppHandle
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

        let max_output_tokens = 1024;
        let thinking_budget = 4000;
        let total_max_tokens = max_output_tokens + thinking_budget;

        let request_payload = AnthropicRequest {
            model: "claude-3-5-sonnet-20240620", // Use Claude 3.5 Sonnet
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

        // --- Tool Call Handling ---
        let app_handle_clone = app_handle.clone();

        let futures = anthropic_response.content.iter().filter_map(|block| {
            if block.type_ == "tool_use" {
                if let (Some(id), Some(name), Some(input)) = (&block.id, &block.name, &block.input) {
                    has_tool_calls = true;
                    info!("Preparing tool execution: {} with input: {:?}", name, input);
                    let id_clone = id.clone();
                    let name_clone = name.clone();
                    let input_clone = input.clone();
                    let desktop_arc_clone = desktop_arc.clone();
                    let app_handle_clone_inner = app_handle_clone.clone();
                    let state_clone_for_async = state.clone();

                    Some(async move {
                        let tool_result_value = handle_tool_call(
                            &desktop_arc_clone,
                            &app_handle_clone_inner,
                            &name_clone,
                            &input_clone,
                            &state_clone_for_async,
                        ).await;

                        let is_error = tool_result_value.get("error").is_some() || tool_result_value.get("status").and_then(|s| s.as_str()) == Some("error");

                        // Handle screenshot resizing
                        let processed_content_value = if name_clone == "captureScreenshot" && !is_error {
                             if let Some(base64_data) = tool_result_value.get("screenshot_base64").or_else(|| tool_result_value.get("image_base64")).and_then(|v| v.as_str()) {
                                match BASE64_STANDARD.decode(base64_data) {
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
                                                    Ok(_) => {
                                                         let resized_base64_data = BASE64_STANDARD.encode(&png_bytes);
                                                         json!([{
                                                            "type": "image",
                                                            "source": {
                                                                "type": "base64",
                                                                "media_type": "image/png",
                                                                "data": resized_base64_data
                                                            }
                                                         }])
                                                    }
                                                    Err(e) => {
                                                        let err_msg = format!("Failed to encode resized image to PNG: {}", e);
                                                        error!("{}", err_msg);
                                                         json!([{"type": "text", "text": err_msg}]) // Return error text
                                                    }
                                                }
                                            }
                                            Err(e) => {
                                                let err_msg = format!("Failed to load image from screenshot bytes: {}", e);
                                                error!("{}", err_msg);
                                                 json!([{"type": "text", "text": err_msg}]) // Return error text
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        let err_msg = format!("Failed to decode base64 screenshot data: {}", e);
                                        error!("{}", err_msg);
                                         json!([{"type": "text", "text": err_msg}]) // Return error text
                                    }
                                }
                            } else {
                                let error_msg = format!("Tool '{}' succeeded but returned unexpected data: {:?}", name_clone, tool_result_value);
                                error!("{}", error_msg);
                                 json!([{"type": "text", "text": error_msg}]) // Return error text
                            }
                        } else if is_error {
                            // For errors, just wrap the error JSON in the text block structure
                             let error_str = serde_json::to_string(&tool_result_value).unwrap_or_else(|e| format!("{{\"error\": \"Failed to serialize error result: {}\"}}", e));
                             json!([{"type": "text", "text": error_str}])
                        } else {
                            // For non-screenshot success, wrap the result JSON in the text block structure
                            let result_str = serde_json::to_string(&tool_result_value).unwrap_or_else(|e| format!("Failed to serialize success result: {}", e));
                             json!([{"type": "text", "text": result_str}])
                        };

                        (id_clone, processed_content_value, is_error)
                    })
                } else {
                    warn!("Warning: Received incomplete tool_use block: {:?}", block);
                    None
                }
            } else {
                // Filter out non-tool_use blocks
                None
            }
        }).collect::<Vec<_>>();

        // Execute all tool calls concurrently
        let results = future::join_all(futures).await;

        // Process results and update tool_results
        for (id, content_value, is_error) in results {
            tool_results.push(ToolResultBlock {
                type_: "tool_result".to_string(),
                tool_use_id: id,
                content: content_value,
                is_error: Some(is_error),
            });
        }
        // --- End Tool Call Handling ---

        // Add text responses from the original response
        for block in &anthropic_response.content {
            if block.type_ == "text" {
                if let Some(text) = &block.text {
                    final_response_text.push_str(text);
                    final_response_text.push('\n');
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

    // Call TTS after the loop
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
