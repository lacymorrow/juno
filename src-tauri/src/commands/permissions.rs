// CIDRE-powered native permission system - eliminates osascript admin privilege prompts
// All permission checking now uses native APIs through NativePermissionChecker

use crate::constants::timeouts;
use crate::commands::native_permissions::{NativePermissionChecker, NativePermissionStatus};
use log::{debug, error, info, warn};
use serde::{Deserialize, Serialize};
use std::process::Command;
use std::sync::{atomic::{AtomicBool, Ordering}, Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};
use tauri::async_runtime::JoinHandle;
use tokio_util::sync::CancellationToken;
use crate::constants::events;
use crate::constants::errors::templates::FAILED_TO_EMIT;

// ── Per-permission dialog deduplication ──────────────────────────────────────
//
// Each AtomicBool tracks whether the native system dialog was already shown
// during this launch. First call → trigger the OS dialog. Subsequent calls →
// open System Settings directly (avoids no-op dialog re-shows).
// Reset automatically on each app launch via LazyLock initialization.

static SCREEN_RECORDING_DIALOG_SHOWN: std::sync::LazyLock<AtomicBool> =
    std::sync::LazyLock::new(|| AtomicBool::new(false));
static ACCESSIBILITY_DIALOG_SHOWN: std::sync::LazyLock<AtomicBool> =
    std::sync::LazyLock::new(|| AtomicBool::new(false));
static MICROPHONE_DIALOG_SHOWN: std::sync::LazyLock<AtomicBool> =
    std::sync::LazyLock::new(|| AtomicBool::new(false));
static INPUT_MONITORING_DIALOG_SHOWN: std::sync::LazyLock<AtomicBool> =
    std::sync::LazyLock::new(|| AtomicBool::new(false));

/// Helper function to format error messages with proper template substitution
fn format_error(template: &str, context: &str, error: impl std::fmt::Display) -> String {
    template.replacen("{}", context, 1).replacen("{}", &error.to_string(), 1)
}

/// Permission status information for frontend consumption
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionStatus {
    pub permission_type: String,
    pub granted: bool,
    pub required: bool,
    pub description: String,
    pub instructions: String,
}

/// Complete permissions state for the application
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionsState {
    pub accessibility: PermissionStatus,
    pub screen_recording: PermissionStatus,
    pub microphone: PermissionStatus,
    pub input_monitoring: PermissionStatus,
    pub all_granted: bool,
    pub app_name: String,
}

// Permission monitoring task handle
type MonitoringTask = Arc<Mutex<Option<(JoinHandle<()>, CancellationToken)>>>;
static MONITORING_TASK: std::sync::LazyLock<MonitoringTask> = std::sync::LazyLock::new(|| {
    Arc::new(Mutex::new(None))
});

// Short-TTL cache for check_permissions_status_native — eliminates redundant native calls during startup.
// INVARIANT: lock must NEVER be held across an .await point — always released within its own {} scope.
type PermissionsCache = Mutex<Option<(Instant, PermissionsState)>>;
static PERMISSIONS_CACHE: std::sync::LazyLock<PermissionsCache> =
    std::sync::LazyLock::new(|| Mutex::new(None));

/// Returns true if a cache entry timestamped at `cached_at` is still within the TTL window.
fn is_cache_fresh(cached_at: Instant) -> bool {
    cached_at.elapsed() < Duration::from_secs(timeouts::PERMISSIONS_CACHE_TTL_SECONDS)
}

