//! Official Anthropic Computer Use tools for desktop screen interaction.
//! Implements the complete Anthropic Computer Use API specification.

use crate::agent::core::{AgentError, ToolDefinition};
use crate::agent::implementations::tool_provider::LocalToolProvider;
use crate::state::AppState;
use crate::utils::coordinates;
use crate::utils::permission_validator::{validate_permission, RequiredPermission};
// Removed unused import - BashResult is handled differently now
// Keep the tool versioning from errors branch (enhanced functionality)
use super::tool_versioning::{ToolVersionConfig, ToolVersionManager};
use crate::state::AgentCursorState;
use crate::utils::coordinate_validation::{
    validate_coordinate_pair, validate_coordinate_parameter, CoordinateValidationError,
};
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use tauri::{Emitter, Manager};
use tracing::{info, warn};

/// Minimum milliseconds between consecutive UI-modifying actions (click, type, key).
/// Prevents "clicked too fast" failures when the UI is still loading/animating.
const ACTION_COOLDOWN_MS: u64 = 300;

/// Process-start baseline for [`monotonic_now_ms`].
///
/// [`std::time::Instant`] is the only clock guaranteed to move forward, but it
/// cannot live in an `AtomicU64`. Measuring from one fixed baseline gives a
/// monotonic millisecond counter that can, so the cooldown stays lock-free.
static MONOTONIC_BASELINE: std::sync::LazyLock<std::time::Instant> =
    std::sync::LazyLock::new(std::time::Instant::now);

/// Milliseconds on the monotonic clock, counted from a process-start baseline.
///
/// Offset by one so a real reading is never `0`, which leaves `0` free to mean
/// "no action recorded yet" in [`LAST_UI_ACTION_MS`].
fn monotonic_now_ms() -> u64 {
    (MONOTONIC_BASELINE.elapsed().as_millis() as u64).saturating_add(1)
}

/// Monotonic millisecond stamp of the last UI-modifying action; `0` if none.
///
/// Monotonic, **not** wall clock. This used to hold `SystemTime` millis since
/// the epoch, which is a silent hazard: an NTP step *forward* makes the
/// measured gap look large, the cooldown then sleeps zero, and the only pacing
/// AX-path clicks and typing ever get disappears — precisely the "clicked too
/// fast" failure [`ACTION_COOLDOWN_MS`] exists to prevent. A step *backward*
/// was harmless by comparison (one spurious full-length sleep), so the
/// dangerous direction was the one that produced no symptom in a log.
///
/// [`crate::agent::input_arbiter::InputArbiter`] already measures its own
/// cooldown with `Instant`; this makes the two agree on what a clock is.
static LAST_UI_ACTION_MS: AtomicU64 = AtomicU64::new(0);

/// Monotonic counter for assigning unique cursor IDs to concurrent agent instances.
static NEXT_AGENT_CURSOR_ID: AtomicU64 = AtomicU64::new(1);

/// Agent cursor color palette — each slot is used round-robin for up to 8 concurrent agents.
const AGENT_CURSOR_COLORS: &[&str] = &[
    "#8B5CF6", // violet  (matches default Juno cursor)
    "#10B981", // emerald
    "#F59E0B", // amber
    "#EF4444", // red
    "#3B82F6", // blue
    "#EC4899", // pink
    "#14B8A6", // teal
    "#F97316", // orange
];

/// Identity of the parallel agent session that owns a tool registration
/// (LAC-1432). When present, the agent's overlay cursor is keyed by the
/// session id and drawn in the session's identity color, and physical
/// input is attributed to the session in the input arbiter. When absent
/// (legacy callers), a process-unique `agent-N` cursor id and the legacy
/// palette are used instead.
#[derive(Clone, Debug)]
pub struct SessionToolContext {
    pub session_id: String,
    pub color: String,
}

/// Run one computer action and show, on screen, that Juno did it.
///
/// The single entry point for computer use, whichever provider asked. The
/// in-process tool registry calls it, and so does the MCP server Juno offers
/// to the Claude CLI, because the alternative is two implementations of
/// "move the mouse and say so" that drift apart until one of them silently
/// stops drawing a cursor. Which is exactly what happened when mouse control
/// was delegated to an outside binary: the smooth-movement setting and the
/// cursor overlay both lived here, on a path nothing took any more.
pub async fn run_computer_action(
    app_handle: &tauri::AppHandle,
    input: Value,
    session_id: Option<&str>,
    cursor_id: &str,
    cursor_color: &str,
) -> Result<Value, String> {
    let result = execute_computer_tool(app_handle, input.clone(), session_id).await;

    // Emit cursor position for the agent overlay (non-blocking).
    if result.is_ok() {
        let action = input["action"].as_str().unwrap_or("");
        if let Some((raw_x, raw_y)) = extract_coordinate(&input) {
            use crate::utils::coordinates;
            let (sx, sy) = coordinates::transform_to_screen_coordinates(raw_x, raw_y);
            let cursor_state = match action {
                "left_click" | "right_click" | "middle_click" | "double_click" | "triple_click" => {
                    "clicking"
                }
                "mouse_move" => "moving",
                _ => "idle",
            };
            emit_agent_cursor_update(app_handle, cursor_id, sx, sy, cursor_state, cursor_color);
        }
        // A screenshot has no coordinate, so the cursor is left where it is
        // rather than moved to nowhere.
    }

    result
}

/// Emit a cursor position update for a named agent. No-op if the app_handle cannot emit.
fn emit_agent_cursor_update(
    app_handle: &tauri::AppHandle,
    agent_id: &str,
    x: f64,
    y: f64,
    cursor_state: &str,
    color: &str,
) {
    let state_manager = app_handle.state::<AppState>();
    let cursor = AgentCursorState {
        agent_id: agent_id.to_string(),
        x,
        y,
        state: cursor_state.to_string(),
        color: color.to_string(),
    };
    state_manager.update_agent_cursor(cursor.clone());

    // Put the overlay on screen. It is declared hidden and nothing ever showed
    // it, so every cursor update so far has been drawn into a window nobody
    // was looking at: the pointer moved on its own with nothing to say why.
    show_cursor_overlay(app_handle);

    if let Err(e) = app_handle.emit(crate::constants::events::ui::AGENT_CURSOR_UPDATE, &cursor) {
        tracing::debug!("agent cursor update emit failed: {}", e);
    }
}

/// Bring up the click-through overlay that draws agent cursors.
///
/// Cheap to call repeatedly: an already-visible window is left alone, so this
/// sits on the hot path of every mouse action without costing anything after
/// the first one.
fn show_cursor_overlay(app_handle: &tauri::AppHandle) {
    use tauri::Manager;
    if let Some(window) =
        app_handle.get_webview_window(crate::window_management::DESKTOP_CURSOR_OVERLAY_LABEL)
    {
        if window.is_visible().unwrap_or(false) {
            return;
        }
        if let Err(e) = window.show() {
            tracing::warn!("Could not show the cursor overlay: {}", e);
        }
        return;
    }

    // Not built yet (it is declared in tauri.conf.json, but a window can be
    // destroyed). Building is async, so it is spawned rather than awaited on
    // the action path.
    let app = app_handle.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(e) = crate::window_management::open_desktop_cursor_overlay(app).await {
            tracing::warn!("Could not open the cursor overlay: {}", e);
        }
    });
}

/// Emit cursor removal for a named agent (call on agent completion or cancellation).
///
/// Called from `SessionHandle::drop` so every session end path — complete,
/// cancel, error, panic unwind — clears the session's overlay cursor.
pub(crate) fn emit_agent_cursor_remove(app_handle: &tauri::AppHandle, agent_id: &str) {
    if let Some(state_manager) = app_handle.try_state::<AppState>() {
        state_manager.remove_agent_cursor(agent_id);
    }
    let payload = serde_json::json!({ "agent_id": agent_id });
    if let Err(e) = app_handle.emit(crate::constants::events::ui::AGENT_CURSOR_REMOVE, &payload) {
        tracing::debug!("agent cursor remove emit failed: {}", e);
    }

    // Once nobody is driving, take the overlay away. It is click-through and
    // transparent, so leaving it up harms nothing, but a window that is only
    // ever shown is a window that is eventually blamed for something.
    let nobody_left = app_handle
        .try_state::<AppState>()
        .map(|state| state.agent_cursors_is_empty())
        .unwrap_or(true);
    if nobody_left {
        use tauri::Manager;
        if let Some(window) =
            app_handle.get_webview_window(crate::window_management::DESKTOP_CURSOR_OVERLAY_LABEL)
        {
            if let Err(e) = window.hide() {
                tracing::debug!("Could not hide the cursor overlay: {}", e);
            }
        }
    }
}

/// Extract [x, y] from a computer tool input's "coordinate" field.
/// Returns None if the field is absent or malformed.
fn extract_coordinate(input: &Value) -> Option<(f64, f64)> {
    let arr = input["coordinate"].as_array()?;
    let x = arr.first()?.as_f64()?;
    let y = arr.get(1)?.as_f64()?;
    Some((x, y))
}

/// Returns true if the action modifies the UI (click, type, key, scroll, drag).
/// Read-only actions (screenshot, cursor_position, wait) skip the cooldown.
///
/// This is the single list of physically-mutating actions. It drives both the
/// inter-action cooldown and [`failure_halts_batch`] — add new mutating actions
/// here, never to a second copy.
pub(crate) fn is_ui_modifying_action(action: &str) -> bool {
    matches!(
        action,
        "left_click"
            | "right_click"
            | "middle_click"
            | "double_click"
            | "triple_click"
            | "left_click_drag"
            | "mouse_move"
            | "left_mouse_down"
            | "left_mouse_up"
            | "key"
            | "hold_key"
            | "type"
            | "scroll"
    )
}

/// Whether a failed tool call should stop the rest of the batch it belongs to.
///
/// **The scope is deliberately narrow — read this before widening or narrowing it.**
///
/// A batch of physical desktop actions is an ordered plan: `left_click`, then
/// `type`, then `key Return`. If the click missed, the remaining actions land in
/// whatever window happens to be focused, so they must not run. That is a real
/// safety problem, and it is the reason the halt exists.
///
/// Nothing else in a batch has that property. Two failed file reads are
/// independent; halting the second because the first failed costs a model
/// round trip and buys no safety. Read-only computer actions are the same:
/// a failed `screenshot` or `cursor_position` changes nothing on screen, the
/// model still receives the error, and the clicks that follow were already
/// planned against an earlier screenshot — so a failing `screenshot` does *not*
/// halt the clicks behind it.
///
/// The action name is taken from `input.action` for the `computer` tool, and
/// from the tool's own name otherwise, then matched against
/// [`is_ui_modifying_action`]. Reusing that one list is the point: a new
/// mutating action added there is covered here for free, and no file, browser
/// or shell tool can ever match it.
///
/// A `computer` call whose action is missing or not a string halts. Such a call
/// may already have moved the pointer or typed, and halting costs a round trip
/// while continuing can type into the wrong window.
pub(crate) fn failure_halts_batch(tool_name: &str, input: &Value) -> bool {
    if tool_name == "computer" {
        return match input.get("action").and_then(Value::as_str) {
            Some(action) => is_ui_modifying_action(action),
            None => true,
        };
    }
    is_ui_modifying_action(tool_name)
}

/// If the action is UI-modifying and the cooldown hasn't elapsed, sleep briefly.
/// Records the current time for the next cooldown check.
///
/// Elapsed time is measured on the monotonic clock (see [`monotonic_now_ms`]),
/// never on the wall clock: a wall-clock jump must not be able to cancel the
/// pacing.
async fn enforce_action_cooldown(action: &str) {
    if !is_ui_modifying_action(action) {
        return;
    }

    let now_ms = monotonic_now_ms();

    let last_ms = LAST_UI_ACTION_MS.load(Ordering::Relaxed);
    if last_ms > 0 {
        let elapsed = now_ms.saturating_sub(last_ms);
        if elapsed < ACTION_COOLDOWN_MS {
            let wait = ACTION_COOLDOWN_MS - elapsed;
            tracing::debug!(
                "Action cooldown: waiting {}ms before {} ({}ms since last action)",
                wait,
                action,
                elapsed
            );
            tokio::time::sleep(std::time::Duration::from_millis(wait)).await;
        }
    }

    // Record this action's timestamp, re-read after any sleep.
    LAST_UI_ACTION_MS.store(monotonic_now_ms(), Ordering::Relaxed);
}

// --- Computer Use Safety Checks ---

/// Juno's own bundle identifier — used for audit logging when the agent
/// targets its own window.
const JUNO_BUNDLE_ID: &str = "com.juno.desktop";

/// Bundle IDs that receive extra audit logging when targeted.
/// These are sensitive system apps — currently observe-only (no blocking).
/// To enforce blocking, check these in `check_app_safety` and return Err.
const NOTABLE_BUNDLE_IDS: &[&str] = &[
    JUNO_BUNDLE_ID,                // Self-automation awareness
    "com.apple.systempreferences", // System Preferences / System Settings
    "com.apple.keychainaccess",    // Keychain Access — credential store
];

