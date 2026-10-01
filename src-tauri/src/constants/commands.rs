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
    /// Beta: one long-lived claude process per conversation.
    /// See docs/plans/cli-persistent-session-spike.md
    pub const GET_CLI_PERSISTENT_SESSION_ENABLED: &str = "get_cli_persistent_session_enabled";
    pub const SET_CLI_PERSISTENT_SESSION_ENABLED: &str = "set_cli_persistent_session_enabled";
    /// Ask before Juno sends (LAC-4058): per-send approval for connector writes.
    pub const GET_CLI_ASK_BEFORE_SEND_ENABLED: &str = "get_cli_ask_before_send_enabled";
    pub const SET_CLI_ASK_BEFORE_SEND_ENABLED: &str = "set_cli_ask_before_send_enabled";
    /// Beta: smart routing, a model and a route per request. See agent/router.rs
    pub const GET_SMART_ROUTING_ENABLED: &str = "get_smart_routing_enabled";
    pub const SET_SMART_ROUTING_ENABLED: &str = "set_smart_routing_enabled";
}

/// Core system command names
pub mod core {
    pub const GET_DEMO_INFO: &str = "get_demo_info";
    pub const GET_DEBUG_MODE: &str = "get_debug_mode";
    pub const SET_DEBUG_MODE: &str = "set_debug_mode";
    pub const GET_PERFORMANCE_MONITORING: &str = "get_performance_monitoring";
    pub const SET_PERFORMANCE_MONITORING: &str = "set_performance_monitoring";
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
    pub const SHARE_AGENT_RESPONSE: &str = "share_agent_response";
    pub const COPY_AGENT_RESPONSE: &str = "copy_agent_response";
    pub const AGENT_VOICE: &str = "agent_voice";
    pub const RESPOND_TO_AGENT_CONTINUATION: &str = "respond_to_agent_continuation";
    pub const STOP_ALL_OPERATIONS: &str = "stop_all_operations";
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
    pub const UPDATE_PROVIDER_LOAD_ACCOUNT_MCP: &str = "update_provider_load_account_mcp";
    pub const GET_PROVIDER_SETTINGS: &str = "get_provider_settings";
    pub const CHECK_API_KEYS_AVAILABLE: &str = "check_api_keys_available";
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
    pub const TOGGLE_ALWAYS_LISTENING_MODE: &str = "toggle_always_listening_mode";
    pub const DEBUG_ALWAYS_LISTENING_STATUS: &str = "debug_always_listening_status";
    pub const FORCE_TRANSCRIPTION_TEST: &str = "force_transcription_test";
    pub const SET_AUDIO_LEVEL_MONITORING: &str = "set_audio_level_monitoring";
    pub const SET_TRANSCRIPTION_DEBUGGING: &str = "set_transcription_debugging";
    pub const TEST_WHISPER_MODEL: &str = "test_whisper_model";
}

/// Speech-to-text model commands. One surface for every engine: the UI passes
/// a catalog id and the backend dispatches by engine.
pub mod stt_models {
    pub const GET_STATUS: &str = "get_stt_models_status";
    pub const DOWNLOAD: &str = "download_stt_model";
    pub const CANCEL_DOWNLOAD: &str = "cancel_stt_model_download";
    pub const USE: &str = "use_stt_model";
    pub const DELETE: &str = "delete_stt_model";
    pub const DECLINE_OFFER: &str = "decline_stt_model_offer";
    pub const GET_LIVE_PARTIAL_TRANSCRIPTION: &str = "get_live_partial_transcription";
    pub const SET_LIVE_PARTIAL_TRANSCRIPTION: &str = "set_live_partial_transcription";
}

/// Microphone and speaker choices, and Juno's voice.
pub mod audio {
    pub const LIST_AUDIO_DEVICES: &str = "list_audio_devices";
    pub const SET_AUDIO_INPUT_DEVICE: &str = "set_audio_input_device";
    pub const SET_AUDIO_OUTPUT_DEVICE: &str = "set_audio_output_device";
    /// The curated voice rows, and which one is in force.
    pub const GET_JUNO_VOICES: &str = "get_juno_voices";
    /// Choose a voice. Speaking the sample is part of choosing.
    pub const SET_JUNO_VOICE: &str = "set_juno_voice";
    /// Say the sample again in the voice already chosen.
    pub const PREVIEW_JUNO_VOICE: &str = "preview_juno_voice";
}

