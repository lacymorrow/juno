// Fixed version of memory manager to prevent race conditions

use crate::agent::core::{AgentError, Memory, Message, Role};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::{RwLock, Mutex};
use tracing::{debug, info, warn};
use std::time::Instant;
use std::collections::HashMap;

/// Thread-safe memory manager with atomic operations
pub struct AtomicMemoryManager {
    /// Messages stored with read-write lock for better concurrency
    messages: Arc<RwLock<Vec<Message>>>,
    
    /// Pending tool calls with dedicated lock
    pending_tool_calls: Arc<Mutex<std::collections::HashSet<String>>>,
    
    /// Configuration
    config: MemoryConfig,
    
    /// Metrics with atomic updates
    metrics: Arc<Mutex<MemoryMetrics>>,
    
    /// Lock for pruning operations to prevent concurrent pruning
    pruning_lock: Arc<Mutex<()>>,
}

#[derive(Clone)]
struct MemoryConfig {
    max_messages: usize,
    max_tokens: usize,
    enable_auto_pruning: bool,
    enable_compression: bool,
}

#[derive(Default)]
struct MemoryMetrics {
    total_messages: usize,
    total_pruned: usize,
    add_message_calls: usize,
    retrieve_calls: usize,
    prune_calls: usize,
    last_pruned: Option<Instant>,
}

impl AtomicMemoryManager {
    pub fn new(max_messages: usize) -> Self {
        Self {
            messages: Arc::new(RwLock::new(Vec::new())),
            pending_tool_calls: Arc::new(Mutex::new(std::collections::HashSet::new())),
            config: MemoryConfig {
                max_messages,
                max_tokens: max_messages * 1000, // Rough estimate
                enable_auto_pruning: true,
                enable_compression: false,
            },
            metrics: Arc::new(Mutex::new(MemoryMetrics::default())),
            pruning_lock: Arc::new(Mutex::new(())),
        }
    }

    /// Add message with atomic operations
    pub async fn add_message(&self, message: Message) -> Result<(), AgentError> {
        // First, handle tool call tracking atomically
        match message.role {
            Role::Assistant => {
                if let Some(tool_calls) = &message.tool_calls {
                    let mut pending = self.pending_tool_calls.lock().await;
                    for tool_call in tool_calls {
                        pending.insert(tool_call.id.clone());
                        debug!("Tracking pending tool call: {}", tool_call.id);
                    }
                }
            }
            Role::Tool => {
                if let Some(tool_call_id) = &message.tool_call_id {
                    let mut pending = self.pending_tool_calls.lock().await;
                    if !pending.remove(tool_call_id) {
                        warn!("Received tool result for unknown tool call ID: {}", tool_call_id);
                    }
                }
            }
            _ => {}
        }

        // Add message with write lock
        {
            let mut messages = self.messages.write().await;
            messages.push(message.clone());
        }

        // Update metrics
        {
            let mut metrics = self.metrics.lock().await;
            metrics.total_messages += 1;
            metrics.add_message_calls += 1;
        }

        // Auto-prune if needed (with pruning lock)
        if self.config.enable_auto_pruning {
            self.prune_if_needed().await?;
        }

        Ok(())
    }

    /// Retrieve messages with read lock (allows concurrent reads)
    pub async fn get_messages(&self) -> Vec<Message> {
        let messages = self.messages.read().await;
        
        // Update metrics
        if let Ok(mut metrics) = self.metrics.lock().await {
            metrics.retrieve_calls += 1;
        }
        
        messages.clone()
    }

