use serde::{Deserialize, Serialize};
use tauri::{Manager, State};

use crate::agent::traits::MemoryManager;
use crate::state::AppState;
use crate::agent::implementations::memory_manager::{VisualContextConfig, VisualContextSummary};
use crate::constants::memory::{tokens, performance};

/// DTOs for memory management commands (simplified version)
#[derive(Debug, Serialize, Deserialize)]
pub struct MemoryStatus {
    pub total_messages: usize,
    pub estimated_tokens: usize,
    pub memory_efficiency_ratio: f64,
}

/// Get current memory status
#[tauri::command]
pub async fn get_memory_status(state: State<'_, AppState>) -> Result<MemoryStatus, String> {
    let memory_manager = state.get_memory_manager().await;
    let memory_guard = memory_manager.lock().await;

    // Use proper metrics from memory manager instead of manual calculation
    let metrics = memory_guard.get_memory_metrics().await;

    Ok(MemoryStatus {
        total_messages: metrics.total_messages,
        estimated_tokens: metrics.estimated_tokens,
        memory_efficiency_ratio: metrics.memory_efficiency_ratio,
    })
}

/// Clear conversation memory
#[tauri::command]
pub async fn clear_conversation_memory(state: State<'_, AppState>) -> Result<(), String> {
    let memory_manager = state.get_memory_manager().await;
    let mut memory_guard = memory_manager.lock().await;

    memory_guard.clear_memory().await
        .map_err(|e| format!("Failed to clear memory: {}", e))
}

/// Clean orphaned tool calls
#[tauri::command]
pub async fn clean_orphaned_tool_calls(state: State<'_, AppState>) -> Result<String, String> {
    let memory_manager = state.get_memory_manager().await;
    let mut memory_guard = memory_manager.lock().await;

    memory_guard.clean_orphaned_tool_calls().await
        .map_err(|e| format!("Failed to clean orphaned tool calls: {}", e))?;

    Ok("Orphaned tool calls cleaned successfully".to_string())
}

/// Clean orphaned tool results that have no corresponding tool calls
#[tauri::command]
pub async fn clean_orphaned_tool_results(state: State<'_, AppState>) -> Result<String, String> {
    let memory_manager = state.get_memory_manager().await;
    let mut memory_guard = memory_manager.lock().await;

    let cleaned_count = memory_guard.clean_orphaned_tool_results().await
        .map_err(|e| format!("Failed to clean orphaned tool results: {}", e))?;

    Ok(format!("Cleaned {} orphaned tool results successfully", cleaned_count))
}

/// Get conversation messages (basic implementation)
#[tauri::command]
pub async fn get_conversation_messages(state: State<'_, AppState>) -> Result<Vec<crate::agent::core::Message>, String> {
    let memory_manager = state.get_memory_manager().await;
    let memory_guard = memory_manager.lock().await;

    memory_guard.get_messages().await
        .map_err(|e| format!("Failed to get messages: {}", e))
}

/// Get last N messages
#[tauri::command]
pub async fn get_last_n_messages(n: usize, state: State<'_, AppState>) -> Result<Vec<crate::agent::core::Message>, String> {
    let memory_manager = state.get_memory_manager().await;
    let memory_guard = memory_manager.lock().await;

    memory_guard.get_last_n_messages(n).await
        .map_err(|e| format!("Failed to get last {} messages: {}", n, e))
}

/// Get visual context summaries from memory manager
#[tauri::command]
pub async fn get_visual_summaries(
    app_handle: tauri::AppHandle,
) -> Result<Vec<VisualContextSummary>, String> {
    let state = app_handle.state::<AppState>();
    let memory_manager = state.get_memory_manager().await;
    let memory_guard = memory_manager.lock().await;

    Ok(memory_guard.get_visual_summaries().await)
}

/// Update visual context configuration
#[tauri::command]
pub async fn update_visual_config(
    app_handle: tauri::AppHandle,
    config: VisualContextConfig,
) -> Result<(), String> {
    let state = app_handle.state::<AppState>();
    let memory_manager = state.get_memory_manager().await;
    let memory_guard = memory_manager.lock().await;

    memory_guard.update_visual_config(config).await
        .map_err(|e| format!("Failed to update visual config: {}", e))
}

/// Get current visual context configuration
#[tauri::command]
pub async fn get_visual_config(
    app_handle: tauri::AppHandle,
) -> Result<VisualContextConfig, String> {
    let state = app_handle.state::<AppState>();
    let memory_manager = state.get_memory_manager().await;
    let memory_guard = memory_manager.lock().await;

    Ok(memory_guard.get_visual_config().await)
}

/// Force compression of all screenshots in current conversation
#[tauri::command]
pub async fn compress_all_screenshots(
    app_handle: tauri::AppHandle,
) -> Result<usize, String> {
    let state = app_handle.state::<AppState>();
    let memory_manager = state.get_memory_manager().await;
    let mut memory_guard = memory_manager.lock().await;

    memory_guard.compress_all_screenshots().await
        .map_err(|e| format!("Failed to compress screenshots: {}", e))
}

