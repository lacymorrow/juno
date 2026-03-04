// Core/Miscellaneous commands (screenshots, app list, clipboard, wait)

use tauri::State;
use tracing::{info, error};
use crate::state::AppState;
use tauri::AppHandle;
use tracing::warn;
use super::send_dev_tool_notification; // Use helper from parent module
use crate::agent::providers::factory::{BrainFactory, ProviderInfo};
use serde::{Deserialize, Serialize};
use tauri_plugin_store::StoreExt;
use crate::settings::{manager::SettingsManager, AgentSettings, AudioSettings};
use crate::utils::coordinates;

#[cfg(not(target_os = "macos"))]
use tauri::AppHandle as DummyAppHandle; // Alias for non-macos signature consistency

#[derive(Serialize, Deserialize, Debug)]
pub struct ScreenshotResult {
    pub base64_image: String,
    pub original_width: u32,
    pub original_height: u32,
    pub resized_width: u32,
    pub resized_height: u32,
    /// Cursor position in standard (API) coordinates, if available
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
}

/// Get cursor position in standard (API) coordinates for inclusion in screenshot results.
/// Returns a human-readable string like "Cursor at (523, 341)." or None if unavailable.
#[cfg(target_os = "macos")]
fn get_cursor_position_text() -> Option<String> {
    use core_graphics::event::CGEvent;
    use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};

    let event_source = CGEventSource::new(CGEventSourceStateID::HIDSystemState).ok()?;
    let event = CGEvent::new(event_source).ok()?;
    let point = event.location();

    // Transform from screen coordinates to standard (API) coordinates
    let (std_x, std_y) = coordinates::transform_screen_to_standard_coordinates(point.x, point.y);
    Some(format!("Cursor at ({}, {}).", std_x as i32, std_y as i32))
}

