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

#[cfg(debug_assertions)]
use chrono;


use crate::agent::core::{AgentAction, AgentError, Message, Role, ToolCall, ToolDefinition};
use crate::agent::providers::types::Provider;
use crate::agent::traits::{AgentBrain, StreamingAgentBrain};

// --- Anthropic API Structs --- //

#[derive(Serialize, Debug, Clone, Default)]
#[serde(untagged)]
pub enum ToolChoice {
    #[serde(rename = "auto")]
    #[default]
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

#[derive(Serialize, Debug)]
struct AnthropicRequest {
    model: String,
    messages: Vec<ApiMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<ApiTool>>,
    /// System prompt — sent as content blocks array to support prompt caching.
    /// When cache_control is present, Anthropic caches the prefix server-side,
    /// reducing input token costs by ~90% and latency by ~50-80% on subsequent turns.
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<Vec<SystemContentBlock>>,
    max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    stream: Option<bool>, // Add streaming support
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_choice: Option<ToolChoice>, // Add tool choice support
}

/// System content block with optional cache_control for Anthropic prompt caching.
#[derive(Serialize, Deserialize, Debug, Clone)]
struct SystemContentBlock {
    #[serde(rename = "type")]
    block_type: String,
    text: String,
    /// Cache control for prompt caching — {"type": "ephemeral"} tells Anthropic
    /// to cache this block for subsequent API calls within the same session.
    #[serde(skip_serializing_if = "Option::is_none")]
    cache_control: Option<CacheControl>,
}

/// Cache control directive for Anthropic prompt caching.
#[derive(Serialize, Deserialize, Debug, Clone)]
struct CacheControl {
    #[serde(rename = "type")]
    cache_type: String,
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

/// Content for tool_result blocks — either a plain string or structured blocks (text + image)
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(untagged)]
enum ApiToolResultContent {
    Text(String),
    Blocks(Vec<ApiToolResultBlock>),
}

/// A single block within a tool_result content array (text or image)
#[derive(Serialize, Deserialize, Debug, Clone)]
struct ApiToolResultBlock {
    #[serde(rename = "type")]
    block_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    source: Option<ApiImageSource>,
    #[serde(skip_serializing_if = "Option::is_none")]
    text: Option<String>,
}

/// Base64 image source for Anthropic API image content blocks
#[derive(Serialize, Deserialize, Debug, Clone)]
struct ApiImageSource {
    #[serde(rename = "type")]
    source_type: String,
    media_type: String,
    data: String,
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
    content: Option<ApiToolResultContent>, // For tool_result content (text or image blocks)
}

#[derive(Serialize, Deserialize, Debug)]
struct AnthropicMessageResponse {
    _id: String,
    #[serde(rename = "type")]
    _response_type: String,
    _role: String, // Should be "assistant"
    content: Vec<ApiContentBlock>,
    _model: String,
    stop_reason: String, // e.g., "end_turn", "tool_use", "max_tokens"
    _stop_sequence: Option<String>,
    // usage: ApiUsageInfo,
}

#[derive(Serialize, Debug)]
#[serde(untagged)]
enum ApiTool {
    /// Anthropic built-in tools (computer, bash, text_editor)
    /// Format: {"type": "computer_20251124", "name": "computer", "display_width_px": 1280, "display_height_px": 800, "enable_zoom": true}
    BuiltIn {
        #[serde(rename = "type")]
        tool_type: String,
        name: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        display_width_px: Option<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        display_height_px: Option<u32>,
        /// Enable zoom action for computer_20251124 — allows Claude to inspect
        /// specific screen regions at full native resolution (critical for Retina displays)
        #[serde(skip_serializing_if = "Option::is_none")]
        enable_zoom: Option<bool>,
        /// Cache control for the last tool in the list to enable prompt caching
        #[serde(skip_serializing_if = "Option::is_none")]
        cache_control: Option<CacheControl>,
    },
    /// Regular function-calling tools
    Custom {
        name: String,
        description: String,
        input_schema: Value,
        /// Cache control for the last tool in the list to enable prompt caching
        #[serde(skip_serializing_if = "Option::is_none")]
        cache_control: Option<CacheControl>,
    },
}

// Streaming event structures for parsing SSE events - removed unused structs
// StreamEvent, MessageStartEvent, etc. are not used for JSON deserialization in handle_streaming_response
// We use manual serde_json::Value parsing instead.

// --- AnthropicBrain Implementation --- //

const ANTHROPIC_API_URL: &str = "https://api.anthropic.com/v1/messages";

/// The Anthropic API hostname used for TLS session warmup.
/// Must match the base domain of ANTHROPIC_API_URL so the cached session ticket applies.
const ANTHROPIC_HOST: &str = "https://api.anthropic.com";

/// Warm up the TLS session to `api.anthropic.com` before the first real API call.
///
/// Sends a lightweight HEAD request to prime the OS-level TLS session ticket cache,
/// saving ~200-500ms on the first `POST /v1/messages` call (eliminates the full TLS
/// handshake on the first interaction). Inspired by Clicky's ClaudeAPI.swift:64-96.
///
/// This is best-effort — a failed warmup only means the first real call may be
/// slightly slower. It never blocks startup or returns an error.
pub async fn warmup_anthropic_tls() {
    let client = match reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(5))
        .timeout(std::time::Duration::from_secs(10))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            log::warn!("TLS warmup: failed to build client: {}", e);
            return;
        }
    };

    let start = std::time::Instant::now();
    match client.head(ANTHROPIC_HOST).send().await {
        Ok(resp) => {
            log::info!(
                "TLS warmup complete: {} {} in {}ms",
                resp.status(),
                ANTHROPIC_HOST,
                start.elapsed().as_millis()
            );
        }
        Err(e) => {
            log::warn!(
                "TLS warmup: HEAD {} failed in {}ms: {}",
                ANTHROPIC_HOST,
                start.elapsed().as_millis(),
                e
            );
        }
    }
}

