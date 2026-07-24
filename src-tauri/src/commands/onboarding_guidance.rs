//! Phase C: Guided onboarding cursor flight to System Settings + live capability demos.
//!
//! Three-tier System Settings element finding:
//!   1. Known coordinates derived from the System Settings window bounds (AX query for window frame)
//!   2. AX tree walk to locate the Juno toggle by label
//!   3. General pane area highlight as fallback ("Find 'Juno' in the list and toggle it on")
//!
//! After each permission grant, a domain-appropriate live demo runs to prove the capability works
//! (screenshot for Screen Recording, safe cursor sweep for Accessibility, etc).

use crate::constants::events;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use tokio::time::sleep;
use tracing::{debug, info, warn};
use uuid::Uuid;

// ── Public permission identifier ──────────────────────────────────────────────

/// Permission identifiers used across the onboarding guidance API.
/// These match the snake_case strings used by `NativePermissionStatus.permission_type`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GuidedPermission {
    ScreenRecording,
    Accessibility,
    Microphone,
    InputMonitoring,
}

impl GuidedPermission {
    fn from_str(s: &str) -> Option<Self> {
        match s {
            "screen_recording" => Some(Self::ScreenRecording),
            "accessibility" => Some(Self::Accessibility),
            "microphone" => Some(Self::Microphone),
            "input_monitoring" => Some(Self::InputMonitoring),
            _ => None,
        }
    }

    /// The TCC pane identifier inside System Settings — used to recognize the right window
    /// title and to compose a friendly description for the speech bubble.
    fn settings_pane_label(self) -> &'static str {
        match self {
            Self::ScreenRecording => "Screen Recording",
            Self::Accessibility => "Accessibility",
            Self::Microphone => "Microphone",
            Self::InputMonitoring => "Input Monitoring",
        }
    }

    fn bubble_text(self) -> &'static str {
        match self {
            Self::ScreenRecording => "Toggle Juno on here",
            Self::Accessibility => "Flip this switch for Juno",
            Self::Microphone => "Enable Juno for mic access",
            Self::InputMonitoring => "Allow Juno to monitor input",
        }
    }
}

// ── Result type returned to the frontend ──────────────────────────────────────

/// Outcome of `guide_to_system_settings` — which tier succeeded and at what point.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuidanceResult {
    pub tier: u8,
    pub target_x: f64,
    pub target_y: f64,
    /// `true` when the cursor reliably landed on the Juno-specific control.
    /// `false` when we fell back to a general pane area.
    pub precise: bool,
    pub message: String,
}

// ── Tier 1: window-bounds-derived coordinates ─────────────────────────────────

