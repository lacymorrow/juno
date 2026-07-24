use std::collections::{BTreeSet, HashMap};
use std::sync::{Arc, Mutex as StdMutex};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};
use tokio::sync::{watch, Mutex as TokioMutex};
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::agent::input_arbiter::InputArbiter;
use crate::constants::events;
use crate::constants::ui::agent_session_colors;

/// Fixed 8-slot identity palette for parallel agent sessions
/// (LAC-2830 spec section 2). Index = color slot.
pub const SESSION_COLOR_SLOTS: [&str; 8] = [
    agent_session_colors::SLOT_0,
    agent_session_colors::SLOT_1,
    agent_session_colors::SLOT_2,
    agent_session_colors::SLOT_3,
    agent_session_colors::SLOT_4,
    agent_session_colors::SLOT_5,
    agent_session_colors::SLOT_6,
    agent_session_colors::SLOT_7,
];

/// Round-robin color slot allocator with slot reuse.
///
/// Freed slots are handed out before fresh ones so long-lived fleets keep
/// stable, distinct colors. If more sessions run than palette slots, slots
/// repeat — a visual collision, not a correctness issue (LAC-2830 spec).
#[derive(Default)]
struct ColorAllocator {
    next_slot: u8,
    freed: BTreeSet<u8>,
}

impl ColorAllocator {
    fn allocate(&mut self) -> u8 {
        if let Some(slot) = self.freed.iter().next().copied() {
            self.freed.remove(&slot);
            return slot;
        }
        let slot = self.next_slot;
        self.next_slot = (self.next_slot + 1) % SESSION_COLOR_SLOTS.len() as u8;
        slot
    }

    fn free(&mut self, slot: u8) {
        if (slot as usize) < SESSION_COLOR_SLOTS.len() {
            self.freed.insert(slot);
        }
    }
}

/// Hex color for a palette slot.
pub fn color_for_slot(slot: u8) -> &'static str {
    SESSION_COLOR_SLOTS[slot as usize % SESSION_COLOR_SLOTS.len()]
}

/// Unique identifier for a parallel agent session.
///
/// Every parallel agent gets its own [`AgentSessionId`] so backend state
/// (memory, tool approvals, cancellation token, cursor overlay window)
/// stays isolated across simultaneous runs.
#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct AgentSessionId(String);

impl AgentSessionId {
    pub fn new() -> Self {
        Self(Uuid::new_v4().to_string())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for AgentSessionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<String> for AgentSessionId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

/// Lifecycle status of an agent session, surfaced to the switcher/status-bar UI.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentSessionStatus {
    Starting,
    Running,
    NeedsInput,
    Cancelling,
    Cancelled,
    Finished,
    Failed,
}

impl AgentSessionStatus {
    /// Terminal states — the session is done and will be removed shortly.
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Cancelled | Self::Finished | Self::Failed)
    }
}

/// Snapshot of a session's user-facing metadata.
///
/// Emitted to the frontend for the parallel-agents switcher and status bar.
/// This is intentionally serializable and free of any backend handles so it
/// can be sent through Tauri events without leaking Arc/Mutex internals.
#[derive(Clone, Debug, Serialize)]
pub struct AgentSessionInfo {
    pub id: String,
    pub agent_name: String,
    pub color_slot: u8,
    pub display_color: String,
    pub status: AgentSessionStatus,
    pub current_action: Option<String>,
    pub started_at_ms: u64,
    pub last_activity_ms: u64,
    pub focused: bool,
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// One agent's private slice of session state.
///
/// Each session owns its own cancellation channel — escape or the switcher
/// UI can cancel a single agent without disturbing the others. Mutable
/// metadata (status, current action) lives behind a `TokioMutex` because
/// it is updated from the async execution loop and read from Tauri command
/// handlers that render the switcher.
pub struct AgentSession {
    id: AgentSessionId,
    agent_name: String,
    color_slot: u8,
    cancel_tx: watch::Sender<bool>,
    cancel_rx: watch::Receiver<bool>,
    started_at_ms: u64,
    inner: TokioMutex<AgentSessionInner>,
}

struct AgentSessionInner {
    status: AgentSessionStatus,
    current_action: Option<String>,
    last_activity_ms: u64,
}

impl AgentSession {
    fn new_with_id(id: AgentSessionId, agent_name: String, color_slot: u8) -> Arc<Self> {
        let (cancel_tx, cancel_rx) = watch::channel(false);
        let started_at_ms = now_ms();
        Arc::new(Self {
            id,
            agent_name,
            color_slot,
            cancel_tx,
            cancel_rx,
            started_at_ms,
            inner: TokioMutex::new(AgentSessionInner {
                status: AgentSessionStatus::Starting,
                current_action: None,
                last_activity_ms: started_at_ms,
            }),
        })
    }