    /// Prune messages atomically
    async fn prune_if_needed(&self) -> Result<(), AgentError> {
        // Use pruning lock to prevent concurrent pruning
        let _pruning_guard = self.pruning_lock.lock().await;
        
        // Check if pruning is needed with read lock first
        let needs_pruning = {
            let messages = self.messages.read().await;
            messages.len() > self.config.max_messages
        };

        if !needs_pruning {
            return Ok(());
        }

        // Perform pruning with write lock
        let pruned_count = {
            let mut messages = self.messages.write().await;
            
            if messages.len() <= self.config.max_messages {
                // Double-check after acquiring write lock
                return Ok(());
            }

            // Calculate how many to prune
            let excess = messages.len() - self.config.max_messages;
            
            // Preserve system messages and recent messages
            let mut preserved_messages = Vec::new();
            let mut prunable_indices = Vec::new();
            
            for (idx, msg) in messages.iter().enumerate() {
                if msg.role == Role::System || (messages.len() - idx) <= self.config.max_messages / 2 {
                    preserved_messages.push(idx);
                } else {
                    prunable_indices.push(idx);
                }
            }

            // Prune oldest prunable messages
            let to_prune = prunable_indices.into_iter()
                .take(excess)
                .collect::<Vec<_>>();
            
            // Remove in reverse order to maintain indices
            for idx in to_prune.iter().rev() {
                messages.remove(*idx);
            }

            to_prune.len()
        };

        // Update metrics
        {
            let mut metrics = self.metrics.lock().await;
            metrics.total_pruned += pruned_count;
            metrics.prune_calls += 1;
            metrics.last_pruned = Some(Instant::now());
        }

        info!("Pruned {} messages to maintain limit of {}", pruned_count, self.config.max_messages);
        Ok(())
    }

    /// Get pending tool calls atomically
    pub async fn get_pending_tool_calls(&self) -> Vec<String> {
        let pending = self.pending_tool_calls.lock().await;
        pending.iter().cloned().collect()
    }

    /// Clear all messages atomically
    pub async fn clear(&self) -> Result<(), AgentError> {
        let mut messages = self.messages.write().await;
        messages.clear();
        
        let mut pending = self.pending_tool_calls.lock().await;
        pending.clear();
        
        Ok(())
    }

