/// TODO: ELIMINATE STRING MATCHING
use std::time::{Duration, Instant};
use std::collections::HashMap;
use serde::{Serialize, Deserialize};
use serde_json::{json, Value};
use tracing::{warn, info, debug, error};
// use crate::constants::error_recovery; // Not needed - using local definitions

use crate::agent::core::{AgentError, ToolCall, ToolResult};

/// Enhanced execution checkpoint that can be restored
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionCheckpoint {
    pub id: String,
    pub timestamp: std::time::SystemTime,
    pub step_number: u32,
    pub agent_state: AgentState,
    pub context: Value,
    pub successful_operations: Vec<ToolCall>,
    pub description: String,
}

/// Rollback information for recovery
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RollbackInfo {
    pub checkpoint_id: String,
    pub operations_to_undo: Vec<ToolCall>,
    pub rollback_reason: String,
    pub recovery_strategy: RecoveryStrategy,
}

/// Agent state that can be checkpointed and restored
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentState {
    pub current_tool: Option<String>,
    pub execution_context: Value,
    pub ui_state: Option<Value>, // Screenshot, element states, etc.
    pub browser_state: Option<Value>, // URL, cookies, session state
    pub file_system_state: Option<Value>, // Working directory, open files
    pub variables: HashMap<String, Value>, // Agent-defined variables
}

impl Default for AgentState {
    fn default() -> Self {
        Self {
            current_tool: None,
            execution_context: json!({}),
            ui_state: None,
            browser_state: None,
            file_system_state: None,
            variables: HashMap::new(),
        }
    }
}

/// Execution history entry for debugging and analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionHistoryEntry {
    pub timestamp: std::time::SystemTime,
    pub tool_call: ToolCall,
    pub result: Result<Value, String>,
    pub execution_time: Duration,
    pub recovery_attempt: Option<RecoveryAttempt>,
    pub checkpoint_created: Option<String>,
}

/// Common error patterns found in agent execution
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ErrorPattern {
    ElementNotFound,
    UnexpectedDialog,
    NoVisualEffect,
    OCRInaccuracy,
    NetworkError,
    FileSystemError,
    PermissionDenied,
    Timeout,
    LLMRateLimit,
    BrowserNotReady,
    ApplicationNotRunning,
    InvalidInput,
    ResourceBusy,
    ServiceUnavailable,
    MaxStepsReached,
    Cancelled,
    StateCorruption, // NEW: For rollback scenarios
    CascadingFailure, // NEW: Multiple related failures
    Unknown(String),
}

/// Recovery strategies for different error patterns
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RecoveryStrategy {
    Retry,
    AlternativeMethod,
    AdjustParameters,
    PromptLLM,
    EscalateToUser,
    WaitAndRetry(Duration),
    RefreshContext,
    FallbackTool,
    SkipStep,
    Abort,
    // NEW: Enhanced recovery strategies
    RollbackToCheckpoint(String), // Rollback to specific checkpoint
    RollbackAndRetry(String),     // Rollback then retry with modifications
    SaveStateAndRetry,            // Create checkpoint before retry
    RestoreLastKnownGood,         // Restore to last successful state
}

/// Enhanced recovery configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryConfig {
    pub max_retries: usize,
    pub base_retry_delay: Duration,
    pub max_retry_delay: Duration,
    pub enable_alternative_methods: bool,
    pub enable_llm_recovery: bool,
    pub enable_user_escalation: bool,
    pub timeout_threshold: Duration,
    // NEW: Checkpoint and rollback configuration
    pub enable_checkpoints: bool,
    pub max_checkpoints: usize,
    pub checkpoint_interval: u32, // Create checkpoint every N steps
    pub enable_automatic_rollback: bool,
    pub rollback_on_cascading_failures: bool,
}

impl Default for RecoveryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            base_retry_delay: Duration::from_millis(1000),
            max_retry_delay: Duration::from_millis(30000),
            enable_alternative_methods: true,
            enable_llm_recovery: true,
            enable_user_escalation: false,
            timeout_threshold: Duration::from_millis(60000),
            // Enhanced defaults
            enable_checkpoints: true,
            max_checkpoints: 10,
            checkpoint_interval: 3, // Checkpoint every 3 successful operations
            enable_automatic_rollback: true,
            rollback_on_cascading_failures: true,
        }
    }
}

/// Recovery attempt result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryAttempt {
    pub strategy: RecoveryStrategy,
    pub success: bool,
    pub error: Option<AgentError>,
    pub modified_tool_call: Option<ToolCall>,
    pub execution_time: Duration,
    pub checkpoint_used: Option<String>, // NEW: Track checkpoint usage
    pub rollback_performed: bool,        // NEW: Track rollback operations
}

/// Enhanced error recovery manager with checkpoint and rollback capabilities
pub struct ErrorRecoveryManager {
    config: RecoveryConfig,
    error_patterns: HashMap<String, ErrorPattern>,
    strategy_mappings: HashMap<ErrorPattern, Vec<RecoveryStrategy>>,
    recovery_history: Vec<RecoveryAttempt>,
    // NEW: Checkpoint and rollback state
    checkpoints: HashMap<String, ExecutionCheckpoint>,
    current_agent_state: AgentState,
    execution_history: Vec<ExecutionHistoryEntry>,
    step_counter: u32,
    cascading_failure_count: u32,
}

