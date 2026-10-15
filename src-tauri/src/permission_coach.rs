//! The help once the person is standing in System Settings.
//!
//! [`crate::permission_gate`] owns the decision to ask, and sends the person to
//! the right pane. This module owns the next five seconds, which until now were
//! nobody's: they are looking at a list of applications, and nothing on screen
//! says which row is Juno's.
//!
//! One card, beside the System Settings window, pointing back at it. It carries
//! Juno's icon, because the task is a visual search through a column of icons,
//! and one line saying what to do with the row once it is found. There is
//! nothing to press and nothing to dismiss.
//!
//! ## Why it does not point at the toggle
//!
//! Because it cannot, and the reason is worth writing down so nobody spends a
//! day on it. Finding the Juno row inside System Settings means reading that
//! application's accessibility tree, and reading another application's
//! accessibility tree needs the Accessibility permission, which is the
//! permission being asked for. An arrow at a guessed row would point at
//! somebody else's application about as often as not, so the card points at the
//! window and names the row in words.
//!
//! Where the *window* is costs nothing: `kCGWindowOwnerName` and
//! `kCGWindowBounds` come out of `CGWindowListCopyWindowInfo` with no
//! permission at all, and only `kCGWindowName`, the title, is gated behind
//! Screen Recording. The card never needs the title.
//!
//! The inversion is the reason Accessibility ships first. For Microphone,
//! Screen Recording and Input Monitoring, Accessibility is usually already on
//! by the time Juno asks, so those panes *can* have an arrow at the exact row
//! later. The one pane that cannot is the one that has to work without it.
//!
//! ## Who owns what
//!
//! Rust decides whether the card exists, where it sits, which way it points and
//! when it leaves. The page renders a card. [`place`] is pure and has its own
//! tests; everything that touches a window is wrapped around it.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, WebviewWindowBuilder};
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

use crate::constants::events;
use crate::constants::ui::window_labels;
use crate::platform::desktop_points::{monitor_rect_points, PointRect};

/// The card, in points. Wide enough for the icon and one line, and no wider: it
/// sits beside a window somebody is reading.
pub const CARD: (f64, f64) = (296.0, 104.0);

/// Between the card and the window, and between the card and the display edge.
const GAP: f64 = 16.0;
const EDGE: f64 = 12.0;

/// A window has to be at least this big to be the System Settings window.
/// Keeps a stray helper window or a tooltip from being treated as the pane.
const SETTINGS_MIN: (f64, f64) = (320.0, 240.0);

/// How often the card checks where System Settings is.
///
/// This is **not** how the grant is noticed. That arrives as an event, through
/// [`note_granted`], off the `permission-granted` the permissions monitor
/// already emits. This timer answers a different question, and one with no
/// event behind it: the person has dragged the System Settings window, or
/// closed it. macOS tells an outside application about neither, so the window
/// list is read instead, which costs about a millisecond and only happens while
/// a card is up.
const FOLLOW_EVERY: Duration = Duration::from_millis(250);

/// How long to wait for System Settings to show up before giving up in silence.
///
/// Nothing is on screen during this wait. A card pointing at a window that
/// never arrived is worse than no card.
const WAIT_FOR_SETTINGS: Duration = Duration::from_secs(8);

/// How long a card may stay up at all. Somebody who walked away mid-task does
/// not come back to Juno still pointing at a window.
const UP_FOR_AT_MOST: Duration = Duration::from_secs(120);

/// How long the card says it worked before it goes.
///
/// It does not simply vanish on the grant. The person flipped a switch in
/// another application and deserves to be told by Juno that Juno saw it;
/// vanishing reads as a glitch, and leaves them wondering whether to flip it
/// again.
const RECEIPT_FOR: Duration = Duration::from_millis(1600);

/// Which side of the System Settings window the card is on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Side {
    Left,
    Right,
    Above,
    Below,
}

impl Side {
    /// Which way the card's arrow points, which is always back at the window.
    pub fn arrow(self) -> &'static str {
        match self {
            Self::Left => "right",
            Self::Right => "left",
            Self::Above => "down",
            Self::Below => "up",
        }
    }
}

/// Where the card goes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placement {
    pub rect: PointRect,
    pub side: Side,
}

