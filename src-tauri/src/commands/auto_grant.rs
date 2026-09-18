//! Post-Accessibility auto-grant: Juno finishes its own permission setup.
//!
//! Once the user flips Accessibility on (the one toggle macOS requires a human
//! to perform), Juno can drive System Settings itself. For each remaining
//! automatable permission (Screen Recording, Input Monitoring) we:
//!   1. make the native request (`CGRequestScreenCaptureAccess` /
//!      `IOHIDRequestAccess`) so macOS creates Juno's row in the pane at all,
//!   2. open the exact Settings pane via deep link (no osascript, no admin),
//!   3. wait for the System Settings window to be reachable over AX,
//!   4. walk the AX tree collecting every switch that names Juno (a stale row
//!      from an older build can sit next to the real one), press the OFF ones
//!      one at a time — AXPress first (no cursor movement), then AXValue, then
//!      an element click as fallbacks,
//!   5. dismiss the "quit and reopen" sheet with "Later" so onboarding keeps
//!      running, and
//!   6. confirm through the native TCC check after each press before moving on.
//!
//! Microphone is deliberately NOT automated: TCC consent dialogs ignore
//! synthetic input by design, so the native one-click prompt is the honest
//! path. Accessibility itself can't be here either — flipping it on requires
//! admin authentication, and it is the very capability this flow runs on.
//!
//! Progress streams to the frontend via `permissions-auto-grant-progress`;
//! grants surface through the existing 1Hz `permission-granted` poller, which
//! stays the single source of truth for checklist row state.

use crate::constants::events;
use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};
use tokio::time::sleep;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

/// Permissions this flow is allowed to drive. Order matters: Screen Recording
/// is required, Input Monitoring is optional, so we secure the required one
/// first in case the run is cancelled partway.
const AUTOMATABLE: [&str; 2] = ["screen_recording", "input_monitoring"];

// ── System Settings window discovery ─────────────────────────────────────────
// Moved here from the retired chat-based onboarding guidance module — the
// auto-grant flow is now the only consumer.

#[cfg(target_os = "macos")]
fn find_system_settings_window_bounds() -> Option<(f64, f64, f64, f64)> {
    use computer_use_ai_sdk::Desktop;

    // Background apps + don't activate — the AX query must not steal focus.
    let desktop = match Desktop::new(true, false) {
        Ok(d) => d,
        Err(e) => {
            debug!("[auto-grant] Desktop init failed: {}", e);
            return None;
        }
    };

    // System Settings is named "System Settings" on macOS Ventura+ and "System Preferences" on Monterey.
    for name in ["System Settings", "System Preferences"] {
        if let Ok(app) = desktop.application(name) {
            // First child of the application is typically its main window.
            if let Ok(children) = app.children() {
                for child in &children {
                    if let Ok(b) = child.bounds() {
                        // Filter out zero-area placeholders
                        if b.2 > 100.0 && b.3 > 100.0 {
                            return Some(b);
                        }
                    }
                }
            }
            // No usable window child — try app bounds directly
            if let Ok(b) = app.bounds() {
                if b.2 > 100.0 && b.3 > 100.0 {
                    return Some(b);
                }
            }
        }
    }
    None
}

#[cfg(not(target_os = "macos"))]
fn find_system_settings_window_bounds() -> Option<(f64, f64, f64, f64)> {
    None
}

/// How long any single accessibility step may take before the run gives up.
const AX_STEP_LIMIT: Duration = Duration::from_secs(3);
/// The native permission requests raise a system alert, so they get longer.
const NATIVE_REQUEST_LIMIT: Duration = Duration::from_secs(15);
/// A whole run, however many permissions it is working through.
const RUN_LIMIT: Duration = Duration::from_secs(90);

/// Why a step stopped early.
#[derive(Debug)]
enum StepHalt {
    Cancelled,
    TimedOut,
}

/// Run one blocking accessibility step without letting it take setup with it.
///
/// Every step here talks synchronously to System Settings over the
/// accessibility API, and System Settings does not answer while it is showing
/// a modal sheet, which this very flow causes it to do. Awaiting one of these
/// unguarded is what froze setup: Stop could not interrupt it because
/// cancellation was only checked between steps, the run guard never dropped,
/// and the flag stayed "already running" for the life of the process.
///
/// A blocking task cannot be killed, so a timeout abandons the thread rather
/// than stopping it. That is the point. The run ends, the guard drops, the
/// setup window comes back, and the orphaned thread finishes into nothing.
async fn guarded<T, F>(
    token: &CancellationToken,
    limit: Duration,
    work: F,
) -> Result<Result<T, tokio::task::JoinError>, StepHalt>
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    tokio::select! {
        biased;
        _ = token.cancelled() => Err(StepHalt::Cancelled),
        outcome = tokio::time::timeout(limit, tokio::task::spawn_blocking(work)) => {
            outcome.map_err(|_| StepHalt::TimedOut)
        }
    }
}

