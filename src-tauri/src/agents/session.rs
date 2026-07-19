use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tokio::sync::{watch, Mutex as TokioMutex};
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::agent::input_arbiter::InputArbiter;

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
    Finished,
    Failed,
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
    display_color: String,
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
    fn new_with_id(
        id: AgentSessionId,
        agent_name: String,
        display_color: String,
    ) -> Arc<Self> {
        let (cancel_tx, cancel_rx) = watch::channel(false);
        let started_at_ms = now_ms();
        Arc::new(Self {
            id,
            agent_name,
            display_color,
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

    pub fn display_color(&self) -> &str {
        &self.display_color
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
            display_color: self.display_color.clone(),
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
pub struct AgentSessionRegistry {
    sessions: TokioMutex<HashMap<AgentSessionId, Arc<AgentSession>>>,
    focused: TokioMutex<Option<AgentSessionId>>,
    input_arbiter: Arc<InputArbiter>,
    max_parallel: usize,
}

impl AgentSessionRegistry {
    pub fn new(max_parallel: usize, input_arbiter: Arc<InputArbiter>) -> Self {
        Self {
            sessions: TokioMutex::new(HashMap::new()),
            focused: TokioMutex::new(None),
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
    /// The first session created becomes the focused session automatically;
    /// callers can override focus later via [`set_focused`].
    pub async fn create(
        &self,
        agent_name: String,
        display_color: String,
    ) -> Result<Arc<AgentSession>, String> {
        let mut sessions = self.sessions.lock().await;
        if sessions.len() >= self.max_parallel {
            return Err(format!(
                "Parallel session cap reached ({}); cancel or finish an existing session first",
                self.max_parallel
            ));
        }
        let id = AgentSessionId::new();
        let session = AgentSession::new_with_id(id.clone(), agent_name, display_color);
        sessions.insert(id.clone(), session.clone());
        drop(sessions);

        // Auto-focus the first session so escape has an obvious target.
        let mut focused = self.focused.lock().await;
        if focused.is_none() {
            *focused = Some(id.clone());
        }
        info!(
            "Registered agent session {} (focused={})",
            id,
            focused.as_ref().map(|f| f == &id).unwrap_or(false)
        );
        Ok(session)
    }

    pub async fn get(&self, id: &AgentSessionId) -> Option<Arc<AgentSession>> {
        self.sessions.lock().await.get(id).cloned()
    }

    pub async fn remove(&self, id: &AgentSessionId) {
        let mut sessions = self.sessions.lock().await;
        if sessions.remove(id).is_some() {
            debug!("Removed agent session {} from registry", id);
        }
        drop(sessions);

        let mut focused = self.focused.lock().await;
        if focused.as_ref() == Some(id) {
            *focused = None;
            let sessions = self.sessions.lock().await;
            if let Some(next) = sessions.keys().next().cloned() {
                *focused = Some(next);
            }
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
        let focused = self.focused.lock().await.clone();
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
        *self.focused.lock().await = id;
        Ok(())
    }

    pub async fn focused(&self) -> Option<AgentSessionId> {
        self.focused.lock().await.clone()
    }

    /// List a snapshot of every session for the switcher/status-bar UI.
    pub async fn list(&self) -> Vec<AgentSessionInfo> {
        let focused = self.focused.lock().await.clone();
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
            .create("desktop".into(), "#ff00aa".into())
            .await
            .expect("first session created");
        let b = registry
            .create("browser".into(), "#00aaff".into())
            .await
            .expect("second session created");

        let listed = registry.list().await;
        assert_eq!(listed.len(), 2);
        assert!(listed.iter().any(|s| s.id == a.id().to_string()));
        assert!(listed.iter().any(|s| s.id == b.id().to_string()));

        // First session auto-focused; snapshot exposes it.
        let focused_id = registry.focused().await.expect("focused set");
        assert_eq!(&focused_id, a.id());
        assert!(listed.iter().any(|s| s.focused && s.id == a.id().to_string()));
    }

    #[tokio::test]
    async fn enforces_parallel_cap() {
        let registry = AgentSessionRegistry::new(1, arbiter());
        registry
            .create("first".into(), "#111111".into())
            .await
            .expect("first ok");
        let result = registry.create("second".into(), "#222222".into()).await;
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
            .create("focused".into(), "#f".into())
            .await
            .expect("focused created");
        let background = registry
            .create("background".into(), "#b".into())
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
        let a = registry.create("a".into(), "#a".into()).await.unwrap();
        let b = registry.create("b".into(), "#b".into()).await.unwrap();

        assert_eq!(registry.focused().await.as_ref(), Some(a.id()));
        registry.remove(a.id()).await;
        // Focus falls back to the remaining session.
        assert_eq!(registry.focused().await.as_ref(), Some(b.id()));

        registry.remove(b.id()).await;
        assert!(registry.focused().await.is_none());
        assert_eq!(registry.len().await, 0);
    }

    #[tokio::test]
    async fn set_focused_rejects_unknown_id() {
        let registry = AgentSessionRegistry::new(4, arbiter());
        let phantom = AgentSessionId::new();
        assert!(registry.set_focused(Some(phantom)).await.is_err());
    }
}
