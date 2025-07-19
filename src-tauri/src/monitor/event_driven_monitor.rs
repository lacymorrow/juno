//! # Event-Driven Monitor
//!
//! Replaces polling-based monitoring with an efficient event-driven system.
//! Uses async channels to receive events and processes them without busy-waiting.

use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, RwLock};
use tauri::{AppHandle, Emitter};
use tracing::{debug, error, info, warn};

use super::{Monitor, MonitorState};
use super::monitor_trait::SharedMonitorState;

/// Events that can be sent to the monitor
#[derive(Debug, Clone)]
pub enum MonitorEvent {
    /// Key/action pressed down
    StartHold,
    /// Key/action released
    EndHold,
    /// Check for timeouts (periodic)
    CheckTimeout,
    /// Force reset the monitor
    ForceReset,
    /// Shutdown the monitor
    Shutdown,
}

/// Configuration for a monitor instance
#[derive(Debug, Clone)]
pub struct MonitorConfig {
    /// Name of the monitor (for logging)
    pub name: String,
    /// How long to hold before starting action (ms)
    pub immediate_start_ms: u64,
    /// How long to hold before committing (ms)
    pub hold_duration_ms: u64,
    /// Maximum action duration (ms)
    pub max_duration_ms: u64,
    /// Force cleanup timeout (ms)
    pub force_cleanup_timeout_ms: u64,
    /// Cooldown after cancellation (ms)
    pub cooldown_after_cancel_ms: u64,
    /// Event to emit when action starts
    pub start_event: String,
    /// Event to emit when threshold is reached
    pub commit_event: String,
    /// Event to emit when stopping
    pub stop_event: String,
}

/// Event-driven monitor implementation
pub struct EventDrivenMonitor {
    /// Monitor configuration
    config: MonitorConfig,
    /// Shared state
    state: SharedMonitorState,
    /// Channel sender for events
    tx: mpsc::Sender<MonitorEvent>,
    /// App handle for emitting events
    app_handle: AppHandle,
}

impl EventDrivenMonitor {
    /// Create a new event-driven monitor
    pub fn new(config: MonitorConfig, app_handle: AppHandle) -> (Self, mpsc::Receiver<MonitorEvent>) {
        let (tx, rx) = mpsc::channel(100);
        let state = Arc::new(RwLock::new(MonitorState::new()));
        
        let monitor = Self {
            config,
            state,
            tx,
            app_handle,
        };
        
        (monitor, rx)
    }
    
    /// Start the monitor's event processing loop
    pub async fn run(self: Arc<Self>, mut rx: mpsc::Receiver<MonitorEvent>) {
        info!("[{}] Starting event-driven monitor", self.config.name);
        
        // Set up periodic timeout checking (only when action is active)
        let monitor_clone = self.clone();
        let timeout_checker = tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_millis(100));
            loop {
                interval.tick().await;
                
                // Only check timeouts if action is active
                let state = monitor_clone.state.read().await;
                if state.action_started {
                    drop(state); // Release read lock before sending
                    if monitor_clone.tx.send(MonitorEvent::CheckTimeout).await.is_err() {
                        break; // Channel closed, monitor is shutting down
                    }
                }
            }
        });
        
        // Main event processing loop
        while let Some(event) = rx.recv().await {
            match event {
                MonitorEvent::StartHold => {
                    self.handle_start_hold().await;
                }
                MonitorEvent::EndHold => {
                    self.handle_end_hold().await;
                }
                MonitorEvent::CheckTimeout => {
                    self.handle_check_timeout().await;
                }
                MonitorEvent::ForceReset => {
                    self.handle_force_reset().await;
                }
                MonitorEvent::Shutdown => {
                    info!("[{}] Shutting down monitor", self.config.name);
                    break;
                }
            }
        }
        
        // Clean up timeout checker
        timeout_checker.abort();
        info!("[{}] Monitor shutdown complete", self.config.name);
    }
    
    /// Send an event to this monitor
    pub async fn send_event(&self, event: MonitorEvent) -> Result<(), String> {
        self.tx.send(event).await
            .map_err(|e| format!("Failed to send event: {}", e))
    }
    
    async fn handle_start_hold(&self) {
        let started = self.start_hold().await;
        if started {
            // Check immediately if we should start action
            if self.check_and_start_action().await {
                self.emit_start_event().await;
            }
            
            // Set up delayed check for hold threshold
            let monitor = self.clone();
            let hold_duration = self.config.hold_duration_ms;
            tokio::spawn(async move {
                tokio::time::sleep(Duration::from_millis(hold_duration)).await;
                if monitor.check_and_reach_threshold().await {
                    monitor.emit_commit_event().await;
                }
            });
        }
    }
    
    async fn handle_end_hold(&self) {
        let (action_started, threshold_reached, duration) = self.end_hold().await;
        
        if threshold_reached {
            info!("[{}] Released after threshold - stopping normally", self.config.name);
            self.emit_stop_event("normal").await;
        } else if action_started {
            info!("[{}] Released before threshold ({}ms) - cancelling", 
                self.config.name, duration.as_millis());
            self.emit_stop_event("cancel").await;
        } else {
            debug!("[{}] Released without starting action ({}ms)", 
                self.config.name, duration.as_millis());
        }
    }
    
    async fn handle_check_timeout(&self) {
        if self.check_timeout().await {
            warn!("[{}] Action timeout detected - forcing stop", self.config.name);
            self.emit_stop_event("force").await;
            self.force_reset().await;
        } else if self.should_force_cleanup().await {
            warn!("[{}] Force cleanup triggered", self.config.name);
            self.emit_stop_event("error").await;
            self.force_reset().await;
        }
    }
    
    async fn handle_force_reset(&self) {
        self.force_reset().await;
        info!("[{}] Force reset completed", self.config.name);
    }
    
    async fn emit_start_event(&self) {
        if let Err(e) = self.app_handle.emit(&self.config.start_event, ()) {
            error!("[{}] Failed to emit start event: {}", self.config.name, e);
        }
    }
    
    async fn emit_commit_event(&self) {
        if let Err(e) = self.app_handle.emit(&self.config.commit_event, ()) {
            error!("[{}] Failed to emit commit event: {}", self.config.name, e);
        }
    }
    
    async fn emit_stop_event(&self, stop_type: &str) {
        let payload = serde_json::json!({
            "stopType": stop_type
        });
        if let Err(e) = self.app_handle.emit(&self.config.stop_event, payload) {
            error!("[{}] Failed to emit stop event: {}", self.config.name, e);
        }
    }
}