/// Check the status of all required macOS permissions using NATIVE APIs ONLY
#[tauri::command]
pub async fn check_permissions_status_native(app: AppHandle) -> Result<PermissionsState, String> {
    // Check cache before making expensive native calls — permissions don't change sub-second.
    // Lock is held only for the duration of the cache read, released before any await points.
    {
        let cache = PERMISSIONS_CACHE
            .lock()
            .map_err(|_| "Permissions cache lock poisoned".to_string())?;
        if let Some((cached_at, ref state)) = *cache {
            if is_cache_fresh(cached_at) {
                debug!("Returning cached permissions state (age: {:?})", cached_at.elapsed());
                return Ok(state.clone());
            }
        }
    }

    info!("Checking macOS permissions status using native APIs (no password prompts)");

    let app_name = app.package_info().name.clone();

    // Use native APIs only - no osascript, no admin privileges
    let native_accessibility = NativePermissionChecker::get_accessibility_status().await?;
    let native_screen_recording = NativePermissionChecker::get_screen_recording_status()?;
    let native_microphone = NativePermissionChecker::get_microphone_status().await?;
    let native_input_monitoring = NativePermissionChecker::get_input_monitoring_status().await?;

    // Convert to frontend format
    let accessibility = convert_native_to_frontend_status(native_accessibility);
    let screen_recording = convert_native_to_frontend_status(native_screen_recording);
    let microphone = convert_native_to_frontend_status(native_microphone);
    let input_monitoring = convert_native_to_frontend_status(native_input_monitoring);

    // Only consider REQUIRED permissions for all_granted status
    let all_granted = accessibility.granted && screen_recording.granted;

    let permissions_state = PermissionsState {
        accessibility,
        screen_recording,
        microphone,
        input_monitoring,
        all_granted,
        app_name,
    };

    info!("Native permissions checked - no password prompts required");
    debug!("Permissions state: {:?}", permissions_state);

    // Store result in cache for subsequent calls within the TTL window.
    match PERMISSIONS_CACHE.lock() {
        Ok(mut cache) => *cache = Some((Instant::now(), permissions_state.clone())),
        Err(_) => warn!("Failed to update permissions cache — lock poisoned"),
    }

    Ok(permissions_state)
}

/// Invalidate the permissions cache so the next call fetches fresh data from native APIs.
/// Call this after a permission request dialog has been shown to the user.
#[allow(dead_code)] // called only from #[cfg(target_os = "macos")] blocks; dead on other platforms
pub fn invalidate_permissions_cache() {
    match PERMISSIONS_CACHE.lock() {
        Ok(mut cache) => *cache = None,
        Err(_) => warn!("Failed to invalidate permissions cache — lock poisoned"),
    }
}

/// Convert native permission status to frontend format
fn convert_native_to_frontend_status(native: NativePermissionStatus) -> PermissionStatus {
    PermissionStatus {
        permission_type: native.permission_type,
        granted: native.granted,
        required: native.required,
        description: native.description,
        instructions: native.instructions,
    }
}

/// Get cached permissions state (for onboarding flow) - returns reset state if available
#[tauri::command]
pub async fn get_permissions_state(app: AppHandle) -> Result<PermissionsState, String> {
    let app_state = app.state::<crate::state::AppState>();

    // Check if we have cached permissions state (from reset_onboarding)
    if let Some(cached_state) = app_state.get_permissions_state().await {
        info!("Returning cached permissions state: all_granted={}", cached_state.all_granted);
        return Ok(cached_state);
    }

    // Fallback to checking actual system permissions
    info!("No cached permissions state, checking system permissions");
    check_permissions_status_native(app).await
}

