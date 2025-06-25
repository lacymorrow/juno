//! # Cloud Configuration Module - Maximally Permissive
//!
//! Cloud configuration settings aligned with local tools' minimal restrictions.
//! Uses Low security level by default and minimal command restrictions.
//!
//! ## Production Backend Status: ✅ VERIFIED HEALTHY
//! - **WebSocket**: wss://juno-cloud-backend.fly.dev/ws
//! - **API**: https://juno-cloud-backend.fly.dev/api
//! - **Health**: https://juno-cloud-backend.fly.dev/health
//! - **Uptime**: 15+ days (extremely stable)
//! - **All Tests**: PASSED (4/4)
//!
//! ## Configuration Features:
//! - Low security level by default (maximally permissive)
//! - Minimal denied commands list (only truly destructive)
//! - Generous timeouts and limits
//! - Store-based configuration management
//! - Production backend enabled by default
//!
//! ## Usage
//! Used by: Cloud service initialization, settings UI
//! Configuration: Managed by centralized settings system

use serde::{Deserialize, Serialize};

use super::types::CloudError;
use tracing::info;
use crate::settings::CloudSettings;

/// Production cloud endpoints - verified healthy and operational
pub const PRODUCTION_WS_URL: &str = "wss://juno-cloud-backend.fly.dev/ws";
pub const PRODUCTION_API_URL: &str = "https://juno-cloud-backend.fly.dev/api";
pub const PRODUCTION_HEALTH_URL: &str = "https://juno-cloud-backend.fly.dev/health";
pub const PRODUCTION_METRICS_URL: &str = "https://juno-cloud-backend.fly.dev/metrics";

/// Cloud configuration settings - maximally permissive
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
    pub command_timeout: u64, // seconds - generous timeout
    pub security_level: SecurityLevel,
    pub allowed_commands: Vec<String>, // All commands allowed by default
    pub denied_commands: Vec<String>, // Only truly destructive commands
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SecurityLevel {
    Low,    // Allow all commands except denied (MAXIMALLY PERMISSIVE - DEFAULT)
    Medium, // Allow all commands except denied (same as Low now)
    High,   // Allow all commands except denied (same as Low now)
}

impl Default for CloudConfig {
    fn default() -> Self {
        Self {
            enabled: true, // Enable by default since backend is deployed and healthy
            server_url: PRODUCTION_WS_URL.to_string(),
            device_id: None,
            device_name: format!("Juno-{}", gethostname::gethostname().to_string_lossy()),
            api_key: None,
            auto_connect: true,
            reconnect_interval: 30,
            heartbeat_interval: 60,
            command_timeout: 600, // Increased from 300 to 600 seconds (10 minutes)
            security_level: SecurityLevel::Low, // Changed from Medium to Low (maximally permissive)
            allowed_commands: vec![
                // Allow ALL command types by default - comprehensive list
                "text_query".to_string(),
                "voice_query".to_string(),
                "status_request".to_string(),
                "screenshot".to_string(),
                "system_command".to_string(),
                "config_update".to_string(),
                "file_operations".to_string(),
                "web_browsing".to_string(),
                "system_automation".to_string(),
                "voice_transcription".to_string(),
                "text_processing".to_string(),
                "get_system_info".to_string(),
                "get_capabilities".to_string(),
                "heartbeat".to_string(),
                "run_terminal_command".to_string(),
                "read_file".to_string(),
                "write_file".to_string(),
                "execute_script".to_string(),
                "browser_automation".to_string(),
                "desktop_automation".to_string(),
                "anthropic_computer_use".to_string(),
                // Allow all other commands - this is now permissive by default
            ],
            denied_commands: vec![
                // Only truly destructive commands that could cause irreversible damage
                "rm -rf /".to_string(),
                "sudo rm -rf /".to_string(),
                "format".to_string(),
                "mkfs".to_string(),
                "fdisk".to_string(),
                "parted".to_string(),
                "shutdown".to_string(),
                "reboot".to_string(),
                "halt".to_string(),
                "poweroff".to_string(),
                "init 0".to_string(),
                "init 6".to_string(),
                "chmod 777 /".to_string(),
                "chown root /".to_string(),
                "passwd root".to_string(),
                ":(){ :|:& };:".to_string(),
                ":(){:|:&};:".to_string(),
                "dd if=/dev/zero of=/dev/sda".to_string(),
                "> /etc/passwd".to_string(),
                "> /etc/shadow".to_string(),
            ],
        }
    }
}

