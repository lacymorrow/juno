//! # Command Constants
//!
//! Centralized constants for all Tauri command names to eliminate magic strings
//! and prevent duplication bugs like reset_all_settings vs reset_centralized_settings.
//!
//! IMPORTANT: Use ONLY these constants when calling commands from frontend or registering
//! commands in lib.rs to prevent duplication and inconsistency issues.

/// Settings command names (centralized - use ONLY these to prevent duplication!)
pub mod settings {
    pub const GET_ALL_SETTINGS: &str = "get_all_settings";
    pub const SAVE_ALL_SETTINGS: &str = "save_all_settings";
    pub const RESET_SETTINGS: &str = "reset_centralized_settings"; // Use centralized version ONLY
    pub const EXPORT_SETTINGS: &str = "export_settings";
    pub const IMPORT_SETTINGS: &str = "import_settings";
    pub const GET_KEYBOARD_SHORTCUTS: &str = "get_centralized_keyboard_shortcuts";
    pub const SET_KEYBOARD_SHORTCUTS: &str = "set_centralized_keyboard_shortcuts";
    pub const GET_FLOATING_BAR_SETTINGS: &str = "get_floating_bar_settings";
    pub const SET_FLOATING_BAR_SETTINGS: &str = "set_floating_bar_settings";
    pub const GET_AGENT_SETTINGS: &str = "get_agent_settings";
    pub const SET_AGENT_SETTINGS: &str = "set_agent_settings";
    pub const GET_PROVIDER_SETTINGS: &str = "get_centralized_provider_settings";
    pub const SET_PROVIDER_SETTINGS: &str = "set_centralized_provider_settings";
    pub const GET_CLOUD_SETTINGS: &str = "get_cloud_settings";
    pub const SET_CLOUD_SETTINGS: &str = "set_cloud_settings";
    pub const GET_AUDIO_SETTINGS: &str = "get_audio_settings";
    pub const SET_AUDIO_SETTINGS: &str = "set_audio_settings";
    pub const GET_TOOL_SETTINGS: &str = "get_tool_settings";
    pub const SET_TOOL_SETTINGS: &str = "set_tool_settings";
    pub const GET_ONBOARDING_SETTINGS: &str = "get_onboarding_settings";
    pub const SET_ONBOARDING_SETTINGS: &str = "set_onboarding_settings";
    pub const SET_AUTOSTART_ENABLED: &str = "set_autostart_enabled";
    pub const GET_ADVANCED_SETTINGS_ENABLED: &str = "get_advanced_settings_enabled";
    pub const SET_ADVANCED_SETTINGS_ENABLED: &str = "set_advanced_settings_enabled";
}

/// Core system command names
pub mod core {
    pub const GET_DEBUG_MODE: &str = "get_debug_mode";
    pub const SET_DEBUG_MODE: &str = "set_debug_mode";
    pub const GET_PERFORMANCE_MONITORING: &str = "get_performance_monitoring";
    pub const SET_PERFORMANCE_MONITORING: &str = "set_performance_monitoring";
    pub const CANCEL_AGENT_EXECUTION: &str = "cancel_agent_execution";
    pub const GET_AGENT_EXECUTION_PROGRESS: &str = "get_agent_execution_progress";
    pub const SET_AGENT_EXECUTION_PROGRESS: &str = "set_agent_execution_progress";
    pub const GET_SYSTEM_CONTEXT: &str = "get_system_context";
}

/// Agent-related command names
pub mod agent {
    pub const SUBMIT_QUERY: &str = "submit_query";
    pub const DISPATCH_QUERY: &str = "dispatch_query";
    pub const GET_AGENT_MODE: &str = "get_agent_mode";
    pub const SET_AGENT_MODE: &str = "set_agent_mode";
    pub const GET_AGENT_TRIGGER_MODE: &str = "get_agent_trigger_mode";
    pub const SET_AGENT_TRIGGER_MODE: &str = "set_agent_trigger_mode";
}

/// Provider-related command names
pub mod providers {
    pub const GET_PROVIDERS: &str = "get_providers";
    pub const GET_ACTIVE_PROVIDER: &str = "get_active_provider";
    pub const SET_ACTIVE_PROVIDER: &str = "set_active_provider";
    pub const VALIDATE_PROVIDER_MODEL: &str = "validate_provider_model";
    pub const GET_PROVIDER_MODELS: &str = "get_provider_models";
    pub const UPDATE_PROVIDER_API_KEY: &str = "update_provider_api_key";
    pub const UPDATE_PROVIDER_MODEL: &str = "update_provider_model";
    pub const UPDATE_PROVIDER_MAX_TOKENS: &str = "update_provider_max_tokens";
    pub const UPDATE_PROVIDER_TEMPERATURE: &str = "update_provider_temperature";
    pub const UPDATE_PROVIDER_SYSTEM_PROMPT: &str = "update_provider_system_prompt";
}

/// Always listening command names
pub mod always_listening {
    pub const START_ALWAYS_LISTENING: &str = "start_always_listening_mode";
    pub const STOP_ALWAYS_LISTENING: &str = "stop_always_listening_mode";
    pub const GET_ALWAYS_LISTENING_STATUS: &str = "get_always_listening_status";
    pub const SET_ALWAYS_LISTENING_SENSITIVITY: &str = "set_always_listening_sensitivity";
    pub const GET_ALWAYS_LISTENING_SENSITIVITY: &str = "get_always_listening_sensitivity";
    pub const SET_ALWAYS_LISTENING_WAKE_WORDS: &str = "set_always_listening_wake_words";
    pub const GET_ALWAYS_LISTENING_WAKE_WORDS: &str = "get_always_listening_wake_words";
}

