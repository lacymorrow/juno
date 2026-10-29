use crate::constants::{events, monitor_sessions};
use crate::hold_gate::{self, HoldEnd, HoldId};
use crate::state::{AgentTriggerMode, AppState, VoiceTarget};
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

// Nothing here runs on a timer. The press opens the session and schedules the
// hold's deadlines (see `schedule_hold_deadlines`); the release ends it.

// State for agent input monitoring
#[derive(Debug)]
pub struct AgentInputMonitorState {
    pub hold_start_time: Option<Instant>,
    pub agent_started: bool,
    pub hold_threshold_reached: bool,
    pub last_cancellation_time: Option<Instant>, // Track when last cancellation occurred
    /// Which press this state belongs to. Bumped on every accepted press, so a
    /// deadline scheduled for one hold can never act on a later one. Separate
    /// from `AGENT_SESSION_GENERATION`, which tracks sessions, not presses.
    pub hold_generation: u64,
    /// The start this hold emitted, so its release can wait for it to land
    /// (see [`crate::hold_gate`]).
    pub hold: Option<HoldId>,
}

#[allow(clippy::new_without_default)]
impl AgentInputMonitorState {
    pub fn new() -> Self {
        Self {
            hold_start_time: None,
            agent_started: false,
            hold_threshold_reached: false,
            last_cancellation_time: None,
            hold_generation: 0,
            hold: None,
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
        self.hold_generation = self.hold_generation.wrapping_add(1);
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
        self.hold = None;
        self.agent_started = false;
        self.hold_threshold_reached = false;

        debug!(
            "[AgentMonitor] Ended agent input hold tracking, duration: {:?}ms, agent_started: {}, threshold_reached: {}",
            duration.as_millis(), agent_was_started, threshold_was_reached
        );
        (agent_was_started, threshold_was_reached, duration)
    }

    /// Start the agent if the hold has lasted long enough, opening a gate
    /// entry for it. Returns the id the start event must carry.
    pub fn open_start(&mut self, open: impl FnOnce() -> HoldId) -> Option<HoldId> {
        if !self.check_and_start_agent() {
            return None;
        }
        let id = open();
        self.hold = Some(id);
        Some(id)
    }

    /// The key came up. Returns `(agent_started, threshold_reached, duration,
    /// deferred)`, where `deferred` says the release has to wait for its start
    /// to land before it is acted on. `defer` is [`hold_gate::defer`] for the
    /// agent; tests pass a local gate.
    pub fn release(
        &mut self,
        defer: impl FnOnce(HoldId, HoldEnd) -> bool,
    ) -> (bool, bool, Duration, bool) {
        let hold = self.hold.take();
        let (agent_started, threshold_reached, duration) = self.end_hold();
        let end = if threshold_reached {
            Some(HoldEnd::Commit)
        } else if agent_started {
            Some(HoldEnd::Cancel)
        } else {
            None
        };
        let deferred = match (hold, end) {
            (Some(id), Some(end)) => defer(id, end),
            _ => false,
        };
        (agent_started, threshold_reached, duration, deferred)
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

    /// The hold deadline for `generation` fired. Commits that hold if its key
    /// is still down, and only once. A deadline from an earlier press, or one
    /// that lands after the release, does nothing: the release already decided
    /// the hold was a tap.
    pub fn reach_threshold(&mut self, generation: u64) -> bool {
        if self.hold_generation != generation
            || self.hold_start_time.is_none()
            || self.hold_threshold_reached
        {
            return false;
        }
        self.hold_threshold_reached = true;
        let held = self
            .hold_start_time
            .map(|start| start.elapsed().as_millis())
            .unwrap_or_default();
        info!(
            "[AgentMonitor] Agent input held for {}ms - threshold reached, committing to Agent Mode",
            held
        );
        true
    }

    /// The max-duration deadline for `generation` fired. True when that same
    /// hold is still open, which means the key has been down for
    /// `MAX_AGENT_DURATION_MS` or its release never arrived.
    pub fn session_overran(&self, generation: u64) -> bool {
        self.hold_generation == generation && self.agent_started
    }

    // Force reset all state - use when stuck
    pub fn force_reset(&mut self) {
        self.hold_start_time = None;
        self.hold = None;
        self.agent_started = false;
        self.hold_threshold_reached = false;
        warn!("[AgentMonitor] Force reset agent input state");
    }
}

// Global state for agent input monitoring
static AGENT_INPUT_STATE: tokio::sync::Mutex<AgentInputMonitorState> =
    tokio::sync::Mutex::const_new(AgentInputMonitorState {
        hold_start_time: None,
        agent_started: false,
        hold_threshold_reached: false,
        last_cancellation_time: None,
        hold_generation: 0,
        hold: None,
    });

/// Open the agent session for the hold that just started. Idempotent: only
/// the first call for a hold opens.
fn emit_agent_start_if_due(app_handle: &AppHandle, state: &mut AgentInputMonitorState) {
    if let Some(hold) = state.open_start(|| hold_gate::open(VoiceTarget::Agent)) {
        // This is the held-key path, so the session records push_to_talk
        // rather than leaving the method unstated for a stop path to guess at.
        if let Err(e) = app_handle.emit(
            events::agent::TRANSCRIPTION_START,
            serde_json::json!({ "method": "push_to_talk", "hold": hold }),
        ) {
            error!(
                "[AgentMonitor] Failed to emit agent-transcription-start: {}",
                e
            );
            // No start is coming to settle it, so a release must not wait for
            // one.
            let _ = hold_gate::settle(VoiceTarget::Agent, hold);
        }
    }
}

/// Schedule the two moments a hold can reach on its own, measured from the
/// press: the hold threshold, where a tap becomes a hold, and the max session
/// length. Each fires once, for this press only; a release or a later press
/// turns it into a no-op (see `reach_threshold` and `session_overran`).
fn schedule_hold_deadlines(app_handle: &AppHandle, generation: u64) {
    let app_handle = app_handle.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_millis(monitor_sessions::HOLD_DURATION_MS)).await;
        {
            let mut state = AGENT_INPUT_STATE.lock().await;
            if !state.reach_threshold(generation) {
                // Released first (a tap) or replaced by a later press: the
                // release has already settled this hold.
                return;
            }
            // The tap just became a hold: this is the moment the person
            // hears the start, not the press and not the microphone opening.
            crate::commands::sound::play_hold_cue(&app_handle, VoiceTarget::Agent);
            if let Err(e) = app_handle.emit(events::agent::COMMITTED, ()) {
                error!("[AgentMonitor] Failed to emit agent-committed: {}", e);
            }
        }

        // Safety net: a hold that outlives the max session length is stopped.
        // It covers a key genuinely held that long and a release event that
        // never arrived, which would otherwise leave the microphone open. The
        // force-stop handler resets this monitor's state.
        tokio::time::sleep(Duration::from_millis(
            monitor_sessions::MAX_AGENT_DURATION_MS
                .saturating_sub(monitor_sessions::HOLD_DURATION_MS),
        ))
        .await;
        let overran = AGENT_INPUT_STATE.lock().await.session_overran(generation);
        if !overran {
            return;
        }
        warn!(
            "[AgentMonitor] Hold still open after {}ms - forcing stop",
            monitor_sessions::MAX_AGENT_DURATION_MS
        );
        if let Err(e) = app_handle.emit(events::agent::FORCE_STOP, ()) {
            error!("[AgentMonitor] Failed to emit agent-force-stop: {}", e);
        }
    });
}