impl ErrorRecoveryManager {
    /// Create a new error recovery manager with enhanced capabilities
    pub fn new() -> Self {
        let mut manager = Self {
            config: RecoveryConfig::default(),
            error_patterns: HashMap::new(),
            strategy_mappings: HashMap::new(),
            recovery_history: Vec::new(),
            checkpoints: HashMap::new(),
            current_agent_state: AgentState::default(),
            execution_history: Vec::new(),
            step_counter: 0,
            cascading_failure_count: 0,
        };

        manager.initialize_enhanced_mappings();
        manager
    }

    /// Create a new error recovery manager with custom configuration
    pub fn with_config(config: RecoveryConfig) -> Self {
        let mut manager = Self {
            config,
            error_patterns: HashMap::new(),
            strategy_mappings: HashMap::new(),
            recovery_history: Vec::new(),
            checkpoints: HashMap::new(),
            current_agent_state: AgentState::default(),
            execution_history: Vec::new(),
            step_counter: 0,
            cascading_failure_count: 0,
        };

        manager.initialize_enhanced_mappings();
        manager
    }

    /// Initialize enhanced error pattern to recovery strategy mappings
    fn initialize_enhanced_mappings(&mut self) {
        // Element not found - try multiple strategies with checkpointing
        self.strategy_mappings.insert(
            ErrorPattern::ElementNotFound,
            vec![
                RecoveryStrategy::SaveStateAndRetry,
                RecoveryStrategy::WaitAndRetry(Duration::from_millis(2000)),
                RecoveryStrategy::RefreshContext,
                RecoveryStrategy::AlternativeMethod,
                RecoveryStrategy::AdjustParameters,
            ]
        );

        // Network errors - retry with backoff and checkpointing
        self.strategy_mappings.insert(
            ErrorPattern::NetworkError,
            vec![
                RecoveryStrategy::SaveStateAndRetry,
                RecoveryStrategy::WaitAndRetry(Duration::from_millis(5000)),
                RecoveryStrategy::Retry,
                RecoveryStrategy::FallbackTool,
            ]
        );

        // Permission denied - escalate or use alternative
        self.strategy_mappings.insert(
            ErrorPattern::PermissionDenied,
            vec![
                RecoveryStrategy::EscalateToUser,
                RecoveryStrategy::AlternativeMethod,
                RecoveryStrategy::SkipStep,
            ]
        );

        // Timeout errors - adjust parameters and retry with rollback option
        self.strategy_mappings.insert(
            ErrorPattern::Timeout,
            vec![
                RecoveryStrategy::AdjustParameters,
                RecoveryStrategy::WaitAndRetry(Duration::from_millis(3000)),
                RecoveryStrategy::RestoreLastKnownGood,
                RecoveryStrategy::AlternativeMethod,
            ]
        );

        // LLM rate limit - wait and retry
        self.strategy_mappings.insert(
            ErrorPattern::LLMRateLimit,
            vec![
                RecoveryStrategy::WaitAndRetry(Duration::from_millis(60000)),
                RecoveryStrategy::FallbackTool,
            ]
        );

        // Browser not ready - wait and refresh
        self.strategy_mappings.insert(
            ErrorPattern::BrowserNotReady,
            vec![
                RecoveryStrategy::WaitAndRetry(Duration::from_secs(3)),
                RecoveryStrategy::RefreshContext,
                RecoveryStrategy::AlternativeMethod,
            ]
        );

        // Application not running - start app and retry
        self.strategy_mappings.insert(
            ErrorPattern::ApplicationNotRunning,
            vec![
                RecoveryStrategy::AlternativeMethod, // This would start the app
                RecoveryStrategy::WaitAndRetry(Duration::from_millis(5000)),
                RecoveryStrategy::EscalateToUser,
            ]
        );

        // Service unavailable - wait and fallback
        self.strategy_mappings.insert(
            ErrorPattern::ServiceUnavailable,
            vec![
                RecoveryStrategy::WaitAndRetry(Duration::from_secs(10)),
                RecoveryStrategy::FallbackTool,
            ]
        );

        // Max steps reached - terminate gracefully
        self.strategy_mappings.insert(
            ErrorPattern::MaxStepsReached,
            vec![
                RecoveryStrategy::EscalateToUser,
            ]
        );

        // User cancelled - terminate immediately
        self.strategy_mappings.insert(
            ErrorPattern::Cancelled,
            vec![
                RecoveryStrategy::EscalateToUser,
            ]
        );

        // NEW: Enhanced error patterns with rollback strategies
        self.strategy_mappings.insert(
            ErrorPattern::StateCorruption,
            vec![
                RecoveryStrategy::RestoreLastKnownGood,
                RecoveryStrategy::RollbackToCheckpoint("last_stable".to_string()),
                RecoveryStrategy::EscalateToUser,
            ]
        );

        self.strategy_mappings.insert(
            ErrorPattern::CascadingFailure,
            vec![
                RecoveryStrategy::RestoreLastKnownGood,
                RecoveryStrategy::RollbackAndRetry("previous_stable".to_string()),
                RecoveryStrategy::Abort,
            ]
        );
    }

