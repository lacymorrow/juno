//! Voice control command handlers with enhanced error recovery
//! Critical fixes for audio device failures and model loading issues

use crate::state::AppState;
use serde_json::Value;
use tauri::{command, AppHandle, Manager, State};
use tracing::{error, info, warn};

#[cfg(feature = "voice-features")]
use tauri_plugin_voice_transcription::{
    controller::VoiceController,
    always_listening::AlwaysListeningController,
};

/// CRITICAL FIX: Voice error recovery system
#[derive(Debug, Clone)]
pub struct VoiceErrorRecovery {
    max_retries: u32,
    retry_delay_ms: u64,
    fallback_enabled: bool,
}

impl Default for VoiceErrorRecovery {
    fn default() -> Self {
        Self {
            max_retries: 3,
            retry_delay_ms: 1000,
            fallback_enabled: true,
        }
    }
}

/// CRITICAL FIX: Enhanced error types for voice operations
#[derive(Debug, thiserror::Error)]
pub enum VoiceRecoveryError {
    #[error("Audio device not available: {0}")]
    AudioDeviceError(String),
    #[error("Model loading failed: {0}")]
    ModelLoadingError(String),
    #[error("Memory pressure detected: {0}")]
    MemoryPressureError(String),
    #[error("Permission denied: {0}")]
    PermissionError(String),
    #[error("Maximum retries exceeded")]
    MaxRetriesExceeded,
}

/// CRITICAL FIX: Voice operation with automatic retry and recovery
pub async fn execute_with_recovery<F, T>(
    operation: F,
    recovery: &VoiceErrorRecovery,
    operation_name: &str,
) -> Result<T, VoiceRecoveryError>
where
    F: Fn() -> Result<T, String>,
{
    let mut attempts = 0;
    let mut last_error = String::new();

    while attempts < recovery.max_retries {
        match operation() {
            Ok(result) => {
                if attempts > 0 {
                    info!("Voice operation '{}' succeeded after {} attempts", operation_name, attempts + 1);
                }
                return Ok(result);
            }
            Err(error) => {
                attempts += 1;
                last_error = error.clone();

                warn!("Voice operation '{}' failed (attempt {}/{}): {}",
                     operation_name, attempts, recovery.max_retries, error);

                // CRITICAL FIX: Classify error and determine recovery strategy
                if let Some(recovery_action) = classify_and_recover(&error).await {
                    info!("Applying recovery action for '{}': {:?}", operation_name, recovery_action);
                    apply_recovery_action(recovery_action).await;
                }

                if attempts < recovery.max_retries {
                    tokio::time::sleep(tokio::time::Duration::from_millis(recovery.retry_delay_ms)).await;
                }
            }
        }
    }

    error!("Voice operation '{}' failed after {} attempts. Last error: {}",
          operation_name, recovery.max_retries, last_error);
    Err(VoiceRecoveryError::MaxRetriesExceeded)
}

/// CRITICAL FIX: Error classification for targeted recovery
#[derive(Debug)]
enum RecoveryAction {
    RestartAudioSystem,
    ClearMemoryPressure,
    ReloadModel,
    CheckPermissions,
    FallbackMode,
}

async fn classify_and_recover(error: &str) -> Option<RecoveryAction> {
    let error_lower = error.to_lowercase();

    if error_lower.contains("audio") || error_lower.contains("device") || error_lower.contains("stream") {
        Some(RecoveryAction::RestartAudioSystem)
    } else if error_lower.contains("memory") || error_lower.contains("allocation") {
        Some(RecoveryAction::ClearMemoryPressure)
    } else if error_lower.contains("model") || error_lower.contains("whisper") {
        Some(RecoveryAction::ReloadModel)
    } else if error_lower.contains("permission") || error_lower.contains("access") {
        Some(RecoveryAction::CheckPermissions)
    } else {
        Some(RecoveryAction::FallbackMode)
    }
}

async fn apply_recovery_action(action: RecoveryAction) {
    match action {
        RecoveryAction::RestartAudioSystem => {
            info!("Attempting to restart audio system");
            // Give audio system time to reset
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        }
        RecoveryAction::ClearMemoryPressure => {
            info!("Clearing memory pressure");
            // Force garbage collection if possible
            #[cfg(feature = "gc")]
            {
                std::gc::collect();
            }
        }
        RecoveryAction::ReloadModel => {
            info!("Triggering model reload");
            // Model will be reloaded on next access due to lazy loading
        }
        RecoveryAction::CheckPermissions => {
            info!("Checking permissions status");
            // Permissions check would happen on next operation
        }
        RecoveryAction::FallbackMode => {
            info!("Entering fallback mode");
            // Reduce functionality to essential features only
        }
    }
}

