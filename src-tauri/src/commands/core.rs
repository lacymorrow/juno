// Core/Miscellaneous commands (screenshots, app list, clipboard, wait)

use tauri::{State, Manager};
use tracing::{info, warn, error};
use crate::state::AppState;
use crate::settings::{SettingsManager, AppSettings};
use tauri::AppHandle;
use super::send_dev_tool_notification;
use crate::agent::providers::factory::{BrainFactory, ProviderInfo};
use serde::{Deserialize, Serialize};

#[cfg(target_os = "macos")]
use computer_use_ai_sdk::platforms::macos::utils as macos_utils;

#[cfg(target_os = "macos")]
mod macos_focused_element {
    use serde_json::Value;

    pub fn get_focused_element_details() -> Result<Value, String> {
        // Placeholder implementation - in a real implementation this would
        // use the computer_use_ai_sdk to get focused element details
        Ok(serde_json::json!({
            "role": "placeholder",
            "title": "Focused element details not implemented",
            "type": "placeholder"
        }))
    }
}
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

/// Set the active AI provider using SettingsManager
#[tauri::command]
pub async fn set_ai_provider(app: AppHandle, provider_id: String) -> Result<(), String> {
    let settings_manager = SettingsManager::new(app);

    // Update the provider setting
    settings_manager.update_section("providers.active_provider", serde_json::Value::String(provider_id.clone()))
        .await
        .map_err(|e| format!("Failed to update AI provider setting: {}", e))?;

    // Also set environment variable for the current process
    std::env::set_var("AI_PROVIDER", provider_id.clone());

    info!("Set AI provider to: {}", provider_id);
    Ok(())
}

/// Set performance monitoring enabled state using SettingsManager
#[tauri::command]
pub async fn set_performance_monitoring(app: AppHandle, enabled: bool, state: State<'_, AppState>) -> Result<(), String> {
    info!("Setting performance monitoring to: {}", enabled);

    let settings_manager = SettingsManager::new(app);

    // Update the performance monitoring setting
    settings_manager.update_section("performance.monitoring_enabled", serde_json::Value::Bool(enabled))
        .await
        .map_err(|e| format!("Failed to update performance monitoring setting: {}", e))?;

    // Update the state
    let _ = state.set_performance_monitoring_enabled(enabled);

    Ok(())
}

