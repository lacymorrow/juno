//! # Settings Constants
//!
//! Centralized constants for all application settings to eliminate magic strings.
//! Used by: Settings manager, individual command modules, frontend integration.

/// Central settings store file name
pub const SETTINGS_STORE_FILE: &str = "app_settings.json";

/// Every other Tauri store file Juno writes, in one list so two features can
/// never pick the same file name by accident. Settings live in
/// `SETTINGS_STORE_FILE`; nothing else should hold settings.
pub mod store_files {
    pub const MEMORY: &str = "memory.json";
    pub const SCHEDULED_AUTOMATIONS: &str = "scheduled_automations.json";
    pub const BAR_POSITION: &str = "bar_position.json";
    pub const ONBOARDING_ANALYTICS: &str = "onboarding_analytics.json";
}

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
    pub const PERMISSION_MODE: &str = "permission_mode";
    pub const DOCK_ICON_VISIBLE: &str = "dock_icon_visible";
    pub const SHOW_TRAY_ICON: &str = "show_tray_icon";
    pub const SHOW_GLOW_BORDER: &str = "show_glow_border";
    pub const AGENT_CURSOR_COLOR: &str = "agent_cursor_color";
    pub const CLI: &str = "cli";
    /// Reuse one long-lived `claude` process per conversation instead of spawning
    /// one per query. On unless explicitly set to false; see
    /// `defaults::CLI_PERSISTENT_SESSION_ENABLED`.
    /// See docs/plans/cli-persistent-session-spike.md
    pub const CLI_PERSISTENT_SESSION_ENABLED: &str = "cli_persistent_session_enabled";
    /// Ask before Juno sends (LAC-4058). Default on. Routes the Claude CLI's
    /// permission prompts into Juno's approval sheet instead of
    /// dangerously-skip-permissions; off restores the old behaviour.
    pub const CLI_ASK_BEFORE_SEND_ENABLED: &str = "cli_ask_before_send_enabled";
    /// Beta, off unless explicitly set. Smart routing: a quick classifier call
    /// picks the model for each request and whether it needs the computer.
    /// See agent/router.rs
    pub const SMART_ROUTING_ENABLED: &str = "smart_routing_enabled";
    pub const VOICE_TRANSCRIPTION: &str = "voice_transcription";
    pub const TRIGGERS: &str = "triggers";
    /// Auto-update behaviour: the auto-check flag and the channel.
    pub const UPDATES: &str = "updates";
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
    pub const DICTATION_INSERTION_MODE: &str = "dictation_insertion_mode";
    pub const DICTATION_COPY_TO_CLIPBOARD: &str = "dictation_copy_to_clipboard";
    pub const ALWAYS_LISTENING_ACTIVE: &str = "always_listening_active";
    pub const ALWAYS_LISTENING_SENSITIVITY: &str = "always_listening_sensitivity";
    pub const ALWAYS_LISTENING_WAKE_WORDS: &str = "always_listening_wake_words";
    pub const PERFORMANCE_MONITORING_ENABLED: &str = "performance_monitoring_enabled";
}

/// How dictation delivers the transcript into the focused app
pub mod dictation_insertion_modes {
    /// Copy to the pasteboard and synthesize Cmd+V. Most compatible.
    pub const PASTE: &str = "paste";
    /// Post unicode keyboard events directly; never touches the pasteboard.
    pub const CLIPBOARD_FREE: &str = "clipboard_free";
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
    /// The persistent Claude CLI session is on for anyone who has not turned it
    /// off. It saves 1.6-3.1s of process start on every follow-up, which is most
    /// of the gap between a key release and the first spoken word on this
    /// provider. An explicit `false` in the store still wins.
    pub const CLI_PERSISTENT_SESSION_ENABLED: bool = true;
    pub const DICTATION_CLIPBOARD_ENABLED: bool = true;
    /// Paste stays the default while clipboard-free proves itself in the field.
    /// Literal (not a re-export of `dictation_insertion_modes::PASTE`) because
    /// the TS constants generator only understands string literals; a test
    /// keeps the two in sync.
    pub const DICTATION_INSERTION_MODE: &str = "paste";

    pub fn dictation_insertion_mode() -> String {
        DICTATION_INSERTION_MODE.to_string()
    }
    pub const ALWAYS_LISTENING_ACTIVE: bool = false;
    pub const ALWAYS_LISTENING_SENSITIVITY: f32 = 0.5;
    pub const PERFORMANCE_MONITORING_ENABLED: bool = true;
    pub const AGENT_EXECUTION_MODE: &str = "single";
    pub const AGENT_TRIGGER_MODE: &str = "tap";
    pub const CLOUD_ENABLED: bool = false;
    pub const AUTO_CONNECT: bool = false;
    /// Seconds a cloud-dispatched command may run before it is abandoned.
    ///
    /// The single source for this default. `CloudConfig::default()` used to
    /// carry its own value of 600 while `CloudSettings::default()` carried 30,
    /// and since `CloudConfig` is built from the stored settings the effective
    /// default was always 30 — the config advertised a ten-minute default it
    /// never got to apply. The conservative value wins: a cloud-dispatched
    /// command that hangs should be given up on in half a minute, not ten, and
    /// picking 30 keeps the behaviour every existing install already has.
    ///
    /// Not validated on the way in: `set_cloud_settings` checks only the
    /// heartbeat interval. The `1..=3600` range belongs to `CLISettings`, which
    /// has a separate field of the same name.
    pub const CLOUD_COMMAND_TIMEOUT_SECONDS: u64 = 30;
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
    /// Ask before anything that changes the Mac.
    pub const PERMISSION_MODE_ASK_FIRST: &str = "ask_first";
    /// Ask before risky things. The default.
    pub const PERMISSION_MODE_ASK_WHEN_RISKY: &str = "ask_when_risky";
    /// Do not ask, except for the irreversible floor.
    pub const PERMISSION_MODE_DONT_ASK: &str = "dont_ask";
    /// The default mode. What each one permits is decided in
    /// [`crate::agent::tools::permission_policy`], which aliases these three
    /// and has a test pinning this default to the middle one. The literal is
    /// spelled out rather than referencing the constant above because the
    /// TypeScript codegen in `scripts/generate-ts-constants.js` reads these
    /// values as text.
    pub const PERMISSION_MODE: &str = "ask_when_risky";

