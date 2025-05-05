use async_trait::async_trait;
use futures::future::BoxFuture;
use futures::FutureExt;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tauri::AppHandle;
use serde_json::Value;

// Use the consolidated core module
use crate::agent::core::{AgentError, ToolCall, ToolDefinition, ToolResult, ToolProvider};
use crate::agent::tool_logger;

// Define an async tool function type
// It takes a Value input and returns a BoxFuture that resolves to Result<Value, String>
// Needs Send + Sync bounds for async execution
// Add 'static lifetime bound
// Make the type alias public
pub type AsyncToolFn = Box<
    dyn Fn(Value) -> BoxFuture<'static, Result<Value, String>> + Send + Sync + 'static
>;

/// A ToolProvider holding tools in memory, supporting async execution.
#[derive(Clone)]
pub struct LocalToolProvider {
    definitions: Arc<RwLock<HashMap<String, ToolDefinition>>>,
    // Use the AsyncToolFn type
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

            let execution_future = async move {
                executor(tool_input).await
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
                // Use the catch_unwind from FutureExt
                match std::panic::AssertUnwindSafe(execution_future).catch_unwind().await {
                    Ok(Ok(output)) => Ok(output),
                    Ok(Err(e)) => Err(e),
                    Err(_) => Err("Tool execution panicked".to_string()),
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
