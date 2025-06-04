use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::Mutex;
use tracing::{info, error, debug, warn};

// Configuration constants
const HOLD_DURATION_MS: u64 = 500; // Hold spacebar for 500ms to commit dictation
const IMMEDIATE_START_MS: u64 = 0; // Start transcription immediately (0ms delay)
const MAX_TRANSCRIPTION_DURATION_MS: u64 = 30_000; // 30 seconds max transcription time
const FORCE_CLEANUP_TIMEOUT_MS: u64 = 5_000; // 5 seconds to force cleanup if stuck

// State for spacebar monitoring
#[derive(Debug)]
pub struct SpacebarMonitorState {
    pub hold_start_time: Option<Instant>,
    pub transcription_started: bool,
    pub hold_threshold_reached: bool,
    pub passthrough_scheduled: bool,
    pub transcription_start_time: Option<Instant>, // Track when transcription actually started
    pub force_cleanup_scheduled: bool,
}

impl SpacebarMonitorState {
    pub fn new() -> Self {
        Self {
            hold_start_time: None,
            transcription_started: false,
            hold_threshold_reached: false,
            passthrough_scheduled: false,
            transcription_start_time: None,
            force_cleanup_scheduled: false,
        }
    }

    pub fn start_hold(&mut self) {
        self.hold_start_time = Some(Instant::now());
        self.transcription_started = false;
        self.hold_threshold_reached = false;
        self.passthrough_scheduled = false;
        self.transcription_start_time = None;
        self.force_cleanup_scheduled = false;
        debug!("[SpacebarMonitor] Started tracking spacebar hold");
    }

    pub fn end_hold(&mut self) -> (bool, bool, Duration) {
        let transcription_was_started = self.transcription_started;
        let threshold_was_reached = self.hold_threshold_reached;
        let duration = self.hold_start_time
            .map(|start| start.elapsed())
            .unwrap_or(Duration::ZERO);

        self.hold_start_time = None;
        self.transcription_started = false;
        self.hold_threshold_reached = false;
        self.passthrough_scheduled = false;
        self.transcription_start_time = None;
        self.force_cleanup_scheduled = false;

        debug!(
            "[SpacebarMonitor] Ended spacebar hold tracking, duration: {:?}ms, transcription_started: {}, threshold_reached: {}",
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
            if duration.as_millis() >= IMMEDIATE_START_MS as u128 {
                self.transcription_started = true;
                self.transcription_start_time = Some(Instant::now());
                info!("[SpacebarMonitor] Spacebar held for {}ms - starting immediate transcription", duration.as_millis());
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
                info!("[SpacebarMonitor] Spacebar held for {}ms - threshold reached, committing to dictation", duration.as_millis());
                return true;
            }
        }
        false
    }

    // Check if transcription has been running too long and needs forced cleanup
    pub fn check_transcription_timeout(&mut self) -> bool {
        if let Some(start_time) = self.transcription_start_time {
            let duration = start_time.elapsed();
            if duration.as_millis() >= MAX_TRANSCRIPTION_DURATION_MS as u128 {
                warn!("[SpacebarMonitor] Transcription has been running for {}ms - forcing cleanup", duration.as_millis());
                return true;
            }
        }
        false
    }

    // Check if we need to force cleanup due to stuck state
    pub fn should_force_cleanup(&mut self) -> bool {
        // If transcription started but spacebar was released and enough time has passed
        if self.transcription_started && self.hold_start_time.is_none() && !self.force_cleanup_scheduled {
            if let Some(start_time) = self.transcription_start_time {
                let duration = start_time.elapsed();
                if duration.as_millis() >= FORCE_CLEANUP_TIMEOUT_MS as u128 {
                    self.force_cleanup_scheduled = true;
                    warn!("[SpacebarMonitor] Scheduling force cleanup - transcription stuck for {}ms", duration.as_millis());
                    return true;
                }
            }
        }
        false
    }

    // Force reset all state - use when stuck
    pub fn force_reset(&mut self) {
        warn!("[SpacebarMonitor] Force resetting all spacebar state");
        self.hold_start_time = None;
        self.transcription_started = false;
        self.hold_threshold_reached = false;
        self.passthrough_scheduled = false;
        self.transcription_start_time = None;
        self.force_cleanup_scheduled = false;
    }
}

// Global state for the spacebar monitor
static SPACEBAR_STATE: once_cell::sync::Lazy<Arc<Mutex<SpacebarMonitorState>>> =
    once_cell::sync::Lazy::new(|| Arc::new(Mutex::new(SpacebarMonitorState::new())));

// Initialize spacebar monitoring for the application
pub async fn init_spacebar_monitoring(app_handle: AppHandle) -> Result<(), String> {
    info!("[SpacebarMonitor] Initializing spacebar monitoring system with immediate transcription start");

    // Start the monitoring task that checks for held spacebar
    let app_handle_clone = app_handle.clone();
    tokio::spawn(async move {
        spacebar_monitoring_task(app_handle_clone).await;
    });

    info!("[SpacebarMonitor] Spacebar monitoring system initialized successfully");
    Ok(())
}