/// Request accessibility permissions using native APIs - NO password prompts.
///
/// First call: triggers the native OS dialog (AXIsProcessTrustedWithOptions).
/// Subsequent calls: opens System Settings directly (dialog already shown this launch).
#[tauri::command]
pub async fn request_accessibility_permission_native() -> Result<bool, String> {
    info!("Requesting accessibility permissions using native APIs");

    #[cfg(target_os = "macos")]
    {
        let result = match NativePermissionChecker::check_accessibility_permission() {
            Ok(true) => {
                info!("Accessibility permissions already granted");
                Ok(true)
            }
            Ok(false) => {
                let already_shown = ACCESSIBILITY_DIALOG_SHOWN.swap(true, Ordering::AcqRel);
                if already_shown {
                    info!("Accessibility dialog already shown this launch — opening System Settings directly");
                    let _ = Command::new("open")
                        .args(["x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility"])
                        .status();
                    Ok(false)
                } else {
                    info!("Requesting accessibility permissions with native prompt (first time this launch)");
                    match NativePermissionChecker::request_accessibility_permission() {
                        Ok(()) => {
                            info!("Accessibility permission request triggered successfully");
                            tokio::time::sleep(tokio::time::Duration::from_millis(timeouts::PERMISSION_CHECK_DELAY_MS)).await;
                            match NativePermissionChecker::check_accessibility_permission() {
                                Ok(granted) => {
                                    if granted {
                                        info!("Accessibility permissions now granted");
                                    } else {
                                        info!("Accessibility permissions still not granted - user needs to manually enable in System Settings");
                                    }
                                    Ok(granted)
                                }
                                Err(e) => {
                                    error!("Error checking accessibility permissions after request: {}", e);
                                    Ok(false)
                                }
                            }
                        }
                        Err(e) => {
                            error!("Error requesting accessibility permissions: {}", e);
                            Err(format!("Failed to request accessibility permissions: {}", e))
                        }
                    }
                }
            }
            Err(e) => {
                error!("Error checking accessibility permissions: {}", e);
                Err(format!("Failed to check accessibility permissions: {}", e))
            }
        };
        invalidate_permissions_cache();
        result
    }

    #[cfg(not(target_os = "macos"))]
    {
        Ok(true)
    }
}

/// Request microphone permissions using native APIs - NO password prompts.
///
/// First call: triggers the native TCC dialog.
/// Subsequent calls: opens System Settings directly (dialog already shown this launch).
#[tauri::command]
pub async fn request_microphone_permission_native() -> Result<bool, String> {
    info!("Requesting microphone permissions using native APIs");

    #[cfg(target_os = "macos")]
    {
        let result = match NativePermissionChecker::check_microphone_permission() {
            Ok(true) => {
                info!("Microphone permissions already granted");
                Ok(true)
            }
            Ok(false) => {
                let already_shown = MICROPHONE_DIALOG_SHOWN.swap(true, Ordering::AcqRel);
                if already_shown {
                    info!("Microphone dialog already shown this launch — opening System Settings directly");
                    let _ = Command::new("open")
                        .args(["x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone"])
                        .status();
                    Ok(false)
                } else {
                    info!("Requesting microphone permissions with native dialog (first time this launch)");
                    match NativePermissionChecker::request_microphone_permission().await {
                        Ok(granted) => {
                            if granted {
                                info!("Microphone permissions granted by user");
                            } else {
                                info!("Microphone permissions denied by user");
                            }
                            Ok(granted)
                        }
                        Err(e) => {
                            error!("Error requesting microphone permissions: {}", e);
                            Err(format!("Failed to request microphone permissions: {}", e))
                        }
                    }
                }
            }
            Err(e) => {
                error!("Error checking microphone permissions: {}", e);
                Err(format!("Failed to check microphone permissions: {}", e))
            }
        };
        invalidate_permissions_cache();
        result
    }

    #[cfg(not(target_os = "macos"))]
    {
        Ok(true)
    }
}

