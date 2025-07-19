//! # Atomic State Management
//!
//! Provides atomic state transitions to prevent race conditions.
//! Uses atomics and compare-and-swap operations for thread safety.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

/// Atomic timestamp representation using milliseconds since epoch
#[derive(Debug)]
pub struct AtomicInstant {
    millis: AtomicU64,
}

impl AtomicInstant {
    pub fn new() -> Self {
        Self {
            millis: AtomicU64::new(0),
        }
    }
    
    pub fn now() -> Self {
        Self {
            millis: AtomicU64::new(Self::current_millis()),
        }
    }
    
    pub fn set(&self, instant: Option<Instant>) {
        let millis = instant.map(|i| Self::instant_to_millis(i)).unwrap_or(0);
        self.millis.store(millis, Ordering::SeqCst);
    }
    
    pub fn get(&self) -> Option<Instant> {
        let millis = self.millis.load(Ordering::SeqCst);
        if millis == 0 {
            None
        } else {
            Some(Self::millis_to_instant(millis))
        }
    }
    
    pub fn elapsed(&self) -> Option<Duration> {
        self.get().map(|instant| instant.elapsed())
    }
    
    pub fn compare_and_swap(&self, current: Option<Instant>, new: Option<Instant>) -> Result<(), Option<Instant>> {
        let current_millis = current.map(|i| Self::instant_to_millis(i)).unwrap_or(0);
        let new_millis = new.map(|i| Self::instant_to_millis(i)).unwrap_or(0);
        
        match self.millis.compare_exchange(
            current_millis,
            new_millis,
            Ordering::SeqCst,
            Ordering::SeqCst,
        ) {
            Ok(_) => Ok(()),
            Err(actual_millis) => {
                let actual = if actual_millis == 0 {
                    None
                } else {
                    Some(Self::millis_to_instant(actual_millis))
                };
                Err(actual)
            }
        }
    }
    
    fn current_millis() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64
    }
    
    fn instant_to_millis(instant: Instant) -> u64 {
        let elapsed = instant.elapsed();
        Self::current_millis() - elapsed.as_millis() as u64
    }
    
    fn millis_to_instant(millis: u64) -> Instant {
        let current = Self::current_millis();
        let elapsed = current.saturating_sub(millis);
        Instant::now() - Duration::from_millis(elapsed)
    }
}

/// Atomic monitor state using lock-free atomics where possible
#[derive(Debug)]
pub struct AtomicMonitorState {
    /// Atomic flags for boolean states
    action_started: AtomicBool,
    hold_threshold_reached: AtomicBool,
    force_cleanup_scheduled: AtomicBool,
    
    /// Atomic timestamps
    hold_start_time: AtomicInstant,
    action_start_time: AtomicInstant,
    last_cancellation_time: AtomicInstant,
    
    /// Generation counter for detecting concurrent modifications
    generation: AtomicU64,
}

impl AtomicMonitorState {
    pub fn new() -> Self {
        Self {
            action_started: AtomicBool::new(false),
            hold_threshold_reached: AtomicBool::new(false),
            force_cleanup_scheduled: AtomicBool::new(false),
            hold_start_time: AtomicInstant::new(),
            action_start_time: AtomicInstant::new(),
            last_cancellation_time: AtomicInstant::new(),
            generation: AtomicU64::new(0),
        }
    }
    
    /// Atomically start hold tracking
    pub fn start_hold(&self) -> Result<(), &'static str> {
        // Check if already started
        if self.action_started.load(Ordering::SeqCst) {
            return Err("Action already started");
        }
        
        // Check cooldown
        if let Some(last_cancel) = self.last_cancellation_time.get() {
            if last_cancel.elapsed().as_millis() < 150 {
                return Err("Still in cooldown period");
            }
        }
        
        // Atomically reset state and set hold start time
        self.action_started.store(false, Ordering::SeqCst);
        self.hold_threshold_reached.store(false, Ordering::SeqCst);
        self.force_cleanup_scheduled.store(false, Ordering::SeqCst);
        self.action_start_time.set(None);
        self.hold_start_time.set(Some(Instant::now()));
        
        // Increment generation to invalidate any concurrent operations
        self.generation.fetch_add(1, Ordering::SeqCst);
        
