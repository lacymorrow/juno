use async_trait::async_trait;
use futures_util::StreamExt;
use regex::Regex;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::env;
use tokio::io::AsyncBufReadExt;
use tokio_stream::wrappers::LinesStream;
use tokio_util::io::StreamReader;


use crate::agent::core::{AgentAction, AgentError, Message, Role, ToolCall, ToolDefinition};
use crate::agent::traits::{AgentBrain, StreamingAgentBrain};

// --- Anthropic API Structs --- //

#[derive(Serialize, Debug, Clone)]
#[serde(untagged)]
pub enum ToolChoice {
    #[serde(rename = "auto")]
    Auto,
    #[serde(rename = "any")]
    Any,
    #[serde(rename = "none")]
    None,
    Tool {
        #[serde(rename = "type")]
        choice_type: String, // "tool"
        name: String,
    },
}

impl Default for ToolChoice {
    fn default() -> Self {
        ToolChoice::Auto
    }
}

#[derive(Serialize, Debug)]
struct AnthropicRequest {
    model: String,
    messages: Vec<ApiMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<ApiTool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
    max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    stream: Option<bool>, // Add streaming support
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_choice: Option<ToolChoice>, // Add tool choice support
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

#[derive(Clone)]
pub struct AnthropicBrain {
    client: Client,
    api_key: String,
    model: String,
    max_tokens: u32,
    system_prompt: Option<String>, // Optional system prompt
    streaming_enabled: bool,       // New field for streaming support
    default_tool_choice: Option<ToolChoice>, // Default tool choice behavior
}

impl AnthropicBrain {
    /// Creates a new AnthropicBrain with the provided API key and optional configuration.
    pub fn new(
        api_key: String,
        model: Option<String>,
        max_tokens: Option<u32>,
        system_prompt: Option<String>,
    ) -> Result<Self, AgentError> {
        use crate::agent::providers::factory::Provider;

        // Use centralized defaults from provider configuration
        let model = model.unwrap_or_else(|| Provider::Anthropic.default_model().to_string());
        let max_tokens = max_tokens.unwrap_or(crate::constants::agent::config::DEFAULT_MAX_TOKENS_STANDARD);

        // Create HTTP client with proper timeout configuration to prevent hanging
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(crate::constants::timeouts::HTTP_REQUEST_TIMEOUT_SECONDS))
            .connect_timeout(std::time::Duration::from_secs(crate::constants::timeouts::HTTP_CONNECT_TIMEOUT_SECONDS))
            .build()
            .map_err(|e| AgentError::LlmError(format!("Failed to create HTTP client: {}", e)))?;

        Ok(AnthropicBrain {
            client,
            api_key,
            model,
            max_tokens,
            system_prompt,
            streaming_enabled: true, // Default to streaming for real-time user experience
            default_tool_choice: None, // Default tool choice behavior
        })
    }

    /// Creates a new AnthropicBrain using the API key from the environment variables.
    pub fn from_env() -> Result<Self, AgentError> {
        let api_key = env::var("ANTHROPIC_API_KEY").map_err(|_| {
            AgentError::ConfigurationError(
                "ANTHROPIC_API_KEY environment variable not set".to_string(),
            )
        })?;

        let model = env::var("ANTHROPIC_MODEL").ok();
        let max_tokens = env::var("ANTHROPIC_MAX_TOKENS")
            .ok()
            .and_then(|s| s.parse::<u32>().ok());

        Self::new(api_key, model, max_tokens, None)
    }

