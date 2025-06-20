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

// Settings management constants
pub mod settings;

// Audio processing constants
pub mod audio;

// Browser automation constants
pub mod browser;

// Permissions constants
pub mod permissions;

// Re-export all constants that actually exist
pub use crate::constants::agent::*;
pub use crate::constants::api::*;
pub use crate::constants::app::*;
pub use crate::constants::audio::*;
pub use crate::constants::browser::*;
pub use crate::constants::events::*;
pub use crate::constants::files::*;
pub use crate::constants::menus::*;
pub use crate::constants::permissions::*;
pub use crate::constants::ports::*;
pub use crate::constants::settings::*;
pub use crate::constants::timeouts::*;
pub use crate::constants::ui::*;
pub use crate::constants::errors::*;

// Platform specific constants
pub use crate::constants::platform::*;
