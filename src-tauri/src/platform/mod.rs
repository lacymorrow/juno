//! # Platform-Specific Module
//!
//! This module organizes platform-specific functionality for different operating systems.
//! Currently supports macOS with comprehensive window management and mouse tracking.

#[cfg(target_os = "macos")]
pub mod cursor_follow; // Keep the bar on the display the cursor is on
#[cfg(target_os = "macos")]
pub mod macos;

pub mod file_panels;
/// Input Monitoring (TCC ListenEvent) check + request over IOKit; stubs elsewhere.
pub mod input_monitoring;
pub mod modifier_key_monitor;
pub mod mouse_button_monitor;
/// Native share sheet (NSSharingServicePicker) for agent responses; stub elsewhere.
pub mod share_sheet;
/// Passive stop-key (Escape) observer — macOS implementation plus stubs elsewhere.
pub mod stop_key_monitor; // Passive mouse-button trigger observer
/// Recognising Juno's own synthesized keyboard events (macOS only).
#[cfg(target_os = "macos")]
pub mod synthetic_events;

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