/// Wait up to `timeout_ms` for the System Settings window to be findable via AX.
/// Polls every 150ms.
#[cfg(target_os = "macos")]
async fn wait_for_settings_window(
    timeout_ms: u64,
    token: &CancellationToken,
) -> Option<(f64, f64, f64, f64)> {
    let deadline = Instant::now() + Duration::from_millis(timeout_ms);
    loop {
        // Run the AX call on a blocking thread to avoid stalling the async
        // runtime, and give up on it rather than waiting forever.
        let bounds = guarded(token, AX_STEP_LIMIT, find_system_settings_window_bounds)
            .await
            .ok()
            .and_then(|joined| joined.ok())
            .flatten();
        if let Some(b) = bounds {
            return Some(b);
        }
        if Instant::now() >= deadline {
            return None;
        }
        sleep(Duration::from_millis(150)).await;
    }
}

#[cfg(not(target_os = "macos"))]
async fn wait_for_settings_window(_timeout_ms: u64) -> Option<(f64, f64, f64, f64)> {
    None
}

static AUTO_GRANT_RUNNING: LazyLock<AtomicBool> = LazyLock::new(|| AtomicBool::new(false));
static AUTO_GRANT_CANCEL: LazyLock<Mutex<Option<CancellationToken>>> =
    LazyLock::new(|| Mutex::new(None));

/// One progress tick of the auto-grant run. `permission_type` is `None` for
/// run-level stages (`done`, `cancelled`).
#[derive(Debug, Clone, Serialize)]
pub struct AutoGrantProgress {
    pub permission_type: Option<String>,
    /// `opening_settings` | `toggling` | `confirming` | `granted` | `failed`
    /// | `cancelled` | `done`
    pub stage: String,
    pub message: Option<String>,
}

fn emit_progress(
    app: &AppHandle,
    permission_type: Option<&str>,
    stage: &str,
    message: Option<String>,
) {
    let payload = AutoGrantProgress {
        permission_type: permission_type.map(str::to_string),
        stage: stage.to_string(),
        message,
    };
    if let Err(e) = app.emit(events::permissions::AUTO_GRANT_PROGRESS, payload) {
        warn!("[auto-grant] Failed to emit progress event: {}", e);
    }
}

/// Keep only the permissions this flow may drive, preserving AUTOMATABLE order.
fn filter_automatable(permissions: &[String]) -> Vec<String> {
    AUTOMATABLE
        .iter()
        .filter(|a| permissions.iter().any(|p| p == *a))
        .map(|a| a.to_string())
        .collect()
}

/// Kick off the auto-grant run in the background and return immediately.
/// The frontend follows along via `permissions-auto-grant-progress` and the
/// existing `permission-granted` poller events.
#[tauri::command]
pub async fn auto_grant_permissions(
    app: AppHandle,
    permissions: Vec<String>,
) -> Result<(), String> {
    let targets = filter_automatable(&permissions);
    if targets.is_empty() {
        return Err("No automatable permissions requested".to_string());
    }

    if AUTO_GRANT_RUNNING.swap(true, Ordering::SeqCst) {
        return Err("Auto-grant is already running".to_string());
    }

    let token = CancellationToken::new();
    match AUTO_GRANT_CANCEL.lock() {
        Ok(mut guard) => *guard = Some(token.clone()),
        Err(_) => {
            AUTO_GRANT_RUNNING.store(false, Ordering::SeqCst);
            return Err("Auto-grant state lock poisoned".to_string());
        }
    }

    info!("[auto-grant] Starting for {:?}", targets);
    tauri::async_runtime::spawn(async move {
        // RAII guard: the running flag and stored token reset even if the AX
        // machinery panics, so the feature can never wedge in "already
        // running" for the rest of the app's lifetime.
        let _reset_on_exit = RunGuard;
        // A whole-run deadline on top of the per-step ones. Belt and braces:
        // whatever happens inside, the guard drops and setup is usable again.
        let app_for_timeout = app.clone();
        if tokio::time::timeout(RUN_LIMIT, run_auto_grant(app, targets, token))
            .await
            .is_err()
        {
            warn!("[auto-grant] Run exceeded {:?}; giving up", RUN_LIMIT);
            emit_progress(
                &app_for_timeout,
                None,
                "failed",
                Some(
                    "Setting these up took too long. You can switch them on yourself.".to_string(),
                ),
            );
            refocus_onboarding_window(&app_for_timeout);
            // Every run ends with a terminal event, always. Without this the
            // checklist stays in its "running" state for good, which greys out
            // every row and leaves a spinner that never stops.
            emit_progress(&app_for_timeout, None, "done", None);
        }
    });

    Ok(())
}

