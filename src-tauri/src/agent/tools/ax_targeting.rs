//! # Accessibility first: acting on an app's real controls
//!
//! The 2026-10-08 Calculator run is why this exists. The agent read button
//! positions off a screenshot, put its grid about one button to the left of
//! the real one, and every click pressed the neighbour of the key it meant:
//! the "1" click landed on the Zed window beside Calculator, the "2" click
//! pressed 1, and so on. Each wrong press came back as a success.
//!
//! An app's accessibility tree already knows where every control is and what
//! it is called. So the agent's first move is to ask for that list
//! (`elements`) and press controls by id; pixels are the last resort, and
//! even then a click aimed at one app is refused if it would land on another.
//!
//! Three pieces of state live here, all reset at the start of each turn:
//!
//! - the **working window**: once the agent names a window, unnamed actions
//!   that follow go there, never to "whatever is on top";
//! - the latest **element listing**, whose ids `left_click`, `right_click` and
//!   `type` accept as `element`;
//! - the **coordinate frame** (in `utils::coordinates`): a screenshot of one
//!   window makes coordinates relative to that window's picture.

use std::collections::HashMap;
use std::sync::Mutex;

use computer_use_ai_sdk::ax_elements::{self, ElementInfo, ElementRef};
use computer_use_ai_sdk::window_target::PinnedWindow;
use once_cell::sync::Lazy;
use serde_json::{json, Value};

use crate::utils::coordinates::{self, CoordinateFrame};

/// The most elements one listing returns. A typical app window has a few
/// dozen controls; the cap keeps a page of web content from costing more
/// tokens than a screenshot would.
pub const MAX_LISTED: usize = 200;

/// Largest window picture sent, in pixels. Matches the full screenshot's
/// standard resolution, so a window never costs more than the whole screen.
const MAX_PICTURE_WIDTH: u32 = 1280;
const MAX_PICTURE_HEIGHT: u32 = 800;

// ── Per-agent state ─────────────────────────────────────────────────────────

/// What one agent is working on. Keyed by agent (see
/// `coordinates::agent_key`), so parallel sessions never steer each other.
#[derive(Default)]
struct AgentTargets {
    working: Option<PinnedWindow>,
    snapshot: Option<Snapshot>,
}

static TARGETS: Lazy<Mutex<HashMap<String, AgentTargets>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

/// Run `f` on the current agent's state. Outside an agent action there is
/// none, and `f` is not run.
fn with_targets<R>(f: impl FnOnce(&mut AgentTargets) -> R) -> Option<R> {
    let key = coordinates::agent_key()?;
    let mut targets = TARGETS.lock().ok()?;
    Some(f(targets.entry(key).or_default()))
}

// ── Working window ──────────────────────────────────────────────────────────

/// Make `pin` the window unnamed actions go to.
pub fn set_working_window(pin: PinnedWindow) {
    with_targets(|t| {
        if t.working != Some(pin) {
            tracing::info!(
                "[AX] Working window is now {} (pid {})",
                pin.window_id,
                pin.pid
            );
        }
        t.working = Some(pin);
    });
}

/// The window the agent is working in, while it is still on screen. A window
/// that has closed is forgotten rather than returned.
pub fn working_window() -> Option<PinnedWindow> {
    let pin = with_targets(|t| t.working).flatten()?;
    if window_on_screen(pin.window_id) {
        Some(pin)
    } else {
        with_targets(|t| t.working = None);
        None
    }
}

/// Forget an agent's working window, element listing and window frame. Runs
/// at the start of every turn so one task's window never steers the next.
pub fn reset_for_new_turn(agent: &str) {
    if let Ok(mut targets) = TARGETS.lock() {
        targets.remove(agent);
        // Each API run has its own session id, so finished runs leave entries
        // behind. Runs are serialized today; keep the map from growing anyway.
        if targets.len() > MAX_REMEMBERED_AGENTS {
            targets.clear();
        }
    }
    coordinates::reset_frame(agent);
}

/// How many agents' targets are kept before old ones are dropped.
const MAX_REMEMBERED_AGENTS: usize = 32;

#[cfg(target_os = "macos")]
fn window_records() -> Vec<computer_use_ai_sdk::window_target::WindowRecord> {
    computer_use_ai_sdk::platforms::macos::display::list_window_records()
}

#[cfg(not(target_os = "macos"))]
fn window_records() -> Vec<computer_use_ai_sdk::window_target::WindowRecord> {
    Vec::new()
}

fn window_on_screen(window_id: u32) -> bool {
    window_records().iter().any(|w| w.id == window_id)
}