    /// Enable or disable streaming for this brain
    pub fn set_streaming(&mut self, enabled: bool) {
        self.streaming_enabled = enabled;
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
    fn sanitize_request_for_logging(request: &AnthropicRequest) -> serde_json::Value {
        match serde_json::to_value(request) {
            Ok(value) => Self::sanitize_for_logging(&value),
            Err(_) => serde_json::Value::String("[SERIALIZATION_ERROR]".to_string()),
        }
    }

    /// Sanitize API response structures for logging
    fn sanitize_response_for_logging(response: &AnthropicMessageResponse) -> serde_json::Value {
        match serde_json::to_value(response) {
            Ok(value) => Self::sanitize_for_logging(&value),
            Err(_) => serde_json::Value::String("[SERIALIZATION_ERROR]".to_string()),
        }
    }

    /// Handle streaming response from Anthropic API with XML-based TTS extraction
    async fn handle_streaming_response<F>(
        &self,
        response: reqwest::Response,
        mut on_text_chunk: F,
    ) -> Result<(String, Vec<ToolCall>, String), AgentError>
    where
        F: FnMut(String, Vec<String>) + Send, // Updated to accept multiple TTS extractions
    {
        let mut accumulated_text = String::new();
        let mut tool_calls = Vec::new();
        let mut stop_reason = String::new();

        // TTS XML parsing state
        let mut tts_buffer = String::new();
        let mut in_tts_tag = false;
        let mut tts_content = String::new();

        // Track content blocks and partial data
        let mut current_tool_call: Option<(String, String, String)> = None; // (id, name, partial_json)

        // Get the response body as a stream
        let stream = response.bytes_stream();
        let reader =
            StreamReader::new(stream.map(|result| {
                result.map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
            }));

        let lines_stream = LinesStream::new(tokio::io::BufReader::new(reader).lines());
        tokio::pin!(lines_stream);

        while let Some(line_result) = lines_stream.next().await {
            let line = line_result
                .map_err(|e| AgentError::LlmError(format!("Failed to read stream line: {}", e)))?;

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
                        log::warn!(
                            "Failed to parse SSE data as JSON: {}, data: {}",
                            e,
                            data_part
                        );
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
                                if let Some(block_type) =
                                    content_block.get("type").and_then(|t| t.as_str())
                                {
                                    if block_type == "tool_use" {
                                        // Start tracking a new tool call
                                        let id = content_block
                                            .get("id")
                                            .and_then(|v| v.as_str())
                                            .unwrap_or("")
                                            .to_string();
                                        let name = content_block
                                            .get("name")
                                            .and_then(|v| v.as_str())
                                            .unwrap_or("")
                                            .to_string();
                                        log::debug!("Stream: started tool call {} ({})", name, id);
                                        current_tool_call = Some((id, name, String::new()));
                                    }
                                }
                            }
                        }
                        "content_block_delta" => {
                            if let Some(delta) = event_data.get("delta") {
                                if let Some(delta_type) = delta.get("type").and_then(|t| t.as_str())
                                {
                                    match delta_type {
                                        "text_delta" => {
                                            if let Some(text) =
                                                delta.get("text").and_then(|t| t.as_str())
                                            {
                                                // Process text for TTS XML tags
                                                let (display_text, extracted_tts_list) = self
                                                    .process_text_with_tts_extraction(
                                                        text,
                                                        &mut tts_buffer,
                                                        &mut in_tts_tag,
                                                        &mut tts_content,
                                                    );

                                                // Accumulate only display text (without TTS tags) for final response
                                                accumulated_text.push_str(&display_text);

                                                // Emit chunk with separated TTS content
                                                on_text_chunk(display_text, extracted_tts_list);
                                            }
                                        }
                                        "input_json_delta" => {
                                            if let Some(partial_json) =
                                                delta.get("partial_json").and_then(|t| t.as_str())
                                            {
                                                // Accumulate JSON for tool call
                                                if let Some((_, _, ref mut json_accumulator)) =
                                                    current_tool_call
                                                {
                                                    json_accumulator.push_str(partial_json);
                                                }
                                            }
                                        }
                                        _ => {
                                            log::debug!(
                                                "Stream: unhandled delta type: {}",
                                                delta_type
                                            );
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
                                        input: serde_json::json!({}),
                                    });
                                } else {
                                    // Parse the complete JSON
                                    match serde_json::from_str(&json_str) {
                                        Ok(input) => {
                                            tool_calls.push(ToolCall { id, name, input });
                                            log::debug!(
                                                "Stream: completed tool call with input: {}",
                                                json_str
                                            );
                                        }
                                        Err(e) => {
                                            log::warn!("Failed to parse tool call input JSON: {}, json: '{}'. Using empty object as fallback.", e, json_str);
                                            // Use empty object as fallback instead of failing
                                            tool_calls.push(ToolCall {
                                                id,
                                                name,
                                                input: serde_json::json!({}),
                                            });
                                        }
                                    }
                                }
                            }
                        }
                        "message_delta" => {
                            if let Some(delta) = event_data.get("delta") {
                                if let Some(reason) =
                                    delta.get("stop_reason").and_then(|r| r.as_str())
                                {
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

        // CRITICAL FIX: Handle any remaining TTS state at end of stream
        if in_tts_tag || !tts_buffer.is_empty() {
            log::warn!(
                "Stream ended with incomplete TTS state: in_tts_tag={}, buffer='{}'",
                in_tts_tag,
                tts_buffer
            );

            // If we're in the middle of a TTS tag, extract what we have as TTS content
            if in_tts_tag && !tts_content.trim().is_empty() {
                log::info!(
                    "Extracting incomplete TTS content at stream end: '{}'",
                    tts_content
                );
                on_text_chunk(String::new(), vec![tts_content.clone()]);
            }

            // If there's remaining buffer content outside TTS tags, add it to accumulated text
            if !in_tts_tag && !tts_buffer.trim().is_empty() {
                log::debug!(
                    "Adding remaining buffer content to accumulated text: '{}'",
                    tts_buffer
                );
                accumulated_text.push_str(&tts_buffer);
            }

            // Clear the buffer to prevent leakage
            tts_buffer.clear();
        }

        Ok((accumulated_text, tool_calls, stop_reason))
    }

    /// Process text chunk to extract TTS XML tags and return display text + extracted TTS content
    ///
    /// This function handles:
    /// - Proper buffer management to avoid character duplication/loss
    /// - Partial XML tags split across streaming chunks
    /// - Multiple TTS blocks within a single chunk
    /// - Complete tag removal to prevent leakage
    fn process_text_with_tts_extraction(
        &self,
        text_chunk: &str,
        tts_buffer: &mut String,
        in_tts_tag: &mut bool,
        tts_content: &mut String,
    ) -> (String, Vec<String>) {
        let mut display_text = String::new();
        let mut extracted_tts_list = Vec::new();

        // Add new text to buffer for processing
        tts_buffer.push_str(text_chunk);

        let mut chars_to_consume = 0;
        let buffer_chars: Vec<char> = tts_buffer.chars().collect();
        let mut i = 0;

        while i < buffer_chars.len() {
            // Check for potential tag boundaries - need at least 5 chars for "<TTS>" or 6 for "</TTS>"
            let remaining_len = buffer_chars.len() - i;
            let remaining_str: String = buffer_chars[i..].iter().collect();

            if !*in_tts_tag {
                // Outside TTS tag - look for opening tag
                if remaining_str.starts_with("<TTS>") {
                    // Found complete opening tag
                    *in_tts_tag = true;
                    i += 5; // Skip "<TTS>"
                    chars_to_consume = i;
                    continue;
                } else if remaining_len < 5 && self.could_be_partial_opening_tag(&remaining_str) {
                    // Potential partial opening tag at end of buffer - stop processing here
                    // CRITICAL FIX: Ensure we mark chars_to_consume correctly to not lose display text
                    break;
                } else {
                    // Regular character outside TTS - add to display
                    display_text.push(buffer_chars[i]);
                    i += 1;
                    chars_to_consume = i;
                }
            } else {
                // Inside TTS tag - look for closing tag
                if remaining_str.starts_with("</TTS>") {
                    // Found complete closing tag
                    if !tts_content.trim().is_empty() {
                        extracted_tts_list.push(tts_content.clone());
                        log::info!("Extracted TTS content: '{}'", tts_content);
                    }

                    // Reset TTS state for next potential block
                    *in_tts_tag = false;
                    tts_content.clear();
                    i += 6; // Skip "</TTS>"
                    chars_to_consume = i;
                    continue;
                } else if remaining_len < 6 && self.could_be_partial_closing_tag(&remaining_str) {
                    // Potential partial closing tag at end of buffer - stop processing here
                    // CRITICAL FIX: Do not consume characters when we have a partial closing tag
                    break;
                } else {
                    // Content inside TTS tag - add to TTS content only (not to display)
                    tts_content.push(buffer_chars[i]);
                    i += 1;
                    chars_to_consume = i;
                }
            }
        }

        // CRITICAL FIX: Enhanced buffer management
        // Remove processed characters from buffer, keeping any unprocessed remainder
        if chars_to_consume > 0 && chars_to_consume <= buffer_chars.len() {
            *tts_buffer = buffer_chars[chars_to_consume..].iter().collect();
        }

        // CRITICAL FIX: Validate that no TTS tags remain in display_text
        // This should never happen if processing is correct
        if display_text.contains("<TTS>") || display_text.contains("</TTS>") {
            log::error!(
                "CRITICAL BUG: TTS tags found in display_text during streaming processing!"
            );
            log::error!("Display text: '{}'", display_text);
            log::error!(
                "Buffer state - in_tts_tag: {}, tts_content: '{}'",
                *in_tts_tag,
                tts_content
            );
            log::error!("This indicates a bug in the streaming TTS processing logic");

            // Emergency cleanup to prevent tag leakage
            display_text = display_text.replace("<TTS>", "").replace("</TTS>", "");
        }

        (display_text, extracted_tts_list)
    }

    /// Check if a string could be the beginning of a partial "<TTS>" tag
    fn could_be_partial_opening_tag(&self, s: &str) -> bool {
        if s.is_empty() {
            return false;
        }
        let partial_tags = ["<", "<T", "<TT", "<TTS"];
        partial_tags.iter().any(|&tag| s == tag)
    }

    /// Check if a string could be the beginning of a partial "</TTS>" tag
    fn could_be_partial_closing_tag(&self, s: &str) -> bool {
        if s.is_empty() {
            return false;
        }
        let partial_tags = ["<", "</", "</T", "</TT", "</TTS"];
        partial_tags.iter().any(|&tag| s == tag)
    }

    /// Strip TTS XML tags from text, removing them completely (content was already processed for TTS)
    fn strip_tts_tags(&self, text: &str) -> String {
        // CRITICAL FIX: Remove TTS tags completely - content was already processed for immediate TTS
        // We don't want TTS content appearing in the final display text
        let tts_regex = Regex::new(r"<TTS>.*?</TTS>").unwrap();
        tts_regex
            .replace_all(text, "")
            .to_string()
            .trim()
            .to_string()
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
                return Err(AgentError::LlmError(
                    "Tool calls are only expected in assistant messages.".to_string(),
                ));
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
        self.decide_next_action_streaming(messages, available_tools, None, None)
            .await
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
        let mut resolved_tool_calls: std::collections::HashSet<String> = std::collections::HashSet::new();

        // First pass: collect all tool call IDs and tool result IDs
        for message in messages {
            match message.role {
                Role::Assistant => {
                    if let Some(tool_calls) = &message.tool_calls {
                        for tool_call in tool_calls {
                            pending_tool_calls.push(tool_call.id.clone());
                        }
                    }
                }
                Role::Tool => {
                    if let Some(tool_call_id) = &message.tool_call_id {
                        resolved_tool_calls.insert(tool_call_id.clone());
                    }
                }
                _ => {}
            }
        }

        // Validate conversation consistency - remove orphaned tool results
        let mut valid_messages = Vec::new();
        let mut orphaned_results_found = false;

        for message in messages {
            match message.role {
                Role::Tool => {
                    if let Some(tool_call_id) = &message.tool_call_id {
                        // Check if this tool result has a corresponding tool call
                        if !pending_tool_calls.contains(tool_call_id) {
                            log::warn!("Removing orphaned tool result with ID: {} - no corresponding tool_use found", tool_call_id);
                            orphaned_results_found = true;
                            continue; // Skip this message
                        }
                    }
                }
                _ => {}
            }
            valid_messages.push(message.clone());
        }

        if orphaned_results_found {
            log::info!("Cleaned up orphaned tool results from conversation before sending to Anthropic API");
        }

        // Reset tracking for the cleaned messages
        pending_tool_calls.clear();

        for message in &valid_messages {
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
                        Err(e) => {
                            log::warn!("Skipping assistant message conversion due to error: {}", e)
                        }
                    }
                }
                Role::Tool => {
                    // Handle tool result messages with proper formatting and ordering validation
                    let tool_call_id = message
                        .tool_call_id
                        .as_ref()
                        .ok_or_else(|| {
                            AgentError::LlmError(
                                "Tool result message missing tool_call_id".to_string(),
                            )
                        })?
                        .clone();

                    // Check if this tool call ID is expected
                    if !pending_tool_calls.contains(&tool_call_id) {
                        log::error!("CRITICAL: Tool result for ID {} has no corresponding tool_use - this should have been filtered out", tool_call_id);
                        return Err(AgentError::LlmError(format!(
                            "Conversation consistency error: tool result {} has no corresponding tool_use block",
                            tool_call_id
                        )));
                    } else {
                        // Remove from pending list
                        pending_tool_calls.retain(|id| id != &tool_call_id);
                    }

                    let tool_result_content = message.content.clone();

                    // Parse the tool result content to extract just the text that needs to be passed
                    let formatted_content =
                        match serde_json::from_str::<serde_json::Value>(&tool_result_content) {
                            Ok(json_value) => {
                                // Extract stdout for command results
                                if let Some(stdout) =
                                    json_value.get("stdout").and_then(|v| v.as_str())
                                {
                                    stdout.trim().to_string()
                                }
                                // Extract content for file reads
                                else if let Some(content) =
                                    json_value.get("content").and_then(|v| v.as_str())
                                {
                                    content.trim().to_string()
                                }
                                // For error messages
                                else if let Some(error) =
                                    json_value.get("error").and_then(|v| v.as_str())
                                {
                                    format!("Error: {}", error.trim())
                                }
                                // If we can't extract a specific field, return a simplified string
                                else {
                                    // Anthropic requires simple string content for tool_result
                                    // We'll use a fallback to the first string value we can find
                                    let simplified = json_value.as_object().and_then(|obj| {
                                        obj.values()
                                            .find_map(|v| v.as_str().map(|s| s.trim().to_string()))
                                    });

                                    simplified
                                        .unwrap_or_else(|| "Tool executed successfully".to_string())
                                }
                            }
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
                            text: None,                      // Remove text field
                            id: None,                        // Not used for tool_result
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
                        Err(e) => {
                            log::warn!("Skipping user message conversion due to error: {}", e)
                        }
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
            log::error!(
                "Found tool calls without corresponding results: {:?}. This will cause API errors.",
                pending_tool_calls
            );
            return Err(AgentError::LlmError(format!(
                "Tool calls without results detected: {:?}. Each tool_use must have a corresponding tool_result.",
                pending_tool_calls
            )));
        }

        log::info!("Conversation validation passed: {} messages prepared for Anthropic API", api_messages.len());

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
            tool_choice: None, // Add tool choice support
        };

        // Enable streaming if configured and we have an app handle
        let use_streaming = self.streaming_enabled && app_handle.is_some();
        if use_streaming {
            request_payload.stream = Some(true);
        }

        // -- DEBUG: Log the request payload --
        match serde_json::to_string_pretty(&Self::sanitize_request_for_logging(&request_payload)) {
            Ok(json_string) => log::debug!("Anthropic Request Payload:\n{}", json_string),
            Err(e) => log::error!("Failed to serialize request payload for logging: {}", e),
        }
        // -- END DEBUG --

        // --- 2. Make API Call ---
        log::debug!(
            "Sending request to Anthropic: {:?}",
            Self::sanitize_request_for_logging(&request_payload)
        );

        let response = self
            .client
            .post(ANTHROPIC_API_URL)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01") // Current stable API version
            .header("anthropic-beta", "computer-use-2025-01-24") // Computer Use beta for Claude 4 and 3.7 models
            .header("content-type", "application/json")
            .json(&request_payload)
            .send()
            .await
            .map_err(|e| AgentError::LlmError(format!("HTTP request failed: {}", e)))?;

        // --- 3. Parse API Response ---
        if !response.status().is_success() {
            let status = response.status();
            let error_body = response
                .text()
                .await
                .unwrap_or_else(|_| "Failed to read error body".to_string());
            log::error!(
                "Anthropic API Error: Status {}, Body: {}",
                status,
                error_body
            );
            return Err(AgentError::LlmError(format!(
                "Anthropic API returned error {}: {}",
                status, error_body
            )));
        }

        // --- 4. Handle Response (Streaming or Non-Streaming) ---
        if use_streaming {
            // Handle streaming response
            let app_handle = app_handle.ok_or("AppHandle required for streaming")?;
            let message_id = message_id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

            // Emit stream start event
            crate::agent::tool_logger::emit_stream_start(&app_handle, message_id.clone());

            let (accumulated_text, tool_calls, stop_reason) = self
                .handle_streaming_response(response, |chunk, tts_list| {
                    // Emit text chunk event - pass first TTS item if available for backward compatibility
                    crate::agent::tool_logger::emit_streaming_text_chunk(
                        &app_handle,
                        chunk,
                        Some(message_id.clone()),
                        tts_list.first().cloned(),
                    );

                    // Emit additional TTS items if there are multiple in this chunk
                    for (i, tts_content) in tts_list.iter().enumerate().skip(1) {
                        crate::agent::tool_logger::emit_streaming_text_chunk(
                            &app_handle,
                            String::new(), // Empty display text for additional TTS-only chunks
                            Some(message_id.clone()),
                            Some(tts_content.clone()),
                        );
                    }
                })
                .await?;

            // TTS tags should now be completely removed during streaming processing
            // If any remain, it indicates a bug in our improved processing logic
            if accumulated_text.contains("<TTS>") || accumulated_text.contains("</TTS>") {
                log::error!("CRITICAL BUG: TTS tags found in final accumulated text after improved processing!");
                log::error!("This should never happen with the fixed streaming logic");
                log::error!("Accumulated text: '{}'", accumulated_text);

                // Emergency fallback - but this indicates a serious bug
                let final_clean_text = self.strip_tts_tags(&accumulated_text);
                log::error!(
                    "Emergency cleanup applied: '{}' -> '{}'",
                    accumulated_text,
                    final_clean_text
                );

                return Err(AgentError::LlmError(
                    "TTS tag processing failed - this is a critical bug in the streaming logic"
                        .to_string(),
                ));
            }

            let final_display_text = accumulated_text;

            // Emit stream end event with final display text
            crate::agent::tool_logger::emit_stream_end(
                &app_handle,
                message_id,
                final_display_text.clone(),
            );

            // Process stop reason and return appropriate action
            match stop_reason.as_str() {
                "tool_use" => {
                    if tool_calls.is_empty() {
                        Err(AgentError::LlmError(
                            "Stop reason is tool_use, but no valid tool calls found in response"
                                .to_string(),
                        ))
                    } else {
                        if !final_display_text.is_empty() {
                            log::info!(
                                "Anthropic response included text before tool use: {}",
                                final_display_text
                            );
                        }
                        Ok(AgentAction::ExecuteTool(tool_calls))
                    }
                }
                "end_turn" | "stop_sequence" | "max_tokens" => {
                    if !tool_calls.is_empty() {
                        log::warn!("Stop reason is {}, but tool calls were also found. Ignoring tool calls.", stop_reason);
                    }

                    // CRITICAL FIX: Return final display text (either clean content or TTS fallback)
                    // TTS content was already extracted and processed during streaming
                    Ok(AgentAction::Finish(final_display_text))
                }
                other => Err(AgentError::LlmError(format!(
                    "Received unexpected stop reason: {}",
                    other
                ))),
            }
        } else {
            // Handle non-streaming response (original logic)
            let response_body: AnthropicMessageResponse = response.json().await.map_err(|e| {
                AgentError::LlmError(format!("Failed to parse API response: {}", e))
            })?;

            log::debug!(
                "Received response from Anthropic: {:?}",
                Self::sanitize_response_for_logging(&response_body)
            );

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
                        let id = block.id.clone().ok_or_else(|| {
                            AgentError::LlmError("Tool call missing 'id' field".to_string())
                        })?;
                        let name = block.name.clone().ok_or_else(|| {
                            AgentError::LlmError(format!("Tool call {} missing 'name' field", id))
                        })?;
                        let input = block.input.clone().ok_or_else(|| {
                            AgentError::LlmError(format!("Tool call {} missing 'input' field", id))
                        })?;

                        // Add to the list of tool calls to execute
                        tool_calls_to_execute.push(ToolCall { id, name, input });
                    }
                    _ => {
                        log::warn!("Unknown content block type: {}", block.block_type);
                    }
                }
            }

            match response_body.stop_reason.as_str() {
                "tool_use" => {
                    if tool_calls_to_execute.is_empty() {
                        Err(AgentError::LlmError(
                            "Stop reason is tool_use, but no valid tool calls found in response"
                                .to_string(),
                        ))
                    } else {
                        if !response_text.is_empty() {
                            log::info!(
                                "Anthropic response included text before tool use: {}",
                                response_text
                            );
                        }
                        Ok(AgentAction::ExecuteTool(tool_calls_to_execute))
                    }
                }
                "end_turn" | "stop_sequence" | "max_tokens" => {
                    if !tool_calls_to_execute.is_empty() {
                        log::warn!("Stop reason is {}, but tool calls were also found. Ignoring tool calls.", response_body.stop_reason);
                    }

                    // Non-streaming mode: return the response text as-is
                    // TTS XML processing only works in streaming mode
                    Ok(AgentAction::Finish(response_text))
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
