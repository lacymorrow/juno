//! Enhanced Error Recovery Commands for Juno Computer Use Agent
//!
//! Provides commands for managing the enhanced error recovery system,
//! including checkpoint management, rollback operations, and recovery statistics.

use crate::agent::error_recovery::{ErrorRecoveryManager, RecoveryConfig};
use crate::constants::errors::templates;
use crate::state::AppState;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::{command, State};
use tokio::sync::Mutex;
use tracing::{error, info};

/// Format error message with template substitution
fn format_error(template: &str, context: &str, error: impl std::fmt::Display) -> String {
    template
        .replacen("{}", context, 1)
        .replacen("{}", &error.to_string(), 1)
}

/// Global error recovery manager instance
static ERROR_RECOVERY_MANAGER: std::sync::OnceLock<Arc<Mutex<ErrorRecoveryManager>>> =
    std::sync::OnceLock::new();

/// Configuration for error recovery operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorRecoveryConfigDTO {
    pub enable_checkpoints: bool,
    pub max_checkpoints: usize,
    pub checkpoint_interval: u32,
    pub enable_automatic_rollback: bool,
    pub rollback_on_cascading_failures: bool,
    pub max_retries: usize,
    pub enable_alternative_methods: bool,
    pub enable_user_escalation: bool,
}

impl From<ErrorRecoveryConfigDTO> for RecoveryConfig {
    fn from(dto: ErrorRecoveryConfigDTO) -> Self {
        Self {
            max_retries: dto.max_retries,
            base_retry_delay: std::time::Duration::from_millis(500),
            max_retry_delay: std::time::Duration::from_secs(
                crate::constants::timeouts::ERROR_RECOVERY_MAX_RETRY_DELAY_SECONDS,
            ),
            enable_alternative_methods: dto.enable_alternative_methods,
            enable_llm_recovery: true,
            enable_user_escalation: dto.enable_user_escalation,
            timeout_threshold: std::time::Duration::from_secs(
                crate::constants::timeouts::ERROR_RECOVERY_TIMEOUT_THRESHOLD_SECONDS,
            ),
            enable_checkpoints: dto.enable_checkpoints,
            max_checkpoints: dto.max_checkpoints,
            checkpoint_interval: dto.checkpoint_interval,
            enable_automatic_rollback: dto.enable_automatic_rollback,
            rollback_on_cascading_failures: dto.rollback_on_cascading_failures,
        }
    }
}

/// Result of checkpoint operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointResult {
    pub success: bool,
    pub checkpoint_id: Option<String>,
    pub description: String,
    pub error: Option<String>,
}

/// Result of rollback operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RollbackResult {
    pub success: bool,
    pub checkpoint_id: Option<String>,
    pub operations_undone: usize,
    pub description: String,
    pub error: Option<String>,
}

/// Get or initialize the global error recovery manager
async fn get_recovery_manager() -> Arc<Mutex<ErrorRecoveryManager>> {
    ERROR_RECOVERY_MANAGER
        .get_or_init(|| Arc::new(Mutex::new(ErrorRecoveryManager::new())))
        .clone()
}

/// Initialize the enhanced error recovery system
#[command]
pub async fn initialize_error_recovery(_app_state: State<'_, AppState>) -> Result<String, String> {
    info!("Initializing Enhanced Error Recovery System");

    let manager = get_recovery_manager().await;
    let mut manager_guard = manager.lock().await;

    // Reset to ensure clean state
    manager_guard.reset_checkpoints();

    info!("Enhanced Error Recovery System initialized successfully");
    Ok(
        "Enhanced Error Recovery System initialized with checkpoint and rollback capabilities"
            .to_string(),
    )
}

/// Create a new execution checkpoint
#[command]
pub async fn create_checkpoint(
    _app_state: State<'_, AppState>,
    description: String,
) -> Result<CheckpointResult, String> {
    info!("Creating execution checkpoint: {}", description);

    let manager = get_recovery_manager().await;
    let mut manager_guard = manager.lock().await;

    match manager_guard.create_checkpoint(description.clone()) {
        Ok(checkpoint_id) => {
            info!("Checkpoint created successfully: {}", checkpoint_id);
            Ok(CheckpointResult {
                success: true,
                checkpoint_id: Some(checkpoint_id),
                description,
                error: None,
            })
        }
        Err(e) => {
            error!(
                "{}",
                format_error(templates::FAILED_TO_CREATE, "checkpoint", &e)
            );
            Ok(CheckpointResult {
                success: false,
                checkpoint_id: None,
                description,
                error: Some(e.to_string()),
            })
        }
    }
}