    /// Create a checkpoint at the current execution state
    pub fn create_checkpoint(&mut self, description: String) -> Result<String, AgentError> {
        if !self.config.enable_checkpoints {
            return Err(AgentError::Unknown("Checkpoints are disabled".to_string()));
        }

        let checkpoint_id = format!("checkpoint_{}", uuid::Uuid::new_v4());
        let checkpoint = ExecutionCheckpoint {
            id: checkpoint_id.clone(),
            timestamp: std::time::SystemTime::now(),
            step_number: self.step_counter,
            agent_state: self.current_agent_state.clone(),
            context: json!({
                "step_counter": self.step_counter,
                "recent_operations": self.execution_history.iter().rev().take(5).collect::<Vec<_>>()
            }),
            successful_operations: self.get_recent_successful_operations(),
            description,
        };

        let description = checkpoint.description.clone();
        self.checkpoints.insert(checkpoint_id.clone(), checkpoint);

        // Limit number of checkpoints
        if self.checkpoints.len() > self.config.max_checkpoints {
            let oldest_checkpoint = self.checkpoints.iter()
                .min_by_key(|(_, cp)| cp.timestamp)
                .map(|(id, _)| id.clone());

            if let Some(old_id) = oldest_checkpoint {
                self.checkpoints.remove(&old_id);
                debug!("Removed oldest checkpoint: {}", old_id);
            }
        }

        info!("Created checkpoint '{}': {}", checkpoint_id, description);
        Ok(checkpoint_id)
    }

    /// Rollback to a specific checkpoint
    pub async fn rollback_to_checkpoint(&mut self, checkpoint_id: &str) -> Result<RollbackInfo, AgentError> {
        let checkpoint = self.checkpoints.get(checkpoint_id).cloned()
            .ok_or_else(|| AgentError::Unknown(format!("Checkpoint '{}' not found", checkpoint_id)))?;

        // Determine operations that need to be undone
        let operations_to_undo = self.execution_history.iter()
            .filter(|entry| entry.timestamp > checkpoint.timestamp)
            .map(|entry| entry.tool_call.clone())
            .collect();

        // Restore agent state
        self.current_agent_state = checkpoint.agent_state.clone();
        self.step_counter = checkpoint.step_number;

        // Create rollback info
        let rollback_info = RollbackInfo {
            checkpoint_id: checkpoint_id.to_string(),
            operations_to_undo,
            rollback_reason: "Manual rollback requested".to_string(),
            recovery_strategy: RecoveryStrategy::RollbackToCheckpoint(checkpoint_id.to_string()),
        };

        // Truncate execution history to checkpoint point
        self.execution_history.retain(|entry| entry.timestamp <= checkpoint.timestamp);

        info!("Rolled back to checkpoint '{}' (step {})", checkpoint_id, checkpoint.step_number);
        Ok(rollback_info)
    }

    /// Rollback to the last known good state
    pub async fn rollback_to_last_known_good(&mut self) -> Result<RollbackInfo, AgentError> {
        // Find the most recent checkpoint
        let latest_checkpoint = self.checkpoints.values()
            .max_by_key(|cp| cp.timestamp)
            .cloned();

        match latest_checkpoint {
            Some(checkpoint) => {
                self.rollback_to_checkpoint(&checkpoint.id).await
            }
            None => {
                // If no checkpoints available, reset to initial state
                let rollback_info = RollbackInfo {
                    checkpoint_id: "initial_state".to_string(),
                    operations_to_undo: self.execution_history.iter().map(|e| e.tool_call.clone()).collect(),
                    rollback_reason: "No checkpoints available, resetting to initial state".to_string(),
                    recovery_strategy: RecoveryStrategy::RestoreLastKnownGood,
                };

                self.current_agent_state = AgentState::default();
                self.step_counter = 0;
                self.execution_history.clear();

                warn!("No checkpoints found, reset to initial state");
                Ok(rollback_info)
            }
        }
    }

    /// Record a successful tool execution
    pub fn record_successful_execution(&mut self, tool_call: ToolCall, result: Value, execution_time: Duration) {
        self.step_counter += 1;
        self.cascading_failure_count = 0; // Reset failure count on success

        let history_entry = ExecutionHistoryEntry {
            timestamp: std::time::SystemTime::now(),
            tool_call: tool_call.clone(),
            result: Ok(result),
            execution_time,
            recovery_attempt: None,
            checkpoint_created: None,
        };

        self.execution_history.push(history_entry);

        // Auto-create checkpoint if configured
        if self.config.enable_checkpoints &&
           self.step_counter % self.config.checkpoint_interval == 0 {
            if let Ok(checkpoint_id) = self.create_checkpoint(format!("Auto-checkpoint at step {}", self.step_counter)) {
                if let Some(last_entry) = self.execution_history.last_mut() {
                    last_entry.checkpoint_created = Some(checkpoint_id);
                }
            }
        }

        debug!("Recorded successful execution of '{}' (step {})", tool_call.name, self.step_counter);
    }

