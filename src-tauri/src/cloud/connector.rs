use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::{Mutex as TokioMutex, mpsc, oneshot};
use tracing::{info, warn, error, debug};
use tauri::{AppHandle, Manager, Emitter};
use uuid::Uuid;

use super::types::{
    CloudError, CloudCommand, DeviceResponse, DeviceStatus, WebSocketMessage, MessageType,
    ConnectionState as CloudConnectionState, ResponseStatus, ResponseData,
};
use super::config::CloudConfig;
use super::auth::DeviceAuth;
use super::security::CloudSecurity;
use super::commands::CloudCommandProcessor;
use crate::constants::permission_types;

/// Production-ready cloud connector using official Tauri WebSocket plugin
#[derive(Debug)]
pub struct ProductionCloudConnector {
    config: CloudConfig,
    auth: DeviceAuth,
    security: CloudSecurity,
    command_processor: CloudCommandProcessor,
    app_handle: AppHandle,

    // Connection management
    connection_id: Arc<TokioMutex<Option<String>>>,
    connection_state: Arc<TokioMutex<ConnectorState>>,

    // Command tracking
    pending_commands: Arc<TokioMutex<HashMap<String, oneshot::Sender<DeviceResponse>>>>,

    // Communication channels
    command_tx: mpsc::UnboundedSender<ConnectorMessage>,
    command_rx: Arc<TokioMutex<mpsc::UnboundedReceiver<ConnectorMessage>>>,
}

/// Enhanced connection state for production use
#[derive(Debug, Clone, PartialEq)]
pub enum ConnectorState {
    Disconnected,
    Connecting,
    Connected,
    Authenticated,
    Synchronizing,
    Ready,
    Error(String),
    Reconnecting(u32), // retry count
}

/// Internal messages for the connector
#[derive(Debug)]
enum ConnectorMessage {
    Connect,
    Disconnect,
    SendCommand(CloudCommand),
    ProcessResponse(DeviceResponse),
    HandleError(String),
    UpdateStatus,
}

/// Connection statistics for monitoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionStats {
    pub connected_at: Option<u64>,
    pub total_commands: u64,
    pub successful_commands: u64,
    pub failed_commands: u64,
    pub reconnection_count: u32,
    pub last_heartbeat: Option<u64>,
    pub latency_ms: Option<u64>,
}

/// Command execution context for better tracking
#[derive(Debug, Clone)]
pub struct CommandContext {
    pub command_id: String,
    pub initiated_at: u64,
    pub timeout_seconds: Option<u64>,
    pub retry_count: u32,
    pub priority: CommandPriority,
}

/// Command priority levels
#[derive(Debug, Clone, PartialEq)]
pub enum CommandPriority {
    Low,
    Normal,
    High,
    Critical,
}

impl ProductionCloudConnector {
    /// Create new production cloud connector
    pub async fn new(app_handle: AppHandle) -> Result<Self, CloudError> {
        let config = CloudConfig::load_from_file(&app_handle)?;
        let auth = DeviceAuth::new(config.clone());
        let security = CloudSecurity::new(config.clone(), auth.clone());
        let command_processor = CloudCommandProcessor::new(app_handle.clone(), security.clone());

        let (command_tx, command_rx) = mpsc::unbounded_channel();

        Ok(Self {
            config,
            auth,
            security,
            command_processor,
            app_handle,
            connection_id: Arc::new(TokioMutex::new(None)),
            connection_state: Arc::new(TokioMutex::new(ConnectorState::Disconnected)),
            pending_commands: Arc::new(TokioMutex::new(HashMap::new())),
            command_tx,
            command_rx: Arc::new(TokioMutex::new(command_rx)),
        })
    }

    /// Start the production connector
    pub async fn start(&self) -> Result<(), CloudError> {
        if !self.config.enabled {
            info!("Production cloud connector is disabled");
            return Ok(());
        }

        info!("Starting production cloud connector...");

        // Validate configuration
        self.config.validate()?;

        // Initialize WebSocket plugin
        self.initialize_websocket_plugin().await?;

        // Start main connector loop
        let connector = self.clone();
        tokio::spawn(async move {
            connector.run_connector_loop().await;
        });

        // Start heartbeat task
        let heartbeat_connector = self.clone();
        tokio::spawn(async move {
            heartbeat_connector.run_heartbeat_loop().await;
        });

        // Start status reporting task
        let status_connector = self.clone();
        tokio::spawn(async move {
            status_connector.run_status_loop().await;
        });

        info!("Production cloud connector started successfully");
        Ok(())
    }

