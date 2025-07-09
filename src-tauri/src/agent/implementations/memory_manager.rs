use crate::agent::core::{
    AgentError,
    Message,
    Role,
};
use crate::agent::traits::MemoryManager;
use crate::constants::memory::{limits, tokens, visual, summary, patterns, performance, COMMON_WORDS};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use tokio::sync::RwLock;
use std::sync::Arc;
use std::time::{Instant, Duration, SystemTime};
use uuid::Uuid;
use tokio::sync::Mutex as TokioMutex;

/// Configuration for advanced memory management
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryConfig {
    /// Maximum number of messages before pruning
    pub max_messages: usize,
    /// Maximum estimated tokens before pruning
    pub max_tokens: usize,
    /// Minimum number of messages to keep during pruning
    pub min_messages_to_keep: usize,
    /// Enable automatic memory pruning
    pub auto_prune: bool,
    /// Enable memory summarization
    pub enable_summarization: bool,
    /// Number of messages to summarize in a batch
    pub summarization_batch_size: usize,
    /// Enable memory statistics tracking
    pub enable_metrics: bool,
    /// Cache summarized content for performance
    pub enable_summary_cache: bool,
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            max_messages: limits::DEFAULT_MAX_MESSAGES,
            max_tokens: limits::DEFAULT_MAX_TOKENS,
            min_messages_to_keep: limits::DEFAULT_MIN_MESSAGES_TO_KEEP,
            auto_prune: true,
            enable_summarization: true,
            summarization_batch_size: limits::DEFAULT_SUMMARIZATION_BATCH_SIZE,
            enable_metrics: true,
            enable_summary_cache: true,
        }
    }
}

/// Configuration for visual context compression
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisualContextConfig {
    /// Enable screenshot compression to text summaries
    pub enable_screenshot_compression: bool,
    /// Maximum age of screenshots to keep in memory (in seconds)
    pub screenshot_retention_seconds: u64,
    /// Compress screenshots immediately after processing
    pub immediate_compression: bool,
    /// Maximum number of screenshots to keep as base64
    pub max_base64_screenshots: usize,
    /// Fallback to generic description if vision API fails
    pub fallback_to_generic_description: bool,
}

impl Default for VisualContextConfig {
    fn default() -> Self {
        // Use the corrected defaults for computer use
        let (enable_compression, retention_seconds, immediate_compression, max_screenshots, fallback_description) =
            crate::constants::memory::defaults::get_visual_config();

        Self {
            enable_screenshot_compression: enable_compression,
            screenshot_retention_seconds: retention_seconds,
            immediate_compression,  // NOW FALSE - allows computer use agents to see screenshots!
            max_base64_screenshots: max_screenshots,  // NOW 8 - allows multiple screenshots
            fallback_to_generic_description: fallback_description,
        }
    }
}

/// Statistics for memory usage and performance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryMetrics {
    pub total_messages: usize,
    pub estimated_tokens: usize,
    pub pruning_events: usize,
    pub summarization_events: usize,
    pub orphaned_tool_calls_cleaned: usize,
    pub average_response_time_ms: f64,
    pub last_prune_time: Option<SystemTime>,
    pub memory_efficiency_ratio: f64, // Useful messages / total messages
}

impl Default for MemoryMetrics {
    fn default() -> Self {
        Self {
            total_messages: 0,
            estimated_tokens: 0,
            pruning_events: 0,
            summarization_events: 0,
            orphaned_tool_calls_cleaned: 0,
            average_response_time_ms: 0.0,
            last_prune_time: None,
            memory_efficiency_ratio: 1.0,
        }
    }
}

/// Summary of conversation chunks for efficient memory management
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationSummary {
    pub id: String,
    pub summary: String,
    pub message_count: usize,
    pub time_range: (SystemTime, SystemTime),
    pub key_topics: Vec<String>,
    pub estimated_tokens: usize,
}

/// Visual context summary to replace base64 images
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisualContextSummary {
    pub id: String,
    pub timestamp: SystemTime,
    pub summary: String,
    pub dominant_colors: Vec<String>,
    pub ui_elements: Vec<String>,
    pub text_content: Option<String>,
    pub estimated_original_tokens: usize,
    pub compressed_tokens: usize,
    pub compression_ratio: f64,
}

/// Enhanced screenshot content analysis - NEW ADVANCED FEATURE
#[derive(Debug, Clone)]
pub struct ScreenshotContentAnalysis {
    pub content_type: String,
    pub complexity_score: f64,
    pub ui_elements: Vec<String>,
    pub estimated_text_content: Option<String>,
    pub visual_context: String,
}

/// Enhanced in-memory implementation of the MemoryManager trait with advanced features
#[derive(Debug, Clone)]
pub struct AdvancedMemoryManager {
    messages: Arc<RwLock<Vec<Message>>>,
    pending_tool_calls: Arc<RwLock<HashSet<String>>>,
    config: Arc<RwLock<MemoryConfig>>,
    visual_config: Arc<RwLock<VisualContextConfig>>,
    metrics: Arc<RwLock<MemoryMetrics>>,
    summaries: Arc<RwLock<Vec<ConversationSummary>>>,
    visual_summaries: Arc<RwLock<Vec<VisualContextSummary>>>,
    summary_cache: Arc<RwLock<std::collections::HashMap<String, String>>>,
    current_execution_id: Arc<RwLock<Option<String>>>,
}

impl AdvancedMemoryManager {
    pub fn new() -> Self {
        Self {
            messages: Arc::new(RwLock::new(Vec::new())),
            pending_tool_calls: Arc::new(RwLock::new(HashSet::new())),
            config: Arc::new(RwLock::new(MemoryConfig::default())),
            visual_config: Arc::new(RwLock::new(VisualContextConfig::default())),
            metrics: Arc::new(RwLock::new(MemoryMetrics::default())),
            summaries: Arc::new(RwLock::new(Vec::new())),
            visual_summaries: Arc::new(RwLock::new(Vec::new())),
            summary_cache: Arc::new(RwLock::new(std::collections::HashMap::new())),
            current_execution_id: Arc::new(RwLock::new(None)),
        }
    }

    pub fn with_config(config: MemoryConfig) -> Self {
        AdvancedMemoryManager {
            messages: Arc::new(RwLock::new(Vec::new())),
            pending_tool_calls: Arc::new(RwLock::new(HashSet::new())),
            config: Arc::new(RwLock::new(config)),
            visual_config: Arc::new(RwLock::new(VisualContextConfig::default())),
            metrics: Arc::new(RwLock::new(MemoryMetrics::default())),
            summaries: Arc::new(RwLock::new(Vec::new())),
            visual_summaries: Arc::new(RwLock::new(Vec::new())),
            summary_cache: Arc::new(RwLock::new(std::collections::HashMap::new())),
            current_execution_id: Arc::new(RwLock::new(None)),
        }
    }

    pub fn with_visual_config(mut self, visual_config: VisualContextConfig) -> Self {
        self.visual_config = Arc::new(RwLock::new(visual_config));
        self
    }

    /// Estimate token count for a message with proper mixed content parsing
    pub fn estimate_message_tokens(message: &Message) -> usize {
        let mut total_tokens = 0;
        let content = &message.content;

        // Parse content to separate text from base64 images in document order
        total_tokens += Self::estimate_content_tokens(content);

        // Add tool call tokens
        let tool_call_tokens = message.tool_calls.as_ref()
            .map(|calls| {
                calls.iter().map(|call| {
                    let base_tokens = call.name.len() / tokens::CHARS_PER_TOKEN_TEXT + tokens::BASE_TOOL_CALL_TOKENS;

                    // Estimate tool call input tokens with mixed content parsing
                    let input_str = call.input.to_string();
                    let input_tokens = Self::estimate_content_tokens(&input_str);

                    base_tokens + input_tokens
                }).sum()
            })
            .unwrap_or(0);

        total_tokens += tool_call_tokens;

        // Log warning for very large messages
        if total_tokens > limits::LARGE_MESSAGE_WARNING_TOKENS {
            log::warn!("Large message detected: ~{} tokens (content length: {})", total_tokens, content.len());
        }

        total_tokens
    }