/// Resets the auto-grant statics on drop — including on unwind.
struct RunGuard;

impl Drop for RunGuard {
    fn drop(&mut self) {
        AUTO_GRANT_RUNNING.store(false, Ordering::SeqCst);
        if let Ok(mut guard) = AUTO_GRANT_CANCEL.lock() {
            *guard = None;
        }
    }
}

/// Cancel a running auto-grant. Safe to call when nothing is running.
#[tauri::command]
pub async fn cancel_auto_grant() -> Result<(), String> {
    if let Ok(guard) = AUTO_GRANT_CANCEL.lock() {
        if let Some(token) = guard.as_ref() {
            info!("[auto-grant] Cancellation requested");
            token.cancel();
        }
    }
    Ok(())
}

async fn run_auto_grant(app: AppHandle, targets: Vec<String>, token: CancellationToken) {
    for perm in &targets {
        if token.is_cancelled() {
            emit_progress(&app, None, "cancelled", None);
            refocus_onboarding_window(&app);
            return;
        }
        match auto_grant_one(&app, perm, &token).await {
            Ok(true) => {
                info!("[auto-grant] {} granted", perm);
                emit_progress(&app, Some(perm), "granted", None);
            }
            Ok(false) => {
                // Cancelled mid-permission; the loop guard above handles emit.
                emit_progress(&app, None, "cancelled", None);
                refocus_onboarding_window(&app);
                return;
            }
            Err(e) => {
                // Per-permission failure is not fatal to the run — the frontend
                // falls that row back to the manual guided flow.
                warn!("[auto-grant] {} failed: {}", perm, e);
                emit_progress(&app, Some(perm), "failed", Some(e));
            }
        }
    }
    refocus_onboarding_window(&app);
    // A Stop click can land while the final permission is confirming; honor it
    // in the terminal event so analytics record what the user actually did.
    if token.is_cancelled() {
        emit_progress(&app, None, "cancelled", None);
    } else {
        emit_progress(&app, None, "done", None);
    }
}

