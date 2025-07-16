// Main module for all Tauri commands, broken down by category.

use crate::state::AppState;
use crate::utils::{format_system_context_for_agent, gather_system_context};
use tauri::{Emitter, State};
use crate::constants::events;

// Declare the submodules
pub mod accessibility;
pub mod registry;
pub mod safari_tools;
pub mod app_url;
pub mod autostart;
pub mod computer;
pub mod core;
pub mod debug_utils;
pub mod dev;
pub mod dictation;
// Removed deprecated dictation_reset module
pub mod agent_continuation;
pub mod always_listening;
pub mod cloud;
pub mod cloud_test;
pub mod collaborative_ai_commands;
pub mod debug_tools;
pub mod dictation_state_manager;
pub mod element;
pub mod enhanced_visual_reasoning_commands;
pub mod error_recovery;
pub mod escape_key_coordinator;
pub mod filesystem;
// floating_bar and floating_panel modules removed - functionality migrated to ui_commands
pub mod keyboard;
pub mod mcp;
pub mod memory;
pub mod mouse;
pub mod native_permissions;
pub mod notifications;
pub mod onboarding;
pub mod orchestrator;
pub mod permissions;
pub mod providers;
// pub mod self_improvement; // TODO: Fix module not found
pub mod settings;
pub mod shell;
pub mod shortcuts;
pub mod sound;
pub mod stop_coordinator;
pub mod stop_operations;
pub mod testing;
pub mod text_editor;
pub mod tool_choice;
pub mod tools;
pub mod tray_commands;
pub mod ui_commands; // Consolidated UI API for all floating elements
pub mod ui_token_selection;
pub mod window; // Debug commands for tool configuration diagnostics

// Re-export commands with explicit imports to avoid ambiguous glob re-exports
// Accessibility commands
pub use self::accessibility::{
    accessibility_scan, accessibility_click, test_accessibility_permissions,
    get_accessibility_tool_definitions, execute_accessibility_tool
};

// Safari tools
pub use self::safari_tools::{
    safari_is_active, safari_extract_dom, safari_click_element, safari_type_text,
    safari_get_url, safari_navigate, safari_list_clickable_elements,
    safari_execute_javascript, safari_clear_cache, execute_safari_tool
};

// Autostart commands
pub use self::autostart::{is_autostart_enabled, enable_autostart, disable_autostart, toggle_autostart};

// Computer commands
pub use self::computer::computer;

// Core commands
// Note: list_apps and check_server_status removed as they are unused

// Dictation commands
pub use self::dictation::{
    get_dictation_clipboard_enabled, set_dictation_clipboard_enabled
};

// Always listening commands
pub use self::always_listening::{
    get_always_listening_status, start_always_listening_mode, stop_always_listening_mode, toggle_always_listening_mode,
    get_always_listening_sensitivity, set_always_listening_sensitivity,
    get_always_listening_wake_words, set_always_listening_wake_words
};

// Cloud commands
pub use self::cloud::{
    enable_cloud, disable_cloud, get_cloud_status,
    send_test_cloud_command, execute_remote_command
};

// Cloud test commands
pub use self::cloud_test::{test_cloud_backend_connection, get_cloud_config_status};

// Collaborative AI commands
pub use self::collaborative_ai_commands::{
    get_collaborative_ai_statistics, get_collaborative_ai_capabilities
};

// Debug tools
pub use self::debug_tools::{debug_tool_configuration, debug_registered_tools, debug_reset_tool_config};

// Dictation state manager
pub use self::dictation_state_manager::{
    force_reset_dictation_state, get_dictation_comprehensive_status, transition_dictation_state,
    update_dictation_component_state,
};
// Enhanced visual reasoning commands
pub use self::enhanced_visual_reasoning_commands::{
    analyze_gui_scene_with_visual_reasoning, get_visual_reasoning_capabilities,
    get_visual_reasoning_statistics, VisualReasoningState, VisualAnalysisRequest, 
    SceneTypeInfo, TestResult, initialize_visual_reasoning_state
};

// Error recovery commands
pub use self::error_recovery::{
    get_recovery_statistics, get_recovery_config, reset_recovery_state
};

// Filesystem commands
pub use self::filesystem::{get_file_content, list_files, save_agent_response, set_file_content};