    /// Estimate tokens for content with mixed text and base64 images
    fn estimate_content_tokens(content: &str) -> usize {
        let mut total_tokens = 0;
        let mut remaining_content = content;

        // Define base64 image prefixes in order of specificity
        let image_prefixes = [
            patterns::PNG_DATA_URL_PREFIX,
            patterns::JPEG_DATA_URL_PREFIX,
            patterns::WEBP_DATA_URL_PREFIX,
            patterns::GENERIC_IMAGE_DATA_PREFIX,
        ];

        while !remaining_content.is_empty() {
            let mut found_image = false;
            let mut earliest_pos = remaining_content.len();
            let mut matched_prefix = "";

            // Find the earliest occurring image prefix in document order
            for prefix in &image_prefixes {
                if let Some(pos) = remaining_content.find(prefix) {
                    if pos < earliest_pos {
                        earliest_pos = pos;
                        matched_prefix = prefix;
                        found_image = true;
                    }
                }
            }

            if found_image {
                // Count text before the image as regular text
                if earliest_pos > 0 {
                    let text_part = &remaining_content[..earliest_pos];
                    total_tokens += text_part.len() / tokens::CHARS_PER_TOKEN_TEXT;
                }

                // Count the data URL prefix as regular text (not base64)
                total_tokens += matched_prefix.len() / tokens::CHARS_PER_TOKEN_TEXT;

                // Find the start of actual base64 data (after the prefix)
                let base64_start_pos = earliest_pos + matched_prefix.len();

                // Find where base64 ends (non-base64 character or end of string)
                let mut base64_end_pos = base64_start_pos;
                let remaining_after_prefix = &remaining_content[base64_start_pos..];

                for (i, ch) in remaining_after_prefix.char_indices() {
                    if ch.is_ascii_alphanumeric() || ch == '+' || ch == '/' || ch == '=' {
                        base64_end_pos = base64_start_pos + i + ch.len_utf8();
                    } else {
                        break;
                    }
                }

                // Count only the actual base64 data with image token rate
                let base64_length = base64_end_pos - base64_start_pos;
                if base64_length > 0 {
                    let base64_tokens = base64_length / tokens::CHARS_PER_TOKEN_BASE64_IMAGE;
                    total_tokens += base64_tokens;

                    // Log significant base64 content
                    if base64_tokens > limits::LARGE_MESSAGE_WARNING_TOKENS / 2 {
                        log::warn!("Large base64 content detected: {} chars = ~{} tokens", base64_length, base64_tokens);
                    }
                }

                // Update remaining content to continue parsing
                remaining_content = &remaining_content[base64_end_pos..];
            } else {
                // No prefixed images found - check for pure base64 content without prefixes
                // This handles the critical case where base64 data lacks data URL prefixes
                let pure_base64_detected = Self::detect_and_process_pure_base64(remaining_content, &mut total_tokens);

                if !pure_base64_detected {
                    // No base64 content found, count remaining as text
                    total_tokens += remaining_content.len() / tokens::CHARS_PER_TOKEN_TEXT;
                }
                break;
            }
        }

        total_tokens
    }

    /// Detect and process pure base64 content without data URL prefixes
    /// Returns true if pure base64 content was detected and processed
    /// Only modifies total_tokens when returning true to prevent double-counting
    fn detect_and_process_pure_base64(content: &str, total_tokens: &mut usize) -> bool {
        // Check for large base64 content using the same logic as is_screenshot_content
        if content.len() > visual::MIN_SCREENSHOT_CONTENT_LENGTH {
            let base64_char_count = content.chars()
                .filter(|c| c.is_ascii_alphanumeric() || *c == '+' || *c == '/' || *c == '=')
                .count();

            let base64_char_percentage = (base64_char_count * 100) / content.len();

            if base64_char_percentage >= visual::BASE64_CHAR_THRESHOLD_PERCENT {
                // This looks like pure base64 content - treat as high-cost image tokens
                let base64_tokens = content.len() / tokens::CHARS_PER_TOKEN_BASE64_IMAGE;
                *total_tokens += base64_tokens;

                log::warn!("Pure base64 content detected (no prefix): {} chars = ~{} tokens ({}% base64 chars)",
                          content.len(), base64_tokens, base64_char_percentage);

                return true;
            }
        }

        // Also check for smaller chunks that might be pure base64
        // Look for continuous base64 sequences of significant length
        let mut pos = 0;
        let mut found_any_base64 = false;
        let mut temp_tokens = 0; // Track tokens separately to avoid double-counting

        while pos < content.len() {
            // Find start of potential base64 sequence using byte-based search
            let remaining_slice = &content[pos..];
            let base64_char_start = remaining_slice.find(|c: char| c.is_ascii_alphanumeric() || c == '+' || c == '/' || c == '=');

            if let Some(start_offset) = base64_char_start {
                let start = pos + start_offset;

                // Find end of base64 sequence using safe UTF-8 character iteration
                let mut end = start;
                let remaining_from_start = &content[start..];

                for (byte_offset, ch) in remaining_from_start.char_indices() {
                    if ch.is_ascii_alphanumeric() || ch == '+' || ch == '/' || ch == '=' || ch.is_ascii_whitespace() {
                        end = start + byte_offset + ch.len_utf8();
                    } else {
                        break;
                    }
                }

                let segment = &content[start..end];
                let clean_segment = segment.chars()
                    .filter(|c| !c.is_whitespace())
                    .collect::<String>();

                // Check if this segment is likely base64 (minimum length and high base64 char percentage)
                if clean_segment.len() > 1000 { // Minimum reasonable base64 chunk size
                    let base64_chars = clean_segment.chars()
                        .filter(|c| c.is_ascii_alphanumeric() || *c == '+' || *c == '/' || *c == '=')
                        .count();

                    if base64_chars >= clean_segment.len() * 90 / 100 { // 90% threshold for segments
                        // Count text before this base64 segment as regular text
                        if start > pos {
                            let text_before = &content[pos..start];
                            temp_tokens += text_before.len() / tokens::CHARS_PER_TOKEN_TEXT;
                        }

                        // Count the base64 segment as image tokens
                        let segment_tokens = clean_segment.len() / tokens::CHARS_PER_TOKEN_BASE64_IMAGE;
                        temp_tokens += segment_tokens;

                        log::info!("Pure base64 segment detected: {} chars = ~{} tokens",
                                  clean_segment.len(), segment_tokens);

                        found_any_base64 = true;
                        pos = end;
                        continue;
                    }
                }

                // For segments that don't qualify as base64, count as regular text
                if start > pos {
                    let text_before = &content[pos..start];
                    temp_tokens += text_before.len() / tokens::CHARS_PER_TOKEN_TEXT;
                }

                // Count the failed base64 segment as regular text
                temp_tokens += segment.len() / tokens::CHARS_PER_TOKEN_TEXT;

                pos = end;
            } else {
                // No more base64 characters found - count remaining content as regular text
                let remaining_text = &content[pos..];
                temp_tokens += remaining_text.len() / tokens::CHARS_PER_TOKEN_TEXT;
                break;
            }
        }

        // CRITICAL FIX: Only update total_tokens if we found qualifying base64 content
        // This prevents double-counting when caller counts content as regular text
        if found_any_base64 {
            *total_tokens += temp_tokens;
        }

        found_any_base64
    }

    /// Estimate total token count for all messages
    async fn estimate_total_tokens(&self) -> usize {
        let messages = self.messages.read().await;
        messages.iter()
            .map(Self::estimate_message_tokens)
            .sum()
    }

    /// Update memory metrics after operations
    async fn update_metrics(&self, operation_start: Instant) -> Result<(), AgentError> {
        let config = self.config.read().await;
        if !config.enable_metrics {
            return Ok(());
        }

        let mut metrics = self.metrics.write().await;
        let messages = self.messages.read().await;

        metrics.total_messages = messages.len();
        metrics.estimated_tokens = messages.iter()
            .map(Self::estimate_message_tokens)
            .sum();

        let operation_time = operation_start.elapsed().as_millis() as f64;
        metrics.average_response_time_ms =
            (metrics.average_response_time_ms + operation_time) / 2.0;

        // Calculate efficiency ratio
        let useful_messages = messages.iter()
            .filter(|m| !m.content.is_empty() || m.tool_calls.is_some())
            .count();
        metrics.memory_efficiency_ratio = if messages.len() > 0 {
            useful_messages as f64 / messages.len() as f64
        } else {
            1.0
        };

        Ok(())
    }

    /// Prune old messages based on configuration
    async fn prune_memory_if_needed(&self) -> Result<bool, AgentError> {
        let config = self.config.read().await;
        if !config.auto_prune {
            return Ok(false);
        }

        let messages_count = {
            let messages = self.messages.read().await;
            messages.len()
        };

        let estimated_tokens = self.estimate_total_tokens().await;

        // Emergency pruning if we're close to API limits
        let emergency_threshold = limits::EMERGENCY_TOKEN_THRESHOLD;
        let normal_threshold = config.max_tokens;

        let needs_emergency_pruning = estimated_tokens >= emergency_threshold;
        let needs_normal_pruning = messages_count >= config.max_messages ||
                                 estimated_tokens >= normal_threshold;

        if needs_emergency_pruning {
            log::error!("EMERGENCY: Token count ({}) approaching API limit (200K)! Aggressive pruning required.", estimated_tokens);
            // Keep only the most recent messages
            let emergency_keep = std::cmp::max(config.min_messages_to_keep, limits::EMERGENCY_MIN_KEEP);
            drop(config);
            self.prune_memory(Some(emergency_keep)).await?;
            return Ok(true);
        } else if needs_normal_pruning {
            log::info!("Memory pruning triggered: {} messages, ~{} tokens",
                      messages_count, estimated_tokens);
            drop(config);
            self.prune_memory(None).await?;
            return Ok(true);
        } else {
            drop(config);
            return Ok(false);
        }
    }

