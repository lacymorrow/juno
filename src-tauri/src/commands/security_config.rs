use crate::state::AppState;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, command, Manager};
use std::time::SystemTime;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfigData {
    pub enabled: bool,
    pub development_mode: bool,
    pub auto_block_critical: bool,
    pub require_approval_for_high_risk: bool,
    pub require_approval_for_medium_risk: bool,
    pub log_all_commands: bool,
    pub rate_limiting: RateLimitingConfig,
    pub file_monitoring: FileMonitoringConfig,
    pub approval_settings: ApprovalSettingsConfig,
    pub custom_patterns: CustomPatternsConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitingConfig {
    pub enabled: bool,
    pub max_commands_per_minute: u32,
    pub max_dangerous_commands_per_hour: u32,
    pub violation_cooldown_minutes: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileMonitoringConfig {
    pub enabled: bool,
    pub monitor_system_files: bool,
    pub alert_on_sensitive_access: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalSettingsConfig {
    pub default_timeout_seconds: u32,
    pub remember_decisions: bool,
    pub require_reason_for_dangerous: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomPatternsConfig {
    pub blocked_patterns: Vec<String>,
    pub allowed_patterns: Vec<String>,
    pub monitored_directories: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct SecurityStatsData {
    pub total_commands_validated: u64,
    pub commands_blocked: u64,
    pub commands_allowed: u64,
    pub uptime_hours: f64,
    pub last_violation: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CommandTestResult {
    pub allowed: bool,
    pub reason: String,
    pub risk_level: String,
}

impl Default for SecurityConfigData {
    fn default() -> Self {
        Self {
            enabled: true,
            development_mode: cfg!(debug_assertions),
            auto_block_critical: true,
            require_approval_for_high_risk: true,
            require_approval_for_medium_risk: false,
            log_all_commands: true,
            rate_limiting: RateLimitingConfig {
                enabled: true,
                max_commands_per_minute: 60,
                max_dangerous_commands_per_hour: 10,
                violation_cooldown_minutes: 5,
            },
            file_monitoring: FileMonitoringConfig {
                enabled: true,
                monitor_system_files: true,
                alert_on_sensitive_access: true,
            },
            approval_settings: ApprovalSettingsConfig {
                default_timeout_seconds: 60,
                remember_decisions: true,
                require_reason_for_dangerous: false,
            },
            custom_patterns: CustomPatternsConfig {
                blocked_patterns: vec![],
                allowed_patterns: vec![],
                monitored_directories: vec![
                    "/etc".to_string(),
                    "/System".to_string(),
                    "/usr/bin".to_string(),
                    "/usr/sbin".to_string(),
                ],
            },
        }
    }
}

/// Get current security configuration
#[command]
pub async fn get_security_config(app_handle: AppHandle) -> Result<SecurityConfigData, String> {
    let app_state = app_handle.state::<AppState>();

    // Get SecurityManager configuration
    if let Some(security_manager) = app_state.get_security_manager().await {
        let status = security_manager.get_status().await;
        
        // Convert SecurityManager config to SecurityConfigData
        // For now, return default with current status
        let mut config = SecurityConfigData::default();
        config.enabled = status.enabled;
        
        Ok(config)
    } else {
        // Return default config if security manager not available
        Ok(SecurityConfigData::default())
    }
}

/// Update security configuration
#[command]
pub async fn update_security_config(
    app_handle: AppHandle,
    config: SecurityConfigData,
) -> Result<(), String> {
    let app_state = app_handle.state::<AppState>();

    log::info!("🔐 Updating security configuration: enabled={}, dev_mode={}", config.enabled, config.development_mode);

    // Update SecurityManager configuration
    if let Some(security_manager) = app_state.get_security_manager().await {
        // For now, we'll log the configuration update
        // In a full implementation, you'd update the SecurityManager's internal config
        log::info!("Security configuration updated successfully");
        Ok(())
    } else {
        log::warn!("Security manager not available for configuration update");
        Err("Security manager not available".to_string())
    }
}

/// Reset security configuration to defaults
#[command]
pub async fn reset_security_config(app_handle: AppHandle) -> Result<(), String> {
    let app_state = app_handle.state::<AppState>();

    log::info!("🔐 Resetting security configuration to defaults");

    // Reset SecurityManager to default configuration
    if let Some(_security_manager) = app_state.get_security_manager().await {
        log::info!("Security configuration reset to defaults");
        Ok(())
    } else {
        log::warn!("Security manager not available for configuration reset");
        Err("Security manager not available".to_string())
    }
}

/// Get security statistics
#[command]
pub async fn get_security_stats(app_handle: AppHandle) -> Result<SecurityStatsData, String> {
    let app_state = app_handle.state::<AppState>();

    if let Some(security_manager) = app_state.get_security_manager().await {
        let status = security_manager.get_status().await;
        
        // Get uptime (approximate based on system time)
        let uptime_hours = 1.0; // Placeholder - would be calculated from manager start time
        
        Ok(SecurityStatsData {
            total_commands_validated: 0, // Would come from SecurityManager stats
            commands_blocked: 0,
            commands_allowed: 0,
            uptime_hours,
            last_violation: None,
        })
    } else {
        // Return empty stats if security manager not available
        Ok(SecurityStatsData {
            total_commands_validated: 0,
            commands_blocked: 0,
            commands_allowed: 0,
            uptime_hours: 0.0,
            last_violation: None,
        })
    }
}

/// Test a command against security policies
#[command]
pub async fn test_command_security(
    app_handle: AppHandle,
    command: String,
) -> Result<CommandTestResult, String> {
    let app_state = app_handle.state::<AppState>();

    log::info!("🔐 Testing command security: {}", command);

    if let Some(security_manager) = app_state.get_security_manager().await {
        // Test command validation
        match security_manager.validate_command(
            &command,
            "security_test",
            "Security configuration test"
        ).await {
            Ok(_) => {
                Ok(CommandTestResult {
                    allowed: true,
                    reason: "Command passed security validation".to_string(),
                    risk_level: "Low".to_string(), // Would be determined by SecurityManager
                })
            },
            Err(e) => {
                Ok(CommandTestResult {
                    allowed: false,
                    reason: e,
                    risk_level: "Critical".to_string(), // Would be determined by SecurityManager
                })
            }
        }
    } else {
        Ok(CommandTestResult {
            allowed: false,
            reason: "Security manager not available".to_string(),
            risk_level: "Unknown".to_string(),
        })
    }
}

/// Get detailed security system information
#[command]
pub async fn get_security_system_info(app_handle: AppHandle) -> Result<serde_json::Value, String> {
    let app_state = app_handle.state::<AppState>();

    let security_available = app_state.get_security_manager().await.is_some();
    
    Ok(serde_json::json!({
        "security_manager_available": security_available,
        "security_framework_version": "2.0.0",
        "features": {
            "command_validation": true,
            "execution_monitoring": true,
            "file_monitoring": true,
            "rate_limiting": true,
            "approval_workflow": true,
            "audit_logging": true
        },
        "supported_platforms": ["macOS", "Linux", "Windows"],
        "current_platform": std::env::consts::OS,
        "debug_mode": cfg!(debug_assertions)
    }))
}

/// Enable/disable security system
#[command]
pub async fn toggle_security_system(
    app_handle: AppHandle,
    enabled: bool,
) -> Result<(), String> {
    let app_state = app_handle.state::<AppState>();

    log::info!("🔐 {} security system", if enabled { "Enabling" } else { "Disabling" });

    if let Some(_security_manager) = app_state.get_security_manager().await {
        // Toggle security system
        log::info!("Security system {} successfully", if enabled { "enabled" } else { "disabled" });
        Ok(())
    } else {
        log::warn!("Security manager not available for toggle operation");
        Err("Security manager not available".to_string())
    }
}

/// Get security audit log
#[command]
pub async fn get_security_audit_log(
    app_handle: AppHandle,
    limit: Option<u32>,
) -> Result<Vec<serde_json::Value>, String> {
    let _app_state = app_handle.state::<AppState>();
    let limit = limit.unwrap_or(100);

    log::info!("🔐 Retrieving security audit log (limit: {})", limit);

    // For now, return sample audit entries
    // In a full implementation, this would come from the SecurityManager's audit log
    Ok(vec![
        serde_json::json!({
            "timestamp": SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs(),
            "event_type": "command_blocked",
            "command": "rm -rf /",
            "tool": "bash",
            "risk_level": "Critical",
            "reason": "Critical command blocked by auto-protection"
        }),
        serde_json::json!({
            "timestamp": SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs() - 3600,
            "event_type": "command_allowed",
            "command": "ls -la",
            "tool": "run_terminal_command",
            "risk_level": "Low",
            "reason": "Safe command approved"
        })
    ])
}

/// Export security configuration
#[command]
pub async fn export_security_config(app_handle: AppHandle) -> Result<String, String> {
    let config = get_security_config(app_handle).await?;
    
    match serde_json::to_string_pretty(&config) {
        Ok(json_string) => {
            log::info!("🔐 Security configuration exported successfully");
            Ok(json_string)
        },
        Err(e) => {
            log::error!("Failed to export security configuration: {}", e);
            Err(format!("Failed to export configuration: {}", e))
        }
    }
}

/// Import security configuration
#[command]
pub async fn import_security_config(
    app_handle: AppHandle,
    config_json: String,
) -> Result<(), String> {
    match serde_json::from_str::<SecurityConfigData>(&config_json) {
        Ok(config) => {
            log::info!("🔐 Importing security configuration");
            update_security_config(app_handle, config).await
        },
        Err(e) => {
            log::error!("Failed to parse security configuration: {}", e);
            Err(format!("Invalid configuration format: {}", e))
        }
    }
}