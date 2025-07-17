//! Pure Event-Driven Memory Manager
//!
//! Clean, lean implementation that stores conversation history as event streams.
//! No backward compatibility - pure TARS architecture optimized for performance.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use crate::agent::core::{AgentError, Message, Role};
use crate::agent::traits::MemoryManager;
use crate::agent::events::{EventBus, JunoAgentEvent};
use super::event_converter::{EventToMessageConverter, MessageToEventConverter};
use super::persistence::{EventMemoryPersistence, PersistenceConfig};

/// Lean configuration for event-driven memory
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventMemoryConfig {
    /// Maximum events to keep in memory
    pub max_events: usize,
    /// Enable automatic pruning when limit reached
    pub auto_prune: bool,
    /// Token limit before emergency pruning
    pub token_limit: usize,
    /// Minimum events to keep after pruning
    pub min_events_after_prune: usize,
    /// Enable persistence to disk
    pub enable_persistence: bool,
    /// Persistence configuration
    pub persistence_config: Option<PersistenceConfig>,
}

impl Default for EventMemoryConfig {
    fn default() -> Self {
        Self {
            max_events: 1000,
            auto_prune: true,
            token_limit: 100000, // 100K tokens
            min_events_after_prune: 50,
            enable_persistence: true,
            persistence_config: Some(PersistenceConfig::default()),
        }
    }
}

/// Pure event-driven memory manager - lean and fast
#[derive(Clone)]
pub struct EventMemoryManager {
    /// Event bus for conversation storage
    event_bus: Arc<EventBus>,
    /// Event-to-message converter
    converter: Arc<EventToMessageConverter>,
    /// Message-to-event converter
    message_converter: Arc<MessageToEventConverter>,
    /// Configuration
    config: Arc<RwLock<EventMemoryConfig>>,
    /// Pending tool calls for consistency
    pending_tool_calls: Arc<RwLock<HashSet<String>>>,
    /// Simple metrics
    metrics: Arc<RwLock<EventMemoryMetrics>>,
    /// Optional persistence layer for enhanced conversation history
    persistence: Arc<RwLock<Option<EventMemoryPersistence>>>,
    /// Current session ID for persistence
    current_session_id: Arc<RwLock<Option<String>>>,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct EventMemoryMetrics {
    pub total_events: usize,
    pub total_messages: usize,
    pub prune_count: usize,
    pub estimated_tokens: usize,
}

impl EventMemoryManager {
    /// Create new event-driven memory manager
    pub async fn new(event_bus: Arc<EventBus>) -> Result<Self, AgentError> {
        Self::with_config(event_bus, EventMemoryConfig::default()).await
    }

    /// Create with custom configuration
    pub async fn with_config(event_bus: Arc<EventBus>, config: EventMemoryConfig) -> Result<Self, AgentError> {
        // Initialize persistence if enabled
        let persistence = if config.enable_persistence {
            if let Some(persistence_config) = &config.persistence_config {
                match EventMemoryPersistence::new(persistence_config.clone()).await {
                    Ok(p) => Some(p),
                    Err(e) => {
                        warn!("Failed to initialize persistence, continuing without it: {}", e);
                        None
                    }
                }
            } else {
                None
            }
        } else {
            None
        };

        Ok(Self {
            event_bus,
            converter: Arc::new(EventToMessageConverter::new()),
            message_converter: Arc::new(MessageToEventConverter::new()),
            config: Arc::new(RwLock::new(config)),
            pending_tool_calls: Arc::new(RwLock::new(HashSet::new())),
            metrics: Arc::new(RwLock::new(EventMemoryMetrics::default())),
            persistence: Arc::new(RwLock::new(persistence)),
            current_session_id: Arc::new(RwLock::new(None)),
        })
    }

    /// Create a minimal memory manager for testing purposes
    /// This method creates a memory manager without requiring an event bus or app handle
    pub async fn new_for_testing(config: EventMemoryConfig) -> Result<Self, AgentError> {
        // Create a minimal mock event bus or use a test-specific implementation
        // For now, we'll return an error indicating this needs proper implementation
        Err(AgentError::MemoryError(
            "Test memory manager not implemented yet. Use proper test setup with event bus.".to_string()
        ))
    }

    /// Set session ID for message-to-event conversion and persistence
    pub async fn set_session_id(&self, session_id: String) {
        // Update the session ID on the existing converter
        self.message_converter.set_session_id(session_id.clone()).await;

        // Set current session for persistence
        {
            let mut current_session = self.current_session_id.write().await;
            *current_session = Some(session_id.clone());
        }

        // Start tracking this session in persistence
        if let Some(persistence) = &*self.persistence.read().await {
            if let Err(e) = persistence.start_session(session_id.clone()).await {
                warn!("Failed to start session tracking in persistence: {}", e);
            }
        }
    }