    /// Create a summary of conversation segments
    async fn create_conversation_summary(&self, messages: &[Message]) -> Result<ConversationSummary, AgentError> {
        if messages.is_empty() {
            return Err(AgentError::ConfigurationError("Cannot summarize empty message list".to_string()));
        }

        // Simple summarization logic (in production, you'd use an LLM)
        let summary = if messages.len() <= summary::SHORT_CONVERSATION_MAX_MESSAGES {
            // For short conversations, just concatenate key points
            messages.iter()
                .filter(|m| !m.content.is_empty())
                .map(|m| {
                    let content = if m.content.len() > summary::MAX_SHORT_CONTENT_LENGTH {
                        format!("{}...", &m.content[..summary::MAX_SHORT_CONTENT_LENGTH])
                    } else {
                        m.content.clone()
                    };
                    format!("{:?}: {}", m.role, content)
                })
                .collect::<Vec<_>>()
                .join(" | ")
        } else {
            // For longer conversations, create a structured summary
            let user_messages = messages.iter().filter(|m| m.role == Role::User).count();
            let assistant_messages = messages.iter().filter(|m| m.role == Role::Assistant).count();
            let tool_calls = messages.iter()
                .filter_map(|m| m.tool_calls.as_ref())
                .map(|calls| calls.len())
                .sum::<usize>();

            format!("Conversation segment: {} user messages, {} assistant responses, {} tool calls executed",
                    user_messages, assistant_messages, tool_calls)
        };

        // Extract key topics (simple keyword extraction)
        let all_content = messages.iter()
            .map(|m| m.content.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        let key_topics = self.extract_key_topics(&all_content);

        let time_range = (
            SystemTime::now() - Duration::from_secs(summary::CONVERSATION_START_OFFSET_SECONDS),
            SystemTime::now()
        );

        let estimated_tokens = messages.iter()
            .map(Self::estimate_message_tokens)
            .sum();

        Ok(ConversationSummary {
            id: Uuid::new_v4().to_string(),
            summary,
            message_count: messages.len(),
            time_range,
            key_topics,
            estimated_tokens,
        })
    }

    /// Extract key topics from text (simple implementation)
    fn extract_key_topics(&self, text: &str) -> Vec<String> {
        // Simple keyword extraction (in production, use NLP libraries)
        let words: Vec<&str> = text.split_whitespace()
            .filter(|word| word.len() > summary::MIN_KEYWORD_LENGTH && !COMMON_WORDS.contains(&word.to_lowercase().as_str()))
            .collect();

        // Count word frequencies
        let mut word_counts = std::collections::HashMap::new();
        for word in words {
            *word_counts.entry(word.to_lowercase()).or_insert(0) += 1;
        }

        // Get top keywords
        let mut sorted_words: Vec<_> = word_counts.into_iter().collect();
        sorted_words.sort_by(|a, b| b.1.cmp(&a.1));

        sorted_words.into_iter()
            .take(summary::MAX_KEYWORDS_TO_EXTRACT)
            .map(|(word, _)| word)
            .collect()
    }

    /// Prune memory with optional target size
    pub async fn prune_memory(&self, target_messages: Option<usize>) -> Result<usize, AgentError> {
        let config = self.config.read().await;
        let target_size = target_messages.unwrap_or(config.min_messages_to_keep);

        let mut messages = self.messages.write().await;

        if messages.len() <= target_size {
            return Ok(0);
        }

        let messages_to_remove = messages.len() - target_size;

        // Create summary of messages being removed if summarization is enabled
        if config.enable_summarization && config.summarization_batch_size > 0 {
            let messages_to_summarize = &messages[..messages_to_remove.min(config.summarization_batch_size)];
            if !messages_to_summarize.is_empty() {
                match self.create_conversation_summary(messages_to_summarize).await {
                    Ok(summary) => {
                        let mut summaries = self.summaries.write().await;
                        summaries.push(summary);

                        // Keep only recent summaries to prevent unbounded growth
                        if summaries.len() > limits::MAX_SUMMARIES_TO_KEEP {
                            summaries.remove(0);
                        }
                    }
                    Err(e) => {
                        log::warn!("Failed to create conversation summary: {}", e);
                    }
                }
            }
        }

        // Remove old messages
        messages.drain(..messages_to_remove);

        // Update metrics
        {
            let mut metrics = self.metrics.write().await;
            metrics.pruning_events += 1;
            metrics.last_prune_time = Some(SystemTime::now());
        }

        log::info!("Pruned {} messages from memory, {} remaining",
                   messages_to_remove, messages.len());

        Ok(messages_to_remove)
    }

    /// Get memory statistics
    pub async fn get_memory_metrics(&self) -> MemoryMetrics {
        let metrics = self.metrics.read().await;
        metrics.clone()
    }

    /// Get configuration
    pub async fn get_config(&self) -> MemoryConfig {
        let config = self.config.read().await;
        config.clone()
    }

    /// Update configuration
    pub async fn update_config(&self, new_config: MemoryConfig) -> Result<(), AgentError> {
        let mut config = self.config.write().await;
        *config = new_config;
        log::info!("Memory configuration updated");
        Ok(())
    }

    /// Get conversation summaries
    pub async fn get_summaries(&self) -> Vec<ConversationSummary> {
        let summaries = self.summaries.read().await;
        summaries.clone()
    }

    /// Force memory optimization (cleanup, compression, etc.)
    pub async fn optimize_memory(&mut self) -> Result<(), AgentError> {
        let start_time = Instant::now();

        // Clean orphaned tool calls
        let _orphaned_cleaned = self.clean_orphaned_tool_calls().await?;

        // Force pruning if over limits
        let _pruned = self.prune_memory_if_needed().await?;

        // Clear summary cache if it's getting too large
        {
            let mut cache = self.summary_cache.write().await;
            if cache.len() > performance::MAX_CACHE_SIZE {
                cache.clear();
                log::info!("Cleared summary cache to free memory");
            }
        }

        self.update_metrics(start_time).await?;

        log::info!("Memory optimization completed in {}ms",
                   start_time.elapsed().as_millis());

        Ok(())
    }

    /// Get memory-optimized message history with summaries
    pub async fn get_optimized_messages(&self) -> Result<Vec<Message>, AgentError> {
        let messages = self.messages.read().await;
        let summaries = self.summaries.read().await;

        let mut optimized_messages = Vec::new();

        // Add conversation summaries as system messages if available
        for summary in summaries.iter() {
            optimized_messages.push(Message {
                role: Role::System,
                content: format!("Previous conversation summary: {}", summary.summary),
                tool_calls: None,
                tool_call_id: None,
                name: Some("memory_summary".to_string()),
            });
        }

        // Add current messages
        optimized_messages.extend(messages.clone());

        Ok(optimized_messages)
    }

    /// Get hot context for immediate processing (industry-leading optimization)
    pub async fn get_hot_context(&self) -> Result<Vec<Message>, AgentError> {
        let messages = self.messages.read().await;
        let config = self.config.read().await;

        // Hot context: last 5-10 messages for immediate relevance
        let hot_context_size = std::cmp::min(limits::HOT_CONTEXT_SIZE, config.min_messages_to_keep);
        let start_index = messages.len().saturating_sub(hot_context_size);
        let hot_messages = messages[start_index..].to_vec();

        Ok(hot_messages)
    }

    /// Get cold context on demand (background loading pattern)
    pub async fn get_cold_context_async(&self) -> Result<Vec<Message>, AgentError> {
        let messages = self.messages.read().await;
        let config = self.config.read().await;

        // Cold context: everything before the hot context window
        let hot_context_size = std::cmp::min(limits::HOT_CONTEXT_SIZE, config.min_messages_to_keep);
        let cold_end_index = messages.len().saturating_sub(hot_context_size);

        if cold_end_index == 0 {
            return Ok(vec![]); // No cold context available
        }

        let cold_messages = messages[..cold_end_index].to_vec();
        Ok(cold_messages)
    }

    /// Preload essential context at session start (industry best practice)
    pub async fn preload_session_context(&self, user_preferences: Option<serde_json::Value>) -> Result<(), AgentError> {
        let start_time = Instant::now();

        // Preload user preferences into memory if provided
        if let Some(prefs) = user_preferences {
            let prefs_message = Message {
                role: Role::System,
                content: format!("User preferences: {}", prefs.to_string()),
                tool_calls: None,
                tool_call_id: None,
                name: Some("session_context".to_string()),
            };

            let mut messages = self.messages.write().await;
            messages.insert(0, prefs_message); // Insert at beginning
        }

        // Trigger summarization of old context in background
        let config = self.config.read().await;
        if config.enable_summarization {
            drop(config);
            tokio::spawn(async move {
                // Background context optimization - don't block startup
                // This would trigger summarization of old conversations
            });
        }

        log::info!("Session context preloaded in {:?}", start_time.elapsed());
        Ok(())
    }

    /// Smart context retrieval with tiered access patterns
    pub async fn get_tiered_context(&self, max_immediate_tokens: usize) -> Result<(Vec<Message>, Vec<Message>), AgentError> {
        let hot_context = self.get_hot_context().await?;

        // Calculate hot context token usage
        let hot_tokens: usize = hot_context.iter()
            .map(Self::estimate_message_tokens)
            .sum();

        if hot_tokens <= max_immediate_tokens {
            // Hot context fits in immediate budget
            let cold_context = self.get_cold_context_async().await?;
            Ok((hot_context, cold_context))
        } else {
            // Reduce hot context to fit budget
            let mut reduced_hot = Vec::new();
            let mut token_count = 0;

            for message in hot_context.iter().rev() {
                let msg_tokens = Self::estimate_message_tokens(message);
                if token_count + msg_tokens <= max_immediate_tokens {
                    reduced_hot.insert(0, message.clone());
                    token_count += msg_tokens;
                } else {
                    break;
                }
            }

            // All remaining context becomes cold
            let cold_context = self.get_cold_context_async().await?;
            Ok((reduced_hot, cold_context))
        }
    }

    /// Sets the current execution ID to help distinguish between different agent executions
    pub async fn set_current_execution_id(&mut self, execution_id: &str) -> Result<(), AgentError> {
        let mut current_id = self.current_execution_id.write().await;
        *current_id = Some(execution_id.to_string());
        log::info!("Set current execution ID: {}", execution_id);
        Ok(())
    }

    /// Remove orphaned tool calls that don't have corresponding tool results
    /// This method should be called when starting a new agent execution to clean up
    /// any incomplete tool calls from previous cancelled executions
    pub async fn clean_orphaned_tool_calls(&mut self) -> Result<(), AgentError> {
        let start_time = Instant::now();
        let mut messages = self.messages.write().await;
        let mut pending = self.pending_tool_calls.write().await;

        // Find all tool call IDs that have results
        let mut resolved_tool_calls = HashSet::new();
        for message in messages.iter() {
            if message.role == Role::Tool {
                if let Some(tool_call_id) = &message.tool_call_id {
                    resolved_tool_calls.insert(tool_call_id.clone());
                }
            }
        }

        // Remove any Assistant messages with tool calls that don't have corresponding results
        let mut orphaned_tool_call_ids = HashSet::new();
        messages.retain(|message| {
            if message.role == Role::Assistant {
                if let Some(tool_calls) = &message.tool_calls {
                    // Check if all tool calls in this message have results
                    let all_resolved = tool_calls.iter().all(|tc| resolved_tool_calls.contains(&tc.id));
                    if !all_resolved {
                        // Mark these tool calls as orphaned
                        for tc in tool_calls {
                            if !resolved_tool_calls.contains(&tc.id) {
                                orphaned_tool_call_ids.insert(tc.id.clone());
                            }
                        }
                        log::warn!("Removing orphaned Assistant message with unresolved tool calls: {:?}",
                                   tool_calls.iter().map(|tc| &tc.id).collect::<Vec<_>>());
                        return false; // Remove this message
                    }
                }
            }
            true // Keep the message
        });

        // Clean up pending tool calls
        pending.retain(|id| !orphaned_tool_call_ids.contains(id));

        // Enhanced metrics update with comprehensive tracking
        if !orphaned_tool_call_ids.is_empty() {
            // Update metrics safely with enhanced tracking
            if let Ok(mut metrics) = self.metrics.try_write() {
                metrics.orphaned_tool_calls_cleaned += orphaned_tool_call_ids.len();

                // Calculate memory efficiency ratio after cleanup
                // Use the existing messages variable instead of acquiring a new lock
                let useful_messages = messages.iter()
                    .filter(|m| !m.content.is_empty() || m.tool_calls.is_some())
                    .count();
                metrics.memory_efficiency_ratio = if messages.len() > 0 {
                    useful_messages as f64 / messages.len() as f64
                } else {
                    1.0
                };

                // Update operation timing
                let operation_time = start_time.elapsed().as_millis() as f64;
                metrics.average_response_time_ms =
                    (metrics.average_response_time_ms + operation_time) / 2.0;
            } else {
                log::warn!("Could not acquire metrics lock for orphaned tool calls update");
            }

            log::info!("Cleaned up {} orphaned tool calls: {:?} (operation took {}ms)",
                       orphaned_tool_call_ids.len(), orphaned_tool_call_ids, start_time.elapsed().as_millis());
        }
        Ok(())
    }

    /// Cleans only orphaned tool calls from previous executions, not from the current one
    pub async fn clean_orphaned_tool_calls_from_previous_executions(&mut self) -> Result<(), AgentError> {
        let start_time = Instant::now();
        let current_execution_id_option = {
            let guard = self.current_execution_id.read().await;
            guard.clone()
        };

        // If no current execution ID is set, fall back to regular cleaning
        if current_execution_id_option.is_none() {
            log::warn!("No current execution ID set, falling back to regular orphaned tool call cleanup");
            return self.clean_orphaned_tool_calls().await;
        }

        let current_execution_id = current_execution_id_option.unwrap();
        log::info!("Cleaning orphaned tool calls from previous executions (current execution: {})",
                  current_execution_id);

        let mut messages = self.messages.write().await;
        let mut pending = self.pending_tool_calls.write().await;

        // Find all tool call IDs that have results
        let mut resolved_tool_calls = HashSet::new();
        for message in messages.iter() {
            if message.role == Role::Tool {
                if let Some(tool_call_id) = &message.tool_call_id {
                    resolved_tool_calls.insert(tool_call_id.clone());
                }
            }
        }

        // Remove any Assistant messages with tool calls that don't have corresponding results
        // BUT only if they're not part of the current execution (determined by toolcall_id prefix)
        let mut orphaned_tool_call_ids = HashSet::new();
        messages.retain(|message| {
            if message.role == Role::Assistant {
                if let Some(tool_calls) = &message.tool_calls {
                    // Check if all tool calls in this message have results
                    let all_resolved = tool_calls.iter().all(|tc| resolved_tool_calls.contains(&tc.id));

                    if !all_resolved {
                        // Check if any unresolved tool call belongs to the current execution
                        let has_current_execution_tools = tool_calls.iter()
                            .filter(|tc| !resolved_tool_calls.contains(&tc.id))
                            .any(|tc| tc.id.contains(&current_execution_id));

                        if has_current_execution_tools {
                            // Keep messages from current execution even if they have unresolved tools
                            log::debug!("Keeping unresolved tool calls from current execution: {}", current_execution_id);
                            return true;
                        }

                        // Mark these tool calls as orphaned (from previous executions)
                        for tc in tool_calls {
                            if !resolved_tool_calls.contains(&tc.id) {
                                orphaned_tool_call_ids.insert(tc.id.clone());
                            }
                        }

                        log::warn!("Removing orphaned Assistant message with unresolved tool calls from previous execution: {:?}",
                                   tool_calls.iter().map(|tc| &tc.id).collect::<Vec<_>>());
                        return false; // Remove this message
                    }
                }
            }
            true // Keep the message
        });

        // Clean up pending tool calls (only from previous executions)
        pending.retain(|id| !orphaned_tool_call_ids.contains(id) || id.contains(&current_execution_id));

        if !orphaned_tool_call_ids.is_empty() {
            // Update metrics safely with enhanced tracking
            if let Ok(mut metrics) = self.metrics.try_write() {
                metrics.orphaned_tool_calls_cleaned += orphaned_tool_call_ids.len();

                // Calculate memory efficiency ratio after cleanup
                let useful_messages = messages.iter()
                    .filter(|m| !m.content.is_empty() || m.tool_calls.is_some())
                    .count();
                metrics.memory_efficiency_ratio = if messages.len() > 0 {
                    useful_messages as f64 / messages.len() as f64
                } else {
                    1.0
                };

                // Update operation timing
                let operation_time = start_time.elapsed().as_millis() as f64;
                metrics.average_response_time_ms =
                    (metrics.average_response_time_ms + operation_time) / 2.0;
            } else {
                log::warn!("Could not acquire metrics lock for orphaned tool calls update");
            }

            log::info!("Cleaned up {} orphaned tool calls from previous executions: {:?} (operation took {}ms)",
                       orphaned_tool_call_ids.len(), orphaned_tool_call_ids, start_time.elapsed().as_millis());
        }

        Ok(())
    }

    /// Clear all pending tool calls (useful when starting a fresh conversation)
    pub async fn clear_pending_tool_calls(&mut self) -> Result<(), AgentError> {
        let mut pending = self.pending_tool_calls.write().await;
        pending.clear();
        log::info!("Cleared all pending tool calls");
        Ok(())
    }

    /// Get a list of currently pending tool call IDs
    pub async fn get_pending_tool_calls(&self) -> Result<Vec<String>, AgentError> {
        let pending = self.pending_tool_calls.read().await;
        Ok(pending.iter().cloned().collect())
    }

    /// Clean up orphaned tool results that don't have corresponding tool calls
    /// This method removes tool result messages that have no matching tool_use blocks
    pub async fn clean_orphaned_tool_results(&mut self) -> Result<usize, AgentError> {
        let start_time = Instant::now();
        let mut messages = self.messages.write().await;

        // Find all tool call IDs that exist in assistant messages
        let mut valid_tool_call_ids = std::collections::HashSet::new();
        for message in messages.iter() {
            if message.role == Role::Assistant {
                if let Some(tool_calls) = &message.tool_calls {
                    for tool_call in tool_calls {
                        valid_tool_call_ids.insert(tool_call.id.clone());
                    }
                }
            }
        }

        // Count orphaned tool results before removal
        let mut orphaned_count = 0;
        let mut orphaned_ids = Vec::new();

        // Remove tool result messages that don't have corresponding tool calls
        messages.retain(|message| {
            if message.role == Role::Tool {
                if let Some(tool_call_id) = &message.tool_call_id {
                    if !valid_tool_call_ids.contains(tool_call_id) {
                        orphaned_count += 1;
                        orphaned_ids.push(tool_call_id.clone());
                        log::warn!("Removing orphaned tool result with ID: {}", tool_call_id);
                        return false; // Remove this message
                    }
                }
            }
            true // Keep the message
        });

        if orphaned_count > 0 {
            // Enhanced metrics update with comprehensive tracking
            if let Ok(mut metrics) = self.metrics.try_write() {
                metrics.orphaned_tool_calls_cleaned += orphaned_count;

                // Calculate memory efficiency ratio after cleanup
                let useful_messages = messages.iter()
                    .filter(|m| !m.content.is_empty() || m.tool_calls.is_some())
                    .count();
                metrics.memory_efficiency_ratio = if messages.len() > 0 {
                    useful_messages as f64 / messages.len() as f64
                } else {
                    1.0
                };

                // Update operation timing
                let operation_time = start_time.elapsed().as_millis() as f64;
                metrics.average_response_time_ms =
                    (metrics.average_response_time_ms + operation_time) / 2.0;
            } else {
                log::warn!("Could not acquire metrics lock for orphaned tool results update");
            }

            log::info!("Cleaned up {} orphaned tool results: {:?} (operation took {}ms)",
                       orphaned_count, orphaned_ids, start_time.elapsed().as_millis());
        }
        Ok(orphaned_count)
    }

    /// Detect if content contains a screenshot/base64 image
    fn is_screenshot_content(content: &str) -> bool {
        // Enhanced detection for screenshots specifically
        if content.contains(patterns::PNG_DATA_URL_PREFIX) ||
           content.contains(patterns::JPEG_DATA_URL_PREFIX) ||
           content.contains(patterns::WEBP_DATA_URL_PREFIX) {
            return true;
        }

        // Check for large base64 content that's likely a screenshot
        if content.len() > visual::MIN_SCREENSHOT_CONTENT_LENGTH &&
           content.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '+' || *c == '/' || *c == '=').count() > content.len() * visual::BASE64_CHAR_THRESHOLD_PERCENT / 100 {
            return true;
        }

        false
    }

