use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::env;
use futures_util::StreamExt;
use tokio::io::AsyncBufReadExt;
use tokio_stream::wrappers::LinesStream;
use tokio_util::io::StreamReader;

use crate::agent::structs::{
    AgentAction, AgentError, Message, Role, ToolCall, ToolDefinition,
};
use crate::agent::traits::{AgentBrain, StreamingAgentBrain};

// --- Anthropic API Structs --- //

#[derive(Serialize, Debug)]
struct AnthropicRequest {
    model: String,
    messages: Vec<ApiMessage>,
    tools: Option<Vec<ApiTool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
    max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    stream: Option<bool>, // Add streaming support
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

// Streaming event structures for parsing SSE events
#[derive(Deserialize, Debug)]
struct StreamEvent {
    #[serde(rename = "type")]
    event_type: String,
}

#[derive(Deserialize, Debug)]
struct MessageStartEvent {
    #[serde(rename = "type")]
    event_type: String, // "message_start"
    message: StreamMessage,
}

#[derive(Deserialize, Debug)]
struct StreamMessage {
    id: String,
    #[serde(rename = "type")]
    message_type: String,
    role: String,
    content: Vec<ApiContentBlock>,
    model: String,
    stop_reason: Option<String>,
    stop_sequence: Option<String>,
}

#[derive(Deserialize, Debug)]
struct ContentBlockStartEvent {
    #[serde(rename = "type")]
    event_type: String, // "content_block_start"
    index: u32,
    content_block: ApiContentBlock,
}

#[derive(Deserialize, Debug)]
struct ContentBlockDeltaEvent {
    #[serde(rename = "type")]
    event_type: String, // "content_block_delta"
    index: u32,
    delta: ContentDelta,
}

#[derive(Deserialize, Debug)]
struct ContentDelta {
    #[serde(rename = "type")]
    delta_type: String, // "text_delta", "input_json_delta", etc.
    #[serde(skip_serializing_if = "Option::is_none")]
    text: Option<String>, // For text_delta
    #[serde(skip_serializing_if = "Option::is_none")]
    partial_json: Option<String>, // For input_json_delta
}

#[derive(Deserialize, Debug)]
struct ContentBlockStopEvent {
    #[serde(rename = "type")]
    event_type: String, // "content_block_stop"
    index: u32,
}

#[derive(Deserialize, Debug)]
struct MessageDeltaEvent {
    #[serde(rename = "type")]
    event_type: String, // "message_delta"
    delta: MessageDelta,
    usage: Option<serde_json::Value>, // Usage info
}

#[derive(Deserialize, Debug)]
struct MessageDelta {
    stop_reason: Option<String>,
    stop_sequence: Option<String>,
}

#[derive(Deserialize, Debug)]
struct MessageStopEvent {
    #[serde(rename = "type")]
    event_type: String, // "message_stop"
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
    streaming_enabled: bool, // New field for streaming support
}

impl AnthropicBrain {
    pub fn new(
        api_key: String,
        model: Option<String>,
        max_tokens: Option<u32>,
        system_prompt: Option<String>,
    ) -> Result<Self, AgentError> {
        Ok(Self {
            client: Client::new(),
            api_key,
            model: model.unwrap_or_else(|| DEFAULT_MODEL.to_string()),
            max_tokens: max_tokens.unwrap_or(DEFAULT_MAX_TOKENS),
            system_prompt,
            streaming_enabled: true, // Enable streaming by default
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

    /// Enable or disable streaming mode
    pub fn set_streaming(&mut self, enabled: bool) {
        self.streaming_enabled = enabled;
    }

    /// Generate dual content: keep original for typing, create concise version for speaking
    fn generate_dual_content(original_text: &str) -> (String, String) {
        let typed_content = original_text.to_string();

        // If the text is already short and concise, use it as-is
        if original_text.len() <= 100 {
            return (typed_content.clone(), typed_content);
        }

        // Extract key information for speech
        let spoken_content = Self::create_concise_speech_version(original_text);

        (typed_content, spoken_content)
    }

    /// Create a concise version optimized for speech synthesis
    fn create_concise_speech_version(text: &str) -> String {
        // Remove markdown formatting for speech
        let text = text.replace("**", "").replace("*", "").replace("#", "");

        // Split into sentences and prioritize key information
        let sentences: Vec<&str> = text.split(['.', '!', '?'])
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect();

        if sentences.is_empty() {
            return text.clone();
        }

        // For longer responses, summarize key points
        if sentences.len() > 3 {
            // Take first sentence (usually the main result) and key action sentences
            let mut key_sentences = vec![sentences[0]];

            // Add sentences that contain action words or results
            for sentence in sentences.iter().skip(1).take(2) {
                let lower = sentence.to_lowercase();
                if lower.contains("completed") || lower.contains("found") ||
                   lower.contains("created") || lower.contains("updated") ||
                   lower.contains("success") || lower.contains("done") ||
                   lower.contains("result") || lower.contains("finished") {
                    key_sentences.push(sentence);
                }
            }

            // If we only have the first sentence, add one more for context
            if key_sentences.len() == 1 && sentences.len() > 1 {
                key_sentences.push(sentences[1]);
            }

            key_sentences.join(". ") + if !key_sentences.last().unwrap_or(&"").ends_with('.') { "." } else { "" }
        } else {
            // For shorter responses, keep as-is but clean up
            sentences.join(". ") + if !text.ends_with(['.', '!', '?']) { "." } else { "" }
        }
    }

    /// Handle streaming response from Anthropic API
    async fn handle_streaming_response<F>(
        &self,
        response: reqwest::Response,
        mut on_text_chunk: F,
    ) -> Result<(String, Vec<ToolCall>, String), AgentError>
    where
        F: FnMut(String) + Send,
    {
        let mut accumulated_text = String::new();
        let mut tool_calls = Vec::new();
        let mut stop_reason = String::new();

        // Track content blocks and partial data
        let mut current_tool_call: Option<(String, String, String)> = None; // (id, name, partial_json)

        // Get the response body as a stream
        let stream = response.bytes_stream();
        let reader = StreamReader::new(stream.map(|result| {
            result.map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
        }));

        let lines_stream = LinesStream::new(tokio::io::BufReader::new(reader).lines());
        tokio::pin!(lines_stream);

        while let Some(line_result) = lines_stream.next().await {
            let line = line_result.map_err(|e| {
                AgentError::LlmError(format!("Failed to read stream line: {}", e))
            })?;

            // Skip empty lines
            if line.trim().is_empty() {
                continue;
            }

            // Parse SSE format: "event: <type>" and "data: <json>"
            if line.starts_with("event:") {
                // Skip event type lines for now, we'll parse from data
                continue;
            }

            if line.starts_with("data:") {
                let data_part = line.strip_prefix("data:").unwrap_or("").trim();

                // Skip ping events
                if data_part.is_empty() {
                    continue;
                }

                // Parse the JSON data
                let event_data: serde_json::Value = match serde_json::from_str(data_part) {
                    Ok(data) => data,
                    Err(e) => {
                        log::warn!("Failed to parse SSE data as JSON: {}, data: {}", e, data_part);
                        continue;
                    }
                };

                // Handle different event types
                if let Some(event_type) = event_data.get("type").and_then(|t| t.as_str()) {
                    match event_type {
                        "message_start" => {
                            log::debug!("Stream: message started");
                        }
                        "content_block_start" => {
                            if let Some(content_block) = event_data.get("content_block") {
                                if let Some(block_type) = content_block.get("type").and_then(|t| t.as_str()) {
                                    if block_type == "tool_use" {
                                        // Start tracking a new tool call
                                                                    let id = content_block.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                            let name = content_block.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                            log::debug!("Stream: started tool call {} ({})", name, id);
                            current_tool_call = Some((id, name, String::new()));
                                    }
                                }
                            }
                        }
                        "content_block_delta" => {
                            if let Some(delta) = event_data.get("delta") {
                                if let Some(delta_type) = delta.get("type").and_then(|t| t.as_str()) {
                                    match delta_type {
                                        "text_delta" => {
                                            if let Some(text) = delta.get("text").and_then(|t| t.as_str()) {
                                                // Accumulate text and emit chunk
                                                accumulated_text.push_str(text);
                                                on_text_chunk(text.to_string());
                                            }
                                        }
                                        "input_json_delta" => {
                                            if let Some(partial_json) = delta.get("partial_json").and_then(|t| t.as_str()) {
                                                // Accumulate JSON for tool call
                                                if let Some((_, _, ref mut json_accumulator)) = current_tool_call {
                                                    json_accumulator.push_str(partial_json);
                                                }
                                            }
                                        }
                                        _ => {
                                            log::debug!("Stream: unhandled delta type: {}", delta_type);
                                        }
                                    }
                                }
                            }
                        }
                        "content_block_stop" => {
                            // Complete current tool call if we have one
                            if let Some((id, name, json_str)) = current_tool_call.take() {
                                // Check if we have any JSON content before parsing
                                if json_str.trim().is_empty() {
                                    log::warn!("Tool call {} ({}) has empty JSON input, using empty object", name, id);
                                    // Use empty object as fallback
                                    tool_calls.push(ToolCall {
                                        id,
                                        name,
                                        input: serde_json::json!({})
                                    });
                                } else {
                                    // Parse the complete JSON
                                    match serde_json::from_str(&json_str) {
                                        Ok(input) => {
                                            tool_calls.push(ToolCall { id, name, input });
                                            log::debug!("Stream: completed tool call with input: {}", json_str);
                                        }
                                        Err(e) => {
                                            log::warn!("Failed to parse tool call input JSON: {}, json: '{}'. Using empty object as fallback.", e, json_str);
                                            // Use empty object as fallback instead of failing
                                            tool_calls.push(ToolCall {
                                                id,
                                                name,
                                                input: serde_json::json!({})
                                            });
                                        }
                                    }
                                }
                            }
                        }
                        "message_delta" => {
                            if let Some(delta) = event_data.get("delta") {
                                if let Some(reason) = delta.get("stop_reason").and_then(|r| r.as_str()) {
                                    stop_reason = reason.to_string();
                                }
                            }
                        }
                        "message_stop" => {
                            log::debug!("Stream: message completed");
                            break;
                        }
                        "ping" => {
                            // Ignore ping events
                        }
                        _ => {
                            log::debug!("Stream: unhandled event type: {}", event_type);
                        }
                    }
                }
            }
        }

        Ok((accumulated_text, tool_calls, stop_reason))
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
        // Delegate to streaming version without streaming parameters
        self.decide_next_action_streaming(messages, available_tools, None, None).await
    }

    fn supports_streaming(&self) -> bool {
        true // AnthropicBrain supports streaming
    }

    async fn decide_next_action_streaming(
        &self,
        messages: &[Message],
        available_tools: &[ToolDefinition],
        app_handle: Option<tauri::AppHandle>,
        message_id: Option<String>,
    ) -> Result<AgentAction, AgentError> {
        // --- 1. Prepare API Request ---
        let mut api_messages = Vec::new();

        // Track tool calls that need results to validate message ordering
        let mut pending_tool_calls: Vec<String> = Vec::new();

        for message in messages {
            match message.role {
                Role::Assistant => {
                    // Convert assistant message normally
                    match Self::convert_message_to_api(message) {
                        Ok(api_msg) => {
                            api_messages.push(api_msg);

                            // Track tool calls from this assistant message
                            if let Some(tool_calls) = &message.tool_calls {
                                for tool_call in tool_calls {
                                    pending_tool_calls.push(tool_call.id.clone());
                                }
                            }
                        }
                        Err(e) => log::warn!("Skipping assistant message conversion due to error: {}", e),
                    }
                }
                Role::Tool => {
                    // Handle tool result messages with proper formatting and ordering validation
                    let tool_call_id = message.tool_call_id.as_ref().ok_or_else(||
                        AgentError::LlmError("Tool result message missing tool_call_id".to_string())
                    )?.clone();

                    // Check if this tool call ID is expected
                    if !pending_tool_calls.contains(&tool_call_id) {
                        log::warn!("Received tool result for unexpected tool call ID: {}. This may cause API ordering issues.", tool_call_id);
                    } else {
                        // Remove from pending list
                        pending_tool_calls.retain(|id| id != &tool_call_id);
                    }

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

                                simplified.unwrap_or_else(|| "Tool executed successfully".to_string())
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
                }
                Role::User => {
                    // Convert user message normally
                    match Self::convert_message_to_api(message) {
                        Ok(api_msg) => api_messages.push(api_msg),
                        Err(e) => log::warn!("Skipping user message conversion due to error: {}", e),
                    }
                }
                Role::System => {
                    // Skip system messages - they should be handled via the system parameter
                    log::debug!("Skipping system message in conversion (should be handled via system parameter)");
                }
            }
        }

        // Validate that all tool calls have corresponding results
        if !pending_tool_calls.is_empty() {
            log::error!("Found tool calls without corresponding results: {:?}. This will cause API errors.", pending_tool_calls);
            return Err(AgentError::LlmError(format!(
                "Tool calls without results detected: {:?}. Each tool_use must have a corresponding tool_result.",
                pending_tool_calls
            )));
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

        let mut request_payload = AnthropicRequest {
            model: self.model.clone(),
            messages: api_messages,
            tools: api_tools,
            system: self.system_prompt.clone(),
            max_tokens: self.max_tokens,
            stream: None, // Will be set based on streaming mode
        };

        // Enable streaming if configured and we have an app handle
        let use_streaming = self.streaming_enabled && app_handle.is_some();
        if use_streaming {
            request_payload.stream = Some(true);
        }

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

        // --- 4. Handle Response (Streaming or Non-Streaming) ---
        if use_streaming {
            // Handle streaming response
            let app_handle = app_handle.ok_or("AppHandle required for streaming")?;
            let message_id = message_id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

            // Emit stream start event
            crate::agent::tool_logger::emit_stream_start(&app_handle, message_id.clone());

            let (accumulated_text, tool_calls, stop_reason) = self.handle_streaming_response(
                response,
                |chunk| {
                    // Emit text chunk event
                    crate::agent::tool_logger::emit_streaming_text_chunk(
                        &app_handle,
                        chunk,
                        Some(message_id.clone()),
                    );
                },
            ).await?;

            // Emit stream end event
            crate::agent::tool_logger::emit_stream_end(&app_handle, message_id, accumulated_text.clone());

            // Process stop reason and return appropriate action
            match stop_reason.as_str() {
                "tool_use" => {
                    if tool_calls.is_empty() {
                        Err(AgentError::LlmError("Stop reason is tool_use, but no valid tool calls found in response".to_string()))
                    } else {
                        if !accumulated_text.is_empty() {
                            log::info!("Anthropic response included text before tool use: {}", accumulated_text);
                        }
                        Ok(AgentAction::ExecuteTool(tool_calls))
                    }
                }
                "end_turn" | "stop_sequence" | "max_tokens" => {
                    if !tool_calls.is_empty() {
                        log::warn!("Stop reason is {}, but tool calls were also found. Ignoring tool calls.", stop_reason);
                    }

                    // Generate dual content for better TTS experience
                    let (typed_content, spoken_content) = Self::generate_dual_content(&accumulated_text);

                    if typed_content != spoken_content {
                        log::info!("Generated dual content - typed: {} chars, spoken: {} chars", typed_content.len(), spoken_content.len());
                        Ok(AgentAction::FinishWithDualContent { typed_content, spoken_content })
                    } else {
                        Ok(AgentAction::Finish(accumulated_text))
                    }
                }
                other => Err(AgentError::LlmError(format!(
                    "Received unexpected stop reason: {}",
                    other
                ))),
            }
        } else {
            // Handle non-streaming response (original logic)
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
                        }
                        Ok(AgentAction::ExecuteTool(tool_calls_to_execute))
                    }
                }
                "end_turn" | "stop_sequence" | "max_tokens" => {
                    if !tool_calls_to_execute.is_empty() {
                        log::warn!("Stop reason is {}, but tool calls were also found. Ignoring tool calls.", response_body.stop_reason);
                    }

                    // Generate dual content for better TTS experience
                    let (typed_content, spoken_content) = Self::generate_dual_content(&response_text);

                    if typed_content != spoken_content {
                        log::info!("Generated dual content - typed: {} chars, spoken: {} chars", typed_content.len(), spoken_content.len());
                        Ok(AgentAction::FinishWithDualContent { typed_content, spoken_content })
                    } else {
                        Ok(AgentAction::Finish(response_text))
                    }
                }
                other => Err(AgentError::LlmError(format!(
                    "Received unexpected stop reason: {}",
                    other
                ))),
            }
        }
    }
}

#[async_trait]
impl StreamingAgentBrain for AnthropicBrain {
    fn is_streaming_enabled(&self) -> bool {
        self.streaming_enabled
    }

    fn set_streaming_enabled(&mut self, enabled: bool) {
        self.streaming_enabled = enabled;
    }
}
