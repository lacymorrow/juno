use std::sync::Arc;
use std::time::Instant;
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::{Mutex, RwLock};
use tracing::{info, warn, error, debug};
use serde::{Serialize, Deserialize};
use std::collections::HashMap;

/// Centralized dictation state that coordinates all components
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DictationState {
    Idle,
    Starting,
    Active { started_at: u64 },
    Stopping,
    Error { message: String },
    ForceResetting,
}

/// State change event with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateChangeEvent {
    pub previous_state: DictationState,
    pub new_state: DictationState,
    pub timestamp: u64,
    pub reason: String,
    pub component: String,
}

/// Component states for cross-validation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentStates {
    pub app_state_active: bool,
    pub voice_controller_active: bool,
    pub monitor_state_active: bool,
    pub floating_bar_state: String,
    pub last_updated: u64,
}

/// Global dictation state manager
pub struct DictationStateManager {
    current_state: Arc<RwLock<DictationState>>,
    component_states: Arc<RwLock<ComponentStates>>,
    state_history: Arc<Mutex<Vec<StateChangeEvent>>>,
    listeners: Arc<Mutex<HashMap<String, Box<dyn Fn(StateChangeEvent) + Send + Sync>>>>,
    inconsistency_threshold: u32,
    force_reset_in_progress: Arc<Mutex<bool>>,
}

impl DictationStateManager {
    pub fn new() -> Self {
        Self {
            current_state: Arc::new(RwLock::new(DictationState::Idle)),
            component_states: Arc::new(RwLock::new(ComponentStates {
                app_state_active: false,
                voice_controller_active: false,
                monitor_state_active: false,
                floating_bar_state: "default".to_string(),
                last_updated: Self::current_timestamp(),
            })),
            state_history: Arc::new(Mutex::new(Vec::new())),
            listeners: Arc::new(Mutex::new(HashMap::new())),
            inconsistency_threshold: 3, // Max inconsistencies before force reset
            force_reset_in_progress: Arc::new(Mutex::new(false)),
        }
    }

    /// Get current unified state
    pub async fn get_current_state(&self) -> DictationState {
        self.current_state.read().await.clone()
    }

    /// Transition to new state with validation and events
    pub async fn transition_to_state(
        &self,
        new_state: DictationState,
        reason: String,
        component: String,
        app_handle: &AppHandle,
    ) -> Result<(), String> {
        // Prevent transitions during force reset
        if *self.force_reset_in_progress.lock().await {
            debug!("[StateManager] Ignoring state transition during force reset: {:?}", new_state);
            return Ok(());
        }

        let previous_state = {
            let mut current = self.current_state.write().await;
            let prev = current.clone();
            *current = new_state.clone();
            prev
        };

        // Validate state transition
        if !self.is_valid_transition(&previous_state, &new_state) {
            error!("[StateManager] Invalid state transition from {:?} to {:?}", previous_state, new_state);
            return Err(format!("Invalid state transition from {:?} to {:?}", previous_state, new_state));
        }

        // Create state change event
        let event = StateChangeEvent {
            previous_state: previous_state.clone(),
            new_state: new_state.clone(),
            timestamp: Self::current_timestamp(),
            reason,
            component,
        };

        // Record in history
        self.state_history.lock().await.push(event.clone());

        // Emit unified state change event
        if let Err(e) = app_handle.emit("dictation-state-changed", &event) {
            error!("[StateManager] Failed to emit state change event: {}", e);
        }

        // Trigger component synchronization
        self.sync_all_components(app_handle, &new_state).await?;

        info!("[StateManager] State transition: {:?} -> {:?} ({})", previous_state, new_state, event.reason);
        Ok(())
    }

