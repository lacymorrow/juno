use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::Mutex;
use tracing::{info, error, debug, warn};

// Configuration constants
const HOLD_DURATION_MS: u64 = 500; // Hold spacebar for 500ms to trigger dictation

// State for spacebar monitoring
#[derive(Debug)]
pub struct SpacebarMonitorState {
    pub hold_start_time: Option<Instant>,
    pub dictation_triggered: bool,
    pub passthrough_scheduled: bool,
}

impl SpacebarMonitorState {
    pub fn new() -> Self {
        Self {
            hold_start_time: None,
            dictation_triggered: false,
            passthrough_scheduled: false,
        }
    }

    pub fn start_hold(&mut self) {
        self.hold_start_time = Some(Instant::now());
        self.dictation_triggered = false;
        self.passthrough_scheduled = false;
        debug!("[SpacebarMonitor] Started tracking spacebar hold");
    }

    pub fn end_hold(&mut self) -> (bool, Duration) {
        let was_triggered = self.dictation_triggered;
        let duration = self.hold_start_time
            .map(|start| start.elapsed())
            .unwrap_or(Duration::ZERO);

        self.hold_start_time = None;
        self.dictation_triggered = false;
        self.passthrough_scheduled = false;

        debug!("[SpacebarMonitor] Ended spacebar hold tracking, duration: {:?}ms, was_triggered: {}", duration.as_millis(), was_triggered);
        (was_triggered, duration)
    }

    pub fn check_and_trigger_dictation(&mut self) -> bool {
        if self.dictation_triggered {
            return false;
        }

        if let Some(start_time) = self.hold_start_time {
            let duration = start_time.elapsed();
            if duration.as_millis() >= HOLD_DURATION_MS as u128 {
                self.dictation_triggered = true;
                info!("[SpacebarMonitor] Spacebar held for {}ms - triggering dictation", duration.as_millis());
                return true;
            }
        }
        false
    }
}

// Global state for the spacebar monitor
static SPACEBAR_STATE: once_cell::sync::Lazy<Arc<Mutex<SpacebarMonitorState>>> =
    once_cell::sync::Lazy::new(|| Arc::new(Mutex::new(SpacebarMonitorState::new())));

// Initialize spacebar monitoring for the application
pub async fn init_spacebar_monitoring(app_handle: AppHandle) -> Result<(), String> {
    info!("[SpacebarMonitor] Initializing spacebar monitoring system with hold-to-dictate logic");

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
    let mut interval = tokio::time::interval(Duration::from_millis(100)); // Check every 100ms

    loop {
        interval.tick().await;

        let mut state = SPACEBAR_STATE.lock().await;

        if state.check_and_trigger_dictation() {
            // Emit event to start dictation
            if let Err(e) = app_handle.emit("spacebar-dictation-start", ()) {
                error!("[SpacebarMonitor] Failed to emit spacebar-dictation-start: {}", e);
            }
        }
    }
}

// Called when spacebar is pressed down
pub async fn on_spacebar_pressed() {
    let mut state = SPACEBAR_STATE.lock().await;
    state.start_hold();
    debug!("[SpacebarMonitor] Spacebar pressed down - starting hold timer");
}

// Called when spacebar is released
pub async fn on_spacebar_released(app_handle: &AppHandle) {
    let mut state = SPACEBAR_STATE.lock().await;
    let (was_triggered, duration) = state.end_hold();

    if was_triggered {
        info!("[SpacebarMonitor] Spacebar released after dictation trigger - stopping dictation");

        // Emit event to stop dictation
        if let Err(e) = app_handle.emit("spacebar-dictation-stop", ()) {
            error!("[SpacebarMonitor] Failed to emit spacebar-dictation-stop: {}", e);
        }
    } else if duration.as_millis() < HOLD_DURATION_MS as u128 {
        // Short press - attempt passthrough
        debug!("[SpacebarMonitor] Short spacebar press ({}ms) - attempting passthrough", duration.as_millis());

        // Unfortunately, with global shortcuts, we can't easily do true passthrough
        // The global shortcut system captures the event before it reaches other apps
        warn!("[SpacebarMonitor] Note: Spacebar passthrough is limited due to global shortcut system");

        // For short presses, we could try to type a space into the currently focused application
        // This is a workaround since true passthrough isn't possible with global shortcuts
        #[cfg(target_os = "macos")]
        {
            attempt_space_passthrough(app_handle).await;
        }
    } else {
        debug!("[SpacebarMonitor] Spacebar released without triggering dictation ({}ms)", duration.as_millis());
    }
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
