// Main module for all Tauri commands, broken down by category.

use crate::utils::{gather_system_context, format_system_context_for_agent};
use crate::state::AppState;
use tauri::{State, Emitter, AppHandle, WebviewUrl, WebviewWindowBuilder, Manager};
use tracing::{warn, info};

// Declare the submodules
pub mod registry;
pub mod app_url;
pub mod autostart;
pub mod core;
pub mod dev;
pub mod dictation;
// Removed deprecated dictation_reset module
pub mod dictation_state_manager;
pub mod element;
pub mod filesystem;
pub mod floating_bar;
pub mod floating_panel;
pub mod keyboard;
pub mod mouse;
pub mod native_permissions;
pub mod permissions;
pub mod providers;
pub mod shell;
pub mod shortcuts;
pub mod text_editor;
pub mod window;
pub mod orchestrator;
pub mod sound;
pub mod tools;
pub mod cloud;
pub mod mcp;
pub mod memory;
pub mod always_listening;
pub mod notifications;
pub mod stop_operations;
pub mod onboarding;
pub mod settings;
pub mod tray_commands;
pub mod testing;
pub mod ui_token_selection;
pub mod error_recovery;

// Re-export commands for easy access in lib.rs
pub use self::autostart::*;
pub use self::core::*;
// Removed unused dev import: pub use self::dev::*;
pub use self::dictation::*;
// Removed deprecated dictation_reset exports
pub use self::dictation_state_manager::{
    force_reset_dictation_state,
    get_dictation_comprehensive_status,
    update_dictation_component_state,
    transition_dictation_state
};
pub use self::floating_bar::{
    floating_bar_click, floating_bar_focus_change, floating_bar_input_blur,
    floating_bar_input_change, floating_bar_submit, get_floating_bar_config,
    set_floating_bar_config, handle_backend_response, handle_dictation_started,
    handle_dictation_partial, handle_dictation_finished, handle_tts_started,
    handle_tts_finished, handle_dictation_mode_change, handle_always_listening_change,
    handle_agent_started, handle_agent_stopped, handle_agent_cancelled,
    initialize_bar_manager
};
pub use self::floating_panel::*;
pub use self::filesystem::{dev_list_files, dev_get_file_content, dev_set_file_content, save_agent_response};
pub use self::mouse::*;
pub use self::permissions::*;
pub use self::shell::*;
pub use self::shortcuts::*;
pub use self::orchestrator::*;
pub use self::sound::*;
pub use self::tools::*;
pub use self::cloud::*;
pub use self::mcp::*;
pub use self::memory::*;
pub use self::always_listening::*;
pub use self::stop_operations::*;
pub use self::onboarding::*;
pub use self::ui_token_selection::*;
pub use self::settings::*;
pub use self::error_recovery::*;

// Explicitly re-export tool functions to ensure they're available
pub use self::tools::{
    get_tool_configurations,
    get_tool_config,
    set_tool_enabled,
    set_tool_category_enabled,
    get_enabled_tools,
    is_tool_enabled,
    reset_tool_configuration,
    get_tool_configuration_summary,
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
    app.emit("dev-tool-notification", payload)
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
        Err(e) => Err(format!("Failed to gather system context: {}", e))
    }
}

/// Load audio settings from centralized settings into AppState
/// Used by: Application startup for audio configuration initialization
pub async fn load_audio_settings_from_centralized_settings(
    app_handle: &tauri::AppHandle,
    state: &crate::state::AppState
) -> Result<(), String> {
    use crate::settings::manager::SettingsManager;

    let settings_manager = SettingsManager::new(app_handle.clone())
        .map_err(|e| format!("Failed to create settings manager: {}", e))?;

    let audio_settings = settings_manager.get_audio_settings().await
        .map_err(|e| format!("Failed to get audio settings: {}", e))?;

    // Update AppState with centralized settings values
    if let Ok(mut tts_provider) = state.tts_provider.lock() {
        *tts_provider = audio_settings.tts_provider;
    }

    if let Ok(mut always_listening_active) = state.always_listening_active.lock() {
        *always_listening_active = audio_settings.always_listening_active;
    }

    if let Ok(mut always_listening_sensitivity) = state.always_listening_sensitivity.lock() {
        *always_listening_sensitivity = audio_settings.always_listening_sensitivity;
    }

    if let Ok(mut always_listening_wake_words) = state.always_listening_wake_words.lock() {
        *always_listening_wake_words = audio_settings.always_listening_wake_words;
    }

    tracing::info!("Loaded audio settings from centralized settings into AppState");
    Ok(())
}

/// Save current AppState audio settings to centralized settings
/// Used by: Application shutdown and settings synchronization
pub async fn save_audio_settings_to_centralized_settings(
    app_handle: &tauri::AppHandle,
    state: &crate::state::AppState
) -> Result<(), String> {
    use crate::settings::manager::SettingsManager;

    let settings_manager = SettingsManager::new(app_handle.clone())
        .map_err(|e| format!("Failed to create settings manager: {}", e))?;

    let mut audio_settings = settings_manager.get_audio_settings().await
        .map_err(|e| format!("Failed to get audio settings: {}", e))?;

    // Update centralized settings with current AppState values
    if let Ok(tts_provider) = state.tts_provider.lock() {
        audio_settings.tts_provider = tts_provider.clone();
    }

    if let Ok(always_listening_active) = state.always_listening_active.lock() {
        audio_settings.always_listening_active = *always_listening_active;
    }

    if let Ok(always_listening_sensitivity) = state.always_listening_sensitivity.lock() {
        audio_settings.always_listening_sensitivity = *always_listening_sensitivity;
    }

    if let Ok(always_listening_wake_words) = state.always_listening_wake_words.lock() {
        audio_settings.always_listening_wake_words = always_listening_wake_words.clone();
    }

    settings_manager.set_audio_settings(&audio_settings).await
        .map_err(|e| format!("Failed to save audio settings: {}", e))?;

    tracing::info!("Saved audio settings from AppState to centralized settings");
    Ok(())
}