/// Rollback to a specific checkpoint
#[command]
pub async fn rollback_to_checkpoint(
    _app_state: State<'_, AppState>,
    checkpoint_id: String,
) -> Result<RollbackResult, String> {
    info!("Rolling back to checkpoint: {}", checkpoint_id);

    let manager = get_recovery_manager().await;
    let mut manager_guard = manager.lock().await;

    match manager_guard.rollback_to_checkpoint(&checkpoint_id).await {
        Ok(rollback_info) => {
            info!(
                "Rollback completed successfully to checkpoint: {}",
                checkpoint_id
            );
            Ok(RollbackResult {
                success: true,
                checkpoint_id: Some(rollback_info.checkpoint_id),
                operations_undone: rollback_info.operations_to_undo.len(),
                description: rollback_info.rollback_reason,
                error: None,
            })
        }
        Err(e) => {
            error!(
                "Failed to rollback to checkpoint '{}': {}",
                checkpoint_id, e
            );
            Ok(RollbackResult {
                success: false,
                checkpoint_id: Some(checkpoint_id),
                operations_undone: 0,
                description: format!("Rollback failed: {}", e),
                error: Some(e.to_string()),
            })
        }
    }
}

/// Rollback to the last known good state
#[command]
pub async fn rollback_to_last_known_good(
    _app_state: State<'_, AppState>,
) -> Result<RollbackResult, String> {
    info!("Rolling back to last known good state");

    let manager = get_recovery_manager().await;
    let mut manager_guard = manager.lock().await;

    match manager_guard.rollback_to_last_known_good().await {
        Ok(rollback_info) => {
            info!("Rollback to last known good state completed successfully");
            Ok(RollbackResult {
                success: true,
                checkpoint_id: Some(rollback_info.checkpoint_id),
                operations_undone: rollback_info.operations_to_undo.len(),
                description: rollback_info.rollback_reason,
                error: None,
            })
        }
        Err(e) => {
            error!("Failed to rollback to last known good state: {}", e);
            Ok(RollbackResult {
                success: false,
                checkpoint_id: None,
                operations_undone: 0,
                description: format!("Rollback failed: {}", e),
                error: Some(e.to_string()),
            })
        }
    }
}

/// Get enhanced recovery statistics
#[command]
pub async fn get_recovery_statistics(
    _app_state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    info!("Retrieving enhanced error recovery statistics");

    let manager = get_recovery_manager().await;
    let manager_guard = manager.lock().await;

    Ok(manager_guard.get_enhanced_recovery_stats())
}

/// Update error recovery configuration
#[command]
pub async fn update_recovery_config(
    _app_state: State<'_, AppState>,
    config: ErrorRecoveryConfigDTO,
) -> Result<String, String> {
    info!("Updating error recovery configuration");

    let manager = get_recovery_manager().await;
    let mut manager_guard = manager.lock().await;

    let recovery_config = RecoveryConfig::from(config);
    *manager_guard = ErrorRecoveryManager::with_config(recovery_config);

    info!("Error recovery configuration updated successfully");
    Ok("Error recovery configuration updated successfully".to_string())
}

/// Get current recovery configuration
#[command]
pub async fn get_recovery_config(
    _app_state: State<'_, AppState>,
) -> Result<ErrorRecoveryConfigDTO, String> {
    info!("Retrieving current error recovery configuration");

    let manager = get_recovery_manager().await;
    let manager_guard = manager.lock().await;
    let stats = manager_guard.get_enhanced_recovery_stats();

    // Extract config from stats
    let config = stats["config"].clone();

    Ok(ErrorRecoveryConfigDTO {
        enable_checkpoints: config["enable_checkpoints"].as_bool().unwrap_or(true),
        max_checkpoints: config["max_checkpoints"].as_u64().unwrap_or(10) as usize,
        checkpoint_interval: config["checkpoint_interval"].as_u64().unwrap_or(3) as u32,
        enable_automatic_rollback: config["enable_automatic_rollback"]
            .as_bool()
            .unwrap_or(true),
        rollback_on_cascading_failures: config["rollback_on_cascading_failures"]
            .as_bool()
            .unwrap_or(true),
        max_retries: config["max_retries"].as_u64().unwrap_or(3) as usize,
        enable_alternative_methods: config["enable_alternative_methods"]
            .as_bool()
            .unwrap_or(true),
        enable_user_escalation: config["enable_user_escalation"].as_bool().unwrap_or(false),
    })
}

