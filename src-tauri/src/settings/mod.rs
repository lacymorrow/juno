//! # Centralized Settings Module
//!
//! Unified settings schema and types for the entire Juno application.
//! Replaces 12+ scattered JSON configuration files with a single, type-safe structure.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use crate::constants::settings::defaults;
use crate::agent::providers::config::{default_provider_entries, DEFAULT_ACTIVE_PROVIDER};
use crate::constants::ui;

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
    /// Prompt configuration and templates
    pub prompts: PromptSettings,
    /// Onboarding completion status
    pub onboarding: OnboardingSettings,
    /// Application autostart setting
    pub autostart_enabled: bool,
    /// CLI configuration settings
    pub cli: CLISettings,
    /// Voice transcription configuration
    pub voice_transcription: VoiceTranscriptionSettings,
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
    pub show_voice_indicator: bool,
    pub enable_animations: bool,
    pub auto_hide: bool,
    pub auto_hide_delay: u32,
    pub opacity: f32,
    pub bar_appearance: String,
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
    pub dictation_trigger_mode: String, // "tap" or "hold"
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
    pub smooth_mouse_movement: bool,
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

/// Prompt configuration and templates
/// Replaces: prompt_config.json
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptSettings {
    /// Active prompt templates by type
    pub active_prompts: HashMap<String, String>,
    /// Custom prompt overrides
    pub custom_prompts: HashMap<String, PromptTemplate>,
    /// Global variables available to all prompts
    pub global_variables: HashMap<String, String>,
    /// Whether to enable prompt customization in UI
    pub allow_customization: bool,
}

/// Prompt template configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptTemplate {
    /// Unique identifier for the prompt
    pub id: String,
    /// Human-readable name
    pub name: String,
    /// Description of the prompt's purpose
    pub description: String,
    /// The actual prompt content with possible placeholders
    pub content: String,
    /// Variables that can be substituted in the content
    pub variables: Vec<String>,
    /// Tags for categorization and filtering
    pub tags: Vec<String>,
    /// Version for tracking changes
    pub version: String,
    /// Whether this prompt is user-customizable
    pub customizable: bool,
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

/// CLI configuration settings
/// Replaces: CLI config.json
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CLISettings {
    /// Enable/disable CLI logging
    pub logging_enabled: bool,
    /// CLI log level
    pub log_level: String,
    /// Maximum number of command history entries to keep
    pub max_history_entries: u32,
    /// Enable colored output in CLI
    pub colored_output: bool,
    /// CLI timeout for commands (seconds)
    pub command_timeout: u64,
    /// Enable CLI autocomplete
    pub autocomplete_enabled: bool,
}

/// Voice transcription configuration settings
/// Replaces: voice-transcription/config.json
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoiceTranscriptionSettings {
    /// Path to the Whisper model file
    pub model_path: String,
    /// Sample rate for audio recording (Hz)
    pub sample_rate: u32,
    /// Number of channels in the audio recording
    pub channels: u16,
    /// Buffer duration for partial transcriptions (ms)
    pub buffer_duration_ms: u64,
    /// Interval between partial transcriptions (ms)
    pub partial_interval_ms: u64,
    /// Enable partial transcription results
    pub enable_partial_transcription: bool,
    /// Enable playback of the transcription
    pub enable_playback: bool,
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
            prompts: PromptSettings::default(),
            onboarding: OnboardingSettings::default(),
            autostart_enabled: defaults::AUTOSTART_ENABLED,
            cli: CLISettings::default(),
            voice_transcription: VoiceTranscriptionSettings::default(),
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
            show_voice_indicator: true,
            enable_animations: true,
            auto_hide: false,
            auto_hide_delay: crate::constants::timeouts::UI_NOTIFICATION_DISPLAY_MS as u32,
            opacity: 0.95,
            bar_appearance: ui::bar_appearances::FLOATING.to_string(),
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
            active_provider: DEFAULT_ACTIVE_PROVIDER.to_string(),
            providers: default_provider_entries()
                .into_iter()
                .map(|p| ProviderConfig {
                    id: p.id,
                    api_key: p.api_key,
                    model: p.model,
                    max_tokens: p.max_tokens,
                    temperature: p.temperature,
                    system_prompt: p.system_prompt,
                })
                .collect(),
        }
    }
}

impl Default for CloudSettings {
    fn default() -> Self {
        Self {
            enabled: defaults::CLOUD_ENABLED,
            server_url: crate::constants::api::endpoints::CLOUD_SERVER_URL.to_string(),
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
            dictation_trigger_mode: "hold".to_string(),
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
            smooth_mouse_movement: true,
        }
    }
}

impl Default for PromptSettings {
    fn default() -> Self {
        Self {
            active_prompts: HashMap::new(),
            custom_prompts: HashMap::new(),
            global_variables: HashMap::new(),
            allow_customization: true,
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

impl Default for CLISettings {
    fn default() -> Self {
        Self {
            logging_enabled: true,
            log_level: "info".to_string(),
            max_history_entries: 100,
            colored_output: true,
            command_timeout: 30,
            autocomplete_enabled: true,
        }
    }
}

impl Default for VoiceTranscriptionSettings {
    fn default() -> Self {
        Self {
            model_path: "models/ggml-tiny.en.bin".to_string(),
            sample_rate: 16000,
            channels: 1,
            buffer_duration_ms: 1500,
            partial_interval_ms: 500,
            enable_partial_transcription: true,
            enable_playback: true,
        }
    }
}
