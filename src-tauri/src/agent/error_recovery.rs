use std::time::{Duration, Instant};
use serde::{Serialize, Deserialize};
use serde_json::{json, Value};
use tracing::{warn, info, debug, error};

use crate::agent::core::{AgentError, ToolCall, ToolResult};

/// Simplified core error types for recovery
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ErrorType {
    Network,      // Network, connection, service unavailable
    Permission,   // Access denied, permission issues
    Timeout,      // Timeouts, rate limits, slow operations
    NotFound,     // Element not found, file not found, app not running
    Other(String), // Everything else
}

/// Simple recovery configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryConfig {
    pub max_retries: usize,
    pub base_delay_ms: u64,
    pub max_delay_ms: u64,
}

impl Default for RecoveryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            base_delay_ms: 500,
            max_delay_ms: 5000,
        }
    }
}

/// Recovery attempt result
#[derive(Debug, Clone)]
pub struct RecoveryAttempt {
    pub error_type: ErrorType,
    pub retry_count: usize,
    pub delay_ms: u64,
    pub success: bool,
}

/// Simplified error recovery manager
pub struct ErrorRecoveryManager {
    config: RecoveryConfig,
    recovery_history: Vec<RecoveryAttempt>,
}

impl ErrorRecoveryManager {
    /// Create a new error recovery manager
    pub fn new() -> Self {
        Self {
            config: RecoveryConfig::default(),
            recovery_history: Vec::new(),
        }
    }

    /// Create with custom configuration
    pub fn with_config(config: RecoveryConfig) -> Self {
        Self {
            config,
            recovery_history: Vec::new(),
        }
    }

    /// Determine error type from an AgentError
    pub fn determine_error_type(&self, error: &AgentError) -> ErrorType {
        let error_message = error.to_string().to_lowercase();

        if error_message.contains("network") || 
           error_message.contains("connection") || 
           error_message.contains("service unavailable") ||
           error_message.contains("server error") {
            return ErrorType::Network;
        }

        if error_message.contains("permission denied") || 
           error_message.contains("access denied") {
            return ErrorType::Permission;
        }

        if error_message.contains("timeout") || 
           error_message.contains("timed out") ||
           error_message.contains("rate limit") ||
           error_message.contains("too many requests") {
            return ErrorType::Timeout;
        }

        if error_message.contains("element not found") || 
           error_message.contains("no such element") ||
           error_message.contains("file not found") ||
           error_message.contains("no such file") ||
           error_message.contains("application not running") ||
           error_message.contains("app not found") {
            return ErrorType::NotFound;
        }

        ErrorType::Other(error_message)
    }

    /// Calculate exponential backoff delay
    fn calculate_delay(&self, retry_count: usize, error_type: &ErrorType) -> u64 {
        let base_delay = match error_type {
            ErrorType::Network => self.config.base_delay_ms * 2,  // Longer for network issues
            ErrorType::Timeout => self.config.base_delay_ms * 3,  // Longer for timeouts
            _ => self.config.base_delay_ms,
        };

        let exponential_delay = base_delay * (2_u64.pow(retry_count as u32));
        std::cmp::min(exponential_delay, self.config.max_delay_ms)
    }

    /// Execute tool call with simple retry logic
    pub async fn execute_with_recovery<F, Fut>(
        &mut self,
        tool_call: ToolCall,
        executor: F,
    ) -> Result<ToolResult, AgentError>
    where
        F: Fn(ToolCall) -> Fut + Clone,
        Fut: std::future::Future<Output = Result<ToolResult, AgentError>>,
    {
        let mut retry_count = 0;
        let mut last_error = None;
        let start_time = Instant::now();

        loop {
            debug!("Executing tool call attempt {} of {}: {}",
                   retry_count + 1, self.config.max_retries + 1, tool_call.name);

            match executor(tool_call.clone()).await {
                Ok(result) => {
                    if retry_count > 0 {
                        info!("Tool call '{}' succeeded after {} retries",
                              tool_call.name, retry_count);
                    }
                    return Ok(result);
                }
                Err(error) => {
                    last_error = Some(error.clone());

                    if retry_count >= self.config.max_retries {
                        break;
                    }

                    let error_type = self.determine_error_type(&error);
                    
                    // Skip retry for certain error types
                    if matches!(error_type, ErrorType::Permission) {
                        warn!("Permission error detected, not retrying: {}", error);
                        break;
                    }

                    let delay_ms = self.calculate_delay(retry_count, &error_type);

                    info!("Tool call '{}' failed (attempt {}): {}. Retrying in {}ms",
                          tool_call.name, retry_count + 1, error, delay_ms);

                    // Record recovery attempt
                    let attempt = RecoveryAttempt {
                        error_type: error_type.clone(),
                        retry_count,
                        delay_ms,
                        success: false,
                    };
                    self.recovery_history.push(attempt);

                    // Wait before retry
                    tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                    retry_count += 1;
                }
            }
        }

        let total_time = start_time.elapsed();
        error!("Tool call '{}' failed after {} attempts in {:?}. Final error: {:?}",
               tool_call.name, retry_count, total_time, last_error);

        Err(last_error.unwrap_or_else(|| AgentError::Unknown("Unknown error during recovery".to_string())))
    }

    /// Get recovery statistics
    pub fn get_recovery_stats(&self) -> Value {
        let total_attempts = self.recovery_history.len();
        let successful_recoveries = self.recovery_history.iter()
            .filter(|attempt| attempt.success)
            .count();

        json!({
            "total_recovery_attempts": total_attempts,
            "successful_recoveries": successful_recoveries,
            "success_rate": if total_attempts > 0 { 
                successful_recoveries as f64 / total_attempts as f64 
            } else { 
                0.0 
            }
        })
    }

    /// Clear recovery history
    pub fn clear_history(&mut self) {
        self.recovery_history.clear();
    }
}
