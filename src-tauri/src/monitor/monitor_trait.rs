//! # Monitor Trait
//!
//! Common trait for all monitor implementations to ensure consistency
//! and enable code reuse between agent and dictation monitors.

use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// Common state that all monitors share
#[derive(Debug, Clone)]
pub struct MonitorState {
    /// When the hold started
    pub hold_start_time: Option<Instant>,
    /// Whether the monitored action has started
    pub action_started: bool,
    /// Whether the hold threshold has been reached
    pub hold_threshold_reached: bool,
    /// When the action actually started
    pub action_start_time: Option<Instant>,
    /// Whether force cleanup is scheduled
    pub force_cleanup_scheduled: bool,
    /// When the last cancellation occurred (for cooldown)
    pub last_cancellation_time: Option<Instant>,
}

impl MonitorState {
    pub fn new() -> Self {
        Self {
            hold_start_time: None,
            action_started: false,
            hold_threshold_reached: false,
            action_start_time: None,
            force_cleanup_scheduled: false,
            last_cancellation_time: None,
        }
    }
    
    /// Reset all state fields to their initial values
    pub fn reset(&mut self) {
        self.hold_start_time = None;
        self.action_started = false;
        self.hold_threshold_reached = false;
        self.action_start_time = None;
        self.force_cleanup_scheduled = false;
        // Don't reset last_cancellation_time as it's used for cooldown
    }
    
    /// Force reset including cooldown state
    pub fn force_reset(&mut self) {
        self.reset();
        self.last_cancellation_time = None;
    }
    
    /// Check if we're in cooldown period
    pub fn is_in_cooldown(&self, cooldown_ms: u64) -> bool {
        if let Some(last_cancel) = self.last_cancellation_time {
            last_cancel.elapsed().as_millis() < cooldown_ms as u128
        } else {
            false
        }
    }
    
    /// Get elapsed time since hold started
    pub fn get_hold_duration(&self) -> Duration {
        self.hold_start_time
            .map(|start| start.elapsed())
            .unwrap_or(Duration::ZERO)
    }
    
    /// Get elapsed time since action started
    pub fn get_action_duration(&self) -> Duration {
        self.action_start_time
            .map(|start| start.elapsed())
            .unwrap_or(Duration::ZERO)
    }
}

/// Trait that all monitors must implement
#[async_trait::async_trait]
pub trait Monitor: Send + Sync {
    /// Get the monitor's name for logging
    fn name(&self) -> &str;
    
    /// Get the current state
    async fn get_state(&self) -> MonitorState;
    
    /// Start monitoring (key pressed)
    async fn start_hold(&self) -> bool;
    
    /// End monitoring (key released)
    async fn end_hold(&self) -> (bool, bool, Duration);
    
    /// Check if action should start (immediate threshold)
    async fn check_and_start_action(&self) -> bool;
    
    /// Check if hold threshold is reached (commit threshold)
    async fn check_and_reach_threshold(&self) -> bool;
    
    /// Check if action has timed out
    async fn check_timeout(&self) -> bool;
    
    /// Check if force cleanup is needed
    async fn should_force_cleanup(&self) -> bool;
    
    /// Force reset the monitor state
    async fn force_reset(&self);
}

/// Thread-safe wrapper for monitor state
pub type SharedMonitorState = Arc<RwLock<MonitorState>>;