/// Get performance monitoring enabled state from SettingsManager
#[tauri::command]
pub async fn get_performance_monitoring(app: AppHandle) -> Result<bool, String> {
    let settings_manager = SettingsManager::new(app);

    let settings = settings_manager.get_settings();

    // Performance field removed from simplified schema
    Ok(false)
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

/// Set debug mode enabled/disabled using SettingsManager
#[tauri::command]
pub async fn set_debug_mode(app: AppHandle, enabled: bool, state: State<'_, AppState>) -> Result<(), String> {
    info!("Setting debug mode to: {}", enabled);

    let settings_manager = SettingsManager::new(app);

    // Update the debug mode setting
    settings_manager.update_section("performance.debug_mode", serde_json::Value::Bool(enabled))
        .await
        .map_err(|e| format!("Failed to update debug mode setting: {}", e))?;

    let _ = state.set_debug_mode(enabled);

    info!("Debug mode successfully set to: {}", enabled);
    Ok(())
}

/// Get current debug mode status from SettingsManager
#[tauri::command]
pub async fn get_debug_mode(app: AppHandle) -> Result<bool, String> {
    let settings_manager = SettingsManager::new(app);

    let settings = settings_manager.get_settings();

    // Performance field removed from simplified schema
    Ok(false)
}

/// Reset all application settings to their default values using SettingsManager
#[tauri::command]
pub async fn reset_all_settings(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    info!("Resetting all settings to defaults");

    let settings_manager = SettingsManager::new(app.clone());

    // Reset all settings to defaults
    settings_manager.reset_all()
        .await
        .map_err(|e| format!("Failed to reset settings: {}", e))?;

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

    // Stop always listening
    if let Err(e) = crate::commands::always_listening::stop_always_listening_mode(app.clone(), state.clone()).await {
        warn!("Failed to stop always listening: {}", e);
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

/// Get the current agent trigger mode (tap or hold) using SettingsManager
#[tauri::command]
pub async fn get_agent_trigger_mode(app: AppHandle) -> Result<String, String> {
    let settings_manager = SettingsManager::new(app);

    let settings = settings_manager.get_settings();

    Ok(settings.agent.trigger_mode.clone())
}

/// Set the agent trigger mode (tap or hold) using SettingsManager
#[tauri::command]
pub async fn set_agent_trigger_mode(app: AppHandle, state: State<'_, AppState>, mode: String) -> Result<(), String> {
    // Validate mode
    if mode != "tap" && mode != "hold" {
        return Err(format!("Invalid agent trigger mode: {}. Must be 'tap' or 'hold'", mode));
    }

    let settings_manager = SettingsManager::new(app);

    // Update the trigger mode setting
    settings_manager.update_section("agent.trigger_mode", serde_json::Value::String(mode.clone()))
        .await
        .map_err(|e| format!("Failed to update agent trigger mode: {}", e))?;

    // Update the state
    let trigger_mode = match mode.as_str() {
        "tap" => crate::state::AgentTriggerMode::Tap,
        "hold" => crate::state::AgentTriggerMode::Hold,
        _ => unreachable!(), // Already validated above
    };

    {
        let mut current_mode = state.agent_trigger_mode.lock()
            .map_err(|e| format!("Failed to lock agent trigger mode: {}", e))?;
        *current_mode = trigger_mode;
    }

    info!("Updated agent trigger mode to: {}", mode);
    Ok(())
}

/// Load agent trigger mode from SettingsManager
pub async fn load_agent_trigger_mode_from_settings(app: &AppHandle, state: &AppState) -> Result<(), String> {
    let settings_manager = SettingsManager::new(app.clone());

    let settings = settings_manager.get_settings();

    let trigger_mode = match settings.agent.trigger_mode.as_str() {
                "tap" => crate::state::AgentTriggerMode::Tap,
                "hold" => crate::state::AgentTriggerMode::Hold,
                _ => {
            warn!("Invalid agent trigger mode in settings: {}. Using default (tap)", settings.agent.trigger_mode);
                    crate::state::AgentTriggerMode::Tap
                }
            };

            let mut current_mode = state.agent_trigger_mode.lock()
                .map_err(|e| format!("Failed to lock agent trigger mode: {}", e))?;
            *current_mode = trigger_mode;

    info!("Loaded agent trigger mode from settings: {}", settings.agent.trigger_mode);
    Ok(())
}

/// Compatibility function for legacy code
pub async fn load_agent_trigger_mode_from_store(app: &AppHandle, state: &AppState) -> Result<(), String> {
    load_agent_trigger_mode_from_settings(app, state).await
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

/// Get all application settings via SettingsManager
#[tauri::command]
pub async fn get_all_app_settings(app: AppHandle) -> Result<AppSettings, String> {
    let settings_manager = SettingsManager::new(app);
    Ok(settings_manager.get_settings())
}

/// Update a specific settings section
#[tauri::command]
pub async fn update_settings_section(
    app: AppHandle,
    section_path: String,
    value: serde_json::Value,
) -> Result<(), String> {
    let settings_manager = SettingsManager::new(app);
    settings_manager.update_section(&section_path, value).await
        .map_err(|e| format!("Failed to update settings section '{}': {}", section_path, e))
}

/// Reset a settings section to defaults
#[tauri::command]
pub async fn reset_settings_section(app: AppHandle, section: String) -> Result<(), String> {
    let settings_manager = SettingsManager::new(app);
    let section_path = section.as_str();

    match section_path {
        "keyboard_shortcuts" | "floating_bar" | "agent" | "providers" | "cloud" | "audio" | "onboarding" => {
            settings_manager.reset_section(&section_path).await
        },
        _ => Err(format!("Unknown settings section: {}", section))
    }
}

/// Update multiple settings at once (atomic update)
#[tauri::command]
pub async fn update_multiple_settings(
    app: AppHandle,
    updates: Vec<(String, serde_json::Value)>,
) -> Result<(), String> {
    let settings_manager = SettingsManager::new(app);
    settings_manager.update_multiple(updates).await
        .map_err(|e| format!("Failed to update multiple settings: {}", e))
}

/// Get a specific settings section
#[tauri::command]
pub async fn get_settings_section(app: AppHandle, section: String) -> Result<serde_json::Value, String> {
    let settings_manager = SettingsManager::new(app);
    let section_path = section.as_str();
    settings_manager.get_section(&section_path).await
}

/// Migrate all legacy settings to centralized system
#[tauri::command]
pub async fn migrate_legacy_settings(app: AppHandle) -> Result<String, String> {
    info!("🔄 Starting migration of all legacy settings...");

    let settings_manager = SettingsManager::new(app.clone());

    // Use the migration system to import from all legacy stores
    // Skip the migration method since it's handled as no-op in SettingsManager now
    match settings_manager.migrate_from_legacy_stores().await {
        Ok(_) => {
            info!("✅ Successfully migrated legacy settings");
            Ok("Migration completed successfully".to_string())
        },
        Err(e) => {
            warn!("Failed to migrate legacy settings: {}", e);
            Err(format!("Migration failed: {}", e))
        }
    }
}

/// Export settings for backup
#[tauri::command]
pub async fn export_settings(app: AppHandle) -> Result<String, String> {
    let settings_manager = SettingsManager::new(app);

    let settings = settings_manager.get_settings();
    serde_json::to_string_pretty(&settings)
        .map_err(|e| format!("Failed to serialize settings for export: {}", e))
}

/// Import settings from backup
#[tauri::command]
pub async fn import_settings(
    app: AppHandle,
    settings_json: String,
) -> Result<(), String> {
    let settings: AppSettings = serde_json::from_str(&settings_json)
        .map_err(|e| format!("Failed to parse settings JSON: {}", e))?;

    let settings_manager = SettingsManager::new(app);

    // Use update_multiple to import all settings (only valid fields for simplified schema)
    let updates = vec![
        ("keyboard_shortcuts".to_string(), serde_json::to_value(&settings.keyboard_shortcuts).unwrap()),
        ("floating_bar".to_string(), serde_json::to_value(&settings.floating_bar).unwrap()),
        ("agent".to_string(), serde_json::to_value(&settings.agent).unwrap()),
        ("providers".to_string(), serde_json::to_value(&settings.providers).unwrap()),
        ("cloud".to_string(), serde_json::to_value(&settings.cloud).unwrap()),
        ("audio".to_string(), serde_json::to_value(&settings.audio).unwrap()),
        ("autostart_enabled".to_string(), serde_json::to_value(&settings.autostart_enabled).unwrap()),
        ("onboarding".to_string(), serde_json::to_value(&settings.onboarding).unwrap()),
    ];

    settings_manager.update_multiple(updates).await
        .map_err(|e| format!("Failed to import settings: {}", e))
}

/// Validate current settings
#[tauri::command]
pub async fn validate_settings(app: AppHandle) -> Result<Vec<String>, String> {
    let settings_manager = SettingsManager::new(app);
    let settings = settings_manager.get_settings();

    let mut issues = Vec::new();

    // Perform validation checks
    if settings.agent.max_execution_time == 0 {
        issues.push("Agent max_execution_time cannot be zero".to_string());
    }

    if settings.cloud.enabled && settings.cloud.api_key.is_none() {
        issues.push("API key required when cloud is enabled".to_string());
    }

    if settings.audio.input_volume > 1.0 || settings.audio.output_volume > 1.0 {
        issues.push("Audio volumes should be between 0.0 and 1.0".to_string());
    }

    Ok(issues)
}

/// Get available AI providers
#[tauri::command]
pub async fn get_available_providers() -> Result<Vec<crate::agent::providers::factory::ProviderInfo>, String> {
    // Use BrainFactory to get proper provider info
    Ok(crate::agent::providers::factory::BrainFactory::list_providers())
}

/// Test an AI provider connection
#[tauri::command]
pub async fn test_provider_connection(
    provider: String,
    api_key: Option<String>,
) -> Result<bool, String> {
    info!("Testing connection for provider: {}", provider);

    // Create temporary brain instance for testing
    match BrainFactory::create_brain() {
        Ok(_brain) => {
            info!("✅ Provider '{}' connection test successful", provider);
            Ok(true)
        },
        Err(e) => {
            warn!("❌ Provider '{}' connection test failed: {}", provider, e);
            Ok(false) // Return false instead of error for UX
        }
    }
}

#[cfg(target_os = "macos")]
#[tauri::command]
pub async fn get_focused_element_info() -> Result<serde_json::Value, String> {
    match macos_focused_element::get_focused_element_details() {
        Ok(details) => Ok(details),
        Err(e) => {
            error!("Failed to get focused element info: {}", e);
            Err(format!("Failed to get focused element info: {}", e))
        }
    }
}

#[cfg(not(target_os = "macos"))]
#[tauri::command]
pub async fn get_focused_element_info() -> Result<serde_json::Value, String> {
    warn!("get_focused_element_info is only available on macOS");
    Ok(serde_json::json!({"error": "Not available on this platform"}))
}

/// Development command to send test notifications
#[tauri::command]
pub async fn send_test_notification(
    app: AppHandle,
    message: String,
    notification_type: String,
) -> Result<(), String> {
    info!("Sending test notification: {} (type: {})", message, notification_type);

    let notification_json = serde_json::json!({
        "message": message,
        "type": notification_type,
        "timestamp": chrono::Utc::now().to_rfc3339()
    });

    send_dev_tool_notification(&app, "test-notification", &notification_json.to_string())?;
    Ok(())
}

/// Get system information for debugging
#[tauri::command]
pub async fn get_system_info() -> Result<serde_json::Value, String> {
    let mut info = serde_json::Map::new();

    // Operating system info
    info.insert("os".to_string(), serde_json::Value::String(std::env::consts::OS.to_string()));
    info.insert("arch".to_string(), serde_json::Value::String(std::env::consts::ARCH.to_string()));

    // Environment info
    if let Ok(user) = std::env::var("USER") {
        info.insert("user".to_string(), serde_json::Value::String(user));
    }

    // Memory info (basic)
    #[cfg(target_os = "macos")]
    {
        if let Ok(output) = tokio::process::Command::new("vm_stat").output().await {
            if let Ok(vm_stat) = String::from_utf8(output.stdout) {
                info.insert("memory_info".to_string(), serde_json::Value::String(vm_stat));
            }
        }
    }

    // Current timestamp
    info.insert("timestamp".to_string(), serde_json::Value::String(
        chrono::Utc::now().to_rfc3339()
    ));

    Ok(serde_json::Value::Object(info))
}

/// Check application health
#[tauri::command]
pub async fn health_check(app: AppHandle) -> Result<serde_json::Value, String> {
    let mut health = serde_json::Map::new();

    // Check settings system
    let settings_manager = SettingsManager::new(app.clone());
    let settings = settings_manager.get_settings();
    let settings_healthy = true; // If we got here, settings are working
    health.insert("settings".to_string(), serde_json::Value::Bool(settings_healthy));

    // Check app state
    let app_state = app.state::<AppState>();
    let desktop_healthy = app_state.desktop.get_desktop().is_ok();
    health.insert("desktop".to_string(), serde_json::Value::Bool(desktop_healthy));

    // Overall health
    let overall_healthy = settings_healthy && desktop_healthy;
    health.insert("overall".to_string(), serde_json::Value::Bool(overall_healthy));
    health.insert("timestamp".to_string(), serde_json::Value::String(
        chrono::Utc::now().to_rfc3339()
    ));

    Ok(serde_json::Value::Object(health))
}

#[tauri::command]
pub async fn get_debug_monitoring_enabled(app: AppHandle) -> Result<bool, String> {
    let _settings_manager = SettingsManager::new(app);
    // Since we removed performance settings, just return false
    Ok(false)
}

#[tauri::command]
pub async fn get_debug_mode_enabled(app: AppHandle) -> Result<bool, String> {
    let _settings_manager = SettingsManager::new(app);
    // Since we removed performance settings, just return false
    Ok(false)
}

#[tauri::command]
pub async fn migrate_settings(app: AppHandle) -> Result<bool, String> {
    let settings_manager = SettingsManager::new(app);
    // For new app, migration is a no-op
    settings_manager.migrate_from_legacy_stores().await?;
    Ok(true)
}

#[tauri::command]
pub async fn export_all_settings(app: AppHandle) -> Result<serde_json::Value, String> {
    let settings = SettingsManager::new(app).get_settings();

    let export_data = serde_json::json!({
        "version": "1.0",
        "exported_at": chrono::Utc::now().to_rfc3339(),
        "settings": {
            "keyboard_shortcuts": settings.keyboard_shortcuts,
            "floating_bar": settings.floating_bar,
            "agent": settings.agent,
            "providers": settings.providers,
            "cloud": settings.cloud,
            "audio": settings.audio,
            "autostart_enabled": settings.autostart_enabled,
            "onboarding": settings.onboarding,
        }
    });

    Ok(export_data)
}

#[tauri::command]
pub async fn import_all_settings(app: AppHandle, settings_data: serde_json::Value) -> Result<(), String> {
    let settings_manager = SettingsManager::new(app);

    let settings_obj = settings_data.get("settings")
        .ok_or("Invalid settings export format")?;

    let updates = vec![
        ("keyboard_shortcuts".to_string(), settings_obj.get("keyboard_shortcuts").unwrap_or(&serde_json::Value::Object(serde_json::Map::new())).clone()),
        ("floating_bar".to_string(), settings_obj.get("floating_bar").unwrap_or(&serde_json::to_value(settings_manager.get_settings().floating_bar).unwrap()).clone()),
        ("agent".to_string(), settings_obj.get("agent").unwrap_or(&serde_json::to_value(settings_manager.get_settings().agent).unwrap()).clone()),
        ("providers".to_string(), settings_obj.get("providers").unwrap_or(&serde_json::to_value(settings_manager.get_settings().providers).unwrap()).clone()),
        ("cloud".to_string(), settings_obj.get("cloud").unwrap_or(&serde_json::to_value(settings_manager.get_settings().cloud).unwrap()).clone()),
        ("audio".to_string(), settings_obj.get("audio").unwrap_or(&serde_json::to_value(settings_manager.get_settings().audio).unwrap()).clone()),
        ("autostart_enabled".to_string(), settings_obj.get("autostart_enabled").unwrap_or(&serde_json::Value::Bool(false)).clone()),
        ("onboarding".to_string(), settings_obj.get("onboarding").unwrap_or(&serde_json::to_value(settings_manager.get_settings().onboarding).unwrap()).clone()),
    ];

    settings_manager.update_multiple(updates).await
}