/// Drive one permission end to end. Returns Ok(true) on confirmed grant,
/// Ok(false) on cancellation, Err on failure (caller falls back to manual).
async fn auto_grant_one(
    app: &AppHandle,
    perm: &str,
    token: &CancellationToken,
) -> Result<bool, String> {
    // Skip work the user has already done (or a prior run finished).
    if is_granted(app, perm).await.unwrap_or(false) {
        return Ok(true);
    }

    emit_progress(app, Some(perm), "opening_settings", None);

    // Register Juno's row BEFORE opening the pane. macOS only lists an app
    // under Screen Recording once it has called CGRequestScreenCaptureAccess,
    // and under Input Monitoring once it has called IOHIDRequestAccess (or
    // created an event tap). A fresh install that skips this step has no row
    // at all, and the AX walk then either finds nothing or, worse, finds a
    // stale "Juno" row left behind by an older build, already on, and reports
    // success for a grant that never happened (seen on hardware, 0.7.0). The
    // native call may also raise a system alert; that is fine, the walk
    // targets System Settings, a different process.
    {
        // This raises the native alert, which is not always-on-top. The bar is,
        // so it has to stand down or the prompt arrives behind it.
        crate::commands::permissions::step_aside_for_prompt(app);
        let p = perm.to_string();
        let registered = guarded(token, NATIVE_REQUEST_LIMIT, move || {
            register_permission_row(&p)
        })
        .await
        .map_err(|halt| format!("Register-row step stopped: {:?}", halt))?
        .map_err(|e| format!("Register-row task failed: {}", e))?;
        debug!(
            "[auto-grant] {} native request before pane open → granted={}",
            perm, registered
        );
        if registered {
            // The system alert itself granted it (or a prior grant landed).
            crate::commands::permissions::invalidate_permissions_cache();
            if is_granted(app, perm).await.unwrap_or(false) {
                return Ok(true);
            }
        }
    }

    // The previous permission's flip raises a "quit and reopen" sheet, and
    // System Settings answers no accessibility call while one is up. If the
    // dismissal after that flip did not land, every step below times out and
    // this permission fails for a reason that has nothing to do with it. That
    // is the likeliest explanation for Screen Recording succeeding and Input
    // Monitoring failing immediately after it.
    let cleared = guarded(token, AX_STEP_LIMIT, dismiss_quit_reopen_sheet)
        .await
        .ok()
        .and_then(|joined| joined.ok())
        .unwrap_or(false);
    if cleared {
        debug!(
            "[auto-grant] cleared a leftover sheet before starting {}",
            perm
        );
    }

    {
        let p = perm.to_string();
        guarded(token, AX_STEP_LIMIT, move || open_settings_pane(&p))
            .await
            .map_err(|halt| format!("Settings-open step stopped: {:?}", halt))?
            .map_err(|e| format!("Settings-open task failed: {}", e))??;
    }

    if wait_for_settings_window(4000, token).await.is_none() {
        return Err("System Settings window did not appear".to_string());
    }
    // Give the pane's SwiftUI content a moment to populate its AX tree.
    sleep(Duration::from_millis(600)).await;

    emit_progress(app, Some(perm), "toggling", None);
    // The AX walk matches on the app's exact display name — never a substring
    // — so another app's switch can't be flipped by accident.
    let app_name = app.package_info().name.clone();
    let mut toggles: Vec<AppToggle> = Vec::new();
    for attempt in 1..=3u8 {
        if token.is_cancelled() {
            return Ok(false);
        }
        let name = app_name.clone();
        let walked = match guarded(token, AX_STEP_LIMIT, move || find_app_toggles(&name)).await {
            Ok(joined) => joined,
            // Timed out walking the tree, almost certainly behind a modal
            // sheet. Stop the run rather than stack up orphaned threads.
            Err(halt) => return Err(format!("Reading the Settings pane stopped: {:?}", halt)),
        };
        match walked {
            Ok(Ok(found)) if !found.is_empty() => {
                info!(
                    "[auto-grant] {} attempt {}: {} '{}' switch(es) in the pane: {:?}",
                    perm,
                    attempt,
                    found.len(),
                    app_name,
                    found
                        .iter()
                        .map(|t| (t.label.as_str(), t.value.as_deref()))
                        .collect::<Vec<_>>()
                );
                toggles = found;
                break;
            }
            Ok(Ok(_)) => {
                debug!(
                    "[auto-grant] {} attempt {}: no '{}' switch yet",
                    perm, attempt, app_name
                );
                sleep(Duration::from_millis(800)).await;
            }
            Ok(Err(e)) => {
                debug!("[auto-grant] {} attempt {} failed: {}", perm, attempt, e);
                sleep(Duration::from_millis(800)).await;
            }
            Err(e) => return Err(format!("Toggle task failed: {}", e)),
        }
    }
    if toggles.is_empty() {
        return Err(format!(
            "Couldn't find the {} switch in System Settings",
            app_name
        ));
    }

    // OFF switches first: a stale row from an older build can sit there
    // already on, and pressing an ON switch would revoke, not grant.
    let rows: Vec<(String, Option<String>)> = toggles
        .iter()
        .map(|t| (t.label.clone(), t.value.clone()))
        .collect();
    let order = pick_toggle_candidates(&rows, &app_name);
    let mut pressed = 0usize;
    for idx in order {
        if token.is_cancelled() {
            return Ok(false);
        }
        let Some(toggle) = toggles.get(idx) else {
            continue;
        };
        if toggle_is_on(toggle.value.as_deref()) {
            // AlreadyOn is informational only. It never means "granted": if TCC
            // agreed we would not be here. Leave it alone and try the next one.
            info!(
                "[auto-grant] {} candidate {} → {:?} (skipping: pressing would switch it off)",
                perm,
                idx,
                FlipOutcome::AlreadyOn
            );
            continue;
        }

        let element = toggle.element.clone();
        let pressed_result =
            match guarded(token, AX_STEP_LIMIT, move || press_toggle(&element)).await {
                Ok(joined) => joined,
                Err(halt) => return Err(format!("Flipping the switch stopped: {:?}", halt)),
            };
        match pressed_result {
            Ok(Ok(outcome)) => {
                info!("[auto-grant] {} candidate {} → {:?}", perm, idx, outcome);
                pressed += 1;
            }
            Ok(Err(e)) => {
                warn!(
                    "[auto-grant] {} candidate {} could not be pressed: {}",
                    perm, idx, e
                );
                continue;
            }
            Err(e) => return Err(format!("Toggle task failed: {}", e)),
        }

        // macOS follows the flip with a "quit and reopen" sheet. Press "Later"
        // so onboarding keeps running — Juno offers its own relaunch when
        // setup ends.
        sleep(Duration::from_millis(700)).await;
        let dismissed = guarded(token, AX_STEP_LIMIT, dismiss_quit_reopen_sheet)
            .await
            .ok()
            .and_then(|joined| joined.ok())
            .unwrap_or(false);
        debug!(
            "[auto-grant] quit-and-reopen sheet dismissed: {}",
            dismissed
        );

        emit_progress(app, Some(perm), "confirming", None);
        // Confirm through TCC itself, not the UI — the toggle can render
        // flipped before the grant actually lands. A short window per
        // candidate: if this switch was the wrong row, move on to the next.
        match confirm_granted(app, perm, token, Duration::from_secs(5)).await {
            Confirm::Granted => return Ok(true),
            Confirm::Cancelled => return Ok(false),
            Confirm::Timeout => debug!(
                "[auto-grant] {} candidate {} flipped but TCC still says denied",
                perm, idx
            ),
        }
    }

    if pressed == 0 {
        return Err(format!(
            "Every {} switch in the pane was already on, but macOS still reports the permission denied — the row for this build is missing",
            app_name
        ));
    }
    // The switch is on. macOS simply will not tell this process about it until
    // Juno restarts, which is the normal case for Input Monitoring rather than
    // a failure. Record it so setup can offer the restart, and say so plainly.
    crate::commands::permissions::note_relaunch_pending(perm);
    Err(format!(
        "Switched {} {} row(s) on. macOS will not report it to Juno until Juno restarts.",
        pressed, app_name
    ))
}

