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

// Re-export specific commonly used constants to avoid naming conflicts
// We avoid glob imports to prevent ambiguous re-export warnings
// But we still need to make essential constants available

// Re-export everything from specific modules that don't cause conflicts
pub use api::*;
pub use audio::*;
pub use browser::*;
pub use cli::*;
pub use error_messages::*;
pub use files::*;
// pub use memory::*; // Removed to avoid ambiguous glob re-exports (limits, defaults, patterns, visual)
pub use menus::*;
pub use mouse::*;
pub use performance::*;
pub use permissions::*;
pub use platform::*;
pub use ports::*;
pub use text::*;
pub use timeouts::*;

// For modules with potential conflicts, re-export specific items only
pub use agent::monitor_sessions;
pub use api::http_headers;
pub use app::{APP_NAME, PRODUCT_NAME, BUNDLE_IDENTIFIER};
pub use browser::chrome_debug_urls;
pub use commands::core::*;
pub use errors::{templates, prefixes};
pub use menus::app_menu_ids;
pub use settings::cloud_keys;
pub use ui::{bar_states, window_labels};

// For heavily namespaced modules, provide access via module re-export only
// Users should access these as: constants::events::agent::STARTED, etc.
// This avoids the naming conflicts between similarly named sub-modules

// Global constants that don't fit in specific modules
pub const MAX_MEMORY_ENTRIES: usize = 1000;

// UI constants
pub const DEFAULT_WINDOW_WIDTH: f64 = 1200.0;
pub const DEFAULT_WINDOW_HEIGHT: f64 = 800.0;

// File system constants
pub const MAX_FILE_SIZE_BYTES: usize = 100 * 1024 * 1024; // 100 MB

// System constants from platform (already re-exported above)

// App messages that can be re-exported for convenience
pub mod messages {
    // Note: Currently no messages module in files, but keeping structure for future use
    pub const GENERIC_SUCCESS: &str = "Operation completed successfully";
    pub const GENERIC_ERROR: &str = "An error occurred during the operation";
}
