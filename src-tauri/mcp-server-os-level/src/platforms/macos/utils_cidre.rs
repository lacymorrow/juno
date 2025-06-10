// Cidre-based implementation for NSWorkspace utilities
// This file demonstrates the migration from manual Objective-C msg_send! to safe Cidre bindings

#[cfg(target_os = "macos")]
use cidre::{ns, cg, cf};
#[cfg(target_os = "macos")]
use cidre::ns::{ApplicationActivationPolicy};
use crate::AutomationError;
use tracing::{debug, warn};

/// Get PIDs of running applications using Cidre (safe implementation)
#[cfg(target_os = "macos")]
pub fn get_running_application_pids_cidre(use_background_apps: bool) -> Result<Vec<i32>, AutomationError> {
    debug!("Getting running application PIDs using Cidre");

    // Use Cidre's safe NSWorkspace API
    let workspace = ns::Workspace::shared();
    let apps = workspace.running_applications();

    let mut pids = Vec::new();
    
    for app in apps.iter() {
        // Filter apps by activation policy if requested
        if !use_background_apps {
            let activation_policy = app.activation_policy();
            match activation_policy {
                ApplicationActivationPolicy::Prohibited | ApplicationActivationPolicy::Accessory => {
                    continue; // Skip background/accessory apps
                }
                ApplicationActivationPolicy::Regular => {
                    // Include regular apps
                }
            }
        }

        // Filter out common background workers by bundle identifier
        if let Some(bundle_id) = app.bundle_identifier() {
            let bundle_id_str = bundle_id.to_string();
            
            // Skip common background processes and workers
            if bundle_id_str.contains(".worker")
                || bundle_id_str.contains("com.apple.WebKit")
                || bundle_id_str.contains("com.apple.CoreServices")
                || bundle_id_str.contains(".helper")
                || bundle_id_str.contains(".agent")
            {
                debug!("Filtered out background worker: {}", bundle_id_str);
                continue;
            }
        }

        let pid = app.process_identifier();
        pids.push(pid);
    }

    debug!("Found {} application PIDs using Cidre", pids.len());
    Ok(pids)
}

/// Get information about the frontmost application using Cidre
#[cfg(target_os = "macos")]
pub fn get_frontmost_application_cidre() -> Result<(i32, String, String), AutomationError> {
    debug!("Getting frontmost application using Cidre");

    let workspace = ns::Workspace::shared();
    
    if let Some(frontmost_app) = workspace.frontmost_application() {
        let pid = frontmost_app.process_identifier();
        
        let app_name = frontmost_app.localized_name()
            .map(|name| name.to_string())
            .unwrap_or_else(|| "Unknown".to_string());
            
        let bundle_id = frontmost_app.bundle_identifier()
            .map(|id| id.to_string())
            .unwrap_or_else(|| "unknown.bundle.id".to_string());

        debug!("Frontmost app: {} (PID: {}, Bundle: {})", app_name, pid, bundle_id);
        Ok((pid, app_name, bundle_id))
    } else {
        Err(AutomationError::PlatformError(
            "Could not determine frontmost application".to_string()
        ))
    }
}

/// Get application info by PID using Cidre
#[cfg(target_os = "macos")]
pub fn get_application_info_by_pid_cidre(pid: i32) -> Result<(String, String), AutomationError> {
    debug!("Getting application info for PID {} using Cidre", pid);

    let workspace = ns::Workspace::shared();
    let apps = workspace.running_applications();

    for app in apps.iter() {
        if app.process_identifier() == pid {
            let app_name = app.localized_name()
                .map(|name| name.to_string())
                .unwrap_or_else(|| "Unknown".to_string());
                
            let bundle_id = app.bundle_identifier()
                .map(|id| id.to_string())
                .unwrap_or_else(|| "unknown.bundle.id".to_string());

            debug!("Found app: {} (Bundle: {})", app_name, bundle_id);
            return Ok((app_name, bundle_id));
        }
    }

    Err(AutomationError::PlatformError(
        format!("No application found with PID {}", pid)
    ))
}