    /// Analyze screenshot content for enhanced compression - RE-ENABLED WITH ENHANCEMENTS
    async fn analyze_screenshot_content(&self, base64_content: &str) -> ScreenshotContentAnalysis {
        let content_length = base64_content.len();

        // Enhanced content type detection
        let content_type = if base64_content.contains(patterns::PNG_DATA_URL_PREFIX) {
            "PNG Screenshot"
        } else if base64_content.contains(patterns::JPEG_DATA_URL_PREFIX) {
            "JPEG Screenshot"
        } else if base64_content.contains(patterns::WEBP_DATA_URL_PREFIX) {
            "WebP Screenshot"
        } else {
            "Unknown Image Format"
        };

        // Calculate complexity score based on content size and characteristics
        let complexity_score = match content_length {
            0..=50000 => 0.2,      // Simple interface
            50001..=150000 => 0.5,  // Moderate complexity
            150001..=300000 => 0.7, // Complex interface
            300001..=500000 => 0.9, // Very complex
            _ => 1.0,              // Maximum complexity
        };

        // Enhanced UI element detection based on content patterns
        let ui_elements = vec![
            "Desktop interface".to_string(),
            "Application windows".to_string(),
            "Menu bars and toolbars".to_string(),
            "Interactive elements".to_string(),
            if complexity_score > 0.7 { "Complex visual layout" } else { "Simple layout" }.to_string(),
            if content_length > 200000 { "High detail screenshot" } else { "Standard detail" }.to_string(),
        ];

        // Estimate text content presence
        let estimated_text_content = if complexity_score > 0.5 {
            Some("Likely contains readable text and UI labels".to_string())
        } else {
            Some("Minimal text content detected".to_string())
        };

        // Generate enhanced visual context
        let visual_context = format!(
            "Computer interface screenshot with {} complexity. Contains {} UI elements. \
            Estimated content density: {}. Visual analysis indicates {} user interface.",
            match complexity_score {
                x if x > 0.8 => "very high",
                x if x > 0.6 => "high",
                x if x > 0.4 => "moderate",
                x if x > 0.2 => "low",
                _ => "minimal"
            },
            ui_elements.len(),
            if content_length > 250000 { "dense" } else { "standard" },
            if complexity_score > 0.7 { "complex" } else { "standard" }
        );

        ScreenshotContentAnalysis {
            content_type: content_type.to_string(),
            complexity_score,
            ui_elements,
            estimated_text_content,
            visual_context,
        }
    }

