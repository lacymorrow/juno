//! # macOS Platform Module
//!
//! This module contains all macOS-specific functionality for the Juno application.
//! It handles window styling, mouse tracking, accessibility features, and other
//! Cocoa/AppKit integrations needed for proper macOS behavior.

use crate::bar_stacking::BarStacking;
use crate::constants;
use crate::constants::{errors::templates, events};
use tauri::{AppHandle, Emitter, Manager};
use tracing::{debug, error, info, warn};

// Helper function for error formatting - properly handles template substitution
fn format_error(template: &str, context: &str, error: impl std::fmt::Display) -> String {
    template
        .replacen("{}", context, 1)
        .replacen("{}", &error.to_string(), 1)
}

// macOS-specific imports - only available on macOS
#[cfg(target_os = "macos")]
use {
    cocoa::{
        appkit::{NSWindow, NSWindowCollectionBehavior},
        base::{id as cocoa_id, nil, BOOL, NO, YES},
        foundation::{NSPoint, NSRect, NSSize},
    },
    objc::{
        class,
        declare::ClassDecl,
        msg_send,
        runtime::{Class, Object, Sel},
        sel, sel_impl,
    },
    std::sync::Mutex,
};

/// Apply comprehensive macOS-specific setup for all application windows
pub fn apply_macos_setup(app_handle: &AppHandle) {
    #[cfg(target_os = "macos")]
    {
        info!("Applying macOS specific setup...");

        // Setup floating bar window. It is the only declared window that
        // exists this early: the rest are built once the bar is on screen
        // (`startup_windows`), and each is set up by
        // `configure_deferred_window` as it is built.
        setup_floating_bar_window(app_handle);

        // Read the keyboard layout while we are still on the main thread.
        // Paste insertion runs on a blocking worker and Text Input Services
        // traps the process if asked from there, so this is the only place
        // the layout can be learned.
        computer_use_ai_sdk::prime_cmd_v_keycode();

        // The bar's level is set above as part of building it. Deriving it once
        // more from the situation means even the resting state comes from the
        // one function that owns the decision, rather than from a line of
        // setup code that could drift from it.
        crate::bar_stacking::apply(app_handle);

        info!("macOS specific setup completed");
    }

    #[cfg(not(target_os = "macos"))]
    {
        // No-op on non-macOS platforms
        info!("macOS setup skipped on non-macOS platform");
    }
}

/// Put the floating bar where [`crate::bar_stacking`] says it belongs.
///
/// Two AppKit calls, and both are needed for the lowered cases. `setLevel:`
/// alone is not enough: it orders a window to the front of the level it moves
/// into, so a bar dropped from floating to normal can land on top of the very
/// window it was getting out from under. The explicit
/// `orderWindow:NSWindowBelow relativeTo:` is app-relative, so it puts the bar
/// under that one window and changes nothing about the rest of the system.
///
/// Levels are not app-relative, which is the whole reason the ordering call is
/// here. See `crate::bar_stacking` for what that cost once already.
#[cfg(target_os = "macos")]
pub fn apply_bar_stacking(
    app_handle: &AppHandle,
    stacking: BarStacking,
    front_label: Option<&'static str>,
) {
    let Some(bar) = app_handle.get_webview_window(constants::window_labels::FLOATING_BAR) else {
        return;
    };
    let Ok(bar_ptr) = bar.ns_window() else {
        return;
    };

    // Only resolved for the intent that asks for it, and only for a window
    // that is actually on screen: ordering against a window that is not there
    // is a silent no-op with a worse failure mode than skipping it.
    let front_ptr = if stacking.orders_below_front() {
        front_label
            .and_then(|label| app_handle.get_webview_window(label))
            .filter(|window| window.is_visible().unwrap_or(false))
            .and_then(|window| window.ns_window().ok())
    } else {
        None
    };

    let level = stacking.ns_window_level();
    let bar_addr = bar_ptr as usize;
    let front_addr = front_ptr.map(|ptr| ptr as usize);

    // NSWindow levels and ordering are main-thread-only, and every caller here
    // is a command or an event handler on some other thread. Queueing also
    // means this runs after any activation AppKit is part-way through, which is
    // the re-order being corrected.
    if let Err(e) = app_handle.run_on_main_thread(move || unsafe {
        let bar = bar_addr as cocoa_id;
        bar.setLevel_(level);
        if let Some(front_addr) = front_addr {
            let front = front_addr as cocoa_id;
            #[allow(unexpected_cfgs)]
            let front_number: i64 = msg_send![front, windowNumber];
            #[allow(unexpected_cfgs)]
            let _: () = msg_send![bar, orderWindow: NS_WINDOW_BELOW relativeTo: front_number];
        }
    }) {
        warn!("Could not restack the floating bar: {}", e);
    }
}

#[cfg(not(target_os = "macos"))]
pub fn apply_bar_stacking(
    _app_handle: &AppHandle,
    _stacking: BarStacking,
    _front_label: Option<&'static str>,
) {
}

