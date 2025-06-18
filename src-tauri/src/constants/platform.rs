//! # Platform-specific Constants
//!
//! Shared constants for platform-specific functionality.
//! This module is used by both the main app and MCP server to avoid duplication.

pub mod macos;

// Re-export commonly used items
pub use macos::*;
