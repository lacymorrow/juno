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

/// Manually prune conversation memory to target size
#[tauri::command]
pub async fn prune_conversation_memory(target_messages: Option<usize>, state: State<'_, AppState>) -> Result<String, String> {
    // Get memory manager
    let memory_manager = state.get_memory_manager().await;
    let memory_guard = memory_manager.lock().await;

    // Check if it's the advanced memory manager that supports pruning
    if let Ok(messages) = memory_guard.get_messages().await {
        let before_count = messages.len();
        let before_tokens: usize = messages.iter().map(|m| m.content.len() / 4).sum();

        drop(memory_guard); // Release lock before potential pruning
        drop(messages); // Release messages reference

        // For now, we'll just clear memory since we can't call prune_memory on the trait object
        // In a future enhancement, we could cast to AdvancedMemoryManager if needed
        let memory_manager = state.get_memory_manager().await;
        let mut memory_guard = memory_manager.lock().await;

        if before_tokens > 150000 {
            // If severely over limit, clear entirely
            memory_guard.clear_memory().await
                .map_err(|e| format!("Failed to clear memory: {}", e))?;
            Ok(format!("Emergency memory clear: {} messages ({} tokens) cleared", before_count, before_tokens))
        } else {
            // Keep some recent messages
            let messages = memory_guard.get_messages().await
                .map_err(|e| format!("Failed to get messages: {}", e))?;
            let keep_count = target_messages.unwrap_or(20).min(messages.len());
            let recent_messages: Vec<_> = messages.into_iter().rev().take(keep_count).rev().collect();

            memory_guard.clear_memory().await
                .map_err(|e| format!("Failed to clear memory: {}", e))?;

            // Re-add recent messages
            for message in recent_messages {
                memory_guard.add_message(message).await
                    .map_err(|e| format!("Failed to re-add message: {}", e))?;
            }

            Ok(format!("Memory pruned: kept {} most recent messages", keep_count))
        }
    } else {
        Err("Failed to access memory manager".to_string())
    }
}
