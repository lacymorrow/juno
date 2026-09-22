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
    /// Extended-thinking configuration. Sent as `{type: "adaptive", display: "summarized"}`
    /// on models that support adaptive thinking so the UI's reasoning panel gets a readable
    /// summary; omitted entirely on older models (which would reject it).
    #[serde(skip_serializing_if = "Option::is_none")]
    thinking: Option<Value>,
    /// Server-side refusal fallbacks (`"default"`): when a safety classifier declines a
    /// request on Fable/Opus-5-tier models the API retries on a fallback model in the
    /// same round trip instead of returning an empty `refusal` turn.
    #[serde(skip_serializing_if = "Option::is_none")]
    fallbacks: Option<Value>,
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
    /// Cache lifetime. Omitted entirely for the default 5-minute cache; `Some("1h")` opts
    /// into the extended 1-hour cache. Skipped when `None` so a default breakpoint
    /// serializes byte-for-byte as it did before this field existed.
    #[serde(skip_serializing_if = "Option::is_none")]
    ttl: Option<String>,
}

impl CacheControl {
    /// A default breakpoint: ephemeral, 5-minute TTL (expressed by omitting `ttl`).
    fn ephemeral() -> Self {
        CacheControl {
            cache_type: "ephemeral".to_string(),
            ttl: None,
        }
    }

    /// A breakpoint with the extended 1-hour TTL. See `CACHE_TTL_EXTENDED` for when this
    /// earns its higher write price.
    fn ephemeral_extended() -> Self {
        CacheControl {
            cache_type: "ephemeral".to_string(),
            ttl: Some(CACHE_TTL_EXTENDED.to_string()),
        }
    }
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
    /// The toolset a `tool_use` block belongs to, and the value that must be
    /// echoed back on the matching `tool_result`.
    ///
    /// Present only on the `computer_toolset_20260801` path, in both
    /// directions: Claude sets it on every member `tool_use`, and the API
    /// requires it back on every corresponding `tool_result`. `None` on the
    /// legacy path, where `skip_serializing_if` keeps it off the wire entirely
    /// so those requests are byte-identical to what they were before.
    #[serde(skip_serializing_if = "Option::is_none")]
    toolset_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_use_id: Option<String>, // For tool result blocks
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<ApiToolResultContent>, // For tool_result content (text or image blocks)
    // --- Extended thinking blocks (replayed verbatim on tool-use turns) ---
    #[serde(skip_serializing_if = "Option::is_none")]
    thinking: Option<String>, // `thinking` block text (may be empty when display is omitted)
    #[serde(skip_serializing_if = "Option::is_none")]
    signature: Option<String>, // `thinking` block signature
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<String>, // `redacted_thinking` block payload
    // --- Image blocks (a picture the person attached to their message) ---
    #[serde(skip_serializing_if = "Option::is_none")]
    source: Option<ApiImageSource>,
    /// Cache breakpoint for Anthropic prompt caching. Set on the last `tool_result` block of
    /// selected turns so the conversation history — which is where the screenshot tokens live —
    /// is cached rather than reprocessed every turn. See `apply_message_cache_breakpoints`.
    #[serde(skip_serializing_if = "Option::is_none")]
    cache_control: Option<CacheControl>,
}

/// Split a `data:` URL into the media type and payload Anthropic wants.
///
/// Returns `None` for anything that is not a base64 data URL, so a malformed
/// paste is dropped rather than sent as a block the API will reject.
fn parse_data_url(url: &str) -> Option<(String, String)> {
    let rest = url.strip_prefix("data:")?;
    let (meta, data) = rest.split_once(',')?;
    let media_type = meta.strip_suffix(";base64")?;
    if !media_type.starts_with("image/") || data.is_empty() {
        return None;
    }
    Some((media_type.to_string(), data.to_string()))
}

impl ApiContentBlock {
    /// An empty block with every optional field unset; callers set only what they need.
    fn empty(block_type: &str) -> Self {
        ApiContentBlock {
            block_type: block_type.to_string(),
            text: None,
            id: None,
            name: None,
            input: None,
            toolset_name: None,
            tool_use_id: None,
            content: None,
            thinking: None,
            signature: None,
            data: None,
            source: None,
            cache_control: None,
        }
    }
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
    /// The `computer_toolset_20260801` toolset.
    ///
    /// Format: `{"type": "computer_toolset_20260801"}` — and that is the whole
    /// entry. Three fields that `BuiltIn` carries are **absent on purpose**, and
    /// each one is a request-level rejection if added:
    ///
    /// - **no `name`.** A toolset is not a tool; it expands server-side into 17
    ///   member tools which supply their own names. This is the single most
    ///   likely thing to get wrong, because every other entry in the array has a
    ///   `name`, so it is a separate variant rather than an `Option` on
    ///   `BuiltIn` — you cannot forget to clear a field that does not exist.
    /// - **no `display_width_px` / `display_height_px`.** The toolset takes no
    ///   display dimensions and the API does not downscale for you, so an
    ///   oversized `tool_result` image is rejected. Juno sizes its own
    ///   screenshots instead (see `ImageTier`).
    /// - **no `enable_zoom`.** Zoom is a member tool, on by default; it is
    ///   turned off through `configs`, not a top-level flag.
    Toolset {
        #[serde(rename = "type")]
        tool_type: String,
        /// Per-member settings, keyed by member name. Omitted entirely when
        /// every member keeps its default, which is Juno's case today.
        #[serde(skip_serializing_if = "Option::is_none")]
        configs: Option<Value>,
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

/// Maximum number of recent screenshots to keep in conversation history.
/// Older screenshots are replaced with text placeholders to reduce token usage.
/// Following the pattern from Cua (only_n_most_recent_images=3).
/// Each 1024x768 screenshot costs ~1,049 tokens — limiting from 10 to 3 saves ~7,000 tokens/step.
///
/// NOTE: this is the *floor* a batch prune drops back to, not a per-turn cap. See
/// `SCREENSHOT_PRUNE_HIGH_WATER` for why pruning is batched instead of run every turn.
const MAX_RECENT_SCREENSHOTS: usize = 3;

// --- Prompt-cache-aware screenshot pruning ---------------------------------------------------
//
// WHY THIS IS BATCHED AND NOT PER-TURN. Anthropic prompt caching is an *exact prefix match*:
// the request hashes as tools -> system -> messages, and a cache entry is only usable up to the
// first byte that differs from the cached request. The conversation history is by far the bulk
// of a computer-use payload (every screenshot is thousands of tokens), so keeping that prefix
// byte-identical between turns is worth far more than the tokens any pruning saves.
//
// The old scheduler kept "the N most recent screenshots" and rewrote everything older into a
// placeholder. Because a new screenshot arrives every turn, that rewrote a *different* message
// in the middle of the prefix on *every* turn:
//
//   turn 10:  msg1..msg5 = placeholder   msg6=IMG  msg7=IMG  msg8=IMG
//   turn 11:  msg1..msg5 = placeholder   msg6=placeholder  msg7=IMG  msg8=IMG  msg9=IMG
//                                             ^ mutated mid-prefix, cache dead from here on
//
// So the prefix diverged at msg6 every single turn and essentially nothing was ever cached.
//
// The fix: the prefix must be APPEND-ONLY between prunes. Pruning now happens in one batch
// when the retained screenshot count reaches the high-water mark, dropping back to the
// low-water mark; in between, no existing message is touched at all, so every turn extends the
// previous turn's prefix and hits the cache. Anthropic's own computer-use guidance says the
// same thing: prune batches of old screenshots every ~25 turns, never every turn.
//
// DO NOT "optimise" this back into per-turn pruning. It looks like it saves tokens and it
// costs roughly an order of magnitude more, because cached input tokens bill at ~10% of
// uncached ones and per-turn pruning makes every turn uncached.

/// Screenshot count at which one batch prune runs. Reaching this many retained screenshots
/// triggers a single pass that prunes the oldest ones down to `MAX_RECENT_SCREENSHOTS`.
/// Between prunes the message prefix is append-only, so 8 of every 9 turns are full cache hits.
const SCREENSHOT_PRUNE_HIGH_WATER: usize = 12;

/// Screenshots that survive a batch prune (the low-water mark). Same value, and same meaning,
/// as `MAX_RECENT_SCREENSHOTS`: it is how many real images the model is guaranteed to still
/// see immediately after a prune. Between prunes the model sees more (up to
/// `SCREENSHOT_PRUNE_HIGH_WATER - 1`), which is strictly better grounding for the same money,
/// because those extra images are served from cache.
const SCREENSHOT_PRUNE_LOW_WATER: usize = MAX_RECENT_SCREENSHOTS;

/// Placeholder that replaces a pruned screenshot. Deliberately carries NO numbers or other
/// varying text: the placeholder is part of the cached prefix, so any turn-dependent content in
/// it would change the bytes of an already-sent message and invalidate the cache.
const PRUNED_SCREENSHOT_PLACEHOLDER: &str =
    "[Older screenshot removed to save context. Take a new screenshot if you need to see the screen.]";

// --- Cache breakpoint budget -----------------------------------------------------------------
//
// Anthropic allows at most 4 `cache_control` breakpoints per request, and the request hashes in
// the order tools -> system -> messages. Juno's budget, in prefix order:
//
//   | # | breakpoint         | caches                                  | TTL | set in                        |
//   |---|--------------------|-----------------------------------------|-----|-------------------------------|
//   | 1 | last tool def      | the tool schemas (~2.1k tokens)         | 1h  | tools build site  [always]    |
//   | 2 | system prompt block| tools + system prompt                   | 1h  | system build site [when set]  |
//   | 3 | message anchor     | history through an older tool_result    | 1h  | apply_message_cache_breakpoints |
//   | 4 | latest tool_result | history through the current turn        | 5m  | apply_message_cache_breakpoints |
//
// Anything that wants another breakpoint has to take one of these four away — there is no fifth.
//
// WHY THREE OF THEM ARE 1-HOUR. Juno is two workloads wearing one coat. In the computer-use
// loop, turns are seconds apart and the default 5-minute cache never gets cold. But Juno is also
// an interactive voice and chat app, and there the user says a command, watches it run, and then
// thinks for a while before the next one. At the default TTL every one of those turns is a cold
// start — and because batch pruning deliberately lets the retained screenshot count grow to
// SCREENSHOT_PRUNE_HIGH_WATER - 1, a cold start now reprocesses up to 11 screenshots where the
// old per-turn pruning would have reprocessed 3. A 1-hour TTL removes that tail outright, which
// is what lets the high-water mark stay where it is instead of being tuned down to buy back the
// interactive case at the loop's expense.
//
// COST SHAPE. A 1-hour cache write costs ~2x the base input price; a 5-minute write ~1.25x;
// reads are ~0.1x either way (~0.025x on Fable 5.1, Juno's default model). So a 1h write pays
// for itself the moment it is read even once more than a cold request would have been — the
// break-even is about 1.1 reads — while a 5-minute write that expires before it is ever read is
// strictly worse than not caching at all (1.25x for nothing).
//
// WHY THE LATEST BREAKPOINT STAYS AT 5 MINUTES. It moves every turn: written at turn N, read at
// turn N+1, then superseded within seconds. Buying it an hour of life is paying the 2x premium
// for something that is thrown away immediately. It is also the one breakpoint whose extra cost
// would be paid on every single turn rather than occasionally.
//
// ORDERING IS A HARD API RULE, NOT A PREFERENCE. Every 1-hour breakpoint must appear before
// every 5-minute breakpoint in the prefix. The table above is in prefix order and the three 1h
// entries all precede the single 5m one, which is why `apply_message_cache_breakpoints` pushes
// the anchor before the latest. Do not reorder them, and do not give the anchor a 5m TTL while
// anything earlier keeps 1h.
//
// WHEN THE 1-HOUR PREMIUM IS ACTUALLY PAID. Billing walks three positions: A = the longest cache
// hit, B = the last 1h breakpoint after A, C = the last breakpoint; you pay a read for A, a 1h
// write for (B - A), and a 5m write for (C - B). In a warm loop the previous turn's latest
// breakpoint is the hit, so A already sits past the anchor, B == A, and the 1h write is zero —
// the loop pays 1.25x on one turn's delta and nothing more. B only moves ahead of A on the turns
// where the anchor advances (every CACHE_ANCHOR_STRIDE turns, when anchor and latest coincide
// and a single 1h breakpoint is written). That is the whole premium: 2x on one turn's delta,
// once per stride, in exchange for an entry that survives the user going away for an hour.

/// `ttl` value for Anthropic's extended cache duration. The default 5-minute cache is expressed
/// by omitting `ttl` entirely rather than by sending `"5m"`.
///
/// Verified against the live docs on 2026-09-22:
/// <https://platform.claude.com/docs/en/build-with-claude/prompt-caching> — `ttl` sits inside
/// `cache_control` alongside `type`, the accepted values are `"5m"` and `"1h"`, the feature is
/// generally available on all active models, and it needs **no** `anthropic-beta` header.
/// (Per LAC-3106: check this against the live docs, never from memory or a cached catalog.)
const CACHE_TTL_EXTENDED: &str = "1h";

/// Hard API limit on `cache_control` breakpoints in a single request.
const MAX_CACHE_BREAKPOINTS: usize = 4;

/// Breakpoints spent outside `messages` (the last tool definition and the system prompt).
const PREFIX_CACHE_BREAKPOINTS: usize = 2;

/// Breakpoints left over for the conversation history.
const MESSAGE_CACHE_BREAKPOINTS: usize = MAX_CACHE_BREAKPOINTS - PREFIX_CACHE_BREAKPOINTS;

/// How often (measured in tool-result turns) the "anchor" message breakpoint advances.
///
/// The newest breakpoint always sits on the latest tool_result, so the next turn gets a full
/// history hit. The anchor is quantised to this stride so it stays on the *same* message for
/// several turns, keeping one longer-lived cache entry alive (each read refreshes its 5-minute
/// TTL) as insurance when the newest entry misses — e.g. on the turn right after a batch prune.
const CACHE_ANCHOR_STRIDE: usize = 8;

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
    /// Thinking blocks captured from tool-use turns, keyed by the first tool_use id of that
    /// turn. The API requires the thinking blocks of the previous assistant turn to be
    /// replayed unchanged whenever that turn contained tool calls; our `Message` history only
    /// stores tool calls, so the provider keeps the blocks here and re-attaches them when the
    /// history is converted back into API messages. Bounded to `THINKING_CACHE_LIMIT` turns.
    thinking_cache:
        std::sync::Arc<std::sync::Mutex<std::collections::HashMap<String, Vec<ApiContentBlock>>>>,
}

/// Maximum number of tool-use turns whose thinking blocks are retained for replay.
const THINKING_CACHE_LIMIT: usize = 64;

/// Shown when the API declines a request with `stop_reason: "refusal"`.
const REFUSAL_MESSAGE: &str =
    "I can't help with that request. If it was a mistake, try rephrasing or narrowing it down.";

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
        let max_tokens =
            max_tokens.unwrap_or(crate::constants::agent::config::DEFAULT_MAX_TOKENS_ANTHROPIC);

