use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use tauri_plugin_store::StoreExt;
use super::types::CloudError;
use tracing::info;

/// Cloud configuration settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudConfig {
    pub enabled: bool,
    pub server_url: String,
    pub device_id: Option<String>,
    pub device_name: String,
    pub api_key: Option<String>,
    pub auto_connect: bool,
    pub reconnect_interval: u64, // seconds
    pub heartbeat_interval: u64, // seconds
    pub command_timeout: u64, // seconds
    pub security_level: SecurityLevel,
    pub allowed_commands: Vec<String>,
    pub denied_commands: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SecurityLevel {
    Low,    // Allow most commands
    Medium, // Require confirmation for sensitive commands
    High,   // Only allow whitelisted commands
}

impl Default for CloudConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            server_url: "wss://juno-cloud-backend.fly.dev/ws".to_string(),
            device_id: None,
            device_name: format!("Juno-{}", gethostname::gethostname().to_string_lossy()),
            api_key: None,
            auto_connect: true,
            reconnect_interval: 30,
            heartbeat_interval: 60,
            command_timeout: 300,
            security_level: SecurityLevel::Medium,
            allowed_commands: vec![
                "text_query".to_string(),
                "voice_query".to_string(),
                "status_request".to_string(),
                "screenshot".to_string(),
            ],
            denied_commands: vec![
                "system_shutdown".to_string(),
                "system_restart".to_string(),
                "file_delete_system".to_string(),
            ],
        }
    }
}

impl CloudConfig {
    /// Load configuration from Tauri store or create default.
    /// Attempts to load existing configuration, creates default if missing.
    /// Used by: Cloud service initialization and settings management.
    pub fn load_from_store(app_handle: &AppHandle) -> Result<Self, CloudError> {
        let store = app_handle.store("cloud_config.json")
            .map_err(|e| CloudError::ConfigError(format!("Failed to access cloud config store: {}", e)))?;

        // Try to load the configuration from store
        if let Some(config_value) = store.get("cloud_config") {
            match serde_json::from_value::<Self>(config_value) {
                Ok(config) => {
                    info!("Loaded cloud configuration from store");
                    return Ok(config);
                }
                Err(e) => {
                    info!("Failed to parse stored cloud config ({}), creating default", e);
                }
            }
        }

        // No valid configuration found, create and save default
        info!("No cloud configuration found in store, creating default");
        let default_config = Self::default();
        default_config.save_to_store(app_handle)?;
        Ok(default_config)
    }

    /// Save configuration to Tauri store.
    /// Serializes current configuration to JSON and saves to store.
    /// Used by: Cloud settings UI and configuration updates.
    pub fn save_to_store(&self, app_handle: &AppHandle) -> Result<(), CloudError> {
        let store = app_handle.store("cloud_config.json")
            .map_err(|e| CloudError::ConfigError(format!("Failed to access cloud config store: {}", e)))?;

        let config_value = serde_json::to_value(self)
            .map_err(|e| CloudError::ConfigError(format!("Failed to serialize cloud config: {}", e)))?;

        store.set("cloud_config", config_value);
        store.save()
            .map_err(|e| CloudError::ConfigError(format!("Failed to save cloud config store: {}", e)))?;

        info!("Saved cloud configuration to store");
        Ok(())
    }

    /// Load configuration from file, creating default if not exists
    /// DEPRECATED: Use load_from_store() instead. Kept for backwards compatibility during migration.
    pub fn load_from_file(app_handle: &tauri::AppHandle) -> Result<Self, CloudError> {
        // Attempt to migrate from old file-based config to new store-based config
        info!("Attempting to load legacy cloud config file for migration");
        Self::load_from_store(app_handle)
    }

    /// Save configuration to file
    /// DEPRECATED: Use save_to_store() instead. Kept for backwards compatibility during migration.
    pub fn save_to_file(&self, app_handle: &tauri::AppHandle) -> Result<(), CloudError> {
        // Redirect to store-based saving
        self.save_to_store(app_handle)
    }

    /// Validate the configuration
    pub fn validate(&self) -> Result<(), CloudError> {
        if self.enabled {
            if self.api_key.is_none() {
                return Err(CloudError::ConfigError("API key is required when cloud is enabled".to_string()));
            }

            if self.server_url.is_empty() {
                return Err(CloudError::ConfigError("Server URL cannot be empty".to_string()));
            }

            if !self.server_url.starts_with("ws://") && !self.server_url.starts_with("wss://") {
                return Err(CloudError::ConfigError("Server URL must be a WebSocket URL (ws:// or wss://)".to_string()));
            }
        }

        Ok(())
    }

    /// Update API key and save
    pub fn set_api_key(&mut self, api_key: String, app_handle: &tauri::AppHandle) -> Result<(), CloudError> {
        self.api_key = Some(api_key);
        self.save_to_store(app_handle)
    }

    /// Enable cloud connectivity
    pub fn enable(&mut self, app_handle: &tauri::AppHandle) -> Result<(), CloudError> {
        self.enabled = true;
        self.validate()?;
        self.save_to_store(app_handle)
    }

    /// Disable cloud connectivity
    pub fn disable(&mut self, app_handle: &tauri::AppHandle) -> Result<(), CloudError> {
        self.enabled = false;
        self.save_to_store(app_handle)
    }

    /// Check if a command is allowed
    pub fn is_command_allowed(&self, command: &str) -> bool {
        // If in denied list, always block
        if self.denied_commands.contains(&command.to_string()) {
            return false;
        }

        match self.security_level {
            SecurityLevel::Low => true, // Allow all except denied
            SecurityLevel::Medium => {
                // Allow if in whitelist, or if it's a safe command
                self.allowed_commands.contains(&command.to_string()) ||
                self.is_safe_command(command)
            },
            SecurityLevel::High => {
                // Only allow if explicitly in whitelist
                self.allowed_commands.contains(&command.to_string())
            }
        }
    }

    /// Check if a command is considered safe
    fn is_safe_command(&self, command: &str) -> bool {
        matches!(command,
            "text_query" | "voice_query" | "status_request" | "screenshot" |
            "get_system_info" | "get_capabilities" | "heartbeat"
        )
    }
}