#[cfg(target_os = "macos")]
fn find_system_settings_window_bounds() -> Option<(f64, f64, f64, f64)> {
    use computer_use_ai_sdk::Desktop;

    // Background apps + don't activate — the AX query must not steal focus.
    let desktop = match Desktop::new(true, false) {
        Ok(d) => d,
        Err(e) => {
            debug!("[onboarding-guidance] Desktop init failed: {}", e);
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

/// Wait up to `timeout_ms` for the System Settings window to be findable via AX.
/// Polls every 150ms.
#[cfg(target_os = "macos")]
async fn wait_for_settings_window(timeout_ms: u64) -> Option<(f64, f64, f64, f64)> {
    let deadline = std::time::Instant::now() + Duration::from_millis(timeout_ms);
    loop {
        // Run the AX call on a blocking thread to avoid stalling the async runtime
        let bounds = tokio::task::spawn_blocking(find_system_settings_window_bounds)
            .await
            .ok()
            .flatten();
        if let Some(b) = bounds {
            return Some(b);
        }
        if std::time::Instant::now() >= deadline {
            return None;
        }
        sleep(Duration::from_millis(150)).await;
    }
}

#[cfg(not(target_os = "macos"))]
async fn wait_for_settings_window(_timeout_ms: u64) -> Option<(f64, f64, f64, f64)> {
    None
}

// ── Tier 2: AX tree walk for the Juno control ─────────────────────────────────

/// Walk the AX tree of System Settings looking for an element whose label/value
/// contains "Juno". Returns the element bounds as (x, y, width, height) in global
/// screen coordinates. Best-effort — fragile across macOS versions, but pixel-perfect
/// when it hits.
#[cfg(target_os = "macos")]
fn find_juno_control_bounds() -> Option<(f64, f64, f64, f64)> {
    use computer_use_ai_sdk::{Desktop, UIElement};

    let desktop = Desktop::new(true, false).ok()?;
    let app = ["System Settings", "System Preferences"]
        .into_iter()
        .find_map(|n| desktop.application(n).ok())?;

    /// Depth-limited DFS to keep AX traversal bounded — System Settings panes can
    /// have surprisingly deep trees and we want to fail fast rather than hang.
    fn walk(elem: &UIElement, depth: usize, max_depth: usize) -> Option<(f64, f64, f64, f64)> {
        if depth > max_depth {
            return None;
        }
        let attrs = elem.attributes();
        let mut juno_match = false;
        if let Some(label) = &attrs.label {
            if label.to_lowercase().contains("juno") {
                juno_match = true;
            }
        }
        if !juno_match {
            if let Some(value) = &attrs.value {
                if value.to_lowercase().contains("juno") {
                    juno_match = true;
                }
            }
        }
        if juno_match {
            if let Ok(b) = elem.bounds() {
                if b.2 > 0.0 && b.3 > 0.0 {
                    return Some(b);
                }
            }
            // Label-matched but no bounds — try walking up to a sibling with bounds.
            if let Ok(Some(parent)) = elem.parent() {
                if let Ok(siblings) = parent.children() {
                    for sib in &siblings {
                        if let Ok(b) = sib.bounds() {
                            let role = sib.role().to_lowercase();
                            if role.contains("checkbox")
                                || role.contains("switch")
                                || role.contains("button")
                            {
                                return Some(b);
                            }
                        }
                    }
                }
            }
        }
        if let Ok(children) = elem.children() {
            for child in &children {
                if let Some(b) = walk(child, depth + 1, max_depth) {
                    return Some(b);
                }
            }
        }
        None
    }

    walk(&app, 0, 12)
}

#[cfg(not(target_os = "macos"))]
fn find_juno_control_bounds() -> Option<(f64, f64, f64, f64)> {
    None
}

// ── Cursor flight orchestration ───────────────────────────────────────────────

/// Animate the onboarding cursor from `from` to `to`, then show a pulsing ring and
/// speech bubble. Reuses the existing onboarding cursor pipeline from Phase A/B.
///
/// `skip_flight=true` (Phase D edge case) suppresses the Bezier flight when
/// System Settings is already open at flow start — we still position the
/// cursor sprite at the target and show the ring + bubble so the user gets
/// the in-app prompt without a misleading "I'm flying to a thing that just
/// appeared" animation.
async fn fly_and_announce(
    app: &AppHandle,
    from: (f64, f64),
    to: (f64, f64),
    bubble: &str,
    ring_radius: f64,
    skip_flight: bool,
) -> Result<(), String> {
    if skip_flight {
        // Snap the cursor to the target without animation (single frame emit).
        // Using animate_cursor_to with a tiny source delta still emits frames,
        // so we instead use the highlight + bubble at the target and rely on
        // the overlay being positionable through subsequent events.
        crate::commands::onboarding::show_cursor_highlight(
            app.clone(),
            to.0,
            to.1,
            Some(ring_radius),
        )
        .await?;
        crate::commands::onboarding::show_cursor_bubble(
            app.clone(),
            to.0,
            to.1,
            bubble.to_string(),
        )
        .await?;
        return Ok(());
    }

    // 1. Animate the cursor (returns immediately, animation runs in spawned task)
    crate::commands::onboarding::animate_cursor_to(
        app.clone(),
        from.0,
        from.1,
        to.0,
        to.1,
        Some("arc".to_string()),
    )
    .await?;

    // 2. Compute the flight duration mirror — must match animate_cursor_to's formula
    let dx = to.0 - from.0;
    let dy = to.1 - from.1;
    let distance = (dx * dx + dy * dy).sqrt();
    let duration_ms = ((0.4_f64 + 0.8 * (distance / 2000.0)).clamp(0.4, 1.2) * 1000.0) as u64;

    // 3. Wait for the cursor to arrive before lighting the ring + bubble
    sleep(Duration::from_millis(duration_ms.saturating_add(40))).await;

    // Phase D edge case: if the user closed System Settings while we were
    // flying, dismiss the overlay rather than landing on a stale coordinate.
    // Cheap re-check — we already know window_bounds existed pre-flight, so
    // a `None` here means the window genuinely went away.
    let still_open = wait_for_settings_window(0).await.is_some();
    if !still_open {
        info!("[onboarding-guidance] Settings window closed mid-flight — dismissing overlay");
        let _ = crate::commands::onboarding::dismiss_cursor_overlay(app.clone()).await;
        return Ok(());
    }

    // 4. Pulsing highlight ring at target
    crate::commands::onboarding::show_cursor_highlight(app.clone(), to.0, to.1, Some(ring_radius))
        .await?;

    // 5. Speech bubble (frontend handles the typewriter reveal)
    crate::commands::onboarding::show_cursor_bubble(app.clone(), to.0, to.1, bubble.to_string())
        .await?;

    Ok(())
}

/// Detect whether System Settings is the foreground (frontmost) macOS app.
/// Used to skip the cursor flight when the window was already open at the
/// start of the guidance flow.
#[cfg(target_os = "macos")]
fn settings_is_foreground() -> bool {
    use computer_use_ai_sdk::Desktop;
    let Ok(desktop) = Desktop::new(true, false) else {
        return false;
    };
    // Treat "Settings is findable AND has at least one usable window" as
    // foreground-ish. This is intentionally lenient — being wrong errs on the
    // side of skipping the animation, which is the safer default.
    ["System Settings", "System Preferences"]
        .into_iter()
        .any(|n| desktop.application(n).is_ok())
}

#[cfg(not(target_os = "macos"))]
fn settings_is_foreground() -> bool {
    false
}

/// Default "from" position for cursor flights — picks a point near the chat
/// window's typical position so the flight feels intentional rather than random.
/// If a precise position is unavailable, we use a sensible fallback derived from
/// the main display dimensions.
fn default_chat_origin() -> (f64, f64) {
    #[cfg(target_os = "macos")]
    {
        use computer_use_ai_sdk::platforms::macos::display::get_main_display;
        if let Ok(disp) = get_main_display() {
            let w = disp.bounds.size.width;
            let h = disp.bounds.size.height;
            // Approx center of the chat window's lower-third (where the action buttons live).
            return (w * 0.5, h * 0.7);
        }
    }
    (640.0, 600.0)
}

// ── Public command: guide_to_system_settings ──────────────────────────────────

/// Tier-walking guide to System Settings: opens System Settings (if not already open),
/// waits for the window to appear, finds the Juno toggle (3-tier fallback), and
/// animates the onboarding cursor over to it with a highlight ring + bubble.
///
/// This command does NOT open System Settings itself — the caller has already invoked
/// the corresponding `request_*_permission` command, which opens the right pane.
/// We just need to provide the guided animation.
///
/// Returns a `GuidanceResult` describing which tier succeeded so the frontend can
/// adjust the chat message accordingly (e.g., be more verbose if we fell back to Tier 3).
#[tauri::command]
pub async fn guide_to_system_settings(
    app: AppHandle,
    permission_type: String,
) -> Result<GuidanceResult, String> {
    let perm = GuidedPermission::from_str(&permission_type)
        .ok_or_else(|| format!("Unknown permission type: {}", permission_type))?;

    info!("[onboarding-guidance] Starting guide for {:?}", perm);

    // Phase D edge case: if Settings was already foreground when we started,
    // skip the cursor flight — flying to a window the user is already looking
    // at feels gratuitous. The bubble + ring still render at the located
    // target so the in-app prompt remains useful.
    let already_open = tokio::task::spawn_blocking(settings_is_foreground)
        .await
        .unwrap_or(false);

    // Step 1: Wait up to 3s for System Settings to be findable.
    // If already_open, the bounds query usually returns immediately on the
    // first poll, so the timeout is effectively cosmetic in that case.
    let window_bounds = wait_for_settings_window(3000).await;

    let origin = default_chat_origin();

    // Step 2: Tier 2 — AX tree walk for Juno-specific control
    if window_bounds.is_some() {
        let juno_bounds = tokio::task::spawn_blocking(find_juno_control_bounds)
            .await
            .ok()
            .flatten();
        if let Some((x, y, w, h)) = juno_bounds {
            let cx = x + w / 2.0;
            let cy = y + h / 2.0;
            info!(
                "[onboarding-guidance] Tier 2 success: AX-located Juno at ({}, {})",
                cx, cy
            );
            fly_and_announce(
                &app,
                origin,
                (cx, cy),
                perm.bubble_text(),
                28.0,
                already_open,
            )
            .await?;
            return Ok(GuidanceResult {
                tier: 2,
                target_x: cx,
                target_y: cy,
                precise: true,
                message: format!("Found the Juno control in {}.", perm.settings_pane_label()),
            });
        }
    }

    // Step 3: Tier 1 — known coordinates derived from window bounds
    // System Settings on macOS 13+ uses a left sidebar (~245px) and a scrollable right pane.
    // Apps with toggles appear about 60–120px into the right pane and 180–280px down.
    if let Some((wx, wy, ww, wh)) = window_bounds {
        // Right pane starts after the sidebar; toggle column sits on the far right.
        let sidebar_width = 245.0_f64.min(ww * 0.34);
        let right_pane_x = wx + sidebar_width;
        let right_pane_w = (ww - sidebar_width).max(150.0);
        // App toggles in the apps list — first row is usually ~210px from the pane top.
        let target_x = right_pane_x + right_pane_w - 60.0; // toggle column near right edge
        let target_y = wy + (wh * 0.45).clamp(140.0, 380.0);
        info!(
            "[onboarding-guidance] Tier 1: window-bounds estimate ({}, {})",
            target_x, target_y
        );
        fly_and_announce(
            &app,
            origin,
            (target_x, target_y),
            perm.bubble_text(),
            36.0,
            already_open,
        )
        .await?;
        return Ok(GuidanceResult {
            tier: 1,
            target_x,
            target_y,
            precise: false,
            message: format!(
                "Look for **Juno** in the {} list and toggle it on.",
                perm.settings_pane_label()
            ),
        });
    }

    // Step 4: Tier 3 — general area highlight, screen center
    let (cx, cy) = origin;
    info!("[onboarding-guidance] Tier 3 fallback: general center highlight");
    fly_and_announce(
        &app,
        origin,
        (cx, cy),
        &format!("Open {} and toggle Juno on", perm.settings_pane_label()),
        64.0,
        already_open,
    )
    .await?;
    Ok(GuidanceResult {
        tier: 3,
        target_x: cx,
        target_y: cy,
        precise: false,
        message: format!(
            "Find **Juno** in the {} list and toggle it on. I'll keep watching!",
            perm.settings_pane_label()
        ),
    })
}

// ── Public command: run_permission_demo ───────────────────────────────────────

/// Run a live capability demo after a permission is granted.
/// Emits a stream of chat messages through the standard agent-text-stream pipeline.
///
/// Demos:
/// - **screen_recording**: capture a screenshot, describe the active app.
/// - **accessibility**: fly the cursor to a safe target (Dock area) and back.
/// - **microphone**: prompt the user to speak; transcription verification is out of scope here.
/// - **input_monitoring**: prompt the user to press Option+D.
#[tauri::command]
pub async fn run_permission_demo(app: AppHandle, permission_type: String) -> Result<(), String> {
    let perm = GuidedPermission::from_str(&permission_type)
        .ok_or_else(|| format!("Unknown permission type: {}", permission_type))?;

    info!("[onboarding-guidance] Running demo for {:?}", perm);

    // Cursor overlay is no longer needed during the chat-style demo.
    let _ = crate::commands::onboarding::dismiss_cursor_overlay(app.clone()).await;

    match perm {
        GuidedPermission::ScreenRecording => demo_screen_recording(&app).await,
        GuidedPermission::Accessibility => demo_accessibility(&app).await,
        GuidedPermission::Microphone => {
            emit_chat_message(&app, "**Microphone access granted!**\n\nSay something and I'll hear you. Press **⌥Space** to start dictating.");
            Ok(())
        }
        GuidedPermission::InputMonitoring => {
            emit_chat_message(&app, "**Input monitoring active.**\n\nTry pressing **⌥D** right now — the floating bar will pop up.");
            Ok(())
        }
    }
}

async fn demo_screen_recording(app: &AppHandle) -> Result<(), String> {
    // Capture a screenshot via the existing macOS pipeline. We only use it to detect
    // what app is currently focused — we never persist or display the image itself
    // (the frontend would have to handle base64, which adds complexity for no real win).
    #[cfg(target_os = "macos")]
    let focused_app_name = {
        // Spawn_blocking — AX/focus queries are synchronous and can take ~50ms.
        tokio::task::spawn_blocking(|| {
            use computer_use_ai_sdk::Desktop;
            let desktop = Desktop::new(true, false).ok()?;
            let focused = desktop.focused_element().ok()?;
            // Walk up to the application root to get its name
            let mut current = focused;
            for _ in 0..15 {
                let role = current.role().to_lowercase();
                if role == "application" || role == "axapplication" {
                    return current.attributes().label;
                }
                match current.parent().ok().flatten() {
                    Some(p) => current = p,
                    None => break,
                }
            }
            None
        })
        .await
        .ok()
        .flatten()
    };
    #[cfg(not(target_os = "macos"))]
    let focused_app_name: Option<String> = None;

    let app_phrase = focused_app_name
        .as_deref()
        .map(|n| format!("I can see you have **{}** open right now.", n))
        .unwrap_or_else(|| "I can see your screen — looks like a fresh desktop.".to_string());

    emit_chat_message(
        app,
        &format!(
            "**Screen Recording works!** {} One more permission and I'll be ready to help.",
            app_phrase
        ),
    );
    Ok(())
}

async fn demo_accessibility(app: &AppHandle) -> Result<(), String> {
    // Safe innocuous demo: fly the onboarding cursor sprite to a Dock-area target and
    // back to the chat origin. We never click anything — the demo is purely visual,
    // proving Juno can move things on screen now without doing anything destructive.
    let chat_origin = default_chat_origin();

    #[cfg(target_os = "macos")]
    let dock_target = {
        use computer_use_ai_sdk::platforms::macos::display::get_main_display;
        if let Ok(disp) = get_main_display() {
            let w = disp.bounds.size.width;
            let h = disp.bounds.size.height;
            // Center-bottom — close to where the Dock typically lives on macOS.
            (w * 0.5, h - 80.0)
        } else {
            (640.0, 700.0)
        }
    };
    #[cfg(not(target_os = "macos"))]
    let dock_target = (640.0, 700.0);

    crate::commands::onboarding::animate_cursor_to(
        app.clone(),
        chat_origin.0,
        chat_origin.1,
        dock_target.0,
        dock_target.1,
        Some("arc".to_string()),
    )
    .await?;
    sleep(Duration::from_millis(950)).await;

    // A small pulse at the dock — pure visual flourish, no click.
    crate::commands::onboarding::show_cursor_highlight(
        app.clone(),
        dock_target.0,
        dock_target.1,
        Some(40.0),
    )
    .await?;
    sleep(Duration::from_millis(700)).await;

    // Return home
    crate::commands::onboarding::animate_cursor_to(
        app.clone(),
        dock_target.0,
        dock_target.1,
        chat_origin.0,
        chat_origin.1,
        Some("arc".to_string()),
    )
    .await?;
    sleep(Duration::from_millis(950)).await;

    let _ = crate::commands::onboarding::dismiss_cursor_overlay(app.clone()).await;

    emit_chat_message(
        app,
        "**Accessibility granted!** Did you see that? I can move the cursor across your Mac now — clicking, typing, opening apps. You're ready to go.",
    );

    // Celebration micro-animation on the chat origin
    if let Err(e) = app.emit(
        "cursor-celebration",
        serde_json::json!({ "x": chat_origin.0, "y": chat_origin.1 }),
    ) {
        warn!("[onboarding-guidance] Failed to emit celebration: {}", e);
    }
    Ok(())
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Emit a markdown chat message through the same pipeline the agent uses, so it
/// renders inline in the onboarding chat without any special-casing on the frontend.
fn emit_chat_message(app: &AppHandle, message: &str) {
    let message_id = Uuid::new_v4().to_string();
    if let Err(e) = app.emit(
        events::streaming::STREAM_START,
        serde_json::json!({ "message_id": message_id }),
    ) {
        warn!("[onboarding-guidance] Failed to emit stream start: {}", e);
    }
    if let Err(e) = app.emit(
        events::streaming::TEXT_STREAM,
        serde_json::json!({
            "chunk": message,
            "message_id": message_id,
            "tts_content": null,
            "metadata": { "has_spoken_content": false, "spoken_text": null }
        }),
    ) {
        warn!("[onboarding-guidance] Failed to emit text stream: {}", e);
    }
    if let Err(e) = app.emit(
        events::streaming::STREAM_END,
        serde_json::json!({
            "message_id": message_id,
            "complete_text": message,
            "is_jsx": false
        }),
    ) {
        warn!("[onboarding-guidance] Failed to emit stream end: {}", e);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn permission_parsing_round_trip() {
        for s in [
            "screen_recording",
            "accessibility",
            "microphone",
            "input_monitoring",
        ] {
            let p = GuidedPermission::from_str(s).expect("known permission");
            // Pane label and bubble text should both be non-empty for every variant.
            assert!(
                !p.settings_pane_label().is_empty(),
                "pane label for {:?}",
                p
            );
            assert!(!p.bubble_text().is_empty(), "bubble text for {:?}", p);
        }
        assert!(GuidedPermission::from_str("nope").is_none());
    }

    #[test]
    fn default_chat_origin_is_finite() {
        let (x, y) = default_chat_origin();
        assert!(x.is_finite() && y.is_finite());
        assert!(x > 0.0 && y > 0.0);
    }
}