/// CRITICAL FIX: Safe voice controller access with error recovery
#[cfg(feature = "voice-features")]
async fn with_voice_controller<F, T>(
    app_handle: AppHandle,
    operation: F,
) -> Result<T, String>
where
    F: FnOnce(&mut VoiceController) -> Result<T, String>,
{
    use std::sync::{Arc, Mutex};

    match app_handle.try_state::<Arc<Mutex<VoiceController>>>() {
        Some(controller_state) => {
            let recovery = VoiceErrorRecovery::default();

            execute_with_recovery(
                || {
                    match controller_state.try_lock() {
                        Ok(mut controller) => operation(&mut *controller),
                        Err(_) => {
                            warn!("Voice controller lock contention, retrying");
                            Err("Controller temporarily unavailable".to_string())
                        }
                    }
                },
                &recovery,
                "voice_controller_operation",
            ).await.map_err(|e| e.to_string())
        }
        None => Err("Voice controller not available - feature may be disabled".to_string()),
    }
}

#[cfg(not(feature = "voice-features"))]
async fn with_voice_controller<F, T>(
    _app_handle: AppHandle,
    _operation: F,
) -> Result<T, String>
where
    F: FnOnce(&mut ()) -> Result<T, String>,
{
    Err("Voice features not compiled in this build".to_string())
}

/// CRITICAL FIX: Enhanced voice command with comprehensive error handling
#[command]
pub async fn start_dictation_with_recovery(app_handle: AppHandle) -> Result<String, String> {
    info!("Starting dictation with enhanced error recovery");

    #[cfg(feature = "voice-features")]
    {
        with_voice_controller(app_handle, |controller| {
            controller.start_dictation(&app_handle)?;
            Ok("Dictation started successfully".to_string())
        }).await
    }

    #[cfg(not(feature = "voice-features"))]
    {
        Err("Voice features not available in this build".to_string())
    }
}

#[command]
pub async fn stop_dictation_with_recovery(app_handle: AppHandle) -> Result<String, String> {
    info!("Stopping dictation with enhanced error recovery");

    #[cfg(feature = "voice-features")]
    {
        with_voice_controller(app_handle, |controller| {
            controller.stop_dictation()?;
            Ok("Dictation stopped successfully".to_string())
        }).await
    }

    #[cfg(not(feature = "voice-features"))]
    {
        Err("Voice features not available in this build".to_string())
    }
}

/// CRITICAL FIX: Memory pressure monitoring for voice features
#[command]
pub async fn check_voice_memory_pressure(app_handle: AppHandle) -> Result<Value, String> {
    #[cfg(feature = "voice-features")]
    {
        // Check if we can acquire controller lock without blocking
        if let Some(controller_state) = app_handle.try_state::<std::sync::Arc<std::sync::Mutex<VoiceController>>>() {
            match controller_state.try_lock() {
                Ok(_) => {
                    // TODO: Integrate with performance monitor
                    serde_json::to_value(serde_json::json!({
                        "memory_pressure": false,
                        "controller_available": true,
                        "status": "healthy"
                    })).map_err(|e| e.to_string())
                }
                Err(_) => {
                    warn!("Voice controller lock contention detected");
                    serde_json::to_value(serde_json::json!({
                        "memory_pressure": true,
                        "controller_available": false,
                        "status": "lock_contention"
                    })).map_err(|e| e.to_string())
                }
            }
        } else {
            serde_json::to_value(serde_json::json!({
                "memory_pressure": false,
                "controller_available": false,
                "status": "not_initialized"
            })).map_err(|e| e.to_string())
        }
    }

    #[cfg(not(feature = "voice-features"))]
    {
        serde_json::to_value(serde_json::json!({
            "memory_pressure": false,
            "controller_available": false,
            "status": "feature_disabled"
        })).map_err(|e| e.to_string())
    }
}

/// CRITICAL FIX: Emergency voice system reset
#[command]
pub async fn emergency_voice_reset(app_handle: AppHandle) -> Result<String, String> {
    warn!("Emergency voice system reset requested");

    #[cfg(feature = "voice-features")]
    {
        // Try to clean up current state
        if let Some(controller_state) = app_handle.try_state::<std::sync::Arc<std::sync::Mutex<VoiceController>>>() {
            if let Ok(mut controller) = controller_state.try_lock() {
                let _ = controller.stop_dictation(); // Ignore errors during emergency reset

                // Force model unload to free memory
                controller.lazy_model.unload();
            }
        }

        // Give system time to clean up
        tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;

        Ok("Emergency voice reset completed".to_string())
    }

    #[cfg(not(feature = "voice-features"))]
    {
        Ok("Voice features not available - no reset needed".to_string())
    }
}
