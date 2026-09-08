//! # Conversation history
//!
//! Past conversations survive an app restart so they can be reopened later. A
//! restart always starts a NEW empty chat (the current id is minted fresh in
//! `AppState::new`); the old conversations simply sit on disk until reloaded.
//!
//! Each conversation is one Tauri Store file (`conversation-<id>.json`) holding
//! the full message list, and a tiny index (`conversations-index.json`) holds
//! the lightweight metadata the history list needs, so listing never loads any
//! message bodies. Writes are captured at the single choke point where messages
//! enter the backend (`AdvancedMemoryManager::add_message`) BEFORE the live
//! 200-message prune, so the saved history is complete even when the model's
//! working context is capped. The capture is teed through an unbounded channel
//! into the background task below, which coalesces bursts into a debounced save.

use crate::agent::core::{Message, Role};
use crate::constants::events;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_store::StoreExt;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::sync::Mutex as TokioMutex;

const INDEX_FILE: &str = "conversations-index.json";
const INDEX_KEY: &str = "conversations";
const CONVERSATION_KEY: &str = "conversation";
const VERSION: u32 = 1;
const TITLE_MAX_CHARS: usize = 60;
const SAVE_DEBOUNCE_MS: u64 = 500;

fn conversation_file(id: &str) -> String {
    format!("conversation-{id}.json")
}

/// A new conversation id (also the startup "new chat" id).
pub fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Lightweight metadata for the history list. No message bodies.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationMeta {
    pub id: String,
    pub title: String,
    pub created_at: u64,
    pub updated_at: u64,
    pub message_count: usize,
}

/// The full persisted form of one conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationFile {
    pub version: u32,
    pub id: String,
    pub title: String,
    pub created_at: u64,
    pub updated_at: u64,
    pub messages: Vec<Message>,
}

/// Title from the first non-empty user message, else a placeholder. Uses
/// `chars().take` so a multi-byte title never panics on a byte boundary.
fn derive_title(messages: &[Message]) -> String {
    for m in messages {
        if m.role == Role::User {
            let t: String = m.content.trim().chars().take(TITLE_MAX_CHARS).collect();
            if !t.is_empty() {
                return t;
            }
        }
    }
    "New conversation".to_string()
}

fn read_index(app: &AppHandle) -> Vec<ConversationMeta> {
    let Ok(store) = app.store(INDEX_FILE) else {
        return Vec::new();
    };
    store
        .get(INDEX_KEY)
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default()
}

fn write_index(app: &AppHandle, index: &[ConversationMeta]) -> Result<(), String> {
    let store = app.store(INDEX_FILE).map_err(|e| e.to_string())?;
    let value = serde_json::to_value(index).map_err(|e| e.to_string())?;
    store.set(INDEX_KEY, value);
    store.save().map_err(|e| e.to_string())
}

fn upsert_index(app: &AppHandle, meta: ConversationMeta) -> Result<(), String> {
    let mut index = read_index(app);
    if let Some(existing) = index.iter_mut().find(|m| m.id == meta.id) {
        *existing = meta;
    } else {
        index.push(meta);
    }
    write_index(app, &index)
}

/// Every saved conversation, newest first. Index read only, no message bodies.
pub fn list(app: &AppHandle) -> Vec<ConversationMeta> {
    let mut index = read_index(app);
    index.sort_by_key(|m| std::cmp::Reverse(m.updated_at));
    index
}

/// The full message list for one conversation, or empty if it does not exist.
pub fn load_messages(app: &AppHandle, id: &str) -> Result<Vec<Message>, String> {
    let store = app
        .store(conversation_file(id))
        .map_err(|e| e.to_string())?;
    match store.get(CONVERSATION_KEY) {
        Some(v) => {
            let file: ConversationFile = serde_json::from_value(v).map_err(|e| e.to_string())?;
            Ok(file.messages)
        }
        None => Ok(Vec::new()),
    }
}

