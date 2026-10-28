use crate::constants::{events, monitor_sessions};
use crate::hold_gate::{self, HoldEnd, HoldId};
use crate::state::{VoiceStartMethod, VoiceTarget};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::Mutex;
use tracing::{debug, error, info, warn};

// Nothing here runs on a timer. The press opens the session and schedules the
// hold's deadlines (see `schedule_hold_deadlines`); the release ends it. A
// start that waited for a delay would need something to come back and fire
// it, so the press is the only start there is and `IMMEDIATE_START_MS` must
// stay 0 (a test holds it there).

// State for dictation input monitoring
#[derive(Debug)]
pub struct DictationInputMonitorState {
    pub hold_start_time: Option<Instant>,
    pub transcription_started: bool,
    pub hold_threshold_reached: bool,
    pub passthrough_scheduled: bool,
    pub last_cancellation_time: Option<Instant>, // Track when last cancellation occurred
    /// Which press this state belongs to. Bumped on every accepted press, so a
    /// deadline scheduled for one hold can never act on a later one.
    pub hold_generation: u64,
    /// The start this hold emitted, so its release can wait for it to land
    /// (see [`crate::hold_gate`]).
    pub hold: Option<HoldId>,
}

/// What letting go of the key meant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HoldRelease {
    /// Held past the threshold: the session ends and the words are typed.
    Committed,
    /// A short tap that opened the microphone but did not commit: the session
    /// is cancelled and nothing is typed.
    Cancelled,
    /// Nothing had started, so nothing happens.
    Nothing,
}

#[allow(clippy::new_without_default)]
impl DictationInputMonitorState {
    pub fn new() -> Self {
        Self {
            hold_start_time: None,
            transcription_started: false,
            hold_threshold_reached: false,
            passthrough_scheduled: false,
            last_cancellation_time: None,
            hold_generation: 0,
            hold: None,
        }
    }

    pub fn start_hold(&mut self) -> bool {
        // Check if we're already in a transcription state
        if self.transcription_started {
            warn!(
                "[DictationMonitor] Ignoring dictation input press - transcription already active"
            );
            return false;
        }

        // Check if we're in cooldown period after a recent cancellation
        if let Some(last_cancel) = self.last_cancellation_time {
            let time_since_cancel = last_cancel.elapsed().as_millis();
            if time_since_cancel < monitor_sessions::COOLDOWN_AFTER_CANCEL_MS as u128 {
                warn!("[DictationMonitor] Ignoring dictation input press - still in cooldown period ({}ms since last cancellation)", time_since_cancel);
                return false; // Don't start tracking during cooldown
            }
            // Cooldown expired, clear it
            info!("[DictationMonitor] Cooldown expired ({}ms since last cancellation), allowing new dictation", time_since_cancel);
            self.last_cancellation_time = None;
        }

        self.hold_start_time = Some(Instant::now());
        self.transcription_started = false;
        self.hold_threshold_reached = false;
        self.passthrough_scheduled = false;
        self.hold_generation = self.hold_generation.wrapping_add(1);
        info!("[DictationMonitor] Started tracking dictation input hold");
        true // Successfully started tracking
    }

    pub fn end_hold(&mut self) -> (HoldRelease, Duration) {
        let transcription_was_started = self.transcription_started;
        let threshold_was_reached = self.hold_threshold_reached;
        let duration = self
            .hold_start_time
            .map(|start| start.elapsed())
            .unwrap_or(Duration::ZERO);

        // Force reset all state immediately to prevent stuck state
        self.hold_start_time = None;
        self.hold = None;
        self.transcription_started = false;
        self.hold_threshold_reached = false;
        self.passthrough_scheduled = false;

        let outcome = if threshold_was_reached {
            HoldRelease::Committed
        } else if transcription_was_started {
            // The microphone opened, but the release came before the hold
            // threshold. Cancel the session and start a cooldown so a bounce
            // does not immediately reopen it.
            self.last_cancellation_time = Some(Instant::now());
            HoldRelease::Cancelled
        } else {
            HoldRelease::Nothing
        };

        debug!(
            "[DictationMonitor] Ended dictation input hold tracking, duration: {:?}ms, transcription_started: {}, threshold_reached: {}, outcome: {:?}",
            duration.as_millis(), transcription_was_started, threshold_was_reached, outcome
        );
        (outcome, duration)
    }

