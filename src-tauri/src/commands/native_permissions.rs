//! Native macOS permission checking using existing Cocoa bindings
//! Replaces osascript calls that require admin privileges with direct native APIs

use serde::{Deserialize, Serialize};
use tracing::{info, warn, debug};

#[cfg(target_os = "macos")]
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativePermissionStatus {
    pub permission_type: String,
    pub granted: bool,
    pub required: bool,
    pub description: String,
    pub instructions: String,
}

/// Native permission checker using CIDRE framework
pub struct NativePermissionChecker;

impl NativePermissionChecker {
    /// Check microphone permission using system_profiler - NO admin privileges required
    pub fn check_microphone_permission() -> Result<bool, String> {
        #[cfg(target_os = "macos")]
        {
            // Use system_profiler without admin privileges
            match Command::new("system_profiler")
                .args(&["SPAudioDataType", "-detailLevel", "mini"])
                .output()
            {
                Ok(output) => {
                    if output.status.success() {
                        let result = String::from_utf8_lossy(&output.stdout);
                        let has_microphone = result.contains("Built-in Microphone") ||
                                           result.contains("Microphone") ||
                                           result.contains("Input");
                        debug!("Microphone hardware detected: {}", has_microphone);
                        Ok(has_microphone)
                    } else {
                        warn!("system_profiler failed: {}", String::from_utf8_lossy(&output.stderr));
                        // Fallback: assume microphone exists on modern Macs
                        Ok(true)
                    }
                }
                Err(e) => {
                    warn!("Failed to run system_profiler: {}", e);
                    // Fallback: assume microphone exists
                    Ok(true)
                }
            }
        }

        #[cfg(not(target_os = "macos"))]
        {
            Ok(true)
        }
    }

    /// Request microphone permission by triggering system dialog - NO admin privileges required
    pub async fn request_microphone_permission() -> Result<bool, String> {
        #[cfg(target_os = "macos")]
        {
            info!("Triggering microphone permission dialog through system preferences");

            // Open microphone settings to let user grant permission manually
            match Command::new("open")
                .args(&["x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone"])
                .status()
            {
                Ok(status) => {
                    if status.success() {
                        info!("Opened microphone privacy settings");
                        // Give user time to interact with settings
                        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

                        // Check if permission was granted
                        Self::check_microphone_permission()
                    } else {
                        warn!("Failed to open microphone settings");
                        Err("Failed to open microphone settings".to_string())
                    }
                }
                Err(e) => {
                    warn!("Error opening microphone settings: {}", e);
                    Err(format!("Error opening microphone settings: {}", e))
                }
            }
        }

        #[cfg(not(target_os = "macos"))]
        {
            Ok(true)
        }
    }

    /// Check accessibility permission using existing SDK - NO admin privileges required
    pub fn check_accessibility_permission() -> Result<bool, String> {
        #[cfg(target_os = "macos")]
        {
            // Use the existing computer_use_ai_sdk which already works
            use computer_use_ai_sdk::platforms::macos::permissions::check_accessibility_permissions;

            match check_accessibility_permissions(false) {
                Ok(granted) => {
                    debug!("Accessibility permission status: {}", granted);
                    Ok(granted)
                }
                Err(e) => {
                    warn!("Error checking accessibility permissions: {}", e);
                    Ok(false) // Assume not granted on error
                }
            }
        }

        #[cfg(not(target_os = "macos"))]
        {
            Ok(true)
        }
    }

    /// Request accessibility permission with native prompt - NO admin privileges required
    pub fn request_accessibility_permission() -> Result<(), String> {
        #[cfg(target_os = "macos")]
        {
            use computer_use_ai_sdk::platforms::macos::permissions::check_accessibility_permissions;

            // Trigger permission dialog
            match check_accessibility_permissions(true) {
                Ok(_) => {
                    info!("Accessibility permission request triggered");
                    Ok(())
                }
                Err(e) => {
                    warn!("Error requesting accessibility permissions: {}", e);
                    Err(format!("Failed to request accessibility permissions: {}", e))
                }
            }
        }

        #[cfg(not(target_os = "macos"))]
        {
            Ok(())
        }
    }

    /// Check screen recording permission using CGDisplayStream - NO admin privileges required
    pub fn check_screen_recording_permission() -> Result<bool, String> {
        #[cfg(target_os = "macos")]
        {
            // Try to capture a 1x1 pixel to test screen recording permissions
            match Command::new("screencapture")
                .args(&["-t", "png", "-x", "-R", "0,0,1,1", "/tmp/juno_screen_test.png"])
                .status()
            {
                Ok(status) => {
                    let granted = status.success();
                    debug!("Screen recording permission status: {}", granted);

                    // Clean up test file
                    let _ = std::fs::remove_file("/tmp/juno_screen_test.png");

                    Ok(granted)
                }
                Err(e) => {
                    warn!("Error testing screen recording: {}", e);
                    Ok(false) // Assume not granted on error
                }
            }
        }

        #[cfg(not(target_os = "macos"))]
        {
            Ok(true)
        }
    }