/// Maximum number of recent screenshots to keep in conversation history.
/// Older screenshots are replaced with text placeholders to reduce token usage.
/// Following the pattern from Cua (only_n_most_recent_images=3).
/// Each 1024x768 screenshot costs ~1,049 tokens — limiting from 10 to 3 saves ~7,000 tokens/step.
const MAX_RECENT_SCREENSHOTS: usize = 3;

#[derive(Clone)]
pub struct AnthropicBrain {
    client: Client,
    api_key: String,
    model: String,
    max_tokens: u32,
    system_prompt: Option<String>, // Optional system prompt
    streaming_enabled: bool,       // New field for streaming support
    #[allow(dead_code)]
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
        use crate::agent::providers::types::Provider;

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

    /// Creates a new AnthropicBrain from a CentralizedProviderConfig struct.
    /// Falls back to the ANTHROPIC_API_KEY env var if the config has no api_key
    /// (e.g., when keys come from a .env file rather than the Tauri Store).
    pub fn from_config(config: &crate::settings::ProviderConfig) -> Result<Self, AgentError> {
        let api_key = config.api_key.clone()
            .or_else(|| env::var("ANTHROPIC_API_KEY").ok())
            .ok_or_else(|| AgentError::ConfigurationError(
                "Anthropic API key not found in settings or ANTHROPIC_API_KEY env var".into()
            ))?;
        Self::new(api_key, config.model.clone(), config.max_tokens, config.system_prompt.clone())
    }

    fn format_anthropic_http_error_for_user(
        status: reqwest::StatusCode,
        error_body: &str,
    ) -> String {
        let trimmed = error_body.trim();

        // Prefer extracting a clean, user-facing message from Anthropic's structured error JSON.
        // Example shape:
        // {"type":"error","error":{"type":"invalid_request_error","message":"..."},"request_id":"..."}
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed) {
            let error_type = value
                .pointer("/error/type")
                .and_then(|v| v.as_str())
                .map(str::to_string);
            let message = value.pointer("/error/message").and_then(|v| v.as_str());
            let request_id = value.get("request_id").and_then(|v| v.as_str());

            if let Some(message) = message {
                let mut formatted = match error_type {
                    Some(error_type) => {
                        format!("Anthropic API error {} ({}): {}", status, error_type, message)
                    }
                    None => format!("Anthropic API error {}: {}", status, message),
                };

                if let Some(request_id) = request_id {
                    formatted.push_str(&format!(" (request_id: {})", request_id));
                }

                return formatted;
            }
        }

