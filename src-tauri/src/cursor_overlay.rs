//! Juno's cursor on screen.
//!
//! Every computer action that lands on a point ends here, and this module
//! decides everything the `desktop-cursor-overlay` window then draws: which
//! display the overlay covers, whether Juno moved the person's real cursor
//! (draw a glow behind it) or worked in the background (draw a ghost cursor
//! where it acted), and which color the glow is. The overlay page only renders
//! what it is told.
//!
//! Why the old ring was never seen, for the record, because each of these
//! alone was enough:
//!   1. The overlay window had no capability (`capabilities/*.json` listed
//!      every other window but not this one), so Tauri refused its
//!      `event.listen` and every window call. The page never heard a single
//!      `agent-cursor-update`. Pinned by
//!      `every_declared_window_has_a_capability_that_can_listen`.
//!   2. The page sized itself to the union of all displays. With "Displays
//!      have separate Spaces" (the macOS default) a window that spans displays
//!      is drawn on one of them only, and the page drew global coordinates
//!      without subtracting the window origin. The overlay now covers exactly
//!      the display Juno is acting on, and every update carries that origin.
//!   3. A window rebuilt by `create_or_show_window` came back at the floating
//!      level with no Spaces behaviour, so it sat under full-screen apps. The
//!      level and collection behaviour are now reapplied on every show.

use crate::constants::ui::{agent_cursor_colors, agent_session_colors};
use crate::state::{AgentCursorState, AppState};
use serde_json::Value;
use std::collections::HashSet;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager};

/// How long the overlay page takes to fade a cursor out. The window is hidden
/// only after this, so a release is seen as a fade, not a cut.
pub const FADE_OUT_MS: u64 = 260;

/// The real cursor's shape often changes a moment after it arrives (a link
/// turns it into a hand once hover lands), so the shape is read twice.
const SHAPE_SETTLE_MS: u64 = 150;

/// Session identity colors that belong to a parallel session after the first.
/// A cursor wearing one of these keeps it, so it matches its roster dot.
const PARALLEL_SESSION_COLORS: &[&str] = &[
    agent_session_colors::SLOT_1,
    agent_session_colors::SLOT_2,
    agent_session_colors::SLOT_3,
    agent_session_colors::SLOT_4,
    agent_session_colors::SLOT_5,
    agent_session_colors::SLOT_6,
    agent_session_colors::SLOT_7,
];

/// A display, in global logical points with the origin at the top left of the
/// primary display (the space computer-use coordinates are in).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DisplayRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl DisplayRect {
    fn contains(&self, x: f64, y: f64) -> bool {
        x >= self.x && x < self.x + self.width && y >= self.y && y < self.y + self.height
    }

    fn distance_sq(&self, x: f64, y: f64) -> f64 {
        let dx = (self.x - x).max(0.0).max(x - (self.x + self.width));
        let dy = (self.y - y).max(0.0).max(y - (self.y + self.height));
        dx * dx + dy * dy
    }
}

/// The display a point is on. A point in a gap between displays (or just off
/// an edge) gets the nearest one, so the cursor is never drawn nowhere.
pub fn display_for_point(x: f64, y: f64, displays: &[DisplayRect]) -> Option<DisplayRect> {
    if let Some(found) = displays.iter().find(|d| d.contains(x, y)) {
        return Some(*found);
    }
    displays.iter().copied().min_by(|a, b| {
        a.distance_sq(x, y)
            .partial_cmp(&b.distance_sq(x, y))
            .unwrap_or(std::cmp::Ordering::Equal)
    })
}

/// Whether the action moved the person's real cursor.
///
/// Every background-capable action reports `foreground` in its result. The few
/// that do not (a plain move with background mode off) took the real cursor
/// exactly when background mode is off.
pub fn takes_real_cursor(result: &Value, background_mode: bool) -> bool {
    result
        .get("foreground")
        .and_then(Value::as_bool)
        .unwrap_or(!background_mode)
}

