//! # Cloud Configuration Module
//!
//! Cloud configuration settings. Cloud is disabled by default (the hosted
//! backend is not running, see LAC-3729) and new configs default to the
//! High security level. The user's stored security level is preserved on
//! load; it is never downgraded (2026-09 security audit).
//!
//! ## Configuration Features:
//! - Disabled by default, High security level by default
//! - Denied commands list for destructive patterns
//! - Store-based configuration management
//!
//! ## Usage
//! Used by: Cloud service initialization, settings UI
//! Configuration: Managed by centralized settings system

use serde::{Deserialize, Serialize};
use std::sync::LazyLock;

use super::types::CloudError;
use crate::settings::CloudSettings;
use tracing::info;

/// Production cloud endpoints - verified healthy and operational
pub const PRODUCTION_WS_URL: &str = "wss://juno-cloud-backend.fly.dev/ws";
pub const PRODUCTION_API_URL: &str = "https://juno-cloud-backend.fly.dev/api";
pub const PRODUCTION_HEALTH_URL: &str = "https://juno-cloud-backend.fly.dev/health";
pub const PRODUCTION_METRICS_URL: &str = "https://juno-cloud-backend.fly.dev/metrics";

/// Static denied commands list - only truly destructive commands
static DENIED_COMMANDS: &[&str] = &[
    "rm -rf /",
    "sudo rm -rf /",
    "format",
    "mkfs",
    "fdisk",
    "parted",
    "shutdown",
    "reboot",
    "halt",
    "poweroff",
    "init 0",
    "init 6",
    "chmod 777 /",
    "chown root /",
    "passwd root",
    ":(){ :|:& };:",
    ":(){:|:&};:",
    "dd if=/dev/zero of=/dev/sda",
    "> /etc/passwd",
    "> /etc/shadow",
];

/// Static allowed commands list - comprehensive command allowlist
static ALLOWED_COMMANDS: &[&str] = &[
    "text_query",
    "voice_query",
    "status_request",
    "screenshot",
    "system_command",
    "config_update",
    "file_operations",
    "web_browsing",
    "system_automation",
    "voice_transcription",
    "text_processing",
    "get_system_info",
    "get_capabilities",
    "heartbeat",
    "read_file",
    "write_file",
    "execute_script",
    "browser_automation",
    "desktop_automation",
    "anthropic_computer_use",
];

/// Lazy-initialized Vec<String> for denied commands
static DENIED_COMMANDS_VEC: LazyLock<Vec<String>> =
    LazyLock::new(|| DENIED_COMMANDS.iter().map(|&s| s.to_string()).collect());

/// Lazy-initialized Vec<String> for allowed commands
static ALLOWED_COMMANDS_VEC: LazyLock<Vec<String>> =
    LazyLock::new(|| ALLOWED_COMMANDS.iter().map(|&s| s.to_string()).collect());

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
    pub command_timeout: u64,    // seconds - generous timeout
    pub security_level: SecurityLevel,
    pub allowed_commands: Vec<String>, // All commands allowed by default
    pub denied_commands: Vec<String>,  // Only truly destructive commands
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SecurityLevel {
    Low,
    Medium,
    High, // Default for new configs (2026-09 security audit)
}

impl Default for CloudConfig {
    fn default() -> Self {
        Self {
            // Off by default: the hosted backend (juno-cloud-backend.fly.dev) is not running,
            // and an enabled connector reconnects in a loop at every launch. See LAC-3729.
            enabled: false,
            server_url: PRODUCTION_WS_URL.to_string(),
            device_id: None,
            device_name: format!("Juno-{}", gethostname::gethostname().to_string_lossy()),
            api_key: None,
            auto_connect: true,
            reconnect_interval: 30,
            heartbeat_interval: 60,
            command_timeout: 600,
            // New configs default to the most restrictive level; the stored level
            // is preserved on load and never downgraded (2026-09 security audit).
            security_level: SecurityLevel::High,
            allowed_commands: ALLOWED_COMMANDS_VEC.clone(), // Use lazy-initialized static
            denied_commands: DENIED_COMMANDS_VEC.clone(),   // Use lazy-initialized static
        }
    }
}

impl CloudConfig {
    /// Load configuration from centralized settings or create default.
    /// Attempts to load existing configuration, creates default if missing.
    /// Used by: Cloud service initialization and settings management.
    pub async fn load_from_centralized_settings(
        settings_manager: &crate::settings::manager::SettingsManager,
    ) -> Result<Self, CloudError> {
        match settings_manager.get_cloud_settings().await {
            Ok(cloud_settings) => {
                // Preserve the user's stored security level as-is. A forced
                // migration to SecurityLevel::Low used to run here on every
                // load; removed in the 2026-09 security audit.
                let config = Self::from_centralized_settings(&cloud_settings);
                info!("Loaded cloud configuration from centralized settings");
                Ok(config)
            }
            Err(e) => {
                info!(
                    "Failed to load cloud settings from centralized system ({}), creating default",
                    e
                );
                // No valid configuration found, create and save the default (disabled, High)
                let default_config = Self::default();
                default_config
                    .save_to_centralized_settings(settings_manager)
                    .await?;
                Ok(default_config)
            }
        }
    }

