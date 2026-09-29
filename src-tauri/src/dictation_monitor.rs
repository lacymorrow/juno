use crate::constants::{events, monitor_sessions};
use crate::state::{SessionClaim, VoiceStartMethod};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::Mutex;
use tracing::{debug, error, info, warn};

// Configuration constants
// const HOLD_DURATION_MS: u64 = 500; // Hold dictation input key for 500ms to commit dictation
// const IMMEDIATE_START_MS: u64 = 0; // Start transcription immediately (0ms delay)
// const MAX_TRANSCRIPTION_DURATION_MS: u64 = 30_000; // 30 seconds max transcription time
// const FORCE_CLEANUP_TIMEOUT_MS: u64 = 5_000; // 5 seconds to force cleanup if stuck
// const COOLDOWN_AFTER_CANCEL_MS: u64 = 150; // Reduced from 300ms to 150ms for better responsiveness

// State for dictation input monitoring
#[derive(Debug)]
pub struct DictationInputMonitorState {
    pub hold_start_time: Option<Instant>,
    pub transcription_started: bool,
    pub hold_threshold_reached: bool,
    pub passthrough_scheduled: bool,
    pub transcription_start_time: Option<Instant>, // Track when transcription actually started
    pub force_cleanup_scheduled: bool,
    pub last_cancellation_time: Option<Instant>, // Track when last cancellation occurred
    /// The key was tapped, not held, so the session it started keeps running
    /// on its own until the key is pressed again. While this is set the hold
    /// timers stand down: nothing is being held.
    pub hands_free: bool,
    /// The press that ended a hands-free session has a release coming; it
    /// must not be read as the end of a hold.
    pub swallow_next_release: bool,
}

/// What letting go of the key meant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HoldRelease {
    /// Held past the threshold: the session ends and the words are typed.
    Committed,
    /// A tap: the session keeps running hands-free until the next tap.
    HandsFree,
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
            transcription_start_time: None,
            force_cleanup_scheduled: false,
            last_cancellation_time: None,
            hands_free: false,
            swallow_next_release: false,
        }
    }

    /// End a hands-free session on the press that asked for it. Returns
    /// whether there was one to end.
    pub fn end_hands_free(&mut self) -> bool {
        if !(self.hands_free && self.transcription_started) {
            return false;
        }
        self.hold_start_time = None;
        self.transcription_started = false;
        self.hold_threshold_reached = false;
        self.passthrough_scheduled = false;
        self.transcription_start_time = None;
        self.force_cleanup_scheduled = false;
        self.hands_free = false;
        self.swallow_next_release = true;
        info!("[DictationMonitor] Hands-free dictation ended by the next press");
        true
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
        self.transcription_start_time = None;
        self.force_cleanup_scheduled = false;
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

        // A tap that already opened the microphone keeps it open. The hold
        // timers stand down and the session belongs to the next press.
        if transcription_was_started && !threshold_was_reached {
            self.hold_start_time = None;
            self.hold_threshold_reached = false;
            self.passthrough_scheduled = false;
            self.hands_free = true;
            info!(
                "[DictationMonitor] Tapped ({}ms) - dictation keeps running hands-free until the next tap",
                duration.as_millis()
            );
            return (HoldRelease::HandsFree, duration);
        }

        // Force reset all state immediately to prevent stuck state
        self.hold_start_time = None;
        self.transcription_started = false;
        self.hold_threshold_reached = false;
        self.passthrough_scheduled = false;
        self.transcription_start_time = None;
        self.force_cleanup_scheduled = false;
        self.hands_free = false;

        debug!(
            "[DictationMonitor] Ended dictation input hold tracking, duration: {:?}ms, transcription_started: {}, threshold_reached: {}",
            duration.as_millis(), transcription_was_started, threshold_was_reached
        );
        let outcome = if threshold_was_reached {
            HoldRelease::Committed
        } else {
            HoldRelease::Nothing
        };
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
                self.transcription_start_time = Some(Instant::now());
                info!("[DictationMonitor] Dictation input held for {}ms - starting immediate transcription", duration.as_millis());
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
                info!("[DictationMonitor] Dictation input held for {}ms - threshold reached, committing to Dictation Mode", duration.as_millis());
                return true;
            }
        }
        false
    }

    // Check if transcription has been running too long and needs forced cleanup
    pub fn check_transcription_timeout(&mut self) -> bool {
        // A hands-free session is meant to run as long as the person talks,
        // the same as a toggled one; the cap is for a stuck hold.
        if self.hands_free {
            return false;
        }
        if let Some(start_time) = self.transcription_start_time {
            let duration = start_time.elapsed();
            if duration.as_millis() >= monitor_sessions::MAX_TRANSCRIPTION_DURATION_MS as u128 {
                warn!(
                    "[DictationMonitor] Transcription has been running for {}ms - forcing cleanup",
                    duration.as_millis()
                );
                return true;
            }
        }
        false
    }

    // Check if we need to force cleanup due to stuck state
    pub fn should_force_cleanup(&mut self) -> bool {
        // If transcription started but dictation input was released and enough time has passed
        if self.transcription_started
            && self.hold_start_time.is_none()
            && !self.hands_free
            && !self.force_cleanup_scheduled
        {
            if let Some(start_time) = self.transcription_start_time {
                let duration = start_time.elapsed();
                if duration.as_millis() >= monitor_sessions::FORCE_CLEANUP_TIMEOUT_MS as u128 {
                    self.force_cleanup_scheduled = true;
                    warn!("[DictationMonitor] Scheduling force cleanup - transcription stuck for {}ms", duration.as_millis());
                    return true;
                }
            }
        }
        false
    }

    // Force reset all state - use when stuck
    pub fn force_reset(&mut self) {
        warn!("[DictationMonitor] Force resetting all dictation input state");
        self.hold_start_time = None;
        self.transcription_started = false;
        self.hold_threshold_reached = false;
        self.passthrough_scheduled = false;
        self.transcription_start_time = None;
        self.force_cleanup_scheduled = false;
        self.last_cancellation_time = None; // Clear cooldown tracking on reset
        self.hands_free = false;
        self.swallow_next_release = false;
    }
}

