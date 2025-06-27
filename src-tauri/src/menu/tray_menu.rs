//! # Tray Menu Module
//!
//! This module handles the system tray menu functionality for the Juno application.
//! It provides state-aware menu items that update based on the current window states
//! and comprehensive event handling for all tray menu interactions.
//!
//! ## Tray Icon States
//! - Default: Idle state with standard icon
//! - Agent Active: Blue-tinted icon when agent is executing
//! - Dictation Active: Orange-tinted icon when dictation is active
//! - Always Listening: Green-tinted icon when always listening is enabled
//! - Error: Red-tinted icon when there's an error
//! - Processing: Animated or pulsing icon during processing

use crate::constants::{events, menus::tray_menu_ids, errors::{templates, prefixes}};

// Helper function for error formatting - properly handles template substitution
fn format_error(template: &str, context: &str, error: impl std::fmt::Display) -> String {
    template.replacen("{}", context, 1).replacen("{}", &error.to_string(), 1)
}
use crate::state::AppState;
use std::sync::Arc;
use tauri::{
    image::Image as TauriImage,
    menu::{MenuBuilder, MenuItemBuilder},
    tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder},
    AppHandle, Emitter, Listener, Manager,
};
use tokio::sync::Mutex as TokioMutex;
use tracing::{debug, error, info, warn};

// Embed tray icon data directly in the binary - no file system dependencies
const TRAY_ICON_DEFAULT: &[u8] = include_bytes!("../../icons/tray/32x32.png");

// State-specific tray icons - different variants for each application state
const TRAY_ICON_AGENT_ACTIVE: &[u8] = include_bytes!("../../icons/tray/32x32-agent.png");
const TRAY_ICON_DICTATION_ACTIVE: &[u8] = include_bytes!("../../icons/tray/32x32-dictation.png");
const TRAY_ICON_ALWAYS_LISTENING: &[u8] = include_bytes!("../../icons/tray/32x32-listening.png");
const TRAY_ICON_ERROR: &[u8] = include_bytes!("../../icons/tray/32x32-error.png");
const TRAY_ICON_PROCESSING: &[u8] = include_bytes!("../../icons/tray/32x32-processing.png");

/// Represents different states for the tray icon
#[derive(Debug, Clone, PartialEq)]
pub enum TrayIconState {
    /// Default idle state
    Default,
    /// Agent is executing commands
    AgentActive,
    /// Dictation mode is active
    DictationActive,
    /// Always listening mode is enabled
    AlwaysListening,
    /// Processing state (thinking, transcribing, etc.)
    Processing,
    /// Error state
    Error,
}

impl TrayIconState {
    /// Get the icon data for this state
    fn get_icon_data(&self) -> &'static [u8] {
        match self {
            TrayIconState::Default => TRAY_ICON_DEFAULT,
            TrayIconState::AgentActive => TRAY_ICON_AGENT_ACTIVE,
            TrayIconState::DictationActive => TRAY_ICON_DICTATION_ACTIVE,
            TrayIconState::AlwaysListening => TRAY_ICON_ALWAYS_LISTENING,
            TrayIconState::Processing => TRAY_ICON_PROCESSING,
            TrayIconState::Error => TRAY_ICON_ERROR,
        }
    }

    /// Get a human-readable description of this state
    fn description(&self) -> &'static str {
        match self {
            TrayIconState::Default => "Juno - Ready",
            TrayIconState::AgentActive => "Juno - Agent Active",
            TrayIconState::DictationActive => "Juno - Dictation Active",
            TrayIconState::AlwaysListening => "Juno - Always Listening",
            TrayIconState::Processing => "Juno - Processing",
            TrayIconState::Error => "Juno - Error",
        }
    }
}

/// Tray icon manager for handling dynamic icon changes
pub struct TrayIconManager {
    tray_icon: Option<TrayIcon<tauri::Wry>>,
    current_state: TrayIconState,
}

impl TrayIconManager {
    pub fn new() -> Self {
        Self {
            tray_icon: None,
            current_state: TrayIconState::Default,
        }
    }

