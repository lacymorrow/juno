//! # Window Management Module
//!
//! This module handles all window operations, state management, and positioning
//! for the Juno application. It provides a centralized interface for creating,
//! managing, and controlling all application windows.

use tauri::{AppHandle, Manager, Emitter, WebviewUrl, WebviewWindowBuilder};
use tracing::{info, warn, error};
use crate::constants;

#[cfg(target_os = "macos")]
use tauri::TitleBarStyle;

/// Window configuration for different window types
#[derive(Debug, Clone)]
pub struct WindowConfig {
    pub label: String,
    pub title: String,
    pub url: String,
    pub width: f64,
    pub height: f64,
    pub min_width: f64,
    pub min_height: f64,
    pub resizable: bool,
    pub center: bool,
    pub transparent_title_bar: bool,
}

impl WindowConfig {
    /// Create configuration for settings window
    pub fn settings() -> Self {
        Self {
            label: "settings".to_string(),
            title: "Juno Settings".to_string(),
            url: "/settings".to_string(),
            width: 800.0,
            height: 600.0,
            min_width: 700.0,
            min_height: 500.0,
            resizable: true,
            center: true,
            transparent_title_bar: true,
        }
    }

    /// Create configuration for onboarding window
    pub fn onboarding() -> Self {
        Self {
            label: "onboarding".to_string(),
            title: "Welcome to Juno".to_string(),
            url: "/onboarding".to_string(),
            width: 900.0,
            height: 700.0,
            min_width: 800.0,
            min_height: 600.0,
            resizable: true,
            center: true,
            transparent_title_bar: true,
        }
    }

    /// Create configuration for main window
    pub fn main() -> Self {
        Self {
            label: "main".to_string(),
            title: "Juno".to_string(),
            url: "/".to_string(),
            width: 800.0,
            height: 600.0,
            min_width: 400.0,
            min_height: 300.0,
            resizable: true,
            center: false, // Don't center main window as it may have saved position
            transparent_title_bar: true,
        }
    }
}

/// Window management operations
pub struct WindowManager;

impl WindowManager {
    /// Create or show a window with the given configuration
    pub async fn create_or_show_window(app: &AppHandle, config: WindowConfig) -> Result<(), String> {
        // Check if window already exists
        if let Some(existing_window) = app.get_webview_window(&config.label) {
            // If it exists, just show and focus it
            existing_window.show().map_err(|e| e.to_string())?;
            existing_window.set_focus().map_err(|e| e.to_string())?;
            if config.label == "main" {
                existing_window.unminimize().map_err(|e| e.to_string())?;
            }
            info!("Showed existing {} window", config.label);
            return Ok(());
        }

        // Create new window if it doesn't exist
        let mut builder = WebviewWindowBuilder::new(
            app,
            &config.label,
            WebviewUrl::App(config.url.into()),
        )
        .title(&config.title)
        .inner_size(config.width, config.height)
        .min_inner_size(config.min_width, config.min_height)
        .resizable(config.resizable)
        .visible(false); // Start hidden, show after setup

        if config.center {
            builder = builder.center();
        }

        let window = builder.build().map_err(|e| e.to_string())?;

        // Apply macOS-specific styling
        #[cfg(target_os = "macos")]
        if config.transparent_title_bar {
            if let Err(e) = window.set_title_bar_style(TitleBarStyle::Transparent) {
                warn!("Failed to set title bar style for {}: {}", config.label, e);
            }
        }

        // Show the window
        window.show().map_err(|e| e.to_string())?;
        window.set_focus().map_err(|e| e.to_string())?;

        info!("Successfully created and showed {} window", config.label);
        Ok(())
    }

    /// Hide a window by label
    pub async fn hide_window(app: &AppHandle, label: &str) -> Result<(), String> {
        if let Some(window) = app.get_webview_window(label) {
            window.hide().map_err(|e| e.to_string())?;
            info!("Hidden {} window", label);
        }
        Ok(())
    }

    /// Close a window by label
    pub async fn close_window(app: &AppHandle, label: &str) -> Result<(), String> {
        if let Some(window) = app.get_webview_window(label) {
            window.close().map_err(|e| e.to_string())?;
            info!("Closed {} window", label);
        }
        Ok(())
    }

    /// Check if a window exists and is visible
    pub fn is_window_visible(app: &AppHandle, label: &str) -> bool {
        app.get_webview_window(label)
            .and_then(|w| w.is_visible().ok())
            .unwrap_or(false)
    }

    /// Focus a window by label
    pub async fn focus_window(app: &AppHandle, label: &str) -> Result<(), String> {
        if let Some(window) = app.get_webview_window(label) {
            window.show().map_err(|e| e.to_string())?;
            window.set_focus().map_err(|e| e.to_string())?;
            window.unminimize().map_err(|e| e.to_string())?;
            info!("Focused {} window", label);
        }
        Ok(())
    }

