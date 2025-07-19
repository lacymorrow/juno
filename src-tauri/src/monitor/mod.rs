//! # Monitor Module
//!
//! Provides event-driven monitoring infrastructure to replace polling-based systems.
//! This module implements efficient, low-overhead monitoring using async channels
//! and event notifications instead of periodic polling.

pub mod atomic_monitor;
pub mod event_driven_monitor;
pub mod monitor_trait;

pub use atomic_monitor::{AtomicEventMonitor, AtomicMonitorConfig};
pub use event_driven_monitor::{EventDrivenMonitor, MonitorConfig, MonitorEvent};
pub use monitor_trait::{Monitor, MonitorState};