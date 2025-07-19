use crate::constants::{events, monitor_sessions};
use crate::error::{MonitorError, MonitorResult};
use crate::monitor::{AtomicEventMonitor, AtomicMonitorConfig, MonitorEvent};
use std::sync::Arc;
use tauri::{AppHandle, Manager};
use crate::state::AppState;
use tracing::{debug, error, info, warn};

// Global atomic monitor instance
static DICTATION_MONITOR: once_cell::sync::OnceCell<Arc<AtomicEventMonitor>> = once_cell::sync::OnceCell::new();

/// Initialize dictation input monitoring with atomic event-driven architecture
pub async fn init_dictation_input_monitoring(app_handle: AppHandle) -> MonitorResult<()> {
    info!("[DictationMonitor] Initializing atomic event-driven dictation monitor");
    
    // Create monitor configuration
    let config = AtomicMonitorConfig {
        name: "DictationMonitor".to_string(),
        immediate_start_ms: monitor_sessions::IMMEDIATE_START_MS,
        hold_duration_ms: monitor_sessions::HOLD_DURATION_MS,
        max_duration_ms: monitor_sessions::MAX_TRANSCRIPTION_DURATION_MS,
        force_cleanup_timeout_ms: monitor_sessions::FORCE_CLEANUP_TIMEOUT_MS,
        cooldown_after_cancel_ms: monitor_sessions::COOLDOWN_AFTER_CANCEL_MS,
        start_event: events::dictation::TRANSCRIPTION_START.to_string(),
        commit_event: events::dictation::COMMITTED.to_string(),
        stop_event: events::dictation::STOP.to_string(),
    };
    
    // Create the monitor
    let (monitor, rx) = AtomicEventMonitor::new(config, app_handle.clone());
    let monitor = Arc::new(monitor);
    
    // Store globally for access
    DICTATION_MONITOR.set(monitor.clone())
        .map_err(|_| MonitorError::AlreadyInitialized)?;
    
    // Start the monitor's event processing loop
    tokio::spawn(async move {
        monitor.run(rx).await;
    });
    
    info!("[DictationMonitor] Event-driven dictation monitor initialized successfully");
    Ok(())
}

// Called when dictation input key is pressed down
pub async fn on_dictation_input_pressed() {
    debug!("[DictationMonitor] on_dictation_input_pressed() called");
    
    if let Some(monitor) = DICTATION_MONITOR.get() {
        if let Err(e) = monitor.send_event(MonitorEvent::StartHold).await {
            error!("[DictationMonitor] Failed to send StartHold event: {}", e);
        }
    } else {
        error!("[DictationMonitor] Monitor not initialized");
    }
}

// Called when dictation input key is released
pub async fn on_dictation_input_released(_app_handle: &AppHandle) {
    debug!("[DictationMonitor] on_dictation_input_released() called");
    
    if let Some(monitor) = DICTATION_MONITOR.get() {
        if let Err(e) = monitor.send_event(MonitorEvent::EndHold).await {
            error!("[DictationMonitor] Failed to send EndHold event: {}", e);
        }
    } else {
        error!("[DictationMonitor] Monitor not initialized");
    }
}

// Public function to force reset the dictation input state
pub async fn force_reset_dictation_input_state() {
    if let Some(monitor) = DICTATION_MONITOR.get() {
        if let Err(e) = monitor.send_event(MonitorEvent::ForceReset).await {
            error!("[DictationMonitor] Failed to send ForceReset event: {}", e);
        }
        info!("[DictationMonitor] Dictation input state force reset completed");
    } else {
        error!("[DictationMonitor] Monitor not initialized");
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

// Legacy function - no longer needed but kept for compatibility
async fn dictation_input_monitoring_task(_app_handle: AppHandle) {
    // No-op - monitoring is handled by event-driven system
    warn!("[DictationMonitor] Legacy monitoring task called - using event-driven system instead");
}