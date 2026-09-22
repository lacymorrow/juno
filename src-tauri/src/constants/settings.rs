//! # Settings Constants
//!
//! Centralized constants for all application settings to eliminate magic strings.
//! Used by: Settings manager, individual command modules, frontend integration.

/// Central settings store file name
pub const SETTINGS_STORE_FILE: &str = "app_settings.json";

/// Top-level settings keys in the unified store
pub mod store_keys {
    pub const KEYBOARD_SHORTCUTS: &str = "keyboard_shortcuts";
    pub const FLOATING_BAR: &str = "floating_bar";
    pub const AGENT: &str = "agent";
    pub const PROVIDERS: &str = "providers";
    pub const CLOUD: &str = "cloud";
    pub const AUDIO: &str = "audio";
    pub const TOOLS: &str = "tools";
    pub const PROMPTS: &str = "prompts";
    pub const ONBOARDING: &str = "onboarding";
    pub const AUTOSTART_ENABLED: &str = "autostart_enabled";
    pub const ADVANCED_SETTINGS_ENABLED: &str = "advanced_settings_enabled";
    pub const BACKGROUND_MODE: &str = "background_mode";
    pub const MOUSE_CONTROL: &str = "mouse_control";
    pub const DOCK_ICON_VISIBLE: &str = "dock_icon_visible";
    pub const SHOW_TRAY_ICON: &str = "show_tray_icon";
    pub const SHOW_GLOW_BORDER: &str = "show_glow_border";
    pub const CLI: &str = "cli";
    /// Experimental. Reuse one long-lived `claude` process per conversation instead
    /// of spawning one per query. Off unless explicitly set.
    /// See docs/plans/cli-persistent-session-spike.md
    pub const CLI_PERSISTENT_SESSION_ENABLED: &str = "cli_persistent_session_enabled";
    pub const VOICE_TRANSCRIPTION: &str = "voice_transcription";
    pub const TRIGGERS: &str = "triggers";
}

/// Keyboard shortcut setting keys
pub mod keyboard_keys {
    pub const AGENT_MODE: &str = "agent_mode";
    pub const DICTATION_INPUT: &str = "dictation_input";
    pub const STOP_CURRENT_TASK: &str = "stop_current_task";
    pub const OPEN_SETTINGS: &str = "open_settings";
}

/// Onboarding setting keys
pub mod onboarding_keys {
    pub const COMPLETED: &str = "completed";
    pub const COMPLETED_AT: &str = "completed_at";
    pub const SKIPPED: &str = "skipped";
    pub const SKIP_COUNT: &str = "skip_count";
}

/// Agent setting keys
pub mod agent_keys {
    pub const TRIGGER_MODE: &str = "trigger_mode";
    pub const MODE: &str = "mode";
    pub const EXECUTION_MODE: &str = "execution_mode";
}

/// Cloud setting keys
pub mod cloud_keys {
    pub const ENABLED: &str = "enabled";
    pub const SERVER_URL: &str = "server_url";
    pub const DEVICE_ID: &str = "device_id";
    pub const DEVICE_NAME: &str = "device_name";
    pub const API_KEY: &str = "api_key";
    pub const AUTO_CONNECT: &str = "auto_connect";
    pub const RECONNECT_INTERVAL: &str = "reconnect_interval";
    pub const HEARTBEAT_INTERVAL: &str = "heartbeat_interval";
    pub const COMMAND_TIMEOUT: &str = "command_timeout";
    pub const SECURITY_LEVEL: &str = "security_level";
}

/// Audio/voice setting keys
pub mod audio_keys {
    pub const TTS_PROVIDER: &str = "tts_provider";
    pub const SOUND_ENABLED: &str = "sound_enabled";
    pub const DICTATION_CLIPBOARD_ENABLED: &str = "dictation_clipboard_enabled";
    pub const ALWAYS_LISTENING_ACTIVE: &str = "always_listening_active";
    pub const ALWAYS_LISTENING_SENSITIVITY: &str = "always_listening_sensitivity";
    pub const ALWAYS_LISTENING_WAKE_WORDS: &str = "always_listening_wake_words";
    pub const PERFORMANCE_MONITORING_ENABLED: &str = "performance_monitoring_enabled";
}

/// Tool configuration keys
pub mod tool_keys {
    pub const TOOLS: &str = "tools";
    pub const CATEGORY_ENABLED: &str = "category_enabled";
    pub const MCP_SERVERS: &str = "mcp_servers";
}

/// Provider configuration keys
pub mod provider_keys {
    pub const ACTIVE_PROVIDER: &str = "active_provider";
    pub const PROVIDERS: &str = "providers";
    pub const API_KEY: &str = "api_key";
    pub const MODEL: &str = "model";
    pub const MAX_TOKENS: &str = "max_tokens";
    pub const TEMPERATURE: &str = "temperature";
    pub const SYSTEM_PROMPT: &str = "system_prompt";
}

/// Floating bar configuration keys
pub mod floating_bar_keys {
    pub const CONFIG: &str = "config";
    pub const UI_STATE: &str = "ui_state";
    pub const POSITION: &str = "position";
    pub const SIZE: &str = "size";
    pub const VISIBILITY: &str = "visibility";
}

/// Prompt configuration keys
pub mod prompt_keys {
    pub const ACTIVE_PROMPTS: &str = "active_prompts";
    pub const CUSTOM_PROMPTS: &str = "custom_prompts";
    pub const GLOBAL_VARIABLES: &str = "global_variables";
    pub const ALLOW_CUSTOMIZATION: &str = "allow_customization";
}