// Called when agent input key is pressed
pub async fn on_agent_input_pressed(app_handle: &AppHandle) {
    info!("[AgentMonitor] on_agent_input_pressed() called");
    let mut state = AGENT_INPUT_STATE.lock().await;
    let started = state.start_hold();
    if started {
        info!("[AgentMonitor] Agent input pressed down - starting immediate tracking");
        // The microphone opens on the press; the threshold and the max
        // session length are deadlines from it.
        emit_agent_start_if_due(app_handle, &mut state);
        schedule_hold_deadlines(app_handle, state.hold_generation);
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
    let (agent_started, threshold_reached, duration, deferred) =
        state.release(|id, end| hold_gate::defer(VoiceTarget::Agent, id, end));

    if deferred {
        // The start this hold emitted is still opening the microphone. Acting
        // now would race it: the stop or cancel would find nothing to end, and
        // the start would then open a microphone no release is coming for. The
        // start handler performs this release when it lands.
        info!(
            "[AgentMonitor] Released ({}ms) before the session finished starting; the release waits for it",
            duration.as_millis()
        );
        return if threshold_reached {
            AgentRelease::Committed
        } else {
            AgentRelease::Cancelled
        };
    }

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

        // Released at 120ms, before the threshold deadline fires.
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
        assert!(state.reach_threshold(state.hold_generation));

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
    fn a_release_before_the_agent_start_lands_is_applied_when_it_lands() {
        let mut state = AgentInputMonitorState::new();
        let mut gate = hold_gate::StartGate::default();
        assert!(state.start_hold());
        held_for_ms(&mut state, 50);
        let hold = state
            .open_start(|| gate.open())
            .expect("the hold has lasted past IMMEDIATE_START_MS");

        let (started, committed, _, deferred) = state.release(|id, end| gate.defer(id, end));
        assert!(started && !committed);
        assert!(deferred, "the cancel must not race the start it ends");
        assert!(!state.is_tracking_hold());

        assert_eq!(gate.settle(hold), Some(HoldEnd::Cancel));
        assert_eq!(
            hold_gate::release_event(VoiceTarget::Agent, HoldEnd::Cancel),
            events::agent::CANCEL
        );

        // The next hold after the cooldown starts and ends normally.
        cooldown_elapsed(&mut state);
        assert!(state.start_hold(), "the next hold is not refused");
        held_for_ms(&mut state, monitor_sessions::HOLD_DURATION_MS + 50);
        let next = state.open_start(|| gate.open()).expect("it starts");
        assert!(state.reach_threshold(state.hold_generation));
        assert_eq!(gate.settle(next), None);
        let (_, committed, _, deferred) = state.release(|id, end| gate.defer(id, end));
        assert!(committed);
        assert!(!deferred, "a landed start is stopped at once");
    }

    #[test]
    fn a_tap_mode_release_is_never_deferred() {
        let mut state = AgentInputMonitorState::new();
        let mut gate = hold_gate::StartGate::default();
        let (started, committed, _, deferred) = state.release(|id, end| gate.defer(id, end));
        assert!(!started && !committed && !deferred);
    }

    fn cooldown_elapsed(state: &mut AgentInputMonitorState) {
        state.last_cancellation_time = Instant::now().checked_sub(Duration::from_millis(
            monitor_sessions::COOLDOWN_AFTER_CANCEL_MS + 10,
        ));
    }

    #[test]
    fn a_deadline_from_an_earlier_hold_does_not_commit_a_later_one() {
        let mut state = AgentInputMonitorState::new();
        assert!(state.start_hold());
        let first = state.hold_generation;
        assert!(state.check_and_start_agent());
        let (_, committed, _) = state.end_hold();
        assert!(!committed);

        // A second press inside the first one's 400ms window.
        cooldown_elapsed(&mut state);
        assert!(state.start_hold());
        let second = state.hold_generation;
        assert_ne!(first, second);

        assert!(
            !state.reach_threshold(first),
            "the first hold's deadline must not commit the second hold"
        );
        assert!(!state.hold_threshold_reached);
        assert!(!state.session_overran(first));
        assert!(state.reach_threshold(second), "its own deadline does");
    }

    #[test]
    fn a_release_before_the_threshold_is_a_tap_and_the_late_deadline_is_a_no_op() {
        let mut state = AgentInputMonitorState::new();
        assert!(state.start_hold());
        let generation = state.hold_generation;
        assert!(state.check_and_start_agent());
        let (started, committed, _) = state.end_hold();
        assert!(started && !committed, "a tap");

        // The deadline fires after the release.
        assert!(!state.reach_threshold(generation));
        assert!(!state.hold_threshold_reached);
        assert!(!state.is_tracking_hold());
        assert!(
            !state.session_overran(generation),
            "the max-duration deadline is a no-op once released"
        );
    }

    #[test]
    fn the_threshold_commits_exactly_once() {
        let mut state = AgentInputMonitorState::new();
        assert!(state.start_hold());
        let generation = state.hold_generation;
        assert!(state.check_and_start_agent());
        assert!(state.reach_threshold(generation));
        assert!(
            !state.reach_threshold(generation),
            "a second firing must not commit again"
        );
        assert!(
            state.session_overran(generation),
            "a hold still open at the max-duration deadline is stopped"
        );
        let (_, committed, _) = state.end_hold();
        assert!(committed);
        assert!(!state.session_overran(generation));
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