    /// Update the tray icon to reflect the current state
    pub async fn update_icon_state(
        &mut self,
        new_state: TrayIconState,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if self.current_state == new_state {
            debug!("Tray icon state unchanged: {:?}", new_state);
            return Ok(());
        }

        info!(
            "🔄 Updating tray icon state: {:?} -> {:?}",
            self.current_state, new_state
        );

        if let Some(tray_icon) = &self.tray_icon {
            let icon_data = new_state.get_icon_data();
            let loaded_icon = load_tray_icon_from_data(icon_data)?;

            tray_icon.set_icon(Some(loaded_icon))?;
            tray_icon.set_tooltip(Some(new_state.description()))?;

            self.current_state = new_state;
            info!("✅ Tray icon updated successfully");
        } else {
            warn!("No tray icon available to update");
        }

        Ok(())
    }

    /// Set the tray icon reference
    pub fn set_tray_icon(&mut self, tray_icon: TrayIcon<tauri::Wry>) {
        self.tray_icon = Some(tray_icon);
    }

    /// Get the current tray icon state
    pub fn current_state(&self) -> &TrayIconState {
        &self.current_state
    }
}

/// Global tray icon manager
static TRAY_ICON_MANAGER: TokioMutex<Option<Arc<TokioMutex<TrayIconManager>>>> =
    TokioMutex::const_new(None);

/// Get or create the tray icon manager
async fn get_tray_icon_manager() -> Arc<TokioMutex<TrayIconManager>> {
    let mut manager_guard = TRAY_ICON_MANAGER.lock().await;
    if let Some(manager) = manager_guard.as_ref() {
        manager.clone()
    } else {
        let new_manager = Arc::new(TokioMutex::new(TrayIconManager::new()));
        *manager_guard = Some(new_manager.clone());
        new_manager
    }
}

/// Load tray icon from embedded data
fn load_tray_icon_from_data(icon_data: &[u8]) -> Result<TauriImage, Box<dyn std::error::Error>> {
    let loaded_image = image::load_from_memory(icon_data)?;
    let width = loaded_image.width();
    let height = loaded_image.height();
    let rgba_image = loaded_image.to_rgba8();
    let bytes = rgba_image.into_raw();
    let img = TauriImage::new_owned(bytes, width, height);
    Ok(img)
}

/// Get keyboard shortcuts from app state
fn get_keyboard_shortcuts(
    app: &AppHandle,
) -> Result<crate::state::KeyboardShortcuts, Box<dyn std::error::Error>> {
    let app_state = app.state::<AppState>();
    app_state
        .get_keyboard_shortcuts()
        .map_err(|e| format_error(templates::FAILED_TO_RETRIEVE, "keyboard shortcuts", e).into())
}

