/// TODO: DO WE NEED?
use futures_util::{SinkExt, StreamExt};
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::{mpsc, Mutex as TokioMutex};
use tokio::time;
use tokio_tungstenite::tungstenite::{protocol::CloseFrame, Message};
use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};
use tracing::{debug, error, info, warn};
use url::Url;

use super::auth::DeviceAuth;
use super::commands::CloudCommandProcessor;
use super::config::CloudConfig;
use super::security::CloudSecurity;
use super::types::{
    AuthResponse, CloudCommand, CloudError, ConnectionState, DeviceResponse, DeviceState,
    DeviceStatus, HardwareInfo, MessageType, SystemInfo, WebSocketMessage,
};
use crate::constants::events;
use crate::constants::permissions;

type WsSender = futures_util::stream::SplitSink<
    WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>,
    Message,
>;
type CloudAuth = DeviceAuth;
type CommandProcessor = CloudCommandProcessor;

/// Cloud client for WebSocket communication
#[derive(Debug)]
pub struct CloudClient {
    config: CloudConfig,
    app_handle: AppHandle,
    connection_state: Arc<TokioMutex<ConnectionState>>,
    auth: CloudAuth,
    #[allow(dead_code)]
    security: CloudSecurity,
    command_processor: CommandProcessor,
    // Communication channels
    #[allow(dead_code)]
    command_tx: mpsc::UnboundedSender<CloudCommand>,
    #[allow(dead_code)]
    command_rx: Arc<TokioMutex<mpsc::UnboundedReceiver<CloudCommand>>>,
}