    /// Enhanced tool execution with checkpoint and rollback support
    pub async fn execute_tool_with_recovery<F, Fut>(
        &mut self,
        mut tool_call: ToolCall,
        executor: F,
    ) -> Result<ToolResult, AgentError>
    where
        F: Fn(ToolCall) -> Fut + Clone,
        Fut: std::future::Future<Output = Result<ToolResult, AgentError>>,
    {
        let start_time = Instant::now();
        let mut retry_count = 0;
        let mut last_error: Option<AgentError> = None;

        // Create a pre-execution checkpoint for complex operations
        let should_checkpoint = self.should_create_checkpoint(&tool_call);
        let pre_execution_checkpoint = if should_checkpoint {
            match self.create_checkpoint(format!("Pre-execution checkpoint for {}", tool_call.name)) {
                Ok(id) => Some(id),
                Err(e) => {
                    warn!("Failed to create pre-execution checkpoint: {}", e);
                    None
                }
            }
        } else {
            None
        };

        while retry_count <= self.config.max_retries {
            match executor(tool_call.clone()).await {
                Ok(result) => {
                    // Record successful execution
                    self.record_successful_execution(tool_call.clone(), serde_json::to_value(&result).unwrap_or_default(), start_time.elapsed());
                    return Ok(result);
                }
                Err(error) => {
                    last_error = Some(error.clone());
                    self.cascading_failure_count += 1;

                    // Record failed execution
                    let history_entry = ExecutionHistoryEntry {
                        timestamp: std::time::SystemTime::now(),
                        tool_call: tool_call.clone(),
                        result: Err(error.to_string()),
                        execution_time: start_time.elapsed(),
                        recovery_attempt: None,
                        checkpoint_created: None,
                    };
                    self.execution_history.push(history_entry);

                    // Determine error pattern and recovery strategy
                    let error_pattern = self.determine_error_pattern(&error);
                    let should_rollback = self.should_trigger_rollback(&error_pattern);

                    if should_rollback && self.config.enable_automatic_rollback {
                        info!("Triggering automatic rollback due to error pattern: {:?}", error_pattern);
                        if let Err(rollback_error) = self.rollback_to_last_known_good().await {
                            error!("Rollback failed: {}", rollback_error);
                        }
                    }

                    // Apply recovery strategies
                    let strategies = self.strategy_mappings.get(&error_pattern).cloned().unwrap_or_default();
                    if !strategies.is_empty() {
                        let mut recovery_successful = false;

                        for strategy in &strategies {
                            let recovery_start = Instant::now();

                            match self.apply_enhanced_recovery_strategy(
                                strategy.clone(),
                                &tool_call,
                                &error,
                                &pre_execution_checkpoint
                            ).await {
                                Ok(Some(modified_call)) => {
                                    tool_call = modified_call;
                                    recovery_successful = true;

                                    let attempt = RecoveryAttempt {
                                        strategy: strategy.clone(),
                                        success: true,
                                        error: None,
                                        modified_tool_call: Some(tool_call.clone()),
                                        execution_time: recovery_start.elapsed(),
                                        checkpoint_used: pre_execution_checkpoint.clone(),
                                        rollback_performed: should_rollback,
                                    };
                                    self.recovery_history.push(attempt);

                                    debug!("Recovery strategy {:?} succeeded for tool '{}'", strategy, tool_call.name);
                                    break;
                                }
                                Ok(None) => {
                                    recovery_successful = true;

                                    let attempt = RecoveryAttempt {
                                        strategy: strategy.clone(),
                                        success: true,
                                        error: None,
                                        modified_tool_call: None,
                                        execution_time: recovery_start.elapsed(),
                                        checkpoint_used: pre_execution_checkpoint.clone(),
                                        rollback_performed: should_rollback,
                                    };
                                    self.recovery_history.push(attempt);
                                    break;
                                }
                                Err(recovery_error) => {
                                    let attempt = RecoveryAttempt {
                                        strategy: strategy.clone(),
                                        success: false,
                                        error: Some(recovery_error.clone()),
                                        modified_tool_call: None,
                                        execution_time: recovery_start.elapsed(),
                                        checkpoint_used: pre_execution_checkpoint.clone(),
                                        rollback_performed: should_rollback,
                                    };
                                    self.recovery_history.push(attempt);

                                    warn!("Recovery strategy {:?} failed for tool '{}': {}", strategy, tool_call.name, recovery_error);
                                }
                            }
                        }

                        if !recovery_successful {
                            error!("All recovery strategies failed for tool '{}' on attempt {}", tool_call.name, retry_count + 1);
                            break;
                        }
                    }

                    retry_count += 1;
                }
            }
        }

        let total_time = start_time.elapsed();
        error!("Tool call '{}' failed after {} retries in {:?}: {}",
               tool_call.name, retry_count, total_time,
               last_error.as_ref().map(|e| e.to_string()).unwrap_or_default());

        Err(last_error.unwrap_or_else(|| AgentError::Unknown("Unknown error during recovery".to_string())))
    }

