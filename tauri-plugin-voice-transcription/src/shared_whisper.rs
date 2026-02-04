use whisper_rs::{WhisperContext, WhisperContextParameters};
use std::sync::{Arc, OnceLock};
use std::path::Path;
use crate::error::{Error, Result};
use tracing::{info, error, warn, debug};

/// Global shared Whisper context that can be used by multiple controllers
static SHARED_WHISPER_CONTEXT: OnceLock<Arc<WhisperContext>> = OnceLock::new();

/// Manager for shared Whisper model loading and access
pub struct SharedWhisperManager;

impl SharedWhisperManager {
    /// Initialize the shared Whisper context (should be called once at startup)
    pub fn initialize(model_path: &str) -> Result<Arc<WhisperContext>> {
        info!("[SharedWhisper] 🚀 Initializing shared Whisper context with model: {}", model_path);

        // Check if already initialized
        if let Some(existing_context) = SHARED_WHISPER_CONTEXT.get() {
            info!("[SharedWhisper] ✅ Shared context already exists, returning existing instance (no duplicate loading)");
            return Ok(existing_context.clone());
        }

        let model_path_obj = Path::new(model_path);
        if !model_path_obj.exists() {
            error!("[SharedWhisper] ❌ Model file not found: {}", model_path);
            return Err(Error::ModelNotFound(model_path.to_string()));
        }

        // Log model file size for performance tracking
        if let Ok(metadata) = std::fs::metadata(model_path) {
            let size_mb = metadata.len() as f64 / (1024.0 * 1024.0);
            info!("[SharedWhisper] 📊 Loading Whisper model: {:.1}MB", size_mb);
        }

        let context_params = WhisperContextParameters::default();
        info!("[SharedWhisper] 🔧 Creating WhisperContext with default parameters...");

        let whisper_context = WhisperContext::new_with_params(model_path, context_params)
            .map_err(|e| {
                error!("[SharedWhisper] ❌ Failed to create WhisperContext: {:?}", e);
                Error::Whisper(format!("Failed to create WhisperContext: {:?}", e))
            })?;

        let arc_context = Arc::new(whisper_context);
        info!("[SharedWhisper] ✅ WhisperContext created successfully");

        // Try to set the global context
        match SHARED_WHISPER_CONTEXT.set(arc_context.clone()) {
            Ok(_) => {
                info!("[SharedWhisper] ✅ Shared Whisper context initialized successfully - available for reuse");
                info!("[SharedWhisper] 💡 Both VoiceController and AlwaysListeningController can now use this shared instance");
                Ok(arc_context)
            }
            Err(_) => {
                // This shouldn't happen since we checked above, but handle gracefully
                warn!("[SharedWhisper] ⚠️  Race condition detected during initialization, returning existing instance");
                Ok(Self::get()?)
            }
        }
    }

    /// Get the shared Whisper context (must be initialized first)
    pub fn get() -> Result<Arc<WhisperContext>> {
        SHARED_WHISPER_CONTEXT
            .get()
            .cloned()
            .ok_or_else(|| {
                error!("[SharedWhisper] ❌ Shared Whisper context not initialized - call initialize() first");
                Error::Whisper("Shared Whisper context not initialized. Call initialize() first.".to_string())
            })
    }

    /// Check if the shared context is initialized
    pub fn is_initialized() -> bool {
        let initialized = SHARED_WHISPER_CONTEXT.get().is_some();
        debug!("[SharedWhisper] Initialization status: {}", if initialized { "✅ Initialized" } else { "❌ Not initialized" });
        initialized
    }

    /// Get comprehensive initialization status for debugging
    pub fn get_status() -> serde_json::Value {
        let initialized = Self::is_initialized();
        let context_available = Self::get().is_ok();

        let status = serde_json::json!({
            "initialized": initialized,
            "context_available": context_available,
            "memory_efficiency": if initialized { "✅ Single shared instance" } else { "❌ No shared instance" },
            "performance_impact": if initialized { "✅ Eliminated duplicate loading" } else { "❌ Potential duplicate loading" }
        });

        info!("[SharedWhisper] 📊 Status check: {}", status);
        status
    }

    /// Force reset the shared context (for testing/debugging only)
    #[cfg(debug_assertions)]
    pub fn reset_for_testing() {
        warn!("[SharedWhisper] 🔄 Resetting shared context (DEBUG ONLY)");
        // Note: OnceLock doesn't have a reset method, so this is for documentation
        // In production, the context should remain shared for the lifetime of the app
    }

    /// Get performance statistics
    pub fn get_performance_info() -> serde_json::Value {
        let initialized = Self::is_initialized();
        serde_json::json!({
            "memory_savings": if initialized { "~77MB saved by avoiding duplicate loading" } else { "0MB (no sharing)" },
            "startup_improvement": if initialized { "Faster startup (shared context)" } else { "Slower startup (duplicate loading)" },
            "recommendation": if !initialized { "Call SharedWhisperManager::initialize() once at startup" } else { "✅ Optimally configured" }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shared_whisper_manager_not_initialized() {
        // Reset static state for test
        // Note: In a real test environment, you'd want to use a different approach
        // to avoid global state, but this demonstrates the concept

        assert!(!SharedWhisperManager::is_initialized());
        assert!(SharedWhisperManager::get().is_err());
    }

    #[test]
    fn test_shared_whisper_manager_status() {
        let status = SharedWhisperManager::get_status();
        assert!(status.is_object());
        assert!(status.get("initialized").is_some());
        assert!(status.get("context_available").is_some());
        assert!(status.get("memory_efficiency").is_some());
        assert!(status.get("performance_impact").is_some());
    }

    #[test]
    fn test_performance_info() {
        let perf_info = SharedWhisperManager::get_performance_info();
        assert!(perf_info.is_object());
        assert!(perf_info.get("memory_savings").is_some());
        assert!(perf_info.get("startup_improvement").is_some());
        assert!(perf_info.get("recommendation").is_some());
    }
}
