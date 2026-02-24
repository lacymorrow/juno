use std::env;
use async_trait::async_trait;
use serde_json::Value;
use tracing::{debug, info};

use crate::agent::{
    core::{AgentAction, AgentError, Message, Role, ToolDefinition},
    traits::AgentBrain,
};
use crate::agent::providers::types::model_ids;

/// Implementation of AgentBrain using Rig library
pub struct RigBrain {
    openai_api_key: String,
    model: String,
    system_prompt: Option<String>,
}

impl RigBrain {
    /// Creates a new RigBrain from a CentralizedProviderConfig struct.
    /// The api_key is expected to be pre-resolved by `ProviderConfig::resolve_provider()`
    /// (which handles the Rig → OpenAI key fallback). Falls back to OPENAI_API_KEY env var.
    pub fn from_config(config: &crate::settings::ProviderConfig) -> Result<Self, AgentError> {
        // api_key already resolved by resolve_provider() (Rig → OpenAI fallback)
        let openai_api_key = config.api_key.clone()
            .or_else(|| env::var("OPENAI_API_KEY").ok())
            .ok_or_else(|| AgentError::ConfigurationError(
                "OpenAI API key not found for Rig provider".into()
            ))?;
        let model = config.model.clone()
            .unwrap_or_else(|| model_ids::OPENAI_CUA.to_string());
        Ok(Self {
            openai_api_key,
            model,
            system_prompt: config.system_prompt.clone(),
        })
    }

    /// Create a new RigBrain instance with a specific model
    pub fn with_model(api_key: String, model: String, system_prompt: Option<String>) -> Self {
        Self {
            openai_api_key: api_key,
            model,
            system_prompt,
        }
    }
}

#[async_trait]
impl AgentBrain for RigBrain {
    async fn decide_next_action(
        &self,
        messages: &[Message],
        available_tools: &[ToolDefinition],
    ) -> Result<AgentAction, AgentError> {
        // We'll use reqwest directly instead of the rig_core library to avoid compatibility issues
        let client = reqwest::Client::new();

        // Format messages for OpenAI API
        let mut openai_messages = Vec::new();

        // Add system prompt if available
        if let Some(system) = &self.system_prompt {
            openai_messages.push(serde_json::json!({
                "role": "system",
                "content": system
            }));
        }

        // Add conversation messages
        for message in messages {
            let role = match message.role {
                Role::User => "user",
                Role::Assistant => "assistant",
                Role::System => "system",
                Role::Tool => "tool",
            };

            if let Some(tool_calls) = &message.tool_calls {
                // Message with tool calls
                let formatted_tool_calls = tool_calls.iter().map(|tc| {
                    serde_json::json!({
                        "id": tc.id,
                        "type": "function",
                        "function": {
                            "name": tc.name,
                            "arguments": tc.input.to_string()
                        }
                    })
                }).collect::<Vec<_>>();

                openai_messages.push(serde_json::json!({
                    "role": role,
                    "content": message.content,
                    "tool_calls": formatted_tool_calls
                }));
            } else if let Some(tool_call_id) = &message.tool_call_id {
                // Tool result message
                openai_messages.push(serde_json::json!({
                    "role": role,
                    "tool_call_id": tool_call_id,
                    "content": message.content
                }));
            } else {
                // Standard message
                openai_messages.push(serde_json::json!({
                    "role": role,
                    "content": message.content
                }));
            }
        }

        // Format tools for OpenAI API
        let tools = available_tools.iter().map(|tool| {
            serde_json::json!({
                "type": "function",
                "function": {
                    "name": tool.name,
                    "description": tool.description,
                    "parameters": tool.input_schema
                }
            })
        }).collect::<Vec<_>>();

        // Prepare the request payload
        let payload = serde_json::json!({
            "model": self.model,
            "messages": openai_messages,
            "tools": tools,
            "tool_choice": "auto"
        });

        debug!("Sending request to OpenAI API");

        // Send request to OpenAI API
        let response = client.post("https://api.openai.com/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", self.openai_api_key))
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await
            .map_err(|e| AgentError::LlmError(format!("Failed to send request to OpenAI: {}", e)))?;

        // Check for HTTP errors
        if !response.status().is_success() {
            let error_text = response.text().await
                .unwrap_or_else(|_| "Could not read error response".to_string());
            return Err(AgentError::LlmError(format!("OpenAI API error: {}", error_text)));
        }

        // Parse the response
        let response_json: serde_json::Value = response.json().await
            .map_err(|e| AgentError::LlmError(format!("Failed to parse OpenAI response: {}", e)))?;

        debug!("Received response from OpenAI");

        // Extract message content and tool calls
        let message = &response_json["choices"][0]["message"];

        // Check if we have tool calls
        if let Some(tool_calls) = message.get("tool_calls") {
            if let Some(tool_calls_array) = tool_calls.as_array() {
                if !tool_calls_array.is_empty() {
                    // Extract the first tool call (can be expanded to handle multiple)
                    let tool_call = &tool_calls_array[0];
                    let tool_name = tool_call["function"]["name"].as_str()
                        .ok_or_else(|| AgentError::LlmError("Tool name missing in response".to_string()))?;

                    // Parse arguments
                    let arguments_str = tool_call["function"]["arguments"].as_str()
                        .ok_or_else(|| AgentError::LlmError("Tool arguments missing in response".to_string()))?;

                    let arguments: Value = serde_json::from_str(arguments_str)
                        .map_err(|e| AgentError::LlmError(format!("Failed to parse tool arguments: {}", e)))?;

                    // If the assistant provided a text response, consider it a thought or intermediate step
                    // For now, we will treat any text from assistant as part of its thought process
                    // leading to a tool call or final answer.
                    let _thought = message["content"].as_str().unwrap_or("").to_string();
                    // TODO: How to best use this thought? Log it? Add to a specific thought history?
                    // For now, we assume the main content is the tool_calls if present.

                    info!("Agent wants to call tool: {}", tool_name);

                    // Use the ExecuteTool variant with a vector of ToolCalls
                    let tool_call = crate::agent::core::ToolCall {
                        id: tool_call["id"].as_str().unwrap_or("1").to_string(),
                        name: tool_name.to_string(),
                        input: arguments,
                    };

                    return Ok(AgentAction::ExecuteTool(vec![tool_call]));
                }
            }
        }

        // No tool calls, extract the message content
        let response_text = message["content"].as_str()
            .ok_or_else(|| AgentError::LlmError("Response content missing".to_string()))?
            .to_string();

        // Return as a direct response
        Ok(AgentAction::Finish(response_text))
    }
}
