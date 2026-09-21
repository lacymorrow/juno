//! Background operation and consent for taking the physical cursor.
//!
//! Background mode is the default: the agent reaches apps through accessibility
//! actions and events posted straight to the target process, so the user can keep
//! typing while it works. A few steps cannot be done that way: a drag on a canvas
//! with no accessibility tree, an app that refuses posted events. Those need the
//! one pointer the user and the agent share.
//!
//! Rather than quietly grabbing it, the tool layer asks. This module owns that
//! conversation: the consent rules, the request/response round trip with the UI,
//! and the "Juno has the mouse" signal that runs for as long as the takeover does.

use crate::constants::events;
use crate::constants::settings::defaults;
use crate::settings::manager::SettingsManager;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::OnceLock;
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::Mutex as TokioMutex;
use tracing::{debug, info, warn};

pub mod commands;

/// How long a pending request waits for the user before it is treated as a refusal.
const CONSENT_TIMEOUT_SECONDS: u64 = 60;

/// Poll interval while waiting, matching the tool-approval flow.
const CONSENT_POLL_INTERVAL_MS: u64 = 50;

// ── Cached background-mode flag ──────────────────────────────────────────────
//
// The setting is the source of truth; this is only a cache, because the flag is
// read on every click and an async settings load per click is not affordable.
// It is invalidated whenever agent settings change, so it can never drift.

const CACHE_UNKNOWN: u8 = 0;
const CACHE_OFF: u8 = 1;
const CACHE_ON: u8 = 2;

static BACKGROUND_MODE_CACHE: AtomicU8 = AtomicU8::new(CACHE_UNKNOWN);

/// Drop the cached background-mode flag. Called when agent settings change.
pub fn invalidate_background_mode_cache() {
    BACKGROUND_MODE_CACHE.store(CACHE_UNKNOWN, Ordering::Relaxed);
}

/// Record a known value and mirror it into the platform layer, which consults it
/// from synchronous code that cannot await a settings read.
fn store_background_mode(enabled: bool) {
    BACKGROUND_MODE_CACHE.store(
        if enabled { CACHE_ON } else { CACHE_OFF },
        Ordering::Relaxed,
    );
    computer_use_ai_sdk::background::set_background_mode(enabled);
}

/// Is the agent working in the background right now?
///
/// Falls back to the compiled-in default when settings are unavailable, so a
/// missing store never silently turns background mode off.
pub async fn background_mode_enabled(app_handle: &AppHandle) -> bool {
    match BACKGROUND_MODE_CACHE.load(Ordering::Relaxed) {
        CACHE_ON => return true,
        CACHE_OFF => return false,
        _ => {}
    }

    let enabled = match agent_settings(app_handle).await {
        Some(settings) => settings.background_mode,
        None => defaults::BACKGROUND_MODE,
    };
    store_background_mode(enabled);
    enabled
}

/// Push the persisted setting into the cache and the platform layer at startup.
pub async fn sync_background_mode(app_handle: &AppHandle) {
    let enabled = match agent_settings(app_handle).await {
        Some(settings) => settings.background_mode,
        None => defaults::BACKGROUND_MODE,
    };
    store_background_mode(enabled);
    info!("Background mode is {}", if enabled { "on" } else { "off" });
}

async fn agent_settings(app_handle: &AppHandle) -> Option<crate::settings::AgentSettings> {
    let manager = app_handle.try_state::<SettingsManager>()?;
    manager.get_agent_settings().await.ok()
}

// ── Consent rules ────────────────────────────────────────────────────────────

/// What a request for the physical cursor should do next.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsentOutcome {
    /// The user has already said yes to every takeover.
    Proceed,
    /// The user has to be asked for this one.
    Ask,
}

/// The user's answer to a single request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InputControlDecision {
    /// Allow this takeover only.
    Once,
    /// Allow this one and stop asking.
    Always,
    /// Refuse; the cursor is not to be touched.
    Deny,
}

