//! Post-Accessibility auto-grant: Juno finishes its own permission setup.
//!
//! Once the user flips Accessibility on (the one toggle macOS requires a human
//! to perform), Juno can drive System Settings itself. For each remaining
//! automatable permission (Screen Recording, Input Monitoring) we:
//!   1. open the exact Settings pane via deep link (no osascript, no admin),
//!   2. wait for the System Settings window to be reachable over AX,
//!   3. walk the AX tree to Juno's own row and flip its toggle — AXPress first
//!      (no cursor movement), then AXValue, then an element click as fallbacks,
//!   4. dismiss the "quit and reopen" sheet with "Later" so onboarding keeps
//!      running, and
//!   5. confirm through the native TCC check before moving on.
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
        run_auto_grant(app, targets, token).await;
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
    {
        let p = perm.to_string();
        tokio::task::spawn_blocking(move || open_settings_pane(&p))
            .await
            .map_err(|e| format!("Settings-open task failed: {}", e))??;
    }

    if crate::commands::onboarding_guidance::wait_for_settings_window(4000)
        .await
        .is_none()
    {
        return Err("System Settings window did not appear".to_string());
    }
    // Give the pane's SwiftUI content a moment to populate its AX tree.
    sleep(Duration::from_millis(600)).await;

    emit_progress(app, Some(perm), "toggling", None);
    // The AX walk matches on the app's exact display name — never a substring
    // — so another app's switch can't be flipped by accident.
    let app_name = app.package_info().name.clone();
    let mut flipped = false;
    for attempt in 1..=3u8 {
        if token.is_cancelled() {
            return Ok(false);
        }
        let name = app_name.clone();
        match tokio::task::spawn_blocking(move || flip_app_toggle(&name)).await {
            Ok(Ok(outcome)) => {
                info!(
                    "[auto-grant] {} toggle attempt {} → {:?}",
                    perm, attempt, outcome
                );
                flipped = true;
                break;
            }
            Ok(Err(e)) => {
                debug!(
                    "[auto-grant] {} toggle attempt {} failed: {}",
                    perm, attempt, e
                );
                sleep(Duration::from_millis(800)).await;
            }
            Err(e) => return Err(format!("Toggle task failed: {}", e)),
        }
    }
    if !flipped {
        return Err("Couldn't find the Juno toggle in System Settings".to_string());
    }

    // macOS follows the flip with a "quit and reopen" sheet. Press "Later" so
    // onboarding keeps running — Juno offers its own relaunch when setup ends.
    sleep(Duration::from_millis(700)).await;
    let dismissed = tokio::task::spawn_blocking(dismiss_quit_reopen_sheet)
        .await
        .unwrap_or(false);
    debug!(
        "[auto-grant] quit-and-reopen sheet dismissed: {}",
        dismissed
    );

    emit_progress(app, Some(perm), "confirming", None);
    // Confirm through TCC itself, not the UI — the toggle can render flipped
    // before the grant actually lands.
    let deadline = Instant::now() + Duration::from_secs(12);
    loop {
        if token.is_cancelled() {
            return Ok(false);
        }
        crate::commands::permissions::invalidate_permissions_cache();
        if is_granted(app, perm).await.unwrap_or(false) {
            return Ok(true);
        }
        if Instant::now() >= deadline {
            return Err(
                "The switch flipped but macOS hasn't registered the grant — it may need a restart"
                    .to_string(),
            );
        }
        sleep(Duration::from_millis(500)).await;
    }
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
    AlreadyOn,
    Pressed,
    ValueSet,
    Clicked,
}

#[cfg(target_os = "macos")]
fn open_settings_pane(perm: &str) -> Result<(), String> {
    computer_use_ai_sdk::platforms::macos::permissions::open_system_settings_for_permission(perm)
}

#[cfg(not(target_os = "macos"))]
fn open_settings_pane(_perm: &str) -> Result<(), String> {
    Err("Auto-grant is only available on macOS".to_string())
}

/// Find this app's row toggle in the frontmost System Settings pane and switch
/// it on. Prefers AXPress on the AXCheckBox/AXSwitch element (no cursor
/// movement, no focus theft), falling back to setting AXValue, then to an
/// element click. `app_name` is the app's exact display name; matching is
/// exact (case-insensitive) — a substring match could press a different app's
/// switch, which would be a consent violation.
#[cfg(target_os = "macos")]
fn flip_app_toggle(app_name: &str) -> Result<FlipOutcome, String> {
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

    fn names_this_app(elem: &UIElement, app_name: &str) -> bool {
        let attrs = elem.attributes();
        let is_exact = |s: &str| s.trim().eq_ignore_ascii_case(app_name);
        attrs.label.as_deref().map(is_exact).unwrap_or(false)
            || attrs.value.as_deref().map(is_exact).unwrap_or(false)
    }

    /// Depth-limited DFS for the toggle belonging to this app's row. On modern
    /// System Settings the switch itself carries the app name as its AX label,
    /// so the direct match usually hits; the sibling scan covers older layouts
    /// where a text cell carries the name and the switch sits beside it.
    fn walk(elem: &UIElement, app_name: &str, depth: usize, max_depth: usize) -> Option<UIElement> {
        if depth > max_depth {
            return None;
        }
        if names_this_app(elem, app_name) {
            if is_toggle_role(&elem.role()) {
                return Some(elem.clone());
            }
            if let Ok(Some(parent)) = elem.parent() {
                if let Some(t) = toggle_among_children(&parent) {
                    return Some(t);
                }
                // One level further up: row → cell → text layouts.
                if let Ok(Some(grandparent)) = parent.parent() {
                    if let Some(t) = toggle_among_children(&grandparent) {
                        return Some(t);
                    }
                }
            }
        }
        if let Ok(children) = elem.children() {
            for child in children {
                if let Some(t) = walk(&child, app_name, depth + 1, max_depth) {
                    return Some(t);
                }
            }
        }
        None
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

    let toggle = walk(&settings, app_name, 0, 14)
        .ok_or_else(|| format!("No {} toggle found in the current pane", app_name))?;

    // AXValue "1" means the switch already shows on — pressing again would
    // switch the permission OFF. Report and let the TCC confirmation loop
    // decide whether the grant actually registered.
    if toggle.attributes().value.as_deref() == Some("1") {
        return Ok(FlipOutcome::AlreadyOn);
    }

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
fn flip_app_toggle(_app_name: &str) -> Result<FlipOutcome, String> {
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
}
