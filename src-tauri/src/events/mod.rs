//! # Events Module
//!
//! This module organizes all event handling functionality for the Juno application.
//! It includes global shortcut handlers, voice transcription event listeners,
//! and various application event management systems.

pub mod handlers;
pub mod shortcuts;
pub mod timer_handlers;

// Re-export key functionality
pub use handlers::*;
pub use shortcuts::*;
