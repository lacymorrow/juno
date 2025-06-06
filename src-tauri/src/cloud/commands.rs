use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};
use std::sync::Arc;
use tokio::sync::Mutex as TokioMutex;
use tauri::{AppHandle, Manager, Emitter};
use tracing::{info, warn, error, debug};

use super::types::{
    CloudError, CloudCommand, CloudCommandType, DeviceResponse, ResponseStatus, ResponseData,
    AgentMode,
};
use super::security::CloudSecurity;
use crate::state::AppState;

/// Remote command that can be executed on the device
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteCommand {
    pub id: String,
    pub command_type: CloudCommandType,
    pub payload: serde_json::Value,
    pub timestamp: u64,
}

/// Command execution result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandResult {
    pub success: bool,
    pub data: Option<serde_json::Value>,
    pub error: Option<String>,
    pub metadata: Option<std::collections::HashMap<String, serde_json::Value>>,
}

/// Cloud command processor
#[derive(Debug, Clone)]
pub struct CloudCommandProcessor {
    app_handle: AppHandle,
    security: Arc<TokioMutex<CloudSecurity>>,
}

impl CloudCommandProcessor {
    /// Create new command processor
    pub fn new(app_handle: AppHandle, security: CloudSecurity) -> Self {
        Self {
            app_handle,
            security: Arc::new(TokioMutex::new(security)),
        }
    }
    
    /// Process incoming cloud command
    pub async fn process_command(&self, command: CloudCommand) -> Result<DeviceResponse, CloudError> {
        let command_id = command.id.clone();
        
        info!("Processing cloud command: {} ({})", command_id, self.command_type_to_string(&command.command_type));
        
        // Validate command security
        {
            let security = self.security.lock().await;
            security.validate_command(&command)?;
            
            // Check rate limits
            security.check_rate_limit(&command.command_type)?;
            
            // Log sanitized command
            let sanitized = security.sanitize_for_logging(&command);
            debug!("Command details: {:?}", sanitized);
        }
        
        // Execute the command
        let result = self.execute_command(command.clone()).await;
        
        // Create audit log
        {
            let security = self.security.lock().await;
            let audit_entry = security.create_audit_log(&command, &result.as_ref().map(|_| ()));
            debug!("Audit log: {:?}", audit_entry);
        }
        
        // Convert result to device response
        let response = match result {
            Ok(command_result) => DeviceResponse {
                command_id,
                status: if command_result.success { ResponseStatus::Success } else { ResponseStatus::Error },
                data: ResponseData {
                    text: command_result.data.as_ref().and_then(|d| d.get("text").and_then(|t| t.as_str().map(String::from))),
                    audio_base64: command_result.data.as_ref().and_then(|d| d.get("audio_base64").and_then(|a| a.as_str().map(String::from))),
                    screenshot_base64: command_result.data.as_ref().and_then(|d| d.get("screenshot_base64").and_then(|s| s.as_str().map(String::from))),
                    agent_state: command_result.data.as_ref().and_then(|d| d.get("agent_state").and_then(|s| s.as_str().map(String::from))),
                    progress: command_result.data.as_ref().and_then(|d| d.get("progress").and_then(|p| p.as_f64().map(|f| f as f32))),
                    metadata: command_result.metadata,
                },
                timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
                error: command_result.error,
            },
            Err(e) => DeviceResponse {
                command_id,
                status: ResponseStatus::Error,
                data: ResponseData {
                    text: None,
                    audio_base64: None,
                    screenshot_base64: None,
                    agent_state: Some("Failed".to_string()),
                    progress: None,
                    metadata: None,
                },
                timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
                error: Some(e.to_string()),
            },
        };
        
        info!("Command {} completed with status: {:?}", command_id, response.status);
        Ok(response)
    }
    