/// Actions considered sensitive/destructive — these get extra audit logging.
/// Kept narrow to avoid false positives on normal text like "remove the space".
const SENSITIVE_PATTERNS: &[&str] = &[
    "rm -rf",
    "rm -r",
    "sudo",
    "format disk",
    "mkfs",
    "drop table",
    "drop database",
    "truncate",
    "password",
    "credential",
    "secret",
    "api_key",
    "api-key",
    "force push",
    "git push -f",
    "git push --force",
    "complete checkout",
    "wire transfer",
    "payment",
];

/// NSString encoding constant for UTF-8 (Apple docs: NSUTF8StringEncoding = 4).
#[cfg(target_os = "macos")]
const NS_UTF8_STRING_ENCODING: usize = 4;

/// Get the frontmost application's bundle ID via NSWorkspace.
/// Returns None if detection fails (non-fatal).
#[cfg(target_os = "macos")]
fn get_frontmost_bundle_id() -> Option<String> {
    use objc::{class, msg_send, sel, sel_impl};

    unsafe {
        let workspace_class = class!(NSWorkspace);
        let shared_workspace: *mut objc::runtime::Object =
            msg_send![workspace_class, sharedWorkspace];
        if shared_workspace.is_null() {
            return None;
        }
        let frontmost_app: *mut objc::runtime::Object =
            msg_send![shared_workspace, frontmostApplication];

        if frontmost_app.is_null() {
            return None;
        }

        let bundle_id_obj: *mut objc::runtime::Object = msg_send![frontmost_app, bundleIdentifier];
        if bundle_id_obj.is_null() {
            return None;
        }

        let bytes: *const std::os::raw::c_char = msg_send![bundle_id_obj, UTF8String];
        let len: usize =
            msg_send![bundle_id_obj, lengthOfBytesUsingEncoding:NS_UTF8_STRING_ENCODING];
        if bytes.is_null() || len == 0 {
            return None;
        }

        let bytes_slice = std::slice::from_raw_parts(bytes as *const u8, len);
        std::str::from_utf8(bytes_slice).ok().map(|s| s.to_string())
    }
}

#[cfg(not(target_os = "macos"))]
fn get_frontmost_bundle_id() -> Option<String> {
    None
}

/// Get the frontmost application's localized name via NSWorkspace.
#[cfg(target_os = "macos")]
fn get_frontmost_app_name() -> Option<String> {
    use objc::{class, msg_send, sel, sel_impl};

    unsafe {
        let workspace_class = class!(NSWorkspace);
        let shared_workspace: *mut objc::runtime::Object =
            msg_send![workspace_class, sharedWorkspace];
        if shared_workspace.is_null() {
            return None;
        }
        let frontmost_app: *mut objc::runtime::Object =
            msg_send![shared_workspace, frontmostApplication];

        if frontmost_app.is_null() {
            return None;
        }

        let name_obj: *mut objc::runtime::Object = msg_send![frontmost_app, localizedName];
        if name_obj.is_null() {
            return None;
        }

        let bytes: *const std::os::raw::c_char = msg_send![name_obj, UTF8String];
        let len: usize = msg_send![name_obj, lengthOfBytesUsingEncoding:NS_UTF8_STRING_ENCODING];
        if bytes.is_null() || len == 0 {
            return None;
        }

        let bytes_slice = std::slice::from_raw_parts(bytes as *const u8, len);
        std::str::from_utf8(bytes_slice).ok().map(|s| s.to_string())
    }
}

#[cfg(not(target_os = "macos"))]
fn get_frontmost_app_name() -> Option<String> {
    None
}

/// Localized name of a running application, by pid.
///
/// In background mode the app the agent is working on is deliberately not the
/// frontmost one, so anything shown to the user about the target has to be
/// resolved from the process the agent actually reached.
#[cfg(target_os = "macos")]
fn app_name_for_pid(pid: i32) -> Option<String> {
    use objc::{class, msg_send, sel, sel_impl};

    unsafe {
        let running_app_class = class!(NSRunningApplication);
        let app: *mut objc::runtime::Object =
            msg_send![running_app_class, runningApplicationWithProcessIdentifier: pid];
        if app.is_null() {
            return None;
        }

        let name_obj: *mut objc::runtime::Object = msg_send![app, localizedName];
        if name_obj.is_null() {
            return None;
        }

        let bytes: *const std::os::raw::c_char = msg_send![name_obj, UTF8String];
        let len: usize = msg_send![name_obj, lengthOfBytesUsingEncoding:NS_UTF8_STRING_ENCODING];
        if bytes.is_null() || len == 0 {
            return None;
        }

        let bytes_slice = std::slice::from_raw_parts(bytes as *const u8, len);
        std::str::from_utf8(bytes_slice).ok().map(|s| s.to_string())
    }
}

#[cfg(not(target_os = "macos"))]
fn app_name_for_pid(_pid: i32) -> Option<String> {
    None
}

/// The app a consent prompt should name: the one the agent has been acting on,
/// falling back to the frontmost app when nothing has been targeted yet.
fn consent_target_app(fallback: Option<&str>) -> Option<String> {
    computer_use_ai_sdk::background::target_pid()
        .and_then(app_name_for_pid)
        .or_else(|| fallback.map(|s| s.to_string()))
}

/// Logs when the agent targets a notable app (Juno itself, system prefs, etc.).
/// Currently observe-only — always allows the action. The infrastructure exists
/// so blocking can be enabled later via user settings if desired.
fn check_app_safety(action: &str) -> Result<(), String> {
    // Read-only actions don't need any logging
    if !is_ui_modifying_action(action) {
        return Ok(());
    }

    if let Some(bundle_id) = get_frontmost_bundle_id() {
        if NOTABLE_BUNDLE_IDS.contains(&bundle_id.as_str()) {
            let app_name = get_frontmost_app_name().unwrap_or_else(|| bundle_id.clone());

            if bundle_id == JUNO_BUNDLE_ID {
                info!(
                    "🔍 Self-targeting: agent is performing '{}' in Juno's own window ({})",
                    action, app_name
                );
            } else {
                info!(
                    "🔍 Notable app target: agent is performing '{}' in {} ({})",
                    action, app_name, bundle_id
                );
            }
        }
    }

    // Always allow — observe only
    Ok(())
}

/// Check if an action's typed text contains sensitive patterns.
/// Returns the matched pattern if found (for audit logging), or None.
fn detect_sensitive_content(input: &Value) -> Option<&'static str> {
    let text = input["text"].as_str().unwrap_or_default().to_lowercase();
    if text.is_empty() {
        return None;
    }

    SENSITIVE_PATTERNS
        .iter()
        .find(|&&pattern| text.contains(pattern))
        .copied()
}

/// Emit an audit log event for the action being performed.
/// This allows the frontend to display a reviewable history of agent actions.
fn emit_action_audit(
    app_handle: &tauri::AppHandle,
    action: &str,
    input: &Value,
    target_app: Option<&str>,
    sensitive_pattern: Option<&str>,
) {
    let audit = json!({
        "action": action,
        "target_app": target_app,
        "sensitive": sensitive_pattern.is_some(),
        "sensitive_pattern": sensitive_pattern,
        "timestamp": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_else(|_| std::time::Duration::from_secs(0))
            .as_millis() as u64,
        "coordinate": input.get("coordinate"),
        "text_preview": input["text"].as_str().map(|t| {
            if t.chars().count() > 50 { format!("{}...", t.chars().take(50).collect::<String>()) } else { t.to_string() }
        }),
    });

    if let Err(e) = app_handle.emit(crate::constants::events::tools::COMPUTER_USE_AUDIT, &audit) {
        tracing::debug!("Failed to emit audit event: {}", e);
    }
}

// --- AX-Grounded Clicking ---

/// Click variants supported by AX grounding.
#[derive(Debug, Clone, Copy)]
enum AxClickKind {
    Left,
    Right,
    Double,
}

/// Result of an AX grounding attempt for a click action.
struct AxGroundingResult {
    /// True if AXPress (accessibility-native click) was used; false means caller
    /// should perform a coordinate-based click as fallback.
    used_ax_click: bool,
    /// AX role of the element at the position (e.g., "AXButton"), if found.
    role: Option<String>,
    /// Label/title of the element, if available.
    label: Option<String>,
}

/// Whether an AX role represents an interactive UI element worth clicking via AXPress.
/// Accepts both prefixed ("AXButton") and unprefixed ("button") forms — the
/// accessibility crate sometimes returns one or the other depending on the app.
fn is_interactive_ax_role(role: &str) -> bool {
    let normalized = role.trim_start_matches("AX").to_lowercase();
    matches!(
        normalized.as_str(),
        "button"
            | "link"
            | "textfield"
            | "textarea"
            | "checkbox"
            | "radiobutton"
            | "popupbutton"
            | "combobox"
            | "tab"
            | "menuitem"
            | "menubutton"       // fixed typo from "menubuttom"
            | "image"
            | "cell"
            | "searchfield"
            | "statictext"
            | "row"
            | "list"
            | "slider"           // kAXIncrementAction / kAXDecrementAction
            | "incrementor"      // steppers (NSStepper)
            | "colorwell"        // color pickers
            | "disclosuretriangle" // disclosure triangles
            | "switch" // toggle switches
    )
}

/// Attempt an AX-grounded click at the given screen coordinates.
///
/// Performs a fast native hit-test (~1-5ms) via `AXUIElementCopyElementAtPosition`.
/// If an interactive element is found, performs an AXPress action (semantic
/// click) instead of a CGEvent coordinate click — more accurate and robust.
///
/// On any failure (no element, non-interactive role, AXPress error, missing
/// permissions), returns `used_ax_click: false` so the caller falls back to
/// the existing coordinate click path. Never panics.
fn try_ax_grounded_click(
    app_handle: &tauri::AppHandle,
    screen_x: f64,
    screen_y: f64,
    kind: AxClickKind,
) -> AxGroundingResult {
    let state = app_handle.state::<AppState>();

    let element = match state.desktop.element_at_position(screen_x, screen_y) {
        Some(el) => el,
        None => {
            return AxGroundingResult {
                used_ax_click: false,
                role: None,
                label: None,
            };
        }
    };

    let attrs = element.attributes();
    let role = attrs.role.clone();
    let label = attrs.label.clone();

    if !is_interactive_ax_role(&role) {
        tracing::debug!(
            "AX grounding: element at ({:.0}, {:.0}) is role='{}' (not interactive) — skipping AXPress",
            screen_x, screen_y, role
        );
        return AxGroundingResult {
            used_ax_click: false,
            role: Some(role),
            label,
        };
    }

    // Attempt AX-native action. Left/Double use semantic click; Right uses kAXShowMenuAction
    // (the true macOS action for context menus — no cursor position required).
    let result = match kind {
        AxClickKind::Left => element.click().map(|_| ()),
        AxClickKind::Double => element.double_click().map(|_| ()),
        // AXShowMenu is the semantic AX action for context menus. Falls back to coordinate
        // right-click (via the outer caller) if the element doesn't support it.
        AxClickKind::Right => element.perform_action("AXShowMenu"),
    };

    match result {
        Ok(()) => {
            info!(
                "✨ AX grounded click ({:?}): {} '{}' at ({:.0}, {:.0})",
                kind,
                role,
                label.as_deref().unwrap_or("<unlabeled>"),
                screen_x,
                screen_y
            );
            AxGroundingResult {
                used_ax_click: true,
                role: Some(role),
                label,
            }
        }
        Err(e) => {
            // Warn-level so we can track AX fallback coverage over time and improve it.
            tracing::warn!(
                "⚠️ AX action failed, falling back to coordinate: {} '{}' at ({:.0}, {:.0}): {}",
                role,
                label.as_deref().unwrap_or("<unlabeled>"),
                screen_x,
                screen_y,
                e
            );
            AxGroundingResult {
                used_ax_click: false,
                role: Some(role),
                label,
            }
        }
    }
}

/// Try to type text directly into the currently focused AX element.
///
/// Fetches the system-wide focused element and calls `type_text()` on it, which
/// first tries `kAXValueAttribute` (no cursor, no clipboard) and falls back to
/// clipboard paste if that fails. Returns true if AX typing succeeded.
///
/// On any failure (no permissions, no focused element, element rejects AXValue),
/// returns false so the caller can fall back to global keyboard simulation.
fn try_ax_type_focused(app_handle: &tauri::AppHandle, text: &str) -> bool {
    let state = app_handle.state::<AppState>();
    match state.desktop.focused_element() {
        Ok(element) => match element.type_text(text) {
            Ok(()) => {
                info!(
                    "✨ AX type: {} chars typed into focused element via AXValue",
                    text.chars().count()
                );
                true
            }
            Err(e) => {
                tracing::warn!(
                    "AX type failed on focused element: {} — falling back to global keyboard",
                    e
                );
                false
            }
        },
        Err(e) => {
            tracing::debug!("Could not get focused element for AX type: {}", e);
            false
        }
    }
}

/// Emit an AX grounding audit event so the frontend can show element metadata
/// in the action audit trail (e.g., "Clicked button 'Send'" instead of just coords).
fn emit_ax_grounding_audit(
    app_handle: &tauri::AppHandle,
    action: &str,
    screen_x: f64,
    screen_y: f64,
    result: &AxGroundingResult,
) {
    let payload = json!({
        "action": action,
        "ax_grounded": result.used_ax_click,
        "ax_role": result.role,
        "ax_label": result.label,
        "screen_coordinate": [screen_x, screen_y],
        "timestamp": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_else(|_| std::time::Duration::from_secs(0))
            .as_millis() as u64,
    });
    if let Err(e) = app_handle.emit(
        crate::constants::events::tools::AX_GROUNDING_AUDIT,
        &payload,
    ) {
        tracing::debug!("Failed to emit AX grounding audit: {}", e);
    }
}