// UI commands (consolidated UI API)
pub use self::ui_commands::{
    ui_handle_interaction, ui_get_element_state, ui_create_element, ui_update_element,
    ui_delete_element, ui_get_bar_config, ui_set_bar_config
};

// MCP commands
pub use self::mcp::{
    get_mcp_tools, add_mcp_server, remove_mcp_server, start_mcp_server, stop_mcp_server,
    get_mcp_servers, get_mcp_server_statuses, update_mcp_server, set_mcp_server_enabled,
    toggle_mcp_server, toggle_mcp_tool, test_mcp_server_connection, initialize_mcp_servers,
    get_mcp_diagnostics, restart_mcp_server_with_diagnostics, troubleshoot_mcp_issues,
    apply_mcp_quick_fixes
};

// Memory commands
pub use self::memory::{
    get_memory_status, clear_conversation_memory, clean_orphaned_tool_calls,
    clean_orphaned_tool_results, get_conversation_messages, get_last_n_messages,
    get_visual_summaries, update_visual_config, get_visual_config,
    compress_all_screenshots, configure_screenshot_compression, get_memory_compression_stats,
    emergency_memory_recovery, get_conversation_summaries, optimize_memory,
    get_memory_config, update_memory_config, get_advanced_memory_metrics,
    force_memory_prune, get_tiered_memory_context
};

// Mouse commands
// Note: get_cursor_position removed as it is unused

// Onboarding commands
pub use self::onboarding::{check_onboarding_status, complete_onboarding, skip_onboarding, reset_onboarding, restart_onboarding};

// Orchestrator commands
pub use self::orchestrator::{
    get_orchestrator_status, configure_orchestrator, create_orchestrator_task,
    get_task_history, get_active_tasks, get_agent_capabilities, cancel_task,
    get_workflow_templates, execute_workflow_template, execute_mcp_task
};

// Permissions commands
pub use self::permissions::{
    check_permissions_status_native, get_permissions_state,
    request_accessibility_permission_native, request_microphone_permission_native,
    request_screen_recording_permission_native, request_input_monitoring_permission_native
};

// Settings commands
pub use self::settings::{
    get_all_settings, save_all_settings, reset_centralized_settings, export_settings, import_settings,
    get_centralized_provider_settings, set_centralized_provider_settings,
    get_floating_bar_settings, set_floating_bar_settings
};

// Shell commands
pub use self::shell::{bash_command};

// Shortcuts commands
pub use self::shortcuts::{
    get_keyboard_shortcuts, set_keyboard_shortcut, set_keyboard_shortcuts, reset_keyboard_shortcuts, validate_keyboard_shortcut
};

// Sound commands
pub use self::sound::{
    play_sound_by_type, play_notification_sound, get_sound_enabled, set_sound_enabled
};

// Stop operations commands
pub use self::stop_operations::{
    stop_all_operations
};

// Tool choice commands
pub use self::tool_choice::{
    get_tool_choice_config, set_tool_choice_config, reset_tool_choice_config
};

// Tools commands
pub use self::tools::{
    get_enabled_tools, get_registered_tools, get_tool_config, get_tool_configuration_summary,
    get_tool_configurations, is_tool_enabled, reset_tool_configuration,
    set_tool_category_enabled, set_tool_enabled, test_dynamic_tool_categorization,
    test_tool_config, test_tool_config_command,
    // Tool approval commands
    approve_tool_execution, clear_pending_tool_approvals, deny_tool_execution, get_pending_tool_approvals,
    get_tool_approval_required, set_tool_approval_required,
};

// UI token selection commands
pub use self::ui_token_selection::{
    get_ui_token_config, update_ui_token_config, set_ui_token_config
};

// Shared helper function for sending notifications from dev tools
// Needs to be pub(crate) so submodules can access it via super::
pub(crate) fn send_dev_tool_notification(
    app: &tauri::AppHandle,
    action: &str,
    message: &str,
) -> Result<(), String> {
    let payload = serde_json::json!({
        "action": action,
        "message": message
    });
    app.emit(events::dev::TOOL_NOTIFICATION, payload)
        .map_err(|e| format!("Failed to emit dev tool notification: {}", e))
}

/// Test command to verify system context gathering
#[tauri::command]
pub async fn test_system_context(state: State<'_, AppState>) -> Result<String, String> {
    match gather_system_context(Some(&*state)).await {
        Ok(context) => {
            let formatted = format_system_context_for_agent(&context);
            Ok(formatted)
        }
        Err(e) => Err(format!("Failed to gather system context: {}", e)),
    }
}

