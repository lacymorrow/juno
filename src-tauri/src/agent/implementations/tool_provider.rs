use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::agent::structs::{AgentError, ToolCall, ToolDefinition, ToolResult};
use crate::agent::traits::ToolProvider;

// TODO: Define a proper Tool executable trait/struct later
type ToolFn = Box<dyn Fn(serde_json::Value) -> Result<serde_json::Value, String> + Send + Sync>;

/// A simple ToolProvider holding tools in memory.
#[derive(Clone)]
pub struct LocalToolProvider {
    // Stores tool definitions (name, description, schema)
    definitions: Arc<RwLock<HashMap<String, ToolDefinition>>>,
    // Stores the actual executable tool functions
    executors: Arc<RwLock<HashMap<String, ToolFn>>>,
}

impl LocalToolProvider {
    pub fn new() -> Self {
        LocalToolProvider {
            definitions: Arc::new(RwLock::new(HashMap::new())),
            executors: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Registers a tool with its definition and execution logic.
    pub async fn register_tool<F>(
        &mut self,
        definition: ToolDefinition,
        executor: F,
    )
    where
        F: Fn(serde_json::Value) -> Result<serde_json::Value, String> + Send + Sync + 'static,
    {
        let name = definition.name.clone();
        let mut defs = self.definitions.write().await;
        defs.insert(name.clone(), definition);
        let mut execs = self.executors.write().await;
        execs.insert(name, Box::new(executor));
    }
}

#[async_trait]
impl ToolProvider for LocalToolProvider {
    async fn list_tools(&self) -> Result<Vec<ToolDefinition>, AgentError> {
        let defs = self.definitions.read().await;
        Ok(defs.values().cloned().collect())
    }

    async fn execute_tool(&self, tool_call: ToolCall) -> Result<ToolResult, AgentError> {
        let execs = self.executors.read().await;
        if let Some(executor) = execs.get(&tool_call.name) {
            match executor(tool_call.input) {
                Ok(output) => Ok(ToolResult {
                    call_id: tool_call.id,
                    output,
                }),
                Err(e) => Err(AgentError::ToolError(format!(
                    "Error executing tool '{}': {}",
                    tool_call.name,
                    e
                ))),
            }
        } else {
            Err(AgentError::ToolNotFound(tool_call.name))
        }
    }
}

impl Default for LocalToolProvider {
    fn default() -> Self {
        Self::new()
    }
}
