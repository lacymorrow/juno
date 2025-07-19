use crate::constants::{events, monitor_sessions};
use crate::error::{MonitorError, MonitorResult};
use crate::monitor::{AtomicEventMonitor, AtomicMonitorConfig, MonitorEvent};
use crate::state::{AgentTriggerMode, AppState};
use std::sync::Arc;
use tauri::{AppHandle, Manager};
use tracing::{error, info, warn};

// Global atomic monitor instance
static AGENT_MONITOR: once_cell::sync::OnceCell<Arc<AtomicEventMonitor>> = once_cell::sync::OnceCell::new();

/// Initialize the agent monitor with atomic event-driven architecture
pub async fn init_agent_monitor(app_handle: AppHandle) -> MonitorResult<()> {
    info!("[AgentMonitor] Initializing atomic event-driven agent monitor");
    
    // Create monitor configuration
    let config = AtomicMonitorConfig {
        name: "AgentMonitor".to_string(),
        immediate_start_ms: monitor_sessions::IMMEDIATE_START_MS,
        hold_duration_ms: monitor_sessions::HOLD_DURATION_MS,
        max_duration_ms: monitor_sessions::MAX_AGENT_DURATION_MS,
        force_cleanup_timeout_ms: monitor_sessions::FORCE_CLEANUP_TIMEOUT_MS,
        cooldown_after_cancel_ms: monitor_sessions::COOLDOWN_AFTER_CANCEL_MS,
        start_event: events::agent::TRANSCRIPTION_START.to_string(),
        commit_event: events::agent::COMMITTED.to_string(),
        stop_event: events::agent::STOP.to_string(),
    };
    
    // Create the monitor
    let (monitor, rx) = AtomicEventMonitor::new(config, app_handle.clone());
    let monitor = Arc::new(monitor);
    
    // Store globally for access
    AGENT_MONITOR.set(monitor.clone())
        .map_err(|_| MonitorError::AlreadyInitialized)?;
    
    // Start the monitor's event processing loop
    tokio::spawn(async move {
        monitor.run(rx).await;
    });
    
    info!("[AgentMonitor] Event-driven agent monitor initialized successfully");
    Ok(())
}

// Called when agent input key is pressed
pub async fn on_agent_input_pressed() {
    info!("[AgentMonitor] on_agent_input_pressed() called");
    
    if let Some(monitor) = AGENT_MONITOR.get() {
        if let Err(e) = monitor.send_event(MonitorEvent::StartHold).await {
            error!("[AgentMonitor] Failed to send StartHold event: {}", e);
        }
    } else {
        error!("[AgentMonitor] Monitor not initialized");
    }
}

// Called when agent input key is released
pub async fn on_agent_input_released(_app_handle: &AppHandle) {
    info!("[AgentMonitor] on_agent_input_released() called");
    
    if let Some(monitor) = AGENT_MONITOR.get() {
        if let Err(e) = monitor.send_event(MonitorEvent::EndHold).await {
            error!("[AgentMonitor] Failed to send EndHold event: {}", e);
        }
    } else {
        error!("[AgentMonitor] Monitor not initialized");
    }
}

// Public function to force reset the agent input state
pub async fn force_reset_agent_input_state() {
    if let Some(monitor) = AGENT_MONITOR.get() {
        if let Err(e) = monitor.send_event(MonitorEvent::ForceReset).await {
            error!("[AgentMonitor] Failed to send ForceReset event: {}", e);
        }
    } else {
        error!("[AgentMonitor] Monitor not initialized");
    }
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

// Legacy function - no longer needed with event-driven architecture
pub fn start_agent_monitor_task(_app_handle: AppHandle) -> tokio::task::JoinHandle<()> {
    info!("[AgentMonitor] Legacy start_agent_monitor_task called - using event-driven system instead");
    tokio::spawn(async {
        // No-op - monitoring is handled by event-driven system
    })
}