// --- Background-first input ---

/// Run one input step, preferring the background route and asking before taking
/// the cursor.
///
/// `attempt` is handed `allow_physical`. It returns `Ok(None)` to say "this can
/// only be done by driving the shared pointer". When background mode is off the
/// closure is called once with the physical tier already unlocked, so behaviour
/// is exactly what it was before background mode existed.
async fn run_background_first<F>(
    app_handle: &tauri::AppHandle,
    action: &str,
    target_app: Option<&str>,
    attempt: F,
) -> Result<computer_use_ai_sdk::InputOutcome, String>
where
    F: Fn(bool) -> Result<Option<computer_use_ai_sdk::InputOutcome>, String>,
{
    use crate::input_control::{commands::describe_request, request_physical_cursor};

    // Every input step funnels through here, so this is the one place that has
    // to notice Accessibility is missing. Onboarding no longer demands it, so
    // the first time Juno reaches for it may well be right now.
    crate::permission_gate::require(
        app_handle,
        crate::permission_gate::Capability::Accessibility,
        action,
    )
    .await?;

    let background = crate::input_control::background_mode_enabled(app_handle).await;

    if !background {
        return attempt(true)?.ok_or_else(|| {
            format!(
                "'{}' could not be performed even with the physical cursor",
                action
            )
        });
    }

    if let Some(outcome) = attempt(false)? {
        tracing::info!(
            "✨ Background {} via {} ({})",
            action,
            outcome.method,
            outcome.tier.as_str()
        );
        return Ok(outcome);
    }

    let named_app = consent_target_app(target_app);
    let request = describe_request(action, named_app.as_deref());
    let grant = request_physical_cursor(app_handle, request)
        .await
        .map_err(|denied| denied.agent_message(action))?;

    // The grant announces the takeover for as long as it lives, so it is held
    // across the retry and dropped the moment the step is done.
    let result = attempt(true);
    drop(grant);

    result?.ok_or_else(|| {
        format!(
            "'{}' could not be performed even with the physical cursor",
            action
        )
    })
}

/// Add the tier that carried an action to a tool response, so the agent (and the
/// audit trail) can see whether the user's cursor was involved.
fn with_input_tier(mut response: Value, outcome: &computer_use_ai_sdk::InputOutcome) -> Value {
    response["input_tier"] = json!(outcome.tier.as_str());
    response["input_method"] = json!(outcome.method);
    response
}

/// Show the on-screen click marker for an action that no longer routes through
/// `commands::mouse`, so the background path looks the same to the user as the
/// physical one did.
fn emit_click_visualization(app_handle: &tauri::AppHandle, x: f64, y: f64, color: &str) {
    if let Err(e) = app_handle.emit(
        crate::constants::events::ui::CLICK_VISUALIZATION,
        (x, y, color),
    ) {
        tracing::debug!("Failed to emit click visualization: {}", e);
    }
}

/// Show the on-screen keystroke marker, for the same reason.
fn emit_key_visualization(app_handle: &tauri::AppHandle, key: &str, modifier: Option<&str>) {
    let payload = json!({ "key": key, "modifier": modifier });
    if let Err(e) = app_handle.emit(
        crate::constants::events::ui::KEY_PRESS_VISUALIZATION,
        payload,
    ) {
        tracing::debug!("Failed to emit key press visualization: {}", e);
    }
}

/// Emit a preview event BEFORE a coordinate-based computer use action executes.
///
/// The desktop-cursor-overlay window listens for this event to show a targeting
/// highlight ring at the click/move position, giving the user visual feedback
/// about WHERE the agent is about to act before it acts.
///
/// Only fires for actions that carry a coordinate (clicks, move, scroll, drag).
/// Read-only actions (screenshot, cursor_position) and text actions (key, type) are skipped.
fn emit_computer_use_preview(app_handle: &tauri::AppHandle, action: &str, input: &Value) {
    let coords: Option<(f64, f64)> = match action {
        "left_click" | "right_click" | "middle_click" | "double_click" | "triple_click"
        | "mouse_move" | "left_mouse_down" | "left_mouse_up" | "scroll" => input["coordinate"]
            .as_array()
            .and_then(|arr| Some((arr.first()?.as_f64()?, arr.get(1)?.as_f64()?))),
        "left_click_drag" => {
            // Show preview at the drag start position.
            // start_coordinate takes precedence; fall back to coordinate (which is
            // the start when end_coordinate is the destination).
            let start = input["start_coordinate"]
                .as_array()
                .or_else(|| input["coordinate"].as_array());
            start.and_then(|arr| Some((arr.first()?.as_f64()?, arr.get(1)?.as_f64()?)))
        }
        _ => None,
    };

    let Some((x, y)) = coords else { return };
    let (screen_x, screen_y) = coordinates::transform_to_screen_coordinates(x, y);

    let payload = json!({
        "action": action,
        "coordinate": [screen_x, screen_y],
        "timestamp": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_else(|_| std::time::Duration::from_secs(0))
            .as_millis() as u64,
    });

    if let Err(e) = app_handle.emit(
        crate::constants::events::tools::COMPUTER_USE_PREVIEW,
        &payload,
    ) {
        tracing::debug!("Failed to emit computer-use-preview: {}", e);
    }
}

// --- Security and Validation Helpers ---

/// Security configuration for text editor operations
struct SecurityConfig {
    max_file_size: usize,
    allowed_extensions: Vec<&'static str>,
    allow_absolute_paths: bool,
}

impl SecurityConfig {
    fn default() -> Self {
        Self {
            max_file_size: 10 * 1024 * 1024, // 10MB in production
            allowed_extensions: vec![
                "txt", "md", "rs", "js", "ts", "py", "java", "c", "cpp", "h", "hpp", "css", "html",
                "xml", "json", "yaml", "yml", "toml", "cfg", "ini", "sh", "bat", "ps1", "sql",
                "go", "rb", "php", "swift", "kt", "scala",
            ],
            allow_absolute_paths: false,
        }
    }

    fn development_mode() -> Self {
        Self {
            max_file_size: 50 * 1024 * 1024, // 50MB in development
            allowed_extensions: vec![
                "txt", "md", "rs", "js", "ts", "py", "java", "c", "cpp", "h", "hpp", "css", "html",
                "xml", "json", "yaml", "yml", "toml", "cfg", "ini", "sh", "bat", "ps1", "sql",
                "go", "rb", "php", "swift", "kt", "scala", "log", "out", "err", "tmp",
            ],
            allow_absolute_paths: true,
        }
    }
}

/// Validates file path for security concerns
///
/// Beyond the fast string checks, the path is canonicalized (resolving `..`
/// segments and symlinks) and must resolve inside an allowed workspace root:
/// the process working directory or the agent's `~/Juno` output directory.
/// This closes the bypass where absolute paths like `/etc/anything` passed
/// the string-only checks (security audit 2026-02-08, items #13/#14).
fn validate_file_path(path: &str, config: &SecurityConfig) -> Result<PathBuf, String> {
    // Check for path traversal attempts
    if path.contains("../") || path.contains("..\\") {
        return Err("Path traversal not allowed".to_string());
    }

    // Check for home directory access (unless allowed)
    if path.starts_with("~/") && !config.allow_absolute_paths {
        return Err("Home directory access not allowed".to_string());
    }

    let path_buf = PathBuf::from(path);

    // Validate file extension if it's a file
    if let Some(extension) = path_buf.extension() {
        let ext_str = extension.to_string_lossy().to_lowercase();
        if !config.allowed_extensions.contains(&ext_str.as_str()) {
            return Err(format!("File extension '{}' not allowed", ext_str));
        }
    }

    // Canonicalize and enforce the workspace boundary (shared helper, same
    // roots as basic_tools plus ~/Juno). Fails closed if no root resolves.
    crate::agent::tools::path_security::resolve_within_default_roots(path)
}

/// Validates file size against security limits
fn validate_file_size(path: &Path, config: &SecurityConfig) -> Result<(), String> {
    match fs::metadata(path) {
        Ok(metadata) => {
            let size = metadata.len() as usize;
            if size > config.max_file_size {
                return Err(format!(
                    "File size {} bytes exceeds limit of {} bytes",
                    size, config.max_file_size
                ));
            }
            Ok(())
        }
        Err(_) => Ok(()), // File doesn't exist yet, that's fine
    }
}

/// Adds line numbers to file content for display
fn add_line_numbers(content: &str) -> String {
    content
        .lines()
        .enumerate()
        .map(|(i, line)| format!("{}: {}", i + 1, line))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Extracts specific line range from content
fn extract_line_range(
    content: &str,
    start_line: usize,
    end_line: Option<usize>,
) -> Result<String, String> {
    let lines: Vec<&str> = content.lines().collect();
    let total_lines = lines.len();

    if start_line == 0 {
        return Err("Line numbers are 1-indexed, start_line cannot be 0".to_string());
    }

    let start_idx = start_line - 1; // Convert to 0-indexed
    if start_idx >= total_lines {
        return Err(format!(
            "Start line {} exceeds file length of {} lines",
            start_line, total_lines
        ));
    }

    let end_idx = match end_line {
        Some(0) => return Err("Line numbers are 1-indexed, end_line cannot be 0".to_string()),
        Some(end) => {
            let end_idx = end;
            if end_idx > total_lines {
                return Err(format!(
                    "End line {} exceeds file length of {} lines",
                    end_idx, total_lines
                ));
            }
            end_idx
        }
        None => total_lines, // None means end of file
    };

    if start_idx >= end_idx {
        return Err("Start line must be less than end line".to_string());
    }

    let selected_lines = &lines[start_idx..end_idx];
    let numbered_content = selected_lines
        .iter()
        .enumerate()
        .map(|(i, line)| format!("{}: {}", start_idx + i + 1, line))
        .collect::<Vec<_>>()
        .join("\n");

    Ok(numbered_content)
}

/// Preserves original line ending style when writing files
#[allow(clippy::if_same_then_else)]
fn detect_line_ending(content: &str) -> &'static str {
    if content.contains("\r\n") {
        "\r\n"
    } else if content.contains('\n') {
        "\n"
    } else {
        "\n" // Default to LF for new files
    }
}

