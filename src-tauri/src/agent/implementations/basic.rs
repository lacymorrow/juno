//! Contains basic, often in-memory, implementations of agent traits.

use async_trait::async_trait;
use futures::future::BoxFuture;
use futures::FutureExt;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{RwLock};
use tauri::AppHandle;
use serde_json::Value;

use crate::agent::core::{AgentError, Message, MemoryManager, ToolCall, ToolDefinition, ToolProvider, ToolResult};
use crate::agent::tool_logger;

// --- Memory Manager Implementation ---

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

// --- Tool Provider Implementation ---

/// Define an async tool function type
pub type AsyncToolFn = Box<
    dyn Fn(Value) -> BoxFuture<'static, Result<Value, String>> + Send + Sync + 'static
>;

/// A ToolProvider holding tools in memory, supporting async execution.
#[derive(Clone)]
pub struct LocalToolProvider {
    definitions: Arc<RwLock<HashMap<String, ToolDefinition>>>,
    executors: Arc<RwLock<HashMap<String, AsyncToolFn>>>,
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

    /// Registers an async tool with its definition and execution logic.
    pub async fn register_async_tool<F, Fut>(&mut self, definition: ToolDefinition, executor: F)
    where
        F: Fn(Value) -> Fut + Send + Sync + 'static,
        Fut: futures::Future<Output = Result<Value, String>> + Send + 'static,
    {
        let name = definition.name.clone();
        let mut defs = self.definitions.write().await;
        defs.insert(name.clone(), definition);

        // Box the executor into the AsyncToolFn type
        let boxed_executor: AsyncToolFn = Box::new(move |input| Box::pin(executor(input)));

        let mut execs = self.executors.write().await;
        execs.insert(name, boxed_executor);
    }

    /// Deprecated: Registers a synchronous tool. Use register_async_tool instead.
    #[deprecated(note = "Use register_async_tool for all tools going forward")]
    pub async fn register_tool<F>(&mut self, definition: ToolDefinition, executor: F)
    where
        F: Fn(Value) -> Result<Value, String> + Send + Sync + 'static,
    {
        let async_executor = move |input: Value| {
            // Wrap the synchronous function in an async block
            let result = executor(input);
            async move { result } // This future resolves immediately
        };
        self.register_async_tool(definition, async_executor).await;
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
            let app_handle = self.app_handle.clone();
            let tool_name = tool_call.name.clone();
            let tool_input = tool_call.input.clone();
            let call_id = tool_call.id.clone();

            // Note: Directly cloning Arc<dyn Fn(...)> might not be what's intended if you need independent state.
            // However, for stateless functions or shared state within the closure, this is typical.
            // Ensure the captured environment in the original `executor` closure is Sync + Send.
            let executor_clone = executor.clone(); // Cloning the Boxed dyn Fn

            let execution_future = async move {
                executor_clone(tool_input).await // Execute the cloned function object
            };

            let result_output = if let Some(handle) = app_handle {
                tool_logger::log_async_tool_execution(
                    &handle,
                    &tool_name,
                    tool_call.input.clone(),
                    execution_future,
                )
                .await
            } else {
                log::warn!("Executing tool '{}' without logging via app handle.", tool_name);
                match std::panic::AssertUnwindSafe(execution_future).catch_unwind().await {
                    Ok(Ok(output)) => Ok(output),
                    Ok(Err(e)) => Err(e),
                    Err(panic_payload) => {
                         let msg = if let Some(s) = panic_payload.downcast_ref::<&str>() {
                            format!("Tool execution panicked: {}", s)
                        } else if let Some(s) = panic_payload.downcast_ref::<String>() {
                            format!("Tool execution panicked: {}", s)
                        } else {
                            "Tool execution panicked with unknown type".to_string()
                        };
                        Err(msg)
                    }
                }
            };

            match result_output {
                Ok(output) => Ok(ToolResult {
                    call_id,
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
