use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::env;

use crate::agent::structs::{
    AgentAction, AgentError, Message, Role, ToolCall, ToolDefinition,
};
use crate::agent::traits::AgentBrain;

// --- Anthropic API Structs --- //

#[derive(Serialize, Debug)]
struct AnthropicRequest {
    model: String,
    messages: Vec<ApiMessage>,
    tools: Option<Vec<ApiTool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
    max_tokens: u32,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct ApiMessage {
    role: String,
    content: ApiContent,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(untagged)]
enum ApiContent {
    Text(String),
    Blocks(Vec<ApiContentBlock>),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct ApiContentBlock {
    #[serde(rename = "type")]
    block_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    input: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_use_id: Option<String>, // For tool result blocks
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<String>, // For tool_result content
}

#[derive(Serialize, Deserialize, Debug)]
struct AnthropicMessageResponse {
    id: String,
    #[serde(rename = "type")]
    response_type: String,
    role: String, // Should be "assistant"
    content: Vec<ApiContentBlock>,
    model: String,
    stop_reason: String, // e.g., "end_turn", "tool_use", "max_tokens"
    stop_sequence: Option<String>,
    // usage: ApiUsageInfo,
}

#[derive(Serialize, Debug)]
struct ApiTool {
    name: String,
    description: String,
    input_schema: Value,
}

// --- AnthropicBrain Implementation --- //

const ANTHROPIC_API_URL: &str = "https://api.anthropic.com/v1/messages";
const DEFAULT_MODEL: &str = "claude-3-7-sonnet-20250219";
const DEFAULT_MAX_TOKENS: u32 = 4096;

#[derive(Clone)]
pub struct AnthropicBrain {
    client: Client,
    api_key: String,
    model: String,
    max_tokens: u32,
    system_prompt: Option<String>, // Optional system prompt
}

impl AnthropicBrain {
    pub fn new(
        api_key: String,
        model: Option<String>,
        max_tokens: Option<u32>,
        system_prompt: Option<String>,
    ) -> Result<Self, AgentError> {
        Ok(AnthropicBrain {
            client: Client::new(),
            api_key,
            model: model.unwrap_or_else(|| DEFAULT_MODEL.to_string()),
            max_tokens: max_tokens.unwrap_or(DEFAULT_MAX_TOKENS),
            system_prompt,
        })
    }

    /// Creates a new AnthropicBrain using the API key from the environment variable.
    pub fn from_env() -> Result<Self, AgentError> {
        let api_key = env::var("ANTHROPIC_API_KEY")
            .map_err(|_| AgentError::ConfigurationError("ANTHROPIC_API_KEY environment variable not set".to_string()))?;
        // Allow overriding model/max_tokens via env vars too, if desired
        let model = env::var("ANTHROPIC_MODEL").ok();
        let max_tokens = env::var("ANTHROPIC_MAX_TOKENS").ok().and_then(|s| s.parse::<u32>().ok());
        let system_prompt = env::var("ANTHROPIC_SYSTEM_PROMPT").ok();

        Self::new(api_key, model, max_tokens, system_prompt)
    }