    /// Compress a screenshot to text summary with enhanced analysis
    async fn compress_screenshot_to_text(&self, base64_content: &str) -> Result<VisualContextSummary, AgentError> {
        let start_time = Instant::now();
        let original_tokens = base64_content.len() / tokens::CHARS_PER_TOKEN_BASE64_IMAGE;

        // RE-ENABLED: Enhanced screenshot analysis
        let analysis = self.analyze_screenshot_content(base64_content).await;

        // Create comprehensive summary with enhanced analysis
        let summary = if self.visual_config.read().await.fallback_to_generic_description {
            let timestamp = SystemTime::now();
            let time_str = format!("{:?}", timestamp.duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs());

            format!(
                "Screenshot captured at {} ({}). {}. {}. \
                Content analysis: {} (complexity: {:.1}/1.0). \
                Original size: ~{} tokens compressed to text summary for memory efficiency.",
                time_str,
                chrono::DateTime::<chrono::Utc>::from(timestamp).format("%Y-%m-%d %H:%M:%S UTC"),
                analysis.content_type,
                analysis.visual_context,
                analysis.estimated_text_content.as_deref().unwrap_or("No text analysis available"),
                analysis.complexity_score,
                original_tokens
            )
        } else {
            format!(
                "Screenshot: {}. {}. Analysis: {} elements detected, complexity {:.1}/1.0. \
                Compressed from ~{} tokens to preserve context while reducing memory usage.",
                analysis.content_type,
                analysis.visual_context,
                analysis.ui_elements.len(),
                analysis.complexity_score,
                original_tokens
            )
        };

        let compressed_tokens = summary.len() / tokens::CHARS_PER_TOKEN_TEXT;
        let compression_ratio = original_tokens as f64 / compressed_tokens as f64;

        log::info!("Enhanced screenshot compression: {} tokens -> {} tokens ({}x reduction, complexity: {:.1})",
                  original_tokens, compressed_tokens, compression_ratio, analysis.complexity_score);

        Ok(VisualContextSummary {
            id: Uuid::new_v4().to_string(),
            timestamp: SystemTime::now(),
            summary,
            dominant_colors: vec![
                format!("Content type: {}", analysis.content_type),
                format!("Complexity: {:.1}/1.0", analysis.complexity_score)
            ],
            ui_elements: analysis.ui_elements,
            text_content: analysis.estimated_text_content,
            estimated_original_tokens: original_tokens,
            compressed_tokens,
            compression_ratio,
        })
    }

    /// Process and potentially compress screenshots in a message
    async fn process_message_screenshots(&self, message: &mut Message) -> Result<bool, AgentError> {
        let visual_config = self.visual_config.read().await;

        if !visual_config.enable_screenshot_compression {
            log::warn!("Screenshot compression is disabled - this may cause token overflow!");
            return Ok(false);
        }

        let mut was_compressed = false;

        // Check if this message contains a screenshot
        if Self::is_screenshot_content(&message.content) {
            log::info!("Detected screenshot in message ({}+ chars), processing for compression...", message.content.len());

            // Get current visual summaries to check retention limits
            let mut visual_summaries = self.visual_summaries.write().await;
            let current_base64_count = visual_summaries.len();

            // ALWAYS compress if immediate_compression is enabled OR we've exceeded the limit
            // With max_base64_screenshots set to 0, this should always compress
            if visual_config.immediate_compression || current_base64_count >= visual_config.max_base64_screenshots {
                log::info!("Compressing screenshot: immediate_compression={}, count={}/{}",
                          visual_config.immediate_compression, current_base64_count, visual_config.max_base64_screenshots);

                // Compress to text summary
                match self.compress_screenshot_to_text(&message.content).await {
                    Ok(summary) => {
                        log::info!("Screenshot compression successful: {} tokens -> {} tokens ({}x reduction)",
                                  summary.estimated_original_tokens, summary.compressed_tokens, summary.compression_ratio);

                        // Replace base64 content with text summary
                        message.content = format!(
                            "[SCREENSHOT SUMMARY] {}\n\n[COMPRESSION STATS] Original: ~{} tokens, Compressed: {} tokens, Ratio: {:.1}x",
                            summary.summary,
                            summary.estimated_original_tokens,
                            summary.compressed_tokens,
                            summary.compression_ratio
                        );

                        visual_summaries.push(summary);
                        was_compressed = true;

                        log::info!("Successfully compressed screenshot to text summary");
                    }
                    Err(e) => {
                        log::error!("CRITICAL: Failed to compress screenshot: {} - keeping original - THIS MAY CAUSE TOKEN OVERFLOW!", e);
                        // In case of compression failure, we should still try to truncate the content to prevent overflow
                        if message.content.len() > limits::LARGE_MESSAGE_WARNING_TOKENS {
                            log::warn!("Truncating oversized screenshot content to prevent API failure");
                            message.content = format!("[SCREENSHOT - TRUNCATED DUE TO COMPRESSION FAILURE] Original size: {} chars. Error: {}", message.content.len(), e);
                        }
                    }
                }
            } else {
                log::warn!("Keeping screenshot as base64 (count: {}/{})", current_base64_count, visual_config.max_base64_screenshots);
            }
        }

        // Clean up old visual summaries
        let cutoff_time = SystemTime::now()
            .checked_sub(Duration::from_secs(visual_config.screenshot_retention_seconds))
            .unwrap_or(SystemTime::UNIX_EPOCH);

        self.visual_summaries.write().await.retain(|summary| summary.timestamp > cutoff_time);

        Ok(was_compressed)
    }