enum Confirm {
    Granted,
    Timeout,
    Cancelled,
}

/// Poll TCC (never the UI) for up to `window` after a press.
async fn confirm_granted(
    app: &AppHandle,
    perm: &str,
    token: &CancellationToken,
    window: Duration,
) -> Confirm {
    let deadline = Instant::now() + window;
    loop {
        if token.is_cancelled() {
            return Confirm::Cancelled;
        }
        crate::commands::permissions::invalidate_permissions_cache();
        if is_granted(app, perm).await.unwrap_or(false) {
            return Confirm::Granted;
        }
        if Instant::now() >= deadline {
            return Confirm::Timeout;
        }
        sleep(Duration::from_millis(500)).await;
    }
}

/// AXValue "1" means the switch renders on.
fn toggle_is_on(value: Option<&str>) -> bool {
    value == Some("1")
}

/// Strip whitespace and a trailing ".app" so "Juno", " juno " and "Juno.app"
/// all name the same app. LaunchServices labels a row "Juno.app" when it
/// holds a stale registration for the bundle.
fn normalize_app_label(label: &str) -> String {
    let trimmed = label.trim();
    let lower = trimmed.to_lowercase();
    match lower.strip_suffix(".app") {
        Some(stem) => stem.trim_end().to_string(),
        None => lower,
    }
}

/// Exact-name match, case-insensitive, trimmed, ".app" suffix tolerated on
/// either side. Never a substring match — "Junosuite" must not count.
fn label_names_app(label: &str, app_name: &str) -> bool {
    let want = normalize_app_label(app_name);
    !want.is_empty() && normalize_app_label(label) == want
}

/// Given the `(label, AXValue)` of every toggle found in the pane, return the
/// indices that belong to this app, OFF switches first (in pane order), then
/// ON ones. Pure so the selection rules are unit-tested without AX.
pub(crate) fn pick_toggle_candidates(
    rows: &[(String, Option<String>)],
    app_name: &str,
) -> Vec<usize> {
    let matching: Vec<usize> = rows
        .iter()
        .enumerate()
        .filter(|(_, (label, _))| label_names_app(label, app_name))
        .map(|(i, _)| i)
        .collect();
    let (on, off): (Vec<usize>, Vec<usize>) = matching
        .into_iter()
        .partition(|&i| toggle_is_on(rows[i].1.as_deref()));
    off.into_iter().chain(on).collect()
}

