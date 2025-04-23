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
use tauri::{Manager, Emitter}; // Import Manager and Emitter

// --- Agent State ---

#[derive(Debug, Clone, PartialEq, Serialize)] // Added Serialize for potential logging/debugging
enum AgentState {
    Thinking,
    Acting,
    ProcessingActionResult,
    Finished,
    Failed,
}

// --- Anthropic API Structs ---

#[derive(Serialize, Clone)]
pub(crate) struct AnthropicMessage {
    pub(crate) role: String,
    pub(crate) content: Value, // Keep as Value to handle complex content
}

#[derive(Deserialize, Debug, Clone, Serialize)]
pub(crate) struct AnthropicContentBlock {
    #[serde(rename = "type")]
    pub(crate) type_: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) text: Option<String>,
    // Fields related to tool_use
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) input: Option<Value>,
    // Fields related to tool_result (we create these, don't expect from API)
    // #[serde(skip_serializing_if = "Option::is_none")]
    // pub(crate) tool_use_id: Option<String>,
    // #[serde(skip_serializing_if = "Option::is_none")]
    // pub(crate) is_error: Option<bool>,
}


#[derive(Serialize, Clone)] // Added Clone
pub(crate) struct ToolResultBlock {
    #[serde(rename = "type")]
    pub(crate) type_: String, // Always "tool_result"
    pub(crate) tool_use_id: String,
    pub(crate) content: Value, // Changed to Value to match Anthropic's structure [ { "type": "text", "text": "..." } ] or [ { "type": "image", ... } ]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) is_error: Option<bool>,
}

// Keep this for payload structure, ensure Clone is derived
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SubmitQueryResult {
    pub text: String,
    pub audio_base64: Option<String>,
    pub agent_state: String, // Send final state to frontend
    // pub conversation_history: Vec<AnthropicMessage>, // Optionally send history for debugging
}

// Define the payload structure for the event
#[derive(Serialize, Clone)]
struct BackendResponsePayload {
    query: String,
    response: SubmitQueryResult,
}

// Removed AnthropicThinkingBudget as it was commented out
// #[derive(Serialize)]
// struct AnthropicThinkingBudget {
//     #[serde(rename = "type")]
//     type_: String,
//     budget_tokens: u32,
// }

#[derive(Serialize)]
struct AnthropicRequest<'a> {
    model: &'a str,
    max_tokens: u32,
    messages: Vec<AnthropicMessage>,
    tools: Vec<computer_use_ai_sdk::ToolDefinition>, // Use full path
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
    // #[serde(skip_serializing_if = "Option::is_none")] // Removed thinking budget
    // thinking: Option<AnthropicThinkingBudget>,
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

// --- Helper Functions ---

async fn call_anthropic_api(
    http_client: &Client,
    api_key: &str,
    request_payload: &AnthropicRequest<'_>,
) -> Result<AnthropicResponse, String> {
    let response = http_client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .header("anthropic-beta", "computer-use-2025-01-24") // Ensure correct beta header
        .header("content-type", "application/json")
        .json(request_payload)
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

    match response.json().await {
        Ok(res) => Ok(res),
        Err(e) => {
            let err_msg = format!("Failed to parse Anthropic JSON response: {}", e);
            error!("Error: {}", err_msg);
            Err(err_msg)
        }
    }
}

