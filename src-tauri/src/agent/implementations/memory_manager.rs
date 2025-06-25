use crate::agent::core::{
    AgentError,
    Message,
    Role,
};
use crate::agent::traits::MemoryManager;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use tokio::sync::RwLock;
use std::sync::Arc;
use std::time::{Instant, Duration, SystemTime};
use uuid::Uuid;

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
            max_messages: 100,
            max_tokens: 32000, // Conservative estimate for context window
            min_messages_to_keep: 10,
            auto_prune: true,
            enable_summarization: true,
            summarization_batch_size: 10,
            enable_metrics: true,
            enable_summary_cache: true,
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

/// Enhanced in-memory implementation of the MemoryManager trait with advanced features
#[derive(Debug, Clone)]
pub struct AdvancedMemoryManager {
    messages: Arc<RwLock<Vec<Message>>>,
    pending_tool_calls: Arc<RwLock<HashSet<String>>>,
    config: Arc<RwLock<MemoryConfig>>,
    metrics: Arc<RwLock<MemoryMetrics>>,
    summaries: Arc<RwLock<Vec<ConversationSummary>>>,
    summary_cache: Arc<RwLock<std::collections::HashMap<String, String>>>,
}

impl AdvancedMemoryManager {
    pub fn new() -> Self {
        Self::with_config(MemoryConfig::default())
    }

    pub fn with_config(config: MemoryConfig) -> Self {
        AdvancedMemoryManager {
            messages: Arc::new(RwLock::new(Vec::new())),
            pending_tool_calls: Arc::new(RwLock::new(HashSet::new())),
            config: Arc::new(RwLock::new(config)),
            metrics: Arc::new(RwLock::new(MemoryMetrics::default())),
            summaries: Arc::new(RwLock::new(Vec::new())),
            summary_cache: Arc::new(RwLock::new(std::collections::HashMap::new())),
        }
    }

    /// Estimate token count for a message (rough approximation)
    fn estimate_message_tokens(message: &Message) -> usize {
        let content_tokens = message.content.len() / 4; // Rough estimate: 4 chars per token
        let tool_call_tokens = message.tool_calls.as_ref()
            .map(|calls| calls.iter().map(|call| call.name.len() / 4 + 50).sum()) // Extra for structure
            .unwrap_or(0);
        content_tokens + tool_call_tokens
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

        // Check if pruning is needed
        if messages_count <= config.max_messages && estimated_tokens <= config.max_tokens {
            return Ok(false);
        }

        drop(config); // Release config lock before calling prune_memory

        log::info!("Memory pruning triggered: {} messages, ~{} tokens",
                   messages_count, estimated_tokens);

        self.prune_memory(None).await?;
        Ok(true)
    }

    /// Create a summary of conversation segments
    async fn create_conversation_summary(&self, messages: &[Message]) -> Result<ConversationSummary, AgentError> {
        if messages.is_empty() {
            return Err(AgentError::ConfigurationError("Cannot summarize empty message list".to_string()));
        }

        // Simple summarization logic (in production, you'd use an LLM)
        let summary = if messages.len() <= 3 {
            // For short conversations, just concatenate key points
            messages.iter()
                .filter(|m| !m.content.is_empty())
                .map(|m| {
                    let content = if m.content.len() > 100 {
                        format!("{}...", &m.content[..100])
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
            SystemTime::now() - Duration::from_secs(3600), // Approximate start
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
        let common_words = ["the", "and", "or", "but", "in", "on", "at", "to", "for", "of", "with", "by"];
        let words: Vec<&str> = text.split_whitespace()
            .filter(|word| word.len() > 3 && !common_words.contains(&word.to_lowercase().as_str()))
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
            .take(5)
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
                        if summaries.len() > 20 {
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
            if cache.len() > 100 {
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
        let hot_context_size = std::cmp::min(10, config.min_messages_to_keep);
        let start_index = messages.len().saturating_sub(hot_context_size);
        let hot_messages = messages[start_index..].to_vec();

        Ok(hot_messages)
    }

    /// Get cold context on demand (background loading pattern)
    pub async fn get_cold_context_async(&self) -> Result<Vec<Message>, AgentError> {
        let messages = self.messages.read().await;
        let config = self.config.read().await;

        // Cold context: everything before the hot context window
        let hot_context_size = std::cmp::min(10, config.min_messages_to_keep);
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

        // Update metrics
        if !orphaned_tool_call_ids.is_empty() {
            let mut metrics = self.metrics.write().await;
            metrics.orphaned_tool_calls_cleaned += orphaned_tool_call_ids.len();

            log::info!("Cleaned up {} orphaned tool calls: {:?}",
                       orphaned_tool_call_ids.len(), orphaned_tool_call_ids);
        }

        self.update_metrics(start_time).await?;
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
            log::info!("Cleaned up {} orphaned tool results: {:?}", orphaned_count, orphaned_ids);

            // Update metrics
            let mut metrics = self.metrics.write().await;
            metrics.orphaned_tool_calls_cleaned += orphaned_count;
        }

        self.update_metrics(start_time).await?;
        Ok(orphaned_count)
    }
}

#[async_trait]
impl MemoryManager for AdvancedMemoryManager {
    async fn add_message(&mut self, message: Message) -> Result<(), AgentError> {
        let start_time = Instant::now();
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

        // Check if pruning is needed
        self.prune_memory_if_needed().await?;
        self.update_metrics(start_time).await?;

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

        self.update_metrics(start_time).await?;
        Ok(result)
    }

    async fn get_last_n_messages(&self, n: usize) -> Result<Vec<Message>, AgentError> {
        let start_time = Instant::now();
        let messages = self.messages.read().await;
        let start_index = messages.len().saturating_sub(n);
        let result = messages[start_index..].to_vec();
        drop(messages);

        self.update_metrics(start_time).await?;
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

        // Reset metrics
        {
            let mut metrics = self.metrics.write().await;
            *metrics = MemoryMetrics::default();
        }

        self.update_metrics(start_time).await?;
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
    async fn add_message(&mut self, message: Message) -> Result<(), AgentError> {
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
        log::info!("Memory: Added message. Role={:?}, Total_count={}, Pending_tool_calls={}",
                   message.role, messages.len(), pending.len());
        Ok(())
    }

    async fn get_messages(&self) -> Result<Vec<Message>, AgentError> {
        let messages = self.messages.read().await;
        let pending = self.pending_tool_calls.read().await;
        log::info!("Memory: Retrieved {} messages, {} pending tool calls", messages.len(), pending.len());
        Ok(messages.clone())
    }

    async fn get_last_n_messages(&self, n: usize) -> Result<Vec<Message>, AgentError> {
        let messages = self.messages.read().await;
        let start_index = messages.len().saturating_sub(n);
        Ok(messages[start_index..].to_vec())
    }

    async fn clear_memory(&mut self) -> Result<(), AgentError> {
        let mut messages = self.messages.write().await;
        let mut pending = self.pending_tool_calls.write().await;
        messages.clear();
        pending.clear();
        log::info!("Memory: Cleared all messages and pending tool calls");
        Ok(())
    }

    async fn clean_orphaned_tool_calls(&mut self) -> Result<(), AgentError> {
        // Call the actual implementation method
        SimpleMemoryManager::clean_orphaned_tool_calls(self).await
    }

    async fn clean_orphaned_tool_results(&mut self) -> Result<usize, AgentError> {
        // Call the actual implementation method
        SimpleMemoryManager::clean_orphaned_tool_results(self).await
    }
}

impl Default for SimpleMemoryManager {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for AdvancedMemoryManager {
    fn default() -> Self {
        Self::new()
    }
}
