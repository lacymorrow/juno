//! # Constants Module
//!
//! This module provides centralized constants for all application modules.
//! Constants are organized into logical groups for better maintainability.

// Re-export all constant modules that actually exist
pub mod agent;
pub mod api;
pub mod app;
pub mod audio;
pub mod browser;
pub mod cli;
pub mod commands;
pub mod error_messages;
pub mod errors;
pub mod events;
pub mod files;
pub mod memory;
pub mod menus;
pub mod mouse;
pub mod performance;
pub mod permissions;
pub mod platform;
pub mod ports;
pub mod settings;
pub mod text;
pub mod timeouts;
pub mod ui;

// Re-export everything from agent for convenience
pub use agent::*;

// Re-export commonly used constants from modules that exist
pub use api::*;
pub use app::*;
pub use audio::*;
pub use browser::*;
pub use cli::*;
pub use commands::*;
pub use error_messages::*;
pub use errors::*;
pub use events::*;
pub use files::*;
pub use memory::*;
pub use menus::*;
pub use mouse::*;
pub use performance::*;
pub use permissions::*;
pub use ports::*;
pub use settings::*;
pub use text::*;
pub use timeouts::*;
pub use ui::*;

// Global constants that don't fit in specific modules
pub const MAX_MEMORY_ENTRIES: usize = 1000;

// UI constants
pub const DEFAULT_WINDOW_WIDTH: f64 = 1200.0;
pub const DEFAULT_WINDOW_HEIGHT: f64 = 800.0;

// File system constants
pub const MAX_FILE_SIZE_BYTES: usize = 100 * 1024 * 1024; // 100 MB

// System constants from platform
pub use platform::*;

// App messages that can be re-exported for convenience
pub mod messages {
    // Note: Currently no messages module in files, but keeping structure for future use
    pub const GENERIC_SUCCESS: &str = "Operation completed successfully";
    pub const GENERIC_ERROR: &str = "An error occurred during the operation";
}
