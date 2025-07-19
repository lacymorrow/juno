//! # Atomic Monitor Implementation
//!
//! Combines event-driven architecture with atomic state transitions
//! to eliminate race conditions in monitor operations.

use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use serde::{Deserialize, Serialize};
use tokio::sync::{RwLock, mpsc};
use tracing::{debug, error, info, warn};

use crate::constants::events;
use crate::error::{MonitorError, MonitorResult};
use crate::state::atomic_state::{AtomicMonitorState, SharedAtomicState};
use crate::events::sequence::{SequencedEmitter};
use super::{Monitor, MonitorState, MonitorEvent};

/// Configuration for atomic monitor
#[derive(Debug, Clone)]
pub struct AtomicMonitorConfig {
    pub name: String,
    pub immediate_start_ms: u64,
    pub hold_duration_ms: u64,
    pub max_duration_ms: u64,
    pub force_cleanup_timeout_ms: u64,
    pub cooldown_after_cancel_ms: u64,
    pub start_event: String,
    pub commit_event: String,
    pub stop_event: String,
}

/// Atomic event-driven monitor implementation
pub struct AtomicEventMonitor {
    config: AtomicMonitorConfig,
    state: SharedAtomicState,
    tx: mpsc::Sender<MonitorEvent>,
    app_handle: tauri::AppHandle,
}

impl AtomicEventMonitor {
    /// Create a new atomic monitor
    pub fn new(config: AtomicMonitorConfig, app_handle: tauri::AppHandle) -> (Self, mpsc::Receiver<MonitorEvent>) {
        let (tx, rx) = mpsc::channel(100);
        let state = Arc::new(AtomicMonitorState::new());
        
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
        info!("[{}] Starting atomic event-driven monitor", self.config.name);
        
        // Set up periodic timeout checking
        let monitor_clone = self.clone();
        let timeout_checker = tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_millis(100));
            loop {
                interval.tick().await;
                
                // Check for timeouts and force cleanup
                if monitor_clone.state.is_timed_out(monitor_clone.config.max_duration_ms) {
                    if monitor_clone.tx.send(MonitorEvent::CheckTimeout).await.is_err() {
                        break;
                    }
                } else if monitor_clone.state.needs_force_cleanup(monitor_clone.config.force_cleanup_timeout_ms) {
                    if monitor_clone.tx.send(MonitorEvent::CheckTimeout).await.is_err() {
                        break;
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
        
        timeout_checker.abort();
        info!("[{}] Monitor shutdown complete", self.config.name);
    }
    
    pub async fn send_event(&self, event: MonitorEvent) -> Result<(), String> {
        self.tx.send(event).await
            .map_err(|e| format!("Failed to send event: {}", e))
    }
    
    async fn handle_start_hold(&self) {
        match self.state.start_hold() {
            Ok(()) => {
                debug!("[{}] Started tracking hold", self.config.name);
                
                // Set up delayed checks
                let monitor = self.clone();
                let immediate_ms = self.config.immediate_start_ms;
                let hold_ms = self.config.hold_duration_ms;
                
                tokio::spawn(async move {
                    // Check for immediate start
                    tokio::time::sleep(Duration::from_millis(immediate_ms)).await;
                    if monitor.state.check_and_start_action(immediate_ms) {
                        monitor.emit_start_event().await;
                    }
                    
                    // Check for hold threshold
                    let remaining = hold_ms.saturating_sub(immediate_ms);
                    if remaining > 0 {
                        tokio::time::sleep(Duration::from_millis(remaining)).await;
                        if monitor.state.check_and_reach_threshold(hold_ms) {
                            monitor.emit_commit_event().await;
                        }
                    }
                });
            }
            Err(reason) => {
                debug!("[{}] Start hold rejected: {}", self.config.name, reason);
            }
        }
    }
    
    async fn handle_end_hold(&self) {
        let (action_started, threshold_reached, duration) = self.state.end_hold();
        
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
        if self.state.is_timed_out(self.config.max_duration_ms) {
            warn!("[{}] Action timeout detected - forcing stop", self.config.name);
            self.emit_stop_event("force").await;
            self.state.force_reset();
        } else if self.state.needs_force_cleanup(self.config.force_cleanup_timeout_ms) {
            warn!("[{}] Force cleanup triggered", self.config.name);
            self.emit_stop_event("error").await;
            self.state.force_reset();
        }
    }
    
    async fn handle_force_reset(&self) {
        self.state.force_reset();
        info!("[{}] Force reset completed", self.config.name);
    }
    
    async fn emit_start_event(&self) {
        if let Err(e) = self.app_handle.emit_sequenced(&self.config.start_event, ()) {
            error!("[{}] Failed to emit start event: {}", self.config.name, e);
        }
    }
    
    async fn emit_commit_event(&self) {
        if let Err(e) = self.app_handle.emit_sequenced(&self.config.commit_event, ()) {
            error!("[{}] Failed to emit commit event: {}", self.config.name, e);
        }
    }
    
    async fn emit_stop_event(&self, stop_type: &str) {
        let payload = serde_json::json!({
            "stopType": stop_type
        });
        if let Err(e) = self.app_handle.emit_sequenced(&self.config.stop_event, payload) {
            error!("[{}] Failed to emit stop event: {}", self.config.name, e);
        }
    }
}

impl Clone for AtomicEventMonitor {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            state: self.state.clone(),
            tx: self.tx.clone(),
            app_handle: self.app_handle.clone(),
        }
    }
}