//! # Armed monitors: what is watching, and how to stop it
//!
//! Screen and file monitors are armed by the model (`agent/tools/timer_tools.rs`),
//! poll the world on an interval, and start a turn of their own when they fire.
//! Until these commands existed nothing outside that one file could even *name*
//! an armed watch, which is how a loop capturing the screen every two seconds
//! became something the person could not see and could not stop.
//!
//! Escape is the primary stop and goes through `stop_coordinator`, which calls
//! the same `cancel_all_timers`. These commands are the queryable state and a
//! direct stop for a surface that wants one.
//!
//! No new bar state is introduced here on purpose: `docs/plans/ambient-awareness.md`
//! proposes a `watching` state and that is a product decision that has not been
//! made.

use crate::agent::tools::timer_tools::{timer_manager, MAX_WAKES_PER_WINDOW};
use serde_json::json;
use tauri::AppHandle;
use tracing::info;

/// What is currently watching, and how much wake budget is left.
#[tauri::command]
pub async fn list_armed_monitors() -> Result<serde_json::Value, String> {
    let manager = timer_manager();
    let monitors = manager.armed_monitors().await;

    let rows: Vec<serde_json::Value> = monitors
        .iter()
        .map(|monitor| {
            json!({
                "id": monitor.id,
                "kind": monitor.timer_type.label(),
                "description": monitor.description,
                "created_at": monitor.created_at,
                "deadline": monitor.trigger_time,
            })
        })
        .collect();

    let wakes_remaining = manager.wakes_remaining().await;

    Ok(json!({
        "count": rows.len(),
        "monitors": rows,
        "wakes_remaining": wakes_remaining,
        "max_wakes_per_hour": MAX_WAKES_PER_WINDOW,
    }))
}

/// Stop every armed watch and pending timer. Idempotent.
///
/// The same call Escape makes through the stop coordinator, exposed on its own
/// so a surface can offer "stop watching" without stopping everything else in
/// the app.
#[tauri::command]
pub async fn stop_armed_monitors(app_handle: AppHandle) -> Result<usize, String> {
    let cancelled = timer_manager().cancel_all_timers(Some(&app_handle)).await;
    info!(
        "[Monitors] Stopped {} armed timer(s) at the person's request",
        cancelled.len()
    );
    Ok(cancelled.len())
}