        Ok(())
    }
    
    /// Atomically end hold tracking
    pub fn end_hold(&self) -> (bool, bool, Duration) {
        let action_was_started = self.action_started.load(Ordering::SeqCst);
        let threshold_was_reached = self.hold_threshold_reached.load(Ordering::SeqCst);
        let duration = self.hold_start_time.elapsed().unwrap_or(Duration::ZERO);
        
        // Record cancellation if needed
        if action_was_started && !threshold_was_reached {
            self.last_cancellation_time.set(Some(Instant::now()));
        }
        
        // Atomically reset state
        self.action_started.store(false, Ordering::SeqCst);
        self.hold_threshold_reached.store(false, Ordering::SeqCst);
        self.force_cleanup_scheduled.store(false, Ordering::SeqCst);
        self.hold_start_time.set(None);
        self.action_start_time.set(None);
        
        // Increment generation
        self.generation.fetch_add(1, Ordering::SeqCst);
        
        (action_was_started, threshold_was_reached, duration)
    }
    
    /// Atomically check and start action
    pub fn check_and_start_action(&self, immediate_start_ms: u64) -> bool {
        // Use compare-and-swap to ensure atomic transition
        match self.action_started.compare_exchange(
            false,
            true,
            Ordering::SeqCst,
            Ordering::SeqCst,
        ) {
            Ok(_) => {
                // Successfully transitioned from false to true
                if let Some(duration) = self.hold_start_time.elapsed() {
                    if duration.as_millis() >= immediate_start_ms as u128 {
                        self.action_start_time.set(Some(Instant::now()));
                        return true;
                    }
                }
                // Rollback if conditions not met
                self.action_started.store(false, Ordering::SeqCst);
                false
            }
            Err(_) => false, // Already started
        }
    }
    
    /// Atomically check and reach threshold
    pub fn check_and_reach_threshold(&self, hold_duration_ms: u64) -> bool {
        // Use compare-and-swap to ensure atomic transition
        match self.hold_threshold_reached.compare_exchange(
            false,
            true,
            Ordering::SeqCst,
            Ordering::SeqCst,
        ) {
            Ok(_) => {
                // Successfully transitioned from false to true
                if let Some(duration) = self.hold_start_time.elapsed() {
                    if duration.as_millis() >= hold_duration_ms as u128 {
                        return true;
                    }
                }
                // Rollback if conditions not met
                self.hold_threshold_reached.store(false, Ordering::SeqCst);
                false
            }
            Err(_) => false, // Already reached
        }
    }
    
    /// Check if action has timed out
    pub fn is_timed_out(&self, max_duration_ms: u64) -> bool {
        if let Some(duration) = self.action_start_time.elapsed() {
            duration.as_millis() >= max_duration_ms as u128
        } else {
            false
        }
    }
    
    /// Check if force cleanup is needed
    pub fn needs_force_cleanup(&self, force_cleanup_timeout_ms: u64) -> bool {
        let action_started = self.action_started.load(Ordering::SeqCst);
        let hold_active = self.hold_start_time.get().is_some();
        let already_scheduled = self.force_cleanup_scheduled.load(Ordering::SeqCst);
        
        if action_started && !hold_active && !already_scheduled {
            if let Some(duration) = self.action_start_time.elapsed() {
                if duration.as_millis() >= force_cleanup_timeout_ms as u128 {
                    // Try to atomically set the flag
                    match self.force_cleanup_scheduled.compare_exchange(
                        false,
                        true,
                        Ordering::SeqCst,
                        Ordering::SeqCst,
                    ) {
                        Ok(_) => return true,
                        Err(_) => return false, // Someone else scheduled it
                    }
                }
            }
        }
        false
    }
    
    /// Force reset all state
    pub fn force_reset(&self) {
        self.action_started.store(false, Ordering::SeqCst);
        self.hold_threshold_reached.store(false, Ordering::SeqCst);
        self.force_cleanup_scheduled.store(false, Ordering::SeqCst);
        self.hold_start_time.set(None);
        self.action_start_time.set(None);
        // Don't reset last_cancellation_time on force reset
        self.generation.fetch_add(1, Ordering::SeqCst);
    }
    
    /// Get current generation for detecting concurrent modifications
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::SeqCst)
    }
}

/// Thread-safe wrapper for atomic monitor state
pub type SharedAtomicState = Arc<AtomicMonitorState>;