        // Create HTTP client with proper timeout configuration to prevent hanging
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(
                crate::constants::timeouts::HTTP_REQUEST_TIMEOUT_SECONDS,
            ))
            .connect_timeout(std::time::Duration::from_secs(
                crate::constants::timeouts::HTTP_CONNECT_TIMEOUT_SECONDS,
            ))
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
            thinking_cache: std::sync::Arc::new(std::sync::Mutex::new(
                std::collections::HashMap::new(),
            )),
        })
    }

    /// Creates a new AnthropicBrain from a CentralizedProviderConfig struct.
    /// The key comes from the settings store, then the ANTHROPIC_API_KEY env
    /// var (a .env file rather than the Tauri Store), then the key baked into
    /// a demo build. The person's own key always wins.
    pub fn from_config(config: &crate::settings::ProviderConfig) -> Result<Self, AgentError> {
        let api_key = crate::demo::resolve_api_key(
            config.api_key.clone(),
            env::var("ANTHROPIC_API_KEY").ok(),
            crate::demo::api_key(),
        )
        .ok_or_else(|| {
            AgentError::ConfigurationError(
                "Anthropic API key not found in settings or ANTHROPIC_API_KEY env var".into(),
            )
        })?;
        Self::new(
            api_key,
            config.model.clone(),
            config.max_tokens,
            config.system_prompt.clone(),
        )
    }

    fn format_anthropic_http_error_for_user(
        status: reqwest::StatusCode,
        error_body: &str,
        using_demo_key: bool,
    ) -> String {
        // A demo build's key is not the person's, so API-shaped advice about it
        // is useless to them. Tell them the demo ended and where to go next.
        if using_demo_key {
            if let Some(message) = crate::demo::ended_message(status.as_u16(), error_body) {
                return message.to_string();
            }
        }

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
                        format!(
                            "Anthropic API error {} ({}): {}",
                            status, error_type, message
                        )
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

    /// The computer-use beta header for the selected model, or `None` when the
    /// model declares no computer-use tool version — a chat-only or unknown
    /// model must not advertise a computer-use beta it cannot honour.
    fn resolve_computer_use_beta_header(&self) -> Option<&'static str> {
        Provider::Anthropic.computer_use_beta_flag(&self.model)
    }

    /// Whether the selected model drives the computer through the
    /// `computer_toolset_20260801` toolset instead of a single `computer` tool.
    ///
    /// Answered from the one capability table, per model. This is the only
    /// question that selects between the two wire formats; there is no model-ID
    /// list here or anywhere else.
    fn uses_computer_toolset(&self) -> bool {
        Provider::Anthropic.uses_computer_toolset(&self.model)
    }

    /// The `toolset_name` this model's computer tool calls arrive with, if any.
    fn computer_toolset_name(&self) -> Option<&'static str> {
        Provider::Anthropic.computer_toolset_name(&self.model)
    }

    /// Route a raw `(name, toolset_name, input)` triple from the wire into the
    /// `(name, input)` pair Juno executes.
    ///
    /// Applied at both response-parsing sites (streaming and non-streaming) so
    /// the two cannot drift. On the legacy path `toolset_name` is always absent,
    /// so this is the identity function and the old behaviour is untouched.
    fn route_tool_call(
        &self,
        name: String,
        toolset_name: Option<&str>,
        input: Value,
    ) -> (String, Value) {
        match crate::agent::tools::anthropic_computer_use::route_toolset_call(
            &name,
            toolset_name,
            &input,
        ) {
            Some((routed_name, routed_input)) => {
                log::debug!(
                    "Routed toolset member '{}' (toolset_name={:?}) to the computer tool",
                    name,
                    toolset_name
                );
                (routed_name, routed_input)
            }
            None => (name, input),
        }
    }

    /// Full `anthropic-beta` header value for the selected model.
    fn beta_header_value(&self) -> String {
        let mut flags = vec![crate::constants::api::beta_flags::PROMPT_CACHING];
        if let Some(computer_use) = self.resolve_computer_use_beta_header() {
            flags.insert(0, computer_use);
        }
        if Provider::Anthropic.supports_server_side_fallbacks(&self.model) {
            flags.push(crate::constants::api::beta_flags::SERVER_SIDE_FALLBACK);
        }
        flags.join(",")
    }

    /// `thinking` request parameter for the selected model, or `None` when the model does
    /// not accept adaptive thinking (older models would return 400).
    fn thinking_param(&self) -> Option<Value> {
        if Provider::Anthropic.supports_adaptive_thinking(&self.model) {
            Some(serde_json::json!({ "type": "adaptive", "display": "summarized" }))
        } else {
            None
        }
    }

    /// `fallbacks` request parameter for the selected model.
    fn fallbacks_param(&self) -> Option<Value> {
        if Provider::Anthropic.supports_server_side_fallbacks(&self.model) {
            Some(Value::String("default".to_string()))
        } else {
            None
        }
    }

    /// Remember the thinking blocks that preceded a set of tool calls so they can be replayed
    /// with that assistant turn on the next request.
    fn remember_thinking(&self, tool_calls: &[ToolCall], thinking_blocks: Vec<ApiContentBlock>) {
        let Some(first) = tool_calls.first() else {
            return;
        };
        if thinking_blocks.is_empty() {
            return;
        }
        let mut cache = match self.thinking_cache.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        if cache.len() >= THINKING_CACHE_LIMIT {
            // Cheap bound: drop everything rather than track insertion order. Older turns'
            // thinking blocks are only needed for the immediately preceding tool-use turn.
            cache.clear();
        }
        cache.insert(first.id.clone(), thinking_blocks);
    }

    /// Thinking blocks previously captured for the assistant turn that issued `tool_calls`.
    fn recall_thinking(&self, tool_calls: &[ToolCall]) -> Vec<ApiContentBlock> {
        let Some(first) = tool_calls.first() else {
            return Vec::new();
        };
        let cache = match self.thinking_cache.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        cache.get(&first.id).cloned().unwrap_or_default()
    }

    /// Resolve the correct tool API type for the selected model.
    /// Opus 4.5+ models require newer tool type identifiers (e.g. computer_20251124,
    /// text_editor_20250728) while older models use the registered defaults.
    fn resolve_tool_api_type(&self, tool_name: &str, registered_type: &str) -> String {
        Provider::Anthropic.resolve_tool_type(tool_name, registered_type, &self.model)
    }

    /// How many of the oldest screenshots to prune, given how many are in the history.
    ///
    /// This is the whole scheduler, kept pure so it can be reasoned about (and tested) on its
    /// own. The contract that matters is **append-only between prunes**: for every `total` in
    /// the band between two prune points this returns the *same* count, so the same oldest
    /// screenshots are replaced by the same placeholder bytes and the serialized prefix of the
    /// request is byte-identical to the previous turn's. That is what makes the Anthropic
    /// prefix cache hit.
    ///
    /// With low=3, high=12 (batch = 9) the schedule is:
    ///
    /// | total screenshots | pruned | retained |
    /// |-------------------|--------|----------|
    /// | 0..=11            | 0      | 0..=11   |
    /// | 12                | 9      | 3        |  <- prune pass
    /// | 13..=20           | 9      | 4..=11   |
    /// | 21                | 18     | 3        |  <- prune pass
    ///
    /// i.e. one prune every 9 turns instead of one every turn.
    fn screenshots_to_prune(total: usize, low_water: usize, high_water: usize) -> usize {
        // A degenerate configuration must never prune, or the "batch" would be zero-sized and
        // we would be back to rewriting the prefix on every turn.
        let batch = high_water.saturating_sub(low_water);
        if batch == 0 || total < high_water {
            return 0;
        }
        // Quantise to whole batches: constant across the whole band, jumps by `batch` at each
        // prune point, and never prunes more than exist.
        let pruned = (total - low_water) / batch * batch;
        pruned.min(total.saturating_sub(low_water))
    }

    /// Batch-prune old screenshots out of the conversation history.
    ///
    /// Scans tool_result blocks for image content and replaces the oldest ones with a text
    /// placeholder — but only on the turns where `screenshots_to_prune` says a batch is due.
    /// On every other turn this is a no-op and the history stays append-only, which is what
    /// keeps Anthropic's prompt cache alive (see the module constants above).
    fn limit_screenshot_history(
        api_messages: &mut [ApiMessage],
        low_water: usize,
        high_water: usize,
    ) {
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
        let to_remove_count = Self::screenshots_to_prune(total_screenshots, low_water, high_water);
        if to_remove_count == 0 {
            // Below the high-water mark, or already pruned for this band: leave the history
            // untouched so the prefix stays byte-identical to the previous request.
            return;
        }
        let locations_to_remove = match screenshot_locations.get(..to_remove_count) {
            Some(slice) => slice,
            None => return,
        };

        log::info!(
            "Screenshot batch prune: {} screenshots in history (high-water {}), pruning {} oldest down to {} retained",
            total_screenshots, high_water, to_remove_count, total_screenshots - to_remove_count
        );

        // Second pass: replace old screenshots with text placeholders
        // Use all three indices (msg_idx, block_idx, rb_idx) to target the exact image block
        for &(msg_idx, block_idx, rb_idx) in locations_to_remove {
            if let Some(msg) = api_messages.get_mut(msg_idx) {
                if let ApiContent::Blocks(blocks) = &mut msg.content {
                    if let Some(block) = blocks.get_mut(block_idx) {
                        if let Some(ApiToolResultContent::Blocks(result_blocks)) =
                            &mut block.content
                        {
                            if let Some(result_block) = result_blocks.get_mut(rb_idx) {
                                *result_block = ApiToolResultBlock {
                                    block_type: "text".to_string(),
                                    source: None,
                                    text: Some(PRUNED_SCREENSHOT_PLACEHOLDER.to_string()),
                                };
                            }
                        }
                    }
                }
            }
        }
    }

    /// Place the message-level cache breakpoints (slots 3 and 4 of the four-breakpoint budget
    /// documented at the top of this file).
    ///
    /// Anthropic's guidance for computer use is to put a breakpoint on the *last* `tool_result`
    /// block of recent turns. Without these, `messages` — which holds every screenshot and every
    /// tool result, i.e. almost the entire payload — is reprocessed uncached on every turn even
    /// though the tools and system prompt are cached.
    ///
    /// Two breakpoints are placed:
    ///   * an anchor quantised to `CACHE_ANCHOR_STRIDE`, given the **1-hour** TTL. It stays on
    ///     the same message for a whole stride, so it is the entry that survives a user who
    ///     stops to think between voice commands;
    ///   * the latest tool_result turn, left at the default **5-minute** TTL, so the *next*
    ///     request gets a hit covering the whole history through this turn.
    ///
    /// The anchor is pushed first because it sits earlier in the prefix and the API requires
    /// every 1h breakpoint to precede every 5m one. When the two coincide — which happens
    /// exactly on the turns where the anchor advances — only the 1h breakpoint is written, and
    /// that is the one turn per stride on which the extended-TTL write premium is paid.
    fn apply_message_cache_breakpoints(api_messages: &mut [ApiMessage]) {
        // Indices of messages that contain at least one tool_result block, in order.
        let tool_result_msgs: Vec<usize> = api_messages
            .iter()
            .enumerate()
            .filter_map(|(idx, msg)| match &msg.content {
                ApiContent::Blocks(blocks) => blocks
                    .iter()
                    .any(|b| b.block_type == "tool_result")
                    .then_some(idx),
                ApiContent::Text(_) => None,
            })
            .collect();

        let Some(latest_pos) = tool_result_msgs.len().checked_sub(1) else {
            return; // No tool results yet — nothing worth a message breakpoint.
        };

        // Quantised anchor: advances only once every CACHE_ANCHOR_STRIDE tool-result turns.
        let anchor_pos = latest_pos / CACHE_ANCHOR_STRIDE * CACHE_ANCHOR_STRIDE;

        // (message index, extended TTL?). The anchor is pushed first so that, in prefix order,
        // the 1h breakpoint always precedes the 5m one — an API requirement, not a style choice.
        let mut targets: Vec<(usize, bool)> = Vec::with_capacity(MESSAGE_CACHE_BREAKPOINTS);
        if let Some(&msg_idx) = tool_result_msgs.get(anchor_pos) {
            targets.push((msg_idx, true));
        }
        if let Some(&msg_idx) = tool_result_msgs.get(latest_pos) {
            // When the anchor has just advanced onto the latest turn the two coincide; keep the
            // single breakpoint at 1h rather than downgrading it to 5m.
            if !targets.iter().any(|(idx, _)| *idx == msg_idx) {
                targets.push((msg_idx, false));
            }
        }
        // Belt and braces: never exceed the share of the 4-breakpoint budget reserved for
        // messages, whatever the stride arithmetic above does.
        targets.truncate(MESSAGE_CACHE_BREAKPOINTS);

        for (msg_idx, extended) in targets {
            if let Some(msg) = api_messages.get_mut(msg_idx) {
                if let ApiContent::Blocks(blocks) = &mut msg.content {
                    // The breakpoint goes on the LAST tool_result block of the turn, so the
                    // cached prefix covers the whole turn.
                    if let Some(block) = blocks
                        .iter_mut()
                        .filter(|b| b.block_type == "tool_result")
                        .next_back()
                    {
                        block.cache_control = Some(if extended {
                            CacheControl::ephemeral_extended()
                        } else {
                            CacheControl::ephemeral()
                        });
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
    /// Returns: (accumulated_text, tool_calls, stop_reason, stream_was_started, thinking_blocks)
    async fn handle_streaming_response<F>(
        &self,
        response: reqwest::Response,
        app_handle: Option<&tauri::AppHandle>,
        message_id: Option<String>,
        mut on_text_chunk: F,
    ) -> Result<(String, Vec<ToolCall>, String, bool, Vec<ApiContentBlock>), AgentError>
    where
        F: FnMut(String, Vec<String>) + Send, // Updated to accept multiple TTS extractions
    {
        let mut accumulated_text = String::new();
        let mut tool_calls = Vec::new();
        let mut stop_reason = String::new();

        // TTS XML parsing state (shared parser, see agent::tts_tags)
        let mut tts_stream = crate::agent::tts_tags::TtsTagStream::new();

        // Thinking XML parsing state (for <thinking> tags in text output)
        let mut thinking_buffer = String::new();
        let mut in_thinking_tag = false;
        let mut thinking_content = String::new();
        let mut thinking_message_id: Option<String> = None; // Track current thinking stream

        // Track whether we've started the main response stream
        // We delay stream_start until we have actual non-thinking text to display
        let mut response_stream_started = false;

        // Track content blocks and partial data
        // (id, name, partial_json, toolset_name)
        // `toolset_name` arrives on the `content_block_start` event alongside
        // the name and must survive until `content_block_stop`, where the call
        // is finalised — it is what routes a member tool onto the computer tool.
        let mut current_tool_call: Option<(String, String, String, Option<String>)> = None;

        // Track thinking content blocks (for extended thinking models via API)
        let mut current_thinking_content: Option<String> = None;
        let mut current_thinking_signature: Option<String> = None;
        let mut api_thinking_message_id: Option<String> = None; // For extended thinking API

        // Completed thinking / redacted_thinking blocks, in order, for replay on tool-use turns
        let mut thinking_blocks: Vec<ApiContentBlock> = Vec::new();

        // Get the response body as a stream
        let stream = response.bytes_stream();
        let reader = StreamReader::new(stream.map(|result| result.map_err(std::io::Error::other)));

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
                                            let toolset_name = content_block
                                                .get("toolset_name")
                                                .and_then(|v| v.as_str())
                                                .map(str::to_string);
                                            log::debug!(
                                                "Stream: started tool call {} ({}) toolset={:?}",
                                                name,
                                                id,
                                                toolset_name
                                            );
                                            current_tool_call =
                                                Some((id, name, String::new(), toolset_name));
                                        }
                                        "thinking" => {
                                            // Start tracking a thinking block (extended thinking models)
                                            log::debug!("Stream: started thinking block");
                                            current_thinking_content = Some(String::new());
                                            current_thinking_signature = None;
                                        }
                                        "redacted_thinking" => {
                                            // Opaque block: arrives complete, replayed verbatim
                                            let mut block =
                                                ApiContentBlock::empty("redacted_thinking");
                                            block.data = content_block
                                                .get("data")
                                                .and_then(|d| d.as_str())
                                                .map(|d| d.to_string());
                                            thinking_blocks.push(block);
                                        }
                                        _ => {
                                            log::debug!(
                                                "Stream: started content block type: {}",
                                                block_type
                                            );
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
                                                        let new_thinking_id =
                                                            uuid::Uuid::new_v4().to_string();
                                                        thinking_message_id =
                                                            Some(new_thinking_id.clone());
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
                                                        if let Some(ref msg_id) =
                                                            thinking_message_id
                                                        {
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
                                                let (display_text, extracted_tts_list) =
                                                    tts_stream.push(&text_without_thinking);
                                                for spoken in &extracted_tts_list {
                                                    log::info!(
                                                        "Extracted TTS content during streaming: '{}'",
                                                        spoken
                                                    );
                                                }

                                                // Emit chunks when we have display text OR TTS content.
                                                // We delay stream_start until after thinking messages for proper ordering,
                                                // but TTS-only responses still need streaming events so the frontend
                                                // can show the TTS content in the conversation.
                                                if !display_text.is_empty()
                                                    || !extracted_tts_list.is_empty()
                                                {
                                                    // Emit stream_start on first chunk (text or TTS-only)
                                                    if !response_stream_started {
                                                        if let (Some(handle), Some(ref msg_id)) =
                                                            (app_handle, &message_id)
                                                        {
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
                                                        let new_thinking_id =
                                                            uuid::Uuid::new_v4().to_string();
                                                        api_thinking_message_id =
                                                            Some(new_thinking_id.clone());
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
                                        "signature_delta" => {
                                            if let Some(signature) =
                                                delta.get("signature").and_then(|t| t.as_str())
                                            {
                                                current_thinking_signature =
                                                    Some(signature.to_string());
                                            }
                                        }
                                        "input_json_delta" => {
                                            if let Some(partial_json) =
                                                delta.get("partial_json").and_then(|t| t.as_str())
                                            {
                                                // Accumulate JSON for tool call
                                                if let Some((_, _, ref mut json_accumulator, _)) =
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
                            if let Some((id, name, json_str, toolset_name)) =
                                current_tool_call.take()
                            {
                                // Resolve the input first, then route once. The
                                // three ways an input can arrive (empty, parsed,
                                // unparseable) used to each push their own
                                // ToolCall, which meant three places to add the
                                // toolset routing and two of them easy to miss.
                                let input = if json_str.trim().is_empty() {
                                    log::debug!("Tool call {} ({}) has empty JSON input, using empty object", name, id);
                                    serde_json::json!({})
                                } else {
                                    match serde_json::from_str(&json_str) {
                                        Ok(input) => {
                                            log::debug!(
                                                "Stream: completed tool call with input: {}",
                                                json_str
                                            );
                                            input
                                        }
                                        Err(e) => {
                                            log::warn!("Failed to parse tool call input JSON: {}, json: '{}'. Using empty object as fallback.", e, json_str);
                                            serde_json::json!({})
                                        }
                                    }
                                };

                                // Dispatch on (name, toolset_name), exactly as
                                // the non-streaming path does.
                                let (name, input) =
                                    self.route_tool_call(name, toolset_name.as_deref(), input);
                                tool_calls.push(ToolCall { id, name, input });
                            }

                            // Emit thinking_end for API thinking blocks
                            if let Some(thinking_text) = current_thinking_content.take() {
                                // Keep the block (even when its text is empty, which is the
                                // default display mode) so it can be replayed unchanged.
                                let mut block = ApiContentBlock::empty("thinking");
                                block.thinking = Some(thinking_text.clone());
                                block.signature = current_thinking_signature.take();
                                thinking_blocks.push(block);

                                if !thinking_text.trim().is_empty() {
                                    log::debug!(
                                        "Stream: completed API thinking block with {} chars",
                                        thinking_text.len()
                                    );
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
                log::debug!(
                    "Emitting incomplete thinking content at stream end: {} chars",
                    thinking_content.len()
                );
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
                log::debug!(
                    "Adding remaining thinking buffer content to accumulated text: '{}'",
                    thinking_buffer
                );
                accumulated_text.push_str(&thinking_buffer);
            }

            thinking_buffer.clear();
        }

        // Flush any TTS state left at end of stream: an unterminated block is
        // still spoken, and a partial tag held in the buffer is shown as text.
        let (tail_display, tail_spoken) = tts_stream.finish();
        if !tail_spoken.is_empty() {
            log::warn!(
                "Stream ended inside a <TTS> block; speaking the partial content: {:?}",
                tail_spoken
            );
            on_text_chunk(String::new(), tail_spoken);
        }
        if !tail_display.trim().is_empty() {
            log::debug!(
                "Adding remaining buffer content to accumulated text: '{}'",
                tail_display
            );
            accumulated_text.push_str(&tail_display);
        }

        Ok((
            accumulated_text,
            tool_calls,
            stop_reason,
            response_stream_started,
            thinking_blocks,
        ))
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
                } else if remaining_len < 10
                    && self.could_be_partial_thinking_opening_tag(&remaining_str)
                {
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
                    log::debug!(
                        "Thinking block ended, total content: {} chars",
                        thinking_content.len()
                    );

                    // Reset thinking state for next potential block
                    *in_thinking_tag = false;
                    i += 11; // Skip "</thinking>"
                    chars_to_consume = i;
                    continue;
                } else if remaining_len < 11
                    && self.could_be_partial_thinking_closing_tag(&remaining_str)
                {
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
            log::error!(
                "CRITICAL BUG: thinking tags found in output_text during streaming processing!"
            );
            output_text = output_text
                .replace("<thinking>", "")
                .replace("</thinking>", "");
        }

        (
            output_text,
            thinking_started,
            thinking_ended,
            thinking_chunk,
        )
    }

    /// Check if a string could be the beginning of a partial "<thinking>" tag
    fn could_be_partial_thinking_opening_tag(&self, s: &str) -> bool {
        if s.is_empty() {
            return false;
        }
        let partial_tags = [
            "<",
            "<t",
            "<th",
            "<thi",
            "<thin",
            "<think",
            "<thinki",
            "<thinkin",
            "<thinking",
        ];
        partial_tags.iter().any(|&tag| s.eq_ignore_ascii_case(tag))
    }

    /// Check if a string could be the beginning of a partial "</thinking>" tag
    fn could_be_partial_thinking_closing_tag(&self, s: &str) -> bool {
        if s.is_empty() {
            return false;
        }
        let partial_tags = [
            "<",
            "</",
            "</t",
            "</th",
            "</thi",
            "</thin",
            "</think",
            "</thinki",
            "</thinkin",
            "</thinking",
        ];
        partial_tags.iter().any(|&tag| s.eq_ignore_ascii_case(tag))
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
    fn convert_message_to_api(&self, message: &Message) -> Result<ApiMessage, AgentError> {
        let role_str = match message.role {
            Role::User => "user".to_string(),
            Role::Assistant => "assistant".to_string(),
            Role::System => return Err(AgentError::LlmError("System messages should be passed via the 'system' parameter, not in the messages list.".to_string())),
            Role::Tool => return Err(AgentError::LlmError("Tool result messages need special handling for Anthropic API format.".to_string())), // Needs specific handling
        };

        let mut content_blocks = Vec::new();

        // Replay the thinking blocks that preceded this turn's tool calls. The API rejects a
        // tool-use assistant turn whose thinking blocks were dropped or edited.
        if message.role == Role::Assistant {
            if let Some(tool_calls) = &message.tool_calls {
                content_blocks.extend(self.recall_thinking(tool_calls));
            }
        }

        // Images first: a turn reads as "here is a picture, now do this", and
        // the API pays attention to order.
        if message.role == Role::User {
            if let Some(images) = &message.images {
                for url in images {
                    let Some((media_type, data)) = parse_data_url(url) else {
                        tracing::warn!(
                            "Dropping an attachment that is not a base64 image data URL"
                        );
                        continue;
                    };
                    let mut block = ApiContentBlock::empty("image");
                    block.source = Some(ApiImageSource {
                        source_type: "base64".to_string(),
                        media_type,
                        data,
                    });
                    content_blocks.push(block);
                }
            }
        }

        // Add text content if present
        if !message.content.is_empty() {
            content_blocks.push(ApiContentBlock {
                block_type: "text".to_string(),
                text: Some(message.content.clone()),
                id: None,
                name: None,
                input: None,
                toolset_name: None,
                tool_use_id: None,
                content: None,
                thinking: None,
                signature: None,
                data: None,
                source: None,
                cache_control: None,
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
                // Undo the toolset routing before replay. Claude sent this as a
                // member name plus `toolset_name`; that is what has to go back,
                // not the `computer` shape Juno executes internally.
                let (name, toolset_name, input) =
                    crate::agent::tools::anthropic_computer_use::unroute_toolset_call(
                        &tool_call.name,
                        &tool_call.input,
                    );
                content_blocks.push(ApiContentBlock {
                    block_type: "tool_use".to_string(),
                    id: Some(tool_call.id.clone()),
                    name: Some(name),
                    input: Some(input),
                    toolset_name,
                    text: None,
                    tool_use_id: None,
                    content: None,
                    thinking: None,
                    signature: None,
                    data: None,
                    source: None,
                    cache_control: None,
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
    use chrono::Utc;
    use std::fs;
    use std::path::PathBuf;

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
                log::warn!(
                    "Failed to write debug request to {}: {}",
                    filepath.display(),
                    e
                );
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
        self.decide_next_action_streaming(messages, available_tools, None, None, None)
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
        // Cancellation is enforced by AgentRunner between steps for the HTTP
        // provider; only subprocess-based brains (Claude CLI) consume this.
        _cancel_rx: Option<crate::state::CancelReceiver>,
    ) -> Result<AgentAction, AgentError> {
        // --- 1. Prepare API Request ---
        let mut api_messages = Vec::new();

        // Track tool calls that need results to validate message ordering
        let mut pending_tool_calls: Vec<String> = Vec::new();
        let mut resolved_tool_calls: std::collections::HashSet<String> =
            std::collections::HashSet::new();

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
                    match self.convert_message_to_api(message) {
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
                    let result_content: ApiToolResultContent = match serde_json::from_str::<
                        serde_json::Value,
                    >(
                        &tool_result_content
                    ) {
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
                                ApiToolResultContent::Text(format!("Error: {}", error.trim()))
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
                            ApiToolResultContent::Text(tool_result_content.trim().to_string())
                        }
                    };

                    // Echo `toolset_name` on results for calls that came from a
                    // toolset. The API requires it on every `tool_result` whose
                    // `tool_use` carried one, and a missing echo is an
                    // `invalid_request_error` for the whole request, not a
                    // degraded single result.
                    //
                    // Model + tool name is an exact test, not a guess: a model
                    // on the toolset path is never sent the legacy `computer`
                    // tool, so every `computer` result it produces belongs to
                    // the toolset. On the legacy path this stays `None` and
                    // `skip_serializing_if` keeps the field off the wire.
                    let result_toolset_name = if tool_name == "computer" {
                        self.computer_toolset_name().map(str::to_string)
                    } else {
                        None
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
                            toolset_name: result_toolset_name,
                            content: Some(result_content),
                            thinking: None,
                            signature: None,
                            data: None,
                            source: None,
                            cache_control: None, // Set by apply_message_cache_breakpoints below
                        }]),
                    });
                }
                Role::User => {
                    // Convert user message normally
                    match self.convert_message_to_api(message) {
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

        log::info!(
            "Conversation validation passed: {} messages prepared for Anthropic API",
            api_messages.len()
        );

        // --- Screenshot History Pruning (batched, cache-prefix safe) ---
        // Prune the oldest screenshots in one batch when the retained count reaches the
        // high-water mark, then leave the history alone until the next batch is due. Between
        // prunes the message array is append-only, which is the precondition for Anthropic's
        // exact-prefix prompt cache to hit. See the constants at the top of this file.
        Self::limit_screenshot_history(
            &mut api_messages,
            SCREENSHOT_PRUNE_LOW_WATER,
            SCREENSHOT_PRUNE_HIGH_WATER,
        );

        // --- Message Cache Breakpoints ---
        // Breakpoints 3 and 4 of the 4-breakpoint budget (1 = last tool, 2 = system prompt).
        // Must run AFTER pruning so a breakpoint never lands on bytes that pruning then edits.
        Self::apply_message_cache_breakpoints(&mut api_messages);

        let api_tools = if available_tools.is_empty() {
            None
        } else {
            let mut tools: Vec<ApiTool> = available_tools
                .iter()
                .filter_map(|t| {
                    if let Some(api_type) = &t.api_type {
                        // The computer *toolset* replaces the computer *tool*.
                        // It is a different entry shape, not a different type
                        // string on the same shape, so it forks first — before
                        // any of the display-dimension work below, which the
                        // toolset must not carry.
                        //
                        // Only the `computer` tool is replaced. `bash` and
                        // `str_replace_based_edit_tool` keep their own named
                        // entries and fall through to `BuiltIn` unchanged.
                        if t.name == "computer" && self.uses_computer_toolset() {
                            // Juno still needs a known display size to map the
                            // model's screenshot-pixel coordinates back onto the
                            // real screen. The toolset does not take the size,
                            // but Juno's own coordinate transform does, and a
                            // wrong transform clicks the wrong place — so the
                            // same guard as the legacy path applies.
                            match crate::utils::coordinates::get_current_standard_resolution() {
                                Ok((w, h)) if w > 0 && h > 0 => {
                                    log::info!(
                                        "Computer toolset enabled for model {} (screen {}x{}, no display dims sent — \
                                         the toolset takes none and Juno sizes its own screenshots)",
                                        self.model, w, h
                                    );
                                }
                                Ok((w, h)) => {
                                    log::warn!(
                                        "Standard resolution not yet initialized ({}x{}), skipping computer toolset to prevent coordinate mismatch",
                                        w, h
                                    );
                                    return None;
                                }
                                Err(e) => {
                                    log::warn!(
                                        "Cannot determine display resolution: {}, skipping computer toolset to prevent coordinate mismatch",
                                        e
                                    );
                                    return None;
                                }
                            }
                            return Some(ApiTool::Toolset {
                                tool_type: self.resolve_tool_api_type(&t.name, api_type),
                                // Every member keeps its default, zoom included.
                                configs: None,
                                cache_control: None, // Set on last tool below
                            });
                        }
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
            // Breakpoint 1 of 4, 1-hour TTL (see the cache breakpoint budget at the top of this
            // file). Add cache_control to the last tool to enable prompt caching of the tool
            // definitions. When Anthropic caches tools, subsequent turns skip re-processing the
            // tool definitions, reducing latency by 50-80% for the cached portion. The tool list
            // is byte-identical on every turn of every session, so this is the single strongest
            // candidate for the extended TTL: written once, read for an hour, and it survives the
            // user closing and reopening the app.
            //
            // This matters more on the toolset path, and the breakpoint must keep covering it.
            // Measured against the live API on otherwise-identical one-token requests:
            //
            //   computer_toolset_20260801 on opus-5-5   4,532 input tokens
            //   computer_20251124 on fable-5-1          2,154 input tokens
            //
            // The 17 member definitions are what roughly doubles the overhead, and they are
            // paid on every uncached request. Prompt caching is a prefix cache, so a breakpoint
            // on the LAST tool covers every tool before it, the toolset entry included.
            // `ApiTool::Toolset` also carries its own `cache_control` so a breakpoint can be
            // placed directly on it. Do not move the toolset entry after the final breakpoint,
            // and do not drop that field — LAC-4007 builds the real caching strategy on top of
            // this, and either change would architect it out.
            if let Some(last_tool) = tools.last_mut() {
                match last_tool {
                    ApiTool::BuiltIn { cache_control, .. }
                    | ApiTool::Toolset { cache_control, .. }
                    | ApiTool::Custom { cache_control, .. } => {
                        *cache_control = Some(CacheControl::ephemeral_extended());
                    }
                }
            }
            if tools.is_empty() {
                None
            } else {
                Some(tools)
            }
        };

        // Breakpoint 2 of 4, 1-hour TTL (see the cache breakpoint budget at the top of this
        // file). Convert system prompt to content block array with cache_control for prompt
        // caching. Like the tool list, the system prompt is stable across every turn, so the
        // extended TTL is written once and read for an hour.
        let system_blocks = self.system_prompt.as_ref().map(|prompt| {
            vec![SystemContentBlock {
                block_type: "text".to_string(),
                text: prompt.clone(),
                cache_control: Some(CacheControl::ephemeral_extended()),
            }]
        });

        let mut request_payload = AnthropicRequest {
            model: self.model.clone(),
            messages: api_messages,
            tools: api_tools,
            system: system_blocks,
            max_tokens: self.max_tokens,
            stream: None,      // Will be set based on streaming mode
            tool_choice: None, // Add tool choice support
            thinking: self.thinking_param(),
            fallbacks: self.fallbacks_param(),
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
            // Computer use beta + prompt caching (+ server-side fallbacks on supported models),
            // comma-separated in a single header. Prompt caching reduces input token costs by
            // ~90% and latency by ~50-80% for the stable system prompt and tool definitions.
            .header("anthropic-beta", self.beta_header_value())
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
            return Err(AgentError::LlmError(
                Self::format_anthropic_http_error_for_user(
                    status,
                    &error_body,
                    crate::demo::is_demo_key(&self.api_key),
                ),
            ));
        }

        // --- 4. Handle Response (Streaming or Non-Streaming) ---
        if use_streaming {
            // Handle streaming response
            let app_handle = app_handle.ok_or("AppHandle required for streaming")?;
            let message_id = message_id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

            // Note: stream_start is now emitted inside handle_streaming_response
            // when we have actual non-thinking text to display. This ensures thinking
            // messages appear BEFORE the response message in the chat.

            let (accumulated_text, tool_calls, stop_reason, stream_was_started, thinking_blocks) =
                self.handle_streaming_response(
                    response,
                    Some(&app_handle),
                    Some(message_id.clone()),
                    |chunk, tts_list| {
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
                    },
                )
                .await?;

            // Clean up any remaining thinking tags from the accumulated text
            let mut accumulated_text = accumulated_text;
            if accumulated_text.contains("<thinking>") || accumulated_text.contains("</thinking>") {
                log::warn!("Thinking tags found in final accumulated text - cleaning up");
                accumulated_text = self.strip_thinking_tags(&accumulated_text);
            }

            // TTS tags are removed during streaming. If any remain it is a parser
            // bug; strip them so the chat never shows raw markup.
            if crate::agent::tts_tags::contains_tts_tags(&accumulated_text) {
                log::error!(
                    "TTS tags survived streaming extraction, stripping them: '{}'",
                    accumulated_text
                );
                let (display, _) = crate::agent::tts_tags::split_tts_tags(&accumulated_text);
                accumulated_text = display;
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
                        self.remember_thinking(&tool_calls, thinking_blocks);
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
                "refusal" => {
                    // Safety classifiers declined the request (HTTP 200). Any partial output
                    // is not a complete answer, so present a clear message instead.
                    log::warn!("Anthropic returned stop_reason=refusal; discarding partial output");
                    Ok(AgentAction::Finish(REFUSAL_MESSAGE.to_string()))
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
            let mut thinking_blocks: Vec<ApiContentBlock> = Vec::new();

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

                        // Dispatch on (name, toolset_name). A toolset member is
                        // routed onto the computer tool; anything else passes
                        // through untouched.
                        let (name, input) =
                            self.route_tool_call(name, block.toolset_name.as_deref(), input);

                        // Add to the list of tool calls to execute
                        tool_calls_to_execute.push(ToolCall { id, name, input });
                    }
                    "thinking" | "redacted_thinking" => {
                        // Kept verbatim for replay on the next request
                        thinking_blocks.push(block.clone());
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
                        self.remember_thinking(&tool_calls_to_execute, thinking_blocks);
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
                "refusal" => {
                    log::warn!("Anthropic returned stop_reason=refusal; discarding partial output");
                    Ok(AgentAction::Finish(REFUSAL_MESSAGE.to_string()))
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

#[cfg(test)]
mod computer_toolset_request_tests {
    use super::*;
    use crate::agent::providers::types::model_ids;
    use crate::agent::tools::anthropic_computer_use::{route_toolset_call, unroute_toolset_call};

    fn brain(model: &str) -> AnthropicBrain {
        AnthropicBrain::new("test-key".to_string(), Some(model.to_string()), None, None)
            .expect("brain construction only builds an HTTP client")
    }

    fn to_json(tool: &ApiTool) -> Value {
        serde_json::to_value(tool).expect("ApiTool serializes")
    }

    // --- The toolset request entry ---

    /// The single most breakable part of the contract: the toolset entry is
    /// `{"type": ...}` and nothing else. A stray `name` makes the API reject
    /// every request, which is worse than not shipping the port at all.
    #[test]
    fn toolset_entry_is_type_only_and_carries_no_name() {
        let json = to_json(&ApiTool::Toolset {
            tool_type: "computer_toolset_20260801".to_string(),
            configs: None,
            cache_control: None,
        });

        assert_eq!(
            json,
            serde_json::json!({"type": "computer_toolset_20260801"})
        );

        let obj = json.as_object().expect("object");
        assert!(!obj.contains_key("name"), "a toolset must not carry a name");
        assert!(!obj.contains_key("display_width_px"));
        assert!(!obj.contains_key("display_height_px"));
        assert!(!obj.contains_key("enable_zoom"));
        assert!(
            !obj.contains_key("configs"),
            "omitted when every member is default"
        );
    }

    #[test]
    fn toolset_entry_includes_configs_only_when_set() {
        let json = to_json(&ApiTool::Toolset {
            tool_type: "computer_toolset_20260801".to_string(),
            configs: Some(serde_json::json!({"zoom": {"enabled": false}})),
            cache_control: None,
        });
        assert_eq!(
            json,
            serde_json::json!({
                "type": "computer_toolset_20260801",
                "configs": {"zoom": {"enabled": false}}
            })
        );
        assert!(!json.as_object().expect("object").contains_key("name"));
    }

    // --- Regression guard: the legacy entry is byte-identical to today ---

    /// The safety net for every model that is NOT toolset-GA. These blobs are
    /// what Juno sent before the toolset existed; if this test ever needs
    /// updating, a legacy model's requests have changed, and that is a
    /// regression rather than a refactor.
    #[test]
    fn legacy_computer_entry_is_unchanged() {
        let json = to_json(&ApiTool::BuiltIn {
            tool_type: "computer_20251124".to_string(),
            name: "computer".to_string(),
            display_width_px: Some(1920),
            display_height_px: Some(1080),
            enable_zoom: Some(true),
            cache_control: None,
        });
        assert_eq!(
            json,
            serde_json::json!({
                "type": "computer_20251124",
                "name": "computer",
                "display_width_px": 1920,
                "display_height_px": 1080,
                "enable_zoom": true
            })
        );

        let older = to_json(&ApiTool::BuiltIn {
            tool_type: "computer_20250124".to_string(),
            name: "computer".to_string(),
            display_width_px: Some(1280),
            display_height_px: Some(800),
            enable_zoom: None,
            cache_control: None,
        });
        assert_eq!(
            older,
            serde_json::json!({
                "type": "computer_20250124",
                "name": "computer",
                "display_width_px": 1280,
                "display_height_px": 800
            })
        );
    }

    /// bash and the text editor are untouched by the toolset — they stay named
    /// entries in the same array on both paths.
    #[test]
    fn bash_and_editor_entries_are_unchanged_on_both_paths() {
        for (tool_type, name) in [
            ("bash_20250124", "bash"),
            ("text_editor_20250728", "str_replace_based_edit_tool"),
        ] {
            let json = to_json(&ApiTool::BuiltIn {
                tool_type: tool_type.to_string(),
                name: name.to_string(),
                display_width_px: None,
                display_height_px: None,
                enable_zoom: None,
                cache_control: None,
            });
            assert_eq!(json, serde_json::json!({"type": tool_type, "name": name}));
        }
    }

    // --- Per-model path selection, from the one capability table ---

    #[test]
    fn toolset_only_models_select_the_toolset_path() {
        // Opus 5.5 accepts no earlier tool type, so it is the one model worth
        // the toolset's ~2x input-token overhead. Every other toolset-GA model
        // stays on `computer_20251124` and is covered by the test below.
        let model = model_ids::CLAUDE_OPUS_5_5;
        let b = brain(model);
        assert!(b.uses_computer_toolset(), "{model} should use the toolset");
        assert_eq!(b.computer_toolset_name(), Some("computer"));
        assert_eq!(
            b.resolve_tool_api_type("computer", "computer_20251124"),
            "computer_toolset_20260801",
            "{model} should resolve the computer tool to the toolset type"
        );
    }

    #[test]
    fn legacy_models_keep_their_single_computer_tool() {
        for (model, expected) in [
            // Toolset-GA but cheaper on the legacy tool, so Juno sends that.
            (model_ids::CLAUDE_FABLE_5_1, "computer_20251124"),
            (model_ids::CLAUDE_SONNET_5, "computer_20251124"),
            (model_ids::CLAUDE_OPUS_5, "computer_20251124"),
            (model_ids::CLAUDE_FABLE_5, "computer_20251124"),
            (model_ids::CLAUDE_OPUS_4_8, "computer_20251124"),
            // Never toolset-GA at all.
            (model_ids::CLAUDE_OPUS_4_7, "computer_20251124"),
            (model_ids::CLAUDE_OPUS_4_6, "computer_20251124"),
            (model_ids::CLAUDE_SONNET_4_6, "computer_20251124"),
            (model_ids::CLAUDE_OPUS_4_5, "computer_20251124"),
            (model_ids::CLAUDE_SONNET_4_5, "computer_20250124"),
            (model_ids::CLAUDE_HAIKU_4_5, "computer_20250124"),
        ] {
            let b = brain(model);
            assert!(
                !b.uses_computer_toolset(),
                "{model} must stay on the legacy path"
            );
            assert_eq!(b.computer_toolset_name(), None);
            assert_eq!(b.resolve_tool_api_type("computer", expected), expected);
        }
    }

    // --- Beta headers ---

    /// GA means no computer-use beta flag. The header still carries the
    /// unrelated flags (prompt caching, and server-side fallback where the
    /// model supports it), so this asserts the absence of the computer-use one
    /// rather than an exact string.
    #[test]
    fn toolset_models_send_no_computer_use_beta_flag() {
        let model = model_ids::CLAUDE_OPUS_5_5;
        let b = brain(model);
        assert_eq!(b.resolve_computer_use_beta_header(), None, "{model}");
        let header = b.beta_header_value();
        assert!(
            !header.contains("computer-use"),
            "{model} sent a computer-use beta flag in {header:?}"
        );
        assert!(
            header.contains(crate::constants::api::beta_flags::PROMPT_CACHING),
            "{model} lost prompt caching"
        );
    }

    /// The legacy regression guard for headers: each older model still sends
    /// exactly the flag it sent before, still leading the header.
    #[test]
    fn legacy_models_keep_their_exact_beta_flags() {
        for (model, flag) in [
            (model_ids::CLAUDE_OPUS_4_7, "computer-use-2025-11-24"),
            (model_ids::CLAUDE_OPUS_4_6, "computer-use-2025-11-24"),
            (model_ids::CLAUDE_SONNET_4_6, "computer-use-2025-11-24"),
            (model_ids::CLAUDE_OPUS_4_5, "computer-use-2025-11-24"),
            (model_ids::CLAUDE_SONNET_4_5, "computer-use-2025-01-24"),
            (model_ids::CLAUDE_HAIKU_4_5, "computer-use-2025-01-24"),
        ] {
            let b = brain(model);
            assert_eq!(b.resolve_computer_use_beta_header(), Some(flag), "{model}");
            assert!(
                b.beta_header_value().starts_with(flag),
                "{model} header was {:?}",
                b.beta_header_value()
            );
        }
    }

    // --- tool_result echo ---

    #[test]
    fn tool_result_omits_toolset_name_on_the_legacy_path() {
        let mut block = ApiContentBlock::empty("tool_result");
        block.tool_use_id = Some("toolu_1".to_string());
        block.content = Some(ApiToolResultContent::Text("OK".to_string()));
        let json = serde_json::to_value(&block).expect("serializes");
        assert!(
            !json
                .as_object()
                .expect("object")
                .contains_key("toolset_name"),
            "legacy tool_result must not gain a toolset_name field"
        );
    }

    #[test]
    fn tool_result_echoes_toolset_name_on_the_toolset_path() {
        let mut block = ApiContentBlock::empty("tool_result");
        block.tool_use_id = Some("toolu_1".to_string());
        block.toolset_name = Some("computer".to_string());
        block.content = Some(ApiToolResultContent::Text("OK".to_string()));
        let json = serde_json::to_value(&block).expect("serializes");
        assert_eq!(json["toolset_name"], serde_json::json!("computer"));
    }

    // --- Replay round trip ---

    /// A routed call must go back out as the member name plus `toolset_name`,
    /// not as the internal `computer` shape. This round trip is what keeps
    /// multi-turn computer use working.
    #[test]
    fn replaying_a_routed_call_restores_the_member_shape() {
        let (name, input) = route_toolset_call(
            "left_click",
            Some("computer"),
            &serde_json::json!({"coordinate": [512, 742]}),
        )
        .expect("routes");
        assert_eq!(name, "computer");

        let (replay_name, toolset_name, replay_input) = unroute_toolset_call(&name, &input);

        assert_eq!(replay_name, "left_click");
        assert_eq!(toolset_name.as_deref(), Some("computer"));
        assert_eq!(replay_input, serde_json::json!({"coordinate": [512, 742]}));
    }

    #[test]
    fn replaying_a_legacy_call_changes_nothing() {
        let input = serde_json::json!({"action": "left_click", "coordinate": [10, 20]});
        let (name, toolset_name, out) = unroute_toolset_call("computer", &input);
        assert_eq!(name, "computer");
        assert_eq!(toolset_name, None);
        assert_eq!(out, input, "legacy replay must be byte-identical");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- Fixtures --------------------------------------------------------------------------

    fn image_block(seed: usize) -> ApiToolResultBlock {
        ApiToolResultBlock {
            block_type: "image".to_string(),
            source: Some(ApiImageSource {
                source_type: "base64".to_string(),
                media_type: "image/jpeg".to_string(),
                data: format!("BASE64_SCREENSHOT_PAYLOAD_{}", seed),
            }),
            text: None,
        }
    }

    /// One `user`/tool_result turn carrying a screenshot plus its sibling text output.
    fn screenshot_turn(seed: usize) -> ApiMessage {
        let mut block = ApiContentBlock::empty("tool_result");
        block.tool_use_id = Some(format!("toolu_{}", seed));
        block.content = Some(ApiToolResultContent::Blocks(vec![
            image_block(seed),
            ApiToolResultBlock {
                block_type: "text".to_string(),
                source: None,
                text: Some(format!("screenshot output {}", seed)),
            },
        ]));
        ApiMessage {
            role: "user".to_string(),
            content: ApiContent::Blocks(vec![block]),
        }
    }

    /// The assistant turn that requested the screenshot.
    fn assistant_turn(seed: usize) -> ApiMessage {
        let mut block = ApiContentBlock::empty("tool_use");
        block.id = Some(format!("toolu_{}", seed));
        block.name = Some("computer".to_string());
        block.input = Some(serde_json::json!({ "action": "screenshot" }));
        ApiMessage {
            role: "assistant".to_string(),
            content: ApiContent::Blocks(vec![block]),
        }
    }

    /// A conversation after `turns` screenshot turns. Deterministic: rebuilding it with a
    /// larger `turns` reproduces exactly the same leading messages, which is precisely how the
    /// real provider rebuilds the API payload from `Message` history on every request.
    fn conversation(turns: usize) -> Vec<ApiMessage> {
        let mut msgs = vec![ApiMessage {
            role: "user".to_string(),
            content: ApiContent::Text("open safari and find the docs".to_string()),
        }];
        for seed in 0..turns {
            msgs.push(assistant_turn(seed));
            msgs.push(screenshot_turn(seed));
        }
        msgs
    }

    fn count_images(msgs: &[ApiMessage]) -> usize {
        let mut n = 0;
        for msg in msgs {
            if let ApiContent::Blocks(blocks) = &msg.content {
                for block in blocks {
                    if let Some(ApiToolResultContent::Blocks(rbs)) = &block.content {
                        n += rbs
                            .iter()
                            .filter(|rb| rb.block_type == "image" && rb.source.is_some())
                            .count();
                    }
                }
            }
        }
        n
    }

    fn count_placeholders(msgs: &[ApiMessage]) -> usize {
        let mut n = 0;
        for msg in msgs {
            if let ApiContent::Blocks(blocks) = &msg.content {
                for block in blocks {
                    if let Some(ApiToolResultContent::Blocks(rbs)) = &block.content {
                        n += rbs
                            .iter()
                            .filter(|rb| rb.text.as_deref() == Some(PRUNED_SCREENSHOT_PLACEHOLDER))
                            .count();
                    }
                }
            }
        }
        n
    }

    /// Message indices carrying a `cache_control` breakpoint.
    fn breakpoint_indices(msgs: &[ApiMessage]) -> Vec<usize> {
        msgs.iter()
            .enumerate()
            .filter_map(|(idx, msg)| match &msg.content {
                ApiContent::Blocks(blocks) => blocks
                    .iter()
                    .any(|b| b.cache_control.is_some())
                    .then_some(idx),
                ApiContent::Text(_) => None,
            })
            .collect()
    }

    /// Every breakpoint as (message index, ttl), in prefix order.
    fn breakpoints(msgs: &[ApiMessage]) -> Vec<(usize, Option<String>)> {
        let mut out = Vec::new();
        for (idx, msg) in msgs.iter().enumerate() {
            if let ApiContent::Blocks(blocks) = &msg.content {
                for block in blocks {
                    if let Some(cc) = &block.cache_control {
                        out.push((idx, cc.ttl.clone()));
                    }
                }
            }
        }
        out
    }

    fn serialize_each(msgs: &[ApiMessage]) -> Vec<String> {
        msgs.iter()
            .map(|m| serde_json::to_string(m).unwrap_or_default())
            .collect()
    }

    // --- Scheduler: below the high-water mark, nothing is pruned ---------------------------

    #[test]
    fn prune_scheduler_is_a_no_op_below_high_water() {
        for total in 0..SCREENSHOT_PRUNE_HIGH_WATER {
            assert_eq!(
                AnthropicBrain::screenshots_to_prune(
                    total,
                    SCREENSHOT_PRUNE_LOW_WATER,
                    SCREENSHOT_PRUNE_HIGH_WATER
                ),
                0,
                "expected no prune at total={}",
                total
            );
        }
    }

    #[test]
    fn prune_leaves_history_untouched_below_high_water() {
        let turns = SCREENSHOT_PRUNE_HIGH_WATER - 1;
        let before = conversation(turns);
        let mut after = conversation(turns);
        AnthropicBrain::limit_screenshot_history(
            &mut after,
            SCREENSHOT_PRUNE_LOW_WATER,
            SCREENSHOT_PRUNE_HIGH_WATER,
        );
        assert_eq!(serialize_each(&before), serialize_each(&after));
        assert_eq!(count_images(&after), turns);
        assert_eq!(count_placeholders(&after), 0);
    }

    // --- Scheduler: crossing the high-water mark batch-prunes to the low-water mark ---------

    #[test]
    fn prune_scheduler_batches_down_to_low_water() {
        let low = SCREENSHOT_PRUNE_LOW_WATER;
        let high = SCREENSHOT_PRUNE_HIGH_WATER;
        let batch = high - low;

        // First prune point: the turn the count reaches the high-water mark.
        let pruned = AnthropicBrain::screenshots_to_prune(high, low, high);
        assert_eq!(pruned, batch);
        assert_eq!(
            high - pruned,
            low,
            "one batch must land exactly on low water"
        );

        // Second prune point, one batch later.
        let second = high + batch;
        let pruned = AnthropicBrain::screenshots_to_prune(second, low, high);
        assert_eq!(pruned, batch * 2);
        assert_eq!(second - pruned, low);
    }

    #[test]
    fn prune_batch_replaces_oldest_images_only() {
        let high = SCREENSHOT_PRUNE_HIGH_WATER;
        let low = SCREENSHOT_PRUNE_LOW_WATER;
        let mut msgs = conversation(high);
        AnthropicBrain::limit_screenshot_history(&mut msgs, low, high);

        assert_eq!(count_images(&msgs), low, "retained images");
        assert_eq!(count_placeholders(&msgs), high - low, "pruned images");

        // The sibling text block of every turn survives untouched — only the image block is
        // replaced, and only inside tool_result blocks.
        let mut text_outputs = 0;
        for msg in &msgs {
            if let ApiContent::Blocks(blocks) = &msg.content {
                for block in blocks {
                    if let Some(ApiToolResultContent::Blocks(rbs)) = &block.content {
                        text_outputs += rbs
                            .iter()
                            .filter(|rb| {
                                rb.text
                                    .as_deref()
                                    .is_some_and(|t| t.starts_with("screenshot output "))
                            })
                            .count();
                    }
                }
            }
        }
        assert_eq!(text_outputs, high);

        // The images that survived are the most recent ones.
        let surviving: Vec<String> = msgs
            .iter()
            .filter_map(|msg| match &msg.content {
                ApiContent::Blocks(blocks) => Some(blocks),
                ApiContent::Text(_) => None,
            })
            .flat_map(|blocks| blocks.iter())
            .filter_map(|b| match &b.content {
                Some(ApiToolResultContent::Blocks(rbs)) => Some(rbs),
                _ => None,
            })
            .flat_map(|rbs| rbs.iter())
            .filter_map(|rb| rb.source.as_ref().map(|s| s.data.clone()))
            .collect();
        let expected: Vec<String> = ((high - low)..high)
            .map(|seed| format!("BASE64_SCREENSHOT_PAYLOAD_{}", seed))
            .collect();
        assert_eq!(surviving, expected);
    }

    #[test]
    fn prune_scheduler_holds_steady_between_prune_points() {
        let low = SCREENSHOT_PRUNE_LOW_WATER;
        let high = SCREENSHOT_PRUNE_HIGH_WATER;
        let batch = high - low;
        for total in high..(high + batch) {
            assert_eq!(
                AnthropicBrain::screenshots_to_prune(total, low, high),
                batch,
                "prune count must not move within a band (total={})",
                total
            );
        }
    }

    #[test]
    fn prune_scheduler_respects_its_water_marks() {
        let low = SCREENSHOT_PRUNE_LOW_WATER;
        let high = SCREENSHOT_PRUNE_HIGH_WATER;
        for total in 0..200usize {
            let pruned = AnthropicBrain::screenshots_to_prune(total, low, high);
            assert!(
                pruned <= total,
                "cannot prune more than exist (total={})",
                total
            );
            let retained = total - pruned;
            assert!(
                retained < high,
                "retained {} must stay under high water",
                retained
            );
            if total >= low {
                assert!(
                    retained >= low,
                    "retained {} must stay at or above low water",
                    retained
                );
            }
        }
    }

    #[test]
    fn prune_scheduler_never_prunes_on_a_degenerate_configuration() {
        // A zero-sized batch would put us back to per-turn pruning, which is the bug this
        // whole scheduler exists to prevent.
        assert_eq!(AnthropicBrain::screenshots_to_prune(100, 5, 5), 0);
        assert_eq!(AnthropicBrain::screenshots_to_prune(100, 9, 5), 0);
    }

    // --- The append-only property (the reason any of this exists) ---------------------------

    #[test]
    fn message_prefix_is_byte_identical_between_prunes() {
        let low = SCREENSHOT_PRUNE_LOW_WATER;
        let high = SCREENSHOT_PRUNE_HIGH_WATER;
        let batch = high - low;

        let mut previous: Option<Vec<String>> = None;
        let mut turns_that_mutated_the_prefix: Vec<usize> = Vec::new();

        for turns in 1..=(high + batch + 3) {
            let mut msgs = conversation(turns);
            AnthropicBrain::limit_screenshot_history(&mut msgs, low, high);
            let serialized = serialize_each(&msgs);
            assert!(
                serialized.iter().all(|s| !s.is_empty()),
                "fixture failed to serialize"
            );

            if let Some(prev) = &previous {
                // Each turn appends exactly one assistant message and one tool_result message.
                assert_eq!(
                    serialized.len(),
                    prev.len() + 2,
                    "turn {} should append two messages",
                    turns
                );
                // Every byte of every previously-sent message must be unchanged, otherwise
                // Anthropic's exact-prefix cache is dead from the first differing message.
                if prev
                    .iter()
                    .zip(serialized.iter())
                    .any(|(before, after)| before != after)
                {
                    turns_that_mutated_the_prefix.push(turns);
                }
            }
            previous = Some(serialized);
        }

        // Exactly two prune passes in this window, and nothing else ever rewrites the prefix.
        // Under the old per-turn scheme this vector would have contained every turn from the
        // fourth onward.
        assert_eq!(turns_that_mutated_the_prefix, vec![high, high + batch]);
    }

    #[test]
    fn pruned_placeholder_carries_no_turn_dependent_text() {
        // Any number in the placeholder (e.g. "older than 3 most recent") would be a value
        // that can change between builds and silently invalidate an already-cached prefix.
        assert!(
            !PRUNED_SCREENSHOT_PLACEHOLDER
                .chars()
                .any(|c| c.is_ascii_digit()),
            "placeholder must not embed counts: {}",
            PRUNED_SCREENSHOT_PLACEHOLDER
        );
    }

    // --- Cache breakpoints -------------------------------------------------------------------

    #[test]
    fn cache_breakpoint_budget_adds_up() {
        assert_eq!(
            PREFIX_CACHE_BREAKPOINTS + MESSAGE_CACHE_BREAKPOINTS,
            MAX_CACHE_BREAKPOINTS
        );
        assert_eq!(MESSAGE_CACHE_BREAKPOINTS, 2);
    }

    #[test]
    fn message_breakpoints_never_exceed_their_share_of_the_budget() {
        for turns in 1..=40usize {
            let mut msgs = conversation(turns);
            AnthropicBrain::apply_message_cache_breakpoints(&mut msgs);
            let placed = breakpoint_indices(&msgs).len();
            assert!(
                (1..=MESSAGE_CACHE_BREAKPOINTS).contains(&placed),
                "turns={} placed={}",
                turns,
                placed
            );
        }
    }

    #[test]
    fn no_message_breakpoint_without_tool_results() {
        let mut msgs = vec![ApiMessage {
            role: "user".to_string(),
            content: ApiContent::Text("hello".to_string()),
        }];
        AnthropicBrain::apply_message_cache_breakpoints(&mut msgs);
        assert!(breakpoint_indices(&msgs).is_empty());
    }

    #[test]
    fn latest_tool_result_always_carries_a_breakpoint() {
        let mut msgs = conversation(10);
        let last_idx = msgs.len() - 1;
        AnthropicBrain::apply_message_cache_breakpoints(&mut msgs);
        assert!(
            breakpoint_indices(&msgs).contains(&last_idx),
            "the newest tool_result must be a breakpoint so the NEXT turn hits the cache"
        );
    }

    #[test]
    fn breakpoint_lands_on_the_last_tool_result_block_of_the_turn() {
        // A turn that returns two tool results: the breakpoint belongs on the second one, so
        // the cached prefix covers the whole turn.
        let mut first = ApiContentBlock::empty("tool_result");
        first.tool_use_id = Some("toolu_a".to_string());
        first.content = Some(ApiToolResultContent::Text("a".to_string()));
        let mut second = ApiContentBlock::empty("tool_result");
        second.tool_use_id = Some("toolu_b".to_string());
        second.content = Some(ApiToolResultContent::Text("b".to_string()));

        let mut msgs = vec![
            ApiMessage {
                role: "user".to_string(),
                content: ApiContent::Text("go".to_string()),
            },
            ApiMessage {
                role: "user".to_string(),
                content: ApiContent::Blocks(vec![first, second]),
            },
        ];
        AnthropicBrain::apply_message_cache_breakpoints(&mut msgs);

        let ApiContent::Blocks(blocks) = &msgs[1].content else {
            panic!("expected block content");
        };
        assert!(blocks[0].cache_control.is_none());
        assert!(blocks[1].cache_control.is_some());
    }

    #[test]
    fn anchor_breakpoint_stays_put_for_a_whole_stride() {
        // The anchor is quantised so it keeps one longer-lived cache entry warm instead of
        // moving (and re-writing) every turn.
        let mut anchors: Vec<usize> = Vec::new();
        for turns in 1..=CACHE_ANCHOR_STRIDE {
            let mut msgs = conversation(turns);
            AnthropicBrain::apply_message_cache_breakpoints(&mut msgs);
            let indices = breakpoint_indices(&msgs);
            let first = indices.first().copied().unwrap_or_default();
            anchors.push(first);
        }
        assert!(
            anchors.windows(2).all(|w| w[0] == w[1]),
            "anchor moved within a single stride: {:?}",
            anchors
        );
    }

    #[test]
    fn breakpoints_are_only_placed_on_tool_result_blocks() {
        let mut msgs = conversation(12);
        AnthropicBrain::apply_message_cache_breakpoints(&mut msgs);
        for msg in &msgs {
            if let ApiContent::Blocks(blocks) = &msg.content {
                for block in blocks {
                    if block.cache_control.is_some() {
                        assert_eq!(block.block_type, "tool_result");
                    }
                }
            }
        }
    }

    #[test]
    fn cache_control_serializes_as_ephemeral_and_is_omitted_otherwise() {
        let mut msgs = conversation(2);
        AnthropicBrain::apply_message_cache_breakpoints(&mut msgs);
        let json = serde_json::to_string(&msgs).unwrap_or_default();
        // The latest breakpoint keeps the default TTL, which is expressed by omitting `ttl`.
        assert!(
            json.contains("\"cache_control\":{\"type\":\"ephemeral\"}"),
            "{}",
            json
        );

        let clean = conversation(2);
        let json = serde_json::to_string(&clean).unwrap_or_default();
        assert!(!json.contains("cache_control"), "{}", json);
    }

    // --- Extended (1-hour) cache TTL ---------------------------------------------------------

    #[test]
    fn default_cache_control_omits_ttl_entirely() {
        // Adding the `ttl` field must not change the bytes of a breakpoint that does not set
        // it — an existing 5-minute breakpoint has to serialize exactly as it did before.
        let json = serde_json::to_string(&CacheControl::ephemeral()).unwrap_or_default();
        assert_eq!(json, r#"{"type":"ephemeral"}"#);
    }

    #[test]
    fn extended_cache_control_serializes_the_documented_shape() {
        // Verified against the live docs 2026-09-22: `ttl` lives inside `cache_control`
        // next to `type`, and the extended value is the string "1h".
        let json = serde_json::to_string(&CacheControl::ephemeral_extended()).unwrap_or_default();
        assert_eq!(json, r#"{"type":"ephemeral","ttl":"1h"}"#);
        assert_eq!(CACHE_TTL_EXTENDED, "1h");
    }

    #[test]
    fn anchor_gets_the_extended_ttl_and_latest_keeps_the_default() {
        let mut msgs = conversation(10); // latest_pos 9, anchor_pos 8 — two distinct breakpoints
        AnthropicBrain::apply_message_cache_breakpoints(&mut msgs);
        let bps = breakpoints(&msgs);
        assert_eq!(bps.len(), 2, "{:?}", bps);
        assert_eq!(bps[0].1.as_deref(), Some("1h"), "anchor must be extended");
        assert_eq!(bps[1].1, None, "latest must keep the default 5m TTL");
        assert!(bps[0].0 < bps[1].0, "anchor must precede latest");
    }

    #[test]
    fn coincident_anchor_and_latest_keep_the_extended_ttl() {
        // The anchor lands on the latest turn exactly when it advances (latest_pos a multiple
        // of the stride). That single breakpoint is the one 1h write per stride — it must not
        // be downgraded to 5m.
        for turns in [1usize, CACHE_ANCHOR_STRIDE + 1, 2 * CACHE_ANCHOR_STRIDE + 1] {
            let mut msgs = conversation(turns);
            AnthropicBrain::apply_message_cache_breakpoints(&mut msgs);
            let bps = breakpoints(&msgs);
            assert_eq!(bps.len(), 1, "turns={} bps={:?}", turns, bps);
            assert_eq!(bps[0].1.as_deref(), Some("1h"), "turns={}", turns);
        }
    }

    #[test]
    fn every_extended_breakpoint_precedes_every_default_one() {
        // Hard API rule: a 1-hour cache entry must appear before any 5-minute entry. Tools and
        // the system prompt are both 1h and both sit ahead of `messages`, so checking the
        // message breakpoints is sufficient to prove the whole request is ordered correctly.
        for turns in 1..=40usize {
            let mut msgs = conversation(turns);
            AnthropicBrain::apply_message_cache_breakpoints(&mut msgs);
            let bps = breakpoints(&msgs);
            let last_extended = bps.iter().rposition(|(_, ttl)| ttl.is_some());
            let first_default = bps.iter().position(|(_, ttl)| ttl.is_none());
            if let (Some(last_ext), Some(first_def)) = (last_extended, first_default) {
                assert!(
                    last_ext < first_def,
                    "turns={} a 5m breakpoint precedes a 1h one: {:?}",
                    turns,
                    bps
                );
            }
        }
    }

    #[test]
    fn at_most_one_default_ttl_message_breakpoint_per_request() {
        // Only the latest turn is allowed to be 5m; anything else would put a short TTL on a
        // prefix that outlives it, and would risk violating the ordering rule.
        for turns in 1..=40usize {
            let mut msgs = conversation(turns);
            AnthropicBrain::apply_message_cache_breakpoints(&mut msgs);
            let defaults = breakpoints(&msgs)
                .into_iter()
                .filter(|(_, ttl)| ttl.is_none())
                .count();
            assert!(defaults <= 1, "turns={} defaults={}", turns, defaults);
        }
    }

    #[test]
    fn extended_ttl_write_is_paid_once_per_stride() {
        // The 1h write premium lands only on the turns where anchor and latest coincide, i.e.
        // once every CACHE_ANCHOR_STRIDE turns. Count them over three full strides.
        let span = 3 * CACHE_ANCHOR_STRIDE;
        let coincident = (1..=span)
            .filter(|&turns| {
                let mut msgs = conversation(turns);
                AnthropicBrain::apply_message_cache_breakpoints(&mut msgs);
                breakpoints(&msgs).len() == 1
            })
            .count();
        assert_eq!(coincident, 3, "expected one 1h write per stride");
    }
}