async fn is_granted(app: &AppHandle, perm: &str) -> Result<bool, String> {
    let state = crate::commands::permissions::check_permissions_status_native(app.clone()).await?;
    Ok(match perm {
        "accessibility" => state.accessibility.granted,
        "screen_recording" => state.screen_recording.granted,
        "microphone" => state.microphone.granted,
        "input_monitoring" => state.input_monitoring.granted,
        _ => false,
    })
}

/// Bring the onboarding window back to front after driving System Settings.
fn refocus_onboarding_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(crate::constants::window_labels::ONBOARDING) {
        if let Err(e) = window.set_focus() {
            debug!("[auto-grant] Could not refocus onboarding window: {}", e);
        }
    }
}

// ── macOS AX mechanics ────────────────────────────────────────────────────────

#[derive(Debug)]
#[allow(dead_code)] // variants are informational (logged); not all constructed on non-macOS
enum FlipOutcome {
    /// The switch already rendered on. Never a success signal on its own.
    AlreadyOn,
    Pressed,
    ValueSet,
    Clicked,
}

/// One switch in the pane that carries this app's name, with the label and
/// AXValue snapshot the selection logic runs on.
struct AppToggle {
    label: String,
    value: Option<String>,
    element: computer_use_ai_sdk::UIElement,
}

/// Make macOS create this app's row in the pane for `perm` (see the comment
/// at the call site). Returns whether the permission is granted right after
/// the request. Accessibility and Microphone are never registered here.
#[cfg(target_os = "macos")]
fn register_permission_row(perm: &str) -> bool {
    match perm {
        "screen_recording" => {
            computer_use_ai_sdk::platforms::macos::permissions::request_screen_recording_permission(
            )
        }
        "input_monitoring" => crate::platform::input_monitoring::request_input_monitoring_access(),
        _ => false,
    }
}

#[cfg(not(target_os = "macos"))]
fn register_permission_row(_perm: &str) -> bool {
    false
}

#[cfg(target_os = "macos")]
fn open_settings_pane(perm: &str) -> Result<(), String> {
    computer_use_ai_sdk::platforms::macos::permissions::open_system_settings_for_permission(perm)
}

#[cfg(not(target_os = "macos"))]
fn open_settings_pane(_perm: &str) -> Result<(), String> {
    Err("Auto-grant is only available on macOS".to_string())
}

