use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::env;

use crate::agent::providers::factory::model_ids;
use crate::agent::core::{AgentAction, AgentError, Message, Role, ToolCall, ToolDefinition};
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

#[derive(Serialize, Deserialize, Debug)]
struct OpenAIResponse {
    #[allow(dead_code)]
    id: String,
    #[allow(dead_code)]
    object: String,
    #[allow(dead_code)]
    created: u64,
    #[allow(dead_code)]
    model: String,
    choices: Vec<OpenAIChoice>,
    // usage: OpenAIUsage,
}

#[derive(Serialize, Deserialize, Debug)]
struct OpenAIChoice {
    #[allow(dead_code)]
    index: u32,
    message: OpenAIMessage,
    #[allow(dead_code)]
    finish_reason: String,
}

// --- OpenAIBrain Implementation --- //

const OPENAI_API_URL: &str = "https://api.openai.com/v1/chat/completions";
const DEFAULT_MODEL: &str = model_ids::GPT_4O;
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
    /// Creates a new OpenAI brain with optional configuration.
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
        let api_key = env::var("OPENAI_API_KEY").map_err(|_| {
            AgentError::ConfigurationError(
                "OPENAI_API_KEY environment variable not set".to_string(),
            )
        })?;

        let model = env::var("OPENAI_MODEL").ok();
        let max_tokens = env::var("OPENAI_MAX_TOKENS")
            .ok()
            .and_then(|s| s.parse::<u32>().ok());
        let temperature = env::var("OPENAI_TEMPERATURE")
            .ok()
            .and_then(|s| s.parse::<f32>().ok());

        Self::new(api_key, model, max_tokens, temperature)
    }

    /// Sanitize log content by removing or truncating base64 data to prevent console spam
    fn sanitize_for_logging(value: &serde_json::Value) -> serde_json::Value {
        match value {
            serde_json::Value::String(s) => {
                // Check if this looks like base64 data (long string with base64 characters)
                if s.len() > 100
                    && s.chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '/' || c == '=')
                {
                    // Truncate base64 data and add indication it was truncated
                    serde_json::Value::String(format!(
                        "{}...[BASE64_DATA_TRUNCATED_{}bytes]",
                        &s[..std::cmp::min(50, s.len())],
                        s.len()
                    ))
                } else {
                    serde_json::Value::String(s.clone())
                }
            }
            serde_json::Value::Object(obj) => {
                let mut sanitized = serde_json::Map::new();
                for (key, val) in obj {
                    sanitized.insert(key.clone(), Self::sanitize_for_logging(val));
                }
                serde_json::Value::Object(sanitized)
            }
            serde_json::Value::Array(arr) => {
                let sanitized: Vec<_> = arr.iter().map(Self::sanitize_for_logging).collect();
                serde_json::Value::Array(sanitized)
            }
            _ => value.clone(),
        }
    }

    /// Sanitize API request/response structures for logging
    fn sanitize_request_for_logging(request: &OpenAIRequest) -> serde_json::Value {
        match serde_json::to_value(request) {
            Ok(value) => Self::sanitize_for_logging(&value),
            Err(_) => serde_json::Value::String("[SERIALIZATION_ERROR]".to_string()),
        }
    }

    /// Sanitize API response structures for logging
    fn sanitize_response_for_logging(response: &OpenAIResponse) -> serde_json::Value {
        match serde_json::to_value(response) {
            Ok(value) => Self::sanitize_for_logging(&value),
            Err(_) => serde_json::Value::String("[SERIALIZATION_ERROR]".to_string()),
        }
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
                let arguments = serde_json::to_string(&call.input).map_err(|e| {
                    AgentError::LlmError(format!("Failed to serialize tool call arguments: {}", e))
                })?;

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
            content: if message.content.is_empty() {
                None
            } else {
                Some(message.content.clone())
            },
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
        match serde_json::to_string_pretty(&Self::sanitize_request_for_logging(&request)) {
            Ok(json) => log::debug!("OpenAI request: {}", json),
            Err(e) => log::error!("Failed to serialize OpenAI request: {}", e),
        }

        // Make the API call to OpenAI
        let response = self
            .client
            .post(OPENAI_API_URL)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| AgentError::LlmError(format!("HTTP request failed: {}", e)))?;

        // Check for HTTP errors
        if !response.status().is_success() {
            let status = response.status();
            let error_body = response
                .text()
                .await
                .unwrap_or_else(|_| "Failed to read error body".to_string());
            log::error!("OpenAI API Error: Status {}, Body: {}", status, error_body);
            return Err(AgentError::LlmError(format!(
                "OpenAI API returned error {}: {}",
                status, error_body
            )));
        }

        // Parse the response
        let response_body: OpenAIResponse = response
            .json()
            .await
            .map_err(|e| AgentError::LlmError(format!("Failed to parse API response: {}", e)))?;

        log::debug!(
            "Received response from OpenAI: {:?}",
            Self::sanitize_response_for_logging(&response_body)
        );

        if response_body.choices.is_empty() {
            return Err(AgentError::LlmError(
                "OpenAI returned empty choices array".to_string(),
            ));
        }

        // Process the first choice (typically there's only one)
        let choice = &response_body.choices[0];
        let message = &choice.message;

        // Determine which action to take based on the response
        if let Some(tool_calls) = &message.tool_calls {
            if !tool_calls.is_empty() {
                // Convert OpenAI tool calls to our internal format
                let mut calls = Vec::new();
                for tool_call in tool_calls {
                    if tool_call.call_type != "function" {
                        log::warn!("Unsupported tool call type: {}", tool_call.call_type);
                        continue;
                    }

                    // Parse arguments JSON
                    let input: Value = match serde_json::from_str(&tool_call.function.arguments) {
                        Ok(json) => json,
                        Err(e) => {
                            log::warn!(
                                "Failed to parse tool arguments: {}, args: {}",
                                e,
                                tool_call.function.arguments
                            );
                            continue;
                        }
                    };

                    calls.push(ToolCall {
                        id: tool_call.id.clone(),
                        name: tool_call.function.name.clone(),
                        input,
                    });
                }

                if !calls.is_empty() {
                    return Ok(AgentAction::ExecuteTool(calls));
                }
            }
        }

        // If no tool calls, return the message content as a text response
        match &message.content {
            Some(content) if !content.is_empty() => Ok(AgentAction::Finish(content.clone())),
            _ => {
                log::warn!("OpenAI response had no content");
                Err(AgentError::LlmError(
                    "OpenAI response had no content".to_string(),
                ))
            }
        }
    }
}