    /// Save configuration to centralized settings.
    /// Converts current configuration to CloudSettings and saves via SettingsManager.
    /// Used by: Cloud settings UI and configuration updates.
    pub async fn save_to_centralized_settings(
        &self,
        settings_manager: &crate::settings::manager::SettingsManager,
    ) -> Result<(), CloudError> {
        let cloud_settings = self.to_centralized_settings();

        settings_manager
            .set_cloud_settings(&cloud_settings)
            .await
            .map_err(|e| {
                CloudError::ConfigError(format!("Failed to save cloud settings: {}", e))
            })?;

        info!("Saved cloud configuration to centralized settings");
        Ok(())
    }

    /// Validate a cloud server URL: parsed with the `url` crate, secure
    /// schemes only (`wss` or `https`), a real host, and no embedded
    /// userinfo (security audit 2026-02-08, item #30).
    pub fn validate_server_url(raw: &str) -> Result<(), CloudError> {
        if raw.trim().is_empty() {
            return Err(CloudError::ConfigError(
                "Server URL cannot be empty".to_string(),
            ));
        }

        let parsed = url::Url::parse(raw)
            .map_err(|e| CloudError::ConfigError(format!("Invalid server URL: {}", e)))?;

        match parsed.scheme() {
            "wss" | "https" => {}
            other => {
                return Err(CloudError::ConfigError(format!(
                    "Server URL scheme '{}' is not allowed; only wss:// or https:// are accepted",
                    other
                )));
            }
        }

        if parsed.host_str().map(str::is_empty).unwrap_or(true) {
            return Err(CloudError::ConfigError(
                "Server URL must include a host".to_string(),
            ));
        }

        if !parsed.username().is_empty() || parsed.password().is_some() {
            return Err(CloudError::ConfigError(
                "Server URL must not contain embedded credentials".to_string(),
            ));
        }

        Ok(())
    }

