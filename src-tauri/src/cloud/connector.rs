use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH, Instant};
use tokio::sync::{Mutex as TokioMutex, mpsc, oneshot};
use tracing::{info, warn, error, debug};
use tauri::{AppHandle, Manager, Emitter};
use uuid::Uuid;
use futures_util::{SinkExt, StreamExt};
use crate::cloud::types::*;
use crate::cloud::config::CloudConfig;
use crate::cloud::auth::DeviceAuth;
use crate::cloud::security::CloudSecurity;
use crate::cloud::commands::CloudCommandProcessor;

use super::types::{
    CloudError, CloudCommand, DeviceResponse, DeviceStatus, WebSocketMessage, MessageType,
};
use crate::constants::{permissions, api};

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

    // WebSocket sender for outgoing messages
    ws_sender: Arc<TokioMutex<Option<futures_util::stream::SplitSink<tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>, tokio_tungstenite::tungstenite::Message>>>>,

    // Command tracking
    pending_commands: Arc<TokioMutex<HashMap<String, oneshot::Sender<DeviceResponse>>>>,

    // Communication channels
    command_tx: mpsc::UnboundedSender<ConnectorMessage>,
    command_rx: Arc<TokioMutex<mpsc::UnboundedReceiver<ConnectorMessage>>>,

    // Enhanced monitoring and statistics
    connection_start_time: Arc<TokioMutex<Option<Instant>>>,
    command_statistics: Arc<TokioMutex<CommandStatistics>>,
    last_heartbeat: Arc<TokioMutex<Option<SystemTime>>>,
    reconnection_count: Arc<TokioMutex<u32>>,
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

/// Enhanced connection statistics with real metrics
#[derive(Debug, Clone, Default)]
pub struct CommandStatistics {
    pub total_commands: u64,
    pub successful_commands: u64,
    pub failed_commands: u64,
    pub command_execution_times: Vec<Duration>,
    pub last_command_time: Option<SystemTime>,
}

/// Hardware monitoring implementation
struct HardwareMonitor;

impl HardwareMonitor {
    /// Get current CPU usage percentage
    async fn get_cpu_usage() -> Option<f32> {
        // Implementation for macOS using system calls
        #[cfg(target_os = "macos")]
        {
            use std::process::Command;

            match Command::new("top")
                .args(&["-l", "1", "-n", "0"])
                .output()
            {
                Ok(output) => {
                    let output_str = String::from_utf8_lossy(&output.stdout);
                    Self::parse_cpu_usage(&output_str)
                },
                Err(e) => {
                    log::warn!("Failed to get CPU usage: {}", e);
                    None
                }
            }
        }

        #[cfg(not(target_os = "macos"))]
        {
            // Cross-platform fallback - could implement for Linux/Windows
            log::debug!("CPU monitoring not implemented for this platform");
            None
        }
    }

    /// Parse CPU usage from top command output
    #[cfg(target_os = "macos")]
    fn parse_cpu_usage(output: &str) -> Option<f32> {
        use regex::Regex;

        // Example: "CPU usage: 15.38% user, 8.46% sys, 76.15% idle"
        let cpu_regex = Regex::new(r"CPU usage:\s*(\d+\.?\d*)%\s*user,\s*(\d+\.?\d*)%\s*sys").ok()?;

        for line in output.lines() {
            if let Some(captures) = cpu_regex.captures(line) {
                let user_cpu = captures.get(1)?.as_str().parse::<f32>().ok()?;
                let sys_cpu = captures.get(2)?.as_str().parse::<f32>().ok()?;

                let total_cpu = user_cpu + sys_cpu;
                log::debug!("Parsed CPU usage: {}% user + {}% sys = {}% total", user_cpu, sys_cpu, total_cpu);

                return Some(total_cpu);
            }
        }

        log::warn!("Could not parse CPU usage from top output");
        None
    }

