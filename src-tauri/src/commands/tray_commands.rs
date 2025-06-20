//! # Tray Commands Module
//!
//! This module provides Tauri commands for manual control and testing of the tray icon system.
//! These commands allow the frontend to manually trigger tray icon state changes for testing
//! and debugging purposes.

use tauri::command;
use tracing::{info, error};
use crate::menu::tray_menu::{TrayIconState, update_tray_icon_state, set_tray_icon_state};

/// Set the tray icon to default state
#[command]
pub async fn set_tray_icon_default() -> Result<(), String> {
    info!("🔄 Setting tray icon to default state");
    crate::menu::tray_menu::set_default().await;
    Ok(())
}

/// Set the tray icon to agent active state
#[command]
pub async fn set_tray_icon_agent_active() -> Result<(), String> {
    info!("🤖 Setting tray icon to agent active state");
    crate::menu::tray_menu::set_agent_active().await;
    Ok(())
}

/// Set the tray icon to dictation active state
#[command]
pub async fn set_tray_icon_dictation_active() -> Result<(), String> {
    info!("🎤 Setting tray icon to dictation active state");
    crate::menu::tray_menu::set_dictation_active().await;
    Ok(())
}

/// Set the tray icon to always listening state
#[command]
pub async fn set_tray_icon_always_listening() -> Result<(), String> {
    info!("👂 Setting tray icon to always listening state");
    crate::menu::tray_menu::set_always_listening().await;
    Ok(())
}

/// Set the tray icon to processing state
#[command]
pub async fn set_tray_icon_processing() -> Result<(), String> {
    info!("⚙️ Setting tray icon to processing state");
    crate::menu::tray_menu::set_processing().await;
    Ok(())
}

/// Set the tray icon to error state
#[command]
pub async fn set_tray_icon_error() -> Result<(), String> {
    info!("❌ Setting tray icon to error state");
    crate::menu::tray_menu::set_error().await;
    Ok(())
}

/// Update tray icon based on current application state
#[command]
pub async fn update_tray_icon_from_state() -> Result<(), String> {
    info!("🔄 Updating tray icon based on current application state");
    // This will be automatically determined by the determine_current_state function
    update_tray_icon_state(TrayIconState::Default).await;
    Ok(())
}

/// Test all tray icon states in sequence (for testing purposes)
#[command]
pub async fn test_all_tray_icon_states() -> Result<(), String> {
    info!("🧪 Testing all tray icon states in sequence");

    let states = vec![
        ("Default", TrayIconState::Default),
        ("Agent Active", TrayIconState::AgentActive),
        ("Dictation Active", TrayIconState::DictationActive),
        ("Always Listening", TrayIconState::AlwaysListening),
        ("Processing", TrayIconState::Processing),
        ("Error", TrayIconState::Error),
    ];

    for (name, state) in states {
        info!("Testing tray icon state: {}", name);
        set_tray_icon_state(state).await;

        // Small delay to see the change
        tokio::time::sleep(tokio::time::Duration::from_millis(1500)).await;
    }

    // Return to default
    set_tray_icon_state(TrayIconState::Default).await;
    info!("✅ Tray icon state testing completed");

    Ok(())
}

/// Get the current tray icon state as a string
#[command]
pub async fn get_current_tray_icon_state() -> Result<String, String> {
    // Note: This is a simplified version since we can't easily access the current state
    // In a real implementation, you might want to store the current state in the app state
    Ok("Unknown (state tracking not implemented in this command)".to_string())
}