/// All available Tauri commands
pub fn get_all_commands() -> Vec<tauri::Command<tauri::Wry>> {
    vec![
        core::execute_system_command,
        core::get_app_context,
        core::launch_application,
        app_url::get_app_url,
        autostart::get_autostart,
        autostart::set_autostart,
        shortcuts::register_shortcut,
        shortcuts::unregister_shortcut,
        filesystem::read_file,
        filesystem::write_file,
        filesystem::list_directory,
        filesystem::create_directory,
        filesystem::delete_file,
        filesystem::delete_directory,
        filesystem::file_exists,
        filesystem::directory_exists,
        mouse::click,
        mouse::double_click,
        mouse::right_click,
        mouse::move_mouse,
        mouse::drag,
        mouse::scroll,
        mouse::get_cursor_position,
        keyboard::type_text,
        keyboard::key_press,
        keyboard::key_combination,
        keyboard::hotkey,
        element::find_element,
        element::click_element,
        element::type_in_element,
        element::get_element_attributes,
        element::screenshot_element,
        window::get_window_list,
        window::get_focused_window,
        window::set_window_focus,
        window::get_window_info,
        window::close_window,
        window::minimize_window,
        window::maximize_window,
        window::resize_window,
        window::move_window,
        floating_panel::toggle_floating_panel,
        floating_panel::update_floating_panel_state,
        floating_panel::get_floating_panel_state,
        floating_bar::toggle_floating_bar,
        floating_bar::update_floating_bar_state,
        floating_bar::get_floating_bar_state,
        shell::execute_command,
        shell::kill_process,
        shell::get_running_processes,
        text_editor::open_editor,
        text_editor::save_file,
        text_editor::get_editor_content,
        dictation::start_dictation,
        dictation::stop_dictation,
        dictation::toggle_dictation,
        dictation::get_dictation_status,
        dictation_state_manager::get_dictation_state,
        always_listening::start_always_listening,
        always_listening::stop_always_listening,
        always_listening::toggle_always_listening,
        always_listening::get_always_listening_status,
        always_listening::set_always_listening_sensitivity,
        always_listening::get_always_listening_sensitivity,
        always_listening::set_always_listening_wake_words,
        always_listening::get_always_listening_wake_words,
        permissions::check_accessibility_permissions,
        permissions::request_accessibility_permissions,
        permissions::check_all_permissions,
        providers::get_available_providers,
        providers::get_current_provider,
        providers::set_current_provider,
        providers::set_provider_api_key,
        providers::get_provider_models,
        providers::test_provider_connection,
        cloud::login,
        cloud::logout,
        cloud::get_auth_status,
        cloud::sync_settings,
        cloud::get_cloud_status,
        cloud::set_cloud_enabled,
        orchestrator::execute_orchestrated_task,
        orchestrator::get_orchestrator_status,
        orchestrator::cancel_orchestrator_task,
        orchestrator::get_task_history,
        orchestrator::get_active_tasks,
        orchestrator::create_task,
        orchestrator::execute_parallel_orchestration,
        orchestrator::get_orchestrator_performance_metrics,
        orchestrator::get_agent_performance_metrics,
        orchestrator::analyze_task_parallelization,
        orchestrator::execute_workflow_template,
        tray_commands::set_tray_icon,
        tray_commands::get_tray_icon_status,
        tray_commands::toggle_tray_icon,
        tray_commands::set_tray_menu_visibility,
        tray_commands::get_tray_menu_status,
        tray_commands::reset_tray_icon,
        tray_commands::set_tray_icon_priority,
        tray_commands::get_tray_icon_priority,
        tray_commands::enable_automatic_tray_updates,
        tools::get_all_tools,
        tools::get_tool_configuration,
        tools::configure_tool,
        tools::enable_tool,
        tools::disable_tool,
        tools::get_tool_categories,
        tools::configure_tool_category,
        tools::execute_tool,
        memory::get_memory_stats,
        memory::get_conversation_summary,
        memory::optimize_memory,
        memory::clear_memory,
        memory::set_memory_configuration,
        error_recovery::create_checkpoint,
        error_recovery::list_checkpoints,
        error_recovery::get_checkpoint_details,
        error_recovery::rollback_to_checkpoint,
        error_recovery::get_rollback_options,
        error_recovery::get_recovery_stats,
        error_recovery::get_execution_timeline,
        error_recovery::analyze_error_patterns,
        dev::keyboard::test_key_sequence,
        dev::keyboard::get_key_sequence_info,
        native_permissions::check_permissions_status,
        native_permissions::request_permissions,
    ]
}


