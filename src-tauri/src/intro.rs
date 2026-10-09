//! # Intro reveal
//!
//! How Juno appears.
//!
//! Until now the floating bar reached the screen with a plain `show()`: a black
//! capsule with a faint hairline, no shadow, dropped onto whatever was there.
//! Over a dark window that is black on black, and the person's contact with
//! Juno was a voice from nowhere saying hello.
//!
//! Now, every time the bar comes onto the screen at launch (and when setup
//! ends, which is the first such time), a transparent click-through window
//! opens around the spot where the bar is about to land. Smoke gathers there,
//! the bar is shown underneath it while it is dense, and the smoke clears. The
//! greeting starts with the smoke, the first thing on screen: the reveal wakes
//! it at [`GREETING_AT_MS`], and the line was rendered while Juno started, so it
//! plays at once. The window is built for the occasion and destroyed
//! when the sequence ends, so it costs nothing for the rest of the session.
//!
//! This module owns the clock. The frontend draws the smoke and nothing else:
//! it asks for the plan, draws for `duration_ms`, and the bar appears at
//! `bar_at_ms` whether or not the frontend got as far as drawing. If anything
//! about the reveal cannot be arranged, the bar is shown the plain way, with no
//! word to the person: she simply appears, as she always did.
//!
//! The smoke keeps clear of the screen edge. The bar docks in a well, which
//! may be a corner, an edge midpoint or the centre, so the plan carries an
//! inward vector: the direction from the pill toward the open screen. Top
//! centre gives "down", a bottom-right corner gives "up and left", the centre
//! gives nothing in particular and the smoke sits all round. One shader, one
//! parameter.
//!
//! At launch the window is not built for the occasion but preloaded, hidden,
//! beside the bar ([`preload`]): its page has loaded and is waiting in
//! [`intro_ready`] by the time the bar asks to be shown, so the smoke starts
//! the moment the bar's frame is set instead of after a second page load
//! (about 0.45 s in the logs). Later reveals (setup closing, a replay) build
//! the window as before.
//!
//! Tweakables: the constants at the top of this file (timing and the window)
//! and the `LOOK` block in `src/components/intro/introModel.ts` (density,
//! size, colour). See `docs/plans/intro-reveal.md`.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};
use tokio::sync::Notify;
use tracing::{debug, info, warn};

use crate::constants::ui::window_labels;
use crate::platform::bar_hit_test::{self, HitRect};
use crate::platform::desktop_points;

/// The whole sequence, start to the last wisp.
pub const DURATION_MS: u64 = 2600;

/// When the bar is shown, inside the sequence. The smoke is densest around
/// here, so the bar's own hard cut is hidden inside it.
pub const BAR_AT_MS: u64 = 1000;

/// When the greeting speaks, inside the sequence: the moment the smoke starts,
/// the first thing on screen. Voice and picture arrive together, and the line
/// is still going when the bar steps out of the smoke. Tweakable; a
/// compile-time check keeps it inside the sequence.
pub const GREETING_AT_MS: u64 = 0;

// The beat falls inside the sequence.
const _: () = assert!(GREETING_AT_MS < DURATION_MS);

/// A beat this recent still counts for a greeting that starts waiting after
/// it: `on_launch` and the reveal race, and either may come first.
pub const GREETING_BEAT_FRESH: Duration = Duration::from_secs(3);

/// The intro window, logical points. Big enough for the cloud to fade out on
/// its own before it reaches any edge.
pub const WINDOW_WIDTH: f64 = 560.0;
pub const WINDOW_HEIGHT: f64 = 360.0;

/// How far the pill sits from the window's edge-facing side, logical points.
/// The smoke never lives on that side, so this only has to clear the pill and
/// its rim.
const PILL_INSET_X: f64 = 70.0;
const PILL_INSET_Y: f64 = 40.0;

/// A well within this fraction of the screen's centre, on an axis, counts as
/// centred on that axis: the smoke is symmetric along it rather than pushed.
const DEAD_ZONE: f64 = 0.15;

