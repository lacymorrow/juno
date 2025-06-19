use async_trait::async_trait;
use futures::future::BoxFuture;
use serde_json::Value;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use tauri::{AppHandle, Manager, Emitter};
use tokio::sync::Mutex;
use tokio::sync::RwLock;
use tracing::{debug, error, warn};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::agent::structs::{AgentError, ToolCall, ToolDefinition, ToolResult};
use crate::agent::tool_logger;
use crate::agent::tools::mcp_integration::MCPManager;
use crate::agent::tools::ToolCategory;
use crate::agent::traits::ToolProvider;
use crate::state::AppState;
// Error recovery will be implemented in future iterations

// Define an async tool function type
// It takes a Value input and returns a BoxFuture that resolves to Result<Value, String>
// Needs Send + Sync bounds for async execution
// Add 'static lifetime bound
// Make the type alias public
pub type AsyncToolFn =
    Box<dyn Fn(Value) -> BoxFuture<'static, Result<Value, String>> + Send + Sync + 'static>;

/// Type alias for asynchronous tool executors
pub type AsyncToolExecutor =
    Arc<dyn Fn(Value) -> Pin<Box<dyn Future<Output = Result<Value, String>> + Send>> + Send + Sync>;

/// Error recovery statistics for tracking tool execution issues
#[derive(Debug, Clone, serde::Serialize)]
pub struct ErrorRecoveryStats {
    pub total_executions: u64,
    pub total_failures: u64,
    pub total_recoveries: u64,
    pub recovery_success_rate: f32,
    pub common_error_patterns: HashMap<String, u32>,
    pub tool_failure_rates: HashMap<String, f32>,
    pub last_recovery_attempt: Option<u64>,
    pub average_recovery_time_ms: f64,
}

impl Default for ErrorRecoveryStats {
    fn default() -> Self {
        Self {
            total_executions: 0,
            total_failures: 0,
            total_recoveries: 0,
            recovery_success_rate: 0.0,
            common_error_patterns: HashMap::new(),
            tool_failure_rates: HashMap::new(),
            last_recovery_attempt: None,
            average_recovery_time_ms: 0.0,
        }
    }
}

/// Error recovery configuration for tools
#[derive(Debug, Clone)]
pub struct ErrorRecoveryConfig {
    pub max_retries: u32,
    pub base_retry_delay_ms: u64,
    pub max_retry_delay_ms: u64,
    pub backoff_multiplier: f32,
    pub enable_pattern_recognition: bool,
    pub timeout_recovery_enabled: bool,
    pub display_error_recovery_enabled: bool,
}

impl Default for ErrorRecoveryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            base_retry_delay_ms: 100,
            max_retry_delay_ms: 5000,
            backoff_multiplier: 2.0,
            enable_pattern_recognition: true,
            timeout_recovery_enabled: true,
            display_error_recovery_enabled: true,
        }
    }
}

/// Recovery strategy based on error type
#[derive(Debug, Clone)]
pub enum RecoveryStrategy {
    Retry,
    RetryWithDelay(Duration),
    Fallback(String), // Tool name to fall back to
    Skip,
    ResetAndRetry,
}

/// Error classification for recovery decisions
#[derive(Debug, Clone, PartialEq)]
pub enum ErrorClass {
    Timeout,
    DisplaySystem,
    NetworkConnectivity,
    ResourceLocked,
    PermissionDenied,
    ToolNotFound,
    InvalidInput,
    Unknown,
}

/// A ToolProvider holding tools in memory, supporting async execution and MCP integration.
#[derive(Clone)]
pub struct LocalToolProvider {
    definitions: Arc<RwLock<HashMap<String, ToolDefinition>>>,
    // Use the AsyncToolFn type
    executors: Arc<RwLock<HashMap<String, AsyncToolExecutor>>>,
    app_handle: Option<AppHandle>,
    mcp_manager: Option<Arc<Mutex<MCPManager>>>,
    error_recovery_stats: Arc<Mutex<ErrorRecoveryStats>>,
    recovery_config: ErrorRecoveryConfig,
    tool_execution_history: Arc<Mutex<HashMap<String, Vec<(Instant, bool)>>>>,
}