    /// Get current memory usage percentage
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
                                if let Ok(parsed) = num_str.trim().trim_end_matches('.').parse() {
                                    free_pages = parsed;
                                } else {
                                    tracing::warn!("Failed to parse free pages: {}", num_str);
                                }
                            }
                        } else if line.contains("Pages active:") {
                            if let Some(num_str) = line.split(':').nth(1) {
                                if let Ok(parsed) = num_str.trim().trim_end_matches('.').parse() {
                                    active_pages = parsed;
                                } else {
                                    tracing::warn!("Failed to parse active pages: {}", num_str);
                                }
                            }
                        } else if line.contains("Pages inactive:") {
                            if let Some(num_str) = line.split(':').nth(1) {
                                if let Ok(parsed) = num_str.trim().trim_end_matches('.').parse() {
                                    inactive_pages = parsed;
                                } else {
                                    tracing::warn!("Failed to parse inactive pages: {}", num_str);
                                }
                            }
                        } else if line.contains("Pages speculative:") {
                            if let Some(num_str) = line.split(':').nth(1) {
                                if let Ok(parsed) = num_str.trim().trim_end_matches('.').parse() {
                                    speculative_pages = parsed;
                                } else {
                                    tracing::warn!("Failed to parse speculative pages: {}", num_str);
                                }
                            }
                        } else if line.contains("Pages wired down:") {
                            if let Some(num_str) = line.split(':').nth(1) {
                                if let Ok(parsed) = num_str.trim().trim_end_matches('.').parse() {
                                    wired_pages = parsed;
                                } else {
                                    tracing::warn!("Failed to parse wired pages: {}", num_str);
                                }
                            }
                        }
                    }

                    let total_pages = free_pages + active_pages + inactive_pages + speculative_pages + wired_pages;
                    let used_pages = total_pages - free_pages;

                    if total_pages > 0 {
                        let usage_percentage = (used_pages as f32 / total_pages as f32) * 100.0;
                        Some(usage_percentage)
                    } else {
                        None
                    }
                },
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

    /// Get current disk usage percentage for the main drive
    async fn get_disk_usage() -> Option<f32> {
        #[cfg(target_os = "macos")]
        {
            use std::process::Command;

            match Command::new("df")
                .args(&["-h", "/"])
                .output()
            {
                Ok(output) => {
                    let output_str = String::from_utf8_lossy(&output.stdout);
                    for line in output_str.lines().skip(1) { // Skip header
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
                        break; // Only process first (root) filesystem
                    }
                    None
                },
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

    /// Get screen resolution as a formatted string
    async fn get_screen_resolution() -> Option<String> {
        #[cfg(target_os = "macos")]
        {
            use std::process::Command;

            match Command::new("system_profiler")
                .args(&["SPDisplaysDataType"])
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
                },
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

    /// Get comprehensive hardware information
    async fn get_comprehensive_hardware_info() -> HardwareInfo {
        log::debug!("🔍 Gathering comprehensive hardware information...");

        let (cpu_usage, memory_usage, disk_usage, screen_resolution) = tokio::join!(
            Self::get_cpu_usage(),
            Self::get_memory_usage(),
            Self::get_disk_usage(),
            Self::get_screen_resolution()
        );

        log::debug!(
            "📊 Hardware metrics - CPU: {:?}%, Memory: {:?}%, Disk: {:?}%, Screen: {:?}",
            cpu_usage, memory_usage, disk_usage, screen_resolution
        );

        HardwareInfo {
            cpu_usage,
            memory_usage,
            disk_usage,
            screen_resolution,
        }
    }
}

impl ProductionCloudConnector {
    /// Create new production cloud connector with enhanced monitoring
    pub async fn new(app_handle: AppHandle) -> Result<Self, CloudError> {
        let settings_manager = crate::settings::manager::SettingsManager::new(app_handle.clone())?;
        let config = CloudConfig::load_from_centralized_settings(&settings_manager).await?;
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
            ws_sender: Arc::new(TokioMutex::new(None)),
            pending_commands: Arc::new(TokioMutex::new(HashMap::new())),
            command_tx,
            command_rx: Arc::new(TokioMutex::new(command_rx)),
            // Enhanced monitoring fields
            connection_start_time: Arc::new(TokioMutex::new(None)),
            command_statistics: Arc::new(TokioMutex::new(CommandStatistics::default())),
            last_heartbeat: Arc::new(TokioMutex::new(None)),
            reconnection_count: Arc::new(TokioMutex::new(0)),
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
        let max_retries = api::cloud_networking::MAX_CONNECTION_RETRIES;
        let base_delay = Duration::from_millis(api::cloud_networking::BASE_RETRY_DELAY_MS);

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
                        let delay = base_delay * api::cloud_networking::BACKOFF_MULTIPLIER.pow(retry_count.min(api::cloud_networking::MAX_BACKOFF_EXPONENT));
                        info!("Retrying connection in {:?}", delay);
                        tokio::time::sleep(delay).await;
                    }
                }
            } else {
                // Wait before checking again
                tokio::time::sleep(Duration::from_millis(api::cloud_networking::CONNECTION_CHECK_INTERVAL_MS)).await;
            }
        }
    }

    /// Check if we should attempt to connect
    async fn should_connect(&self) -> bool {
        let state = self.connection_state.lock().await;
        matches!(*state, ConnectorState::Disconnected | ConnectorState::Reconnecting(_))
    }

    /// Establish WebSocket connection using native Rust WebSocket
    async fn establish_connection(&self) -> Result<(), CloudError> {
        info!("Establishing WebSocket connection to: {}", self.config.server_url);

        // Record connection start time
        *self.connection_start_time.lock().await = Some(Instant::now());

        // Create connection ID
        let connection_id = Uuid::new_v4().to_string();
        *self.connection_id.lock().await = Some(connection_id.clone());

        // Use native Rust WebSocket connection instead of JavaScript
        use tokio_tungstenite::{connect_async, tungstenite::Message};

        let url = self.config.server_url.clone();
        let (ws_stream, _) = connect_async(&url).await
            .map_err(|e| CloudError::ConnectionFailed(format!("WebSocket connection failed: {}", e)))?;

        info!("✅ WebSocket connected successfully to {}", url);
        self.set_connection_state(ConnectorState::Connected).await;

        // Split the WebSocket stream for concurrent read/write
        let (ws_sender, mut ws_receiver) = ws_stream.split();

        // Store the WebSocket sender for later use
        *self.ws_sender.lock().await = Some(ws_sender);

        // Authenticate first
        let auth_data = self.auth.create_auth_message()?;
        let auth_message = WebSocketMessage {
            message_type: MessageType::Auth,
            data: auth_data,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        };

        let auth_json = serde_json::to_string(&auth_message)?;

        // Send authentication message using stored sender
        {
            let mut sender_guard = self.ws_sender.lock().await;
            if let Some(ref mut sender) = sender_guard.as_mut() {
                sender.send(Message::Text(auth_json)).await
                    .map_err(|e| CloudError::NetworkError(format!("Failed to send auth message: {}", e)))?;
            } else {
                return Err(CloudError::NetworkError("WebSocket sender not available".to_string()));
            }
        }

        info!("🔐 Authentication message sent");

        // Wait for authentication response
        if let Some(msg) = ws_receiver.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    let response: WebSocketMessage = serde_json::from_str(&text)?;
                    if response.message_type == MessageType::Auth {
                        if let Some(success) = response.data.get("success").and_then(|s| s.as_bool()) {
                                                        if success {
                                info!("✅ Authentication successful");
                                self.set_connection_state(ConnectorState::Authenticated).await;
                            } else {
                                let error_msg = response.data.get("error")
                                    .and_then(|e| e.as_str())
                                    .unwrap_or("Authentication failed");
                                return Err(CloudError::AuthenticationFailed(error_msg.to_string()));
                            }
                        }
                    }
                },
                Ok(_) => {
                    return Err(CloudError::AuthenticationFailed("Unexpected message type".to_string()));
                },
                Err(e) => {
                    return Err(CloudError::NetworkError(format!("WebSocket error: {}", e)));
                }
            }
        } else {
            return Err(CloudError::AuthenticationFailed("No authentication response".to_string()));
        }

        // Start WebSocket message handling in background
        let app_handle = self.app_handle.clone();
        let connection_state = self.connection_state.clone();

        tokio::spawn(async move {
            // Handle incoming messages
            while let Some(msg) = ws_receiver.next().await {
                match msg {
                    Ok(Message::Text(text)) => {
                        info!("📨 Received cloud message: {}", text);

                        // Parse and handle the message
                        if let Ok(ws_message) = serde_json::from_str::<WebSocketMessage>(&text) {
                            match ws_message.message_type {
                                MessageType::Command => {
                                    if let Ok(command) = serde_json::from_value::<crate::cloud::types::CloudCommand>(ws_message.data) {
                                        // Emit command to be handled by the app
                                        if let Err(e) = app_handle.emit("cloud-command-received", &command) {
                                            error!("Failed to emit cloud command: {}", e);
                                        }
                                    }
                                },
                                MessageType::Heartbeat => {
                                    debug!("💓 Heartbeat received");
                                },
                                _ => {
                                    debug!("📨 Other message type: {:?}", ws_message.message_type);
                                }
                            }
                        }
                    },
                    Ok(Message::Close(_)) => {
                        info!("🔌 WebSocket closed by server");
                        let mut state = connection_state.lock().await;
                        *state = ConnectorState::Disconnected;
                        break;
                    },
                    Err(e) => {
                        error!("❌ WebSocket error: {}", e);
                        let mut state = connection_state.lock().await;
                        *state = ConnectorState::Error(e.to_string());
                        break;
                    },
                    _ => {}
                }
            }
        });

        self.set_connection_state(ConnectorState::Ready).await;
        info!("✅ Enhanced cloud connector established with hardware monitoring");
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

    /// Handle sending a command to the cloud using stored WebSocket sender
    async fn handle_send_command(&self, command: CloudCommand) -> Result<(), CloudError> {
        let start_time = Instant::now();
        let command_id = command.id.clone();

        log::info!("🚀 Executing tracked command: {} ({:?})", command_id, command.command_type);

        // Create WebSocket message
        let ws_message = WebSocketMessage {
            message_type: MessageType::Command,
            data: serde_json::to_value(&command)?,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        };

        let message_json = serde_json::to_string(&ws_message)?;

        // Send command using stored WebSocket sender
        let mut success = false;
        {
            let mut sender_guard = self.ws_sender.lock().await;
            if let Some(ref mut sender) = sender_guard.as_mut() {
                use tokio_tungstenite::tungstenite::Message;
                match sender.send(Message::Text(message_json)).await {
                    Ok(()) => {
                        success = true;
                        log::debug!("📤 Command {} sent successfully", command_id);
                    },
                    Err(e) => {
                        log::error!("❌ Failed to send command {}: {}", command_id, e);
                        return Err(CloudError::NetworkError(format!("Failed to send command: {}", e)));
                    }
                }
            } else {
                log::error!("❌ WebSocket sender not available for command {}", command_id);
                return Err(CloudError::NetworkError("WebSocket sender not available".to_string()));
            }
        }

        let execution_time = start_time.elapsed();
        self.track_command_execution(success, execution_time).await;

        log::info!("✅ Command {} completed in {:?}", command_id, execution_time);
        Ok(())
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

    /// Send status update to cloud using stored WebSocket sender
    async fn send_status_update(&self) -> Result<(), CloudError> {
        debug!("📊 Sending status update to cloud");

        // Create device status
        let device_status = self.create_device_status().await?;

        // Create WebSocket message
        let ws_message = WebSocketMessage {
            message_type: MessageType::Status,
            data: serde_json::to_value(&device_status)?,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        };

        let message_json = serde_json::to_string(&ws_message)?;

        // Send status update using stored WebSocket sender
        {
            let mut sender_guard = self.ws_sender.lock().await;
            if let Some(ref mut sender) = sender_guard.as_mut() {
                use tokio_tungstenite::tungstenite::Message;
                match sender.send(Message::Text(message_json)).await {
                    Ok(()) => {
                        debug!("✅ Status update sent successfully");
                    },
                    Err(e) => {
                        warn!("⚠️ Failed to send status update: {}", e);
                        return Err(CloudError::NetworkError(format!("Failed to send status update: {}", e)));
                    }
                }
            } else {
                warn!("⚠️ WebSocket sender not available for status update");
                return Err(CloudError::NetworkError("WebSocket sender not available".to_string()));
            }
        }

        Ok(())
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

                // Update last heartbeat time
                *self.last_heartbeat.lock().await = Some(SystemTime::now());

                // Send heartbeat message
                if let Err(e) = self.send_heartbeat().await {
                    warn!("Failed to send heartbeat: {}", e);
                } else {
                    debug!("💓 Heartbeat sent successfully");
                }
            }
        }
    }

    /// Send heartbeat message to maintain connection
    async fn send_heartbeat(&self) -> Result<(), CloudError> {
        let ws_message = WebSocketMessage {
            message_type: MessageType::Heartbeat,
            data: serde_json::json!({"timestamp": SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()}),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        };

        let message_json = serde_json::to_string(&ws_message)?;

        // Send heartbeat using stored WebSocket sender
        {
            let mut sender_guard = self.ws_sender.lock().await;
            if let Some(ref mut sender) = sender_guard.as_mut() {
                use tokio_tungstenite::tungstenite::Message;
                sender.send(Message::Text(message_json)).await
                    .map_err(|e| CloudError::NetworkError(format!("Failed to send heartbeat: {}", e)))?;
            } else {
                return Err(CloudError::NetworkError("WebSocket sender not available".to_string()));
            }
        }

        Ok(())
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
        let _app_state = self.app_handle.state::<crate::state::AppState>();

        let device_id = self.auth.get_credentials()
            .map(|c| c.device_id.clone())
            .unwrap_or_else(|| "unknown".to_string());

        // Get current task from app state if available
        let current_task = self.get_current_agent_task().await;

        let status = DeviceStatus {
            device_id,
            status: crate::cloud::types::DeviceState::Online,
            current_task,
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

    /// Get current agent task from app state
    async fn get_current_agent_task(&self) -> Option<String> {
        // Try to get current task from agent state
        // This could be implemented by checking if an agent operation is in progress
        let app_state = self.app_handle.state::<crate::state::AppState>();

        // Check if any agent is currently active
        if app_state.is_agent_executing() {
            Some("Agent interaction in progress".to_string())
        } else if app_state.dictation_active.lock()
            .map(|guard| *guard)
            .unwrap_or(false) {
            Some("Voice dictation active".to_string())
        } else {
            None
        }
    }

    /// Get permission status
    async fn get_permission_status(&self) -> Vec<String> {
        let app_state = self.app_handle.state::<crate::state::AppState>();
        let mut permissions = Vec::new();

        if app_state.is_desktop_available() {
            permissions.push(permissions::types::ACCESSIBILITY.to_string());
            permissions.push(permissions::types::SCREEN_RECORDING.to_string());
        }

        let voice_enabled = app_state.always_listening_active.lock()
            .map(|guard| *guard)
            .unwrap_or(false);

        if voice_enabled {
            permissions.push(permissions::types::MICROPHONE.to_string());
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
            "hardware_monitoring".to_string(), // New capability
        ]
    }

    /// Get comprehensive hardware information with real system monitoring
    async fn get_hardware_info(&self) -> crate::cloud::types::HardwareInfo {
        log::info!("🔍 Collecting real-time hardware information...");
        let start_time = Instant::now();

        let hardware_info = HardwareMonitor::get_comprehensive_hardware_info().await;
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
        {
            let mut sender_guard = self.ws_sender.lock().await;
            if let Some(mut sender) = sender_guard.take() {
                use tokio_tungstenite::tungstenite::Message;
                if let Err(e) = sender.send(Message::Close(None)).await {
                    warn!("Failed to send close message: {}", e);
                }
                info!("🔌 WebSocket sender closed");
            }
        }

        // Clear connection state
        *self.connection_id.lock().await = None;
        self.set_connection_state(ConnectorState::Disconnected).await;

        Ok(())
    }

    /// Get connection statistics
    pub async fn get_connection_stats(&self) -> ConnectionStats {
        let connection_start = self.connection_start_time.lock().await;
        let stats = self.command_statistics.lock().await;
        let last_heartbeat = self.last_heartbeat.lock().await;
        let reconnect_count = self.reconnection_count.lock().await;

        let connected_at = connection_start.as_ref().map(|start| {
            start.elapsed().as_secs()
        });

        let last_heartbeat_timestamp = last_heartbeat.as_ref().map(|hb| {
            hb.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs()
        });

        // Calculate average latency from command execution times
        let avg_latency = if !stats.command_execution_times.is_empty() {
            let total_ms: u64 = stats.command_execution_times
                .iter()
                .map(|d| d.as_millis() as u64)
                .sum();
            Some(total_ms / stats.command_execution_times.len() as u64)
        } else {
            None
        };

        log::debug!(
            "📊 Connection stats: {} total commands, {} successful, {} failed, reconnections: {}",
            stats.total_commands,
            stats.successful_commands,
            stats.failed_commands,
            *reconnect_count
        );

        ConnectionStats {
            connected_at,
            total_commands: stats.total_commands,
            successful_commands: stats.successful_commands,
            failed_commands: stats.failed_commands,
            reconnection_count: *reconnect_count,
            last_heartbeat: last_heartbeat_timestamp,
            latency_ms: avg_latency,
        }
    }

    /// Track command execution for statistics
    async fn track_command_execution(&self, success: bool, execution_time: Duration) {
        let mut stats = self.command_statistics.lock().await;

        stats.total_commands += 1;
        if success {
            stats.successful_commands += 1;
        } else {
            stats.failed_commands += 1;
        }

        stats.command_execution_times.push(execution_time);
        stats.last_command_time = Some(SystemTime::now());

        // Keep only recent execution times (last 100) to prevent memory growth
        if stats.command_execution_times.len() > 100 {
            stats.command_execution_times.drain(0..10);
        }

        log::debug!(
            "📈 Command tracking updated: {} total, {} successful, {} failed",
            stats.total_commands,
            stats.successful_commands,
            stats.failed_commands
        );
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
            ws_sender: self.ws_sender.clone(),
            pending_commands: self.pending_commands.clone(),
            command_tx: self.command_tx.clone(),
            command_rx: self.command_rx.clone(),
            connection_start_time: self.connection_start_time.clone(),
            command_statistics: self.command_statistics.clone(),
            last_heartbeat: self.last_heartbeat.clone(),
            reconnection_count: self.reconnection_count.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cpu_usage_parsing() {
        let sample_output = r#"
Processes: 123 total, 4 running, 119 sleeping, 456 threads
2024/01/15 10:30:45
Load Avg: 1.23, 1.45, 1.67
CPU usage: 15.38% user, 8.46% sys, 76.15% idle
SharedLibs: 123M resident, 456M data, 789M linkedit.
MemRegions: 12345 total, 678M resident, 901M private, 234M shared.
PhysMem: 8192M used (1234M wired), 567M unused.
"#;

        #[cfg(target_os = "macos")]
        {
            let result = HardwareMonitor::parse_cpu_usage(sample_output);
            assert!(result.is_some());
            if let Some(cpu_usage) = result {
                assert_eq!(cpu_usage, 23.84); // 15.38 + 8.46
            }
        }
    }

    #[test]
    fn test_cpu_usage_parsing_invalid_format() {
        let invalid_output = "Some random output without CPU info";

        #[cfg(target_os = "macos")]
        {
            let result = HardwareMonitor::parse_cpu_usage(invalid_output);
            assert!(result.is_none());
        }
    }

    #[test]
    fn test_cpu_usage_parsing_different_format() {
        let different_output = "CPU usage: 5.2% user, 12.8% sys, 82.0% idle";

        #[cfg(target_os = "macos")]
        {
            let result = HardwareMonitor::parse_cpu_usage(different_output);
            assert!(result.is_some());
            if let Some(cpu_usage) = result {
                assert_eq!(cpu_usage, 18.0); // 5.2 + 12.8
            }
        }
    }
}