async fn process_screenshot(base64_data: &str) -> Result<Value, String> {
     match BASE64_STANDARD.decode(base64_data) {
        Ok(image_bytes) => {
            match image::load_from_memory(&image_bytes) {
                Ok(img) => {
                    let (width, height) = img.dimensions();
                    let max_dim = 1024.0; // Max dimension for resizing
                    let scale = if width > height {
                        max_dim / width as f32
                    } else {
                        max_dim / height as f32
                    };

                    let resized_img = if scale < 1.0 {
                        let new_width = (width as f32 * scale).round() as u32;
                        let new_height = (height as f32 * scale).round() as u32;
                        img.resize_exact(new_width, new_height, image::imageops::FilterType::Lanczos3)
                    } else {
                         img // No resize needed if already smaller or equal
                    };

                    let mut png_bytes = Vec::new();
                    match resized_img.write_to(&mut Cursor::new(&mut png_bytes), ImageFormat::Png) {
                        Ok(_) => {
                             let resized_base64_data = BASE64_STANDARD.encode(&png_bytes);
                             Ok(json!([{ // Return as array of content blocks
                                "type": "image",
                                "source": {
                                    "type": "base64",
                                    "media_type": "image/png",
                                    "data": resized_base64_data
                                }
                             }]))
                        }
                        Err(e) => {
                            let err_msg = format!("Failed to encode resized image to PNG: {}", e);
                            error!("{}", err_msg);
                            // Return error as text block within the array
                            Ok(json!([{"type": "text", "text": err_msg}]))
                        }
                    }
                }
                Err(e) => {
                    let err_msg = format!("Failed to load image from screenshot bytes: {}", e);
                    error!("{}", err_msg);
                     Ok(json!([{"type": "text", "text": err_msg}]))
                }
            }
        }
        Err(e) => {
            let err_msg = format!("Failed to decode base64 screenshot data: {}", e);
            error!("{}", err_msg);
             Ok(json!([{"type": "text", "text": err_msg}]))
        }
    }
}


// --- Submit Query Function (Refactored with State Machine) ---

