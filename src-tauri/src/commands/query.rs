use crate::AppState;
use crate::api::anthropic::*;
use crate::models::anthropic::*;
use crate::tts::elevenlabs::invoke_elevenlabs_tts;
use reqwest::Client;
use serde_json::{json, Value};
use std::sync::Arc;

// Updated command to handle user queries with agent loop and TTS
#[tauri::command]
pub async fn submit_query(
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

        // Send request to Anthropic
        let anthropic_response = match send_anthropic_request(
            &conversation_history,
            &available_tools,
            &api_key,
            desktop_arc.clone(),
        ).await {
            Ok(response) => response,
            Err(e) => return Err(e),
        };

        println!("Anthropic Raw Response: {:?}", anthropic_response);
        desktop_arc.log(
            "debug",
            format!("Anthropic Raw Response: {:?}", anthropic_response),
        ); // Log raw response

        // Filter out thinking blocks before adding to history
        let filtered_content = filter_thinking_blocks(anthropic_response.content.clone());

        // Add assistant's response (filtered content blocks) to history
        let assistant_content_value = match serde_json::to_value(filtered_content.clone()) {
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

        // Process each content block
        for block in &filtered_content {
            if block.type_ == "tool_use" {
                has_tool_calls = true;
                match process_tool_call(block, &desktop_arc) {
                    Ok(result) => tool_results.push(result),
                    Err(e) => {
                        let err_msg = format!("Failed to process tool call: {}", e);
                        desktop_arc.log("error", err_msg.clone());
                        // Create an error tool result
                        tool_results.push(ToolResultBlock {
                            type_: "tool_result".to_string(),
                            tool_use_id: block.id.clone().unwrap_or_default(),
                            content: json!({"error": err_msg}),
                            is_error: Some(true),
                        });
                    }
                }
            } else if block.type_ == "text" {
                // Extract text for the final response
                if let Some(text) = &block.text {
                    if !has_tool_calls {
                        // If there are no tool calls, this is the final response
                        final_response_text = text.clone();
                    }
                }
            }
        }

        // If there are tool results, add them to the conversation and continue
        if !tool_results.is_empty() {
            let tool_results_value = match serde_json::to_value(tool_results) {
                Ok(v) => v,
                Err(e) => {
                    let err_msg = format!("Failed to serialize tool results: {}", e);
                    desktop_arc.log("error", err_msg.clone());
                    return Err(err_msg);
                }
            };
            conversation_history.push(AnthropicMessage {
                role: "user".to_string(),
                content: tool_results_value,
            });
        } else {
            // No tool calls, we're done
            break;
        }

        // Check if we've reached the stop reason
        if anthropic_response.stop_reason == "end_turn" && !has_tool_calls {
            break;
        }
    }

    // Generate audio for the final response if possible
    let audio_base64 = match std::env::var("ELEVENLABS_API_KEY") {
        Ok(_) => {
            match invoke_elevenlabs_tts(final_response_text.clone(), state).await {
                Ok(audio) => Some(audio),
                Err(e) => {
                    desktop_arc.log("warn", format!("TTS generation failed: {}", e));
                    None
                }
            }
        }
        Err(_) => None,
    };

    Ok(SubmitQueryResult {
        text: final_response_text,
        audio_base64,
    })
}