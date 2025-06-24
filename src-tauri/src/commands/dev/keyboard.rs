//! Development keyboard commands with enhanced debugging and validation
//!
//! These commands wrap the production keyboard functions with additional
//! development-specific features like enhanced logging, validation, and debugging utilities.

use tauri::{State, AppHandle};
use crate::state::AppState;
use crate::commands::keyboard;
use tracing::{info, warn, debug};

/// Development wrapper for type_text with enhanced logging and validation
#[tauri::command]
pub(crate) async fn dev_type_text(text: String, app_handle: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    debug!("DEV: type_text called with text length: {}", text.len());

    // Development-specific validation
    if text.is_empty() {
        warn!("DEV: Attempted to type empty text");
        return Err("Cannot type empty text".to_string());
    }

    if text.len() > 10000 {
        warn!("DEV: Attempting to type very long text ({} chars), this may be slow", text.len());
    }

    // Call the production function
    keyboard::type_text(text, app_handle, state).await
}

/// Development wrapper for press_key with enhanced logging and validation
#[tauri::command]
pub(crate) async fn dev_press_key(key: String, modifier: Option<String>, app_handle: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    debug!("DEV: press_key called - key: '{}', modifier: {:?}", key, modifier);

    // Development-specific validation
    if key.trim().is_empty() {
        warn!("DEV: Attempted to press empty/whitespace key");
        return Err("Cannot press empty key".to_string());
    }

    // Call the production function
    keyboard::press_key(key, modifier, app_handle, state).await
}

/// Development wrapper for global_type_text with enhanced logging
#[tauri::command]
pub(crate) async fn dev_global_type_text(text: String, app_handle: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    debug!("DEV: global_type_text called with text length: {}", text.len());

    // Development-specific validation
    if text.is_empty() {
        warn!("DEV: Attempted to globally type empty text");
        return Err("Cannot globally type empty text".to_string());
    }

    // Call the production function
    keyboard::global_type_text(text, app_handle, state).await
}

/// Development wrapper for hold_key with enhanced logging and duration validation
#[tauri::command]
pub(crate) async fn dev_hold_key(key: String, duration_ms: Option<u64>, app_handle: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    debug!("DEV: hold_key called - key: '{}', duration: {:?}ms", key, duration_ms);

    // Development-specific validation
    if key.trim().is_empty() {
        warn!("DEV: Attempted to hold empty/whitespace key");
        return Err("Cannot hold empty key".to_string());
    }

    if let Some(duration) = duration_ms {
        if duration > 30000 { // 30 seconds
            warn!("DEV: Holding key for very long duration ({}ms), this may cause issues", duration);
        }
    }

    // Call the production function
    keyboard::hold_key(key, duration_ms, app_handle, state).await
}

/// Development wrapper for release_key with enhanced logging and validation
#[tauri::command]
pub(crate) async fn dev_release_key(key: String, app_handle: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    debug!("DEV: release_key called - key: '{}'", key);

    // Development-specific validation
    if key.trim().is_empty() {
        warn!("DEV: Attempted to release empty/whitespace key");
        return Err("Cannot release empty key".to_string());
    }

    // Call the production function
    keyboard::release_key(key, app_handle, state).await
}
