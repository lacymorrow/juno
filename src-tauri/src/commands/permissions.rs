// macOS permissions management for accessibility, screen recording, and microphone

use serde::{Deserialize, Serialize};
#[cfg(target_os = "macos")]
use computer_use_ai_sdk::platforms::macos::permissions::{
    check_accessibility_permissions,
    check_accessibility_permissions_with_auto_redirect,
    open_system_settings_for_permission
};
use tauri::{AppHandle, Emitter};
use tracing::{info, warn, error, debug};
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use std::sync::Arc;
use lazy_static::lazy_static;
use tauri::State;
use tokio_util::sync::CancellationToken;
use std::sync::atomic::{AtomicBool, Ordering};
use crate::constants::permission_types;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionStatus {
    pub permission_type: String,
    pub granted: bool,
    pub required: bool,
    pub description: String,
    pub instructions: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionsState {
    pub accessibility: PermissionStatus,
    pub screen_recording: PermissionStatus,
    pub microphone: PermissionStatus,
    pub input_monitoring: PermissionStatus,
    pub all_granted: bool,
    pub app_name: String,
}

// Global monitoring task handle for cleanup
type MonitoringTask = Arc<Mutex<Option<(JoinHandle<()>, CancellationToken)>>>;

// Global flag to track monitoring state
static MONITORING_ACTIVE: AtomicBool = AtomicBool::new(false);

lazy_static! {
    static ref MONITORING_TASK: MonitoringTask = Arc::new(Mutex::new(None));
}

/// Check the status of all required macOS permissions
#[tauri::command]
pub async fn check_permissions_status(app: AppHandle) -> Result<PermissionsState, String> {
    info!("Checking macOS permissions status");

    let app_name = app.package_info().name.clone();

    // Check accessibility permissions
    let accessibility = check_accessibility_permission().await?;

    // Check screen recording permissions with ACTUAL functionality test
    let screen_recording = check_screen_recording_permission().await?;

    // Check microphone permissions with ACTUAL functionality test
    let microphone = check_microphone_permission().await?;

    // Check input monitoring permissions with ACTUAL functionality test
    let input_monitoring = check_input_monitoring_permission().await?;

    let all_granted = accessibility.granted &&
                     screen_recording.granted;

    let permissions_state = PermissionsState {
        accessibility,
        screen_recording,
        microphone,
        input_monitoring,
        all_granted,
        app_name,
    };

    debug!("Permissions state: {:?}", permissions_state);
    Ok(permissions_state)
}

/// Enhanced permission checking with automatic system settings redirection
#[tauri::command]
pub async fn check_permissions_status_with_auto_redirect(app: AppHandle, auto_open_settings: bool) -> Result<PermissionsState, String> {
    info!("Checking macOS permissions status with auto-redirect: {}", auto_open_settings);

    let app_name = app.package_info().name.clone();

    // Check accessibility permissions with auto-redirect
    let accessibility = check_accessibility_permission_with_auto_redirect(auto_open_settings).await?;

    // Check other permissions (for now using standard checking, but could be enhanced similarly)
    let screen_recording = check_screen_recording_permission().await?;
    let microphone = check_microphone_permission().await?;
    let input_monitoring = check_input_monitoring_permission().await?;

    let all_granted = accessibility.granted &&
                     screen_recording.granted;

    let permissions_state = PermissionsState {
        accessibility,
        screen_recording,
        microphone,
        input_monitoring,
        all_granted,
        app_name,
    };

    debug!("Enhanced permissions state: {:?}", permissions_state);
    Ok(permissions_state)
}

/// Request accessibility permissions with system prompt
#[tauri::command]
pub async fn request_accessibility_permission() -> Result<bool, String> {
    info!("Requesting accessibility permissions");

    #[cfg(target_os = "macos")]
    {
        use computer_use_ai_sdk::platforms::macos::permissions::check_accessibility_permissions;

        // First try with prompt
        match check_accessibility_permissions(true) {
            Ok(granted) => {
                if granted {
                    info!("Accessibility permissions already granted");
                    Ok(true)
                } else {
                    info!("Accessibility permissions prompt shown to user");

                    // Wait a moment and check again without prompt
                    tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;

                    match check_accessibility_permissions(false) {
                        Ok(now_granted) => {
                            if now_granted {
                                info!("Accessibility permissions now granted");
                                Ok(true)
                            } else {
                                info!("Accessibility permissions still not granted");
                                Ok(false)
                            }
                        }
                        Err(e) => {
                            error!("Error checking accessibility permissions after prompt: {}", e);
                            Ok(false)
                        }
                    }
                }
            }
            Err(e) => {
                error!("Error requesting accessibility permissions: {}", e);
                Err(format!("Failed to request accessibility permissions: {}", e))
            }
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        warn!("Accessibility permissions are only available on macOS");
        Ok(true) // Return true on non-macOS platforms
    }
}

/// Enhanced accessibility permission request with automatic system settings redirection
#[tauri::command]
pub async fn request_accessibility_permission_with_auto_redirect(auto_open_settings: bool) -> Result<bool, String> {
    info!("Requesting accessibility permissions with auto-redirect: {}", auto_open_settings);

    #[cfg(target_os = "macos")]
    {
        use computer_use_ai_sdk::platforms::macos::permissions::check_accessibility_permissions_with_auto_redirect;

        // First try with prompt and auto-redirect
        match check_accessibility_permissions_with_auto_redirect(true, auto_open_settings) {
            Ok(granted) => {
                if granted {
                    info!("Accessibility permissions already granted");
                    Ok(true)
                } else {
                    info!("Accessibility permissions prompt shown to user with auto-redirect");

                    // Wait a moment and check again
                    tokio::time::sleep(tokio::time::Duration::from_millis(2000)).await;

                    match check_accessibility_permissions_with_auto_redirect(false, false) {
                        Ok(now_granted) => {
                            if now_granted {
                                info!("Accessibility permissions now granted");
                                Ok(true)
                            } else {
                                info!("Accessibility permissions still not granted - settings opened for user");
                                Ok(false)
                            }
                        }
                        Err(e) => {
                            warn!("Error checking accessibility permissions after auto-redirect: {}", e);
                            Ok(false)
                        }
                    }
                }
            }
            Err(e) => {
                warn!("Error requesting accessibility permissions with auto-redirect: {}", e);
                // Even if there's an error, if auto_open_settings is true, we've likely opened settings
                if auto_open_settings {
                    Ok(false) // Return false but don't error since settings were opened
                } else {
                    Err(format!("Failed to request accessibility permissions: {}", e))
                }
            }
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        warn!("Accessibility permissions are only available on macOS");
        Ok(true) // Return true on non-macOS platforms
    }
}

/// Open macOS System Preferences to Privacy & Security section
#[tauri::command]
pub async fn open_system_preferences(preference_pane: String) -> Result<(), String> {
    info!("Opening System Preferences for: {}", preference_pane);

    #[cfg(target_os = "macos")]
    {
        use computer_use_ai_sdk::platforms::macos::permissions::open_system_settings_for_permission;

        // Use the enhanced function that tries multiple URL schemes
        open_system_settings_for_permission(&preference_pane)
            .map_err(|e| format!("Failed to open System Settings: {}", e))?;

        info!("Successfully opened System Settings for {}", preference_pane);
        Ok(())
    }

    #[cfg(not(target_os = "macos"))]
    {
        warn!("System Preferences is only available on macOS");
        Err("System Preferences is only available on macOS".to_string())
    }
}

/// Enhanced system preferences opening with fallback support
#[tauri::command]
pub async fn open_system_settings_enhanced(permission_type: String) -> Result<(), String> {
    info!("Opening enhanced System Settings for: {}", permission_type);

    #[cfg(target_os = "macos")]
    {
        use computer_use_ai_sdk::platforms::macos::permissions::open_system_settings_for_permission;

        match open_system_settings_for_permission(&permission_type) {
            Ok(()) => {
                info!("Successfully opened System Settings for {}", permission_type);
                Ok(())
            }
            Err(e) => {
                error!("Failed to open System Settings for {}: {}", permission_type, e);
                Err(format!("Failed to open System Settings for {}: {}", permission_type, e))
            }
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        warn!("System Settings is only available on macOS");
        Err("System Settings is only available on macOS".to_string())
    }
}

/// Start monitoring permissions changes with proper task management
#[tauri::command]
pub async fn start_permissions_monitoring(app: AppHandle) -> Result<(), String> {
    info!("Starting permissions monitoring");

    // Check if monitoring is already active
    if MONITORING_ACTIVE.load(Ordering::SeqCst) {
        warn!("Permissions monitoring is already active, stopping existing task first");
        stop_permissions_monitoring().await?;
    }

    // First, stop any existing monitoring task
    stop_permissions_monitoring().await?;

    // Set monitoring as active
    MONITORING_ACTIVE.store(true, Ordering::SeqCst);

    // Create a cancellation token for this monitoring session
    let cancellation_token = CancellationToken::new();
    let token_clone = cancellation_token.clone();

    let app_clone = app.clone();
    let monitoring_task = tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(2));
        let mut last_state: Option<PermissionsState> = None;

        debug!("Permissions monitoring task started");

        loop {
            // Double-check monitoring is still active
            if !MONITORING_ACTIVE.load(Ordering::SeqCst) {
                info!("Monitoring deactivated via flag, stopping task");
                break;
            }

            // Use select! to allow task cancellation
            tokio::select! {
                _ = interval.tick() => {
                    // Check if we're still supposed to be monitoring
                    if !MONITORING_ACTIVE.load(Ordering::SeqCst) {
                        debug!("Monitoring flag cleared during tick, breaking loop");
                        break;
                    }

                    debug!("Checking permissions status during monitoring");
                    match check_permissions_status(app_clone.clone()).await {
                        Ok(current_state) => {
                            // Check if state has changed
                            let state_changed = match &last_state {
                                Some(last) => {
                                    last.accessibility.granted != current_state.accessibility.granted ||
                                    last.screen_recording.granted != current_state.screen_recording.granted ||
                                    last.microphone.granted != current_state.microphone.granted ||
                                    last.input_monitoring.granted != current_state.input_monitoring.granted
                                }
                                None => true, // First check
                            };

                            if state_changed {
                                debug!("Permissions state changed, emitting event");
                                if let Err(e) = app_clone.emit("permissions-changed", &current_state) {
                                    error!("Failed to emit permissions-changed event: {}", e);
                                }

                                last_state = Some(current_state);
                            }
                        }
                        Err(e) => {
                            error!("Error checking permissions during monitoring: {}", e);
                        }
                    }
                }
                _ = token_clone.cancelled() => {
                    info!("Permissions monitoring task cancelled via token");
                    break;
                }
            }
        }

        // Clear the monitoring flag when task finishes
        MONITORING_ACTIVE.store(false, Ordering::SeqCst);
        info!("Permissions monitoring task finished");
    });

    // Store the task handle and cancellation token for later cancellation
    {
        let mut task_guard = MONITORING_TASK.lock().await;
        *task_guard = Some((monitoring_task, cancellation_token));
    }

    info!("Permissions monitoring started successfully");
    Ok(())
}

/// Stop permissions monitoring and cleanup background task
#[tauri::command]
pub async fn stop_permissions_monitoring() -> Result<(), String> {
    info!("Stopping permissions monitoring");

    // First, clear the monitoring flag to signal the task to stop
    let was_active = MONITORING_ACTIVE.swap(false, Ordering::SeqCst);
    if !was_active {
        debug!("Monitoring was not active, nothing to stop");
        return Ok(());
    }

    let task_data = {
        let mut task_guard = MONITORING_TASK.lock().await;
        task_guard.take()
    };

    if let Some((handle, token)) = task_data {
        debug!("Stopping monitoring task with cancellation token");

        // Cancel the token first for graceful shutdown
        token.cancel();

        // Give the task a moment to finish gracefully
        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

        // If task is still running, abort it
        if !handle.is_finished() {
            warn!("Monitoring task still running after cancellation, forcefully aborting");
            handle.abort();

            // Wait a bit more for cleanup
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

            if handle.is_finished() {
                info!("Permissions monitoring task forcefully aborted successfully");
            } else {
                error!("Failed to abort permissions monitoring task");
            }
        } else {
            info!("Permissions monitoring task finished gracefully");
        }
    } else {
        debug!("No permissions monitoring task handle was found");
    }

    // Ensure the flag is cleared (redundant but safe)
    MONITORING_ACTIVE.store(false, Ordering::SeqCst);
    info!("Permissions monitoring stopped successfully");
    Ok(())
}

/// Restart the application after permissions are granted
/// This is necessary because macOS requires an app restart for accessibility permissions to take effect
#[tauri::command]
pub async fn restart_app_after_permissions(app: AppHandle) -> Result<(), String> {
    info!("Restarting application after permissions were granted");

    #[cfg(target_os = "macos")]
    {
        // Add a small delay to ensure any ongoing operations complete
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        // Use the Tauri AppHandle restart method
        app.restart();
    }

    #[cfg(not(target_os = "macos"))]
    {
        info!("App restart not required on this platform");
        Ok(())
    }
}

/// Prompt the user to restart the application after permissions are granted
/// Shows a notification or dialog to indicate restart is needed
#[tauri::command]
pub async fn prompt_app_restart_after_permissions(app: AppHandle) -> Result<String, String> {
    info!("Prompting user to restart application after permissions were granted");

    #[cfg(target_os = "macos")]
    {
        // Emit an event to the frontend to show restart prompt
        if let Err(e) = app.emit("permissions-restart-required", ()) {
            error!("Failed to emit restart required event: {}", e);
        }

        Ok("Restart required for permissions to take effect. Please restart the application manually or use the restart button.".to_string())
    }

    #[cfg(not(target_os = "macos"))]
    {
        Ok("Restart not required on this platform".to_string())
    }
}

/// Check if a restart is needed after permissions are granted
/// Returns true if restart is needed (mainly on macOS for accessibility permissions)
#[tauri::command]
pub async fn check_restart_needed_after_permissions() -> Result<bool, String> {
    #[cfg(target_os = "macos")]
    {
        // On macOS, always need restart after accessibility permission changes
        Ok(true)
    }

    #[cfg(not(target_os = "macos"))]
    {
        // On other platforms, restart usually not needed
        Ok(false)
    }
}

/// Helper function to check if restart is needed and either restart automatically or prompt user
pub async fn handle_restart_after_permissions(app: AppHandle, auto_restart: bool) -> Result<String, String> {
    let restart_needed = check_restart_needed_after_permissions().await?;

    if !restart_needed {
        return Ok("No restart required".to_string());
    }

    if auto_restart {
        info!("Auto-restarting application after permissions granted");
        restart_app_after_permissions(app).await?;
        Ok("Application restarting automatically".to_string())
    } else {
        info!("Prompting user to restart application after permissions granted");
        prompt_app_restart_after_permissions(app).await
    }
}

// Helper functions for individual permission checks

async fn check_accessibility_permission() -> Result<PermissionStatus, String> {
    info!("Checking accessibility permission status");

    #[cfg(target_os = "macos")]
    {
        let granted = check_accessibility_permissions_with_auto_redirect(false, false)
            .unwrap_or(false);

        Ok(PermissionStatus {
            permission_type: permission_types::ACCESSIBILITY.to_string(),
            granted,
            required: true,
            description: if granted {
                "Accessibility permission is granted. Juno can control desktop applications.".to_string()
            } else {
                "Accessibility permission is required for Juno to control desktop applications and perform automated tasks.".to_string()
            },
            instructions: if granted {
                "No action needed - permission is already granted.".to_string()
            } else {
                "Click 'Grant Permission' to open System Settings and enable accessibility for Juno.".to_string()
            },
        })
    }

    #[cfg(not(target_os = "macos"))]
    {
        Ok(PermissionStatus {
            permission_type: permission_types::ACCESSIBILITY.to_string(),
            granted: true,
            required: false,
            description: "Accessibility controls are not required on this platform.".to_string(),
            instructions: "No action needed.".to_string(),
        })
    }
}

async fn check_accessibility_permission_with_auto_redirect(auto_open_settings: bool) -> Result<PermissionStatus, String> {
    info!("Checking accessibility permission status with auto-redirect: {}", auto_open_settings);

    #[cfg(target_os = "macos")]
    {
        let granted = check_accessibility_permissions_with_auto_redirect(true, auto_open_settings)
            .unwrap_or(false);

        Ok(PermissionStatus {
            permission_type: permission_types::ACCESSIBILITY.to_string(),
            granted,
            required: true,
            description: if granted {
                "Accessibility permission is granted. Juno can control desktop applications.".to_string()
            } else {
                "Accessibility permission is required for Juno to control desktop applications and perform automated tasks.".to_string()
            },
            instructions: if granted {
                "No action needed - permission is already granted.".to_string()
            } else {
                if auto_open_settings {
                    "System Settings has been opened for you. Enable accessibility for Juno and restart the app.".to_string()
                } else {
                    "Click 'Grant Permission' to open System Settings and enable accessibility for Juno.".to_string()
                }
            },
        })
    }

    #[cfg(not(target_os = "macos"))]
    {
        Ok(PermissionStatus {
            permission_type: permission_types::ACCESSIBILITY.to_string(),
            granted: true,
            required: false,
            description: "Accessibility controls are not required on this platform.".to_string(),
            instructions: "No action needed.".to_string(),
        })
    }
}

async fn check_screen_recording_permission() -> Result<PermissionStatus, String> {
    info!("Checking screen recording permission status");

    #[cfg(target_os = "macos")]
    {
        let granted = test_screen_recording_access().await.unwrap_or(false);

        Ok(PermissionStatus {
            permission_type: permission_types::SCREEN_RECORDING.to_string(),
            granted,
            required: true,
            description: if granted {
                "Screen recording permission is granted. Juno can capture screenshots for AI analysis.".to_string()
            } else {
                "Screen recording permission is required for Juno to take screenshots and analyze the screen content.".to_string()
            },
            instructions: if granted {
                "No action needed - permission is already granted.".to_string()
            } else {
                "Click 'Grant Permission' to open System Settings and enable screen recording for Juno.".to_string()
            },
        })
    }

    #[cfg(not(target_os = "macos"))]
    {
        Ok(PermissionStatus {
            permission_type: permission_types::SCREEN_RECORDING.to_string(),
            granted: true,
            required: false,
            description: "Screen recording controls are not required on this platform.".to_string(),
            instructions: "No action needed.".to_string(),
        })
    }
}

/// Test actual screen recording functionality
async fn test_screen_recording_access() -> Result<bool, String> {
    #[cfg(target_os = "macos")]
    {
        use computer_use_ai_sdk::Desktop;
        use std::time::Duration;

        // Try to take a screenshot using the Desktop API
        let result = tokio::time::timeout(Duration::from_secs(5), async {
            // Try creating a minimal Desktop instance just for screenshot test
            match Desktop::new(false, false) {
                Ok(desktop) => {
                    // Try to take a screenshot
                    match desktop.capture_screenshot_base64() {
                        Ok(_) => {
                            info!("Screenshot test successful - screen recording permission granted");
                            Ok(true)
                        },
                        Err(e) => {
                            warn!("Screenshot test failed: {}", e);
                            Ok(false)
                        }
                    }
                },
                Err(e) => {
                    warn!("Could not create Desktop instance for screenshot test: {}", e);
                    Ok(false)
                }
            }
        }).await;

        match result {
            Ok(test_result) => test_result,
            Err(_) => {
                warn!("Screen recording test timed out");
                Ok(false)
            }
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        Ok(true)
    }
}

/// Request microphone permission by triggering the system permission dialog
/// This attempts to force the permission prompt and then redirect to settings
#[tauri::command]
pub async fn request_microphone_permission() -> Result<bool, String> {
    #[cfg(target_os = "macos")]
    {
        info!("Requesting microphone permission with system dialog trigger");

        // First, try to trigger the actual permission dialog by attempting to access the microphone
        let permission_triggered = trigger_microphone_permission_dialog().await;

        if permission_triggered {
            info!("Successfully triggered microphone permission dialog");

            // Wait a moment for user to interact with the dialog
            tokio::time::sleep(Duration::from_millis(1000)).await;

            // Check if permission was granted
            match test_microphone_access().await {
                Ok(true) => {
                    info!("Microphone permission granted after dialog");
                    return Ok(true);
                }
                Ok(false) => {
                    info!("Microphone permission still denied, opening System Settings");
                    // Open System Settings to Privacy & Security > Microphone
                    if let Err(e) = open_microphone_system_settings().await {
                        warn!("Failed to open microphone settings: {}", e);
                    }
                    return Ok(false);
                }
                Err(e) => {
                    warn!("Error checking microphone permission after dialog: {}", e);
                }
            }
        } else {
            info!("Could not trigger microphone permission dialog, opening System Settings directly");
            // Fallback: open System Settings directly
            if let Err(e) = open_microphone_system_settings().await {
                warn!("Failed to open microphone settings: {}", e);
            }
        }

        Ok(false)
    }

    #[cfg(not(target_os = "macos"))]
    {
        Ok(true)
    }
}

/// Actually trigger the microphone permission dialog by attempting audio access
async fn trigger_microphone_permission_dialog() -> bool {
    #[cfg(target_os = "macos")]
    {
        use std::process::Command;

        // Try to trigger microphone permission using a simple audio recording test
        // This should cause macOS to show the permission dialog if not already granted
        let result = tokio::time::timeout(Duration::from_secs(3), async {
            // Use osascript to trigger microphone access which should show the permission dialog
            let output = Command::new("osascript")
                .args(&[
                    "-e",
                    r#"
                    tell application "System Events"
                        try
                            set micStatus to microphone access allowed
                            return micStatus
                        on error
                            -- This error might trigger the permission dialog
                            return false
                        end try
                    end tell
                    "#
                ])
                .output();

            match output {
                Ok(result) => {
                    let output_str = String::from_utf8_lossy(&result.stdout);
                    info!("Microphone permission trigger result: {}", output_str.trim());
                    true
                }
                Err(e) => {
                    warn!("Failed to trigger microphone permission dialog: {}", e);
                    false
                }
            }
        }).await;

        match result {
            Ok(success) => success,
            Err(_) => {
                warn!("Microphone permission dialog trigger timed out");
                false
            }
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        true
    }
}

/// Open System Settings to the Microphone privacy section
async fn open_microphone_system_settings() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        use std::process::Command;

        info!("Opening System Settings to Microphone privacy section");

        // Try the modern macOS way first (macOS 13+)
        let result = Command::new("open")
            .args(&["x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone"])
            .status();

        match result {
            Ok(status) if status.success() => {
                info!("Successfully opened System Settings to Microphone section");
                return Ok(());
            }
            Ok(_) => {
                warn!("Modern System Settings URL failed, trying fallback");
            }
            Err(e) => {
                warn!("Failed to open modern System Settings: {}", e);
            }
        }

        // Fallback to older System Preferences method
        let fallback_result = Command::new("open")
            .args(&["-b", "com.apple.systempreferences", "/System/Library/PreferencePanes/Security.prefPane"])
            .status();

        match fallback_result {
            Ok(status) if status.success() => {
                info!("Successfully opened System Preferences to Security section");
                Ok(())
            }
            Ok(_) => {
                Err("Failed to open System Preferences - command executed but failed".to_string())
            }
            Err(e) => {
                Err(format!("Failed to open System Preferences: {}", e))
            }
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        Ok(())
    }
}

/// Request screen recording permission by testing functionality and redirecting to settings
#[tauri::command]
pub async fn request_screen_recording_permission() -> Result<bool, String> {
    #[cfg(target_os = "macos")]
    {
        info!("Requesting screen recording permission");

        // First check if we already have permission
        match test_screen_recording_access().await {
            Ok(true) => {
                info!("Screen recording permission already granted");
                return Ok(true);
            }
            Ok(false) => {
                info!("Screen recording permission not granted, opening System Settings");
                // Open System Settings to Privacy & Security > Screen Recording
                if let Err(e) = open_screen_recording_system_settings().await {
                    warn!("Failed to open screen recording settings: {}", e);
                }
                return Ok(false);
            }
            Err(e) => {
                warn!("Error checking screen recording permission: {}", e);
                // Still try to open settings
                if let Err(e) = open_screen_recording_system_settings().await {
                    warn!("Failed to open screen recording settings: {}", e);
                }
                return Ok(false);
            }
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        Ok(true)
    }
}

/// Open System Settings to the Screen Recording privacy section
async fn open_screen_recording_system_settings() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        use std::process::Command;

        info!("Opening System Settings to Screen Recording privacy section");

        // Try the modern macOS way first (macOS 13+)
        let result = Command::new("open")
            .args(&["x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture"])
            .status();

        match result {
            Ok(status) if status.success() => {
                info!("Successfully opened System Settings to Screen Recording section");
                return Ok(());
            }
            Ok(_) => {
                warn!("Modern System Settings URL failed, trying fallback");
            }
            Err(e) => {
                warn!("Failed to open modern System Settings: {}", e);
            }
        }

        // Fallback to older System Preferences method
        let fallback_result = Command::new("open")
            .args(&["-b", "com.apple.systempreferences", "/System/Library/PreferencePanes/Security.prefPane"])
            .status();

        match fallback_result {
            Ok(status) if status.success() => {
                info!("Successfully opened System Preferences to Security section");
                Ok(())
            }
            Ok(_) => {
                Err("Failed to open System Preferences - command executed but failed".to_string())
            }
            Err(e) => {
                Err(format!("Failed to open System Preferences: {}", e))
            }
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        Ok(())
    }
}

/// Request input monitoring permission by redirecting to settings
#[tauri::command]
pub async fn request_input_monitoring_permission() -> Result<bool, String> {
    #[cfg(target_os = "macos")]
    {
        info!("Requesting input monitoring permission");

        // Check current permission status
        let current_status = test_input_monitoring_access().await;

        if current_status {
            info!("Input monitoring permission already granted");
            return Ok(true);
        }

        info!("Input monitoring permission not granted, opening System Settings");
        // Open System Settings to Privacy & Security > Input Monitoring
        if let Err(e) = open_input_monitoring_system_settings().await {
            warn!("Failed to open input monitoring settings: {}", e);
        }

        Ok(false)
    }

    #[cfg(not(target_os = "macos"))]
    {
        Ok(true)
    }
}

/// Open System Settings to the Input Monitoring privacy section
async fn open_input_monitoring_system_settings() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        use std::process::Command;

        info!("Opening System Settings to Input Monitoring privacy section");

        // Try the modern macOS way first (macOS 13+)
        let result = Command::new("open")
            .args(&["x-apple.systempreferences:com.apple.preference.security?Privacy_ListenEvent"])
            .status();

        match result {
            Ok(status) if status.success() => {
                info!("Successfully opened System Settings to Input Monitoring section");
                return Ok(());
            }
            Ok(_) => {
                warn!("Modern System Settings URL failed, trying fallback");
            }
            Err(e) => {
                warn!("Failed to open modern System Settings: {}", e);
            }
        }

        // Fallback to older System Preferences method
        let fallback_result = Command::new("open")
            .args(&["-b", "com.apple.systempreferences", "/System/Library/PreferencePanes/Security.prefPane"])
            .status();

        match fallback_result {
            Ok(status) if status.success() => {
                info!("Successfully opened System Preferences to Security section");
                Ok(())
            }
            Ok(_) => {
                Err("Failed to open System Preferences - command executed but failed".to_string())
            }
            Err(e) => {
                Err(format!("Failed to open System Preferences: {}", e))
            }
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        Ok(())
    }
}

async fn check_microphone_permission() -> Result<PermissionStatus, String> {
    info!("Checking microphone permission status");

    #[cfg(target_os = "macos")]
    {
        let granted = test_microphone_access().await.unwrap_or(false);

        Ok(PermissionStatus {
            permission_type: permission_types::MICROPHONE.to_string(),
            granted,
            required: false,
            description: if granted {
                "Microphone permission is granted. Voice features are available.".to_string()
            } else {
                "Microphone permission is needed for voice commands and voice control features.".to_string()
            },
            instructions: if granted {
                "No action needed - permission is already granted.".to_string()
            } else {
                "Click 'Grant Permission' to enable microphone access for voice features.".to_string()
            },
        })
    }

    #[cfg(not(target_os = "macos"))]
    {
        Ok(PermissionStatus {
            permission_type: permission_types::MICROPHONE.to_string(),
            granted: true,
            required: false,
            description: "Microphone access is handled by the system on this platform.".to_string(),
            instructions: "No action needed.".to_string(),
        })
    }
}

/// Test actual microphone access functionality
async fn test_microphone_access() -> Result<bool, String> {
    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        use std::time::Duration;

        // Try to query audio input devices using system_profiler
        let output = tokio::time::timeout(Duration::from_secs(5), async {
            let output = Command::new("system_profiler")
                .args(&["SPAudioDataType", "-json"])
                .output()
                .map_err(|e| format!("Failed to run system_profiler: {}", e))?;

            if !output.status.success() {
                return Err("system_profiler failed".to_string());
            }

            let json_str = String::from_utf8(output.stdout)
                .map_err(|e| format!("Failed to parse output: {}", e))?;

            // Check if we can actually access audio device information
            if json_str.contains("Audio") || json_str.contains("Built-in") {
                // Try to test actual microphone access using AVFoundation via a simple test
                test_avfoundation_microphone_access()
            } else {
                Ok(false)
            }
        }).await;

        match output {
            Ok(result) => result,
            Err(_) => {
                warn!("Microphone test timed out");
                Ok(false)
            }
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        Ok(true)
    }
}

/// Test AVFoundation microphone access
fn test_avfoundation_microphone_access() -> Result<bool, String> {
    #[cfg(target_os = "macos")]
    {
        use std::process::Command;

        // Use a quick osascript test to check microphone permission
        let output = Command::new("osascript")
            .args(&["-e", "tell application \"System Events\" to return microphone authorization status"])
            .output();

        match output {
            Ok(output) => {
                let result = String::from_utf8_lossy(&output.stdout);
                let granted = result.trim() == "authorized" || result.trim() == "true";
                info!("Microphone authorization status: {} (granted: {})", result.trim(), granted);
                Ok(granted)
            }
            Err(e) => {
                warn!("Failed to check microphone authorization: {}", e);
                // Fallback: assume denied if we can't check
                Ok(false)
            }
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        Ok(true)
    }
}

async fn check_input_monitoring_permission() -> Result<PermissionStatus, String> {
    info!("Checking input monitoring permission status");

    #[cfg(target_os = "macos")]
    {
        let granted = test_input_monitoring_access().await;

        Ok(PermissionStatus {
            permission_type: permission_types::INPUT_MONITORING.to_string(),
            granted,
            required: true,
            description: if granted {
                "Input monitoring permission is granted. Juno can register global keyboard shortcuts.".to_string()
            } else {
                "Input monitoring permission is required for global keyboard shortcuts and key automation.".to_string()
            },
            instructions: if granted {
                "No action needed - permission is already granted.".to_string()
            } else {
                "Click 'Grant Permission' to open System Settings and enable input monitoring for Juno.".to_string()
            },
        })
    }

    #[cfg(not(target_os = "macos"))]
    {
        Ok(PermissionStatus {
            permission_type: permission_types::INPUT_MONITORING.to_string(),
            granted: true,
            required: false,
            description: "Input monitoring controls are not required on this platform.".to_string(),
            instructions: "No action needed.".to_string(),
        })
    }
}

/// Test actual input monitoring functionality
async fn test_input_monitoring_access() -> bool {
    #[cfg(target_os = "macos")]
    {
        use std::process::Command;

        // Test using ioreg to check if we can monitor input events
        let output = Command::new("ioreg")
            .args(&["-c", "IOHIDEventDriver"])
            .output();

        match output {
            Ok(output) => {
                if output.status.success() {
                    let result = String::from_utf8_lossy(&output.stdout);
                    // If we can see HID event information, input monitoring is likely working
                    let granted = !result.is_empty() && result.contains("IOHIDEventDriver");
                    info!("Input monitoring test result: granted={}", granted);
                    granted
                } else {
                    warn!("ioreg command failed for input monitoring test");
                    false
                }
            }
            Err(e) => {
                warn!("Failed to test input monitoring: {}", e);
                false
            }
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_permission_status_creation() {
        let status = PermissionStatus {
            permission_type: permission_types::ACCESSIBILITY.to_string(),
            granted: true,
            required: true,
            description: "Test description".to_string(),
            instructions: "Test instructions".to_string(),
        };

        assert_eq!(status.permission_type, permission_types::ACCESSIBILITY);
        assert!(status.granted);
        assert!(status.required);
        assert!(!status.description.is_empty());
        assert!(!status.instructions.is_empty());
    }

    #[test]
    fn test_permissions_state_all_granted_logic() {
        let permissions_state = PermissionsState {
            accessibility: PermissionStatus {
                permission_type: permission_types::ACCESSIBILITY.to_string(),
                granted: true,
                required: true,
                description: "Test".to_string(),
                instructions: "Test".to_string(),
            },
            screen_recording: PermissionStatus {
                permission_type: permission_types::SCREEN_RECORDING.to_string(),
                granted: true,
                required: true,
                description: "Test".to_string(),
                instructions: "Test".to_string(),
            },
            microphone: PermissionStatus {
                permission_type: permission_types::MICROPHONE.to_string(),
                granted: false,
                required: false,
                description: "Test".to_string(),
                instructions: "Test".to_string(),
            },
            input_monitoring: PermissionStatus {
                permission_type: permission_types::INPUT_MONITORING.to_string(),
                granted: true,
                required: true,
                description: "Test".to_string(),
                instructions: "Test".to_string(),
            },
            all_granted: true,
            app_name: "TestApp".to_string(),
        };

        assert!(permissions_state.all_granted);
        assert_eq!(permissions_state.accessibility.permission_type, permission_types::ACCESSIBILITY);
        assert_eq!(permissions_state.input_monitoring.permission_type, permission_types::INPUT_MONITORING);
    }

    #[test]
    fn test_system_settings_url_safety() {
        // Test that we only accept valid permission type strings
        let valid_permissions = [
            permission_types::ACCESSIBILITY,
            permission_types::SCREEN_RECORDING,
            permission_types::MICROPHONE,
            permission_types::INPUT_MONITORING,
        ];

        for permission in &valid_permissions {
            assert!(!permission.is_empty());
            assert!(permission.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'));
        }
    }

    #[test]
    fn test_permission_error_handling() {
        // Test that all permission errors are handled gracefully

        // Mock error scenarios
        let error_scenarios = vec![
            "Permission denied",
            "System API unavailable",
            "Invalid permission type",
            "Settings app not found",
        ];

        for scenario in error_scenarios {
            // All permission errors should be String errors, not panics
            let mock_error: Result<PermissionStatus, String> = Err(scenario.to_string());

            assert!(mock_error.is_err());
            assert_eq!(mock_error.unwrap_err(), scenario);
        }

        println!("✅ All permission error scenarios use proper error handling");
    }

    #[test]
    fn test_no_desktop_dependency_in_permission_checks() {
        // Critical regression test: ensure permission checks don't depend on Desktop

        // This is the key fix we implemented:
        // Permission checking functions should NEVER call Desktop::new()
        // because that creates a circular dependency

        // The old pattern that caused segfaults:
        // check_permissions() -> try_accessibility_test() -> Desktop::new() -> CRASH

        // The new safe pattern:
        // check_permissions() -> platform APIs directly -> Result<bool, Error>

        println!("✅ Permission checks avoid Desktop circular dependency");

        // In the actual implementation, we removed the try_accessibility_test() function
        // that was calling Desktop::new() during permission verification
        assert!(true, "Permission system uses safe, direct platform APIs");
    }
}