/// The color a cursor is drawn in.
///
/// The primary agent (a single run, the Claude CLI, or the first parallel
/// session) wears the color chosen in Settings. Parallel sessions after the
/// first keep their identity color so the cursor matches the roster.
pub fn cursor_color(identity_color: &str, chosen: &str) -> String {
    if PARALLEL_SESSION_COLORS
        .iter()
        .any(|c| c.eq_ignore_ascii_case(identity_color))
    {
        identity_color.to_string()
    } else {
        agent_cursor_colors::hex(chosen).to_string()
    }
}

/// The overlay state named by an action.
pub fn cursor_state_for_action(action: &str) -> &'static str {
    match action {
        "left_click" | "right_click" | "middle_click" | "double_click" | "triple_click"
        | "left_mouse_down" => "clicking",
        "mouse_move" | "left_click_drag" => "moving",
        _ => "idle",
    }
}

/// The display the overlay currently covers; `None` while it is hidden.
static OVERLAY_FRAME: Mutex<Option<DisplayRect>> = Mutex::new(None);

/// Cursors that belong to a run with no session (a single run, the Claude
/// CLI). Nothing drops them the way a session drops its own, so they are
/// released when the query that made them ends.
static TRANSIENT_CURSORS: Mutex<Option<HashSet<String>>> = Mutex::new(None);

fn note_transient(agent_id: &str) {
    if let Ok(mut guard) = TRANSIENT_CURSORS.lock() {
        guard
            .get_or_insert_with(HashSet::new)
            .insert(agent_id.to_string());
    }
}

/// What a computer action did, as far as the overlay is concerned.
pub struct CursorAction<'a> {
    pub agent_id: &'a str,
    pub identity_color: &'a str,
    /// Global logical point the action landed on.
    pub x: f64,
    pub y: f64,
    pub action: &'a str,
    pub foreground: bool,
    /// True when no session owns this cursor (see [`release_transient`]).
    pub transient: bool,
}

/// Juno acted at a point: put its cursor there.
pub async fn take(app: &AppHandle, act: CursorAction<'_>) {
    let chosen = chosen_color(app).await;
    let color = cursor_color(act.identity_color, &chosen);
    if act.transient {
        note_transient(act.agent_id);
    }

    let origin = place_overlay(app, act.x, act.y);
    let cursor = AgentCursorState {
        agent_id: act.agent_id.to_string(),
        x: act.x,
        y: act.y,
        state: cursor_state_for_action(act.action).to_string(),
        color,
        foreground: act.foreground,
        origin_x: origin.x,
        origin_y: origin.y,
    };
    if let Some(state) = app.try_state::<AppState>() {
        state.update_agent_cursor(cursor.clone());
    }

    show_overlay(app);

    // Someone is driving another application, so the floating bar is where the
    // work shows and the only place it can be stopped. Cheap to repeat.
    crate::bar_stacking::note_agent_driving(app, true);

    if let Err(e) = app.emit(crate::constants::events::ui::AGENT_CURSOR_UPDATE, &cursor) {
        tracing::debug!("agent cursor update emit failed: {}", e);
    }

    // The glow is drawn in the real cursor's outline, so read it, but only
    // when it is Juno's to read: in the background the real cursor belongs to
    // the person and the ghost wears its own arrow.
    if act.foreground {
        crate::platform::system_cursor::refresh(app);
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(SHAPE_SETTLE_MS)).await;
            crate::platform::system_cursor::refresh(&app);
        });
    }
}

/// The color id chosen in Settings, Appearance.
async fn chosen_color(app: &AppHandle) -> String {
    match crate::settings::manager::SettingsManager::new(app.clone()) {
        Ok(manager) => manager
            .get_floating_bar_settings()
            .await
            .map(|s| agent_cursor_colors::resolve(&s.agent_cursor_color).to_string())
            .unwrap_or_else(|_| agent_cursor_colors::DEFAULT.to_string()),
        Err(_) => agent_cursor_colors::DEFAULT.to_string(),
    }
}

