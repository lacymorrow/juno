//! # Centralized Settings Module
//!
//! Unified settings schema and types for the entire Juno application.
//! Replaces 12+ scattered JSON configuration files with a single, type-safe structure.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use crate::constants::settings::defaults;

pub mod manager;

/// Main application settings structure
/// This replaces all individual JSON config files with a single, centralized structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    /// Keyboard shortcuts configuration
    pub keyboard_shortcuts: KeyboardShortcuts,
    /// Floating bar UI settings
    pub floating_bar: FloatingBarSettings,
    /// Agent behavior and execution settings
    pub agent: AgentSettings,
    /// AI provider configurations
    pub providers: ProviderSettings,
    /// Cloud connectivity settings
    pub cloud: CloudSettings,
    /// Audio and voice settings
    pub audio: AudioSettings,
    /// Tool enable/disable configurations
    pub tools: ToolSettings,
    /// Onboarding completion status
    pub onboarding: OnboardingSettings,
    /// Application autostart setting
    pub autostart_enabled: bool,
}

/// Keyboard shortcut configuration
/// Replaces: keyboard_shortcuts.json
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyboardShortcuts {
    pub agent_mode_toggle: String,
    pub dictation_input: String,
    pub stop_current_task: String,
    pub open_settings: String,
}

/// Floating bar UI configuration
/// Replaces: floating_bar_config.json
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FloatingBarSettings {
    pub ui_state: String,
    pub position: Option<FloatingBarPosition>,
    pub size: Option<FloatingBarSize>,
    pub visibility: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FloatingBarPosition {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FloatingBarSize {
    pub width: f64,
    pub height: f64,
}

/// Agent behavior and execution settings
/// Replaces: agent_settings.json
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSettings {
    pub trigger_mode: String, // "tap" or "hold"
    pub execution_mode: String, // "single" or "multi"
}

/// AI provider configurations
/// Replaces: provider_config.json
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderSettings {
    pub active_provider: String,
    pub providers: Vec<ProviderConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub id: String,
    pub api_key: Option<String>,
    pub model: Option<String>,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
    pub system_prompt: Option<String>,
}

/// Cloud connectivity settings
/// Replaces: cloud_config.json
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudSettings {
    pub enabled: bool,
    pub server_url: String,
    pub device_id: Option<String>,
    pub device_name: String,
    pub api_key: Option<String>,
    pub auto_connect: bool,
    pub reconnect_interval: u64,
    pub heartbeat_interval: u64,
    pub command_timeout: u64,
    pub security_level: String,
}

/// Audio and voice settings
/// Replaces parts of multiple files
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioSettings {
    pub tts_provider: String,
    pub sound_enabled: bool,
    pub dictation_clipboard_enabled: bool,
    pub always_listening_active: bool,
    pub always_listening_sensitivity: f32,
    pub always_listening_wake_words: Vec<String>,
    pub performance_monitoring_enabled: bool,
}

/// Tool enable/disable configurations
/// Replaces: tool_config.json
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSettings {
    pub tools: HashMap<String, ToolConfig>,
    pub category_enabled: HashMap<String, bool>,
    pub mcp_servers: Vec<MCPServerConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolConfig {
    pub name: String,
    pub category: String,
    pub enabled: bool,
    pub description: Option<String>,
    pub required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MCPServerConfig {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub command: String,
    pub args: Vec<String>,
    pub working_directory: Option<String>,
    pub environment_variables: HashMap<String, String>,
    pub enabled: bool,
    pub auto_start: bool,
    pub timeout_seconds: u64,
    pub max_retries: u32,
}

/// Onboarding completion status
/// Replaces: onboarding.json
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnboardingSettings {
    pub completed: bool,
    pub completed_at: Option<String>,
    pub skipped: bool,
    pub skip_count: u32,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            keyboard_shortcuts: KeyboardShortcuts::default(),
            floating_bar: FloatingBarSettings::default(),
            agent: AgentSettings::default(),
            providers: ProviderSettings::default(),
            cloud: CloudSettings::default(),
            audio: AudioSettings::default(),
            tools: ToolSettings::default(),
            onboarding: OnboardingSettings::default(),
            autostart_enabled: defaults::AUTOSTART_ENABLED,
        }
    }
}

impl Default for KeyboardShortcuts {
    fn default() -> Self {
        Self {
            agent_mode_toggle: defaults::AGENT_MODE_TOGGLE.to_string(),
            dictation_input: defaults::DICTATION_INPUT.to_string(),
            stop_current_task: defaults::STOP_CURRENT_TASK.to_string(),
            open_settings: defaults::OPEN_SETTINGS.to_string(),
        }
    }
}

impl Default for FloatingBarSettings {
    fn default() -> Self {
        Self {
            ui_state: "compact".to_string(),
            position: None,
            size: None,
            visibility: true,
        }
    }
}

impl Default for AgentSettings {
    fn default() -> Self {
        Self {
            trigger_mode: defaults::AGENT_TRIGGER_MODE.to_string(),
            execution_mode: defaults::AGENT_MODE.to_string(),
        }
    }
}

impl Default for ProviderSettings {
    fn default() -> Self {
        Self {
            active_provider: "anthropic".to_string(),
            providers: vec![
                ProviderConfig {
                    id: "anthropic".to_string(),
                    api_key: None,
                    model: Some("claude-3-5-sonnet-20241022".to_string()),
                    max_tokens: Some(4096),
                    temperature: Some(0.7),
                    system_prompt: None,
                },
                ProviderConfig {
                    id: "openai".to_string(),
                    api_key: None,
                    model: Some("gpt-4".to_string()),
                    max_tokens: Some(4096),
                    temperature: Some(0.7),
                    system_prompt: None,
                },
            ],
        }
    }
}

impl Default for CloudSettings {
    fn default() -> Self {
        Self {
            enabled: defaults::CLOUD_ENABLED,
            server_url: "wss://localhost:8080".to_string(),
            device_id: None,
            device_name: "Juno-Desktop".to_string(),
            api_key: None,
            auto_connect: defaults::AUTO_CONNECT,
            reconnect_interval: 30,
            heartbeat_interval: 60,
            command_timeout: 30,
            security_level: "low".to_string(),
        }
    }
}

impl Default for AudioSettings {
    fn default() -> Self {
        Self {
            tts_provider: defaults::TTS_PROVIDER.to_string(),
            sound_enabled: defaults::SOUND_ENABLED,
            dictation_clipboard_enabled: defaults::DICTATION_CLIPBOARD_ENABLED,
            always_listening_active: defaults::ALWAYS_LISTENING_ACTIVE,
            always_listening_sensitivity: defaults::ALWAYS_LISTENING_SENSITIVITY,
            always_listening_wake_words: vec!["hey juno".to_string(), "computer".to_string()],
            performance_monitoring_enabled: defaults::PERFORMANCE_MONITORING_ENABLED,
        }
    }
}

impl Default for ToolSettings {
    fn default() -> Self {
        Self {
            tools: HashMap::new(),
            category_enabled: HashMap::new(),
            mcp_servers: Vec::new(),
        }
    }
}

impl Default for OnboardingSettings {
    fn default() -> Self {
        Self {
            completed: defaults::ONBOARDING_COMPLETED,
            completed_at: None,
            skipped: false,
            skip_count: 0,
        }
    }
}
