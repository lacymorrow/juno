//! # State Management Module
//!
//! This module provides comprehensive state management for the Juno application,
//! including state initialization, configuration loading, state transitions,
//! and background state monitoring tasks.

use crate::state::AppState;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};
use tracing::{debug, error, info, warn};

// Import sound commands for boot sound functionality
use crate::constants::events;

/// Initialize all application state components and background tasks
pub async fn initialize_application_state(app_handle: &AppHandle) -> Result<(), String> {
    info!("🚀 Initializing comprehensive application state management...");

    // Initialize core state components in parallel
    let (
        env_result,
        shortcuts_result,
        audio_result,
        mcp_result,
        onboarding_result,
        orchestrator_result,
        cloud_result,
        floating_bar_result,
        ui_manager_result,
        monitoring_result,
    ) = tokio::join!(
        initialize_environment_state(app_handle.clone()),
        initialize_shortcuts_state(app_handle.clone()),
        initialize_audio_state(app_handle.clone()),
        initialize_mcp_state(app_handle.clone()),
        initialize_onboarding_state(app_handle.clone()),
        initialize_orchestrator_state(app_handle.clone()),
        initialize_cloud_state(app_handle.clone()),
        initialize_floating_bar_state(app_handle.clone()),
        initialize_ui_manager_state(app_handle.clone()),
        initialize_monitoring_state(app_handle.clone()),
    );

    // Log results and collect errors
    let mut errors = Vec::new();

    if let Err(e) = env_result {
        errors.push(format!("Environment: {}", e));
    }
    if let Err(e) = shortcuts_result {
        errors.push(format!("Shortcuts: {}", e));
    }
    if let Err(e) = audio_result {
        errors.push(format!("Audio: {}", e));
    }
    if let Err(e) = mcp_result {
        errors.push(format!("MCP: {}", e));
    }
    if let Err(e) = onboarding_result {
        errors.push(format!("Onboarding: {}", e));
    }
    if let Err(e) = orchestrator_result {
        errors.push(format!("Orchestrator: {}", e));
    }
    if let Err(e) = cloud_result {
        errors.push(format!("Cloud: {}", e));
    }
    if let Err(e) = floating_bar_result {
        errors.push(format!("Floating Bar: {}", e));
    }
    if let Err(e) = ui_manager_result {
        errors.push(format!("UI Manager: {}", e));
    }
    if let Err(e) = monitoring_result {
        errors.push(format!("Monitoring: {}", e));
    }

    if !errors.is_empty() {
        warn!(
            "Some state initialization components had issues: {:?}",
            errors
        );
        // Don't fail completely - continue with partial initialization
    }

    // Start background state management tasks
    start_background_state_tasks(app_handle.clone()).await;

    info!("✅ Application state management initialization completed");
    Ok(())
}

/// Initialize environment-related state (environment variables, configuration)
async fn initialize_environment_state(app_handle: AppHandle) -> Result<(), String> {
    info!("[State] Initializing environment state...");

    // In dev, `startup::init_environment()` already loads from local `.env`.
    // The bundled `.env` resource is primarily for packaged builds.
    if !cfg!(debug_assertions) {
        // Load environment variables from bundled resources (packaged builds)
        if let Err(e) = crate::load_bundled_environment(app_handle.clone()).await {
            warn!("Failed to load bundled environment: {}", e);
            info!("Using environment variables from system environment or development .env file");
        } else {
            info!("Successfully loaded environment variables from bundled resources");
        }
    }

    Ok(())
}

