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
    /// Token counts. Previously dropped as "unused"; the cost log reads it
    /// (LAC-4127). Defaulted, so a response without it still parses.
    #[serde(rename = "usageMetadata", default)]
    usage_metadata: GeminiUsageMetadata,
}

#[derive(Deserialize, Debug)]
struct GeminiCandidate {
    content: GeminiContent,
    /// Why generation stopped (`STOP`, `MAX_TOKENS`, `SAFETY`). Recorded as
    /// the turn's outcome in the cost log; nothing else reads it.
    #[serde(rename = "finishReason", default)]
    finish_reason: Option<String>,
    // safety_ratings field removed - unused for performance
}

/// Gemini's token counts.
///
/// `promptTokenCount` **includes** `cachedContentTokenCount`, as on OpenAI and
/// unlike Anthropic, so the cached part is subtracted out rather than billed
/// twice. Gemini bills cache *storage* by the hour rather than per written
/// token, so there is no cache-write count to record.
///
/// **`candidatesTokenCount` does NOT include `thoughtsTokenCount`**, and
/// thinking tokens are billed at the output rate: "response pricing is the sum
/// of output tokens and thinking tokens". Reading candidates alone under-costs
/// every thinking turn by the whole reasoning volume — silently, because the
/// analyzer recomputes from these same tokens and would agree. Every field
/// defaults: a model that is not thinking omits the field entirely, and some
/// preview models omit it even when they are.
#[derive(Deserialize, Debug, Default)]
struct GeminiUsageMetadata {
    #[serde(rename = "promptTokenCount", default)]
    prompt_token_count: u64,
    #[serde(rename = "candidatesTokenCount", default)]
    candidates_token_count: u64,
    #[serde(rename = "cachedContentTokenCount", default)]
    cached_content_token_count: u64,
    /// Reasoning tokens, billed at the output rate and reported apart from
    /// `candidatesTokenCount`.
    #[serde(rename = "thoughtsTokenCount", default)]
    thoughts_token_count: u64,
    /// Content a **server-side** tool pulled in on demand (search grounding,
    /// video frames, a transcript), billed as input. Juno sends only
    /// client-side function declarations, which ride in the ordinary prompt,
    /// so this should be zero on every request Juno makes — see the warning
    /// in the conversion below for what it means if it ever is not.
    #[serde(rename = "toolUsePromptTokenCount", default)]
    tool_use_prompt_token_count: u64,
}

impl From<&GeminiUsageMetadata> for crate::cost_log::TokenUsage {
    fn from(usage: &GeminiUsageMetadata) -> Self {
        // Google documents this as a bucket of its own, beside
        // `promptTokenCount` rather than inside it, but publishes no formula
        // tying the fields to the total. So it is added rather than ignored —
        // under-counting a real billed category is the failure this module
        // exists to prevent — and it says so out loud the first time it ever
        // happens, because the alternative reading would make this a double
        // count. It should never fire on Juno's own requests.
        if usage.tool_use_prompt_token_count > 0 {
            tracing::warn!(
                target: crate::cost_log::TARGET,
                "[CostLog] Gemini reported {} tool-use prompt tokens; counted as input. \
                 Juno sends no server-side tools, so verify this is not already \
                 inside promptTokenCount before trusting the figure.",
                usage.tool_use_prompt_token_count
            );
        }
        crate::cost_log::TokenUsage {
            input: usage
                .prompt_token_count
                .saturating_sub(usage.cached_content_token_count)
                .saturating_add(usage.tool_use_prompt_token_count),
            output: usage
                .candidates_token_count
                .saturating_add(usage.thoughts_token_count),
            cache_read: usage.cached_content_token_count,
            cache_write: 0,
        }
    }
}

const GEMINI_API_BASE: &str = crate::constants::api::endpoints::GEMINI_API_BASE;

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
        use crate::agent::providers::types::Provider;

        // Use centralized defaults from provider configuration
        let model = model.unwrap_or_else(|| Provider::Gemini.default_model().to_string());
        let max_tokens =
            max_tokens.unwrap_or(crate::constants::agent::config::DEFAULT_MAX_TOKENS_COMPACT);
        let temperature = temperature.unwrap_or(0.1); // Low temperature for consistent routing decisions

        Ok(GeminiBrain {
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(120))
                .build()
                .map_err(|e| {
                    AgentError::ConfigurationError(format!("Failed to create HTTP client: {}", e))
                })?,
            api_key,
            model,
            max_tokens,
            system_prompt,
            temperature,
        })
    }

    /// Creates a new GeminiBrain from a CentralizedProviderConfig struct.
    /// Falls back to the GEMINI_API_KEY env var if the config has no api_key.
    /// Note: GeminiBrain uses i32 for max_tokens, CentralizedProviderConfig uses u32.
    pub fn from_config(config: &crate::settings::ProviderConfig) -> Result<Self, AgentError> {
        let api_key = config
            .api_key
            .clone()
            .or_else(|| env::var("GEMINI_API_KEY").ok())
            .ok_or_else(|| {
                AgentError::ConfigurationError(
                    "Gemini API key not found in settings or GEMINI_API_KEY env var".into(),
                )
            })?;
        let max_tokens = config.max_tokens.map(|t| t as i32);
        Self::new(
            api_key,
            config.model.clone(),
            max_tokens,
            config.system_prompt.clone(),
            config.temperature,
        )
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

        let url = format!("{}/{}:generateContent", GEMINI_API_BASE, self.model);

        // The API key goes in the x-goog-api-key header, never in the URL:
        // query strings end up in HTTP client logs, proxy logs, and error
        // messages (audit 2026-02-08 item #21).
        // Times the request for the cost log's `duration_ms`.
        let request_started = std::time::Instant::now();
        let response = self
            .client
            .post(&url)
            .header("x-goog-api-key", &self.api_key)
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

        crate::cost_log::record_turn(
            &crate::agent::providers::types::Provider::Gemini,
            &self.model,
            (&gemini_response.usage_metadata).into(),
            request_started.elapsed().as_millis() as u64,
            gemini_response
                .candidates
                .first()
                .and_then(|c| c.finish_reason.as_deref())
                .unwrap_or("unknown"),
        );

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
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(120))
                .build()
                .unwrap_or_else(|e| {
                    // `Default` cannot fail, so the fallback is an untimed
                    // client — which is exactly the hang this timeout exists
                    // to prevent. Say so loudly rather than degrade in
                    // silence; only a TLS backend failure gets here.
                    tracing::error!(
                        "Gemini HTTP client could not be built with a timeout ({}); \
                         falling back to a client with NO request timeout",
                        e
                    );
                    Client::new()
                }),
            api_key: String::new(),
            model: crate::agent::providers::types::Provider::Gemini
                .default_model()
                .to_string(),
            max_tokens: 8192,
            system_prompt: None,
            temperature: 0.1,
        }
    }
}

