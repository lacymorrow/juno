//! Self-Improvement System - Modular Implementation
//!
//! This module implements a comprehensive self-improvement system for the Juno AI agent,
//! split into focused submodules for better maintainability.

pub mod types;
pub mod config;
pub mod engine;
pub mod analysis;
pub mod validation;
pub mod benchmarks;

// Re-export all public types
pub use types::*;
pub use config::*;
pub use engine::*;
pub use analysis::*;
pub use validation::*;
pub use benchmarks::*;
