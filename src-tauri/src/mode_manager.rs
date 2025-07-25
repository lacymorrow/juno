//! # Mode Manager
//! 
//! Centralized mode management for Juno application.
//! This module provides a clean, state machine-based approach to handling
//! the different operational modes of the application.

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

/// The operational modes of the application
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppMode {
    /// Default idle state - no active mode
    Idle,
    /// Agent mode - voice commands are sent to AI agent
    Agent,
    /// Dictation mode - voice is transcribed to text at cursor
    Dictation,
}

/// Configuration for mode behavior
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModeConfig {
    /// Whether always listening is enabled (triggers agent mode on wake word)
    pub always_listening_enabled: bool,
    /// Wake words for always listening mode
    pub wake_words: Vec<String>,
    /// Sensitivity for wake word detection (0.0 - 1.0)
    pub wake_sensitivity: f32,
}

impl Default for ModeConfig {
    fn default() -> Self {
        Self {
            always_listening_enabled: false,
            wake_words: vec!["hey juno".to_string(), "ok juno".to_string()],
            wake_sensitivity: 0.5,
        }
    }
}

/// Mode transition request
#[derive(Debug, Clone)]
pub struct ModeTransition {
    pub from: AppMode,
    pub to: AppMode,
    pub reason: String,
    pub timestamp: u64,
}

/// The centralized mode manager
pub struct ModeManager {
    /// Current active mode
    current_mode: Arc<RwLock<AppMode>>,
    /// Mode configuration
    config: Arc<RwLock<ModeConfig>>,
    /// Transition history for debugging
    history: Arc<RwLock<Vec<ModeTransition>>>,
}

