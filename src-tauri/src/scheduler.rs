//! # Scheduled Automations
//!
//! User-facing scheduled automations: persistent cron-based tasks that fire agent
//! queries on a schedule ("check my emails every morning", "remind me to review PRs
//! on Friday").
//!
//! ## Architecture
//! - Automations are persisted in the Tauri Store (`scheduled_automations.json`) —
//!   the store is the single source of truth; the tick loop reloads it each cycle so
//!   there is no in-memory cache to drift out of sync.
//! - A background loop (started from `lib.rs` setup) checks for due automations
//!   every `TICK_INTERVAL_SECS` and fires them sequentially through the same
//!   orchestrator entry point the cloud connector uses (`anthropic::submit_query`).
//! - Cron parsing uses the `cron` crate (6/7-field expressions with seconds).
//!   Standard 5-field expressions are accepted and normalized by prepending `0 `.
//!
//! Used by: `commands/scheduled_tasks.rs` (user CRUD), `agent/tools/schedule_tools.rs`
//! (agent self-scheduling from natural language).

use chrono::Local;
use cron::Schedule;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashSet;
use std::str::FromStr;
use std::sync::{Mutex as StdMutex, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_notification::NotificationExt;
use tauri_plugin_store::StoreExt;
use tokio::sync::Mutex as TokioMutex;
use tracing::{info, warn};

use crate::constants::events;

/// Store file holding all scheduled automations.
pub const STORE_FILE: &str = "scheduled_automations.json";
/// Key inside the store under which the automation list is saved.
pub const STORE_KEY: &str = "automations";
/// How often the scheduler checks for due automations.
const TICK_INTERVAL_SECS: u64 = 30;
/// Minimum allowed gap between consecutive runs of an automation. Each firing
/// is a full unattended agent run, so per-second cron schedules would burn
/// tokens continuously.
pub const MIN_INTERVAL_SECS: u64 = 60;

/// Serializes every load→mutate→save cycle on the automations store so
/// concurrent writers (CRUD commands, agent tools, the tick loop) cannot
/// clobber each other's changes with stale snapshots (lost updates).
static STORE_LOCK: OnceLock<TokioMutex<()>> = OnceLock::new();

/// Ids of automations currently firing — prevents the tick loop and a manual
/// "Run now" from double-firing the same automation concurrently.
static IN_FLIGHT: OnceLock<StdMutex<HashSet<String>>> = OnceLock::new();

fn in_flight() -> &'static StdMutex<HashSet<String>> {
    IN_FLIGHT.get_or_init(|| StdMutex::new(HashSet::new()))
}

/// RAII claim on a firing automation; releases the id when dropped, so every
/// exit path (success, error, panic unwind) frees the claim.
pub struct InFlightClaim(String);

impl Drop for InFlightClaim {
    fn drop(&mut self) {
        let mut set = in_flight().lock().unwrap_or_else(|p| p.into_inner());
        set.remove(&self.0);
    }
}

/// Claims `id` for firing. Returns `None` if that automation is already firing.
pub fn try_claim_firing(id: &str) -> Option<InFlightClaim> {
    let mut set = in_flight().lock().unwrap_or_else(|p| p.into_inner());
    if set.insert(id.to_string()) {
        Some(InFlightClaim(id.to_string()))
    } else {
        None
    }
}