    /// Apply enhanced recovery strategy with checkpoint and rollback support
    async fn apply_enhanced_recovery_strategy(
        &mut self,
        strategy: RecoveryStrategy,
        tool_call: &ToolCall,
        error: &AgentError,
        pre_execution_checkpoint: &Option<String>,
    ) -> Result<Option<ToolCall>, AgentError> {
        match strategy {
            RecoveryStrategy::SaveStateAndRetry => {
                // Create checkpoint before retry
                let checkpoint_id = self.create_checkpoint(format!("Pre-retry checkpoint for {}", tool_call.name))?;
                info!("Created pre-retry checkpoint: {}", checkpoint_id);
                Ok(None)
            }

            RecoveryStrategy::RestoreLastKnownGood => {
                info!("Restoring to last known good state for tool '{}'", tool_call.name);
                let _rollback_info = self.rollback_to_last_known_good().await?;
                Ok(None)
            }

            RecoveryStrategy::RollbackToCheckpoint(checkpoint_id) => {
                info!("Rolling back to checkpoint '{}' for tool '{}'", checkpoint_id, tool_call.name);
                let _rollback_info = self.rollback_to_checkpoint(&checkpoint_id).await?;
                Ok(None)
            }

            RecoveryStrategy::RollbackAndRetry(checkpoint_id) => {
                info!("Rolling back to checkpoint '{}' and retrying tool '{}'", checkpoint_id, tool_call.name);
                let _rollback_info = self.rollback_to_checkpoint(&checkpoint_id).await?;

                // Modify the tool call slightly for retry
                let mut modified_call = tool_call.clone();
                if let Some(modified) = self.adjust_tool_parameters(&modified_call, error)? {
                    return Ok(Some(modified));
                }
                Ok(None)
            }

            // Delegate to existing recovery strategies
            _ => self.apply_recovery_strategy(strategy, tool_call, error).await
        }
    }

    /// Determine if a checkpoint should be created for this tool
    fn should_create_checkpoint(&self, tool_call: &ToolCall) -> bool {
        if !self.config.enable_checkpoints {
            return false;
        }

        // Create checkpoints for high-risk operations
        match tool_call.name.as_str() {
            "browser_navigate" | "left_click" | "right_click" | "type_text" | "key_combination" => true,
            "file_write" | "file_delete" | "directory_create" | "directory_delete" => true,
            "bash" => true, // Use official bash command for high-risk operations
            _ => false,
        }
    }

    /// Determine if rollback should be triggered for this error pattern
    fn should_trigger_rollback(&self, error_pattern: &ErrorPattern) -> bool {
        if !self.config.enable_automatic_rollback {
            return false;
        }

        match error_pattern {
            ErrorPattern::StateCorruption => true,
            ErrorPattern::CascadingFailure => true,
            _ => {
                // Trigger rollback on cascading failures
                self.config.rollback_on_cascading_failures && self.cascading_failure_count >= 3
            }
        }
    }

    /// Get recent successful operations for checkpoint context
    fn get_recent_successful_operations(&self) -> Vec<ToolCall> {
        self.execution_history.iter()
            .rev()
            .filter(|entry: &&ExecutionHistoryEntry| {
                match &entry.result {
                    Ok(output) => {
                        // Check if this is an Anthropic error response (computer tools)
                        !crate::agent::tools::anthropic_computer_use::is_anthropic_error_response(output)
                    }
                    Err(_) => false, // Traditional error format
                }
            })
            .take(5)
            .map(|entry| entry.tool_call.clone())
            .collect()
    }

    /// Update agent state during execution
    pub fn update_agent_state(&mut self, key: &str, value: Value) {
        self.current_agent_state.variables.insert(key.to_string(), value);
    }

    /// Get current execution statistics
    pub fn get_enhanced_recovery_stats(&self) -> Value {
        let total_attempts = self.recovery_history.len();
        let successful_attempts = self.recovery_history.iter().filter(|a| a.success).count();
        let rollback_attempts = self.recovery_history.iter().filter(|a| a.rollback_performed).count();

        let strategy_stats = self.recovery_history.iter()
            .fold(HashMap::new(), |mut acc, attempt| {
                let strategy_name = format!("{:?}", attempt.strategy);
                *acc.entry(strategy_name).or_insert(0) += 1;
                acc
            });

        json!({
            "total_recovery_attempts": total_attempts,
            "successful_attempts": successful_attempts,
            "rollback_attempts": rollback_attempts,
            "success_rate": if total_attempts > 0 {
                successful_attempts as f64 / total_attempts as f64
            } else {
                0.0
            },
            "strategy_usage": strategy_stats,
            "checkpoints": {
                "total_created": self.checkpoints.len(),
                "current_step": self.step_counter,
                "cascading_failure_count": self.cascading_failure_count
            },
            "execution_history": {
                "total_operations": self.execution_history.len(),
                "successful_operations": self.execution_history.iter().filter(|e: &&ExecutionHistoryEntry| {
                    match &e.result {
                        Ok(output) => !crate::agent::tools::anthropic_computer_use::is_anthropic_error_response(output),
                        Err(_) => false,
                    }
                }).count(),
                "failed_operations": self.execution_history.iter().filter(|e: &&ExecutionHistoryEntry| {
                    match &e.result {
                        Ok(output) => crate::agent::tools::anthropic_computer_use::is_anthropic_error_response(output),
                        Err(_) => true,
                    }
                }).count()
            },
            "config": self.config
        })
    }