    /// Execute specific command based on type
    async fn execute_command(&self, command: CloudCommand) -> Result<CommandResult, CloudError> {
        match command.command_type {
            CloudCommandType::TextQuery => self.execute_text_query(command).await,
            CloudCommandType::VoiceQuery => self.execute_voice_query(command).await,
            CloudCommandType::SystemCommand => self.execute_system_command(command).await,
            CloudCommandType::StatusRequest => self.execute_status_request(command).await,
            CloudCommandType::Screenshot => self.execute_screenshot_command(command).await,
            CloudCommandType::ConfigUpdate => self.execute_config_update(command).await,
        }
    }
    
    /// Execute text query command
    async fn execute_text_query(&self, command: CloudCommand) -> Result<CommandResult, CloudError> {
        let query = command.payload.query
            .ok_or_else(|| CloudError::ValidationFailed("Text query requires query field".to_string()))?;
        
        info!("Executing text query: {}", query.chars().take(100).collect::<String>());
        
        // Get the app state
        let app_state = self.app_handle.state::<AppState>();
        
        // Execute the query using the existing submit_query function
        match crate::anthropic::submit_query(query.clone(), app_state, self.app_handle.clone()).await {
            Ok(()) => {
                // The submit_query function handles the response via events
                // For cloud response, we need to capture the result differently
                // For now, return success - we'll improve this in the integration phase
                Ok(CommandResult {
                    success: true,
                    data: Some(serde_json::json!({
                        "text": "Query submitted successfully",
                        "agent_state": "Processing"
                    })),
                    error: None,
                    metadata: None,
                })
            },
            Err(e) => {
                error!("Failed to execute text query: {}", e);
                Ok(CommandResult {
                    success: false,
                    data: None,
                    error: Some(e),
                    metadata: None,
                })
            }
        }
    }
    
    /// Execute voice query command
    async fn execute_voice_query(&self, command: CloudCommand) -> Result<CommandResult, CloudError> {
        let audio_base64 = command.payload.audio_base64
            .ok_or_else(|| CloudError::ValidationFailed("Voice query requires audio_base64 field".to_string()))?;
        
        info!("Executing voice query with audio data");
        
        // Decode audio and save temporarily
        let audio_data = base64::decode(audio_base64)
            .map_err(|e| CloudError::ValidationFailed(format!("Invalid base64 audio: {}", e)))?;
        
        // For now, return a placeholder response
        // In a full implementation, we would:
        // 1. Save the audio to a temporary file
        // 2. Use the voice transcription plugin to convert to text
        // 3. Execute the resulting text query
        
        Ok(CommandResult {
            success: true,
            data: Some(serde_json::json!({
                "text": "Voice query processing not yet implemented",
                "agent_state": "Processing"
            })),
            error: None,
            metadata: Some([("audio_size".to_string(), serde_json::json!(audio_data.len()))].iter().cloned().collect()),
        })
    }
    
    /// Execute system command
    async fn execute_system_command(&self, command: CloudCommand) -> Result<CommandResult, CloudError> {
        let parameters = command.payload.parameters
            .ok_or_else(|| CloudError::ValidationFailed("System command requires parameters".to_string()))?;
        
        info!("Executing system command with parameters: {:?}", parameters);
        
        // Basic system commands
        match parameters.get("action").and_then(|v| v.as_str()) {
            Some("get_system_info") => {
                let system_info = self.get_system_info().await?;
                Ok(CommandResult {
                    success: true,
                    data: Some(system_info),
                    error: None,
                    metadata: None,
                })
            },
            Some("get_permissions") => {
                let permissions = self.get_permissions_status().await?;
                Ok(CommandResult {
                    success: true,
                    data: Some(permissions),
                    error: None,
                    metadata: None,
                })
            },
            Some(action) => {
                warn!("Unknown system command action: {}", action);
                Err(CloudError::ValidationFailed(format!("Unknown system action: {}", action)))
            },
            None => {
                Err(CloudError::ValidationFailed("System command requires 'action' parameter".to_string()))
            }
        }
    }
    