/// Launch application by bundle identifier using Cidre
#[cfg(target_os = "macos")]
pub fn launch_application_cidre(bundle_id: &str) -> Result<(), AutomationError> {
    debug!("Launching application with bundle ID: {}", bundle_id);

    let workspace = ns::Workspace::shared();
    let bundle_id_str = cf::String::from_str(bundle_id);
    
    let success = workspace.launch_application_at_url_options_configuration_error(
        &bundle_id_str,
        None, // No specific URL
        ns::WorkspaceLaunchOptions::empty(), // Default launch options
        None, // No configuration
    );

    if success.is_ok() {
        debug!("Successfully launched application: {}", bundle_id);
        Ok(())
    } else {
        Err(AutomationError::PlatformError(
            format!("Failed to launch application: {}", bundle_id)
        ))
    }
}

/// Hide application by PID using Cidre
#[cfg(target_os = "macos")]
pub fn hide_application_cidre(pid: i32) -> Result<(), AutomationError> {
    debug!("Hiding application with PID: {}", pid);

    let workspace = ns::Workspace::shared();
    let apps = workspace.running_applications();

    for app in apps.iter() {
        if app.process_identifier() == pid {
            app.hide();
            debug!("Successfully hid application with PID: {}", pid);
            return Ok(());
        }
    }

    Err(AutomationError::PlatformError(
        format!("No application found with PID {}", pid)
    ))
}

/// Activate application by PID using Cidre
#[cfg(target_os = "macos")]
pub fn activate_application_cidre(pid: i32) -> Result<(), AutomationError> {
    debug!("Activating application with PID: {}", pid);

    let workspace = ns::Workspace::shared();
    let apps = workspace.running_applications();

    for app in apps.iter() {
        if app.process_identifier() == pid {
            let success = app.activate_with_options(ns::ApplicationActivationOptions::empty());
            if success {
                debug!("Successfully activated application with PID: {}", pid);
                return Ok(());
            } else {
                return Err(AutomationError::PlatformError(
                    format!("Failed to activate application with PID: {}", pid)
                ));
            }
        }
    }

    Err(AutomationError::PlatformError(
        format!("No application found with PID {}", pid)
    ))
}

/// Get display information using Cidre
#[cfg(target_os = "macos")]
pub fn get_display_bounds_cidre(display_id: Option<u32>) -> Result<(f64, f64, f64, f64), AutomationError> {
    debug!("Getting display bounds using Cidre");

    let display_id = display_id.unwrap_or_else(|| {
        cg::Display::main().id()
    });

    let display = cg::Display::from_id(display_id);
    let bounds = display.bounds();

    debug!("Display bounds: ({}, {}, {}, {})", bounds.origin.x, bounds.origin.y, bounds.size.width, bounds.size.height);
    Ok((bounds.origin.x, bounds.origin.y, bounds.size.width, bounds.size.height))
}

/// Find display containing point using Cidre
#[cfg(target_os = "macos")]
pub fn find_display_containing_point_cidre(x: f64, y: f64) -> Result<u32, AutomationError> {
    debug!("Finding display containing point ({}, {}) using Cidre", x, y);

    let point = cg::Point::new(x, y);
    let displays = cg::Display::active_displays()
        .map_err(|_| AutomationError::PlatformError("Failed to get active displays".to_string()))?;

    for display in displays.iter() {
        let bounds = display.bounds();
        let rect = cg::Rect::new(bounds.origin, bounds.size);
        
        if rect.contains(&point) {
            let display_id = display.id();
            debug!("Point ({}, {}) is on display {}", x, y, display_id);
            return Ok(display_id);
        }
    }

    // Fallback to main display
    let main_display = cg::Display::main();
    let display_id = main_display.id();
    debug!("Point ({}, {}) not found on any display, defaulting to main display {}", x, y, display_id);
    Ok(display_id)
}