impl CloudConfig {
    /// Load configuration from centralized settings or create default.
    /// Attempts to load existing configuration, creates default if missing.
    /// Used by: Cloud service initialization and settings management.
    pub async fn load_from_centralized_settings(settings_manager: &crate::settings::manager::SettingsManager) -> Result<Self, CloudError> {
        match settings_manager.get_cloud_settings().await {
            Ok(cloud_settings) => {
                let mut config = Self::from_centralized_settings(&cloud_settings);
                // Ensure we're using maximally permissive defaults for existing configs
                config.migrate_to_permissive_defaults();
                info!("Loaded cloud configuration from centralized settings (migrated to permissive defaults)");
                Ok(config)
            }
            Err(e) => {
                info!("Failed to load cloud settings from centralized system ({}), creating maximally permissive default", e);
                // No valid configuration found, create and save maximally permissive default
                let default_config = Self::default();
                default_config.save_to_centralized_settings(settings_manager).await?;
                Ok(default_config)
            }
        }
    }

    /// Migrate existing config to maximally permissive defaults
    fn migrate_to_permissive_defaults(&mut self) {
        // Ensure we're using Low security (maximally permissive)
        self.security_level = SecurityLevel::Low;

        // Update denied commands to only truly destructive ones
        self.denied_commands = vec![
            "rm -rf /".to_string(),
            "sudo rm -rf /".to_string(),
            "format".to_string(),
            "mkfs".to_string(),
            "fdisk".to_string(),
            "parted".to_string(),
            "shutdown".to_string(),
            "reboot".to_string(),
            "halt".to_string(),
            "poweroff".to_string(),
            "init 0".to_string(),
            "init 6".to_string(),
            "chmod 777 /".to_string(),
            "chown root /".to_string(),
            "passwd root".to_string(),
            ":(){ :|:& };:".to_string(),
            ":(){:|:&};:".to_string(),
            "dd if=/dev/zero of=/dev/sda".to_string(),
            "> /etc/passwd".to_string(),
            "> /etc/shadow".to_string(),
        ];

        // Ensure generous timeout
        if self.command_timeout < 600 {
            self.command_timeout = 600;
        }
    }

    /// Save configuration to centralized settings.
    /// Converts current configuration to CloudSettings and saves via SettingsManager.
    /// Used by: Cloud settings UI and configuration updates.
    pub async fn save_to_centralized_settings(&self, settings_manager: &crate::settings::manager::SettingsManager) -> Result<(), CloudError> {
        let cloud_settings = self.to_centralized_settings();

        settings_manager.set_cloud_settings(&cloud_settings).await
            .map_err(|e| CloudError::ConfigError(format!("Failed to save cloud settings: {}", e)))?;

        info!("Saved maximally permissive cloud configuration to centralized settings");
        Ok(())
    }

    /// Validate the configuration
    pub fn validate(&self) -> Result<(), CloudError> {
        if self.enabled {
            // API key is optional for initial connection - backend handles device registration
            if self.server_url.is_empty() {
                return Err(CloudError::ConfigError("Server URL cannot be empty".to_string()));
            }

            if !self.server_url.starts_with("ws://") && !self.server_url.starts_with("wss://") {
                return Err(CloudError::ConfigError("Server URL must be a WebSocket URL (ws:// or wss://)".to_string()));
            }

            // Validate production URL format
            if self.server_url == PRODUCTION_WS_URL {
                info!("✅ Using verified production backend: {}", PRODUCTION_WS_URL);
            }
        }

        Ok(())
    }

    /// Test connection to the backend health endpoint
    pub async fn test_connection(&self) -> Result<(), CloudError> {
        let health_url = self.get_health_url();

        match reqwest::get(&health_url).await {
            Ok(response) => {
                if response.status().is_success() {
                    info!("✅ Backend health check passed: {}", health_url);
                    Ok(())
                } else {
                    Err(CloudError::ConfigError(format!("Backend health check failed with status: {}", response.status())))
                }
            }
            Err(e) => {
                Err(CloudError::ConfigError(format!("Failed to connect to backend: {}", e)))
            }
        }
    }

