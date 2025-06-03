use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::env;

use crate::agent::structs::{
    AgentAction, AgentError, Message, Role, ToolCall, ToolDefinition,
};
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
    #[serde(skip_serializing_if = "Option::is_none")]
    usage_metadata: Option<GeminiUsageMetadata>,
}

#[derive(Deserialize, Debug)]
struct GeminiCandidate {
    content: GeminiContent,
    finish_reason: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    safety_ratings: Option<Vec<GeminiSafetyRating>>,
}

#[derive(Deserialize, Debug)]
struct GeminiUsageMetadata {
    prompt_tokens: i32,
    candidates_tokens: i32,
    total_tokens: i32,
}

#[derive(Deserialize, Debug)]
struct GeminiSafetyRating {
    category: String,
    probability: String,
}

const GEMINI_API_BASE: &str = "https://generativelanguage.googleapis.com/v1beta/models";
const DEFAULT_MODEL: &str = "gemini-1.5-flash"; // Smaller, faster model for orchestration
const DEFAULT_MAX_TOKENS: i32 = 1024; // Smaller token limit for orchestrator

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
        Ok(GeminiBrain {
            client: Client::new(),
            api_key,
            model: model.unwrap_or_else(|| DEFAULT_MODEL.to_string()),
            max_tokens: max_tokens.unwrap_or(DEFAULT_MAX_TOKENS),
            system_prompt,
            temperature: temperature.unwrap_or(0.1), // Low temperature for consistent routing decisions
        })
    }

    pub fn from_env() -> Result<Self, AgentError> {
        let api_key = env::var("GOOGLE_GEMINI_API_KEY")
            .map_err(|_| AgentError::ConfigurationError(
                "GOOGLE_GEMINI_API_KEY environment variable not set".to_string()
            ))?;

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
            Role::System => return Err(AgentError::LlmError(
                "System messages should be passed via system_instruction parameter".to_string()
            )),
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
            let tool_name = message.name.as_ref()
                .ok_or_else(|| AgentError::LlmError(
                    "Tool result message missing tool name".to_string()
                ))?;

            let response_content = if message.content.starts_with('{') || message.content.starts_with('[') {
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
        tools.iter().map(|tool| {
            GeminiFunctionDeclaration {
                name: tool.name.clone(),
                description: tool.description.clone(),
                parameters: tool.input_schema.clone(),
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
        let system_instruction = self.system_prompt.as_ref().map(|prompt| {
            GeminiContent {
                role: "user".to_string(),
                parts: vec![GeminiPart::Text(GeminiTextPart {
                    text: prompt.clone(),
                })],
            }
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

        let url = format!("{}/{}:generateContent?key={}",
                         GEMINI_API_BASE, self.model, self.api_key);

        let response = self.client
            .post(&url)
            .json(&request)
            .send()
            .await
            .map_err(|e| AgentError::LlmError(format!("HTTP request failed: {}", e)))?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(AgentError::LlmError(format!("API error: {}", error_text)));
        }

        let gemini_response: GeminiResponse = response.json().await
            .map_err(|e| AgentError::LlmError(format!("Failed to parse response: {}", e)))?;

        let candidate = gemini_response.candidates.into_iter().next()
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
            if text_content.to_lowercase().contains("final answer") ||
               text_content.to_lowercase().contains("complete") {
                Ok(AgentAction::Finish(text_content))
            } else {
                Ok(AgentAction::RespondToUser(text_content))
            }
        } else {
            Err(AgentError::LlmError("Empty response from model".to_string()))
        }
    }
}

impl Default for GeminiBrain {
    fn default() -> Self {
        Self::new(
            String::new(),
            None,
            None,
            None,
            None,
        ).unwrap()
    }
}
