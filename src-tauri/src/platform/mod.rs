//! # Platform-Specific Module
//!
//! This module organizes platform-specific functionality for different operating systems.
//! Currently supports macOS with comprehensive window management and mouse tracking.

#[cfg(target_os = "macos")]
pub mod macos;

/// Passive stop-key (Escape) observer — macOS implementation plus stubs elsewhere.
pub mod stop_key_monitor;

#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(target_os = "linux")]
pub mod linux;

// Re-export platform-specific functionality based on current OS
#[cfg(target_os = "macos")]
pub use macos::*;

#[cfg(target_os = "windows")]
pub use windows::*;

#[cfg(target_os = "linux")]
pub use linux::*;
