pub mod structs;
pub mod traits;
pub mod implementations;
pub mod tools;
pub mod tool_logger;
pub mod providers;
pub mod prompts; // Centralized prompt management system
pub mod core; // Core agent traits and types for orchestration
pub mod multi_agent; // Multi-agent orchestration system
pub mod security;  // Add the new security module

// Re-export commonly used items
pub use core::*;

// Re-export key security types for easy access
pub use security::{
    SecurityManager, SecurityConfig, CommandValidator, ApprovalManager,
    ExecutionMonitor, FileMonitor, RateLimiter, RiskLevel
};