    /// Execute status request
    async fn execute_status_request(&self, _command: CloudCommand) -> Result<CommandResult, CloudError> {
        let app_state = self.app_handle.state::<AppState>();
        
        let status = serde_json::json!({
            "device_status": "online",
            "agent_mode": crate::agent::providers::factory::BrainFactory::get_agent_mode(),
            "cloud_enabled": true,
            "desktop_available": app_state.is_desktop_available(),
            "permissions_checked": app_state.are_permissions_checked(),
            "timestamp": SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()
        });
        
        Ok(CommandResult {
            success: true,
            data: Some(status),
            error: None,
            metadata: None,
        })
    }
    
    /// Execute screenshot command
    async fn execute_screenshot_command(&self, _command: CloudCommand) -> Result<CommandResult, CloudError> {
        info!("Capturing screenshot for cloud");
        
        let app_state = self.app_handle.state::<AppState>();
        
        // Use existing screenshot functionality
        match crate::commands::capture_screenshot_command(app_state).await {
            Ok(screenshot_base64) => {
                Ok(CommandResult {
                    success: true,
                    data: Some(serde_json::json!({
                        "screenshot_base64": screenshot_base64
                    })),
                    error: None,
                    metadata: Some([("capture_time".to_string(), serde_json::json!(SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()))].iter().cloned().collect()),
                })
            },
            Err(e) => {
                error!("Failed to capture screenshot: {}", e);
                Ok(CommandResult {
                    success: false,
                    data: None,
                    error: Some(e),
                    metadata: None,
                })
            }
        }
    }
    
    /// Execute configuration update
    async fn execute_config_update(&self, command: CloudCommand) -> Result<CommandResult, CloudError> {
        let _config_data = command.payload.config
            .ok_or_else(|| CloudError::ValidationFailed("Config update requires config field".to_string()))?;
        
        info!("Processing configuration update");
        
        // For now, just acknowledge the config update
        // In a full implementation, we would update the appropriate settings
        
        Ok(CommandResult {
            success: true,
            data: Some(serde_json::json!({
                "text": "Configuration update received",
                "agent_state": "Updated"
            })),
            error: None,
            metadata: None,
        })
    }
    
    /// Get system information
    async fn get_system_info(&self) -> Result<serde_json::Value, CloudError> {
        let app_state = self.app_handle.state::<AppState>();
        
        let system_info = serde_json::json!({
            "platform": std::env::consts::OS,
            "architecture": std::env::consts::ARCH,
            "version": env!("CARGO_PKG_VERSION"),
            "desktop_available": app_state.is_desktop_available(),
            "agent_mode": format!("{:?}", crate::agent::providers::factory::BrainFactory::get_agent_mode()),
            "capabilities": self.get_device_capabilities(),
            "uptime": "unknown", // TODO: Track application uptime
        });
        
        Ok(system_info)
    }
    
    /// Get permissions status
    async fn get_permissions_status(&self) -> Result<serde_json::Value, CloudError> {
        let app_state = self.app_handle.state::<AppState>();
        
        let permissions = serde_json::json!({
            "permissions_checked": app_state.are_permissions_checked(),
            "desktop_available": app_state.is_desktop_available(),
            "required_permissions": [
                "accessibility",
                "screen_recording",
                "microphone"
            ],
            "status": if app_state.is_desktop_available() { "granted" } else { "pending" }
        });
        
        Ok(permissions)
    }
    
    /// Get device capabilities
    fn get_device_capabilities(&self) -> Vec<String> {
        vec![
            "text_processing".to_string(),
            "voice_transcription".to_string(),
            "screenshot_capture".to_string(),
            "system_automation".to_string(),
            "file_operations".to_string(),
            "web_browsing".to_string(),
        ]
    }
    
    /// Convert command type to string for logging
    fn command_type_to_string(&self, command_type: &CloudCommandType) -> &'static str {
        match command_type {
            CloudCommandType::VoiceQuery => "voice_query",
            CloudCommandType::TextQuery => "text_query",
            CloudCommandType::SystemCommand => "system_command",
            CloudCommandType::StatusRequest => "status_request",
            CloudCommandType::Screenshot => "screenshot",
            CloudCommandType::ConfigUpdate => "config_update",
        }
    }
}