    /// Start a new conversation session
    pub async fn start_new_session(&self) -> Result<String, AgentError> {
        let session_id = uuid::Uuid::new_v4().to_string();
        self.set_session_id(session_id.clone()).await;
        info!("Started new conversation session: {}", session_id);
        Ok(session_id)
    }

    /// Load a previous session from persistence
    pub async fn load_session(&self, session_id: &str) -> Result<Vec<Message>, AgentError> {
        if let Some(persistence) = &*self.persistence.read().await {
            let events = persistence.load_session(session_id).await?;

            // Convert events back to messages
            let messages = self.converter.convert_events_to_messages(&events).await
                .map_err(|e| AgentError::MemoryError(format!("Event conversion failed: {}", e)))?;

            // Set this as the current session
            self.set_session_id(session_id.to_string()).await;

            info!("Loaded session {} with {} messages", session_id, messages.len());
            Ok(messages)
        } else {
            Err(AgentError::MemoryError("Persistence not enabled".to_string()))
        }
    }

    /// Get list of available sessions
    pub async fn list_sessions(&self) -> Result<Vec<String>, AgentError> {
        if let Some(persistence) = &*self.persistence.read().await {
            persistence.list_sessions().await
        } else {
            Err(AgentError::MemoryError("Persistence not enabled".to_string()))
        }
    }

    /// Delete a session permanently
    pub async fn delete_session(&self, session_id: &str) -> Result<(), AgentError> {
        if let Some(persistence) = &*self.persistence.read().await {
            persistence.delete_session(session_id).await?;
            info!("Deleted session: {}", session_id);
            Ok(())
        } else {
            Err(AgentError::MemoryError("Persistence not enabled".to_string()))
        }
    }

    /// Force checkpoint current session
    pub async fn checkpoint_current_session(&self) -> Result<(), AgentError> {
        if let Some(session_id) = &*self.current_session_id.read().await {
            if let Some(persistence) = &*self.persistence.read().await {
                persistence.checkpoint_session(session_id).await?;
                info!("Checkpointed current session: {}", session_id);
                Ok(())
            } else {
                Err(AgentError::MemoryError("Persistence not enabled".to_string()))
            }
        } else {
            Err(AgentError::MemoryError("No active session".to_string()))
        }
    }

    /// Get storage statistics
    pub async fn get_storage_stats(&self) -> Result<super::persistence::StorageStats, AgentError> {
        if let Some(persistence) = &*self.persistence.read().await {
            persistence.get_storage_stats().await
        } else {
            Err(AgentError::MemoryError("Persistence not enabled".to_string()))
        }
    }

    /// Estimate token count for current conversation using event bus
    async fn estimate_token_count(&self) -> Result<usize, AgentError> {
        // Use event bus token estimation which is more accurate
        let estimated = self.event_bus.estimate_conversation_tokens().await;

        // Update metrics
        {
            let mut metrics = self.metrics.write().await;
            metrics.estimated_tokens = estimated;
        }

        Ok(estimated)
    }

    /// Simple token estimation (can be enhanced later)
    fn estimate_message_tokens(&self, message: &Message) -> usize {
        let content_tokens = message.content.len() / 4; // Rough estimate: 4 chars per token
        let tool_call_tokens = message.tool_calls.as_ref()
            .map(|calls| calls.len() * 50) // Rough estimate per tool call
            .unwrap_or(0);
        content_tokens + tool_call_tokens + 10 // Base overhead
    }

    /// Prune old events if over limits
    async fn prune_if_needed(&self) -> Result<bool, AgentError> {
        let config = self.config.read().await;

        if !config.auto_prune {
            return Ok(false);
        }

        // Check event count limit
        let stats = self.event_bus.get_stats().await;
        let needs_prune_by_count = stats.total_events > config.max_events;

        // Check token limit
        let token_count = self.estimate_token_count().await?;
        let needs_prune_by_tokens = token_count > config.token_limit;

        if needs_prune_by_count || needs_prune_by_tokens {
            let keep_count = config.min_events_after_prune.max(config.max_events / 2);
            self.event_bus.prune_old_events(keep_count).await
                .map_err(|e| AgentError::MemoryError(format!("Pruning failed: {}", e)))?;

            // Update metrics
            {
                let mut metrics = self.metrics.write().await;
                metrics.prune_count += 1;
            }

            info!("Pruned events: count={}, tokens={}, kept={}",
                  needs_prune_by_count, needs_prune_by_tokens, keep_count);

            return Ok(true);
        }

        Ok(false)
    }

