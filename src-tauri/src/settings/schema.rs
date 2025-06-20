//! # Settings Schema
//!
//! Clean, simple settings schema for the Juno app.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Main application settings structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub keyboard_shortcuts: HashMap<String, KeyboardShortcut>,
    pub floating_bar: FloatingBarConfig,
    pub agent: AgentSettings,
    pub providers: ProviderConfig,
    pub cloud: CloudConfig,
    pub audio: AudioSettings,
    pub autostart_enabled: bool,
    pub onboarding: OnboardingState,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            keyboard_shortcuts: HashMap::new(),
            floating_bar: FloatingBarConfig::default(),
            agent: AgentSettings::default(),
            providers: ProviderConfig::default(),
            cloud: CloudConfig::default(),
            audio: AudioSettings::default(),
            autostart_enabled: false,
            onboarding: OnboardingState::default(),
        }
    }
}

/// Individual keyboard shortcut
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyboardShortcut {
    pub key: String,
    pub enabled: bool,
}

impl Default for KeyboardShortcut {
    fn default() -> Self {
        Self {
            key: "".to_string(),
            enabled: true,
        }
    }
}

/// Floating bar configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FloatingBarConfig {
    pub show_voice_indicator: bool,
    pub enable_animations: bool,
    pub auto_hide: bool,
    pub opacity: f32,
}

impl Default for FloatingBarConfig {
    fn default() -> Self {
        Self {
            show_voice_indicator: true,
            enable_animations: true,
            auto_hide: false,
            opacity: 0.95,
        }
    }
}

/// Agent-specific settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSettings {
    pub mode: String,
    pub trigger_mode: String,
    pub default_provider: String,
    pub max_execution_time: u64,
    pub enable_memory: bool,
}

impl Default for AgentSettings {
    fn default() -> Self {
        Self {
            mode: "multi".to_string(),
            trigger_mode: "tap".to_string(),
            default_provider: "anthropic".to_string(),
            max_execution_time: 300,
            enable_memory: true,
        }
    }
}

/// AI Provider configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub active_provider: String,
    pub providers: Vec<ProviderInfo>,
}

impl Default for ProviderConfig {
    fn default() -> Self {
        Self {
            active_provider: "anthropic".to_string(),
            providers: vec![
                ProviderInfo {
                    id: "anthropic".to_string(),
                    name: "Anthropic Claude".to_string(),
                    api_key: None,
                    model: "claude-3-5-sonnet-20241022".to_string(),
                    enabled: true,
                },
            ],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderInfo {
    pub id: String,
    pub name: String,
    pub api_key: Option<String>,
    pub model: String,
    pub enabled: bool,
}

/// Cloud connectivity settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudConfig {
    pub enabled: bool,
    pub server_url: String,
    pub device_name: String,
    pub device_id: Option<String>,
    pub api_key: Option<String>,
    pub auto_connect: bool,
    pub heartbeat_interval: u64,
    pub reconnect_interval: u64,
    pub security_level: String,
}

impl Default for CloudConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            server_url: "wss://juno-cloud-backend.fly.dev/ws".to_string(),
            device_name: format!("Juno-{}", gethostname::gethostname().to_string_lossy()),
            device_id: None,
            api_key: None,
            auto_connect: true,
            heartbeat_interval: 30,
            reconnect_interval: 5,
            security_level: "development".to_string(),
        }
    }
}

impl CloudConfig {
    /// Validate cloud configuration
    pub fn validate(&self) -> Result<(), String> {
        if self.server_url.is_empty() {
            return Err("Server URL cannot be empty".to_string());
        }

        if self.device_name.is_empty() {
            return Err("Device name cannot be empty".to_string());
        }

        if self.heartbeat_interval == 0 {
            return Err("Heartbeat interval must be greater than 0".to_string());
        }

        Ok(())
    }
}

/// Audio settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioSettings {
    pub sound_enabled: bool,
    pub input_volume: f32,
    pub output_volume: f32,
}

impl Default for AudioSettings {
    fn default() -> Self {
        Self {
            sound_enabled: true,
            input_volume: 0.8,
            output_volume: 0.8,
        }
    }
}

/// Onboarding state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnboardingState {
    pub completed: bool,
    pub current_step: u32,
}

impl Default for OnboardingState {
    fn default() -> Self {
        Self {
            completed: false,
            current_step: 0,
        }
    }
}

/// Settings update event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettingsUpdateEvent {
    pub section: String,
    pub settings: AppSettings,
}