    pub fn id(&self) -> &AgentSessionId {
        &self.id
    }

    pub fn agent_name(&self) -> &str {
        &self.agent_name
    }

    pub fn color_slot(&self) -> u8 {
        self.color_slot
    }

    pub fn display_color(&self) -> &'static str {
        color_for_slot(self.color_slot)
    }

    /// Clone the cancellation receiver so the agent's execution loop can
    /// observe cancellation requests without holding a lock on the registry.
    pub fn cancel_receiver(&self) -> watch::Receiver<bool> {
        self.cancel_rx.clone()
    }

    /// Signal cancellation for this session only. Idempotent.
    pub fn cancel(&self) {
        if let Err(e) = self.cancel_tx.send(true) {
            warn!(
                "Failed to signal cancellation for session {}: {}",
                self.id, e
            );
        }
    }

    pub fn is_cancelled(&self) -> bool {
        *self.cancel_rx.borrow()
    }

    pub async fn set_status(&self, status: AgentSessionStatus) {
        let mut guard = self.inner.lock().await;
        guard.status = status;
        guard.last_activity_ms = now_ms();
    }

    pub async fn set_current_action(&self, action: Option<String>) {
        let mut guard = self.inner.lock().await;
        guard.current_action = action;
        guard.last_activity_ms = now_ms();
    }

    pub async fn snapshot(&self, focused: bool) -> AgentSessionInfo {
        let guard = self.inner.lock().await;
        AgentSessionInfo {
            id: self.id.0.clone(),
            agent_name: self.agent_name.clone(),
            color_slot: self.color_slot,
            display_color: self.display_color().to_string(),
            status: guard.status,
            current_action: guard.current_action.clone(),
            started_at_ms: self.started_at_ms,
            last_activity_ms: guard.last_activity_ms,
            focused,
        }
    }
}

/// Registry of every live agent session.
///
/// Owns the input arbiter shared by all sessions so coordinate-based
/// physical input is serialized across the fleet (macOS has one pointer).
/// AX-grounded actions do not touch the arbiter and run in parallel.
///
/// `sessions` uses a `TokioMutex` because `list()` calls `session.snapshot().await`
/// while iterating. `focused` uses a plain `StdMutex` — no async work happens
/// while holding it, so the lighter-weight lock is appropriate.
pub struct AgentSessionRegistry {
    sessions: TokioMutex<HashMap<AgentSessionId, Arc<AgentSession>>>,
    focused: StdMutex<Option<AgentSessionId>>,
    colors: StdMutex<ColorAllocator>,
    input_arbiter: Arc<InputArbiter>,
    max_parallel: usize,
}

impl AgentSessionRegistry {
    pub fn new(max_parallel: usize, input_arbiter: Arc<InputArbiter>) -> Self {
        Self {
            sessions: TokioMutex::new(HashMap::new()),
            focused: StdMutex::new(None),
            colors: StdMutex::new(ColorAllocator::default()),
            input_arbiter,
            max_parallel,
        }
    }

    pub fn max_parallel(&self) -> usize {
        self.max_parallel
    }

    pub fn input_arbiter(&self) -> Arc<InputArbiter> {
        self.input_arbiter.clone()
    }