    /// Update component state and check for inconsistencies
    pub async fn update_component_state(
        &self,
        app_state_active: Option<bool>,
        voice_controller_active: Option<bool>,
        monitor_state_active: Option<bool>,
        floating_bar_state: Option<String>,
        app_handle: &AppHandle,
    ) -> Result<(), String> {
        let mut components = self.component_states.write().await;

        if let Some(active) = app_state_active {
            components.app_state_active = active;
        }
        if let Some(active) = voice_controller_active {
            components.voice_controller_active = active;
        }
        if let Some(active) = monitor_state_active {
            components.monitor_state_active = active;
        }
        if let Some(state) = floating_bar_state {
            components.floating_bar_state = state;
        }

        components.last_updated = Self::current_timestamp();

        // Check for inconsistencies
        let inconsistencies = self.detect_inconsistencies(&components).await;
        if inconsistencies.len() > self.inconsistency_threshold as usize {
            warn!("[StateManager] Detected {} inconsistencies, triggering auto-recovery", inconsistencies.len());
            self.auto_recover_from_inconsistencies(app_handle, inconsistencies).await?;
        }

        Ok(())
    }

    /// Comprehensive force reset that coordinates all components
    pub async fn force_reset_all_state(&self, app_handle: &AppHandle, reason: String) -> Result<String, String> {
        // Set force reset flag to prevent race conditions
        *self.force_reset_in_progress.lock().await = true;

        warn!("[StateManager] Starting comprehensive state reset: {}", reason);

        // 1. Reset voice controller with timeout
        let voice_reset_result = self.reset_voice_controller(app_handle).await;

        // 2. Reset dictation monitor
        crate::dictation_monitor::force_reset_dictation_input_state().await;

        // 3. Reset app state
        let app_state = app_handle.state::<crate::state::AppState>();
        if let Ok(mut dictation_active) = app_state.dictation_active.lock() {
            *dictation_active = false;
        }

        // 4. Reset floating bar
        crate::commands::floating_bar::handle_dictation_mode_change(app_handle, false).await;

        // 5. Reset manager state
        *self.current_state.write().await = DictationState::Idle;
        *self.component_states.write().await = ComponentStates {
            app_state_active: false,
            voice_controller_active: false,
            monitor_state_active: false,
            floating_bar_state: "default".to_string(),
            last_updated: Self::current_timestamp(),
        };

        // 6. Emit comprehensive reset events
        if let Err(e) = app_handle.emit("dictation-active", false) {
            error!("[StateManager] Failed to emit dictation-active event: {}", e);
        }

        if let Err(e) = app_handle.emit("dictation-state-force-reset", &reason) {
            error!("[StateManager] Failed to emit force reset event: {}", e);
        }

        // Clear force reset flag
        *self.force_reset_in_progress.lock().await = false;

        let summary = format!(
            "Comprehensive state reset completed. Voice controller: {:?}, Monitor: reset, App state: reset, UI: reset",
            voice_reset_result
        );

        info!("[StateManager] {}", summary);
        Ok(summary)
    }

    /// Get comprehensive status for debugging
    pub async fn get_comprehensive_status(&self, app_handle: &AppHandle) -> serde_json::Value {
        let current_state = self.current_state.read().await.clone();
        let component_states = self.component_states.read().await.clone();

        // Get real-time component states
        let app_state = app_handle.state::<crate::state::AppState>();
        let real_app_state = app_state.dictation_active.lock()
            .map(|active| *active)
            .unwrap_or(false);

        let real_voice_state = match app_handle.try_state::<Arc<std::sync::Mutex<tauri_plugin_voice_transcription::controller::VoiceController>>>() {
            Some(controller_state) => {
                controller_state.lock()
                    .map(|controller| controller.is_dictating())
                    .unwrap_or(false)
            }
            None => false
        };

        let inconsistencies = self.detect_real_time_inconsistencies(
            &current_state,
            &component_states,
            real_app_state,
            real_voice_state,
        ).await;

        serde_json::json!({
            "unified_state": current_state,
            "component_states": component_states,
            "real_time_states": {
                "app_state_active": real_app_state,
                "voice_controller_active": real_voice_state
            },
            "inconsistencies": inconsistencies,
            "state_consistent": inconsistencies.is_empty(),
            "force_reset_in_progress": *self.force_reset_in_progress.lock().await,
            "timestamp": Self::current_timestamp()
        })
    }

    // Private helper methods