/// Order the floating bar directly above another of Juno's windows, when both
/// are on screen at the same level. Ordering only: the level is untouched, so
/// nothing changes for any other app, and a bar that has stepped down to the
/// normal level (see [`crate::bar_stacking`]) is left exactly where it is.
///
/// The drop overlay is the caller. It shares the bar's floating level, and
/// showing it orders it to the front of that level, so without this its dim
/// could be drawn over the very pill being dragged.
#[cfg(target_os = "macos")]
pub fn order_bar_above(app_handle: &AppHandle, other_label: &str) {
    let Some(bar) = app_handle.get_webview_window(constants::window_labels::FLOATING_BAR) else {
        return;
    };
    let Some(other) = app_handle.get_webview_window(other_label) else {
        return;
    };
    let (Ok(bar_ptr), Ok(other_ptr)) = (bar.ns_window(), other.ns_window()) else {
        return;
    };
    let bar_addr = bar_ptr as usize;
    let other_addr = other_ptr as usize;
    // SAFETY: live NSWindow pointers handed out by Tauri for two windows that
    // live for the app's lifetime; standard NSWindow selectors; main thread.
    if let Err(e) = app_handle.run_on_main_thread(move || unsafe {
        let bar = bar_addr as cocoa_id;
        let other = other_addr as cocoa_id;
        #[allow(unexpected_cfgs)]
        let bar_visible: BOOL = msg_send![bar, isVisible];
        #[allow(unexpected_cfgs)]
        let other_visible: BOOL = msg_send![other, isVisible];
        if bar_visible == NO || other_visible == NO {
            return;
        }
        #[allow(unexpected_cfgs)]
        let bar_level: i64 = msg_send![bar, level];
        #[allow(unexpected_cfgs)]
        let other_level: i64 = msg_send![other, level];
        if bar_level != other_level {
            return;
        }
        #[allow(unexpected_cfgs)]
        let other_number: i64 = msg_send![other, windowNumber];
        #[allow(unexpected_cfgs)]
        let _: () = msg_send![bar, orderWindow: NS_WINDOW_ABOVE relativeTo: other_number];
    }) {
        warn!(
            "Could not order the floating bar above '{}': {}",
            other_label, e
        );
    }
}

/// Setup macOS-specific styling and behavior for the floating bar window
#[cfg(target_os = "macos")]
fn setup_floating_bar_window(app_handle: &AppHandle) {
    if let Some(window) = app_handle.get_webview_window(constants::window_labels::FLOATING_BAR) {
        info!("Found floating-bar for macOS setup.");

        // Apply window styling
        match window.ns_window() {
            Ok(ns_window_ptr) => {
                let ns_window = ns_window_ptr as cocoa_id;
                // The bar's resting place in the stack. Every change to it
                // afterwards goes through `crate::bar_stacking`, which owns the
                // decision; this is only the value it starts at.
                let resting_level = BarStacking::Floating.ns_window_level();
                unsafe {
                    // NSFloatingWindowLevel. This used to be 5, which put the
                    // bar above system alerts: a screen-recording prompt came
                    // up *behind* it, unreadable and unclickable. Floating is
                    // the documented level for an accessory window, above
                    // ordinary windows and below anything the system needs to
                    // put in front of a person.
                    ns_window.setLevel_(resting_level);
                    ns_window.setOpaque_(NO);
                    ns_window.setHasShadow_(NO);
                    // Visible across all spaces, full-screen apps, and Cmd+` cycle excluded
                    ns_window.setCollectionBehavior_(
                        NSWindowCollectionBehavior::NSWindowCollectionBehaviorCanJoinAllSpaces |
                        NSWindowCollectionBehavior::NSWindowCollectionBehaviorStationary |
                        NSWindowCollectionBehavior::NSWindowCollectionBehaviorIgnoresCycle |
                        NSWindowCollectionBehavior::NSWindowCollectionBehaviorFullScreenAuxiliary
                    );

                    // Enable mouse events for floating bar interactions
                    #[allow(unexpected_cfgs)]
                    let _: BOOL = msg_send![ns_window, setIgnoresMouseEvents: NO];

                    // Stay visible when the user switches to another app — this is the key
                    // non-activating behavior: Juno overlays must persist across app switches
                    ns_window.setHidesOnDeactivate_(NO);

                    // Receive mouseMoved: so hover works while Juno is inactive.
                    #[allow(unexpected_cfgs)]
                    let _: () = msg_send![ns_window, setAcceptsMouseMovedEvents: YES];

                    info!("macOS standard styling applied to floating-bar.");
                }
            }
            Err(e) => {
                error!("Error getting NSWindow for styling floating-bar: {}", e);
            }
        }

        // Setup mouse tracking
        if let Err(e) = mouse_tracking::setup_tracking_area(&window, app_handle.clone()) {
            error!("Failed to setup mouse tracking area: {}", e);
        }

        // Where the bar belongs, applied while it is still hidden. The webview
        // works the well out for itself once it has loaded and applies the same
        // frame, but it cannot be relied on to get there first, and until it
        // does the window sits at the spot macOS cascaded it to. Doing this here
        // means every path that puts the bar on screen puts it on screen in the
        // right place.
        crate::commands::bar_position::restore_bar_position(app_handle);

        // Ensure proper window activation
        activate_floating_bar_window(window);
    } else {
        error!("Warning: floating-bar window not found during macOS specific setup.");
    }
}