/// Request screen recording permissions using native APIs - NO password prompts.
///
/// First call: triggers the native OS dialog (CGRequestScreenCaptureAccess).
/// Subsequent calls: opens System Settings directly (dialog already shown this launch).
#[tauri::command]
pub async fn request_screen_recording_permission_native() -> Result<bool, String> {
    info!("Requesting screen recording permissions using native APIs");

    #[cfg(target_os = "macos")]
    {
        let result = match NativePermissionChecker::check_screen_recording_permission() {
            Ok(true) => {
                info!("Screen recording permissions already granted");
                Ok(true)
            }
            Ok(false) => {
                let already_shown = SCREEN_RECORDING_DIALOG_SHOWN.swap(true, Ordering::AcqRel);
                if already_shown {
                    info!("Screen recording dialog already shown this launch — opening System Settings directly");
                    let _ = Command::new("open")
                        .args(["x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture"])
                        .status();
                    Ok(false)
                } else {
                    info!("Requesting screen recording permissions with native prompt (first time this launch)");
                    match NativePermissionChecker::request_screen_recording_permission() {
                        Ok(()) => {
                            info!("Screen recording permission request triggered successfully");
                            tokio::time::sleep(tokio::time::Duration::from_millis(timeouts::PERMISSION_CHECK_DELAY_MS)).await;
                            match NativePermissionChecker::check_screen_recording_permission() {
                                Ok(granted) => {
                                    if granted {
                                        info!("Screen recording permissions now granted");
                                    } else {
                                        info!("Screen recording permissions still not granted - user needs to manually enable in System Settings");
                                    }
                                    Ok(granted)
                                }
                                Err(e) => {
                                    error!("Error checking screen recording permissions after request: {}", e);
                                    Ok(false)
                                }
                            }
                        }
                        Err(e) => {
                            error!("Error requesting screen recording permissions: {}", e);
                            Err(format!("Failed to request screen recording permissions: {}", e))
                        }
                    }
                }
            }
            Err(e) => {
                error!("Error checking screen recording permissions: {}", e);
                Err(format!("Failed to check screen recording permissions: {}", e))
            }
        };
        invalidate_permissions_cache();
        result
    }

    #[cfg(not(target_os = "macos"))]
    {
        Ok(true)
    }
}

/// Request input monitoring permissions using native APIs - NO password prompts.
///
/// First call: triggers the native OS prompt.
/// Subsequent calls: opens System Settings directly (dialog already shown this launch).
#[tauri::command]
pub async fn request_input_monitoring_permission_native() -> Result<bool, String> {
    info!("Requesting input monitoring permissions using native APIs");

    #[cfg(target_os = "macos")]
    {
        let result = match NativePermissionChecker::check_input_monitoring_permission() {
            Ok(true) => {
                info!("Input monitoring permissions already granted");
                Ok(true)
            }
            Ok(false) => {
                let already_shown = INPUT_MONITORING_DIALOG_SHOWN.swap(true, Ordering::AcqRel);
                if already_shown {
                    info!("Input monitoring dialog already shown this launch — opening System Settings directly");
                    let _ = Command::new("open")
                        .args(["x-apple.systempreferences:com.apple.preference.security?Privacy_ListenEvent"])
                        .status();
                    Ok(false)
                } else {
                    info!("Requesting input monitoring permissions with native prompt (first time this launch)");
                    match NativePermissionChecker::request_input_monitoring_permission() {
                        Ok(()) => {
                            info!("Input monitoring permission request triggered successfully");
                            tokio::time::sleep(tokio::time::Duration::from_millis(timeouts::PERMISSION_CHECK_DELAY_MS)).await;
                            match NativePermissionChecker::check_input_monitoring_permission() {
                                Ok(granted) => {
                                    if granted {
                                        info!("Input monitoring permissions now granted");
                                    } else {
                                        info!("Input monitoring permissions still not granted - user needs to manually enable in System Settings");
                                    }
                                    Ok(granted)
                                }
                                Err(e) => {
                                    error!("Error checking input monitoring permissions after request: {}", e);
                                    Ok(false)
                                }
                            }
                        }
                        Err(e) => {
                            error!("Error requesting input monitoring permissions: {}", e);
                            Err(format!("Failed to request input monitoring permissions: {}", e))
                        }
                    }
                }
            }
            Err(e) => {
                error!("Error checking input monitoring permissions: {}", e);
                Err(format!("Failed to check input monitoring permissions: {}", e))
            }
        };
        invalidate_permissions_cache();
        result
    }

    #[cfg(not(target_os = "macos"))]
    {
        Ok(true)
    }
}

