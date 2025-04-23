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
use tokio_stream::StreamExt; // Add StreamExt
use tracing::{debug, error, info, warn};
use tauri::State;
use futures::future; // Add futures import
use tauri::{Manager, Emitter}; // Import Manager and Emitter
use std::sync::Arc; // Import Arc

// --- Event Payloads for Frontend Communication ---

#[derive(Serialize, Clone)]
struct AssistantTextDeltaPayload {
    delta: String,
}

// Keep this for final response structure, ensure Clone is derived
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SubmitQueryResult {
    pub text: String,
    pub audio_base64: Option<String>,
}

// Define the payload structure for the final event
#[derive(Serialize, Clone)]
struct FinalAssistantResponsePayload {
    query: String, // Include original query for context
    response: SubmitQueryResult,
}

// --- Anthropic API Structs ---

#[derive(Debug, Serialize, Clone)]
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
    pub(crate) id: Option<String>, // Needed for tool_use
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) name: Option<String>, // Needed for tool_use
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) input: Option<Value>, // Needed for tool_use
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) index: Option<u32>, // Added for delta handling
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) delta: Option<Value>, // Added for delta handling
}

#[derive(Debug, Serialize)]
pub(crate) struct ToolResultBlock {
    #[serde(rename = "type")]
    pub(crate) type_: String, // Always "tool_result"
    pub(crate) tool_use_id: String,
    pub(crate) content: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) is_error: Option<bool>,
}

// Define the payload structure for the event - REMOVED, replaced by FinalAssistantResponsePayload
// #[derive(Serialize, Clone)]
// struct BackendResponsePayload {
//     query: String,
//     response: SubmitQueryResult,
// }

#[derive(Clone, Debug, Serialize)]
struct AnthropicThinkingBudget {
    #[serde(rename = "type")]
    type_: String,
    budget_tokens: u32,
}

#[derive(Debug, Serialize, Clone)]
pub(crate) struct AnthropicRequest<'a> {
    model: &'a str,
    max_tokens: u32,
    messages: Vec<AnthropicMessage>,
    tools: Vec<computer_use_ai_sdk::ToolDefinition>, // Use full path
    stream: bool, // Add stream parameter
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    thinking: Option<AnthropicThinkingBudget>,
}

#[derive(Deserialize, Debug, Clone)]
struct AnthropicUsage {
    #[allow(dead_code)] // Allow dead code for potentially unused fields
    input_tokens: u32,
    #[allow(dead_code)] // Allow dead code for potentially unused fields
    output_tokens: u32,
}

// --- SSE Event Structs ---

#[derive(Deserialize, Debug, Clone)]
struct MessageStartEventData {
    message: MessageStartMessage,
}

#[derive(Deserialize, Debug, Clone)]
struct MessageStartMessage {
    id: String,
    #[serde(rename = "type")]
    type_: String,
    role: String,
    // content: Vec<Value>, // Initially empty
    model: String,
    stop_reason: Option<String>,
    stop_sequence: Option<String>,
    usage: AnthropicUsage,
}

#[derive(Deserialize, Debug, Clone)]
struct ContentBlockStartEventData {
    index: u32,
    content_block: AnthropicContentBlock,
}

#[derive(Deserialize, Debug, Clone)]
struct ContentBlockDeltaEventData {
    index: u32,
    delta: Value, // Can be {"type": "text_delta", "text": "..."} or {"type": "input_json_delta", "partial_json": "..."}
}

#[derive(Deserialize, Debug, Clone)]
struct ContentBlockStopEventData {
    index: u32,
}

#[derive(Deserialize, Debug, Clone)]
struct MessageDeltaEventData {
    delta: MessageDelta,
    usage: MessageDeltaUsage,
}

#[derive(Deserialize, Debug, Clone)]
struct MessageDelta {
    stop_reason: Option<String>,
    stop_sequence: Option<String>,
}

#[derive(Deserialize, Debug, Clone)]
struct MessageDeltaUsage {
    output_tokens: u32,
}