/// A user-visible scheduled automation that fires an agent query on a cron schedule.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduledAutomation {
    /// Unique identifier (UUID v4)
    pub id: String,
    /// Short human-readable name shown in the UI
    pub name: String,
    /// The agent query submitted to the orchestrator when the schedule fires
    pub query: String,
    /// Cron expression (6/7-field with seconds; 5-field input is normalized)
    pub cron: String,
    /// Original natural-language schedule ("every Monday at 9am") for display
    #[serde(default)]
    pub natural_language: Option<String>,
    /// Whether the automation is active
    pub enabled: bool,
    /// Whether to show a system notification when the automation runs
    pub notify: bool,
    /// Unix timestamp (seconds) when the automation was created
    pub created_at: u64,
    /// Unix timestamp of the most recent run, if any
    #[serde(default)]
    pub last_run_at: Option<u64>,
    /// Unix timestamp of the next scheduled run
    #[serde(default)]
    pub next_run_at: Option<u64>,
    /// Outcome of the most recent run ("submitted" or "error: ...")
    #[serde(default)]
    pub last_result: Option<String>,
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| Duration::from_secs(0))
        .as_secs()
}

/// Normalizes a cron expression for the `cron` crate, which requires a seconds field.
/// Standard 5-field expressions ("0 9 * * MON") get a `0` seconds field prepended.
pub fn normalize_cron(expr: &str) -> String {
    let trimmed = expr.trim();
    if trimmed.split_whitespace().count() == 5 {
        format!("0 {}", trimmed)
    } else {
        trimmed.to_string()
    }
}

/// Parses a cron expression, returning a user-facing error on failure.
pub fn parse_cron(expr: &str) -> Result<Schedule, String> {
    Schedule::from_str(&normalize_cron(expr))
        .map_err(|e| format!("Invalid cron expression '{}': {}", expr, e))
}

/// Computes the next run time (Unix seconds, local timezone) for a cron expression.
pub fn compute_next_run(expr: &str) -> Result<u64, String> {
    parse_cron(expr)?
        .upcoming(Local)
        .next()
        .map(|dt| dt.timestamp().max(0) as u64)
        .ok_or_else(|| format!("Cron expression '{}' has no future occurrences", expr))
}

/// Returns the next `count` run times for a cron expression (for UI preview).
pub fn preview_next_runs(expr: &str, count: usize) -> Result<Vec<u64>, String> {
    Ok(parse_cron(expr)?
        .upcoming(Local)
        .take(count)
        .map(|dt| dt.timestamp().max(0) as u64)
        .collect())
}

/// Rejects cron expressions whose upcoming runs are closer together than
/// `MIN_INTERVAL_SECS`. Checked at create/update time by both the user CRUD
/// commands and the agent tool.
pub fn validate_cron_interval(expr: &str) -> Result<(), String> {
    let runs = preview_next_runs(expr, 3)?;
    for pair in runs.windows(2) {
        if pair[1].saturating_sub(pair[0]) < MIN_INTERVAL_SECS {
            return Err(format!(
                "Schedule fires too frequently — the minimum interval between runs is {} seconds",
                MIN_INTERVAL_SECS
            ));
        }
    }
    Ok(())
}

/// Loads all automations from the Tauri Store.
pub fn load_automations(app: &AppHandle) -> Result<Vec<ScheduledAutomation>, String> {
    let store = app
        .store(STORE_FILE)
        .map_err(|e| format!("Failed to open automations store: {}", e))?;
    match store.get(STORE_KEY) {
        Some(value) => {
            let entries: Vec<serde_json::Value> = serde_json::from_value(value)
                .map_err(|e| format!("Failed to parse stored automations: {}", e))?;
            // Parse per-entry so one malformed record (crash mid-write, manual
            // edit, schema drift) degrades to a logged drop instead of failing
            // every CRUD call and permanently stalling the tick loop.
            Ok(entries
                .into_iter()
                .filter_map(|entry| match serde_json::from_value(entry) {
                    Ok(automation) => Some(automation),
                    Err(e) => {
                        warn!("Dropping malformed scheduled automation entry: {}", e);
                        None
                    }
                })
                .collect())
        }
        None => Ok(Vec::new()),
    }
}