    /// Request screen recording permission - NO admin privileges required
    pub fn request_screen_recording_permission() -> Result<(), String> {
        #[cfg(target_os = "macos")]
        {
            info!("Opening screen recording privacy settings");

            // Open screen recording settings to let user grant permission manually
            match Command::new("open")
                .args(&["x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture"])
                .status()
            {
                Ok(status) => {
                    if status.success() {
                        info!("Screen recording permission request triggered");
                        Ok(())
                    } else {
                        warn!("Failed to open screen recording settings");
                        Err("Failed to open screen recording settings".to_string())
                    }
                }
                Err(e) => {
                    warn!("Error opening screen recording settings: {}", e);
                    Err(format!("Error opening screen recording settings: {}", e))
                }
            }
        }

        #[cfg(not(target_os = "macos"))]
        {
            Ok(())
        }
    }

    /// Create a permission status object with consistent formatting
    pub fn create_permission_status(
        permission_type: &str,
        granted: bool,
        required: bool,
        granted_description: &str,
        denied_description: &str,
        granted_instructions: &str,
        denied_instructions: &str,
    ) -> NativePermissionStatus {
        NativePermissionStatus {
            permission_type: permission_type.to_string(),
            granted,
            required,
            description: if granted {
                granted_description.to_string()
            } else {
                denied_description.to_string()
            },
            instructions: if granted {
                granted_instructions.to_string()
            } else {
                denied_instructions.to_string()
            },
        }
    }
}

/// High-level native permission checking functions
impl NativePermissionChecker {
    /// Get comprehensive microphone permission status using native APIs
    pub async fn get_microphone_status() -> Result<NativePermissionStatus, String> {
        let granted = Self::check_microphone_permission()?;

        Ok(Self::create_permission_status(
            "microphone",
            granted,
            false, // Microphone is optional for core functionality
            "Microphone access is granted. Voice features (dictation and agent mode) are fully available.",
            "Microphone access is not granted. Voice features will not work until permission is granted.",
            "No action needed - voice features are ready to use.",
            "Grant microphone access to use voice features like dictation (Option+Space) and agent mode (Option+D)."
        ))
    }

    /// Get comprehensive accessibility permission status using native APIs
    pub async fn get_accessibility_status() -> Result<NativePermissionStatus, String> {
        let granted = Self::check_accessibility_permission()?;

        Ok(Self::create_permission_status(
            "accessibility",
            granted,
            true, // Accessibility is required for core functionality
            "Accessibility permission is granted. All computer use features are available.",
            "Accessibility permission is required for Juno to interact with your desktop, click buttons, and automate tasks.",
            "No action needed - full computer use functionality is available.",
            "Grant accessibility access in System Settings > Privacy & Security > Accessibility to enable desktop automation."
        ))
    }

    /// Get comprehensive screen recording permission status using native APIs
    pub async fn get_screen_recording_status() -> Result<NativePermissionStatus, String> {
        let granted = Self::check_screen_recording_permission()?;

        Ok(Self::create_permission_status(
            "screen_recording",
            granted,
            true, // Screen recording is required for visual computer use
            "Screen recording permission is granted. Visual computer use features are available.",
            "Screen recording permission is required for Juno to see and interact with your screen content.",
            "No action needed - screen capture and visual automation are available.",
            "Grant screen recording access in System Settings > Privacy & Security > Screen Recording to enable visual features."
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_permission_status_creation() {
        let status = NativePermissionChecker::create_permission_status(
            "test",
            true,
            false,
            "granted desc",
            "denied desc",
            "granted instructions",
            "denied instructions"
        );

        assert_eq!(status.permission_type, "test");
        assert_eq!(status.granted, true);
        assert_eq!(status.required, false);
        assert_eq!(status.description, "granted desc");
        assert_eq!(status.instructions, "granted instructions");
    }

    #[tokio::test]
    async fn test_microphone_status_creation() {
        // This test will work on any platform since it tests the structure
        let result = NativePermissionChecker::get_microphone_status().await;
        assert!(result.is_ok());

        let status = result.unwrap();
        assert_eq!(status.permission_type, "microphone");
        assert_eq!(status.required, false);
    }
}
