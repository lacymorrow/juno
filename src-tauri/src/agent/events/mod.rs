//! Event-driven architecture for Juno's agent system
//! 
//! This module implements a comprehensive event system inspired by TARS's
//! event stream architecture with modern event-driven patterns.

pub mod event_types;
pub mod event_processor;
pub mod event_bus;
pub mod optimized_event_bus;

pub use event_types::{JunoAgentEvent, EventSubscriber, EventFilter};
pub use event_processor::{JunoEventStreamProcessor, EventProcessorConfig, LoggingSubscriber};
pub use event_bus::{EventBus, EventHandler, EventBusConfig, generate_session_id, now};
pub use optimized_event_bus::{OptimizedEventBus, OptimizedEventBusConfig, HandlerStats};

// Re-export commonly used types
pub use event_types::JunoAgentEvent as AgentEvent;
pub use event_processor::JunoEventStreamProcessor as EventProcessor;