/// Generate a descriptive tool name based on the computer action
fn get_descriptive_tool_name(action: &str, input: &Value) -> String {
    match action {
        "screenshot" => "computer/screenshot".to_string(),
        "cursor_position" => "computer/get_cursor_position".to_string(),
        "mouse_move" => {
            if let Some(coord) = input["coordinate"].as_array() {
                format!(
                    "computer/move_to({}, {})",
                    coord[0].as_f64().unwrap_or(0.0) as i32,
                    coord[1].as_f64().unwrap_or(0.0) as i32
                )
            } else {
                "computer/mouse_move".to_string()
            }
        }
        "left_click" => {
            if let Some(coord) = input["coordinate"].as_array() {
                format!(
                    "computer/click({}, {})",
                    coord[0].as_f64().unwrap_or(0.0) as i32,
                    coord[1].as_f64().unwrap_or(0.0) as i32
                )
            } else {
                "computer/left_click".to_string()
            }
        }
        "right_click" => {
            if let Some(coord) = input["coordinate"].as_array() {
                format!(
                    "computer/right_click({}, {})",
                    coord[0].as_f64().unwrap_or(0.0) as i32,
                    coord[1].as_f64().unwrap_or(0.0) as i32
                )
            } else {
                "computer/right_click".to_string()
            }
        }
        "middle_click" => {
            if let Some(coord) = input["coordinate"].as_array() {
                format!(
                    "computer/middle_click({}, {})",
                    coord[0].as_f64().unwrap_or(0.0) as i32,
                    coord[1].as_f64().unwrap_or(0.0) as i32
                )
            } else {
                "computer/middle_click".to_string()
            }
        }
        "double_click" => {
            if let Some(coord) = input["coordinate"].as_array() {
                format!(
                    "computer/double_click({}, {})",
                    coord[0].as_f64().unwrap_or(0.0) as i32,
                    coord[1].as_f64().unwrap_or(0.0) as i32
                )
            } else {
                "computer/double_click".to_string()
            }
        }
        "triple_click" => {
            if let Some(coord) = input["coordinate"].as_array() {
                format!(
                    "computer/triple_click({}, {})",
                    coord[0].as_f64().unwrap_or(0.0) as i32,
                    coord[1].as_f64().unwrap_or(0.0) as i32
                )
            } else {
                "computer/triple_click".to_string()
            }
        }
        "left_click_drag" => {
            if let Some(start) = input["start_coordinate"].as_array() {
                if let Some(end) = input["coordinate"].as_array() {
                    format!(
                        "computer/drag({},{} → {},{})",
                        start[0].as_f64().unwrap_or(0.0) as i32,
                        start[1].as_f64().unwrap_or(0.0) as i32,
                        end[0].as_f64().unwrap_or(0.0) as i32,
                        end[1].as_f64().unwrap_or(0.0) as i32
                    )
                } else {
                    "computer/left_click_drag".to_string()
                }
            } else if let Some(end) = input["end_coordinate"].as_array() {
                if let Some(start) = input["coordinate"].as_array() {
                    format!(
                        "computer/drag({},{} → {},{})",
                        start[0].as_f64().unwrap_or(0.0) as i32,
                        start[1].as_f64().unwrap_or(0.0) as i32,
                        end[0].as_f64().unwrap_or(0.0) as i32,
                        end[1].as_f64().unwrap_or(0.0) as i32
                    )
                } else {
                    "computer/left_click_drag".to_string()
                }
            } else if let Some(coord) = input["coordinate"].as_array() {
                format!(
                    "computer/drag(cursor → {},{})",
                    coord[0].as_f64().unwrap_or(0.0) as i32,
                    coord[1].as_f64().unwrap_or(0.0) as i32
                )
            } else {
                "computer/left_click_drag".to_string()
            }
        }
        "left_mouse_down" => {
            if let Some(coord) = input["coordinate"].as_array() {
                format!(
                    "computer/mouse_down({}, {})",
                    coord[0].as_f64().unwrap_or(0.0) as i32,
                    coord[1].as_f64().unwrap_or(0.0) as i32
                )
            } else {
                "computer/left_mouse_down".to_string()
            }
        }
        "left_mouse_up" => {
            if let Some(coord) = input["coordinate"].as_array() {
                format!(
                    "computer/mouse_up({}, {})",
                    coord[0].as_f64().unwrap_or(0.0) as i32,
                    coord[1].as_f64().unwrap_or(0.0) as i32
                )
            } else {
                "computer/left_mouse_up".to_string()
            }
        }
        "scroll" => {
            let direction = input["scroll_direction"].as_str().unwrap_or("up");
            let amount = input["scroll_amount"].as_i64().unwrap_or(3);
            if let Some(coord) = input["coordinate"].as_array() {
                format!(
                    "computer/scroll_{}({},{} × {})",
                    direction,
                    coord[0].as_f64().unwrap_or(0.0) as i32,
                    coord[1].as_f64().unwrap_or(0.0) as i32,
                    amount
                )
            } else {
                format!("computer/scroll_{} × {}", direction, amount)
            }
        }
        "type" => {
            let text = input["text"].as_str().unwrap_or("");
            if text.chars().count() > 30 {
                format!(
                    "computer/type(\"{}...\")",
                    text.chars().take(27).collect::<String>()
                )
            } else {
                format!("computer/type(\"{}\")", text)
            }
        }
        "key" => {
            let key = input["text"].as_str().unwrap_or("");
            format!("computer/press_key({})", key)
        }
        "hold_key" => {
            let key = input["key"]
                .as_str()
                .or_else(|| input["text"].as_str())
                .unwrap_or("");
            match resolve_hold_key_duration_ms(input) {
                Ok(duration_ms) => format!("computer/hold_key({}, {}ms)", key, duration_ms),
                Err(_) => format!("computer/hold_key({})", key),
            }
        }
        "wait" => {
            // Read the same two keys, in the same order and as the same type,
            // as the handler that actually sleeps. This read only `duration`
            // and only `as_u64()`, so `{"seconds": 5}` was labelled "wait(1s)"
            // and so was `{"duration": 1.5}` — the label disagreed with what
            // the agent had just done, which is how a unit bug hides.
            let seconds = input
                .get("seconds")
                .and_then(Value::as_f64)
                .or_else(|| input.get("duration").and_then(Value::as_f64))
                .unwrap_or(1.0);
            format!("computer/wait({}s)", seconds)
        }
        "zoom" => {
            if let Some(region) = input["region"].as_array() {
                if region.len() == 4 {
                    format!(
                        "computer/zoom([{},{},{},{}])",
                        region[0].as_i64().unwrap_or(0),
                        region[1].as_i64().unwrap_or(0),
                        region[2].as_i64().unwrap_or(0),
                        region[3].as_i64().unwrap_or(0)
                    )
                } else {
                    "computer/zoom".to_string()
                }
            } else {
                "computer/zoom".to_string()
            }
        }
        _ => format!("computer/{}", action),
    }
}

impl From<CoordinateValidationError> for String {
    fn from(error: CoordinateValidationError) -> String {
        error.to_string()
    }
}

/// Helper function to check if a JSON Value contains an Anthropic Computer Use API error
/// Returns true if the value contains { "is_error": true, ... }
pub fn is_anthropic_error_response(value: &Value) -> bool {
    value
        .get("is_error")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
}

/// Helper function to extract error message from Anthropic error response
/// Returns the error message if this is an error response, None otherwise
pub fn extract_anthropic_error_message(value: &Value) -> Option<String> {
    if is_anthropic_error_response(value) {
        value
            .get("error")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    } else {
        None
    }
}

/// Anthropic's computer tool caps `hold_key` at 300 seconds.
pub const MAX_HOLD_KEY_MS: u64 = 300_000;

/// Resolve a `hold_key` duration to milliseconds.
///
/// The unit depends on which parameter the model sent, because the two request
/// paths see two different schemas:
///
/// - `duration` — Anthropic's canonical computer-tool schema, in **seconds**
///   (max 300). The direct Anthropic API path uses this schema because built-in
///   tools are serialized as `ApiTool::BuiltIn`, which sends no `description`
///   and no `input_schema` — the model never sees Juno's wording. Reading this
///   as milliseconds turned a 2-second hold into 2ms, a silent no-op.
///   It also makes `hold_key` consistent with `wait`, which already reads
///   `duration` as seconds.
/// - `duration_ms` — Juno's own schema, in **milliseconds**. This is what the
///   Claude CLI path sees, since MCP tools carry their own schema.
///
/// `duration_ms` wins when both are present. Returns `Err` with a
/// model-readable message when the value is missing, non-numeric, or not
/// greater than zero; values above the 300-second cap are clamped.
pub fn resolve_hold_key_duration_ms(input: &Value) -> Result<u64, String> {
    let millis = match input.get("duration_ms").and_then(Value::as_f64) {
        Some(ms) => ms,
        None => match input.get("duration").and_then(Value::as_f64) {
            Some(seconds) => seconds * 1000.0,
            None => {
                return Err(
                    "Missing hold duration: provide 'duration' in seconds (max 300), or 'duration_ms' in milliseconds"
                        .to_string(),
                )
            }
        },
    };

    if !millis.is_finite() || millis <= 0.0 {
        return Err(
            "Invalid hold duration: must be greater than zero ('duration' is in seconds, 'duration_ms' is in milliseconds)"
                .to_string(),
        );
    }

    // Float-to-int casts saturate in Rust, so an absurd value lands on u64::MAX
    // and is then clamped by `min` rather than wrapping.
    Ok((millis.round() as u64).min(MAX_HOLD_KEY_MS))
}

/// Convert error messages to Anthropic Computer Use API compliant format
/// According to Anthropic's specification, errors should be returned as successful JSON responses
/// with is_error: true and error: "message" instead of using Rust's Err() pattern
fn create_anthropic_error_response(error_message: String) -> Value {
    json!({
        "is_error": true,
        "error": error_message
    })
}

/// Helper macro to convert Result<T, E> to proper Anthropic format
/// This ensures all tools follow the same error handling pattern
/// Works with any error type that can be converted to String
macro_rules! handle_anthropic_result {
    ($result:expr) => {
        match $result {
            Ok(value) => value,
            Err(error_msg) => return Ok(create_anthropic_error_response(error_msg.to_string())),
        }
    };
}

// --- Main computer tool execution function ---