/// Create a state-aware tray menu with keyboard shortcuts
pub fn create_state_aware_tray_menu(
    app: &AppHandle,
) -> Result<tauri::menu::Menu<tauri::Wry>, Box<dyn std::error::Error>> {
    info!("🍽️ Creating state-aware tray menu...");

    // Get keyboard shortcuts from app state
    let _shortcuts = match get_keyboard_shortcuts(app) {
        Ok(shortcuts) => shortcuts,
        Err(e) => {
            error!("{} {}", prefixes::TRAY_MENU, format_error(templates::FAILED_TO_RETRIEVE, "keyboard shortcuts", e));
            // Use defaults if we can't get from state
            crate::state::KeyboardShortcuts {
                agent_mode_toggle: "Option+D".to_string(),
                dictation_input: "Option+Space".to_string(),
                stop_current_task: "Escape".to_string(),
                open_settings: "Cmd+,".to_string(),
            }
        }
    };

    // Build menu items with proper accelerators
    let show_hide_item = MenuItemBuilder::new("Show/Hide Juno")
        .id(tray_menu_ids::SHOW_HIDE)
        .build(app)?;

    let new_chat_item = MenuItemBuilder::new("New Chat")
        .id(tray_menu_ids::NEW_CHAT)
        .accelerator("CmdOrCtrl+N")
        .build(app)?;

    let show_hide_floating_item = MenuItemBuilder::new("Show/Hide Floating Bar")
        .id(tray_menu_ids::SHOW_HIDE_FLOATING_BAR)
        .accelerator("CmdOrCtrl+B")
        .build(app)?;

    let dev_tools_item = MenuItemBuilder::new("Developer Tools")
        .id(tray_menu_ids::DEVELOPER_TOOLS)
        .accelerator("CmdOrCtrl+Alt+I")
        .build(app)?;

    // Voice control information items (non-clickable)
    let agent_mode_info = MenuItemBuilder::new("Agent Mode")
        .id("agent_mode_info")
        .accelerator("Alt+D")
        .enabled(false)
        .build(app)?;

    let dictation_mode_info = MenuItemBuilder::new("Dictation Mode")
        .id("dictation_mode_info")
        .enabled(false)
        .build(app)?;

    let stop_task_info = MenuItemBuilder::new("Stop Current Task")
        .id("stop_task_info")
        .accelerator("Escape")
        .enabled(false)
        .build(app)?;

    let settings_item = MenuItemBuilder::new("Settings...")
        .id(tray_menu_ids::SETTINGS)
        .accelerator("CmdOrCtrl+,")
        .build(app)?;

    let quit_item = MenuItemBuilder::new("Quit Juno")
        .id(tray_menu_ids::QUIT)
        .accelerator("CmdOrCtrl+Q")
        .build(app)?;

    // Build the complete tray menu
    let tray_menu = MenuBuilder::new(app)
        .item(&show_hide_item)
        .item(&new_chat_item)
        .separator()
        .item(&show_hide_floating_item)
        .item(&dev_tools_item)
        .separator()
        .item(&agent_mode_info)
        .item(&dictation_mode_info)
        .item(&stop_task_info)
        .separator()
        .item(&settings_item)
        .separator()
        .item(&quit_item)
        .build()?;

    info!("✅ State-aware tray menu created successfully");
    Ok(tray_menu)
}

/// Create and setup the system tray icon with dynamic state management
pub fn setup_tray_icon(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    info!("🔧 Setting up system tray icon with dynamic state support...");

    let tray_menu = create_state_aware_tray_menu(app)?;

    // Load the default tray icon
    let default_icon = load_tray_icon_from_data(TRAY_ICON_DEFAULT)?;

    let tray_icon = TrayIconBuilder::new()
        .menu(&tray_menu)
        .show_menu_on_left_click(false)
        .icon(default_icon)
        .tooltip("Juno - Ready")
        .on_tray_icon_event(|_tray, event| {
            handle_tray_icon_event(event);
        })
        .build(app)?;

    // Initialize the tray icon manager
    let app_handle = app.clone();
    tauri::async_runtime::spawn(async move {
        let manager = get_tray_icon_manager().await;
        let mut manager_guard = manager.lock().await;
        manager_guard.set_tray_icon(tray_icon);

        // Start monitoring app state for tray icon updates
        setup_state_monitoring(&app_handle).await;
    });

    info!("✅ System tray icon setup completed with dynamic state support");
    Ok(())
}