/// Fallback implementations for non-macOS targets
#[cfg(not(target_os = "macos"))]
pub fn get_running_application_pids_cidre(_use_background_apps: bool) -> Result<Vec<i32>, AutomationError> {
    Err(AutomationError::PlatformError(
        "NSWorkspace functionality is only available on macOS".to_string()
    ))
}

#[cfg(not(target_os = "macos"))]
pub fn get_frontmost_application_cidre() -> Result<(i32, String, String), AutomationError> {
    Err(AutomationError::PlatformError(
        "NSWorkspace functionality is only available on macOS".to_string()
    ))
}

#[cfg(not(target_os = "macos"))]
pub fn get_application_info_by_pid_cidre(_pid: i32) -> Result<(String, String), AutomationError> {
    Err(AutomationError::PlatformError(
        "NSWorkspace functionality is only available on macOS".to_string()
    ))
}

#[cfg(not(target_os = "macos"))]
pub fn launch_application_cidre(_bundle_id: &str) -> Result<(), AutomationError> {
    Err(AutomationError::PlatformError(
        "NSWorkspace functionality is only available on macOS".to_string()
    ))
}

#[cfg(not(target_os = "macos"))]
pub fn hide_application_cidre(_pid: i32) -> Result<(), AutomationError> {
    Err(AutomationError::PlatformError(
        "NSWorkspace functionality is only available on macOS".to_string()
    ))
}

#[cfg(not(target_os = "macos"))]
pub fn activate_application_cidre(_pid: i32) -> Result<(), AutomationError> {
    Err(AutomationError::PlatformError(
        "NSWorkspace functionality is only available on macOS".to_string()
    ))
}

#[cfg(not(target_os = "macos"))]
pub fn get_display_bounds_cidre(_display_id: Option<u32>) -> Result<(f64, f64, f64, f64), AutomationError> {
    Err(AutomationError::PlatformError(
        "Core Graphics functionality is only available on macOS".to_string()
    ))
}

#[cfg(not(target_os = "macos"))]
pub fn find_display_containing_point_cidre(_x: f64, _y: f64) -> Result<u32, AutomationError> {
    Err(AutomationError::PlatformError(
        "Core Graphics functionality is only available on macOS".to_string()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(target_os = "macos")]
    fn test_get_running_application_pids_cidre() {
        // Test that the function doesn't panic and returns a valid result
        let result = get_running_application_pids_cidre(false);
        assert!(result.is_ok());
        let pids = result.unwrap();
        assert!(!pids.is_empty(), "Should find at least some running applications");
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn test_get_frontmost_application_cidre() {
        // Test that the function doesn't panic and returns a valid result
        let result = get_frontmost_application_cidre();
        assert!(result.is_ok());
        let (pid, name, bundle_id) = result.unwrap();
        assert!(pid > 0, "PID should be positive");
        assert!(!name.is_empty(), "App name should not be empty");
        assert!(!bundle_id.is_empty(), "Bundle ID should not be empty");
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn test_get_display_bounds_cidre() {
        // Test that the function doesn't panic and returns valid bounds
        let result = get_display_bounds_cidre(None);
        assert!(result.is_ok());
        let (x, y, width, height) = result.unwrap();
        assert!(width > 0.0 && height > 0.0, "Display should have positive dimensions");
    }

    #[test]
    #[cfg(not(target_os = "macos"))]
    fn test_functions_fail_on_non_macos() {
        // Test that all functions return appropriate errors on non-macOS platforms
        assert!(get_running_application_pids_cidre(false).is_err());
        assert!(get_frontmost_application_cidre().is_err());
        assert!(get_application_info_by_pid_cidre(1).is_err());
        assert!(launch_application_cidre("test").is_err());
        assert!(hide_application_cidre(1).is_err());
        assert!(activate_application_cidre(1).is_err());
        assert!(get_display_bounds_cidre(None).is_err());
        assert!(find_display_containing_point_cidre(0.0, 0.0).is_err());
    }
}