/// Execute computer tool
///
/// `session_id` identifies the parallel agent session issuing the action
/// (LAC-1432). It attributes physical input in the arbiter and drives the
/// session's "current action" shown in the roster/switcher UI. Legacy
/// callers without a session pass `None`.
pub async fn execute_computer_tool(
    app_handle: &tauri::AppHandle,
    input: Value,
    session_id: Option<&str>,
) -> Result<Value, String> {
    let action = match input["action"].as_str() {
        Some(action) => action,
        None => {
            return Ok(create_anthropic_error_response(
                "Missing 'action' parameter".to_string(),
            ))
        }
    };

    let state_manager = app_handle.state::<AppState>();

    // Generate descriptive tool name for better logging
    let descriptive_tool_name = get_descriptive_tool_name(action, &input);

    // Enhanced logging with descriptive tool name and action details
    info!("🖥️ Computer Use: {} → {}", descriptive_tool_name, action);

    // Record this session's current action so the roster/switcher UI can
    // show what each parallel agent is doing right now.
    if let Some(session_id) = session_id {
        let registry = state_manager.agent_sessions();
        let id = crate::agents::AgentSessionId::from(session_id.to_string());
        if let Some(session) = registry.get(&id).await {
            session
                .set_current_action(Some(descriptive_tool_name.clone()))
                .await;
            crate::agents::broadcast_sessions_updated(app_handle, &registry).await;
        }
    }

    // Log enhanced tool call request with descriptive name
    crate::agent::tool_logger::log_enhanced_tool_call_request(
        app_handle,
        &descriptive_tool_name,
        input.clone(),
        Some(format!("Executing computer action: {}", action)),
        Some(&*state_manager),
    )
    .await;

    // Enforce cooldown between rapid UI actions to prevent "clicked too fast" failures
    enforce_action_cooldown(action).await;

    // Serialize coordinate-based physical input across parallel sessions
    // (LAC-1432). macOS has one hardware pointer, so CGEvent-based actions
    // from different sessions must not interleave. Actions listed here are
    // ALWAYS physical; the click/type actions that attempt AX-grounded
    // interaction first acquire the guard inside their physical fallback
    // blocks instead, so AX-only actions keep running in parallel.
    let always_physical = matches!(
        action,
        "middle_click"
            | "triple_click"
            | "left_click_drag"
            | "mouse_move"
            | "left_mouse_down"
            | "left_mouse_up"
            | "key"
            | "hold_key"
            | "scroll"
    );
    let _physical_input_guard = if always_physical {
        Some(state_manager.input_arbiter().acquire(session_id).await)
    } else {
        None
    };

    // --- Safety checks ---
    // 1. Self-automation prevention + blocked app check
    if let Err(blocked_msg) = check_app_safety(action) {
        return Ok(create_anthropic_error_response(blocked_msg));
    }

    // 2. Sensitive content detection (for audit logging)
    let sensitive_pattern = detect_sensitive_content(&input);
    if let Some(pattern) = sensitive_pattern {
        info!(
            "⚠️ Sensitive action detected: '{}' contains pattern '{}' — logged to audit",
            action, pattern
        );
    }

    // 3. Emit audit log event for frontend action history
    let target_app = get_frontmost_app_name();
    emit_action_audit(
        app_handle,
        action,
        &input,
        target_app.as_deref(),
        sensitive_pattern,
    );

    // 4. Emit preview event so the desktop overlay can highlight the target position
    //    BEFORE the action fires. Fires only for coordinate-based actions.
    emit_computer_use_preview(app_handle, action, &input);

    // Execute action
    let execution_start = std::time::Instant::now();
    let result = match action {
        "screenshot" => {
            // Use the pre-captured PTT screenshot if available (parallelized at PTT release).
            // Falls back to a fresh capture if none is cached or serialization fails.
            let app_state = app_handle.state::<crate::state::AppState>();
            let pre_captured = app_state
                .take_pending_ptt_screenshot()
                .await
                .and_then(|s| serde_json::to_value(s).ok());

            if let Some(value) = pre_captured {
                info!("[Computer Use] Using pre-captured PTT screenshot (saved capture latency)");
                Ok::<Value, String>(value)
            } else {
                // Validate screen recording permission
                handle_anthropic_result!(validate_permission(
                    app_handle,
                    RequiredPermission::ScreenRecording,
                    "computer (screenshot)"
                )
                .await
                .map_err(|e: AgentError| format!("Permission validation failed: {}", e)));

                let screenshot_result =
                    handle_anthropic_result!(crate::commands::core::capture_screenshot_command(
                        app_handle.clone(),
                        state_manager.clone()
                    )
                    .await
                    .map_err(|e| format!("Screenshot failed: {}", e)));

                // The result is a struct with the screenshot data and dimensions.
                // Serialize it to a JSON value to return to the agent.
                match serde_json::to_value(screenshot_result) {
                    Ok(value) => Ok::<Value, String>(value),
                    Err(e) => Ok::<Value, String>(create_anthropic_error_response(format!(
                        "Failed to serialize screenshot data: {}",
                        e
                    ))),
                }
            }
        }
        "left_click" | "right_click" | "middle_click" | "double_click" | "triple_click"
        | "left_click_drag" | "mouse_move" | "left_mouse_down" | "left_mouse_up" => {
            // Validate accessibility permission for mouse operations
            handle_anthropic_result!(validate_permission(
                app_handle,
                RequiredPermission::Accessibility,
                &format!("computer ({})", action)
            )
            .await
            .map_err(|e: AgentError| format!("Permission validation failed: {}", e)));

            // Extract modifier key from `text` parameter (Anthropic API spec).
            // When present on click/scroll actions, `text` holds a modifier key name
            // (shift, ctrl, alt, super) to be held during the action.
            let modifier = input["text"]
                .as_str()
                .filter(|t| {
                    matches!(
                        *t,
                        "shift" | "ctrl" | "alt" | "super" | "command" | "cmd" | "meta" | "option"
                    )
                })
                .map(|m| m.to_string());

            match action {
                "left_click" => {
                    // Strict coordinate validation per Anthropic Computer Use API specification
                    let coordinate = handle_anthropic_result!(validate_coordinate_parameter(
                        &input,
                        "coordinate"
                    ));
                    let (x, y) = coordinate.to_f64();

                    // Transform coordinates from scaled screenshot to screen coordinates
                    let (screen_x, screen_y) = coordinates::transform_to_screen_coordinates(x, y);

                    // Try AX-grounded click first (uses AXPress on the element under the cursor).
                    // If it succeeds we skip the coordinate click. Modifier keys force coordinate
                    // path because AXPress doesn't accept modifiers.
                    let ax_result = if modifier.is_none() {
                        try_ax_grounded_click(app_handle, screen_x, screen_y, AxClickKind::Left)
                    } else {
                        AxGroundingResult {
                            used_ax_click: false,
                            role: None,
                            label: None,
                        }
                    };
                    emit_ax_grounding_audit(app_handle, action, screen_x, screen_y, &ax_result);

                    let mut outcome = None;
                    if !ax_result.used_ax_click {
                        // Physical fallback — serialize with other sessions' input.
                        let _guard = state_manager.input_arbiter().acquire(session_id).await;
                        // Process-targeted injection (SkyLight → CGEventPostToPid) bypasses AX
                        // and works on canvas, games, Chromium web content and non-AX apps.
                        // Only if that fails does the shared cursor come into it.
                        outcome = Some(handle_anthropic_result!(
                            run_background_first(
                                app_handle,
                                action,
                                target_app.as_deref(),
                                |allow_physical| {
                                    state_manager.desktop.left_click_no_warp(
                                        screen_x,
                                        screen_y,
                                        modifier.as_deref(),
                                        allow_physical,
                                    )
                                },
                            )
                            .await
                        ));
                    }

                    let mut response =
                        json!({ "success": true, "ax_grounded": ax_result.used_ax_click });
                    if let Some(outcome) = &outcome {
                        response = with_input_tier(response, outcome);
                    }
                    if let Some(role) = &ax_result.role {
                        response["ax_role"] = json!(role);
                    }
                    if let Some(label) = &ax_result.label {
                        response["ax_label"] = json!(label);
                    }
                    Ok(response)
                }
                "right_click" => {
                    // Strict coordinate validation per Anthropic Computer Use API specification
                    let coordinate = handle_anthropic_result!(validate_coordinate_parameter(
                        &input,
                        "coordinate"
                    ));
                    let (x, y) = coordinate.to_f64();

                    // Transform coordinates from scaled screenshot to screen coordinates
                    let (screen_x, screen_y) = coordinates::transform_to_screen_coordinates(x, y);

                    let ax_result = if modifier.is_none() {
                        try_ax_grounded_click(app_handle, screen_x, screen_y, AxClickKind::Right)
                    } else {
                        AxGroundingResult {
                            used_ax_click: false,
                            role: None,
                            label: None,
                        }
                    };
                    emit_ax_grounding_audit(app_handle, action, screen_x, screen_y, &ax_result);

                    let mut outcome = None;
                    if !ax_result.used_ax_click {
                        // Physical fallback — serialize with other sessions' input.
                        let _guard = state_manager.input_arbiter().acquire(session_id).await;
                        outcome = Some(handle_anthropic_result!(
                            run_background_first(
                                app_handle,
                                action,
                                target_app.as_deref(),
                                |allow_physical| {
                                    state_manager.desktop.right_click_no_warp(
                                        screen_x,
                                        screen_y,
                                        allow_physical,
                                    )
                                },
                            )
                            .await
                        ));
                    }

                    let mut response =
                        json!({ "success": true, "ax_grounded": ax_result.used_ax_click });
                    if let Some(outcome) = &outcome {
                        response = with_input_tier(response, outcome);
                    }
                    if let Some(role) = &ax_result.role {
                        response["ax_role"] = json!(role);
                    }
                    if let Some(label) = &ax_result.label {
                        response["ax_label"] = json!(label);
                    }
                    Ok(response)
                }
                "middle_click" => {
                    // Strict coordinate validation per Anthropic Computer Use API specification
                    let coordinate = handle_anthropic_result!(validate_coordinate_parameter(
                        &input,
                        "coordinate"
                    ));
                    let (x, y) = coordinate.to_f64();

                    // Transform coordinates from scaled screenshot to screen coordinates
                    let (screen_x, screen_y) = coordinates::transform_to_screen_coordinates(x, y);

                    emit_click_visualization(app_handle, screen_x, screen_y, "#FFFF00");
                    let outcome = handle_anthropic_result!(
                        run_background_first(
                            app_handle,
                            action,
                            target_app.as_deref(),
                            |allow_physical| {
                                state_manager.desktop.middle_click_no_warp(
                                    screen_x,
                                    screen_y,
                                    modifier.as_deref(),
                                    allow_physical,
                                )
                            },
                        )
                        .await
                    );

                    Ok(with_input_tier(json!({ "success": true }), &outcome))
                }
                "double_click" => {
                    // Strict coordinate validation per Anthropic Computer Use API specification
                    let coordinate = handle_anthropic_result!(validate_coordinate_parameter(
                        &input,
                        "coordinate"
                    ));
                    let (x, y) = coordinate.to_f64();

                    // Transform coordinates from scaled screenshot to screen coordinates
                    let (screen_x, screen_y) = coordinates::transform_to_screen_coordinates(x, y);

                    let ax_result = if modifier.is_none() {
                        try_ax_grounded_click(app_handle, screen_x, screen_y, AxClickKind::Double)
                    } else {
                        AxGroundingResult {
                            used_ax_click: false,
                            role: None,
                            label: None,
                        }
                    };
                    emit_ax_grounding_audit(app_handle, action, screen_x, screen_y, &ax_result);

                    let mut outcome = None;
                    if !ax_result.used_ax_click {
                        // Physical fallback — serialize with other sessions' input.
                        let _guard = state_manager.input_arbiter().acquire(session_id).await;
                        outcome = Some(handle_anthropic_result!(
                            run_background_first(
                                app_handle,
                                action,
                                target_app.as_deref(),
                                |allow_physical| {
                                    state_manager.desktop.double_click_no_warp(
                                        screen_x,
                                        screen_y,
                                        modifier.as_deref(),
                                        allow_physical,
                                    )
                                },
                            )
                            .await
                        ));
                    }

                    let mut response =
                        json!({ "success": true, "ax_grounded": ax_result.used_ax_click });
                    if let Some(outcome) = &outcome {
                        response = with_input_tier(response, outcome);
                    }
                    if let Some(role) = &ax_result.role {
                        response["ax_role"] = json!(role);
                    }
                    if let Some(label) = &ax_result.label {
                        response["ax_label"] = json!(label);
                    }
                    Ok(response)
                }
                "triple_click" => {
                    // Strict coordinate validation per Anthropic Computer Use API specification
                    let coordinate = handle_anthropic_result!(validate_coordinate_parameter(
                        &input,
                        "coordinate"
                    ));
                    let (x, y) = coordinate.to_f64();

                    // Transform coordinates from scaled screenshot to screen coordinates
                    let (screen_x, screen_y) = coordinates::transform_to_screen_coordinates(x, y);

                    emit_click_visualization(app_handle, screen_x, screen_y, "#800080");
                    let outcome = handle_anthropic_result!(
                        run_background_first(
                            app_handle,
                            action,
                            target_app.as_deref(),
                            |allow_physical| {
                                state_manager.desktop.triple_click_no_warp(
                                    screen_x,
                                    screen_y,
                                    modifier.as_deref(),
                                    allow_physical,
                                )
                            },
                        )
                        .await
                    );

                    Ok(with_input_tier(json!({ "success": true }), &outcome))
                }
                "left_click_drag" => {
                    // Proper Anthropic Computer Use API specification compliance
                    // Support both single coordinate (standard) and dual coordinate formats
                    let (start_x, start_y, end_x, end_y) = if input
                        .get("start_coordinate")
                        .is_some()
                    {
                        // Format: start_coordinate + coordinate (end) - explicit start/end coordinates
                        let (start_coord, end_coord) = handle_anthropic_result!(
                            validate_coordinate_pair(&input, "start_coordinate", "coordinate")
                        );
                        let (start_x, start_y) = start_coord.to_f64();
                        let (end_x, end_y) = end_coord.to_f64();
                        (start_x, start_y, end_x, end_y)
                    } else if input.get("end_coordinate").is_some() {
                        // Format: coordinate (start) + end_coordinate - explicit start/end coordinates
                        let (start_coord, end_coord) = handle_anthropic_result!(
                            validate_coordinate_pair(&input, "coordinate", "end_coordinate")
                        );
                        let (start_x, start_y) = start_coord.to_f64();
                        let (end_x, end_y) = end_coord.to_f64();
                        (start_x, start_y, end_x, end_y)
                    } else {
                        // Standard format: single coordinate (end position) - drag from current cursor position
                        // This is the official Anthropic Computer Use API specification behavior
                        let end_coord = handle_anthropic_result!(validate_coordinate_parameter(
                            &input,
                            "coordinate"
                        ));
                        let (end_x, end_y) = end_coord.to_f64();

                        // Get current cursor position as start point (already in screen coordinates)
                        let (start_x, start_y) =
                            handle_anthropic_result!(crate::commands::mouse::get_cursor_position(
                                app_handle.clone(),
                                state_manager.clone(),
                            )
                            .await
                            .map_err(|e| format!("Failed to get cursor position for drag: {}", e)));

                        // Transform only the end coordinates since start coordinates are already screen coordinates
                        let (screen_end_x, screen_end_y) =
                            coordinates::transform_to_screen_coordinates(end_x, end_y);

                        // Return start coordinates as-is (already screen coordinates) and transformed end coordinates
                        (start_x, start_y, screen_end_x, screen_end_y)
                    };

                    // Transform coordinates from scaled screenshot to screen coordinates (only for explicit coordinate cases)
                    let (screen_start_x, screen_start_y, screen_end_x, screen_end_y) =
                        if input.get("start_coordinate").is_some()
                            || input.get("end_coordinate").is_some()
                        {
                            // Both coordinates need transformation for explicit coordinate cases
                            let (screen_start_x, screen_start_y) =
                                coordinates::transform_to_screen_coordinates(start_x, start_y);
                            let (screen_end_x, screen_end_y) =
                                coordinates::transform_to_screen_coordinates(end_x, end_y);
                            (screen_start_x, screen_start_y, screen_end_x, screen_end_y)
                        } else {
                            // For cursor position case, coordinates are already handled above
                            (start_x, start_y, end_x, end_y)
                        };

                    let outcome = handle_anthropic_result!(
                        run_background_first(
                            app_handle,
                            action,
                            target_app.as_deref(),
                            |allow_physical| {
                                state_manager.desktop.left_click_drag_no_warp(
                                    screen_start_x,
                                    screen_start_y,
                                    screen_end_x,
                                    screen_end_y,
                                    allow_physical,
                                )
                            },
                        )
                        .await
                    );

                    Ok(with_input_tier(json!({ "success": true }), &outcome))
                }
                "mouse_move" => {
                    // Strict coordinate validation per Anthropic Computer Use API specification
                    let coordinate = handle_anthropic_result!(validate_coordinate_parameter(
                        &input,
                        "coordinate"
                    ));
                    let (x, y) = coordinate.to_f64();

                    // Transform coordinates from scaled screenshot to screen coordinates
                    let (screen_x, screen_y) = coordinates::transform_to_screen_coordinates(x, y);

                    // A move has no effect on the target app on its own: it only
                    // says where the agent is looking. In background mode that is
                    // Juno's own cursor, drawn by the overlay from the preview and
                    // agent-cursor events already emitted for this action, and the
                    // user's pointer is left exactly where they put it.
                    if crate::input_control::background_mode_enabled(app_handle).await {
                        Ok(json!({
                            "success": true,
                            "input_tier": computer_use_ai_sdk::InputTier::Accessibility.as_str(),
                            "virtual_cursor_only": true
                        }))
                    } else {
                        handle_anthropic_result!(crate::commands::mouse::mouse_move(
                            app_handle.clone(),
                            state_manager,
                            screen_x,
                            screen_y
                        )
                        .await
                        .map_err(|e| format!("Mouse move failed: {}", e)));

                        Ok(json!({
                            "success": true
                        }))
                    }
                }
                "left_mouse_down" => {
                    // Strict coordinate validation per Anthropic Computer Use API specification
                    let coordinate = handle_anthropic_result!(validate_coordinate_parameter(
                        &input,
                        "coordinate"
                    ));
                    let (x, y) = coordinate.to_f64();

                    // Transform coordinates from scaled screenshot to screen coordinates
                    let (screen_x, screen_y) = coordinates::transform_to_screen_coordinates(x, y);

                    let outcome = handle_anthropic_result!(
                        run_background_first(
                            app_handle,
                            action,
                            target_app.as_deref(),
                            |allow_physical| {
                                state_manager.desktop.left_mouse_down_no_warp(
                                    screen_x,
                                    screen_y,
                                    allow_physical,
                                )
                            },
                        )
                        .await
                    );

                    Ok(with_input_tier(json!({ "success": true }), &outcome))
                }
                "left_mouse_up" => {
                    // Strict coordinate validation per Anthropic Computer Use API specification
                    let coordinate = handle_anthropic_result!(validate_coordinate_parameter(
                        &input,
                        "coordinate"
                    ));
                    let (x, y) = coordinate.to_f64();

                    // Transform coordinates from scaled screenshot to screen coordinates
                    let (screen_x, screen_y) = coordinates::transform_to_screen_coordinates(x, y);

                    let outcome = handle_anthropic_result!(
                        run_background_first(
                            app_handle,
                            action,
                            target_app.as_deref(),
                            |allow_physical| {
                                state_manager.desktop.left_mouse_up_no_warp(
                                    screen_x,
                                    screen_y,
                                    allow_physical,
                                )
                            },
                        )
                        .await
                    );

                    Ok(with_input_tier(json!({ "success": true }), &outcome))
                }
                _ => unreachable!("Mouse action already matched in outer pattern"),
            }
        }
        "key" | "hold_key" | "type" => {
            // Validate accessibility permission for keyboard operations
            handle_anthropic_result!(validate_permission(
                app_handle,
                RequiredPermission::Accessibility,
                &format!("computer ({})", action)
            )
            .await
            .map_err(|e: AgentError| format!("Permission validation failed: {}", e)));

            match action {
                "key" => {
                    // Support both 'key' and 'text' parameters for backward compatibility
                    let key = match input["key"].as_str().or_else(|| input["text"].as_str()) {
                        Some(key) => key,
                        None => {
                            return Ok(create_anthropic_error_response(
                                "Missing 'key' or 'text' parameter".to_string(),
                            ))
                        }
                    };

                    emit_key_visualization(app_handle, key, None);

                    // Keyboard events posted to a process are only routed there
                    // once the WindowServer's input focus has been pointed at it
                    // without raising it, which is what the no-warp path does.
                    let outcome = handle_anthropic_result!(
                        run_background_first(
                            app_handle,
                            action,
                            target_app.as_deref(),
                            |allow_physical| {
                                state_manager
                                    .desktop
                                    .press_key_no_warp(key, None, allow_physical)
                            },
                        )
                        .await
                    );

                    Ok(with_input_tier(json!({ "success": true }), &outcome))
                }
                "hold_key" => {
                    // Support both 'key' and 'text' parameters for backward compatibility
                    let key = match input["key"].as_str().or_else(|| input["text"].as_str()) {
                        Some(key) => key,
                        None => {
                            return Ok(create_anthropic_error_response(
                                "Missing 'key' or 'text' parameter".to_string(),
                            ))
                        }
                    };

                    // 'duration' is SECONDS (Anthropic's canonical schema, which the
                    // direct API path uses); 'duration_ms' is milliseconds (Juno's
                    // own schema, which the MCP/Claude CLI path sees).
                    let duration_ms = match resolve_hold_key_duration_ms(&input) {
                        Ok(duration_ms) => duration_ms,
                        Err(message) => return Ok(create_anthropic_error_response(message)),
                    };

                    emit_key_visualization(
                        app_handle,
                        &format!("Hold: {}", key),
                        Some(&format!("{}ms", duration_ms)),
                    );
                    let outcome = handle_anthropic_result!(
                        run_background_first(
                            app_handle,
                            action,
                            target_app.as_deref(),
                            |allow_physical| {
                                state_manager.desktop.hold_key_no_warp(
                                    key,
                                    Some(duration_ms),
                                    allow_physical,
                                )
                            },
                        )
                        .await
                    );

                    Ok(with_input_tier(json!({ "success": true }), &outcome))
                }
                "type" => {
                    let text = match input["text"].as_str() {
                        Some(text) => text,
                        None => {
                            return Ok(create_anthropic_error_response(
                                "Missing 'text' parameter".to_string(),
                            ))
                        }
                    };

                    // Try AX-based typing first: finds the focused element and sets AXValue
                    // directly (no cursor movement, no clipboard). Falls back to global
                    // keyboard simulation (clipboard paste) when AX isn't supported.
                    let typed_via_ax = try_ax_type_focused(app_handle, text);
                    let mut outcome = None;
                    if !typed_via_ax {
                        // Fallback is clipboard + Cmd+V. In background mode the
                        // paste is posted to the process the agent is working on
                        // after pointing input focus at it without raising it, so
                        // it cannot land in whatever app the user is looking at.
                        let _guard = state_manager.input_arbiter().acquire(session_id).await;
                        let preview: String = text
                            .chars()
                            .take(
                                crate::constants::ui::text_display::MAX_KEYPRESS_VISUALIZATION_TEXT_LENGTH,
                            )
                            .collect();
                        emit_key_visualization(app_handle, &format!("Type: {}", preview), None);
                        outcome = Some(handle_anthropic_result!(
                            run_background_first(
                                app_handle,
                                action,
                                target_app.as_deref(),
                                |allow_physical| {
                                    state_manager
                                        .desktop
                                        .type_text_no_warp(text, allow_physical)
                                },
                            )
                            .await
                        ));
                    }

                    let mut response = json!({
                        "success": true,
                        "ax_grounded": typed_via_ax
                    });
                    if let Some(outcome) = &outcome {
                        response = with_input_tier(response, outcome);
                    }
                    Ok(response)
                }
                _ => unreachable!("Keyboard action already matched in outer pattern"),
            }
        }
        "scroll" => {
            // Validate accessibility permission for scroll operations
            handle_anthropic_result!(validate_permission(
                app_handle,
                RequiredPermission::Accessibility,
                "computer (scroll)"
            )
            .await
            .map_err(|e: AgentError| format!("Permission validation failed: {}", e)));

            // Strict coordinate validation per Anthropic Computer Use API specification
            let coordinate =
                handle_anthropic_result!(validate_coordinate_parameter(&input, "coordinate"));
            let (x, y) = coordinate.to_f64();

            let scroll_direction = match input["scroll_direction"].as_str() {
                Some(direction) => direction,
                None => {
                    return Ok(create_anthropic_error_response(
                        "Missing 'scroll_direction' parameter".to_string(),
                    ))
                }
            };
            let scroll_amount = input["scroll_amount"].as_u64().unwrap_or(3);

            // Transform coordinates from scaled screenshot to screen coordinates
            let (screen_x, screen_y) = coordinates::transform_to_screen_coordinates(x, y);

            // Modifiers ride on `text` for scroll the same way they do for clicks.
            let scroll_modifier = input["text"]
                .as_str()
                .filter(|t| {
                    matches!(
                        *t,
                        "shift" | "ctrl" | "alt" | "super" | "command" | "cmd" | "meta" | "option"
                    )
                })
                .map(|m| m.to_string());

            let outcome = handle_anthropic_result!(
                run_background_first(
                    app_handle,
                    action,
                    target_app.as_deref(),
                    |allow_physical| {
                        state_manager.desktop.scroll_no_warp(
                            screen_x,
                            screen_y,
                            scroll_direction,
                            scroll_amount as f64,
                            scroll_modifier.as_deref(),
                            allow_physical,
                        )
                    },
                )
                .await
            );

            Ok(with_input_tier(json!({ "success": true }), &outcome))
        }
        "zoom" => {
            // Zoom action (computer_20251124): inspect a specific screen region at native resolution
            // This is critical for Retina displays where the standard resolution downscaling
            // makes small text and UI elements unreadable.
            //
            // Claude sends region: [x0, y0, x1, y1] in API (standard resolution) coordinate space.
            // We scale to screen coordinates, capture a full-res screenshot, crop to the region,
            // and return the crop WITHOUT downscaling (native Retina resolution).
            handle_anthropic_result!(validate_permission(
                app_handle,
                RequiredPermission::ScreenRecording,
                "computer (zoom)"
            )
            .await
            .map_err(|e: AgentError| format!("Permission validation failed: {}", e)));

            let region = match input["region"].as_array() {
                Some(arr) if arr.len() == 4 => {
                    let coords: Result<Vec<i64>, _> = arr
                        .iter()
                        .map(|v| {
                            v.as_i64().ok_or_else(|| {
                                "Zoom region coordinates must be integers".to_string()
                            })
                        })
                        .collect();
                    handle_anthropic_result!(coords)
                }
                Some(arr) => {
                    return Ok(create_anthropic_error_response(format!(
                        "Zoom region must have exactly 4 coordinates [x0, y0, x1, y1], got {}",
                        arr.len()
                    )))
                }
                None => {
                    return Ok(create_anthropic_error_response(
                        "Missing 'region' parameter for zoom action".to_string(),
                    ))
                }
            };

            let (api_x0, api_y0, api_x1, api_y1) = (region[0], region[1], region[2], region[3]);

            // Validate region bounds
            if api_x0 < 0 || api_y0 < 0 || api_x1 < 0 || api_y1 < 0 {
                return Ok(create_anthropic_error_response(
                    "Zoom region coordinates must be non-negative".to_string(),
                ));
            }
            if api_x0 >= api_x1 || api_y0 >= api_y1 {
                return Ok(create_anthropic_error_response(format!(
                    "Invalid zoom region: top-left ({},{}) must be before bottom-right ({},{})",
                    api_x0, api_y0, api_x1, api_y1
                )));
            }

            // Capture screenshot (already scaled to standard resolution by capture_screenshot_command)
            let screenshot_result =
                handle_anthropic_result!(crate::commands::core::capture_screenshot_command(
                    app_handle.clone(),
                    state_manager.clone()
                )
                .await
                .map_err(|e| format!("Zoom screenshot capture failed: {}", e)));

            // Decode the base64 screenshot for cropping
            use base64::Engine;
            use image::ImageFormat;
            use std::io::Cursor;
            let engine = base64::engine::general_purpose::STANDARD;
            let image_data = handle_anthropic_result!(engine
                .decode(&screenshot_result.base64_image)
                .map_err(|e| format!("Failed to decode screenshot for zoom: {}", e)));

            let img = handle_anthropic_result!(image::load_from_memory(&image_data)
                .map_err(|e| format!("Failed to load screenshot image for zoom: {}", e)));

            // The screenshot was already resized to standard resolution by capture_screenshot_command.
            // We need to map the API coordinates to the screenshot pixel space.
            // Since capture_screenshot_command resizes to standard_width x standard_height,
            // and the API coordinates ARE in standard resolution space, we can crop directly
            // using the API coordinates (they map 1:1 to screenshot pixels).
            let crop_x = api_x0.max(0) as u32;
            let crop_y = api_y0.max(0) as u32;
            let crop_w = ((api_x1 - api_x0) as u32).min(img.width().saturating_sub(crop_x));
            let crop_h = ((api_y1 - api_y0) as u32).min(img.height().saturating_sub(crop_y));

            if crop_w == 0 || crop_h == 0 {
                return Ok(create_anthropic_error_response(
                    "Zoom region results in zero-size crop after bounds clamping".to_string(),
                ));
            }

            // Crop the image to the specified region
            let cropped = img.crop_imm(crop_x, crop_y, crop_w, crop_h);

            // Encode cropped region as PNG (no downscaling — return at native resolution)
            let mut png_buffer = Cursor::new(Vec::new());
            handle_anthropic_result!(cropped
                .write_to(&mut png_buffer, ImageFormat::Png)
                .map_err(|e| format!("Failed to encode zoomed region: {}", e)));

            let zoomed_base64 = engine.encode(png_buffer.into_inner());

            info!("Zoom action: API region [{},{},{},{}] → crop {}x{} at ({},{}) from {}x{} screenshot",
                api_x0, api_y0, api_x1, api_y1, crop_w, crop_h, crop_x, crop_y, img.width(), img.height());

            Ok(json!({
                "base64_image": zoomed_base64,
                "region": [api_x0, api_y0, api_x1, api_y1],
                "crop_width": crop_w,
                "crop_height": crop_h
            }))
        }
        "cursor_position" => {
            // No permission validation needed for cursor position query
            let (x, y) = handle_anthropic_result!(crate::commands::mouse::get_cursor_position(
                app_handle.clone(),
                state_manager,
            )
            .await
            .map_err(|e| format!("Get cursor position failed: {}", e)));

            Ok(json!({
                "coordinate": [x, y]
            }))
        }
        "wait" => {
            // No permission validation needed for wait operation
            // Support both 'seconds' and 'duration' parameters for backward compatibility
            let seconds = match input["seconds"]
                .as_f64()
                .or_else(|| input["duration"].as_f64())
            {
                Some(seconds) => seconds,
                None => {
                    return Ok(create_anthropic_error_response(
                        "Missing 'seconds' or 'duration' parameter".to_string(),
                    ))
                }
            };

            handle_anthropic_result!(crate::commands::core::wait(
                seconds,
                app_handle.clone(),
                state_manager.clone(),
            )
            .await
            .map_err(|e| format!("Wait failed: {}", e)));

            Ok(json!({
                "success": true
            }))
        }
        _ => Ok(create_anthropic_error_response(format!(
            "Unknown action: {}",
            action
        ))),
    };

    // Calculate execution time
    let execution_time_ms = execution_start.elapsed().as_millis() as u64;

    // Determine if execution was successful - handle Anthropic Computer Use API error format
    let success = match &result {
        Ok(output) => !is_anthropic_error_response(output),
        Err(_) => false,
    };

    // Get screenshot from result if applicable AND operation was successful
    let screenshot_base64 = if success && action == "screenshot" {
        match &result {
            Ok(output) => output.as_str().map(|s| s.to_string()),
            Err(_) => None,
        }
    } else {
        None
    };

    // Enhanced result logging with descriptive name and execution time
    let result_content = if success {
        Some(format!(
            "✅ {} completed successfully in {}ms",
            descriptive_tool_name, execution_time_ms
        ))
    } else {
        let error_msg = match &result {
            Ok(output) => extract_anthropic_error_message(output)
                .unwrap_or_else(|| "Unknown error".to_string()),
            Err(e) => e.clone(),
        };
        Some(format!(
            "❌ {} failed: {}",
            descriptive_tool_name, error_msg
        ))
    };

    crate::agent::tool_logger::log_enhanced_tool_call_result_with_inputs(
        app_handle,
        &descriptive_tool_name,
        Some(input.clone()),
        result.as_ref().unwrap_or(&json!({})).clone(),
        success,
        result_content,
        screenshot_base64,
        Some(execution_time_ms),
        Some(&*app_handle.state::<AppState>()),
    )
    .await;

    result
}

