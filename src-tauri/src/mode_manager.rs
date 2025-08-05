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
pub async fn get_mode_status() -> Result<serde_json::Value, String> {
    Ok(get_mode_manager().get_status().await)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    /// Helper to create a test mode manager instance
    fn create_test_manager() -> ModeManager {
        ModeManager::new()
    }

    #[tokio::test]
    async fn test_initial_state() {
        let manager = create_test_manager();
        
        // Should start in idle mode
        assert_eq!(manager.get_mode().await, AppMode::Idle);
        
        // Config should have sensible defaults
        let config = manager.get_config().await;
        assert!(!config.always_listening_enabled);
        assert_eq!(config.wake_words, vec!["hey juno".to_string(), "ok juno".to_string()]);
        assert_eq!(config.wake_sensitivity, 0.5);
        
        // History should be empty
        let status = manager.get_status().await;
        assert_eq!(status["history"].as_array().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn test_valid_transitions() {
        let manager = create_test_manager();
        
        // For these tests, we'll simulate the app handle behavior
        // by checking mode changes without actual Tauri app
        
        // Test that mode can be set
        {
            let mut mode = manager.current_mode.write().await;
            *mode = AppMode::Agent;
        }
        assert_eq!(manager.get_mode().await, AppMode::Agent);
        
        // Test transition back to idle
        {
            let mut mode = manager.current_mode.write().await;
            *mode = AppMode::Idle;
        }
        assert_eq!(manager.get_mode().await, AppMode::Idle);
        
        // Test transition to dictation
        {
            let mut mode = manager.current_mode.write().await;
            *mode = AppMode::Dictation;
        }
        assert_eq!(manager.get_mode().await, AppMode::Dictation);
    }

    #[tokio::test]
    async fn test_invalid_transitions() {
        let manager = create_test_manager();
        
        // Test Agent -> Dictation (should fail validation)
        assert!(!manager.is_valid_transition(AppMode::Agent, AppMode::Dictation));
        
        // Test Dictation -> Agent (should fail validation)
        assert!(!manager.is_valid_transition(AppMode::Dictation, AppMode::Agent));
        
        // Test valid transitions
        assert!(manager.is_valid_transition(AppMode::Idle, AppMode::Agent));
        assert!(manager.is_valid_transition(AppMode::Idle, AppMode::Dictation));
        assert!(manager.is_valid_transition(AppMode::Agent, AppMode::Idle));
        assert!(manager.is_valid_transition(AppMode::Dictation, AppMode::Idle));
    }

    #[tokio::test]
    async fn test_same_mode_transition() {
        let manager = create_test_manager();
        
        // Same mode transitions should be valid
        assert!(manager.is_valid_transition(AppMode::Idle, AppMode::Idle));
        assert!(manager.is_valid_transition(AppMode::Agent, AppMode::Agent));
        assert!(manager.is_valid_transition(AppMode::Dictation, AppMode::Dictation));
    }

    #[tokio::test]
    async fn test_transition_history() {
        let manager = create_test_manager();
        
        // Add some transitions to history
        {
            let mut history = manager.history.write().await;
            history.push(ModeTransition {
                from: AppMode::Idle,
                to: AppMode::Agent,
                reason: "Test 1".to_string(),
                timestamp: 1000,
            });
            history.push(ModeTransition {
                from: AppMode::Agent,
                to: AppMode::Idle,
                reason: "Test 2".to_string(),
                timestamp: 2000,
            });
        }
        
        // Check history
        let status = manager.get_status().await;
        let history = status["history"].as_array().unwrap();
        assert_eq!(history.len(), 2);
        
        // Verify transitions are in reverse order (most recent first)
        assert_eq!(history[0]["reason"], "Test 2");
        assert_eq!(history[1]["reason"], "Test 1");
    }

    #[tokio::test]
    async fn test_history_size_limit() {
        let manager = create_test_manager();
        
        // Add more than 100 transitions
        {
            let mut history = manager.history.write().await;
            for i in 0..105 {
                history.push(ModeTransition {
                    from: AppMode::Idle,
                    to: AppMode::Agent,
                    reason: format!("Transition {}", i),
                    timestamp: i as u64,
                });
            }
        }
        
        // History should still have 105 (limit is applied during transition)
        let history_len = manager.history.read().await.len();
        assert_eq!(history_len, 105);
    }

    #[tokio::test]
    async fn test_config_updates() {
        let manager = create_test_manager();
        
        // Update config
        manager.update_config(|config| {
            config.always_listening_enabled = true;
            config.wake_words = vec!["hello world".to_string()];
            config.wake_sensitivity = 0.8;
        }).await.unwrap();
        
        // Verify updates
        let config = manager.get_config().await;
        assert!(config.always_listening_enabled);
        assert_eq!(config.wake_words, vec!["hello world".to_string()]);
        assert_eq!(config.wake_sensitivity, 0.8);
    }

    #[tokio::test]
    async fn test_wake_word_detection_config() {
        let manager = create_test_manager();
        
        // Test wake word detection with different configs
        let mut config = manager.get_config().await;
        
        // Default config
        assert_eq!(config.wake_words.len(), 2);
        assert!(config.wake_words.contains(&"hey juno".to_string()));
        assert!(config.wake_words.contains(&"ok juno".to_string()));
        
        // Update wake words
        manager.update_config(|c| {
            c.wake_words = vec!["hello assistant".to_string(), "wake up".to_string()];
        }).await.unwrap();
        
        config = manager.get_config().await;
        assert_eq!(config.wake_words.len(), 2);
        assert!(config.wake_words.contains(&"hello assistant".to_string()));
        assert!(config.wake_words.contains(&"wake up".to_string()));
    }

    #[tokio::test]
    async fn test_concurrent_mode_access() {
        let manager = Arc::new(create_test_manager());
        
        // Test concurrent reads
        let manager1 = Arc::clone(&manager);
        let manager2 = Arc::clone(&manager);
        
        let handle1 = tokio::spawn(async move {
            manager1.get_mode().await
        });
        
        let handle2 = tokio::spawn(async move {
            manager2.get_mode().await
        });
        
        let mode1 = handle1.await.unwrap();
        let mode2 = handle2.await.unwrap();
        
        // Both should read the same mode
        assert_eq!(mode1, mode2);
        assert_eq!(mode1, AppMode::Idle);
    }

    #[tokio::test]
    async fn test_mode_status_structure() {
        let manager = create_test_manager();
        
        // Add a transition for testing
        {
            let mut history = manager.history.write().await;
            history.push(ModeTransition {
                from: AppMode::Idle,
                to: AppMode::Agent,
                reason: "Status test".to_string(),
                timestamp: 12345,
            });
        }
        
        // Check status structure
        let status = manager.get_status().await;
        
        assert!(status.get("current_mode").is_some());
        assert!(status.get("config").is_some());
        assert!(status.get("history").is_some());
        assert!(status.get("timestamp").is_some());
        
        // Verify current mode
        assert_eq!(status["current_mode"], "idle");
        
        // Verify timestamp is reasonable
        let timestamp = status["timestamp"].as_u64().unwrap();
        assert!(timestamp > 0);
    }

    #[tokio::test]
    async fn test_mode_serialization() {
        // Test that AppMode serializes correctly
        let mode = AppMode::Agent;
        let serialized = serde_json::to_string(&mode).unwrap();
        assert_eq!(serialized, "\"agent\"");
        
        let mode = AppMode::Dictation;
        let serialized = serde_json::to_string(&mode).unwrap();
        assert_eq!(serialized, "\"dictation\"");
        
        let mode = AppMode::Idle;
        let serialized = serde_json::to_string(&mode).unwrap();
        assert_eq!(serialized, "\"idle\"");
    }

    #[tokio::test]
    async fn test_config_serialization() {
        let config = ModeConfig::default();
        let serialized = serde_json::to_string(&config).unwrap();
        let deserialized: ModeConfig = serde_json::from_str(&serialized).unwrap();
        
        assert_eq!(config.always_listening_enabled, deserialized.always_listening_enabled);
        assert_eq!(config.wake_words, deserialized.wake_words);
        assert_eq!(config.wake_sensitivity, deserialized.wake_sensitivity);
    }

    #[tokio::test]
    async fn test_concurrent_config_updates() {
        let manager = Arc::new(create_test_manager());
        
        // Test concurrent configuration updates
        let manager1 = Arc::clone(&manager);
        let handle1 = tokio::spawn(async move {
            manager1.update_config(|config| {
                config.wake_sensitivity = 0.7;
            }).await
        });
        
        let manager2 = Arc::clone(&manager);
        let handle2 = tokio::spawn(async move {
            manager2.update_config(|config| {
                config.always_listening_enabled = true;
            }).await
        });
        
        // Both updates should succeed
        assert!(handle1.await.unwrap().is_ok());
        assert!(handle2.await.unwrap().is_ok());
        
        // Verify both updates were applied
        let config = manager.get_config().await;
        assert_eq!(config.wake_sensitivity, 0.7);
        assert!(config.always_listening_enabled);
    }

    #[tokio::test]
    async fn test_mode_transition_edge_cases() {
        let manager = create_test_manager();
        
        // Test empty reason string
        {
            let mut history = manager.history.write().await;
            history.push(ModeTransition {
                from: AppMode::Idle,
                to: AppMode::Agent,
                reason: "".to_string(),
                timestamp: 1000,
            });
        }
        
        let status = manager.get_status().await;
        let history = status["history"].as_array().unwrap();
        assert_eq!(history[0]["reason"], "");
        
        // Test very long reason string
        let long_reason = "a".repeat(1000);
        {
            let mut history = manager.history.write().await;
            history.push(ModeTransition {
                from: AppMode::Agent,
                to: AppMode::Idle,
                reason: long_reason.clone(),
                timestamp: 2000,
            });
        }
        
        let status = manager.get_status().await;
        let history = status["history"].as_array().unwrap();
        assert_eq!(history[0]["reason"].as_str().unwrap().len(), 1000);
    }

    #[tokio::test]
    async fn test_wake_sensitivity_bounds() {
        let manager = create_test_manager();
        
        // Test sensitivity at boundaries
        manager.update_config(|config| {
            config.wake_sensitivity = 0.0;
        }).await.unwrap();
        assert_eq!(manager.get_config().await.wake_sensitivity, 0.0);
        
        manager.update_config(|config| {
            config.wake_sensitivity = 1.0;
        }).await.unwrap();
        assert_eq!(manager.get_config().await.wake_sensitivity, 1.0);
        
        // Test values outside bounds (should be allowed, validation is elsewhere)
        manager.update_config(|config| {
            config.wake_sensitivity = -0.5;
        }).await.unwrap();
        assert_eq!(manager.get_config().await.wake_sensitivity, -0.5);
        
        manager.update_config(|config| {
            config.wake_sensitivity = 1.5;
        }).await.unwrap();
        assert_eq!(manager.get_config().await.wake_sensitivity, 1.5);
    }
}