// Monitoring task that checks hold duration and triggers events
async fn spacebar_monitoring_task(app_handle: AppHandle) {
    let mut interval = tokio::time::interval(Duration::from_millis(50)); // Check every 50ms for better responsiveness

    loop {
        interval.tick().await;

        let mut state = SPACEBAR_STATE.lock().await;

        // Check if we should start transcription immediately
        if state.check_and_start_transcription() {
            // Emit event to start transcription immediately
            if let Err(e) = app_handle.emit("spacebar-transcription-start", ()) {
                error!("[SpacebarMonitor] Failed to emit spacebar-transcription-start: {}", e);
            }
        }

        // Check if we've reached the hold threshold (commit to dictation)
        if state.check_and_reach_threshold() {
            // Emit event to confirm dictation commitment
            if let Err(e) = app_handle.emit("spacebar-dictation-committed", ()) {
                error!("[SpacebarMonitor] Failed to emit spacebar-dictation-committed: {}", e);
            }
        }

        // Check for transcription timeout (safety mechanism)
        if state.check_transcription_timeout() {
            warn!("[SpacebarMonitor] Transcription timeout detected - forcing stop");
            if let Err(e) = app_handle.emit("spacebar-transcription-force-stop", ()) {
                error!("[SpacebarMonitor] Failed to emit spacebar-transcription-force-stop: {}", e);
            }

            // Force cleanup of app state
            let app_state = app_handle.state::<crate::state::AppState>();
            if let Ok(mut spacebar_active) = app_state.spacebar_dictation_active.lock() {
                *spacebar_active = false;
            }

            state.force_reset();
        }

        // Check if we need to force cleanup due to stuck state
        if state.should_force_cleanup() {
            warn!("[SpacebarMonitor] Force cleanup triggered");
            if let Err(e) = app_handle.emit("spacebar-transcription-force-cleanup", ()) {
                error!("[SpacebarMonitor] Failed to emit spacebar-transcription-force-cleanup: {}", e);
            }

            // Force cleanup of app state
            let app_state = app_handle.state::<crate::state::AppState>();
            if let Ok(mut spacebar_active) = app_state.spacebar_dictation_active.lock() {
                *spacebar_active = false;
            }

            // Try to force stop the voice controller
            force_stop_voice_controller(&app_handle).await;

            state.force_reset();
        }
    }
}

// Helper function to force stop the voice controller
async fn force_stop_voice_controller(app_handle: &AppHandle) {
    warn!("[SpacebarMonitor] Attempting to force stop voice controller");

    // Try to stop the voice transcription plugin
    match tauri_plugin_voice_transcription::commands::stop_dictation(
        app_handle.clone(),
        app_handle.state::<Arc<std::sync::Mutex<tauri_plugin_voice_transcription::controller::VoiceController>>>()
    ).await {
        Ok(_) => {
            info!("[SpacebarMonitor] Successfully force stopped voice controller");
        }
        Err(e) => {
            error!("[SpacebarMonitor] Failed to force stop voice controller: {}", e);
        }
    }
}

// Called when spacebar is pressed down
pub async fn on_spacebar_pressed() {
    let mut state = SPACEBAR_STATE.lock().await;

    // If we're already in a transcription state, ignore this press
    if state.transcription_started {
        warn!("[SpacebarMonitor] Spacebar pressed while transcription already active - ignoring");
        return;
    }

    state.start_hold();
    debug!("[SpacebarMonitor] Spacebar pressed down - starting immediate tracking");
}

// Called when spacebar is released
pub async fn on_spacebar_released(app_handle: &AppHandle) {
    let mut state = SPACEBAR_STATE.lock().await;
    let (transcription_started, threshold_reached, duration) = state.end_hold();

    if threshold_reached {
        info!("[SpacebarMonitor] Spacebar released after threshold reached - completing dictation normally");

        // Emit event to stop dictation normally
        if let Err(e) = app_handle.emit("spacebar-dictation-stop", ()) {
            error!("[SpacebarMonitor] Failed to emit spacebar-dictation-stop: {}", e);
        }
    } else if transcription_started {
        info!(
            "[SpacebarMonitor] Spacebar released before threshold ({}ms) - cancelling transcription",
            duration.as_millis()
        );

        // Emit event to cancel transcription and do passthrough
        if let Err(e) = app_handle.emit("spacebar-transcription-cancel", ()) {
            error!("[SpacebarMonitor] Failed to emit spacebar-transcription-cancel: {}", e);
        }

        // Attempt passthrough space typing
        #[cfg(target_os = "macos")]
        {
            attempt_space_passthrough(app_handle).await;
        }
    } else {
        debug!(
            "[SpacebarMonitor] Spacebar released without starting transcription ({}ms)",
            duration.as_millis()
        );

        // Very short press - just do passthrough
        #[cfg(target_os = "macos")]
        {
            attempt_space_passthrough(app_handle).await;
        }
    }
}

// Public function to force reset the spacebar state (for emergency cleanup)
pub async fn force_reset_spacebar_state() {
    let mut state = SPACEBAR_STATE.lock().await;
    state.force_reset();
    info!("[SpacebarMonitor] Spacebar state force reset completed");
}

// Attempt to pass through a space character to the currently focused application
#[cfg(target_os = "macos")]
async fn attempt_space_passthrough(app_handle: &AppHandle) {
    debug!("[SpacebarMonitor] Attempting to type space character for passthrough");

    // Get the app state to access the desktop automation
    let app_state = app_handle.state::<crate::state::AppState>();

    // Use the global type text function to insert a space
    match crate::commands::keyboard::dev_global_type_text(" ".to_string(), app_state.clone()).await {
        Ok(()) => {
            debug!("[SpacebarMonitor] Successfully typed space character for passthrough");
        }
        Err(e) => {
            error!("[SpacebarMonitor] Failed to type space character for passthrough: {}", e);
        }
    }
}