/// Runs `mutate` against a freshly loaded automation list while holding the
/// store lock, saving only when the closure reports the list dirty. All
/// read-modify-write cycles on the store must go through this — a bare
/// `load_automations`/`save_automations` pair races with concurrent writers.
pub async fn with_automations<T, F>(app: &AppHandle, mutate: F) -> Result<T, String>
where
    F: FnOnce(&mut Vec<ScheduledAutomation>) -> Result<(T, bool), String>,
{
    let _guard = STORE_LOCK.get_or_init(|| TokioMutex::new(())).lock().await;
    let mut automations = load_automations(app)?;
    let (result, dirty) = mutate(&mut automations)?;
    if dirty {
        save_automations(app, &automations)?;
    }
    Ok(result)
}

/// Persists the automation list and notifies the frontend that it changed.
pub fn save_automations(
    app: &AppHandle,
    automations: &[ScheduledAutomation],
) -> Result<(), String> {
    let store = app
        .store(STORE_FILE)
        .map_err(|e| format!("Failed to open automations store: {}", e))?;
    let value = serde_json::to_value(automations)
        .map_err(|e| format!("Failed to serialize automations: {}", e))?;
    store.set(STORE_KEY, value);
    store
        .save()
        .map_err(|e| format!("Failed to save automations store: {}", e))?;
    if let Err(e) = app.emit(events::scheduler::AUTOMATIONS_CHANGED, ()) {
        warn!("Failed to emit automations-changed event: {}", e);
    }
    Ok(())
}

/// Applies `mutate` to the stored automation with `id` against a fresh reload of
/// the store, so edits the user made while an automation was running (which can
/// take minutes) are not clobbered by a stale snapshot. Returns `false` if the
/// automation no longer exists (deleted mid-run) — the update is dropped.
pub async fn update_automation<F: FnOnce(&mut ScheduledAutomation)>(
    app: &AppHandle,
    id: &str,
    mutate: F,
) -> Result<bool, String> {
    with_automations(app, |automations| {
        match automations.iter_mut().find(|a| a.id == id) {
            Some(automation) => {
                mutate(automation);
                Ok((true, true))
            }
            None => Ok((false, false)),
        }
    })
    .await
}

/// Starts the background scheduler loop. Called once from application setup.
pub fn start_scheduler(app_handle: AppHandle) {
    tauri::async_runtime::spawn(async move {
        info!(
            "Scheduled automations service started (tick every {}s)",
            TICK_INTERVAL_SECS
        );
        loop {
            tokio::time::sleep(Duration::from_secs(TICK_INTERVAL_SECS)).await;
            if let Err(e) = tick(&app_handle).await {
                warn!("Scheduled automations tick failed: {}", e);
            }
        }
    });
}

/// One scheduler cycle: fire every enabled, due automation and reschedule it.
///
/// Automations fire sequentially on purpose — the orchestrator processes one
/// query at a time, and concurrent submissions would race on shared agent state.
///
/// Each result is written back via `update_automation` (reload + merge by id)
/// rather than bulk-saving the snapshot taken at the top of the tick: firing an
/// automation awaits the full agent run, and the user may edit or delete
/// automations in Settings while it is in flight.
async fn tick(app: &AppHandle) -> Result<(), String> {
    let automations = load_automations(app)?;
    let now = now_secs();

    for automation in automations {
        if !automation.enabled {
            continue;
        }

        match automation.next_run_at {
            None => {
                // Heal automations persisted without a next run (e.g. older versions)
                match compute_next_run(&automation.cron) {
                    Ok(t) => {
                        update_automation(app, &automation.id, |a| a.next_run_at = Some(t)).await?;
                    }
                    Err(e) => {
                        warn!(
                            "Disabling automation '{}' with unparseable cron: {}",
                            automation.name, e
                        );
                        update_automation(app, &automation.id, |a| {
                            a.enabled = false;
                            a.last_result = Some(format!("error: {}", e));
                        })
                        .await?;
                    }
                }
            }
            Some(next_run) if next_run <= now => {
                // Claim the id so a concurrent "Run now" (or a fire still in
                // flight) cannot double-fire this automation; skip this cycle
                // if it is already running.
                let Some(_claim) = try_claim_firing(&automation.id) else {
                    continue;
                };
                let mut fired = automation.clone();
                fire_automation(app, &mut fired).await;
                // Missed occurrences (e.g. the app was closed) collapse into the
                // single run above; reschedule from now rather than replaying
                // each missed slot.
                let next = compute_next_run(&fired.cron).ok();
                update_automation(app, &automation.id, |a| {
                    a.last_run_at = fired.last_run_at;
                    a.last_result = fired.last_result;
                    a.next_run_at = next;
                })
                .await?;
            }
            Some(_) => {}
        }
    }

    Ok(())
}