/// Setup macOS-specific styling and behavior for the floating panel window
/// Atomically move + resize the floating bar in a single `NSWindow setFrame:`.
///
/// Tauri exposes `set_position` and `set_size` separately; issuing both let
/// the WindowServer composite them in different frames, so for one frame the
/// window showed its new size still at the old top-left. One
/// `setFrame:display:animate:NO` changes origin and size in one transaction.
///
/// `x`/`y` (top-left) and `w_pt`/`h_pt` are global desktop points, top-left
/// origin at the primary display. Cocoa's frame is the same space with y
/// pointing up from the primary display's bottom edge, so the conversion is
/// one flip against the primary display's height (`NSScreen.screens[0]`, the
/// screen at the origin). That is exact on every display; the delta this used
/// to apply to physical pixels was scaled by the display the window was
/// leaving, which put a frame aimed at a second display of another density
/// in the wrong place.
///
/// `grab`, when given, is a point inside the window (from its top-left, in
/// points) that must end up under the cursor: `x`/`y` are then ignored and the
/// origin is computed from `NSEvent.mouseLocation`, read here on the main
/// thread in the same call as `setFrame:`. The bar's drag window is placed
/// this way so the spot the user pressed is under the cursor when the OS drag
/// starts, however far a fast flick has carried the cursor since the press.
///
/// Returns where the window actually is afterwards, in points (AppKit may
/// constrain the frame, e.g. below the menu bar), and the cursor it was
/// placed by, when it was placed by one.
#[cfg(target_os = "macos")]
pub fn set_bar_frame_atomic(
    app_handle: &AppHandle,
    x: f64,
    y: f64,
    w_pt: f64,
    h_pt: f64,
    grab: Option<(f64, f64)>,
) -> Result<BarFramePlaced, String> {
    use crate::platform::desktop_points::{
        cocoa_frame_top_left, cocoa_point_to_points, origin_under_cursor,
    };
    let label = constants::ui::window_labels::FLOATING_BAR;
    let window = app_handle
        .get_webview_window(label)
        .ok_or("floating-bar window not found")?;
    let ns_window = window.ns_window().map_err(|e| e.to_string())? as cocoa_id;

    // SAFETY: `screens`, `objectAtIndex:`, `frame` and `setFrame:` are standard
    // AppKit selectors on live objects; ns_window is the bar's window handle
    // from Tauri. The command dispatches this to the main thread.
    unsafe {
        let primary_h = primary_screen_height()?;
        let cursor = grab.map(|_| {
            let mouse: NSPoint = msg_send![class!(NSEvent), mouseLocation];
            cocoa_point_to_points((mouse.x, mouse.y), primary_h)
        });
        let (x, y) = match (grab, cursor) {
            (Some(grab), Some(cursor)) => origin_under_cursor(cursor, grab),
            _ => (x, y),
        };
        let new_frame = NSRect::new(
            NSPoint::new(x, primary_h - y - h_pt),
            NSSize::new(w_pt, h_pt),
        );
        let _: () = msg_send![ns_window, setFrame: new_frame display: YES animate: NO];
        let placed: NSRect = msg_send![ns_window, frame];
        let (x, y) = cocoa_frame_top_left(
            (placed.origin.x, placed.origin.y),
            placed.size.height,
            primary_h,
        );
        Ok(BarFramePlaced { x, y, cursor })
    }
}

/// Where `set_bar_frame_atomic` left the bar, in global points.
#[cfg(target_os = "macos")]
#[derive(Debug, Clone, Copy)]
pub struct BarFramePlaced {
    pub x: f64,
    pub y: f64,
    /// The cursor the window was placed by, read in the same call.
    pub cursor: Option<(f64, f64)>,
}

/// The primary display's height in points: the flip between Cocoa's
/// bottom-up frames and global top-left points.
///
/// # Safety
/// Main thread only (AppKit).
#[cfg(target_os = "macos")]
unsafe fn primary_screen_height() -> Result<f64, String> {
    let screens: cocoa_id = msg_send![class!(NSScreen), screens];
    let count: usize = msg_send![screens, count];
    if count == 0 {
        return Err("No screens to place the bar on".to_string());
    }
    let primary: cocoa_id = msg_send![screens, objectAtIndex: 0usize];
    let primary_frame: NSRect = msg_send![primary, frame];
    Ok(primary_frame.size.height)
}

/// Start carrying the bar with the cursor: from here until the release, every
/// mouse-dragged event puts `grab` (points from the window's top-left) under
/// the cursor (`platform::bar_drag_follow`).
///
/// This replaced handing the window to the OS drag (`performWindowDragWithEvent:`
/// with a mouse-down made at the cursor, #766). That drag is the
/// WindowServer's: Juno cannot see it, and with a mouse-down that is not the
/// real press the window drifted off the cursor as the drag went on, and once
/// was left behind off screen. See the module docs for the evidence.
///
/// Must run on the main thread, right after the frame was set, so no event is
/// handled between the placement and the first move. Returns the cursor
/// (global points) the drag started at. Never touches the window's class or
/// its level: every move goes through `setFrameTopLeftPoint:`, so the menu bar
/// constraint holds exactly as it does for any frame change, and the #728
/// rules in `docs/plans/appearance-steady-frame.md` are untouched.
#[cfg(target_os = "macos")]
pub fn start_bar_drag_follow(
    app_handle: &AppHandle,
    grab: (f64, f64),
) -> Result<(f64, f64), String> {
    use crate::platform::desktop_points::cocoa_point_to_points;
    let label = constants::ui::window_labels::FLOATING_BAR;
    let window = app_handle
        .get_webview_window(label)
        .ok_or("floating-bar window not found")?;
    let ns_window = window.ns_window().map_err(|e| e.to_string())? as cocoa_id;

    // SAFETY: standard AppKit selectors on the main thread (the caller is a
    // `run_on_main_thread` closure).
    let cursor = unsafe {
        let primary_h = primary_screen_height()?;
        let mouse: NSPoint = msg_send![class!(NSEvent), mouseLocation];
        cocoa_point_to_points((mouse.x, mouse.y), primary_h)
    };
    crate::platform::bar_drag_follow::start(app_handle, ns_window, grab)?;
    Ok(cursor)
}

