use crate::platforms::macos::ffi::{
    AXIsProcessTrustedWithOptions, CGPreflightScreenCaptureAccess, CGRequestScreenCaptureAccess,
}; // Import from ffi module
use crate::AutomationError;
use core_foundation::base::TCFType;
use core_foundation::boolean::CFBoolean;
use core_foundation::dictionary::CFDictionary;
use core_foundation::string::CFString;
#[cfg(test)]
use once_cell::sync::Lazy;
use std::process::Command;
#[cfg(test)]
use std::sync::Mutex;
use tracing::{debug, info, warn};

#[cfg(test)]
static SCREEN_RECORDING_CHECK_OVERRIDE: Lazy<Mutex<Option<Box<dyn Fn() -> bool + Send + Sync>>>> =
    Lazy::new(|| Mutex::new(None));
#[cfg(test)]
static SCREEN_RECORDING_REQUEST_OVERRIDE: Lazy<Mutex<Option<Box<dyn Fn() -> bool + Send + Sync>>>> =
    Lazy::new(|| Mutex::new(None));

// Make the function public so it can be called from server.rs
pub fn check_accessibility_permissions(show_prompt: bool) -> Result<bool, AutomationError> {
    debug!("checking accessibility permissions");

    unsafe {
        // Create the options dictionary more safely
        let key = CFString::new("AXTrustedCheckOptionPrompt");
        let value = if show_prompt {
            CFBoolean::true_value()
        } else {
            CFBoolean::false_value()
        };

        // Create dictionary with proper memory management
        let options = CFDictionary::from_CFType_pairs(&[(key.as_CFType(), value.as_CFType())]);

        // Call the function with proper type conversion
        let is_trusted = AXIsProcessTrustedWithOptions(options.as_concrete_TypeRef());

        if is_trusted {
            debug!("accessibility permissions are granted");
            Ok(true)
        } else if !show_prompt {
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

/// Enhanced permission checking that automatically opens system settings when denied
pub fn check_accessibility_permissions_with_auto_redirect(
    show_prompt: bool,
    auto_open_settings: bool,
) -> Result<bool, AutomationError> {
    debug!(
        "checking accessibility permissions with auto-redirect option: {}",
        auto_open_settings
    );

    unsafe {
        // Create the options dictionary more safely
        let key = CFString::new("AXTrustedCheckOptionPrompt");
        let value = if show_prompt {
            CFBoolean::true_value()
        } else {
            CFBoolean::false_value()
        };

        // Create dictionary with proper memory management
        let options = CFDictionary::from_CFType_pairs(&[(key.as_CFType(), value.as_CFType())]);

        // Call the function with proper type conversion
        let is_trusted = AXIsProcessTrustedWithOptions(options.as_concrete_TypeRef());

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
}

/// Open macOS System Settings directly to the Accessibility privacy section
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
            info!(
                "Successfully opened System Settings for accessibility permissions using URL: {}",
                url
            );
            return Ok(());
        } else {
            debug!(
                "Failed to open with URL {}: {}",
                url,
                String::from_utf8_lossy(&output.stderr)
            );
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
        Err(format!(
            "Failed to open System Settings: {}",
            String::from_utf8_lossy(&fallback_output.stderr)
        ))
    }
}

/// Open macOS System Settings for a specific permission type
pub fn open_system_settings_for_permission(permission_type: &str) -> Result<(), String> {
    debug!(
        "Opening System Settings for permission type: {}",
        permission_type
    );

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
            info!(
                "Successfully opened System Settings for {} permissions using URL: {}",
                permission_type, url
            );
            return Ok(());
        } else {
            debug!(
                "Failed to open with URL {}: {}",
                url,
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }

    // Fallback to general Privacy & Security settings
    let fallback_output = Command::new("open")
        .arg("x-apple.systempreferences:com.apple.preference.security")
        .output()
        .map_err(|e| format!("Failed to execute fallback open command: {}", e))?;

    if fallback_output.status.success() {
        info!(
            "Opened general Privacy & Security settings as fallback for {}",
            permission_type
        );
        Ok(())
    } else {
        Err(format!(
            "Failed to open System Settings for {}: {}",
            permission_type,
            String::from_utf8_lossy(&fallback_output.stderr)
        ))
    }
}

/// Check screen recording permission using native CGPreflightScreenCaptureAccess API
/// This is a lightweight check that doesn't require creating a Desktop instance
pub fn check_screen_recording_permission() -> bool {
    debug!("checking screen recording permission using native API");

    let has_permission = preflight_screen_capture_access();

    if has_permission {
        debug!("screen recording permission is granted");
    } else {
        debug!("screen recording permission is not granted");
    }

    has_permission
}

/// Request screen recording permission using native CGRequestScreenCaptureAccess API
/// This may trigger a system dialog prompting the user to grant permission
pub fn request_screen_recording_permission() -> bool {
    debug!("requesting screen recording permission using native API");

    let granted = request_screen_capture_access();

    if granted {
        info!("screen recording permission was granted");
    } else {
        debug!("screen recording permission was denied or user needs to grant in System Settings");
    }

    granted
}

fn preflight_screen_capture_access() -> bool {
    #[cfg(test)]
    {
        if let Ok(guard) = SCREEN_RECORDING_CHECK_OVERRIDE.lock() {
            if let Some(callback) = guard.as_ref() {
                return callback();
            }
        }
    }

    unsafe { CGPreflightScreenCaptureAccess() }
}

fn request_screen_capture_access() -> bool {
    #[cfg(test)]
    {
        if let Ok(guard) = SCREEN_RECORDING_REQUEST_OVERRIDE.lock() {
            if let Some(callback) = guard.as_ref() {
                return callback();
            }
        }
    }

    unsafe { CGRequestScreenCaptureAccess() }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set_check_override(value: bool) {
        let mut guard = super::SCREEN_RECORDING_CHECK_OVERRIDE
            .lock()
            .expect("lock poisoned");
        *guard = Some(Box::new(move || value));
    }

    fn clear_check_override() {
        if let Ok(mut guard) = super::SCREEN_RECORDING_CHECK_OVERRIDE.lock() {
            guard.take();
        }
    }

    fn set_request_override(value: bool) {
        let mut guard = super::SCREEN_RECORDING_REQUEST_OVERRIDE
            .lock()
            .expect("lock poisoned");
        *guard = Some(Box::new(move || value));
    }

    fn clear_request_override() {
        if let Ok(mut guard) = super::SCREEN_RECORDING_REQUEST_OVERRIDE.lock() {
            guard.take();
        }
    }

    #[test]
    fn check_screen_recording_permission_respects_override() {
        set_check_override(true);
        assert!(check_screen_recording_permission());

        set_check_override(false);
        assert!(!check_screen_recording_permission());

        clear_check_override();
    }

    #[test]
    fn request_screen_recording_permission_respects_override() {
        set_request_override(true);
        assert!(request_screen_recording_permission());

        set_request_override(false);
        assert!(!request_screen_recording_permission());

        clear_request_override();
    }
}
