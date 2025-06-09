pub mod command_validator;
pub mod approval_manager;
pub mod execution_monitor;
pub mod file_monitor;
pub mod rate_limiter;

// Re-export key types
pub use command_validator::{CommandValidator, DangerousPattern, RiskLevel, ValidationResult};
pub use approval_manager::{ApprovalManager, PendingApproval, ApprovalDecision};
pub use execution_monitor::{ExecutionMonitor, CommandLogEntry};
pub use file_monitor::{FileMonitor, FileChangeEntry, FileChangeType};
pub use rate_limiter::{CommandRateLimiter, GlobalLimits};

use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime};

/// Security configuration for the agent system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    pub enabled: bool,
    pub development_mode_restrictions: bool,
    pub command_validation: CommandValidationConfig,
    pub rate_limiting: RateLimitingConfig,
    pub file_monitoring: FileMonitoringConfig,
    pub approval_system: ApprovalSystemConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandValidationConfig {
    pub enable_blacklist: bool,
    pub require_approval_for_sudo: bool,
    pub require_approval_for_destructive: bool,
    pub auto_deny_critical_commands: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitingConfig {
    pub max_commands_per_minute: u32,
    pub max_dangerous_commands_per_hour: u32,
    pub enable_abuse_detection: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileMonitoringConfig {
    pub enable_change_tracking: bool,
    pub protected_paths: Vec<String>,
    pub auto_backup_before_changes: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalSystemConfig {
    pub timeout_seconds: u64,
    pub require_explicit_approval: bool,
    pub log_all_decisions: bool,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            development_mode_restrictions: true,
            command_validation: CommandValidationConfig {
                enable_blacklist: true,
                require_approval_for_sudo: true,
                require_approval_for_destructive: true,
                auto_deny_critical_commands: true,
            },
            rate_limiting: RateLimitingConfig {
                max_commands_per_minute: 60,
                max_dangerous_commands_per_hour: 10,
                enable_abuse_detection: true,
            },
            file_monitoring: FileMonitoringConfig {
                enable_change_tracking: true,
                protected_paths: vec![
                    "/System".to_string(),
                    "/usr/bin".to_string(),
                    "/etc".to_string(),
                    "/boot".to_string(),
                    "C:\\Windows".to_string(),
                    "C:\\Program Files".to_string(),
                    "/Applications".to_string(),
                ],
                auto_backup_before_changes: true,
            },
            approval_system: ApprovalSystemConfig {
                timeout_seconds: 30,
                require_explicit_approval: true,
                log_all_decisions: true,
            },
        }
    }
}

/// Core security manager that coordinates all security subsystems
pub struct SecurityManager {
    config: SecurityConfig,
    validator: CommandValidator,
    approval_manager: ApprovalManager,
    execution_monitor: ExecutionMonitor,
    file_monitor: FileMonitor,
    rate_limiter: CommandRateLimiter,
}

impl SecurityManager {
    pub fn new(config: SecurityConfig) -> Result<Self, Box<dyn std::error::Error>> {
        let validator = CommandValidator::new(&config.command_validation)?;
        let approval_manager = ApprovalManager::new(
            Duration::from_secs(config.approval_system.timeout_seconds)
        );
        let execution_monitor = ExecutionMonitor::new();
        let file_monitor = FileMonitor::new(&config.file_monitoring.protected_paths)?;
        let rate_limiter = CommandRateLimiter::new(GlobalLimits {
            max_commands_per_minute: config.rate_limiting.max_commands_per_minute,
            max_dangerous_commands_per_hour: config.rate_limiting.max_dangerous_commands_per_hour,
            max_file_operations_per_minute: 30, // Additional safety limit
        });

        Ok(Self {
            config,
            validator,
            approval_manager,
            execution_monitor,
            file_monitor,
            rate_limiter,
        })
    }

    /// Validate and potentially approve a command before execution
    pub async fn validate_command(&self, 
        command: &str, 
        tool_name: &str,
        context: &str
    ) -> Result<bool, String> {
        if !self.config.enabled {
            return Ok(true);
        }

        // Check rate limits first
        if !self.rate_limiter.check_rate_limit(tool_name, command).await? {
            return Err("Rate limit exceeded for command execution".to_string());
        }

        // Validate command
        let validation_result = self.validator.validate_command(command)?;
        
        match validation_result.risk_level {
            RiskLevel::Critical if self.config.command_validation.auto_deny_critical_commands => {
                return Err(format!("Critical command blocked: {}", validation_result.reason));
            },
            RiskLevel::Critical | RiskLevel::High => {
                // Require approval
                let approval_id = self.approval_manager.request_approval(
                    command.to_string(),
                    validation_result.risk_level,
                    context.to_string()
                ).await?;
                
                self.approval_manager.wait_for_approval(approval_id).await
            },
            RiskLevel::Medium => {
                // Log and allow
                tracing::warn!("Medium risk command executed: {} - {}", command, validation_result.reason);
                Ok(true)
            },
            RiskLevel::Low => {
                // Allow with minimal logging
                Ok(true)
            }
        }
    }

    /// Start monitoring a command execution
    pub async fn start_monitoring(&self, command: &str, tool_name: &str) -> String {
        self.execution_monitor.start_monitoring(command, tool_name).await
    }

    /// Complete monitoring and log results
    pub async fn complete_monitoring(&self, 
        monitor_id: &str,
        exit_code: Option<i32>,
        stdout: &str,
        stderr: &str,
        execution_time: Duration
    ) -> Result<(), String> {
        self.execution_monitor.complete_monitoring(
            monitor_id, 
            exit_code, 
            stdout, 
            stderr, 
            execution_time
        ).await
    }

    /// Get recent command log entries
    pub async fn get_command_history(&self, limit: usize) -> Vec<CommandLogEntry> {
        self.execution_monitor.get_recent_entries(limit).await
    }

    /// Get current security status
    pub async fn get_security_status(&self) -> SecurityStatus {
        SecurityStatus {
            enabled: self.config.enabled,
            pending_approvals: self.approval_manager.get_pending_count().await,
            recent_violations: self.rate_limiter.get_recent_violations().await,
            active_monitors: self.execution_monitor.get_active_count().await,
            file_changes_today: self.file_monitor.get_changes_since(
                SystemTime::now() - Duration::from_secs(86400)
            ).await.len(),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct SecurityStatus {
    pub enabled: bool,
    pub pending_approvals: usize,
    pub recent_violations: usize,
    pub active_monitors: usize,
    pub file_changes_today: usize,
}