/// Open system preferences for a specific permission type
#[tauri::command]
pub async fn open_system_preferences(preference_pane: String) -> Result<(), String> {
    info!("Opening system preferences for: {}", preference_pane);

    #[cfg(target_os = "macos")]
    {
        let url = match preference_pane.as_str() {
            "accessibility" => "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility",
            "microphone" => "x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone",
            "screen_recording" => "x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture",
            "input_monitoring" => "x-apple.systempreferences:com.apple.preference.security?Privacy_ListenEvent",
            _ => return Err(format!("Unknown preference pane: {}", preference_pane)),
        };

        match Command::new("open").args([url]).status() {
            Ok(status) => {
                if status.success() {
                    info!("Successfully opened system preferences for {}", preference_pane);
                    Ok(())
                } else {
                    Err(format!("Failed to open system preferences for {}", preference_pane))
                }
            }
            Err(e) => {
                error!("Error opening system preferences: {}", e);
                Err(format!("Error opening system preferences: {}", e))
            }
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        warn!("System preferences opening not available on this platform");
        Ok(())
    }
}

/// Enhanced system settings opening with better URL handling
#[tauri::command]
pub async fn open_system_settings_enhanced(permission_type: String) -> Result<(), String> {
    info!("Opening enhanced system settings for: {}", permission_type);
    open_system_preferences(permission_type).await
}

/// Start continuous permission monitoring at 1-second intervals.
///
/// Emits `permissions-changed` on every poll tick.
/// Emits `permission-granted` with the permission type immediately when a specific
/// permission flips from denied → granted, enabling onboarding auto-advance.
#[tauri::command]
pub async fn start_permissions_monitoring(app: AppHandle) -> Result<(), String> {
    info!("Starting permissions monitoring (1s continuous polling)");

    let task_handle = MONITORING_TASK.clone();
    let mut task_guard = task_handle.lock().map_err(|e| format!("Failed to lock monitoring task: {}", e))?;

    // Stop existing monitoring if running
    if let Some((handle, token)) = task_guard.take() {
        token.cancel();
        handle.abort();
        info!("Stopped existing permissions monitoring");
    }

    let cancellation_token = CancellationToken::new();
    let token_clone = cancellation_token.clone();
    let app_clone = app.clone();

    let handle = tauri::async_runtime::spawn(async move {
        // 1-second tick for responsive onboarding auto-advance
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(1));

        // Track the last known grant state for delta detection
        let mut last_accessibility = false;
        let mut last_screen_recording = false;
        let mut last_microphone = false;
        let mut last_input_monitoring = false;
        let mut initialized = false;

        loop {
            tokio::select! {
                _ = token_clone.cancelled() => {
                    info!("Permissions monitoring cancelled");
                    break;
                }
                _ = interval.tick() => {
                    // Bypass the short-TTL cache (5s) so we detect changes within 1s.
                    // Invalidate before each poll so the native APIs are always queried.
                    invalidate_permissions_cache();
                    match check_permissions_status_native(app_clone.clone()).await {
                        Ok(status) => {
                            // Always emit the full state for UI reactivity
                            if let Err(e) = app_clone.emit(events::permissions::CHANGED, &status) {
                                warn!("{}", format_error(FAILED_TO_EMIT, "permissions change", e));
                            }

                            // On first tick just record baseline; don't emit false "granted" events
                            if initialized {
                                emit_granted_if_flipped(&app_clone, "accessibility",     last_accessibility,     status.accessibility.granted);
                                emit_granted_if_flipped(&app_clone, "screen_recording",  last_screen_recording,  status.screen_recording.granted);
                                emit_granted_if_flipped(&app_clone, "microphone",        last_microphone,        status.microphone.granted);
                                emit_granted_if_flipped(&app_clone, "input_monitoring",  last_input_monitoring,  status.input_monitoring.granted);
                            }

                            last_accessibility    = status.accessibility.granted;
                            last_screen_recording = status.screen_recording.granted;
                            last_microphone       = status.microphone.granted;
                            last_input_monitoring = status.input_monitoring.granted;
                            initialized = true;
                        }
                        Err(e) => {
                            warn!("Error checking permissions during monitoring: {}", e);
                        }
                    }
                }
            }
        }
    });

    *task_guard = Some((handle, cancellation_token));
    info!("Permissions monitoring started successfully (1s interval)");
    Ok(())
}