/// Collect EVERY toggle in the frontmost System Settings pane whose row names
/// this app, in pane order. The pane can hold more than one: a fresh install
/// next to a stale row left by an older build (labelled "Juno" or "Juno.app"),
/// and the stale one is often already on. Returning the first match let the
/// walker pick that stale row and call it done (hardware run, 0.7.0), so the
/// caller now decides which switch to press from the full list.
///
/// `app_name` is the app's exact display name; matching is exact
/// (case-insensitive, ".app" tolerated) — a substring match could press a
/// different app's switch, which would be a consent violation.
#[cfg(target_os = "macos")]
fn find_app_toggles(app_name: &str) -> Result<Vec<AppToggle>, String> {
    use computer_use_ai_sdk::{Desktop, UIElement};

    // Background apps + don't activate — same as the guidance flow, the AX
    // walk must not steal focus from what the user is looking at.
    let desktop = Desktop::new(true, false).map_err(|e| format!("AX engine init failed: {}", e))?;
    let settings = ["System Settings", "System Preferences"]
        .into_iter()
        .find_map(|n| desktop.application(n).ok())
        .ok_or_else(|| "System Settings not reachable over AX".to_string())?;

    fn is_toggle_role(role: &str) -> bool {
        let r = role.to_lowercase();
        r.contains("checkbox") || r.contains("switch")
    }

    /// The text on this element that names the app, if any.
    fn app_name_on(elem: &UIElement, app_name: &str) -> Option<String> {
        let attrs = elem.attributes();
        [attrs.label, attrs.value]
            .into_iter()
            .flatten()
            .find(|s| label_names_app(s, app_name))
    }

    /// Two AX handles to the same switch hash alike (the SDK's stable id
    /// ignores position), so identity is the on-screen rect instead: two
    /// distinct switches never share one. Pressing a duplicate would flip the
    /// row straight back off.
    fn identity(elem: &UIElement) -> String {
        match elem.bounds() {
            Ok((x, y, w, h)) => format!(
                "{}:{}:{}:{}",
                x.round() as i64,
                y.round() as i64,
                w.round() as i64,
                h.round() as i64
            ),
            Err(_) => elem.id().unwrap_or_default(),
        }
    }

    /// Depth-limited DFS collecting the toggle belonging to every row that
    /// names this app. On modern System Settings the switch itself carries the
    /// app name as its AX label, so the direct match usually hits; the sibling
    /// scan covers older layouts where a text cell carries the name and the
    /// switch sits beside it.
    fn walk(
        elem: &UIElement,
        app_name: &str,
        depth: usize,
        max_depth: usize,
        out: &mut Vec<AppToggle>,
        seen: &mut std::collections::HashSet<String>,
    ) {
        if depth > max_depth {
            return;
        }
        if let Some(label) = app_name_on(elem, app_name) {
            let toggle = if is_toggle_role(&elem.role()) {
                Some(elem.clone())
            } else if let Ok(Some(parent)) = elem.parent() {
                // One level further up covers row → cell → text layouts.
                toggle_among_children(&parent).or_else(|| {
                    parent
                        .parent()
                        .ok()
                        .flatten()
                        .and_then(|gp| toggle_among_children(&gp))
                })
            } else {
                None
            };
            if let Some(toggle) = toggle {
                if seen.insert(identity(&toggle)) {
                    let value = toggle.attributes().value;
                    out.push(AppToggle {
                        label,
                        value,
                        element: toggle,
                    });
                }
            }
        }
        if let Ok(children) = elem.children() {
            for child in children {
                walk(&child, app_name, depth + 1, max_depth, out, seen);
            }
        }
    }

    fn toggle_among_children(parent: &UIElement) -> Option<UIElement> {
        let children = parent.children().ok()?;
        for child in &children {
            if is_toggle_role(&child.role()) {
                return Some(child.clone());
            }
            if let Ok(grandchildren) = child.children() {
                for gc in grandchildren {
                    if is_toggle_role(&gc.role()) {
                        return Some(gc);
                    }
                }
            }
        }
        None
    }

    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    walk(&settings, app_name, 0, 14, &mut out, &mut seen);
    Ok(out)
}

#[cfg(not(target_os = "macos"))]
fn find_app_toggles(_app_name: &str) -> Result<Vec<AppToggle>, String> {
    Err("Auto-grant is only available on macOS".to_string())
}

/// Switch one OFF toggle on. Prefers AXPress (no cursor movement, no focus
/// theft), falling back to setting AXValue, then to an element click. The
/// caller has already ruled out switches that render on.
#[cfg(target_os = "macos")]
fn press_toggle(toggle: &computer_use_ai_sdk::UIElement) -> Result<FlipOutcome, String> {
    if toggle.perform_action("AXPress").is_ok() {
        return Ok(FlipOutcome::Pressed);
    }
    if toggle.set_value("1").is_ok() {
        return Ok(FlipOutcome::ValueSet);
    }
    toggle
        .click()
        .map_err(|e| format!("AXPress, AXValue, and click all failed (last: {})", e))?;
    Ok(FlipOutcome::Clicked)
}

#[cfg(not(target_os = "macos"))]
fn press_toggle(_toggle: &computer_use_ai_sdk::UIElement) -> Result<FlipOutcome, String> {
    Err("Auto-grant is only available on macOS".to_string())
}

/// Best-effort dismissal of the "quit and reopen" sheet System Settings shows
/// after a Screen Recording / Input Monitoring flip. English-labeled button
/// only — a localized system quietly leaves the sheet for the user, which is
/// harmless.
#[cfg(target_os = "macos")]
fn dismiss_quit_reopen_sheet() -> bool {
    use computer_use_ai_sdk::{Desktop, UIElement};

    let Ok(desktop) = Desktop::new(true, false) else {
        return false;
    };
    let Some(settings) = ["System Settings", "System Preferences"]
        .into_iter()
        .find_map(|n| desktop.application(n).ok())
    else {
        return false;
    };

    fn walk(elem: &UIElement, depth: usize) -> bool {
        if depth > 10 {
            return false;
        }
        if elem.role().to_lowercase().contains("button")
            && elem.attributes().label.as_deref() == Some("Later")
        {
            return elem.perform_action("AXPress").is_ok();
        }
        if let Ok(children) = elem.children() {
            for child in children {
                if walk(&child, depth + 1) {
                    return true;
                }
            }
        }
        false
    }

    walk(&settings, 0)
}

