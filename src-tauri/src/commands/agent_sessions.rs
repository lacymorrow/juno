//! Tauri commands that expose the parallel [`AgentSessionRegistry`] to the frontend.
//!
//! LAC-1432 introduces the ability to run multiple agents in parallel, each
//! with its own cursor overlay, cancellation, and status. The frontend
//! session-switcher and status-bar UIs need three things from the backend:
//!
//! 1. A snapshot list of every live session for the switcher.
//! 2. A way to change which session is "focused" (the one whose cursor is
//!    highlighted and whose escape key cancels).
//! 3. A way to cancel a specific session, or the focused one.
//!
//! These commands are the surface for that. Every mutating command emits
//! [`events::agent_sessions::UPDATED`] with the fresh snapshot so the
//! frontend never polls; focus changes also emit
//! [`events::agent_sessions::FOCUSED`] so cursor overlays can react to
//! the focus change specifically without diffing the list.

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};
use tracing::{debug, warn};

use crate::agents::{AgentSessionId, AgentSessionInfo};
use crate::constants::events;
use crate::state::AppState;

/// Emit the current session list to the frontend.
///
/// Called after any mutation so the switcher/status-bar re-render without
/// polling. Failure to emit is logged and swallowed — a broken event bus
/// must not abort the underlying registry mutation.
pub(crate) async fn emit_sessions_updated(app: &AppHandle, state: &AppState) {
    let list = state.agent_sessions().list().await;
    if let Err(e) = app.emit(events::agent_sessions::UPDATED, &list) {
        warn!("Failed to emit agent-sessions-updated: {}", e);
    } else {
        debug!(
            "Emitted agent-sessions-updated with {} sessions",
            list.len()
        );
    }
}

#[derive(Serialize)]
struct FocusedPayload {
    session_id: Option<String>,
}

fn emit_focused(app: &AppHandle, session_id: Option<String>) {
    let payload = FocusedPayload { session_id };
    if let Err(e) = app.emit(events::agent_sessions::FOCUSED, &payload) {
        warn!("Failed to emit agent-session-focused: {}", e);
    }
}

/// Return a snapshot of every live agent session.
///
/// Ordered by `started_at_ms` ascending so newer sessions appear at the
/// bottom of the switcher regardless of HashMap iteration order.
#[tauri::command]
pub async fn list_agent_sessions(
    state: State<'_, AppState>,
) -> Result<Vec<AgentSessionInfo>, String> {
    Ok(state.agent_sessions().list().await)
}

/// Return the id of the currently focused session, if any.
///
/// The frontend uses this on cold start to know which overlay to draw
/// with the "focused" outline before the first `UPDATED` event lands.
#[tauri::command]
pub async fn get_focused_agent_session(
    state: State<'_, AppState>,
) -> Result<Option<String>, String> {
    Ok(state.agent_sessions().focused().map(|id| id.to_string()))
}

/// Focus a session so escape cancels it and its cursor overlay highlights.
///
/// Pass `null`/`None` to clear focus. Rejects unknown ids so the UI can
/// surface stale focus attempts (e.g. after a session finishes between
/// the switcher render and the click).
#[tauri::command]
pub async fn focus_agent_session(
    app: AppHandle,
    state: State<'_, AppState>,
    session_id: Option<String>,
) -> Result<(), String> {
    let registry = state.agent_sessions();
    let target = session_id.clone().map(AgentSessionId::from);
    registry.set_focused(target).await?;
    emit_focused(&app, session_id);
    emit_sessions_updated(&app, &state).await;
    Ok(())
}

/// Cancel the currently focused session, if any. Returns `true` if a
/// session was cancelled.
///
/// This is the command the global escape shortcut invokes when the
/// parallel-agent switcher is active. Background sessions keep running
/// so the user can walk away from one agent and come back to another.
#[tauri::command]
pub async fn cancel_focused_agent_session(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<bool, String> {
    let registry = state.agent_sessions();
    let cancelled = registry.cancel_focused().await?;
    if cancelled {
        emit_sessions_updated(&app, &state).await;
    }
    Ok(cancelled)
}

/// Cancel a specific session by id.
///
/// Used by the switcher's per-row "cancel" affordance. Returns an error
/// if the session id is unknown (already finished or never existed) so
/// the UI can distinguish "cancel raced with completion" from success.
#[tauri::command]
pub async fn cancel_agent_session(
    app: AppHandle,
    state: State<'_, AppState>,
    session_id: String,
) -> Result<(), String> {
    let registry = state.agent_sessions();
    let id = AgentSessionId::from(session_id);
    registry.cancel(&id).await?;
    emit_sessions_updated(&app, &state).await;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::input_arbiter::InputArbiter;
    use crate::agents::AgentSessionRegistry;
    use std::sync::Arc;
    use std::time::Duration;

    fn registry() -> AgentSessionRegistry {
        AgentSessionRegistry::new(4, Arc::new(InputArbiter::new(Duration::from_millis(0))))
    }

    // Command handlers require a Tauri State/AppHandle so we cover them via
    // the registry directly — the commands are thin adapters and the
    // registry's own tests exercise the actual state machine.

    #[tokio::test]
    async fn focus_missing_session_is_rejected() {
        let registry = registry();
        let phantom = AgentSessionId::from("does-not-exist".to_string());
        let result = registry.set_focused(Some(phantom)).await;
        assert!(result.is_err(), "expected focus on unknown id to error");
    }

    #[tokio::test]
    async fn cancel_focused_reports_false_when_empty() {
        let registry = registry();
        let cancelled = registry
            .cancel_focused()
            .await
            .expect("cancel_focused with no sessions returns Ok(false)");
        assert!(!cancelled);
    }
}