/// How long to wait for the frontend to say it is drawing before giving up
/// and showing the bar the plain way.
const READY_WAIT: Duration = Duration::from_millis(1500);

/// A preloaded intro window nobody used by now is destroyed: the bar came up
/// some other way (setup was on screen, say), and the window should cost
/// nothing for the rest of the session.
const PRELOAD_LIFETIME: Duration = Duration::from_secs(20);

/// How long the intro page waits in [`intro_ready`] for a reveal to start.
/// Longer than [`PRELOAD_LIFETIME`], so a preloaded page is never told "no"
/// while its window is still wanted.
const PAGE_WAIT: Duration = Duration::from_secs(30);

/// How long to wait, at most, for a steady look to report what it is drawing
/// before measuring. At launch the page asks to be shown the moment its frame
/// is set, which can be a beat before it reports its drawn rectangles.
const REGIONS_WAIT: Duration = Duration::from_millis(400);
const REGIONS_POLL: Duration = Duration::from_millis(25);

/// What the bar's shape is taken to be when the look has not reported what it
/// is drawing: the resting Pill, 56 by 16, centred in its window.
const FALLBACK_PILL: (f64, f64) = (56.0, 16.0);

/// What the frontend needs to draw the reveal. Logical points, origin at the
/// intro window's top-left, y down.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct IntroPlan {
    pub duration_ms: u64,
    pub bar_at_ms: u64,
    /// The pill's centre inside the intro window.
    pub pill_x: f64,
    pub pill_y: f64,
    /// The pill's size, and its corner radius (a capsule: half its height).
    pub pill_w: f64,
    pub pill_h: f64,
    pub pill_radius: f64,
    /// Unit vector from the pill toward the open screen, or zero when the
    /// pill is mid-screen.
    pub inward_x: f64,
    pub inward_y: f64,
}

/// A rectangle in logical points.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

/// Which way the open screen lies from a well at this slot (0..1 along each
/// axis). Components inside the dead zone are dropped so edge midpoints give a
/// pure axis and the centre gives nothing.
pub fn inward_for(slot_x: f64, slot_y: f64) -> (f64, f64) {
    let dead = |v: f64| if v.abs() < DEAD_ZONE { 0.0 } else { v };
    let (vx, vy) = (dead(0.5 - slot_x), dead(0.5 - slot_y));
    let len = (vx * vx + vy * vy).sqrt();
    if len == 0.0 {
        (0.0, 0.0)
    } else {
        (vx / len, vy / len)
    }
}

/// Where the intro window goes for a pill at this rectangle on this screen,
/// and the plan the frontend draws from. Both in points.
///
/// The window is pushed from the pill toward the open screen so the pill sits
/// near the edge-facing side and the cloud has the rest, then clamped onto the
/// screen. The pill's position inside the window follows from wherever the
/// window ended up, so clamping never moves the smoke off the pill.
pub fn place(pill: Rect, screen: Rect) -> (Rect, IntroPlan) {
    let pill_cx = pill.x + pill.w / 2.0;
    let pill_cy = pill.y + pill.h / 2.0;
    let slot_x = if screen.w > 0.0 {
        (pill_cx - screen.x) / screen.w
    } else {
        0.5
    };
    let slot_y = if screen.h > 0.0 {
        (pill_cy - screen.y) / screen.h
    } else {
        0.5
    };
    let (ix, iy) = inward_for(slot_x, slot_y);

    let cx = pill_cx + ix * (WINDOW_WIDTH / 2.0 - PILL_INSET_X);
    let cy = pill_cy + iy * (WINDOW_HEIGHT / 2.0 - PILL_INSET_Y);
    let x = clamp_span(cx - WINDOW_WIDTH / 2.0, WINDOW_WIDTH, screen.x, screen.w);
    let y = clamp_span(cy - WINDOW_HEIGHT / 2.0, WINDOW_HEIGHT, screen.y, screen.h);

    let rect = Rect {
        x,
        y,
        w: WINDOW_WIDTH,
        h: WINDOW_HEIGHT,
    };
    let plan = IntroPlan {
        duration_ms: DURATION_MS,
        bar_at_ms: BAR_AT_MS,
        pill_x: pill_cx - x,
        pill_y: pill_cy - y,
        pill_w: pill.w,
        pill_h: pill.h,
        pill_radius: pill.h / 2.0,
        inward_x: ix,
        inward_y: iy,
    };
    (rect, plan)
}

