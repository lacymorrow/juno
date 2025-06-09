use async_trait::async_trait;
use std::sync::Arc;
use std::collections::HashSet;
use tokio::sync::RwLock;

use crate::agent::structs::{AgentError, Message, Role, ToolCall};
use crate::agent::traits::MemoryManager;

/// A simple in-memory implementation of the MemoryManager trait.
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
}

impl Default for SimpleMemoryManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_orphaned_tool_call_cleanup() {
        let mut memory = SimpleMemoryManager::new();

        // Add a user message
        memory.add_message(Message {
            role: Role::User,
            content: "Test query".to_string(),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        }).await.unwrap();

        // Add an assistant message with tool calls
        memory.add_message(Message {
            role: Role::Assistant,
            content: "I'll help you with that".to_string(),
            tool_calls: Some(vec![
                ToolCall {
                    id: "tool_1".to_string(),
                    name: "test_tool".to_string(),
                    input: json!({"param": "value"}),
                },
                ToolCall {
                    id: "tool_2".to_string(),
                    name: "another_tool".to_string(),
                    input: json!({"param": "value2"}),
                }
            ]),
            tool_call_id: None,
            name: None,
        }).await.unwrap();

        // Add result for only one tool call (tool_1)
        memory.add_message(Message {
            role: Role::Tool,
            content: "Tool result".to_string(),
            tool_calls: None,
            tool_call_id: Some("tool_1".to_string()),
            name: Some("test_tool".to_string()),
        }).await.unwrap();

        // Check that we have 3 messages and 1 pending tool call
        let messages = memory.get_messages().await.unwrap();
        assert_eq!(messages.len(), 3);

        let pending = memory.get_pending_tool_calls().await.unwrap();
        assert_eq!(pending.len(), 1);
        assert!(pending.contains(&"tool_2".to_string()));

        // Clean up orphaned tool calls
        memory.clean_orphaned_tool_calls().await.unwrap();

        // Check that the orphaned assistant message was removed
        let messages = memory.get_messages().await.unwrap();
        assert_eq!(messages.len(), 2); // User message and tool result message should remain

        // Check message types
        assert_eq!(messages[0].role, Role::User);
        assert_eq!(messages[1].role, Role::Tool);

        // Check that pending tool calls were cleaned up
        let pending = memory.get_pending_tool_calls().await.unwrap();
        assert_eq!(pending.len(), 0);
    }

    #[tokio::test]
    async fn test_clean_orphaned_tool_calls_with_all_resolved() {
        let mut memory = SimpleMemoryManager::new();

        // Add messages with fully resolved tool calls
        memory.add_message(Message {
            role: Role::Assistant,
            content: "I'll help".to_string(),
            tool_calls: Some(vec![
                ToolCall {
                    id: "tool_1".to_string(),
                    name: "test_tool".to_string(),
                    input: json!({"param": "value"}),
                }
            ]),
            tool_call_id: None,
            name: None,
        }).await.unwrap();

        memory.add_message(Message {
            role: Role::Tool,
            content: "Tool result".to_string(),
            tool_calls: None,
            tool_call_id: Some("tool_1".to_string()),
            name: Some("test_tool".to_string()),
        }).await.unwrap();

        // Clean up should not remove anything
        memory.clean_orphaned_tool_calls().await.unwrap();

        let messages = memory.get_messages().await.unwrap();
        assert_eq!(messages.len(), 2); // Both messages should remain

        let pending = memory.get_pending_tool_calls().await.unwrap();
        assert_eq!(pending.len(), 0); // No pending tool calls
    }
}
