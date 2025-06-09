use std::time::{Duration, Instant};
use std::collections::HashMap;
use serde::{Serialize, Deserialize};
use serde_json::{json, Value};
use tracing::{warn, info, debug, error};

use crate::agent::core::{AgentError, ToolCall, ToolResult};

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
}

/// Configuration for error recovery behavior
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryConfig {
    pub max_retries: usize,
    pub base_retry_delay: Duration,
    pub max_retry_delay: Duration,
    pub enable_alternative_methods: bool,
    pub enable_llm_recovery: bool,
    pub enable_user_escalation: bool,
    pub timeout_threshold: Duration,
}

impl Default for RecoveryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            base_retry_delay: Duration::from_millis(500),
            max_retry_delay: Duration::from_secs(10),
            enable_alternative_methods: true,
            enable_llm_recovery: true,
            enable_user_escalation: false, // Default to false for autonomous operation
            timeout_threshold: Duration::from_secs(30),
        }
    }
}

/// Recovery attempt result
#[derive(Debug, Clone)]
pub struct RecoveryAttempt {
    pub strategy: RecoveryStrategy,
    pub success: bool,
    pub error: Option<AgentError>,
    pub modified_tool_call: Option<ToolCall>,
    pub execution_time: Duration,
}

/// Error recovery manager that handles systematic error recovery
pub struct ErrorRecoveryManager {
    config: RecoveryConfig,
    error_patterns: HashMap<String, ErrorPattern>,
    strategy_mappings: HashMap<ErrorPattern, Vec<RecoveryStrategy>>,
    recovery_history: Vec<RecoveryAttempt>,
}

impl ErrorRecoveryManager {
    /// Create a new error recovery manager with default configuration
    pub fn new() -> Self {
        let mut manager = Self {
            config: RecoveryConfig::default(),
            error_patterns: HashMap::new(),
            strategy_mappings: HashMap::new(),
            recovery_history: Vec::new(),
        };

        manager.initialize_default_mappings();
        manager
    }

    /// Create a new error recovery manager with custom configuration
    pub fn with_config(config: RecoveryConfig) -> Self {
        let mut manager = Self {
            config,
            error_patterns: HashMap::new(),
            strategy_mappings: HashMap::new(),
            recovery_history: Vec::new(),
        };

        manager.initialize_default_mappings();
        manager
    }

    /// Initialize default error pattern to recovery strategy mappings
    fn initialize_default_mappings(&mut self) {
        // Element not found - try multiple strategies
        self.strategy_mappings.insert(
            ErrorPattern::ElementNotFound,
            vec![
                RecoveryStrategy::WaitAndRetry(Duration::from_millis(1000)),
                RecoveryStrategy::RefreshContext,
                RecoveryStrategy::AlternativeMethod,
                RecoveryStrategy::AdjustParameters,
            ]
        );

        // Network errors - retry with backoff
        self.strategy_mappings.insert(
            ErrorPattern::NetworkError,
            vec![
                RecoveryStrategy::WaitAndRetry(Duration::from_secs(2)),
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

        // Timeout errors - adjust parameters and retry
        self.strategy_mappings.insert(
            ErrorPattern::Timeout,
            vec![
                RecoveryStrategy::AdjustParameters,
                RecoveryStrategy::WaitAndRetry(Duration::from_secs(5)),
                RecoveryStrategy::AlternativeMethod,
            ]
        );

        // LLM rate limit - wait and retry
        self.strategy_mappings.insert(
            ErrorPattern::LLMRateLimit,
            vec![
                RecoveryStrategy::WaitAndRetry(Duration::from_secs(60)),
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
                RecoveryStrategy::WaitAndRetry(Duration::from_secs(2)),
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
}