    /// Validate the configuration
    pub fn validate(&self) -> Result<(), CloudError> {
        if self.enabled {
            // API key is optional for initial connection - backend handles device registration
            Self::validate_server_url(&self.server_url)?;

            // Validate production URL format
            if self.server_url == PRODUCTION_WS_URL {
                info!(
                    "✅ Using verified production backend: {}",
                    PRODUCTION_WS_URL
                );
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
                    Err(CloudError::ConfigError(format!(
                        "Backend health check failed with status: {}",
                        response.status()
                    )))
                }
            }
            Err(e) => Err(CloudError::ConfigError(format!(
                "Failed to connect to backend: {}",
                e
            ))),
        }
    }

    /// Update API key and save
    pub async fn set_api_key(
        &mut self,
        api_key: String,
        settings_manager: &crate::settings::manager::SettingsManager,
    ) -> Result<(), CloudError> {
        self.api_key = Some(api_key);
        self.save_to_centralized_settings(settings_manager).await
    }

    /// Enable cloud connectivity
    pub async fn enable(
        &mut self,
        settings_manager: &crate::settings::manager::SettingsManager,
    ) -> Result<(), CloudError> {
        self.enabled = true;
        self.validate()?;
        self.save_to_centralized_settings(settings_manager).await
    }

    /// Disable cloud connectivity
    pub async fn disable(
        &mut self,
        settings_manager: &crate::settings::manager::SettingsManager,
    ) -> Result<(), CloudError> {
        self.enabled = false;
        self.save_to_centralized_settings(settings_manager).await
    }

    /// Check if a command is allowed.
    ///
    /// The command is normalized (lowercased, whitespace runs collapsed) via
    /// the same tokenizer the shell validator uses before matching the denied
    /// patterns, so case/spacing tricks and `rm` flag permutations
    /// (`rm -r -f /`, `rm --recursive --force /`) cannot dodge the blocklist
    /// (security audit 2026-02-08, item #25).
    pub fn is_command_allowed(&self, command: &str) -> bool {
        if let Some(denied_cmd) = Self::matched_denied_pattern(command) {
            log::warn!(
                "🚫 Command '{}' blocked due to destructive pattern: '{}'",
                command,
                denied_cmd
            );
            return false;
        }

        // All security levels now behave the same - maximally permissive
        // Allow all commands except those in the denied list
        log::info!(
            "✅ Command '{}' allowed (maximally permissive mode)",
            command
        );
        true
    }

    /// Check if a command is considered safe - now almost everything is safe
    #[allow(dead_code)]
    fn is_safe_command(&self, command: &str) -> bool {
        Self::matched_denied_pattern(command).is_none()
    }

    /// Return the denied pattern the command matches, if any, after
    /// normalization. Shares the shell validator's normalization and `rm`
    /// flag-permutation tokenizer (security audit 2026-02-08, item #25).
    fn matched_denied_pattern(command: &str) -> Option<String> {
        let normalized = crate::commands::shell::normalize_command(command);

        for &denied_cmd in DENIED_COMMANDS.iter() {
            if normalized.contains(denied_cmd) {
                return Some(denied_cmd.to_string());
            }
        }

        if crate::commands::shell::is_catastrophic_rm(&normalized) {
            return Some("recursive forced rm of /".to_string());
        }

        None
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
                _ => SecurityLevel::High, // Unknown values fail closed to the strictest level
            },
            // Set default values for fields not in CloudSettings (use static references)
            allowed_commands: ALLOWED_COMMANDS_VEC.clone(),
            denied_commands: DENIED_COMMANDS_VEC.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_is_disabled_and_high_security() {
        let config = CloudConfig::default();
        assert!(!config.enabled);
        assert!(matches!(config.security_level, SecurityLevel::High));
    }

    #[test]
    fn stored_security_level_is_preserved_not_downgraded() {
        let settings = CloudSettings {
            security_level: "medium".to_string(),
            ..Default::default()
        };
        let config = CloudConfig::from_centralized_settings(&settings);
        assert!(matches!(config.security_level, SecurityLevel::Medium));

        let settings = CloudSettings {
            security_level: "high".to_string(),
            ..Default::default()
        };
        let config = CloudConfig::from_centralized_settings(&settings);
        assert!(matches!(config.security_level, SecurityLevel::High));
    }

    #[test]
    fn unknown_security_level_fails_closed_to_high() {
        let settings = CloudSettings {
            security_level: "garbage".to_string(),
            ..Default::default()
        };
        let config = CloudConfig::from_centralized_settings(&settings);
        assert!(matches!(config.security_level, SecurityLevel::High));
    }

    // --- Denied-command matching is normalized, not raw substring (#25) ---

    #[test]
    fn denied_exact_patterns_blocked() {
        let config = CloudConfig::default();
        assert!(!config.is_command_allowed("rm -rf /"));
        assert!(!config.is_command_allowed("sudo rm -rf /"));
        assert!(!config.is_command_allowed("dd if=/dev/zero of=/dev/sda"));
    }

    #[test]
    fn denied_rm_flag_permutations_blocked() {
        let config = CloudConfig::default();
        assert!(!config.is_command_allowed("rm -r -f /"));
        assert!(!config.is_command_allowed("rm -f -r /"));
        assert!(!config.is_command_allowed("rm -fr /"));
        assert!(!config.is_command_allowed("rm --recursive --force /"));
        assert!(!config.is_command_allowed("rm -Rf /*"));
    }

    #[test]
    fn denied_case_and_whitespace_variants_blocked() {
        let config = CloudConfig::default();
        assert!(!config.is_command_allowed("RM -RF /"));
        assert!(!config.is_command_allowed("rm  -rf   /"));
        assert!(!config.is_command_allowed("rm\t-r\t-f\t/"));
        assert!(!config.is_command_allowed("SHUTDOWN now"));
    }

    #[test]
    fn benign_commands_still_allowed() {
        let config = CloudConfig::default();
        assert!(config.is_command_allowed("ls -la"));
        assert!(config.is_command_allowed("git status"));
        assert!(config.is_command_allowed("rm -rf ./build"));
    }

    // --- Server URL validation (#30) ---

    #[test]
    fn server_url_accepts_wss_and_https() {
        assert!(CloudConfig::validate_server_url(PRODUCTION_WS_URL).is_ok());
        assert!(CloudConfig::validate_server_url("https://example.com/api").is_ok());
    }

    #[test]
    fn server_url_rejects_insecure_and_non_web_schemes() {
        assert!(CloudConfig::validate_server_url("ws://example.com/ws").is_err());
        assert!(CloudConfig::validate_server_url("http://example.com/ws").is_err());
        assert!(CloudConfig::validate_server_url("file:///etc/passwd").is_err());
        assert!(CloudConfig::validate_server_url("javascript:alert(1)").is_err());
    }

    #[test]
    fn server_url_rejects_empty_hostless_and_userinfo() {
        assert!(CloudConfig::validate_server_url("").is_err());
        assert!(CloudConfig::validate_server_url("   ").is_err());
        assert!(CloudConfig::validate_server_url("not a url").is_err());
        assert!(CloudConfig::validate_server_url("wss://user:pass@example.com/ws").is_err());
        assert!(CloudConfig::validate_server_url("wss://user@example.com/ws").is_err());
    }
}