    /// Get memory statistics
    pub async fn get_metrics(&self) -> EventMemoryMetrics {
        let mut metrics = self.metrics.read().await.clone();

        // Update current stats from enhanced event bus
        let stats = self.event_bus.get_stats().await;
        metrics.total_events = stats.total_events;
        metrics.estimated_tokens = stats.estimated_tokens;

        metrics
    }

    /// Clean up orphaned tool calls
    pub async fn cleanup_orphaned_tool_calls(&self) -> Result<usize, AgentError> {
        let events = self.event_bus.get_conversation_events().await
            .map_err(|e| AgentError::MemoryError(format!("Failed to get events: {}", e)))?;

        let mut tool_calls = HashSet::new();
        let mut tool_results = HashSet::new();

        // Collect all tool calls and results
        for event in &events {
            match event {
                JunoAgentEvent::ToolCall { id, .. } => {
                    tool_calls.insert(id.clone());
                }
                JunoAgentEvent::ToolResult { tool_call_id, .. } => {
                    tool_results.insert(tool_call_id.clone());
                }
                _ => {}
            }
        }

        // Find orphaned calls (calls without results)
        let orphaned: Vec<_> = tool_calls.difference(&tool_results).collect();
        let orphaned_count = orphaned.len();

        if orphaned_count > 0 {
            warn!("Found {} orphaned tool calls", orphaned_count);

            // Update pending tool calls
            let mut pending = self.pending_tool_calls.write().await;
            for orphaned_id in orphaned {
                pending.remove(orphaned_id);
            }
        }

        Ok(orphaned_count)
    }

    /// Clear all messages (alias for clear_memory)
    pub async fn clear(&mut self) -> Result<(), AgentError> {
        self.clear_memory().await
    }

    /// Prune memory if needed (alias for prune_if_needed)
    pub async fn prune_memory_if_needed(&mut self) -> Result<bool, AgentError> {
        self.prune_if_needed().await
    }
}

#[async_trait]
impl MemoryManager for EventMemoryManager {
    async fn add_message(&mut self, message: Message) -> Result<(), AgentError> {
        debug!("Adding message to event-driven memory: {:?}", message.role);

        // Track tool calls
        if let Some(tool_calls) = &message.tool_calls {
            let mut pending = self.pending_tool_calls.write().await;
            for tool_call in tool_calls {
                pending.insert(tool_call.id.clone());
            }
        }

        // Remove completed tool calls
        if message.role == Role::Tool {
            if let Some(tool_call_id) = &message.tool_call_id {
                let mut pending = self.pending_tool_calls.write().await;
                pending.remove(tool_call_id);
            }
        }

        // Convert message to events and emit
        let events = self.message_converter.convert_message_to_events(&message).await
            .map_err(|e| AgentError::MemoryError(format!("Message conversion failed: {}", e)))?;

        for event in &events {
            self.event_bus.emit(event.clone()).await
                .map_err(|e| AgentError::MemoryError(format!("Event emission failed: {}", e)))?;
        }

        // Add to persistence if available and we have an active session
        if let Some(session_id) = &*self.current_session_id.read().await {
            if let Some(persistence) = &*self.persistence.read().await {
                if let Err(e) = persistence.add_events(session_id, events).await {
                    warn!("Failed to persist events: {}", e);
                }
            }
        }

        // Update metrics
        {
            let mut metrics = self.metrics.write().await;
            metrics.total_messages += 1;
        }

        // Auto-prune if needed
        self.prune_if_needed().await?;

        Ok(())
    }

    async fn get_messages(&self) -> Result<Vec<Message>, AgentError> {
        debug!("Getting messages from event-driven memory");

        let events = self.event_bus.get_conversation_events().await
            .map_err(|e| AgentError::MemoryError(format!("Failed to get events: {}", e)))?;

        let messages = self.converter.convert_events_to_messages(&events).await
            .map_err(|e| AgentError::MemoryError(format!("Event conversion failed: {}", e)))?;

        debug!("Retrieved {} messages from {} events", messages.len(), events.len());
        Ok(messages)
    }

    async fn get_last_n_messages(&self, n: usize) -> Result<Vec<Message>, AgentError> {
        let all_messages = self.get_messages().await?;
        let start_index = all_messages.len().saturating_sub(n);
        Ok(all_messages[start_index..].to_vec())
    }