#[async_trait::async_trait]
impl Monitor for EventDrivenMonitor {
    fn name(&self) -> &str {
        &self.config.name
    }
    
    async fn get_state(&self) -> MonitorState {
        self.state.read().await.clone()
    }
    
    async fn start_hold(&self) -> bool {
        let mut state = self.state.write().await;
        
        // Check if already active
        if state.action_started {
            debug!("[{}] Ignoring start - action already active", self.config.name);
            return false;
        }
        
        // Check cooldown
        if state.is_in_cooldown(self.config.cooldown_after_cancel_ms) {
            debug!("[{}] Ignoring start - in cooldown period", self.config.name);
            return false;
        }
        
        // Start tracking
        state.reset();
        state.hold_start_time = Some(Instant::now());
        debug!("[{}] Started tracking hold", self.config.name);
        true
    }
    
    async fn end_hold(&self) -> (bool, bool, Duration) {
        let mut state = self.state.write().await;
        
        let action_was_started = state.action_started;
        let threshold_was_reached = state.hold_threshold_reached;
        let duration = state.get_hold_duration();
        
        // Record cancellation time if needed
        if action_was_started && !threshold_was_reached {
            state.last_cancellation_time = Some(Instant::now());
            warn!("[{}] Cancelling after {}ms", self.config.name, duration.as_millis());
        }
        
        // Reset state
        state.reset();
        
        debug!("[{}] Ended hold - duration: {}ms, started: {}, threshold: {}",
            self.config.name, duration.as_millis(), action_was_started, threshold_was_reached);
        
        (action_was_started, threshold_was_reached, duration)
    }
    
    async fn check_and_start_action(&self) -> bool {
        let mut state = self.state.write().await;
        
        if state.action_started {
            return false;
        }
        
        let duration = state.get_hold_duration();
        if duration.as_millis() >= self.config.immediate_start_ms as u128 {
            state.action_started = true;
            state.action_start_time = Some(Instant::now());
            info!("[{}] Starting action after {}ms", self.config.name, duration.as_millis());
            return true;
        }
        
        false
    }
    
    async fn check_and_reach_threshold(&self) -> bool {
        let mut state = self.state.write().await;
        
        if state.hold_threshold_reached {
            return false;
        }
        
        let duration = state.get_hold_duration();
        if duration.as_millis() >= self.config.hold_duration_ms as u128 {
            state.hold_threshold_reached = true;
            info!("[{}] Threshold reached after {}ms", self.config.name, duration.as_millis());
            return true;
        }
        
        false
    }
    
    async fn check_timeout(&self) -> bool {
        let state = self.state.read().await;
        
        if let Some(start_time) = state.action_start_time {
            let duration = start_time.elapsed();
            if duration.as_millis() >= self.config.max_duration_ms as u128 {
                warn!("[{}] Action timeout after {}ms", self.config.name, duration.as_millis());
                return true;
            }
        }
        
        false
    }
    
    async fn should_force_cleanup(&self) -> bool {
        let mut state = self.state.write().await;
        
        if state.action_started && state.hold_start_time.is_none() && !state.force_cleanup_scheduled {
            if let Some(start_time) = state.action_start_time {
                let duration = start_time.elapsed();
                if duration.as_millis() >= self.config.force_cleanup_timeout_ms as u128 {
                    state.force_cleanup_scheduled = true;
                    warn!("[{}] Scheduling force cleanup after {}ms", 
                        self.config.name, duration.as_millis());
                    return true;
                }
            }
        }
        
        false
    }
    
    async fn force_reset(&self) {
        let mut state = self.state.write().await;
        state.force_reset();
        warn!("[{}] Force reset completed", self.config.name);
    }
}

impl Clone for EventDrivenMonitor {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            state: self.state.clone(),
            tx: self.tx.clone(),
            app_handle: self.app_handle.clone(),
        }
    }
}