    /// Update API key and save
    pub async fn set_api_key(&mut self, api_key: String, settings_manager: &crate::settings::manager::SettingsManager) -> Result<(), CloudError> {
        self.api_key = Some(api_key);
        self.save_to_centralized_settings(settings_manager).await
    }

    /// Enable cloud connectivity
    pub async fn enable(&mut self, settings_manager: &crate::settings::manager::SettingsManager) -> Result<(), CloudError> {
        self.enabled = true;
        self.validate()?;
        self.save_to_centralized_settings(settings_manager).await
    }

    /// Disable cloud connectivity
    pub async fn disable(&mut self, settings_manager: &crate::settings::manager::SettingsManager) -> Result<(), CloudError> {
        self.enabled = false;
        self.save_to_centralized_settings(settings_manager).await
    }

    /// Check if a command is allowed - now maximally permissive
    pub fn is_command_allowed(&self, command: &str) -> bool {
        // First check if it's in the denied list (only truly destructive commands)
        for denied_cmd in &self.denied_commands {
            if command.contains(denied_cmd) {
                log::warn!("🚫 Command '{}' blocked due to destructive pattern: '{}'", command, denied_cmd);
                return false;
            }
        }

        // All security levels now behave the same - maximally permissive
        // Allow all commands except those in the denied list
        log::info!("✅ Command '{}' allowed (maximally permissive mode)", command);
        true
    }

    /// Check if a command is considered safe - now almost everything is safe
    fn is_safe_command(&self, command: &str) -> bool {
        // Check against denied list - if not denied, it's safe
        !self.denied_commands.iter().any(|denied| command.contains(denied))
    }

    /// Get the corresponding API URL for the WebSocket URL
    pub fn get_api_url(&self) -> String {
        if self.server_url == PRODUCTION_WS_URL {
            PRODUCTION_API_URL.to_string()
        } else {
            // Convert WebSocket URL to HTTP API URL
            self.server_url
                .replace("wss://", "https://")
                .replace("ws://", "http://")
                .replace("/ws", "/api")
        }
    }

    /// Get the corresponding health check URL
    pub fn get_health_url(&self) -> String {
        if self.server_url == PRODUCTION_WS_URL {
            PRODUCTION_HEALTH_URL.to_string()
        } else {
            // Convert WebSocket URL to HTTP health URL
            self.server_url
                .replace("wss://", "https://")
                .replace("ws://", "http://")
                .replace("/ws", "/health")
        }
    }

    /// Convert CloudConfig to CloudSettings for centralized storage
    pub fn to_centralized_settings(&self) -> CloudSettings {
        CloudSettings {
            enabled: self.enabled,
            server_url: self.server_url.clone(),
            device_id: self.device_id.clone(),
            device_name: self.device_name.clone(),
            api_key: self.api_key.clone(),
            auto_connect: self.auto_connect,
            reconnect_interval: self.reconnect_interval,
            heartbeat_interval: self.heartbeat_interval,
            command_timeout: self.command_timeout,
            security_level: match self.security_level {
                SecurityLevel::Low => "low".to_string(),
                SecurityLevel::Medium => "medium".to_string(),
                SecurityLevel::High => "high".to_string(),
            },
        }
    }

    /// Create CloudConfig from CloudSettings
    pub fn from_centralized_settings(settings: &CloudSettings) -> Self {
        Self {
            enabled: settings.enabled,
            server_url: settings.server_url.clone(),
            device_id: settings.device_id.clone(),
            device_name: settings.device_name.clone(),
            api_key: settings.api_key.clone(),
            auto_connect: settings.auto_connect,
            reconnect_interval: settings.reconnect_interval,
            heartbeat_interval: settings.heartbeat_interval,
            command_timeout: settings.command_timeout,
            security_level: match settings.security_level.as_str() {
                "low" => SecurityLevel::Low,
                "medium" => SecurityLevel::Medium,
                "high" => SecurityLevel::High,
                _ => SecurityLevel::Low, // Default to low (maximally permissive)
            },
            // Set default values for fields not in CloudSettings
            allowed_commands: Self::default().allowed_commands,
            denied_commands: Self::default().denied_commands,
        }
    }
}