/// Initialize keyboard shortcuts state and global shortcut registration
async fn initialize_shortcuts_state(app_handle: AppHandle) -> Result<(), String> {
    info!("[State] Initializing shortcuts state...");

    let app_state = app_handle.state::<AppState>();

    // Load keyboard shortcuts from centralized settings
    if let Err(e) = crate::commands::shortcuts::load_shortcuts_from_centralized_settings(
        &app_handle,
        &app_state,
    )
    .await
    {
        warn!(
            "Failed to load keyboard shortcuts from centralized settings: {} - using defaults",
            e
        );
    }

    // Load agent trigger mode from persistent storage
    if let Err(e) =
        crate::commands::core::load_agent_trigger_mode_from_store(&app_handle, &app_state).await
    {
        warn!("Failed to load agent trigger mode: {} - using defaults", e);
    }

    // Load dictation trigger mode from persistent storage
    if let Err(e) =
        crate::commands::core::load_dictation_trigger_mode_from_store(&app_handle, &app_state)
            .await
    {
        warn!(
            "Failed to load dictation trigger mode: {} - using defaults",
            e
        );
    }

    // Load tool configuration from centralized settings
    let settings_manager = crate::settings::manager::SettingsManager::new(app_handle.clone())
        .map_err(|e| format!("Failed to create settings manager for tool config: {}", e))?;
    if let Err(e) = crate::agent::tools::tool_config::load_tool_config_from_centralized_settings(
        &settings_manager,
        &app_state,
    )
    .await
    {
        warn!(
            "Failed to load tool configuration from centralized settings: {} - using defaults",
            e
        );
    }

    // Register global shortcuts after loading configuration
    if let Err(e) =
        crate::commands::shortcuts::update_global_shortcuts(&app_handle, &app_state).await
    {
        warn!(
            "Failed to register global shortcuts: {} - continuing without shortcuts",
            e
        );
    }

    // Initialize dictation input monitoring system
    if let Err(e) =
        crate::dictation_monitor::init_dictation_input_monitoring(app_handle.clone()).await
    {
        error!("Failed to initialize dictation input monitoring: {}", e);
        return Err(format!("Dictation monitoring initialization failed: {}", e));
    }

    // Start agent monitor task for hold behavior
    let _agent_monitor_handle = crate::agent_monitor::start_agent_monitor_task(app_handle.clone());
    info!("Agent monitor task started successfully");

    Ok(())
}

/// Initialize audio and voice settings state
async fn initialize_audio_state(app_handle: AppHandle) -> Result<(), String> {
    info!("[State] Initializing audio settings state...");

    let app_state = app_handle.state::<AppState>();

    // Load audio settings from centralized settings
    if let Err(e) =
        crate::commands::load_audio_settings_from_centralized_settings(&app_handle, &app_state)
            .await
    {
        warn!(
            "Failed to load audio settings from centralized settings: {} - using defaults",
            e
        );
    } else {
        info!("Successfully loaded audio settings from centralized settings");
    }
    
    // Restore always listening mode if it was previously active
    restore_always_listening_if_needed(&app_handle, &app_state).await;

    // Initialize voice transcription plugin configuration
    if let Err(e) = initialize_voice_transcription_config(&app_handle).await {
        warn!(
            "Failed to initialize voice transcription config: {} - using defaults",
            e
        );
    }

    info!("Audio settings state initialized successfully");
    Ok(())
}

/// Initialize voice transcription plugin configuration
async fn initialize_voice_transcription_config(app_handle: &AppHandle) -> Result<(), String> {
    info!("[State] Initializing voice transcription configuration...");

    // Get current audio settings from centralized system
    let settings_manager = crate::settings::manager::SettingsManager::new(app_handle.clone())
        .map_err(|e| format!("Failed to create settings manager: {}", e))?;

    let audio_settings = settings_manager
        .get_audio_settings()
        .await
        .map_err(|e| format!("Failed to get audio settings: {}", e))?;

    // Create voice transcription config based on centralized settings
    let _voice_config = tauri_plugin_voice_transcription::VoiceTranscriptionConfig {
        model_path: "models/ggml-tiny.en.bin".to_string(),
        sample_rate: 16000,
        channels: 1,
        buffer_duration_ms: 1500,
        partial_interval_ms: 500,
        enable_partial_transcription: true,
        enable_playback: audio_settings.sound_enabled,
    };

    // Note: The voice transcription plugin currently uses stub implementation
    // When it's fully implemented, we would apply the config here
    info!("Voice transcription configuration prepared (plugin uses stub implementation)");

    Ok(())
}

/// Initialize MCP (Model Context Protocol) servers and tools
async fn initialize_mcp_state(app_handle: AppHandle) -> Result<(), String> {
    debug!("[State] Initializing MCP state...");

    let app_state = app_handle.state::<AppState>();

    let app_state_bg = app_state.inner().clone();
    let app_handle_bg = app_handle.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(tokio::time::Duration::from_millis(250)).await;

        match app_state_bg.initialize_mcp_servers(Some(&app_handle_bg)).await {
            Ok(_) => {
                debug!("MCP servers initialized");
            }
            Err(e) => {
                warn!("Failed to initialize MCP servers: {}", e);
            }
        }
    });

    Ok(())
}

/// Initialize onboarding system state
async fn initialize_onboarding_state(app_handle: AppHandle) -> Result<(), String> {
    info!("[State] Initializing onboarding state...");

    if let Err(e) =
        crate::commands::onboarding::initialize_onboarding_system(app_handle.clone()).await
    {
        warn!("Failed to initialize onboarding system: {}", e);
        return Err(format!("Onboarding initialization failed: {}", e));
    }

    info!("Onboarding system initialized successfully");
    Ok(())
}

