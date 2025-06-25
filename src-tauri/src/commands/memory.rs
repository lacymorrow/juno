use serde::{Deserialize, Serialize};
use tauri::{Manager, State};

use crate::agent::traits::MemoryManager;
use crate::state::AppState;
use crate::agent::implementations::memory_manager::{VisualContextConfig, VisualContextSummary, AdvancedMemoryManager};

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

    let messages = memory_guard.get_messages().await
        .map_err(|e| format!("Failed to get messages: {}", e))?;

    // Calculate basic metrics
    let total_messages = messages.len();
    let estimated_tokens = messages.iter()
        .map(|m| m.content.len() / 4) // Rough token estimate
        .sum::<usize>();

    // Calculate efficiency ratio (non-empty messages / total messages)
    let useful_messages = messages.iter()
        .filter(|m| !m.content.is_empty() || m.tool_calls.is_some())
        .count();

    let memory_efficiency_ratio = if total_messages > 0 {
        useful_messages as f64 / total_messages as f64
    } else {
        1.0
    };

    Ok(MemoryStatus {
        total_messages,
        estimated_tokens,
        memory_efficiency_ratio,
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

    // For now, return empty vector since visual summaries are only available with AdvancedMemoryManager
    // and the current memory manager is SimpleMemoryManager
    Ok(vec![])
}

/// Update visual context configuration
#[tauri::command]
pub async fn update_visual_config(
    app_handle: tauri::AppHandle,
    config: VisualContextConfig,
) -> Result<(), String> {
    let state = app_handle.state::<AppState>();

    // For now, return error since visual config is only available with AdvancedMemoryManager
    // and the current memory manager is SimpleMemoryManager
    Err("Visual config only available with AdvancedMemoryManager".to_string())
}

/// Get current visual context configuration
#[tauri::command]
pub async fn get_visual_config(
    app_handle: tauri::AppHandle,
) -> Result<VisualContextConfig, String> {
    let state = app_handle.state::<AppState>();

    // For now, return default config since visual config is only available with AdvancedMemoryManager
    // and the current memory manager is SimpleMemoryManager
    Ok(VisualContextConfig::default())
}

/// Force compression of all screenshots in current conversation
#[tauri::command]
pub async fn compress_all_screenshots(
    app_handle: tauri::AppHandle,
) -> Result<usize, String> {
    let state = app_handle.state::<AppState>();

    // For now, return 0 since screenshot compression is only available with AdvancedMemoryManager
    // and the current memory manager is SimpleMemoryManager
    Ok(0)
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

    // For now, return basic stats since memory compression is only available with AdvancedMemoryManager
    // and the current memory manager is SimpleMemoryManager
    let memory_manager = state.get_memory_manager().await;
    let memory_guard = memory_manager.lock().await;

    let messages = memory_guard.get_messages().await
        .map_err(|e| format!("Failed to get messages: {}", e))?;

    // Calculate basic metrics
    let total_messages = messages.len();
    let estimated_tokens = messages.iter()
        .map(|m| m.content.len() / 4) // Rough token estimate
        .sum::<usize>();

    Ok(serde_json::json!({
        "memory_metrics": {
            "total_messages": total_messages,
            "estimated_tokens": estimated_tokens,
            "memory_efficiency_ratio": 1.0
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
