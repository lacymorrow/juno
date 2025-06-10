use async_trait::async_trait;
use futures::future::BoxFuture;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::sync::Mutex;
use tauri::AppHandle;
use tauri::Manager;
use serde_json::Value;
use tracing::{debug, error, info, warn};
use std::future::Future;
use std::pin::Pin;
use crate::agent::traits::{MemoryManager, ToolProvider};
use crate::agent::tools::mcp_integration::MCPManager;
use crate::agent::structs::{AgentError, ToolCall, ToolDefinition, ToolResult};
use crate::agent::tool_logger;
use crate::state::AppState;

// Define an async tool function type
// It takes a Value input and returns a BoxFuture that resolves to Result<Value, String>
// Needs Send + Sync bounds for async execution
// Add 'static lifetime bound
// Make the type alias public
pub type AsyncToolFn = Box<
    dyn Fn(Value) -> BoxFuture<'static, Result<Value, String>> + Send + Sync + 'static
>;

/// Type alias for asynchronous tool executors
pub type AsyncToolExecutor = Arc<dyn Fn(Value) -> Pin<Box<dyn Future<Output = Result<Value, String>> + Send>> + Send + Sync>;

/// A ToolProvider holding tools in memory, supporting async execution and MCP integration.
#[derive(Clone)]
pub struct LocalToolProvider {
    definitions: Arc<RwLock<HashMap<String, ToolDefinition>>>,
    // Use the AsyncToolFn type
    executors: Arc<RwLock<HashMap<String, AsyncToolExecutor>>>,
    app_handle: Option<AppHandle>,
    mcp_manager: Option<Arc<Mutex<MCPManager>>>,
}

impl LocalToolProvider {
    pub fn new() -> Self {
        Self {
            definitions: Arc::new(RwLock::new(HashMap::new())),
            executors: Arc::new(RwLock::new(HashMap::new())),
            app_handle: None,
            mcp_manager: None,
        }
    }

    /// Create a new tool provider with an app handle
    pub fn with_app_handle(app_handle: AppHandle) -> Self {
        Self {
            definitions: Arc::new(RwLock::new(HashMap::new())),
            executors: Arc::new(RwLock::new(HashMap::new())),
            app_handle: Some(app_handle),
            mcp_manager: None,
        }
    }

    /// Add MCP manager to an existing provider
    pub fn with_mcp_manager(mut self, mcp_manager: Arc<Mutex<MCPManager>>) -> Self {
        self.mcp_manager = Some(mcp_manager);
        self
    }

    /// Set app handle on existing provider
    pub fn set_app_handle(&mut self, app_handle: AppHandle) {
        self.app_handle = Some(app_handle);
    }

    /// Set MCP manager on existing provider
    pub fn set_mcp_manager(&mut self, mcp_manager: Arc<Mutex<MCPManager>>) {
        self.mcp_manager = Some(mcp_manager);
    }

    /// Register an asynchronous tool with this provider
    pub async fn register_async_tool<F, Fut>(&self, definition: ToolDefinition, executor: F)
    where
        F: Fn(Value) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<Value, String>> + Send + 'static,
    {
        let tool_name = definition.name.clone();

        // Store the definition
        {
            let mut definitions = self.definitions.write().await;
            definitions.insert(tool_name.clone(), definition);
        }

        // Wrap the executor in an Arc and store it
        let wrapped_executor: AsyncToolExecutor = Arc::new(move |input| {
            let fut = executor(input);
            Box::pin(fut)
        });

        {
            let mut executors = self.executors.write().await;
            executors.insert(tool_name.clone(), wrapped_executor);
        }

        debug!("Registered async tool: {}", tool_name);
    }

    /// Registers an async tool with configuration awareness
    pub async fn register_async_tool_with_config<F, Fut>(
        &mut self,
        definition: ToolDefinition,
        executor: F,
        app_state: Option<&AppState>
    )
    where
        F: Fn(Value) -> Fut + Send + Sync + 'static,
        Fut: futures::Future<Output = Result<Value, String>> + Send + 'static,
    {
        let _name = definition.name.clone();

        // Check if tool should be enabled based on configuration
        let should_register = if let Some(state) = app_state {
            let _config_manager = state.get_tool_config_manager().await;
            // For now, register all tools - configuration filtering will be implemented later
            true
        } else {
            true // Default to enabled if no state available
        };

        if should_register {
            self.register_async_tool(definition, executor).await;
        }
    }