/// Execute bash tool - Anthropic Computer Use API compliant
pub async fn execute_bash_tool(
    app_handle: &tauri::AppHandle,
    input: Value,
) -> Result<Value, String> {
    let command = match input["command"].as_str() {
        Some(command) => command,
        None => {
            return Ok(create_anthropic_error_response(
                "Missing 'command' parameter".to_string(),
            ))
        }
    };

    // Handle restart parameter if provided (Anthropic Computer Use API requirement)
    let restart = input["restart"].as_bool().unwrap_or(false);

    let state_manager = app_handle.state::<AppState>();

    // Use the Anthropic-compliant bash command execution - NO STRING COMPARISONS
    let result = handle_anthropic_result!(crate::commands::shell::bash_command(
        app_handle.clone(),
        state_manager,
        command.to_string(),
        None,          // timeout_seconds
        Some(restart), // restart parameter
        None,          // debug_mode
    )
    .await
    .map_err(|e| format!("Bash command failed: {}", e)));

    // Log the result for debugging
    info!("Anthropic compliant bash result: {:?}", result);

    // Handle structured result - NO STRING COMPARISONS NEEDED
    match result {
        crate::commands::shell::BashResult::Restarted => {
            // Tool was restarted - return official Anthropic message
            Ok(json!({
                "output": "tool has been restarted."
            }))
        }
        crate::commands::shell::BashResult::Output(output) => {
            // Regular output
            Ok(json!({
                "output": output
            }))
        }
        crate::commands::shell::BashResult::CommandResult { output, success } => {
            // Command execution result with exit code information
            let exit_code = if success { 0 } else { 1 };
            Ok(json!({
                "output": output,
                "exit_code": exit_code
            }))
        }
    }
}

