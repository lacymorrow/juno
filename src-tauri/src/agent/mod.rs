pub mod structs;
pub mod traits;
pub mod implementations;
pub mod tools;
pub mod tool_logger;
pub mod providers;
pub mod prompts; // Centralized prompt management system
pub mod core; // Core agent traits and types for orchestration
pub mod multi_agent; // Multi-agent orchestration system
pub mod error_recovery; // Enhanced error recovery with checkpoint and rollback
pub mod intelligence; // Tool choice intelligence system
pub mod events; // Event-driven architecture for TARS integration
pub mod memory; // TARS Phase 3: Hybrid memory management with event streams
pub mod state_machine; // Event-driven state machine
pub mod handlers; // Event-driven handlers
pub mod event_system_manager; // Centralized event system management
pub mod testing; // Comprehensive testing framework

// Re-export commonly used items
pub use core::*;
pub use memory::{EventMemoryManager, EventMemoryConfig, EventMemoryMetrics}; // TARS Phase 3: Pure event-driven memory
pub use state_machine::{AgentState, AgentStateMachine, StateMachineConfig};
pub use handlers::{UserInputHandler, AgentOrchestrator};
pub use event_system_manager::EventSystemManager;