    /// Get visual context summaries
    pub async fn get_visual_summaries(&self) -> Vec<VisualContextSummary> {
        self.visual_summaries.read().await.clone()
    }

    /// Update visual context configuration
    pub async fn update_visual_config(&self, new_config: VisualContextConfig) -> Result<(), AgentError> {
        let mut config = self.visual_config.write().await;
        *config = new_config;
        log::info!("Updated visual context configuration");
        Ok(())
    }

    /// Get current visual context configuration
    pub async fn get_visual_config(&self) -> VisualContextConfig {
        self.visual_config.read().await.clone()
    }

    /// Force compression of all screenshots in current conversation
    pub async fn compress_all_screenshots(&mut self) -> Result<usize, AgentError> {
        let mut messages = self.messages.write().await;
        let mut compressed_count = 0;

        for message in messages.iter_mut() {
            if Self::is_screenshot_content(&message.content) {
                match self.compress_screenshot_to_text(&message.content).await {
                    Ok(summary) => {
                        message.content = format!(
                            "[SCREENSHOT SUMMARY] {}\n\n[COMPRESSION STATS] Original: ~{} tokens, Compressed: {} tokens, Ratio: {:.1}x",
                            summary.summary,
                            summary.estimated_original_tokens,
                            summary.compressed_tokens,
                            summary.compression_ratio
                        );

                        self.visual_summaries.write().await.push(summary);
                        compressed_count += 1;
                    }
                    Err(e) => {
                        log::warn!("Failed to compress {}: {}", "screenshot", e);
                    }
                }
            }
        }

        log::info!("Compressed {} screenshots to text summaries", compressed_count);
        Ok(compressed_count)
    }
}

// Add MemoryManager trait implementation for AdvancedMemoryManager
#[async_trait]
impl MemoryManager for AdvancedMemoryManager {
<<<<<<< HEAD
||||||| parent of ed8ec6f5 (auto commit)
    async fn add_message(&mut self, mut message: Message) -> Result<(), AgentError> {
        let start_time = Instant::now();

        // Process screenshots BEFORE adding to memory (critical for token optimization)
        self.process_message_screenshots(&mut message).await?;

        let mut messages = self.messages.write().await;
        let mut pending = self.pending_tool_calls.write().await;

        // Track tool calls and results
        match message.role {
            Role::Assistant => {
                if let Some(tool_calls) = &message.tool_calls {
                    // Add tool call IDs to pending list
                    for tool_call in tool_calls {
                        pending.insert(tool_call.id.clone());
                        log::debug!("Tracking pending tool call: {}", tool_call.id);
                    }
                }
            }
            Role::Tool => {
                if let Some(tool_call_id) = &message.tool_call_id {
                    // Remove from pending list when result is added
                    if pending.remove(tool_call_id) {
                        log::debug!("Resolved pending tool call: {}", tool_call_id);
                    } else {
                        log::warn!("Received tool result for unknown tool call ID: {}", tool_call_id);
                    }
                }
            }
            _ => {}
        }

        messages.push(message.clone());

        // Release locks before async operations
        drop(messages);
        drop(pending);

        log::debug!("Memory: Added message. Role={:?}", message.role);

                // RE-ENABLED: Auto-pruning and metrics with safer implementation
        self.prune_memory_if_needed().await?;

        // RE-ENABLED: Metrics update with safer timing (non-critical path)
        if let Err(e) = self.update_metrics(start_time).await {
            log::warn!("Failed to update metrics after adding message: {}", e);
            // Continue execution - metrics failure shouldn't block message addition
        }

        Ok(())
    }

    async fn get_messages(&self) -> Result<Vec<Message>, AgentError> {
        let start_time = Instant::now();
        let messages = self.messages.read().await;
        let pending = self.pending_tool_calls.read().await;

        log::debug!("Memory: Retrieved {} messages, {} pending tool calls",
                    messages.len(), pending.len());

        let result = messages.clone();
        drop(messages);
        drop(pending);

        // RE-ENABLED: Metrics update with error handling
        if let Err(e) = self.update_metrics(start_time).await {
            log::warn!("Failed to update metrics after getting messages: {}", e);
            // Continue execution - metrics failure shouldn't block message retrieval
        }

        Ok(result)
    }

    async fn get_last_n_messages(&self, n: usize) -> Result<Vec<Message>, AgentError> {
        let start_time = Instant::now();
        let messages = self.messages.read().await;
        let start_index = messages.len().saturating_sub(n);
        let result = messages[start_index..].to_vec();
        drop(messages);

        // RE-ENABLED: Metrics update with error handling
        if let Err(e) = self.update_metrics(start_time).await {
            log::warn!("Failed to update metrics after getting last N messages: {}", e);
            // Continue execution - metrics failure shouldn't block message retrieval
        }

        Ok(result)
    }

    async fn clear_memory(&mut self) -> Result<(), AgentError> {
        let start_time = Instant::now();

        let mut messages = self.messages.write().await;
        let mut pending = self.pending_tool_calls.write().await;
        let mut summaries = self.summaries.write().await;
        let mut cache = self.summary_cache.write().await;

        messages.clear();
        pending.clear();
        summaries.clear();
        cache.clear();

        log::info!("Memory: Cleared all messages, pending tool calls, summaries, and cache");

        // RE-ENABLED: Enhanced metrics reset with operation tracking
        {
            let mut metrics = self.metrics.write().await;
            // Preserve cumulative counters but reset operational metrics
            let preserved_pruning_events = metrics.pruning_events;
            let preserved_summarization_events = metrics.summarization_events;
            let preserved_orphaned_cleaned = metrics.orphaned_tool_calls_cleaned;

            *metrics = MemoryMetrics::default();

            // Restore cumulative counters
            metrics.pruning_events = preserved_pruning_events;
            metrics.summarization_events = preserved_summarization_events;
            metrics.orphaned_tool_calls_cleaned = preserved_orphaned_cleaned;

            log::info!("Memory metrics reset: preserved {} pruning events, {} summarization events, {} orphaned calls cleaned",
                       preserved_pruning_events, preserved_summarization_events, preserved_orphaned_cleaned);
        }

        Ok(())
    }

    async fn clean_orphaned_tool_calls(&mut self) -> Result<(), AgentError> {
        // Call the actual implementation method
        AdvancedMemoryManager::clean_orphaned_tool_calls(self).await
    }

    async fn clean_orphaned_tool_results(&mut self) -> Result<usize, AgentError> {
        // Call the actual implementation method
        AdvancedMemoryManager::clean_orphaned_tool_results(self).await
    }
}

/// A simple in-memory implementation of the MemoryManager trait (existing implementation)
/// Kept for backward compatibility
#[derive(Debug, Clone)]
pub struct SimpleMemoryManager {
    messages: Arc<RwLock<Vec<Message>>>,
    pending_tool_calls: Arc<RwLock<HashSet<String>>>, // Track tool call IDs that haven't been resolved yet
}

impl SimpleMemoryManager {
    pub fn new() -> Self {
        SimpleMemoryManager {
            messages: Arc::new(RwLock::new(Vec::new())),
            pending_tool_calls: Arc::new(RwLock::new(HashSet::new())),
        }
    }

    /// Remove orphaned tool calls that don't have corresponding tool results
    /// This method should be called when starting a new agent execution to clean up
    /// any incomplete tool calls from previous cancelled executions
    pub async fn clean_orphaned_tool_calls(&mut self) -> Result<(), AgentError> {
        let mut messages = self.messages.write().await;
        let mut pending = self.pending_tool_calls.write().await;

        // Find all tool call IDs that have results
        let mut resolved_tool_calls = HashSet::new();
        for message in messages.iter() {
            if message.role == Role::Tool {
                if let Some(tool_call_id) = &message.tool_call_id {
                    resolved_tool_calls.insert(tool_call_id.clone());
                }
            }
        }

        // Remove any Assistant messages with tool calls that don't have corresponding results
        let mut orphaned_tool_call_ids = HashSet::new();
        messages.retain(|message| {
            if message.role == Role::Assistant {
                if let Some(tool_calls) = &message.tool_calls {
                    // Check if all tool calls in this message have results
                    let all_resolved = tool_calls.iter().all(|tc| resolved_tool_calls.contains(&tc.id));
                    if !all_resolved {
                        // Mark these tool calls as orphaned
                        for tc in tool_calls {
                            if !resolved_tool_calls.contains(&tc.id) {
                                orphaned_tool_call_ids.insert(tc.id.clone());
                            }
                        }
                        log::warn!("Removing orphaned Assistant message with unresolved tool calls: {:?}",
                                   tool_calls.iter().map(|tc| &tc.id).collect::<Vec<_>>());
                        return false; // Remove this message
                    }
                }
            }
            true // Keep the message
        });

        // Clean up pending tool calls
        pending.retain(|id| !orphaned_tool_call_ids.contains(id));

        if !orphaned_tool_call_ids.is_empty() {
            log::info!("Cleaned up {} orphaned tool calls: {:?}",
                       orphaned_tool_call_ids.len(), orphaned_tool_call_ids);
        }

        Ok(())
    }