/// Settings validation constants
pub mod validation {
    pub const MIN_SENSITIVITY: f32 = 0.0;
    pub const MAX_SENSITIVITY: f32 = 1.0;
    pub const MIN_TEMPERATURE: f32 = 0.0;
    pub const MAX_TEMPERATURE: f32 = 2.0;
    pub const MIN_MAX_TOKENS: u32 = 1;
    pub const MAX_MAX_TOKENS: u32 = 100000;
    pub const MIN_HEARTBEAT_INTERVAL: u64 = 10; // seconds
    pub const MAX_HEARTBEAT_INTERVAL: u64 = 300; // seconds
}

/// Default values for settings
pub mod defaults {
    pub const TTS_PROVIDER: &str = "system";
    pub const SOUND_ENABLED: bool = true;
    pub const DICTATION_CLIPBOARD_ENABLED: bool = true;
    pub const ALWAYS_LISTENING_ACTIVE: bool = false;
    pub const ALWAYS_LISTENING_SENSITIVITY: f32 = 0.5;
    pub const PERFORMANCE_MONITORING_ENABLED: bool = true;
    pub const AGENT_EXECUTION_MODE: &str = "multi";
    pub const AGENT_TRIGGER_MODE: &str = "tap";
    pub const CLOUD_ENABLED: bool = false;
    pub const AUTO_CONNECT: bool = false;
    pub const AUTOSTART_ENABLED: bool = false;
    /// The settings window shows the trimmed "basic" set until the user opts in.
    pub const ADVANCED_SETTINGS_ENABLED: bool = false;
    pub const ONBOARDING_COMPLETED: bool = false;
    /// The agent works without taking the cursor or the frontmost app from the
    /// user. On by default: a person can keep typing while Juno works.
    pub const BACKGROUND_MODE: bool = true;
    /// Ask before driving the physical mouse; the other value is "always".
    pub const MOUSE_CONTROL: &str = "ask";
    pub const MOUSE_CONTROL_ALWAYS: &str = "always";
    /// Juno starts as a normal Dock app; the menu-bar-only mode is opt-in.
    pub const DOCK_ICON_VISIBLE: bool = true;
    /// Juno shows its menu-bar (tray) icon by default; hiding it is opt-in.
    pub const SHOW_TRAY_ICON: bool = true;

    pub fn background_mode() -> bool {
        BACKGROUND_MODE
    }
    pub fn mouse_control() -> String {
        MOUSE_CONTROL.to_string()
    }
    pub fn dock_icon_visible() -> bool {
        DOCK_ICON_VISIBLE
    }
    pub fn show_tray_icon() -> bool {
        SHOW_TRAY_ICON
    }

    pub fn advanced_settings_enabled() -> bool {
        ADVANCED_SETTINGS_ENABLED
    }
    /// The floating bar follows the cursor to whichever display it is on, so the
    /// user never hunts for it. On by default; older stores lack the key.
    pub const FOLLOW_CURSOR_DISPLAY: bool = true;
    pub fn follow_cursor_display() -> bool {
        FOLLOW_CURSOR_DISPLAY
    }
    /// The floating bar wears its glowing activity border by default. Off makes
    /// the bar show no flame border in any state; older stores lack the key.
    pub const SHOW_GLOW_BORDER: bool = true;
    pub fn show_glow_border() -> bool {
        SHOW_GLOW_BORDER
    }

    // Default keyboard shortcuts (cross-platform)
    #[cfg(target_os = "macos")]
    pub const AGENT_MODE: &str = "Option+D";
    #[cfg(not(target_os = "macos"))]
    pub const AGENT_MODE: &str = "Alt+D";

    #[cfg(target_os = "macos")]
    pub const DICTATION_INPUT: &str = "Option+Space";
    #[cfg(not(target_os = "macos"))]
    pub const DICTATION_INPUT: &str = "Alt+Space";

    // Fixed, not a default. Escape is the universal cancel key and Cmd+Comma
    // is the macOS convention for settings, so there is no version of either
    // that a person is better off rebinding. Nothing writes these any more:
    // the registration and dispatch paths read these constants directly.
    pub const STOP_CURRENT_TASK: &str = "Escape";

    #[cfg(target_os = "macos")]
    pub const OPEN_SETTINGS: &str = "Cmd+Comma";
    #[cfg(not(target_os = "macos"))]
    pub const OPEN_SETTINGS: &str = "Ctrl+Comma";
}

/// Command names for settings operations (to prevent duplication)
pub mod commands {
    pub const GET_ALL_SETTINGS: &str = "get_all_settings";
    pub const UPDATE_SETTINGS: &str = "update_settings";
    pub const RESET_SETTINGS: &str = "reset_centralized_settings";
    pub const EXPORT_SETTINGS: &str = "export_settings";
    pub const IMPORT_SETTINGS: &str = "import_settings";
}

/// Event names for settings changes (for reactivity)
pub mod events {
    pub const SETTINGS_CHANGED: &str = "settings_changed";
    pub const KEYBOARD_SHORTCUTS_CHANGED: &str = "keyboard_shortcuts_changed";
    pub const AGENT_SETTINGS_CHANGED: &str = "agent_settings_changed";
    pub const PROVIDER_SETTINGS_CHANGED: &str = "provider_settings_changed";
    pub const CLOUD_SETTINGS_CHANGED: &str = "cloud_settings_changed";
    pub const AUDIO_SETTINGS_CHANGED: &str = "audio_settings_changed";
    pub const TOOL_SETTINGS_CHANGED: &str = "tool_settings_changed";
    pub const FLOATING_BAR_SETTINGS_CHANGED: &str = "floating_bar_settings_changed";
    pub const PROMPT_SETTINGS_CHANGED: &str = "prompt_settings_changed";
    pub const CLI_SETTINGS_CHANGED: &str = "cli_settings_changed";
    pub const VOICE_TRANSCRIPTION_SETTINGS_CHANGED: &str = "voice_transcription_settings_changed";
}