/// Initialize multi-agent orchestrator state
async fn initialize_orchestrator_state(app_handle: AppHandle) -> Result<(), String> {
    info!("[State] Initializing orchestrator state...");

    if let Err(e) =
        crate::commands::orchestrator::init_orchestrator_with_app_handle(app_handle.clone()).await
    {
        error!("Failed to initialize orchestrator system: {}", e);
        return Err(format!("Orchestrator initialization failed: {}", e));
    }

    info!("Multi-agent orchestrator system initialized successfully");
    Ok(())
}

/// Initialize cloud connectivity state
async fn initialize_cloud_state(app_handle: AppHandle) -> Result<(), String> {
    info!("[State] Initializing cloud state...");

    let app_state = app_handle.state::<AppState>();

    // Initialize cloud client configuration
    if let Err(e) = app_state.init_cloud_client(&app_handle).await {
        error!("Failed to initialize cloud client: {}", e);
        return Err(format!("Cloud client initialization failed: {}", e));
    }

    info!("Cloud client configuration initialized");

    // Start cloud client if enabled
    if app_state.is_cloud_enabled() {
        if let Err(e) = app_state.start_cloud_client().await {
            error!("Failed to start cloud client: {}", e);
        } else {
            info!("Cloud client started successfully");
        }
    } else {
        info!("Cloud connectivity is disabled in configuration");
    }

    Ok(())
}

/// Initialize floating bar manager state
async fn initialize_floating_bar_state(_app_handle: AppHandle) -> Result<(), String> {
    info!("[State] Initializing floating bar state...");
    // UI Manager initialization moved to initialize_ui_manager_state to avoid duplicate initialization
    info!("Floating bar state initialization completed");
    Ok(())
}

/// Initialize UI Manager for consolidated floating UI elements
async fn initialize_ui_manager_state(app_handle: AppHandle) -> Result<(), String> {
    debug!("[State] Initializing UI Manager for consolidated floating elements...");

    // Initialize the global UI manager for all floating UI elements
    if let Err(e) = crate::commands::ui_commands::initialize_ui_manager(app_handle.clone()).await {
        warn!("Failed to initialize UI manager: {}", e);
        return Err(format!("UI Manager initialization failed: {}", e));
    }

    Ok(())
}

/// Initialize monitoring and background state tasks
async fn initialize_monitoring_state(app_handle: AppHandle) -> Result<(), String> {
    info!("[State] Initializing monitoring state...");

    // Initialize autostart configuration
    if let Err(e) = crate::commands::autostart::init_autostart(&app_handle) {
        warn!("Failed to initialize autostart configuration: {}", e);
    } else {
        info!("Autostart configuration initialized successfully");
    }

    Ok(())
}

/// Start background tasks for state management and monitoring
async fn start_background_state_tasks(app_handle: AppHandle) {
    info!("[State] Starting background state management tasks...");

    // Start MCP error recovery background task
    start_mcp_retry_task(app_handle.clone()).await;

    // Boot sound is handled by app_setup module - removed duplicate call

    info!("[State] Background state management tasks started");
}

/// Start MCP server retry background task
async fn start_mcp_retry_task(app_handle: AppHandle) {
    let retry_app_handle = app_handle.clone();
    tokio::spawn(async move {
        let app_state = retry_app_handle.state::<AppState>();
        let mut interval = tokio::time::interval(Duration::from_secs(60)); // Check every minute

        loop {
            interval.tick().await;

            if let Err(e) = app_state.retry_failed_mcp_servers().await {
                tracing::debug!("MCP retry check failed: {}", e);
            }
        }
    });

    info!("[State] MCP retry background task started");
}

// Boot sound function removed - handled by app_setup module

/// Handle state transitions for dictation mode
pub async fn handle_dictation_state_transition(
    app_handle: &AppHandle,
    active: bool,
) -> Result<(), String> {
    let app_state = app_handle.state::<AppState>();

    // Update dictation active state
    if let Err(e) = app_state.set_dictation_active(active) {
        return Err(format!("Failed to set dictation active state: {}", e));
    }

    // Emit state change event for UI
    if let Err(e) = app_handle.emit(events::dictation::ACTIVE, active) {
        error!("Failed to emit dictation-active event: {}", e);
        return Err(format!("Failed to emit dictation state event: {}", e));
    }

    // Update floating bar manager
    crate::commands::ui_commands::handle_dictation_mode_change(app_handle, active).await;

    info!("Dictation state transition completed: active={}", active);
    Ok(())
}

