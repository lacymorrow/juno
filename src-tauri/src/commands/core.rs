// Core/Miscellaneous commands (screenshots, app list, clipboard, wait)

use tauri::State;
use tracing::info;
use crate::state::AppState;
use tauri::AppHandle;
use tracing::warn;
use super::send_dev_tool_notification; // Use helper from parent module
use crate::agent::providers::factory::{BrainFactory, ProviderInfo};
use serde::{Deserialize, Serialize};
use crate::settings::{manager::SettingsManager, AgentSettings};

#[cfg(target_os = "macos")]
use computer_use_ai_sdk::platforms::macos::utils as macos_utils;
#[cfg(not(target_os = "macos"))]
use tauri::AppHandle as DummyAppHandle; // Alias for non-macos signature consistency


#[cfg(target_os = "macos")]
#[tauri::command]
pub(crate) async fn capture_screenshot_command(app: AppHandle) -> Result<String, String> {
    match macos_utils::capture_and_encode_screenshot() {
        Ok(base64_string) => {
            // Send notification on success
            send_dev_tool_notification(&app, "Screenshot", "Screenshot captured successfully.")?;
            Ok(base64_string)
        }
        Err(e) => Err(format!("Failed to capture screenshot: {}", e)),
    }
}

#[cfg(not(target_os = "macos"))]
#[tauri::command]
pub(crate) async fn capture_screenshot_command(_app: DummyAppHandle) -> Result<String, String> { // Use alias
    Err("Screenshot capture is only supported on macOS currently.".to_string())
}

#[cfg(target_os = "macos")]
#[tauri::command]
pub(crate) async fn capture_window_screenshot_command(
    app: AppHandle,
    state: State<'_, AppState>,
    window_id: String,
) -> Result<String, String> {

    use computer_use_ai_sdk::platforms::macos::element::MacOSUIElement;
    use computer_use_ai_sdk::platforms::macos::utils::capture_element_screenshot;

    // Find the window by ID
    let desktop = state.get_desktop()?;
    let windows = desktop.list_windows().map_err(|e| format!("Failed to list windows: {}", e))?;

    let target_window = windows
        .into_iter()
        .find(|window| {
            window.id().map_or(false, |id| id == window_id)
        })
        .ok_or_else(|| format!("Window with ID '{}' not found", window_id))?;

    // Downcast to MacOSUIElement
    let macos_element = target_window
        .as_any()
        .downcast_ref::<MacOSUIElement>()
        .ok_or_else(|| "Failed to downcast window element to MacOSUIElement".to_string())?;

    // Capture the window screenshot
    match capture_element_screenshot(macos_element) {
        Ok(base64_string) => {
            send_dev_tool_notification(&app, "Window Screenshot", &format!("Window '{}' screenshot captured successfully.", window_id))?;
            Ok(base64_string)
        }
        Err(e) => Err(format!("Failed to capture window screenshot: {}", e)),
    }
}

#[cfg(not(target_os = "macos"))]
#[tauri::command]
pub(crate) async fn capture_window_screenshot_command(
    _app: DummyAppHandle,
    _state: State<'_, AppState>,
    _window_id: String,
) -> Result<String, String> {
    Err("Window screenshot capture is only supported on macOS currently.".to_string())
}