/// The displays, in global logical points.
fn displays(app: &AppHandle) -> Vec<DisplayRect> {
    let Ok(monitors) = app.available_monitors() else {
        return Vec::new();
    };
    monitors
        .iter()
        .map(|m| {
            let scale = m.scale_factor();
            let pos = m.position().to_logical::<f64>(scale);
            let size = m.size().to_logical::<f64>(scale);
            DisplayRect {
                x: pos.x,
                y: pos.y,
                width: size.width,
                height: size.height,
            }
        })
        .collect()
}

/// Cover the display the point is on, and return that display's origin, which
/// the page subtracts to draw in window coordinates.
fn place_overlay(app: &AppHandle, x: f64, y: f64) -> DisplayRect {
    let fallback = DisplayRect {
        x: 0.0,
        y: 0.0,
        width: 0.0,
        height: 0.0,
    };
    let Some(target) = display_for_point(x, y, &displays(app)) else {
        return fallback;
    };
    let current = OVERLAY_FRAME.lock().ok().and_then(|g| *g);
    if current == Some(target) {
        return target;
    }
    let Some(window) =
        app.get_webview_window(crate::window_management::DESKTOP_CURSOR_OVERLAY_LABEL)
    else {
        return target;
    };
    if let Err(e) = window.set_size(tauri::LogicalSize::new(target.width, target.height)) {
        tracing::debug!("Could not size the cursor overlay: {}", e);
    }
    if let Err(e) = window.set_position(tauri::LogicalPosition::new(target.x, target.y)) {
        tracing::debug!("Could not place the cursor overlay: {}", e);
    }
    if let Ok(mut guard) = OVERLAY_FRAME.lock() {
        *guard = Some(target);
    }
    target
}

/// Bring up the click-through overlay. Cheap when it is already up.
fn show_overlay(app: &AppHandle) {
    if let Some(window) =
        app.get_webview_window(crate::window_management::DESKTOP_CURSOR_OVERLAY_LABEL)
    {
        if window.is_visible().unwrap_or(false) {
            return;
        }
        // Level, Spaces and click-through are reapplied on every show: a
        // rebuilt window comes back without them.
        style_overlay(app, &window);
        if let Err(e) = window.show() {
            tracing::warn!("Could not show the cursor overlay: {}", e);
        }
        return;
    }

    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(e) = crate::window_management::open_desktop_cursor_overlay(app.clone()).await {
            tracing::warn!("Could not open the cursor overlay: {}", e);
            return;
        }
        if let Some(window) =
            app.get_webview_window(crate::window_management::DESKTOP_CURSOR_OVERLAY_LABEL)
        {
            style_overlay(&app, &window);
        }
    });
}

#[cfg(target_os = "macos")]
fn style_overlay(app: &AppHandle, window: &tauri::WebviewWindow) {
    crate::platform::macos::style_cursor_overlay(app, window);
}

#[cfg(not(target_os = "macos"))]
fn style_overlay(_app: &AppHandle, _window: &tauri::WebviewWindow) {}

/// Juno let go of a cursor: fade it out, and take the overlay away once
/// nobody is left. Called when a session ends, on every end path.
pub fn release(app: &AppHandle, agent_id: &str) {
    if let Some(state) = app.try_state::<AppState>() {
        state.remove_agent_cursor(agent_id);
    }
    if let Ok(mut guard) = TRANSIENT_CURSORS.lock() {
        if let Some(set) = guard.as_mut() {
            set.remove(agent_id);
        }
    }
    let payload = serde_json::json!({ "agent_id": agent_id });
    if let Err(e) = app.emit(crate::constants::events::ui::AGENT_CURSOR_REMOVE, &payload) {
        tracing::debug!("agent cursor remove emit failed: {}", e);
    }

    if !nobody_driving(app) {
        return;
    }
    // Nobody is driving any more, so the bar goes back to whatever the rest of
    // the situation asks for.
    crate::bar_stacking::note_agent_driving(app, false);

    // Hide only after the page's fade, and only if nobody took a cursor in
    // the meantime.
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(FADE_OUT_MS)).await;
        if !nobody_driving(&app) {
            return;
        }
        if let Some(window) =
            app.get_webview_window(crate::window_management::DESKTOP_CURSOR_OVERLAY_LABEL)
        {
            if let Err(e) = window.hide() {
                tracing::debug!("Could not hide the cursor overlay: {}", e);
            }
        }
        if let Ok(mut guard) = OVERLAY_FRAME.lock() {
            *guard = None;
        }
    });
}

