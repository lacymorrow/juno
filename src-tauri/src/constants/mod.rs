//! # Constants Module
//!
//! Modular constants organization for the Juno application.
//! This replaces the monolithic constants.rs file with a more organized structure.

// Core application constants
pub mod app;
pub mod events;
pub mod timeouts;

// Network and API constants
pub mod api;
pub mod ports;

// Platform-specific constants
pub mod platform;

// UI and interaction constants
pub mod ui;
pub mod menus;

// Agent and AI constants
pub mod agent;

// Error handling constants
pub mod errors;

// File and system constants
pub mod files;

// Audio processing constants
pub mod audio;

// Browser automation constants
pub mod browser;

// Permissions constants
pub mod permissions;

// Re-export commonly used modules for compatibility (being specific to avoid conflicts)
pub use app::*;
pub use events::*;
pub use timeouts::*;
pub use api::*;
pub use ports::*;
pub use platform::*;
pub use ui::*;
pub use menus::*;
pub use agent::*;
// Only re-export specific parts of errors to avoid conflict with events::messages
pub use errors::{codes, recovery, cloud_networking};
pub use files::*;
pub use audio::*;
pub use browser::*;
pub use permissions::*;

// Legacy compatibility - gradually remove these
pub mod legacy {
    // Deprecated modules - use specific modules instead
    pub use crate::constants::events as events;
    pub use crate::constants::timeouts as timeouts;
    pub use crate::constants::api as api_endpoints;
    pub use crate::constants::platform::macos as macos_system;
}