    /// Helper method to register tools from an AppState with configuration checking
    pub async fn register_async_tool_from_state<F, Fut>(
        &mut self,
        definition: ToolDefinition,
        executor: F,
        app_state: Option<&AppState>
    )
    where
        F: Fn(Value) -> Fut + Send + Sync + 'static,
        Fut: futures::Future<Output = Result<Value, String>> + Send + 'static,
    {
        let _config_manager = if let Some(state) = app_state {
            let config_arc = state.get_tool_config_manager().await;
            let config_guard = config_arc.lock().await;
            let is_enabled = config_guard.is_tool_enabled(&definition.name);
            drop(config_guard); // Release the lock early

            if !is_enabled {
                tracing::debug!("Tool '{}' is disabled, skipping registration", definition.name);
                return;
            }
        };

        self.register_async_tool(definition, executor).await;
    }

    /// Refresh MCP tools from connected servers
    pub async fn refresh_mcp_tools(&mut self) -> Result<(), String> {
        if let Some(ref mcp_manager) = self.mcp_manager {
            let manager_guard = mcp_manager.lock().await;
            let mcp_tools = manager_guard.get_all_tools().await;
            drop(manager_guard);

            // Clear existing MCP tools first (they have prefixed names)
            let mut defs = self.definitions.write().await;
            defs.retain(|name, _| !name.contains("mcp-server-"));

            // Add fresh MCP tools to our local definitions
            let mut added_count = 0;
            for tool_info in mcp_tools {
                if tool_info.enabled {
                    defs.insert(tool_info.tool_definition.name.clone(), tool_info.tool_definition);
                    added_count += 1;
                }
            }

            log::info!("Refreshed and cached {} MCP tools in provider definitions", added_count);
        }
        Ok(())
    }

    /// Check if a tool is an MCP tool
    fn is_mcp_tool(&self, tool_name: &str) -> bool {
        // MCP tools are prefixed with server name
        tool_name.contains('_') &&
        self.mcp_manager.is_some() &&
        !self.executors.try_read().map(|execs| execs.contains_key(tool_name)).unwrap_or(false)
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

    /// Get error recovery statistics (placeholder for future implementation)
    pub async fn get_recovery_stats(&self) -> Value {
        serde_json::json!({
            "error_recovery": "not_implemented",
            "note": "Error recovery will be implemented in future iterations"
        })
    }

    /// Clear error recovery history (placeholder for future implementation)
    pub async fn clear_recovery_history(&self) {
        // Placeholder for future error recovery implementation
        tracing::debug!("Error recovery history clear requested - feature not yet implemented");
    }
}

#[async_trait]
impl ToolProvider for LocalToolProvider {
    async fn list_tools(&self) -> Result<Vec<ToolDefinition>, AgentError> {
        let mut all_tools = Vec::new();

        // Get local tools (which includes previously cached MCP tools)
        let defs = self.definitions.read().await;
        all_tools.extend(defs.values().cloned());
        drop(defs);

        // Only fetch MCP tools if we don't have any cached and we have an MCP manager
        if let Some(ref mcp_manager) = self.mcp_manager {
            // Check if we already have MCP tools cached (they have prefixed names)
            let has_mcp_tools = all_tools.iter().any(|tool| tool.name.contains("mcp-server-"));

            if !has_mcp_tools {
                let manager_guard = mcp_manager.lock().await;
                let mcp_tools = manager_guard.get_all_tools().await;
                drop(manager_guard);

                for tool_info in mcp_tools {
                    if tool_info.enabled {
                        all_tools.push(tool_info.tool_definition);
                    }
                }

                log::debug!("Fetched {} fresh MCP tools", all_tools.iter().filter(|t| t.name.contains("mcp-server-")).count());
            } else {
                log::debug!("Using cached MCP tools, skipping fresh fetch");
            }
        }

        // Debug logging to identify duplicates
        let mut tool_names = std::collections::HashSet::new();
        let mut duplicates = Vec::new();

        for tool in &all_tools {
            if !tool_names.insert(tool.name.clone()) {
                duplicates.push(tool.name.clone());
            }
        }

        if !duplicates.is_empty() {
            log::error!("DUPLICATE TOOLS DETECTED: {:?}", duplicates);
            log::debug!("All tool names: {:?}", all_tools.iter().map(|t| &t.name).collect::<Vec<_>>());
        } else {
            log::debug!("All {} tools are unique", all_tools.len());
        }

        Ok(all_tools)
    }