#[cfg(target_os = "macos")]
fn setup_floating_panel_window(app_handle: &AppHandle) {
    if let Some(window) = app_handle.get_webview_window(constants::window_labels::FLOATING_PANEL) {
        info!("Found floating-panel for macOS setup.");

        // Apply window styling
        match window.ns_window() {
            Ok(ns_window_ptr) => {
                let ns_window = ns_window_ptr as cocoa_id;
                unsafe {
                    // NSFloatingWindowLevel (3) — appropriate for accessory windows
                    ns_window.setLevel_(3);
                    ns_window.setOpaque_(NO);
                    ns_window.setHasShadow_(NO);
                    ns_window.setBackgroundColor_(msg_send![class!(NSColor), clearColor]);

                    // Visible across all spaces and full-screen apps; excluded from Cmd+` cycle.
                    // NSWindowCollectionBehaviorTransient is intentionally omitted: it conflicts
                    // with Stationary and would cause macOS to hide the panel on app deactivation,
                    // negating the setHidesOnDeactivate_(NO) call below.
                    ns_window.setCollectionBehavior_(
                        NSWindowCollectionBehavior::NSWindowCollectionBehaviorCanJoinAllSpaces |
                        NSWindowCollectionBehavior::NSWindowCollectionBehaviorStationary |
                        NSWindowCollectionBehavior::NSWindowCollectionBehaviorIgnoresCycle |
                        NSWindowCollectionBehavior::NSWindowCollectionBehaviorFullScreenAuxiliary
                    );

                    // Click-through by default; toggled interactively via ui_set_panel_click_through
                    #[allow(unexpected_cfgs)]
                    let _: BOOL = msg_send![ns_window, setIgnoresMouseEvents: YES];

                    // Stay visible when user switches to another app — non-activating behavior
                    ns_window.setHidesOnDeactivate_(NO);

                    // Accessibility role and label
                    #[allow(unexpected_cfgs)]
                    let accessibility_role_string: cocoa_id = msg_send![class!(NSString), stringWithUTF8String: "AXFloatingWindow".as_ptr()];
                    let _: () =
                        msg_send![ns_window, setAccessibilityRole: accessibility_role_string];

                    #[allow(unexpected_cfgs)]
                    let accessibility_label_string: cocoa_id = msg_send![class!(NSString), stringWithUTF8String: "Juno AI Assistant Panel".as_ptr()];
                    let _: () =
                        msg_send![ns_window, setAccessibilityLabel: accessibility_label_string];

                    info!("macOS Setup: Floating panel configured.");
                }
            }
            Err(e) => {
                error!("Error getting NSWindow for styling floating-panel: {}", e);
            }
        }

        // Setup mouse tracking for floating panel
        if let Err(e) = mouse_tracking::setup_tracking_area(&window, app_handle.clone()) {
            error!(
                "Failed to setup mouse tracking area for floating panel: {}",
                e
            );
        }
    } else {
        error!("Warning: floating-panel window not found during macOS specific setup.");
    }
}

/// Setup macOS-specific behavior for the main application window
#[cfg(target_os = "macos")]
fn setup_main_window(app_handle: &AppHandle) {
    if let Some(main_window) = app_handle.get_webview_window(constants::window_labels::MAIN) {
        info!("Setting up main window for proper focus handling.");

        // Apply macOS-specific fixes for the main window
        match main_window.ns_window() {
            Ok(ns_window_ptr) => {
                let ns_window = ns_window_ptr as cocoa_id;
                unsafe {
                    // Ensure main window can receive mouse events
                    #[allow(unexpected_cfgs)]
                    let _: BOOL = msg_send![ns_window, setIgnoresMouseEvents: NO];

                    // Make sure the window accepts first responder status
                    #[allow(unexpected_cfgs)]
                    let _: BOOL = msg_send![ns_window, setAcceptsMouseMovedEvents: YES];

                    info!("macOS Setup: Main window mouse events enabled.");
                }
            }
            Err(e) => {
                error!("Error getting NSWindow for main window setup: {}", e);
            }
        }

        // NOTE: Main window activation (show/focus) is handled by
        // initialize_onboarding_system() in state_management, NOT here.
        // This avoids a race where the main window appears on top of
        // the onboarding window before the user completes onboarding.
    } else {
        error!("Warning: main window not found during macOS specific setup.");
    }
}

/// Setup macOS-specific behavior for the desktop cursor overlay window.
#[cfg(target_os = "macos")]
fn setup_desktop_cursor_overlay_window(app_handle: &AppHandle) {
    match app_handle.get_webview_window(crate::window_management::DESKTOP_CURSOR_OVERLAY_LABEL) {
        Some(window) => style_cursor_overlay(app_handle, &window),
        None => info!(
            "desktop-cursor-overlay not found during macOS setup; it is styled when first shown."
        ),
    }
}