/// What the page draws. Every word of it decided here.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoachState {
    /// The permission being coached, so the page can be sure it is current.
    pub permission: String,
    /// The one line. The whole instruction, and nothing about why macOS works
    /// this way.
    pub headline: String,
    /// Which way the arrow points: `left`, `right`, `up` or `down`.
    pub arrow: String,
    /// The grant landed. The card is a receipt for a moment, then it goes.
    pub granted: bool,
    /// What the receipt says, once it is one.
    pub receipt: String,
}

/// One pane Juno knows how to coach.
///
/// A pane is data, not code, so Microphone, Screen Recording, Input Monitoring
/// and Full Disk Access are each a row here and nothing else. See
/// [`pane_for`] for why only one row exists today.
#[derive(Debug, Clone, Copy)]
pub struct CoachPane {
    /// The permission key, matching [`crate::permission_gate::Capability::key`].
    pub permission: &'static str,
    /// The one line of text.
    pub headline: &'static str,
    /// What the card says once the switch is on.
    pub receipt: &'static str,
}

/// The pane Juno coaches for this permission, if it coaches one.
///
/// Accessibility only, deliberately. The other panes are the same code and one
/// row each, and they are not here because each owes its own verification on a
/// real machine and its own answer to a state Accessibility does not have:
/// macOS hands a running process an Accessibility grant live
/// (`AXIsProcessTrustedWithOptions` asks TCC per call), so "flipped it and
/// nothing happened" cannot occur for this pane. It can for the other three,
/// and a card that sat there saying "turn it on" after somebody already had
/// would be worse than no card. See
/// [`crate::permission_gate::restart_is_the_missing_step`].
pub fn pane_for(permission: &str) -> Option<CoachPane> {
    match permission {
        "accessibility" => Some(CoachPane {
            permission: "accessibility",
            headline: "Find Juno in the list and turn its switch on",
            receipt: "Accessibility is on",
        }),
        _ => None,
    }
}

/// Where the card goes, given where System Settings is.
///
/// Tried in order: left of the window, right of it, below, above. The first that
/// fits inside `usable` without touching the window wins. `None` means none of
/// them did, which happens when System Settings has been resized to fill the
/// display, and `None` shows nothing at all: a card over the list is worse than
/// no card, and the person is then exactly where they were before this feature
/// existed.
///
/// Left is tried first because System Settings keeps its application list on
/// the right of its own window, so a card on the left is as far from the row it
/// is talking about as it can be while still touching the window.
///
/// `usable` is the display's work area, so the menu bar and the Dock are
/// already out of it.
pub fn place(settings: PointRect, usable: PointRect, card: (f64, f64)) -> Option<Placement> {
    let (w, h) = card;

    // Beside: level with the top of the window, pulled back inside the display
    // if the window itself is hanging off the bottom of it.
    let beside_y = clamp_into(settings.y, usable.y, usable.y + usable.height, h);
    // Above or below: centred on the window, pulled back the same way.
    let centred_x = clamp_into(
        settings.x + settings.width / 2.0 - w / 2.0,
        usable.x,
        usable.x + usable.width,
        w,
    );

    let candidates = [
        (
            Side::Left,
            PointRect {
                x: settings.x - GAP - w,
                y: beside_y,
                width: w,
                height: h,
            },
        ),
        (
            Side::Right,
            PointRect {
                x: settings.x + settings.width + GAP,
                y: beside_y,
                width: w,
                height: h,
            },
        ),
        (
            Side::Below,
            PointRect {
                x: centred_x,
                y: settings.y + settings.height + GAP,
                width: w,
                height: h,
            },
        ),
        (
            Side::Above,
            PointRect {
                x: centred_x,
                y: settings.y - GAP - h,
                width: w,
                height: h,
            },
        ),
    ];

    candidates
        .into_iter()
        .find(|(_, rect)| fits_in(rect, &usable) && !overlaps(rect, &settings))
        .map(|(side, rect)| Placement { rect, side })
}

/// Hold a `size`-long span starting at `want` inside `[low, high]`, preferring
/// `want` and never returning a start before `low`.
fn clamp_into(want: f64, low: f64, high: f64, size: f64) -> f64 {
    let last_start = (high - EDGE - size).max(low + EDGE);
    want.clamp(low + EDGE, last_start)
}

fn fits_in(rect: &PointRect, usable: &PointRect) -> bool {
    rect.x >= usable.x + EDGE
        && rect.y >= usable.y + EDGE
        && rect.x + rect.width <= usable.x + usable.width - EDGE
        && rect.y + rect.height <= usable.y + usable.height - EDGE
}