    /// Create a new session and register it. Fails if the parallel cap is hit.
    ///
    /// Assigns the next free identity-color slot (LAC-2830 palette); the slot
    /// is returned to the allocator when the session is removed. The first
    /// session created becomes the focused session automatically; callers can
    /// override focus later via [`set_focused`].
    pub async fn create(&self, agent_name: String) -> Result<Arc<AgentSession>, String> {
        let mut sessions = self.sessions.lock().await;
        if sessions.len() >= self.max_parallel {
            return Err(format!(
                "Parallel session cap reached ({}); cancel or finish an existing session first",
                self.max_parallel
            ));
        }
        let color_slot = {
            let mut colors = self.colors.lock().unwrap_or_else(|e| e.into_inner());
            colors.allocate()
        };
        let id = AgentSessionId::new();
        let session = AgentSession::new_with_id(id.clone(), agent_name, color_slot);
        sessions.insert(id.clone(), session.clone());
        drop(sessions);

        // Auto-focus the first session so escape has an obvious target.
        let mut focused = self.focused.lock().unwrap_or_else(|e| e.into_inner());
        let is_focused = focused.is_none();
        if is_focused {
            *focused = Some(id.clone());
        }
        info!("Registered agent session {} (focused={})", id, is_focused);
        Ok(session)
    }

    pub async fn get(&self, id: &AgentSessionId) -> Option<Arc<AgentSession>> {
        self.sessions.lock().await.get(id).cloned()
    }

    pub async fn remove(&self, id: &AgentSessionId) {
        // Capture the next candidate before releasing the sessions lock so we
        // don't need to re-acquire it inside the focused critical section.
        let mut sessions = self.sessions.lock().await;
        let removed = sessions.remove(id);
        let next_id = sessions.keys().next().cloned();
        drop(sessions);

        if let Some(session) = removed {
            debug!("Removed agent session {} from registry", id);
            let mut colors = self.colors.lock().unwrap_or_else(|e| e.into_inner());
            colors.free(session.color_slot());
        }

        let mut focused = self.focused.lock().unwrap_or_else(|e| e.into_inner());
        if focused.as_ref() == Some(id) {
            *focused = next_id;
        }
    }

    pub async fn cancel(&self, id: &AgentSessionId) -> Result<(), String> {
        let session = self
            .get(id)
            .await
            .ok_or_else(|| format!("Session {} not found", id))?;
        session.set_status(AgentSessionStatus::Cancelling).await;
        session.cancel();
        Ok(())
    }

    /// Cancel the currently focused session, if any.
    ///
    /// Escape handling uses this so pressing escape only kills the agent
    /// the user is watching; background sessions keep running.
    pub async fn cancel_focused(&self) -> Result<bool, String> {
        let focused = self.focused.lock().unwrap_or_else(|e| e.into_inner()).clone();
        match focused {
            Some(id) => {
                self.cancel(&id).await?;
                Ok(true)
            }
            None => Ok(false),
        }
    }

    pub async fn set_focused(&self, id: Option<AgentSessionId>) -> Result<(), String> {
        if let Some(ref candidate) = id {
            let sessions = self.sessions.lock().await;
            if !sessions.contains_key(candidate) {
                return Err(format!("Cannot focus unknown session {}", candidate));
            }
        }
        *self.focused.lock().unwrap_or_else(|e| e.into_inner()) = id;
        Ok(())
    }