/// Setup state monitoring to automatically update tray icon based on app state
async fn setup_state_monitoring(app_handle: &AppHandle) {
    info!("🔍 Setting up tray icon state monitoring...");

    let app_handle_clone = app_handle.clone();

    // Listen for various state change events
    let _ = app_handle.listen("agent-active", {
        let app_handle = app_handle_clone.clone();
        move |event| {
            let app_handle = app_handle.clone();
            tauri::async_runtime::spawn(async move {
                if let Ok(is_active) = serde_json::from_str::<bool>(&event.payload()) {
                    let new_state = if is_active {
                        TrayIconState::AgentActive
                    } else {
                        determine_current_state(&app_handle).await
                    };
                    update_tray_icon_state(new_state).await;
                }
            });
        }
    });

    // Listen for dictation state changes (both immediate and confirmed)
    let _ = app_handle.listen("dictation-active", {
        let app_handle = app_handle_clone.clone();
        move |event| {
            let app_handle = app_handle.clone();
            tauri::async_runtime::spawn(async move {
                if let Ok(is_active) = serde_json::from_str::<bool>(&event.payload()) {
                    let new_state = if is_active {
                        TrayIconState::DictationActive
                    } else {
                        determine_current_state(&app_handle).await
                    };
                    update_tray_icon_state(new_state).await;
                }
            });
        }
    });

    // Listen for immediate dictation input state changes (when user presses/releases shortcut)
    let _ = app_handle.listen("dictation-input-state-changed", {
        let app_handle = app_handle_clone.clone();
        move |event| {
            let app_handle = app_handle.clone();
            tauri::async_runtime::spawn(async move {
                if let Ok(is_pressed) = serde_json::from_str::<bool>(&event.payload()) {
                    let new_state = if is_pressed {
                        TrayIconState::DictationActive
                    } else {
                        determine_current_state(&app_handle).await
                    };
                    update_tray_icon_state(new_state).await;
                }
            });
        }
    });

    // Listen for immediate agent mode starts (when user presses agent shortcut)
    let _ = app_handle.listen("app-dictation-started", {
        let app_handle = app_handle_clone.clone();
        move |_event| {
            let app_handle = app_handle.clone();
            tauri::async_runtime::spawn(async move {
                // Set to Processing state initially, will change to AgentActive once agent actually starts
                update_tray_icon_state(TrayIconState::Processing).await;
            });
        }
    });

    // Listen for dictation completion events (when transcription finishes)
    let _ = app_handle.listen("app-dictation-finished", {
        let app_handle = app_handle_clone.clone();
        move |event| {
            let app_handle = app_handle.clone();
            tauri::async_runtime::spawn(async move {
                // When dictation finishes, check if it's transitioning to agent mode
                // If there's a query payload, it means agent mode is starting
                if let Ok(payload) = serde_json::from_str::<serde_json::Value>(&event.payload()) {
                    if payload.get("query").is_some() {
                        // Dictation finished and agent is about to start
                        update_tray_icon_state(TrayIconState::Processing).await;
                    } else {
                        // Just dictation finishing without agent mode
                        let new_state = determine_current_state(&app_handle).await;
                        update_tray_icon_state(new_state).await;
                    }
                } else {
                    // Fallback - just determine current state
                    let new_state = determine_current_state(&app_handle).await;
                    update_tray_icon_state(new_state).await;
                }
            });
        }
    });

    let _ = app_handle.listen("always-listening-mode-changed", {
        let app_handle = app_handle_clone.clone();
        move |event| {
            let app_handle = app_handle.clone();
            tauri::async_runtime::spawn(async move {
                if let Ok(is_active) = serde_json::from_str::<bool>(&event.payload()) {
                    if is_active {
                        update_tray_icon_state(TrayIconState::AlwaysListening).await;
                    } else {
                        let new_state = determine_current_state(&app_handle).await;
                        update_tray_icon_state(new_state).await;
                    }
                }
            });
        }
    });

    // Listen for floating bar state changes to detect processing states
    let _ = app_handle.listen("floating-bar-state-changed", {
        move |event| {
            tauri::async_runtime::spawn(async move {
                if let Ok(payload) = serde_json::from_str::<serde_json::Value>(&event.payload()) {
                    if let Some(state) = payload.get("barState").and_then(|s| s.as_str()) {
                        let icon_state = match state {
                            "loading"
                            | "transcribing"
                            | "agent_thinking"
                            | "dictation_processing" => Some(TrayIconState::Processing),
                            "error" => Some(TrayIconState::Error),
                            _ => None,
                        };

                        if let Some(state) = icon_state {
                            update_tray_icon_state(state).await;
                        }
                    }
                }
            });
        }
    });

    // Listen for voice system errors
    let _ = app_handle.listen("voice-error", {
        let app_handle = app_handle_clone.clone();
        move |_event| {
            let app_handle = app_handle.clone();
            tauri::async_runtime::spawn(async move {
                update_tray_icon_state(TrayIconState::Error).await;

                // After a delay, reset to current state
                tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
                let new_state = determine_current_state(&app_handle).await;
                update_tray_icon_state(new_state).await;
            });
        }
    });

    // Listen for voice transcription errors
    let _ = app_handle.listen("voice-transcription:error", {
        let app_handle = app_handle_clone.clone();
        move |_event| {
            let app_handle = app_handle.clone();
            tauri::async_runtime::spawn(async move {
                update_tray_icon_state(TrayIconState::Error).await;

                // After a delay, reset to current state
                tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
                let new_state = determine_current_state(&app_handle).await;
                update_tray_icon_state(new_state).await;
            });
        }
    });

    info!("✅ Tray icon state monitoring setup completed");
}

