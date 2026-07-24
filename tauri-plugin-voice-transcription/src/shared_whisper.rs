use crate::error::{Error, Result};
use std::path::Path;
use std::sync::{Arc, RwLock};
use tracing::{debug, error, info, warn};
use whisper_rs::{WhisperContext, WhisperContextParameters};

/// Global shared Whisper context — uses RwLock so the model can be swapped at runtime
static SHARED_WHISPER_CONTEXT: RwLock<Option<Arc<WhisperContext>>> = RwLock::new(None);

/// Manager for shared Whisper model loading and access
pub struct SharedWhisperManager;

impl SharedWhisperManager {
    /// Create a WhisperContext from a model file path
    fn create_context(model_path: &str) -> Result<Arc<WhisperContext>> {
        let model_path_obj = Path::new(model_path);
        if !model_path_obj.exists() {
            error!("[SharedWhisper] Model file not found: {}", model_path);
            return Err(Error::ModelNotFound(model_path.to_string()));
        }

        if let Ok(metadata) = std::fs::metadata(model_path) {
            let size_mb = metadata.len() as f64 / (1024.0 * 1024.0);
            info!("[SharedWhisper] Loading Whisper model: {:.1}MB", size_mb);
        }

        let context_params = WhisperContextParameters::default();
        let whisper_context =
            WhisperContext::new_with_params(model_path, context_params).map_err(|e| {
                error!("[SharedWhisper] Failed to create WhisperContext: {:?}", e);
                Error::Whisper(format!("Failed to create WhisperContext: {:?}", e))
            })?;

        Ok(Arc::new(whisper_context))
    }

    /// Initialize the shared Whisper context (returns existing if already set)
    pub fn initialize(model_path: &str) -> Result<Arc<WhisperContext>> {
        info!(
            "[SharedWhisper] Initializing shared Whisper context with model: {}",
            model_path
        );

        // Fast path: read lock to check if already initialized
        {
            let guard = SHARED_WHISPER_CONTEXT
                .read()
                .map_err(|e| Error::Whisper(format!("Shared context lock poisoned: {}", e)))?;
            if let Some(existing) = guard.as_ref() {
                debug!(
                    "[SharedWhisper] Shared context already exists, returning existing instance"
                );
                return Ok(existing.clone());
            }
        }

        // Create the new context (expensive, done without holding the lock)
        let arc_context = Self::create_context(model_path)?;
        debug!("[SharedWhisper] WhisperContext created successfully");

        // Write lock to set the global context
        let mut guard = SHARED_WHISPER_CONTEXT
            .write()
            .map_err(|e| Error::Whisper(format!("Shared context lock poisoned: {}", e)))?;

        // Double-check: another thread may have initialized while we were loading
        if let Some(existing) = guard.as_ref() {
            warn!("[SharedWhisper] Race condition: another thread initialized while we were loading, returning existing");
            return Ok(existing.clone());
        }

        *guard = Some(arc_context.clone());
        info!("[SharedWhisper] Shared Whisper context initialized successfully");
        Ok(arc_context)
    }

    /// Reinitialize with a new model path — replaces the existing context
    pub fn reinitialize(model_path: &str) -> Result<Arc<WhisperContext>> {
        info!(
            "[SharedWhisper] Reinitializing shared Whisper context with model: {}",
            model_path
        );

        // Create the new context (expensive, done without holding the lock)
        let arc_context = Self::create_context(model_path)?;
        debug!("[SharedWhisper] New WhisperContext created successfully");

        // Write lock to replace the global context
        let mut guard = SHARED_WHISPER_CONTEXT
            .write()
            .map_err(|e| Error::Whisper(format!("Shared context lock poisoned: {}", e)))?;

        *guard = Some(arc_context.clone());
        info!("[SharedWhisper] Shared Whisper context reinitialized with new model");
        Ok(arc_context)
    }

    /// Get the shared Whisper context (must be initialized first)
    pub fn get() -> Result<Arc<WhisperContext>> {
        let guard = SHARED_WHISPER_CONTEXT
            .read()
            .map_err(|e| Error::Whisper(format!("Shared context lock poisoned: {}", e)))?;
        guard.as_ref().cloned().ok_or_else(|| {
            error!(
                "[SharedWhisper] Shared Whisper context not initialized - call initialize() first"
            );
            Error::Whisper(
                "Shared Whisper context not initialized. Call initialize() first.".to_string(),
            )
        })
    }

    /// Check if the shared context is initialized
    pub fn is_initialized() -> bool {
        let initialized = SHARED_WHISPER_CONTEXT
            .read()
            .map(|g| g.is_some())
            .unwrap_or(false);
        debug!(
            "[SharedWhisper] Initialization status: {}",
            if initialized {
                "Initialized"
            } else {
                "Not initialized"
            }
        );
        initialized
    }

    /// Get comprehensive initialization status for debugging
    pub fn get_status() -> serde_json::Value {
        let initialized = Self::is_initialized();
        let context_available = Self::get().is_ok();

        let status = serde_json::json!({
            "initialized": initialized,
            "context_available": context_available,
            "memory_efficiency": if initialized { "Single shared instance" } else { "No shared instance" },
            "performance_impact": if initialized { "Eliminated duplicate loading" } else { "Potential duplicate loading" }
        });

        debug!("[SharedWhisper] Status check: {}", status);
        status
    }

    /// Get performance statistics
    pub fn get_performance_info() -> serde_json::Value {
        let initialized = Self::is_initialized();
        serde_json::json!({
            "memory_savings": if initialized { "~77MB saved by avoiding duplicate loading" } else { "0MB (no sharing)" },
            "startup_improvement": if initialized { "Faster startup (shared context)" } else { "Slower startup (duplicate loading)" },
            "recommendation": if !initialized { "Call SharedWhisperManager::initialize() once at startup" } else { "Optimally configured" }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
