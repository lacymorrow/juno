use serde::{Deserialize, Serialize};
use tauri::State;

use crate::agent::traits::MemoryManager;
use crate::state::AppState;

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
pub async fn get_conversation_messages(state: State<'_, AppState>) -> Result<Vec<crate::agent::structs::Message>, String> {
    let memory_manager = state.get_memory_manager().await;
    let memory_guard = memory_manager.lock().await;

    memory_guard.get_messages().await
        .map_err(|e| format!("Failed to get messages: {}", e))
}

/// Get last N messages
#[tauri::command]
pub async fn get_last_n_messages(n: usize, state: State<'_, AppState>) -> Result<Vec<crate::agent::structs::Message>, String> {
    let memory_manager = state.get_memory_manager().await;
    let memory_guard = memory_manager.lock().await;

    memory_guard.get_last_n_messages(n).await
        .map_err(|e| format!("Failed to get last {} messages: {}", n, e))
}