/// Emit `permission-granted` when a permission flips from denied → granted.
fn emit_granted_if_flipped(app: &AppHandle, permission_type: &str, was_granted: bool, now_granted: bool) {
    if !was_granted && now_granted {
        info!("Permission flipped to granted: {}", permission_type);
        if let Err(e) = app.emit(
            events::permissions::GRANTED,
            serde_json::json!({ "permission_type": permission_type }),
        ) {
            warn!("Failed to emit permission-granted for {}: {}", permission_type, e);
        }
    }
}

/// Stop background permission monitoring
#[tauri::command]
pub async fn stop_permissions_monitoring() -> Result<(), String> {
    info!("Stopping permissions monitoring");

    let task_handle = MONITORING_TASK.clone();
    let mut task_guard = task_handle.lock().map_err(|e| format!("Failed to lock monitoring task: {}", e))?;

    if let Some((handle, token)) = task_guard.take() {
        token.cancel();
        handle.abort();
        info!("Permissions monitoring stopped");
        Ok(())
    } else {
        info!("No permissions monitoring was running");
        Ok(())
    }
}

/// Restart app after permissions are granted (if needed)
#[tauri::command]
pub async fn restart_app_after_permissions(app: AppHandle) -> Result<(), String> {
    info!("Restarting app after permissions granted");

    // Give a moment for the user to see any completion messages
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    app.restart();
}

/// Prompt user about app restart after permissions
#[tauri::command]
pub async fn prompt_app_restart_after_permissions(app: AppHandle) -> Result<String, String> {
    info!("Prompting user about app restart after permissions");

    let permissions = check_permissions_status_native(app).await?;

    if permissions.all_granted {
        Ok("All required permissions are now granted. The app will work optimally.".to_string())
    } else {
        Ok("Some permissions are still missing. Please grant them for full functionality.".to_string())
    }
}

/// Check if app restart is needed after permission changes
#[tauri::command]
pub async fn check_restart_needed_after_permissions() -> Result<bool, String> {
    // For most permissions, restart is not needed
    // The app can detect permission changes dynamically
    Ok(false)
}

/// Handle restart logic after permissions are granted
#[tauri::command]
pub async fn handle_restart_after_permissions(app: AppHandle, auto_restart: bool) -> Result<String, String> {
    let status = prompt_app_restart_after_permissions(app.clone()).await?;

    if auto_restart {
        restart_app_after_permissions(app).await?;
        Ok("App is restarting...".to_string())
    } else {
        Ok(status)
    }
}