#[cfg(not(target_os = "macos"))]
fn dismiss_quit_reopen_sheet() -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filter_automatable_keeps_order_and_drops_unknowns() {
        let input = vec![
            "microphone".to_string(),
            "input_monitoring".to_string(),
            "screen_recording".to_string(),
            "accessibility".to_string(),
            "nonsense".to_string(),
        ];
        // Screen Recording (required) always comes first regardless of input order.
        assert_eq!(
            filter_automatable(&input),
            vec![
                "screen_recording".to_string(),
                "input_monitoring".to_string()
            ]
        );
    }

    #[test]
    fn filter_automatable_empty_for_non_automatable() {
        let input = vec!["microphone".to_string(), "accessibility".to_string()];
        assert!(filter_automatable(&input).is_empty());
    }

    // ── pick_toggle_candidates ──────────────────────────────────────────────

    fn row(label: &str, value: Option<&str>) -> (String, Option<String>) {
        (label.to_string(), value.map(str::to_string))
    }

    #[test]
    fn stale_on_row_sorts_after_the_fresh_off_row() {
        // The 0.7.0 hardware layout: two stale rows already on, then the row
        // the fresh install just registered, off.
        let rows = vec![
            row("juno", Some("1")),
            row("Juno.app", Some("1")),
            row("Google Chrome", Some("0")),
            row("Juno", Some("0")),
        ];
        assert_eq!(pick_toggle_candidates(&rows, "Juno"), vec![3, 0, 1]);
    }

    #[test]
    fn off_first_then_on_preserving_pane_order_within_each_group() {
        let rows = vec![
            row("Juno", Some("1")),
            row("Juno", Some("0")),
            row("Juno", None),
            row("Juno", Some("1")),
            row("Juno", Some("0")),
        ];
        // None counts as off (not rendered on).
        assert_eq!(pick_toggle_candidates(&rows, "Juno"), vec![1, 2, 4, 0, 3]);
    }

    #[test]
    fn dot_app_suffix_and_whitespace_and_case_all_match() {
        let rows = vec![
            row("  Juno  ", Some("0")),
            row("JUNO.APP", Some("0")),
            row("juno.app ", Some("0")),
        ];
        assert_eq!(pick_toggle_candidates(&rows, "Juno"), vec![0, 1, 2]);
        // The app name itself may carry ".app" too.
        assert_eq!(pick_toggle_candidates(&rows, "Juno.app"), vec![0, 1, 2]);
    }

    #[test]
    fn substrings_and_lookalikes_never_match() {
        let rows = vec![
            row("Junosuite", Some("0")),
            row("Juno Helper", Some("0")),
            row("MyJuno", Some("0")),
            row("Juno.app.bak", Some("0")),
            row("Jun", Some("0")),
            row("", Some("0")),
        ];
        assert!(pick_toggle_candidates(&rows, "Juno").is_empty());
    }

    #[test]
    fn no_rows_or_empty_app_name_yields_nothing() {
        assert!(pick_toggle_candidates(&[], "Juno").is_empty());
        let rows = vec![row("Juno", Some("0")), row("", Some("0"))];
        assert!(pick_toggle_candidates(&rows, "").is_empty());
        assert!(pick_toggle_candidates(&rows, ".app").is_empty());
    }

    #[test]
    fn only_on_rows_are_still_returned_so_the_caller_can_report_them() {
        let rows = vec![row("Juno", Some("1")), row("Juno.app", Some("1"))];
        assert_eq!(pick_toggle_candidates(&rows, "Juno"), vec![0, 1]);
    }

    #[test]
    fn label_matching_is_exact_after_normalisation() {
        assert!(label_names_app("Juno", "Juno"));
        assert!(label_names_app("juno", "JUNO"));
        assert!(label_names_app("Juno.app", "Juno"));
        assert!(label_names_app("Juno", "Juno.app"));
        assert!(label_names_app(" Juno .app", "Juno"));
        assert!(!label_names_app("Juno2", "Juno"));
        assert!(!label_names_app("Juno", "Jun"));
        assert!(!label_names_app("Ju", "Juno"));
    }

    #[test]
    fn toggle_value_semantics() {
        assert!(toggle_is_on(Some("1")));
        assert!(!toggle_is_on(Some("0")));
        assert!(!toggle_is_on(Some("")));
        assert!(!toggle_is_on(None));
    }
}