/// Execute str_replace_based_edit_tool
pub async fn execute_str_replace_tool(
    _app_handle: &tauri::AppHandle,
    input: Value,
) -> Result<Value, String> {
    let command = match input["command"].as_str() {
        Some(command) => command,
        None => {
            return Ok(create_anthropic_error_response(
                "Missing 'command' parameter".to_string(),
            ))
        }
    };

    let path = match input["path"].as_str() {
        Some(path) => path,
        None => {
            return Ok(create_anthropic_error_response(
                "Missing 'path' parameter".to_string(),
            ))
        }
    };

    // Get security config based on debug mode
    let config = if cfg!(debug_assertions) {
        SecurityConfig::development_mode()
    } else {
        SecurityConfig::default()
    };

    match command {
        "view" => {
            // Validate file path
            let file_path = handle_anthropic_result!(validate_file_path(path, &config));
            handle_anthropic_result!(validate_file_size(&file_path, &config));

            // Handle view_range if provided
            if let (Some(start), end) = (
                input["view_range"]
                    .as_array()
                    .and_then(|arr| arr.first())
                    .and_then(|v| v.as_u64()),
                input["view_range"]
                    .as_array()
                    .and_then(|arr| arr.get(1))
                    .and_then(|v| v.as_u64()),
            ) {
                let start_line = start as usize;
                let end_line = end.map(|e| e as usize);

                // Read file content through a boundary-verified handle (#28)
                let roots = crate::agent::tools::path_security::default_workspace_roots();
                let content = handle_anthropic_result!(
                    crate::agent::tools::path_security::read_to_string_checked(&file_path, &roots)
                );

                let range_content =
                    handle_anthropic_result!(extract_line_range(&content, start_line, end_line));

                Ok(json!({
                    "content": range_content,
                    "view_range": [start_line, end_line.unwrap_or(content.lines().count())]
                }))
            } else {
                // Read entire file through a boundary-verified handle (#28)
                let roots = crate::agent::tools::path_security::default_workspace_roots();
                let content = handle_anthropic_result!(
                    crate::agent::tools::path_security::read_to_string_checked(&file_path, &roots)
                );

                let numbered_content = add_line_numbers(&content);

                Ok(json!({
                    "content": numbered_content
                }))
            }
        }
        "str_replace" => {
            let old_str = match input["old_str"].as_str() {
                Some(old_str) => old_str,
                None => {
                    return Ok(create_anthropic_error_response(
                        "Missing 'old_str' parameter".to_string(),
                    ))
                }
            };
            let new_str = match input["new_str"].as_str() {
                Some(new_str) => new_str,
                None => {
                    return Ok(create_anthropic_error_response(
                        "Missing 'new_str' parameter".to_string(),
                    ))
                }
            };

            // Validate file path
            let file_path = handle_anthropic_result!(validate_file_path(path, &config));
            handle_anthropic_result!(validate_file_size(&file_path, &config));

            // Read file content through a boundary-verified handle (#28)
            let roots = crate::agent::tools::path_security::default_workspace_roots();
            let content = handle_anthropic_result!(
                crate::agent::tools::path_security::read_to_string_checked(&file_path, &roots)
            );

            // Check if old_str exists in file
            if !content.contains(old_str) {
                return Ok(create_anthropic_error_response(format!(
                    "String '{}' not found in file '{}'",
                    old_str, path
                )));
            }

            // Detect original line ending style
            let original_line_ending = detect_line_ending(&content);

            // Normalize the replacement text to match the original file's line ending style
            let normalized_new_str = if original_line_ending == "\r\n" {
                // If original uses CRLF, normalize replacement text to use CRLF
                new_str.replace("\r\n", "\n").replace('\n', "\r\n")
            } else {
                // If original uses LF, normalize replacement text to use LF
                new_str.replace("\r\n", "\n")
            };

            // Perform replacement with normalized replacement text
            let new_content = content.replace(old_str, &normalized_new_str);

            // Write back through a boundary-verified handle: the handle's
            // real path is re-checked before truncation, closing the
            // canonicalize-then-open TOCTOU gap (#28)
            handle_anthropic_result!(crate::agent::tools::path_security::write_checked(
                &file_path,
                new_content.as_bytes(),
                &roots
            ));

            Ok(json!({
                "success": true,
                "message": format!("Successfully replaced text in '{}'", path)
            }))
        }
        "create" => {
            let file_content = match input["file_text"].as_str() {
                Some(file_content) => file_content,
                None => {
                    return Ok(create_anthropic_error_response(
                        "Missing 'file_text' parameter".to_string(),
                    ))
                }
            };

            // Validate file path
            let file_path = handle_anthropic_result!(validate_file_path(path, &config));

            // Check if file already exists
            if file_path.exists() {
                return Ok(create_anthropic_error_response(format!(
                    "File '{}' already exists",
                    path
                )));
            }

            // Create parent directories if they don't exist
            if let Some(parent) = file_path.parent() {
                handle_anthropic_result!(fs::create_dir_all(parent)
                    .map_err(|e| format!("Failed to create directories for '{}': {}", path, e)));
            }

            // Write file through a boundary-verified handle; create_new
            // (O_EXCL) never follows a swapped symlink (#28)
            let roots = crate::agent::tools::path_security::default_workspace_roots();
            handle_anthropic_result!(crate::agent::tools::path_security::create_new_checked(
                &file_path,
                file_content.as_bytes(),
                &roots
            ));

            Ok(json!({
                "success": true,
                "message": format!("Successfully created file '{}'", path)
            }))
        }
        _ => Ok(create_anthropic_error_response(format!(
            "Unknown str_replace_based_edit_tool command: {}",
            command
        ))),
    }
}

/// Register all Anthropic Computer Use tools with the provider
/// Create versioned Anthropic Computer Use tools based on API version
///
/// This function creates tools with proper API types and versioning to ensure
/// compliance with the official Anthropic Computer Use specification
pub fn create_versioned_tools(version_config: Option<ToolVersionConfig>) -> Vec<ToolDefinition> {
    let manager = if let Some(config) = version_config {
        ToolVersionManager::with_config(config)
    } else {
        ToolVersionManager::new()
    };

    let mut tools = Vec::new();

    // Computer tool - main screen interaction tool (Official Anthropic Computer Use API)
    let computer_tool = ToolDefinition {
        name: "computer".to_string(),
        description: "Use a computer to complete tasks. This tool gives you access to interact with any desktop application using the mouse and keyboard, take screenshots, and perform various system operations.

The computer tool accepts these actions:
- screenshot: Take a screenshot of the current screen
- left_click: Click at coordinates with left mouse button
- right_click: Click at coordinates with right mouse button
- middle_click: Click at coordinates with middle mouse button
- double_click: Double-click at coordinates
- triple_click: Triple-click at coordinates
- left_click_drag: Drag from start coordinates to end coordinates
- mouse_move: Move mouse to coordinates
- left_mouse_down: Press and hold left mouse button at coordinates
- left_mouse_up: Release left mouse button at coordinates
- key: Press a key (supports modifiers like 'cmd+c', 'ctrl+v', etc.)
- hold_key: Hold a key down for a duration ('duration' in seconds, max 300; or 'duration_ms' in milliseconds)
- type: Type text at current cursor position
- scroll: Scroll at coordinates in specified direction
- cursor_position: Get current mouse cursor position
- wait: Wait for specified number of seconds
- zoom: View a specific screen region at full native resolution (region: [x0, y0, x1, y1])

Coordinates are provided as [x, y] arrays and are automatically transformed from screenshot coordinates to screen coordinates.".to_string(),
        api_type: None, // Will be set by version manager
        beta_flag: None, // Will be set by version manager
        input_schema: json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "description": "The action to perform",
                    "enum": ["screenshot", "left_click", "right_click", "middle_click", "double_click", "triple_click", "left_click_drag", "mouse_move", "left_mouse_down", "left_mouse_up", "key", "hold_key", "type", "scroll", "cursor_position", "wait", "zoom"]
                },
                "coordinate": {
                    "type": "array",
                    "description": "The [x, y] coordinate for mouse actions. For drag operations, this is the end coordinate (drag starts from current cursor position)",
                    "items": {"type": "number"}
                },
                "key": {
                    "type": "string",
                    "description": "The key to press (supports modifiers like 'cmd+c'). Preferred parameter name."
                },
                "text": {
                    "type": "string",
                    "description": "Text to type, or key to press (backward compatibility for key action)"
                },
                "duration_ms": {
                    "type": "number",
                    "description": "Duration in MILLISECONDS for hold_key action. Preferred parameter name when you want sub-second precision."
                },
                "duration": {
                    "type": "number",
                    "description": "Duration in SECONDS — for hold_key (max 300) and for wait. Use duration_ms instead to give a hold_key duration in milliseconds."
                },
                "scroll_direction": {
                    "type": "string",
                    "description": "Direction to scroll: 'up', 'down', 'left', 'right'"
                },
                "scroll_amount": {
                    "type": "number",
                    "description": "Amount to scroll (default: 3)"
                },
                "seconds": {
                    "type": "number",
                    "description": "Number of seconds to wait. Preferred parameter name."
                },
                "region": {
                    "type": "array",
                    "description": "The [x0, y0, x1, y1] bounding box for zoom action. Coordinates define the top-left and bottom-right corners of the region to inspect at full resolution.",
                    "items": {"type": "integer"}
                }
            },
            "required": ["action"]
        }),
    };

    // Bash tool - command execution (Official Anthropic Computer Use API)
    let bash_tool = ToolDefinition {
        name: "bash".to_string(),
        description: "Execute bash commands on the system. Use this tool to run shell commands, scripts, and interact with the command line.

The tool accepts a 'command' parameter with the bash command to execute.
Returns the command output and exit code.

Example usage:
- List files: {\"command\": \"ls -la\"}
- Check system info: {\"command\": \"uname -a\"}
- Run scripts: {\"command\": \"./script.sh\"}".to_string(),
        api_type: None, // Will be set by version manager
        beta_flag: None, // Will be set by version manager
        input_schema: json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The bash command to execute"
                }
            },
            "required": ["command"]
        }),
    };

    // String replacement based edit tool (Official Anthropic Computer Use API)
    let str_replace_tool = ToolDefinition {
        name: "str_replace_based_edit_tool".to_string(),
        description: "Edit files using string replacement operations. This tool provides safe file editing capabilities with security validation.

Supports these commands:
- view: Read file content with optional line range
- str_replace: Replace exact string matches in files
- create: Create new files with specified content

The tool includes security features:
- Path traversal protection
- File extension validation
- File size limits
- Safe file operations

Example usage:
- View file: {\"command\": \"view\", \"path\": \"file.txt\"}
- View range: {\"command\": \"view\", \"path\": \"file.txt\", \"view_range\": [1, 10]}
- Replace text: {\"command\": \"str_replace\", \"path\": \"file.txt\", \"old_str\": \"old text\", \"new_str\": \"new text\"}
- Create file: {\"command\": \"create\", \"path\": \"new_file.txt\", \"file_text\": \"content\"}".to_string(),
        api_type: None, // Will be set by version manager
        beta_flag: None, // Will be set by version manager
        input_schema: json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The operation to perform",
                    "enum": ["view", "str_replace", "create"]
                },
                "path": {
                    "type": "string",
                    "description": "Path to the file"
                },
                "view_range": {
                    "type": "array",
                    "description": "Optional [start_line, end_line] for view command",
                    "items": {"type": "number"}
                },
                "old_str": {
                    "type": "string",
                    "description": "String to replace (for str_replace command)"
                },
                "new_str": {
                    "type": "string",
                    "description": "Replacement string (for str_replace command)"
                },
                "file_text": {
                    "type": "string",
                    "description": "Content for new file (for create command)"
                }
            },
            "required": ["command", "path"]
        }),
    };

    // Apply versioning to all tools
    tools.push(manager.apply_versioning(computer_tool));
    tools.push(manager.apply_versioning(bash_tool));
    tools.push(manager.apply_versioning(str_replace_tool));

    tools
}