#[cfg(test)]
mod usage_metadata_tests {
    use super::*;
    use crate::cost_log::TokenUsage;

    fn usage(json: serde_json::Value) -> TokenUsage {
        let parsed: GeminiUsageMetadata =
            serde_json::from_value(json).expect("the fixture matches the struct");
        (&parsed).into()
    }

    /// The regression this module's doc comment is about. Gemini reports
    /// reasoning in `thoughtsTokenCount`, outside `candidatesTokenCount` and
    /// billed at the output rate. Reading candidates alone under-costs the
    /// turn by the entire thinking volume, and nothing downstream notices:
    /// the analyzer recomputes the cost from these same understated tokens
    /// and agrees with itself.
    #[test]
    fn thinking_tokens_are_output_tokens() {
        let usage = usage(serde_json::json!({
            "promptTokenCount": 120,
            "candidatesTokenCount": 40,
            "thoughtsTokenCount": 860,
            "totalTokenCount": 1020,
        }));

        assert_eq!(
            usage.output, 900,
            "40 answer tokens + 860 thinking tokens, not 40"
        );
        assert_eq!(usage.input, 120);
        assert_eq!(usage.cache_read, 0);
    }

    /// A thinking model that answers from thought alone still bills for it.
    #[test]
    fn a_turn_that_is_all_thinking_still_costs_output() {
        let usage = usage(serde_json::json!({
            "promptTokenCount": 10,
            "thoughtsTokenCount": 500,
        }));

        assert_eq!(usage.output, 500);
    }

    /// Not every model reports the field — a non-thinking model omits it, and
    /// some preview models omit it even while thinking. Absent must read as
    /// zero rather than failing the parse and costing the turn its whole row.
    #[test]
    fn an_absent_thoughts_field_reads_as_zero() {
        let usage = usage(serde_json::json!({
            "promptTokenCount": 100,
            "candidatesTokenCount": 25,
        }));

        assert_eq!(usage.output, 25);
        assert_eq!(usage.input, 100);
    }

    /// `promptTokenCount` is inclusive of the cached part, so the cached
    /// tokens are moved to `cache_read` rather than billed twice at the full
    /// input rate. Thinking tokens are unaffected by any of that.
    #[test]
    fn cached_tokens_come_out_of_input_and_leave_thinking_alone() {
        let usage = usage(serde_json::json!({
            "promptTokenCount": 5000,
            "cachedContentTokenCount": 4800,
            "candidatesTokenCount": 30,
            "thoughtsTokenCount": 200,
        }));

        assert_eq!(usage.input, 200, "5000 total prompt less 4800 cached");
        assert_eq!(usage.cache_read, 4800);
        assert_eq!(usage.output, 230);
        assert_eq!(usage.cache_write, 0, "Gemini bills cache storage by time");
    }

    /// Content a server-side tool pulled in is input-side and billed, so it is
    /// added rather than dropped. Juno sends no server-side tools, so this is
    /// a guard against a silent under-count rather than a path in use.
    #[test]
    fn tool_use_prompt_tokens_are_counted_as_input() {
        let usage = usage(serde_json::json!({
            "promptTokenCount": 300,
            "toolUsePromptTokenCount": 1200,
            "candidatesTokenCount": 15,
        }));

        assert_eq!(usage.input, 1500);
        assert_eq!(usage.output, 15);
    }

    /// Cache accounting must not underflow into a vast number when a provider
    /// reports a cached count larger than the prompt it came from.
    #[test]
    fn a_cached_count_larger_than_the_prompt_saturates_at_zero() {
        let usage = usage(serde_json::json!({
            "promptTokenCount": 10,
            "cachedContentTokenCount": 99,
        }));

        assert_eq!(usage.input, 0);
    }
}