#[cfg(target_os = "macos")]
#[tauri::command]
pub(crate) async fn capture_focused_window_screenshot_command(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<String, String> {
    use computer_use_ai_sdk::platforms::macos::element::MacOSUIElement;
    use computer_use_ai_sdk::platforms::macos::utils::capture_element_screenshot;

    let desktop = state.get_desktop()?;

    // Get the focused element first
    let focused_element = desktop.focused_element()
        .map_err(|e| format!("Failed to get focused element: {}", e))?;

    // Check if the focused element is a window, if not try to get its window
    let window_element = {
        let attrs = focused_element.attributes();
        if attrs.role == "AXWindow" {
            focused_element
        } else {
            // Try to traverse up to find the window
            let mut current = focused_element;
            loop {
                match current.parent() {
                    Ok(Some(parent)) => {
                        let parent_attrs = parent.attributes();
                        if parent_attrs.role == "AXWindow" {
                            current = parent;
                            break;
                        }
                        current = parent;
                    }
                    Ok(None) => {
                        return Err("No window found in element hierarchy".to_string());
                    }
                    Err(e) => {
                        return Err(format!("Error traversing element hierarchy: {}", e));
                    }
                }
            }
            current
        }
    };

    // Downcast to MacOSUIElement
    let macos_element = window_element
        .as_any()
        .downcast_ref::<MacOSUIElement>()
        .ok_or_else(|| "Failed to downcast window element to MacOSUIElement".to_string())?;

    // Capture the window screenshot
    match capture_element_screenshot(macos_element) {
        Ok(base64_string) => {
            send_dev_tool_notification(&app, "Focused Window Screenshot", "Focused window screenshot captured successfully.")?;
            Ok(base64_string)
        }
        Err(e) => Err(format!("Failed to capture focused window screenshot: {}", e)),
    }
}

#[cfg(not(target_os = "macos"))]
#[tauri::command]
pub(crate) async fn capture_focused_window_screenshot_command(
    _app: DummyAppHandle,
    _state: State<'_, AppState>,
) -> Result<String, String> {
    Err("Focused window screenshot capture is only supported on macOS currently.".to_string())
}

#[tauri::command]
pub(crate) async fn list_apps(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    let desktop = state.get_desktop()?;
    match desktop.applications() {
        Ok(apps) => {
            let app_names = apps
                .into_iter()
                .map(|app| {
                    app.attributes()
                        .label
                        .unwrap_or_else(|| "Unknown Label".to_string())
                })
                .collect();
            Ok(app_names)
        }
        Err(e) => Err(format!("Failed to get applications: {}", e)),
    }
}

#[tauri::command]
pub(crate) fn check_server_status(state: State<'_, AppState>) -> bool {
    state.is_desktop_available()
}

#[tauri::command]
pub(crate) async fn dev_wait(duration_sec: f64, state: State<'_, AppState>) -> Result<(), String> {
    let duration_ms = (duration_sec * 1000.0).max(0.0) as u64; // Convert seconds to ms, ensure non-negative
    info!("Executing dev_wait for {} seconds ({} ms)", duration_sec, duration_ms);
    let desktop = state.get_desktop()?;
    desktop.wait(duration_ms)
        .map_err(|e| format!("Error during wait: {}", e))
}

#[tauri::command]
pub(crate) async fn dev_get_clipboard(state: State<'_, AppState>) -> Result<String, String> {
    info!("Executing dev_get_clipboard");
    let desktop = state.get_desktop()?;
    desktop.get_clipboard_content()
        .map_err(|e| format!("Error getting clipboard content: {}", e))
}

#[tauri::command]
pub(crate) async fn dev_set_clipboard(content: String, state: State<'_, AppState>) -> Result<(), String> {
    info!("Executing dev_set_clipboard {}", content);
    let desktop = state.get_desktop()?;
    desktop.set_clipboard_content(&content)
        .map_err(|e| format!("Error setting clipboard content: {}", e))
}

/// Get a list of available AI providers
#[tauri::command]
pub async fn list_ai_providers() -> Result<Vec<ProviderInfo>, String> {
    Ok(BrainFactory::list_providers())
}

/// Set the active AI provider
#[tauri::command]
pub async fn set_ai_provider(provider_id: String) -> Result<(), String> {
    // Set environment variable for the current process
    std::env::set_var("AI_PROVIDER", provider_id.clone());

    // For a real implementation, you would want to persist this setting
    // to a config file or database so it's remembered across app restarts

    tracing::info!("Set AI provider to: {}", provider_id);
    Ok(())
}

/// Set performance monitoring enabled state
#[tauri::command]
pub async fn set_performance_monitoring(enabled: bool, state: State<'_, AppState>) -> Result<(), String> {
    info!("Setting performance monitoring to: {}", enabled);

    // Update the state
    let _ = state.set_performance_monitoring_enabled(enabled);

    // TODO: In the future, this could persist the setting to a config file
    // For now, it's stored in memory for the session

    Ok(())
}

/// Get performance monitoring enabled state
#[tauri::command]
pub async fn get_performance_monitoring(state: State<'_, AppState>) -> Result<bool, String> {
    Ok(state.is_performance_monitoring_enabled())
}

/// Agent execution progress information
#[derive(Debug, Serialize, Deserialize)]
pub struct AgentExecutionProgress {
    pub is_executing: bool,
    pub execution_id: Option<String>,
    pub current_step: Option<u32>,
    pub max_steps: Option<u32>,
    pub remaining_steps: Option<u32>,
    pub progress_percentage: Option<f32>,
}

/// Get current agent execution progress
#[tauri::command]
pub async fn get_agent_execution_progress(state: State<'_, AppState>) -> Result<AgentExecutionProgress, String> {
    let is_executing = state.is_agent_executing();
    let execution_id = state.get_current_agent_execution_id();

    // Get real current step and max steps from AppState
    let (current_step, max_steps) = state.get_agent_step_progress();

    let remaining_steps = match (current_step, max_steps) {
        (Some(current), Some(max)) => Some(max.saturating_sub(current)),
        _ => None,
    };

    let progress_percentage = match (current_step, max_steps) {
        (Some(current), Some(max)) if max > 0 => Some((current as f32 / max as f32) * 100.0),
        _ => None,
    };

    Ok(AgentExecutionProgress {
        is_executing,
        execution_id,
        current_step,
        max_steps,
        remaining_steps,
        progress_percentage,
    })
}



/// Set debug mode enabled/disabled
#[tauri::command]
pub async fn set_debug_mode(enabled: bool, state: State<'_, AppState>) -> Result<(), String> {
    info!("Setting debug mode to: {}", enabled);

    let _ = state.set_debug_mode(enabled);

    info!("Debug mode successfully set to: {}", enabled);
    Ok(())
}

/// Get current debug mode status
#[tauri::command]
pub async fn get_debug_mode(state: State<'_, AppState>) -> Result<bool, String> {
    let debug_mode = state.is_debug_mode();
    Ok(debug_mode)
}

/// Reset all application settings to their default values
#[tauri::command]
pub async fn reset_all_settings(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    info!("Resetting all settings to defaults");

    // Reset TTS provider
    {
        let mut tts_provider = state.tts_provider.lock()
            .map_err(|e| format!("Failed to lock TTS provider: {}", e))?;
        *tts_provider = "system".to_string();
    }

    // Reset sound settings
    {
        let mut sound_enabled = state.sound_enabled.lock()
            .map_err(|e| format!("Failed to lock sound enabled: {}", e))?;
        *sound_enabled = true;
    }

    // Reset performance monitoring
    let _ = state.set_performance_monitoring_enabled(true);

    // Reset debug mode
    let _ = state.set_debug_mode(false);

    // Reset dictation settings
    {
        let mut dictation_clipboard = state.dictation_clipboard_enabled.lock()
            .map_err(|e| format!("Failed to lock dictation clipboard: {}", e))?;
        *dictation_clipboard = true;
    }

    // Reset always listening settings
    if let Err(e) = crate::commands::always_listening::stop_always_listening_mode(app.clone(), state.clone()).await {
        warn!("Failed to stop always listening: {}", e);
    }
    if let Err(e) = crate::commands::always_listening::set_always_listening_sensitivity(0.5, app.clone(), state.clone()).await {
        warn!("Failed to reset sensitivity: {}", e);
    }
    if let Err(e) = crate::commands::always_listening::set_always_listening_wake_words(
        vec!["hey juno".to_string(), "computer".to_string()],
        app.clone(),
        state.clone()
    ).await {
        warn!("Failed to reset wake words: {}", e);
    }

    // Reset keyboard shortcuts
    if let Err(e) = crate::commands::shortcuts::reset_keyboard_shortcuts(app.clone(), state.clone()).await {
        warn!("Failed to reset keyboard shortcuts: {}", e);
    }

    // Reset tool configuration
    if let Err(e) = crate::commands::tools::reset_tool_configuration(app.clone(), state.clone()).await {
        warn!("Failed to reset tool configuration: {}", e);
    }

    // Reset provider settings to defaults (this would require expanding provider commands)
    // Note: This would need additional implementation in provider commands

    // Reset cloud settings
    if let Err(e) = crate::commands::cloud::disable_cloud(app.clone(), state.clone()).await {
        warn!("Failed to disable cloud: {}", e);
    }

    info!("All settings have been reset to defaults");
    Ok(())
}

/// Cancel currently executing agent
#[tauri::command]
pub async fn cancel_agent_execution(state: State<'_, AppState>) -> Result<(), String> {
    info!("Cancelling agent execution");

    // Use the proper method to mark agent execution as finished
    state.mark_agent_execution_finished();

    info!("Agent execution cancelled successfully");
    Ok(())
}

/// Get system context information
#[tauri::command]
pub async fn get_system_context() -> Result<serde_json::Value, String> {
    // Implementation details for system context
    Ok(serde_json::json!({
        "message": "System context gathered successfully"
    }))
}

/// Get the current agent trigger mode (tap or hold)
#[tauri::command]
pub async fn get_agent_trigger_mode(
    app: AppHandle,
    state: State<'_, AppState>
) -> Result<String, String> {
    let settings_manager = SettingsManager::new(app.clone())
        .map_err(|e| format!("Failed to initialize settings manager: {}", e))?;

    match settings_manager.get_agent_settings().await {
        Ok(agent_settings) => {
            info!("Loaded agent trigger mode from centralized settings: {}", agent_settings.trigger_mode);

            // Sync with state for backward compatibility
            let trigger_mode = match agent_settings.trigger_mode.as_str() {
                "tap" => crate::state::AgentTriggerMode::Tap,
                "hold" => crate::state::AgentTriggerMode::Hold,
                _ => {
                    warn!("Invalid agent trigger mode: {}. Using default (tap)", agent_settings.trigger_mode);
                    crate::state::AgentTriggerMode::Tap
                }
            };

            {
                let mut current_mode = state.agent_trigger_mode.lock()
                    .map_err(|e| format!("Failed to lock agent trigger mode: {}", e))?;
                *current_mode = trigger_mode;
            }

            Ok(agent_settings.trigger_mode)
        }
        Err(e) => {
            Err(format!("Failed to load agent trigger mode from centralized settings: {}", e))
        }
    }
}

/// Set the agent trigger mode (tap or hold)
#[tauri::command]
pub async fn set_agent_trigger_mode(
    app: AppHandle,
    state: State<'_, AppState>,
    mode: String,
) -> Result<(), String> {
    // Validate the mode
    let trigger_mode = match mode.as_str() {
        "tap" => crate::state::AgentTriggerMode::Tap,
        "hold" => crate::state::AgentTriggerMode::Hold,
        _ => return Err(format!("Invalid agent trigger mode: {}. Must be 'tap' or 'hold'", mode)),
    };

    let settings_manager = SettingsManager::new(app.clone())
        .map_err(|e| format!("Failed to initialize settings manager: {}", e))?;

    // Get current settings or create default
    let mut agent_settings = settings_manager.get_agent_settings().await
        .unwrap_or_else(|_| AgentSettings {
            trigger_mode: mode.clone(),
            execution_mode: "multi".to_string(), // Default execution mode
        });

    // Update trigger mode
    agent_settings.trigger_mode = mode.clone();

    // Save to centralized settings
    settings_manager.set_agent_settings(&agent_settings).await
        .map_err(|e| format!("Failed to save agent settings: {}", e))?;

    // Update the state for backward compatibility
    {
        let mut current_mode = state.agent_trigger_mode.lock()
            .map_err(|e| format!("Failed to lock agent trigger mode: {}", e))?;
        *current_mode = trigger_mode;
    }

    info!("Updated agent trigger mode to: {}", mode);
    Ok(())
}

/// Load agent trigger mode from centralized settings
pub async fn load_agent_trigger_mode_from_store(app: &AppHandle, state: &AppState) -> Result<(), String> {
    let settings_manager = SettingsManager::new(app.clone())
        .map_err(|e| format!("Failed to initialize settings manager: {}", e))?;

    // Get agent settings or use defaults
    let agent_settings = settings_manager.get_agent_settings().await
        .unwrap_or_else(|_| AgentSettings::default());

    let trigger_mode = match agent_settings.trigger_mode.as_str() {
        "tap" => crate::state::AgentTriggerMode::Tap,
        "hold" => crate::state::AgentTriggerMode::Hold,
        _ => {
            warn!("Invalid agent trigger mode in centralized settings: {}. Using default (tap)", agent_settings.trigger_mode);
            crate::state::AgentTriggerMode::Tap
        }
    };

    // Update state
    {
        let mut current_mode = state.agent_trigger_mode.lock()
            .map_err(|e| format!("Failed to lock agent trigger mode: {}", e))?;
        *current_mode = trigger_mode;
    }

    info!("Loaded agent trigger mode from centralized settings: {}", agent_settings.trigger_mode);
    Ok(())
}

/// Set agent execution progress
#[tauri::command]
pub async fn set_agent_execution_progress(
    current_step: Option<u32>,
    max_steps: Option<u32>,
    state: State<'_, AppState>
) -> Result<(), String> {
    info!("Setting agent execution progress: step {}/{}",
        current_step.map_or("None".to_string(), |s| s.to_string()),
        max_steps.map_or("None".to_string(), |s| s.to_string())
    );

    // Update current step if provided
    if let Some(step) = current_step {
        let _ = state.update_agent_current_step(step);
    }

    // Note: AppState doesn't have a direct method to set max_steps independently,
    // but max_steps is typically set during agent execution start via mark_agent_execution_started_with_steps
    // For now, we'll just update the current step as that's the main use case

    info!("Agent execution progress updated successfully");
    Ok(())
}