/// Fires a single automation: submits its query to the orchestrator, records the
/// outcome, emits a frontend event, and (optionally) shows a system notification.
pub async fn fire_automation(app: &AppHandle, automation: &mut ScheduledAutomation) {
    info!(
        "Firing scheduled automation '{}' ({})",
        automation.name, automation.id
    );

    let state = app.state::<crate::state::AppState>();
    let result = crate::anthropic::submit_query(automation.query.clone(), state, app.clone()).await;

    automation.last_run_at = Some(now_secs());
    automation.last_result = Some(match &result {
        Ok(()) => "submitted".to_string(),
        Err(e) => format!("error: {}", e),
    });

    let payload = json!({
        "id": automation.id,
        "name": automation.name,
        "query": automation.query,
        "success": result.is_ok(),
        "error": result.as_ref().err(),
    });
    if let Err(e) = app.emit(events::scheduler::AUTOMATION_FIRED, payload) {
        warn!("Failed to emit automation-fired event: {}", e);
    }

    if automation.notify {
        let body = match &result {
            Ok(()) => format!("Running: {}", automation.query),
            Err(e) => format!("Failed to start: {}", e),
        };
        let notification = app
            .notification()
            .builder()
            .title(format!("Juno automation: {}", automation.name))
            .body(body.chars().take(200).collect::<String>())
            .show();
        if let Err(e) = notification {
            warn!("Failed to show automation notification: {}", e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_five_field_cron() {
        assert_eq!(normalize_cron("0 9 * * MON"), "0 0 9 * * MON");
        assert_eq!(normalize_cron("  0 9 * * MON  "), "0 0 9 * * MON");
    }

    #[test]
    fn keeps_six_field_cron() {
        assert_eq!(normalize_cron("0 0 9 * * MON"), "0 0 9 * * MON");
    }

    #[test]
    fn computes_next_run_in_future() {
        let next = compute_next_run("0 9 * * MON").expect("valid cron");
        assert!(next > now_secs());
    }

    #[test]
    fn rejects_invalid_cron() {
        assert!(compute_next_run("not a cron").is_err());
        assert!(compute_next_run("99 99 99 * *").is_err());
    }

    #[test]
    fn preview_returns_requested_count() {
        let runs = preview_next_runs("0 * * * *", 3).expect("valid cron");
        assert_eq!(runs.len(), 3);
        assert!(runs[0] < runs[1] && runs[1] < runs[2]);
    }

    #[test]
    fn rejects_subminute_intervals() {
        assert!(validate_cron_interval("* * * * * *").is_err());
        assert!(validate_cron_interval("*/10 * * * * *").is_err());
        // Every minute is exactly MIN_INTERVAL_SECS — allowed
        assert!(validate_cron_interval("0 * * * * *").is_ok());
        assert!(validate_cron_interval("0 9 * * MON").is_ok());
    }

    #[test]
    fn claim_prevents_concurrent_fire_and_releases_on_drop() {
        let claim = try_claim_firing("test-claim-id");
        assert!(claim.is_some());
        assert!(try_claim_firing("test-claim-id").is_none());
        drop(claim);
        assert!(try_claim_firing("test-claim-id").is_some());
    }
}