/// Load audio settings from centralized settings into AppState
/// Used by: Application startup for audio configuration initialization
pub async fn load_audio_settings_from_centralized_settings(
    app_handle: &tauri::AppHandle,
    state: &crate::state::AppState,
) -> Result<(), String> {
    use crate::settings::manager::SettingsManager;

    let settings_manager = SettingsManager::new(app_handle.clone())
        .map_err(|e| format!("Failed to create settings manager: {}", e))?;

    let audio_settings = settings_manager
        .get_audio_settings()
        .await
        .map_err(|e| format!("Failed to get audio settings: {}", e))?;

    // CRITICAL FIX: Only update AppState if centralized settings has non-empty TTS provider
    // This prevents empty/uninitialized centralized settings from overriding correct AppState defaults
    // Note: "off" is a valid user preference for disabling TTS and should be honored
    let should_update_centralized_settings = {
        let current_tts_provider = state
            .get_tts_provider()
            .map_err(|e| format!("Failed to get TTS provider: {}", e))?;
        if !audio_settings.tts_provider.is_empty() {
            tracing::info!(
                "Loading TTS provider from centralized settings: {}",
                audio_settings.tts_provider
            );
            state
                .set_tts_provider(audio_settings.tts_provider.clone())
                .map_err(|e| format!("Failed to set TTS provider: {}", e))?;
            None // No need to update centralized settings
        } else {
            tracing::warn!(
                "Centralized settings TTS provider is empty ('{}'), keeping AppState default: {}",
                audio_settings.tts_provider,
                current_tts_provider
            );

            // Return the current valid AppState value for updating centralized settings
            Some(current_tts_provider)
        }
    };

    // Handle updating centralized settings outside the mutex scope
    if let Some(valid_tts_provider) = should_update_centralized_settings {
        let mut updated_audio_settings = audio_settings.clone();
        updated_audio_settings.tts_provider = valid_tts_provider.clone();

        if let Err(e) = settings_manager
            .set_audio_settings(&updated_audio_settings)
            .await
        {
            tracing::warn!(
                "Failed to update centralized settings with correct TTS provider: {}",
                e
            );
        } else {
            tracing::info!(
                "Updated centralized settings with correct TTS provider: {}",
                valid_tts_provider
            );
        }
    }

    let _ = state.set_always_listening_active(audio_settings.always_listening_active);
    let _ = state.set_always_listening_sensitivity(audio_settings.always_listening_sensitivity);
    let _ = state.set_always_listening_wake_words(audio_settings.always_listening_wake_words);

    tracing::info!("Loaded audio settings from centralized settings into AppState");
    Ok(())
}

/// Save current AppState audio settings to centralized settings
/// Used by: Application shutdown and settings synchronization
pub async fn save_audio_settings_to_centralized_settings(
    app_handle: &tauri::AppHandle,
    state: &crate::state::AppState,
) -> Result<(), String> {
    use crate::settings::manager::SettingsManager;

    let settings_manager = SettingsManager::new(app_handle.clone())
        .map_err(|e| format!("Failed to create settings manager: {}", e))?;

    let mut audio_settings = settings_manager
        .get_audio_settings()
        .await
        .map_err(|e| format!("Failed to get audio settings: {}", e))?;

    // Update centralized settings with current AppState values
    if let Ok(tts_provider) = state.get_tts_provider() {
        audio_settings.tts_provider = tts_provider;
    }

    if let Ok(always_listening_active) = state.get_always_listening_active() {
        audio_settings.always_listening_active = always_listening_active;
    }

    if let Ok(always_listening_sensitivity) = state.get_always_listening_sensitivity() {
        audio_settings.always_listening_sensitivity = always_listening_sensitivity;
    }

    if let Ok(always_listening_wake_words) = state.get_always_listening_wake_words() {
        audio_settings.always_listening_wake_words = always_listening_wake_words;
    }

    // All mutex guards are automatically dropped here before the await
    settings_manager
        .set_audio_settings(&audio_settings)
        .await
        .map_err(|e| format!("Failed to save audio settings: {}", e))?;

    tracing::info!("Saved audio settings from AppState to centralized settings");
    Ok(())
}