    async fn reset_voice_controller(&self, app_handle: &AppHandle) -> Result<(), String> {
        match app_handle.try_state::<Arc<std::sync::Mutex<tauri_plugin_voice_transcription::controller::VoiceController>>>() {
            Some(controller_state) => {
                let stop_result = tokio::time::timeout(
                    std::time::Duration::from_secs(3),
                    tauri_plugin_voice_transcription::commands::stop_dictation(
                        app_handle.clone(),
                        controller_state
                    )
                ).await;

                match stop_result {
                    Ok(Ok(_)) => {
                        info!("[StateManager] Voice controller reset successfully");
                        Ok(())
                    }
                    Ok(Err(e)) => {
                        error!("[StateManager] Voice controller reset failed: {}", e);
                        Err(format!("Voice controller reset failed: {}", e))
                    }
                    Err(_) => {
                        error!("[StateManager] Voice controller reset timed out - may be deadlocked");
                        Err("Voice controller reset timed out".to_string())
                    }
                }
            }
            None => {
                warn!("[StateManager] Voice controller not available for reset");
                Ok(())
            }
        }
    }

    async fn sync_all_components(&self, app_handle: &AppHandle, target_state: &DictationState) -> Result<(), String> {
        let is_active = matches!(target_state, DictationState::Active { .. });

        // Sync app state
        let app_state = app_handle.state::<crate::state::AppState>();
        if let Ok(mut dictation_active) = app_state.dictation_active.lock() {
            *dictation_active = is_active;
        }

        // Handle escape key registration/unregistration
        if is_active {
            // Register escape key when dictation becomes active
            if let Err(e) = crate::commands::shortcuts::register_escape_key_handler(app_handle.clone()).await {
                warn!("[StateManager] Failed to register escape key for dictation: {}", e);
            } else {
                info!("[StateManager] Registered escape key for dictation");
            }
        } else {
            // Unregister escape key when dictation becomes inactive
            if let Err(e) = crate::commands::shortcuts::unregister_escape_key_handler(app_handle.clone()).await {
                warn!("[StateManager] Failed to unregister escape key for dictation: {}", e);
            } else {
                info!("[StateManager] Unregistered escape key for dictation");
            }
        }

        // Sync floating bar
        crate::commands::floating_bar::handle_dictation_mode_change(app_handle, is_active).await;

        // Emit state events
        if let Err(e) = app_handle.emit("dictation-active", is_active) {
            error!("[StateManager] Failed to emit dictation-active event: {}", e);
        }

        Ok(())
    }

    async fn detect_inconsistencies(&self, components: &ComponentStates) -> Vec<String> {
        let mut inconsistencies = Vec::new();
        let current_state = self.current_state.read().await;

        let should_be_active = matches!(*current_state, DictationState::Active { .. });

        if components.app_state_active != should_be_active {
            inconsistencies.push(format!(
                "App state mismatch: expected {}, got {}",
                should_be_active, components.app_state_active
            ));
        }

        if components.voice_controller_active != should_be_active {
            inconsistencies.push(format!(
                "Voice controller mismatch: expected {}, got {}",
                should_be_active, components.voice_controller_active
            ));
        }

        inconsistencies
    }

    async fn detect_real_time_inconsistencies(
        &self,
        unified_state: &DictationState,
        component_states: &ComponentStates,
        real_app_state: bool,
        real_voice_state: bool,
    ) -> Vec<String> {
        let mut inconsistencies = Vec::new();
        let should_be_active = matches!(unified_state, DictationState::Active { .. });

        if real_app_state != should_be_active {
            inconsistencies.push(format!("App state real-time mismatch"));
        }

        if real_voice_state != should_be_active {
            inconsistencies.push(format!("Voice controller real-time mismatch"));
        }

        if component_states.app_state_active != real_app_state {
            inconsistencies.push(format!("App state tracking drift"));
        }

        if component_states.voice_controller_active != real_voice_state {
            inconsistencies.push(format!("Voice controller tracking drift"));
        }

        inconsistencies
    }

    async fn auto_recover_from_inconsistencies(
        &self,
        app_handle: &AppHandle,
        inconsistencies: Vec<String>,
    ) -> Result<(), String> {
        warn!("[StateManager] Auto-recovering from inconsistencies: {:?}", inconsistencies);

        self.force_reset_all_state(
            app_handle,
            format!("Auto-recovery triggered by {} inconsistencies", inconsistencies.len())
        ).await?;

        Ok(())
    }