impl InputControlDecision {
    /// Parse a decision sent from the UI. Unknown values are refusals, because
    /// the safe reading of "I did not understand you" is "do not take the mouse".
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_lowercase().as_str() {
            "once" => Some(Self::Once),
            "always" => Some(Self::Always),
            "deny" | "no" => Some(Self::Deny),
            _ => None,
        }
    }

    pub fn grants(&self) -> bool {
        matches!(self, Self::Once | Self::Always)
    }
}

/// Decide whether the user has to be asked, given their standing preference.
pub fn consent_outcome(mouse_control: &str) -> ConsentOutcome {
    if mouse_control.eq_ignore_ascii_case(defaults::MOUSE_CONTROL_ALWAYS) {
        ConsentOutcome::Proceed
    } else {
        ConsentOutcome::Ask
    }
}

/// Should the UI be offered the chance to stop asking every time?
///
/// Only after a takeover the user actually granted, only while they are still on
/// "ask", and only if they have not already waved the offer away.
pub fn should_offer_to_stop_asking(
    mouse_control: &str,
    prompt_dismissed: bool,
    already_offered: bool,
) -> bool {
    !already_offered && !prompt_dismissed && consent_outcome(mouse_control) == ConsentOutcome::Ask
}

// ── Pending requests ─────────────────────────────────────────────────────────
//
// Transient, process-local, and emptied as each request resolves. Shaped after
// the tool-approval flow in state.rs: the command records a decision, the waiting
// task polls for it.

type PendingRequests = TokioMutex<HashMap<String, Option<InputControlDecision>>>;

static PENDING: OnceLock<PendingRequests> = OnceLock::new();

fn pending() -> &'static PendingRequests {
    PENDING.get_or_init(|| TokioMutex::new(HashMap::new()))
}

/// Record the user's answer to a pending request. Returns false when the request
/// is unknown, which usually means it already timed out.
pub async fn record_decision(request_id: &str, decision: InputControlDecision) -> bool {
    let mut guard = pending().lock().await;
    match guard.get_mut(request_id) {
        Some(slot) => {
            *slot = Some(decision);
            true
        }
        None => false,
    }
}

/// The OFFER event is shown at most once per app run.
static OFFER_EMITTED: AtomicBool = AtomicBool::new(false);

// ── Taking the cursor ────────────────────────────────────────────────────────

/// Why a step could not be completed in the background.
#[derive(Debug, Clone)]
pub struct PhysicalCursorRequest {
    /// The computer-use action that needs the cursor, e.g. "left_click_drag".
    pub tool: String,
    /// One line the user can act on, without jargon.
    pub reason: String,
    /// The application the agent is working in, when it is known.
    pub target_app: Option<String>,
}

/// The user refused, or never answered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputControlDenied {
    /// The user said no.
    Denied,
    /// Nobody answered within the timeout.
    TimedOut,
}

impl InputControlDenied {
    /// A message the agent can read and recover from: it says what happened and
    /// what to try instead, rather than reading as a crash.
    pub fn agent_message(&self, tool: &str) -> String {
        match self {
            Self::Denied => format!(
                "'{}' needs control of the physical mouse and the user declined. \
                 Juno is running in the background. Try an accessibility-based route \
                 instead, such as clicking a named element, or ask the user to do this step.",
                tool
            ),
            Self::TimedOut => format!(
                "'{}' needs control of the physical mouse and the request timed out. \
                 The cursor was not touched. Try an accessibility-based route instead, \
                 or ask the user to allow mouse control.",
                tool
            ),
        }
    }
}

/// Live for as long as Juno is driving the physical cursor.
///
/// Creating it announces the takeover, dropping it announces the end, so no exit
/// path (success, error, cancellation, panic unwind) can leave the UI believing
/// the agent still has the mouse.
pub struct PhysicalCursorGrant {
    app_handle: AppHandle,
    tool: String,
    target_app: Option<String>,
    offer_when_done: bool,
}

impl PhysicalCursorGrant {
    fn new(
        app_handle: &AppHandle,
        tool: &str,
        target_app: Option<&str>,
        offer_when_done: bool,
    ) -> Self {
        let grant = Self {
            app_handle: app_handle.clone(),
            tool: tool.to_string(),
            target_app: target_app.map(|s| s.to_string()),
            offer_when_done,
        };
        grant.emit_state(true);
        grant
    }