    pub fn check_and_start_transcription(&mut self) -> bool {
        if self.transcription_started {
            return false;
        }

        if let Some(start_time) = self.hold_start_time {
            let duration = start_time.elapsed();
            if duration.as_millis() >= monitor_sessions::IMMEDIATE_START_MS as u128 {
                self.transcription_started = true;
                info!("[DictationMonitor] Dictation input held for {}ms - starting immediate transcription", duration.as_millis());
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
            "[DictationMonitor] Dictation input held for {}ms - threshold reached, committing to Dictation Mode",
            held
        );
        true
    }

    /// The max-duration deadline for `generation` fired. True when that same
    /// hold is still open, which means the key has been down for
    /// `MAX_TRANSCRIPTION_DURATION_MS` or its release never arrived.
    pub fn session_overran(&self, generation: u64) -> bool {
        self.hold_generation == generation && self.transcription_started
    }

    // Force reset all state - use when stuck
    pub fn force_reset(&mut self) {
        warn!("[DictationMonitor] Force resetting all dictation input state");
        self.hold_start_time = None;
        self.hold = None;
        self.transcription_started = false;
        self.hold_threshold_reached = false;
        self.passthrough_scheduled = false;
        self.last_cancellation_time = None; // Clear cooldown tracking on reset
    }

    /// Start transcription if the hold has lasted long enough, opening a gate
    /// entry for it. Returns the id the start event must carry.
    pub fn open_start(&mut self, open: impl FnOnce() -> HoldId) -> Option<HoldId> {
        if !self.check_and_start_transcription() {
            return None;
        }
        let id = open();
        self.hold = Some(id);
        Some(id)
    }

    /// The key came up. Returns what the release meant and whether it has to
    /// wait for its start to land before it is acted on. `defer` is
    /// [`hold_gate::defer`] for this target; tests pass a local gate.
    pub fn release(
        &mut self,
        defer: impl FnOnce(HoldId, HoldEnd) -> bool,
    ) -> (HoldRelease, Duration, bool) {
        let hold = self.hold.take();
        let (outcome, duration) = self.end_hold();
        let end = match outcome {
            HoldRelease::Committed => Some(HoldEnd::Commit),
            HoldRelease::Cancelled => Some(HoldEnd::Cancel),
            HoldRelease::Nothing => None,
        };
        let deferred = match (hold, end) {
            (Some(id), Some(end)) => defer(id, end),
            _ => false,
        };
        (outcome, duration, deferred)
    }

    /// True while the monitor is watching a held key.
    pub fn is_tracking_hold(&self) -> bool {
        self.hold_start_time.is_some() || self.transcription_started
    }
}

// Global state for the dictation input monitor
static DICTATION_INPUT_STATE: once_cell::sync::Lazy<Arc<Mutex<DictationInputMonitorState>>> =
    once_cell::sync::Lazy::new(|| Arc::new(Mutex::new(DictationInputMonitorState::new())));

/// Open the session for the hold that just started. Idempotent: only the
/// first call for a hold opens.
fn emit_start_if_due(app_handle: &AppHandle, state: &mut DictationInputMonitorState) {
    if let Some(hold) = state.open_start(|| hold_gate::open(VoiceTarget::Dictation)) {
        // The payload says how this session is being triggered: this monitor
        // only ever watches a held key or button, so the session it opens is
        // a push-to-talk one and says so rather than leaving the start
        // handler to work it out.
        // The capture this press opened, carried the same way the hold id is,
        // so every stage the start records lands on this press's line and not
        // on a later press's. A start that arrives without one records no
        // timing at all rather than guessing. See `capture_timing`.
        let capture = tauri_plugin_voice_transcription::capture_timing::open_id();
        if let Err(e) = app_handle.emit(
            events::dictation::TRANSCRIPTION_START,
            serde_json::json!({
                "method": VoiceStartMethod::PushToTalk.as_wire(),
                "hold": hold,
                "capture": capture,
            }),
        ) {
            error!(
                "[DictationMonitor] Failed to emit dictation-transcription-start: {}",
                e
            );
            // No start is coming to settle it, so a release must not wait
            // for one.
            let _ = hold_gate::settle(VoiceTarget::Dictation, hold);
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
            let mut state = DICTATION_INPUT_STATE.lock().await;
            if !state.reach_threshold(generation) {
                // Released first (a tap) or replaced by a later press: the
                // release has already settled this hold.
                return;
            }
            if let Err(e) = app_handle.emit(events::dictation::COMMITTED, ()) {
                error!(
                    "[DictationMonitor] Failed to emit dictation-committed: {}",
                    e
                );
            }
        }

        // Safety net: a hold that outlives the max session length is stopped.
        // It covers a key genuinely held that long and a release event that
        // never arrived, which would otherwise leave the microphone open.
        tokio::time::sleep(Duration::from_millis(
            monitor_sessions::MAX_TRANSCRIPTION_DURATION_MS
                .saturating_sub(monitor_sessions::HOLD_DURATION_MS),
        ))
        .await;
        let mut state = DICTATION_INPUT_STATE.lock().await;
        if !state.session_overran(generation) {
            return;
        }
        warn!(
            "[DictationMonitor] Hold still open after {}ms - forcing stop",
            monitor_sessions::MAX_TRANSCRIPTION_DURATION_MS
        );
        if let Err(e) = app_handle.emit(events::dictation::TRANSCRIPTION_FORCE_STOP, ()) {
            error!(
                "[DictationMonitor] Failed to emit dictation-transcription-force-stop: {}",
                e
            );
        }

        // Force cleanup of app state
        let app_state = app_handle.state::<crate::state::AppState>();
        if let Err(e) = app_state.set_dictation_active(false) {
            warn!("Failed to reset dictation active state: {}", e);
        }

        state.force_reset();
    });
}

