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

    memory_manager.clear_memory().await
        .map_err(|e| format!("Failed to clear memory: {}", e))
}

/// Clean orphaned tool calls
#[tauri::command]
pub async fn clean_orphaned_tool_calls(state: State<'_, AppState>) -> Result<String, String> {
    let mut memory_manager = state.get_memory_manager().await
        .ok_or("EventMemoryManager not initialized")?;

    memory_manager.clean_orphaned_tool_calls().await
        .map_err(|e| format!("Failed to clean orphaned tool calls: {}", e))?;

    Ok("Orphaned tool calls cleaned successfully".to_string())
}

/// Get last N messages from conversation
#[tauri::command]
pub async fn get_last_messages(
    state: State<'_, AppState>,
    n: usize,
) -> Result<Vec<crate::agent::core::Message>, String> {
    let memory_manager = state.get_memory_manager().await
        .ok_or("EventMemoryManager not initialized")?;

    memory_manager.get_last_n_messages(n).await
        .map_err(|e| format!("Failed to get last {} messages: {}", n, e))
}

/// Get current memory metrics - EventMemoryManager version
#[tauri::command]
pub async fn get_memory_metrics(
    app_handle: tauri::AppHandle,
) -> Result<crate::agent::EventMemoryMetrics, String> {
    let state = app_handle.state::<AppState>();
    let memory_manager = state.get_memory_manager().await
        .ok_or("EventMemoryManager not initialized")?;

    Ok(memory_manager.get_metrics().await)
}

/// Emergency memory recovery - simplified for EventMemoryManager
#[tauri::command]
pub async fn emergency_memory_recovery(
    app_handle: tauri::AppHandle,
) -> Result<String, String> {
    let state = app_handle.state::<AppState>();
    let mut memory_manager = state.get_memory_manager().await
        .ok_or("EventMemoryManager not initialized")?;

    let metrics = memory_manager.get_metrics().await;

    // Token limit check (using a reasonable default if not available)
    let token_limit = 120000; // Default from EventMemoryConfig
    
    if metrics.estimated_tokens > token_limit {
        // Clear memory as emergency recovery
        memory_manager.clear_memory().await
            .map_err(|e| format!("Failed to clear memory during emergency recovery: {}", e))?;

        Ok(format!(
            "Emergency recovery complete: Cleared all memory due to high token count ({} tokens > {} limit)",
            metrics.estimated_tokens, token_limit
        ))
    } else {
        Ok(format!(
            "No emergency recovery needed: Token count {} within limits (max: {})",
            metrics.estimated_tokens, token_limit
        ))
    }
}