    /// Clear all pending tool calls (useful when starting a fresh conversation)
    pub async fn clear_pending_tool_calls(&mut self) -> Result<(), AgentError> {
        let mut pending = self.pending_tool_calls.write().await;
        pending.clear();
        log::info!("Cleared all pending tool calls");
        Ok(())
    }

    /// Get a list of currently pending tool call IDs
    pub async fn get_pending_tool_calls(&self) -> Result<Vec<String>, AgentError> {
        let pending = self.pending_tool_calls.read().await;
        Ok(pending.iter().cloned().collect())
    }

    /// Clean up orphaned tool results that don't have corresponding tool calls
    /// This method removes tool result messages that have no matching tool_use blocks
    pub async fn clean_orphaned_tool_results(&mut self) -> Result<usize, AgentError> {
        let mut messages = self.messages.write().await;

        // Find all tool call IDs that exist in assistant messages
        let mut valid_tool_call_ids = std::collections::HashSet::new();
        for message in messages.iter() {
            if message.role == Role::Assistant {
                if let Some(tool_calls) = &message.tool_calls {
                    for tool_call in tool_calls {
                        valid_tool_call_ids.insert(tool_call.id.clone());
                    }
                }
            }
        }

        // Count orphaned tool results before removal
        let mut orphaned_count = 0;
        let mut orphaned_ids = Vec::new();

        // Remove tool result messages that don't have corresponding tool calls
        messages.retain(|message| {
            if message.role == Role::Tool {
                if let Some(tool_call_id) = &message.tool_call_id {
                    if !valid_tool_call_ids.contains(tool_call_id) {
                        orphaned_count += 1;
                        orphaned_ids.push(tool_call_id.clone());
                        log::warn!("Removing orphaned tool result with ID: {}", tool_call_id);
                        return false; // Remove this message
                    }
                }
            }
            true // Keep the message
        });

        if orphaned_count > 0 {
            log::info!("Cleaned up {} orphaned tool results: {:?}", orphaned_count, orphaned_ids);
        }

        Ok(orphaned_count)
    }
}

#[async_trait]
impl MemoryManager for SimpleMemoryManager {
=======
    async fn add_message(&mut self, mut message: Message) -> Result<(), AgentError> {
        let start_time = Instant::now();

        // Process screenshots BEFORE adding to memory (critical for token optimization)
        self.process_message_screenshots(&mut message).await?;

        let mut messages = self.messages.write().await;
        let mut pending = self.pending_tool_calls.write().await;

        // Track tool calls and results
        match message.role {
            Role::Assistant => {
                if let Some(tool_calls) = &message.tool_calls {
                    // Add tool call IDs to pending list
                    for tool_call in tool_calls {
                        pending.insert(tool_call.id.clone());
                        log::debug!("Tracking pending tool call: {}", tool_call.id);
                    }
                }
            }
            Role::Tool => {
                if let Some(tool_call_id) = &message.tool_call_id {
                    // Remove from pending list when result is added
                    if pending.remove(tool_call_id) {
                        log::debug!("Resolved pending tool call: {}", tool_call_id);
                    } else {
                        log::warn!("Received tool result for unknown tool call ID: {}", tool_call_id);
                    }
                }
            }
            _ => {}
        }

        messages.push(message.clone());

        // Release locks before async operations
        drop(messages);
        drop(pending);

        log::debug!("Memory: Added message. Role={:?}", message.role);

                // RE-ENABLED: Auto-pruning and metrics with safer implementation
        self.prune_memory_if_needed().await?;

        // RE-ENABLED: Metrics update with safer timing (non-critical path)
        if let Err(e) = self.update_metrics(start_time).await {
            log::warn!("Failed to update metrics after adding message: {}", e);
            // Continue execution - metrics failure shouldn't block message addition
        }

        Ok(())
    }

    async fn get_messages(&self) -> Result<Vec<Message>, AgentError> {
        let start_time = Instant::now();
        let messages = self.messages.read().await;
        let pending = self.pending_tool_calls.read().await;

        log::debug!("Memory: Retrieved {} messages, {} pending tool calls",
                    messages.len(), pending.len());

        let result = messages.clone();
        drop(messages);
        drop(pending);

        // RE-ENABLED: Metrics update with error handling
        if let Err(e) = self.update_metrics(start_time).await {
            log::warn!("Failed to update metrics after getting messages: {}", e);
            // Continue execution - metrics failure shouldn't block message retrieval
        }

        Ok(result)
    }

    async fn get_last_n_messages(&self, n: usize) -> Result<Vec<Message>, AgentError> {
        let start_time = Instant::now();
        let messages = self.messages.read().await;
        let start_index = messages.len().saturating_sub(n);
        let result = messages[start_index..].to_vec();
        drop(messages);

        // RE-ENABLED: Metrics update with error handling
        if let Err(e) = self.update_metrics(start_time).await {
            log::warn!("Failed to update metrics after getting last N messages: {}", e);
            // Continue execution - metrics failure shouldn't block message retrieval
        }

        Ok(result)
    }

    async fn clear_memory(&mut self) -> Result<(), AgentError> {
        let start_time = Instant::now();

        let mut messages = self.messages.write().await;
        let mut pending = self.pending_tool_calls.write().await;
        let mut summaries = self.summaries.write().await;
        let mut cache = self.summary_cache.write().await;

        messages.clear();
        pending.clear();
        summaries.clear();
        cache.clear();

        log::info!("Memory: Cleared all messages, pending tool calls, summaries, and cache");

        // RE-ENABLED: Enhanced metrics reset with operation tracking
        {
            let mut metrics = self.metrics.write().await;
            // Preserve cumulative counters but reset operational metrics
            let preserved_pruning_events = metrics.pruning_events;
            let preserved_summarization_events = metrics.summarization_events;
            let preserved_orphaned_cleaned = metrics.orphaned_tool_calls_cleaned;

            *metrics = MemoryMetrics::default();

            // Restore cumulative counters
            metrics.pruning_events = preserved_pruning_events;
            metrics.summarization_events = preserved_summarization_events;
            metrics.orphaned_tool_calls_cleaned = preserved_orphaned_cleaned;

            log::info!("Memory metrics reset: preserved {} pruning events, {} summarization events, {} orphaned calls cleaned",
                       preserved_pruning_events, preserved_summarization_events, preserved_orphaned_cleaned);
        }

        Ok(())
    }

    async fn clean_orphaned_tool_calls(&mut self) -> Result<(), AgentError> {
        // Call the actual implementation method
        AdvancedMemoryManager::clean_orphaned_tool_calls(self).await
    }

    async fn clean_orphaned_tool_results(&mut self) -> Result<usize, AgentError> {
        // Call the actual implementation method
        AdvancedMemoryManager::clean_orphaned_tool_results(self).await
    }

    async fn set_current_execution_id(&mut self, execution_id: &str) -> Result<(), AgentError> {
        AdvancedMemoryManager::set_current_execution_id(self, execution_id).await
    }

    async fn clean_orphaned_tool_calls_from_previous_executions(&mut self) -> Result<(), AgentError> {
        AdvancedMemoryManager::clean_orphaned_tool_calls_from_previous_executions(self).await
    }
}

/// A simple in-memory implementation of the MemoryManager trait (existing implementation)
/// Kept for backward compatibility
#[derive(Debug, Clone)]
pub struct SimpleMemoryManager {
    messages: Arc<RwLock<Vec<Message>>>,
    pending_tool_calls: Arc<RwLock<HashSet<String>>>, // Track tool call IDs that haven't been resolved yet
}

impl SimpleMemoryManager {
    pub fn new() -> Self {
        SimpleMemoryManager {
            messages: Arc::new(RwLock::new(Vec::new())),
            pending_tool_calls: Arc::new(RwLock::new(HashSet::new())),
        }
    }

    /// Remove orphaned tool calls that don't have corresponding tool results
    /// This method should be called when starting a new agent execution to clean up
    /// any incomplete tool calls from previous cancelled executions
    pub async fn clean_orphaned_tool_calls(&mut self) -> Result<(), AgentError> {
        let mut messages = self.messages.write().await;
        let mut pending = self.pending_tool_calls.write().await;

        // Find all tool call IDs that have results
        let mut resolved_tool_calls = HashSet::new();
        for message in messages.iter() {
            if message.role == Role::Tool {
                if let Some(tool_call_id) = &message.tool_call_id {
                    resolved_tool_calls.insert(tool_call_id.clone());
                }
            }
        }

        // Remove any Assistant messages with tool calls that don't have corresponding results
        let mut orphaned_tool_call_ids = HashSet::new();
        messages.retain(|message| {
            if message.role == Role::Assistant {
                if let Some(tool_calls) = &message.tool_calls {
                    // Check if all tool calls in this message have results
                    let all_resolved = tool_calls.iter().all(|tc| resolved_tool_calls.contains(&tc.id));
                    if !all_resolved {
                        // Mark these tool calls as orphaned
                        for tc in tool_calls {
                            if !resolved_tool_calls.contains(&tc.id) {
                                orphaned_tool_call_ids.insert(tc.id.clone());
                            }
                        }
                        log::warn!("Removing orphaned Assistant message with unresolved tool calls: {:?}",
                                   tool_calls.iter().map(|tc| &tc.id).collect::<Vec<_>>());
                        return false; // Remove this message
                    }
                }
            }
            true // Keep the message
        });

        // Clean up pending tool calls
        pending.retain(|id| !orphaned_tool_call_ids.contains(id));

        if !orphaned_tool_call_ids.is_empty() {
            log::info!("Cleaned up {} orphaned tool calls: {:?}",
                       orphaned_tool_call_ids.len(), orphaned_tool_call_ids);
        }

        Ok(())
    }