// Called when dictation input key is pressed down
pub async fn on_dictation_input_pressed(app_handle: &AppHandle) {
    info!("[DictationMonitor] on_dictation_input_pressed called");
    let mut state = DICTATION_INPUT_STATE.lock().await;

    info!("[DictationMonitor] Acquired lock, calling start_hold");
    let started = state.start_hold();
    if started {
        // No start cue here. The press starts the microphone; the cue, like
        // the pill, follows the event that says it is running
        // (`show_live_microphone` in integration.rs), so nothing is heard
        // before the recording is.
        info!("[DictationMonitor] Dictation input pressed down - starting immediate tracking");
        // The microphone opens on the press; the threshold and the max
        // session length are deadlines from it.
        emit_start_if_due(app_handle, &mut state);
        schedule_hold_deadlines(app_handle, state.hold_generation);
    } else {
        warn!("[DictationMonitor] Dictation input pressed down - ignored (transcription_started={}, last_cancel={:?})",
              state.transcription_started,
              state.last_cancellation_time.map(|t| t.elapsed().as_millis()));
    }
}

// Called when dictation input key is released. Returns what the release meant.
pub async fn on_dictation_input_released(app_handle: &AppHandle) -> HoldRelease {
    let mut state = DICTATION_INPUT_STATE.lock().await;

    let (outcome, duration, deferred) =
        state.release(|id, end| hold_gate::defer(VoiceTarget::Dictation, id, end));
    if deferred {
        // The start this hold emitted is still opening the microphone. Acting
        // now would race it: the stop or cancel would find nothing to end, and
        // the start would then open a microphone no release is coming for. The
        // start handler performs this release when it lands.
        info!(
            "[DictationMonitor] Released ({}ms) before the session finished starting; the release waits for it",
            duration.as_millis()
        );
    }

    match outcome {
        HoldRelease::Committed => {
            info!("[DictationMonitor] Dictation input released after threshold reached - completing Dictation Mode normally");

            // Fire the stop cue on the key-up edge, before emitting STOP. The STOP
            // handler awaits speech-to-text finalization (seconds), and the old
            // end cue played only after that — which is exactly why stopping felt
            // unresponsive. Play it now so the user hears the release immediately.
            crate::commands::sound::play_cue(
                app_handle,
                crate::commands::sound::SoundType::NotificationDecorative01,
            );

            // Emit event to stop dictation normally
            if !deferred {
                if let Err(e) = app_handle.emit(events::dictation::STOP, ()) {
                    error!("[DictationMonitor] Failed to emit dictation-stop: {}", e);
                }
            }
        }
        HoldRelease::Cancelled => {
            info!(
                "[DictationMonitor] Dictation input released before threshold ({}ms) - cancelling",
                duration.as_millis()
            );
            if !deferred {
                if let Err(e) = app_handle.emit(events::dictation::TRANSCRIPTION_CANCEL, ()) {
                    error!(
                        "[DictationMonitor] Failed to emit dictation-transcription-cancel: {}",
                        e
                    );
                }
            }
        }
        HoldRelease::Nothing => {
            debug!(
                "[DictationMonitor] Dictation input released without starting transcription ({}ms) - no action needed",
                duration.as_millis()
            );
            // No passthrough needed since we're using Option+Space, not intercepting normal spacebar
        }
    }

    outcome
}

