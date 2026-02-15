//! # Testing Module
//!
//! Provides test harness and mock implementations for headless integration testing.
//! This module is not gated behind `#[cfg(test)]` because integration tests in
//! `tests/` are separate crates and cannot see `#[cfg(test)]` items.

pub mod harness;
pub mod mock_brain;