/// Touching at all counts as covering. The whole job of the card is to not be
/// over the control it is pointing at.
fn overlaps(a: &PointRect, b: &PointRect) -> bool {
    a.x < b.x + b.width && b.x < a.x + a.width && a.y < b.y + b.height && b.y < a.y + a.height
}

/// The card that is up, if one is.
struct Live {
    /// Which card this is. A card that has been replaced must not be put away
    /// by a timer its predecessor started: the receipt sleeps for a beat before
    /// hiding, and a second ask landing inside that beat would otherwise have
    /// its brand new card hidden out from under it.
    id: u64,
    permission: String,
    /// Stops the follow task. Held so a second ask, or a grant, can end it.
    token: CancellationToken,
    /// The last state handed to the page, so a page that mounts late can ask
    /// for it instead of rendering an empty card.
    state: CoachState,
    /// Which side the card is on, so a tick that did not move it says nothing.
    side: Option<Side>,
}

static LIVE: LazyLock<Mutex<Option<Live>>> = LazyLock::new(|| Mutex::new(None));

/// Hands out [`Live::id`]. Monotonic, so an id is never reused.
static NEXT_ID: AtomicU64 = AtomicU64::new(1);

/// What the page asks for when it mounts, because the event that carries this
/// may have been emitted before the page was listening.
#[tauri::command]
pub async fn permission_coach_state() -> Result<Option<CoachState>, String> {
    Ok(LIVE
        .lock()
        .map_err(|_| "The coach's state lock is poisoned".to_string())?
        .as_ref()
        .map(|live| live.state.clone()))
}

/// Juno has just sent somebody to a Privacy pane. Coach them through it.
///
/// Returns without doing anything for a permission with no pane
/// ([`pane_for`]), which is every permission but Accessibility today, so the
/// call is safe to make from anywhere that opens a pane.
pub fn point_at(app: &AppHandle, permission: &str) {
    let Some(pane) = pane_for(permission) else {
        debug!(
            "No coaching card for '{}'; the pane was opened without one",
            permission
        );
        return;
    };
    let Some((id, token)) = register(pane) else {
        return;
    };

    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        follow(app, id, token).await;
    });
}

/// The switch was flipped. Say so, then go.
///
/// Called from the one place a flip is observed, so the card leaves on an event
/// rather than on a poll of its own.
pub fn note_granted(app: &AppHandle, permission: &str) {
    let Some((id, granted_state)) = mark_granted(permission) else {
        return;
    };

    info!(
        "{} is on; the card says so for a moment and goes",
        permission
    );
    emit_state(app, &granted_state);

    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(RECEIPT_FOR).await;
        // Only this card. A second ask during the receipt put a new card up,
        // and that one is not ours to take down.
        put_away(&app, id);
    });
}

// ── The registry ─────────────────────────────────────────────────────────────
//
// Which card is up, and nothing about windows. Separated from the two functions
// above so the handover rules have tests, because they are where the bugs are.
//
// **Every teardown names the card it is taking down.** A follow task and a
// receipt timer both outlive the card that started them: a second ask cancels
// the token and replaces the card, but a task already past its cancellation
// check still runs to its end. Without the id, that tail hides the card the
// second ask had just put up.

/// Record a new card and hand back its id and its follow token.
///
/// A card already up is replaced rather than raced: its follow task is
/// cancelled first. `None` means the registry could not be written and no card
/// should be put up.
fn register(pane: CoachPane) -> Option<(u64, CancellationToken)> {
    let token = CancellationToken::new();
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let state = CoachState {
        permission: pane.permission.to_string(),
        headline: pane.headline.to_string(),
        // Replaced by the real side the first time the window is found. Nothing
        // is on screen before then.
        arrow: Side::Left.arrow().to_string(),
        granted: false,
        receipt: pane.receipt.to_string(),
    };

    let mut live = LIVE
        .lock()
        .inspect_err(|_| warn!("The coach's state lock is poisoned; not putting a card up"))
        .ok()?;
    if let Some(previous) = live.take() {
        previous.token.cancel();
    }
    *live = Some(Live {
        id,
        permission: pane.permission.to_string(),
        token: token.clone(),
        state,
        side: None,
    });
    Some((id, token))
}

