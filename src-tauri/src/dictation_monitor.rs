use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::Mutex;
use tracing::{info, error, debug, warn};
use crate::constants::{monitor_sessions, events};

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
}

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
        }
    }

    pub fn start_hold(&mut self) -> bool {
        // Check if we're already in a transcription state
        if self.transcription_started {
            debug!("[DictationMonitor] Ignoring dictation input press - transcription already active");
            return false;
        }

        // Check if we're in cooldown period after a recent cancellation
        if let Some(last_cancel) = self.last_cancellation_time {
            let time_since_cancel = last_cancel.elapsed().as_millis();
            if time_since_cancel < monitor_sessions::COOLDOWN_AFTER_CANCEL_MS as u128 {
                debug!("[DictationMonitor] Ignoring dictation input press - still in cooldown period ({}ms since last cancellation)", time_since_cancel);
                return false; // Don't start tracking during cooldown
            }
        }

        self.hold_start_time = Some(Instant::now());
        self.transcription_started = false;
        self.hold_threshold_reached = false;
        self.passthrough_scheduled = false;
        self.transcription_start_time = None;
        self.force_cleanup_scheduled = false;
        debug!("[DictationMonitor] Started tracking dictation input hold");
        true // Successfully started tracking
    }

    pub fn end_hold(&mut self) -> (bool, bool, Duration) {
        let transcription_was_started = self.transcription_started;
        let threshold_was_reached = self.hold_threshold_reached;
        let duration = self.hold_start_time
            .map(|start| start.elapsed())
            .unwrap_or(Duration::ZERO);

        // If transcription was started but threshold wasn't reached, it means we're cancelling
        if transcription_was_started && !threshold_was_reached {
            self.last_cancellation_time = Some(Instant::now());
            warn!("[DictationMonitor] Cancelling transcription after {}ms - recording cancellation time for cooldown", duration.as_millis());
        }

        // Force reset all state immediately to prevent stuck state
        self.hold_start_time = None;
        self.transcription_started = false;
        self.hold_threshold_reached = false;
        self.passthrough_scheduled = false;
        self.transcription_start_time = None;
        self.force_cleanup_scheduled = false;

        debug!(
            "[DictationMonitor] Ended dictation input hold tracking, duration: {:?}ms, transcription_started: {}, threshold_reached: {}",
            duration.as_millis(), transcription_was_started, threshold_was_reached
        );
        (transcription_was_started, threshold_was_reached, duration)
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
        if let Some(start_time) = self.transcription_start_time {
            let duration = start_time.elapsed();
            if duration.as_millis() >= monitor_sessions::MAX_TRANSCRIPTION_DURATION_MS as u128 {
                warn!("[DictationMonitor] Transcription has been running for {}ms - forcing cleanup", duration.as_millis());
                return true;
            }
        }
        false
    }

    // Check if we need to force cleanup due to stuck state
    pub fn should_force_cleanup(&mut self) -> bool {
        // If transcription started but dictation input was released and enough time has passed
        if self.transcription_started && self.hold_start_time.is_none() && !self.force_cleanup_scheduled {
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
    tokio::spawn(async move {
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
            // Emit event to start transcription immediately
            if let Err(e) = app_handle.emit(events::dictation::TRANSCRIPTION_START, ()) {
                error!("[DictationMonitor] Failed to emit dictation-transcription-start: {}", e);
            }
        }

        // Check if we've reached the hold threshold (commit to dictation)
        if state.check_and_reach_threshold() {
            // Emit event to confirm dictation commitment
            if let Err(e) = app_handle.emit(events::dictation::COMMITTED, ()) {
                error!("[DictationMonitor] Failed to emit dictation-committed: {}", e);
            }
        }

        // Check for transcription timeout (safety mechanism)
        if state.check_transcription_timeout() {
            warn!("[DictationMonitor] Transcription timeout detected - forcing stop");
            if let Err(e) = app_handle.emit(events::dictation::TRANSCRIPTION_FORCE_STOP, ()) {
                error!("[DictationMonitor] Failed to emit dictation-transcription-force-stop: {}", e);
            }

            // Force cleanup of app state
            let app_state = app_handle.state::<crate::state::AppState>();
            if let Ok(mut dictation_active) = app_state.dictation_active.lock() {
                *dictation_active = false;
            }

            state.force_reset();
        }

        // Check if we need to force cleanup due to stuck state
        if state.should_force_cleanup() {
            warn!("[DictationMonitor] Force cleanup triggered");
            if let Err(e) = app_handle.emit(events::dictation::TRANSCRIPTION_FORCE_CLEANUP, ()) {
                error!("[DictationMonitor] Failed to emit dictation-transcription-force-cleanup: {}", e);
            }

            // Force cleanup of app state
            let app_state = app_handle.state::<crate::state::AppState>();
            if let Ok(mut dictation_active) = app_state.dictation_active.lock() {
                *dictation_active = false;
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
pub async fn on_dictation_input_pressed() {
    let mut state = DICTATION_INPUT_STATE.lock().await;
    let started = state.start_hold();
    if started {
        debug!("[DictationMonitor] Dictation input pressed down - starting immediate tracking");
    } else {
        debug!("[DictationMonitor] Dictation input pressed down - ignored (transcription active or cooldown period)");
    }
}

// Called when dictation input key is released
pub async fn on_dictation_input_released(app_handle: &AppHandle) {
    let mut state = DICTATION_INPUT_STATE.lock().await;
    let (transcription_started, threshold_reached, duration) = state.end_hold();

    if threshold_reached {
        info!("[DictationMonitor] Dictation input released after threshold reached - completing Dictation Mode normally");

        // Emit event to stop dictation normally
        if let Err(e) = app_handle.emit(events::dictation::STOP, ()) {
            error!("[DictationMonitor] Failed to emit dictation-stop: {}", e);
        }
    } else if transcription_started {
        info!(
            "[DictationMonitor] Dictation input released before threshold ({}ms) - cancelling transcription",
            duration.as_millis()
        );

        // Emit event to cancel transcription - no passthrough needed since we use Option+Space now
        if let Err(e) = app_handle.emit(events::dictation::TRANSCRIPTION_CANCEL, ()) {
            error!("[DictationMonitor] Failed to emit dictation-transcription-cancel: {}", e);
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