#[derive(Deserialize, Debug, Clone)]
struct MessageStopEventData {
    #[serde(rename = "type")]
    _type: String, // "message_stop"
    #[serde(rename = "amazon-bedrock-invocationMetrics")]
    _invocation_metrics: Option<Value>, // Ignore bedrock metrics for now
}

// --- Helper Function for SSE Parsing ---

fn parse_sse_event(event_str: &str) -> Option<(String, String)> {
    let mut event_type = None;
    let mut data = None;
    for line in event_str.lines() {
        if line.starts_with("event:") {
            event_type = Some(line["event:".len()..].trim().to_string());
        } else if line.starts_with("data:") {
            data = Some(line["data:".len()..].trim().to_string());
        }
    }
    match (event_type, data) {
        (Some(et), Some(d)) => Some((et, d)),
        _ => None,
    }
}

// --- Submit Query Function (Streaming Version) ---

#[tauri::command]
pub async fn submit_query(
    query: String,
    state: State<'_, AppState>,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    info!("Received query for streaming: {}", query);

    let api_key = std::env::var("ANTHROPIC_API_KEY")
        .map_err(|_| "ANTHROPIC_API_KEY not configured.".to_string())?;

    let desktop_arc = state.desktop.clone();
    let http_client = Arc::new(Client::new()); // Use Arc for client
    let mut conversation_history: Vec<AnthropicMessage> = Vec::new();
    let mut final_response_text_aggregator = String::new(); // Aggregate text across iterations if needed
    const MAX_ITERATIONS: u32 = 10;

    let window = app_handle.get_window("main").ok_or_else(|| "Main window not found".to_string())?;

    // Initial user message
    conversation_history.push(AnthropicMessage {
        role: "user".to_string(),
        content: Value::String(query.clone()),
    });

    let available_tools = list_tools(&desktop_arc);

    for iteration in 0..MAX_ITERATIONS {
        info!("Agent Iteration: {}", iteration + 1);

        // Check for max iterations (safety break) before making the request
        if iteration == MAX_ITERATIONS {
            let warn_msg = "Warning: Max iterations reached.";
            warn!("{}", warn_msg);
            final_response_text_aggregator.push_str("
[Agent reached maximum iterations]");
            break; // Exit loop if max iterations hit
        }

        // --- Accumulators for the current streaming response ---
        let mut current_iteration_text = String::new();
        let mut current_tool_calls: Vec<AnthropicContentBlock> = Vec::new(); // Store complete tool_use blocks
        let mut current_stop_reason: Option<String> = None;
        // let mut current_usage: Option<AnthropicUsage> = None; // Track usage if needed

        let max_output_tokens = 4096; // Increased max tokens for potentially longer streams/tool use
        // Thinking budget is not used with streaming
        // let thinking_budget = 4000;
        // let total_max_tokens = max_output_tokens + thinking_budget;

        let request_payload = AnthropicRequest {
            model: "claude-3-5-sonnet-20240620",
            max_tokens: max_output_tokens, // Only max_tokens needed for stream
            messages: conversation_history.clone(),
            tools: available_tools.clone(),
            stream: true, // Enable streaming
            system: Some("You are an AI assistant that can use tools to interact with the user's computer desktop environment. Use the provided tools to fulfill the user's request. Respond with the final result or status. Prefer using tools over asking the user for information if a tool can obtain it.".to_string()),
            thinking: None, // Thinking budget not applicable for streaming
        };

        let request = http_client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &api_key)
            .header("anthropic-version", "2023-06-01")
            .header("anthropic-beta", "tools-2024-05-16") // Use the May 16 tools beta or newer if available
            .header("content-type", "application/json")
            .json(&request_payload);

        debug!("Sending Anthropic request (Iteration {}): {:?}", iteration + 1, request_payload);

        let response = match request.send().await {
            Ok(res) => res,
            Err(e) => {
                let err_msg = format!("HTTP request to Anthropic failed: {}", e);
                error!("Error: {}", err_msg);
                // Emit final error response
                let result = SubmitQueryResult { text: err_msg, audio_base64: None };
                let payload = FinalAssistantResponsePayload { query: query.clone(), response: result };
                window.emit("final-assistant-response", payload)
                    .map_err(|e| format!("Failed to emit final error event: {}", e))?;
                return Ok(()); // Command technically succeeded in emitting the error state
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
            // Emit final error response
            let result = SubmitQueryResult { text: err_msg, audio_base64: None };
            let payload = FinalAssistantResponsePayload { query: query.clone(), response: result };
            window.emit("final-assistant-response", payload)
                .map_err(|e| format!("Failed to emit final error event: {}", e))?;
            return Ok(());
        }

        // --- Process SSE Stream ---
        let mut stream = response.bytes_stream();
        let mut assistant_response_accumulator: Vec<AnthropicContentBlock> = Vec::new(); // Accumulate blocks for history

        while let Some(chunk_result) = stream.next().await {
            match chunk_result {
                Ok(chunk) => {
                    let chunk_str = match std::str::from_utf8(&chunk) {
                        Ok(s) => s,
                        Err(e) => {
                            warn!("Failed to decode chunk as UTF-8: {}", e);
                            continue;
                        }
                    };

                    // SSE format can contain multiple events separated by double newlines
                    for event_str in chunk_str.split("

") {
                         if event_str.trim().is_empty() {
                             continue;
                         }
                        if let Some((event_type, data)) = parse_sse_event(event_str) {
                            debug!("Received SSE Event: type={}, data={}", event_type, data);
                            match event_type.as_str() {
                                "message_start" => {
                                    // Optional: Handle message start if needed, e.g., store message ID
                                     match serde_json::from_str::<MessageStartEventData>(&data) {
                                         Ok(event_data) => {
                                             debug!("Message Start: {:?}", event_data.message);
                                             // current_usage = Some(event_data.message.usage); // Store initial usage
                                         }
                                         Err(e) => warn!("Failed to parse message_start data: {}", e),
                                     }
                                }
                                "content_block_start" => {
                                     match serde_json::from_str::<ContentBlockStartEventData>(&data) {
                                         Ok(event_data) => {
                                             debug!("Content Block Start (Index {}): {:?}", event_data.index, event_data.content_block);
                                             // Ensure accumulator has space
                                             if assistant_response_accumulator.len() <= event_data.index as usize {
                                                assistant_response_accumulator.resize(event_data.index as usize + 1, AnthropicContentBlock { type_: "".to_string(), text: None, id: None, name: None, input: None, index: None, delta: None });
                                             }
                                             assistant_response_accumulator[event_data.index as usize] = event_data.content_block;
                                         }
                                         Err(e) => warn!("Failed to parse content_block_start data: {}", e),
                                     }
                                }
                                "content_block_delta" => {
                                     match serde_json::from_str::<ContentBlockDeltaEventData>(&data) {
                                         Ok(event_data) => {
                                             debug!("Content Block Delta (Index {}): {:?}", event_data.index, event_data.delta);
                                             if assistant_response_accumulator.len() > event_data.index as usize {
                                                 let block = &mut assistant_response_accumulator[event_data.index as usize];
                                                 // Handle text delta
                                                 if let Some(text_delta) = event_data.delta.get("text").and_then(|t| t.as_str()) {
                                                      if block.type_ == "text" {
                                                            let current_text = block.text.get_or_insert_with(String::new);
                                                            current_text.push_str(text_delta);
                                                            current_iteration_text.push_str(text_delta); // Also update iteration aggregate

                                                            // --- Emit text delta to frontend ---
                                                            let delta_payload = AssistantTextDeltaPayload { delta: text_delta.to_string() };
                                                            if let Err(e) = window.emit("assistant-text-delta", delta_payload) {
                                                                error!("Failed to emit text delta event: {}", e);
                                                            }
                                                            // --- ---
                                                      }
                                                 }
                                                 // Handle tool input delta (less common, might be JSON parts)
                                                 else if block.type_ == "tool_use" {
                                                    if let Some(partial_json) = event_data.delta.get("partial_json").and_then(|pj| pj.as_str()) {
                                                        // Accumulate partial JSON for tool input if necessary
                                                        // For simplicity now, we assume input arrives mostly complete in content_block_start
                                                        // or rely on Anthropic assembling it before stop.
                                                        debug!("Tool input delta received (partial_json): {}", partial_json);
                                                        // A more robust implementation might try to parse/merge partial JSON here.
                                                    }
                                                 }
                                             } else {
                                                 warn!("Received delta for unknown block index: {}", event_data.index);
                                             }
                                         }
                                         Err(e) => warn!("Failed to parse content_block_delta data: {}", e),
                                     }
                                }
                                "content_block_stop" => {
                                     match serde_json::from_str::<ContentBlockStopEventData>(&data) {
                                         Ok(event_data) => {
                                             debug!("Content Block Stop (Index {})", event_data.index);
                                             if assistant_response_accumulator.len() > event_data.index as usize {
                                                let block = &assistant_response_accumulator[event_data.index as usize];
                                                // If it's a completed tool_use block, add it to our list for execution
                                                if block.type_ == "tool_use" && block.id.is_some() && block.name.is_some() && block.input.is_some() {
                                                    info!("Accumulated complete tool call: {}", block.name.as_deref().unwrap_or("unknown"));
                                                    current_tool_calls.push(block.clone());
                                                }
                                             }
                                         }
                                         Err(e) => warn!("Failed to parse content_block_stop data: {}", e),
                                     }
                                }
                                "message_delta" => {
                                    match serde_json::from_str::<MessageDeltaEventData>(&data) {
                                        Ok(event_data) => {
                                            debug!("Message Delta: {:?}", event_data.delta);
                                            if event_data.delta.stop_reason.is_some() {
                                                current_stop_reason = event_data.delta.stop_reason;
                                            }
                                            // Update usage if needed: current_usage.output_tokens += event_data.usage.output_tokens;
                                        }
                                        Err(e) => warn!("Failed to parse message_delta data: {}", e),
                                    }
                                }
                                "message_stop" => {
                                    info!("Message Stop event received.");
                                    // The stream officially ends here for this request.
                                    // current_stop_reason should have been set by a preceding message_delta.
                                    break; // Exit the inner chunk processing loop
                                }
                                "ping" => {
                                    // Ignore ping events
                                }
                                _ => {
                                    warn!("Received unknown SSE event type: {}", event_type);
                                }
                            }
                        } else {
                            warn!("Failed to parse SSE event line(s): '{}'", event_str);
                        }
                    }
                }
                Err(e) => {
                    error!("Error receiving stream chunk: {}", e);
                    // Attempt to emit an error, but the connection might be broken
                     let result = SubmitQueryResult { text: format!("Stream error: {}", e), audio_base64: None };
                     let payload = FinalAssistantResponsePayload { query: query.clone(), response: result };
                     let _ = window.emit("final-assistant-response", payload); // Ignore error if window is gone
                    return Ok(()); // Exit function on stream error
                }
            }
        } // End of stream processing loop

        // --- Post-Stream Processing for the Iteration ---

        // Add the accumulated assistant message to history (regardless of tool use)
        // Filter out thinking blocks just in case, although streaming shouldn't include them.
        let filtered_assistant_content: Vec<AnthropicContentBlock> = assistant_response_accumulator
            .into_iter()
            .filter(|block| block.type_ != "thinking" && !block.type_.is_empty()) // Ensure type is not empty from resize placeholder
            .collect();

        if !filtered_assistant_content.is_empty() {
             let assistant_content_value = match serde_json::to_value(filtered_assistant_content) {
                 Ok(v) => v,
                 Err(e) => {
                     error!("Failed to serialize accumulated assistant content: {}", e);
                     // Create a minimal error representation for history
                     json!([{"type": "text", "text": format!("Error: Failed to serialize response: {}", e)}])
                 }
             };
             conversation_history.push(AnthropicMessage {
                role: "assistant".to_string(),
                content: assistant_content_value,
             });
        } else if !current_iteration_text.is_empty() {
            // Handle cases where only text deltas were received without full block structure (less common)
             conversation_history.push(AnthropicMessage {
                role: "assistant".to_string(),
                content: json!([{"type": "text", "text": current_iteration_text }]),
             });
        }

        // --- Handle Tool Calls accumulated in this iteration ---
        if !current_tool_calls.is_empty() {
            info!("Executing {} tool calls from iteration {}", current_tool_calls.len(), iteration + 1);
            let mut tool_results_for_next_request: Vec<ToolResultBlock> = Vec::new();

            let tool_execution_futures = current_tool_calls.iter().map(|block| {
                let id_clone = block.id.clone().unwrap_or_default(); // Should always exist here
                let name_clone = block.name.clone().unwrap_or_default();
                let input_clone = block.input.clone().unwrap_or_default();
                let desktop_arc_clone = desktop_arc.clone();
                let app_handle_clone = app_handle.clone();
                let state_clone_for_async = state.clone(); // Clone state for async block

                async move {
                    info!("Executing tool: {} with ID: {}", name_clone, id_clone);
                    let tool_result_value = handle_tool_call(
                        &desktop_arc_clone,
                        &app_handle_clone,
                        &name_clone,
                        &input_clone,
                        &state_clone_for_async, // Pass cloned state
                    ).await;

                    let is_error = tool_result_value.get("error").is_some() || tool_result_value.get("status").and_then(|s| s.as_str()) == Some("error");
                    info!("Tool {} (ID: {}) result (is_error: {}): {:?}", name_clone, id_clone, is_error, tool_result_value);

                    // Handle screenshot resizing (Copied from non-streaming version, might need Arc for client)
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
                                                let scale = scale.min(1.0); // Assign result back to scale

                                                let new_width = (width as f32 * scale).round() as u32;
                                                let new_height = (height as f32 * scale).round() as u32;

                                                let resized_img = if scale < 1.0 {
                                                     img.resize_exact(new_width, new_height, image::imageops::FilterType::Lanczos3)
                                                } else {
                                                     img // No resize needed if scale >= 1.0
                                                };

                                                let mut png_bytes = Vec::new();
                                                match resized_img.write_to(&mut Cursor::new(&mut png_bytes), ImageFormat::Png) {
                                                    Ok(_) => {
                                                         let resized_base64_data = BASE64_STANDARD.encode(&png_bytes);
                                                         // Format for Anthropic API tool result content
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
                                 let error_msg = format!("Tool '{}' succeeded but returned unexpected data for screenshot processing: {:?}", name_clone, tool_result_value);
                                 error!("{}", error_msg);
                                  json!([{"type": "text", "text": error_msg}]) // Return error text
                            }
                        } else if is_error {
                            // For errors, wrap the error JSON in the text block structure
                             let error_str = serde_json::to_string(&tool_result_value).unwrap_or_else(|e| format!("{{\"error\": \"Failed to serialize error result: {}\"}}", e));
                             json!([{"type": "text", "text": error_str}])
                        } else {
                            // For non-screenshot success, wrap the result JSON in the text block structure
                            // Check if the result is already in the desired [{"type": "text", "text": "..."}] format
                            if let Some(arr) = tool_result_value.as_array() {
                                if arr.len() == 1 && arr[0].get("type").and_then(|t| t.as_str()) == Some("text") && arr[0].get("text").is_some() {
                                    tool_result_value // Already formatted correctly
                                } else {
                                    // Wrap other JSON results
                                    let result_str = serde_json::to_string(&tool_result_value).unwrap_or_else(|e| format!("Failed to serialize success result: {}", e));
                                    json!([{"type": "text", "text": result_str}])
                                }
                            } else {
                                // Wrap non-array JSON results
                                let result_str = serde_json::to_string(&tool_result_value).unwrap_or_else(|e| format!("Failed to serialize success result: {}", e));
                                json!([{"type": "text", "text": result_str}])
                            }
                        };

                    // Return tuple for aggregation
                    (id_clone, processed_content_value, is_error)
                }
            }).collect::<Vec<_>>();

            // Execute all tool calls concurrently
            let results = future::join_all(tool_execution_futures).await;

            // Process results and prepare for the next request
            for (id, content_value, is_error) in results {
                tool_results_for_next_request.push(ToolResultBlock {
                    type_: "tool_result".to_string(),
                    tool_use_id: id,
                    content: content_value, // Use the processed content
                    is_error: Some(is_error),
                });
            }

            // Add the tool results message to history for the *next* iteration's request
            let tool_results_value = match serde_json::to_value(tool_results_for_next_request) {
                 Ok(v) => v,
                 Err(e) => {
                    error!("Failed to serialize tool results block: {}", e);
                    // Add an error message to history instead
                     json!([{ "type": "text", "text": format!("Error serializing tool results: {}", e)}])
                 }
            };

            conversation_history.push(AnthropicMessage {
                role: "user".to_string(), // Role is 'user' for tool results message
                content: tool_results_value,
            });

            // Continue the loop to send results back to the model
             info!("Continuing loop to send tool results back to Anthropic.");
             continue;

        } else {
            // --- No Tool Calls in this iteration ---
            // Aggregate the text from this iteration
            final_response_text_aggregator.push_str(&current_iteration_text); // Append text from this iteration

            // Check stop reason
            match current_stop_reason.as_deref() {
                 Some("end_turn") | Some("stop_sequence") => {
                     info!("Stop reason '{}' received and no tools called. Finishing.", current_stop_reason.as_deref().unwrap_or("N/A"));
                     break; // Exit the main iteration loop
                 }
                 Some("tool_use") => {
                     // This case should technically be handled by the `if !current_tool_calls.is_empty()` block above.
                     // If we reach here, it means the model *intended* to use a tool but didn't output a valid block.
                     warn!("Stop reason 'tool_use' but no valid tool calls were accumulated. Finishing.");
                      final_response_text_aggregator.push_str("
[Agent intended to use a tool but failed to provide details]");
                     break;
                 }
                 Some(reason) => {
                     warn!("Unhandled stop reason '{}' without tool calls. Finishing.", reason);
                     break;
                 }
                 None => {
                     // This might happen if the stream ends abruptly or max tokens are hit mid-stream.
                     warn!("Stream ended without a clear stop reason. Finishing.");
                     break;
                 }
            }
        }
    } // End of main iteration loop

    // --- Post-Loop: Final Response Handling ---

    info!("Agent loop finished. Final aggregated text length: {}", final_response_text_aggregator.len());

    // Ensure there's some text, provide a default if not.
    if final_response_text_aggregator.is_empty() {
        if conversation_history.len() <= 1 { // Only initial user message
             final_response_text_aggregator = "No response generated by the AI.".to_string();
             warn!("submit_query finished with no response text and minimal history.");
        } else {
            final_response_text_aggregator = "Task completed (no final text response generated).".to_string();
             info!("submit_query finished with no final text, likely tool-only execution.");
        }
    }

    // Perform TTS synthesis on the final aggregated text
    let audio_base64 = match tts::invoke_tts(final_response_text_aggregator.clone(), state.clone()).await {
        Ok(base64) => Some(base64),
        Err(e) => {
            error!("TTS synthesis failed for final response: {}", e);
            None
        }
    };

    // Create the final result payload
    let final_result = SubmitQueryResult {
        text: final_response_text_aggregator.clone(), // Use the aggregated text
        audio_base64,
    };
    let final_payload = FinalAssistantResponsePayload {
        query: query.clone(), // Include original query
        response: final_result,
    };

    // Emit the final event to the frontend
     debug!("Emitting final-assistant-response event.");
    window.emit("final-assistant-response", final_payload)
        .map_err(|e| format!("Failed to emit final-assistant-response event: {}", e))?;

    info!("Successfully emitted final-assistant-response for query: {}", query);

    Ok(()) // Return Ok(()) as the command succeeded in emitting the final event
}
