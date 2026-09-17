pub mod core; // Core agent traits and types for orchestration
pub mod error_recovery; // Enhanced error recovery with checkpoint and rollback
pub mod implementations;
pub mod input_arbiter; // Physical input serialization across parallel agent sessions
pub mod intelligence;
pub mod local_intents; // Deterministic requests answered without a model round-trip
pub mod multi_agent; // Multi-agent orchestration system
pub mod prompts; // Centralized prompt management system
pub mod providers;
pub mod tool_logger;
pub mod tools;
pub mod traits; // Tool choice intelligence system
pub mod tts_tags; // Shared <TTS> extraction used by every provider

// Re-export commonly used items
pub use core::*;
