use serde::{Deserialize, Serialize};
use tauri::{Manager, State};

use crate::agent::traits::MemoryManager;
use crate::state::AppState;

/// DTOs for memory management commands (simplified for EventMemoryManager)
#[derive(Debug, Serialize, Deserialize)]
pub struct MemoryStatus {
    pub total_messages: usize,
    pub estimated_tokens: usize,
    pub memory_efficiency_ratio: f64,
}

/// Get current memory status - EventMemoryManager version
#[tauri::command]
pub async fn get_memory_status(state: State<'_, AppState>) -> Result<MemoryStatus, String> {
    let memory_manager = state.get_memory_manager().await
        .ok_or("EventMemoryManager not initialized")?;

    // Get enhanced metrics from EventMemoryManager
    let messages = memory_manager.get_messages().await
        .map_err(|e| format!("Failed to get messages: {}", e))?;

    let total_messages = messages.len();
    // Enhanced token estimation: ~4 chars per token
    let estimated_tokens = messages.iter()
        .map(|msg| msg.content.len() / 4)
        .sum();

    // Get efficiency ratio from EventMemoryManager metrics
    let metrics = memory_manager.get_metrics().await;
    let efficiency_ratio = if metrics.total_messages > 0 {
        metrics.estimated_tokens as f64 / (metrics.total_messages as f64 * 100.0) // Rough efficiency calculation
    } else {
        1.0
    };

    Ok(MemoryStatus {
        total_messages,
        estimated_tokens,
        memory_efficiency_ratio: efficiency_ratio,
    })
}

/// Clear conversation memory
#[tauri::command]
pub async fn clear_conversation_memory(state: State<'_, AppState>) -> Result<(), String> {
    let mut memory_manager = state.get_memory_manager().await
        .ok_or("EventMemoryManager not initialized")?;

    memory_manager.clear().await
        .map_err(|e| format!("Failed to clear memory: {}", e))?;

    Ok(())
}

/// Clean orphaned tool calls - EventMemoryManager version
#[tauri::command]
pub async fn clean_orphaned_tool_calls(state: State<'_, AppState>) -> Result<String, String> {
    let mut memory_manager = state.get_memory_manager().await
        .ok_or("EventMemoryManager not initialized")?;

    memory_manager.clean_orphaned_tool_calls().await
        .map_err(|e| format!("Failed to clean orphaned tool calls: {}", e))?;

    Ok("Orphaned tool calls cleaned successfully".to_string())
}

/// Get last N messages - EventMemoryManager version
#[tauri::command]
pub async fn get_last_messages(state: State<'_, AppState>, count: Option<usize>) -> Result<Vec<crate::agent::Message>, String> {
    let memory_manager = state.get_memory_manager().await
        .ok_or("EventMemoryManager not initialized")?;

    let messages = memory_manager.get_messages().await
        .map_err(|e| format!("Failed to get messages: {}", e))?;

    let count = count.unwrap_or(10); // Default to last 10 messages
    let last_messages = messages.into_iter()
        .rev()
        .take(count)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();

    Ok(last_messages)
}

/// Force memory prune - simplified for EventMemoryManager
#[tauri::command]
pub async fn force_memory_prune(state: State<'_, AppState>) -> Result<String, String> {
    let mut memory_manager = state.get_memory_manager().await
        .ok_or("EventMemoryManager not initialized")?;

    let before_metrics = memory_manager.get_metrics().await;

    // Force prune using existing method
    memory_manager.prune_memory_if_needed().await
        .map_err(|e| format!("Failed to prune memory: {}", e))?;

    let after_metrics = memory_manager.get_metrics().await;

    let messages_removed = before_metrics.total_messages.saturating_sub(after_metrics.total_messages);
    let tokens_saved = before_metrics.estimated_tokens.saturating_sub(after_metrics.estimated_tokens);

    Ok(format!(
        "Memory pruning complete: {} messages removed, {} tokens saved. Now: {} messages, {} tokens",
        messages_removed, tokens_saved, after_metrics.total_messages, after_metrics.estimated_tokens
    ))
}

/// Get detailed memory metrics - EventMemoryManager version
#[tauri::command]
pub async fn get_memory_metrics(
    app_handle: tauri::AppHandle,
) -> Result<crate::agent::EventMemoryMetrics, String> {
    let state = app_handle.state::<AppState>();
    let memory_manager = state.get_memory_manager().await
        .ok_or("EventMemoryManager not initialized")?;

    Ok(memory_manager.get_metrics().await)
}

/// Update visual context configuration - supported by AdvancedMemoryManager
#[tauri::command]
pub async fn update_visual_config(
    app_handle: tauri::AppHandle,
    config: VisualContextConfig,
) -> Result<(), String> {
    let state = app_handle.state::<AppState>();
    let memory_manager = state.get_memory_manager().await
        .ok_or("EventMemoryManager not initialized")?;

    // EventMemoryManager doesn't have visual config, so this is a no-op for now
    // TODO: Add visual config support to EventMemoryManager if needed
    Ok(())
}

/// Get current visual context configuration - supported by AdvancedMemoryManager
#[tauri::command]
pub async fn get_visual_config(
    app_handle: tauri::AppHandle,
) -> Result<VisualContextConfig, String> {
    // Return default config since EventMemoryManager doesn't support visual config
    Ok(VisualContextConfig {
        enable_screenshot_compression: false,
        immediate_compression: false,
        max_base64_screenshots: 5,
        screenshot_retention_seconds: 300,
        fallback_to_generic_description: true,
    })
}

/// Emergency memory recovery - simplified for EventMemoryManager
#[tauri::command]
pub async fn emergency_memory_recovery(
    app_handle: tauri::AppHandle,
    token_limit: Option<usize>,
) -> Result<String, String> {
    let state = app_handle.state::<AppState>();
    let mut memory_manager = state.get_memory_manager().await
        .ok_or("EventMemoryManager not initialized")?;

    let metrics = memory_manager.get_metrics().await;
    let token_limit = token_limit.unwrap_or(50000); // Default safe limit

    if metrics.estimated_tokens > token_limit {
        // Force aggressive pruning
        memory_manager.clear().await
            .map_err(|e| format!("Emergency recovery failed: {}", e))?;

        let new_metrics = memory_manager.get_metrics().await;
        Ok(format!(
            "Emergency recovery completed: Reduced from {} to {} tokens ({} messages cleared)",
            metrics.estimated_tokens, new_metrics.estimated_tokens,
            metrics.total_messages - new_metrics.total_messages
        ))
    } else {
        Ok(format!(
            "No emergency recovery needed: Token count {} within limits (max: {})",
            metrics.estimated_tokens, token_limit
        ))
    }
}

// Visual context config struct for compatibility
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisualContextConfig {
    pub enable_screenshot_compression: bool,
    pub immediate_compression: bool,
    pub max_base64_screenshots: usize,
    pub screenshot_retention_seconds: u64,
    pub fallback_to_generic_description: bool,
}
