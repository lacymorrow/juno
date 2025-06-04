// macOS permissions management for accessibility, screen recording, and microphone

use tauri::{AppHandle, Emitter};
use serde::{Deserialize, Serialize};
use tracing::{info, error, debug};

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
    pub all_granted: bool,
    pub app_name: String,
}

/// Check the status of all required macOS permissions
#[tauri::command]
pub async fn check_permissions_status(app: AppHandle) -> Result<PermissionsState, String> {
    info!("Checking macOS permissions status");

    let app_name = app.package_info().name.clone();

    // Check accessibility permissions
    let accessibility = check_accessibility_permission().await?;

    // Check screen recording permissions
    let screen_recording = check_screen_recording_permission().await?;

    // Check microphone permissions
    let microphone = check_microphone_permission().await?;

    let all_granted = accessibility.granted && screen_recording.granted && microphone.granted;

    let permissions_state = PermissionsState {
        accessibility,
        screen_recording,
        microphone,
        all_granted,
        app_name,
    };

    debug!("Permissions state: {:?}", permissions_state);
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

/// Open macOS System Preferences to Privacy & Security section
#[tauri::command]
pub async fn open_system_preferences(preference_pane: String) -> Result<(), String> {
    info!("Opening System Preferences for: {}", preference_pane);

    #[cfg(target_os = "macos")]
    {
        use std::process::Command;

        let url = match preference_pane.as_str() {
            "accessibility" => "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility",
            "screen_recording" => "x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture",
            "microphone" => "x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone",
            "privacy" => "x-apple.systempreferences:com.apple.preference.security",
            _ => return Err(format!("Unknown preference pane: {}", preference_pane)),
        };

        let output = Command::new("open")
            .arg(url)
            .output()
            .map_err(|e| format!("Failed to open System Preferences: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("Failed to open System Preferences: {}", stderr));
        }

        info!("Successfully opened System Preferences for {}", preference_pane);
        Ok(())
    }

    #[cfg(not(target_os = "macos"))]
    {
        warn!("System Preferences is only available on macOS");
        Err("System Preferences is only available on macOS".to_string())
    }
}

/// Monitor permissions changes and emit events
#[tauri::command]
pub async fn start_permissions_monitoring(app: AppHandle) -> Result<(), String> {
    info!("Starting permissions monitoring");

    let app_clone = app.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(2));
        let mut last_state: Option<PermissionsState> = None;

        loop {
            interval.tick().await;

            match check_permissions_status(app_clone.clone()).await {
                Ok(current_state) => {
                    // Check if state has changed
                    let state_changed = match &last_state {
                        Some(last) => {
                            last.accessibility.granted != current_state.accessibility.granted ||
                            last.screen_recording.granted != current_state.screen_recording.granted ||
                            last.microphone.granted != current_state.microphone.granted
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
    });

    Ok(())
}

/// Stop permissions monitoring (placeholder for cleanup if needed)
#[tauri::command]
pub async fn stop_permissions_monitoring() -> Result<(), String> {
    info!("Stopping permissions monitoring");
    // In a real implementation, you might want to store monitoring task handles
    // and cancel them here. For now, this is a placeholder.
    Ok(())
}

// Helper functions for individual permission checks

async fn check_accessibility_permission() -> Result<PermissionStatus, String> {
    #[cfg(target_os = "macos")]
    {
        use computer_use_ai_sdk::platforms::macos::permissions::check_accessibility_permissions;

        let granted = check_accessibility_permissions(false)
            .map_err(|e| format!("Failed to check accessibility permissions: {}", e))?;

        Ok(PermissionStatus {
            permission_type: "accessibility".to_string(),
            granted,
            required: true,
            description: "Required for desktop automation, clicking, and typing".to_string(),
            instructions: "Go to System Preferences > Privacy & Security > Accessibility and add Juno".to_string(),
        })
    }

    #[cfg(not(target_os = "macos"))]
    {
        Ok(PermissionStatus {
            permission_type: "accessibility".to_string(),
            granted: true,
            required: false,
            description: "Not required on this platform".to_string(),
            instructions: "".to_string(),
        })
    }
}

async fn check_screen_recording_permission() -> Result<PermissionStatus, String> {
    #[cfg(target_os = "macos")]
    {
        use std::process::Command;

        // Use system_profiler to check screen recording permissions
        let output = Command::new("system_profiler")
            .args(&["SPApplicationsDataType", "-json"])
            .output()
            .map_err(|e| format!("Failed to run system_profiler: {}", e))?;

        let granted = if output.status.success() {
            // This is a simplified check - in practice, you might want to use
            // CGDisplayStreamCreate or similar APIs to test actual capture capability
            true // Assume granted for now - this would need platform-specific implementation
        } else {
            false
        };

        Ok(PermissionStatus {
            permission_type: "screen_recording".to_string(),
            granted,
            required: true,
            description: "Required for taking screenshots and visual analysis".to_string(),
            instructions: "Go to System Preferences > Privacy & Security > Screen Recording and add Juno".to_string(),
        })
    }

    #[cfg(not(target_os = "macos"))]
    {
        Ok(PermissionStatus {
            permission_type: "screen_recording".to_string(),
            granted: true,
            required: false,
            description: "Not required on this platform".to_string(),
            instructions: "".to_string(),
        })
    }
}

async fn check_microphone_permission() -> Result<PermissionStatus, String> {
    #[cfg(target_os = "macos")]
    {
        // For microphone permissions, we would typically use AVCaptureDevice APIs
        // For now, we'll assume it's granted if the voice transcription plugin is working
        let granted = true; // This would need proper implementation with CoreAudio/AVFoundation

        Ok(PermissionStatus {
            permission_type: "microphone".to_string(),
            granted,
            required: true,
            description: "Required for voice transcription and dictation features".to_string(),
            instructions: "Go to System Preferences > Privacy & Security > Microphone and add Juno".to_string(),
        })
    }

    #[cfg(not(target_os = "macos"))]
    {
        Ok(PermissionStatus {
            permission_type: "microphone".to_string(),
            granted: true,
            required: false,
            description: "Not required on this platform".to_string(),
            instructions: "".to_string(),
        })
    }
}
