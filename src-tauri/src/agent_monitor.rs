use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::Mutex;
use tracing::{info, error, debug, warn};
use crate::state::{AppState, AgentTriggerMode};

// Configuration constants
const HOLD_DURATION_MS: u64 = 500; // Hold agent key for 500ms to commit to agent mode
const IMMEDIATE_START_MS: u64 = 0; // Start agent immediately (0ms delay)
const MAX_AGENT_DURATION_MS: u64 = 120_000; // 2 minutes max agent session time
const FORCE_CLEANUP_TIMEOUT_MS: u64 = 5_000; // 5 seconds to force cleanup if stuck
const COOLDOWN_AFTER_CANCEL_MS: u64 = 150; // 150ms cooldown for better responsiveness

// State for agent input monitoring
#[derive(Debug)]
pub struct AgentInputMonitorState {
    pub hold_start_time: Option<Instant>,
    pub agent_started: bool,
    pub hold_threshold_reached: bool,
    pub agent_start_time: Option<Instant>, // Track when agent actually started
    pub force_cleanup_scheduled: bool,
    pub last_cancellation_time: Option<Instant>, // Track when last cancellation occurred
}

impl AgentInputMonitorState {
    pub fn new() -> Self {
        Self {
            hold_start_time: None,
            agent_started: false,
            hold_threshold_reached: false,
            agent_start_time: None,
            force_cleanup_scheduled: false,
            last_cancellation_time: None,
        }
    }

    pub fn start_hold(&mut self) -> bool {
        // Check if we're already in an agent state
        if self.agent_started {
            debug!("[AgentMonitor] Ignoring agent input press - agent already active");
            return false;
        }

        // Check if we're in cooldown period after a recent cancellation
        if let Some(last_cancel) = self.last_cancellation_time {
            let time_since_cancel = last_cancel.elapsed().as_millis();
            if time_since_cancel < COOLDOWN_AFTER_CANCEL_MS as u128 {
                debug!("[AgentMonitor] Ignoring agent input press - still in cooldown period ({}ms since last cancellation)", time_since_cancel);
                return false; // Don't start tracking during cooldown
            }
        }

        self.hold_start_time = Some(Instant::now());
        self.agent_started = false;
        self.hold_threshold_reached = false;
        self.agent_start_time = None;
        self.force_cleanup_scheduled = false;
        debug!("[AgentMonitor] Started tracking agent input hold");
        true // Successfully started tracking
    }

    pub fn end_hold(&mut self) -> (bool, bool, Duration) {
        let agent_was_started = self.agent_started;
        let threshold_was_reached = self.hold_threshold_reached;
        let duration = self.hold_start_time
            .map(|start| start.elapsed())
            .unwrap_or(Duration::ZERO);

        // If agent was started but threshold wasn't reached, it means we're cancelling
        if agent_was_started && !threshold_was_reached {
            self.last_cancellation_time = Some(Instant::now());
            warn!("[AgentMonitor] Cancelling agent after {}ms - recording cancellation time for cooldown", duration.as_millis());
        }

        // Force reset all state immediately to prevent stuck state
        self.hold_start_time = None;
        self.agent_started = false;
        self.hold_threshold_reached = false;
        self.agent_start_time = None;
        self.force_cleanup_scheduled = false;

        debug!(
            "[AgentMonitor] Ended agent input hold tracking, duration: {:?}ms, agent_started: {}, threshold_reached: {}",
            duration.as_millis(), agent_was_started, threshold_was_reached
        );
        (agent_was_started, threshold_was_reached, duration)
    }

    pub fn check_and_start_agent(&mut self) -> bool {
        if self.agent_started {
            return false;
        }

        if let Some(start_time) = self.hold_start_time {
            let duration = start_time.elapsed();
            if duration.as_millis() >= IMMEDIATE_START_MS as u128 {
                self.agent_started = true;
                self.agent_start_time = Some(Instant::now());
                info!("[AgentMonitor] Agent input held for {}ms - starting immediate agent mode", duration.as_millis());
                return true;
            }
        }
        false
    }

    pub fn check_and_reach_threshold(&mut self) -> bool {
        if self.hold_threshold_reached {
            return false;
        }

        if let Some(start_time) = self.hold_start_time {
            let duration = start_time.elapsed();
            if duration.as_millis() >= HOLD_DURATION_MS as u128 {
                self.hold_threshold_reached = true;
                info!("[AgentMonitor] Agent input held for {}ms - threshold reached, committing to Agent Mode", duration.as_millis());
                return true;
            }
        }
        false
    }

    // Check if agent has been running too long and needs forced cleanup
    pub fn check_agent_timeout(&mut self) -> bool {
        if let Some(start_time) = self.agent_start_time {
            let duration = start_time.elapsed();
            if duration.as_millis() >= MAX_AGENT_DURATION_MS as u128 {
                warn!("[AgentMonitor] Agent has been running for {}ms - forcing cleanup", duration.as_millis());
                return true;
            }
        }
        false
    }