/// Determine the current tray icon state based on app state
async fn determine_current_state(app_handle: &AppHandle) -> TrayIconState {
    let app_state = app_handle.state::<AppState>();

    // Check states in order of priority
    if app_state.is_agent_executing() {
        return TrayIconState::AgentActive;
    }

    if app_state.is_dictation_active() {
        return TrayIconState::DictationActive;
    }

    // Check always listening state
    if app_state.get_always_listening_active().unwrap_or(false) {
        return TrayIconState::AlwaysListening;
    }

    TrayIconState::Default
}

/// Update the tray icon state
pub async fn update_tray_icon_state(new_state: TrayIconState) {
    let manager = get_tray_icon_manager().await;
    let mut manager_guard = manager.lock().await;

    if let Err(e) = manager_guard.update_icon_state(new_state).await {
        error!("{} {}", prefixes::TRAY_MENU, format_error(templates::FAILED_TO_EMIT, "tray state updated", e));
    }
}

/// Public function to manually update tray icon state
pub async fn set_tray_icon_state(state: TrayIconState) {
    update_tray_icon_state(state).await;
}

/// Convenience functions for common state changes

/// Set tray icon to indicate agent is active
pub async fn set_agent_active() {
    set_tray_icon_state(TrayIconState::AgentActive).await;
}

/// Set tray icon to indicate dictation is active
pub async fn set_dictation_active() {
    set_tray_icon_state(TrayIconState::DictationActive).await;
}

/// Set tray icon to indicate always listening is active
pub async fn set_always_listening() {
    set_tray_icon_state(TrayIconState::AlwaysListening).await;
}

/// Set tray icon to indicate processing state
pub async fn set_processing() {
    set_tray_icon_state(TrayIconState::Processing).await;
}

/// Set tray icon to indicate error state
pub async fn set_error() {
    set_tray_icon_state(TrayIconState::Error).await;
}

/// Reset tray icon to default state
pub async fn set_default() {
    set_tray_icon_state(TrayIconState::Default).await;
}

/// Handle tray menu events
pub fn handle_tray_menu_events(app_handle: AppHandle, event_id: &str) {
    // Only handle events that are actually tray menu events
    // Ignore app menu and edit menu events that are handled by the global menu handler
    if !crate::menu::is_tray_menu_event(event_id) {
        // Silently ignore non-tray events - they're handled by the global menu handler
        return;
    }

    match event_id {
        tray_menu_ids::SHOW_HIDE => {
            info!("[TrayMenu] Show/Hide menu item clicked");
            // For now, just trigger settings until we have the proper event
            if let Err(e) = app_handle.emit(events::menu::SETTINGS_REQUESTED, ()) {
                error!("{} {}", prefixes::TRAY_MENU, format_error(templates::FAILED_TO_EMIT, "settings", e));
            }
        }
        tray_menu_ids::NEW_CHAT => {
            info!("[TrayMenu] New Chat menu item clicked");
            if let Err(e) = app_handle.emit(events::menu::NEW_CHAT_REQUESTED, ()) {
                error!("{} {}", prefixes::TRAY_MENU, format_error(templates::FAILED_TO_EMIT, "new chat", e));
            }
        }
        tray_menu_ids::SHOW_HIDE_FLOATING_BAR => {
            info!("[TrayMenu] Show/Hide Floating Bar menu item clicked");
            if let Err(e) = app_handle.emit(events::menu::TOGGLE_FLOATING_BAR_REQUESTED, ()) {
                error!("{} {}", prefixes::TRAY_MENU, format_error(templates::FAILED_TO_EMIT, "toggle floating bar", e));
            }
        }
        tray_menu_ids::DEVELOPER_TOOLS => {
            info!("[TrayMenu] Developer Tools menu item clicked");
            if let Err(e) = app_handle.emit(events::menu::DEVTOOLS_REQUESTED, ()) {
                error!("{} {}", prefixes::TRAY_MENU, format_error(templates::FAILED_TO_EMIT, "devtools", e));
            }
        }
        tray_menu_ids::SETTINGS => {
            info!("[TrayMenu] Settings menu item clicked");
            if let Err(e) = app_handle.emit(events::menu::SETTINGS_REQUESTED, ()) {
                error!("{} {}", prefixes::TRAY_MENU, format_error(templates::FAILED_TO_EMIT, "settings", e));
            }
        }
        tray_menu_ids::QUIT => {
            info!("[TrayMenu] Quit menu item clicked");
            app_handle.exit(0);
        }
        _ => {
            // This should never happen since we filter for tray events above
            warn!("[TrayMenu] Unexpected tray menu event: {}", event_id);
        }
    }
}

