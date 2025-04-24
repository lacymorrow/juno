use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::agent::structs::{AgentError, Message};
use crate::agent::traits::MemoryManager;

/// A simple in-memory implementation of the MemoryManager trait.
#[derive(Debug, Clone)]
pub struct SimpleMemoryManager {
    messages: Arc<RwLock<Vec<Message>>>,
}

impl SimpleMemoryManager {
    pub fn new() -> Self {
        SimpleMemoryManager {
            messages: Arc::new(RwLock::new(Vec::new())),
        }
    }
}

#[async_trait]
impl MemoryManager for SimpleMemoryManager {
    async fn add_message(&mut self, message: Message) -> Result<(), AgentError> {
        let mut messages = self.messages.write().await;
        messages.push(message);
        Ok(())
    }

    async fn get_messages(&self) -> Result<Vec<Message>, AgentError> {
        let messages = self.messages.read().await;
        Ok(messages.clone())
    }

    async fn get_last_n_messages(&self, n: usize) -> Result<Vec<Message>, AgentError> {
        let messages = self.messages.read().await;
        let start_index = messages.len().saturating_sub(n);
        Ok(messages[start_index..].to_vec())
    }

    async fn clear_memory(&mut self) -> Result<(), AgentError> {
        let mut messages = self.messages.write().await;
        messages.clear();
        Ok(())
    }
}

impl Default for SimpleMemoryManager {
    fn default() -> Self {
        Self::new()
    }
}