/// TTS command names
pub mod tts {
    pub const INVOKE_TTS: &str = "invoke_tts";
    pub const SET_TTS_PROVIDER: &str = "set_tts_provider_command";
    pub const GET_TTS_PROVIDER: &str = "get_tts_provider_command";
    pub const STOP_TTS: &str = "stop_tts";
    pub const GET_CHATTERBOX_SETTINGS: &str = "get_chatterbox_settings_command";
    pub const SET_CHATTERBOX_SETTINGS: &str = "set_chatterbox_settings_command";
    pub const GET_SUPERTONIC_SETTINGS: &str = "get_supertonic_settings_command";
    pub const SET_SUPERTONIC_SETTINGS: &str = "set_supertonic_settings_command";
}

/// Dictation command names
pub mod dictation {
    pub const GET_DICTATION_TRIGGER_MODE: &str = "get_dictation_trigger_mode";
    pub const SET_DICTATION_TRIGGER_MODE: &str = "set_dictation_trigger_mode";
    pub const GET_DICTATION_CLIPBOARD_ENABLED: &str = "get_dictation_clipboard_enabled";
    pub const SET_DICTATION_CLIPBOARD_ENABLED: &str = "set_dictation_clipboard_enabled";
    pub const GET_DICTATION_INSERTION_MODE: &str = "get_dictation_insertion_mode";
    pub const SET_DICTATION_INSERTION_MODE: &str = "set_dictation_insertion_mode";
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
    pub const AUTO_GRANT_PERMISSIONS: &str = "auto_grant_permissions";
    pub const CANCEL_AUTO_GRANT: &str = "cancel_auto_grant";
    /// Opens the exact Privacy pane for one permission, so nobody has to hunt
    pub const OPEN_SYSTEM_SETTINGS: &str = "open_system_settings_enhanced";
    /// Which granted permissions are waiting on a restart before Juno can use them
    pub const AWAITING_RELAUNCH: &str = "permissions_awaiting_relaunch";
    /// Restart Juno so a granted permission takes effect
    pub const RESTART_AFTER_PERMISSIONS: &str = "restart_app_after_permissions";
    /// Where one capability stands now, including whether a restart is owed
    pub const PERMISSION_MOMENT: &str = "permission_moment";
    /// Lets macOS raise its Automation dialog, after Juno has explained it
    pub const ALLOW_AUTOMATION: &str = "allow_automation_permission";
    pub const GET_PERMISSION_DIAGNOSTICS: &str = "get_permission_diagnostics";
    pub const RESET_PERMISSION_GRANT: &str = "reset_permission_grant";
    pub const OPEN_SYSTEM_PREFERENCES: &str = "open_system_preferences";
    pub const START_PERMISSIONS_MONITORING: &str = "start_permissions_monitoring";
    pub const STOP_PERMISSIONS_MONITORING: &str = "stop_permissions_monitoring";
}

/// Window management
pub mod windows {
    /// Put the full-size chat window away and give the bar the conversation back
    pub const CLOSE_MAIN_WINDOW: &str = "close_main_window";
    pub const OPEN_MAIN_WINDOW: &str = "open_main_window";
    pub const OPEN_SETTINGS_WINDOW: &str = "open_settings_window";
    pub const CLOSE_WINDOW: &str = "close_window";
    pub const FOCUS_WINDOW: &str = "focus_window";
    pub const GET_WINDOW_INFO: &str = "get_window_info";
    pub const GET_WINDOW_LIST: &str = "get_window_list";
    pub const MOVE_WINDOW: &str = "move_window";
    pub const RESIZE_WINDOW: &str = "resize_window";
}

/// Activation triggers
pub mod triggers {
    pub const GET_TRIGGERS: &str = "get_triggers";
    pub const SET_TRIGGERS: &str = "set_triggers";
    /// The key worth naming for each target, so onboarding can teach the real one
    pub const GET_TRIGGER_HINTS: &str = "get_trigger_hints";
    /// What is wrong with a trigger list right now, so an inline error can be
    /// derived from what is on screen instead of remembered from a refusal
    pub const GET_TRIGGER_ISSUES: &str = "get_trigger_issues";
    /// Listen for a bare modifier key while a screen asks someone to press theirs
    pub const SET_TRIGGER_CAPTURE: &str = "set_trigger_capture";
    /// Open the macOS Keyboard pane, where "Press globe key to" lives
    pub const OPEN_KEYBOARD_SETTINGS: &str = "open_keyboard_settings";
}