/// Enable/disable screenshot compression with specific settings
#[tauri::command]
pub async fn configure_screenshot_compression(
    app_handle: tauri::AppHandle,
    enable_compression: bool,
    immediate_compression: bool,
    max_base64_screenshots: usize,
    retention_seconds: u64,
) -> Result<(), String> {
    let config = VisualContextConfig {
        enable_screenshot_compression: enable_compression,
        immediate_compression,
        max_base64_screenshots,
        screenshot_retention_seconds: retention_seconds,
        fallback_to_generic_description: true,
    };

    update_visual_config(app_handle, config).await
}

/// Get memory statistics including visual context compression stats
#[tauri::command]
pub async fn get_memory_compression_stats(
    app_handle: tauri::AppHandle,
) -> Result<serde_json::Value, String> {
    let state = app_handle.state::<AppState>();
    let memory_manager = state.get_memory_manager().await;
    let memory_guard = memory_manager.lock().await;

    let metrics = memory_guard.get_memory_metrics().await;
    let visual_summaries = memory_guard.get_visual_summaries().await;

    // Calculate visual compression stats
    let total_screenshots_compressed = visual_summaries.len();
    let total_original_tokens: usize = visual_summaries.iter()
        .map(|s| s.estimated_original_tokens)
        .sum();
    let total_compressed_tokens: usize = visual_summaries.iter()
        .map(|s| s.compressed_tokens)
        .sum();
    let tokens_saved = total_original_tokens.saturating_sub(total_compressed_tokens);
    let average_compression_ratio = if total_screenshots_compressed > 0 {
        visual_summaries.iter()
            .map(|s| s.compression_ratio)
            .sum::<f64>() / total_screenshots_compressed as f64
    } else {
        0.0
    };

    Ok(serde_json::json!({
        "memory_metrics": {
            "total_messages": metrics.total_messages,
            "estimated_tokens": metrics.estimated_tokens,
            "memory_efficiency_ratio": metrics.memory_efficiency_ratio,
            "pruning_events": metrics.pruning_events,
            "summarization_events": metrics.summarization_events,
            "orphaned_tool_calls_cleaned": metrics.orphaned_tool_calls_cleaned,
            "average_response_time_ms": metrics.average_response_time_ms
        },
        "visual_compression": {
            "total_screenshots_compressed": total_screenshots_compressed,
            "total_original_tokens": total_original_tokens,
            "total_compressed_tokens": total_compressed_tokens,
            "tokens_saved": tokens_saved,
            "average_compression_ratio": average_compression_ratio,
            "latest_summaries": visual_summaries.into_iter().take(performance::MAX_LATEST_SUMMARIES_RETURNED).collect::<Vec<_>>()
        }
    }))
}

/// Emergency function to recover from token overflow by compressing all screenshots and clearing memory if needed
#[tauri::command]
pub async fn emergency_memory_recovery(
    app_handle: tauri::AppHandle,
) -> Result<String, String> {
    let state = app_handle.state::<AppState>();
    let memory_manager = state.get_memory_manager().await;
    let mut memory_guard = memory_manager.lock().await;

    // First, try to compress all existing screenshots
    let compressed_count = memory_guard.compress_all_screenshots().await
        .map_err(|e| format!("Failed to compress screenshots: {}", e))?;

    // Use proper token estimation from memory manager metrics
    let metrics = memory_guard.get_memory_metrics().await;
    let total_tokens = metrics.estimated_tokens;

    // Get the configured max tokens threshold from memory config
    let config = memory_guard.get_config().await;
    let emergency_threshold = (config.max_tokens as f64 * tokens::EMERGENCY_THRESHOLD_MULTIPLIER) as usize; // 20% above max_tokens for emergency
    let critical_threshold = config.max_tokens; // Use configured max_tokens as critical threshold

    log::info!("Emergency recovery: Compressed {} screenshots, current tokens: {} (thresholds: critical={}, emergency={})",
               compressed_count, total_tokens, critical_threshold, emergency_threshold);

    // If still above emergency threshold, clear memory
    if total_tokens > emergency_threshold {
        log::error!("Token count critically high ({}), clearing memory (emergency threshold: {})", total_tokens, emergency_threshold);
        memory_guard.clear_memory().await
            .map_err(|e| format!("Failed to clear memory: {}", e))?;

        Ok(format!("Emergency recovery complete: Compressed {} screenshots and cleared memory due to critically high token count ({} > {})",
                   compressed_count, total_tokens, emergency_threshold))
    } else if total_tokens > critical_threshold {
        log::warn!("Token count high ({}) but below emergency threshold ({}), memory compression completed", total_tokens, emergency_threshold);
        Ok(format!("Emergency recovery complete: Compressed {} screenshots, token count: {} (above critical threshold {} but below emergency threshold {})",
                   compressed_count, total_tokens, critical_threshold, emergency_threshold))
    } else {
        Ok(format!("Emergency recovery complete: Compressed {} screenshots, token count now: {} (within safe limits)",
                   compressed_count, total_tokens))
    }
}
