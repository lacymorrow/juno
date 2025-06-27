use std::sync::{Arc, Mutex, atomic::{AtomicBool, AtomicU64, Ordering}};
use std::collections::HashSet;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager, Emitter};
use tracing::{info, warn, error, debug};
use tokio::sync::RwLock;
use once_cell::sync::Lazy;
use crate::state::AppState;
use crate::constants::events;
use crate::constants::errors::templates::FAILED_TO_EMIT;
use crate::constants::errors::prefixes::STOP_COORDINATOR as STOP_COORDINATOR_PREFIX;

/// Centralized stop coordinator to prevent race conditions and cascading cleanup operations
pub struct StopCoordinator {
    /// Track active operations to prevent redundant stops
    active_operations: Arc<RwLock<HashSet<String>>>,
    /// Track cleanup operations in progress
    cleanup_in_progress: Arc<AtomicBool>,
    /// Last cleanup timestamp to prevent rapid successive cleanups
    last_cleanup: Arc<Mutex<Option<Instant>>>,
    /// Operation counter for unique operation IDs
    operation_counter: Arc<AtomicU64>,
    /// Emergency stop flag
    emergency_stop_active: Arc<AtomicBool>,
}

impl StopCoordinator {
    pub fn new() -> Self {
        Self {
            active_operations: Arc::new(RwLock::new(HashSet::new())),
            cleanup_in_progress: Arc::new(AtomicBool::new(false)),
            last_cleanup: Arc::new(Mutex::new(None)),
            operation_counter: Arc::new(AtomicU64::new(1)),
            emergency_stop_active: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Atomically try to start cleanup, checking timing and state in one operation
    /// Returns true if cleanup was successfully started, false if already in progress or too recent
    async fn try_start_cleanup(&self) -> bool {
        // First, atomically try to set cleanup_in_progress from false to true
        let was_already_in_progress = self.cleanup_in_progress.compare_exchange(
            false,
            true,
            Ordering::SeqCst,
            Ordering::SeqCst
        ).is_err();

        if was_already_in_progress {
            debug!("[StopCoordinator] Cleanup already in progress, skipping");
            return false;
        }

        // We successfully set the flag, now check timing
        if let Ok(last_cleanup_guard) = self.last_cleanup.lock() {
            if let Some(last_time) = *last_cleanup_guard {
                let elapsed = last_time.elapsed();
                if elapsed < Duration::from_millis(500) {
                    debug!("[StopCoordinator] Recent cleanup detected ({}ms ago), skipping", elapsed.as_millis());
                    // Reset the flag since we're not proceeding
                    self.cleanup_in_progress.store(false, Ordering::SeqCst);
                    return false;
                }
            }
        }

        // Update timestamp now that we're proceeding
        if let Ok(mut last_cleanup_guard) = self.last_cleanup.lock() {
            *last_cleanup_guard = Some(Instant::now());
        }

        true
    }

    /// Mark cleanup as completed
    async fn mark_cleanup_completed(&self) {
        self.cleanup_in_progress.store(false, Ordering::SeqCst);
    }

    /// Register an operation to track its lifecycle
    pub async fn register_operation(&self, operation_type: &str) -> String {
        let operation_id = format!("{}_{}", operation_type, self.operation_counter.fetch_add(1, Ordering::SeqCst));
        let mut operations = self.active_operations.write().await;
        operations.insert(operation_id.clone());
        debug!("[StopCoordinator] Registered operation: {}", operation_id);
        operation_id
    }

    /// Unregister an operation when it completes
    pub async fn unregister_operation(&self, operation_id: &str) {
        let mut operations = self.active_operations.write().await;
        operations.remove(operation_id);
        debug!("[StopCoordinator] Unregistered operation: {}", operation_id);
    }

    /// Check if a specific operation type is already active
    pub async fn is_operation_active(&self, operation_type: &str) -> bool {
        let operations = self.active_operations.read().await;
        operations.iter().any(|op| op.starts_with(operation_type))
    }

    /// Atomically check if an operation type is active and register a new one if not
    /// Returns Some(operation_id) if successfully registered, None if already active
    pub async fn try_register_operation(&self, operation_type: &str) -> Option<String> {
        let mut operations = self.active_operations.write().await;

        // Check if operation type is already active
        if operations.iter().any(|op| op.starts_with(operation_type)) {
            debug!("[StopCoordinator] Operation type '{}' already active, skipping registration", operation_type);
            return None;
        }

        // Register the new operation atomically
        let operation_id = format!("{}_{}", operation_type, self.operation_counter.fetch_add(1, Ordering::SeqCst));
        operations.insert(operation_id.clone());
        debug!("[StopCoordinator] Atomically registered operation: {}", operation_id);
        Some(operation_id)
    }

    /// Coordinated stop all operations with deduplication
    pub async fn stop_all_operations(&self, app_handle: &AppHandle, reason: &str) -> Result<String, String> {
        info!("[StopCoordinator] Stop all operations requested: {}", reason);

        // Atomically try to start cleanup
        if !self.try_start_cleanup().await {
            return Ok("Cleanup skipped - already in progress or too recent".to_string());
        }

        // Register this cleanup operation
        let cleanup_id = self.register_operation("cleanup").await;

        let result = self.perform_coordinated_cleanup(app_handle, reason).await;

        // Mark cleanup as completed
        self.mark_cleanup_completed().await;
        self.unregister_operation(&cleanup_id).await;

        result
    }

    /// Perform the actual coordinated cleanup
    async fn perform_coordinated_cleanup(&self, app_handle: &AppHandle, reason: &str) -> Result<String, String> {
        info!("[StopCoordinator] Performing coordinated cleanup: {}", reason);

        let app_state = app_handle.state::<AppState>();
        let mut cleanup_results = Vec::new();

        // 1. Stop TTS immediately (highest priority)
        if let Some(tts_op_id) = self.try_register_operation("tts_stop").await {
            info!("[StopCoordinator] Stopping TTS");
            crate::tts::stop_speech();

            // Emit TTS stop event once
            self.emit_tts_stop_event(app_handle);
            cleanup_results.push("TTS stopped".to_string());
            self.unregister_operation(&tts_op_id).await;
        }

        // 2. Signal agent cancellation
        if let Some(agent_op_id) = self.try_register_operation("agent_stop").await {
            info!("[StopCoordinator] Signaling agent cancellation");

            let cancel_requested = *app_state.cancel_rx.borrow();
            if !cancel_requested {
                app_state.signal_cancel();
                cleanup_results.push("Agent cancellation signaled".to_string());
            }

            app_state.mark_agent_execution_finished();
            self.unregister_operation(&agent_op_id).await;
        }

        // 3. Stop dictation through state manager
        if let Some(dictation_op_id) = self.try_register_operation("dictation_stop").await {
            info!("[StopCoordinator] Stopping dictation");

            if let Err(e) = crate::commands::dictation_state_manager::force_stop_dictation(app_handle).await {
                warn!("[StopCoordinator] Failed to stop dictation: {}", e);
            } else {
                cleanup_results.push("Dictation stopped".to_string());
            }
            self.unregister_operation(&dictation_op_id).await;
        }

        // 4. Stop always listening mode
        if let Some(al_op_id) = self.try_register_operation("always_listening_stop").await {
            info!("[StopCoordinator] Stopping always listening mode");

            if let Err(e) = crate::commands::always_listening::stop_always_listening_mode(
                app_handle.clone(),
                app_state.clone()
            ).await {
                warn!("[StopCoordinator] Failed to stop always listening: {}", e);
            } else {
                cleanup_results.push("Always listening stopped".to_string());
            }
            self.unregister_operation(&al_op_id).await;
        }

        // 5. Force reset monitoring states
        if let Some(monitor_op_id) = self.try_register_operation("monitor_reset").await {
            info!("[StopCoordinator] Resetting monitoring states");

            crate::agent_monitor::force_reset_agent_input_state().await;
            crate::dictation_monitor::force_reset_dictation_input_state().await;
            cleanup_results.push("Monitoring states reset".to_string());

            self.unregister_operation(&monitor_op_id).await;
        }

        // 6. Emit state update events (once only)
        self.emit_state_events(app_handle).await;
        cleanup_results.push("State events emitted".to_string());

        // 7. Update floating bar
        crate::commands::floating_bar::handle_backend_response(
            app_handle,
            "Stopped",
            Some(format!("All operations stopped: {}", reason))
        ).await;
        cleanup_results.push("Floating bar updated".to_string());

        // 8. CRITICAL: Force unregister escape key to release it back to other applications
        if let Some(escape_op_id) = self.try_register_operation("escape_key_cleanup").await {
            info!("[StopCoordinator] Force unregistering escape key to release to other applications");

            // Force reset the escape key coordinator to ensure complete cleanup
            let escape_coordinator = crate::commands::escape_key_coordinator::get_escape_key_coordinator();
            if let Err(e) = escape_coordinator.force_reset(app_handle).await {
                warn!("[StopCoordinator] Failed to force reset escape key coordinator: {}", e);
            } else {
                cleanup_results.push("Escape key released to other applications".to_string());
            }

            self.unregister_operation(&escape_op_id).await;
        }

        let result_summary = format!("Coordinated cleanup completed: [{}]", cleanup_results.join(", "));
        info!("[StopCoordinator] {}", result_summary);
        Ok(result_summary)
    }

    /// Emit state update events once only
    async fn emit_state_events(&self, app_handle: &AppHandle) {
        let events = [
            ("agent-active", false),
            ("dictation-active", false),
            ("always-listening-mode-changed", false),
        ];

        for (event_name, value) in events.iter() {
            self.emit_event(app_handle, event_name);
        }
    }

    fn emit_tts_stop_event(&self, app_handle: &AppHandle) {
        if let Err(e) = app_handle.emit(crate::constants::events::tts::STOP_REQUESTED, ()) {
            warn!("[StopCoordinator] Failed to emit TTS stop event: {}", e);
        }
    }

    fn emit_event(&self, app_handle: &AppHandle, event_name: &str) {
        if let Err(e) = app_handle.emit(event_name, ()) {
            warn!("[StopCoordinator] Failed to emit event '{}': {}", event_name, e);
        }
    }

    /// Emergency stop with immediate effect
    pub async fn emergency_stop(&self, app_handle: &AppHandle, reason: &str) -> Result<String, String> {
        warn!("[StopCoordinator] EMERGENCY STOP requested: {}", reason);

        // Set emergency flag
        self.emergency_stop_active.store(true, Ordering::SeqCst);

        // Force immediate cleanup regardless of timing
        self.cleanup_in_progress.store(false, Ordering::SeqCst); // Reset to allow emergency cleanup

        let result = self.stop_all_operations(app_handle, &format!("EMERGENCY: {}", reason)).await;

        // Reset emergency flag
        self.emergency_stop_active.store(false, Ordering::SeqCst);

        result
    }

    /// Get coordinator status for debugging
    pub async fn get_status(&self) -> serde_json::Value {
        let operations = self.active_operations.read().await;
        let cleanup_in_progress = self.cleanup_in_progress.load(Ordering::SeqCst);
        let emergency_active = self.emergency_stop_active.load(Ordering::SeqCst);

        let last_cleanup_time = if let Ok(guard) = self.last_cleanup.lock() {
            guard.map(|t| t.elapsed().as_millis())
        } else {
            None
        };

        serde_json::json!({
            "active_operations": operations.iter().collect::<Vec<_>>(),
            "cleanup_in_progress": cleanup_in_progress,
            "emergency_active": emergency_active,
            "last_cleanup_ms_ago": last_cleanup_time,
            "operation_count": operations.len()
        })
    }
}

// Global coordinator instance
static STOP_COORDINATOR: Lazy<StopCoordinator> =
    Lazy::new(|| StopCoordinator::new());

/// Get the global stop coordinator
pub fn get_stop_coordinator() -> &'static StopCoordinator {
    &STOP_COORDINATOR
}

/// Tauri command for coordinated stop operations
#[tauri::command]
pub async fn coordinated_stop_all_operations(app_handle: AppHandle, reason: Option<String>) -> Result<String, String> {
    let coordinator = get_stop_coordinator();
    let stop_reason = reason.unwrap_or_else(|| "Manual stop requested".to_string());
    coordinator.stop_all_operations(&app_handle, &stop_reason).await
}

/// Tauri command for emergency stop via coordinator
#[tauri::command]
pub async fn coordinator_emergency_stop_all_operations(app_handle: AppHandle, reason: Option<String>) -> Result<String, String> {
    let coordinator = get_stop_coordinator();
    let stop_reason = reason.unwrap_or_else(|| "Emergency stop requested".to_string());
    coordinator.emergency_stop(&app_handle, &stop_reason).await
}

/// Tauri command to get coordinator status
#[tauri::command]
pub async fn get_stop_coordinator_status() -> Result<serde_json::Value, String> {
    let coordinator = get_stop_coordinator();
    Ok(coordinator.get_status().await)
}