// Public function to force reset the dictation input state
pub async fn force_reset_dictation_input_state() {
    let mut state = DICTATION_INPUT_STATE.lock().await;
    state.force_reset();
    info!("[DictationMonitor] Dictation input state force reset completed");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn held_for_ms(state: &mut DictationInputMonitorState, ms: u64) {
        state.hold_start_time = Some(Instant::now() - Duration::from_millis(ms));
    }

    #[test]
    fn a_short_tap_cancels_the_session_it_opened() {
        let mut state = DictationInputMonitorState::new();
        assert!(state.start_hold());
        held_for_ms(&mut state, 120);
        assert!(state.check_and_start_transcription());

        // Released at 120ms, before the threshold deadline fires.
        let (outcome, _) = state.end_hold();
        assert_eq!(outcome, HoldRelease::Cancelled);
        assert!(!state.transcription_started);
        assert!(state.hold_start_time.is_none(), "nothing is being held");
        assert!(
            state.last_cancellation_time.is_some(),
            "a short tap arms the cooldown so a bounce cannot immediately reopen"
        );
    }

    #[test]
    fn a_hold_past_the_threshold_commits_on_release() {
        let mut state = DictationInputMonitorState::new();
        assert!(state.start_hold());
        held_for_ms(&mut state, monitor_sessions::HOLD_DURATION_MS + 50);
        assert!(state.check_and_start_transcription());
        assert!(state.reach_threshold(state.hold_generation));

        let (outcome, _) = state.end_hold();
        assert_eq!(outcome, HoldRelease::Committed);
        assert!(!state.transcription_started);
        assert!(
            state.last_cancellation_time.is_none(),
            "commit is not a cancel"
        );
    }

    #[test]
    fn a_release_before_transcription_opens_is_nothing() {
        let mut state = DictationInputMonitorState::new();
        assert!(state.start_hold());
        // Release before IMMEDIATE_START_MS elapses: transcription never opened.
        let (outcome, _) = state.end_hold();
        assert_eq!(outcome, HoldRelease::Nothing);
        assert!(state.last_cancellation_time.is_none());
    }

    #[test]
    fn cooldown_blocks_an_immediate_reopen_after_a_cancel() {
        let mut state = DictationInputMonitorState::new();
        assert!(state.start_hold());
        held_for_ms(&mut state, 120);
        assert!(state.check_and_start_transcription());
        assert_eq!(state.end_hold().0, HoldRelease::Cancelled);

        // A press inside the cooldown window is refused, so a bouncy key
        // cannot immediately reopen the session.
        assert!(
            !state.start_hold(),
            "cooldown must swallow a press that lands inside COOLDOWN_AFTER_CANCEL_MS"
        );
    }

    fn cooldown_elapsed(state: &mut DictationInputMonitorState) {
        state.last_cancellation_time = Instant::now().checked_sub(Duration::from_millis(
            monitor_sessions::COOLDOWN_AFTER_CANCEL_MS + 10,
        ));
    }

    #[test]
    fn a_press_released_before_anything_starts_leaves_nothing_to_end() {
        let mut state = DictationInputMonitorState::new();
        let mut gate = hold_gate::StartGate::default();
        assert!(state.start_hold());
        // Released before anything opened a start.
        let (outcome, _, deferred) = state.release(|id, end| gate.defer(id, end));
        assert_eq!(outcome, HoldRelease::Nothing);
        assert!(!deferred);
        assert!(!state.is_tracking_hold());
    }

    #[test]
    fn a_release_before_the_start_lands_is_applied_when_it_lands() {
        // The ~150ms press: the start is out, the microphone is still opening,
        // and the key comes up.
        let mut state = DictationInputMonitorState::new();
        let mut gate = hold_gate::StartGate::default();
        assert!(state.start_hold());
        held_for_ms(&mut state, 50);
        let hold = state
            .open_start(|| gate.open())
            .expect("the hold has lasted past IMMEDIATE_START_MS");

        let (outcome, _, deferred) = state.release(|id, end| gate.defer(id, end));
        assert_eq!(outcome, HoldRelease::Cancelled);
        assert!(deferred, "the cancel must not race the start it ends");
        assert!(
            !state.is_tracking_hold(),
            "the monitor itself is at rest the moment the key is up"
        );

        // The start lands and hands the release back: the session is
        // cancelled, which closes the microphone.
        assert_eq!(gate.settle(hold), Some(HoldEnd::Cancel));
        assert_eq!(
            hold_gate::release_event(VoiceTarget::Dictation, HoldEnd::Cancel),
            events::dictation::TRANSCRIPTION_CANCEL
        );

        // And the next hold is an ordinary one.
        cooldown_elapsed(&mut state);
        assert!(state.start_hold(), "the next hold is not refused");
        held_for_ms(&mut state, monitor_sessions::HOLD_DURATION_MS + 50);
        let next = state.open_start(|| gate.open()).expect("it starts");
        assert_ne!(next, hold);
        assert!(state.reach_threshold(state.hold_generation));
        assert_eq!(gate.settle(next), None, "landed with the key still down");
        let (outcome, _, deferred) = state.release(|id, end| gate.defer(id, end));
        assert_eq!(outcome, HoldRelease::Committed);
        assert!(!deferred, "a landed start is stopped at once");
    }

    #[test]
    fn the_session_opens_on_the_press() {
        // Nothing comes back after the press to open a delayed start.
        assert_eq!(monitor_sessions::IMMEDIATE_START_MS, 0);
    }

    #[test]
    fn a_deadline_from_an_earlier_hold_does_not_commit_a_later_one() {
        let mut state = DictationInputMonitorState::new();
        assert!(state.start_hold());
        let first = state.hold_generation;
        assert!(state.check_and_start_transcription());
        assert_eq!(state.end_hold().0, HoldRelease::Cancelled);

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
        let mut state = DictationInputMonitorState::new();
        assert!(state.start_hold());
        let generation = state.hold_generation;
        assert!(state.check_and_start_transcription());
        assert_eq!(state.end_hold().0, HoldRelease::Cancelled);

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
        let mut state = DictationInputMonitorState::new();
        assert!(state.start_hold());
        let generation = state.hold_generation;
        assert!(state.check_and_start_transcription());
        assert!(state.reach_threshold(generation));
        assert!(
            !state.reach_threshold(generation),
            "a second firing must not commit again"
        );
        assert!(
            state.session_overran(generation),
            "a hold still open at the max-duration deadline is stopped"
        );
        assert_eq!(state.end_hold().0, HoldRelease::Committed);
        assert!(!state.session_overran(generation));
    }

    #[test]
    fn is_tracking_hold_reflects_state() {
        let mut state = DictationInputMonitorState::new();
        assert!(!state.is_tracking_hold());
        assert!(state.start_hold());
        assert!(
            state.is_tracking_hold(),
            "a press without a release is a live hold"
        );
        let _ = state.end_hold();
        assert!(!state.is_tracking_hold(), "the release clears the hold");
    }
}
