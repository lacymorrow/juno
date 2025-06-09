use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::{Mutex as TokioMutex, mpsc, oneshot, broadcast};
use tokio::time;
use tokio_tungstenite::{connect_async, WebSocketStream, MaybeTlsStream};
use tokio_tungstenite::tungstenite::{Message, protocol::CloseFrame};
use futures_util::{SinkExt, StreamExt};
use url::Url;
use tracing::{info, warn, error, debug};
use tauri::{AppHandle, Manager, Emitter};

use super::types::{
    CloudError, CloudCommand, DeviceResponse, DeviceStatus, AuthResponse,
    WebSocketMessage, MessageType, ConnectionState, HardwareInfo, DeviceState, SystemInfo
};
use super::config::CloudConfig;
use super::auth::DeviceAuth;
use super::security::CloudSecurity;
use super::commands::CloudCommandProcessor;

// Type alias for WebSocket sender to simplify function signatures
type WsSender = futures_util::stream::SplitSink<WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>, Message>;

/// Cloud client for WebSocket communication
#[derive(Debug)]
pub struct CloudClient {
    config: CloudConfig,
    auth: DeviceAuth,
    security: CloudSecurity,
    command_processor: CloudCommandProcessor,
    connection_state: Arc<TokioMutex<ConnectionState>>,
    app_handle: AppHandle,

    // Communication channels
    command_tx: mpsc::UnboundedSender<CloudCommand>,
    command_rx: Arc<TokioMutex<mpsc::UnboundedReceiver<CloudCommand>>>,
}