/// TTS command names
pub mod tts {
    pub const INVOKE_TTS: &str = "invoke_tts";
    pub const SET_TTS_PROVIDER: &str = "set_tts_provider_command";
    pub const GET_TTS_PROVIDER: &str = "get_tts_provider_command";
    pub const STOP_TTS: &str = "stop_tts";
}

/// Dictation command names
pub mod dictation {
    pub const GET_DICTATION_TRIGGER_MODE: &str = "get_dictation_trigger_mode";
    pub const SET_DICTATION_TRIGGER_MODE: &str = "set_dictation_trigger_mode";
    pub const GET_DICTATION_CLIPBOARD_ENABLED: &str = "get_dictation_clipboard_enabled";
    pub const SET_DICTATION_CLIPBOARD_ENABLED: &str = "set_dictation_clipboard_enabled";
    pub const FORCE_RESET_DICTATION_STATE: &str = "force_reset_dictation_state";
    pub const GET_DICTATION_COMPREHENSIVE_STATUS: &str = "get_dictation_comprehensive_status";
    pub const UPDATE_DICTATION_COMPONENT_STATE: &str = "update_dictation_component_state";
    pub const TRANSITION_DICTATION_STATE: &str = "transition_dictation_state";
}

/// Permission command names
pub mod permissions {
    pub const CHECK_PERMISSIONS_STATUS: &str = "check_permissions_status_native";
    pub const REQUEST_ACCESSIBILITY_PERMISSION: &str = "request_accessibility_permission_native";
    pub const REQUEST_MICROPHONE_PERMISSION: &str = "request_microphone_permission_native";
    pub const REQUEST_SCREEN_RECORDING_PERMISSION: &str =
        "request_screen_recording_permission_native";
    pub const REQUEST_INPUT_MONITORING_PERMISSION: &str =
        "request_input_monitoring_permission_native";
    pub const TEST_MICROPHONE_FUNCTIONALITY: &str = "test_microphone_functionality";
}

/// Utility command names
pub mod utils {
    pub const WAIT: &str = "wait";
    pub const GET_CLIPBOARD: &str = "get_clipboard";
    pub const SET_CLIPBOARD: &str = "set_clipboard";
    pub const LIST_APPS: &str = "list_apps";
    pub const CHECK_SERVER_STATUS: &str = "check_server_status";
}

/// Screenshot command names
pub mod screenshots {
    pub const CAPTURE_SCREENSHOT: &str = "capture_screenshot_command";
    pub const CAPTURE_ELEMENT_SCREENSHOT: &str = "capture_element_screenshot_command";
    pub const CAPTURE_WINDOW_SCREENSHOT: &str = "capture_window_screenshot_command";
    pub const CAPTURE_FOCUSED_WINDOW_SCREENSHOT: &str = "capture_focused_window_screenshot_command";
}

/// Cloud connectivity command names
pub mod cloud {
    // Production cloud connector commands
    pub const START_PRODUCTION_CLOUD_CONNECTOR: &str = "start_production_cloud_connector";
    pub const STOP_PRODUCTION_CLOUD_CONNECTOR: &str = "stop_production_cloud_connector";
    pub const GET_PRODUCTION_CLOUD_STATUS: &str = "get_production_cloud_status";

    // Cloud test commands
    pub const GET_CLOUD_CONFIG_STATUS: &str = "get_cloud_config_status";
    pub const TEST_CLOUD_BACKEND_CONNECTION: &str = "test_cloud_backend_connection";
    pub const ENABLE_CLOUD_BACKEND: &str = "enable_cloud_backend";
    pub const DISABLE_CLOUD_BACKEND: &str = "disable_cloud_backend";

    // WebSocket testing commands
    pub const TEST_WEBSOCKET_CONNECTION: &str = "test_websocket_connection";
    pub const SEND_TEST_CLOUD_COMMAND: &str = "send_test_cloud_command";
    pub const SIMULATE_CLOUD_COMMAND: &str = "simulate_cloud_command";
    pub const GET_WEBSOCKET_DIAGNOSTICS: &str = "get_websocket_diagnostics";
    pub const RUN_WEBSOCKET_TEST_SUITE: &str = "run_websocket_test_suite";

    // Cloud message handling
    pub const HANDLE_CLOUD_MESSAGE: &str = "handle_cloud_message";
    pub const EXECUTE_REMOTE_COMMAND: &str = "execute_remote_command";
    pub const GET_CLOUD_CONNECTION_DIAGNOSTICS: &str = "get_cloud_connection_diagnostics";
}

/// Skill discovery command names for slash-command autocomplete
pub mod media {
    /// Read a media player's live state (`<NowPlayingCard>`).
    pub const GET_STATE: &str = "media_get_state";
    /// Send play/pause/next/previous to a running player.
    pub const CONTROL: &str = "media_control";
}

pub mod skills {
    pub const LIST_AVAILABLE_SKILLS: &str = "list_available_skills";
}
