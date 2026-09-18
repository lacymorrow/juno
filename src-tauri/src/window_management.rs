//! # Window Management Module
//!
//! This module handles all window operations, state management, and positioning
//! for the Juno application. It provides a centralized interface for creating,
//! managing, and controlling all application windows.

use crate::constants::{self, ui::window_labels};
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};
use tracing::{error, info, warn};

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
    /// Whether to steal application focus when the window is shown.
    /// Must be false for all overlay windows (floating-bar, floating-panel,
    /// desktop-cursor-overlay) — setting focus calls [NSApp activateIgnoringOtherApps:YES]
    /// which yanks keyboard focus away from the user's active application.
    pub focus: bool,
}

impl WindowConfig {
    /// Create configuration for settings window from Tauri config
    pub fn settings() -> Self {
        Self {
            label: window_labels::SETTINGS.to_string(),
            title: "Juno Settings".to_string(),
            url: "/settings".to_string(),
            width: 700.0,
            height: 600.0,
            min_width: 420.0,
            min_height: 300.0,
            resizable: true,
            center: true,
            transparent_title_bar: true,
            focus: true,
        }
    }

    /// Create configuration for onboarding window from Tauri config
    pub fn onboarding() -> Self {
        Self {
            label: window_labels::ONBOARDING.to_string(),
            title: "Welcome to Juno".to_string(),
            url: "/onboarding".to_string(),
            width: 440.0,
            height: 700.0,
            min_width: 200.0,
            min_height: 300.0,
            resizable: false,
            center: true,
            transparent_title_bar: false,
            focus: true,
        }
    }

    /// Create configuration for main window from Tauri config
    pub fn main() -> Self {
        Self {
            label: window_labels::MAIN.to_string(),
            title: "Juno".to_string(),
            url: "/".to_string(),
            width: 600.0,
            height: 700.0,
            min_width: 400.0,
            min_height: 300.0,
            resizable: true,
            center: false,
            transparent_title_bar: true,
            focus: true,
        }
    }

    /// Create configuration for desktop cursor overlay window from Tauri config
    pub fn desktop_cursor_overlay() -> Self {
        Self {
            label: "desktop-cursor-overlay".to_string(),
            title: "Desktop Cursor Overlay".to_string(),
            url: "/desktop-cursor-overlay".to_string(),
            width: 1.0,
            height: 1.0,
            min_width: 1.0,
            min_height: 1.0,
            resizable: false,
            center: false,
            transparent_title_bar: true,
            focus: false, // Overlay must never steal focus from the user's active app
        }
    }
}

/// Put a window in front of the person and make Juno the app they are in.
///
/// Three calls, in this order, because macOS treats them as three separate
/// things and the order is load-bearing:
///
/// - Showing a window only orders it into Juno's own window list. It does not
///   make Juno the active application, so a window shown while the person is
///   in another app comes up behind that app.
/// - Only focusing activates the application, and it quietly does nothing when
///   the window is hidden or minimized. So it has to come last, after the
///   other two have made it eligible.
///
/// Unminimize, then show, then focus. Focusing before unminimizing is why a
/// minimized chat window could be shown and still not come back.
fn present_window(window: &tauri::WebviewWindow) -> Result<(), String> {
    window.unminimize().map_err(|e| e.to_string())?;
    window.show().map_err(|e| e.to_string())?;
    window.set_focus().map_err(|e| e.to_string())?;
    Ok(())
}

/// Which chat surface a show-or-hide request should act on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatSurface {
    /// The full-size chat window is on screen.
    MainWindow {
        /// Whether it is the window the person is actually looking at. A
        /// visible window that is not frontmost is the "I cannot get back to
        /// it" case, and it wants bringing forward, not putting away.
        frontmost: bool,
    },
    /// The bar is holding the conversation, either in its pane or collapsed to
    /// the idle pill. Which of those two it is, is the bar's own answer.
    BarPane,
}

/// Resolve which chat surface is active.
///
/// This reads the same fact `announce_main_window` broadcasts, the visibility
/// of the full-size window, so the tray and the bar cannot reach different
/// conclusions: Rust is the side that knows, and the bar is told. Everything
/// below the full-size window belongs to the bar, including whether its pane
/// is up, which is why that is one variant here and not two.
pub fn active_chat_surface(app: &AppHandle) -> ChatSurface {
    if !WindowManager::is_window_visible(app, window_labels::MAIN) {
        return ChatSurface::BarPane;
    }
    let frontmost = app
        .get_webview_window(window_labels::MAIN)
        .and_then(|window| window.is_focused().ok())
        .unwrap_or(false);
    ChatSurface::MainWindow { frontmost }
}

