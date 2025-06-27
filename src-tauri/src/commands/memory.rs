use serde::{Deserialize, Serialize};
use tauri::{Manager, State};

use crate::agent::traits::MemoryManager;
use crate::state::AppState;
use crate::agent::implementations::memory_manager::{SimpleMemoryManager, VisualContextConfig, VisualContextSummary};

/// DTOs for memory management commands (simplified version)
#[derive(Debug, Serialize, Deserialize)]
pub struct MemoryStatus {
    pub total_messages: usize,
    pub estimated_tokens: usize,
    pub memory_efficiency_ratio: f64,
}

/// Get current memory status - simplified for SimpleMemoryManager
#[tauri::command]
pub async fn get_memory_status(state: State<'_, AppState>) -> Result<MemoryStatus, String> {
    let memory_manager = state.get_memory_manager().await;
    let memory_guard = memory_manager.lock().await;

    // Get basic metrics from SimpleMemoryManager
    let messages = memory_guard.get_messages().await
        .map_err(|e| format!("Failed to get messages: {}", e))?;

    let total_messages = messages.len();
    // Simple token estimation: ~4 chars per token
    let estimated_tokens = messages.iter()
        .map(|msg| msg.content.len() / 4)
        .sum();

    Ok(MemoryStatus {
        total_messages,
        estimated_tokens,
        memory_efficiency_ratio: 1.0, // SimpleMemoryManager doesn't track efficiency
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

/// Get visual context summaries - not supported by SimpleMemoryManager
#[tauri::command]
pub async fn get_visual_summaries(
    _app_handle: tauri::AppHandle,
) -> Result<Vec<VisualContextSummary>, String> {
    // SimpleMemoryManager doesn't support visual summaries
    Ok(vec![])
}

/// Update visual context configuration - not supported by SimpleMemoryManager
#[tauri::command]
pub async fn update_visual_config(
    _app_handle: tauri::AppHandle,
    _config: VisualContextConfig,
) -> Result<(), String> {
    // SimpleMemoryManager doesn't support visual config
    Ok(())
}

/// Get current visual context configuration - not supported by SimpleMemoryManager
#[tauri::command]
pub async fn get_visual_config(
    _app_handle: tauri::AppHandle,
) -> Result<VisualContextConfig, String> {
    // Return default config for SimpleMemoryManager
    Ok(VisualContextConfig::default())
}

/// Force compression of all screenshots - not supported by SimpleMemoryManager
#[tauri::command]
pub async fn compress_all_screenshots(
    _app_handle: tauri::AppHandle,
) -> Result<usize, String> {
    // SimpleMemoryManager doesn't support screenshot compression
    Ok(0)
}

/// Enable/disable screenshot compression - not supported by SimpleMemoryManager
#[tauri::command]
pub async fn configure_screenshot_compression(
    _app_handle: tauri::AppHandle,
    _enable_compression: bool,
    _immediate_compression: bool,
    _max_base64_screenshots: usize,
    _retention_seconds: u64,
) -> Result<(), String> {
    // SimpleMemoryManager doesn't support screenshot compression
    Ok(())
}

/// Get memory statistics - simplified for SimpleMemoryManager
#[tauri::command]
pub async fn get_memory_compression_stats(
    app_handle: tauri::AppHandle,
) -> Result<serde_json::Value, String> {
    let state = app_handle.state::<AppState>();
    let memory_manager = state.get_memory_manager().await;
    let memory_guard = memory_manager.lock().await;

    let messages = memory_guard.get_messages().await
        .map_err(|e| format!("Failed to get messages: {}", e))?;

    let total_messages = messages.len();
    let estimated_tokens = messages.iter()
        .map(|msg| msg.content.len() / 4)
        .sum::<usize>();

    Ok(serde_json::json!({
        "memory_metrics": {
            "total_messages": total_messages,
            "estimated_tokens": estimated_tokens,
            "memory_efficiency_ratio": 1.0,
            "pruning_events": 0,
            "summarization_events": 0,
            "orphaned_tool_calls_cleaned": 0,
            "average_response_time_ms": 0.0
        },
        "visual_compression": {
            "total_screenshots_compressed": 0,
            "total_original_tokens": 0,
            "total_compressed_tokens": 0,
            "tokens_saved": 0,
            "average_compression_ratio": 0.0,
            "latest_summaries": []
        }
    }))
}

/// Emergency function to recover from token overflow - simplified for SimpleMemoryManager
#[tauri::command]
pub async fn emergency_memory_recovery(
    app_handle: tauri::AppHandle,
) -> Result<String, String> {
    let state = app_handle.state::<AppState>();
    let memory_manager = state.get_memory_manager().await;
    let mut memory_guard = memory_manager.lock().await;

    // SimpleMemoryManager doesn't have advanced features, so just clear memory if needed
    let messages = memory_guard.get_messages().await
        .map_err(|e| format!("Failed to get messages: {}", e))?;

    let total_tokens = messages.iter()
        .map(|msg| msg.content.len() / 4)
        .sum::<usize>();

    // If we have too many tokens (rough estimate), clear memory
    if total_tokens > 100_000 {
        log::warn!("Token count high ({}), clearing memory", total_tokens);
        memory_guard.clear_memory().await
            .map_err(|e| format!("Failed to clear memory: {}", e))?;

        Ok(format!("Emergency recovery complete: Cleared memory due to high token count ({})", total_tokens))
    } else {
        Ok(format!("Emergency recovery complete: Token count {} within limits", total_tokens))
    }
}
