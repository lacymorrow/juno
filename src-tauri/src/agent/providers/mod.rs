//! Manages different AI provider configurations and instantiations.

pub mod config;
pub mod factory;
pub mod openai;    // Assuming these are provider implementations
pub mod anthropic; // Assuming these are provider implementations

// Re-export key items if needed
pub use config::{ProviderConfig, ProviderSettings};
pub use factory::{BrainFactory, ProviderInfo};