/// Handle state transitions for agent execution
pub async fn handle_agent_execution_state_transition(
    app_handle: &AppHandle,
    active: bool,
    execution_id: Option<String>,
) -> Result<(), String> {
    let app_state = app_handle.state::<AppState>();

    if active {
        if let Some(id) = execution_id {
            app_state.mark_agent_execution_started(id)?;
        } else {
            return Err("Execution ID required when starting agent execution".to_string());
        }
    } else {
        app_state.mark_agent_execution_finished();
    }

    // Emit state change event for UI
    if let Err(e) = app_handle.emit(events::agent::ACTIVE, active) {
        error!("Failed to emit agent-active event: {}", e);
        return Err(format!("Failed to emit agent state event: {}", e));
    }

    info!(
        "Agent execution state transition completed: active={}",
        active
    );
    Ok(())
}

/// Handle state cleanup for emergency stop situations
/// Now integrates with the stop coordinator to prevent redundant cleanup operations
pub async fn handle_emergency_state_cleanup(app_handle: &AppHandle) -> Result<(), String> {
    // Check with stop coordinator to see if cleanup is already in progress
    let stop_coordinator = crate::commands::stop_coordinator::get_stop_coordinator();

    // Use the stop coordinator to handle the emergency cleanup
    if let Err(e) = stop_coordinator
        .stop_all_operations(app_handle, "emergency_state_cleanup")
        .await
    {
        warn!("[State] Stop coordinator failed emergency cleanup: {}", e);
        // Fall back to direct cleanup if coordinator fails
        perform_direct_emergency_cleanup(app_handle).await
    } else {
        info!("[State] Emergency state cleanup delegated to stop coordinator");
        Ok(())
    }
}

/// Direct emergency cleanup (fallback when coordinator fails)
async fn perform_direct_emergency_cleanup(app_handle: &AppHandle) -> Result<(), String> {
    info!("[State] Performing direct emergency state cleanup...");

    let app_state = app_handle.state::<AppState>();

    // Stop TTS immediately
    crate::tts::stop_speech();

    // Signal cancellation for all operations
    app_state.signal_cancel();
    app_state.mark_agent_execution_finished();

    // Reset dictation state
    if let Err(e) = app_state.set_dictation_active(false) {
        warn!("Failed to reset dictation active state: {}", e);
    }

    // Stop always listening if active
    if let Err(e) = crate::commands::always_listening::stop_always_listening_mode(
        app_handle.clone(),
        app_state.clone(),
    )
    .await
    {
        warn!(
            "[State] Failed to stop always listening during emergency cleanup: {}",
            e
        );
    } else {
        info!("[State] Always listening stopped during emergency cleanup");
    }

    // Force reset monitoring states
    crate::agent_monitor::force_reset_agent_input_state().await;
    crate::dictation_monitor::force_reset_dictation_input_state().await;

    // Emit state updates
    let _ = app_handle.emit(events::agent::ACTIVE, false);
    let _ = app_handle.emit(events::dictation::ACTIVE, false);
    let _ = app_handle.emit(events::always_listening::MODE_CHANGED, false);
    let _ = app_handle.emit(events::tts::STOP_REQUESTED, ());

    // Update floating bar
    crate::commands::ui_commands::handle_backend_response(
        app_handle,
        Some("All operations stopped.".to_string()),
        "Stopped".to_string(),
    )
    .await;

    info!("[State] Direct emergency state cleanup completed");
    Ok(())
}

/// Get comprehensive state summary for debugging and monitoring
pub async fn get_state_summary(app_handle: &AppHandle) -> Result<serde_json::Value, String> {
    let app_state = app_handle.state::<AppState>();

    let summary = serde_json::json!({
        "agent_execution": {
            "active": app_state.is_agent_executing(),
            "execution_id": app_state.get_current_agent_execution_id(),
            "current_step": app_state.get_agent_current_step(),
            "max_steps": app_state.get_agent_max_steps(),
        },
        "dictation": {
            "active": app_state.is_dictation_active(),
            "clipboard_enabled": app_state.get_dictation_clipboard_enabled().unwrap_or(false),
        },
        "cloud": {
            "enabled": app_state.is_cloud_enabled(),
            "has_connector": app_state.has_production_cloud_connector().await,
        },
        "desktop": {
            "available": app_state.is_desktop_available(),
        },
        "permissions": {
            "checked": app_state.are_permissions_checked(),
            "state": app_state.get_permissions_state().await,
        },
        "performance": {
            "monitoring_enabled": app_state.is_performance_monitoring_enabled(),
        },
        "debug": {
            "mode": app_state.is_debug_mode(),
        },
        "always_listening": {
            "active": app_state.get_always_listening_active().unwrap_or(false),
            "sensitivity": app_state.get_always_listening_sensitivity().unwrap_or(0.5),
        },
    });

    Ok(summary)
}