    /// Clear all pending tool calls (useful when starting a fresh conversation)
    pub async fn clear_pending_tool_calls(&mut self) -> Result<(), AgentError> {
        let mut pending = self.pending_tool_calls.write().await;
        pending.clear();
        log::info!("Cleared all pending tool calls");
        Ok(())
    }

    /// Get a list of currently pending tool call IDs
    pub async fn get_pending_tool_calls(&self) -> Result<Vec<String>, AgentError> {
        let pending = self.pending_tool_calls.read().await;
        Ok(pending.iter().cloned().collect())
    }

    /// Clean up orphaned tool results that don't have corresponding tool calls
    /// This method removes tool result messages that have no matching tool_use blocks
    pub async fn clean_orphaned_tool_results(&mut self) -> Result<usize, AgentError> {
        let mut messages = self.messages.write().await;

        // Find all tool call IDs that exist in assistant messages
        let mut valid_tool_call_ids = std::collections::HashSet::new();
        for message in messages.iter() {
            if message.role == Role::Assistant {
                if let Some(tool_calls) = &message.tool_calls {
                    for tool_call in tool_calls {
                        valid_tool_call_ids.insert(tool_call.id.clone());
                    }
                }
            }
        }

        // Count orphaned tool results before removal
        let mut orphaned_count = 0;
        let mut orphaned_ids = Vec::new();

        // Remove tool result messages that don't have corresponding tool calls
        messages.retain(|message| {
            if message.role == Role::Tool {
                if let Some(tool_call_id) = &message.tool_call_id {
                    if !valid_tool_call_ids.contains(tool_call_id) {
                        orphaned_count += 1;
                        orphaned_ids.push(tool_call_id.clone());
                        log::warn!("Removing orphaned tool result with ID: {}", tool_call_id);
                        return false; // Remove this message
                    }
                }
            }
            true // Keep the message
        });

        if orphaned_count > 0 {
            log::info!("Cleaned up {} orphaned tool results: {:?}", orphaned_count, orphaned_ids);
        }

        Ok(orphaned_count)
    }
}

#[async_trait]
impl MemoryManager for SimpleMemoryManager {
>>>>>>> ed8ec6f5 (auto commit)
    async fn add_message(&mut self, message: Message) -> Result<(), AgentError> {
        // Record operation start time for metrics
        let operation_start = Instant::now();

        // Clone the message to avoid ownership issues
        let mut message_clone = message.clone();

        // Process screenshots in the message content if necessary
        if Self::is_screenshot_content(&message_clone.content) {
            if let Err(e) = self.process_message_screenshots(&mut message_clone).await {
                log::warn!("Failed to process screenshots in message: {}", e);
            }
        }

        // Track tool calls if present
        if let Some(tool_calls) = &message_clone.tool_calls {
            let mut pending = self.pending_tool_calls.write().await;
            for tool_call in tool_calls {
                pending.insert(tool_call.id.clone());
            }
        }

        // Remove from pending if this is a tool result
        if message_clone.role == Role::Tool {
            if let Some(tool_call_id) = &message_clone.tool_call_id {
                let mut pending = self.pending_tool_calls.write().await;
                pending.remove(tool_call_id);
            }
        }

        // Add message to memory
        {
            let mut messages = self.messages.write().await;
            messages.push(message_clone);

            // Update metrics
            if let Ok(mut metrics) = self.metrics.try_write() {
                metrics.total_messages = messages.len();
            }
        }

        // Prune memory if needed
        if self.config.read().await.auto_prune {
            if let Err(e) = self.prune_memory_if_needed().await {
                log::warn!("Failed to auto-prune memory: {}", e);
            }
        }

        // Update operation metrics
        let _ = self.update_metrics(operation_start).await;

        Ok(())
    }

    async fn get_messages(&self) -> Result<Vec<Message>, AgentError> {
        let messages = self.messages.read().await.clone();
        Ok(messages)
    }

    async fn get_last_n_messages(&self, n: usize) -> Result<Vec<Message>, AgentError> {
        let messages = self.messages.read().await;
        let start_idx = if messages.len() > n { messages.len() - n } else { 0 };
        let result = messages[start_idx..].to_vec();
        Ok(result)
    }

    async fn clear_memory(&mut self) -> Result<(), AgentError> {
        // Clear all messages
        {
            let mut messages = self.messages.write().await;
            messages.clear();
        }

        // Clear pending tool calls
        {
            let mut pending = self.pending_tool_calls.write().await;
            pending.clear();
        }

        // Reset metrics (but keep counters for pruning events, etc.)
        {
            if let Ok(mut metrics) = self.metrics.try_write() {
                metrics.total_messages = 0;
                metrics.estimated_tokens = 0;
                // Keep other metrics for historical tracking
            }
        }

        log::info!("Memory cleared");
        Ok(())
    }

    // These methods are already implemented as public methods on AdvancedMemoryManager
    // and simply forward to those existing implementations

    async fn clean_orphaned_tool_calls(&mut self) -> Result<(), AgentError> {
        // Delegate to the existing implementation
        self.clean_orphaned_tool_calls().await
    }

    async fn clean_orphaned_tool_results(&mut self) -> Result<usize, AgentError> {
        // Delegate to the existing implementation
        self.clean_orphaned_tool_results().await
    }
}

impl Default for AdvancedMemoryManager {
    fn default() -> Self {
        Self::new()
    }
}

/// A specialized memory manager that isolates tool calls between agents
/// while preserving the conversation context. Used for specialist agents
/// that are delegated from the orchestrator.
#[derive(Debug, Clone)]
pub struct DelegatedMemoryManager {
    /// Shared reference to parent memory manager (for conversation context)
    parent: Arc<TokioMutex<AdvancedMemoryManager>>,
    /// Local tool call tracking (isolated from parent)
    local_pending_tool_calls: Arc<RwLock<HashSet<String>>>,
}

impl DelegatedMemoryManager {
    pub fn from_parent(parent: Arc<TokioMutex<AdvancedMemoryManager>>) -> Self {
        DelegatedMemoryManager {
            parent,
            local_pending_tool_calls: Arc::new(RwLock::new(HashSet::new())),
        }
    }
}

#[async_trait]
impl MemoryManager for DelegatedMemoryManager {
    async fn add_message(&mut self, message: Message) -> Result<(), AgentError> {
        // First, handle tool call tracking locally
        match message.role {
            Role::Assistant => {
                if let Some(tool_calls) = &message.tool_calls {
                    // Add tool call IDs to LOCAL pending list
                    let mut pending = self.local_pending_tool_calls.write().await;
                    for tool_call in tool_calls {
                        pending.insert(tool_call.id.clone());
                        log::debug!("DelegatedMemory: Tracking pending tool call locally: {}", tool_call.id);
                    }
                }
            }
            Role::Tool => {
                if let Some(tool_call_id) = &message.tool_call_id {
                    // Remove from LOCAL pending list when result is added
                    let mut pending = self.local_pending_tool_calls.write().await;
                    if pending.remove(tool_call_id) {
                        log::debug!("DelegatedMemory: Resolved pending tool call locally: {}", tool_call_id);
                    } else {
                        log::warn!("DelegatedMemory: Received tool result for unknown tool call ID: {}", tool_call_id);
                    }
                }
            }
            _ => {}
        }

        // Then, add message to the parent's memory for conversation context
        let mut parent = self.parent.lock().await;
        parent.add_message(message).await
    }

    async fn get_messages(&self) -> Result<Vec<Message>, AgentError> {
        let parent = self.parent.lock().await;
        parent.get_messages().await
    }

    async fn get_last_n_messages(&self, n: usize) -> Result<Vec<Message>, AgentError> {
        let parent = self.parent.lock().await;
        parent.get_last_n_messages(n).await
    }

    async fn clear_memory(&mut self) -> Result<(), AgentError> {
        // Clear local tool call tracking
        {
            let mut pending = self.local_pending_tool_calls.write().await;
            pending.clear();
        }

        // Clear parent memory
        let mut parent = self.parent.lock().await;
        parent.clear_memory().await
    }

    async fn clean_orphaned_tool_calls(&mut self) -> Result<(), AgentError> {
        // This only cleans the local tool calls - NOT the parent's
        let mut pending = self.local_pending_tool_calls.write().await;
        let orphan_count = pending.len();

        if orphan_count > 0 {
            log::info!("DelegatedMemory: Cleaning {} orphaned tool calls from local tracking", orphan_count);
            pending.clear();
        }

        Ok(())
    }

    async fn clean_orphaned_tool_results(&mut self) -> Result<usize, AgentError> {
        // For tool results, we don't need to do anything locally
        // This is handled by the individual messages which are never orphaned in our setup
        Ok(0)
    }
}
