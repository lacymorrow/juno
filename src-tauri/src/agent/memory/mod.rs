//! Pure Event-Driven Memory Management System
//!
//! This module implements TARS Phase 3: Clean event-driven memory management.
//! 
//! Features:
//! - Pure event stream storage - no legacy compatibility
//! - Lean, fast, maintainable architecture
//! - Real-time conversation streaming
//! - Smart token-aware pruning
//! - Enhanced debugging capabilities
//! - Session-based persistence with automatic checkpointing (Phase 3.5)

pub mod event_memory_manager;
pub mod event_converter;
pub mod persistence;
pub mod performance;

pub use event_memory_manager::{EventMemoryManager, EventMemoryConfig, EventMemoryMetrics};
pub use event_converter::{EventToMessageConverter, MessageToEventConverter};
pub use persistence::{EventMemoryPersistence, PersistenceConfig, SessionMetadata, StorageStats};
pub use performance::{
    PerformanceConfig, PerformanceMetrics, PerformanceSummary, ObjectPool, SmartCache,
    BatchProcessor, ConcurrencyLimiter, PoolStats, CacheStats,
};