        // Never surface raw JSON blobs to the user; keep details in logs instead.
        if trimmed.is_empty() || trimmed.starts_with('{') || trimmed.starts_with('[') {
            format!("Anthropic API returned error {}.", status)
        } else {
            format!("Anthropic API returned error {}: {}", status, trimmed)
        }
    }

    /// Enable or disable streaming for this brain
    pub fn set_streaming(&mut self, enabled: bool) {
        self.streaming_enabled = enabled;
    }

    /// Determine the correct computer-use beta header for the selected model
    fn resolve_computer_use_beta_header(&self) -> &'static str {
        Provider::Anthropic.computer_use_beta_flag(&self.model)
    }

    /// Resolve the correct tool API type for the selected model.
    /// Opus 4.5+ models require newer tool type identifiers (e.g. computer_20251124,
    /// text_editor_20250728) while older models use the registered defaults.
    fn resolve_tool_api_type(&self, tool_name: &str, registered_type: &str) -> String {
        Provider::Anthropic.resolve_tool_type(tool_name, registered_type, &self.model)
    }

    /// Limit screenshot history to keep only the N most recent screenshots.
    /// Older screenshots are replaced with a text placeholder to dramatically reduce token usage.
    ///
    /// This scans tool_result blocks for image content, counts them from the end (most recent),
    /// and replaces any beyond the limit with "[Screenshot removed — older than N most recent]".
    fn limit_screenshot_history(api_messages: &mut [ApiMessage], max_recent: usize) {
        // First pass: find all screenshots and their exact locations
        let mut screenshot_locations: Vec<(usize, usize, usize)> = Vec::new(); // (msg_idx, block_idx, result_block_idx)

        for (msg_idx, msg) in api_messages.iter().enumerate() {
            if let ApiContent::Blocks(blocks) = &msg.content {
                for (block_idx, block) in blocks.iter().enumerate() {
                    if block.block_type == "tool_result" {
                        if let Some(ApiToolResultContent::Blocks(result_blocks)) = &block.content {
                            for (rb_idx, rb) in result_blocks.iter().enumerate() {
                                if rb.block_type == "image" && rb.source.is_some() {
                                    screenshot_locations.push((msg_idx, block_idx, rb_idx));
                                }
                            }
                        }
                    }
                }
            }
        }

        let total_screenshots = screenshot_locations.len();
        if total_screenshots <= max_recent {
            return; // Nothing to trim
        }

        let to_remove_count = total_screenshots - max_recent;
        let locations_to_remove = &screenshot_locations[..to_remove_count];

        log::info!(
            "Screenshot history limiting: {} total screenshots, keeping {} most recent, removing {} older ones",
            total_screenshots, max_recent, to_remove_count
        );

        // Second pass: replace old screenshots with text placeholders
        // Use all three indices (msg_idx, block_idx, rb_idx) to target the exact image block
        for &(msg_idx, block_idx, rb_idx) in locations_to_remove {
            if let Some(msg) = api_messages.get_mut(msg_idx) {
                if let ApiContent::Blocks(blocks) = &mut msg.content {
                    if let Some(block) = blocks.get_mut(block_idx) {
                        if let Some(ApiToolResultContent::Blocks(result_blocks)) = &mut block.content {
                            if let Some(result_block) = result_blocks.get_mut(rb_idx) {
                                *result_block = ApiToolResultBlock {
                                    block_type: "text".to_string(),
                                    source: None,
                                    text: Some(format!(
                                        "[Screenshot removed — older than {} most recent]",
                                        max_recent
                                    )),
                                };
                            }
                        }
                    }
                }
            }
        }
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
    /// Returns: (accumulated_text, tool_calls, stop_reason, stream_was_started)
    async fn handle_streaming_response<F>(
        &self,
        response: reqwest::Response,
        app_handle: Option<&tauri::AppHandle>,
        message_id: Option<String>,
        mut on_text_chunk: F,
    ) -> Result<(String, Vec<ToolCall>, String, bool), AgentError>
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

        // Thinking XML parsing state (for <thinking> tags in text output)
        let mut thinking_buffer = String::new();
        let mut in_thinking_tag = false;
        let mut thinking_content = String::new();
        let mut thinking_message_id: Option<String> = None; // Track current thinking stream

        // Track whether we've started the main response stream
        // We delay stream_start until we have actual non-thinking text to display
        let mut response_stream_started = false;

        // Track content blocks and partial data
        let mut current_tool_call: Option<(String, String, String)> = None; // (id, name, partial_json)

        // Track thinking content blocks (for extended thinking models via API)
        let mut current_thinking_content: Option<String> = None;
        let mut api_thinking_message_id: Option<String> = None; // For extended thinking API

        // Get the response body as a stream
        let stream = response.bytes_stream();
        let reader =
            StreamReader::new(stream.map(|result| {
                result.map_err(std::io::Error::other)
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
                                    match block_type {
                                        "tool_use" => {
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
                                        "thinking" => {
                                            // Start tracking a thinking block (extended thinking models)
                                            log::debug!("Stream: started thinking block");
                                            current_thinking_content = Some(String::new());
                                        }
                                        _ => {
                                            log::debug!("Stream: started content block type: {}", block_type);
                                        }
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
                                                // Process thinking XML tags with streaming support
                                                let (text_without_thinking, thinking_started, thinking_ended, thinking_chunk) = self
                                                    .process_text_with_thinking_extraction_streaming(
                                                        text,
                                                        &mut thinking_buffer,
                                                        &mut in_thinking_tag,
                                                        &mut thinking_content,
                                                    );

                                                // Handle thinking streaming events
                                                if let Some(handle) = app_handle {
                                                    // Emit thinking_start if we just entered a thinking block
                                                    if thinking_started {
                                                        let new_thinking_id = uuid::Uuid::new_v4().to_string();
                                                        thinking_message_id = Some(new_thinking_id.clone());
                                                        crate::agent::tool_logger::emit_thinking_start(handle, new_thinking_id);
                                                    }

                                                    // Emit thinking_chunk if we have thinking content
                                                    if !thinking_chunk.is_empty() {
                                                        crate::agent::tool_logger::emit_thinking_chunk(
                                                            handle,
                                                            thinking_chunk,
                                                            thinking_message_id.clone(),
                                                        );
                                                    }

                                                    // Emit thinking_end if we just exited a thinking block
                                                    if thinking_ended {
                                                        if let Some(ref msg_id) = thinking_message_id {
                                                            crate::agent::tool_logger::emit_thinking_end(
                                                                handle,
                                                                msg_id.clone(),
                                                                thinking_content.clone(),
                                                            );
                                                        }
                                                        // Clear thinking content for next potential block
                                                        thinking_content.clear();
                                                        thinking_message_id = None;
                                                    }
                                                }

                                                // Then process TTS XML tags from the remaining text
                                                let (display_text, extracted_tts_list) = self
                                                    .process_text_with_tts_extraction(
                                                        &text_without_thinking,
                                                        &mut tts_buffer,
                                                        &mut in_tts_tag,
                                                        &mut tts_content,
                                                    );

                                                // Emit chunks when we have display text OR TTS content.
                                                // We delay stream_start until after thinking messages for proper ordering,
                                                // but TTS-only responses still need streaming events so the frontend
                                                // can show the TTS content in the conversation.
                                                if !display_text.is_empty() || !extracted_tts_list.is_empty() {
                                                    // Emit stream_start on first chunk (text or TTS-only)
                                                    if !response_stream_started {
                                                        if let (Some(handle), Some(ref msg_id)) = (app_handle, &message_id) {
                                                            crate::agent::tool_logger::emit_stream_start(handle, msg_id.clone());
                                                            response_stream_started = true;
                                                        }
                                                    }

                                                    // Accumulate only display text (without TTS or thinking tags) for final response
                                                    accumulated_text.push_str(&display_text);

                                                    // Emit chunk with separated TTS content
                                                    on_text_chunk(display_text, extracted_tts_list);
                                                }
                                            }
                                        }
                                        "thinking_delta" => {
                                            // Stream thinking content from extended thinking API
                                            if let Some(thinking_text) =
                                                delta.get("thinking").and_then(|t| t.as_str())
                                            {
                                                if let Some(handle) = app_handle {
                                                    // Emit thinking_start if this is the first chunk
                                                    if api_thinking_message_id.is_none() {
                                                        let new_thinking_id = uuid::Uuid::new_v4().to_string();
                                                        api_thinking_message_id = Some(new_thinking_id.clone());
                                                        crate::agent::tool_logger::emit_thinking_start(handle, new_thinking_id);
                                                    }

                                                    // Stream thinking chunk
                                                    crate::agent::tool_logger::emit_thinking_chunk(
                                                        handle,
                                                        thinking_text.to_string(),
                                                        api_thinking_message_id.clone(),
                                                    );
                                                }

                                                // Also accumulate for final thinking_end
                                                if let Some(ref mut thinking_accumulator) =
                                                    current_thinking_content
                                                {
                                                    thinking_accumulator.push_str(thinking_text);
                                                }
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
                                    log::debug!("Tool call {} ({}) has empty JSON input, using empty object", name, id);
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

                            // Emit thinking_end for API thinking blocks
                            if let Some(thinking_text) = current_thinking_content.take() {
                                if !thinking_text.trim().is_empty() {
                                    log::debug!("Stream: completed API thinking block with {} chars", thinking_text.len());
                                    if let Some(handle) = app_handle {
                                        if let Some(ref msg_id) = api_thinking_message_id {
                                            crate::agent::tool_logger::emit_thinking_end(
                                                handle,
                                                msg_id.clone(),
                                                thinking_text,
                                            );
                                        }
                                    }
                                }
                                api_thinking_message_id = None;
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

        // Handle any remaining thinking state at end of stream
        if in_thinking_tag || !thinking_buffer.is_empty() {
            log::debug!(
                "Stream ended with remaining thinking state: in_thinking_tag={}, buffer='{}'",
                in_thinking_tag,
                thinking_buffer
            );

            // If we're in the middle of a thinking tag, emit thinking_end with what we have
            if in_thinking_tag && !thinking_content.trim().is_empty() {
                log::debug!("Emitting incomplete thinking content at stream end: {} chars", thinking_content.len());
                if let Some(handle) = app_handle {
                    if let Some(ref msg_id) = thinking_message_id {
                        crate::agent::tool_logger::emit_thinking_end(
                            handle,
                            msg_id.clone(),
                            thinking_content.clone(),
                        );
                    }
                }
            }

            // If there's remaining buffer content outside thinking tags, it needs to be processed
            if !in_thinking_tag && !thinking_buffer.trim().is_empty() {
                log::debug!("Adding remaining thinking buffer content to accumulated text: '{}'", thinking_buffer);
                accumulated_text.push_str(&thinking_buffer);
            }

            thinking_buffer.clear();
        }

        // CRITICAL FIX: Handle any remaining TTS state at end of stream
        if in_tts_tag || !tts_buffer.is_empty() {
            log::debug!(
                "Stream ended with remaining TTS state: in_tts_tag={}, buffer='{}'",
                in_tts_tag,
                tts_buffer
            );

            // If we're in the middle of a TTS tag, extract what we have as TTS content
            if in_tts_tag && !tts_content.trim().is_empty() {
                log::info!(
                    "⚠️  FALLBACK: Extracting incomplete TTS content at stream end: '{}'",
                    tts_content
                );
                log::warn!("TTS was processed at stream end instead of immediately during streaming. This indicates the TTS tags may have been split across chunks.");
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

        Ok((accumulated_text, tool_calls, stop_reason, response_stream_started))
    }

    /// Process text chunk to extract thinking XML tags with streaming support
    ///
    /// This function handles:
    /// - Proper buffer management to avoid character duplication/loss
    /// - Partial XML tags split across streaming chunks
    /// - Streaming thinking content as it arrives
    /// - Complete tag removal to prevent leakage
    ///
    /// Returns: (output_text, thinking_started, thinking_ended, thinking_chunk)
    /// - output_text: Text without thinking tags (for regular streaming)
    /// - thinking_started: True if we just entered a <thinking> tag
    /// - thinking_ended: True if we just exited a </thinking> tag
    /// - thinking_chunk: Content to stream for thinking (may be empty)
    fn process_text_with_thinking_extraction_streaming(
        &self,
        text_chunk: &str,
        thinking_buffer: &mut String,
        in_thinking_tag: &mut bool,
        thinking_content: &mut String,
    ) -> (String, bool, bool, String) {
        let mut output_text = String::new();
        let mut thinking_chunk = String::new();
        let mut thinking_started = false;
        let mut thinking_ended = false;

        // Add new text to buffer for processing
        thinking_buffer.push_str(text_chunk);

        let mut chars_to_consume = 0;
        let buffer_chars: Vec<char> = thinking_buffer.chars().collect();
        let mut i = 0;

        while i < buffer_chars.len() {
            let remaining_len = buffer_chars.len() - i;
            let remaining_str: String = buffer_chars[i..].iter().collect();

            if !*in_thinking_tag {
                // Outside thinking tag - look for opening tag
                if remaining_str.starts_with("<thinking>") {
                    // Found complete opening tag
                    *in_thinking_tag = true;
                    thinking_started = true;
                    i += 10; // Skip "<thinking>"
                    chars_to_consume = i;
                    continue;
                } else if remaining_len < 10 && self.could_be_partial_thinking_opening_tag(&remaining_str) {
                    // Potential partial opening tag at end of buffer - stop processing here
                    break;
                } else {
                    // Regular character outside thinking - add to output
                    output_text.push(buffer_chars[i]);
                    i += 1;
                    chars_to_consume = i;
                }
            } else {
                // Inside thinking tag - look for closing tag
                if remaining_str.starts_with("</thinking>") {
                    // Found complete closing tag
                    thinking_ended = true;
                    log::debug!("Thinking block ended, total content: {} chars", thinking_content.len());

                    // Reset thinking state for next potential block
                    *in_thinking_tag = false;
                    i += 11; // Skip "</thinking>"
                    chars_to_consume = i;
                    continue;
                } else if remaining_len < 11 && self.could_be_partial_thinking_closing_tag(&remaining_str) {
                    // Potential partial closing tag at end of buffer - stop processing
                    break;
                } else {
                    // Content inside thinking tag - stream it
                    let char_to_stream = buffer_chars[i];
                    thinking_chunk.push(char_to_stream);
                    thinking_content.push(char_to_stream);
                    i += 1;
                    chars_to_consume = i;
                }
            }
        }

        // Remove processed characters from buffer
        if chars_to_consume > 0 && chars_to_consume <= buffer_chars.len() {
            *thinking_buffer = buffer_chars[chars_to_consume..].iter().collect();
        }

        // Validate no thinking tags remain in output
        if output_text.contains("<thinking>") || output_text.contains("</thinking>") {
            log::error!("CRITICAL BUG: thinking tags found in output_text during streaming processing!");
            output_text = output_text.replace("<thinking>", "").replace("</thinking>", "");
        }

        (output_text, thinking_started, thinking_ended, thinking_chunk)
    }

    /// Check if a string could be the beginning of a partial "<thinking>" tag
    fn could_be_partial_thinking_opening_tag(&self, s: &str) -> bool {
        if s.is_empty() {
            return false;
        }
        let partial_tags = ["<", "<t", "<th", "<thi", "<thin", "<think", "<thinki", "<thinkin", "<thinking"];
        partial_tags.iter().any(|&tag| s.eq_ignore_ascii_case(tag))
    }

    /// Check if a string could be the beginning of a partial "</thinking>" tag
    fn could_be_partial_thinking_closing_tag(&self, s: &str) -> bool {
        if s.is_empty() {
            return false;
        }
        let partial_tags = ["<", "</", "</t", "</th", "</thi", "</thin", "</think", "</thinki", "</thinkin", "</thinking"];
        partial_tags.iter().any(|&tag| s.eq_ignore_ascii_case(tag))
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
                        log::info!("✅ IMMEDIATE: Extracted TTS content during streaming: '{}'", tts_content);
                    }

                    // Reset TTS state for next potential block
                    *in_tts_tag = false;
                    tts_content.clear();
                    i += 6; // Skip "</TTS>"
                    chars_to_consume = i;
                    continue;
                } else if remaining_len < 6 && self.could_be_partial_closing_tag(&remaining_str) {
                    // Potential partial closing tag at end of buffer — wait for the
                    // next chunk so we can match the complete "</TTS>" tag.
                    // We must NOT extract TTS content early here because the remaining
                    // tag characters (e.g. "TS>") would leak into the next chunk's
                    // display text once in_tts_tag is set to false.
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
        partial_tags.contains(&s)
    }

    /// Check if a string could be the beginning of a partial "</TTS>" tag
    fn could_be_partial_closing_tag(&self, s: &str) -> bool {
        if s.is_empty() {
            return false;
        }
        let partial_tags = ["<", "</", "</T", "</TT", "</TTS"];
        partial_tags.contains(&s)
    }

    /// Strip TTS XML tags from text, removing them completely (content was already processed for TTS)
    fn strip_tts_tags(&self, text: &str) -> String {
        // CRITICAL FIX: Remove TTS tags completely - content was already processed for immediate TTS
        // We don't want TTS content appearing in the final display text
        match Regex::new(r"<TTS>.*?</TTS>") {
            Ok(tts_regex) => tts_regex
                .replace_all(text, "")
                .to_string()
                .trim()
                .to_string(),
            Err(e) => {
                tracing::warn!("Failed to compile TTS regex: {}", e);
                text.to_string()
            }
        }
    }

    /// Strip thinking XML tags from text, removing them completely (content was already emitted as thinking events)
    fn strip_thinking_tags(&self, text: &str) -> String {
        match Regex::new(r"(?i)<thinking>[\s\S]*?</thinking>") {
            Ok(thinking_regex) => thinking_regex
                .replace_all(text, "")
                .to_string()
                .trim()
                .to_string(),
            Err(e) => {
                tracing::warn!("Failed to compile thinking regex: {}", e);
                text.to_string()
            }
        }
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

/// Save API request to file for debugging in development mode only
#[cfg(debug_assertions)]
async fn save_debug_request(request: &AnthropicRequest) {
    use std::fs;
    use std::path::PathBuf;
    use chrono::Utc;

    // Create debug directory if it doesn't exist
    let debug_dir = PathBuf::from("debug");
    if let Err(e) = fs::create_dir_all(&debug_dir) {
        log::warn!("Failed to create debug directory: {}", e);
        return;
    }

    // Generate filename with timestamp
    let timestamp = Utc::now().format("%Y%m%d_%H%M%S_%3f");
    let filename = format!("agent_request_{}.json", timestamp);
    let filepath = debug_dir.join(filename);

    // Serialize the FULL request (unsanitized) for debugging
    match serde_json::to_string_pretty(request) {
        Ok(json_string) => {
            if let Err(e) = fs::write(&filepath, json_string) {
                log::warn!("Failed to write debug request to {}: {}", filepath.display(), e);
            } else {
                log::info!("💾 Debug request saved to: {}", filepath.display());
            }
        }
        Err(e) => {
            log::warn!("Failed to serialize request for debug saving: {}", e);
        }
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
            if message.role == Role::Tool {
                if let Some(tool_call_id) = &message.tool_call_id {
                    // Check if this tool result has a corresponding tool call
                    if !pending_tool_calls.contains(tool_call_id) {
                        log::warn!("Removing orphaned tool result with ID: {} - no corresponding tool_use found", tool_call_id);
                        orphaned_results_found = true;
                        continue; // Skip this message
                    }
                }
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
                    let tool_name = message.name.as_deref().unwrap_or("");

                    // Build the tool_result content — special handling for computer tool screenshots
                    let result_content: ApiToolResultContent =
                        match serde_json::from_str::<serde_json::Value>(&tool_result_content) {
                            Ok(json_value) => {
                                // Check for computer tool with base64 screenshot data
                                if tool_name == "computer" {
                                    if let Some(base64_data) =
                                        json_value.get("base64_image").and_then(|v| v.as_str())
                                    {
                                        // Return image content block so the model can see the screenshot
                                        let mut blocks = vec![ApiToolResultBlock {
                                            block_type: "image".to_string(),
                                            source: Some(ApiImageSource {
                                                source_type: "base64".to_string(),
                                                media_type: "image/jpeg".to_string(),
                                                data: base64_data.to_string(),
                                            }),
                                            text: None,
                                        }];
                                        // Include any text output alongside the image
                                        if let Some(output_text) =
                                            json_value.get("output").and_then(|v| v.as_str())
                                        {
                                            if !output_text.is_empty() {
                                                blocks.push(ApiToolResultBlock {
                                                    block_type: "text".to_string(),
                                                    source: None,
                                                    text: Some(output_text.to_string()),
                                                });
                                            }
                                        }
                                        log::debug!(
                                            "Computer tool result: image content block ({} bytes base64)",
                                            base64_data.len()
                                        );
                                        ApiToolResultContent::Blocks(blocks)
                                    } else {
                                        // Computer tool result without screenshot (e.g., click, type)
                                        let text = json_value
                                            .get("output")
                                            .and_then(|v| v.as_str())
                                            .unwrap_or("Action completed")
                                            .to_string();
                                        ApiToolResultContent::Text(text)
                                    }
                                }
                                // Extract stdout for command results
                                else if let Some(stdout) =
                                    json_value.get("stdout").and_then(|v| v.as_str())
                                {
                                    ApiToolResultContent::Text(stdout.trim().to_string())
                                }
                                // Extract content for file reads
                                else if let Some(content) =
                                    json_value.get("content").and_then(|v| v.as_str())
                                {
                                    ApiToolResultContent::Text(content.trim().to_string())
                                }
                                // For error messages
                                else if let Some(error) =
                                    json_value.get("error").and_then(|v| v.as_str())
                                {
                                    ApiToolResultContent::Text(
                                        format!("Error: {}", error.trim()),
                                    )
                                }
                                // Fallback: find first string value or use generic message
                                else {
                                    let simplified = json_value.as_object().and_then(|obj| {
                                        obj.values()
                                            .find_map(|v| v.as_str().map(|s| s.trim().to_string()))
                                    });
                                    ApiToolResultContent::Text(
                                        simplified.unwrap_or_else(|| {
                                            "Tool executed successfully".to_string()
                                        }),
                                    )
                                }
                            }
                            Err(_) => {
                                // If content is not JSON, use it directly (trimmed)
                                ApiToolResultContent::Text(
                                    tool_result_content.trim().to_string(),
                                )
                            }
                        };

                    api_messages.push(ApiMessage {
                        role: "user".to_string(), // Tool results have role "user"
                        content: ApiContent::Blocks(vec![ApiContentBlock {
                            block_type: "tool_result".to_string(),
                            tool_use_id: Some(tool_call_id),
                            text: None,
                            id: None,
                            name: None,
                            input: None,
                            content: Some(result_content),
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

        // --- Screenshot History Limiting ---
        // Keep only the N most recent screenshots in the conversation to reduce token usage.
        // Older screenshots are replaced with a text placeholder. This follows the pattern
        // used by Cua (only_n_most_recent_images=3) and reduces costs by ~60-70%.
        Self::limit_screenshot_history(&mut api_messages, MAX_RECENT_SCREENSHOTS);

        let api_tools = if available_tools.is_empty() {
            None
        } else {
            let mut tools: Vec<ApiTool> = available_tools
                .iter()
                .filter_map(|t| {
                    if let Some(api_type) = &t.api_type {
                        // Built-in Anthropic tool (computer, bash, text_editor)
                        let (dw, dh) = if t.name == "computer" {
                            match crate::utils::coordinates::get_current_standard_resolution() {
                                Ok((w, h)) if w > 0 && h > 0 => {
                                    log::info!(
                                        "Computer tool configured with display_width_px={}, display_height_px={}",
                                        w, h
                                    );
                                    (Some(w), Some(h))
                                }
                                Ok((w, h)) => {
                                    log::warn!(
                                        "Standard resolution not yet initialized ({}x{}), skipping computer tool to prevent coordinate mismatch",
                                        w, h
                                    );
                                    return None;
                                }
                                Err(e) => {
                                    log::warn!(
                                        "Cannot determine display resolution: {}, skipping computer tool to prevent coordinate mismatch",
                                        e
                                    );
                                    return None;
                                }
                            }
                        } else {
                            (None, None)
                        };
                        // Enable zoom for computer_20251124 (Opus 4.5+)
                        // This allows Claude to inspect specific screen regions at native resolution
                        let enable_zoom = if t.name == "computer" && api_type.contains("20251124") {
                            Some(true)
                        } else {
                            None
                        };
                        Some(ApiTool::BuiltIn {
                            tool_type: self.resolve_tool_api_type(&t.name, api_type),
                            name: t.name.clone(),
                            display_width_px: dw,
                            display_height_px: dh,
                            enable_zoom,
                            cache_control: None, // Set on last tool below
                        })
                    } else {
                        Some(ApiTool::Custom {
                            name: t.name.clone(),
                            description: t.description.clone(),
                            input_schema: t.input_schema.clone(),
                            cache_control: None, // Set on last tool below
                        })
                    }
                })
                .collect();
            // Add cache_control to the last tool to enable prompt caching of the tool definitions.
            // When Anthropic caches tools, subsequent turns skip re-processing ~2,146 tokens of
            // tool definitions, reducing latency by 50-80% for the cached portion.
            if let Some(last_tool) = tools.last_mut() {
                match last_tool {
                    ApiTool::BuiltIn { cache_control, .. } => {
                        *cache_control = Some(CacheControl { cache_type: "ephemeral".to_string() });
                    }
                    ApiTool::Custom { cache_control, .. } => {
                        *cache_control = Some(CacheControl { cache_type: "ephemeral".to_string() });
                    }
                }
            }
            if tools.is_empty() { None } else { Some(tools) }
        };

        // Convert system prompt to content block array with cache_control for prompt caching
        let system_blocks = self.system_prompt.as_ref().map(|prompt| {
            vec![SystemContentBlock {
                block_type: "text".to_string(),
                text: prompt.clone(),
                cache_control: Some(CacheControl { cache_type: "ephemeral".to_string() }),
            }]
        });

        let mut request_payload = AnthropicRequest {
            model: self.model.clone(),
            messages: api_messages,
            tools: api_tools,
            system: system_blocks,
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

        // -- DEVELOPMENT MODE: Save full request for debugging --
        #[cfg(debug_assertions)]
        {
            save_debug_request(&request_payload).await;
        }

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
            // Combine computer use beta + prompt caching beta in a single header (comma-separated).
            // Prompt caching reduces input token costs by ~90% and latency by ~50-80% for
            // system prompt and tool definitions that remain stable across agent loop turns.
            .header("anthropic-beta", format!("{},{}", self.resolve_computer_use_beta_header(), crate::constants::api::beta_flags::PROMPT_CACHING))
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
            return Err(AgentError::LlmError(Self::format_anthropic_http_error_for_user(
                status,
                &error_body,
            )));
        }

        // --- 4. Handle Response (Streaming or Non-Streaming) ---
        if use_streaming {
            // Handle streaming response
            let app_handle = app_handle.ok_or("AppHandle required for streaming")?;
            let message_id = message_id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

            // Note: stream_start is now emitted inside handle_streaming_response
            // when we have actual non-thinking text to display. This ensures thinking
            // messages appear BEFORE the response message in the chat.

            let (accumulated_text, tool_calls, stop_reason, stream_was_started) = self
                .handle_streaming_response(response, Some(&app_handle), Some(message_id.clone()), |chunk, tts_list| {
                    // Emit text chunk event - pass first TTS item if available for backward compatibility
                    crate::agent::tool_logger::emit_streaming_text_chunk(
                        &app_handle,
                        chunk,
                        Some(message_id.clone()),
                        tts_list.first().cloned(),
                    );

                    // Emit additional TTS items if there are multiple in this chunk
                    for tts_content in tts_list.iter().skip(1) {
                        crate::agent::tool_logger::emit_streaming_text_chunk(
                            &app_handle,
                            String::new(), // Empty display text for additional TTS-only chunks
                            Some(message_id.clone()),
                            Some(tts_content.clone()),
                        );
                    }
                })
                .await?;

            // Clean up any remaining thinking tags from the accumulated text
            let mut accumulated_text = accumulated_text;
            if accumulated_text.contains("<thinking>") || accumulated_text.contains("</thinking>") {
                log::warn!("Thinking tags found in final accumulated text - cleaning up");
                accumulated_text = self.strip_thinking_tags(&accumulated_text);
            }

            // TTS tags should now be completely removed during streaming processing
            // If any remain, it indicates a bug in our improved processing logic
            if accumulated_text.contains("<TTS>") || accumulated_text.contains("</TTS>") {
                log::error!("CRITICAL BUG: TTS tags found in final accumulated text after improved processing!");
                log::error!("This should never happen with the fixed streaming logic");
                log::error!("Accumulated text: '{}'", accumulated_text);

                // Emergency fallback - but this indicates a serious bug
                accumulated_text = self.strip_tts_tags(&accumulated_text);
                log::error!("Emergency TTS cleanup applied");
            }

            let final_display_text = accumulated_text;

            // Always emit stream_start + stream_end so the frontend knows a response happened.
            // For TTS-only responses, display_text is empty but we still need to notify the
            // frontend so it can show a "Complete" indicator rather than appearing to hang.
            if !stream_was_started {
                crate::agent::tool_logger::emit_stream_start(&app_handle, message_id.clone());
            }
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