    pub fn focused(&self) -> Option<AgentSessionId> {
        self.focused.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// List a snapshot of every session for the switcher/status-bar UI.
    pub async fn list(&self) -> Vec<AgentSessionInfo> {
        let focused = self.focused.lock().unwrap_or_else(|e| e.into_inner()).clone();
        let sessions = self.sessions.lock().await.clone();
        let mut out = Vec::with_capacity(sessions.len());
        for (id, session) in sessions.iter() {
            let is_focused = focused.as_ref() == Some(id);
            out.push(session.snapshot(is_focused).await);
        }
        out.sort_by(|a, b| a.started_at_ms.cmp(&b.started_at_ms));
        out
    }

    pub async fn len(&self) -> usize {
        self.sessions.lock().await.len()
    }
}

/// Emit the current session list to the frontend.
///
/// Standalone helper so background tasks and RAII cleanup can broadcast
/// updates without needing a `State<AppState>` handle.
pub async fn broadcast_sessions_updated(app: &AppHandle, registry: &Arc<AgentSessionRegistry>) {
    let list = registry.list().await;
    if let Err(e) = app.emit(events::agent_sessions::UPDATED, &list) {
        warn!("Failed to emit agent-sessions-updated: {}", e);
    }
}

/// RAII guard that removes an agent session from the registry on drop.
///
/// `execute_agent_internal` has ~8 explicit `return Err` paths plus a
/// fall-through success path; threading manual `registry.remove()` calls
/// through every path is brittle. This guard removes the session on any
/// exit (including panic unwinds) and broadcasts an `agent-sessions-updated`
/// event so the switcher UI drops the row.
///
/// Registry mutation is async, so cleanup is scheduled on the Tauri async
/// runtime — `Drop` itself stays cheap and synchronous.
pub struct SessionHandle {
    registry: Arc<AgentSessionRegistry>,
    session: Arc<AgentSession>,
    app_handle: AppHandle,
    active: bool,
}

impl SessionHandle {
    pub fn new(
        registry: Arc<AgentSessionRegistry>,
        session: Arc<AgentSession>,
        app_handle: AppHandle,
    ) -> Self {
        Self {
            registry,
            session,
            app_handle,
            active: true,
        }
    }

    pub fn session(&self) -> &Arc<AgentSession> {
        &self.session
    }

    /// Mark the session as finished/failed/cancelled and broadcast the state
    /// before the RAII cleanup removes the row entirely. Callers that know
    /// whether the run succeeded or failed should call this to give the
    /// UI a final status snapshot instead of the row just disappearing.
    ///
    /// Also emits the discrete lifecycle event for the terminal state
    /// (completed / cancelled / failed) so the roster UI can play its
    /// pulse / shake animations and the backend can decide whether to fire
    /// a system notification.
    pub async fn mark_terminal(&self, status: AgentSessionStatus) {
        self.session.set_status(status).await;
        let focused = self.registry.focused().as_ref() == Some(self.session.id());
        let snapshot = self.session.snapshot(focused).await;
        let event = match status {
            AgentSessionStatus::Cancelled => Some(events::agent_sessions::CANCELLED),
            AgentSessionStatus::Finished => Some(events::agent_sessions::COMPLETED),
            AgentSessionStatus::Failed => Some(events::agent_sessions::FAILED),
            _ => None,
        };
        if let Some(event) = event {
            if let Err(e) = self.app_handle.emit(event, &snapshot) {
                warn!("Failed to emit {} for session {}: {}", event, snapshot.id, e);
            }
        }
        broadcast_sessions_updated(&self.app_handle, &self.registry).await;
    }

