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
use std::collections::HashMap;
use std::sync::{Arc, Mutex as StdMutex, OnceLock};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_store::StoreExt;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::sync::Mutex as TokioMutex;

pub(crate) const INDEX_FILE: &str = "conversations-index.json";
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

/// What the person actually said. A saved user message carries the system
/// context block (time, focused app, running apps, location) the model was
/// given, followed by `User Query: <text>`. History shows and titles only the
/// query. The stored message is left whole (the model still gets the context
/// it was given), so old files need no migration.
pub(crate) fn visible_user_text(content: &str) -> &str {
    const MARKER: &str = "\n\nUser Query: ";
    if content.starts_with("Current time: ") {
        if let Some(at) = content.find(MARKER) {
            return &content[at + MARKER.len()..];
        }
    }
    content
}

/// Title from the first non-empty user message, else a placeholder. Uses
/// `chars().take` so a multi-byte title never panics on a byte boundary.
fn derive_title(messages: &[Message]) -> String {
    for m in messages {
        if m.role == Role::User {
            let t: String = visible_user_text(&m.content)
                .trim()
                .chars()
                .take(TITLE_MAX_CHARS)
                .collect();
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

/// Text streamed to the person so far, per streaming message id. Every
/// provider streams through `emit_streaming_text_chunk`, so this is the one
/// place that knows what was shown or spoken when a turn is cut short.
fn streamed() -> &'static StdMutex<HashMap<String, String>> {
    static STREAMED: OnceLock<StdMutex<HashMap<String, String>>> = OnceLock::new();
    STREAMED.get_or_init(|| StdMutex::new(HashMap::new()))
}

/// Remember a streamed chunk (shown text and/or a spoken block).
pub(crate) fn record_streamed(message_id: &str, shown: &str, spoken: Option<&str>) {
    let Ok(mut map) = streamed().lock() else {
        return;
    };
    // Entries are taken by the runner at the end of every step. A caller that
    // never takes (a local intent) must not grow the map without bound.
    if map.len() > 64 && !map.contains_key(message_id) {
        map.clear();
    }
    let entry = map.entry(message_id.to_string()).or_default();
    for piece in [spoken.unwrap_or(""), shown] {
        if piece.trim().is_empty() {
            continue;
        }
        if !entry.is_empty() && !entry.ends_with(char::is_whitespace) {
            entry.push(' ');
        }
        entry.push_str(piece);
    }
}

/// Take (and forget) what was streamed under this id.
pub(crate) fn take_streamed(message_id: &str) -> String {
    streamed()
        .lock()
        .ok()
        .and_then(|mut map| map.remove(message_id))
        .unwrap_or_default()
}

/// The assistant message to keep for a finished turn, or `None` when there is
/// no text. `<TTS>` markers are dropped but the spoken words stay, in order.
/// A turn that was cut short ends in an ellipsis. Compact by construction:
/// text only, no tool calls, no thinking.
pub(crate) fn reply_message(text: &str, interrupted: bool) -> Option<Message> {
    let cleaned = text.replace("<TTS>", "").replace("</TTS>", " ");
    let cleaned = cleaned.trim();
    if cleaned.is_empty() {
        return None;
    }
    let content = if interrupted {
        format!("{cleaned}\u{2026}")
    } else {
        cleaned.to_string()
    };
    Some(Message {
        role: Role::Assistant,
        content,
        tool_calls: None,
        tool_call_id: None,
        name: None,
        images: None,
    })
}

fn build_file(
    id: &str,
    messages: &[Message],
    created_at: u64,
    updated_at: u64,
) -> ConversationFile {
    ConversationFile {
        version: VERSION,
        id: id.to_string(),
        title: derive_title(messages),
        created_at,
        updated_at,
        messages: messages.to_vec(),
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
    let file = build_file(id, messages, created_at, updated_at);
    let title = file.title.clone();
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

/// Append one message to a conversation that is not the live one, straight
/// to its file. For a turn Claude ran on its own in a conversation the person
/// has since moved away from (`claude_cli_own_turn`). The live conversation
/// goes through the memory manager instead, like every other message.
pub fn append_message(app: &AppHandle, id: &str, message: Message) -> Result<(), String> {
    let mut messages = load_messages(app, id)?;
    messages.push(message);
    save_conversation(app, id, &messages)
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
            Role::User => out.push(
                serde_json::json!({ "role": "user", "content": visible_user_text(&m.content) }),
            ),
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
                    let saved = match &accum {
                        Some((aid, msgs)) => match save_conversation(&app, aid, msgs) {
                            Ok(()) => true,
                            Err(e) => {
                                log::warn!("[History] debounced save failed: {e}");
                                false
                            }
                        },
                        None => false,
                    };
                    if saved {
                        // The file is now the whole truth, so the next message
                        // reseeds from it. A message appended to the file in
                        // the meantime (`append_message`) is then kept rather
                        // than overwritten by this stale copy.
                        accum = None;
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
            images: None,
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

    #[test]
    fn visible_text_drops_context_block() {
        let raw = "Current time: 9am\nPlatform: macOS\n\nUser Query: open Notes";
        assert_eq!(visible_user_text(raw), "open Notes");
        assert_eq!(visible_user_text("plain question"), "plain question");
        // A query that mentions the marker is not cut when there is no context.
        let odd = "say\n\nUser Query: hi";
        assert_eq!(visible_user_text(odd), odd);
        assert_eq!(derive_title(&[msg(Role::User, raw)]), "open Notes");
        let ui = to_ui_messages(&[msg(Role::User, raw)]);
        assert_eq!(ui[0]["content"], "open Notes");
    }

    #[test]
    fn a_turn_saves_both_sides() {
        let reply = reply_message("<TTS>It is 4pm.</TTS>\n\nSet by your clock.", false).unwrap();
        assert_eq!(reply.role, Role::Assistant);
        assert!(reply.content.starts_with("It is 4pm."));
        assert!(!reply.content.contains("TTS"));
        assert!(reply.tool_calls.is_none());
        let file = build_file("c1", &[msg(Role::User, "what time is it"), reply], 1, 2);
        let back: ConversationFile =
            serde_json::from_value(serde_json::to_value(&file).unwrap()).unwrap();
        assert_eq!(back.messages.len(), 2);
        assert_eq!(back.messages[1].role, Role::Assistant);
        let ui = to_ui_messages(&back.messages);
        assert_eq!(ui[1]["role"], "assistant");
    }

    #[test]
    fn empty_reply_is_not_saved() {
        assert!(reply_message("  <TTS></TTS> ", false).is_none());
    }

    #[test]
    fn a_cancelled_turn_keeps_what_was_streamed() {
        record_streamed("cancel-test", "", Some("Opening Notes."));
        record_streamed("cancel-test", "Looking for", None);
        let partial = take_streamed("cancel-test");
        assert_eq!(partial, "Opening Notes. Looking for");
        assert_eq!(take_streamed("cancel-test"), "");
        let kept = reply_message(&partial, true).unwrap();
        assert!(kept.content.ends_with('\u{2026}'));
        assert!(reply_message("", true).is_none());
    }

    #[test]
    fn old_file_without_replies_still_loads() {
        let old = serde_json::json!({
            "version": 1,
            "id": "old",
            "title": "Current time: 9am",
            "created_at": 1,
            "updated_at": 2,
            "messages": [
                { "role": "User", "content": "Current time: 9am\n\nUser Query: hi" }
            ]
        });
        let file: ConversationFile = serde_json::from_value(old).unwrap();
        assert_eq!(file.messages.len(), 1);
        let ui = to_ui_messages(&file.messages);
        assert_eq!(ui.len(), 1);
        assert_eq!(ui[0]["content"], "hi");
    }
}
