//! # A turn Claude started on its own
//!
//! A persistent `claude` process can run a turn nobody asked for: a background
//! task that outlived an interrupted turn finishes, and the CLI wakes up and
//! answers it (see `claude_cli_session`, "A persistent process starts turns
//! nobody asked for"). That turn can act on the desktop, so it is shown, never
//! hidden. It belongs to the conversation that owns the process and appears
//! there as a turn of its own: never as the answer to whatever the person asks
//! next.
//!
//! `claude_cli_session` reads and renders the turn's frames with the same code
//! a normal turn uses. This module is everything around them that a normal
//! turn gets from `execute_agent_internal`: the bar and the working state, a
//! row in the session roster so Escape has something to cancel, the turn's
//! place in the conversation's history, and, when the person is looking at a
//! different conversation, a way back to the one it ran in.
//!
//! No notice, no prompt, no setting. Anything that fails here is logged and
//! dropped.

use std::sync::Mutex;

use tauri::{AppHandle, Emitter, Manager};
use tracing::{debug, info};

use crate::agent::core::{Message, Role};
use crate::agent::traits::MemoryManager;
use crate::agents::{AgentSessionStatus, SessionHandle};
use crate::constants::events;
use crate::state::AppState;

/// The roster row's name, and what the bar calls the turn.
const AGENT_NAME: &str = "Claude";

/// The escape-key ledger entry held for the turn's duration.
const ESCAPE_USER: &str = "claude_own_turn";

/// The conversation the last off-screen turn ran in, until the person opens it
/// or opens it some other way.
static ELSEWHERE: Mutex<Option<String>> = Mutex::new(None);

/// Is `conversation_id` the conversation the chat surfaces are showing?
pub async fn is_on_screen(app: &AppHandle, conversation_id: &str) -> bool {
    match app.try_state::<AppState>() {
        Some(state) => *state.current_conversation_id.lock().await == conversation_id,
        None => false,
    }
}

/// Everything a turn Claude started holds while it runs. Built at the turn's
/// first visible frame and consumed by [`Stage::finish`].
pub struct Stage {
    app: AppHandle,
    conversation_id: String,
    on_screen: bool,
    session: Option<SessionHandle>,
    /// Whether this turn put the app into its working state, and so must take
    /// it back out. False when a run was already going.
    owns_run: bool,
    escape: bool,
    /// Inside the person's own turn (frames that arrived before its `started`
    /// frame). That turn already owns the bar, the roster row and Escape, so
    /// this stage only renders and records.
    nested: bool,
}

impl Stage {
    /// Raise the turn: the bar goes to work, Escape is armed, the roster gets
    /// a row. `nested` skips all three (see the field).
    pub async fn begin(app: &AppHandle, conversation_id: &str, nested: bool) -> Self {
        let on_screen = is_on_screen(app, conversation_id).await;
        info!(
            "[CliOwnTurn] Claude started a turn in conversation {conversation_id} ({})",
            if on_screen {
                "on screen"
            } else {
                "not on screen"
            }
        );

        // Escape latches the last turn's speech off. This is a new turn, and
        // the person should hear it.
        crate::tts::begin_turn();

        let mut stage = Stage {
            app: app.clone(),
            conversation_id: conversation_id.to_string(),
            on_screen,
            session: None,
            owns_run: false,
            escape: false,
            nested,
        };
        if !on_screen {
            set_elsewhere(app, Some(conversation_id));
        }
        if nested {
            return stage;
        }

        let headless = crate::cli::headless::is_headless_mode();
        if let Some(state) = app.try_state::<AppState>() {
            if !state.is_agent_executing() {
                stage.owns_run = true;
                if let Err(e) = crate::state_management::handle_agent_execution_state_transition(
                    app,
                    true,
                    Some(uuid::Uuid::new_v4().to_string()),
                    None,
                )
                .await
                {
                    debug!("[CliOwnTurn] Could not mark the run started: {e}");
                }
                if !headless {
                    crate::commands::ui_commands::handle_agent_started(app).await;
                }
            }
            stage.session =
                crate::agents::begin_session_run(&state.agent_sessions(), AGENT_NAME, app).await;
        }
        if !headless {
            let coordinator = crate::commands::escape_key_coordinator::get_escape_key_coordinator();
            stage.escape = coordinator
                .register_escape_user(app, ESCAPE_USER)
                .await
                .is_ok();
        }
        stage
    }

    /// Whether the chat surfaces show this turn as it streams. Off screen, the
    /// bar and speech still carry it, and the history keeps it.
    pub fn on_screen(&self) -> bool {
        self.on_screen
    }

