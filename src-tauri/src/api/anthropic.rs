use crate::models::anthropic::*;
use crate::AppState;
use reqwest::Client;
use serde_json::{json, Value};
use std::sync::Arc;
use computer_use_ai_sdk::Desktop;

pub async fn send_anthropic_request(
    conversation_history: &[AnthropicMessage],
    available_tools: &[computer_use_ai_sdk::ToolDefinition],
    api_key: &str,
    desktop_arc: Arc<Desktop>,
) -> Result<AnthropicResponseWithContent, String> {
    let http_client = Client::new();
    
    let max_output_tokens = 1024; // Desired max output tokens for the final answer
    let thinking_budget = 4000; // Recommended thinking budget
    let total_max_tokens = max_output_tokens + thinking_budget; // Total max tokens including thinking

    let request_payload = AnthropicRequest {
        model: "claude-3-7-sonnet-20250219", // Use Claude 3.5 Sonnet
        max_tokens: total_max_tokens, // Set total max tokens
        messages: conversation_history.to_vec(), // Clone history for this request
        tools: available_tools.to_vec(),
        system: Some("You are an AI assistant that can use tools to interact with the user's computer desktop environment. Use the provided tools to fulfill the user's request. Respond with the final result or status.".to_string()),
        thinking: None, // Commented out: Some(AnthropicThinkingBudget { type_: "enabled".to_string(), budget_tokens: thinking_budget, }),
    };

    // Send request to Anthropic
    let response = http_client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", api_key)
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
        desktop_arc.log("error", err_msg.clone());
        return Err(err_msg);
    }

    // Parse the successful response
    match response.json().await {
        Ok(res) => Ok(res),
        Err(e) => {
            let err_msg = format!("Failed to parse Anthropic JSON response: {}", e);
            desktop_arc.log("error", err_msg.clone());
            Err(err_msg)
        }
    }
}

pub fn filter_thinking_blocks(content: Vec<AnthropicContentBlock>) -> Vec<AnthropicContentBlock> {
    content.into_iter()
        .filter(|block| block.type_ != "thinking")
        .collect()
}

pub fn process_tool_call(
    tool_block: &AnthropicContentBlock,
    desktop_arc: &Arc<Desktop>,
) -> Result<ToolResultBlock, String> {
    let tool_id = tool_block.id.as_ref().ok_or_else(|| "Tool ID missing".to_string())?;
    let tool_name = tool_block.name.as_ref().ok_or_else(|| "Tool name missing".to_string())?;
    let tool_input = tool_block.input.as_ref().ok_or_else(|| "Tool input missing".to_string())?;
    
    desktop_arc.log("info", format!("Processing tool call: {}", tool_name));
    
    // Call the tool via the desktop API
    match desktop_arc.call_tool(tool_name, tool_input.clone()) {
        Ok(result) => {
            // Create a successful tool result
            Ok(ToolResultBlock {
                type_: "tool_result".to_string(),
                tool_use_id: tool_id.clone(),
                content: result,
                is_error: None,
            })
        }
        Err(e) => {
            let error_message = format!("Error executing tool {}: {}", tool_name, e);
            desktop_arc.log("error", error_message.clone());
            
            // Create an error tool result
            Ok(ToolResultBlock {
                type_: "tool_result".to_string(),
                tool_use_id: tool_id.clone(),
                content: json!({"error": error_message}),
                is_error: Some(true),
            })
        }
    }
}