use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Command from cloud to device
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudCommand {
    pub id: String,
    #[serde(rename = "type")]
    pub command_type: CloudCommandType,
    pub payload: CloudCommandPayload,
    pub timestamp: u64,
    pub signature: Option<String>,
    pub metadata: Option<HashMap<String, String>>,
}

/// Types of commands that can be sent from cloud
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CloudCommandType {
    VoiceQuery,
    TextQuery,
    SystemCommand,
    StatusRequest,
    Screenshot,
    ConfigUpdate,
}

/// Payload for cloud commands
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudCommandPayload {
    pub query: Option<String>,
    pub audio_base64: Option<String>,
    pub mode: Option<AgentMode>,
    pub config: Option<HashMap<String, serde_json::Value>>,
    pub parameters: Option<HashMap<String, String>>,
}

/// Agent mode for command execution
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentMode {
    Agent,
    Dictation,
    System,
}

/// Response from device to cloud
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceResponse {
    pub command_id: String,
    pub status: ResponseStatus,
    pub data: ResponseData,
    pub timestamp: u64,
    pub error: Option<String>,
}

/// Status of command execution
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResponseStatus {
    Success,
    Error,
    InProgress,
    Cancelled,
}

/// Data payload in device response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseData {
    pub text: Option<String>,
    pub audio_base64: Option<String>,
    pub screenshot_data: Option<serde_json::Value>,
    pub agent_state: Option<String>,
    pub progress: Option<f32>,
    pub metadata: Option<HashMap<String, serde_json::Value>>,
}

/// Device status updates sent to cloud
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceStatus {
    pub device_id: String,
    pub status: DeviceState,
    pub current_task: Option<String>,
    pub system_info: SystemInfo,
    pub timestamp: u64,
}

/// Current state of the device
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeviceState {
    Online,
    Busy,
    Offline,
    Error,
    Maintenance,
}

/// System information about the device
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemInfo {
    pub platform: String,
    pub permissions: Vec<String>,
    pub agent_mode: String,
    pub version: String,
    pub capabilities: Vec<String>,
    pub hardware_info: Option<HardwareInfo>,
}

/// Hardware information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareInfo {
    pub cpu_usage: Option<f32>,
    pub memory_usage: Option<f32>,
    pub disk_usage: Option<f32>,
    pub screen_resolution: Option<String>,
}

/// Device registration information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceRegistration {
    pub device_id: String,
    pub device_name: String,
    pub api_key: String,
    pub platform: String,
    pub version: String,
    pub capabilities: Vec<String>,
    pub user_id: Option<String>,
}

/// Authentication response from cloud
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthResponse {
    pub success: bool,
    pub token: Option<String>,
    pub device_id: Option<String>,
    pub permissions: Option<Vec<String>>,
    pub expires_at: Option<u64>,
    pub error: Option<String>,
}

/// WebSocket message wrapper
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSocketMessage {
    #[serde(rename = "type")]
    pub message_type: MessageType,
    pub data: serde_json::Value,
    pub timestamp: u64,
}

/// Types of WebSocket messages
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum MessageType {
    Command,
    Response,
    Status,
    Heartbeat,
    Auth,
    Error,
}

/// Error types for cloud operations
#[derive(Debug, Clone, thiserror::Error)]
pub enum CloudError {
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),

    #[error("Authentication failed: {0}")]
    AuthenticationFailed(String),

    #[error("Command validation failed: {0}")]
    ValidationFailed(String),

    #[error("Command execution failed: {0}")]
    ExecutionFailed(String),

    #[error("Invalid command: {0}")]
    InvalidCommand(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("Network error: {0}")]
    NetworkError(String),

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Security error: {0}")]
    SecurityError(String),
}

impl From<serde_json::Error> for CloudError {
    fn from(error: serde_json::Error) -> Self {
        CloudError::SerializationError(error.to_string())
    }
}

impl From<tokio_tungstenite::tungstenite::Error> for CloudError {
    fn from(error: tokio_tungstenite::tungstenite::Error) -> Self {
        CloudError::NetworkError(error.to_string())
    }
}

impl From<String> for CloudError {
    fn from(error: String) -> Self {
        CloudError::ConfigError(error)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
    Authenticated,
    Reconnecting,
    Failed(String),
    Error(String),
}
