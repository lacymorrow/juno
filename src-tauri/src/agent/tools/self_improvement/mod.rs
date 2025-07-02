//! Self-Improvement System - Modular Implementation
//!
//! This module implements a comprehensive self-improvement system for the Juno AI agent,
//! split into focused submodules for better maintainability.

pub mod types;
pub mod config;

// Re-export all public types from existing modules
pub use types::*;
pub use config::*;

/// Register self-improvement tools with a provider (stub for compilation)
pub async fn register_self_improvement_tools_with_provider(
    _provider: &mut crate::agent::implementations::tool_provider::LocalToolProvider,
) -> Result<(), crate::agent::core::AgentError> {
    // Stub implementation - self-improvement tools not fully implemented yet
    Ok(())
}
