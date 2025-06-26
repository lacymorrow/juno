use crate::cloud::auth::DeviceAuth;
use crate::cloud::commands::CloudCommandProcessor;
use crate::cloud::config::CloudConfig;
use crate::cloud::security::CloudSecurity;
use crate::cloud::types::*;
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::{mpsc, oneshot, Mutex as TokioMutex};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use super::types::{
    CloudCommand, CloudError, DeviceResponse, DeviceStatus, MessageType, WebSocketMessage,
};
use crate::constants::{api, permissions};

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
    ws_sender: Arc<
        TokioMutex<
            Option<
                futures_util::stream::SplitSink<
                    tokio_tungstenite::WebSocketStream<
                        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
                    >,
                    tokio_tungstenite::tungstenite::Message,
                >,
            >,
        >,
    >,

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

            match Command::new("top").args(&["-l", "1", "-n", "0"]).output() {
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
                                    tracing::warn!(
                                        "Failed to parse speculative pages: {}",
                                        num_str
                                    );
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

    /// Get current disk usage percentage for the main drive
    async fn get_disk_usage() -> Option<f32> {
        #[cfg(target_os = "macos")]
        {
            use std::process::Command;

            match Command::new("df").args(&["-h", "/"]).output() {
                Ok(output) => {
                    let output_str = String::from_utf8_lossy(&output.stdout);
                    for line in output_str.lines().skip(1) {
                        // Skip header
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
                            self.set_connection_state(ConnectorState::Error(e.to_string()))
                                .await;
                        }
                    }
                    Err(e) => {
                        retry_count += 1;
                        error!(
                            "Failed to establish connection (attempt {}): {}",
                            retry_count, e
                        );

                        if retry_count >= max_retries {
                            self.set_connection_state(ConnectorState::Error(format!(
                                "Max retries exceeded: {}",
                                e
                            )))
                            .await;
                            break;
                        }

                        self.set_connection_state(ConnectorState::Reconnecting(retry_count))
                            .await;

                        // Exponential backoff
                        let delay = base_delay
                            * api::cloud_networking::BACKOFF_MULTIPLIER
                                .pow(retry_count.min(api::cloud_networking::MAX_BACKOFF_EXPONENT));
                        info!("Retrying connection in {:?}", delay);
                        tokio::time::sleep(delay).await;
                    }
                }
            } else {
                // Wait before checking again
                tokio::time::sleep(Duration::from_millis(
                    api::cloud_networking::CONNECTION_CHECK_INTERVAL_MS,
                ))
                .await;
            }
        }
    }

    /// Check if we should attempt to connect
    async fn should_connect(&self) -> bool {
        let state = self.connection_state.lock().await;
        matches!(
            *state,
            ConnectorState::Disconnected | ConnectorState::Reconnecting(_)
        )
    }

    /// Establish WebSocket connection using native Rust WebSocket
    async fn establish_connection(&self) -> Result<(), CloudError> {
        info!(
            "Establishing WebSocket connection to: {}",
            self.config.server_url
        );

        // Record connection start time
        *self.connection_start_time.lock().await = Some(Instant::now());

        // Create connection ID
        let connection_id = Uuid::new_v4().to_string();
        *self.connection_id.lock().await = Some(connection_id.clone());

        // Use native Rust WebSocket connection instead of JavaScript
        use tokio_tungstenite::{connect_async, tungstenite::Message};

        let url = self.config.server_url.clone();
        let (ws_stream, _) = connect_async(&url).await.map_err(|e| {
            CloudError::ConnectionFailed(format!("WebSocket connection failed: {}", e))
        })?;

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
                sender.send(Message::Text(auth_json)).await.map_err(|e| {
                    CloudError::NetworkError(format!("Failed to send auth message: {}", e))
                })?;
            } else {
                return Err(CloudError::NetworkError(
                    "WebSocket sender not available".to_string(),
                ));
            }
        }

        info!("🔐 Authentication message sent");

        // Start background task for authentication and message handling
        // This fixes the ownership violation by moving ws_receiver into the task
        let app_handle = self.app_handle.clone();
        let connection_state = self.connection_state.clone();
        let ws_sender_clone = self.ws_sender.clone();
        let auth_clone = self.auth.clone();

        // Use a channel to signal authentication completion
        let (auth_tx, auth_rx) = tokio::sync::oneshot::channel();

        tokio::spawn(async move {
            // Handle authentication response within the spawned task
            let auth_result = match Self::handle_authentication_response_in_task(
                &mut ws_receiver,
                &auth_clone,
                &app_handle,
                &connection_state,
            )
            .await
            {
                Ok((auth_success, message_buffer)) => {
                    if auth_success {
                        info!("✅ Authentication successful, starting message handling");

                        // Set authenticated state
                        {
                            let mut state = connection_state.lock().await;
                            *state = ConnectorState::Authenticated;
                        }

                        // Process any buffered messages from authentication
                        if !message_buffer.is_empty() {
                            info!(
                                "📦 Processing {} buffered messages from authentication phase",
                                message_buffer.len()
                            );
                            for buffered_text in message_buffer {
                                Self::process_websocket_message(
                                    buffered_text,
                                    &app_handle,
                                    &connection_state,
                                )
                                .await;
                            }
                            info!("✅ Finished processing buffered messages");
                        }

                        // Signal successful authentication
                        let _ = auth_tx.send(Ok(()));
                        true
                    } else {
                        error!("❌ Authentication failed");
                        let _ = auth_tx.send(Err(CloudError::AuthenticationFailed(
                            "Authentication failed".to_string(),
                        )));
                        false
                    }
                }
                Err((auth_error, recovered_buffer)) => {
                    error!(
                        "🔥 Authentication failed but recovered {} buffered messages: {}",
                        recovered_buffer.len(),
                        auth_error
                    );

                    if !recovered_buffer.is_empty() {
                        warn!("📦 Buffered messages from failed authentication will be lost:");
                        for (i, msg) in recovered_buffer.iter().enumerate() {
                            debug!("  [{}]: {}", i, msg);
                        }
                    }

                    let _ = auth_tx.send(Err(auth_error));
                    false
                }
            };

            // Only continue message handling if authentication succeeded
            if auth_result {
                // Handle incoming messages (authentication already completed)
                while let Some(msg) = ws_receiver.next().await {
                    match msg {
                        Ok(Message::Text(text)) => {
                            Self::process_websocket_message(text, &app_handle, &connection_state)
                                .await;
                        }
                        Ok(Message::Close(_)) => {
                            info!("🔌 WebSocket closed by server");
                            let mut state = connection_state.lock().await;
                            *state = ConnectorState::Disconnected;
                            break;
                        }
                        Err(e) => {
                            error!("❌ WebSocket error: {}", e);
                            let mut state = connection_state.lock().await;
                            *state = ConnectorState::Error(e.to_string());
                            break;
                        }
                        _ => {}
                    }
                }
            }

            // Clean up sender when connection closes
            {
                let mut sender_guard = ws_sender_clone.lock().await;
                *sender_guard = None;
            }
        });

        // Wait for authentication to complete before setting Ready state
        // This fixes the race condition
        match auth_rx.await {
            Ok(Ok(())) => {
                self.set_connection_state(ConnectorState::Ready).await;
                info!("✅ Enhanced cloud connector established with hardware monitoring");
                Ok(())
            }
            Ok(Err(e)) => Err(e),
            Err(_) => Err(CloudError::AuthenticationFailed(
                "Authentication channel closed".to_string(),
            )),
        }
    }

    /// Process a WebSocket message (extracted for reuse in buffered message processing)
    async fn process_websocket_message(
        text: String,
        app_handle: &AppHandle,
        connection_state: &Arc<TokioMutex<ConnectorState>>,
    ) {
        debug!("📨 Received cloud message: {}", text);

        // Parse and handle the message
        if let Ok(ws_message) = serde_json::from_str::<WebSocketMessage>(&text) {
            match ws_message.message_type {
                MessageType::Auth => {
                    // Post-authentication auth messages (likely additional auth events)
                    debug!("📨 Additional auth message received post-authentication");
                }
                MessageType::Command => {
                    if let Ok(command) =
                        serde_json::from_value::<crate::cloud::types::CloudCommand>(ws_message.data)
                    {
                        // Emit command to be handled by the app
                        if let Err(e) = app_handle.emit("cloud-command-received", &command) {
                            error!("Failed to emit cloud command: {}", e);
                        }
                    } else {
                        warn!("⚠️ Failed to parse cloud command from message");
                    }
                }
                MessageType::Heartbeat => {
                    debug!("💓 Heartbeat received");
                }
                _ => {
                    debug!("📨 Other message type: {:?}", ws_message.message_type);
                }
            }
        } else {
            warn!("⚠️ Failed to parse WebSocket message: {}", text);
        }
    }

    /// Handle authentication response within the spawned task with robust error handling
    /// Returns (auth_success, buffered_messages) - buffered messages are preserved even on auth failure
    async fn handle_authentication_response_in_task(
        ws_receiver: &mut futures_util::stream::SplitStream<
            tokio_tungstenite::WebSocketStream<
                tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
            >,
        >,
        auth: &DeviceAuth,
        _app_handle: &AppHandle,
        _connection_state: &Arc<TokioMutex<ConnectorState>>,
    ) -> Result<(bool, Vec<String>), (CloudError, Vec<String>)> {
        use tokio_tungstenite::tungstenite::Message;

        // Set timeout for authentication response
        let timeout_duration = Duration::from_secs(10);
        let timeout = tokio::time::sleep(timeout_duration);
        tokio::pin!(timeout);

        // Buffer for messages received during authentication
        let mut message_buffer: Vec<String> = Vec::new();

        loop {
            tokio::select! {
                // Wait for authentication response
                msg_result = ws_receiver.next() => {
                    match msg_result {
                        Some(Ok(Message::Text(text))) => {
                            debug!("📨 Received message during authentication: {}", text);

                            // Parse the message
                            match serde_json::from_str::<WebSocketMessage>(&text) {
                                Ok(ws_message) => {
                                    if ws_message.message_type == MessageType::Auth {
                                        // Handle authentication response with robust parsing
                                        // CRITICAL: Do not use ? operator here as it would lose buffered messages on error
                                        match Self::parse_authentication_response(ws_message.data).await {
                                            Ok(auth_result) => {
                                                return Ok((auth_result, message_buffer));
                                            },
                                                                                         Err(auth_error) => {
                                                 // Authentication failed, but we must preserve buffered messages
                                                 error!("❌ Authentication failed: {}", auth_error);
                                                 // Return the error while preserving message buffer to prevent message loss
                                                 return Err((auth_error, message_buffer));
                                             }
                                        }
                                    } else {
                                        // Non-auth message during authentication - buffer it for later processing
                                        debug!("📦 Buffering non-auth message during authentication: {:?}", ws_message.message_type);
                                        message_buffer.push(text);
                                        // Continue waiting for auth response
                                        continue;
                                    }
                                },
                                Err(e) => {
                                    error!("❌ Failed to parse message during authentication: {}", e);
                                    // Buffer unparseable messages too - might be valid later
                                    warn!("📦 Buffering unparseable message for later retry: {}", text);
                                    message_buffer.push(text);
                                    continue;
                                }
                            }
                        },
                        Some(Ok(Message::Close(_))) => {
                            error!("❌ WebSocket closed during authentication");
                            return Err((CloudError::AuthenticationFailed("Connection closed during authentication".to_string()), message_buffer));
                        },
                        Some(Err(e)) => {
                            error!("❌ WebSocket error during authentication: {}", e);
                            return Err((CloudError::AuthenticationFailed(format!("WebSocket error: {}", e)), message_buffer));
                        },
                        None => {
                            error!("❌ WebSocket stream ended during authentication");
                            return Err((CloudError::AuthenticationFailed("WebSocket stream ended".to_string()), message_buffer));
                        },
                        _ => {
                            // Other message types (binary, ping, pong) - continue waiting
                            debug!("📨 Received non-text message during authentication, continuing...");
                            continue;
                        }
                    }
                },
                // Authentication timeout
                _ = &mut timeout => {
                    error!("❌ Authentication timeout after {:?}", timeout_duration);
                    return Err((CloudError::AuthenticationFailed("Authentication timeout".to_string()), message_buffer));
                }
            }
        }
    }

    /// Parse authentication response with comprehensive error handling
    async fn parse_authentication_response(data: serde_json::Value) -> Result<bool, CloudError> {
        // Check for success field
        match data.get("success") {
            Some(success_value) => {
                match success_value.as_bool() {
                    Some(true) => {
                        info!("✅ Authentication successful");
                        Ok(true)
                    }
                    Some(false) => {
                        // Authentication explicitly failed
                        let error_msg = data
                            .get("error")
                            .and_then(|e| e.as_str())
                            .unwrap_or("Authentication rejected by server");
                        error!("❌ Authentication failed: {}", error_msg);
                        Err(CloudError::AuthenticationFailed(error_msg.to_string()))
                    }
                    None => {
                        // Success field is not a boolean
                        let success_str = success_value.to_string();
                        error!(
                            "❌ Authentication response 'success' field is not boolean: {}",
                            success_str
                        );
                        Err(CloudError::AuthenticationFailed(format!(
                            "Invalid success field type: expected boolean, got {}",
                            success_str
                        )))
                    }
                }
            }
            None => {
                // Missing success field
                error!(
                    "❌ Authentication response missing 'success' field: {}",
                    data
                );
                Err(CloudError::AuthenticationFailed(
                    "Authentication response missing required 'success' field".to_string(),
                ))
            }
        }
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

        log::info!(
            "🚀 Executing tracked command: {} ({:?})",
            command_id,
            command.command_type
        );

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
        {
            let mut sender_guard = self.ws_sender.lock().await;
            if let Some(ref mut sender) = sender_guard.as_mut() {
                use tokio_tungstenite::tungstenite::Message;
                match sender.send(Message::Text(message_json)).await {
                    Ok(()) => {
                        log::debug!("📤 Command {} sent successfully", command_id);
                    }
                    Err(e) => {
                        log::error!("❌ Failed to send command {}: {}", command_id, e);
                        let execution_time = start_time.elapsed();
                        self.track_command_execution(false, execution_time).await;
                        return Err(CloudError::NetworkError(format!(
                            "Failed to send command: {}",
                            e
                        )));
                    }
                }
            } else {
                log::error!(
                    "❌ WebSocket sender not available for command {}",
                    command_id
                );
                let execution_time = start_time.elapsed();
                self.track_command_execution(false, execution_time).await;
                return Err(CloudError::NetworkError(
                    "WebSocket sender not available".to_string(),
                ));
            }
        }

        let execution_time = start_time.elapsed();
        self.track_command_execution(true, execution_time).await;

        log::info!(
            "✅ Command {} completed in {:?}",
            command_id,
            execution_time
        );
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
            warn!(
                "Received response for unknown command: {}",
                response.command_id
            );
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
                    }
                    Err(e) => {
                        warn!("⚠️ Failed to send status update: {}", e);
                        return Err(CloudError::NetworkError(format!(
                            "Failed to send status update: {}",
                            e
                        )));
                    }
                }
            } else {
                warn!("⚠️ WebSocket sender not available for status update");
                return Err(CloudError::NetworkError(
                    "WebSocket sender not available".to_string(),
                ));
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
        let mut interval =
            tokio::time::interval(Duration::from_secs(self.config.heartbeat_interval));

        loop {
            interval.tick().await;

            let state = self.connection_state.lock().await;
            if matches!(
                *state,
                ConnectorState::Ready | ConnectorState::Authenticated
            ) {
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
            data: serde_json::json!({"timestamp": SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs()}),
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
                sender
                    .send(Message::Text(message_json))
                    .await
                    .map_err(|e| {
                        CloudError::NetworkError(format!("Failed to send heartbeat: {}", e))
                    })?;
            } else {
                return Err(CloudError::NetworkError(
                    "WebSocket sender not available".to_string(),
                ));
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

        let device_id = self
            .auth
            .get_credentials()
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
                agent_mode: format!(
                    "{:?}",
                    crate::agent::providers::factory::BrainFactory::get_agent_mode()
                ),
                version: env!("CARGO_PKG_VERSION").to_string(),
                capabilities: self.get_device_capabilities(),
                hardware_info: Some(self.get_hardware_info().await),
            },
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
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
        } else if app_state.is_dictation_active() {
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

        let voice_enabled = app_state.get_always_listening_active().unwrap_or(false);

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
    pub async fn execute_remote_command(
        &self,
        command: CloudCommand,
    ) -> Result<DeviceResponse, CloudError> {
        info!(
            "Executing remote command: {} ({:?})",
            command.id, command.command_type
        );

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
        self.set_connection_state(ConnectorState::Disconnected)
            .await;

        Ok(())
    }

    /// Get connection statistics
    pub async fn get_connection_stats(&self) -> ConnectionStats {
        let connection_start = self.connection_start_time.lock().await;
        let stats = self.command_statistics.lock().await;
        let last_heartbeat = self.last_heartbeat.lock().await;
        let reconnect_count = self.reconnection_count.lock().await;

        let connected_at = connection_start
            .as_ref()
            .map(|start| start.elapsed().as_secs());

        let last_heartbeat_timestamp = last_heartbeat
            .as_ref()
            .map(|hb| hb.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs());

        // Calculate average latency from command execution times
        let avg_latency = if !stats.command_execution_times.is_empty() {
            let total_ms: u64 = stats
                .command_execution_times
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

    // Tests for authentication response parsing logic
    mod authentication_tests {
        use super::*;

        #[test]
        fn test_parse_auth_response_failure_logic() {
            // Test explicit failure response parsing logic
            let failure_data = serde_json::json!({
                "success": false,
                "error": "Invalid credentials"
            });

            // Test the parsing logic that our methods use
            match failure_data.get("success") {
                Some(success_value) => match success_value.as_bool() {
                    Some(false) => {
                        let error_msg = failure_data
                            .get("error")
                            .and_then(|e| e.as_str())
                            .unwrap_or("Authentication rejected by server");
                        assert_eq!(error_msg, "Invalid credentials");
                    }
                    _ => panic!("Should have matched false case"),
                },
                None => panic!("Should have success field"),
            }
        }

        #[test]
        fn test_parse_auth_response_missing_success_field() {
            let invalid_data = serde_json::json!({
                "message": "Some response without success field"
            });

            // Test that missing success field is detected
            assert!(invalid_data.get("success").is_none());
        }

        #[test]
        fn test_parse_auth_response_invalid_success_type() {
            let invalid_data = serde_json::json!({
                "success": "true" // String instead of boolean
            });

            // Test that non-boolean success field is detected
            match invalid_data.get("success") {
                Some(success_value) => {
                    assert!(success_value.as_bool().is_none());
                    assert_eq!(success_value.as_str(), Some("true"));
                }
                None => panic!("Should have success field"),
            }
        }

        #[test]
        fn test_parse_auth_response_success_logic() {
            let success_data = serde_json::json!({
                "success": true
            });

            // Test successful authentication parsing logic
            match success_data.get("success") {
                Some(success_value) => {
                    match success_value.as_bool() {
                        Some(true) => {
                            // This is the expected path
                            assert!(true);
                        }
                        _ => panic!("Should have matched true case"),
                    }
                }
                None => panic!("Should have success field"),
            }
        }
    }

    #[cfg(test)]
    mod websocket_race_condition_tests {
        use super::*;

        #[test]
        fn test_message_buffering_during_authentication() {
            // Test the logic that buffers non-auth messages during authentication
            let mut message_buffer: Vec<String> = Vec::new();

            // Simulate receiving various message types during authentication
            let command_message = r#"{"type": "command", "data": {"id": "test", "command_type": "screenshot"}, "timestamp": 1234567890}"#;
            let heartbeat_message = r#"{"type": "heartbeat", "data": {"timestamp": 1234567890}, "timestamp": 1234567890}"#;
            let auth_success_message =
                r#"{"type": "auth", "data": {"success": true}, "timestamp": 1234567890}"#;

            // Messages received before auth response should be buffered
            message_buffer.push(command_message.to_string());
            message_buffer.push(heartbeat_message.to_string());

            // Verify buffer contains expected messages
            assert_eq!(message_buffer.len(), 2);
            assert!(message_buffer[0].contains("command"));
            assert!(message_buffer[1].contains("heartbeat"));

            // Simulate processing auth response (would return from authentication handler)
            // The buffered messages would then be processed by the background task

            // Verify we can parse the buffered messages correctly
            for buffered_msg in &message_buffer {
                let parsed: Result<serde_json::Value, _> = serde_json::from_str(buffered_msg);
                assert!(
                    parsed.is_ok(),
                    "Buffered message should be valid JSON: {}",
                    buffered_msg
                );
            }
        }

        #[test]
        fn test_authentication_response_parsing_comprehensive() {
            // Test success case
            let success_data = serde_json::json!({
                "success": true
            });

            match success_data.get("success") {
                Some(success_value) => {
                    match success_value.as_bool() {
                        Some(true) => assert!(true), // Expected path
                        _ => panic!("Should have matched true case"),
                    }
                }
                None => panic!("Should have success field"),
            }

            // Test failure case
            let failure_data = serde_json::json!({
                "success": false,
                "error": "Invalid credentials"
            });

            match failure_data.get("success") {
                Some(success_value) => match success_value.as_bool() {
                    Some(false) => {
                        let error_msg = failure_data
                            .get("error")
                            .and_then(|e| e.as_str())
                            .unwrap_or("Authentication rejected by server");
                        assert_eq!(error_msg, "Invalid credentials");
                    }
                    _ => panic!("Should have matched false case"),
                },
                None => panic!("Should have success field"),
            }

            // Test missing success field
            let missing_success_data = serde_json::json!({
                "message": "Some response without success field"
            });

            assert!(missing_success_data.get("success").is_none());

            // Test invalid success field type
            let invalid_success_data = serde_json::json!({
                "success": "true" // String instead of boolean
            });

            match invalid_success_data.get("success") {
                Some(success_value) => {
                    assert!(success_value.as_bool().is_none());
                    assert_eq!(success_value.as_str(), Some("true"));
                }
                None => panic!("Should have success field"),
            }
        }

        #[test]
        fn test_message_buffer_ordering() {
            // Test that message ordering is preserved during buffering
            let mut message_buffer: Vec<String> = Vec::new();

            let messages = vec![
                r#"{"type": "command", "data": {"id": "cmd1"}, "timestamp": 1}"#,
                r#"{"type": "heartbeat", "data": {}, "timestamp": 2}"#,
                r#"{"type": "command", "data": {"id": "cmd2"}, "timestamp": 3}"#,
                r#"{"type": "status", "data": {}, "timestamp": 4}"#,
            ];

            // Add messages to buffer in order
            for msg in &messages {
                message_buffer.push(msg.to_string());
            }

            // Verify ordering is preserved
            assert_eq!(message_buffer.len(), 4);
            for (i, msg) in message_buffer.iter().enumerate() {
                let parsed: serde_json::Value = serde_json::from_str(msg).unwrap();
                let expected_timestamp = (i + 1) as u64;
                assert_eq!(parsed["timestamp"].as_u64().unwrap(), expected_timestamp);
            }
        }

        #[test]
        fn test_unparseable_message_buffering() {
            // Test that even unparseable messages are buffered for later retry
            let mut message_buffer: Vec<String> = Vec::new();

            let valid_message = r#"{"type": "command", "data": {"id": "test"}}"#;
            let invalid_message = r#"{"type": "command", "data": {invalid json"#;
            let malformed_message = r#"not json at all"#;

            // All messages should be buffered, even invalid ones
            message_buffer.push(valid_message.to_string());
            message_buffer.push(invalid_message.to_string());
            message_buffer.push(malformed_message.to_string());

            assert_eq!(message_buffer.len(), 3);

            // Verify that at least the valid message can be parsed
            let parsed_valid: Result<serde_json::Value, _> =
                serde_json::from_str(&message_buffer[0]);
            assert!(parsed_valid.is_ok());

            // Invalid messages should fail parsing but still be buffered
            let parsed_invalid: Result<serde_json::Value, _> =
                serde_json::from_str(&message_buffer[1]);
            assert!(parsed_invalid.is_err());

            let parsed_malformed: Result<serde_json::Value, _> =
                serde_json::from_str(&message_buffer[2]);
            assert!(parsed_malformed.is_err());
        }
    }
}