    /// How hard the Claude CLI provider thinks per turn — its `--effort` flag.
    /// Hidden advanced setting with no UI: it trades latency for depth, and
    /// Juno's policy is capability first, so "high" rather than "medium".
    /// Anything outside `CLAUDE_CLI_EFFORT_LEVELS` is ignored and this is used.
    pub const CLAUDE_CLI_EFFORT: &str = "high";
    /// The levels the CLI accepts. A store value outside this list is dropped
    /// rather than passed through, so a stale setting cannot make every spawn
    /// fail on an unknown argument.
    pub const CLAUDE_CLI_EFFORT_LEVELS: [&str; 5] = ["low", "medium", "high", "xhigh", "max"];

    /// Whether the Claude CLI provider loads the MCP servers on the person's
    /// own Claude account — claude.ai connectors like Slack, Gmail and Drive,
    /// plus user-level servers. On by default: someone who wired Slack into
    /// their account expects Juno to have it too. Off passes
    /// `--strict-mcp-config`, so only Juno's own tool server loads (LAC-4056).
    pub const CLAUDE_CLI_LOAD_ACCOUNT_MCP: bool = true;

    pub fn claude_cli_load_account_mcp() -> bool {
        CLAUDE_CLI_LOAD_ACCOUNT_MCP
    }

    /// Whether Juno looks for a new version on its own (shortly after launch,
    /// then every few hours). On: staying current should not be a chore.
    /// Off still leaves the Check button in Settings working.
    pub const AUTO_UPDATE_CHECK_ENABLED: bool = true;
    pub fn auto_update_check_enabled() -> bool {
        AUTO_UPDATE_CHECK_ENABLED
    }

    /// Which builds this install takes: "stable" (promoted releases only) or
    /// "prerelease" (every build). Prerelease by default because the people
    /// running Juno today are the people writing it, and a dogfood build that
    /// arrives a week late is not dogfooding. Flip it to "stable" before there
    /// are users who did not sign up to find the bugs.
    pub const UPDATE_CHANNEL: &str = "prerelease";
    pub fn update_channel() -> String {
        UPDATE_CHANNEL.to_string()
    }

    pub fn background_mode() -> bool {
        BACKGROUND_MODE
    }
    pub fn mouse_control() -> String {
        MOUSE_CONTROL.to_string()
    }
    pub fn permission_mode() -> String {
        PERMISSION_MODE.to_string()
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
    /// The glow around Juno's cursor starts in system blue. Older stores lack
    /// the key and get it too.
    pub fn agent_cursor_color() -> String {
        crate::constants::ui::agent_cursor_colors::DEFAULT.to_string()
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

// Settings command names live in `constants::commands::settings`, the one
// place every command name is defined.

/// Event names for settings changes (for reactivity)
pub mod events {
    pub const SETTINGS_CHANGED: &str = "settings_changed";
    pub const KEYBOARD_SHORTCUTS_CHANGED: &str = "keyboard_shortcuts_changed";
    pub const AGENT_SETTINGS_CHANGED: &str = "agent_settings_changed";
    /// Defined once, in `constants::events::system`, and re-exported here so
    /// the settings manager can keep emitting it by its settings name.
    pub use crate::constants::events::system::PROVIDER_SETTINGS_CHANGED;
    pub const CLOUD_SETTINGS_CHANGED: &str = "cloud_settings_changed";
    pub const AUDIO_SETTINGS_CHANGED: &str = "audio_settings_changed";
    pub const TOOL_SETTINGS_CHANGED: &str = "tool_settings_changed";
    pub const FLOATING_BAR_SETTINGS_CHANGED: &str = "floating_bar_settings_changed";
    pub const PROMPT_SETTINGS_CHANGED: &str = "prompt_settings_changed";
    pub const CLI_SETTINGS_CHANGED: &str = "cli_settings_changed";
    pub const VOICE_TRANSCRIPTION_SETTINGS_CHANGED: &str = "voice_transcription_settings_changed";
}

#[cfg(test)]
mod tests {
    #[test]
    fn default_insertion_mode_matches_the_paste_mode_constant() {
        // `defaults::DICTATION_INSERTION_MODE` is a literal for the TS
        // constants generator's sake; it must stay the paste mode.
        assert_eq!(
            super::defaults::DICTATION_INSERTION_MODE,
            super::dictation_insertion_modes::PASTE
        );
    }
}