/// The smallest rectangle around everything the bar is drawing, in points on
/// the desktop, or `None` when the look has not said what it draws.
///
/// A steady look reports its drawn rectangles relative to its window; at rest
/// that is the pill alone, which is the shape the smoke should hug.
pub fn drawn_bounds(origin: (f64, f64), regions: &[HitRect]) -> Option<Rect> {
    let first = regions.first()?;
    let (mut x0, mut y0, mut x1, mut y1) = (
        first.x,
        first.y,
        first.x + first.width,
        first.y + first.height,
    );
    for r in regions {
        x0 = x0.min(r.x);
        y0 = y0.min(r.y);
        x1 = x1.max(r.x + r.width);
        y1 = y1.max(r.y + r.height);
    }
    Some(Rect {
        x: origin.0 + x0,
        y: origin.1 + y0,
        w: x1 - x0,
        h: y1 - y0,
    })
}

/// The pill when nothing has been reported: [`FALLBACK_PILL`] centred in the
/// bar window, which is where every look puts its resting shape.
pub fn fallback_pill(origin: (f64, f64), window: (f64, f64)) -> Rect {
    Rect {
        x: origin.0 + (window.0 - FALLBACK_PILL.0) / 2.0,
        y: origin.1 + (window.1 - FALLBACK_PILL.1) / 2.0,
        w: FALLBACK_PILL.0,
        h: FALLBACK_PILL.1,
    }
}

/// Keep `[start, start + len]` inside `[lo, lo + span]`, pinning to `lo` when
/// the span is too small to hold it.
fn clamp_span(start: f64, len: f64, lo: f64, span: f64) -> f64 {
    let hi = lo + span - len;
    if hi < lo {
        lo
    } else {
        start.clamp(lo, hi)
    }
}

/// The reveal in flight: what the frontend will ask for, and how it says it
/// has started drawing.
struct Run {
    plan: IntroPlan,
    ready: Arc<Notify>,
}

static RUN: Mutex<Option<Run>> = Mutex::new(None);

/// When the bar was last brought on, and the greeting woken for it.
static GREETING_BEAT_AT: Mutex<Option<std::time::Instant>> = Mutex::new(None);
static GREETING_BEAT: Notify = Notify::const_new();

/// The bar is on screen and the reveal is at its greeting beat.
fn give_greeting_beat() {
    if let Ok(mut at) = GREETING_BEAT_AT.lock() {
        *at = Some(std::time::Instant::now());
    }
    GREETING_BEAT.notify_waiters();
}

fn beat_is_fresh() -> bool {
    GREETING_BEAT_AT
        .lock()
        .ok()
        .and_then(|at| *at)
        .is_some_and(|at| at.elapsed() <= GREETING_BEAT_FRESH)
}

/// Wait for the moment the greeting should speak: the reveal's beat, or one
/// that happened in the last [`GREETING_BEAT_FRESH`]. False on timeout.
pub async fn greeting_beat(timeout: Duration) -> bool {
    let notified = GREETING_BEAT.notified();
    tokio::pin!(notified);
    let _ = notified.as_mut().enable();
    if beat_is_fresh() {
        return true;
    }
    tokio::time::timeout(timeout, notified).await.is_ok()
}

/// A reveal posted its plan: wakes an intro page waiting in [`intro_ready`].
static PLAN_POSTED: Notify = Notify::const_new();

/// An intro window built ahead, hidden, and not yet used by a reveal.
static PRELOADED: AtomicBool = AtomicBool::new(false);

/// Reveals run one at a time. A request while one is in flight is dropped:
/// the running reveal is already bringing the bar on.
static IN_FLIGHT: AtomicBool = AtomicBool::new(false);