    /// Clear all checkpoints and reset state
    pub fn reset_checkpoints(&mut self) {
        self.checkpoints.clear();
        self.current_agent_state = AgentState::default();
        self.step_counter = 0;
        self.cascading_failure_count = 0;
        info!("Reset all checkpoints and agent state");
    }

    /// Determine error pattern from an AgentError
    pub fn determine_error_pattern(&self, error: &AgentError) -> ErrorPattern {
        // First check for specific AgentError types
        match error {
            AgentError::PermissionDenied(_) => return ErrorPattern::PermissionDenied,
            AgentError::MaxStepsReached => return ErrorPattern::MaxStepsReached,
            AgentError::Terminated => return ErrorPattern::Cancelled,
            _ => {}
        }

        let error_message = error.to_string().to_lowercase();

        // Check for specific patterns in error messages
        if error_message.contains("element not found") || error_message.contains("no such element") {
            return ErrorPattern::ElementNotFound;
        }

        if error_message.contains("unexpected dialog") || error_message.contains("modal") {
            return ErrorPattern::UnexpectedDialog;
        }

        if error_message.contains("network") || error_message.contains("connection") {
            return ErrorPattern::NetworkError;
        }

        if error_message.contains("file not found") || error_message.contains("no such file") {
            return ErrorPattern::FileSystemError;
        }

        if error_message.contains("permission denied") ||
           error_message.contains("access denied") ||
           error_message.contains("accessibility permissions") ||
           error_message.contains("screen recording permission") ||
           error_message.contains("microphone permission") ||
           error_message.contains("desktop automation is not available") {
            return ErrorPattern::PermissionDenied;
        }

        if error_message.contains("timeout") || error_message.contains("timed out") {
            return ErrorPattern::Timeout;
        }

        if error_message.contains("rate limit") || error_message.contains("too many requests") {
            return ErrorPattern::LLMRateLimit;
        }

        if error_message.contains("browser") && (error_message.contains("not ready") || error_message.contains("not initialized")) {
            return ErrorPattern::BrowserNotReady;
        }

        if error_message.contains("application not running") || error_message.contains("app not found") {
            return ErrorPattern::ApplicationNotRunning;
        }

        if error_message.contains("service unavailable") || error_message.contains("server error") {
            return ErrorPattern::ServiceUnavailable;
        }

        if error_message.contains("corrupt") || error_message.contains("invalid state") {
            return ErrorPattern::StateCorruption;
        }

        // Check specific error types
        match error {
            AgentError::LlmError(_) => ErrorPattern::LLMRateLimit,
            AgentError::ToolError(_) => ErrorPattern::ElementNotFound,
            AgentError::InputError(_) => ErrorPattern::InvalidInput,
            AgentError::ConfigurationError(_) => ErrorPattern::InvalidInput,
            _ => ErrorPattern::Unknown(error_message),
        }
    }

    /// Choose recovery strategies for a given error pattern and retry count
    pub fn choose_recovery_strategies(&self, pattern: ErrorPattern, retry_count: usize) -> Vec<RecoveryStrategy> {
        let strategies = self.strategy_mappings.get(&pattern)
            .cloned()
            .unwrap_or_else(|| vec![RecoveryStrategy::Retry, RecoveryStrategy::Abort]);

        // Limit strategies based on retry count and configuration
        let max_strategies = std::cmp::min(strategies.len(), self.config.max_retries - retry_count);
        strategies.into_iter().take(max_strategies).collect()
    }