impl ModeManager {
    /// Create a new mode manager
    pub fn new() -> Self {
        Self {
            current_mode: Arc::new(RwLock::new(AppMode::Idle)),
            config: Arc::new(RwLock::new(ModeConfig::default())),
            history: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Get the current mode
    pub async fn get_mode(&self) -> AppMode {
        *self.current_mode.read().await
    }

    /// Get the mode configuration
    pub async fn get_config(&self) -> ModeConfig {
        self.config.read().await.clone()
    }

    /// Update mode configuration
    pub async fn update_config<F>(&self, updater: F) -> Result<(), String>
    where
        F: FnOnce(&mut ModeConfig),
    {
        let mut config = self.config.write().await;
        updater(&mut config);
        info!("Mode configuration updated: {:?}", *config);
        Ok(())
    }

    /// Transition to a new mode
    pub async fn transition_to(
        &self,
        new_mode: AppMode,
        reason: String,
        app_handle: &AppHandle,
    ) -> Result<(), String> {
        let current = *self.current_mode.read().await;
        
        // Check if transition is valid
        if !self.is_valid_transition(current, new_mode) {
            return Err(format!(
                "Invalid mode transition from {:?} to {:?}",
                current, new_mode
            ));
        }

        // If already in the target mode, no-op
        if current == new_mode {
            debug!("Already in mode {:?}, skipping transition", new_mode);
            return Ok(());
        }

        info!("Mode transition: {:?} -> {:?} ({})", current, new_mode, reason);

        // Perform pre-transition cleanup
        self.cleanup_current_mode(current, app_handle).await?;

        // Update the mode
        {
            let mut mode = self.current_mode.write().await;
            *mode = new_mode;
        }

        // Record transition
        {
            let mut history = self.history.write().await;
            history.push(ModeTransition {
                from: current,
                to: new_mode,
                reason: reason.clone(),
                timestamp: crate::utils::current_timestamp_ms(),
            });
            
            // Keep only last 100 transitions
            if history.len() > 100 {
                history.drain(0..history.len() - 100);
            }
        }

        // Setup new mode
        self.setup_new_mode(new_mode, app_handle).await?;

        // Emit mode change event
        if let Err(e) = app_handle.emit("mode:changed", serde_json::json!({
            "from": current,
            "to": new_mode,
            "reason": reason,
        })) {
            error!("Failed to emit mode change event: {}", e);
        }

        Ok(())
    }

    /// Check if a mode transition is valid
    fn is_valid_transition(&self, from: AppMode, to: AppMode) -> bool {
        match (from, to) {
            // Can always go to idle
            (_, AppMode::Idle) => true,
            // Can go from idle to any mode
            (AppMode::Idle, _) => true,
            // Cannot transition between agent and dictation directly
            (AppMode::Agent, AppMode::Dictation) | (AppMode::Dictation, AppMode::Agent) => false,
            // Same mode transitions are allowed (no-op)
            _ => true,
        }
    }

    /// Cleanup when leaving a mode
    async fn cleanup_current_mode(
        &self,
        mode: AppMode,
        app_handle: &AppHandle,
    ) -> Result<(), String> {
        match mode {
            AppMode::Agent => {
                // Stop any active agent execution
                if let Err(e) = self.stop_agent_mode(app_handle).await {
                    warn!("Error stopping agent mode: {}", e);
                }
            }
            AppMode::Dictation => {
                // Stop any active dictation
                if let Err(e) = self.stop_dictation_mode(app_handle).await {
                    warn!("Error stopping dictation mode: {}", e);
                }
            }
            AppMode::Idle => {
                // Nothing to cleanup
            }
        }
        Ok(())
    }

    /// Setup when entering a new mode
    async fn setup_new_mode(
        &self,
        mode: AppMode,
        app_handle: &AppHandle,
    ) -> Result<(), String> {
        match mode {
            AppMode::Agent => {
                self.start_agent_mode(app_handle).await?;
            }
            AppMode::Dictation => {
                self.start_dictation_mode(app_handle).await?;
            }
            AppMode::Idle => {
                // Check if always listening should be active
                let config = self.config.read().await;
                if config.always_listening_enabled {
                    self.start_always_listening(app_handle).await?;
                }
            }
        }
        Ok(())
    }

    /// Start agent mode
    async fn start_agent_mode(&self, app_handle: &AppHandle) -> Result<(), String> {
        info!("Starting agent mode");
        
        // Register escape key handler
        crate::commands::shortcuts::register_escape_key_handler(app_handle.clone()).await?;
        
        // Start voice transcription for agent
        if let Some(controller) = app_handle.try_state::<Arc<std::sync::Mutex<tauri_plugin_voice_transcription::controller::VoiceController>>>() {
            tauri_plugin_voice_transcription::commands::start_dictation(
                app_handle.clone(),
                controller,
            ).await.map_err(|e| format!("Failed to start voice transcription: {}", e))?;
        }
        
        Ok(())
    }

    /// Stop agent mode
    async fn stop_agent_mode(&self, app_handle: &AppHandle) -> Result<(), String> {
        info!("Stopping agent mode");
        
        // Cancel any active agent execution
        let app_state = app_handle.state::<crate::state::AppState>();
        app_state.cancel();
        
        // Stop voice transcription
        if let Some(controller) = app_handle.try_state::<Arc<std::sync::Mutex<tauri_plugin_voice_transcription::controller::VoiceController>>>() {
            let _ = tauri_plugin_voice_transcription::commands::stop_dictation(
                app_handle.clone(),
                controller,
            ).await;
        }
        
        // Unregister escape key
        let _ = crate::commands::shortcuts::unregister_escape_key_handler(app_handle.clone()).await;
        
        Ok(())
    }

    /// Start dictation mode
    async fn start_dictation_mode(&self, app_handle: &AppHandle) -> Result<(), String> {
        info!("Starting dictation mode");
        
        // Register escape key handler
        crate::commands::shortcuts::register_escape_key_handler(app_handle.clone()).await?;
        
        // Start voice transcription for dictation
        if let Some(controller) = app_handle.try_state::<Arc<std::sync::Mutex<tauri_plugin_voice_transcription::controller::VoiceController>>>() {
            tauri_plugin_voice_transcription::commands::start_dictation(
                app_handle.clone(),
                controller,
            ).await.map_err(|e| format!("Failed to start dictation: {}", e))?;
        }
        
        Ok(())
    }

    /// Stop dictation mode
    async fn stop_dictation_mode(&self, app_handle: &AppHandle) -> Result<(), String> {
        info!("Stopping dictation mode");
        
        // Stop voice transcription
        if let Some(controller) = app_handle.try_state::<Arc<std::sync::Mutex<tauri_plugin_voice_transcription::controller::VoiceController>>>() {
            let _ = tauri_plugin_voice_transcription::commands::stop_dictation(
                app_handle.clone(),
                controller,
            ).await;
        }
        
        // Unregister escape key
        let _ = crate::commands::shortcuts::unregister_escape_key_handler(app_handle.clone()).await;
        
        Ok(())
    }

    /// Start always listening
    async fn start_always_listening(&self, app_handle: &AppHandle) -> Result<(), String> {
        info!("Starting always listening");
        
        let app_state = app_handle.state::<crate::state::AppState>();
        crate::commands::always_listening::start_always_listening_mode(
            app_handle.clone(),
            app_state,
        ).await.map(|_| ())
    }

    /// Handle wake word detection from always listening
    pub async fn handle_wake_word_detected(
        &self,
        app_handle: &AppHandle,
    ) -> Result<(), String> {
        let current = self.get_mode().await;
        
        // Only trigger agent mode if we're idle
        if current == AppMode::Idle {
            self.transition_to(
                AppMode::Agent,
                "Wake word detected".to_string(),
                app_handle,
            ).await?;
        } else {
            debug!("Wake word detected but not in idle mode, ignoring");
        }
        
        Ok(())
    }

    /// Get mode status for debugging
    pub async fn get_status(&self) -> serde_json::Value {
        let mode = self.get_mode().await;
        let config = self.get_config().await;
        let history = self.history.read().await;
        
        serde_json::json!({
            "current_mode": mode,
            "config": config,
            "history": history.iter().rev().take(10).collect::<Vec<_>>(),
            "timestamp": crate::utils::current_timestamp_ms(),
        })
    }
}

// Global mode manager instance
static MODE_MANAGER: once_cell::sync::Lazy<ModeManager> =
    once_cell::sync::Lazy::new(|| ModeManager::new());

/// Get the global mode manager
pub fn get_mode_manager() -> &'static ModeManager {
    &MODE_MANAGER
}

// Tauri commands for mode management

#[tauri::command]
pub async fn get_current_mode() -> Result<AppMode, String> {
    Ok(get_mode_manager().get_mode().await)
}

#[tauri::command]
pub async fn set_mode(
    mode: AppMode,
    reason: String,
    app: AppHandle,
) -> Result<(), String> {
    get_mode_manager().transition_to(mode, reason, &app).await
}

#[tauri::command]
pub async fn get_mode_config() -> Result<ModeConfig, String> {
    Ok(get_mode_manager().get_config().await)
}

#[tauri::command]
pub async fn set_always_listening_enabled(
    enabled: bool,
    app: AppHandle,
) -> Result<(), String> {
    let manager = get_mode_manager();
    
    // Update config
    manager.update_config(|config| {
        config.always_listening_enabled = enabled;
    }).await?;
    
    // If enabling and we're idle, start always listening
    if enabled && manager.get_mode().await == AppMode::Idle {
        let app_state = app.state::<crate::state::AppState>();
        crate::commands::always_listening::start_always_listening_mode(app.clone(), app_state).await?;
    }
    // If disabling, stop always listening
    else if !enabled {
        let app_state = app.state::<crate::state::AppState>();
        crate::commands::always_listening::stop_always_listening_mode(app.clone(), app_state).await?;
    }
    
    Ok(())
}

#[tauri::command]
pub async def get_mode_status() -> Result<serde_json::Value, String> {
    Ok(get_mode_manager().get_status().await)
}