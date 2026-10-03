use crate::constants::{events, monitor_sessions};
use crate::state::{AgentTriggerMode, AppState};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};
use tracing::{debug, error, info, warn};

/// What a release meant for the agent input monitor. Mirrors dictation's
/// `HoldRelease` so a release reads the same way on both targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentRelease {
    /// Held past the threshold: the transcription is stopped and handed to
    /// the agent for processing.
    Committed,
    /// A short tap that opened the microphone but did not commit: the agent
    /// session is cancelled.
    Cancelled,
    /// Release without a preceding transcription start: for `AgentTriggerMode::Tap`
    /// this fired the toggle path; for `Hold` nothing happened.
    Idle,
}

/// Generation counter to prevent race conditions between start and cancel.
/// Incremented when an agent session starts and when it's cancelled, so that
/// async handlers can detect if their session was invalidated mid-flight.
static AGENT_SESSION_GENERATION: AtomicU64 = AtomicU64::new(0);

/// True while a spoken query started from the floating bar's mic is open.
/// The bar mic bypasses the hold/tap monitor, so this flag lets the agent
/// and dictation shortcuts (and the bar's Stop control) end that session
/// instead of trying to start a second one on top of it.
static BAR_VOICE_ACTIVE: AtomicBool = AtomicBool::new(false);

/// Is a bar-initiated spoken query currently open?
pub fn bar_voice_active() -> bool {
    BAR_VOICE_ACTIVE.load(Ordering::SeqCst)
}

/// Mark a bar-initiated spoken query as open or closed.
pub fn set_bar_voice_active(active: bool) {
    BAR_VOICE_ACTIVE.store(active, Ordering::SeqCst);
}

/// Returns the current agent session generation.
pub fn current_agent_generation() -> u64 {
    AGENT_SESSION_GENERATION.load(Ordering::SeqCst)
}

/// Increments and returns the new agent session generation.
fn increment_agent_generation() -> u64 {
    let new_gen = AGENT_SESSION_GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
    debug!("[AgentMonitor] Generation incremented to {}", new_gen);
    new_gen
}

// Configuration constants
// const HOLD_DURATION_MS: u64 = 500; // Hold agent key for 500ms to commit to agent mode
// const IMMEDIATE_START_MS: u64 = 0; // Start agent immediately (0ms delay)
// const MAX_AGENT_DURATION_MS: u64 = 120_000; // 2 minutes max agent session time
// const FORCE_CLEANUP_TIMEOUT_MS: u64 = 5_000; // 5 seconds to force cleanup if stuck
// const COOLDOWN_AFTER_CANCEL_MS: u64 = 150; // 150ms cooldown for better responsiveness

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

