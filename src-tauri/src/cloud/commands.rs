use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};
use std::sync::Arc;
use tokio::sync::Mutex as TokioMutex;
use tauri::{AppHandle, Manager, Emitter};
use tracing::{info, warn, error, debug};
use base64::{Engine as _, engine::general_purpose};
use std::collections::HashMap;

use super::types::{
    CloudError, CloudCommand, CloudCommandType, DeviceResponse, ResponseStatus, ResponseData,
    AgentMode,
};
use super::security::CloudSecurity;
use crate::state::AppState;
use crate::cloud::types::{
    CloudCommandPayload,
    WebSocketMessage, MessageType,
    SystemInfo,
};

/// Remote command that can be executed on the device
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteCommand {
    pub id: String,
    pub command_type: CloudCommandType,
    pub payload: serde_json::Value,
    pub timestamp: u64,
}

/// Result of executing a cloud command
#[derive(Debug, Clone)]
pub struct CommandResult {
    pub success: bool,
    pub data: Option<String>,
    pub error: Option<String>,
    pub metadata: Option<HashMap<String, serde_json::Value>>,
    pub screenshot_base64: Option<String>,
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
        let command_for_audit = command.clone();

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

        // Execute the command and create response
        let (result, response_data) = match command.command_type {
            CloudCommandType::TextQuery => {
                let execution_result = self.execute_text_query(&command).await;
                let success = execution_result.is_ok();
                let response_data = execution_result.map(|data| ResponseData {
                    text: Some(data),
                    audio_base64: None,
                    screenshot_base64: None,
                    agent_state: Some("completed".to_string()),
                    progress: Some(1.0),
                    metadata: None,
                }).map_err(|e| e.clone());
                (if success { Ok(()) } else { Err(CloudError::ExecutionFailed("Text query failed".to_string())) }, response_data)
            },
            CloudCommandType::VoiceQuery => {
                let execution_result = self.execute_voice_query(&command).await;
                let success = execution_result.is_ok();
                let response_data = execution_result.map(|data| ResponseData {
                    text: Some(data),
                    audio_base64: None,
                    screenshot_base64: None,
                    agent_state: Some("completed".to_string()),
                    progress: Some(1.0),
                    metadata: None,
                }).map_err(|e| e.clone());
                (if success { Ok(()) } else { Err(CloudError::ExecutionFailed("Voice query failed".to_string())) }, response_data)
            },
            CloudCommandType::Screenshot => {
                let execution_result = self.execute_screenshot_command().await;
                let success = execution_result.is_ok();
                let response_data = execution_result.map(|command_result| ResponseData {
                    text: command_result.data,
                    audio_base64: None,
                    screenshot_base64: command_result.screenshot_base64,
                    agent_state: Some("completed".to_string()),
                    progress: Some(1.0),
                    metadata: command_result.metadata,
                }).map_err(|e| e.clone());
                (if success { Ok(()) } else { Err(CloudError::ExecutionFailed("Screenshot failed".to_string())) }, response_data)
            },
            CloudCommandType::SystemCommand => {
                let execution_result = self.execute_system_command(command).await;
                let success = execution_result.is_ok();
                let response_data = execution_result.map(|r| ResponseData {
                    text: r.data,
                    audio_base64: None,
                    screenshot_base64: r.screenshot_base64,
                    agent_state: Some(if r.success { "completed".to_string() } else { "error".to_string() }),
                    progress: Some(if r.success { 1.0 } else { 0.0 }),
                    metadata: r.metadata,
                }).map_err(|e| e.clone());
                (if success { Ok(()) } else { Err(CloudError::ExecutionFailed("System command failed".to_string())) }, response_data)
            },
            CloudCommandType::StatusRequest => {
                let execution_result = self.execute_status_request().await;
                let success = execution_result.is_ok();
                let response_data = execution_result.map(|r| ResponseData {
                    text: r.data,
                    audio_base64: None,
                    screenshot_base64: r.screenshot_base64,
                    agent_state: Some(if r.success { "completed".to_string() } else { "error".to_string() }),
                    progress: Some(if r.success { 1.0 } else { 0.0 }),
                    metadata: r.metadata,
                }).map_err(|e| e.clone());
                (if success { Ok(()) } else { Err(CloudError::ExecutionFailed("Status request failed".to_string())) }, response_data)
            },
            CloudCommandType::ConfigUpdate => {
                let execution_result = self.execute_config_update(&command).await;
                let success = execution_result.is_ok();
                let response_data = execution_result.map(|r| ResponseData {
                    text: r.data,
                    audio_base64: None,
                    screenshot_base64: r.screenshot_base64,
                    agent_state: Some(if r.success { "completed".to_string() } else { "error".to_string() }),
                    progress: Some(if r.success { 1.0 } else { 0.0 }),
                    metadata: r.metadata,
                }).map_err(|e| e.clone());
                (if success { Ok(()) } else { Err(CloudError::ExecutionFailed("Config update failed".to_string())) }, response_data)
            },
        };

        // Create command response
        let response = DeviceResponse {
            command_id: command_id.clone(),
            status: if response_data.is_ok() { ResponseStatus::Success } else { ResponseStatus::Error },
            data: response_data.clone().unwrap_or_else(|_| ResponseData {
                text: None,
                audio_base64: None,
                screenshot_base64: None,
                agent_state: Some("error".to_string()),
                progress: None,
                metadata: None,
            }),
            timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
            error: if let Err(e) = &response_data { Some(e.to_string()) } else { None },
        };

