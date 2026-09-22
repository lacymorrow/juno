//! # Tray Commands Module
//!
//! Tauri commands for manual control and testing of the menu bar icon. They let
//! the frontend force a state for debugging; in normal use the icon follows app
//! state through the listeners in `menu::tray_menu`.

use crate::menu::tray_menu::{
    current_tray_icon_state, set_tray_icon_state, set_tray_icon_visible_now,
    update_tray_icon_state, TrayIconState,
};
use crate::settings::manager::SettingsManager;
use tauri::{command, State};
use tracing::info;

/// Set the tray icon to idle
#[command]
pub async fn set_tray_icon_default() -> Result<(), String> {
    info!("Setting tray icon to idle");
    crate::menu::tray_menu::set_idle().await;
    Ok(())
}

/// Set the tray icon to agent running
#[command]
pub async fn set_tray_icon_agent_active() -> Result<(), String> {
    info!("Setting tray icon to agent running");
    crate::menu::tray_menu::set_agent().await;
    Ok(())
}

/// Set the tray icon to recording
#[command]
pub async fn set_tray_icon_dictation_active() -> Result<(), String> {
    info!("Setting tray icon to recording");
    crate::menu::tray_menu::set_recording().await;
    Ok(())
}

/// Set the tray icon to armed (always listening on)
#[command]
pub async fn set_tray_icon_always_listening() -> Result<(), String> {
    info!("Setting tray icon to armed");
    crate::menu::tray_menu::set_armed().await;
    Ok(())
}

/// Set the tray icon to transcribing
#[command]
pub async fn set_tray_icon_processing() -> Result<(), String> {
    info!("Setting tray icon to transcribing");
    crate::menu::tray_menu::set_transcribing().await;
    Ok(())
}

/// Set the tray icon to error
#[command]
pub async fn set_tray_icon_error() -> Result<(), String> {
    info!("Setting tray icon to error");
    crate::menu::tray_menu::set_error().await;
    Ok(())
}

/// Set the tray icon to paused
#[command]
pub async fn set_tray_icon_paused() -> Result<(), String> {
    info!("Setting tray icon to paused");
    crate::menu::tray_menu::set_paused().await;
    Ok(())
}

/// Update tray icon based on current application state
#[command]
pub async fn update_tray_icon_from_state() -> Result<(), String> {
    info!("Updating tray icon based on current application state");
    update_tray_icon_state(TrayIconState::Idle).await;
    Ok(())
}

/// Step through every tray icon state, 1.5 s each, then return to idle
#[command]
pub async fn test_all_tray_icon_states() -> Result<(), String> {
    info!("Testing all tray icon states in sequence");

    let states = [
        TrayIconState::Idle,
        TrayIconState::Armed,
        TrayIconState::Recording,
        TrayIconState::Transcribing,
        TrayIconState::Agent,
        TrayIconState::Error,
        TrayIconState::Paused,
    ];

    for state in states {
        info!("Testing tray icon state: {:?}", state);
        set_tray_icon_state(state).await;
        tokio::time::sleep(tokio::time::Duration::from_millis(1500)).await;
    }

    set_tray_icon_state(TrayIconState::Idle).await;
    info!("Tray icon state testing completed");

    Ok(())
}

/// Get the current tray icon state as its menu label
#[command]
pub async fn get_current_tray_icon_state() -> Result<String, String> {
    Ok(current_tray_icon_state().await.label().to_string())
}

/// Is Juno's menu-bar (tray) icon currently shown?
#[command]
pub async fn get_tray_icon_visible(
    settings_manager: State<'_, SettingsManager>,
) -> Result<bool, String> {
    let agent_settings = settings_manager.get_agent_settings().await?;
    Ok(agent_settings.show_tray_icon)
}

/// Show or hide Juno's menu-bar (tray) icon, and remember the choice.
#[command]
pub async fn set_tray_icon_visible(
    settings_manager: State<'_, SettingsManager>,
    visible: bool,
) -> Result<(), String> {
    let mut agent_settings = settings_manager.get_agent_settings().await?;
    agent_settings.show_tray_icon = visible;
    settings_manager.set_agent_settings(&agent_settings).await?;

    set_tray_icon_visible_now(visible).await;
    info!(
        "[TrayIcon] Menu bar icon is now {}",
        if visible { "shown" } else { "hidden" }
    );
    Ok(())
}