    /// Fires when Escape (or the roster's stop) cancels this turn. `None` for
    /// a nested stage, whose enclosing turn handles cancellation.
    pub fn cancel_rx(&self) -> Option<tokio::sync::watch::Receiver<bool>> {
        self.session
            .as_ref()
            .map(|handle| handle.session().cancel_receiver())
    }

    /// Put the turn down: record it, release Escape, close the roster row and,
    /// if nothing else is running, return the bar to rest.
    pub async fn finish(self, text: String, cancelled: bool) {
        record(&self.app, &self.conversation_id, &text);
        if self.nested {
            return;
        }

        if self.escape {
            let coordinator = crate::commands::escape_key_coordinator::get_escape_key_coordinator();
            if let Err(e) = coordinator
                .unregister_escape_user(&self.app, ESCAPE_USER)
                .await
            {
                debug!("[CliOwnTurn] Could not release Escape: {e}");
            }
        }

        let status = if cancelled {
            AgentSessionStatus::Cancelled
        } else {
            AgentSessionStatus::Finished
        };
        if let Some(handle) = self.session.as_ref() {
            handle.mark_terminal(status).await;
        }

        // The person may have asked something while this ran; their run is
        // waiting for this process and already owns the working state.
        if !self.owns_run || another_run_is_live(&self.app, self.session.as_ref()).await {
            return;
        }
        if let Err(e) = crate::state_management::handle_agent_execution_state_transition(
            &self.app, false, None, None,
        )
        .await
        {
            debug!("[CliOwnTurn] Could not mark the run finished: {e}");
        }
        if !crate::cli::headless::is_headless_mode() {
            crate::commands::ui_commands::handle_agent_stopped(&self.app).await;
            let state = if cancelled { "Cancelled" } else { "Finished" };
            crate::commands::ui_commands::handle_backend_response(
                &self.app,
                Some(text),
                state.to_string(),
            )
            .await;
        }
    }
}

/// Is any roster row other than `ours` still running?
async fn another_run_is_live(app: &AppHandle, ours: Option<&SessionHandle>) -> bool {
    let Some(state) = app.try_state::<AppState>() else {
        return false;
    };
    let ours = ours.map(|handle| handle.session().id().to_string());
    state
        .agent_sessions()
        .list()
        .await
        .iter()
        .any(|info| Some(&info.id) != ours.as_ref() && !info.status.is_terminal())
}

/// Keep the turn in its conversation's history, as an assistant message of
/// its own. The live conversation takes it through the memory manager like
/// any other message; one the person has moved away from takes it on disk.
///
/// Off the caller's path: a run in flight can hold the memory manager.
fn record(app: &AppHandle, conversation_id: &str, text: &str) {
    if text.trim().is_empty() {
        return;
    }
    let message = Message {
        role: Role::Assistant,
        content: text.to_string(),
        tool_calls: None,
        tool_call_id: None,
        name: None,
        images: None,
    };
    let app = app.clone();
    let id = conversation_id.to_string();
    tauri::async_runtime::spawn(async move {
        if is_on_screen(&app, &id).await {
            let Some(state) = app.try_state::<AppState>() else {
                return;
            };
            let memory = state.get_memory_manager().await;
            let mut memory = memory.lock().await;
            if let Err(e) = memory.add_message(message).await {
                debug!("[CliOwnTurn] Could not record the turn: {e}");
            }
        } else if let Err(e) = crate::conversation_history::append_message(&app, &id, message) {
            debug!("[CliOwnTurn] Could not record the turn in {id}: {e}");
        }
    });
}

fn elsewhere() -> std::sync::MutexGuard<'static, Option<String>> {
    ELSEWHERE.lock().unwrap_or_else(|e| e.into_inner())
}

/// Tell the bar which conversation a click should open, or that none should.
fn set_elsewhere(app: &AppHandle, conversation_id: Option<&str>) {
    *elsewhere() = conversation_id.map(str::to_string);
    if let Err(e) = app.emit(
        events::bar::CONVERSATION_ELSEWHERE,
        serde_json::json!({ "conversation_id": conversation_id }),
    ) {
        debug!("[CliOwnTurn] Could not tell the bar: {e}");
    }
}

/// The conversation a click on the bar should open, handed over once.
pub fn take_elsewhere(app: &AppHandle) -> Option<String> {
    let taken = elsewhere().take();
    if taken.is_some() {
        set_elsewhere(app, None);
    }
    taken
}

/// The person reached `conversation_id` some other way (or left for a new
/// one): the bar stops offering it.
pub fn conversation_opened(app: &AppHandle, conversation_id: &str) {
    let pending = elsewhere().as_deref() == Some(conversation_id);
    if pending {
        set_elsewhere(app, None);
    }
}
