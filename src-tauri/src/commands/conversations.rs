//! # Conversation history commands
//!
//! List, load, delete past conversations and start a new one. The store itself
//! lives in `crate::conversation_history`; these commands wire it to the live
//! `AppState` (the active conversation id + the in-memory manager) and the UI.

use tauri::{AppHandle, State};

use crate::conversation_history::{self, ConversationMeta};
use crate::state::AppState;

/// Every saved conversation, newest first (metadata only, no message bodies).
#[tauri::command]
pub async fn list_conversations(app_handle: AppHandle) -> Result<Vec<ConversationMeta>, String> {
    Ok(conversation_history::list(&app_handle))
}

/// The active conversation's id, so the history list can highlight it.
#[tauri::command]
pub async fn get_current_conversation_id(state: State<'_, AppState>) -> Result<String, String> {
    Ok(state.current_conversation_id.lock().await.clone())
}

/// Load a past conversation into the live chat: replace the backend messages,
/// make it the active conversation, and emit `conversation-loaded` so the UI
/// repopulates. Replacing the messages does not re-save the file.
#[tauri::command]
pub async fn load_conversation(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> Result<(), String> {
    let messages = conversation_history::load_messages(&app_handle, &id)?;
    {
        let memory_manager = state.get_memory_manager().await;
        let guard = memory_manager.lock().await;
        guard.replace_messages(messages.clone()).await;
    }
    *state.current_conversation_id.lock().await = id;
    conversation_history::emit_loaded(&app_handle, &messages)
}

/// Start a fresh chat: rotate the active id and clear the live conversation. The
/// previous conversation is already persisted, so it stays in history. This does
/// not emit `conversation-loaded`; the caller (the frontend "New Chat" button)
/// clears its own view, which avoids a redundant clear racing an added message.
#[tauri::command]
pub async fn new_conversation(state: State<'_, AppState>) -> Result<(), String> {
    *state.current_conversation_id.lock().await = conversation_history::new_id();
    let memory_manager = state.get_memory_manager().await;
    let guard = memory_manager.lock().await;
    guard.replace_messages(Vec::new()).await;
    Ok(())
}

/// Delete a saved conversation. If it is the active one, start a fresh chat and
/// clear the live view (via an empty `conversation-loaded`), since its messages
/// no longer have anywhere to be saved.
#[tauri::command]
pub async fn delete_conversation(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> Result<(), String> {
    conversation_history::delete(&app_handle, &id)?;
    let is_current = { *state.current_conversation_id.lock().await == id };
    if is_current {
        new_conversation(state).await?;
        conversation_history::emit_loaded(&app_handle, &[])?;
    }
    Ok(())
}