impl LocalToolProvider {
    pub fn new() -> Self {
        LocalToolProvider {
            definitions: Arc::new(RwLock::new(HashMap::new())),
            executors: Arc::new(RwLock::new(HashMap::new())),
            app_handle: None,
            mcp_manager: None,
            error_recovery_stats: Arc::new(Mutex::new(ErrorRecoveryStats::default())),
            recovery_config: ErrorRecoveryConfig::default(),
            tool_execution_history: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Create a tool provider with an app handle for emitting events
    pub fn with_app_handle(app_handle: AppHandle) -> Self {
        LocalToolProvider {
            definitions: Arc::new(RwLock::new(HashMap::new())),
            executors: Arc::new(RwLock::new(HashMap::new())),
            app_handle: Some(app_handle),
            mcp_manager: None,
            error_recovery_stats: Arc::new(Mutex::new(ErrorRecoveryStats::default())),
            recovery_config: ErrorRecoveryConfig::default(),
            tool_execution_history: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Create a tool provider with both app handle and MCP manager for external tool support
    pub fn with_mcp_support(app_handle: AppHandle, mcp_manager: Arc<Mutex<MCPManager>>) -> Self {
        LocalToolProvider {
            definitions: Arc::new(RwLock::new(HashMap::new())),
            executors: Arc::new(RwLock::new(HashMap::new())),
            app_handle: Some(app_handle),
            mcp_manager: Some(mcp_manager),
            error_recovery_stats: Arc::new(Mutex::new(ErrorRecoveryStats::default())),
            recovery_config: ErrorRecoveryConfig::default(),
            tool_execution_history: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Set the app handle for emitting events
    pub fn set_app_handle(&mut self, app_handle: AppHandle) {
        self.app_handle = Some(app_handle);
    }

    /// Set the MCP manager for external tool support
    pub fn set_mcp_manager(&mut self, mcp_manager: Arc<Mutex<MCPManager>>) {
        self.mcp_manager = Some(mcp_manager);
    }

    /// Configure error recovery settings
    pub fn set_recovery_config(&mut self, config: ErrorRecoveryConfig) {
        self.recovery_config = config;
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

        // Wrap the executor with additional error handling for display-related operations
        let wrapped_executor: AsyncToolExecutor = Arc::new(move |input| {
            let fut = executor(input);
            Box::pin(async move {
                // Add specific handling for tools that might interact with display system
                match fut.await {
                    Ok(result) => Ok(result),
                    Err(e) => {
                        // Check if this is a display-related error
                        if e.contains("displayID")
                            || e.contains("RemoteLayerTree")
                            || e.contains("scheduleDisplayLink")
                        {
                            warn!("Display-related error detected in tool execution: {}", e);
                            // Return a more graceful error message
                            Err(format!(
                                "Display system error (this may be temporary): {}",
                                e
                            ))
                        } else {
                            Err(e)
                        }
                    }
                }
            })
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
        app_state: Option<&AppState>,
    ) where
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
        app_state: Option<&AppState>,
    ) where
        F: Fn(Value) -> Fut + Send + Sync + 'static,
        Fut: futures::Future<Output = Result<Value, String>> + Send + 'static,
    {
        let _config_manager = if let Some(state) = app_state {
            let config_arc = state.get_tool_config_manager().await;
            let config_guard = config_arc.lock().await;
            let is_enabled = config_guard.is_tool_enabled(&definition.name);
            drop(config_guard); // Release the lock early

            if !is_enabled {
                tracing::debug!(
                    "Tool '{}' is disabled, skipping registration",
                    definition.name
                );
                return;
            }
        };

        self.register_async_tool(definition, executor).await;
    }

    /// Refresh MCP tools from connected servers
    pub async fn refresh_mcp_tools(&mut self) -> Result<(), String> {
        if let Some(ref mcp_manager) = self.mcp_manager {
            // Add timeout to prevent hanging on display-related operations
            let timeout_duration = std::time::Duration::from_secs(10);

            let refresh_result = tokio::time::timeout(timeout_duration, async {
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
                        defs.insert(
                            tool_info.tool_definition.name.clone(),
                            tool_info.tool_definition,
                        );
                        added_count += 1;
                    }
                }

                log::info!(
                    "Refreshed and cached {} MCP tools in provider definitions",
                    added_count
                );
                Ok::<(), String>(())
            })
            .await;

            match refresh_result {
                Ok(Ok(())) => Ok(()),
                Ok(Err(e)) => Err(e),
                Err(_) => {
                    warn!("MCP tools refresh timed out after {:?}", timeout_duration);
                    Err("MCP tools refresh timeout".to_string())
                }
            }
        } else {
            Ok(())
        }
    }

    /// Check if a tool is an MCP tool using proper tool configuration
    async fn is_mcp_tool(&self, tool_name: &str) -> bool {
        // Check if we have app handle to access tool configuration
        if let Some(ref app_handle) = self.app_handle {
            let state = app_handle.state::<AppState>();
            let config_manager = state.get_tool_config_manager().await;
            let config_guard = config_manager.lock().await;

            if let Some(tool_config) = config_guard.get_tool_config(tool_name) {
                return tool_config.category == ToolCategory::MCP;
            }
        }

        // Fallback to basic MCP manager availability check if no configuration access
        self.mcp_manager.is_some()
            && !self
                .executors
                .try_read()
                .map(|execs| execs.contains_key(tool_name))
                .unwrap_or(false)
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

    /// Get comprehensive error recovery statistics
    pub async fn get_recovery_stats(&self) -> Value {
        let stats = self.error_recovery_stats.lock().await;
        serde_json::to_value(&*stats).unwrap_or_else(|_| {
            serde_json::json!({
                "error": "Failed to serialize recovery stats"
            })
        })
    }

    /// Clear error recovery history
    pub async fn clear_recovery_history(&self) {
        let mut stats = self.error_recovery_stats.lock().await;
        *stats = ErrorRecoveryStats::default();

        let mut history = self.tool_execution_history.lock().await;
        history.clear();

        tracing::info!("Error recovery history cleared");
    }

    /// Get tool-specific failure rates
    pub async fn get_tool_failure_rates(&self) -> HashMap<String, f32> {
        let stats = self.error_recovery_stats.lock().await;
        stats.tool_failure_rates.clone()
    }

    /// Get most common error patterns
    pub async fn get_error_patterns(&self) -> Vec<(String, u32)> {
        let stats = self.error_recovery_stats.lock().await;
        let mut patterns: Vec<_> = stats.common_error_patterns.iter()
            .map(|(pattern, count)| (pattern.clone(), *count))
            .collect();
        patterns.sort_by(|a, b| b.1.cmp(&a.1));
        patterns.truncate(10); // Return top 10 patterns
        patterns
    }

	/// TODO: ELIMINATE STRING MATCHING
    /// Classify error for recovery strategy selection
    fn classify_error(&self, error_msg: &str) -> ErrorClass {
        let error_lower = error_msg.to_lowercase();

        if error_lower.contains("timeout") || error_lower.contains("timed out") {
            ErrorClass::Timeout
        } else if error_lower.contains("displayid")
                || error_lower.contains("remotelayertree")
                || error_lower.contains("display")
                || error_lower.contains("scheduleDisplayLink") {
            ErrorClass::DisplaySystem
        } else if error_lower.contains("network")
                || error_lower.contains("connection")
                || error_lower.contains("unreachable") {
            ErrorClass::NetworkConnectivity
        } else if error_lower.contains("locked")
                || error_lower.contains("busy")
                || error_lower.contains("in use") {
            ErrorClass::ResourceLocked
        } else if error_lower.contains("permission")
                || error_lower.contains("denied")
                || error_lower.contains("unauthorized") {
            ErrorClass::PermissionDenied
        } else if error_lower.contains("not found")
                || error_lower.contains("unknown tool") {
            ErrorClass::ToolNotFound
        } else if error_lower.contains("invalid")
                || error_lower.contains("malformed")
                || error_lower.contains("parse") {
            ErrorClass::InvalidInput
        } else {
            ErrorClass::Unknown
        }
    }

    /// Determine recovery strategy based on error class and history
    async fn determine_recovery_strategy(&self,
        error_class: ErrorClass,
        _tool_name: &str,
        retry_count: u32
    ) -> RecoveryStrategy {
        if retry_count >= self.recovery_config.max_retries {
            return RecoveryStrategy::Skip;
        }

        match error_class {
            ErrorClass::Timeout => {
                if self.recovery_config.timeout_recovery_enabled {
                    let delay = std::cmp::min(
                        self.recovery_config.base_retry_delay_ms *
                        (self.recovery_config.backoff_multiplier.powi(retry_count as i32) as u64),
                        self.recovery_config.max_retry_delay_ms
                    );
                    RecoveryStrategy::RetryWithDelay(Duration::from_millis(delay))
                } else {
                    RecoveryStrategy::Skip
                }
            },
            ErrorClass::DisplaySystem => {
                if self.recovery_config.display_error_recovery_enabled {
                    // For display errors, try to reset and retry after a short delay
                    RecoveryStrategy::RetryWithDelay(Duration::from_millis(200))
                } else {
                    RecoveryStrategy::Skip
                }
            },
            ErrorClass::ResourceLocked => {
                // Wait longer for locked resources
                RecoveryStrategy::RetryWithDelay(Duration::from_millis(500))
            },
            ErrorClass::NetworkConnectivity => {
                // Progressive backoff for network issues
                let delay = self.recovery_config.base_retry_delay_ms * (retry_count + 1) as u64;
                RecoveryStrategy::RetryWithDelay(Duration::from_millis(delay))
            },
            ErrorClass::PermissionDenied | ErrorClass::ToolNotFound => {
                // These errors typically don't benefit from retries
                RecoveryStrategy::Skip
            },
            ErrorClass::InvalidInput => {
                // Input validation errors shouldn't be retried
                RecoveryStrategy::Skip
            },
            ErrorClass::Unknown => {
                // Conservative retry with backoff for unknown errors
                let delay = self.recovery_config.base_retry_delay_ms * 2_u64.pow(retry_count);
                RecoveryStrategy::RetryWithDelay(Duration::from_millis(
                    std::cmp::min(delay, self.recovery_config.max_retry_delay_ms)
                ))
            }
        }
    }

    /// Update error recovery statistics
    async fn update_recovery_stats(&self,
        tool_name: &str,
        error_msg: &str,
        recovery_attempted: bool,
        recovery_successful: bool,
        recovery_time: Option<Duration>
    ) {
        let mut stats = self.error_recovery_stats.lock().await;

        stats.total_executions += 1;
        stats.total_failures += 1;

        if recovery_attempted {
            stats.total_recoveries += 1;
            if recovery_successful {
                if let Some(time) = recovery_time {
                    let time_ms = time.as_millis() as f64;
                    if stats.total_recoveries == 1 {
                        stats.average_recovery_time_ms = time_ms;
                    } else {
                        stats.average_recovery_time_ms =
                            (stats.average_recovery_time_ms * (stats.total_recoveries - 1) as f64 + time_ms)
                            / stats.total_recoveries as f64;
                    }
                }
            }
            stats.last_recovery_attempt = Some(
                SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()
            );
        }

        // Update recovery success rate
        if stats.total_recoveries > 0 {
            stats.recovery_success_rate =
                (stats.total_recoveries as f32 - stats.total_failures as f32 + 1.0)
                / stats.total_recoveries as f32;
        }

        // Track error patterns
        if self.recovery_config.enable_pattern_recognition {
            let error_pattern = self.extract_error_pattern(error_msg);
            *stats.common_error_patterns.entry(error_pattern).or_insert(0) += 1;
        }

        // Update tool failure rates
        let mut history = self.tool_execution_history.lock().await;
        let tool_history = history.entry(tool_name.to_string()).or_insert_with(Vec::new);
        tool_history.push((Instant::now(), !recovery_successful));

        // Keep only recent history (last 100 executions)
        if tool_history.len() > 100 {
            tool_history.remove(0);
        }

        // Calculate failure rate for this tool
        let failures = tool_history.iter().filter(|(_, failed)| *failed).count();
        let failure_rate = failures as f32 / tool_history.len() as f32;
        stats.tool_failure_rates.insert(tool_name.to_string(), failure_rate);
    }

    /// Extract error pattern for tracking
    fn extract_error_pattern(&self, error_msg: &str) -> String {
        // Simplify error message to pattern by removing specific details
        let pattern = error_msg
            .chars()
            .filter(|c| c.is_alphabetic() || c.is_whitespace())
            .collect::<String>()
            .split_whitespace()
            .take(5) // Take first 5 words
            .collect::<Vec<_>>()
            .join(" ");

        if pattern.len() > 50 {
            pattern[..50].to_string()
        } else {
            pattern
        }
    }

    /// Execute tool with comprehensive error recovery
    async fn execute_tool_with_recovery(&self, tool_call: ToolCall) -> Result<ToolResult, AgentError> {
        let tool_name = tool_call.name.clone();
        let mut retry_count = 0;
        let recovery_start = Instant::now();

        loop {
            // Try to execute the tool
            let result = self.execute_tool_direct(tool_call.clone()).await;

            match result {
                Ok(tool_result) => {
                    // Success - update stats if this was a recovery
                    if retry_count > 0 {
                        self.update_recovery_stats(
                            &tool_name,
                            "recovered",
                            true,
                            true,
                            Some(recovery_start.elapsed())
                        ).await;
                    }
                    return Ok(tool_result);
                },
                Err(error) => {
                    // Classify the error
                    let error_msg = error.to_string();
                    let error_class = self.classify_error(&error_msg);

                    // Determine recovery strategy
                    let strategy = self.determine_recovery_strategy(error_class, &tool_name, retry_count).await;

                    match strategy {
                        RecoveryStrategy::Skip => {
                            // No recovery possible, record failure and return error
                            self.update_recovery_stats(
                                &tool_name,
                                &error_msg,
                                retry_count > 0,
                                false,
                                None
                            ).await;
                            return Err(error);
                        },
                        RecoveryStrategy::RetryWithDelay(delay) => {
                            retry_count += 1;
                            warn!(
                                "Tool '{}' failed (attempt {}), retrying after {:?}: {}",
                                tool_name, retry_count, delay, error_msg
                            );
                            tokio::time::sleep(delay).await;
                            continue;
                        },
                        RecoveryStrategy::Retry => {
                            retry_count += 1;
                            warn!(
                                "Tool '{}' failed (attempt {}), retrying immediately: {}",
                                tool_name, retry_count, error_msg
                            );
                            continue;
                        },
                        RecoveryStrategy::ResetAndRetry => {
                            retry_count += 1;
                            warn!(
                                "Tool '{}' failed (attempt {}), resetting and retrying: {}",
                                tool_name, retry_count, error_msg
                            );
                            // Add a small delay for reset
                            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                            continue;
                        },
                        RecoveryStrategy::Fallback(_fallback_tool) => {
                            // TODO: Implement fallback tool execution
                            warn!("Fallback strategy not yet implemented for tool '{}'", tool_name);
                            self.update_recovery_stats(
                                &tool_name,
                                &error_msg,
                                true,
                                false,
                                None
                            ).await;
                            return Err(error);
                        }
                    }
                }
            }
        }
    }

    /// Direct tool execution without recovery
    async fn execute_tool_direct(&self, tool_call: ToolCall) -> Result<ToolResult, AgentError> {
        let tool_name = &tool_call.name;

        // Add timeout for all tool executions
        let timeout_duration = std::time::Duration::from_secs(30);

        let execution_result = tokio::time::timeout(timeout_duration, async {
            if self.is_mcp_tool(tool_name).await {
                // Execute via MCP manager
                if let Some(ref mcp_manager) = self.mcp_manager {
                    let manager_guard = mcp_manager.lock().await;
                    manager_guard
                        .execute_tool(
                            &tool_call.name,
                            tool_call.input.clone(),
                            tool_call.id.clone(),
                        )
                        .await
                } else {
                    Err(AgentError::ToolNotFound(format!(
                        "MCP tool '{}' requested but no MCP manager available",
                        tool_name
                    )))
                }
            } else {
                // Execute local tool
                let executors_guard = self.executors.read().await;
                if let Some(executor) = executors_guard.get(tool_name) {
                    match executor(tool_call.input.clone()).await {
                        Ok(output) => Ok(ToolResult {
                            call_id: tool_call.id.clone(),
                            output,
                        }),
                        Err(error_msg) => Err(AgentError::ToolError(error_msg)),
                    }
                } else {
                    Err(AgentError::ToolNotFound(tool_call.name.clone()))
                }
            }
        })
        .await;

        match execution_result {
            Ok(result) => result,
            Err(_) => {
                // Timeout occurred - create a proper timeout error with the correct tool call ID
                let timeout_error = format!(
                    "Tool '{}' execution timed out after {:?}",
                    tool_name, timeout_duration
                );

                // Return a proper ToolResult with timeout error instead of AgentError
                // This ensures the conversation remains consistent
                Ok(ToolResult {
                    call_id: tool_call.id.clone(), // Use the original tool call ID
                    output: serde_json::json!({
                        "error": timeout_error,
                        "timeout": true,
                        "duration_seconds": timeout_duration.as_secs()
                    }),
                })
            }
        }
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
            let has_mcp_tools = all_tools
                .iter()
                .any(|tool| tool.name.contains("mcp-server-"));

            if !has_mcp_tools {
                // Add timeout to MCP tool fetching to prevent hanging on display-related operations
                let timeout_duration = std::time::Duration::from_secs(5);

                match tokio::time::timeout(timeout_duration, async {
                    let manager_guard = mcp_manager.lock().await;
                    let mcp_tools = manager_guard.get_all_tools().await;
                    drop(manager_guard);
                    mcp_tools
                })
                .await
                {
                    Ok(mcp_tools) => {
                        for tool_info in mcp_tools {
                            if tool_info.enabled {
                                all_tools.push(tool_info.tool_definition);
                            }
                        }
                        debug!(
                            "Fetched {} fresh MCP tools",
                            all_tools
                                .iter()
                                .filter(|t| t.name.contains("mcp-server-"))
                                .count()
                        );
                    }
                    Err(_) => {
                        warn!(
                            "MCP tools fetch timed out after {:?}, continuing without MCP tools",
                            timeout_duration
                        );
                    }
                }
            } else {
                debug!("Using cached MCP tools, skipping fresh fetch");
            }
        }

        // Debug logging to identify duplicates and deduplicate if needed
        let mut tool_names = std::collections::HashSet::new();
        let mut duplicates = Vec::new();
        let mut unique_tools = Vec::new();

        for tool in all_tools {
            if tool_names.insert(tool.name.clone()) {
                unique_tools.push(tool);
            } else {
                duplicates.push(tool.name.clone());
            }
        }

        if !duplicates.is_empty() {
            warn!(
                "Removed {} duplicate tools: {:?}",
                duplicates.len(),
                duplicates
            );
            debug!("Keeping {} unique tools", unique_tools.len());
        } else {
            debug!("All {} tools are unique", unique_tools.len());
        }

        all_tools = unique_tools;

        Ok(all_tools)
    }

    async fn execute_tool(&self, tool_call: ToolCall) -> Result<ToolResult, AgentError> {
        let tool_name = tool_call.name.clone();

        // Generate unique command ID for tracking
        let command_id = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        // Record start time for duration calculation
        let start_time = std::time::Instant::now();

        // Emit command execution start event if app handle is available
        if let Some(ref app_handle) = self.app_handle {
            if let Err(e) = app_handle.emit("command-execution-start", serde_json::json!({
                "command": tool_name,
                "id": command_id
            })) {
                error!("Failed to emit command-execution-start event: {}", e);
            }

            tool_logger::log_tool_call_request(
                app_handle,
                &tool_name,
                tool_call.input.clone(),
                Some(format!("Executing tool: {}", tool_name)),
            );
        }

        // Execute tool with error recovery
        let result = self.execute_tool_with_recovery(tool_call).await;

        // Calculate execution duration
        let duration_ms = start_time.elapsed().as_millis() as u64;

        // Emit command execution end event if app handle is available
        if let Some(ref app_handle) = self.app_handle {
            let (success, error_msg) = match &result {
                Ok(_) => (true, None),
                Err(e) => (false, Some(e.to_string())),
            };

            if let Err(e) = app_handle.emit("command-execution-end", serde_json::json!({
                "id": command_id,
                "success": success,
                "duration": duration_ms,
                "error": error_msg
            })) {
                error!("Failed to emit command-execution-end event: {}", e);
            }

            match &result {
                Ok(tool_result) => {
                    tool_logger::log_tool_call_result(
                        app_handle,
                        &tool_name,
                        tool_result.output.clone(),
                        true, // success = true
                        Some(format!("Tool {} completed successfully", tool_name)),
                        None,
                    );
                }
                Err(error) => {
                    tool_logger::log_tool_call_result(
                        app_handle,
                        &tool_name,
                        serde_json::json!({"error": error.to_string()}),
                        false, // success = false
                        Some(format!("Tool {} failed: {}", tool_name, error)),
                        None,
                    );
                }
            }
        }

        result
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