// Global state for the dictation input monitor
static DICTATION_INPUT_STATE: once_cell::sync::Lazy<Arc<Mutex<DictationInputMonitorState>>> =
    once_cell::sync::Lazy::new(|| Arc::new(Mutex::new(DictationInputMonitorState::new())));

// Initialize dictation input monitoring for the application
pub async fn init_dictation_input_monitoring(app_handle: AppHandle) -> Result<(), String> {
    info!("[DictationMonitor] Initializing dictation input monitoring system with immediate transcription start");

    // Start the monitoring task that checks for held dictation input
    let app_handle_clone = app_handle.clone();
    tauri::async_runtime::spawn(async move {
        dictation_input_monitoring_task(app_handle_clone).await;
    });

    info!("[DictationMonitor] Dictation input monitoring system initialized successfully");
    Ok(())
}

// Monitoring task that checks hold duration and triggers events
async fn dictation_input_monitoring_task(app_handle: AppHandle) {
    let mut interval = tokio::time::interval(Duration::from_millis(50)); // Check every 50ms for better responsiveness

    loop {
        interval.tick().await;

        let mut state = DICTATION_INPUT_STATE.lock().await;

        // Check if we should start transcription immediately
        if state.check_and_start_transcription() {
            // Emit event to start transcription immediately. The payload says
            // how this session is being triggered: this monitor only ever
            // watches a held key or button, so the session it opens is a
            // push-to-talk one and says so rather than leaving the start
            // handler to work it out.
            if let Err(e) = app_handle.emit(
                events::dictation::TRANSCRIPTION_START,
                serde_json::json!({ "method": VoiceStartMethod::PushToTalk.as_wire() }),
            ) {
                error!(
                    "[DictationMonitor] Failed to emit dictation-transcription-start: {}",
                    e
                );
            }
        }

        // Check if we've reached the hold threshold (commit to dictation)
        if state.check_and_reach_threshold() {
            // Emit event to confirm dictation commitment
            if let Err(e) = app_handle.emit(events::dictation::COMMITTED, ()) {
                error!(
                    "[DictationMonitor] Failed to emit dictation-committed: {}",
                    e
                );
            }
        }

        // Check for transcription timeout (safety mechanism)
        if state.check_transcription_timeout() {
            warn!("[DictationMonitor] Transcription timeout detected - forcing stop");
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
        }

        // Check if we need to force cleanup due to stuck state
        if state.should_force_cleanup() {
            warn!("[DictationMonitor] Force cleanup triggered");
            if let Err(e) = app_handle.emit(events::dictation::TRANSCRIPTION_FORCE_CLEANUP, ()) {
                error!(
                    "[DictationMonitor] Failed to emit dictation-transcription-force-cleanup: {}",
                    e
                );
            }

            // Force cleanup of app state
            let app_state = app_handle.state::<crate::state::AppState>();
            if let Err(e) = app_state.set_dictation_active(false) {
                warn!("Failed to reset dictation active state: {}", e);
            }

            // Try to force stop the voice controller
            force_stop_voice_controller(&app_handle).await;

            state.force_reset();
        }
    }
}