impl CloudClient {
    /// Create new cloud client
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
            connection_state: Arc::new(TokioMutex::new(ConnectionState::Disconnected)),
            app_handle,
            command_tx,
            command_rx: Arc::new(TokioMutex::new(command_rx)),
        })
    }

    /// Start the cloud client
    pub async fn start(&mut self) -> Result<(), CloudError> {
        if !self.config.enabled {
            info!("Cloud connectivity is disabled");
            return Ok(());
        }

        info!("Starting cloud client...");

        // Validate configuration
        self.config.validate()?;

        // Start connection loop
        let client = self.clone_for_task();
        tokio::spawn(async move {
            client.connection_loop().await;
        });

        Ok(())
    }

    /// Get current connection state
    pub async fn get_connection_state(&self) -> ConnectionState {
        self.connection_state.lock().await.clone()
    }

    /// Send response back to cloud
    pub async fn send_response(&self, response: DeviceResponse) -> Result<(), CloudError> {
        // For now, we'll implement this when we have the WebSocket connection
        // This would send the response through the WebSocket
        debug!("Would send response: {:?}", response);
        Ok(())
    }

    /// Main connection loop
    async fn connection_loop(&self) {
        let mut retry_interval = Duration::from_secs(self.config.reconnect_interval);

        loop {
            match self.connect_and_run().await {
                Ok(()) => {
                    info!("Cloud connection ended normally");
                    break;
                },
                Err(e) => {
                    error!("Cloud connection error: {}", e);
                    self.set_connection_state(ConnectionState::Error(e.to_string())).await;

                    // Exponential backoff with max limit
                    info!("Retrying connection in {:?}", retry_interval);
                    time::sleep(retry_interval).await;
                    retry_interval = std::cmp::min(retry_interval * 2, Duration::from_secs(300));
                }
            }
        }
    }

    /// Connect to cloud and run main loop
    async fn connect_and_run(&self) -> Result<(), CloudError> {
        self.set_connection_state(ConnectionState::Connecting).await;

        info!("Connecting to cloud server: {}", self.config.server_url);

        // Parse WebSocket URL
        let url = Url::parse(&self.config.server_url)
            .map_err(|e| CloudError::ConfigError(format!("Invalid server URL: {}", e)))?;

        // Connect to WebSocket
        let (ws_stream, _) = connect_async(&url).await
            .map_err(|e| CloudError::ConnectionFailed(format!("WebSocket connection failed: {}", e)))?;

        info!("WebSocket connected successfully");
        self.set_connection_state(ConnectionState::Connected).await;

        // Run the WebSocket handler
        self.handle_websocket(ws_stream).await
    }

    /// Handle WebSocket communication
    async fn handle_websocket(&self, ws_stream: WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>) -> Result<(), CloudError> {
        let (mut ws_sender, mut ws_receiver) = ws_stream.split();
        let ws_sender = Arc::new(TokioMutex::new(ws_sender));

        // Authenticate first
        {
            let mut sender_guard = ws_sender.lock().await;
            self.authenticate(&mut *sender_guard).await?;
        }

        // Start heartbeat task
        let heartbeat_handle = {
            let sender = ws_sender.clone();
            tokio::spawn(async move {
                let mut heartbeat_timer = time::interval(Duration::from_secs(30));
                loop {
                    heartbeat_timer.tick().await;

                    let heartbeat = WebSocketMessage {
                        message_type: MessageType::Heartbeat,
                        data: serde_json::json!({
                            "timestamp": SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()
                        }),
                        timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
                    };

                    if let Ok(message_json) = serde_json::to_string(&heartbeat) {
                        let mut sender_guard = sender.lock().await;
                        if sender_guard.send(Message::Text(message_json)).await.is_err() {
                            break;
                        }
                    }
                }
            })
        };

        // Start status reporting task
        let status_handle = {
            let sender = ws_sender.clone();
            let client = self.clone_for_task();
            tokio::spawn(async move {
                let mut status_timer = time::interval(Duration::from_secs(30));
                loop {
                    status_timer.tick().await;

                    if let Ok(status) = client.create_device_status().await {
                        let status_message = WebSocketMessage {
                            message_type: MessageType::Status,
                            data: serde_json::to_value(status).unwrap_or_default(),
                            timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
                        };

                        if let Ok(message_json) = serde_json::to_string(&status_message) {
                            let mut sender_guard = sender.lock().await;
                            if sender_guard.send(Message::Text(message_json)).await.is_err() {
                                break;
                            }
                        }
                    }
                }
            })
        };

        // Main message handling loop
        loop {
            tokio::select! {
                // Handle incoming WebSocket messages
                msg = ws_receiver.next() => {
                    match msg {
                        Some(Ok(Message::Text(text))) => {
                            let sender_clone = ws_sender.clone();
                            if let Err(e) = self.handle_message(text, sender_clone).await {
                                error!("Error handling message: {}", e);
                            }
                        },
                        Some(Ok(Message::Close(_))) => {
                            info!("WebSocket closed by server");
                            break;
                        },
                        Some(Err(e)) => {
                            error!("WebSocket error: {}", e);
                            break;
                        },
                        None => {
                            info!("WebSocket stream ended");
                            break;
                        },
                        _ => {
                            // Handle other message types (binary, ping, pong)
                        }
                    }
                },

                // Handle timeout for connection health
                _ = time::sleep(Duration::from_secs(self.config.heartbeat_interval * 3)) => {
                    warn!("No heartbeat response, connection may be dead");
                    break;
                }
            }
        }

        // Cleanup
        heartbeat_handle.abort();
        status_handle.abort();

        // Send close frame
        let _ = ws_sender.lock().await.send(Message::Close(Some(CloseFrame {
            code: tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode::Normal,
            reason: "Client shutdown".into(),
        }))).await;

        Ok(())
    }

    /// Authenticate with the cloud server
    async fn authenticate(&self, ws_sender: &mut WsSender) -> Result<(), CloudError> {
        info!("Authenticating with cloud server");

        let auth_data = self.auth.create_auth_message()?;
        let auth_message = WebSocketMessage {
            message_type: MessageType::Auth,
            data: auth_data,
            timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
        };

        let message_json = serde_json::to_string(&auth_message)?;
        ws_sender.send(Message::Text(message_json)).await
            .map_err(|e| CloudError::NetworkError(format!("Failed to send auth message: {}", e)))?;

        // TODO: Wait for auth response and validate
        // For now, assume authentication succeeds
        self.set_connection_state(ConnectionState::Authenticated).await;
        info!("Authentication completed");

        Ok(())
    }

    /// Handle incoming WebSocket message
    async fn handle_message(&self, text: String, ws_sender: Arc<TokioMutex<WsSender>>) -> Result<(), CloudError> {
        debug!("Received message: {}", text);

        let message: WebSocketMessage = serde_json::from_str(&text)?;

        match message.message_type {
            MessageType::Command => {
                let command: CloudCommand = serde_json::from_value(message.data)?;
                self.handle_command(command, ws_sender.clone()).await?;
            },
            MessageType::Auth => {
                let auth_response: AuthResponse = serde_json::from_value(message.data)?;
                self.handle_auth_response(auth_response).await?;
            },
            MessageType::Heartbeat => {
                // Respond to heartbeat
                let response = WebSocketMessage {
                    message_type: MessageType::Heartbeat,
                    data: serde_json::json!({
                        "timestamp": SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
                        "response": true
                    }),
                    timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
                };

                let response_json = serde_json::to_string(&response)?;
                ws_sender.lock().await.send(Message::Text(response_json)).await
                    .map_err(|e| CloudError::NetworkError(format!("Failed to send heartbeat response: {}", e)))?;
            },
            MessageType::Error => {
                let error_msg = message.data.get("message")
                    .and_then(|m| m.as_str())
                    .unwrap_or("Unknown error");
                error!("Received error from server: {}", error_msg);
            },
            _ => {
                debug!("Unhandled message type: {:?}", message.message_type);
            }
        }

        Ok(())
    }

    /// Handle incoming command from cloud
    async fn handle_command(&self, command: CloudCommand, ws_sender: Arc<TokioMutex<WsSender>>) -> Result<(), CloudError> {
        info!("Processing command from cloud: {}", command.id);

        // Process the command
        let response = self.command_processor.process_command(command).await?;

        // Send response back to cloud
        let response_message = WebSocketMessage {
            message_type: MessageType::Response,
            data: serde_json::to_value(response)?,
            timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
        };

        let response_json = serde_json::to_string(&response_message)?;
        ws_sender.lock().await.send(Message::Text(response_json)).await
            .map_err(|e| CloudError::NetworkError(format!("Failed to send response: {}", e)))?;

        Ok(())
    }

    /// Handle authentication response
    async fn handle_auth_response(&self, response: AuthResponse) -> Result<(), CloudError> {
        info!("Received authentication response: success={}", response.success);

        if response.success {
            self.set_connection_state(ConnectionState::Authenticated).await;
            // TODO: Store auth credentials
        } else {
            let error_msg = response.error.unwrap_or_else(|| "Authentication failed".to_string());
            error!("Authentication failed: {}", error_msg);
            return Err(CloudError::AuthenticationFailed(error_msg));
        }

        Ok(())
    }

    /// Create device status report
    async fn create_device_status(&self) -> Result<DeviceStatus, CloudError> {
        let app_state = self.app_handle.state::<crate::state::AppState>();

        let device_id = self.auth.get_credentials()
            .map(|c| c.device_id.clone())
            .unwrap_or_else(|| "unknown".to_string());

        let status = DeviceStatus {
            device_id,
            status: DeviceState::Online,
            current_task: None, // TODO: Track current task
            system_info: SystemInfo {
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
            permissions.push("accessibility".to_string());
            permissions.push("screen_recording".to_string());
        }

        // TODO: Check microphone permission
        permissions.push("microphone".to_string());

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
        ]
    }

    /// Get hardware information
    async fn get_hardware_info(&self) -> HardwareInfo {
        HardwareInfo {
            cpu_usage: None, // TODO: Implement CPU usage monitoring
            memory_usage: None, // TODO: Implement memory usage monitoring
            disk_usage: None, // TODO: Implement disk usage monitoring
            screen_resolution: None, // TODO: Get screen resolution
        }
    }

    /// Set connection state
    async fn set_connection_state(&self, state: ConnectionState) {
        let mut current_state = self.connection_state.lock().await;
        *current_state = state.clone();

        // Emit event to frontend
        let state_str = match state {
            ConnectionState::Disconnected => "disconnected",
            ConnectionState::Connecting => "connecting",
            ConnectionState::Connected => "connected",
            ConnectionState::Authenticated => "authenticated",
            ConnectionState::Reconnecting => "reconnecting",
            ConnectionState::Failed(_) => "failed",
            ConnectionState::Error(_) => "error",
        };

        if let Err(e) = self.app_handle.emit("cloud-connection-state", state_str) {
            error!("Failed to emit cloud connection state: {}", e);
        }
    }

    /// Clone for use in async tasks
    fn clone_for_task(&self) -> CloudClientTask {
        CloudClientTask {
            config: self.config.clone(),
            auth: self.auth.clone(),
            security: self.security.clone(),
            command_processor: self.command_processor.clone(),
            connection_state: self.connection_state.clone(),
            app_handle: self.app_handle.clone(),
        }
    }
}

/// Simplified version of CloudClient for async tasks
#[derive(Debug, Clone)]
struct CloudClientTask {
    config: CloudConfig,
    auth: DeviceAuth,
    security: CloudSecurity,
    command_processor: CloudCommandProcessor,
    connection_state: Arc<TokioMutex<ConnectionState>>,
    app_handle: AppHandle,
}

impl CloudClientTask {
    async fn connection_loop(&self) {
        // Implementation would be the same as CloudClient::connection_loop
        // This is a placeholder to show the pattern
    }

    async fn create_device_status(&self) -> Result<DeviceStatus, CloudError> {
        // Implementation would be the same as CloudClient::create_device_status
        // This is a placeholder
        Ok(DeviceStatus {
            device_id: "unknown".to_string(),
            status: DeviceState::Online,
            current_task: None,
            system_info: SystemInfo {
                platform: std::env::consts::OS.to_string(),
                permissions: vec![],
                agent_mode: "unknown".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
                capabilities: vec![],
                hardware_info: None,
            },
            timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
        })
    }
}