/// Show or hide a chat surface.
///
/// The single answer to "give me the chat back", used by the tray menu row and
/// by a click on the tray icon so those two cannot disagree about what is on
/// screen. It carries more weight than it sounds: Cmd+backtick only cycles
/// windows inside the frontmost app, so it can never reach Juno once focus has
/// left Juno, and with the Dock icon hidden Juno is not in Cmd+Tab either. The
/// menu bar is then the only way back, which means it has to be the dependable
/// one.
///
/// The surface is passed in rather than resolved here so the caller can decide
/// *when* to read it. The tray reads it as the pointer arrives, before AppKit
/// starts tracking a menu, because what counts is which window was in front of
/// the person when they reached for the menu bar.
pub async fn toggle_chat_surface(app: &AppHandle, surface: ChatSurface) {
    match surface {
        // In front of the person and asked about: put it away. This goes
        // through the same command the bar's own reopen uses, so the bar hears
        // the window has gone and can take the conversation back.
        ChatSurface::MainWindow { frontmost: true } => {
            if let Err(e) = close_main_window(app.clone()).await {
                warn!("Could not put the chat window away: {}", e);
            }
        }
        // On screen, but behind something else. Asking for the chat here means
        // asking to get back to it, so bring it forward rather than hiding a
        // window the person cannot currently see.
        ChatSurface::MainWindow { frontmost: false } => {
            if let Err(e) = open_main_window(app.clone()).await {
                warn!("Could not bring the chat window forward: {}", e);
            }
        }
        // The bar owns the conversation. Put the bar itself on screen if it is
        // not (shown, never focused: it is a panel, and focusing it would pull
        // the person out of the app they are in), then let the bar flip its own
        // pane. Going through the bar's toggle rather than deciding here is
        // what lets the idle pill become the pane, which is what someone with
        // no chat on screen is asking for.
        ChatSurface::BarPane => {
            if let Some(bar) = app.get_webview_window(window_labels::FLOATING_BAR) {
                if !bar.is_visible().unwrap_or(false) {
                    if let Err(e) = bar.show() {
                        warn!("Could not put the floating bar on screen: {}", e);
                    }
                }
            }
            if let Err(e) = app.emit(constants::events::bar::TOGGLE_PANE, ()) {
                warn!("Could not ask the bar to toggle its chat pane: {}", e);
            }
        }
    }
}

/// Window management operations
pub struct WindowManager;

impl WindowManager {
    /// Create or show a window with the given configuration
    pub async fn create_or_show_window(
        app: &AppHandle,
        config: WindowConfig,
    ) -> Result<(), String> {
        // Check if window already exists and is valid
        if let Some(existing_window) = app.get_webview_window(&config.label) {
            // Check if window is actually valid (not destroyed)
            match existing_window.is_visible() {
                Ok(_) => {
                    // Only steal app focus for windows that explicitly request it.
                    // Overlay windows (cursor overlay, floating panel) must not activate
                    // Juno: that would yank keyboard focus from the user's active app.
                    if config.focus {
                        present_window(&existing_window)?;
                    } else {
                        existing_window.show().map_err(|e| e.to_string())?;
                    }

                    info!("Showed existing {} window", config.label);
                    return Ok(());
                }
                Err(_) => {
                    // Window exists in registry but is invalid/destroyed, create new one
                    info!(
                        "Existing {} window is invalid, creating new one",
                        config.label
                    );
                }
            }
        }

        // Create new window if it doesn't exist
        let mut builder =
            WebviewWindowBuilder::new(app, &config.label, WebviewUrl::App(config.url.into()))
                .title(&config.title)
                .inner_size(config.width, config.height)
                .min_inner_size(config.min_width, config.min_height)
                .resizable(config.resizable)
                .visible(true);

        if config.center {
            builder = builder.center();
        }

        let window = builder.build().map_err(|e| {
            error!("Failed to build {} window: {}", config.label, e);
            e.to_string()
        })?;

        info!("Successfully built {} window", config.label);

        // Apply macOS-specific styling after window is created
        #[cfg(target_os = "macos")]
        if config.transparent_title_bar {
            if let Err(e) = window.set_title_bar_style(TitleBarStyle::Transparent) {
                warn!("Failed to set title bar style for {}: {}", config.label, e);
            }
        }

        // Only take focus for non-overlay windows
        if config.focus {
            if let Err(e) = window.set_focus() {
                warn!("Failed to set focus for {} window: {}", config.label, e);
            }
        }

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
            present_window(&window)?;
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
            window
                .set_fullscreen(!is_fullscreen)
                .map_err(|e| e.to_string())?;
            info!(
                "Toggled fullscreen for {} window to {}",
                label, !is_fullscreen
            );
        }
        Ok(())
    }
}