/// Persist a conversation's full message list and refresh its index entry. An
/// empty conversation is never written, so a bare "new chat" leaves no file.
fn save_conversation(app: &AppHandle, id: &str, messages: &[Message]) -> Result<(), String> {
    if messages.is_empty() {
        return Ok(());
    }
    let created_at = read_index(app)
        .into_iter()
        .find(|m| m.id == id)
        .map(|m| m.created_at)
        .unwrap_or_else(now_secs);
    let updated_at = now_secs();
    let title = derive_title(messages);
    let file = ConversationFile {
        version: VERSION,
        id: id.to_string(),
        title: title.clone(),
        created_at,
        updated_at,
        messages: messages.to_vec(),
    };
    let store = app
        .store(conversation_file(id))
        .map_err(|e| e.to_string())?;
    store.set(
        CONVERSATION_KEY,
        serde_json::to_value(&file).map_err(|e| e.to_string())?,
    );
    store.save().map_err(|e| e.to_string())?;
    upsert_index(
        app,
        ConversationMeta {
            id: id.to_string(),
            title,
            created_at,
            updated_at,
            message_count: messages.len(),
        },
    )
}

/// Remove a conversation from the index and delete its store file. Emptying the
/// cached store before removing the file stops the plugin from resurrecting it
/// with stale content on the next auto-save.
pub fn delete(app: &AppHandle, id: &str) -> Result<(), String> {
    let mut index = read_index(app);
    index.retain(|m| m.id != id);
    write_index(app, &index)?;
    if let Ok(store) = app.store(conversation_file(id)) {
        store.clear();
        let _ = store.save();
    }
    if let Ok(dir) = app.path().app_data_dir() {
        let _ = std::fs::remove_file(dir.join(conversation_file(id)));
    }
    Ok(())
}

/// Map the backend conversation to the frontend `ChatMessage[]` shape so a
/// reloaded conversation drops straight into the UI. The backend keeps only
/// role/content/tool-calls, so a reload shows text, tool calls and results, but
/// not thinking blocks or per-tool screenshots (those are frontend-only).
pub fn to_ui_messages(messages: &[Message]) -> Vec<serde_json::Value> {
    let mut out = Vec::new();
    for m in messages {
        match m.role {
            Role::User => out.push(serde_json::json!({ "role": "user", "content": m.content })),
            Role::System => out.push(serde_json::json!({ "role": "system", "content": m.content })),
            Role::Assistant => {
                if !m.content.trim().is_empty() {
                    out.push(serde_json::json!({ "role": "assistant", "content": m.content }));
                }
                if let Some(tool_calls) = &m.tool_calls {
                    for call in tool_calls {
                        out.push(serde_json::json!({
                            "role": "tool_call_request",
                            "content": "",
                            "tool_name": call.name,
                            "tool_args": call.input,
                            "tool_id": call.id,
                        }));
                    }
                }
            }
            Role::Tool => out.push(serde_json::json!({
                "role": "tool_call_result",
                "content": m.content,
                "tool_name": m.name,
                "tool_output": m.content,
                "tool_id": m.tool_call_id,
                "success": true,
            })),
        }
    }
    out
}

/// Emit the reloaded conversation to the frontend so it repopulates the chat.
pub fn emit_loaded(app: &AppHandle, messages: &[Message]) -> Result<(), String> {
    app.emit(
        events::messages::CONVERSATION_LOADED,
        serde_json::json!({ "messages": to_ui_messages(messages) }),
    )
    .map_err(|e| e.to_string())
}