    fn emit_state(&self, active: bool) {
        let payload = json!({
            "active": active,
            "tool": self.tool,
            "target_app": self.target_app,
        });
        if let Err(e) = self.app_handle.emit(events::input_control::STATE, payload) {
            debug!("Failed to emit input control state: {}", e);
        }
    }
}

impl Drop for PhysicalCursorGrant {
    fn drop(&mut self) {
        self.emit_state(false);

        if self.offer_when_done && !OFFER_EMITTED.swap(true, Ordering::Relaxed) {
            if let Err(e) = self.app_handle.emit(events::input_control::OFFER, ()) {
                debug!("Failed to emit input control offer: {}", e);
            }
        }
    }
}

/// Ask for the physical cursor, waiting for the user when their setting says to.
///
/// Callers hold the returned grant for the duration of the takeover and drop it
/// as soon as the step is done.
pub async fn request_physical_cursor(
    app_handle: &AppHandle,
    request: PhysicalCursorRequest,
) -> Result<PhysicalCursorGrant, InputControlDenied> {
    let settings = agent_settings(app_handle).await;
    let mouse_control = settings
        .as_ref()
        .map(|s| s.mouse_control.clone())
        .unwrap_or_else(|| defaults::MOUSE_CONTROL.to_string());
    let prompt_dismissed = settings
        .as_ref()
        .map(|s| s.mouse_control_prompt_dismissed)
        .unwrap_or(false);

    if consent_outcome(&mouse_control) == ConsentOutcome::Proceed {
        info!(
            "Taking the physical cursor for '{}' (mouse control is set to always)",
            request.tool
        );
        return Ok(PhysicalCursorGrant::new(
            app_handle,
            &request.tool,
            request.target_app.as_deref(),
            false,
        ));
    }

    let request_id = uuid::Uuid::new_v4().to_string();
    pending().lock().await.insert(request_id.clone(), None);

    let payload = json!({
        "request_id": request_id,
        "tool": request.tool,
        "reason": request.reason,
        "target_app": request.target_app,
        "timeout_seconds": CONSENT_TIMEOUT_SECONDS,
    });
    // A notification as well as the in-app prompt: a run started by the
    // scheduler or the cloud has no conversation on screen, so the pane the
    // prompt lives in may not be open at all. Without this the agent would
    // wait out the whole timeout asking nobody.
    notify_consent_request(app_handle, &request);

    if let Err(e) = app_handle.emit(events::input_control::REQUEST, payload) {
        // With no UI listening there is nobody to answer, so refuse rather than
        // wait out the full timeout holding the agent up.
        warn!("Failed to emit input control request: {}", e);
        pending().lock().await.remove(&request_id);
        return Err(InputControlDenied::Denied);
    }

    info!(
        "Waiting for permission to use the physical cursor for '{}'",
        request.tool
    );

    let decision = wait_for_decision(&request_id).await;
    pending().lock().await.remove(&request_id);

    let decision = match decision {
        Some(decision) => decision,
        None => {
            warn!("Input control request for '{}' timed out", request.tool);
            return Err(InputControlDenied::TimedOut);
        }
    };

    if decision == InputControlDecision::Always {
        if let Err(e) = persist_mouse_control(app_handle, defaults::MOUSE_CONTROL_ALWAYS).await {
            // A failed write is not a reason to refuse a takeover the user just
            // approved; they will simply be asked again next time.
            warn!("Failed to persist mouse control preference: {}", e);
        }
    }

    if !decision.grants() {
        info!("User declined the physical cursor for '{}'", request.tool);
        return Err(InputControlDenied::Denied);
    }

    let offer_when_done = decision == InputControlDecision::Once
        && should_offer_to_stop_asking(
            &mouse_control,
            prompt_dismissed,
            OFFER_EMITTED.load(Ordering::Relaxed),
        );

    Ok(PhysicalCursorGrant::new(
        app_handle,
        &request.tool,
        request.target_app.as_deref(),
        offer_when_done,
    ))
}

