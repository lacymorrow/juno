// Core/Miscellaneous commands (screenshots, app list, clipboard, wait)

use tauri::State;
use tracing::{info, error};
use crate::state::AppState;
use tauri::AppHandle;
use tracing::warn;
use super::send_dev_tool_notification; // Use helper from parent module
use crate::agent::providers::factory::{BrainFactory, ProviderInfo};
use serde::{Deserialize, Serialize};
use crate::settings::{manager::SettingsManager, AgentSettings, AudioSettings};

#[cfg(target_os = "macos")]
use computer_use_ai_sdk::platforms::macos::utils as macos_utils;
#[cfg(not(target_os = "macos"))]
use tauri::AppHandle as DummyAppHandle; // Alias for non-macos signature consistency


#[cfg(target_os = "macos")]
#[tauri::command]
pub(crate) async fn capture_screenshot_command(app: AppHandle) -> Result<String, String> {
    use crate::utils::coordinates;
    use base64::Engine;
    use image::ImageFormat;

    match macos_utils::capture_and_encode_screenshot() {
        Ok(base64_string) => {
            // Parse the screenshot to get its dimensions
            let engine = base64::engine::general_purpose::STANDARD;
            if let Ok(image_data) = engine.decode(&base64_string) {
                if let Ok(img) = image::load_from_memory(&image_data) {
                    let screenshot_width = img.width();
                    let screenshot_height = img.height();

                    // Get display information to calculate proper scaling
                    match get_display_dimensions() {
                        Ok((display_width, display_height)) => {
                            // Calculate scaling factors for both dimensions
                            let scale_x = if display_width > 0 {
                                screenshot_width as f32 / display_width as f32
                            } else {
                                1.0
                            };
                            let scale_y = if display_height > 0 {
                                screenshot_height as f32 / display_height as f32
                            } else {
                                1.0
                            };

                            // Use separate scale factors for proper non-uniform scaling support
                            // Update scaling information with separate X and Y scale factors
                            coordinates::update_scaling_info_with_separate_factors(
                                display_width,
                                display_height,
                                screenshot_width,
                                screenshot_height,
                                scale_x,
                                scale_y,
                            );

                            info!("Screenshot scaling updated: display {}x{} → screenshot {}x{}, scale_x: {:.3}, scale_y: {:.3}",
                                display_width, display_height, screenshot_width, screenshot_height, scale_x, scale_y);
                        }
                        Err(e) => {
                            tracing::warn!("Failed to get display dimensions for scaling: {}", e);
                            // Fallback: assume no scaling needed
                            coordinates::update_scaling_info(
                                screenshot_width,
                                screenshot_height,
                                screenshot_width,
                                screenshot_height,
                                1.0,
                            );
                        }
                    }
                } else {
                    tracing::warn!("Failed to decode screenshot image for scaling calculation");
                }
            } else {
                tracing::warn!("Failed to decode base64 screenshot for scaling calculation");
            }

            // Send notification on success
            send_dev_tool_notification(&app, "Screenshot", "Screenshot captured successfully.")?;
            Ok(base64_string)
        }
        Err(e) => Err(format!("Failed to capture screenshot: {}", e)),
    }
}