/// Own the message stream and persist it. Each message carries no id, so the
/// task reads the current conversation id when the message arrives (a single
/// conversation is ever active, so there is no interleaving). When the id
/// changes, the previous conversation is flushed and the new one is seeded from
/// its file, so appending after a reload keeps the loaded history intact rather
/// than overwriting it with just the new turn.
pub fn spawn_persist_task(
    app: AppHandle,
    current_id: Arc<TokioMutex<String>>,
    mut rx: UnboundedReceiver<Message>,
) {
    tauri::async_runtime::spawn(async move {
        let mut accum: Option<(String, Vec<Message>)> = None;
        let mut dirty = false;
        loop {
            tokio::select! {
                maybe = rx.recv() => {
                    match maybe {
                        Some(message) => {
                            let id = current_id.lock().await.clone();
                            let switched = accum.as_ref().map(|(aid, _)| aid != &id).unwrap_or(true);
                            if switched {
                                if dirty {
                                    if let Some((aid, msgs)) = &accum {
                                        if let Err(e) = save_conversation(&app, aid, msgs) {
                                            log::warn!("[History] flush on switch failed: {e}");
                                        }
                                    }
                                }
                                let seed = load_messages(&app, &id).unwrap_or_default();
                                accum = Some((id.clone(), seed));
                            }
                            if let Some((_, msgs)) = accum.as_mut() {
                                msgs.push(message);
                            }
                            dirty = true;
                        }
                        None => {
                            if dirty {
                                if let Some((aid, msgs)) = &accum {
                                    let _ = save_conversation(&app, aid, msgs);
                                }
                            }
                            break;
                        }
                    }
                }
                _ = tokio::time::sleep(Duration::from_millis(SAVE_DEBOUNCE_MS)), if dirty => {
                    if let Some((aid, msgs)) = &accum {
                        if let Err(e) = save_conversation(&app, aid, msgs) {
                            log::warn!("[History] debounced save failed: {e}");
                        }
                    }
                    dirty = false;
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::core::ToolCall;

    fn msg(role: Role, content: &str) -> Message {
        Message {
            role,
            content: content.to_string(),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        }
    }

    #[test]
    fn title_uses_first_user_message() {
        let messages = vec![
            msg(Role::System, "boot"),
            msg(Role::User, "Rename my files"),
            msg(Role::Assistant, "sure"),
        ];
        assert_eq!(derive_title(&messages), "Rename my files");
    }

    #[test]
    fn title_placeholder_when_no_user_message() {
        assert_eq!(
            derive_title(&[msg(Role::Assistant, "hi")]),
            "New conversation"
        );
        assert_eq!(derive_title(&[]), "New conversation");
    }

    #[test]
    fn title_truncates_on_char_boundary() {
        let long = "é".repeat(100); // multi-byte; byte-slicing would panic
        let title = derive_title(&[msg(Role::User, &long)]);
        assert_eq!(title.chars().count(), TITLE_MAX_CHARS);
    }

    #[test]
    fn ui_mapping_folds_roles_and_expands_tool_calls() {
        let mut assistant = msg(Role::Assistant, "let me look");
        assistant.tool_calls = Some(vec![ToolCall {
            id: "call_1".to_string(),
            name: "screenshot".to_string(),
            input: serde_json::json!({ "region": "full" }),
        }]);
        let mut tool = msg(Role::Tool, "done");
        tool.tool_call_id = Some("call_1".to_string());
        tool.name = Some("screenshot".to_string());

        let ui = to_ui_messages(&[msg(Role::User, "hi"), assistant, tool]);

        let roles: Vec<&str> = ui.iter().filter_map(|m| m["role"].as_str()).collect();
        assert_eq!(
            roles,
            vec!["user", "assistant", "tool_call_request", "tool_call_result"]
        );
        assert_eq!(ui[2]["tool_name"], "screenshot");
        assert_eq!(ui[2]["tool_id"], "call_1");
        assert_eq!(ui[3]["role"], "tool_call_result");
    }

    #[test]
    fn ui_mapping_skips_empty_assistant_text_but_keeps_tool_call() {
        let mut assistant = msg(Role::Assistant, "   ");
        assistant.tool_calls = Some(vec![ToolCall {
            id: "c".to_string(),
            name: "click".to_string(),
            input: serde_json::json!({}),
        }]);
        let ui = to_ui_messages(&[assistant]);
        let roles: Vec<&str> = ui.iter().filter_map(|m| m["role"].as_str()).collect();
        assert_eq!(roles, vec!["tool_call_request"]);
    }
}