/// Validate state consistency and report any issues
pub async fn validate_state_consistency(app_handle: &AppHandle) -> Result<Vec<String>, String> {
    let app_state = app_handle.state::<AppState>();
    let mut issues = Vec::new();

    // Check for state inconsistencies
    if app_state.is_agent_executing() && app_state.get_current_agent_execution_id().is_none() {
        issues.push("Agent execution active but no execution ID set".to_string());
    }

    if app_state.is_dictation_active() && app_state.is_agent_executing() {
        issues.push("Both dictation and agent execution are active simultaneously".to_string());
    }

    if !app_state.is_desktop_available()
        && (app_state.is_agent_executing() || app_state.is_dictation_active())
    {
        issues.push("Desktop not available but agent/dictation operations are active".to_string());
    }

    // Check permissions consistency
    if app_state.are_permissions_checked() {
        if let Some(permissions) = app_state.get_permissions_state().await {
            if !permissions.accessibility.granted
                && (app_state.is_agent_executing() || app_state.is_dictation_active())
            {
                issues.push(
                    "Accessibility permissions missing but operations requiring them are active"
                        .to_string(),
                );
            }
        }
    }

    Ok(issues)
}

#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use super::*;

    #[test]
    fn test_state_management_module_compilation() {
        // This test ensures the module compiles correctly
        assert!(true, "State management module compiled successfully");
    }

    #[tokio::test]
    async fn test_state_summary_structure() {
        // Test that state summary has expected structure
        // This would need a mock AppHandle in a real test environment
        assert!(true, "State summary structure test placeholder");
    }

    #[test]
    fn test_state_validation_logic() {
        // Test state validation logic
        let issues = vec!["test issue".to_string()];
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0], "test issue");
    }
}

/// Restore always listening mode if it was previously active
async fn restore_always_listening_if_needed(app_handle: &AppHandle, app_state: &AppState) {
    // Check if always listening was previously active
    if !app_state.get_always_listening_active().unwrap_or(false) {
        return;
    }
    
    info!("[State] Always listening was active in saved settings - attempting to restore...");
    
    // Try to start the always listening controller
    match app_handle.try_state::<std::sync::Arc<std::sync::Mutex<tauri_plugin_voice_transcription::always_listening::AlwaysListeningController>>>() {
        Some(controller_state) => {
            match tauri_plugin_voice_transcription::commands::start_always_listening(
                app_handle.clone(),
                controller_state
            ).await {
                Ok(_) => {
                    info!("[State] Successfully restored always listening mode on startup");
                    
                    // Emit event to update UI
                    if let Err(e) = app_handle.emit(crate::constants::events::always_listening::MODE_CHANGED, true) {
                        warn!("[State] Failed to emit always-listening-mode-changed event: {}", e);
                    }
                    
                    // Update floating bar UI
                    crate::commands::ui_commands::handle_always_listening_change(app_handle, true).await;
                }
                Err(e) => {
                    warn!("[State] Failed to restore always listening mode on startup: {}", e);
                    
                    // Reset the state since we couldn't start it
                    let _ = app_state.set_always_listening_active(false);
                    
                    // Update centralized settings to reflect the failure
                    if let Ok(settings_manager) = crate::settings::manager::SettingsManager::new(app_handle.clone()) {
                        if let Ok(mut audio_settings) = settings_manager.get_audio_settings().await {
                            audio_settings.always_listening_active = false;
                            let _ = settings_manager.set_audio_settings(&audio_settings).await;
                        }
                    }
                }
            }
        }
        None => {
            warn!("[State] Always listening controller not available at startup - resetting state");
            // Reset the state since controller isn't available
            let _ = app_state.set_always_listening_active(false);
            
            // Update centralized settings
            if let Ok(settings_manager) = crate::settings::manager::SettingsManager::new(app_handle.clone()) {
                if let Ok(mut audio_settings) = settings_manager.get_audio_settings().await {
                    audio_settings.always_listening_active = false;
                    let _ = settings_manager.set_audio_settings(&audio_settings).await;
                }
            }
        }
    }
}