/// Is a reveal bringing the bar on right now? Between the smoke starting and
/// the bar itself appearing ([`BAR_AT_MS`]) the bar window is still hidden, so
/// "is the bar visible" alone reads a reveal in progress as no bar at all.
pub fn reveal_in_flight() -> bool {
    IN_FLIGHT.load(Ordering::SeqCst)
}

/// Put the bar on screen the way she appears or, if the reveal cannot be
/// arranged, the plain way. Either way the bar is on screen afterwards;
/// nothing here is allowed to leave the person without one.
///
/// A bar that is already visible is left alone, and so is a reveal already in
/// flight. The startup routes that all end in "show the bar" (the page asking
/// once its frame is set, the macOS fallback timer, setup closing) therefore
/// produce one reveal between them, and showing a visible bar stays the no-op
/// it always was.
pub fn show_bar_with_reveal(app: &AppHandle) {
    let Some(bar) = app.get_webview_window(window_labels::FLOATING_BAR) else {
        return;
    };
    if bar.is_visible().unwrap_or(false) {
        return;
    }
    if IN_FLIGHT.swap(true, Ordering::SeqCst) {
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(reason) = reveal(&app).await {
            debug!("[Intro] No reveal ({}); showing the bar plainly", reason);
            show_bar_plainly(&app);
            give_greeting_beat();
            // A reveal that gave up early may hold a window it never used
            // (the preloaded one, say). Nothing else can be using it: reveals
            // run one at a time.
            if let Some(window) = app.get_webview_window(window_labels::INTRO) {
                if let Err(e) = window.destroy() {
                    warn!("[Intro] Could not destroy the unused intro window: {}", e);
                }
            }
            if let Ok(mut run) = RUN.lock() {
                *run = None;
            }
        }
        IN_FLIGHT.store(false, Ordering::SeqCst);
    });
}

fn show_bar_plainly(app: &AppHandle) {
    if let Some(bar) = app.get_webview_window(window_labels::FLOATING_BAR) {
        match bar.show() {
            Ok(()) => {
                crate::startup_timing::mark_once("bar shown");
                // The bar is up: now the windows that waited for it.
                crate::startup_windows::bar_is_up(app);
            }
            Err(e) => warn!("[Intro] Could not show the floating bar: {}", e),
        }
    }
}

/// Build the launch reveal's window now, hidden, so its page loads alongside
/// the bar's. Placed at the bar's provisional spot; [`reveal`] moves it to the
/// real one. Called from setup, after the bar is built and its stored position
/// applied.
pub fn preload(app: &AppHandle) {
    if app.get_webview_window(window_labels::INTRO).is_some() {
        return;
    }
    let origin = app
        .get_webview_window(window_labels::FLOATING_BAR)
        .and_then(|bar| desktop_points::window_origin_points(&bar))
        .unwrap_or((0.0, 0.0));
    match build_window(
        app,
        Rect {
            x: origin.0,
            y: origin.1,
            w: WINDOW_WIDTH,
            h: WINDOW_HEIGHT,
        },
    ) {
        Ok(_) => {
            PRELOADED.store(true, Ordering::SeqCst);
            crate::startup_timing::mark("smoke window preloaded");
        }
        Err(e) => {
            debug!(
                "[Intro] Could not preload the intro window ({}); it is built on demand",
                e
            );
            return;
        }
    }

    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(PRELOAD_LIFETIME).await;
        if PRELOADED.swap(false, Ordering::SeqCst) {
            if let Some(window) = app.get_webview_window(window_labels::INTRO) {
                debug!("[Intro] The preloaded intro window went unused; destroying it");
                if let Err(e) = window.destroy() {
                    warn!("[Intro] Could not destroy the unused intro window: {}", e);
                }
            }
        }
    });
}