/// List all available checkpoints
#[command]
pub async fn list_checkpoints(
    _app_state: State<'_, AppState>,
) -> Result<Vec<serde_json::Value>, String> {
    info!("Listing all available checkpoints");

    let manager = get_recovery_manager().await;
    let manager_guard = manager.lock().await;
    let stats = manager_guard.get_enhanced_recovery_stats();

    // Extract checkpoint information from stats
    let checkpoint_count = stats["checkpoints"]["total_created"].as_u64().unwrap_or(0);

    // For now, return basic checkpoint info
    // In a full implementation, we'd expose the actual checkpoint list
    let mut checkpoints = Vec::new();
    for i in 0..checkpoint_count {
        checkpoints.push(serde_json::json!({
            "id": format!("checkpoint_{}", i),
            "description": format!("Checkpoint {}", i + 1),
            "step": i + 1,
            "timestamp": "recent"
        }));
    }

    Ok(checkpoints)
}

/// Clear all checkpoints and reset state
#[command]
pub async fn reset_recovery_state(_app_state: State<'_, AppState>) -> Result<String, String> {
    info!("Resetting error recovery state");

    let manager = get_recovery_manager().await;
    let mut manager_guard = manager.lock().await;

    manager_guard.reset_checkpoints();
    manager_guard.clear_history();

    info!("Error recovery state reset successfully");
    Ok("Error recovery state reset successfully".to_string())
}

/// Test the error recovery system with a simulated failure
#[command]
pub async fn test_error_recovery(
    _app_state: State<'_, AppState>,
    error_type: String,
) -> Result<serde_json::Value, String> {
    info!(
        "Testing error recovery system with error type: {}",
        error_type
    );

    let manager = get_recovery_manager().await;
    let manager_guard = manager.lock().await;

    // Simulate different error patterns for testing
    use crate::agent::core::AgentError;

    let test_error = match error_type.as_str() {
        "element_not_found" => AgentError::ToolError("Element not found on screen".to_string()),
        "network_error" => AgentError::ToolError("Connection failed".to_string()),
        "permission_denied" => {
            AgentError::PermissionDenied("Accessibility permission required".to_string())
        }
        "timeout" => AgentError::ToolError("Operation timed out".to_string()),
        "state_corruption" => AgentError::Unknown("Invalid state detected".to_string()),
        "cascading_failure" => AgentError::Unknown("Multiple related failures".to_string()),
        _ => AgentError::Unknown("Generic test error".to_string()),
    };

    let error_pattern = manager_guard.determine_error_pattern(&test_error);
    let strategies = manager_guard
        .get_strategy_mappings()
        .get(&error_pattern)
        .cloned()
        .unwrap_or_default();

    Ok(serde_json::json!({
        "error_type": error_type,
        "detected_pattern": format!("{:?}", error_pattern),
        "recovery_strategies": strategies.iter().map(|s| format!("{:?}", s)).collect::<Vec<_>>(),
        "test_successful": true
    }))
}

/// Update agent state for checkpoint context
#[command]
pub async fn update_agent_state(
    _app_state: State<'_, AppState>,
    key: String,
    value: serde_json::Value,
) -> Result<String, String> {
    info!("Updating agent state: {} = {:?}", key, value);

    let manager = get_recovery_manager().await;
    let mut manager_guard = manager.lock().await;

    manager_guard.update_agent_state(&key, value);

    Ok(format!("Agent state updated: {}", key))
}

/// Get execution history summary
#[command]
pub async fn get_execution_history(
    _app_state: State<'_, AppState>,
    limit: Option<usize>,
) -> Result<serde_json::Value, String> {
    info!("Retrieving execution history (limit: {:?})", limit);

    let manager = get_recovery_manager().await;
    let manager_guard = manager.lock().await;
    let stats = manager_guard.get_enhanced_recovery_stats();

    let history = stats["execution_history"].clone();

    Ok(serde_json::json!({
        "summary": history,
        "note": "Full execution history tracking requires integration with agent execution system"
    }))
}