/// Get display dimensions using macOS Core Graphics
#[cfg(target_os = "macos")]
fn get_display_dimensions() -> Result<(u32, u32), String> {
    use computer_use_ai_sdk::platforms::macos::display::get_main_display;

    match get_main_display() {
        Ok(display_info) => {
            let width = display_info.bounds.size.width as u32;
            let height = display_info.bounds.size.height as u32;

            if width == 0 || height == 0 {
                return Err("Invalid display dimensions".to_string());
            }

            Ok((width, height))
        }
        Err(e) => Err(format!("Failed to get display info: {}", e))
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
pub async fn set_performance_monitoring(
    app_handle: AppHandle,
    enabled: bool,
    state: State<'_, AppState>
) -> Result<(), String> {
    info!("Setting performance monitoring to: {}", enabled);

    let settings_manager = crate::settings::manager::SettingsManager::new(app_handle)
        .map_err(|e| format!("Failed to initialize settings manager: {}", e))?;

    let mut audio_settings = settings_manager.get_audio_settings().await
        .map_err(|e| format!("Failed to load audio settings: {}", e))?;

    audio_settings.performance_monitoring_enabled = enabled;

    settings_manager.set_audio_settings(&audio_settings).await
        .map_err(|e| format!("Failed to save audio settings: {}", e))?;

    // Update state for backward compatibility
    let _ = state.set_performance_monitoring_enabled(enabled);

    info!("Performance monitoring successfully set to: {}", enabled);
    Ok(())
}

/// Get performance monitoring enabled state
#[tauri::command]
pub async fn get_performance_monitoring(
    app_handle: AppHandle,
    state: State<'_, AppState>
) -> Result<bool, String> {
    let settings_manager = crate::settings::manager::SettingsManager::new(app_handle)
        .map_err(|e| format!("Failed to initialize settings manager: {}", e))?;

    let audio_settings = settings_manager.get_audio_settings().await
        .map_err(|e| format!("Failed to load audio settings: {}", e))?;

    // Sync with state for backward compatibility
    let _ = state.set_performance_monitoring_enabled(audio_settings.performance_monitoring_enabled);

    Ok(audio_settings.performance_monitoring_enabled)
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
    let cfg_debug = cfg!(debug_assertions);
    let rust_log = std::env::var("RUST_LOG").unwrap_or_default();
    let has_debug_log = rust_log.contains("debug");

    let result = debug_mode || cfg_debug || has_debug_log;

    info!("Debug mode check: state={}, cfg={}, rust_log_debug={}, result={}",
          debug_mode, cfg_debug, has_debug_log, result);

    Ok(result)
}

// NOTE: reset_all_settings was removed and replaced with reset_centralized_settings
// in src-tauri/src/commands/settings.rs to eliminate duplication and use proper
// centralized settings management. The centralized version is the single source of truth.

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

/// Get current screenshot scaling information for debugging
#[tauri::command]
pub async fn get_screenshot_scaling_info() -> Result<serde_json::Value, String> {
    use crate::utils::coordinates;

    match coordinates::get_scaling_info() {
        Ok(scaling_info) => {
            Ok(serde_json::to_value(scaling_info).unwrap_or(serde_json::Value::Null))
        }
        Err(e) => Err(format!("Failed to get scaling info: {}", e))
    }
}

/// Reset screenshot scaling information to defaults
#[tauri::command]
pub async fn reset_screenshot_scaling() -> Result<(), String> {
    use crate::utils::coordinates;

    coordinates::reset_scaling_info();
    info!("Screenshot scaling information reset to defaults");
    Ok(())
}

/// Test coordinate transformation with current scaling
#[tauri::command]
pub async fn test_coordinate_transformation(
    screenshot_x: f64,
    screenshot_y: f64,
) -> Result<serde_json::Value, String> {
    use crate::utils::coordinates;

    let (screen_x, screen_y) = coordinates::transform_to_screen_coordinates(screenshot_x, screenshot_y);
    let (back_to_screenshot_x, back_to_screenshot_y) = coordinates::transform_to_scaled_coordinates(screen_x, screen_y);

    let roundtrip_error_x = (screenshot_x - back_to_screenshot_x).abs();
    let roundtrip_error_y = (screenshot_y - back_to_screenshot_y).abs();

    let result = serde_json::json!({
        "input_screenshot": { "x": screenshot_x, "y": screenshot_y },
        "calculated_screen": { "x": screen_x, "y": screen_y },
        "roundtrip_screenshot": { "x": back_to_screenshot_x, "y": back_to_screenshot_y },
        "roundtrip_error": { "x": roundtrip_error_x, "y": roundtrip_error_y },
        "is_accurate": roundtrip_error_x < 1.0 && roundtrip_error_y < 1.0,
        "scaling_info": coordinates::get_scaling_info().unwrap_or_default()
    });

    Ok(result)
}

// --- PRODUCTION CORE FUNCTIONS WITH DEBUG CAPABILITIES ---
// These functions replace the dev_ prefixed functions by incorporating debug features conditionally

#[tauri::command]
pub(crate) async fn wait(duration_sec: f64, app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    use crate::commands::debug_utils::{DebugConfig, should_enable_debug, log_debug_operation, send_debug_notification, validators};

    let debug_enabled = should_enable_debug(false, &state);
    let debug_config = if debug_enabled { DebugConfig::development_mode() } else { DebugConfig::production_mode() };

    // Debug validation
    if debug_config.validate_inputs {
        validators::valid_duration_seconds(duration_sec)?;
    }

    let duration_ms = (duration_sec * 1000.0).max(0.0) as u64; // Convert seconds to ms, ensure non-negative

    log_debug_operation("wait", &format!("Waiting for {} seconds ({} ms)", duration_sec, duration_ms), &debug_config);
    info!("Executing wait for {} seconds ({} ms)", duration_sec, duration_ms);

    let desktop = state.get_desktop()?;
    match desktop.wait(duration_ms) {
        Ok(_) => {
            info!("Successfully completed wait for {} seconds", duration_sec);

            // Send debug notification if enabled
            if debug_config.send_notifications {
                let _ = send_debug_notification(&app, "Wait", &format!("Waited for {} seconds", duration_sec));
            }

            Ok(())
        }
        Err(e) => {
            let error_msg = format!("Error during wait: {}", e);
            error!("{}", error_msg);
            Err(error_msg)
        }
    }
}

#[tauri::command]
pub(crate) async fn get_clipboard(app: AppHandle, state: State<'_, AppState>) -> Result<String, String> {
    use crate::commands::debug_utils::{DebugConfig, should_enable_debug, log_debug_operation, send_debug_notification};

    let debug_enabled = should_enable_debug(false, &state);
    let debug_config = if debug_enabled { DebugConfig::development_mode() } else { DebugConfig::production_mode() };

    log_debug_operation("get_clipboard", "Getting clipboard content", &debug_config);
    info!("Executing get_clipboard");

    let desktop = state.get_desktop()?;
    match desktop.get_clipboard_content() {
        Ok(content) => {
            info!("Successfully retrieved clipboard content (length: {})", content.len());

            // Send debug notification if enabled
            if debug_config.send_notifications {
                let preview = if content.len() > 50 {
                    format!("{}...", &content[..50])
                } else {
                    content.clone()
                };
                let _ = send_debug_notification(&app, "Get Clipboard", &format!("Retrieved: {}", preview));
            }

            Ok(content)
        }
        Err(e) => {
            let error_msg = format!("Error getting clipboard content: {}", e);
            error!("{}", error_msg);
            Err(error_msg)
        }
    }
}

#[tauri::command]
pub(crate) async fn set_clipboard(content: String, app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    use crate::commands::debug_utils::{DebugConfig, should_enable_debug, log_debug_operation, send_debug_notification, validators};

    let debug_enabled = should_enable_debug(false, &state);
    let debug_config = if debug_enabled { DebugConfig::development_mode() } else { DebugConfig::production_mode() };

    // Debug validation
    if debug_config.validate_inputs {
        validators::non_empty_text(&content)?;
    }

    log_debug_operation("set_clipboard", &format!("Setting clipboard content (length: {})", content.len()), &debug_config);
    info!("Executing set_clipboard with content length: {}", content.len());

    let desktop = state.get_desktop()?;
    match desktop.set_clipboard_content(&content) {
        Ok(_) => {
            info!("Successfully set clipboard content");

            // Send debug notification if enabled
            if debug_config.send_notifications {
                let preview = if content.len() > 50 {
                    format!("{}...", &content[..50])
                } else {
                    content.clone()
                };
                let _ = send_debug_notification(&app, "Set Clipboard", &format!("Set: {}", preview));
            }

            Ok(())
        }
        Err(e) => {
            let error_msg = format!("Error setting clipboard content: {}", e);
            error!("{}", error_msg);
            Err(error_msg)
        }
    }
}

/// Get the current dictation trigger mode (tap or hold)
#[tauri::command]
pub async fn get_dictation_trigger_mode(
    app: AppHandle,
    state: State<'_, AppState>
) -> Result<String, String> {
    let settings_manager = SettingsManager::new(app.clone())
        .map_err(|e| format!("Failed to initialize settings manager: {}", e))?;

    match settings_manager.get_audio_settings().await {
        Ok(audio_settings) => {
            info!("Loaded dictation trigger mode from centralized settings: {}", audio_settings.dictation_trigger_mode);

            // Sync with state for backward compatibility
            let trigger_mode = match audio_settings.dictation_trigger_mode.as_str() {
                "tap" => crate::state::DictationTriggerMode::Tap,
                "hold" => crate::state::DictationTriggerMode::Hold,
                _ => {
                    warn!("Invalid dictation trigger mode: {}. Using default (hold)", audio_settings.dictation_trigger_mode);
                    crate::state::DictationTriggerMode::Hold
                }
            };

            {
                let mut current_mode = state.dictation_trigger_mode.lock()
                    .map_err(|e| format!("Failed to lock dictation trigger mode: {}", e))?;
                *current_mode = trigger_mode;
            }

            Ok(audio_settings.dictation_trigger_mode)
        }
        Err(e) => {
            Err(format!("Failed to load dictation trigger mode from centralized settings: {}", e))
        }
    }
}

/// Set the dictation trigger mode (tap or hold)
#[tauri::command]
pub async fn set_dictation_trigger_mode(
    app: AppHandle,
    state: State<'_, AppState>,
    mode: String,
) -> Result<(), String> {
    // Validate the mode
    let trigger_mode = match mode.as_str() {
        "tap" => crate::state::DictationTriggerMode::Tap,
        "hold" => crate::state::DictationTriggerMode::Hold,
        _ => return Err(format!("Invalid dictation trigger mode: {}. Must be 'tap' or 'hold'", mode)),
    };

    let settings_manager = SettingsManager::new(app.clone())
        .map_err(|e| format!("Failed to initialize settings manager: {}", e))?;

    // Get current audio settings
    let mut audio_settings = settings_manager.get_audio_settings().await
        .map_err(|e| format!("Failed to load audio settings: {}", e))?;

    // Update dictation trigger mode
    audio_settings.dictation_trigger_mode = mode.clone();

    // Save to centralized settings
    settings_manager.set_audio_settings(&audio_settings).await
        .map_err(|e| format!("Failed to save audio settings: {}", e))?;

    // Update the state for backward compatibility
    {
        let mut current_mode = state.dictation_trigger_mode.lock()
            .map_err(|e| format!("Failed to lock dictation trigger mode: {}", e))?;
        *current_mode = trigger_mode;
    }

    info!("Updated dictation trigger mode to: {}", mode);
    Ok(())
}

/// Load dictation trigger mode from centralized settings
pub async fn load_dictation_trigger_mode_from_store(app: &AppHandle, state: &AppState) -> Result<(), String> {
    let settings_manager = SettingsManager::new(app.clone())
        .map_err(|e| format!("Failed to initialize settings manager: {}", e))?;

    // Get audio settings or use defaults
    let audio_settings = settings_manager.get_audio_settings().await
        .unwrap_or_else(|_| AudioSettings::default());

    let trigger_mode = match audio_settings.dictation_trigger_mode.as_str() {
        "tap" => crate::state::DictationTriggerMode::Tap,
        "hold" => crate::state::DictationTriggerMode::Hold,
        _ => {
            warn!("Invalid dictation trigger mode in centralized settings: {}. Using default (hold)", audio_settings.dictation_trigger_mode);
            crate::state::DictationTriggerMode::Hold
        }
    };

    // Update state
    {
        let mut current_mode = state.dictation_trigger_mode.lock()
            .map_err(|e| format!("Failed to lock dictation trigger mode: {}", e))?;
        *current_mode = trigger_mode;
    }

    info!("Loaded dictation trigger mode from centralized settings: {}", audio_settings.dictation_trigger_mode);
    Ok(())
}
