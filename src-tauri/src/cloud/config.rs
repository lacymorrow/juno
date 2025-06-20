//! # Cloud Configuration Management
//!
//! Simple cloud settings management using centralized SettingsManager.

use crate::settings::{SettingsManager, CloudConfig};
use tauri::AppHandle;
use tracing::info;

// Cloud configuration functions are now handled in commands/cloud.rs
// This file is kept for future cloud-specific utilities if needed