    /// Get metrics snapshot
    pub async fn get_metrics(&self) -> MemoryMetricsSnapshot {
        let metrics = self.metrics.lock().await;
        let messages = self.messages.read().await;
        
        MemoryMetricsSnapshot {
            current_messages: messages.len(),
            total_messages: metrics.total_messages,
            total_pruned: metrics.total_pruned,
            add_message_calls: metrics.add_message_calls,
            retrieve_calls: metrics.retrieve_calls,
            prune_calls: metrics.prune_calls,
            last_pruned_ago_secs: metrics.last_pruned
                .map(|t| t.elapsed().as_secs())
                .unwrap_or(0),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct MemoryMetricsSnapshot {
    pub current_messages: usize,
    pub total_messages: usize,
    pub total_pruned: usize,
    pub add_message_calls: usize,
    pub retrieve_calls: usize,
    pub prune_calls: usize,
    pub last_pruned_ago_secs: u64,
}

/// Advanced memory manager with compression and intelligent pruning
pub struct AdvancedMemoryManager {
    inner: AtomicMemoryManager,
    compression_cache: Arc<Mutex<HashMap<String, String>>>,
}

impl AdvancedMemoryManager {
    pub fn new(max_messages: usize) -> Self {
        Self {
            inner: AtomicMemoryManager::new(max_messages),
            compression_cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Add message with optional compression
    pub async fn add_message(&self, mut message: Message) -> Result<(), AgentError> {
        // Compress long messages if enabled
        if self.inner.config.enable_compression && message.content.len() > 1000 {
            let compressed = self.compress_content(&message.content).await;
            message.content = compressed;
        }

        self.inner.add_message(message).await
    }

    /// Compress content (placeholder - implement actual compression)
    async fn compress_content(&self, content: &str) -> String {
        // For now, just truncate very long content
        if content.len() > 5000 {
            format!("{}... [truncated from {} chars]", &content[..5000], content.len())
        } else {
            content.to_string()
        }
    }

    /// Get messages with decompression
    pub async fn get_messages(&self) -> Vec<Message> {
        let messages = self.inner.get_messages().await;
        
        // In a real implementation, we would decompress messages here
        messages
    }

    /// Intelligent pruning that preserves important context
    pub async fn intelligent_prune(&self) -> Result<(), AgentError> {
        let _pruning_guard = self.inner.pruning_lock.lock().await;
        
        let mut messages = self.inner.messages.write().await;
        
        if messages.len() <= self.inner.config.max_messages {
            return Ok(());
        }

        // Group messages by importance
        let mut system_messages = Vec::new();
        let mut tool_messages = Vec::new();
        let mut recent_messages = Vec::new();
        let mut other_messages = Vec::new();
        
        let total = messages.len();
        for (idx, msg) in messages.iter().enumerate() {
            if msg.role == Role::System {
                system_messages.push((idx, msg));
            } else if msg.role == Role::Tool || msg.tool_calls.is_some() {
                tool_messages.push((idx, msg));
            } else if total - idx <= 10 { // Keep last 10 messages
                recent_messages.push((idx, msg));
            } else {
                other_messages.push((idx, msg));
            }
        }

        // Rebuild message list with priority
        let mut new_messages = Vec::new();
        
        // Always keep system messages
        for (_, msg) in system_messages {
            new_messages.push(msg.clone());
        }
        
        // Keep recent messages
        for (_, msg) in recent_messages {
            new_messages.push(msg.clone());
        }
        
        // Keep as many tool messages as possible
        let remaining_space = self.inner.config.max_messages.saturating_sub(new_messages.len());
        for (_, msg) in tool_messages.into_iter().rev().take(remaining_space) {
            new_messages.push(msg.clone());
        }
        
        // Fill remaining space with other messages (most recent first)
        let remaining_space = self.inner.config.max_messages.saturating_sub(new_messages.len());
        for (_, msg) in other_messages.into_iter().rev().take(remaining_space) {
            new_messages.push(msg.clone());
        }

        // Sort by original order
        new_messages.sort_by_key(|msg| {
            messages.iter().position(|m| m == msg).unwrap_or(0)
        });

        let pruned = messages.len() - new_messages.len();
        *messages = new_messages;

        info!("Intelligently pruned {} messages", pruned);
        Ok(())
    }
}

#[async_trait]
impl Memory for AtomicMemoryManager {
    async fn add_message(&mut self, message: Message) -> Result<(), AgentError> {
        AtomicMemoryManager::add_message(self, message).await
    }

    async fn get_messages(&self) -> Result<Vec<Message>, AgentError> {
        Ok(AtomicMemoryManager::get_messages(self).await)
    }

    async fn clear(&mut self) -> Result<(), AgentError> {
        AtomicMemoryManager::clear(self).await
    }

    async fn get_pending_tool_calls(&self) -> Result<Vec<String>, AgentError> {
        Ok(AtomicMemoryManager::get_pending_tool_calls(self).await)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_concurrent_add_messages() {
        let manager = Arc::new(AtomicMemoryManager::new(10));
        
        // Spawn multiple tasks adding messages concurrently
        let mut handles = vec![];
        
        for i in 0..20 {
            let manager_clone = manager.clone();
            let handle = tokio::spawn(async move {
                let message = Message {
                    role: Role::User,
                    content: format!("Message {}", i),
                    tool_calls: None,
                    tool_call_id: None,
                    name: None,
                };
                manager_clone.add_message(message).await
            });
            handles.push(handle);
        }
        
        // Wait for all tasks
        for handle in handles {
            assert!(handle.await.unwrap().is_ok());
        }
        
        // Check that we have exactly max_messages
        let messages = manager.get_messages().await;
        assert_eq!(messages.len(), 10);
    }

    #[tokio::test]
    async fn test_no_double_pruning() {
        let manager = Arc::new(AtomicMemoryManager::new(5));
        
        // Add messages that will trigger pruning
        for i in 0..10 {
            let message = Message {
                role: Role::User,
                content: format!("Message {}", i),
                tool_calls: None,
                tool_call_id: None,
                name: None,
            };
            manager.add_message(message).await.unwrap();
        }
        
        // Verify metrics
        let metrics = manager.get_metrics().await;
        assert_eq!(metrics.current_messages, 5);
        assert!(metrics.total_pruned >= 5);
    }
}