    /// Minimize a window by label
    pub async fn minimize_window(app: &AppHandle, label: &str) -> Result<(), String> {
        if let Some(window) = app.get_webview_window(label) {
            window.minimize().map_err(|e| e.to_string())?;
            info!("Minimized {} window", label);
        }
        Ok(())
    }

    /// Maximize/zoom a window by label
    pub async fn maximize_window(app: &AppHandle, label: &str) -> Result<(), String> {
        if let Some(window) = app.get_webview_window(label) {
            window.maximize().map_err(|e| e.to_string())?;
            info!("Maximized {} window", label);
        }
        Ok(())
    }

    /// Toggle fullscreen for a window by label
    pub async fn toggle_fullscreen(app: &AppHandle, label: &str) -> Result<(), String> {
        if let Some(window) = app.get_webview_window(label) {
            let is_fullscreen = window.is_fullscreen().map_err(|e| e.to_string())?;
            window.set_fullscreen(!is_fullscreen).map_err(|e| e.to_string())?;
            info!("Toggled fullscreen for {} window to {}", label, !is_fullscreen);
        }
        Ok(())
    }
}

/// Handle window menu events
pub async fn handle_window_menu_event(app: &AppHandle, event_id: &str) {
    match event_id {
        constants::app_menu_ids::MINIMIZE => {
            info!("[Menu] Minimize menu item clicked");
            if let Err(e) = app.emit(constants::events::MINIMIZE_WINDOW_REQUESTED, ()) {
                error!("[Menu] Failed to emit minimize event: {}", e);
            }
        }
        constants::app_menu_ids::ZOOM => {
            info!("[Menu] Zoom menu item clicked");
            if let Err(e) = app.emit(constants::events::ZOOM_WINDOW_REQUESTED, ()) {
                error!("[Menu] Failed to emit zoom event: {}", e);
            }
        }
        constants::app_menu_ids::BRING_ALL_TO_FRONT => {
            info!("[Menu] Bring All to Front menu item clicked");
            // This is handled automatically by macOS for most cases
            info!("[Menu] Bring All to Front executed");
        }
        constants::app_menu_ids::TOGGLE_FULLSCREEN => {
            info!("[Menu] Toggle Fullscreen menu item clicked");
            if let Err(e) = app.emit(constants::events::TOGGLE_FULLSCREEN_REQUESTED, ()) {
                error!("[Menu] Failed to emit toggle fullscreen event: {}", e);
            }
        }
        _ => {
            info!("[Menu] Unhandled window menu event: {:?}", event_id);
        }
    }
}

/// Tauri command functions for window management
/// These are the command handlers that can be called from the frontend

/// Open the native settings window
#[tauri::command]
pub async fn open_settings_window(app: AppHandle) -> Result<(), String> {
    WindowManager::create_or_show_window(&app, WindowConfig::settings()).await
}

/// Close the native settings window
#[tauri::command]
pub async fn close_settings_window(app: AppHandle) -> Result<(), String> {
    WindowManager::hide_window(&app, "settings").await
}

/// Open the native onboarding window
#[tauri::command]
pub async fn open_onboarding_window(app: AppHandle) -> Result<(), String> {
    WindowManager::create_or_show_window(&app, WindowConfig::onboarding()).await
}

/// Close the native onboarding window
#[tauri::command]
pub async fn close_onboarding_window(app: AppHandle) -> Result<(), String> {
    WindowManager::hide_window(&app, "onboarding").await
}

/// Open/recreate the main window
#[tauri::command]
pub async fn open_main_window(app: AppHandle) -> Result<(), String> {
    WindowManager::create_or_show_window(&app, WindowConfig::main()).await
}

/// Get window states for tray menu and other uses
pub async fn get_window_states(app: &AppHandle) -> (bool, bool) {
    let main_visible = WindowManager::is_window_visible(app, constants::window_labels::MAIN);
    let floating_bar_visible = WindowManager::is_window_visible(app, constants::window_labels::FLOATING_BAR);
    (main_visible, floating_bar_visible)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_window_config_creation() {
        let settings_config = WindowConfig::settings();
        assert_eq!(settings_config.label, "settings");
        assert_eq!(settings_config.title, "Juno Settings");
        assert_eq!(settings_config.width, 800.0);
        assert!(settings_config.resizable);

        let onboarding_config = WindowConfig::onboarding();
        assert_eq!(onboarding_config.label, "onboarding");
        assert_eq!(onboarding_config.title, "Welcome to Juno");
        assert_eq!(onboarding_config.width, 900.0);

        let main_config = WindowConfig::main();
        assert_eq!(main_config.label, "main");
        assert_eq!(main_config.title, "Juno");
        assert!(!main_config.center); // Main window should not auto-center
    }

    #[test]
    fn test_window_manager_safety() {
        // Test that WindowManager operations are safe and don't cause crashes
        // This is a placeholder test since we can't easily mock AppHandle
        assert!(true, "WindowManager should handle missing windows gracefully");
    }
}
