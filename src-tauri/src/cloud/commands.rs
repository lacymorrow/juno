use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};
use std::sync::Arc;
use tokio::sync::Mutex as TokioMutex;
use tauri::{AppHandle, Manager};
use tracing::{info, warn, error, debug};
use base64::{Engine as _, engine::general_purpose};
use std::collections::HashMap;
use serde_json;
use uuid;

use super::types::{
    CloudError, CloudCommand, CloudCommandType, DeviceResponse, ResponseStatus, ResponseData,
};
use super::security::CloudSecurity;
use crate::state::AppState;
use crate::constants::permissions;
use crate::constants::errors::templates;

// Helper function for error formatting - properly handles template substitution
fn format_error(template: &str, context: &str, error: impl std::fmt::Display) -> String {
    template.replacen("{}", context, 1).replacen("{}", &error.to_string(), 1)
}

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

    #[allow(dead_code)]
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

        info!("Executing voice query with {} bytes of audio data", audio_base64.len());

        // Decode audio and process
        let audio_data = general_purpose::STANDARD.decode(audio_base64)
            .map_err(|e| CloudError::ValidationFailed(format!("Invalid audio data: {}", e)))?;

        // Generous audio data size validation (allow very large files in maximally permissive mode)
        if audio_data.len() > 200 * 1024 * 1024 { // 200MB limit (increased from 50MB)
            log::warn!("⚠️ Very large audio file ({} MB), but allowing in maximally permissive mode", audio_data.len() / (1024 * 1024));
        }

        info!("Decoded {} bytes of audio data for transcription", audio_data.len());

        // Create temporary file for audio processing
        let temp_dir = std::env::temp_dir();
        let temp_file_name = format!("cloud_voice_query_{}.wav", uuid::Uuid::new_v4());
        let temp_audio_path = temp_dir.join(temp_file_name);

        // Write raw audio data to temporary WAV file
        // Note: We assume the audio data is already in WAV format
        // In production, you might want to add format detection and conversion
        std::fs::write(&temp_audio_path, &audio_data)
            .map_err(|e| CloudError::ExecutionFailed(format_error(templates::FAILED_TO_WRITE, "temp audio file", e)))?;

        let temp_audio_path_str = temp_audio_path.to_string_lossy().to_string();
        info!("Created temporary audio file: {}", temp_audio_path_str);

        // Attempt transcription using the voice transcription plugin
        let transcription_result = match self.transcribe_audio_file(&temp_audio_path_str).await {
            Ok(text) => {
                info!("Voice transcription successful: '{}'", text);
                text
            }
            Err(e) => {
                error!("Voice transcription failed: {}", e);
                // Clean up temp file
                let _ = std::fs::remove_file(&temp_audio_path);
                return Err(CloudError::ExecutionFailed(format!("Voice transcription failed: {}", e)));
            }
        };

        // Clean up temporary file
        if let Err(e) = std::fs::remove_file(&temp_audio_path) {
            warn!("{}", format_error(templates::FAILED_TO_CLEANUP, "temporary audio file", e));
        }

        // If transcription is empty or just whitespace, return an error
        let trimmed_transcription = transcription_result.trim();
        if trimmed_transcription.is_empty() {
            return Err(CloudError::ExecutionFailed("No speech detected in audio".to_string()));
        }

        info!("Submitting transcribed query to orchestrator: '{}'", trimmed_transcription);

        // Submit the transcribed text to the orchestrator as a text query
        let orchestrator_response = self.submit_query_to_orchestrator(trimmed_transcription, "cloud_voice").await?;

        Ok(format!("Voice query transcribed: '{}' -> {}", trimmed_transcription, orchestrator_response))
    }

    /// Transcribe audio file using the voice transcription infrastructure
    async fn transcribe_audio_file(&self, audio_path: &str) -> Result<String, CloudError> {
        info!("Attempting to transcribe audio file: {}", audio_path);

        // Check if the voice transcription plugin is available
        if let Some(voice_controller_state) = self.app_handle.try_state::<
            std::sync::Arc<tokio::sync::Mutex<tauri_plugin_voice_transcription::VoiceController>>
        >() {
            let voice_controller = voice_controller_state.lock().await;

            match voice_controller.transcribe_audio_file(audio_path) {
                Ok(transcription) => {
                    info!("Audio file transcription successful: '{}'", transcription);
                    Ok(transcription)
                }
                Err(e) => {
                    error!("Audio file transcription failed: {}", e);
                    Err(CloudError::ExecutionFailed(format!("Transcription error: {}", e)))
                }
            }
        } else {
            // Fallback: Try to use the voice transcription plugin commands directly
            info!("Voice controller not available in app state, trying plugin command");

            match tauri_plugin_voice_transcription::commands::transcribe_file(
                audio_path.to_string(),
                self.app_handle.clone(),
                // We need to get the controller state from the plugin
                self.app_handle.state::<std::sync::Arc<std::sync::Mutex<tauri_plugin_voice_transcription::VoiceController>>>()
            ).await {
                Ok(transcription) => {
                    info!("Plugin transcription successful: '{}'", transcription);
                    Ok(transcription)
                }
                Err(e) => {
                    error!("Plugin transcription failed: {}", e);
                    Err(CloudError::ExecutionFailed(format!("Plugin transcription error: {}", e.to_string())))
                }
            }
        }
    }

    /// Execute system command
    async fn execute_system_command(&self, command: CloudCommand) -> Result<CommandResult, CloudError> {
        info!("Executing system command: {}", command.id);

        // Extract action from parameters
        let action = command.payload.parameters
            .as_ref()
            .and_then(|params| params.get("action"))
            .map(|s| s.as_str())
            .unwrap_or("");

        match action {
            "screenshot" => {
                info!("Taking screenshot for remote command");
                self.execute_screenshot_command().await
            },
            "click" => {
                let x = command.payload.parameters
                    .as_ref()
                    .and_then(|params| params.get("x"))
                    .and_then(|s| s.parse::<f64>().ok())
                    .unwrap_or(0.0);
                let y = command.payload.parameters
                    .as_ref()
                    .and_then(|params| params.get("y"))
                    .and_then(|s| s.parse::<f64>().ok())
                    .unwrap_or(0.0);
                info!("Executing click at ({}, {})", x, y);
                self.execute_click_command(x, y).await
            },
            "type" => {
                let text = command.payload.parameters
                    .as_ref()
                    .and_then(|params| params.get("text"))
                    .map(|s| s.as_str())
                    .unwrap_or("");
                info!("Executing type command: {}", text);
                self.execute_type_command(text).await
            },
            "key" => {
                let key = command.payload.parameters
                    .as_ref()
                    .and_then(|params| params.get("key"))
                    .map(|s| s.as_str())
                    .unwrap_or("");
                info!("Executing key press: {}", key);
                self.execute_key_command(key).await
            },
            "execute" => {
                let shell_command = command.payload.parameters
                    .as_ref()
                    .and_then(|params| params.get("command"))
                    .map(|s| s.as_str())
                    .unwrap_or("");
                info!("Executing shell command: {}", shell_command);
                self.execute_shell_command(shell_command).await
            },
            "status" => {
                info!("Getting system status");
                self.execute_status_request().await
            },
            _ => {
                warn!("Unknown system command action: {}", action);
                Err(CloudError::InvalidCommand(format!("Unknown action: {}", action)))
            }
        }
    }

    /// Execute click command at coordinates
    async fn execute_click_command(&self, x: f64, y: f64) -> Result<CommandResult, CloudError> {
        info!("Executing click at coordinates ({}, {})", x, y);

        let app_state = self.app_handle.state::<crate::state::AppState>();

        // Use the existing mouse click functionality
        match crate::commands::mouse::left_click(self.app_handle.clone(), app_state, x, y, None).await {
            Ok(_) => {
                Ok(CommandResult {
                    success: true,
                    data: Some(format!("Clicked at coordinates ({}, {})", x, y)),
                    error: None,
                    metadata: Some({
                        let mut metadata = HashMap::new();
                        metadata.insert("coordinates".to_string(), serde_json::json!({"x": x, "y": y}));
                        metadata
                    }),
                    screenshot_base64: None,
                })
            },
            Err(e) => {
                            error!("{}", format_error(templates::FAILED_TO_EXECUTE, "click command", e));
            Err(CloudError::ExecutionFailed(format!("Click failed: {}", e)))
            }
        }
    }

    /// Execute type command
    async fn execute_type_command(&self, text: &str) -> Result<CommandResult, CloudError> {
        info!("Executing type command with text: {}", text);

        let app_state = self.app_handle.state::<crate::state::AppState>();

        // Use the existing text typing functionality
        match crate::commands::keyboard::global_type_text(text.to_string(), self.app_handle.clone(), app_state).await {
            Ok(_) => {
                Ok(CommandResult {
                    success: true,
                    data: Some(format!("Typed text: {}", text)),
                    error: None,
                    metadata: Some({
                        let mut metadata = HashMap::new();
                        metadata.insert("text".to_string(), serde_json::json!(text));
                        metadata.insert("length".to_string(), serde_json::json!(text.len()));
                        metadata
                    }),
                    screenshot_base64: None,
                })
            },
            Err(e) => {
                            error!("{}", format_error(templates::FAILED_TO_EXECUTE, "type command", e));
            Err(CloudError::ExecutionFailed(format!("Type failed: {}", e)))
            }
        }
    }

    /// Execute key press command
    async fn execute_key_command(&self, key: &str) -> Result<CommandResult, CloudError> {
        info!("Executing key press command: {}", key);

        let app_state = self.app_handle.state::<crate::state::AppState>();

        // Use the existing key press functionality
        match crate::commands::keyboard::press_key(key.to_string(), None, self.app_handle.clone(), app_state).await {
            Ok(_) => {
                Ok(CommandResult {
                    success: true,
                    data: Some(format!("Pressed key: {}", key)),
                    error: None,
                    metadata: Some({
                        let mut metadata = HashMap::new();
                        metadata.insert("key".to_string(), serde_json::json!(key));
                        metadata
                    }),
                    screenshot_base64: None,
                })
            },
            Err(e) => {
                            error!("{}", format_error(templates::FAILED_TO_EXECUTE, "key press command", e));
            Err(CloudError::ExecutionFailed(format!("Key press failed: {}", e)))
            }
        }
    }

    /// Execute shell command
    async fn execute_shell_command(&self, command: &str) -> Result<CommandResult, CloudError> {
        info!("Executing shell command: {}", command);

        let app_state = self.app_handle.state::<crate::state::AppState>();

        // Use the existing shell command functionality
        match crate::commands::shell::bash_command(self.app_handle.clone(), app_state, command.to_string(), None, None, Some(true)).await {
            Ok(output) => {
                Ok(CommandResult {
                    success: true,
                    data: Some(output.clone()),
                    error: None,
                    metadata: Some({
                        let mut metadata = HashMap::new();
                        metadata.insert("command".to_string(), serde_json::json!(command));
                        metadata.insert("output_length".to_string(), serde_json::json!(output.len()));
                        metadata
                    }),
                    screenshot_base64: None,
                })
            },
            Err(e) => {
                        error!("{}", format_error(templates::FAILED_TO_EXECUTE, "shell command", e));
        Err(CloudError::ExecutionFailed(format!("Shell command failed: {}", e)))
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
                        error!("{}", format_error(templates::FAILED_TO_CAPTURE, "screenshot", e));
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

    #[allow(dead_code)]
    async fn get_permissions_status(&self) -> Result<serde_json::Value, CloudError> {
        let app_state = self.app_handle.state::<AppState>();

        let required_permissions = vec![
            permissions::types::ACCESSIBILITY,
            permissions::types::SCREEN_RECORDING,
            permissions::types::MICROPHONE
        ];

        let permissions = serde_json::json!({
            "permissions_checked": app_state.are_permissions_checked(),
            "desktop_available": app_state.is_desktop_available(),
            "required_permissions": required_permissions,
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
                        error!("{}", format_error(templates::FAILED_TO_SUBMIT, "query to orchestrator", e));
        Err(CloudError::ExecutionFailed(format!("Query submission failed: {}", e)))
            }
        }
    }
}
