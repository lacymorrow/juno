//! Development-specific commands and utilities
//!
//! This module contains commands that are primarily used for development, debugging,
//! and testing purposes. These commands often wrap production functionality with
//! additional logging, validation, or debugging features.

pub mod keyboard;

use tracing::{info, warn};

/// Check network connectivity status
#[tauri::command]
pub async fn check_network_connectivity() -> Result<serde_json::Value, String> {
    use crate::utils::network;

    info!("Manual network connectivity check requested");

    let start_time = std::time::Instant::now();
    let is_online = network::is_online().await;
    let duration = start_time.elapsed();

    let offline_message = network::get_offline_message();
    let status = serde_json::json!({
        "online": is_online,
        "status": if is_online { "Connected" } else { "Offline" },
        "message": if is_online {
            "Network connectivity is available"
        } else {
            offline_message.as_str()
        },
        "check_duration_ms": duration.as_millis(),
        "timestamp": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    });

    if is_online {
        info!("Network connectivity check: ONLINE ({}ms)", duration.as_millis());
    } else {
        warn!("Network connectivity check: OFFLINE ({}ms)", duration.as_millis());
    }

    Ok(status)
}

/// Test if specific error messages are detected as network errors
#[tauri::command]
pub async fn test_network_error_detection(error_message: String) -> Result<bool, String> {
    let is_network_error = crate::utils::network::is_network_error(&error_message);

    info!("Network error detection test: '{}' -> {}", error_message, is_network_error);

    Ok(is_network_error)
}

// Re-export dev command functions for backward compatibility
pub use keyboard::*;