pub async fn register_anthropic_computer_use_tools(
    provider: &mut LocalToolProvider,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    register_anthropic_computer_use_tools_with_version(provider, app_handle, None, None).await
}

/// Register Anthropic Computer Use tools bound to a parallel agent session.
///
/// The session's id becomes the overlay cursor id and its identity color is
/// used for the cursor ring, so the desktop overlay and the roster UI agree
/// on which agent is which (LAC-1432).
pub async fn register_anthropic_computer_use_tools_for_session(
    provider: &mut LocalToolProvider,
    app_handle: tauri::AppHandle,
    session: SessionToolContext,
) -> Result<(), String> {
    register_anthropic_computer_use_tools_with_version(provider, app_handle, None, Some(session))
        .await
}

/// Register Anthropic Computer Use tools with specific API version
pub async fn register_anthropic_computer_use_tools_with_version(
    provider: &mut LocalToolProvider,
    app_handle: tauri::AppHandle,
    version_config: Option<ToolVersionConfig>,
    session: Option<SessionToolContext>,
) -> Result<(), String> {
    let version_info = version_config
        .as_ref()
        .map(|c| format!("{:?}", c.current_version))
        .unwrap_or_else(|| "latest".to_string());

    info!(
        "Registering official Anthropic Computer Use tools (API version: {})...",
        version_info
    );

    // Cursor identity: prefer the parallel-session identity (session id +
    // palette color) so overlay cursors match the roster UI. Legacy callers
    // without a session get a process-unique `agent-N` id and the legacy
    // palette, exactly as before.
    let (agent_cursor_id, agent_cursor_color, session_id) = match session {
        Some(ctx) => (ctx.session_id.clone(), ctx.color, Some(ctx.session_id)),
        None => {
            let cursor_slot = NEXT_AGENT_CURSOR_ID.fetch_add(1, Ordering::Relaxed);
            (
                format!("agent-{}", cursor_slot),
                AGENT_CURSOR_COLORS[(cursor_slot as usize - 1) % AGENT_CURSOR_COLORS.len()]
                    .to_string(),
                None,
            )
        }
    };

    info!(
        "🖱️ Agent cursor ID: {} (color: {})",
        agent_cursor_id, agent_cursor_color
    );

    // Create versioned tools
    let versioned_tools = create_versioned_tools(version_config);
    let tool_count = versioned_tools.len();

    for tool in versioned_tools {
        match tool.name.as_str() {
            "computer" => {
                provider
                    .register_async_tool(tool, {
                        let handle = app_handle.clone();
                        let cursor_id = agent_cursor_id.clone();
                        let cursor_color = agent_cursor_color.clone();
                        let session_id = session_id.clone();
                        move |input: Value| {
                            let handle = handle.clone();
                            let cursor_id = cursor_id.clone();
                            let cursor_color = cursor_color.clone();
                            let session_id = session_id.clone();
                            async move {
                                run_computer_action(
                                    &handle,
                                    input,
                                    session_id.as_deref(),
                                    &cursor_id,
                                    &cursor_color,
                                )
                                .await
                            }
                        }
                    })
                    .await;
            }
            "bash" => {
                provider
                    .register_async_tool(tool, {
                        let handle = app_handle.clone();
                        move |input: Value| {
                            let handle = handle.clone();
                            async move { execute_bash_tool(&handle, input).await }
                        }
                    })
                    .await;
            }
            "str_replace_based_edit_tool" => {
                provider
                    .register_async_tool(tool, {
                        let handle = app_handle.clone();
                        move |input: Value| {
                            let handle = handle.clone();
                            async move { execute_str_replace_tool(&handle, input).await }
                        }
                    })
                    .await;
            }
            _ => {
                warn!("Unknown tool name in versioned tools: {}", tool.name);
            }
        }
    }

    info!(
        "Successfully registered {} official Anthropic Computer Use tools",
        tool_count
    );
    Ok(())
}

#[cfg(test)]
mod hold_key_duration_tests {
    use super::*;

    #[test]
    fn duration_is_seconds() {
        // Anthropic's canonical schema, which the direct API path uses.
        // A 2-second hold must be 2000ms, not 2ms.
        let input = json!({ "action": "hold_key", "key": "shift", "duration": 2 });
        assert_eq!(resolve_hold_key_duration_ms(&input), Ok(2000));
    }

    #[test]
    fn fractional_seconds_are_supported() {
        let input = json!({ "action": "hold_key", "key": "shift", "duration": 0.25 });
        assert_eq!(resolve_hold_key_duration_ms(&input), Ok(250));
    }

    #[test]
    fn duration_ms_is_milliseconds() {
        // Juno's own schema, which the MCP / Claude CLI path sees.
        let input = json!({ "action": "hold_key", "key": "shift", "duration_ms": 2000 });
        assert_eq!(resolve_hold_key_duration_ms(&input), Ok(2000));
    }

    #[test]
    fn duration_ms_wins_over_duration() {
        let input = json!({ "key": "shift", "duration_ms": 500, "duration": 30 });
        assert_eq!(resolve_hold_key_duration_ms(&input), Ok(500));
    }

    #[test]
    fn duration_is_clamped_to_the_300_second_maximum() {
        let input = json!({ "key": "shift", "duration": 600 });
        assert_eq!(resolve_hold_key_duration_ms(&input), Ok(MAX_HOLD_KEY_MS));

        let input_ms = json!({ "key": "shift", "duration_ms": 900_000u64 });
        assert_eq!(resolve_hold_key_duration_ms(&input_ms), Ok(MAX_HOLD_KEY_MS));

        // A float-to-int cast saturates rather than wrapping, so an absurd value
        // still lands on the cap.
        let absurd = json!({ "key": "shift", "duration": 1e30 });
        assert_eq!(resolve_hold_key_duration_ms(&absurd), Ok(MAX_HOLD_KEY_MS));
    }

    #[test]
    fn missing_duration_is_an_error() {
        let input = json!({ "action": "hold_key", "key": "shift" });
        assert!(resolve_hold_key_duration_ms(&input).is_err());
    }

    #[test]
    fn zero_and_negative_durations_are_errors() {
        // A zero-length hold is a silent no-op; tell the model instead.
        assert!(resolve_hold_key_duration_ms(&json!({ "duration": 0 })).is_err());
        assert!(resolve_hold_key_duration_ms(&json!({ "duration_ms": 0 })).is_err());
        assert!(resolve_hold_key_duration_ms(&json!({ "duration": -1 })).is_err());
    }

    #[test]
    fn non_numeric_duration_is_an_error() {
        assert!(resolve_hold_key_duration_ms(&json!({ "duration": "2" })).is_err());
        assert!(resolve_hold_key_duration_ms(&json!({ "duration": null })).is_err());
    }

    #[test]
    fn error_messages_name_both_units() {
        let message = resolve_hold_key_duration_ms(&json!({ "key": "shift" }))
            .err()
            .unwrap_or_default();
        assert!(message.contains("seconds"));
        assert!(message.contains("milliseconds"));
    }
}

/// The action cooldown must be paced by a clock that only moves forward.
#[cfg(test)]
mod action_cooldown_tests {
    use super::*;

    #[test]
    fn monotonic_now_ms_never_returns_the_sentinel() {
        // `0` means "no action recorded yet". A real reading must never
        // collide with it, or the first action after process start would be
        // treated as "never happened" a second time.
        assert!(monotonic_now_ms() > 0);
    }

    #[test]
    fn monotonic_now_ms_does_not_go_backwards() {
        let first = monotonic_now_ms();
        let second = monotonic_now_ms();
        assert!(
            second >= first,
            "monotonic clock went backwards: {first} then {second}"
        );
    }

    #[tokio::test]
    async fn read_only_actions_are_not_paced() {
        // A screenshot changes nothing on screen, so it must not pay the
        // cooldown.
        let start = std::time::Instant::now();
        enforce_action_cooldown("screenshot").await;
        enforce_action_cooldown("cursor_position").await;
        enforce_action_cooldown("wait").await;
        enforce_action_cooldown("zoom").await;
        assert!(
            start.elapsed() < std::time::Duration::from_millis(ACTION_COOLDOWN_MS),
            "read-only actions slept; they took {:?}",
            start.elapsed()
        );
    }

    #[tokio::test]
    async fn consecutive_ui_actions_are_separated_by_the_cooldown() {
        // Prime the stamp, then time the next one. The gap between the two
        // returns is what a real click-then-type sequence sees.
        enforce_action_cooldown("left_click").await;
        let start = std::time::Instant::now();
        enforce_action_cooldown("type").await;
        let elapsed = start.elapsed();
        // Tolerate the millisecond or two that may pass between the priming
        // call returning and `start` being taken.
        let floor = std::time::Duration::from_millis(ACTION_COOLDOWN_MS)
            .saturating_sub(std::time::Duration::from_millis(10));
        assert!(
            elapsed >= floor,
            "second UI action ran {:?} after the first, cooldown is {}ms",
            elapsed,
            ACTION_COOLDOWN_MS
        );
    }

    #[tokio::test]
    async fn the_cooldown_stamp_is_monotonic_millis_not_epoch_millis() {
        // The regression guard. Epoch millis are ~1.7e12 and climbing; millis
        // since process start are small. If this ever reads like a wall-clock
        // timestamp again, a forward NTP step can make the measured gap look
        // enormous and skip the sleep entirely — the failure this fix removes.
        enforce_action_cooldown("left_click").await;
        let stamp = LAST_UI_ACTION_MS.load(Ordering::Relaxed);
        assert!(stamp > 0, "a UI action must record a stamp");
        assert!(
            stamp < 1_000_000_000,
            "cooldown stamp {stamp} looks like epoch millis, not monotonic millis"
        );
    }
}

/// The label an action gets in the log and the UI has to agree with what the
/// action did. A label that silently rounds or ignores the parameter it is
/// reporting is how a unit bug survives a code review.
#[cfg(test)]
mod descriptive_name_tests {
    use super::*;

    #[test]
    fn wait_reports_the_seconds_key() {
        // The handler prefers `seconds`; the label used to read only
        // `duration`, so every `{"seconds": n}` wait was labelled "wait(1s)".
        assert_eq!(
            get_descriptive_tool_name("wait", &json!({ "seconds": 5 })),
            "computer/wait(5s)"
        );
    }

    #[test]
    fn wait_reports_the_duration_key_when_seconds_is_absent() {
        assert_eq!(
            get_descriptive_tool_name("wait", &json!({ "duration": 3 })),
            "computer/wait(3s)"
        );
    }

    #[test]
    fn wait_reports_fractional_seconds() {
        // `as_u64()` returned None here, so a 1.5-second wait read as 1s.
        assert_eq!(
            get_descriptive_tool_name("wait", &json!({ "seconds": 1.5 })),
            "computer/wait(1.5s)"
        );
    }

    #[test]
    fn wait_prefers_seconds_over_duration_like_the_handler_does() {
        assert_eq!(
            get_descriptive_tool_name("wait", &json!({ "seconds": 2, "duration": 9 })),
            "computer/wait(2s)"
        );
    }

    #[test]
    fn wait_with_no_parameter_falls_back_to_one_second() {
        assert_eq!(
            get_descriptive_tool_name("wait", &json!({})),
            "computer/wait(1s)"
        );
    }

    #[test]
    fn hold_key_is_labelled_in_milliseconds_whichever_key_was_sent() {
        // `duration` is seconds, `duration_ms` is milliseconds, and the label
        // says ms in both cases — so the two spellings cannot be confused by
        // reading a log.
        assert_eq!(
            get_descriptive_tool_name("hold_key", &json!({ "key": "shift", "duration": 2 })),
            "computer/hold_key(shift, 2000ms)"
        );
        assert_eq!(
            get_descriptive_tool_name("hold_key", &json!({ "key": "shift", "duration_ms": 2 })),
            "computer/hold_key(shift, 2ms)"
        );
    }
}