    /// Execute tool call with comprehensive error recovery
    pub async fn execute_with_recovery<F, Fut>(
        &mut self,
        tool_call: ToolCall,
        executor: F,
    ) -> Result<ToolResult, AgentError>
    where
        F: Fn(ToolCall) -> Fut + Clone,
        Fut: std::future::Future<Output = Result<ToolResult, AgentError>>,
    {
        let mut current_tool_call = tool_call.clone();
        let mut retry_count = 0;
        let mut last_error = None;
        let start_time = Instant::now();

        while retry_count <= self.config.max_retries {
            debug!("Executing tool call attempt {} of {}: {}",
                   retry_count + 1, self.config.max_retries + 1, current_tool_call.name);

            match executor(current_tool_call.clone()).await {
                Ok(result) => {
                    if retry_count > 0 {
                        info!("Tool call '{}' succeeded after {} retries",
                              current_tool_call.name, retry_count);
                    }
                    return Ok(result);
                }
                Err(error) => {
                    last_error = Some(error.clone());

                    if retry_count >= self.config.max_retries {
                        break;
                    }

                    // Determine error pattern and choose recovery strategy
                    let pattern = self.determine_error_pattern(&error);
                    let strategies = self.choose_recovery_strategies(pattern.clone(), retry_count);

                    info!("Tool call '{}' failed (attempt {}): {}. Trying {} recovery strategies",
                          current_tool_call.name, retry_count + 1, error, strategies.len());

                    // Try each recovery strategy
                    let mut recovery_successful = false;
                    for strategy in strategies {
                        let recovery_start = Instant::now();

                        match self.apply_recovery_strategy(
                            strategy.clone(),
                            &current_tool_call,
                            &error
                        ).await {
                            Ok(Some(modified_call)) => {
                                current_tool_call = modified_call;
                                recovery_successful = true;

                                let attempt = RecoveryAttempt {
                                    strategy: strategy.clone(),
                                    success: true,
                                    error: None,
                                    modified_tool_call: Some(current_tool_call.clone()),
                                    execution_time: recovery_start.elapsed(),
                                    checkpoint_used: None,
                                    rollback_performed: false,
                                };
                                self.recovery_history.push(attempt);

                                debug!("Recovery strategy {:?} succeeded for tool '{}'",
                                       strategy, current_tool_call.name);
                                break;
                            }
                            Ok(None) => {
                                // Strategy applied but no modification needed
                                recovery_successful = true;

                                let attempt = RecoveryAttempt {
                                    strategy: strategy.clone(),
                                    success: true,
                                    error: None,
                                    modified_tool_call: None,
                                    execution_time: recovery_start.elapsed(),
                                    checkpoint_used: None,
                                    rollback_performed: false,
                                };
                                self.recovery_history.push(attempt);
                                break;
                            }
                            Err(recovery_error) => {
                                let attempt = RecoveryAttempt {
                                    strategy: strategy.clone(),
                                    success: false,
                                    error: Some(recovery_error.clone()),
                                    modified_tool_call: None,
                                    execution_time: recovery_start.elapsed(),
                                    checkpoint_used: None,
                                    rollback_performed: false,
                                };
                                self.recovery_history.push(attempt);

                                warn!("Recovery strategy {:?} failed for tool '{}': {}",
                                      strategy, current_tool_call.name, recovery_error);
                            }
                        }
                    }

                    if !recovery_successful {
                        error!("All recovery strategies failed for tool '{}' on attempt {}",
                               current_tool_call.name, retry_count + 1);
                        break;
                    }

                    retry_count += 1;
                }
            }
        }

        let total_time = start_time.elapsed();
        error!("Tool call '{}' failed after {} attempts in {:?}. Final error: {:?}",
               tool_call.name, retry_count, total_time, last_error);

        Err(last_error.unwrap_or_else(|| AgentError::Unknown("Unknown error during recovery".to_string())))
    }

    /// Apply a specific recovery strategy
    async fn apply_recovery_strategy(
        &self,
        strategy: RecoveryStrategy,
        tool_call: &ToolCall,
        error: &AgentError,
    ) -> Result<Option<ToolCall>, AgentError> {
        match strategy {
            RecoveryStrategy::Retry => {
                // Simple retry - no modification needed
                Ok(None)
            }

            RecoveryStrategy::WaitAndRetry(duration) => {
                info!("Waiting {:?} before retry for tool '{}'", duration, tool_call.name);
                tokio::time::sleep(duration).await;
                Ok(None)
            }

            RecoveryStrategy::AdjustParameters => {
                self.adjust_tool_parameters(tool_call, error)
            }

            RecoveryStrategy::AlternativeMethod => {
                self.find_alternative_method(tool_call, error)
            }

            RecoveryStrategy::RefreshContext => {
                // For now, just wait a bit - in a full implementation this would
                // refresh browser contexts, accessibility trees, etc.
                tokio::time::sleep(Duration::from_millis(500)).await;
                Ok(None)
            }

            RecoveryStrategy::FallbackTool => {
                self.find_fallback_tool(tool_call, error)
            }

            RecoveryStrategy::SkipStep => {
                info!("Skipping tool call '{}' due to recovery strategy", tool_call.name);
                Err(AgentError::Unknown("Step skipped by recovery strategy".to_string()))
            }

            RecoveryStrategy::EscalateToUser => {
                if self.config.enable_user_escalation {
                    // In a full implementation, this would prompt the user
                    Err(AgentError::Unknown("User intervention required".to_string()))
                } else {
                    Err(AgentError::Unknown("Escalation not enabled".to_string()))
                }
            }

            RecoveryStrategy::PromptLLM => {
                if self.config.enable_llm_recovery {
                    // In a full implementation, this would ask the LLM for help
                    self.ask_llm_for_recovery(tool_call, error).await
                } else {
                    Err(AgentError::Unknown("LLM recovery not enabled".to_string()))
                }
            }

            RecoveryStrategy::Abort => {
                Err(AgentError::Unknown("Recovery strategy chose to abort".to_string()))
            }

            RecoveryStrategy::RollbackToCheckpoint(_checkpoint_id) => {
                // This should be handled by the enhanced recovery system
                Err(AgentError::Unknown("Rollback strategies require enhanced recovery manager".to_string()))
            }

            RecoveryStrategy::RollbackAndRetry(_checkpoint_id) => {
                // This should be handled by the enhanced recovery system
                Err(AgentError::Unknown("Rollback strategies require enhanced recovery manager".to_string()))
            }

            RecoveryStrategy::SaveStateAndRetry => {
                // This should be handled by the enhanced recovery system
                Err(AgentError::Unknown("Checkpoint strategies require enhanced recovery manager".to_string()))
            }

            RecoveryStrategy::RestoreLastKnownGood => {
                // This should be handled by the enhanced recovery system
                Err(AgentError::Unknown("Rollback strategies require enhanced recovery manager".to_string()))
            }
        }
    }

