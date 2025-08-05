use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::env;
use tracing;

use crate::agent::core::{AgentAction, AgentError, Message, Role, ToolCall, ToolDefinition};
use crate::agent::traits::AgentBrain;

#[derive(Serialize, Debug)]
struct GeminiRequest {
    contents: Vec<GeminiContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<GeminiTool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    system_instruction: Option<GeminiContent>,
    generation_config: GeminiGenerationConfig,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct GeminiContent {
    role: String,
    parts: Vec<GeminiPart>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(untagged)]
enum GeminiPart {
    Text(GeminiTextPart),
    FunctionCall(GeminiFunctionCall),
    FunctionResponse(GeminiFunctionResponse),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct GeminiTextPart {
    text: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct GeminiFunctionCall {
    function_call: GeminiFunctionCallData,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct GeminiFunctionCallData {
    name: String,
    args: Value,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct GeminiFunctionResponse {
    function_response: GeminiFunctionResponseData,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct GeminiFunctionResponseData {
    name: String,
    response: Value,
}

#[derive(Serialize, Debug)]
struct GeminiTool {
    function_declarations: Vec<GeminiFunctionDeclaration>,
}

#[derive(Serialize, Debug)]
struct GeminiFunctionDeclaration {
    name: String,
    description: String,
    parameters: Value,
}

#[derive(Serialize, Debug)]
struct GeminiGenerationConfig {
    temperature: f32,
    top_p: f32,
    top_k: i32,
    max_output_tokens: i32,
    response_mime_type: String,
}

#[derive(Deserialize, Debug)]
struct GeminiResponse {
    candidates: Vec<GeminiCandidate>,
    // usage_metadata field removed - unused for performance
}

#[derive(Deserialize, Debug)]
struct GeminiCandidate {
    content: GeminiContent,
    // finish_reason and safety_ratings fields removed - unused for performance
}

// Removed unused structs: GeminiUsageMetadata and GeminiSafetyRating
// These were never accessed in the code, removing for performance

const GEMINI_API_BASE: &str = "https://generativelanguage.googleapis.com/v1beta/models";

#[derive(Clone)]
pub struct GeminiBrain {
    client: Client,
    api_key: String,
    model: String,
    max_tokens: i32,
    system_prompt: Option<String>,
    temperature: f32,
}

impl GeminiBrain {
    pub fn new(
        api_key: String,
        model: Option<String>,
        max_tokens: Option<i32>,
        system_prompt: Option<String>,
        temperature: Option<f32>,
    ) -> Result<Self, AgentError> {
        use crate::agent::providers::factory::Provider;

        // Use centralized defaults from provider configuration
        let model = model.unwrap_or_else(|| Provider::Gemini.default_model().to_string());
        let max_tokens = max_tokens.unwrap_or(crate::constants::agent::config::DEFAULT_MAX_TOKENS_COMPACT);
        let temperature = temperature.unwrap_or(0.1); // Low temperature for consistent routing decisions

        Ok(GeminiBrain {
            client: Client::new(),
            api_key,
            model,
            max_tokens,
            system_prompt,
            temperature,
        })
    }

    pub fn from_env() -> Result<Self, AgentError> {
        let api_key = env::var("GEMINI_API_KEY").map_err(|_| {
            AgentError::ConfigurationError(
                "GEMINI_API_KEY environment variable not set".to_string(),
            )
        })?;

        let model = env::var("GEMINI_MODEL").ok();
        let max_tokens = env::var("GEMINI_MAX_TOKENS")
            .ok()
            .and_then(|s| s.parse::<i32>().ok());
        let system_prompt = env::var("GEMINI_SYSTEM_PROMPT").ok();
        let temperature = env::var("GEMINI_TEMPERATURE")
            .ok()
            .and_then(|s| s.parse::<f32>().ok());

        Self::new(api_key, model, max_tokens, system_prompt, temperature)
    }

    fn convert_message_to_gemini(message: &Message) -> Result<GeminiContent, AgentError> {
        let role = match message.role {
            Role::User => "user",
            Role::Assistant => "model",
            Role::System => {
                return Err(AgentError::LlmError(
                    "System messages should be passed via system_instruction parameter".to_string(),
                ))
            }
            Role::Tool => "model", // Tool responses are handled as model responses with function_response
        };

        let mut parts = Vec::new();

        // Handle regular text content
        if !message.content.is_empty() && message.role != Role::Tool {
            parts.push(GeminiPart::Text(GeminiTextPart {
                text: message.content.clone(),
            }));
        }

        // Handle tool calls (for assistant messages)
        if let Some(tool_calls) = &message.tool_calls {
            for tool_call in tool_calls {
                parts.push(GeminiPart::FunctionCall(GeminiFunctionCall {
                    function_call: GeminiFunctionCallData {
                        name: tool_call.name.clone(),
                        args: tool_call.input.clone(),
                    },
                }));
            }
        }

        // Handle tool results
        if message.role == Role::Tool {
            let tool_name = message.name.as_ref().ok_or_else(|| {
                AgentError::LlmError("Tool result message missing tool name".to_string())
            })?;

            let response_content =
                if message.content.starts_with('{') || message.content.starts_with('[') {
                    // Try to parse as JSON
                    serde_json::from_str::<Value>(&message.content)
                        .unwrap_or_else(|_| Value::String(message.content.clone()))
                } else {
                    Value::String(message.content.clone())
                };

            parts.push(GeminiPart::FunctionResponse(GeminiFunctionResponse {
                function_response: GeminiFunctionResponseData {
                    name: tool_name.clone(),
                    response: response_content,
                },
            }));
        }

        Ok(GeminiContent {
            role: role.to_string(),
            parts,
        })
    }

    fn convert_tool_definitions(tools: &[ToolDefinition]) -> Vec<GeminiFunctionDeclaration> {
        tools.iter().enumerate().map(|(index, tool)| {
            // Debug logging to identify problematic tools
            tracing::debug!("Converting tool {}: {} (index {})", tool.name, tool.description, index);

            // Validate and potentially fix the input schema for Gemini API compatibility
            let sanitized_schema = match &tool.input_schema {
                Value::Object(map) => {
                    let mut sanitized = map.clone();

                    // Ensure the schema follows JSON Schema format expected by Gemini
                    if !sanitized.contains_key("type") {
                        sanitized.insert("type".to_string(), Value::String("object".to_string()));
                    }

                    // Ensure properties is an object, not an array
                    if let Some(properties) = sanitized.get("properties") {
                        if !properties.is_object() {
                            tracing::warn!("Tool {} has invalid properties field (not an object), attempting to fix", tool.name);
                            sanitized.insert("properties".to_string(), Value::Object(serde_json::Map::new()));
                        }
                    } else {
                        sanitized.insert("properties".to_string(), Value::Object(serde_json::Map::new()));
                    }

                    // Ensure required is an array
                    if let Some(required) = sanitized.get("required") {
                        if !required.is_array() {
                            tracing::warn!("Tool {} has invalid required field (not an array), attempting to fix", tool.name);
                            sanitized.insert("required".to_string(), Value::Array(vec![]));
                        }
                    }

                    Value::Object(sanitized)
                }
                _ => {
                    tracing::warn!("Tool {} has invalid input_schema (not an object), using default", tool.name);
                    serde_json::json!({
                        "type": "object",
                        "properties": {},
                        "required": []
                    })
                }
            };

            tracing::debug!("Sanitized schema for tool {}: {}", tool.name, sanitized_schema);

            GeminiFunctionDeclaration {
                name: tool.name.clone(),
                description: tool.description.clone(),
                parameters: sanitized_schema,
            }
        }).collect()
    }
}

#[async_trait]
impl AgentBrain for GeminiBrain {
    async fn decide_next_action(
        &self,
        messages: &[Message],
        available_tools: &[ToolDefinition],
    ) -> Result<AgentAction, AgentError> {
        let mut contents = Vec::new();

        // Convert messages, filtering out system messages
        for message in messages {
            if message.role != Role::System {
                contents.push(Self::convert_message_to_gemini(message)?);
            }
        }

        // Prepare tools if available
        let tools = if !available_tools.is_empty() {
            Some(vec![GeminiTool {
                function_declarations: Self::convert_tool_definitions(available_tools),
            }])
        } else {
            None
        };

        // Prepare system instruction if available
        let system_instruction = self.system_prompt.as_ref().map(|prompt| GeminiContent {
            role: "user".to_string(),
            parts: vec![GeminiPart::Text(GeminiTextPart {
                text: prompt.clone(),
            })],
        });

        let request = GeminiRequest {
            contents,
            tools,
            system_instruction,
            generation_config: GeminiGenerationConfig {
                temperature: self.temperature,
                top_p: 0.95,
                top_k: 64,
                max_output_tokens: self.max_tokens,
                response_mime_type: "application/json".to_string(),
            },
        };

        let url = format!(
            "{}/{}:generateContent?key={}",
            GEMINI_API_BASE, self.model, self.api_key
        );

        let response = self
            .client
            .post(&url)
            .json(&request)
            .send()
            .await
            .map_err(|e| AgentError::LlmError(format!("HTTP request failed: {}", e)))?;

        if !response.status().is_success() {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(AgentError::LlmError(format!("API error: {}", error_text)));
        }

        let gemini_response: GeminiResponse = response
            .json()
            .await
            .map_err(|e| AgentError::LlmError(format!("Failed to parse response: {}", e)))?;

        let candidate = gemini_response
            .candidates
            .into_iter()
            .next()
            .ok_or_else(|| AgentError::LlmError("No candidates in response".to_string()))?;

        // Process the response parts
        let mut tool_calls = Vec::new();
        let mut text_content = String::new();

        for part in candidate.content.parts {
            match part {
                GeminiPart::Text(text_part) => {
                    text_content.push_str(&text_part.text);
                }
                GeminiPart::FunctionCall(function_call) => {
                    tool_calls.push(ToolCall {
                        id: format!("call_{}", uuid::Uuid::new_v4()),
                        name: function_call.function_call.name,
                        input: function_call.function_call.args,
                    });
                }
                GeminiPart::FunctionResponse(_) => {
                    // This shouldn't appear in a response from the model
                    continue;
                }
            }
        }

        // Determine action based on response
        if !tool_calls.is_empty() {
            Ok(AgentAction::ExecuteTool(tool_calls))
        } else if !text_content.trim().is_empty() {
            // Check if this should finish or continue thinking
            if text_content.to_lowercase().contains("final answer")
                || text_content.to_lowercase().contains("complete")
            {
                Ok(AgentAction::Finish(text_content))
            } else {
                Ok(AgentAction::RespondToUser(text_content))
            }
        } else {
            Err(AgentError::LlmError(
                "Empty response from model".to_string(),
            ))
        }
    }
}

impl Default for GeminiBrain {
    fn default() -> Self {
        // Default implementation should provide safe defaults
        // Using empty API key will fail when actually trying to use the provider
        Self {
            client: Client::new(),
            api_key: String::new(),
            model: "gemini-1.5-flash".to_string(),
            max_tokens: 8192,
            system_prompt: None,
            temperature: 0.1,
        }
    }
}