    // Check if we need to force cleanup due to stuck state
    pub fn should_force_cleanup(&mut self) -> bool {
        // If agent started but agent input was released and enough time has passed
        if self.agent_started && self.hold_start_time.is_none() && !self.force_cleanup_scheduled {
            if let Some(start_time) = self.agent_start_time {
                let duration = start_time.elapsed();
                if duration.as_millis() >= FORCE_CLEANUP_TIMEOUT_MS as u128 {
                    self.force_cleanup_scheduled = true;
                    warn!("[AgentMonitor] Scheduling force cleanup - agent stuck for {}ms", duration.as_millis());
                    return true;
                }
            }
        }
        false
    }

    // Force reset all state - use when stuck
    pub fn force_reset(&mut self) {
        self.hold_start_time = None;
        self.agent_started = false;
        self.hold_threshold_reached = false;
        self.agent_start_time = None;
        self.force_cleanup_scheduled = false;
        warn!("[AgentMonitor] Force reset agent input state");
    }
}

// Global state for agent input monitoring
static AGENT_INPUT_STATE: tokio::sync::Mutex<AgentInputMonitorState> =
    tokio::sync::Mutex::const_new(AgentInputMonitorState {
        hold_start_time: None,
        agent_started: false,
        hold_threshold_reached: false,
        agent_start_time: None,
        force_cleanup_scheduled: false,
        last_cancellation_time: None,
    });

// Called when agent input key is pressed
pub async fn on_agent_input_pressed() {
    info!("[AgentMonitor] on_agent_input_pressed() called");
    let mut state = AGENT_INPUT_STATE.lock().await;
    let started = state.start_hold();
    if started {
        info!("[AgentMonitor] Agent input pressed down - starting immediate tracking");
    } else {
        info!("[AgentMonitor] Agent input pressed down - ignored (agent active or cooldown period)");
    }
}

// Called when agent input key is released
pub async fn on_agent_input_released(app_handle: &AppHandle) {
    info!("[AgentMonitor] on_agent_input_released() called");
    let mut state = AGENT_INPUT_STATE.lock().await;
    let (agent_started, threshold_reached, duration) = state.end_hold();

    if threshold_reached {
        info!("[AgentMonitor] Agent input released after threshold reached - completing Agent Mode normally");

        // Emit event to stop agent normally
        if let Err(e) = app_handle.emit("agent-stop", ()) {
            error!("[AgentMonitor] Failed to emit agent-stop: {}", e);
        }
    } else if agent_started {
        info!(
            "[AgentMonitor] Agent input released before threshold ({}ms) - cancelling agent",
            duration.as_millis()
        );

        // Emit event to cancel agent
        if let Err(e) = app_handle.emit("agent-cancel", ()) {
            error!("[AgentMonitor] Failed to emit agent-cancel: {}", e);
        }
    } else {
        debug!(
            "[AgentMonitor] Agent input released without starting agent ({}ms) - no action needed",
            duration.as_millis()
        );
    }
}

// Public function to force reset the agent input state (for emergency cleanup)
pub async fn force_reset_agent_input_state() {
    let mut state = AGENT_INPUT_STATE.lock().await;
    state.force_reset();
}

// Background task to monitor agent state and handle timeouts
pub fn start_agent_monitor_task(app_handle: AppHandle) -> tokio::task::JoinHandle<()> {
    info!("[AgentMonitor] Starting background monitoring task");
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(100));

        loop {
            interval.tick().await;

            let mut state = AGENT_INPUT_STATE.lock().await;

            // Check if we should start agent mode
            if state.check_and_start_agent() {
                info!("[AgentMonitor] Background task detected agent should start - emitting agent-transcription-start");
                // Emit event to start agent
                if let Err(e) = app_handle.emit("agent-transcription-start", ()) {
                    error!("[AgentMonitor] Failed to emit agent-transcription-start: {}", e);
                } else {
                    info!("[AgentMonitor] Successfully emitted agent-transcription-start event");
                }
            }

            // Check if we should reach threshold
            if state.check_and_reach_threshold() {
                // Emit event for threshold reached
                if let Err(e) = app_handle.emit("agent-committed", ()) {
                    error!("[AgentMonitor] Failed to emit agent-committed: {}", e);
                }
            }

            // Check for timeouts
            if state.check_agent_timeout() {
                if let Err(e) = app_handle.emit("agent-force-stop", ()) {
                    error!("[AgentMonitor] Failed to emit agent-force-stop: {}", e);
                }
            }

            // Check for stuck state cleanup
            if state.should_force_cleanup() {
                if let Err(e) = app_handle.emit("agent-force-cleanup", ()) {
                    error!("[AgentMonitor] Failed to emit agent-force-cleanup: {}", e);
                }
            }
        }
    })
}

// Check if agent should handle key press based on trigger mode
pub async fn should_handle_agent_key(app_handle: &AppHandle, key_state: &str) -> bool {
    let app_state = app_handle.state::<AppState>();

    let trigger_mode = app_state.agent_trigger_mode.lock()
        .map(|mode| mode.clone())
        .unwrap_or(AgentTriggerMode::Tap);

    match trigger_mode {
        AgentTriggerMode::Tap => {
            // Only handle key release (press+release = tap)
            key_state == "released"
        },
        AgentTriggerMode::Hold => {
            // Handle both press and release for hold behavior
            true
        }
    }
}
