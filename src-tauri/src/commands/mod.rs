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
pub mod config_file;
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
pub mod persistent_memory;
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
pub mod whisper_model;
pub mod window; // Debug commands for tool configuration diagnostics

// Re-export commands for easy access in lib.rs
pub use self::accessibility::{
    accessibility_scan, accessibility_click, test_accessibility_permissions,
    get_accessibility_tool_definitions, execute_accessibility_tool
};
pub use self::safari_tools::{
    safari_is_active, safari_extract_dom, safari_click_element, safari_type_text,
    safari_get_url, safari_navigate, safari_list_clickable_elements,
    safari_execute_javascript, safari_clear_cache, execute_safari_tool
};
pub use self::autostart::*;
pub use self::computer::*;
pub use self::core::*;
// Removed unused dev import: pub use self::dev::*;
pub use self::dictation::*;
// Removed deprecated dictation_reset exports
pub use self::always_listening::*;
pub use self::cloud::*;
pub use self::cloud_test::*;
pub use self::config_file::*;
pub use self::collaborative_ai_commands::*;
pub use self::debug_tools::*; // Re-export debug tool commands
pub use self::dictation_state_manager::{
    force_reset_dictation_state, get_dictation_comprehensive_status, transition_dictation_state,
    update_dictation_component_state,
};
// Exports from dev2 branch - preserving existing functionality
// Note: Specific exports from element, keyboard, text_editor, window modules
// are not publicly re-exported as they don't have pub visibility
// Exports from main branch - new features
pub use self::enhanced_visual_reasoning_commands::{
    VisualReasoningState, VisualAnalysisRequest, SceneTypeInfo, TestResult, initialize_visual_reasoning_state
};
pub use self::error_recovery::*;
pub use self::filesystem::{get_file_content, list_files, save_agent_response, set_file_content};
// Floating bar functionality fully migrated to ui_commands.rs - no longer needed
pub use self::ui_commands::*; // Re-export consolidated UI API commands
pub use self::mcp::*;
pub use self::memory::*;
pub use self::mouse::*;
pub use self::onboarding::*;
pub use self::orchestrator::*;
pub use self::permissions::*;
// pub use self::self_improvement::*; // TODO: Fix module not found

pub use self::settings::*;
pub use self::shell::*;
pub use self::shortcuts::*;
pub use self::sound::*;
pub use self::stop_operations::*;
pub use self::tool_choice::*;
pub use self::ui_token_selection::*; // Re-export tool choice intelligence commands
pub use self::whisper_model::{
    download_whisper_model, get_current_whisper_model, get_whisper_download_status,
    get_whisper_models, set_whisper_model,
};

// Explicitly re-export tool functions to ensure they're available
pub use self::tools::{
    get_enabled_tools, get_registered_tools, get_tool_config, get_tool_configuration_summary, get_tool_configurations,
    is_tool_enabled, reset_tool_configuration, set_tool_category_enabled, set_tool_enabled, test_dynamic_tool_categorization,
    test_tool_config, test_tool_config_command,
    // Tool approval commands
    approve_tool_execution, clear_pending_tool_approvals, deny_tool_execution, get_pending_tool_approvals,
    get_tool_approval_required, set_tool_approval_required,
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