/// Tell the person Juno is waiting on them, in case nothing is on screen.
fn notify_consent_request(app_handle: &AppHandle, request: &PhysicalCursorRequest) {
    let body = match request.target_app.as_deref() {
        Some(app_name) => format!("Juno wants to {} in {}.", request.reason, app_name),
        None => format!("Juno wants to {}.", request.reason),
    };
    crate::commands::notifications::notify(app_handle, "Juno needs your mouse", &body);
}

async fn wait_for_decision(request_id: &str) -> Option<InputControlDecision> {
    let iterations = CONSENT_TIMEOUT_SECONDS * 1000 / CONSENT_POLL_INTERVAL_MS;
    for _ in 0..iterations {
        if let Some(decision) = pending().lock().await.get(request_id).copied().flatten() {
            return Some(decision);
        }
        tokio::time::sleep(std::time::Duration::from_millis(CONSENT_POLL_INTERVAL_MS)).await;
    }
    None
}

/// Write one field of the agent settings back, preserving everything else.
async fn persist_mouse_control(app_handle: &AppHandle, value: &str) -> Result<(), String> {
    let manager = app_handle
        .try_state::<SettingsManager>()
        .ok_or_else(|| "Settings manager is not available".to_string())?;
    let mut settings = manager.get_agent_settings().await?;
    settings.mouse_control = value.to_string();
    manager.set_agent_settings(&settings).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn always_proceeds_and_ask_asks() {
        assert_eq!(consent_outcome("always"), ConsentOutcome::Proceed);
        assert_eq!(consent_outcome("Always"), ConsentOutcome::Proceed);
        assert_eq!(consent_outcome("ask"), ConsentOutcome::Ask);
        // An unrecognized value must not be read as blanket permission.
        assert_eq!(consent_outcome("whatever"), ConsentOutcome::Ask);
    }

    #[test]
    fn decisions_parse_and_unknown_values_are_refusals() {
        assert_eq!(
            InputControlDecision::parse("once"),
            Some(InputControlDecision::Once)
        );
        assert_eq!(
            InputControlDecision::parse(" ALWAYS "),
            Some(InputControlDecision::Always)
        );
        assert_eq!(
            InputControlDecision::parse("deny"),
            Some(InputControlDecision::Deny)
        );
        assert_eq!(InputControlDecision::parse("maybe"), None);
        assert!(InputControlDecision::Once.grants());
        assert!(InputControlDecision::Always.grants());
        assert!(!InputControlDecision::Deny.grants());
    }

    #[test]
    fn the_offer_is_made_once_and_only_while_still_asking() {
        assert!(should_offer_to_stop_asking("ask", false, false));
        assert!(!should_offer_to_stop_asking("ask", true, false));
        assert!(!should_offer_to_stop_asking("ask", false, true));
        assert!(!should_offer_to_stop_asking("always", false, false));
    }

    #[test]
    fn denial_messages_name_the_tool_and_a_way_forward() {
        let denied = InputControlDenied::Denied.agent_message("left_click_drag");
        assert!(denied.contains("left_click_drag"));
        assert!(denied.contains("accessibility"));
        let timed_out = InputControlDenied::TimedOut.agent_message("scroll");
        assert!(timed_out.contains("scroll"));
        assert!(timed_out.contains("not touched"));
    }

    #[tokio::test]
    async fn a_decision_for_an_unknown_request_is_rejected() {
        assert!(!record_decision("no-such-request", InputControlDecision::Once).await);
    }

    #[tokio::test]
    async fn a_recorded_decision_is_visible_to_the_waiter() {
        let id = "test-request-visible";
        pending().lock().await.insert(id.to_string(), None);
        assert!(record_decision(id, InputControlDecision::Always).await);
        assert_eq!(
            pending().lock().await.get(id).copied().flatten(),
            Some(InputControlDecision::Always)
        );
        pending().lock().await.remove(id);
    }
}