/// Handle window menu events
pub async fn handle_window_menu_event(app: &AppHandle, event_id: &str) {
    match event_id {
        constants::app_menu_ids::MINIMIZE => {
            info!("[Menu] Minimize menu item clicked");
            if let Err(e) = app.emit(constants::events::menu::MINIMIZE_WINDOW_REQUESTED, ()) {
                error!("[Menu] Failed to emit minimize event: {}", e);
            }
        }
        constants::app_menu_ids::ZOOM => {
            info!("[Menu] Zoom menu item clicked");
            if let Err(e) = app.emit(constants::events::menu::ZOOM_WINDOW_REQUESTED, ()) {
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
            if let Err(e) = app.emit(constants::events::menu::TOGGLE_FULLSCREEN_REQUESTED, ()) {
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
///
/// Apply native macOS sidebar vibrancy to the settings window so its
/// translucent sidebar blurs the desktop behind it, like System Settings.
///
/// This is best-effort: any failure is logged and ignored so the window still
/// opens normally (e.g. on non-macOS builds this is compiled out entirely).
#[cfg(target_os = "macos")]
fn apply_settings_vibrancy(app: &AppHandle) {
    // `apply_vibrancy` touches AppKit and must run on the main thread; this is
    // called from an async command (a worker thread), so hop over explicitly.
    // Without this the call fails with "can only be used on the main thread"
    // and the sidebar renders as bare alpha instead of a frosted blur.
    let app = app.clone();
    let _ = app.clone().run_on_main_thread(move || {
        use window_vibrancy::{apply_vibrancy, NSVisualEffectMaterial};

        if let Some(window) = app.get_webview_window(window_labels::SETTINGS) {
            if let Err(e) = apply_vibrancy(&window, NSVisualEffectMaterial::Sidebar, None, None) {
                warn!("Failed to apply settings window vibrancy: {}", e);
            }
        }
    });
}

/// Open the native settings window
#[tauri::command]
pub async fn open_settings_window(app: AppHandle) -> Result<(), String> {
    WindowManager::create_or_show_window(&app, WindowConfig::settings()).await?;

    // Give the settings window the native translucent-sidebar look. Applied
    // after the window exists/shows; idempotent across repeated opens.
    #[cfg(target_os = "macos")]
    apply_settings_vibrancy(&app);

    Ok(())
}

/// Close the native settings window
#[tauri::command]
pub async fn close_settings_window(app: AppHandle) -> Result<(), String> {
    WindowManager::hide_window(&app, window_labels::SETTINGS).await
}

/// Open the native onboarding window.
///
/// The floating bar is always on top, so during setup it sits over the
/// onboarding window and covers the copy. It also has nothing to offer someone
/// who has not finished setting up, so it waits until they have.
#[tauri::command]
pub async fn open_onboarding_window(app: AppHandle) -> Result<(), String> {
    let bar_was_visible = WindowManager::is_window_visible(&app, window_labels::FLOATING_BAR);
    if bar_was_visible {
        mark_bar_withheld_for_onboarding();
        if let Err(e) = WindowManager::hide_window(&app, window_labels::FLOATING_BAR).await {
            // Not worth failing setup over; the bar merely sits in the way.
            warn!("Could not hide the floating bar for onboarding: {}", e);
        }
    }
    WindowManager::create_or_show_window(&app, WindowConfig::onboarding()).await
}

/// Close the native onboarding window, putting the bar back if we took it away.
#[tauri::command]
pub async fn close_onboarding_window(app: AppHandle) -> Result<(), String> {
    let result = WindowManager::close_window(&app, window_labels::ONBOARDING).await;
    if BAR_HIDDEN_FOR_ONBOARDING.swap(false, std::sync::atomic::Ordering::SeqCst) {
        if let Some(bar) = app.get_webview_window(window_labels::FLOATING_BAR) {
            if let Err(e) = bar.show() {
                warn!("Could not restore the floating bar after onboarding: {}", e);
            }
        }
    }
    result
}

/// Whether onboarding is holding the floating bar back, so closing it only
/// restores a bar that was actually going to be there.
static BAR_HIDDEN_FOR_ONBOARDING: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// True while the setup assistant is on screen.
pub fn onboarding_is_open(app: &AppHandle) -> bool {
    WindowManager::is_window_visible(app, window_labels::ONBOARDING)
}

/// Record that the bar is being held back for onboarding, so it is put on
/// screen when setup finishes.
///
/// The bar is shown on a short timer at startup, which can land either side of
/// onboarding opening. Hiding it on open alone loses that race: the timer fires
/// afterwards and puts it straight back over the setup window. So the show path
/// checks too, and both routes mark it withheld.
pub fn mark_bar_withheld_for_onboarding() {
    BAR_HIDDEN_FOR_ONBOARDING.store(true, std::sync::atomic::Ordering::SeqCst);
}

/// Open the full-size chat window, and get the bar out of its way.
///
/// The bar and this window show the same conversation, so having both up is
/// the same thing twice. The bar collapses to its idle pill rather than
/// hiding, which keeps the hotkey and the mic reachable while the big window
/// is up.
#[tauri::command]
pub async fn open_main_window(app: AppHandle) -> Result<(), String> {
    WindowManager::create_or_show_window(&app, WindowConfig::main()).await?;
    announce_main_window(&app, true);
    Ok(())
}

/// Put the full-size chat window away and give the conversation back to the bar.
///
/// Hides rather than closes: the webview keeps its React state, its scroll
/// position, and the audio element that plays TTS for the whole app.
#[tauri::command]
pub async fn close_main_window(app: AppHandle) -> Result<(), String> {
    WindowManager::hide_window(&app, window_labels::MAIN).await?;
    announce_main_window(&app, false);
    Ok(())
}

/// Tell the bar what the full-size window is doing.
///
/// Every route that shows the main window goes through here (the bar's button,
/// a Dock click, the tray, the reopen handler), because coordination that lives
/// in only one of them leaves the other three showing two copies of the same
/// conversation.
pub fn announce_main_window(app: &AppHandle, open: bool) {
    if open {
        // The bar already knows how to put its pane away.
        if let Err(e) = app.emit(constants::events::bar::DISMISS_PANE, ()) {
            warn!("Could not tell the bar to dismiss its pane: {}", e);
        }
    }
    let event = if open {
        constants::events::bar::MAIN_WINDOW_OPENED
    } else {
        constants::events::bar::MAIN_WINDOW_CLOSED
    };
    if let Err(e) = app.emit(event, ()) {
        warn!("Could not announce the main window state: {}", e);
    }
}

/// Open the desktop cursor overlay window
#[tauri::command]
pub async fn open_desktop_cursor_overlay(app: AppHandle) -> Result<(), String> {
    WindowManager::create_or_show_window(&app, WindowConfig::desktop_cursor_overlay()).await
}

/// Get window states for tray menu and other uses
pub async fn get_window_states(app: &AppHandle) -> (bool, bool) {
    let main_visible = WindowManager::is_window_visible(app, constants::window_labels::MAIN);
    let floating_bar_visible =
        WindowManager::is_window_visible(app, constants::window_labels::FLOATING_BAR);
    (main_visible, floating_bar_visible)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_window_config_creation() {
        let settings_config = WindowConfig::settings();
        assert_eq!(settings_config.label, window_labels::SETTINGS);
        assert_eq!(settings_config.title, "Juno Settings");
        assert_eq!(settings_config.width, 700.0);
        assert!(settings_config.resizable);

        let onboarding_config = WindowConfig::onboarding();
        assert_eq!(onboarding_config.label, window_labels::ONBOARDING);
        assert_eq!(onboarding_config.title, "Welcome to Juno");
        assert_eq!(onboarding_config.width, 440.0); // Match tauri.conf.json
        assert!(!onboarding_config.resizable); // Match tauri.conf.json

        let main_config = WindowConfig::main();
        assert_eq!(main_config.label, window_labels::MAIN);
        assert_eq!(main_config.title, "Juno");
        assert!(!main_config.center); // Main window should not auto-center
    }
}