    /// True when this session is the currently focused one.
    pub fn is_focused(&self) -> bool {
        self.registry.focused().as_ref() == Some(self.session.id())
    }
}

/// Remove any cursor overlay identity left behind by a session.
///
/// The desktop cursor overlay renders one cursor slot per agent id; the
/// computer-use tool registers cursors under the session id (LAC-1432), so
/// clearing that id here guarantees the overlay cursor disappears on every
/// session end path — complete, cancel, or error.
fn cleanup_session_cursor(app_handle: &AppHandle, session_id: &str) {
    crate::agent::tools::anthropic_computer_use::emit_agent_cursor_remove(app_handle, session_id);
}

impl Drop for SessionHandle {
    fn drop(&mut self) {
        if !self.active {
            return;
        }
        let registry = self.registry.clone();
        let session_id = self.session.id().clone();
        let app_handle = self.app_handle.clone();
        tauri::async_runtime::spawn(async move {
            cleanup_session_cursor(&app_handle, session_id.as_str());
            registry.remove(&session_id).await;
            broadcast_sessions_updated(&app_handle, &registry).await;
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn arbiter() -> Arc<InputArbiter> {
        Arc::new(InputArbiter::new(Duration::from_millis(0)))
    }

    #[tokio::test]
    async fn creates_and_lists_sessions() {
        let registry = AgentSessionRegistry::new(4, arbiter());
        let a = registry
            .create("desktop".into())
            .await
            .expect("first session created");
        let b = registry
            .create("browser".into())
            .await
            .expect("second session created");

        let listed = registry.list().await;
        assert_eq!(listed.len(), 2);
        assert!(listed.iter().any(|s| s.id == a.id().to_string()));
        assert!(listed.iter().any(|s| s.id == b.id().to_string()));

        // First session auto-focused; snapshot exposes it.
        let focused_id = registry.focused().expect("focused set");
        assert_eq!(&focused_id, a.id());
        assert!(listed.iter().any(|s| s.focused && s.id == a.id().to_string()));
    }

    #[tokio::test]
    async fn enforces_parallel_cap() {
        let registry = AgentSessionRegistry::new(1, arbiter());
        registry
            .create("first".into())
            .await
            .expect("first ok");
        let result = registry.create("second".into()).await;
        let err = match result {
            Ok(_) => panic!("expected second create to fail"),
            Err(e) => e,
        };
        assert!(err.contains("cap reached"), "unexpected error: {err}");
    }

    #[tokio::test]
    async fn cancel_focused_kills_only_focused_session() {
        let registry = AgentSessionRegistry::new(4, arbiter());
        let focused_session = registry
            .create("focused".into())
            .await
            .expect("focused created");
        let background = registry
            .create("background".into())
            .await
            .expect("background created");

        let mut focused_rx = focused_session.cancel_receiver();
        let background_rx = background.cancel_receiver();

        let cancelled = registry
            .cancel_focused()
            .await
            .expect("cancel_focused ok");
        assert!(cancelled);

        // The focused session sees the cancel; the background session does not.
        assert!(focused_rx.has_changed().unwrap_or(false));
        focused_rx.borrow_and_update();
        assert!(*focused_rx.borrow());
        assert!(!*background_rx.borrow());
    }

    #[tokio::test]
    async fn remove_clears_and_reassigns_focus() {
        let registry = AgentSessionRegistry::new(4, arbiter());
        let a = registry.create("a".into()).await.unwrap();
        let b = registry.create("b".into()).await.unwrap();

        assert_eq!(registry.focused().as_ref(), Some(a.id()));
        registry.remove(a.id()).await;
        // Focus falls back to the remaining session.
        assert_eq!(registry.focused().as_ref(), Some(b.id()));

        registry.remove(b.id()).await;
        assert!(registry.focused().is_none());
        assert_eq!(registry.len().await, 0);
    }

    #[tokio::test]
    async fn color_slots_assigned_round_robin_and_freed_on_remove() {
        let registry = AgentSessionRegistry::new(12, arbiter());
        let a = registry.create("a".into()).await.unwrap();
        let b = registry.create("b".into()).await.unwrap();
        let c = registry.create("c".into()).await.unwrap();
        assert_eq!(a.color_slot(), 0);
        assert_eq!(b.color_slot(), 1);
        assert_eq!(c.color_slot(), 2);
        assert_eq!(a.display_color(), SESSION_COLOR_SLOTS[0]);

        // Removing a session frees its slot; the next session reuses the
        // lowest freed slot instead of advancing the round-robin counter.
        registry.remove(b.id()).await;
        let d = registry.create("d".into()).await.unwrap();
        assert_eq!(d.color_slot(), 1, "freed slot must be reused first");

        let e = registry.create("e".into()).await.unwrap();
        assert_eq!(e.color_slot(), 3, "fresh slots continue round-robin");
    }

    #[tokio::test]
    async fn set_focused_rejects_unknown_id() {
        let registry = AgentSessionRegistry::new(4, arbiter());
        let phantom = AgentSessionId::new();
        assert!(registry.set_focused(Some(phantom)).await.is_err());
    }
}