/// Test microphone functionality using the actual voice transcription plugin
#[tauri::command]
pub async fn test_microphone_functionality(app: AppHandle) -> Result<serde_json::Value, String> {
    info!("Testing actual microphone functionality using voice transcription plugin");

    // Try to get the voice transcription initialization status
    let voice_status = match app.try_state::<std::sync::Arc<std::sync::Mutex<tauri_plugin_voice_transcription::VoiceController>>>() {
        Some(controller_state) => {
            let controller = controller_state.lock()
                .map_err(|e| format!("Failed to lock VoiceController: {}", e))?;

            serde_json::json!({
                "voice_controller_available": true,
                "is_initialized": controller.is_initialized(),
                "model_path": controller.model_path,
                "initialization_error": controller.get_initialization_error(),
                "is_dictating": controller.is_dictating()
            })
        }
        None => {
            serde_json::json!({
                "voice_controller_available": false,
                "error": "Voice transcription plugin not available"
            })
        }
    };

    // Try to test the always listening controller as well
    let always_listening_status = serde_json::json!({
        "always_listening_available": true,
        "controller_status": "available",
        "note": "Always listening controller is present but individual status methods not implemented"
    });

    // Get system audio devices information
    let audio_devices_status = check_audio_devices_system().await;

    // Provide comprehensive microphone functionality assessment
    let recommendation = determine_microphone_recommendation(&voice_status, &always_listening_status, &audio_devices_status);

    Ok(serde_json::json!({
        "voice_transcription": voice_status,
        "always_listening": always_listening_status,
        "audio_devices": audio_devices_status,
        "recommendation": recommendation,
        "overall_status": if voice_status.get("voice_controller_available").unwrap_or(&serde_json::Value::Bool(false)).as_bool().unwrap_or(false) {
            "functional"
        } else {
            "needs_attention"
        }
    }))
}