/// Why a pinned pointer action would miss its app, or `None` when it lands on
/// one of the app's windows. A miss is refused, never sent: sending it is how
/// the Calculator run typed into the person's editor.
pub fn pinned_point_refusal(pin: PinnedWindow, screen_x: f64, screen_y: f64) -> Option<String> {
    let records = window_records();
    if computer_use_ai_sdk::window_target::app_window_contains(
        &records, pin.pid, screen_x, screen_y,
    ) {
        return None;
    }
    let target = records.iter().find(|w| w.id == pin.window_id);
    let name = target
        .and_then(|w| w.title.clone().filter(|t| !t.is_empty()))
        .or_else(|| target.map(|w| w.owner.clone()))
        .unwrap_or_else(|| format!("window {}", pin.window_id));
    let bounds = target.map(|w| {
        let (x0, y0) = coordinates::screen_to_frame(w.bounds.0, w.bounds.1);
        let (x1, y1) =
            coordinates::screen_to_frame(w.bounds.0 + w.bounds.2, w.bounds.1 + w.bounds.3);
        format!(
            " In the coordinates you are using, it spans x {:.0} to {:.0} and y {:.0} to {:.0}.",
            x0, x1, y0, y1
        )
    });
    Some(format!(
        "That point is not on {name}, the window you are working in, so the click would land on \
         another app. Nothing was clicked.{} Prefer {{\"action\": \"elements\"}} and press a \
         control by its id. To act on a different window, name it with 'window'.",
        bounds.unwrap_or_default()
    ))
}

// ── Element listing ─────────────────────────────────────────────────────────

struct Snapshot {
    pid: i32,
    window_id: u32,
    elements: Vec<(ElementInfo, ElementRef)>,
}

/// `e1`, `e2`, ... for listing positions 0, 1, ...
pub fn element_id(index: usize) -> String {
    format!("e{}", index + 1)
}

/// The listing position an id names. Accepts `e12`, `E12` and a bare `12`.
pub fn parse_element_id(id: &str) -> Option<usize> {
    let digits = id.trim().trim_start_matches(['e', 'E']);
    digits
        .parse::<usize>()
        .ok()
        .filter(|n| *n > 0)
        .map(|n| n - 1)
}

/// One listed element as the agent reads it. `at` is its centre in the
/// coordinates the agent is using, when that frame can express it.
pub fn element_entry(id: &str, info: &ElementInfo, at: Option<(f64, f64)>) -> Value {
    let mut entry = json!({ "id": id, "role": info.role });
    if let Some(name) = &info.name {
        entry["name"] = json!(name);
    }
    if let Some(value) = &info.value {
        entry["value"] = json!(value);
    }
    if !info.enabled {
        entry["enabled"] = json!(false);
    }
    if let Some((x, y)) = at {
        entry["at"] = json!([x.round() as i64, y.round() as i64]);
    }
    entry
}

fn center(frame: (f64, f64, f64, f64)) -> (f64, f64) {
    (frame.0 + frame.2 / 2.0, frame.1 + frame.3 / 2.0)
}

/// A window's controls and text as read from the accessibility tree, before
/// they are handed to the agent.
pub struct ElementListing {
    pin: PinnedWindow,
    elements: Vec<(ElementInfo, ElementRef)>,
    truncated: bool,
    target: computer_use_ai_sdk::ax_text::TargetInfo,
}

/// Read a window's controls and text. Blocking (it walks another app's
/// accessibility tree), so call it from a blocking thread, then hand the
/// result to [`present_elements`] back on the agent's task.
pub fn read_elements(pin: PinnedWindow) -> Result<ElementListing, String> {
    let mut elements = ax_elements::window_elements(pin.pid, pin.window_id, MAX_LISTED + 1)?;
    let truncated = elements.len() > MAX_LISTED;
    elements.truncate(MAX_LISTED);
    let target = computer_use_ai_sdk::ax_text::describe_target(Some(pin.pid), Some(pin.window_id));
    Ok(ElementListing {
        pin,
        elements,
        truncated,
        target,
    })
}