#[allow(clippy::new_without_default)]
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
            if time_since_cancel < monitor_sessions::COOLDOWN_AFTER_CANCEL_MS as u128 {
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
        let duration = self
            .hold_start_time
            .map(|start| start.elapsed())
            .unwrap_or(Duration::ZERO);

        // A short tap that opened the microphone but did not commit is a
        // cancellation: bump the generation so any in-flight start handler
        // knows its session was invalidated, and arm the cooldown so a bounce
        // cannot immediately reopen.
        if agent_was_started && !threshold_was_reached {
            let gen = increment_agent_generation();
            self.last_cancellation_time = Some(Instant::now());
            debug!(
                "[AgentMonitor] Short-tap cancel invalidated session (generation={})",
                gen
            );
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

    /// True while the monitor is watching a held key.
    pub fn is_tracking_hold(&self) -> bool {
        self.hold_start_time.is_some() || self.agent_started
    }

    pub fn check_and_start_agent(&mut self) -> bool {
        if self.agent_started {
            return false;
        }

        if let Some(start_time) = self.hold_start_time {
            let duration = start_time.elapsed();
            if duration.as_millis() >= monitor_sessions::IMMEDIATE_START_MS as u128 {
                self.agent_started = true;
                self.agent_start_time = Some(Instant::now());
                let gen = increment_agent_generation();
                info!(
                    "[AgentMonitor] Agent input held for {}ms - starting immediate agent mode (generation={})",
                    duration.as_millis(), gen
                );
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
            if duration.as_millis() >= monitor_sessions::HOLD_DURATION_MS as u128 {
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
            if duration.as_millis() >= monitor_sessions::MAX_AGENT_DURATION_MS as u128 {
                warn!(
                    "[AgentMonitor] Agent has been running for {}ms - forcing cleanup",
                    duration.as_millis()
                );
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
                if duration.as_millis() >= monitor_sessions::FORCE_CLEANUP_TIMEOUT_MS as u128 {
                    self.force_cleanup_scheduled = true;
                    warn!(
                        "[AgentMonitor] Scheduling force cleanup - agent stuck for {}ms",
                        duration.as_millis()
                    );
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
        info!(
            "[AgentMonitor] Agent input pressed down - ignored (agent active or cooldown period)"
        );
    }
}

// Called when agent input key is released. Uses the target's configured
// trigger mode from state.
pub async fn on_agent_input_released(app_handle: &AppHandle) -> AgentRelease {
    let trigger_mode = app_handle
        .state::<AppState>()
        .get_agent_trigger_mode()
        .unwrap_or(AgentTriggerMode::Tap);
    on_agent_input_released_with_mode(app_handle, trigger_mode).await
}

/// Release handler with an explicit trigger mode, so a specific trigger
/// (push-to-talk vs toggle) drives the behavior regardless of the global
/// setting. This lets the unified triggers matrix bind both methods. Returns
/// what the release meant.
pub async fn on_agent_input_released_with_mode(
    app_handle: &AppHandle,
    trigger_mode: AgentTriggerMode,
) -> AgentRelease {
    info!("[AgentMonitor] on_agent_input_released() called");
    let mut state = AGENT_INPUT_STATE.lock().await;
    let (agent_started, threshold_reached, duration) = state.end_hold();

    if threshold_reached {
        info!("[AgentMonitor] Agent input released after threshold reached - stopping transcription to process with agent");

        // When threshold is reached, we want to stop the transcription so the final result
        // gets processed by the voice-transcription:final-result handler, which will
        // send the transcribed text to the agent for processing
        if let Err(e) = app_handle.emit(events::agent::TRANSCRIPTION_STOP, ()) {
            error!(
                "[AgentMonitor] Failed to emit agent-transcription-stop: {}",
                e
            );
        }
        AgentRelease::Committed
    } else if agent_started {
        // A short tap on a hold key: cancel the spoken query the press
        // opened.
        info!(
            "[AgentMonitor] Tapped ({}ms) - cancelling spoken query",
            duration.as_millis()
        );
        if let Err(e) = app_handle.emit(events::agent::CANCEL, ()) {
            error!("[AgentMonitor] Failed to emit agent-cancel: {}", e);
        }
        AgentRelease::Cancelled
    } else {
        // Tap mode: no start yet on press; release should initiate agent transcription
        if matches!(trigger_mode, AgentTriggerMode::Tap) {
            info!("[AgentMonitor] Tap trigger: starting agent transcription on release");
            // Mark the agent voice session open so the NEXT press of this key
            // is caught by the stop-guard in `fire_key_edge` and finalizes the
            // query, instead of starting a second session. Cleared when the
            // session ends in handle_agent_transcription_stop / _cancel.
            set_bar_voice_active(true);
            // Say how this was triggered, so the session records its method at
            // birth instead of a stop path inferring it later.
            if let Err(e) = app_handle.emit(
                events::agent::TRANSCRIPTION_START,
                serde_json::json!({ "method": "toggle" }),
            ) {
                error!(
                    "[AgentMonitor] Failed to emit agent-transcription-start: {}",
                    e
                );
                set_bar_voice_active(false);
            }
        }
        debug!(
            "[AgentMonitor] Agent input released without starting agent ({}ms) - no action needed",
            duration.as_millis()
        );
        AgentRelease::Idle
    }
}

/// The floating bar's mic button. Drives the same event pipeline the agent
/// hotkey does (`agent-transcription-start` / `-stop`), so a spoken query from
/// the bar is transcribed and handed to the agent exactly like a hotkey one.
/// `start` opens the microphone, `stop` closes it and sends what was said, and
/// `cancel` closes it and throws what was said away. The bar only ever had the
/// first two, which is why its one control said "Stop" and submitted.
#[tauri::command]
pub async fn agent_voice(app: AppHandle, action: String) -> Result<(), String> {
    let event = match action.as_str() {
        "start" => events::agent::TRANSCRIPTION_START,
        "stop" => events::agent::TRANSCRIPTION_STOP,
        "cancel" => events::agent::CANCEL,
        other => return Err(format!("unknown agent_voice action: {other}")),
    };
    info!("[AgentMonitor] agent_voice({action}) from the bar");
    set_bar_voice_active(action == "start");
    // A start says how it was triggered so the session can record it. Stop and
    // cancel carry no method: they act on whatever session is standing.
    let payload = if action == "start" {
        serde_json::json!({ "method": "mouse" })
    } else {
        serde_json::Value::Null
    };
    app.emit(event, payload).map_err(|e| e.to_string())
}

// Public function to force reset the agent input state
pub async fn force_reset_agent_input_state() {
    let mut state = AGENT_INPUT_STATE.lock().await;
    state.force_reset();
}

// Background task to monitor agent state and handle timeouts
pub fn start_agent_monitor_task(app_handle: AppHandle) -> tauri::async_runtime::JoinHandle<()> {
    info!("[AgentMonitor] Starting background monitoring task");
    tauri::async_runtime::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(100));

        loop {
            interval.tick().await;

            let mut state = AGENT_INPUT_STATE.lock().await;

            // Check if we should start agent mode
            if state.check_and_start_agent() {
                info!("[AgentMonitor] Background task detected agent should start - emitting agent-transcription-start");
                // Emit event to start agent. This is the held-key path, so the
                // session records push_to_talk rather than leaving the method
                // unstated for a stop path to guess at.
                if let Err(e) = app_handle.emit(
                    events::agent::TRANSCRIPTION_START,
                    serde_json::json!({ "method": "push_to_talk" }),
                ) {
                    error!(
                        "[AgentMonitor] Failed to emit agent-transcription-start: {}",
                        e
                    );
                } else {
                    info!("[AgentMonitor] Successfully emitted agent-transcription-start event");
                }
            }

            // Check if we should reach threshold
            if state.check_and_reach_threshold() {
                // Emit event for threshold reached
                if let Err(e) = app_handle.emit(events::agent::COMMITTED, ()) {
                    error!("[AgentMonitor] Failed to emit agent-committed: {}", e);
                }
            }

            // Check for timeouts
            if state.check_agent_timeout() {
                if let Err(e) = app_handle.emit(events::agent::FORCE_STOP, ()) {
                    error!("[AgentMonitor] Failed to emit agent-force-stop: {}", e);
                }
            }

            // Check for stuck state cleanup
            if state.should_force_cleanup() {
                if let Err(e) = app_handle.emit(events::agent::FORCE_CLEANUP, ()) {
                    error!("[AgentMonitor] Failed to emit agent-force-cleanup: {}", e);
                }
            }
        }
    })
}

// Check if agent should handle key press based on trigger mode
pub async fn should_handle_agent_key(app_handle: &AppHandle, key_state: &str) -> bool {
    let app_state = app_handle.state::<AppState>();

    let trigger_mode = app_state
        .get_agent_trigger_mode()
        .unwrap_or(AgentTriggerMode::Tap);

    match trigger_mode {
        AgentTriggerMode::Tap => {
            // Only handle key release (press+release = tap)
            key_state == "released"
        }
        AgentTriggerMode::Hold => {
            // Handle both press and release for hold behavior
            true
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn held_for_ms(state: &mut AgentInputMonitorState, ms: u64) {
        state.hold_start_time = Some(Instant::now() - Duration::from_millis(ms));
    }

    #[test]
    fn a_short_tap_cancels_the_spoken_query() {
        let mut state = AgentInputMonitorState::new();
        assert!(state.start_hold());
        held_for_ms(&mut state, 120);
        assert!(state.check_and_start_agent());
        assert!(
            !state.check_and_reach_threshold(),
            "120ms is a tap, not a hold"
        );

        let start_gen = current_agent_generation();
        let (agent_started, threshold_reached, _) = state.end_hold();
        assert!(agent_started);
        assert!(!threshold_reached);
        assert!(
            current_agent_generation() > start_gen,
            "the tap-cancel must bump the generation so a late start handler knows its session is dead"
        );
        assert!(
            state.last_cancellation_time.is_some(),
            "the cooldown arms so a bounce cannot immediately reopen"
        );
    }

    #[test]
    fn a_hold_past_the_threshold_commits_on_release() {
        let mut state = AgentInputMonitorState::new();
        assert!(state.start_hold());
        held_for_ms(&mut state, monitor_sessions::HOLD_DURATION_MS + 50);
        assert!(state.check_and_start_agent());
        assert!(state.check_and_reach_threshold());

        let (_agent_started, threshold_reached, _) = state.end_hold();
        assert!(threshold_reached);
        assert!(
            state.last_cancellation_time.is_none(),
            "a commit is not a cancel"
        );
    }

    #[test]
    fn cooldown_blocks_an_immediate_reopen_after_a_cancel() {
        let mut state = AgentInputMonitorState::new();
        assert!(state.start_hold());
        held_for_ms(&mut state, 120);
        assert!(state.check_and_start_agent());
        let _ = state.end_hold();

        // A press inside the cooldown window is refused.
        assert!(!state.start_hold());
    }

    #[test]
    fn is_tracking_hold_reflects_state() {
        let mut state = AgentInputMonitorState::new();
        assert!(!state.is_tracking_hold());
        assert!(state.start_hold());
        assert!(state.is_tracking_hold());
        let _ = state.end_hold();
        assert!(!state.is_tracking_hold());
    }
}