/// Background operation and the consent it needs
pub mod input_control {
    pub const RESPOND_TO_REQUEST: &str = "respond_to_input_control";
    pub const GET_MOUSE_CONTROL: &str = "get_mouse_control";
    pub const SET_MOUSE_CONTROL: &str = "set_mouse_control";
    pub const DISMISS_PROMPT: &str = "dismiss_mouse_control_prompt";
    pub const GET_BACKGROUND_MODE: &str = "get_background_mode";
    pub const SET_BACKGROUND_MODE: &str = "set_background_mode";
    pub const GET_DOCK_ICON_VISIBLE: &str = "get_dock_icon_visible";
    pub const SET_DOCK_ICON_VISIBLE: &str = "set_dock_icon_visible";
    pub const GET_TRAY_ICON_VISIBLE: &str = "get_tray_icon_visible";
    pub const SET_TRAY_ICON_VISIBLE: &str = "set_tray_icon_visible";
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
    pub const GET_CLOUD_STATUS: &str = "get_cloud_status";
    pub const GET_CLOUD_CONFIG: &str = "get_cloud_config";
    pub const UPDATE_CLOUD_CONFIG: &str = "update_cloud_config";
    pub const GENERATE_DEVICE_ID: &str = "generate_device_id";
}

/// MCP server management command names
pub mod mcp {
    /// Explicit user approval for an MCP server to spawn its configured command
    /// (spawn-approval gate, 2026-09 security audit).
    pub const APPROVE_MCP_SERVER: &str = "approve_mcp_server";
    pub const GET_MCP_SERVERS: &str = "get_mcp_servers";
    pub const ADD_MCP_SERVER: &str = "add_mcp_server";
    pub const GET_MCP_SERVER_STATUSES: &str = "get_mcp_server_statuses";
    pub const GET_MCP_TOOLS: &str = "get_mcp_tools";
    pub const TOGGLE_MCP_SERVER: &str = "toggle_mcp_server";
    pub const TOGGLE_MCP_TOOL: &str = "toggle_mcp_tool";
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

/// Auto-update command names. The backend owns the schedule, the channel and
/// the install; these three are the whole bridge to the UI.
pub mod updates {
    /// Read the current `UpdateStatus` without touching the network.
    pub const GET_STATUS: &str = "get_update_status";
    /// Look now, and install anything found.
    pub const CHECK_NOW: &str = "check_for_updates_now";
    /// Relaunch into a version that is already installed.
    pub const RESTART_TO_UPDATE: &str = "restart_to_update";
    /// Read the auto-check flag and the channel.
    pub const GET_SETTINGS: &str = "get_update_settings";
    /// Write the auto-check flag and the channel.
    pub const SET_SETTINGS: &str = "set_update_settings";
}

/// Parallel agent sessions (the session switcher)
pub mod agent_sessions {
    pub const LIST_AGENT_SESSIONS: &str = "list_agent_sessions";
    pub const FOCUS_AGENT_SESSION: &str = "focus_agent_session";
    pub const CANCEL_AGENT_SESSION: &str = "cancel_agent_session";
}

/// App-level commands: build info, config directory, diagnostics
pub mod app {
    pub const GET_BUILD_INFO: &str = "get_build_info";
    pub const OPEN_CONFIG_DIRECTORY: &str = "open_config_directory";
    pub const TEST_SYSTEM_CONTEXT: &str = "test_system_context";
}

/// Launch at login
pub mod autostart {
    pub const IS_AUTOSTART_ENABLED: &str = "is_autostart_enabled";
    pub const ENABLE_AUTOSTART: &str = "enable_autostart";
    pub const DISABLE_AUTOSTART: &str = "disable_autostart";
}

/// The floating bar: position, frame, pane and interaction
pub mod bar {
    pub const GET_BAR_POSITION: &str = "get_bar_position";
    pub const SET_BAR_POSITION: &str = "set_bar_position";
    pub const SET_BAR_FRAME: &str = "set_bar_frame";
    pub const SHOW_BAR_WHEN_READY: &str = "show_bar_when_ready";
    pub const SET_BAR_PANE_OPEN: &str = "set_bar_pane_open";
    pub const UI_GET_BAR_CONFIG: &str = "ui_get_bar_config";
    pub const UI_SET_BAR_CONFIG: &str = "ui_set_bar_config";
    pub const UI_HANDLE_INTERACTION: &str = "ui_handle_interaction";
}

/// Conversation history, import and export
pub mod conversations {
    pub const LIST_CONVERSATIONS: &str = "list_conversations";
    pub const LOAD_CONVERSATION: &str = "load_conversation";
    pub const NEW_CONVERSATION: &str = "new_conversation";
    pub const DELETE_CONVERSATION: &str = "delete_conversation";
    pub const GET_CURRENT_CONVERSATION_ID: &str = "get_current_conversation_id";
    pub const SAVE_CHAT_EXPORT: &str = "save_chat_export";
    pub const LOAD_CHAT_IMPORT: &str = "load_chat_import";
}

/// Developer tools panel
pub mod debug {
    pub const DEBUG_REGISTERED_TOOLS: &str = "debug_registered_tools";
    pub const DEBUG_RESET_TOOL_CONFIG: &str = "debug_reset_tool_config";
    pub const DEBUG_TOOL_CONFIGURATION: &str = "debug_tool_configuration";
}

/// Direct desktop actions (developer tools and ActionButton)
pub mod desktop {
    pub const COMPUTER: &str = "computer";
    pub const OPEN_APPLICATION: &str = "open_application";
    pub const OPEN_URL: &str = "open_url";
    pub const TYPE_TEXT: &str = "type_text";
    pub const RELEASE_KEY: &str = "release_key";
    pub const GLOBAL_TYPE_TEXT: &str = "global_type_text";
    pub const GET_FOCUSED_ELEMENT_INFO: &str = "get_focused_element_info";
    pub const GET_SELECTED_TEXT: &str = "get_selected_text";
}

/// File access from the developer tools panel
pub mod files {
    pub const GET_FILE_CONTENT: &str = "get_file_content";
    pub const SET_FILE_CONTENT: &str = "set_file_content";
    pub const LIST_FILES: &str = "list_files";
}

/// Pointer behaviour
pub mod mouse {
    pub const GET_COMPANION_MODE: &str = "get_companion_mode";
    pub const SET_COMPANION_MODE: &str = "set_companion_mode";
    pub const GET_SMOOTH_MOUSE_MOVEMENT_SETTING: &str = "get_smooth_mouse_movement_setting";
    pub const SET_SMOOTH_MOUSE_MOVEMENT_SETTING: &str = "set_smooth_mouse_movement_setting";
}

/// System notifications
pub mod notifications {
    pub const CHECK_NOTIFICATION_PERMISSION: &str = "check_notification_permission";
    pub const REQUEST_NOTIFICATION_PERMISSION: &str = "request_notification_permission";
    pub const GET_NOTIFICATION_SETTINGS: &str = "get_notification_settings";
    pub const SET_NOTIFICATIONS_ENABLED: &str = "set_notifications_enabled";
    pub const TEST_NOTIFICATION: &str = "test_notification";
}

/// First-run setup
pub mod onboarding {
    pub const GET_ONBOARDING_INFO: &str = "get_onboarding_info";
    pub const GET_ONBOARDING_STATE: &str = "get_onboarding_state";
    pub const COMPLETE_ONBOARDING: &str = "complete_onboarding";
    pub const SKIP_ONBOARDING: &str = "skip_onboarding";
    pub const RESTART_ONBOARDING: &str = "restart_onboarding";
    pub const SET_ONBOARDING_ACTIVE: &str = "set_onboarding_active";
    pub const CLOSE_ONBOARDING_WINDOW: &str = "close_onboarding_window";
    pub const CHECK_CLAUDE_CLI_AVAILABLE: &str = "check_claude_cli_available";
    pub const TEST_GLOBAL_SHORTCUTS_WORKING: &str = "test_global_shortcuts_working";
    pub const GET_LAST_ONBOARDING_PHASE: &str = "get_last_onboarding_phase";
    pub const RECORD_ONBOARDING_EVENT: &str = "record_onboarding_event";
}

/// Scheduled automations
pub mod scheduler {
    pub const LIST_SCHEDULED_TASKS: &str = "list_scheduled_tasks";
    pub const CREATE_SCHEDULED_TASK: &str = "create_scheduled_task";
    pub const UPDATE_SCHEDULED_TASK: &str = "update_scheduled_task";
    pub const DELETE_SCHEDULED_TASK: &str = "delete_scheduled_task";
    pub const RUN_SCHEDULED_TASK_NOW: &str = "run_scheduled_task_now";
    pub const PREVIEW_CRON_SCHEDULE: &str = "preview_cron_schedule";
}

/// Keyboard shortcuts
pub mod shortcuts {
    pub const GET_KEYBOARD_SHORTCUTS: &str = "get_keyboard_shortcuts";
    pub const VALIDATE_KEYBOARD_SHORTCUT: &str = "validate_keyboard_shortcut";
}

/// Sound effects
pub mod sound {
    pub const GET_SOUND_ENABLED: &str = "get_sound_enabled";
    pub const SET_SOUND_ENABLED: &str = "set_sound_enabled";
    pub const GET_AVAILABLE_SOUNDS: &str = "get_available_sounds";
    pub const PLAY_SOUND_BY_TYPE: &str = "play_sound_by_type";
    pub const PLAY_SOUND_FILE: &str = "play_sound_file";
    pub const PLAY_NOTIFICATION_SOUND: &str = "play_notification_sound";
    pub const PLAY_SUCCESS_SOUND: &str = "play_success_sound";
    pub const PLAY_ERROR_SOUND: &str = "play_error_sound";
    pub const PLAY_ALERT_SOUND: &str = "play_alert_sound";
    pub const PLAY_AGENT_START_SOUND: &str = "play_agent_start_sound";
    pub const PLAY_AGENT_SUCCESS_SOUND: &str = "play_agent_success_sound";
    pub const PLAY_AGENT_ERROR_SOUND: &str = "play_agent_error_sound";
    pub const PLAY_AGENT_ATTENTION_SOUND: &str = "play_agent_attention_sound";
    pub const PLAY_VOICE_START_SOUND: &str = "play_voice_start_sound";
    pub const PLAY_VOICE_END_SOUND: &str = "play_voice_end_sound";
    pub const PLAY_VOICE_ERROR_SOUND: &str = "play_voice_error_sound";
    pub const PLAY_DICTATION_START_SOUND: &str = "play_dictation_start_sound";
    pub const PLAY_DICTATION_END_SOUND: &str = "play_dictation_end_sound";
    pub const PLAY_BOOT_SOUND: &str = "play_boot_sound";
    pub const PLAY_SYSTEM_READY_SOUND: &str = "play_system_ready_sound";
    pub const PLAY_CONNECTION_SOUND: &str = "play_connection_sound";
    pub const PLAY_DISCONNECTION_SOUND: &str = "play_disconnection_sound";
}

/// Tool approval and configuration
pub mod tools {
    pub const APPROVE_TOOL_EXECUTION: &str = "approve_tool_execution";
    pub const DENY_TOOL_EXECUTION: &str = "deny_tool_execution";
    pub const GET_REGISTERED_TOOLS: &str = "get_registered_tools";
    pub const GET_TOOL_CONFIGURATIONS: &str = "get_tool_configurations";
    pub const RESET_TOOL_CONFIGURATION: &str = "reset_tool_configuration";
    pub const SET_TOOL_ENABLED: &str = "set_tool_enabled";
    pub const SET_ALL_TOOLS_ENABLED: &str = "set_all_tools_enabled";
    pub const SET_TOOL_CATEGORY_ENABLED: &str = "set_tool_category_enabled";
    /// How much Juno interrupts to ask. Replaces the
    /// get/set_tool_approval_required pair, which was a boolean that nothing
    /// persisted and a risk threshold outvoted.
    pub const GET_PERMISSION_MODE: &str = "get_permission_mode";
    pub const SET_PERMISSION_MODE: &str = "set_permission_mode";
    /// Approve a pending request and stop asking about that tool for the rest
    /// of this conversation.
    pub const ALLOW_TOOL_FOR_CONVERSATION: &str = "allow_tool_for_conversation";
}