// Helper function to force stop the voice controller
async fn force_stop_voice_controller(app_handle: &AppHandle) {
    warn!("[DictationMonitor] Attempting to force stop voice controller");

    // Ungated, like every force path: it runs when a session is already stuck.
    // Claiming matters because this finalises the audio, and a transcript with
    // no session to own it is dropped rather than delivered.
    let app_state = app_handle.state::<crate::state::AppState>();
    match app_state.claim_voice_commit(SessionClaim::Current) {
        Ok(session) => info!(
            "[DictationMonitor] Force stop is finalising voice session {}",
            session.describe()
        ),
        Err(rejection) => warn!(
            "[DictationMonitor] Force stop with no session to claim: {}",
            rejection.reason()
        ),
    }

    // Try to stop the voice transcription plugin only if the controller exists
    match app_handle.try_state::<Arc<std::sync::Mutex<tauri_plugin_voice_transcription::controller::VoiceController>>>() {
        Some(controller_state) => {
            match tauri_plugin_voice_transcription::commands::stop_dictation(
                app_handle.clone(),
                controller_state
            ).await {
                Ok(_) => {
                    info!("[DictationMonitor] Successfully force stopped voice controller");
                }
                Err(e) => {
                    error!("[DictationMonitor] Failed to force stop voice controller: {}", e);
                }
            }
        }
        None => {
            warn!("[DictationMonitor] Voice controller not available - cannot force stop");
        }
    }
}

// Called when dictation input key is pressed down
pub async fn on_dictation_input_pressed(app_handle: &AppHandle) {
    info!("[DictationMonitor] on_dictation_input_pressed called");
    let mut state = DICTATION_INPUT_STATE.lock().await;

    // A session left running by a tap ends on the next press, the way a
    // toggled one does. The cue and the stop go out on this edge so it feels
    // like letting go of a held key.
    if state.end_hands_free() {
        crate::commands::sound::play_cue(
            app_handle,
            crate::commands::sound::SoundType::NotificationDecorative01,
        );
        if let Err(e) = app_handle.emit(events::dictation::STOP, ()) {
            error!("[DictationMonitor] Failed to emit dictation-stop: {}", e);
        }
        return;
    }

    info!("[DictationMonitor] Acquired lock, calling start_hold");
    let started = state.start_hold();
    if started {
        // Fire the start cue right here, on the key-down edge — not from the
        // downstream transcription-start handler, which only runs after audio
        // capture has initialized. The user must hear feedback the instant they
        // press, with no delay. `play_cue` is fire-and-forget (returns in µs).
        crate::commands::sound::play_cue(
            app_handle,
            crate::commands::sound::SoundType::NotificationAmbient,
        );
        info!("[DictationMonitor] Dictation input pressed down - starting immediate tracking");
    } else {
        warn!("[DictationMonitor] Dictation input pressed down - ignored (transcription_started={}, last_cancel={:?})",
              state.transcription_started,
              state.last_cancellation_time.map(|t| t.elapsed().as_millis()));
    }
}