/// Make the cursor overlay a window that can be seen wherever Juno acts.
///
/// It must be:
///   - At NSScreenSaverWindowLevel (1000) so it appears above all app content
///   - Fully click-through (ignoresMouseEvents) so it never blocks user input
///   - Non-activating: must not steal focus from the user's active application
///   - Persistent across spaces and full-screen apps
///
/// Called at setup and again on every show, because a window rebuilt from its
/// config comes back at the floating level with none of this, which put it
/// under every full-screen app.
#[cfg(target_os = "macos")]
pub fn style_cursor_overlay(app_handle: &AppHandle, window: &tauri::WebviewWindow) {
    let ns_window_addr = match window.ns_window() {
        Ok(ptr) => ptr as usize,
        Err(e) => {
            error!(
                "Error getting NSWindow for styling desktop-cursor-overlay: {}",
                e
            );
            return;
        }
    };
    // AppKit, so the main thread. The address crosses the hop because the
    // raw pointer is not Send.
    if let Err(e) = app_handle.run_on_main_thread(move || unsafe {
        let ns_window = ns_window_addr as cocoa_id;
        // NSScreenSaverWindowLevel (1000): above all app content, full-screen included.
        ns_window.setLevel_(1000);
        // Fully click-through: the agent cursor must never intercept input.
        #[allow(unexpected_cfgs)]
        let _: () = msg_send![ns_window, setIgnoresMouseEvents: YES];
        // On every Space, and allowed over a full-screen app.
        ns_window.setCollectionBehavior_(
            NSWindowCollectionBehavior::NSWindowCollectionBehaviorCanJoinAllSpaces
                | NSWindowCollectionBehavior::NSWindowCollectionBehaviorStationary
                | NSWindowCollectionBehavior::NSWindowCollectionBehaviorFullScreenAuxiliary
                | NSWindowCollectionBehavior::NSWindowCollectionBehaviorIgnoresCycle,
        );
        // Stay visible when the person switches to another app.
        ns_window.setHidesOnDeactivate_(NO);
        ns_window.setOpaque_(NO);
        ns_window.setHasShadow_(NO);
    }) {
        warn!("Could not style the cursor overlay: {}", e);
    }
}

/// How long the bar is given to place itself before it is shown regardless.
///
/// Short enough that a bar which never reports in is still on screen while the
/// person is looking for it. It is deliberately no longer sized to win a race:
/// a `tauri:dev` build routinely needs more than this to boot its webview, so
/// this timer does fire there, and the reason that used to matter (the bar
/// appearing at the cascade spot and then jumping) is handled by restoring the
/// stored position before the window is ever shown.
#[cfg(target_os = "macos")]
const BAR_SHOW_FALLBACK_MS: u64 = 2500;

/// Show the floating bar without stealing application focus.
///
/// Overlay windows must never call set_focus()/makeKeyAndOrderFront: — that
/// triggers [NSApp activateIgnoringOtherApps:YES] which yanks keyboard focus
/// away from whatever app the user is currently in.  orderFront: (via show())
/// makes the window visible without changing the active application.
///
/// This is only the safety net. The bar normally puts itself on screen through
/// `show_bar_when_ready`, once the webview has worked out which well it belongs
/// in and set its first frame. A webview that never gets that far still gets a
/// bar, just a late one, and it is in the right place: `restore_bar_position`
/// has already applied the stored well to the hidden window, so this show no
/// longer exposes the spot macOS cascaded the window to.
#[cfg(target_os = "macos")]
fn activate_floating_bar_window(window: tauri::WebviewWindow<tauri::Wry>) {
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(tokio::time::Duration::from_millis(BAR_SHOW_FALLBACK_MS)).await;

        // The normal path already won: leave it alone. That includes a reveal
        // whose smoke is up but whose bar is not yet: the window is still
        // hidden for that second, and this used to log a second "put up" for
        // a show the reveal guard then dropped.
        if window.is_visible().unwrap_or(false) || crate::intro::reveal_in_flight() {
            return;
        }

        // If setup is on screen, stay off it. The bar is always on top, so
        // showing it here would cover the setup window, and it has nothing to
        // offer someone who has not finished setting up. It goes up when
        // onboarding closes instead.
        if crate::window_management::onboarding_is_open(&window.app_handle().clone()) {
            crate::window_management::mark_bar_withheld_for_onboarding();
            info!("Floating bar held back while onboarding is on screen");
            return;
        }

        // Revealed, never focused: the same smoke the normal path uses, and
        // it leaves a reveal already in flight alone. Overlays must not
        // activate Juno.
        info!("Floating bar put up by the startup fallback (no focus steal)");
        crate::intro::show_bar_with_reveal(&window.app_handle().clone());
    });
}

// NOTE: activate_main_window was removed — main window visibility is now
// controlled by initialize_onboarding_system() to avoid showing the main
// window on top of the onboarding window during first launch.

/// Mouse tracking functionality for macOS windows
#[cfg(target_os = "macos")]
pub mod mouse_tracking {
    use super::*;
    use std::collections::HashMap;
    use std::sync::LazyLock;

    // Constants for NSTrackingAreaOptions
    const NS_TRACKING_MOUSE_ENTERED_AND_EXITED: u64 = 0x01;
    // Deliver mouseMoved: to the owner (the delegate) even while another app
    // is active. This is how the floating bar gets hover feedback on its
    // buttons without being the frontmost app: WKWebView never sees these
    // moves, so we forward the cursor position to the page ourselves.
    const NS_TRACKING_MOUSE_MOVED: u64 = 0x02;
    const NS_TRACKING_ACTIVE_ALWAYS: u64 = 0x80;
    // Track the view's *current* visible rect, not the bounds captured at
    // setup: the floating bar resizes itself (compact → hover → chat pane),
    // and a fixed rect would stop firing entered/exited at the new edges.
    const NS_TRACKING_IN_VISIBLE_RECT: u64 = 0x200;
    const TRACKING_OPTIONS: u64 = NS_TRACKING_MOUSE_ENTERED_AND_EXITED
        | NS_TRACKING_MOUSE_MOVED
        | NS_TRACKING_ACTIVE_ALWAYS
        | NS_TRACKING_IN_VISIBLE_RECT;

    // Throttle forwarded mouse-moved events to ~60/s per window.
    static MOVE_THROTTLE: LazyLock<Mutex<HashMap<u64, std::time::Instant>>> =
        LazyLock::new(|| Mutex::new(HashMap::new()));

    // Static storage for the AppHandle, wrapped for thread safety
    static APP_HANDLE: Mutex<Option<AppHandle>> = Mutex::new(None);