/// Remember a listing for `element` ids, make its window the working window,
/// and describe it for the agent. Runs on the agent's task, where its state
/// lives.
pub fn present_elements(listing: ElementListing) -> Value {
    let ElementListing {
        pin,
        elements,
        truncated,
        target,
    } = listing;

    // Centres are only meaningful in the frame the agent is reading: the
    // screen, or a picture of this same window.
    let frame_fits = match coordinates::current_frame().window() {
        None => true,
        Some((_, id)) => id == pin.window_id,
    };
    let entries: Vec<Value> = elements
        .iter()
        .enumerate()
        .map(|(i, (info, _))| {
            let at = frame_fits.then(|| {
                let (x, y) = center(info.frame);
                coordinates::screen_to_frame(x, y)
            });
            element_entry(&element_id(i), info, at)
        })
        .collect();
    tracing::info!(
        "[AX] Listed {} elements of window {} ({:?})",
        entries.len(),
        pin.window_id,
        target.app
    );

    with_targets(|t| {
        t.snapshot = Some(Snapshot {
            pid: pin.pid,
            window_id: pin.window_id,
            elements,
        })
    });
    set_working_window(pin);

    let mut response = json!({
        "window": {
            "id": pin.window_id,
            "app": target.app,
            "title": target.window_title,
        },
        "elements": entries,
        "how": "Press a control with {\"action\": \"left_click\", \"element\": \"e1\"}. Type into a \
                field with {\"action\": \"type\", \"element\": \"e3\", \"text\": \"...\"}. Read results \
                from the values here (list again after acting) before taking a screenshot. Ids last \
                until the next listing.",
    });
    if entries_is_empty(&response) {
        response["how"] = json!(
            "This window exposes no controls to accessibility (a game, canvas or remote screen). \
             Take a screenshot with this 'window' and click by coordinates instead."
        );
    }
    if truncated {
        response["truncated"] = json!(format!("Only the first {MAX_LISTED} elements are listed."));
    }
    response
}

fn entries_is_empty(response: &Value) -> bool {
    response["elements"]
        .as_array()
        .is_none_or(|elements| elements.is_empty())
}

/// An element from the latest listing, ready to act on.
pub struct ResolvedElement {
    pub info: ElementInfo,
    pub element: ElementRef,
    pub pin: PinnedWindow,
}

impl ResolvedElement {
    /// Its centre in screen points, for the pointer overlay and for the
    /// coordinate fallback when an element refuses `AXPress`.
    pub fn screen_center(&self) -> (f64, f64) {
        center(self.info.frame)
    }

    /// How a result names it: `button "1"`.
    pub fn describe(&self) -> Value {
        let mut described = json!({ "role": self.info.role });
        if let Some(name) = &self.info.name {
            described["name"] = json!(name);
        }
        described
    }
}

/// Look up an `element` id from the latest listing.
pub fn resolve_element(id: &str) -> Result<ResolvedElement, String> {
    let index = parse_element_id(id)
        .ok_or_else(|| format!("'{id}' is not an element id; ids look like \"e12\""))?;
    let found = with_targets(|t| {
        let snapshot = t.snapshot.as_ref().ok_or_else(|| {
            "No elements have been listed yet. Call {\"action\": \"elements\"} first".to_string()
        })?;
        let (info, element) = snapshot.elements.get(index).ok_or_else(|| {
            format!(
                "There is no {id} in the latest listing ({} elements). List again",
                snapshot.elements.len()
            )
        })?;
        Ok(ResolvedElement {
            info: info.clone(),
            element: element.clone(),
            pin: PinnedWindow {
                pid: snapshot.pid,
                window_id: snapshot.window_id,
            },
        })
    });
    found.unwrap_or_else(|| Err("Elements can only be used inside an agent's action".to_string()))
}

// ── Window pictures ─────────────────────────────────────────────────────────

/// The size a window picture is sent at: its size in points, shrunk to fit
/// within the standard resolution, never enlarged.
pub fn picture_size(width: u32, height: u32) -> (u32, u32) {
    if width == 0 || height == 0 {
        return (width, height);
    }
    let fit = (MAX_PICTURE_WIDTH as f64 / width as f64)
        .min(MAX_PICTURE_HEIGHT as f64 / height as f64)
        .min(1.0);
    (
        ((width as f64 * fit).round() as u32).max(1),
        ((height as f64 * fit).round() as u32).max(1),
    )
}

/// A picture of one window and the frame its pixels are measured in.
pub struct WindowPicture {
    pin: PinnedWindow,
    response: Value,
    frame: CoordinateFrame,
}

