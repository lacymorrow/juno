//! Comprehensive Testing Framework for Event-Driven Memory System
//!
//! TARS Phase 3.6.3 & 3.6.4: Comprehensive testing framework and performance benchmarking
//! 
//! This module provides:
//! - Integration testing for the complete event-driven system
//! - Performance testing with configurable load scenarios
//! - Chaos testing for resilience validation
//! - Memory leak detection and resource validation
//! - End-to-end conversation flow testing
//! - Performance benchmarking suite with statistical analysis
//! - Real-time performance monitoring with alerting
//! - Command-line interface for running comprehensive performance tests

pub mod integration_tests;
pub mod performance_tests;
pub mod chaos_tests;
pub mod memory_tests;
pub mod conversation_tests;
pub mod test_fixtures;
pub mod test_utilities;

// TARS Phase 3.6.4: Performance benchmarking and metrics
pub mod benchmark_suite;
pub mod performance_monitor;
pub mod performance_command;

pub use integration_tests::*;
pub use performance_tests::*;
pub use chaos_tests::*;
pub use memory_tests::*;
pub use conversation_tests::*;
pub use test_fixtures::*;
pub use test_utilities::*;

// Re-export performance benchmarking components
pub use benchmark_suite::*;
pub use performance_monitor::*;
pub use performance_command::*;