    // Store multiple window labels for tracking - HashMap maps delegate pointer to window label
    static TRACKED_WINDOWS: LazyLock<Mutex<HashMap<u64, String>>> =
        LazyLock::new(|| Mutex::new(HashMap::new()));

    /// Mouse entered event handler - now receives delegate pointer to identify window
    extern "C" fn mouse_entered(this: &Object, _cmd: Sel, _event: cocoa_id) {
        debug!("[Tracking Delegate] Mouse Entered");
        let delegate_ptr = this as *const Object as u64;

        let app_handle = match APP_HANDLE.lock() {
            Ok(handle) => handle.as_ref().cloned(),
            Err(e) => {
                error!(
                    "[Tracking Delegate Error] {}",
                    format_error(templates::FAILED_TO_ACCESS, "APP_HANDLE lock", e)
                );
                return;
            }
        };

        if let Some(handle) = app_handle {
            let window_label = match TRACKED_WINDOWS.lock() {
                Ok(tracked) => tracked.get(&delegate_ptr).cloned(),
                Err(e) => {
                    error!(
                        "[Tracking Delegate Error] Failed to acquire TRACKED_WINDOWS lock: {}",
                        e
                    );
                    return;
                }
            };

            if let Some(window_label) = window_label {
                // A steady bar window is mostly transparent; while it has hit
                // regions, enter and leave mean the drawn content, and
                // `bar_hit_test` reports them instead of this whole-window area.
                if crate::platform::bar_hit_test::owns_hover(&window_label) {
                    return;
                }
                if let Some(window) = handle.get_webview_window(&window_label) {
                    let _ = window.emit(events::system::MOUSE_ENTERED_WINDOW, ()); // Emit specific event
                    debug!(
                        "[Tracking Delegate] Emitted mouse-entered-window for window: {}",
                        window_label
                    );
                } else {
                    error!(
                        "[Tracking Delegate Error] Window '{}' not found for mouse_entered emit.",
                        window_label
                    );
                }
            } else {
                error!(
                    "[Tracking Delegate Error] No window label found for delegate pointer: {}",
                    delegate_ptr
                );
            }
        }
    }

    /// Mouse moved handler — forwards the cursor position (in CSS pixels,
    /// top-left origin) to the window's page so it can show hover states
    /// while Juno is not the active app.
    extern "C" fn mouse_moved(this: &Object, _cmd: Sel, event: cocoa_id) {
        let delegate_ptr = this as *const Object as u64;

        // Throttle to ~60/s per window.
        match MOVE_THROTTLE.lock() {
            Ok(mut last) => {
                let now = std::time::Instant::now();
                if let Some(prev) = last.get(&delegate_ptr) {
                    if now.duration_since(*prev).as_millis() < 16 {
                        return;
                    }
                }
                last.insert(delegate_ptr, now);
            }
            Err(_) => return,
        }

        let app_handle = match APP_HANDLE.lock() {
            Ok(handle) => handle.as_ref().cloned(),
            Err(_) => return,
        };
        let Some(handle) = app_handle else { return };
        let window_label = match TRACKED_WINDOWS.lock() {
            Ok(tracked) => tracked.get(&delegate_ptr).cloned(),
            Err(_) => return,
        };
        let Some(window_label) = window_label else {
            return;
        };
        let Some(window) = handle.get_webview_window(&window_label) else {
            return;
        };
        let ns_window_ptr = match window.ns_window() {
            Ok(ptr) => ptr as cocoa_id,
            Err(_) => return,
        };

        unsafe {
            let view = ns_window_ptr.contentView();
            if view == nil {
                return;
            }
            #[allow(unexpected_cfgs)]
            let location: cocoa::foundation::NSPoint = msg_send![event, locationInWindow];
            #[allow(unexpected_cfgs)]
            let view_point: cocoa::foundation::NSPoint =
                msg_send![view, convertPoint: location fromView: nil];
            #[allow(unexpected_cfgs)]
            let bounds: NSRect = msg_send![view, bounds];
            // AppKit is bottom-left origin; the web page is top-left origin.
            let x = view_point.x;
            let y = bounds.size.height - view_point.y;
            let _ = window.emit(
                events::system::MOUSE_MOVED_WINDOW,
                serde_json::json!({ "x": x, "y": y }),
            );
        }
    }

    /// Mouse exited event handler - now receives delegate pointer to identify window
    extern "C" fn mouse_exited(this: &Object, _cmd: Sel, _event: cocoa_id) {
        debug!("[Tracking Delegate] Mouse Exited");
        let delegate_ptr = this as *const Object as u64;

        let app_handle = match APP_HANDLE.lock() {
            Ok(handle) => handle.as_ref().cloned(),
            Err(e) => {
                error!(
                    "[Tracking Delegate Error] Failed to acquire APP_HANDLE lock: {}",
                    e
                );
                return;
            }
        };

        if let Some(handle) = app_handle {
            let window_label = match TRACKED_WINDOWS.lock() {
                Ok(tracked) => tracked.get(&delegate_ptr).cloned(),
                Err(e) => {
                    error!(
                        "[Tracking Delegate Error] Failed to acquire TRACKED_WINDOWS lock: {}",
                        e
                    );
                    return;
                }
            };

            if let Some(window_label) = window_label {
                // A steady bar window is mostly transparent; while it has hit
                // regions, enter and leave mean the drawn content, and
                // `bar_hit_test` reports them instead of this whole-window area.
                if crate::platform::bar_hit_test::owns_hover(&window_label) {
                    return;
                }
                if let Some(window) = handle.get_webview_window(&window_label) {
                    let _ = window.emit(events::system::MOUSE_LEFT_WINDOW, ()); // Emit specific event
                    debug!(
                        "[Tracking Delegate] Emitted mouse-left-window for window: {}",
                        window_label
                    );
                } else {
                    error!(
                        "[Tracking Delegate Error] Window '{}' not found for mouse_exited emit.",
                        window_label
                    );
                }
            } else {
                error!(
                    "[Tracking Delegate Error] No window label found for delegate pointer: {}",
                    delegate_ptr
                );
            }
        }
    }

