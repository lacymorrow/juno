use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::env;

use crate::agent::structs::{
    AgentAction, AgentError, Message, Role, ToolCall, ToolDefinition,
};
use crate::agent::traits::AgentBrain;

// --- OpenAI API Structs --- //

#[derive(Serialize, Debug)]
struct OpenAIRequest {
    model: String,
    messages: Vec<OpenAIMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<OpenAITool>>,
    temperature: f32,
    max_tokens: u32,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct OpenAIMessage {
    role: String,
    content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<OpenAIToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct OpenAIToolCall {
    id: String,
    #[serde(rename = "type")]
    call_type: String,
    function: OpenAIFunction,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct OpenAIFunction {
    name: String,
    arguments: String,
}

#[derive(Serialize, Debug)]
struct OpenAITool {
    #[serde(rename = "type")]
    tool_type: String,
    function: OpenAIToolFunction,
}

#[derive(Serialize, Debug)]
struct OpenAIToolFunction {
    name: String,
    description: String,
    parameters: Value,
}

#[derive(Deserialize, Debug)]
struct OpenAIResponse {
    id: String,
    object: String,
    created: u64,
    model: String,
    choices: Vec<OpenAIChoice>,
    // usage: OpenAIUsage,
}

#[derive(Deserialize, Debug)]
struct OpenAIChoice {
    index: u32,
    message: OpenAIMessage,
    finish_reason: String,
}

// --- OpenAIBrain Implementation --- //

const OPENAI_API_URL: &str = "https://api.openai.com/v1/chat/completions";
const DEFAULT_MODEL: &str = "gpt-4o";
const DEFAULT_MAX_TOKENS: u32 = 4096;
const DEFAULT_TEMPERATURE: f32 = 0.7;

#[derive(Clone)]
pub struct OpenAIBrain {
    client: Client,
    api_key: String,
    model: String,
    max_tokens: u32,
    temperature: f32,
}

impl OpenAIBrain {
    pub fn new(
        api_key: String,
        model: Option<String>,
        max_tokens: Option<u32>,
        temperature: Option<f32>,
    ) -> Result<Self, AgentError> {
        Ok(OpenAIBrain {
            client: Client::new(),
            api_key,
            model: model.unwrap_or_else(|| DEFAULT_MODEL.to_string()),
            max_tokens: max_tokens.unwrap_or(DEFAULT_MAX_TOKENS),
            temperature: temperature.unwrap_or(DEFAULT_TEMPERATURE),
        })
    }

    /// Creates a new OpenAIBrain using the API key from the environment variables.
    pub fn from_env() -> Result<Self, AgentError> {
        let api_key = env::var("OPENAI_API_KEY")
            .map_err(|_| AgentError::ConfigurationError("OPENAI_API_KEY environment variable not set".to_string()))?;

        let model = env::var("OPENAI_MODEL").ok();
        let max_tokens = env::var("OPENAI_MAX_TOKENS").ok().and_then(|s| s.parse::<u32>().ok());
        let temperature = env::var("OPENAI_TEMPERATURE").ok().and_then(|s| s.parse::<f32>().ok());

        Self::new(api_key, model, max_tokens, temperature)
    }

    // Helper to convert our internal Message format to OpenAI's format
    fn convert_message_to_openai(&self, message: &Message) -> Result<OpenAIMessage, AgentError> {
        let role = match message.role {
            Role::User => "user",
            Role::Assistant => "assistant",
            Role::System => "system",
            Role::Tool => "tool",
        };

        // Convert tool calls to OpenAI format if present
        let tool_calls = if let Some(ref calls) = message.tool_calls {
            let mut openai_calls = Vec::new();
            for call in calls {
                // Convert input JSON to string for OpenAI
                let arguments = serde_json::to_string(&call.input)
                    .map_err(|e| AgentError::LlmError(format!("Failed to serialize tool call arguments: {}", e)))?;

                openai_calls.push(OpenAIToolCall {
                    id: call.id.clone(),
                    call_type: "function".to_string(),
                    function: OpenAIFunction {
                        name: call.name.clone(),
                        arguments,
                    },
                });
            }
            Some(openai_calls)
        } else {
            None
        };

        Ok(OpenAIMessage {
            role: role.to_string(),
            content: if message.content.is_empty() { None } else { Some(message.content.clone()) },
            tool_calls,
            tool_call_id: message.tool_call_id.clone(),
            name: message.name.clone(),
        })
    }
}

#[async_trait]
impl AgentBrain for OpenAIBrain {
    async fn decide_next_action(
        &self,
        messages: &[Message],
        available_tools: &[ToolDefinition],
    ) -> Result<AgentAction, AgentError> {
        // Convert messages to OpenAI format
        let mut openai_messages = Vec::new();
        for message in messages {
            match self.convert_message_to_openai(message) {
                Ok(msg) => openai_messages.push(msg),
                Err(e) => {
                    log::warn!("Error converting message to OpenAI format: {}", e);
                    continue;
                }
            }
        }

        // Convert tools to OpenAI format
        let tools = if !available_tools.is_empty() {
            let mut openai_tools = Vec::new();
            for tool in available_tools {
                openai_tools.push(OpenAITool {
                    tool_type: "function".to_string(),
                    function: OpenAIToolFunction {
                        name: tool.name.clone(),
                        description: tool.description.clone(),
                        parameters: tool.input_schema.clone(),
                    },
                });
            }
            Some(openai_tools)
        } else {
            None
        };

        // Create request payload
        let request = OpenAIRequest {
            model: self.model.clone(),
            messages: openai_messages,
            tools,
            temperature: self.temperature,
            max_tokens: self.max_tokens,
        };

        // Log request for debugging
        match serde_json::to_string_pretty(&request) {
            Ok(json) => log::debug!("OpenAI request: {}", json),
            Err(e) => log::error!("Failed to serialize OpenAI request: {}", e),
        }

        // PLACEHOLDER: Actual OpenAI API call implementation
        // TODO: Implement actual API call to OpenAI

        log::warn!("OpenAI Brain implementation is incomplete. Returning placeholder response.");

        // For now, return a placeholder response
        Ok(AgentAction::Finish("This is a placeholder response from the OpenAI provider. The implementation is not yet complete.".to_string()))
    }
}