/// The intro window: transparent, click-through, never focused, hidden until
/// the reveal shows it. Always [`WINDOW_WIDTH`] by [`WINDOW_HEIGHT`], so a
/// preloaded page's viewport is already the one it will draw in.
fn build_window(app: &AppHandle, rect: Rect) -> Result<tauri::WebviewWindow, String> {
    // Built here rather than declared in tauri.conf.json, because a declared
    // window stays alive for the whole session and this one is wanted for
    // under three seconds of it.
    let window =
        WebviewWindowBuilder::new(app, window_labels::INTRO, WebviewUrl::App("/intro".into()))
            .title("Juno")
            .inner_size(rect.w, rect.h)
            .position(rect.x, rect.y)
            .decorations(false)
            .transparent(true)
            .always_on_top(true)
            .resizable(false)
            .skip_taskbar(true)
            .shadow(false)
            .focused(false)
            // Never the key window: a window that can take the keyboard hands it
            // on when it hides, and AppKit picks the recipient. See the overlay
            // rule in `bar_stacking`.
            .focusable(false)
            .visible(false)
            .build()
            .map_err(|e| e.to_string())?;
    // Pure picture: a click on the smoke goes to whatever is under it.
    if let Err(e) = window.set_ignore_cursor_events(true) {
        warn!(
            "[Intro] Could not make the intro window click-through: {}",
            e
        );
    }
    Ok(window)
}

/// Build the intro window around the bar's spot, run the clock, and put the
/// bar up on time. Returns only once the sequence is over.
///
/// The window is built hidden. The frontend loads, asks for the plan through
/// [`intro_ready`], and that is the moment the window is shown and the clock
/// starts, so the first frame drawn is the first frame seen. If the frontend
/// never asks, the bar is shown at [`READY_WAIT`] and the window goes away.
async fn reveal(app: &AppHandle) -> Result<(), String> {
    let bar = app
        .get_webview_window(window_labels::FLOATING_BAR)
        .ok_or("no floating bar")?;
    // The preloaded window, if launch built one and nothing has used it.
    let preloaded = PRELOADED.swap(false, Ordering::SeqCst);
    if !preloaded && app.get_webview_window(window_labels::INTRO).is_some() {
        return Err("the last intro window is still going away".into());
    }

    // A steady look reports its drawn rectangles a beat after it asks to be
    // shown. Give it that beat, but no more: a look that never reports (the
    // Island, the Orb) gets the fallback pill and should not wait for it.
    let deadline = tokio::time::Instant::now() + REGIONS_WAIT;
    while bar_hit_test::current_regions().is_none() && tokio::time::Instant::now() < deadline {
        tokio::time::sleep(REGIONS_POLL).await;
    }

    // Everything in global desktop points (`platform::desktop_points`): the
    // window by its display's factor, the display by its own.
    let origin = desktop_points::window_origin_points(&bar).ok_or("no bar position")?;
    let scale = bar.scale_factor().map_err(|e| e.to_string())?;
    let size = bar.outer_size().map_err(|e| e.to_string())?;
    let bar_size = (
        f64::from(size.width) / scale,
        f64::from(size.height) / scale,
    );
    let monitor = bar
        .current_monitor()
        .map_err(|e| e.to_string())?
        .ok_or("the bar is on no monitor")?;
    let area = monitor.work_area();
    let work = desktop_points::monitor_rect_points(
        (area.position.x, area.position.y),
        (area.size.width, area.size.height),
        monitor.scale_factor(),
    );
    let screen = Rect {
        x: work.x,
        y: work.y,
        w: work.width,
        h: work.height,
    };
    // The pill is wherever the look says it is drawing. A steady look's window
    // is far bigger than its resting shape, so the window's centre is no guide.
    let pill = bar_hit_test::current_regions()
        .and_then(|regions| drawn_bounds(origin, &regions))
        .unwrap_or_else(|| fallback_pill(origin, bar_size));
    let (rect, plan) = place(pill, screen);

    let window = match app
        .get_webview_window(window_labels::INTRO)
        .filter(|_| preloaded)
    {
        Some(window) => {
            // Same size it was built at; only the spot changes.
            window
                .set_position(tauri::LogicalPosition::new(rect.x, rect.y))
                .map_err(|e| e.to_string())?;
            window
        }
        None => build_window(app, rect)?,
    };

    let ready = Arc::new(Notify::new());
    {
        let mut run = RUN.lock().map_err(|_| "intro state poisoned")?;
        *run = Some(Run {
            plan,
            ready: Arc::clone(&ready),
        });
    }
    // A preloaded page is already waiting for exactly this.
    PLAN_POSTED.notify_waiters();

    let drawing = tokio::time::timeout(READY_WAIT, ready.notified())
        .await
        .is_ok();
    if drawing {
        info!("[Intro] Revealing the bar");
        crate::startup_timing::mark_once("smoke shown");
        if let Err(e) = window.show() {
            warn!("[Intro] Could not show the intro window: {}", e);
        }
        // The greeting lands on the reveal's beat, timed from the smoke, not
        // on whenever something next notices the bar is visible.
        tauri::async_runtime::spawn(async {
            tokio::time::sleep(Duration::from_millis(GREETING_AT_MS)).await;
            give_greeting_beat();
        });
        tokio::time::sleep(Duration::from_millis(BAR_AT_MS)).await;
    } else {
        debug!("[Intro] The intro window never reported in; showing the bar plainly");
    }

    show_bar_plainly(app);

    if drawing {
        tokio::time::sleep(Duration::from_millis(DURATION_MS.saturating_sub(BAR_AT_MS))).await;
    } else {
        give_greeting_beat();
    }
    // Destroyed, not closed: the next reveal checks for this label, and a
    // close that is still being requested would read as one still running.
    if let Err(e) = window.destroy() {
        warn!("[Intro] Could not destroy the intro window: {}", e);
    }
    if let Ok(mut run) = RUN.lock() {
        *run = None;
    }

    Ok(())
}