    /// Setup mouse tracking area for a window
    pub fn setup_tracking_area(
        window: &tauri::WebviewWindow<tauri::Wry>,
        app_handle: AppHandle,
    ) -> Result<(), String> {
        let window_label = window.label().to_string();
        debug!(
            "Setting up macOS tracking area for window: {}",
            window_label
        );

        // Store the AppHandle (only needs to be set once)
        let should_store_handle = match APP_HANDLE.lock() {
            Ok(handle) => handle.is_none(),
            Err(e) => {
                error!("Failed to check APP_HANDLE status: {}", e);
                return Err(format!("Failed to check APP_HANDLE status: {}", e));
            }
        };

        if should_store_handle {
            match APP_HANDLE.lock() {
                Ok(mut handle) => *handle = Some(app_handle.clone()),
                Err(e) => {
                    error!("Failed to store APP_HANDLE: {}", e);
                    return Err(format!("Failed to store APP_HANDLE: {}", e));
                }
            }
        }

        let ns_window = match window.ns_window() {
            Ok(ptr) => ptr as cocoa_id,
            Err(e) => {
                error!("Failed to get NSWindow for tracking area setup: {}", e);
                return Err(format!(
                    "Failed to get NSWindow for tracking area setup: {}",
                    e
                ));
            }
        };

        unsafe {
            let view = ns_window.contentView();
            if view == nil {
                error!("Failed to get contentView for tracking area setup.");
                return Err("Failed to get contentView for tracking area setup.".to_string());
            }

            // Create a unique delegate class name for this window
            let delegate_class_name =
                format!("MouseTrackingDelegate_{}", window_label.replace("-", "_"));
            let mut delegate_class = Class::get(&delegate_class_name);

            // Declare class only if it doesn't exist yet
            if delegate_class.is_none() {
                debug!("Declaring {} class...", delegate_class_name);
                #[allow(unexpected_cfgs)] // Allow cfg from class! macro
                let superclass = class!(NSObject);
                let mut decl = match ClassDecl::new(&delegate_class_name, superclass) {
                    Some(decl) => decl,
                    None => {
                        error!(
                            "Failed to create Objective-C class declaration for {}",
                            delegate_class_name
                        );
                        return Err("Failed to create delegate class".to_string());
                    }
                };

                // Add mouseEntered: method
                #[allow(unexpected_cfgs)] // Allow cfg from sel! macro
                decl.add_method(
                    sel!(mouseEntered:),
                    mouse_entered as extern "C" fn(&Object, Sel, cocoa_id),
                );

                // Add mouseExited: method
                #[allow(unexpected_cfgs)] // Allow cfg from sel! macro
                decl.add_method(
                    sel!(mouseExited:),
                    mouse_exited as extern "C" fn(&Object, Sel, cocoa_id),
                );

                // Add mouseMoved: method (hover feedback while app is inactive)
                #[allow(unexpected_cfgs)] // Allow cfg from sel! macro
                decl.add_method(
                    sel!(mouseMoved:),
                    mouse_moved as extern "C" fn(&Object, Sel, cocoa_id),
                );

                delegate_class = Some(decl.register());
                debug!("{} class registered.", delegate_class_name);
            }

            #[allow(unexpected_cfgs)] // Allow cfg from msg_send macro
            let delegate: cocoa_id = match delegate_class {
                Some(class) => msg_send![class, new],
                None => {
                    error!("Delegate class was None when trying to create instance");
                    return Err("Failed to get delegate class".to_string());
                }
            };
            debug!("{} instance created: {:?}", delegate_class_name, delegate);

            // Store the mapping between delegate pointer and window label
            let delegate_ptr = delegate as u64;
            match TRACKED_WINDOWS.lock() {
                Ok(mut tracked) => {
                    tracked.insert(delegate_ptr, window_label.clone());
                    debug!(
                        "Registered delegate pointer {} for window: {}",
                        delegate_ptr, window_label
                    );
                }
                Err(e) => {
                    error!(
                        "Failed to register delegate pointer for window '{}': {}",
                        window_label, e
                    );
                    return Err(format!(
                        "Failed to register delegate pointer for window '{}': {}",
                        window_label, e
                    ));
                }
            }

            // Keep the delegate alive. Leaking it here is simpler than complex lifetime management.
            let _ = Box::leak(Box::new(delegate)); // Box the delegate and leak it

            #[allow(unexpected_cfgs)] // Allow cfg from msg_send macro
            let bounds: NSRect = msg_send![view, bounds];
            debug!("Got view bounds for tracking area.");

            #[allow(unexpected_cfgs)] // Allow cfg from msg_send and class! macros
            let tracking_area: cocoa_id = msg_send![class!(NSTrackingArea), alloc];
            #[allow(unexpected_cfgs)] // Allow cfg from msg_send macro
            let tracking_area_ptr: cocoa_id = msg_send![
                tracking_area,
                initWithRect: bounds
                options: TRACKING_OPTIONS
                owner: delegate // Use the delegate instance as the owner
                userInfo: nil
            ];
            debug!("NSTrackingArea created: {:?}", tracking_area_ptr);

            #[allow(unexpected_cfgs)] // Allow cfg from msg_send macro
            let _: () = msg_send![view, addTrackingArea: tracking_area_ptr];
            #[allow(unexpected_cfgs)] // Allow cfg from msg_send macro
            let _: () = msg_send![tracking_area_ptr, release]; // Release after adding (view retains it)
                                                               // Note: Do not release the delegate here, it's leaked via Box::leak

            info!("NSTrackingArea added to view for window: {}", window_label);
        }

        Ok(())
    }
}