impl CloudClient {
    /// Create new cloud client
    pub async fn new(app_handle: AppHandle) -> Result<Self, CloudError> {
        let settings_manager = crate::settings::manager::SettingsManager::new(app_handle.clone())?;
        let config = CloudConfig::load_from_centralized_settings(&settings_manager).await?;
        let auth = CloudAuth::new(config.clone());
        let security = CloudSecurity::new(config.clone(), auth.clone());
        let command_processor = CommandProcessor::new(app_handle.clone(), security.clone());

        let (command_tx, command_rx) = mpsc::unbounded_channel();

        Ok(Self {
            config,
            connection_state: Arc::new(TokioMutex::new(ConnectionState::Disconnected)),
            app_handle,
            auth,
            security,
            command_processor,
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

        // For now, just log that cloud client would start
        // Full implementation can be added when cloud connectivity is actually needed
        debug!("Cloud client configured but connection loop not implemented yet");

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
    #[allow(dead_code)]
    async fn connection_loop(&self) {
        let mut retry_interval = Duration::from_secs(self.config.reconnect_interval);

        loop {
            match self.connect_and_run().await {
                Ok(()) => {
                    info!("Cloud connection ended normally");
                    break;
                }
                Err(e) => {
                    error!("Cloud connection error: {}", e);
                    self.set_connection_state(ConnectionState::Error(e.to_string()))
                        .await;

                    // Exponential backoff with max limit
                    info!("Retrying connection in {:?}", retry_interval);
                    time::sleep(retry_interval).await;
                    retry_interval = std::cmp::min(
                        retry_interval * 2,
                        Duration::from_secs(
                            crate::constants::agent::config::DEFAULT_TASK_TIMEOUT_SECONDS,
                        ),
                    );
                }
            }
        }
    }

    #[allow(dead_code)]
    async fn connect_and_run(&self) -> Result<(), CloudError> {
        self.set_connection_state(ConnectionState::Connecting).await;

        info!("Connecting to cloud server: {}", self.config.server_url);

        // Parse WebSocket URL
        let url = Url::parse(&self.config.server_url)
            .map_err(|e| CloudError::ConfigError(format!("Invalid server URL: {}", e)))?;

        // Connect to WebSocket
        let (ws_stream, _) = connect_async(&url).await.map_err(|e| {
            CloudError::ConnectionFailed(format!("WebSocket connection failed: {}", e))
        })?;

        info!("WebSocket connected successfully");
        self.set_connection_state(ConnectionState::Connected).await;

        // Run the WebSocket handler
        self.handle_websocket(ws_stream).await
    }

    #[allow(dead_code)]
    async fn handle_websocket(
        &self,
        ws_stream: WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>,
    ) -> Result<(), CloudError> {
        let (ws_sender, mut ws_receiver) = ws_stream.split();
        let ws_sender = Arc::new(TokioMutex::new(ws_sender));

        // Authenticate first
        {
            let mut sender_guard = ws_sender.lock().await;
            self.authenticate(&mut sender_guard).await?;
        }

        // Start heartbeat task
        let heartbeat_handle = {
            let sender = ws_sender.clone();
            tokio::spawn(async move {
                let mut heartbeat_timer = time::interval(Duration::from_secs(
                    crate::constants::timeouts::CLOUD_HEARTBEAT_INTERVAL_SECONDS,
                ));
                loop {
                    heartbeat_timer.tick().await;

                    let heartbeat = WebSocketMessage {
                        message_type: MessageType::Heartbeat,
                        data: serde_json::json!({
                            "timestamp": crate::utils::current_timestamp_secs()
                        }),
                        timestamp: crate::utils::current_timestamp_secs(),
                    };

                    if let Ok(message_json) = serde_json::to_string(&heartbeat) {
                        let mut sender_guard = sender.lock().await;
                        if sender_guard
                            .send(Message::Text(message_json))
                            .await
                            .is_err()
                        {
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
                let mut status_timer = time::interval(Duration::from_secs(
                    crate::constants::timeouts::CLOUD_STATUS_INTERVAL_SECONDS,
                ));
                loop {
                    status_timer.tick().await;

                    if let Ok(status) = client.create_device_status().await {
                        let status_message = WebSocketMessage {
                            message_type: MessageType::Status,
                            data: serde_json::to_value(status).unwrap_or_default(),
                            timestamp: crate::utils::current_timestamp_secs(),
                        };

                        if let Ok(message_json) = serde_json::to_string(&status_message) {
                            let mut sender_guard = sender.lock().await;
                            if sender_guard
                                .send(Message::Text(message_json))
                                .await
                                .is_err()
                            {
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
        let _ = ws_sender
            .lock()
            .await
            .send(Message::Close(Some(CloseFrame {
                code: tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode::Normal,
                reason: "Client shutdown".into(),
            })))
            .await;

        Ok(())
    }

    #[allow(dead_code)]
    async fn authenticate(&self, ws_sender: &mut WsSender) -> Result<(), CloudError> {
        info!("Authenticating with cloud server");

        let auth_data = self.auth.create_auth_message()?;
        let auth_message = WebSocketMessage {
            message_type: MessageType::Auth,
            data: auth_data,
            timestamp: crate::utils::current_timestamp_secs(),
        };

        let message_json = serde_json::to_string(&auth_message)?;
        ws_sender
            .send(Message::Text(message_json))
            .await
            .map_err(|e| CloudError::NetworkError(format!("Failed to send auth message: {}", e)))?;

        // Note: Auth response validation not implemented yet
        // For now, assume authentication succeeds
        self.set_connection_state(ConnectionState::Authenticated)
            .await;
        info!("Authentication completed");

        Ok(())
    }

    #[allow(dead_code)]
    async fn handle_message(
        &self,
        text: String,
        ws_sender: Arc<TokioMutex<WsSender>>,
    ) -> Result<(), CloudError> {
        debug!("Received message: {}", text);

        let message: WebSocketMessage = serde_json::from_str(&text)?;

        match message.message_type {
            MessageType::Command => {
                let command: CloudCommand = serde_json::from_value(message.data)?;
                self.handle_command(command, ws_sender.clone()).await?;
            }
            MessageType::Auth => {
                let auth_response: AuthResponse = serde_json::from_value(message.data)?;
                self.handle_auth_response(auth_response).await?;
            }
            MessageType::Heartbeat => {
                // Respond to heartbeat
                let response = WebSocketMessage {
                    message_type: MessageType::Heartbeat,
                    data: serde_json::json!({
                        "timestamp": crate::utils::current_timestamp_secs(),
                        "response": true
                    }),
                    timestamp: crate::utils::current_timestamp_secs(),
                };

                let response_json = serde_json::to_string(&response)?;
                ws_sender
                    .lock()
                    .await
                    .send(Message::Text(response_json))
                    .await
                    .map_err(|e| {
                        CloudError::NetworkError(format!(
                            "Failed to send heartbeat response: {}",
                            e
                        ))
                    })?;
            }
            MessageType::Error => {
                let error_msg = message
                    .data
                    .get("message")
                    .and_then(|m| m.as_str())
                    .unwrap_or("Unknown error");
                error!("Received error from server: {}", error_msg);
            }
            _ => {
                debug!("Unhandled message type: {:?}", message.message_type);
            }
        }

        Ok(())
    }

    #[allow(dead_code)]
    async fn handle_command(
        &self,
        command: CloudCommand,
        ws_sender: Arc<TokioMutex<WsSender>>,
    ) -> Result<(), CloudError> {
        info!("Processing command from cloud: {}", command.id);

        // Process the command
        let response = self.command_processor.process_command(command).await?;

        // Send response back to cloud
        let response_message = WebSocketMessage {
            message_type: MessageType::Response,
            data: serde_json::to_value(response)?,
            timestamp: crate::utils::current_timestamp_secs(),
        };

        let response_json = serde_json::to_string(&response_message)?;
        ws_sender
            .lock()
            .await
            .send(Message::Text(response_json))
            .await
            .map_err(|e| CloudError::NetworkError(format!("Failed to send response: {}", e)))?;

        Ok(())
    }

    #[allow(dead_code)]
    async fn handle_auth_response(&self, response: AuthResponse) -> Result<(), CloudError> {
        info!(
            "Received authentication response: success={}",
            response.success
        );

        if response.success {
            self.set_connection_state(ConnectionState::Authenticated)
                .await;
            // Note: Auth credential storage not implemented yet
        } else {
            let error_msg = response
                .error
                .unwrap_or_else(|| "Authentication failed".to_string());
            error!("Authentication failed: {}", error_msg);
            return Err(CloudError::AuthenticationFailed(error_msg));
        }

        Ok(())
    }

    #[allow(dead_code)]
    async fn create_device_status(&self) -> Result<DeviceStatus, CloudError> {
        let _app_state = self.app_handle.state::<crate::state::AppState>();

        let device_id = self
            .auth
            .get_credentials()
            .map(|c| c.device_id.clone())
            .unwrap_or_else(|| "unknown".to_string());

        let status = DeviceStatus {
            device_id,
            status: DeviceState::Online,
            current_task: None, // TODO: Track current task
            system_info: SystemInfo {
                platform: std::env::consts::OS.to_string(),
                permissions: self.get_permission_status().await,
                agent_mode: format!(
                    "{:?}",
                    crate::agent::providers::factory::BrainFactory::get_agent_mode()
                ),
                version: env!("CARGO_PKG_VERSION").to_string(),
                capabilities: self.get_device_capabilities(),
                hardware_info: Some(self.get_hardware_info().await),
            },
            timestamp: crate::utils::current_timestamp_secs(),
        };

        Ok(status)
    }

    #[allow(dead_code)]
    async fn get_permission_status(&self) -> Vec<String> {
        let app_state = self.app_handle.state::<crate::state::AppState>();
        let mut permissions = Vec::new();

        if app_state.is_desktop_available() {
            permissions.push(permissions::types::ACCESSIBILITY.to_string());
            permissions.push(permissions::types::SCREEN_RECORDING.to_string());
        }

        let voice_enabled = app_state.get_always_listening_active().unwrap_or(false);

        if voice_enabled {
            permissions.push(permissions::types::MICROPHONE.to_string());
        }

        permissions
    }

    #[allow(dead_code)]
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

    #[allow(dead_code)]
    async fn get_hardware_info(&self) -> HardwareInfo {
        // Use the same comprehensive hardware monitoring as the connector
        log::info!("🔍 Collecting real-time hardware information...");
        let start_time = std::time::Instant::now();

        let hardware_info = self.get_comprehensive_hardware_info().await;
        let collection_time = start_time.elapsed();

        log::info!(
            "✅ Hardware information collected in {:?} - CPU: {:?}%, Memory: {:?}%, Disk: {:?}%",
            collection_time,
            hardware_info.cpu_usage,
            hardware_info.memory_usage,
            hardware_info.disk_usage
        );

        hardware_info
    }

    #[allow(dead_code)]
    async fn get_comprehensive_hardware_info(&self) -> HardwareInfo {
        log::debug!("🔍 Gathering comprehensive hardware information...");

        let (cpu_usage, memory_usage, disk_usage, screen_resolution) = tokio::join!(
            Self::get_cpu_usage(),
            Self::get_memory_usage(),
            Self::get_disk_usage(),
            Self::get_screen_resolution()
        );

        log::debug!(
            "📊 Hardware metrics - CPU: {:?}%, Memory: {:?}%, Disk: {:?}%, Screen: {:?}",
            cpu_usage,
            memory_usage,
            disk_usage,
            screen_resolution
        );

        HardwareInfo {
            cpu_usage,
            memory_usage,
            disk_usage,
            screen_resolution,
        }
    }

    #[allow(dead_code)]
    async fn get_cpu_usage() -> Option<f32> {
        #[cfg(target_os = "macos")]
        {
            use std::process::Command;

            match Command::new("top").args(["-l", "1", "-n", "0"]).output() {
                Ok(output) => {
                    let output_str = String::from_utf8_lossy(&output.stdout);
                    Self::parse_cpu_usage(&output_str)
                }
                Err(e) => {
                    log::warn!("Failed to get CPU usage: {}", e);
                    None
                }
            }
        }

        #[cfg(not(target_os = "macos"))]
        {
            log::debug!("CPU monitoring not implemented for this platform");
            None
        }
    }

    #[allow(dead_code)]
    fn parse_cpu_usage(output: &str) -> Option<f32> {
        use regex::Regex;

        // Example: "CPU usage: 15.38% user, 8.46% sys, 76.15% idle"
        let cpu_regex =
            Regex::new(r"CPU usage:\s*(\d+\.?\d*)%\s*user,\s*(\d+\.?\d*)%\s*sys").ok()?;

        for line in output.lines() {
            if let Some(captures) = cpu_regex.captures(line) {
                let user_cpu = captures.get(1)?.as_str().parse::<f32>().ok()?;
                let sys_cpu = captures.get(2)?.as_str().parse::<f32>().ok()?;

                let total_cpu = user_cpu + sys_cpu;
                log::debug!(
                    "Parsed CPU usage: {}% user + {}% sys = {}% total",
                    user_cpu,
                    sys_cpu,
                    total_cpu
                );

                return Some(total_cpu);
            }
        }

        log::warn!("Could not parse CPU usage from top output");
        None
    }

    #[allow(dead_code)]
    async fn get_memory_usage() -> Option<f32> {
        #[cfg(target_os = "macos")]
        {
            use std::process::Command;

            match Command::new("vm_stat").output() {
                Ok(output) => {
                    let output_str = String::from_utf8_lossy(&output.stdout);
                    let mut free_pages = 0u64;
                    let mut active_pages = 0u64;
                    let mut inactive_pages = 0u64;
                    let mut speculative_pages = 0u64;
                    let mut wired_pages = 0u64;

                    for line in output_str.lines() {
                        if line.contains("Pages free:") {
                            if let Some(num_str) = line.split(':').nth(1) {
                                free_pages =
                                    num_str.trim().trim_end_matches('.').parse().unwrap_or(0);
                            }
                        } else if line.contains("Pages active:") {
                            if let Some(num_str) = line.split(':').nth(1) {
                                active_pages =
                                    num_str.trim().trim_end_matches('.').parse().unwrap_or(0);
                            }
                        } else if line.contains("Pages inactive:") {
                            if let Some(num_str) = line.split(':').nth(1) {
                                inactive_pages =
                                    num_str.trim().trim_end_matches('.').parse().unwrap_or(0);
                            }
                        } else if line.contains("Pages speculative:") {
                            if let Some(num_str) = line.split(':').nth(1) {
                                speculative_pages =
                                    num_str.trim().trim_end_matches('.').parse().unwrap_or(0);
                            }
                        } else if line.contains("Pages wired down:") {
                            if let Some(num_str) = line.split(':').nth(1) {
                                wired_pages =
                                    num_str.trim().trim_end_matches('.').parse().unwrap_or(0);
                            }
                        }
                    }

                    let total_pages = free_pages
                        + active_pages
                        + inactive_pages
                        + speculative_pages
                        + wired_pages;
                    let used_pages = total_pages - free_pages;

                    if total_pages > 0 {
                        let usage_percentage = (used_pages as f32 / total_pages as f32) * 100.0;
                        Some(usage_percentage)
                    } else {
                        None
                    }
                }
                Err(e) => {
                    log::warn!("Failed to get memory usage: {}", e);
                    None
                }
            }
        }

        #[cfg(not(target_os = "macos"))]
        {
            log::debug!("Memory monitoring not implemented for this platform");
            None
        }
    }

    #[allow(dead_code)]
    async fn get_disk_usage() -> Option<f32> {
        #[cfg(target_os = "macos")]
        {
            use std::process::Command;

            match Command::new("df").args(["-h", "/"]).output() {
                Ok(output) => {
                    let output_str = String::from_utf8_lossy(&output.stdout);
                    if let Some(line) = output_str.lines().nth(1) {
                        // Skip header, only process first (root) filesystem
                        let parts: Vec<&str> = line.split_whitespace().collect();
                        if parts.len() >= 5 {
                            // Format: Filesystem Size Used Avail Capacity Mounted
                            if let Some(capacity_str) = parts.get(4) {
                                if let Some(percentage_str) = capacity_str.strip_suffix('%') {
                                    if let Ok(percentage) = percentage_str.parse::<f32>() {
                                        return Some(percentage);
                                    }
                                }
                            }
                        }
                    }
                    None
                }
                Err(e) => {
                    log::warn!("Failed to get disk usage: {}", e);
                    None
                }
            }
        }

        #[cfg(not(target_os = "macos"))]
        {
            log::debug!("Disk monitoring not implemented for this platform");
            None
        }
    }

    #[allow(dead_code)]
    async fn get_screen_resolution() -> Option<String> {
        #[cfg(target_os = "macos")]
        {
            use std::process::Command;

            match Command::new("system_profiler")
                .args(["SPDisplaysDataType"])
                .output()
            {
                Ok(output) => {
                    let output_str = String::from_utf8_lossy(&output.stdout);
                    for line in output_str.lines() {
                        if line.trim().starts_with("Resolution:") {
                            if let Some(resolution) = line.split(':').nth(1) {
                                return Some(resolution.trim().to_string());
                            }
                        }
                    }
                    None
                }
                Err(e) => {
                    log::warn!("Failed to get screen resolution: {}", e);
                    None
                }
            }
        }

        #[cfg(not(target_os = "macos"))]
        {
            log::debug!("Screen resolution monitoring not implemented for this platform");
            None
        }
    }

    #[allow(dead_code)]
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

        if let Err(e) = self
            .app_handle
            .emit(events::cloud::CONNECTION_STATE, state_str)
        {
            error!("Failed to emit cloud connection state: {}", e);
        }
    }

    /// Clone for use in async tasks
    fn clone_for_task(&self) -> CloudClientTask {
        CloudClientTask {
            config: self.config.clone(),
            auth: self.auth.clone(),
            command_processor: self.command_processor.clone(),
            connection_state: self.connection_state.clone(),
            app_handle: self.app_handle.clone(),
        }
    }
}

/// Simplified version of CloudClient for async tasks
#[derive(Debug, Clone)]
struct CloudClientTask {
    #[allow(dead_code)]
    config: CloudConfig,
    #[allow(dead_code)]
    auth: DeviceAuth,
    #[allow(dead_code)]
    command_processor: CloudCommandProcessor,
    #[allow(dead_code)]
    connection_state: Arc<TokioMutex<ConnectionState>>,
    #[allow(dead_code)]
    app_handle: AppHandle,
}

impl CloudClientTask {
    #[allow(dead_code)]
    async fn connection_loop(&self) {
        // Implementation would be the same as CloudClient::connection_loop
        // This is a placeholder to show the pattern
    }

    #[allow(dead_code)]
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
            timestamp: crate::utils::current_timestamp_secs(),
        })
    }
}