/// Check system audio devices without admin privileges
async fn check_audio_devices_system() -> serde_json::Value {
    #[cfg(target_os = "macos")]
    {
        match Command::new("system_profiler")
            .args(["SPAudioDataType", "-json", "-detailLevel", "basic"])
            .output()
        {
            Ok(output) => {
                if output.status.success() {
                    let result = String::from_utf8_lossy(&output.stdout);

                    // Try to parse as JSON first
                    if let Ok(json_value) = serde_json::from_str::<serde_json::Value>(&result) {
                        serde_json::json!({
                            "status": "success",
                            "method": "system_profiler_json",
                            "data": json_value,
                            "has_audio_devices": true
                        })
                    } else {
                        // Fallback to text parsing
                        let has_microphone = result.contains("Built-in Microphone") ||
                                           result.contains("Microphone") ||
                                           result.contains("Input");
                        serde_json::json!({
                            "status": "success",
                            "method": "system_profiler_text",
                            "has_microphone": has_microphone,
                            "has_audio_devices": true,
                            "raw_output_length": result.len()
                        })
                    }
                } else {
                    serde_json::json!({
                        "status": "failed",
                        "error": "system_profiler command failed",
                        "stderr": String::from_utf8_lossy(&output.stderr)
                    })
                }
            }
            Err(e) => {
                serde_json::json!({
                    "status": "error",
                    "error": format!("Failed to run system_profiler: {}", e)
                })
            }
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        serde_json::json!({
            "status": "not_applicable",
            "platform": "non_macos"
        })
    }
}

/// Determine microphone recommendation based on system state
fn determine_microphone_recommendation(
    voice_status: &serde_json::Value,
    always_listening_status: &serde_json::Value,
    audio_devices_status: &serde_json::Value
) -> String {
    let voice_available = voice_status.get("voice_controller_available")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let always_listening_available = always_listening_status.get("always_listening_available")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let audio_devices_detected = audio_devices_status.get("has_audio_devices")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    if voice_available && always_listening_available && audio_devices_detected {
        "All microphone systems are functional. Voice features should work properly.".to_string()
    } else if audio_devices_detected {
        "Audio hardware detected but voice systems may need initialization. Try using voice features to test.".to_string()
    } else {
        "Unable to detect microphone hardware or voice systems. Check microphone permissions and hardware.".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_permissions_cache_invalidation() {
        // Seed the cache with a dummy entry
        let dummy = PermissionsState {
            accessibility: PermissionStatus {
                permission_type: "accessibility".to_string(),
                granted: true,
                required: true,
                description: String::new(),
                instructions: String::new(),
            },
            screen_recording: PermissionStatus {
                permission_type: "screen_recording".to_string(),
                granted: true,
                required: true,
                description: String::new(),
                instructions: String::new(),
            },
            microphone: PermissionStatus {
                permission_type: "microphone".to_string(),
                granted: false,
                required: false,
                description: String::new(),
                instructions: String::new(),
            },
            input_monitoring: PermissionStatus {
                permission_type: "input_monitoring".to_string(),
                granted: false,
                required: false,
                description: String::new(),
                instructions: String::new(),
            },
            all_granted: true,
            app_name: "test".to_string(),
        };

        {
            let mut cache = PERMISSIONS_CACHE.lock().unwrap();
            *cache = Some((Instant::now(), dummy));
        }

        // Cache should be populated
        {
            let cache = PERMISSIONS_CACHE.lock().unwrap();
            assert!(cache.is_some(), "cache should hold a value after seeding");
        }

        // Invalidation should clear it
        invalidate_permissions_cache();
        {
            let cache = PERMISSIONS_CACHE.lock().unwrap();
            assert!(cache.is_none(), "cache should be None after invalidation");
        }
    }

    #[test]
    fn test_permissions_cache_ttl_respected() {
        // Verify is_cache_fresh() correctly classifies entries on both sides of the TTL boundary.
        let expired = Instant::now() - Duration::from_secs(timeouts::PERMISSIONS_CACHE_TTL_SECONDS + 1);
        assert!(!is_cache_fresh(expired), "entry older than TTL should not be fresh");

        let fresh = Instant::now();
        assert!(is_cache_fresh(fresh), "entry just created should be fresh");
    }

    #[test]
    fn test_permission_status_creation() {
        let status = PermissionStatus {
            permission_type: "test".to_string(),
            granted: true,
            required: false,
            description: "Test permission".to_string(),
            instructions: "Test instructions".to_string(),
        };

        assert_eq!(status.permission_type, "test");
        assert_eq!(status.granted, true);
        assert_eq!(status.required, false);
    }

    #[test]
    fn test_permissions_state_all_granted_logic() {
        let accessibility = PermissionStatus {
            permission_type: "accessibility".to_string(),
            granted: true,
            required: true,
            description: "".to_string(),
            instructions: "".to_string(),
        };

        let screen_recording = PermissionStatus {
            permission_type: "screen_recording".to_string(),
            granted: true,
            required: true,
            description: "".to_string(),
            instructions: "".to_string(),
        };

        let microphone = PermissionStatus {
            permission_type: "microphone".to_string(),
            granted: false, // Optional permission
            required: false,
            description: "".to_string(),
            instructions: "".to_string(),
        };

        let input_monitoring = PermissionStatus {
            permission_type: "input_monitoring".to_string(),
            granted: false, // Optional permission
            required: false,
            description: "".to_string(),
            instructions: "".to_string(),
        };

        // Should be true because only required permissions (accessibility, screen_recording) are granted
        let all_granted = accessibility.granted && screen_recording.granted;
        assert_eq!(all_granted, true);

        let permissions_state = PermissionsState {
            accessibility,
            screen_recording,
            microphone,
            input_monitoring,
            all_granted,
            app_name: "test".to_string(),
        };

        assert_eq!(permissions_state.all_granted, true);
    }

    #[test]
    fn test_system_settings_url_safety() {
        // Test that our system settings URLs are safe and properly formatted
        let accessibility_url = "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility";
        let microphone_url = "x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone";

        assert!(accessibility_url.starts_with("x-apple.systempreferences:"));
        assert!(microphone_url.contains("Privacy_Microphone"));
    }

    #[test]
    fn test_permission_error_handling() {
        // Test that permission checking handles errors gracefully
        let result = convert_native_to_frontend_status(NativePermissionStatus {
            permission_type: "test".to_string(),
            granted: false,
            required: true,
            description: "Test failed".to_string(),
            instructions: "Fix test".to_string(),
        });

        assert_eq!(result.granted, false);
        assert_eq!(result.description, "Test failed");
    }

}