    async fn clear_memory(&mut self) -> Result<(), AgentError> {
        debug!("Clearing event-driven memory");

        self.event_bus.clear_events().await;

        // Clear pending tool calls
        {
            let mut pending = self.pending_tool_calls.write().await;
            pending.clear();
        }

        // Reset metrics
        {
            let mut metrics = self.metrics.write().await;
            *metrics = EventMemoryMetrics::default();
        }

        info!("Event-driven memory cleared");
        Ok(())
    }

    async fn clean_orphaned_tool_calls(&mut self) -> Result<(), AgentError> {
        let orphaned_count = self.cleanup_orphaned_tool_calls().await?;
        if orphaned_count > 0 {
            info!("Cleaned {} orphaned tool calls", orphaned_count);
        }
        Ok(())
    }

    async fn clean_orphaned_tool_results(&mut self) -> Result<usize, AgentError> {
        let events = self.event_bus.get_conversation_events().await
            .map_err(|e| AgentError::MemoryError(format!("Failed to get events: {}", e)))?;

        let mut tool_calls = HashSet::new();
        let mut orphaned_results = Vec::new();

        // First pass: collect all tool calls
        for event in &events {
            if let JunoAgentEvent::ToolCall { id, .. } = event {
                tool_calls.insert(id.clone());
            }
        }

        // Second pass: find orphaned results
        for event in &events {
            if let JunoAgentEvent::ToolResult { tool_call_id, .. } = event {
                if !tool_calls.contains(tool_call_id) {
                    orphaned_results.push(tool_call_id.clone());
                }
            }
        }

        let orphaned_count = orphaned_results.len();
        if orphaned_count > 0 {
            warn!("Found {} orphaned tool results", orphaned_count);
            // Note: In a pure event-driven system, we don't actually remove events
            // We just log the inconsistency for monitoring
        }

        Ok(orphaned_count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::core::ToolCall;
    use crate::agent::events::EventBus;
    use serde_json::json;
    use tauri::test::mock_app;

    fn create_test_event_bus() -> Arc<EventBus> {
        let app = mock_app().build();
        Arc::new(EventBus::new(app.handle().clone()))
    }

    #[tokio::test]
    async fn test_event_memory_basic_operations() {
        let event_bus = create_test_event_bus();
        let mut manager = EventMemoryManager::new(event_bus).await.unwrap();

        let message = Message {
            role: Role::User,
            content: "Hello, world!".to_string(),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        };

        // Add message
        manager.add_message(message.clone()).await.unwrap();

        // Get messages
        let messages = manager.get_messages().await.unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].content, "Hello, world!");
        assert_eq!(messages[0].role, Role::User);
    }

    #[tokio::test]
    async fn test_event_memory_tool_tracking() {
        let event_bus = create_test_event_bus();
        let mut manager = EventMemoryManager::new(event_bus).await.unwrap();

        // Add message with tool call
        let tool_call = ToolCall {
            id: "test_call_123".to_string(),
            name: "test_tool".to_string(),
            input: json!({"param": "value"}),
        };

        let message_with_tool = Message {
            role: Role::Assistant,
            content: "Using tool".to_string(),
            tool_calls: Some(vec![tool_call.clone()]),
            tool_call_id: None,
            name: None,
        };

        manager.add_message(message_with_tool).await.unwrap();

        // Check pending tool calls
        {
            let pending = manager.pending_tool_calls.read().await;
            assert!(pending.contains("test_call_123"));
        }

        // Add tool result
        let tool_result = Message {
            role: Role::Tool,
            content: "Tool result".to_string(),
            tool_calls: None,
            tool_call_id: Some("test_call_123".to_string()),
            name: Some("test_tool".to_string()),
        };

        manager.add_message(tool_result).await.unwrap();

        // Check pending tool calls removed
        {
            let pending = manager.pending_tool_calls.read().await;
            assert!(!pending.contains("test_call_123"));
        }

        // Verify conversation structure
        let messages = manager.get_messages().await.unwrap();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, Role::Assistant);
        assert!(messages[0].tool_calls.is_some());
        assert_eq!(messages[1].role, Role::Tool);
        assert_eq!(messages[1].tool_call_id, Some("test_call_123".to_string()));
    }

    #[tokio::test]
    async fn test_event_memory_metrics() {
        let event_bus = create_test_event_bus();
        let mut manager = EventMemoryManager::new(event_bus).await.unwrap();

        let message = Message {
            role: Role::User,
            content: "Test message".to_string(),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        };

        manager.add_message(message).await.unwrap();

        let metrics = manager.get_metrics().await;
        assert_eq!(metrics.total_messages, 1);
        assert!(metrics.estimated_tokens > 0);
    }
}