    fn is_valid_transition(&self, from: &DictationState, to: &DictationState) -> bool {
        use DictationState::*;
        match (from, to) {
            (Idle, Starting) => true,
            (Starting, Active { .. }) => true,
            (Starting, Error { .. }) => true,
            (Active { .. }, Stopping) => true,
            (Active { .. }, Error { .. }) => true,
            (Stopping, Idle) => true,
            (Error { .. }, Idle) => true,
            (_, ForceResetting) => true,
            (ForceResetting, Idle) => true,
            _ => false,
        }
    }

    fn current_timestamp() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64
    }
}

// Global state manager instance
static DICTATION_STATE_MANAGER: once_cell::sync::Lazy<DictationStateManager> =
    once_cell::sync::Lazy::new(|| DictationStateManager::new());

// Public API functions

/// Get the global state manager
pub fn get_state_manager() -> &'static DictationStateManager {
    &DICTATION_STATE_MANAGER
}

/// Unified force reset command
#[tauri::command]
pub async fn force_reset_dictation_state(
    app: AppHandle,
    reason: Option<String>,
) -> Result<String, String> {
    let manager = get_state_manager();
    let reset_reason = reason.unwrap_or_else(|| "Manual force reset requested".to_string());

    manager.force_reset_all_state(&app, reset_reason).await
}

/// Get comprehensive dictation status
#[tauri::command]
pub async fn get_dictation_comprehensive_status(
    app: AppHandle,
) -> Result<serde_json::Value, String> {
    let manager = get_state_manager();
    Ok(manager.get_comprehensive_status(&app).await)
}

/// Update component state (called by individual components)
#[tauri::command]
pub async fn update_dictation_component_state(
    app: AppHandle,
    component: String,
    app_state_active: Option<bool>,
    voice_controller_active: Option<bool>,
    monitor_state_active: Option<bool>,
    floating_bar_state: Option<String>,
) -> Result<(), String> {
    let manager = get_state_manager();

    manager.update_component_state(
        app_state_active,
        voice_controller_active,
        monitor_state_active,
        floating_bar_state,
        &app,
    ).await?;

    info!("[StateManager] Component '{}' updated state", component);
    Ok(())
}

/// Transition to new state (called by individual components)
#[tauri::command]
pub async fn transition_dictation_state(
    app: AppHandle,
    new_state: String,
    reason: String,
    component: String,
) -> Result<(), String> {
    let manager = get_state_manager();

    let state = match new_state.as_str() {
        "idle" => DictationState::Idle,
        "starting" => DictationState::Starting,
        "active" => DictationState::Active { started_at: DictationStateManager::current_timestamp() },
        "stopping" => DictationState::Stopping,
        "force_resetting" => DictationState::ForceResetting,
        error_msg if error_msg.starts_with("error:") => {
            DictationState::Error { message: error_msg[6..].to_string() }
        }
        _ => return Err(format!("Invalid state: {}", new_state)),
    };

    manager.transition_to_state(state, reason, component, &app).await
}

/// Force stop dictation (convenience function for stop operations)
pub async fn force_stop_dictation(app_handle: &AppHandle) -> Result<(), String> {
    info!("[DictationStateManager] Force stopping dictation");
    
    let manager = get_state_manager();
    
    // Force reset all dictation state
    manager.force_reset_all_state(
        app_handle,
        "Force stop dictation requested".to_string()
    ).await?;
    
    info!("[DictationStateManager] Force stop dictation completed");
    Ok(())
}

/// Synchronize dictation state (for internal use)
pub async fn sync_dictation_state(active: bool) -> Result<(), String> {
    let manager = get_state_manager();
    
    let target_state = if active {
        DictationState::Active { started_at: DictationStateManager::current_timestamp() }
    } else {
        DictationState::Idle
    };
    
    // Note: This is a simplified sync - in practice you'd need an app_handle
    // This function is mainly for internal coordination
    info!("[DictationStateManager] Syncing dictation state to: {:?}", target_state);
    Ok(())
}
