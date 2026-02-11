//! # Testing Module
//!
//! Provides test harness and mock implementations for headless integration testing.
//! This module is not gated behind `#[cfg(test)]` because integration tests in
//! `tests/` need access to it as a public module.

pub mod harness;

#[cfg(test)]
pub mod mock_brain;