#[cfg(target_os = "macos")]
#[tauri::command]
pub(crate) async fn capture_screenshot_command(app: AppHandle, state: State<'_, AppState>) -> Result<ScreenshotResult, String> {
    use computer_use_ai_sdk::platforms::macos::utils::capture_and_encode_screenshot;
    use image::ImageReader;
    use std::io::Cursor;
    use base64::Engine;
    
    // Rate limiting check for screenshot operations
    if let Err(e) = state.rate_limiters.screenshots.check("default_user").await {
        return Err(e.to_user_message());
    }

    match capture_and_encode_screenshot() {
        Ok(base64_string) => {
            // Parse the screenshot to get its dimensions
            let engine = base64::engine::general_purpose::STANDARD;
            if let Ok(image_data) = engine.decode(&base64_string) {
                if let Ok(reader) = ImageReader::new(Cursor::new(&image_data))
                    .with_guessed_format()
                    .map_err(|e| format!("Failed to read image format: {}", e)) {
                    if let Ok(img) = reader.decode()
                        .map_err(|e| format!("Failed to decode image: {}", e)) {

                    let original_width = img.width();
                    let original_height = img.height();

                    // Get display information to calculate proper scaling
                    match get_display_dimensions() {
                        Ok((display_width, display_height, origin_x, origin_y, display_id)) => {
                            // Select the best standard resolution for this display
                            let (standard_width, standard_height) =
                                crate::constants::ui::standard_resolutions::select_best_resolution(display_width, display_height);

                            // Determine if we need to scale the screenshot to match standard resolution
                            let needs_scaling = original_width != standard_width || original_height != standard_height;

                            let final_base64 = if needs_scaling {
                                info!("Scaling screenshot from {}x{} to standard resolution {}x{} for Anthropic Computer Use API compliance",
                                    original_width, original_height, standard_width, standard_height);

                                // Scale the image to the standard resolution
                                let scaled_img = img.resize_exact(
                                    standard_width,
                                    standard_height,
                                    image::imageops::FilterType::Lanczos3,
                                );

                                // Encode the scaled image as JPEG (quality 85) — ~60% smaller than PNG
                                let mut scaled_buffer = Cursor::new(Vec::new());
                                let jpeg_encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut scaled_buffer, 85);
                                scaled_img.write_with_encoder(jpeg_encoder)
                                    .map_err(|e| format!("Failed to encode scaled image as JPEG: {}", e))?;

                                engine.encode(scaled_buffer.into_inner())
                            } else {
                                info!("Screenshot already at standard resolution {}x{}, re-encoding as JPEG",
                                    original_width, original_height);
                                // Re-encode as JPEG even when no scaling needed (saves ~60% vs PNG)
                                let mut jpeg_buffer = Cursor::new(Vec::new());
                                let jpeg_encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg_buffer, 85);
                                img.write_with_encoder(jpeg_encoder)
                                    .map_err(|e| format!("Failed to encode image as JPEG: {}", e))?;
                                engine.encode(jpeg_buffer.into_inner())
                            };

                            // Update scaling information with standard resolution data AND display origin
                            coordinates::update_standard_resolution_scaling_with_display(
                                display_width,
                                display_height,
                                standard_width,  // The screenshot is now at standard resolution
                                standard_height,
                                origin_x,
                                origin_y,
                                Some(display_id),
                            );

                            info!("Screenshot scaling updated: display {}x{} at origin ({}, {}) → standard resolution {}x{} (Anthropic Computer Use API compliant)",
                                display_width, display_height, origin_x, origin_y, standard_width, standard_height);

                            // Send notification on success
                            send_dev_tool_notification(&app, "Screenshot", &format!(
                                "Screenshot captured at standard resolution {}x{} (scaled from display {}x{} at origin ({}, {}))",
                                standard_width, standard_height, display_width, display_height, origin_x, origin_y
                            ))?;

                            Ok(ScreenshotResult {
                                base64_image: final_base64,
                                original_width: display_width,
                                original_height: display_height,
                                resized_width: standard_width,
                                resized_height: standard_height,
                                output: get_cursor_position_text(),
                            })
                        }
                        Err(e) => {
                            tracing::warn!("Failed to get display dimensions for standard resolution scaling: {}", e);

                            // Fallback: assume the screenshot is already properly sized
                            // This maintains some level of functionality even if display detection fails
                            let (_fallback_standard_width, _fallback_standard_height) =
                                crate::constants::ui::standard_resolutions::XGA; // Default to XGA

                            coordinates::update_standard_resolution_scaling(
                                original_width,  // Use screenshot dimensions as display dimensions
                                original_height,
                                original_width,  // Assume screenshot is at correct size
                                original_height,
                            );

                            tracing::warn!("Using fallback scaling with screenshot dimensions {}x{}",
                                original_width, original_height);

                            send_dev_tool_notification(&app, "Screenshot", "Screenshot captured (display detection failed, using fallback scaling)")?;
                            Ok(ScreenshotResult {
                                base64_image: base64_string,
                                original_width,
                                original_height,
                                resized_width: original_width, // Fallback: no resize
                                resized_height: original_height,
                                output: get_cursor_position_text(),
                            })
                        }
                    }
                    } else {
                        let error_msg = "Failed to decode screenshot image for standard resolution scaling";
                        tracing::warn!("{}", error_msg);

                        // Still return the screenshot but without proper scaling
                        send_dev_tool_notification(&app, "Screenshot", "Screenshot captured (scaling unavailable)")?;
                        Ok(ScreenshotResult {
                            base64_image: base64_string,
                            original_width: 0, // Unknown
                            original_height: 0, // Unknown
                            resized_width: 0,
                            resized_height: 0,
                            output: None,
                        })
                    }
                } else {
                    let error_msg = "Failed to read image format for standard resolution scaling";
                    tracing::warn!("{}", error_msg);

                    // Still return the screenshot but without proper scaling
                    send_dev_tool_notification(&app, "Screenshot", "Screenshot captured (format reading unavailable)")?;
                    Ok(ScreenshotResult {
                        base64_image: base64_string,
                        original_width: 0, // Unknown
                        original_height: 0, // Unknown
                        resized_width: 0,
                        resized_height: 0,
                        output: None,
                    })
                }
            } else {
                let error_msg = "Failed to decode base64 screenshot for standard resolution scaling";
                tracing::warn!("{}", error_msg);

                // Still return the screenshot but without proper scaling
                send_dev_tool_notification(&app, "Screenshot", "Screenshot captured (scaling unavailable)")?;
                Ok(ScreenshotResult {
                    base64_image: base64_string,
                    original_width: 0, // Unknown
                    original_height: 0, // Unknown
                    resized_width: 0,
                    resized_height: 0,
                    output: None,
                })
            }
        }
        Err(e) => Err(format!("Failed to capture screenshot: {}", e)),
    }
}