#[tauri::command]
pub async fn submit_query(
    query: String,
    state: State<'_, AppState>,
    app_handle: tauri::AppHandle, // Pass AppHandle
) -> Result<(), String> { // Return Ok(()) or Err(string) for command result
    info!("Received query: {}", query);

    let api_key = match std::env::var("ANTHROPIC_API_KEY") {
         Ok(key) => key,
         Err(_) => {
             let err_msg = "ANTHROPIC_API_KEY not configured.".to_string();
             error!("{}", err_msg);
              // Emit error response immediately if API key is missing
             let result = SubmitQueryResult {
                 text: err_msg.clone(),
                 audio_base64: None,
                 agent_state: format!("{:?}", AgentState::Failed), // Indicate failure state
             };
             let payload = BackendResponsePayload { query, response: result };
             app_handle.get_window("main").ok_or("Main window not found")?.emit("backend-response", payload).map_err(|e| format!("Emit failed: {}", e))?;
             return Err(err_msg); // Return error from the command itself
         }
     };

    let desktop_arc = state.desktop.clone();
    let http_client = Client::new();
    let mut conversation_history: Vec<AnthropicMessage> = Vec::new();
    let mut final_response_text = String::new();
    let mut last_error_message: Option<String> = None; // Store last error for final reporting
    const MAX_ITERATIONS: u32 = 25;
    let mut iteration = 0;
    let mut agent_state = AgentState::Thinking;
    let mut current_tool_results: Vec<ToolResultBlock> = Vec::new();

    // Initial user message
    conversation_history.push(AnthropicMessage {
        role: "user".to_string(),
        content: json!([{ "type": "text", "text": query.clone() }]), // Wrap initial query in content block structure
    });

    let available_tools = list_tools(&desktop_arc); // Get tools once

    // Agent Loop
    while agent_state != AgentState::Finished && agent_state != AgentState::Failed && iteration < MAX_ITERATIONS {
        info!("Agent Iteration: {}, State: {:?}", iteration + 1, agent_state);
        iteration += 1;

        match agent_state {
            AgentState::Thinking => {
                let max_output_tokens = 1024;
                // let thinking_budget = 4000; // Anthropic calculates this based on max_tokens
                // let total_max_tokens = max_output_tokens + thinking_budget; // Use only max_tokens

                let request_payload = AnthropicRequest {
                    model: "claude-3-5-sonnet-20240620",
                    max_tokens: max_output_tokens, // Specify desired output tokens
                    messages: conversation_history.clone(),
                    tools: available_tools.clone(),
                    system: Some("You are an AI assistant that can use tools to interact with the user's computer desktop environment. Use the provided tools to fulfill the user's request. Respond with the final result or status. When using tools, provide necessary thought process or context before the tool call. When finished, provide a final concise answer.".to_string()),
                };

                match call_anthropic_api(&http_client, &api_key, &request_payload).await {
                    Ok(anthropic_response) => {
                        debug!("Anthropic Raw Response: {:?}", anthropic_response);

                        // --- Process Response ---
                        let mut has_tool_calls = false;
                        let mut current_text_parts = Vec::new();
                        let mut tool_use_blocks = Vec::new();

                        for block in anthropic_response.content.iter() {
                            match block.type_.as_str() {
                                "text" => {
                                    if let Some(text) = &block.text {
                                        current_text_parts.push(text.clone());
                                    }
                                }
                                "tool_use" => {
                                    if block.id.is_some() && block.name.is_some() && block.input.is_some() {
                                        has_tool_calls = true;
                                        tool_use_blocks.push(block.clone()); // Store for Acting state
                                    } else {
                                        warn!("Received incomplete tool_use block: {:?}", block);
                                    }
                                }
                                "thinking" => { /* Ignore thinking blocks */ }
                                _ => { warn!("Unknown content block type: {}", block.type_); }
                            }
                        }

                        // Add assistant message to history (including any tool uses it requested)
                        let assistant_content_value = match serde_json::to_value(anthropic_response.content.clone()) {
                             Ok(v) => v,
                             Err(e) => {
                                 let err_msg = format!("Failed to serialize assistant content: {}", e);
                                 error!("Error: {}", err_msg);
                                 last_error_message = Some(err_msg);
                                 agent_state = AgentState::Failed;
                                 continue; // Skip to next loop iteration (will fail)
                             }
                         };
                        conversation_history.push(AnthropicMessage {
                            role: "assistant".to_string(),
                            content: assistant_content_value,
                        });

                        // Update final_response_text if new text exists
                        if !current_text_parts.is_empty() {
                            final_response_text = current_text_parts.join("
"); // Join text parts
                             info!("Assistant text: {}", final_response_text);
                        }

                        // --- State Transition ---
                        match anthropic_response.stop_reason.as_str() {
                            "tool_use" => {
                                if has_tool_calls {
                                    agent_state = AgentState::Acting;
                                } else {
                                    // Should not happen if stop_reason is tool_use, but handle defensively
                                    warn!("Stop reason is 'tool_use' but no valid tool calls found. Finishing.");
                                     agent_state = AgentState::Finished;
                                     if final_response_text.is_empty() {
                                         final_response_text = "Task ended unexpectedly after tool signal.".to_string();
                                     }
                                }
                            }
                            "stop_sequence" | "end_turn" => {
                                agent_state = AgentState::Finished;
                                info!("Stop reason '{}' received. Finishing task.", anthropic_response.stop_reason);
                                if final_response_text.is_empty() {
                                    final_response_text = "Task completed (no text response generated).".to_string();
                                }
                            }
                            "max_tokens" => {
                                warn!("Stop reason 'max_tokens'. Task may be incomplete. Finishing.");
                                final_response_text.push_str("
[Agent stopped due to maximum token limit]");
                                agent_state = AgentState::Finished; // Treat as finished for now
                            }
                            _ => { // Other reasons like "error" might imply failure
                                let err_msg = format!("Task stopped due to unexpected reason: {}", anthropic_response.stop_reason);
                                warn!("{}", err_msg);
                                last_error_message = Some(err_msg);
                                agent_state = AgentState::Failed; // Assume failure for unknown/error reasons
                            }
                        }
                    }
                    Err(e) => {
                        last_error_message = Some(e);
                        agent_state = AgentState::Failed;
                    }
                }
            } // End Thinking state

            AgentState::Acting => {
                // Extract tool calls from the last assistant message
                let tool_calls_to_execute = conversation_history.last().map_or(Vec::new(), |last_msg| {
                    if last_msg.role == "assistant" {
                        serde_json::from_value::<Vec<AnthropicContentBlock>>(last_msg.content.clone())
                            .unwrap_or_default()
                            .into_iter()
                            .filter(|block| block.type_ == "tool_use" && block.id.is_some() && block.name.is_some() && block.input.is_some())
                            .collect()
                    } else {
                        Vec::new()
                    }
                });

                if tool_calls_to_execute.is_empty() {
                     warn!("Entered Acting state but found no tool calls in last assistant message. Finishing.");
                     agent_state = AgentState::Finished;
                     if final_response_text.is_empty() { final_response_text = "Task ended unexpectedly after planning to act.".to_string(); }
                     continue;
                 }

                let app_handle_clone = app_handle.clone();
                let state_clone = state.clone(); // Clone State for async block

                let futures = tool_calls_to_execute.iter().map(|block| {
                    let id_clone = block.id.clone().unwrap(); // Already checked Some in filter
                    let name_clone = block.name.clone().unwrap();
                    let input_clone = block.input.clone().unwrap();
                    let desktop_arc_clone = desktop_arc.clone();
                    let app_handle_clone_inner = app_handle_clone.clone();
                     // Create a new clone of State inside the map closure for each future
                     let state_clone_for_async = state_clone.clone();


                    async move {
                        info!("Executing tool: {} (ID: {})", name_clone, id_clone);
                        let tool_result_value = handle_tool_call(
                            &desktop_arc_clone,
                            &app_handle_clone_inner,
                            &name_clone,
                            &input_clone,
                            &state_clone_for_async, // Pass the cloned State
                        ).await;
                        debug!("Raw tool result for {}: {:?}", name_clone, tool_result_value);

                        let is_error = tool_result_value.get("error").is_some() || tool_result_value.get("status").and_then(|s| s.as_str()) == Some("error");

                        // Process result into Anthropic content block format [ { "type": "text", ... } ] or [ { "type": "image", ... } ]
                        let processed_content_value = if (name_clone == "captureScreenshot" || name_clone == "capture_element_screenshot_command") && !is_error {
                            // Try fetching base64 data with known keys
                            if let Some(base64_data) = tool_result_value.get("screenshot_base64").and_then(|v| v.as_str()) {
                                process_screenshot(base64_data).await.unwrap_or_else(|err_msg| {
                                    json!([{"type": "text", "text": err_msg}])
                                })
                            } else if let Some(base64_data) = tool_result_value.get("image_base64").and_then(|v| v.as_str()) {
                                process_screenshot(base64_data).await.unwrap_or_else(|err_msg| {
                                    json!([{"type": "text", "text": err_msg}])
                                })
                            } else {
                                // Handle case where neither key is found
                                let error_msg = format!("Tool '{}' succeeded but missing expected base64 key ('screenshot_base64' or 'image_base64') in result: {:?}", name_clone, tool_result_value);
                                error!("{}", error_msg);
                                json!([{"type": "text", "text": error_msg}])
                            }
                        } else { // Handle non-screenshot results or errors
                             // Use serde_json::to_string for proper JSON serialization
                             let result_str = serde_json::to_string(&tool_result_value).unwrap_or_else(|e| {
                                 // Fallback JSON string if serialization fails
                                 format!("{{\"error\": \"Failed to serialize tool result: {}\"}}", e)
                             });
                             json!([{"type": "text", "text": result_str}])
                        };

                        (id_clone, processed_content_value, is_error)
                    }
                }).collect::<Vec<_>>();

                // Execute all tool calls concurrently
                let results = future::join_all(futures).await;

                // Store results for the next state
                current_tool_results = results.into_iter().map(|(id, content_value, is_error)| {
                    ToolResultBlock {
                        type_: "tool_result".to_string(),
                        tool_use_id: id,
                        content: content_value, // Already formatted as content blocks
                        is_error: Some(is_error),
                    }
                }).collect();

                agent_state = AgentState::ProcessingActionResult;
            } // End Acting state

            AgentState::ProcessingActionResult => {
                if current_tool_results.is_empty() {
                    // Should not happen if we came from Acting, but handle defensively
                    warn!("Entered ProcessingActionResult state but no tool results found. Thinking again.");
                     agent_state = AgentState::Thinking; // Go back to thinking maybe something went wrong
                     continue;
                 }

                // Add tool results as a "user" message to the history
                 let tool_results_value = match serde_json::to_value(&current_tool_results) {
                     Ok(v) => v,
                     Err(e) => {
                         let err_msg = format!("Failed to serialize tool results: {}", e);
                         error!("Error: {}", err_msg);
                         last_error_message = Some(err_msg);
                         agent_state = AgentState::Failed;
                         continue; // Skip to next loop iteration (will fail)
                     }
                 };
                 conversation_history.push(AnthropicMessage {
                    role: "user".to_string(), // Role is 'user' for tool results message
                    content: tool_results_value, // Content is the array of ToolResultBlock
                });

                // Clear the temporary results
                current_tool_results.clear();

                // Go back to thinking with the new tool results in history
                agent_state = AgentState::Thinking;
            } // End ProcessingActionResult state

            AgentState::Finished | AgentState::Failed => {
                // Loop condition will handle this, but break just in case
                break;
            }
        } // End match agent_state
    } // End while loop

    // --- Post-Loop Processing ---

    // Determine final state and message
    let final_state = if agent_state == AgentState::Finished {
        AgentState::Finished
    } else if iteration >= MAX_ITERATIONS {
        warn!("Agent reached maximum iterations ({})", MAX_ITERATIONS);
        final_response_text.push_str(&format!("
[Agent reached maximum iterations ({})]", MAX_ITERATIONS));
        AgentState::Failed // Consider max iterations a failure or incomplete state
    } else {
        // Must be AgentState::Failed
        error!("Agent finished in Failed state. Last error: {:?}", last_error_message);
        if final_response_text.is_empty() {
            final_response_text = last_error_message.unwrap_or_else(|| "Agent failed due to an unknown error.".to_string());
        } else if let Some(err) = last_error_message {
            final_response_text.push_str(&format!("
[Agent Error: {}]", err));
        }
        AgentState::Failed
    };

    info!("Agent finished with state: {:?}", final_state);

    // Perform TTS synthesis
    let audio_base64 = match tts::invoke_tts(final_response_text.clone(), state.clone()).await {
        Ok(base64) => Some(base64),
        Err(e) => {
            error!("TTS synthesis failed: {}", e);
            None // Proceed without audio if TTS fails
        }
    };

    // Prepare final result payload
    let result = SubmitQueryResult {
        text: final_response_text,
        audio_base64,
        agent_state: format!("{:?}", final_state), // Send final state as string
        // conversation_history: conversation_history, // Uncomment to send history for debugging
    };
    let payload = BackendResponsePayload {
        query: query.clone(), // Use the original query
        response: result,
    };

    // Emit the final response event
    info!("Emitting final backend-response");
     match app_handle.get_window("main") {
         Some(window) => {
             window.emit("backend-response", payload)
                 .map_err(|e| format!("Failed to emit backend-response event: {}", e))?;
             info!("Successfully emitted backend-response event.");
             Ok(()) // Command succeeded
         }
         None => {
             let err_msg = "Main window not found, cannot emit event.".to_string();
             error!("{}", err_msg);
             Err(err_msg) // Command failed
         }
     }
}