    /// Initialize the official Tauri WebSocket plugin
    async fn initialize_websocket_plugin(&self) -> Result<(), CloudError> {
        debug!("Initializing Tauri WebSocket plugin...");

        // The plugin will be initialized when we create the WebSocket connection
        // For now, we just validate that we can use it

        Ok(())
    }

    /// Main connector loop
    async fn run_connector_loop(&self) {
        let mut retry_count = 0u32;
        let max_retries = 10;
        let base_delay = Duration::from_secs(2);

        loop {
            // Check if we should connect
            if self.should_connect().await {
                self.set_connection_state(ConnectorState::Connecting).await;

                match self.establish_connection().await {
                    Ok(()) => {
                        retry_count = 0;
                        self.set_connection_state(ConnectorState::Ready).await;
                        info!("Production cloud connector established and ready");

                        // Run connection until it fails
                        if let Err(e) = self.run_connection().await {
                            error!("Connection failed: {}", e);
                            self.set_connection_state(ConnectorState::Error(e.to_string())).await;
                        }
                    },
                    Err(e) => {
                        retry_count += 1;
                        error!("Failed to establish connection (attempt {}): {}", retry_count, e);

                        if retry_count >= max_retries {
                            self.set_connection_state(ConnectorState::Error(format!("Max retries exceeded: {}", e))).await;
                            break;
                        }

                        self.set_connection_state(ConnectorState::Reconnecting(retry_count)).await;

                        // Exponential backoff
                        let delay = base_delay * 2_u32.pow(retry_count.min(5));
                        info!("Retrying connection in {:?}", delay);
                        tokio::time::sleep(delay).await;
                    }
                }
            } else {
                // Wait before checking again
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
        }
    }

    /// Check if we should attempt to connect
    async fn should_connect(&self) -> bool {
        let state = self.connection_state.lock().await;
        matches!(*state, ConnectorState::Disconnected | ConnectorState::Reconnecting(_))
    }

    /// Establish WebSocket connection using Tauri plugin
    async fn establish_connection(&self) -> Result<(), CloudError> {
        info!("Establishing WebSocket connection to: {}", self.config.server_url);

        // Create connection ID
        let connection_id = Uuid::new_v4().to_string();
        *self.connection_id.lock().await = Some(connection_id.clone());

        // Using the Tauri WebSocket plugin
        let websocket_code = format!(r#"
            import WebSocket from '@tauri-apps/plugin-websocket';

            const ws = await WebSocket.connect('{}');

            ws.addListener((msg) => {{
                window.__TAURI__.invoke('handle_cloud_message', {{
                    connectionId: '{}',
                    message: msg
                }});
            }});

            // Store websocket reference globally for sending
            window.__JUNO_CLOUD_WS = ws;
        "#, self.config.server_url, connection_id);

        // Emit WebSocket connection event instead of using eval
        if let Err(e) = self.app_handle.emit("websocket-connect", &websocket_code) {
            error!("Failed to emit websocket-connect event: {}", e);
        }

        self.set_connection_state(ConnectorState::Connected).await;

        // Authenticate
        self.authenticate().await?;

        Ok(())
    }

    /// Authenticate with the cloud server
    async fn authenticate(&self) -> Result<(), CloudError> {
        info!("Authenticating with cloud server");

        let auth_data = self.auth.create_auth_message()?;
        let auth_message = WebSocketMessage {
            message_type: MessageType::Auth,
            data: auth_data,
            timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
        };

        self.send_websocket_message(auth_message).await?;
        self.set_connection_state(ConnectorState::Authenticated).await;

        info!("Authentication completed");
        Ok(())
    }

    /// Send message via WebSocket
    async fn send_websocket_message(&self, message: WebSocketMessage) -> Result<(), CloudError> {
        let message_json = serde_json::to_string(&message)?;

        let send_code = format!(r#"
            if (window.__JUNO_CLOUD_WS) {{
                await window.__JUNO_CLOUD_WS.send('{}');
            }} else {{
                throw new Error('WebSocket not connected');
            }}
        "#, message_json.replace('\'', "\\'"));

        // Emit message send event instead of using eval
        if let Err(e) = self.app_handle.emit("websocket-send", &send_code) {
            error!("Failed to emit websocket-send event: {}", e);
        }

        Ok(())
    }

    /// Run the active connection
    async fn run_connection(&self) -> Result<(), CloudError> {
        let mut command_rx = self.command_rx.lock().await;

        loop {
            tokio::select! {
                // Handle internal connector messages
                msg = command_rx.recv() => {
                    match msg {
                        Some(ConnectorMessage::SendCommand(command)) => {
                            if let Err(e) = self.handle_send_command(command).await {
                                error!("Failed to send command: {}", e);
                            }
                        },
                        Some(ConnectorMessage::ProcessResponse(response)) => {
                            self.handle_command_response(response).await;
                        },
                        Some(ConnectorMessage::HandleError(error)) => {
                            error!("Connector error: {}", error);
                            return Err(CloudError::NetworkError(error));
                        },
                        Some(ConnectorMessage::UpdateStatus) => {
                            if let Err(e) = self.send_status_update().await {
                                warn!("Failed to send status update: {}", e);
                            }
                        },
                        Some(ConnectorMessage::Disconnect) => {
                            info!("Received disconnect command");
                            break;
                        },
                        Some(ConnectorMessage::Connect) => {
                            // Already connected, ignore
                        },
                        None => {
                            error!("Command channel closed");
                            break;
                        }
                    }
                },

                // Connection timeout check
                _ = tokio::time::sleep(Duration::from_secs(60)) => {
                    // Check if connection is still alive
                    if !self.is_connection_healthy().await {
                        warn!("Connection health check failed");
                        return Err(CloudError::NetworkError("Connection timeout".to_string()));
                    }
                }
            }
        }

        Ok(())
    }

    /// Handle sending a command to the cloud
    async fn handle_send_command(&self, command: CloudCommand) -> Result<(), CloudError> {
        let command_message = WebSocketMessage {
            message_type: MessageType::Command,
            data: serde_json::to_value(command)?,
            timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
        };

        self.send_websocket_message(command_message).await
    }

    /// Handle command response from cloud
    async fn handle_command_response(&self, response: DeviceResponse) {
        let mut pending = self.pending_commands.lock().await;
        if let Some(sender) = pending.remove(&response.command_id) {
            if let Err(_) = sender.send(response) {
                warn!("Failed to deliver command response - receiver dropped");
            }
        } else {
            warn!("Received response for unknown command: {}", response.command_id);
        }
    }

    /// Send status update to cloud
    async fn send_status_update(&self) -> Result<(), CloudError> {
        let status = self.create_device_status().await?;
        let status_message = WebSocketMessage {
            message_type: MessageType::Status,
            data: serde_json::to_value(status)?,
            timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
        };

        self.send_websocket_message(status_message).await
    }

    /// Check if connection is healthy
    async fn is_connection_healthy(&self) -> bool {
        // For now, just check if we have a connection ID
        // In a full implementation, we would check WebSocket state
        self.connection_id.lock().await.is_some()
    }

    /// Heartbeat loop to maintain connection
    async fn run_heartbeat_loop(&self) {
        let mut interval = tokio::time::interval(Duration::from_secs(self.config.heartbeat_interval));

        loop {
            interval.tick().await;

            let state = self.connection_state.lock().await;
            if matches!(*state, ConnectorState::Ready | ConnectorState::Authenticated) {
                drop(state);

                let heartbeat = WebSocketMessage {
                    message_type: MessageType::Heartbeat,
                    data: serde_json::json!({
                        "timestamp": SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
                        "device_id": self.auth.get_credentials().map(|c| c.device_id.clone()).unwrap_or_default()
                    }),
                    timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
                };

                if let Err(e) = self.send_websocket_message(heartbeat).await {
                    error!("Failed to send heartbeat: {}", e);
                }
            }
        }
    }

    /// Status reporting loop
    async fn run_status_loop(&self) {
        let mut interval = tokio::time::interval(Duration::from_secs(30));

        loop {
            interval.tick().await;

            let state = self.connection_state.lock().await;
            if matches!(*state, ConnectorState::Ready) {
                drop(state);

                if let Err(_) = self.command_tx.send(ConnectorMessage::UpdateStatus) {
                    warn!("Failed to queue status update");
                }
            }
        }
    }

    /// Create device status for reporting
    async fn create_device_status(&self) -> Result<DeviceStatus, CloudError> {
        let app_state = self.app_handle.state::<crate::state::AppState>();

        let device_id = self.auth.get_credentials()
            .map(|c| c.device_id.clone())
            .unwrap_or_else(|| "unknown".to_string());

        let status = DeviceStatus {
            device_id,
            status: crate::cloud::types::DeviceState::Online,
            current_task: None, // TODO: Track current task from agent
            system_info: crate::cloud::types::SystemInfo {
                platform: std::env::consts::OS.to_string(),
                permissions: self.get_permission_status().await,
                agent_mode: format!("{:?}", crate::agent::providers::factory::BrainFactory::get_agent_mode()),
                version: env!("CARGO_PKG_VERSION").to_string(),
                capabilities: self.get_device_capabilities(),
                hardware_info: Some(self.get_hardware_info().await),
            },
            timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
        };

        Ok(status)
    }

    /// Get permission status
    async fn get_permission_status(&self) -> Vec<String> {
        let app_state = self.app_handle.state::<crate::state::AppState>();
        let mut permissions = Vec::new();

        if app_state.is_desktop_available() {
            permissions.push(permission_types::ACCESSIBILITY.to_string());
            permissions.push(permission_types::SCREEN_RECORDING.to_string());
        }

        let voice_enabled = {
            let always_listening = app_state.always_listening_active.lock().unwrap();
            *always_listening
        };

        if voice_enabled {
            permissions.push(permission_types::MICROPHONE.to_string());
        }

        permissions
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
            "anthropic_computer_use".to_string(),
        ]
    }

    /// Get hardware information
    async fn get_hardware_info(&self) -> crate::cloud::types::HardwareInfo {
        crate::cloud::types::HardwareInfo {
            cpu_usage: None, // TODO: Implement system monitoring
            memory_usage: None,
            disk_usage: None,
            screen_resolution: None,
        }
    }

    /// Set connection state and emit events
    async fn set_connection_state(&self, state: ConnectorState) {
        let mut current_state = self.connection_state.lock().await;
        *current_state = state.clone();

        // Emit event to frontend
        let state_str = match state {
            ConnectorState::Disconnected => "disconnected",
            ConnectorState::Connecting => "connecting",
            ConnectorState::Connected => "connected",
            ConnectorState::Authenticated => "authenticated",
            ConnectorState::Synchronizing => "synchronizing",
            ConnectorState::Ready => "ready",
            ConnectorState::Error(_) => "error",
            ConnectorState::Reconnecting(_) => "reconnecting",
        };

        if let Err(e) = self.app_handle.emit("cloud-connector-state", state_str) {
            error!("Failed to emit cloud connector state: {}", e);
        }

        info!("Cloud connector state changed to: {:?}", state);
    }

    /// Get current connection state
    pub async fn get_connection_state(&self) -> ConnectorState {
        self.connection_state.lock().await.clone()
    }

    /// Execute remote command (for use by cloud server)
    pub async fn execute_remote_command(&self, command: CloudCommand) -> Result<DeviceResponse, CloudError> {
        info!("Executing remote command: {} ({:?})", command.id, command.command_type);

        // Use the existing command processor
        self.command_processor.process_command(command).await
    }

    /// Disconnect from cloud
    pub async fn disconnect(&self) -> Result<(), CloudError> {
        info!("Disconnecting from cloud");

        // Send disconnect message
        if let Err(_) = self.command_tx.send(ConnectorMessage::Disconnect) {
            warn!("Failed to send disconnect message");
        }

        // Close WebSocket connection
        let disconnect_code = r#"
            if (window.__JUNO_CLOUD_WS) {
                await window.__JUNO_CLOUD_WS.disconnect();
                window.__JUNO_CLOUD_WS = null;
            }
        "#;

        // Emit disconnect event instead of using eval
        if let Err(e) = self.app_handle.emit("websocket-disconnect", disconnect_code) {
            error!("Failed to emit websocket-disconnect event: {}", e);
        }

        // Clear connection state
        *self.connection_id.lock().await = None;
        self.set_connection_state(ConnectorState::Disconnected).await;

        Ok(())
    }

    /// Get connection statistics
    pub async fn get_connection_stats(&self) -> ConnectionStats {
        ConnectionStats {
            connected_at: None, // TODO: Track connection time
            total_commands: 0,  // TODO: Track command metrics
            successful_commands: 0,
            failed_commands: 0,
            reconnection_count: 0,
            last_heartbeat: None,
            latency_ms: None,
        }
    }
}

impl Clone for ProductionCloudConnector {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            auth: self.auth.clone(),
            security: self.security.clone(),
            command_processor: self.command_processor.clone(),
            app_handle: self.app_handle.clone(),
            connection_id: self.connection_id.clone(),
            connection_state: self.connection_state.clone(),
            pending_commands: self.pending_commands.clone(),
            command_tx: self.command_tx.clone(),
            command_rx: self.command_rx.clone(),
        }
    }
}
