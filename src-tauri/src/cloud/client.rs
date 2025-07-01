use futures_util::{SinkExt, StreamExt};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
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
use crate::constants::permissions;
use crate::constants::events;

type WsSender = futures_util::stream::SplitSink<WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>, Message>;
type CloudAuth = DeviceAuth;
type CommandProcessor = CloudCommandProcessor;

/// Cloud client for WebSocket communication
#[derive(Debug)]
pub struct CloudClient {
    config: CloudConfig,
    app_handle: AppHandle,
    connection_state: Arc<TokioMutex<ConnectionState>>,
    auth: CloudAuth,
    command_processor: CommandProcessor,
    // Communication channels
    command_tx: mpsc::UnboundedSender<CloudCommand>,
    command_rx: Arc<TokioMutex<mpsc::UnboundedReceiver<CloudCommand>>>,
}

impl CloudClient {
    /// Create new cloud client
    pub async fn new(app_handle: AppHandle) -> Result<Self, CloudError> {
        let settings_manager = crate::settings::manager::SettingsManager::new(app_handle.clone())?;
        let config = CloudConfig::load_from_centralized_settings(&settings_manager).await?;
        let auth = CloudAuth::new(&config);
        let command_processor = CommandProcessor::new(app_handle.clone());

        let (command_tx, command_rx) = mpsc::unbounded_channel();

        Ok(Self {
            config,
            connection_state: Arc::new(TokioMutex::new(ConnectionState::Disconnected)),
            app_handle,
            auth,
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
}