/// Get display dimensions using macOS Core Graphics
/// NEW: Now detects cursor position and returns info for the display containing the cursor
#[cfg(target_os = "macos")]
fn get_display_dimensions() -> Result<(u32, u32, f64, f64, u32), String> {
    use computer_use_ai_sdk::platforms::macos::display::{get_main_display, find_display_containing_point};
    use core_graphics::event::CGEvent;
    use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};

    // Get current cursor position to determine which display to use
    let cursor_point = {
        let event_source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
            .map_err(|_| "Failed to create HID event source".to_string())?;
        let event = CGEvent::new(event_source)
            .map_err(|_| "Failed to create CGEvent to get cursor location".to_string())?;
        event.location()
    };

    // Find the display containing the cursor
    match find_display_containing_point(cursor_point) {
        Ok(display_info) => {
            let width = display_info.bounds.size.width as u32;
            let height = display_info.bounds.size.height as u32;
            let origin_x = display_info.bounds.origin.x;
            let origin_y = display_info.bounds.origin.y;
            let display_id = display_info.id;

            if width == 0 || height == 0 {
                return Err("Invalid display dimensions".to_string());
            }

            tracing::info!("Using display containing cursor: {}x{} at origin ({}, {}), ID: {}",
                width, height, origin_x, origin_y, display_id);

            Ok((width, height, origin_x, origin_y, display_id))
        }
        Err(e) => {
            tracing::warn!("Failed to find display for cursor position, falling back to main display: {}", e);

            // Fallback to main display
            match get_main_display() {
                Ok(display_info) => {
                    let width = display_info.bounds.size.width as u32;
                    let height = display_info.bounds.size.height as u32;
                    let origin_x = display_info.bounds.origin.x;
                    let origin_y = display_info.bounds.origin.y;
                    let display_id = display_info.id;

                    if width == 0 || height == 0 {
                        return Err("Invalid main display dimensions".to_string());
                    }

                    Ok((width, height, origin_x, origin_y, display_id))
                }
                Err(e) => Err(format!("Failed to get main display info: {}", e))
            }
        }
    }
}

#[cfg(not(target_os = "macos"))]
#[tauri::command]
pub(crate) async fn capture_screenshot_command(_app: DummyAppHandle) -> Result<ScreenshotResult, String> { // Use alias
    Err("Screenshot capture is only supported on macOS currently.".to_string())
}

#[cfg(target_os = "macos")]
#[tauri::command]
#[allow(dead_code)]
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
            window.id().is_some_and(|id| id == window_id)
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
#[allow(dead_code)]
pub(crate) async fn capture_window_screenshot_command(
    _app: DummyAppHandle,
    _state: State<'_, AppState>,
    _window_id: String,
) -> Result<String, String> {
    Err("Window screenshot capture is only supported on macOS currently.".to_string())
}

#[cfg(target_os = "macos")]
#[tauri::command]
#[allow(dead_code)]
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
#[allow(dead_code)]
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

#[derive(Serialize, Debug)]
pub struct ServerStatus {
    pub backend_running: bool,
    pub desktop_available: bool,
}

#[tauri::command]
pub(crate) fn check_server_status(state: State<'_, AppState>) -> ServerStatus {
    ServerStatus {
        backend_running: true, // If this command executes, the backend is running
        desktop_available: state.is_desktop_available(),
    }
}



/// Get a list of available AI providers
#[tauri::command]
pub async fn list_ai_providers() -> Result<Vec<ProviderInfo>, String> {
    Ok(BrainFactory::list_providers())
}

/// Set the active AI provider
#[tauri::command]
pub async fn set_ai_provider(provider_id: String, app_handle: AppHandle) -> Result<(), String> {
    // Persist via Tauri store instead of unsafe std::env::set_var
    let store = app_handle.store("settings.json")
        .map_err(|e| format!("Failed to open settings store: {}", e))?;
    store.set("ai_provider", serde_json::json!(provider_id.clone()));
    store.save().map_err(|e| format!("Failed to save settings: {}", e))?;

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

            state.set_agent_trigger_mode(trigger_mode)
                .map_err(|e| format!("Failed to set agent trigger mode: {}", e))?;

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
    state.set_agent_trigger_mode(trigger_mode)
        .map_err(|e| format!("Failed to set agent trigger mode: {}", e))?;

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
    state.set_agent_trigger_mode(trigger_mode)
        .map_err(|e| format!("Failed to set agent trigger mode: {}", e))?;

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
            serde_json::to_value(scaling_info)
                .map_err(|e| format!("Failed to serialize scaling info: {}", e))
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
                    format!("{}...", content.chars().take(50).collect::<String>())
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
                    format!("{}...", content.chars().take(50).collect::<String>())
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

            state.set_dictation_trigger_mode(trigger_mode)
                .map_err(|e| format!("Failed to set dictation trigger mode: {}", e))?;

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
    state.set_dictation_trigger_mode(trigger_mode)
        .map_err(|e| format!("Failed to set dictation trigger mode: {}", e))?;

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
    state.set_dictation_trigger_mode(trigger_mode)
        .map_err(|e| format!("Failed to set dictation trigger mode: {}", e))?;

    info!("Loaded dictation trigger mode from centralized settings: {}", audio_settings.dictation_trigger_mode);
    Ok(())
}