/// The intro window is loaded and about to draw: hand it the plan and start
/// the clock. Called once per reveal, by the intro window.
///
/// A page that loads before its reveal (the preloaded one, at launch) waits
/// here until the reveal posts its plan, up to [`PAGE_WAIT`].
#[tauri::command]
pub async fn intro_ready() -> Result<IntroPlan, String> {
    let deadline = tokio::time::Instant::now() + PAGE_WAIT;
    loop {
        // Registered before the check, so a plan posted in between is not
        // missed.
        let posted = PLAN_POSTED.notified();
        tokio::pin!(posted);
        let _ = posted.as_mut().enable();
        if let Some(plan) = take_plan()? {
            return Ok(plan);
        }
        if tokio::time::timeout_at(deadline, posted).await.is_err() {
            return Err("no reveal is running".into());
        }
    }
}

/// The plan of the reveal in flight, starting its clock; `None` when no reveal
/// has posted one yet.
fn take_plan() -> Result<Option<IntroPlan>, String> {
    let run = RUN.lock().map_err(|_| "intro state poisoned")?;
    Ok(run.as_ref().map(|run| {
        run.ready.notify_one();
        run.plan.clone()
    }))
}

/// Run the reveal again on the bar that is already on screen. A developer
/// control: the only way to watch the sequence without relaunching.
#[tauri::command]
pub async fn replay_intro(app: AppHandle) -> Result<(), String> {
    if let Some(bar) = app.get_webview_window(window_labels::FLOATING_BAR) {
        bar.hide().map_err(|e| e.to_string())?;
    }
    show_bar_with_reveal(&app);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    #[tokio::test]
    async fn a_greeting_that_starts_after_the_beat_still_hears_it() {
        give_greeting_beat();
        assert!(greeting_beat(Duration::from_millis(10)).await);
    }

    #[test]
    fn edge_midpoints_point_straight_in() {
        assert_eq!(inward_for(0.5, 0.0), (0.0, 1.0)); // top centre: down
        assert_eq!(inward_for(0.5, 1.0), (0.0, -1.0)); // bottom centre: up
        assert_eq!(inward_for(0.0, 0.5), (1.0, 0.0)); // left: right
        assert_eq!(inward_for(1.0, 0.5), (-1.0, 0.0)); // right: left
    }

    #[test]
    fn corners_point_diagonally_and_the_centre_nowhere() {
        let (x, y) = inward_for(1.0, 1.0);
        assert!(close(x, -std::f64::consts::FRAC_1_SQRT_2));
        assert!(close(y, -std::f64::consts::FRAC_1_SQRT_2));
        assert_eq!(inward_for(0.5, 0.5), (0.0, 0.0));
        // Just off centre still counts as centre.
        assert_eq!(inward_for(0.6, 0.45), (0.0, 0.0));
    }

    const SCREEN: Rect = Rect {
        x: 0.0,
        y: 25.0,
        w: 1512.0,
        h: 957.0,
    };

    /// A resting Pill (56 by 16) centred at a point.
    fn pill_at(cx: f64, cy: f64) -> Rect {
        Rect {
            x: cx - 28.0,
            y: cy - 8.0,
            w: 56.0,
            h: 16.0,
        }
    }

    #[test]
    fn top_centre_puts_the_pill_near_the_top_of_the_window() {
        let (rect, plan) = place(pill_at(756.0, 70.0), SCREEN);
        assert!(close(rect.x, 756.0 - WINDOW_WIDTH / 2.0));
        assert!(close(plan.pill_x, WINDOW_WIDTH / 2.0));
        assert!(close(plan.pill_y, PILL_INSET_Y));
        assert_eq!((plan.inward_x, plan.inward_y), (0.0, 1.0));
        assert_eq!(plan.duration_ms, DURATION_MS);
        assert_eq!(plan.bar_at_ms, BAR_AT_MS);
    }

    #[test]
    fn the_plan_carries_the_pills_own_shape() {
        let (_, plan) = place(pill_at(756.0, 70.0), SCREEN);
        assert_eq!(
            (plan.pill_w, plan.pill_h, plan.pill_radius),
            (56.0, 16.0, 8.0)
        );
    }

    #[test]
    fn a_corner_keeps_the_window_on_screen_and_the_pill_where_it_is() {
        // Bottom-right well: the window would hang off the screen unless clamped.
        let (rect, plan) = place(pill_at(1440.0, 940.0), SCREEN);
        assert!(rect.x + rect.w <= SCREEN.x + SCREEN.w + 1e-9);
        assert!(rect.y + rect.h <= SCREEN.y + SCREEN.h + 1e-9);
        // Wherever the window landed, the plan still points at the pill.
        assert!(close(rect.x + plan.pill_x, 1440.0));
        assert!(close(rect.y + plan.pill_y, 940.0));
        assert!(plan.inward_x < 0.0 && plan.inward_y < 0.0);
    }

    #[test]
    fn a_centred_pill_gets_a_centred_window() {
        let (rect, plan) = place(pill_at(756.0, 503.5), SCREEN);
        assert!(close(rect.x + rect.w / 2.0, 756.0));
        assert!(close(rect.y + rect.h / 2.0, 503.5));
        assert_eq!((plan.inward_x, plan.inward_y), (0.0, 0.0));
    }

    #[test]
    fn a_screen_too_small_for_the_window_pins_to_its_origin() {
        let tiny = Rect {
            x: 10.0,
            y: 10.0,
            w: 300.0,
            h: 200.0,
        };
        let (rect, _) = place(pill_at(100.0, 100.0), tiny);
        assert_eq!((rect.x, rect.y), (10.0, 10.0));
    }

    #[test]
    fn drawn_bounds_is_the_union_of_what_the_look_draws_moved_onto_the_desktop() {
        let regions = [
            HitRect {
                x: 200.0,
                y: 300.0,
                width: 56.0,
                height: 16.0,
            },
            HitRect {
                x: 190.0,
                y: 320.0,
                width: 80.0,
                height: 30.0,
            },
        ];
        let b = drawn_bounds((1000.0, 100.0), &regions).expect("two regions have bounds");
        assert_eq!((b.x, b.y, b.w, b.h), (1190.0, 400.0, 80.0, 50.0));
        assert!(drawn_bounds((0.0, 0.0), &[]).is_none());
    }

    #[test]
    fn without_regions_the_pill_is_assumed_centred_in_the_bar_window() {
        let p = fallback_pill((100.0, 50.0), (88.0, 48.0));
        assert_eq!((p.x, p.y, p.w, p.h), (116.0, 66.0, 56.0, 16.0));
    }
}