/// Release every cursor no session owns. Called when a query ends.
pub fn release_transient(app: &AppHandle) {
    let ids: Vec<String> = match TRANSIENT_CURSORS.lock() {
        Ok(mut guard) => guard
            .take()
            .map(|s| s.into_iter().collect())
            .unwrap_or_default(),
        Err(_) => Vec::new(),
    };
    for id in ids {
        release(app, &id);
    }
}

fn nobody_driving(app: &AppHandle) -> bool {
    app.try_state::<AppState>()
        .map(|state| state.agent_cursors_is_empty())
        .unwrap_or(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn rect(x: f64, y: f64, width: f64, height: f64) -> DisplayRect {
        DisplayRect {
            x,
            y,
            width,
            height,
        }
    }

    #[test]
    fn a_point_picks_the_display_it_is_on() {
        let laptop = rect(0.0, 0.0, 1512.0, 982.0);
        // An external display to the left of and above the primary: negative
        // origin, which the old union-window drawing got wrong.
        let external = rect(-2560.0, -458.0, 2560.0, 1440.0);
        let displays = [laptop, external];

        assert_eq!(display_for_point(700.0, 400.0, &displays), Some(laptop));
        assert_eq!(display_for_point(-10.0, 10.0, &displays), Some(external));
        assert_eq!(
            display_for_point(-2560.0, -458.0, &displays),
            Some(external)
        );
    }

    #[test]
    fn a_point_off_every_display_gets_the_nearest() {
        let laptop = rect(0.0, 0.0, 1512.0, 982.0);
        let right = rect(1512.0, 0.0, 1920.0, 1080.0);
        // Below the laptop, which is shorter than its neighbour.
        assert_eq!(
            display_for_point(200.0, 1000.0, &[laptop, right]),
            Some(laptop)
        );
        assert_eq!(
            display_for_point(5000.0, 10.0, &[laptop, right]),
            Some(right)
        );
        assert_eq!(display_for_point(1.0, 1.0, &[]), None);
    }

    #[test]
    fn the_result_says_who_moved_the_cursor() {
        let bg = json!({ "success": true, "input_tier": "process_targeted", "foreground": false });
        let fg = json!({ "success": true, "input_tier": "physical", "foreground": true });
        assert!(!takes_real_cursor(&bg, false));
        assert!(takes_real_cursor(&fg, true));
        // No tier reported: the real cursor moved exactly when background mode
        // is off.
        let bare = json!({ "success": true });
        assert!(takes_real_cursor(&bare, false));
        assert!(!takes_real_cursor(&bare, true));
    }

    #[test]
    fn the_primary_cursor_wears_the_chosen_color() {
        // The first session slot, the Claude CLI pink and no color at all
        // become the setting. The old per-query violet is not listed: it is
        // also parallel slot 4's color, and that slot keeps its own.
        for identity in [agent_session_colors::SLOT_0, "#FF2D55", ""] {
            assert_eq!(
                cursor_color(identity, agent_cursor_colors::GREEN),
                agent_cursor_colors::GREEN_HEX
            );
        }
        assert_eq!(
            cursor_color(agent_session_colors::SLOT_0, "nonsense"),
            agent_cursor_colors::BLUE_HEX
        );
    }

    #[test]
    fn parallel_sessions_keep_their_roster_color() {
        for slot in PARALLEL_SESSION_COLORS {
            assert_eq!(cursor_color(slot, agent_cursor_colors::PINK), *slot);
        }
        assert_eq!(
            cursor_color(&agent_session_colors::SLOT_3.to_lowercase(), "blue"),
            agent_session_colors::SLOT_3.to_lowercase()
        );
    }

    #[test]
    fn clicks_pulse_and_moves_glide() {
        for a in [
            "left_click",
            "double_click",
            "right_click",
            "left_mouse_down",
        ] {
            assert_eq!(cursor_state_for_action(a), "clicking");
        }
        assert_eq!(cursor_state_for_action("mouse_move"), "moving");
        assert_eq!(cursor_state_for_action("scroll"), "idle");
    }

    /// The payload the overlay page reads. Field names are the contract with
    /// `src/lib/agentCursor.ts`; renaming one silently blanks the overlay.
    #[test]
    fn an_update_carries_what_the_page_draws_with() {
        let cursor = AgentCursorState {
            agent_id: "s1".into(),
            x: -100.0,
            y: 20.0,
            state: "clicking".into(),
            color: agent_cursor_colors::BLUE_HEX.into(),
            foreground: false,
            origin_x: -2560.0,
            origin_y: -458.0,
        };
        let v = serde_json::to_value(cursor).unwrap();
        for key in [
            "agent_id",
            "x",
            "y",
            "state",
            "color",
            "foreground",
            "origin_x",
            "origin_y",
        ] {
            assert!(v.get(key).is_some(), "update is missing {key}");
        }
        assert_eq!(v["origin_x"], json!(-2560.0));
    }

    #[test]
    fn transient_cursors_are_remembered_once() {
        note_transient("agent-test-1");
        note_transient("agent-test-1");
        let held = TRANSIENT_CURSORS
            .lock()
            .unwrap()
            .as_ref()
            .map(|s| s.iter().filter(|id| *id == "agent-test-1").count())
            .unwrap_or(0);
        assert_eq!(held, 1);
    }

    /// Root cause of the never-seen ring: a window no capability names cannot
    /// listen for events, so the overlay never heard an update. Every window
    /// Juno declares must be granted `core:event:default` by some capability.
    #[test]
    fn every_declared_window_has_a_capability_that_can_listen() {
        let manifest = env!("CARGO_MANIFEST_DIR");
        let conf: Value = serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        let declared: Vec<String> = conf["app"]["windows"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|w| w["label"].as_str().map(String::from))
            .collect();

        let mut listening: HashSet<String> = HashSet::new();
        let dir = std::path::Path::new(manifest).join("capabilities");
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let cap: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap())
                .unwrap_or_else(|e| panic!("{} is not JSON: {e}", path.display()));
            let grants_listen = cap["permissions"]
                .as_array()
                .map(|perms| {
                    perms.iter().any(|p| {
                        matches!(
                            p.as_str(),
                            Some("core:default" | "core:event:default" | "core:event:allow-listen")
                        )
                    })
                })
                .unwrap_or(false);
            if !grants_listen {
                continue;
            }
            for w in cap["windows"].as_array().into_iter().flatten() {
                if let Some(label) = w.as_str() {
                    listening.insert(label.to_string());
                }
            }
        }

        for label in &declared {
            assert!(
                listening.contains(label),
                "window '{label}' is declared but no capability lets it listen for events"
            );
        }
        assert!(listening.contains(crate::window_management::DESKTOP_CURSOR_OVERLAY_LABEL));
    }

    /// The overlay must be able to place itself and stay click-through, which
    /// are window permissions `core:window:default` does not include.
    #[test]
    fn the_overlay_capability_grants_what_the_page_calls() {
        let cap: Value =
            serde_json::from_str(include_str!("../capabilities/overlays.json")).unwrap();
        let windows: Vec<&str> = cap["windows"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(Value::as_str)
            .collect();
        assert!(windows.contains(&crate::window_management::DESKTOP_CURSOR_OVERLAY_LABEL));
        let perms: Vec<&str> = cap["permissions"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(Value::as_str)
            .collect();
        for needed in [
            "core:event:default",
            "core:window:allow-set-ignore-cursor-events",
        ] {
            assert!(perms.contains(&needed), "overlays.json lacks {needed}");
        }
    }
}