/// Turn the live card into its receipt, if it is the card for `permission`.
///
/// Cancels the follow task before returning: a tick landing during the receipt
/// would move the card, or hide it early.
fn mark_granted(permission: &str) -> Option<(u64, CoachState)> {
    let mut live = LIVE
        .lock()
        .inspect_err(|_| warn!("The coach's state lock is poisoned; no receipt"))
        .ok()?;
    let current = live.as_mut()?;
    if current.permission != permission {
        return None;
    }
    current.token.cancel();
    current.state.granted = true;
    Some((current.id, current.state.clone()))
}

/// Forget the live card, but only if it is still the one `id` names.
///
/// Returns whether it was forgotten, which is also whether the window may be
/// hidden. `false` means a later ask owns the card now. A poisoned lock counts
/// as forgotten, so a card is never left on screen forever.
fn forget(id: u64) -> bool {
    let Ok(mut live) = LIVE.lock() else {
        warn!("The coach's state lock is poisoned; hiding the card anyway");
        return true;
    };
    if live.as_ref().map(|current| current.id) != Some(id) {
        return false;
    }
    if let Some(previous) = live.take() {
        previous.token.cancel();
    }
    true
}

/// Take down the card `id` names, if it is still the card that is up.
fn put_away(app: &AppHandle, id: u64) {
    if !forget(id) {
        return;
    }
    if let Some(window) = app.get_webview_window(window_labels::PERMISSION_COACH) {
        if let Err(e) = window.hide() {
            warn!("Could not put the coaching card away: {}", e);
        }
    }
}

/// Keep card `id` beside the System Settings window for as long as it belongs
/// there, and take it down when it stops belonging.
///
/// Three ways out, and they are the three states the issue asked to be designed
/// rather than defaulted: the window went away (they closed Settings, with or
/// without flipping the switch), the window never arrived, or the card has been
/// up long enough that nobody is still reading it. The fourth way out is the
/// grant, which is [`note_granted`]'s and arrives as an event.
async fn follow(app: AppHandle, id: u64, token: CancellationToken) {
    let started = Instant::now();
    let mut ever_found = false;
    let mut interval = tokio::time::interval(FOLLOW_EVERY);

    loop {
        tokio::select! {
            _ = token.cancelled() => return,
            _ = interval.tick() => {}
        }

        if started.elapsed() > UP_FOR_AT_MOST {
            info!("The coaching card has been up long enough; putting it away");
            put_away(&app, id);
            return;
        }

        match settings_window().await {
            Some(settings) => {
                ever_found = true;
                if let Err(e) = show_beside(&app, id, settings) {
                    warn!("Could not place the coaching card: {}", e);
                }
            }
            None if ever_found => {
                // They closed System Settings. Whether or not they flipped the
                // switch, they are done being coached.
                info!("System Settings has gone; putting the coaching card away");
                put_away(&app, id);
                return;
            }
            None => {
                if started.elapsed() > WAIT_FOR_SETTINGS {
                    info!("System Settings never appeared; no coaching card");
                    put_away(&app, id);
                    return;
                }
            }
        }
    }
}