// Called when dictation input key is released
pub async fn on_dictation_input_released(app_handle: &AppHandle) {
    let mut state = DICTATION_INPUT_STATE.lock().await;

    // The release of the press that ended a hands-free session.
    if state.swallow_next_release {
        state.swallow_next_release = false;
        return;
    }

    let (outcome, duration) = state.end_hold();

    if outcome == HoldRelease::Committed {
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
        if let Err(e) = app_handle.emit(events::dictation::STOP, ()) {
            error!("[DictationMonitor] Failed to emit dictation-stop: {}", e);
        }
    } else if outcome == HoldRelease::HandsFree {
        // The session is now committed the way a long hold would be, so
        // downstream sees the same event a held key produces at the threshold.
        if let Err(e) = app_handle.emit(events::dictation::COMMITTED, ()) {
            error!(
                "[DictationMonitor] Failed to emit dictation-committed: {}",
                e
            );
        }
    } else {
        debug!(
            "[DictationMonitor] Dictation input released without starting transcription ({}ms) - no action needed",
            duration.as_millis()
        );
        // No passthrough needed since we're using Option+Space, not intercepting normal spacebar
    }
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
    fn a_tap_keeps_the_session_running_hands_free() {
        let mut state = DictationInputMonitorState::new();
        assert!(state.start_hold());
        held_for_ms(&mut state, 120);
        assert!(state.check_and_start_transcription());
        assert!(
            !state.check_and_reach_threshold(),
            "120ms is a tap, not a hold"
        );

        let (outcome, _) = state.end_hold();
        assert_eq!(outcome, HoldRelease::HandsFree);
        assert!(state.transcription_started, "the microphone stays open");
        assert!(state.hands_free);
        assert!(state.hold_start_time.is_none(), "nothing is being held");
        assert!(
            state.last_cancellation_time.is_none(),
            "a tap is not a cancellation"
        );
    }

    #[test]
    fn a_hold_past_the_threshold_commits_on_release() {
        let mut state = DictationInputMonitorState::new();
        assert!(state.start_hold());
        held_for_ms(&mut state, monitor_sessions::HOLD_DURATION_MS + 50);
        assert!(state.check_and_start_transcription());
        assert!(state.check_and_reach_threshold());

        let (outcome, _) = state.end_hold();
        assert_eq!(outcome, HoldRelease::Committed);
        assert!(!state.transcription_started);
        assert!(!state.hands_free);
    }

    #[test]
    fn the_next_press_ends_a_hands_free_session_and_its_release_is_swallowed() {
        let mut state = DictationInputMonitorState::new();
        assert!(state.start_hold());
        held_for_ms(&mut state, 100);
        assert!(state.check_and_start_transcription());
        assert_eq!(state.end_hold().0, HoldRelease::HandsFree);

        assert!(
            state.end_hands_free(),
            "the press that follows a tap ends the session"
        );
        assert!(!state.transcription_started);
        assert!(!state.hands_free);
        assert!(state.swallow_next_release);
        assert!(!state.end_hands_free(), "there is nothing left to end");
    }

    #[test]
    fn a_hands_free_session_is_not_a_stuck_hold() {
        let mut state = DictationInputMonitorState::new();
        assert!(state.start_hold());
        held_for_ms(&mut state, 100);
        assert!(state.check_and_start_transcription());
        assert_eq!(state.end_hold().0, HoldRelease::HandsFree);

        // Make both watchdogs think plenty of time has passed.
        state.transcription_start_time = Some(
            Instant::now()
                - Duration::from_millis(monitor_sessions::MAX_TRANSCRIPTION_DURATION_MS + 1000),
        );
        assert!(
            !state.should_force_cleanup(),
            "hands-free is on purpose, not stuck"
        );
        assert!(
            !state.check_transcription_timeout(),
            "hands-free runs as long as the person talks"
        );
    }

    #[test]
    fn force_reset_clears_hands_free() {
        let mut state = DictationInputMonitorState::new();
        state.hands_free = true;
        state.swallow_next_release = true;
        state.force_reset();
        assert!(!state.hands_free);
        assert!(!state.swallow_next_release);
    }
}
