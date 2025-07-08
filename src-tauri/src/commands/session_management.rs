//! Session Management Commands
//! 
//! TARS Phase 3.5: Enhanced conversation history persistence
//! Provides commands for managing conversation sessions with persistence

use serde::{Deserialize, Serialize};
use tauri::{Manager, State};

use crate::agent::core::Message;
use crate::agent::memory::StorageStats;
use crate::agent::traits::MemoryManager;
use crate::state::AppState;

/// Session information for the frontend
#[derive(Debug, Serialize, Deserialize)]
pub struct SessionInfo {
    pub session_id: String,
    pub created_at: i64,
    pub last_updated: i64,
    pub message_count: usize,
    pub estimated_tokens: usize,
}

/// Start a new conversation session
#[tauri::command]
pub async fn start_new_session(state: State<'_, AppState>) -> Result<String, String> {
    let memory_manager = state.get_memory_manager().await
        .ok_or("EventMemoryManager not initialized")?;

    memory_manager.start_new_session().await
        .map_err(|e| format!("Failed to start new session: {}", e))
}

/// Load a previous conversation session
#[tauri::command]
pub async fn load_session(
    session_id: String,
    state: State<'_, AppState>
) -> Result<Vec<Message>, String> {
    let memory_manager = state.get_memory_manager().await
        .ok_or("EventMemoryManager not initialized")?;

    memory_manager.load_session(&session_id).await
        .map_err(|e| format!("Failed to load session: {}", e))
}

/// Get list of all available sessions
#[tauri::command]
pub async fn list_sessions(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    let memory_manager = state.get_memory_manager().await
        .ok_or("EventMemoryManager not initialized")?;

    memory_manager.list_sessions().await
        .map_err(|e| format!("Failed to list sessions: {}", e))
}

/// Delete a session permanently
#[tauri::command]
pub async fn delete_session(
    session_id: String,
    state: State<'_, AppState>
) -> Result<(), String> {
    let memory_manager = state.get_memory_manager().await
        .ok_or("EventMemoryManager not initialized")?;

    memory_manager.delete_session(&session_id).await
        .map_err(|e| format!("Failed to delete session: {}", e))
}

/// Force checkpoint the current session
#[tauri::command]
pub async fn checkpoint_current_session(state: State<'_, AppState>) -> Result<(), String> {
    let memory_manager = state.get_memory_manager().await
        .ok_or("EventMemoryManager not initialized")?;

    memory_manager.checkpoint_current_session().await
        .map_err(|e| format!("Failed to checkpoint session: {}", e))
}

/// Get storage statistics and usage information
#[tauri::command]
pub async fn get_storage_stats(state: State<'_, AppState>) -> Result<StorageStats, String> {
    let memory_manager = state.get_memory_manager().await
        .ok_or("EventMemoryManager not initialized")?;

    memory_manager.get_storage_stats().await
        .map_err(|e| format!("Failed to get storage stats: {}", e))
}

/// Clean up old sessions based on age
#[tauri::command]
pub async fn cleanup_old_sessions(
    _max_age_days: u32,
    state: State<'_, AppState>
) -> Result<usize, String> {
    let _memory_manager = state.get_memory_manager().await
        .ok_or("EventMemoryManager not initialized")?;

    // This would require adding a cleanup method to EventMemoryManager
    // For now, return a placeholder - would need to implement cleanup_old_sessions in EventMemoryManager
    Ok(0)
}

/// Export session data (for backup or analysis)
#[tauri::command]
pub async fn export_session(
    session_id: String,
    state: State<'_, AppState>
) -> Result<String, String> {
    let memory_manager = state.get_memory_manager().await
        .ok_or("EventMemoryManager not initialized")?;

    let messages = memory_manager.load_session(&session_id).await
        .map_err(|e| format!("Failed to load session for export: {}", e))?;

    // Convert to JSON for export
    serde_json::to_string_pretty(&messages)
        .map_err(|e| format!("Failed to serialize session data: {}", e))
}

/// Import session data (from backup)
#[tauri::command]
pub async fn import_session(
    session_data: String,
    state: State<'_, AppState>
) -> Result<String, String> {
    let mut memory_manager = state.get_memory_manager().await
        .ok_or("EventMemoryManager not initialized")?;

    // Parse the session data
    let messages: Vec<Message> = serde_json::from_str(&session_data)
        .map_err(|e| format!("Failed to parse session data: {}", e))?;

    // Start a new session for the imported data
    let session_id = memory_manager.start_new_session().await
        .map_err(|e| format!("Failed to start session for import: {}", e))?;

    // Add all the imported messages
    for message in messages {
        memory_manager.add_message(message).await
            .map_err(|e| format!("Failed to add imported message: {}", e))?;
    }

    // Checkpoint the imported session
    memory_manager.checkpoint_current_session().await
        .map_err(|e| format!("Failed to checkpoint imported session: {}", e))?;

    Ok(session_id)
}

/// Search sessions by content (basic text search)
#[tauri::command]
pub async fn search_sessions(
    query: String,
    state: State<'_, AppState>
) -> Result<Vec<String>, String> {
    let memory_manager = state.get_memory_manager().await
        .ok_or("EventMemoryManager not initialized")?;

    let session_ids = memory_manager.list_sessions().await
        .map_err(|e| format!("Failed to list sessions: {}", e))?;

    let mut matching_sessions = Vec::new();

    // Simple search through session content
    for session_id in session_ids {
        if let Ok(messages) = memory_manager.load_session(&session_id).await {
            for message in messages {
                if message.content.to_lowercase().contains(&query.to_lowercase()) {
                    matching_sessions.push(session_id.clone());
                    break; // Found a match in this session, move to next
                }
            }
        }
    }

    Ok(matching_sessions)
}

/// Get session summary/metadata
#[tauri::command]
pub async fn get_session_info(
    session_id: String,
    state: State<'_, AppState>
) -> Result<SessionInfo, String> {
    let memory_manager = state.get_memory_manager().await
        .ok_or("EventMemoryManager not initialized")?;

    let messages = memory_manager.load_session(&session_id).await
        .map_err(|e| format!("Failed to load session: {}", e))?;

    // Calculate basic metadata
    let message_count = messages.len();
    let estimated_tokens: usize = messages.iter()
        .map(|msg| msg.content.len() / 4) // Rough token estimation
        .sum();

    // For now, use current time for timestamps (would be better to get from persistence metadata)
    let now = chrono::Utc::now().timestamp();

    Ok(SessionInfo {
        session_id,
        created_at: now,
        last_updated: now,
        message_count,
        estimated_tokens,
    })
}