/// The macOS setup for a window built after launch by `startup_windows`:
/// what `apply_macos_setup` used to do for it when every declared window
/// existed before setup ran. AppKit, so it hops to the main thread.
#[cfg(target_os = "macos")]
pub fn configure_deferred_window(app_handle: &AppHandle, label: &str) {
    if label == crate::window_management::DESKTOP_CURSOR_OVERLAY_LABEL {
        // Already dispatches to the main thread itself.
        setup_desktop_cursor_overlay_window(app_handle);
        return;
    }
    let app = app_handle.clone();
    let owned = label.to_string();
    if let Err(e) = app_handle.run_on_main_thread(move || {
        if owned == constants::window_labels::FLOATING_PANEL {
            setup_floating_panel_window(&app);
        } else if owned == constants::window_labels::MAIN {
            setup_main_window(&app);
        }
    }) {
        warn!("Could not set up the {} window: {}", label, e);
    }
}

// Non-macOS platforms get stub implementations
#[cfg(not(target_os = "macos"))]
pub fn apply_macos_setup(_app_handle: &AppHandle) {
    // No-op on non-macOS platforms
}

#[cfg(not(target_os = "macos"))]
pub mod mouse_tracking {
    use tauri::AppHandle;

    /// Stub implementation for non-macOS platforms
    pub fn setup_tracking_area(_window: &tauri::WebviewWindow<tauri::Wry>, _app_handle: AppHandle) {
        // No-op on non-macOS platforms
    }
}

/// `NSWindowAbove`: order one window directly above another in the same level.
#[cfg(target_os = "macos")]
const NS_WINDOW_ABOVE: i64 = 1;

/// `NSWindowBelow`: order one window directly below another. The window stays
/// on screen; `NSWindowOut` (0) is the one that takes it away.
#[cfg(target_os = "macos")]
const NS_WINDOW_BELOW: i64 = -1;

/// Put `label` directly above Juno's chat window, without raising its level.
///
/// Showing and focusing a window is not enough on its own. Activating an
/// application makes AppKit re-order that application's windows, and a window
/// built or shown a moment earlier can lose that re-order and come up behind
/// a sibling. Settings opened while the chat window was up is exactly that
/// case, and from a non-activating surface like the floating bar it is the
/// common case rather than the rare one.
///
/// This is deliberately a re-order and not a window level. A level is a
/// system-wide band: anything above `NSNormalWindowLevel` floats over every
/// other application's ordinary windows too, and there is no level that means
/// "above Juno's own windows and nothing else". Settings is not an accessory
/// panel, and the floating bar already recorded what too high a level costs:
/// at level 5 it sat above a system permission prompt, which arrived behind
/// Juno unreadable and unclickable. Settings is where the permission rows
/// live, so it is the last window that should be able to cover that prompt.
///
/// Ordering is relative to Juno's own chat window, so the rest of the system
/// is untouched: click another app and settings goes behind it, as a settings
/// window should.
///
/// This raises a window above the *chat* window and nothing more. The floating
/// bar is at a higher level than either of them, so on its own this left
/// settings above the chat window and still underneath the bar. Lowering the
/// bar is `crate::bar_stacking`'s job, and the window being raised here reports
/// itself to it.
#[cfg(target_os = "macos")]
pub fn raise_above_chat_window(app_handle: &AppHandle, label: &str) {
    let Some(window) = app_handle.get_webview_window(label) else {
        return;
    };
    let Ok(window_ptr) = window.ns_window() else {
        return;
    };

    // No chat window on screen means there is nothing to get out from under,
    // and ordering relative to a window that is not there would be a no-op
    // with a worse failure mode than skipping.
    let chat_ptr = app_handle
        .get_webview_window(constants::window_labels::MAIN)
        .filter(|chat| chat.is_visible().unwrap_or(false))
        .and_then(|chat| chat.ns_window().ok());

    let window_addr = window_ptr as usize;
    let chat_addr = chat_ptr.map(|ptr| ptr as usize);

    // NSWindow ordering is main-thread-only, and this is called from an async
    // command. Queueing it also means it runs after the activation AppKit is
    // part-way through, which is the re-order being corrected.
    if let Err(e) = app_handle.run_on_main_thread(move || unsafe {
        let ns_window = window_addr as cocoa_id;
        if let Some(chat_addr) = chat_addr {
            let chat = chat_addr as cocoa_id;
            #[allow(unexpected_cfgs)]
            let chat_number: i64 = msg_send![chat, windowNumber];
            #[allow(unexpected_cfgs)]
            let _: () = msg_send![ns_window, orderWindow: NS_WINDOW_ABOVE relativeTo: chat_number];
        }
        // Key status last: ordering alone leaves the keyboard pointed at
        // whatever had it, so typing would land in the window underneath.
        #[allow(unexpected_cfgs)]
        let _: () = msg_send![ns_window, makeKeyAndOrderFront: nil];
    }) {
        warn!(
            "Could not raise the {} window above the chat window: {}",
            label, e
        );
    }
}

#[cfg(not(target_os = "macos"))]
pub fn raise_above_chat_window(_app_handle: &AppHandle, _label: &str) {}