    /// Adjust tool parameters based on the error
    fn adjust_tool_parameters(
        &self,
        tool_call: &ToolCall,
        error: &AgentError,
    ) -> Result<Option<ToolCall>, AgentError> {
        let mut modified_call = tool_call.clone();
        let mut input = modified_call.input.clone();

        // Adjust parameters based on tool type and error
        match tool_call.name.as_str() {
            "left_click" | "right_click" | "double_click" => {
                // Add small random offset for click operations
                if let Some(x) = input.get("x").and_then(|v| v.as_f64()) {
                    if let Some(y) = input.get("y").and_then(|v| v.as_f64()) {
                        input["x"] = json!(x + 2.0);
                        input["y"] = json!(y + 2.0);
                        modified_call.input = input;
                        return Ok(Some(modified_call));
                    }
                }
            }

            "type_text" => {
                // Add slight delay for typing
                input["typing_delay"] = json!(50);
                modified_call.input = input;
                return Ok(Some(modified_call));
            }

            "screenshot" => {
                // Increase timeout for screenshots
                input["timeout"] = json!(10000);
                modified_call.input = input;
                return Ok(Some(modified_call));
            }

            _ => {}
        }

        Ok(None)
    }

    /// Find an alternative method for the tool call
    fn find_alternative_method(
        &self,
        tool_call: &ToolCall,
        _error: &AgentError,
    ) -> Result<Option<ToolCall>, AgentError> {
        // Define alternative methods for common tools
        let alternative_tool = match tool_call.name.as_str() {
            "left_click" => "double_click", // Sometimes elements need double-click
            "type_text" => "key_combination", // Could use key combinations
            "browser_navigate" => "browser_navigate_with_wait", // Add waiting
            _ => return Ok(None),
        };

        let mut modified_call = tool_call.clone();
        modified_call.name = alternative_tool.to_string();

        Ok(Some(modified_call))
    }

    /// Find a fallback tool for the current tool call
    fn find_fallback_tool(
        &self,
        tool_call: &ToolCall,
        _error: &AgentError,
    ) -> Result<Option<ToolCall>, AgentError> {
        // Define fallback tools
        let fallback_tool = match tool_call.name.as_str() {
            "browser_extract_content" => "screenshot", // Fallback to visual
            "get_focused_element_info" => "screenshot", // Fallback to visual
            "browser_navigate" => "type_text", // Could type URL directly
            _ => return Ok(None),
        };

        let mut modified_call = tool_call.clone();
        modified_call.name = fallback_tool.to_string();

        // Adjust input for fallback tool
        match fallback_tool {
            "screenshot" => {
                modified_call.input = json!({});
            }
            _ => {}
        }

        Ok(Some(modified_call))
    }

    /// Ask LLM for recovery assistance
    async fn ask_llm_for_recovery(
        &self,
        _tool_call: &ToolCall,
        _error: &AgentError,
    ) -> Result<Option<ToolCall>, AgentError> {
        // Placeholder for LLM-based recovery
        // In a full implementation, this would query the LLM for suggestions
        Err(AgentError::Unknown("LLM recovery not implemented".to_string()))
    }

    /// Get recovery statistics
    pub fn get_recovery_stats(&self) -> Value {
        let total_attempts = self.recovery_history.len();
        let successful_attempts = self.recovery_history.iter()
            .filter(|a| a.success)
            .count();

        let strategy_stats: HashMap<String, usize> = self.recovery_history.iter()
            .fold(HashMap::new(), |mut acc, attempt| {
                let strategy_name = format!("{:?}", attempt.strategy);
                *acc.entry(strategy_name).or_insert(0) += 1;
                acc
            });

        json!({
            "total_recovery_attempts": total_attempts,
            "successful_attempts": successful_attempts,
            "success_rate": if total_attempts > 0 {
                successful_attempts as f64 / total_attempts as f64
            } else {
                0.0
            },
            "strategy_usage": strategy_stats,
            "config": self.config
        })
    }

    /// Clear recovery history
    pub fn clear_history(&mut self) {
        self.recovery_history.clear();
    }

    /// Get strategy mappings for external access
    pub fn get_strategy_mappings(&self) -> &HashMap<ErrorPattern, Vec<RecoveryStrategy>> {
        &self.strategy_mappings
    }
}
