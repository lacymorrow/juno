//! Onboarding analytics — append-only event buffer persisted to Tauri Store.
//!
//! Events are recorded locally only; nothing is sent over the network. The
//! buffer is bounded to `MAX_EVENTS` to keep the store file size predictable.
//!
//! Event names and payload contracts are documented in
//! `docs/specs/AGENT_RESPONSE_SPEC.md` and on the LAC-1882 issue. The frontend
//! is the primary caller for everything except `onboarding_first_query`, which
//! is fired by `anthropic::submit_query` once `OnboardingPhase::Complete` has
//! been reached.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::AppHandle;
use tauri_plugin_store::StoreExt;
use tracing::{debug, warn};

const STORE_NAME: &str = "onboarding_analytics.json";
const EVENTS_KEY: &str = "events";
/// Stores the last-recorded phase string so the modal Onboarding can resume
/// from the right step on app restart (Phase D edge case #12).
const LAST_PHASE_KEY: &str = "last_phase";

/// Hard cap on retained events. Buffer is FIFO past this limit so the store
/// file stays bounded even if a user replays onboarding many times.
const MAX_EVENTS: usize = 500;

/// Tracks whether `onboarding_first_query` has already been emitted this
/// process lifetime. Reset only by a full app restart (intentional — the event
/// is meant to fire once per "fresh first interaction").
static FIRST_QUERY_RECORDED: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredEvent {
    name: String,
    payload: Value,
    t_unix_ms: u64,
}

fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn append_event(app: &AppHandle, name: &str, payload: Value) -> Result<(), String> {
    let store = app
        .store(STORE_NAME)
        .map_err(|e| format!("Failed to open analytics store: {}", e))?;

    let mut events: Vec<StoredEvent> = match store.get(EVENTS_KEY) {
        Some(Value::Array(arr)) => arr
            .into_iter()
            .filter_map(|v| serde_json::from_value::<StoredEvent>(v).ok())
            .collect(),
        _ => Vec::new(),
    };

    events.push(StoredEvent {
        name: name.to_string(),
        payload,
        t_unix_ms: now_unix_ms(),
    });

    // FIFO trim — drop oldest events when over cap.
    if events.len() > MAX_EVENTS {
        let drop = events.len() - MAX_EVENTS;
        events.drain(0..drop);
    }

    let serialized = serde_json::to_value(&events)
        .map_err(|e| format!("Failed to serialize analytics events: {}", e))?;
    store.set(EVENTS_KEY, serialized);
    store
        .save()
        .map_err(|e| format!("Failed to save analytics store: {}", e))?;
    debug!(
        "[onboarding-analytics] recorded {} (buffer={})",
        name,
        events.len()
    );
    Ok(())
}

/// Frontend-callable: append an arbitrary onboarding analytics event.
///
/// `event_name` must be one of the names documented on LAC-1882:
/// `onboarding_started`, `onboarding_phase_entered`,
/// `onboarding_permission_granted`, `onboarding_permission_skipped`,
/// `onboarding_error_recovery`, `onboarding_completed`,
/// `onboarding_first_query`. The command does not enforce the whitelist; new
/// events can be added without a Rust change.
///
/// Side effect: when `event_name == "onboarding_phase_entered"` and the
/// payload includes `phase`, we persist that phase as the resume point. On
/// `onboarding_completed` we clear the resume point so the next "Restart
/// onboarding" run starts fresh.
#[tauri::command]
pub async fn record_onboarding_event(
    app: AppHandle,
    event_name: String,
    payload: Option<Value>,
) -> Result<(), String> {
    let payload = payload.unwrap_or(Value::Null);

    // Persist the last-entered phase for restart resume.
    if event_name == "onboarding_phase_entered" {
        if let Some(phase) = payload.get("phase").and_then(|v| v.as_str()) {
            if let Ok(store) = app.store(STORE_NAME) {
                store.set(LAST_PHASE_KEY, Value::String(phase.to_string()));
                let _ = store.save();
            }
        }
    } else if event_name == "onboarding_completed" {
        if let Ok(store) = app.store(STORE_NAME) {
            store.delete(LAST_PHASE_KEY);
            let _ = store.save();
        }
    }

    append_event(&app, &event_name, payload)
}

/// Return the most recently entered phase, if any, so the modal Onboarding
/// can resume on app restart. Returns `None` if the user has not entered any
/// phase or has already completed onboarding.
#[tauri::command]
pub async fn get_last_onboarding_phase(app: AppHandle) -> Result<Option<String>, String> {
    let store = app
        .store(STORE_NAME)
        .map_err(|e| format!("Failed to open analytics store: {}", e))?;
    Ok(store
        .get(LAST_PHASE_KEY)
        .and_then(|v| v.as_str().map(String::from)))
}

/// Backend helper: fire `onboarding_first_query` once if the user has
/// completed onboarding. Idempotent across process lifetime via
/// `FIRST_QUERY_RECORDED`.
///
/// Called from `submit_query`. Returns silently if onboarding is incomplete
/// or the event has already been recorded.
pub async fn maybe_record_first_query(app: &AppHandle) {
    if FIRST_QUERY_RECORDED.load(Ordering::Relaxed) {
        return;
    }

    // Inspect the canonical phase. Only fire if the user has actually finished
    // onboarding — pre-completion queries (e.g. agent submits during Phase A
    // chat) don't count as the "first real query."
    let phase = crate::commands::onboarding::get_onboarding_state().await;
    match phase {
        Ok(info) if info.phase == crate::commands::onboarding::OnboardingPhase::Complete => {}
        _ => return,
    }

    // Compute t_ms_since_completed by scanning the buffer for the latest
    // onboarding_completed event. If no such event is on file (e.g. the user
    // completed onboarding before analytics shipped), report null.
    let t_ms_since_completed = read_t_since_last(app, "onboarding_completed");

    let payload = serde_json::json!({
        "t_ms_since_completed": t_ms_since_completed,
    });

    if FIRST_QUERY_RECORDED
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return; // Lost the race — another caller already recorded.
    }

    if let Err(e) = append_event(app, "onboarding_first_query", payload) {
        warn!("[onboarding-analytics] first_query record failed: {}", e);
        // Allow a retry if the store write failed.
        FIRST_QUERY_RECORDED.store(false, Ordering::Release);
    }
}

/// Read elapsed ms since the most recent event with `name`.
/// Returns `None` if no such event is present in the buffer.
fn read_t_since_last(app: &AppHandle, name: &str) -> Option<u64> {
    let store = app.store(STORE_NAME).ok()?;
    let raw = store.get(EVENTS_KEY)?;
    let arr = raw.as_array()?;
    let latest_ts = arr
        .iter()
        .rev()
        .filter_map(|v| serde_json::from_value::<StoredEvent>(v.clone()).ok())
        .find(|e| e.name == name)
        .map(|e| e.t_unix_ms)?;
    Some(now_unix_ms().saturating_sub(latest_ts))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn now_unix_ms_is_monotonic_ish() {
        let a = now_unix_ms();
        let b = now_unix_ms();
        assert!(b >= a);
    }
}