    // Helper function to convert our internal Message format to Anthropic's API format
    fn convert_message_to_api(message: &Message) -> Result<ApiMessage, AgentError> {
        let role_str = match message.role {
            Role::User => "user".to_string(),
            Role::Assistant => "assistant".to_string(),
            Role::System => return Err(AgentError::LlmError("System messages should be passed via the 'system' parameter, not in the messages list.".to_string())),
            Role::Tool => return Err(AgentError::LlmError("Tool result messages need special handling for Anthropic API format.".to_string())), // Needs specific handling
        };

        let mut content_blocks = Vec::new();

        // Add text content if present
        if !message.content.is_empty() {
            content_blocks.push(ApiContentBlock {
                block_type: "text".to_string(),
                text: Some(message.content.clone()),
                id: None,
                name: None,
                input: None,
                tool_use_id: None,
                content: None,
            });
        }

        // Add tool calls if present (for assistant messages)
        if let Some(tool_calls) = &message.tool_calls {
            if message.role != Role::Assistant {
                return Err(AgentError::LlmError("Tool calls are only expected in assistant messages.".to_string()));
            }
            for tool_call in tool_calls {
                content_blocks.push(ApiContentBlock {
                    block_type: "tool_use".to_string(),
                    id: Some(tool_call.id.clone()),
                    name: Some(tool_call.name.clone()),
                    input: Some(tool_call.input.clone()),
                    text: None,
                    tool_use_id: None,
                    content: None,
                });
            }
        }

        Ok(ApiMessage {
            role: role_str,
            content: if content_blocks.is_empty() {
                ApiContent::Text("".to_string())
            } else {
                ApiContent::Blocks(content_blocks)
            },
        })
    }
}

#[async_trait]
impl AgentBrain for AnthropicBrain {
    async fn decide_next_action(
        &self,
        messages: &[Message],
        available_tools: &[ToolDefinition],
    ) -> Result<AgentAction, AgentError> {
        // --- 1. Prepare API Request ---
        let mut api_messages = Vec::new();

        for message in messages {
            // Special handling for Tool messages which need to be converted to Anthropic's tool_result format
            if message.role == Role::Tool {
                let tool_call_id = message.tool_call_id.as_ref().ok_or_else(||
                    AgentError::LlmError("Tool result message missing tool_call_id".to_string())
                )?.clone();
                let tool_result_content = message.content.clone();

                // Parse the tool result content to extract just the text that needs to be passed
                let formatted_content = match serde_json::from_str::<serde_json::Value>(&tool_result_content) {
                    Ok(json_value) => {
                        // Extract stdout for command results
                        if let Some(stdout) = json_value.get("stdout").and_then(|v| v.as_str()) {
                            stdout.trim().to_string()
                        }
                        // Extract content for file reads
                        else if let Some(content) = json_value.get("content").and_then(|v| v.as_str()) {
                            content.trim().to_string()
                        }
                        // For error messages
                        else if let Some(error) = json_value.get("error").and_then(|v| v.as_str()) {
                            format!("Error: {}", error.trim())
                        }
                        // If we can't extract a specific field, return a simplified string
                        else {
                            // Anthropic requires simple string content for tool_result
                            // We'll use a fallback to the first string value we can find
                            let simplified = json_value.as_object().and_then(|obj| {
                                obj.values().find_map(|v| v.as_str().map(|s| s.trim().to_string()))
                            });

                            simplified.unwrap_or_else(|| "Command executed successfully".to_string())
                        }
                    },
                    Err(_) => {
                        // If content is not JSON, use it directly (trimmed)
                        tool_result_content.trim().to_string()
                    }
                };

                api_messages.push(ApiMessage {
                    role: "user".to_string(), // Tool results have role "user"
                    content: ApiContent::Blocks(vec![ApiContentBlock {
                        block_type: "tool_result".to_string(),
                        tool_use_id: Some(tool_call_id), // Use tool_use_id instead of id
                        text: None, // Remove text field
                        id: None, // Not used for tool_result
                        name: None,
                        input: None,
                        content: Some(formatted_content), // Add content field
                    }]),
                });
            } else if message.role != Role::System { // Skip system messages here
                match Self::convert_message_to_api(message) {
                    Ok(api_msg) => api_messages.push(api_msg),
                    Err(e) => log::warn!("Skipping message conversion due to error: {}", e),
                }
            }
        }

        let api_tools = if available_tools.is_empty() {
            None
        } else {
            Some(
                available_tools
                    .iter()
                    .map(|t| ApiTool {
                        name: t.name.clone(),
                        description: t.description.clone(),
                        input_schema: t.input_schema.clone(),
                    })
                    .collect(),
            )
        };

        let request_payload = AnthropicRequest {
            model: self.model.clone(),
            messages: api_messages,
            tools: api_tools,
            system: self.system_prompt.clone(),
            max_tokens: self.max_tokens,
        };

        // -- DEBUG: Log the request payload --
        match serde_json::to_string_pretty(&request_payload) {
            Ok(json_string) => log::debug!("Anthropic Request Payload:\n{}", json_string),
            Err(e) => log::error!("Failed to serialize request payload for logging: {}", e),
        }
        // -- END DEBUG --

        // --- 2. Make API Call ---
        log::debug!("Sending request to Anthropic: {:?}", request_payload);

        let response = self
            .client
            .post(ANTHROPIC_API_URL)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01") // Required header
            .header("content-type", "application/json")
            .json(&request_payload)
            .send()
            .await
            .map_err(|e| AgentError::LlmError(format!("HTTP request failed: {}", e)))?;

        // --- 3. Parse API Response ---
        if !response.status().is_success() {
            let status = response.status();
            let error_body = response.text().await.unwrap_or_else(|_| "Failed to read error body".to_string());
            log::error!("Anthropic API Error: Status {}, Body: {}", status, error_body);
            return Err(AgentError::LlmError(format!(
                "Anthropic API returned error {}: {}",
                status,
                error_body
            )));
        }

        let response_body: AnthropicMessageResponse = response
            .json()
            .await
            .map_err(|e| AgentError::LlmError(format!("Failed to parse API response: {}", e)))?;

        log::debug!("Received response from Anthropic: {:?}", response_body);

        // --- 4. Determine AgentAction ---

        let mut tool_calls_to_execute = Vec::new();
        let mut response_text = String::new();

        // Extract and parse tool calls and text from the response
        for block in response_body.content.iter() {
            match block.block_type.as_str() {
                "text" => {
                    if let Some(text) = &block.text {
                        // Append to response text
                        if !response_text.is_empty() {
                            response_text.push('\n');
                        }
                        response_text.push_str(text);
                    }
                }
                "tool_use" => {
                    // Check if we have the required fields for a tool call
                    let id = block.id.clone().ok_or_else(|| AgentError::LlmError("Tool call missing 'id' field".to_string()))?;
                    let name = block.name.clone().ok_or_else(|| AgentError::LlmError(format!("Tool call {} missing 'name' field", id)))?;
                    let input = block.input.clone().ok_or_else(|| AgentError::LlmError(format!("Tool call {} missing 'input' field", id)))?;

                    // Add to the list of tool calls to execute
                    tool_calls_to_execute.push(ToolCall {
                        id,
                        name,
                        input,
                    });
                }
                _ => {
                    log::warn!("Unknown content block type: {}", block.block_type);
                }
            }
        }

        match response_body.stop_reason.as_str() {
            "tool_use" => {
                if tool_calls_to_execute.is_empty() {
                     Err(AgentError::LlmError("Stop reason is tool_use, but no valid tool calls found in response".to_string()))
                } else {
                     if !response_text.is_empty() {
                         log::info!("Anthropic response included text before tool use: {}", response_text);
                         // TODO: Decide how to handle this text. Add to memory?
                     }
                     // Return all parsed tool calls
                     Ok(AgentAction::ExecuteTool(tool_calls_to_execute))
                }
            }
            "end_turn" | "stop_sequence" | "max_tokens" => {
                 if !tool_calls_to_execute.is_empty() {
                     log::warn!("Stop reason is {}, but tool calls were also found. Ignoring tool calls.", response_body.stop_reason);
                 }
                // If the turn ended naturally or via stop sequence, finish with the text response.
                // If max_tokens, it also ends, but the response might be incomplete.
                 Ok(AgentAction::Finish(response_text))
            }
            other => Err(AgentError::LlmError(format!(
                "Received unexpected stop reason: {}",
                other
            ))),
        }
    }
}