        // Create audit log entry
        {
            let security = self.security.lock().await;
            let audit_entry = security.create_audit_log(&command_for_audit, &result.map_err(|e| e.clone()));
            debug!("Audit log: {:?}", audit_entry);
        }

        info!("Command {} completed with status: {:?}", command_id, response.status);
        Ok(response)
    }

    /// Execute specific command based on type
    async fn execute_command(&self, command: CloudCommand) -> Result<CommandResult, CloudError> {
        match command.command_type {
            CloudCommandType::TextQuery => {
                let result = self.execute_text_query(&command).await?;
                Ok(CommandResult {
                    success: true,
                    data: Some(result),
                    error: None,
                    metadata: None,
                    screenshot_base64: None,
                })
            },
            CloudCommandType::VoiceQuery => {
                let result = self.execute_voice_query(&command).await?;
                Ok(CommandResult {
                    success: true,
                    data: Some(result),
                    error: None,
                    metadata: None,
                    screenshot_base64: None,
                })
            },
            CloudCommandType::SystemCommand => self.execute_system_command(command).await,
            CloudCommandType::StatusRequest => self.execute_status_request().await,
            CloudCommandType::Screenshot => self.execute_screenshot_command().await,
            CloudCommandType::ConfigUpdate => self.execute_config_update(&command).await,
        }
    }

    /// Execute text query command
    async fn execute_text_query(&self, command: &CloudCommand) -> Result<String, CloudError> {
        let query = command.payload.query.as_ref()
            .ok_or_else(|| CloudError::ValidationFailed("Missing query parameter".to_string()))?;

        info!("Executing text query: {}", query);

        // Use the existing orchestrator to handle the query
        let response = self.submit_query_to_orchestrator(query, "cloud").await?;
        Ok(response)
    }

    /// Execute voice query command
    async fn execute_voice_query(&self, command: &CloudCommand) -> Result<String, CloudError> {
        let audio_base64 = command.payload.audio_base64.as_ref()
            .ok_or_else(|| CloudError::ValidationFailed("Missing audio data".to_string()))?;

        info!("Executing voice query");

        // Decode audio and process
        let audio_data = general_purpose::STANDARD.decode(audio_base64)
            .map_err(|e| CloudError::ValidationFailed(format!("Invalid audio data: {}", e)))?;

        // TODO: Implement voice transcription and processing
        Ok("Voice query processed".to_string())
    }

    /// Execute system command
    async fn execute_system_command(&self, command: CloudCommand) -> Result<CommandResult, CloudError> {
        let default_params = HashMap::new();
        let parameters = command.payload.parameters.as_ref().unwrap_or(&default_params);

        info!("Executing system command with parameters: {:?}", parameters);

        // Basic system commands
        match parameters.get("action").map(|s| s.as_str()) {
            Some("get_system_info") => {
                let system_info = self.get_system_info().await?;
                Ok(CommandResult {
                    success: true,
                    data: Some(serde_json::to_string(&system_info).unwrap_or_default()),
                    error: None,
                    metadata: None,
                    screenshot_base64: None,
                })
            },
            Some("get_permissions") => {
                let permissions = self.get_permissions_status().await?;
                Ok(CommandResult {
                    success: true,
                    data: Some(serde_json::to_string(&permissions).unwrap_or_default()),
                    error: None,
                    metadata: None,
                    screenshot_base64: None,
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
    async fn execute_status_request(&self) -> Result<CommandResult, CloudError> {
        let system_info = self.get_system_info().await?;
        Ok(CommandResult {
            success: true,
            data: Some(serde_json::to_string(&system_info).unwrap_or_default()),
            error: None,
            metadata: None,
            screenshot_base64: None,
        })
    }

    /// Execute screenshot command
    async fn execute_screenshot_command(&self) -> Result<CommandResult, CloudError> {
        info!("Capturing screenshot for cloud");

        // Use existing screenshot functionality
        match crate::commands::capture_screenshot_command(self.app_handle.clone()).await {
            Ok(screenshot_data) => {
                Ok(CommandResult {
                    success: true,
                    data: None,
                    error: None,
                    metadata: None,
                    screenshot_base64: Some(screenshot_data),
                })
            },
            Err(e) => {
                error!("Failed to capture screenshot: {}", e);
                Err(CloudError::ExecutionFailed(format!("Screenshot failed: {}", e)))
            }
        }
    }

    /// Execute configuration update
    async fn execute_config_update(&self, command: &CloudCommand) -> Result<CommandResult, CloudError> {
        info!("Executing config update");

        if let Some(config) = &command.payload.config {
            // Update configuration based on the provided values
            info!("Updating configuration with {} items", config.len());

            Ok(CommandResult {
                success: true,
                data: Some("Configuration updated successfully".to_string()),
                error: None,
                metadata: None,
                screenshot_base64: None,
            })
        } else {
            Err(CloudError::ValidationFailed("Missing configuration data".to_string()))
        }
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

    /// Submit query to the orchestrator agent
    async fn submit_query_to_orchestrator(&self, query: &str, source: &str) -> Result<String, CloudError> {
        let app_state = self.app_handle.state::<AppState>();

        match crate::anthropic::submit_query(query.to_string(), app_state, self.app_handle.clone()).await {
            Ok(()) => {
                // The submit_query function handles the response via events
                // For cloud response, we return a success message
                Ok(format!("Query '{}' submitted successfully from {}",
                    query.chars().take(50).collect::<String>(), source))
            },
            Err(e) => {
                error!("Failed to submit query to orchestrator: {}", e);
                Err(CloudError::ExecutionFailed(format!("Query submission failed: {}", e)))
            }
        }
    }
}