/// Capture one window on its own, even when other windows cover it.
/// Blocking: call it from a blocking thread, then hand the result to
/// [`adopt_picture`] back on the agent's task.
#[cfg(target_os = "macos")]
pub fn capture_window(pin: PinnedWindow) -> Result<WindowPicture, String> {
    use base64::Engine;
    use image::DynamicImage;
    use std::io::Cursor;

    let (buffer, frame) =
        computer_use_ai_sdk::platforms::macos::utils::capture_window_buffer(pin.window_id)
            .map_err(|e| e.to_string())?;
    let image = DynamicImage::ImageRgba8(buffer);
    let (width, height) = picture_size(image.width(), image.height());
    let image = if (width, height) == (image.width(), image.height()) {
        image
    } else {
        image.resize_exact(width, height, image::imageops::FilterType::Lanczos3)
    };

    let mut jpeg = Cursor::new(Vec::new());
    let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, 85);
    image
        .to_rgb8()
        .write_with_encoder(encoder)
        .map_err(|e| format!("Failed to encode the window picture: {e}"))?;

    let scale = if width > 0 {
        frame.2 / width as f64
    } else {
        1.0
    };
    let target = computer_use_ai_sdk::ax_text::describe_target(Some(pin.pid), Some(pin.window_id));
    let name = target
        .window_title
        .clone()
        .filter(|t| !t.is_empty())
        .or(target.app.clone())
        .unwrap_or_else(|| format!("window {}", pin.window_id));
    tracing::info!(
        "[AX] Window picture of {} ({}x{} px, {:.3} pt/px)",
        name,
        width,
        height,
        scale
    );

    Ok(WindowPicture {
        pin,
        frame: CoordinateFrame::Window {
            pid: pin.pid,
            window_id: pin.window_id,
            origin: (frame.0, frame.1),
            scale,
        },
        response: json!({
            "base64_image": base64::engine::general_purpose::STANDARD.encode(jpeg.into_inner()),
            "original_width": frame.2.round() as u32,
            "original_height": frame.3.round() as u32,
            "resized_width": width,
            "resized_height": height,
            "output": format!(
                "This picture is only the {name} window ({width}x{height}), drawn even if other \
                 windows cover it. Coordinates you send now are pixels in this picture from its \
                 top-left corner, and clicks go to this window. A screenshot without 'window' \
                 switches back to screen coordinates."
            ),
        }),
    })
}

#[cfg(not(target_os = "macos"))]
pub fn capture_window(_pin: PinnedWindow) -> Result<WindowPicture, String> {
    Err("Window pictures are only available on macOS".to_string())
}

/// Make the agent's coordinates relative to a window picture it is about to
/// see, and that window its working window. Runs on the agent's task.
pub fn adopt_picture(picture: WindowPicture) -> Value {
    coordinates::set_frame(picture.frame);
    set_working_window(picture.pin);
    picture.response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn element_ids_round_trip() {
        assert_eq!(element_id(0), "e1");
        assert_eq!(parse_element_id("e1"), Some(0));
        assert_eq!(parse_element_id("E12"), Some(11));
        assert_eq!(parse_element_id(" 3 "), Some(2));
        assert_eq!(parse_element_id("e0"), None);
        assert_eq!(parse_element_id("button"), None);
    }

    #[test]
    fn an_entry_names_the_control_and_says_where_it_is() {
        let info = ElementInfo {
            role: "button".into(),
            name: Some("1".into()),
            value: None,
            enabled: true,
            frame: (368.0, 700.0, 57.0, 48.0),
        };
        let entry = element_entry("e7", &info, Some((352.4, 649.6)));
        assert_eq!(
            entry,
            json!({ "id": "e7", "role": "button", "name": "1", "at": [352, 650] })
        );
    }

    #[test]
    fn a_disabled_control_says_so_and_text_carries_its_value() {
        let info = ElementInfo {
            role: "statictext".into(),
            name: None,
            value: Some("1,035".into()),
            enabled: false,
            frame: (0.0, 0.0, 10.0, 10.0),
        };
        let entry = element_entry("e1", &info, None);
        assert_eq!(entry["value"], json!("1,035"));
        assert_eq!(entry["enabled"], json!(false));
        assert!(entry.get("at").is_none());
    }

    #[test]
    fn a_window_picture_is_never_larger_than_a_screenshot() {
        assert_eq!(picture_size(230, 408), (230, 408));
        assert_eq!(picture_size(2560, 1600), (1280, 800));
        assert_eq!(picture_size(1440, 400), (1280, 356));
        assert_eq!(picture_size(0, 10), (0, 10));
    }

    #[tokio::test]
    async fn an_unknown_id_is_refused_with_how_to_get_one() {
        let err = coordinates::scoped_to_agent(Some("test-unknown-id"), async {
            resolve_element("e1").err().unwrap_or_default()
        })
        .await;
        assert!(err.contains("elements"), "{err}");
    }

    #[tokio::test]
    async fn each_agent_keeps_its_own_working_window() {
        let a = PinnedWindow {
            pid: 10,
            window_id: 1,
        };
        coordinates::scoped_to_agent(Some("test-agent-a"), async move {
            set_working_window(a);
        })
        .await;
        let seen_by_b = coordinates::scoped_to_agent(Some("test-agent-b"), async {
            with_targets(|t| t.working).flatten()
        })
        .await;
        assert_eq!(seen_by_b, None);
        let seen_by_a = coordinates::scoped_to_agent(Some("test-agent-a"), async {
            with_targets(|t| t.working).flatten()
        })
        .await;
        assert_eq!(seen_by_a, Some(a));
        reset_for_new_turn("test-agent-a");
    }

    #[test]
    fn outside_an_agent_action_there_is_no_working_window() {
        assert_eq!(with_targets(|t| t.working), None);
        assert_eq!(coordinates::current_frame(), CoordinateFrame::Screen);
    }
}