    async fn execute_tool(&self, tool_call: ToolCall) -> Result<ToolResult, AgentError> {
        let tool_name = &tool_call.name;
        let start_time = std::time::Instant::now(); // Track execution time

        // Use enhanced tool call request logging if app handle is available
        if let Some(ref app_handle) = self.app_handle {
            // Get app_state from app_handle when needed
            let app_state_ref = app_handle.try_state::<crate::state::AppState>();
            let app_state_option = app_state_ref.as_ref().map(|s| s.inner());

            // Use enhanced logging with tool metadata
            tool_logger::log_enhanced_tool_call_request(
                app_handle,
                tool_name,
                tool_call.input.clone(),
                None,
                app_state_option,
            ).await;
        } else {
            // Fallback to standard logging
            debug!("Executing tool: {} with input: {:?}", tool_name, tool_call.input);
        }

        // Get the executor
        let executors = self.executors.read().await;
        if let Some(executor) = executors.get(tool_name) {
            let exec_fn = executor.clone();
            drop(executors); // Release the lock before async execution

            // Execute the tool
            let result = exec_fn(tool_call.input.clone()).await;
            let execution_time = start_time.elapsed();

            // Log the result with enhanced logging if available
            if let Some(ref app_handle) = self.app_handle {
                let app_state_ref = app_handle.try_state::<crate::state::AppState>();
                let app_state_option = app_state_ref.as_ref().map(|s| s.inner());

                let success = result.is_ok();
                let output = result.as_ref().map(|v| v.clone()).unwrap_or_else(|e| {
                    serde_json::json!({ "error": e })
                });

                tool_logger::log_enhanced_tool_call_result(
                    app_handle,
                    tool_name,
                    output,
                    success,
                    None, // content
                    None, // screenshot_base64 - could be extracted from result if needed
                    Some(execution_time.as_millis() as u64),
                    app_state_option,
                ).await;
            }

            // Convert the result
            match result {
                Ok(output) => Ok(ToolResult {
                    call_id: tool_call.id,
                    output: serde_json::json!({ "result": output.to_string() }),
                }),
                Err(error) => Ok(ToolResult {
                    call_id: tool_call.id,
                    output: serde_json::json!({ "error": error }),
                }),
            }
        } else {
            warn!("Tool not found: {}", tool_name);
            Err(AgentError::ToolNotFound(tool_name.clone()))
        }
    }
}

impl Default for LocalToolProvider {
    fn default() -> Self {
        Self::new()
    }
}

/// Trait for tool providers that can be combined
#[async_trait]
pub trait CombinableToolProvider: Send + Sync {
    async fn get_tools(&self) -> Result<Vec<ToolDefinition>, AgentError>;
    async fn execute_tool(&self, tool_call: ToolCall) -> Result<ToolResult, AgentError>;
}

/// Combined tool provider that can aggregate multiple providers
pub struct CombinedToolProvider {
    providers: Vec<Box<dyn CombinableToolProvider>>,
}

impl CombinedToolProvider {
    pub fn new() -> Self {
        Self {
            providers: Vec::new(),
        }
    }

    pub fn add_provider(&mut self, provider: Box<dyn CombinableToolProvider>) {
        self.providers.push(provider);
    }

    /// Get error recovery statistics (placeholder for future implementation)
    pub async fn get_recovery_stats(&self) -> Value {
        serde_json::json!({
            "error_recovery": "not_implemented",
            "note": "Error recovery will be implemented in future iterations"
        })
    }
}

#[async_trait]
impl ToolProvider for CombinedToolProvider {
    async fn list_tools(&self) -> Result<Vec<ToolDefinition>, AgentError> {
        let mut all_tools = Vec::new();

        for provider in &self.providers {
            match provider.get_tools().await {
                Ok(mut tools) => all_tools.append(&mut tools),
                Err(e) => warn!("Failed to get tools from provider: {}", e),
            }
        }

        Ok(all_tools)
    }

    async fn execute_tool(&self, tool_call: ToolCall) -> Result<ToolResult, AgentError> {
        // Try each provider until one succeeds or we run out
        let mut last_error = AgentError::ToolNotFound(tool_call.name.clone());

        for provider in &self.providers {
            match provider.execute_tool(tool_call.clone()).await {
                Ok(result) => return Ok(result),
                Err(AgentError::ToolNotFound(_)) => continue, // Try next provider
                Err(e) => {
                    // Error recovery will be implemented in future iterations
                    last_error = e;
                    continue; // Try next provider
                }
            }
        }

        Err(last_error)
    }
}
