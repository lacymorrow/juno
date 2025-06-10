// Cidre-based implementation for accessibility permissions
// This file demonstrates the migration from manual FFI to safe Cidre bindings

#[cfg(target_os = "macos")]
use cidre::{ax, cf};
use crate::AutomationError;
use tracing::{debug, info, warn};
use std::process::Command;

/// Check accessibility permissions using Cidre (safe implementation)
#[cfg(target_os = "macos")]
pub fn check_accessibility_permissions_cidre(show_prompt: bool) -> Result<bool, AutomationError> {
    debug!("checking accessibility permissions with Cidre");

    // Use Cidre's safe accessibility API
    let is_trusted = ax::is_process_trusted_with_options(
        &ax::TrustedCheckOptions::new().prompt(show_prompt)
    );

    if is_trusted {
        debug!("accessibility permissions are granted");
        Ok(true)
    } else {
        if !show_prompt {
            debug!("accessibility permissions not granted");
            Err(AutomationError::PermissionDenied(
                "Accessibility permissions not granted. Go to System Preferences > Security & Privacy > Privacy > Accessibility and add this application.".to_string(),
            ))
        } else {
            debug!("accessibility permissions prompt displayed");
            Ok(false)
        }
    }
}

/// Enhanced permission checking with auto-redirect using Cidre
#[cfg(target_os = "macos")]
pub fn check_accessibility_permissions_with_auto_redirect_cidre(
    show_prompt: bool, 
    auto_open_settings: bool
) -> Result<bool, AutomationError> {
    debug!("checking accessibility permissions with auto-redirect option using Cidre: {}", auto_open_settings);

    // Use Cidre's safe accessibility API
    let is_trusted = ax::is_process_trusted_with_options(
        &ax::TrustedCheckOptions::new().prompt(show_prompt)
    );

    if is_trusted {
        debug!("accessibility permissions are granted");
        Ok(true)
    } else {
        debug!("accessibility permissions not granted");

        if auto_open_settings {
            info!("Automatically opening System Settings for accessibility permissions");
            if let Err(e) = open_accessibility_settings() {
                warn!("Failed to open System Settings: {}", e);
            }
        }

        if !show_prompt {
            Err(AutomationError::PermissionDenied(
                "Accessibility permissions not granted. System Settings has been opened for you to grant permissions.".to_string(),
            ))
        } else {
            debug!("accessibility permissions prompt displayed");
            Ok(false)
        }
    }
}

/// Fallback implementation for non-macOS targets
#[cfg(not(target_os = "macos"))]
pub fn check_accessibility_permissions_cidre(_show_prompt: bool) -> Result<bool, AutomationError> {
    Err(AutomationError::PlatformError(
        "Accessibility permissions are only available on macOS".to_string()
    ))
}

/// Fallback implementation for non-macOS targets
#[cfg(not(target_os = "macos"))]
pub fn check_accessibility_permissions_with_auto_redirect_cidre(
    _show_prompt: bool, 
    _auto_open_settings: bool
) -> Result<bool, AutomationError> {
    Err(AutomationError::PlatformError(
        "Accessibility permissions are only available on macOS".to_string()
    ))
}

/// Open macOS System Settings directly to the Accessibility privacy section
/// (This function remains the same as it uses system commands, not FFI)
pub fn open_accessibility_settings() -> Result<(), String> {
    debug!("Opening System Settings for accessibility permissions");

    // Try modern macOS Sequoia URL scheme first, then fall back to older schemes
    let urls = [
        "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility",
        "x-apple.systemsettings:com.apple.preference.security?Privacy_Accessibility",
    ];

    for url in &urls {
        let output = Command::new("open")
            .arg(url)
            .output()
            .map_err(|e| format!("Failed to execute open command: {}", e))?;

        if output.status.success() {
            info!("Successfully opened System Settings for accessibility permissions using URL: {}", url);
            return Ok(());
        } else {
            debug!("Failed to open with URL {}: {}", url, String::from_utf8_lossy(&output.stderr));
        }
    }

    // If all URL schemes fail, try opening general Privacy & Security settings
    let fallback_output = Command::new("open")
        .arg("x-apple.systempreferences:com.apple.preference.security")
        .output()
        .map_err(|e| format!("Failed to execute fallback open command: {}", e))?;

    if fallback_output.status.success() {
        info!("Opened general Privacy & Security settings as fallback");
        Ok(())
    } else {
        Err(format!("Failed to open System Settings: {}", String::from_utf8_lossy(&fallback_output.stderr)))
    }
}

/// Open macOS System Settings for a specific permission type
/// (This function remains the same as it uses system commands, not FFI)
pub fn open_system_settings_for_permission(permission_type: &str) -> Result<(), String> {
    debug!("Opening System Settings for permission type: {}", permission_type);

    let urls = match permission_type {
        "accessibility" => vec![
            "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility",
            "x-apple.systemsettings:com.apple.preference.security?Privacy_Accessibility",
        ],
        "screen_recording" | "screen_capture" => vec![
            "x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture",
            "x-apple.systemsettings:com.apple.preference.security?Privacy_ScreenCapture",
        ],
        "microphone" => vec![
            "x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone",
            "x-apple.systemsettings:com.apple.preference.security?Privacy_Microphone",
        ],
        "input_monitoring" => vec![
            "x-apple.systempreferences:com.apple.preference.security?Privacy_ListenEvent",
            "x-apple.systemsettings:com.apple.preference.security?Privacy_ListenEvent",
        ],
        "full_disk_access" => vec![
            "x-apple.systempreferences:com.apple.preference.security?Privacy_AllFiles",
            "x-apple.systemsettings:com.apple.preference.security?Privacy_AllFiles",
        ],
        "camera" => vec![
            "x-apple.systempreferences:com.apple.preference.security?Privacy_Camera",
            "x-apple.systemsettings:com.apple.preference.security?Privacy_Camera",
        ],
        "automation" => vec![
            "x-apple.systempreferences:com.apple.preference.security?Privacy_Automation",
            "x-apple.systemsettings:com.apple.preference.security?Privacy_Automation",
        ],
        _ => {
            return Err(format!("Unknown permission type: {}", permission_type));
        }
    };

    for url in &urls {
        let output = Command::new("open")
            .arg(url)
            .output()
            .map_err(|e| format!("Failed to execute open command: {}", e))?;

        if output.status.success() {
            info!("Successfully opened System Settings for {} permissions using URL: {}", permission_type, url);
            return Ok(());
        } else {
            debug!("Failed to open with URL {}: {}", url, String::from_utf8_lossy(&output.stderr));
        }
    }

    // Fallback to general Privacy & Security settings
    let fallback_output = Command::new("open")
        .arg("x-apple.systempreferences:com.apple.preference.security")
        .output()
        .map_err(|e| format!("Failed to execute fallback open command: {}", e))?;

    if fallback_output.status.success() {
        info!("Opened general Privacy & Security settings as fallback for {}", permission_type);
        Ok(())
    } else {
        Err(format!("Failed to open System Settings for {}: {}", permission_type, String::from_utf8_lossy(&fallback_output.stderr)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(target_os = "macos")]
    fn test_accessibility_permissions_cidre() {
        // Test that the function doesn't panic and returns a valid result
        let result = check_accessibility_permissions_cidre(false);
        assert!(result.is_ok() || result.is_err()); // Either state is valid
    }

    #[test]
    #[cfg(not(target_os = "macos"))]
    fn test_accessibility_permissions_cidre_non_macos() {
        // Test that non-macOS targets return appropriate error
        let result = check_accessibility_permissions_cidre(false);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("only available on macOS"));
    }
}