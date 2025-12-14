// Unrestricted Mode Commands - Control full system access
use crate::state::AppState;
use crate::commands::computer::unrestricted_computer::{UnrestrictedConfig, UnrestrictedComputer};
use serde::{Deserialize, Serialize};
use tauri::State;
use tracing::{info, warn};

#[derive(Debug, Serialize, Deserialize)]
pub struct UnrestrictedModeStatus {
    pub enabled: bool,
    pub config: UnrestrictedConfig,
    pub warning: String,
}

/// Enable unrestricted mode - grants full system access
#[tauri::command]
pub async fn enable_unrestricted_mode(
    state: State<'_, AppState>,
) -> Result<UnrestrictedModeStatus, String> {
    warn!("ENABLING UNRESTRICTED MODE - Full system access will be granted");
    
    // Enable unrestricted mode
    state.enable_unrestricted_mode();
    
    Ok(UnrestrictedModeStatus {
        enabled: true,
        config: state.get_unrestricted_config(),
        warning: "⚠️ UNRESTRICTED MODE ACTIVE: Juno has full system access. All security restrictions are disabled.".to_string(),
    })
}

/// Disable unrestricted mode - returns to normal security
#[tauri::command]
pub async fn disable_unrestricted_mode(
    state: State<'_, AppState>,
) -> Result<UnrestrictedModeStatus, String> {
    info!("Disabling unrestricted mode - returning to normal security");
    
    // Disable unrestricted mode
    state.disable_unrestricted_mode();
    
    Ok(UnrestrictedModeStatus {
        enabled: false,
        config: UnrestrictedConfig {
            bypass_all_permissions: false,
            allow_system_modifications: false,
            allow_kernel_access: false,
            allow_driver_installation: false,
            allow_firmware_access: false,
            disable_all_sandboxing: false,
            full_admin_privileges: false,
        },
        warning: "Normal security mode active. System access is restricted.".to_string(),
    })
}

/// Get current unrestricted mode status
#[tauri::command]
pub async fn get_unrestricted_status(
    state: State<'_, AppState>,
) -> Result<UnrestrictedModeStatus, String> {
    let enabled = state.is_unrestricted_mode();
    let config = state.get_unrestricted_config();
    
    let warning = if enabled {
        "⚠️ UNRESTRICTED MODE ACTIVE: Juno has full system access.".to_string()
    } else {
        "Normal security mode active.".to_string()
    };
    
    Ok(UnrestrictedModeStatus {
        enabled,
        config,
        warning,
    })
}

/// Update unrestricted mode configuration
#[tauri::command]
pub async fn update_unrestricted_config(
    config: UnrestrictedConfig,
    state: State<'_, AppState>,
) -> Result<UnrestrictedModeStatus, String> {
    info!("Updating unrestricted mode configuration");
    
    // Update the configuration
    state.set_unrestricted_config(config.clone());
    
    // If any high-risk feature is enabled, ensure unrestricted mode is on
    if config.allow_kernel_access || config.allow_driver_installation || config.allow_firmware_access {
        state.enable_unrestricted_mode();
        warn!("High-risk features enabled - activating unrestricted mode");
    }
    
    Ok(UnrestrictedModeStatus {
        enabled: state.is_unrestricted_mode(),
        config,
        warning: "Configuration updated".to_string(),
    })
}

/// Execute a system-level operation in unrestricted mode
#[tauri::command]
pub async fn execute_unrestricted(
    operation: String,
    parameters: serde_json::Value,
    state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    // Check if unrestricted mode is enabled
    if !state.is_unrestricted_mode() {
        return Err("Unrestricted mode is not enabled. Enable it first to perform system-level operations.".to_string());
    }
    
    warn!("Executing unrestricted operation: {}", operation);
    
    // Create unrestricted computer instance
    let computer = UnrestrictedComputer::new();
    
    // Execute the requested operation
    match operation.as_str() {
        "system_command" => {
            let command = parameters["command"].as_str()
                .ok_or("Missing command parameter")?;
            let args = parameters["args"].as_array()
                .map(|arr| arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect())
                .unwrap_or_default();
            
            let result = computer.execute_system_command(command, args).await?;
            
            use base64::Engine;
            Ok(serde_json::json!({
                "success": true,
                "result": base64::engine::general_purpose::STANDARD.encode(result)
            }))
        },
        "admin_command" => {
            let command = parameters["command"].as_str()
                .ok_or("Missing command parameter")?;
            let args = parameters["args"].as_array()
                .map(|arr| arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect())
                .unwrap_or_default();
            
            let result = computer.execute_as_admin(command, args).await
                .map_err(|e| e.to_string())?;
            
            Ok(serde_json::json!({
                "success": true,
                "output": result
            }))
        },
        "file_operation" => {
            let path = parameters["path"].as_str()
                .ok_or("Missing path parameter")?;
            let path_buf = std::path::PathBuf::from(path);
            
            let operation = if let Some(data_str) = parameters["write_data"].as_str() {
                use base64::Engine;
                let data = base64::engine::general_purpose::STANDARD.decode(data_str)
                    .map_err(|e| format!("Invalid base64 data: {}", e))?;
                crate::commands::computer::unrestricted_computer::FileOperation::Write(data)
            } else if parameters["delete"].as_bool().unwrap_or(false) {
                crate::commands::computer::unrestricted_computer::FileOperation::Delete
            } else if parameters["execute"].as_bool().unwrap_or(false) {
                crate::commands::computer::unrestricted_computer::FileOperation::Execute
            } else {
                crate::commands::computer::unrestricted_computer::FileOperation::Read
            };
            
            let result = computer.access_any_file(&path_buf, operation).await?;
            
            use base64::Engine;
            Ok(serde_json::json!({
                "success": true,
                "data": base64::engine::general_purpose::STANDARD.encode(result)
            }))
        },
        _ => {
            Err(format!("Unknown unrestricted operation: {}", operation))
        }
    }
}

/// Emergency shutdown - immediately disable all unrestricted access
#[tauri::command]
pub async fn emergency_shutdown(
    state: State<'_, AppState>,
) -> Result<String, String> {
    warn!("EMERGENCY SHUTDOWN - Disabling all unrestricted access");
    
    // Disable unrestricted mode
    state.disable_unrestricted_mode();
    
    // Reset configuration to safe defaults
    state.set_unrestricted_config(UnrestrictedConfig {
        bypass_all_permissions: false,
        allow_system_modifications: false,
        allow_kernel_access: false,
        allow_driver_installation: false,
        allow_firmware_access: false,
        disable_all_sandboxing: false,
        full_admin_privileges: false,
    });
    
    Ok("Emergency shutdown complete. All unrestricted access has been disabled.".to_string())
}