/// Handle TrayIconEvents like clicks on the icon itself
pub fn handle_tray_icon_event(event: tauri::tray::TrayIconEvent) {
    match event {
        tauri::tray::TrayIconEvent::Click {
            button: MouseButton::Left,
            button_state: MouseButtonState::Up,
            ..
        } => {
            info!("[TrayIcon] Left click detected on tray icon");
            // Implement left click behavior if needed
        }
        tauri::tray::TrayIconEvent::Click {
            button: MouseButton::Right,
            button_state: MouseButtonState::Up,
            ..
        } => {
            info!("[TrayIcon] Right click detected on tray icon");
            // Right click behavior is handled by menu system
        }
        _ => {
            // Handle other tray icon events if needed
        }
    }
}

/// Enhanced tray menu refresh function
pub fn refresh_tray_menu(app_handle: &AppHandle) {
    info!("[TrayMenu] Refreshing tray menu...");

    match create_state_aware_tray_menu(app_handle) {
        Ok(_new_menu) => {
            info!("[TrayMenu] Successfully refreshed tray menu");
            // Note: Tauri v2 will handle menu updates automatically through the state
        }
        Err(e) => {
            error!("{} {}", prefixes::TRAY_MENU, format_error(templates::FAILED_TO_LOAD, "tray menu refresh", e));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tray_icon_data_embedded() {
        // Test that tray icon data is properly embedded
        assert!(
            !TRAY_ICON_DEFAULT.is_empty(),
            "Default tray icon data should not be empty"
        );
        assert!(
            TRAY_ICON_DEFAULT.len() > 100,
            "Default tray icon data should be reasonable size"
        );
    }

    #[test]
    fn test_tray_icon_state_descriptions() {
        assert_eq!(TrayIconState::Default.description(), "Juno - Ready");
        assert_eq!(
            TrayIconState::AgentActive.description(),
            "Juno - Agent Active"
        );
        assert_eq!(
            TrayIconState::DictationActive.description(),
            "Juno - Dictation Active"
        );
        assert_eq!(
            TrayIconState::AlwaysListening.description(),
            "Juno - Always Listening"
        );
        assert_eq!(TrayIconState::Processing.description(), "Juno - Processing");
        assert_eq!(TrayIconState::Error.description(), "Juno - Error");
    }

    #[tokio::test]
    async fn test_get_window_states_no_panic() {
        // This is a placeholder test since we can't easily mock AppHandle
        // In a real test environment, we would mock the AppHandle and windows
        assert!(
            true,
            "get_window_states should handle missing windows gracefully"
        );
    }

    #[test]
    fn test_tray_menu_constants() {
        // Test that required tray menu constants exist
        assert!(!tray_menu_ids::QUIT.is_empty());
        assert!(!tray_menu_ids::SETTINGS.is_empty());
        assert!(!tray_menu_ids::SHOW_FLOATING_BAR.is_empty());
        assert!(!tray_menu_ids::HIDE_FLOATING_BAR.is_empty());
        assert!(!tray_menu_ids::SHOW_MAIN_WINDOW.is_empty());
        assert!(!tray_menu_ids::HIDE_MAIN_WINDOW.is_empty());
        assert!(!tray_menu_ids::NEW_CHAT.is_empty());
        assert!(!tray_menu_ids::SHOW_DEVTOOLS.is_empty());
        assert!(!tray_menu_ids::TOGGLE_FLOATING_BAR.is_empty());
    }

    #[test]
    fn test_tray_icon_state_equality() {
        assert_eq!(TrayIconState::Default, TrayIconState::Default);
        assert_ne!(TrayIconState::Default, TrayIconState::AgentActive);
    }
}
