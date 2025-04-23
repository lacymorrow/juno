use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tauri::AppHandle;

use crate::agent::structs::{AgentError, ToolCall, ToolDefinition, ToolResult};
use crate::agent::traits::ToolProvider;
use crate::agent::tool_logger;

// TODO: Define a proper Tool executable trait/struct later
type ToolFn = Box<dyn Fn(serde_json::Value) -> Result<serde_json::Value, String> + Send + Sync>;

/// A simple ToolProvider holding tools in memory.
#[derive(Clone)]
pub struct LocalToolProvider {
    // Stores tool definitions (name, description, schema)
    definitions: Arc<RwLock<HashMap<String, ToolDefinition>>>,
    // Stores the actual executable tool functions
    executors: Arc<RwLock<HashMap<String, ToolFn>>>,
    // App handle to emit events
    app_handle: Option<AppHandle>,
}

impl LocalToolProvider {
    pub fn new() -> Self {
        LocalToolProvider {
            definitions: Arc::new(RwLock::new(HashMap::new())),
            executors: Arc::new(RwLock::new(HashMap::new())),
            app_handle: None,
        }
    }

    /// Create a tool provider with an app handle for emitting events
    pub fn with_app_handle(app_handle: AppHandle) -> Self {
        LocalToolProvider {
            definitions: Arc::new(RwLock::new(HashMap::new())),
            executors: Arc::new(RwLock::new(HashMap::new())),
            app_handle: Some(app_handle),
        }
    }

    /// Set the app handle for emitting events
    pub fn set_app_handle(&mut self, app_handle: AppHandle) {
        self.app_handle = Some(app_handle);
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
            // Use the tool logger if we have an app handle
            if let Some(app_handle) = &self.app_handle {
                match tool_logger::log_tool_execution(
                    app_handle,
                    &tool_call.name,
                    tool_call.input.clone(),
                    |input| executor(input),
                ).await {
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
                // Fall back to direct execution without logging if no app handle
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