/// The System Settings window, in points, if it is on screen.
///
/// Matched on the owning application rather than the window title, because the
/// title needs Screen Recording and this must work with no permissions at all.
/// Both names are checked: the application was called System Preferences until
/// Ventura. The largest qualifying window wins, so a sheet or a helper window
/// cannot be mistaken for the pane.
async fn settings_window() -> Option<PointRect> {
    #[cfg(target_os = "macos")]
    {
        let windows = tokio::task::spawn_blocking(
            computer_use_ai_sdk::platforms::macos::display::list_visible_windows,
        )
        .await
        .ok()?
        .ok()?;

        windows
            .into_iter()
            .filter(|w| {
                let owner = w.app_name.as_str();
                owner == "System Settings" || owner == "System Preferences"
            })
            // Already points with a top-left origin, which is the space
            // `PointRect` is in, so nothing is converted here.
            .map(|w| PointRect {
                x: w.position.0,
                y: w.position.1,
                width: w.size.0,
                height: w.size.1,
            })
            .filter(|r| r.width >= SETTINGS_MIN.0 && r.height >= SETTINGS_MIN.1)
            .max_by(|a, b| {
                (a.width * a.height)
                    .partial_cmp(&(b.width * b.height))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    }

    #[cfg(not(target_os = "macos"))]
    {
        None
    }
}

/// Put card `id` beside this window, building and styling it the first time.
fn show_beside(app: &AppHandle, id: u64, settings: PointRect) -> Result<(), String> {
    let window = coach_window(app)?;

    let usable = usable_area_for(app, &settings);
    let Some(placement) = place(settings, usable, CARD) else {
        // System Settings fills the display. Nothing fits beside it, and over
        // it is not an option, so say nothing.
        debug!("No room beside System Settings for the coaching card");
        put_away(app, id);
        return Ok(());
    };

    // The side can change under the person: they drag System Settings to the
    // left edge and the card has to swap to its right. The page is only told
    // when it actually changes.
    let changed_side = match LIVE.lock() {
        Ok(mut live) => match live.as_mut().filter(|current| current.id == id) {
            Some(current) => {
                if current.side == Some(placement.side) {
                    None
                } else {
                    current.side = Some(placement.side);
                    current.state.arrow = placement.side.arrow().to_string();
                    Some(current.state.clone())
                }
            }
            // This card is gone, or a later one replaced it. Either way a
            // cancelled follow got one more tick in, and the window is not
            // this task's to move.
            None => return Ok(()),
        },
        Err(_) => return Err("The coach's state lock is poisoned".to_string()),
    };

    window
        .set_size(LogicalSize::new(CARD.0, CARD.1))
        .map_err(|e| format!("Could not size the coaching card: {}", e))?;
    window
        .set_position(LogicalPosition::new(placement.rect.x, placement.rect.y))
        .map_err(|e| format!("Could not place the coaching card: {}", e))?;

    if let Some(state) = changed_side {
        emit_state(app, &state);
    }

    if !window.is_visible().unwrap_or(false) {
        window
            .show()
            .map_err(|e| format!("Could not show the coaching card: {}", e))?;
    }
    Ok(())
}

/// The work area of the display System Settings is on, in points.
///
/// The work area rather than the full frame, so the menu bar and the Dock are
/// already excluded and the card cannot be placed under either. Falls back to
/// the whole of the first display, and then to nothing placeable, rather than
/// guessing at a frame.
fn usable_area_for(app: &AppHandle, settings: &PointRect) -> PointRect {
    let centre = (
        settings.x + settings.width / 2.0,
        settings.y + settings.height / 2.0,
    );

    let monitors = app.available_monitors().unwrap_or_default();
    let areas: Vec<(PointRect, PointRect)> = monitors
        .iter()
        .map(|m| {
            let scale = m.scale_factor();
            let position = m.position();
            let size = m.size();
            let work = m.work_area();
            (
                monitor_rect_points((position.x, position.y), (size.width, size.height), scale),
                monitor_rect_points(
                    (work.position.x, work.position.y),
                    (work.size.width, work.size.height),
                    scale,
                ),
            )
        })
        .collect();

    areas
        .iter()
        .find(|(frame, _)| frame.contains(centre))
        .or_else(|| areas.first())
        .map(|(_, work)| *work)
        .unwrap_or(PointRect {
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 0.0,
        })
}

/// The card's window, built from its declaration the first time it is needed.
///
/// Normally already built: it is one of the windows `startup_windows` brings up
/// after the bar, so its page is mounted and waiting. Built here for the case
/// where a permission is asked for in the first seconds of a launch.
fn coach_window(app: &AppHandle) -> Result<tauri::WebviewWindow, String> {
    if let Some(window) = app.get_webview_window(window_labels::PERMISSION_COACH) {
        return Ok(window);
    }
    let mut config =
        crate::window_management::declared_window_config(app, window_labels::PERMISSION_COACH)
            .ok_or("the coaching card is not declared in tauri.conf.json")?;
    config.visible = false;
    let window = WebviewWindowBuilder::from_config(app, &config)
        .map_err(|e| e.to_string())?
        .build()
        .map_err(|e| e.to_string())?;
    #[cfg(target_os = "macos")]
    crate::platform::configure_deferred_window(app, window_labels::PERMISSION_COACH);
    Ok(window)
}

fn emit_state(app: &AppHandle, state: &CoachState) {
    if let Err(e) = app.emit(events::permissions::COACH, state) {
        warn!("Could not tell the coaching card what to say: {}", e);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 1512x982 point display with a 24 point menu bar, which is the laptop
    /// most of this will happen on.
    fn laptop() -> PointRect {
        PointRect {
            x: 0.0,
            y: 24.0,
            width: 1512.0,
            height: 958.0,
        }
    }

    /// System Settings at its default size, roughly centred.
    fn settings_centred() -> PointRect {
        PointRect {
            x: 400.0,
            y: 180.0,
            width: 715.0,
            height: 600.0,
        }
    }

    #[test]
    fn the_card_goes_left_when_there_is_room() {
        let p = place(settings_centred(), laptop(), CARD).expect("a centred window has room");
        assert_eq!(p.side, Side::Left);
        // Touching the window's left edge, one gap away.
        assert_eq!(p.rect.x + p.rect.width + GAP, settings_centred().x);
        // Level with the top of the window.
        assert_eq!(p.rect.y, settings_centred().y);
    }

    #[test]
    fn the_card_never_covers_the_window_it_points_at() {
        // Every position System Settings can be dragged to, in steps, on one
        // display. Wherever a card is placed at all, it is not over the pane.
        let usable = laptop();
        let mut placed = 0;
        for x in (-300..1500).step_by(50) {
            for y in (24..900).step_by(50) {
                let settings = PointRect {
                    x: x as f64,
                    y: y as f64,
                    width: 715.0,
                    height: 600.0,
                };
                if let Some(p) = place(settings, usable, CARD) {
                    placed += 1;
                    assert!(
                        !overlaps(&p.rect, &settings),
                        "a card at {:?} covers settings at {:?}",
                        p.rect,
                        settings
                    );
                    assert!(
                        fits_in(&p.rect, &usable),
                        "a card at {:?} is off the display",
                        p.rect
                    );
                }
            }
        }
        assert!(placed > 100, "only {} placements worked at all", placed);
    }

    #[test]
    fn the_card_swaps_to_the_right_against_the_left_edge() {
        let settings = PointRect {
            x: 20.0,
            y: 180.0,
            width: 715.0,
            height: 600.0,
        };
        let p = place(settings, laptop(), CARD).expect("there is room on the right");
        assert_eq!(p.side, Side::Right);
        assert_eq!(p.rect.x, settings.x + settings.width + GAP);
    }

    #[test]
    fn a_window_spanning_the_width_is_coached_from_below() {
        // No room either side, and the window leaves a band at the bottom.
        let settings = PointRect {
            x: 0.0,
            y: 24.0,
            width: 1512.0,
            height: 700.0,
        };
        let p = place(settings, laptop(), CARD).expect("there is a band below");
        assert_eq!(p.side, Side::Below);
        assert_eq!(p.rect.y, settings.y + settings.height + GAP);
        // Centred on the window.
        assert_eq!(
            p.rect.x + p.rect.width / 2.0,
            settings.x + settings.width / 2.0
        );
    }

    #[test]
    fn a_bottom_anchored_full_width_window_is_coached_from_above() {
        let settings = PointRect {
            x: 0.0,
            y: 300.0,
            width: 1512.0,
            height: 682.0,
        };
        let p = place(settings, laptop(), CARD).expect("there is a band above");
        assert_eq!(p.side, Side::Above);
        assert_eq!(p.rect.y + p.rect.height + GAP, settings.y);
    }

    #[test]
    fn a_window_filling_the_display_gets_no_card() {
        // Nothing fits beside it, and over it is not an option. Showing nothing
        // leaves the person exactly where they were before this existed.
        let settings = laptop();
        assert!(place(settings, laptop(), CARD).is_none());
    }

    #[test]
    fn the_card_stays_on_the_display_the_window_is_on() {
        // A second display to the right of the laptop, 1x.
        let external = PointRect {
            x: 1512.0,
            y: 0.0,
            width: 1920.0,
            height: 1055.0,
        };
        let settings = PointRect {
            x: 1600.0,
            y: 100.0,
            width: 715.0,
            height: 600.0,
        };
        let p = place(settings, external, CARD).expect("the external has room");
        assert!(
            p.rect.x >= external.x,
            "the card wandered onto the other display: {:?}",
            p.rect
        );
        assert!(fits_in(&p.rect, &external));
    }

    #[test]
    fn a_window_hanging_off_the_bottom_pulls_the_card_back_on() {
        let settings = PointRect {
            x: 400.0,
            y: 900.0,
            width: 715.0,
            height: 600.0,
        };
        let p = place(settings, laptop(), CARD).expect("beside it still works");
        assert!(
            fits_in(&p.rect, &laptop()),
            "the card is off the bottom: {:?}",
            p.rect
        );
    }

    #[test]
    fn the_arrow_always_points_back_at_the_window() {
        assert_eq!(Side::Left.arrow(), "right");
        assert_eq!(Side::Right.arrow(), "left");
        assert_eq!(Side::Above.arrow(), "down");
        assert_eq!(Side::Below.arrow(), "up");
    }

    #[test]
    fn only_accessibility_is_coached_today() {
        assert!(pane_for("accessibility").is_some());
        // Each of these is one row away, and each owes its own verification and
        // its own answer to "flipped it and nothing happened". See pane_for.
        for later in [
            "microphone",
            "screen_recording",
            "input_monitoring",
            "full_disk_access",
        ] {
            assert!(
                pane_for(later).is_none(),
                "'{}' is coached but was not verified on a real machine",
                later
            );
        }
    }

    #[test]
    fn the_one_line_is_one_line_and_says_what_to_do() {
        let pane = pane_for("accessibility").expect("accessibility is coached");
        assert!(!pane.headline.contains('\n'));
        assert!(pane.headline.len() < 60, "{}", pane.headline);
        // Names what the person is looking for and what to do with it.
        assert!(pane.headline.contains("Juno"));
        assert!(pane.headline.contains("switch"));
        // Nothing about TCC, privacy databases or why macOS is like this.
        for jargon in ["TCC", "permission", "privacy", "macOS", "database"] {
            assert!(
                !pane.headline.contains(jargon),
                "the headline explains itself instead of saying what to do: {}",
                pane.headline
            );
        }
        assert!(!pane.headline.contains('—'), "em dash in the headline");
        assert!(!pane.receipt.contains('—'), "em dash in the receipt");
    }

    #[test]
    fn the_receipt_says_the_switch_is_on_rather_than_thanking_anyone() {
        let pane = pane_for("accessibility").expect("accessibility is coached");
        assert!(pane.receipt.contains("on"));
        assert!(!pane.receipt.to_lowercase().contains("thank"));
    }

    /// The whole handover, as one test on purpose: the registry is a static,
    /// and two tests taking turns with it would interleave.
    #[test]
    fn a_card_is_only_ever_taken_down_by_whoever_put_it_up() {
        let pane = pane_for("accessibility").expect("accessibility is coached");

        // One ask puts a card up, pointing rather than granted.
        let (first, first_token) = register(pane).expect("the registry took the card");
        assert!(!first_token.is_cancelled());

        // A grant for something else is not this card's business.
        assert!(mark_granted("microphone").is_none());
        assert!(
            !first_token.is_cancelled(),
            "a card was cancelled by a grant it was not about"
        );

        // The grant lands. The card becomes its receipt and the follow stops.
        let (granted_id, state) = mark_granted("accessibility").expect("the card was granted");
        assert_eq!(granted_id, first);
        assert!(state.granted);
        assert_eq!(state.receipt, pane.receipt);
        assert!(
            first_token.is_cancelled(),
            "the follow task kept running during the receipt, so a tick could move or hide the card"
        );

        // A second ask lands inside the receipt's beat and replaces the card.
        let (second, second_token) = register(pane).expect("the registry took the second card");
        assert_ne!(second, first, "an id was reused");

        // The first card's timer now fires. It must not touch the new card.
        assert!(
            !forget(first),
            "the receipt's timer took down the card a second ask had just put up"
        );
        assert!(
            !second_token.is_cancelled(),
            "the new card's follow was stopped by the old timer"
        );

        // The second card's own teardown does take it down.
        assert!(forget(second));
        assert!(second_token.is_cancelled());

        // And a teardown for a card that is already gone is a no-op rather
        // than a hide of whatever happens to be up.
        assert!(!forget(second));
        assert!(!forget(first));
    }

    #[test]
    fn a_span_is_held_inside_the_display_whatever_is_wanted() {
        // Wanting a position past the right edge gets the last one that fits.
        assert_eq!(
            clamp_into(5000.0, 0.0, 1000.0, 100.0),
            1000.0 - EDGE - 100.0
        );
        // Wanting one off the left gets the first.
        assert_eq!(clamp_into(-500.0, 0.0, 1000.0, 100.0), EDGE);
        // A display too small for the card still yields a position on it
        // rather than a negative one. `fits_in` is what rejects it.
        assert_eq!(clamp_into(10.0, 0.0, 50.0, 100.0), EDGE);
    }
}
