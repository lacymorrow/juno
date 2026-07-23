//! # macOS Platform Module
//!
//! This module contains all macOS-specific functionality for the Juno application.
//! It handles window styling, mouse tracking, accessibility features, and other
//! Cocoa/AppKit integrations needed for proper macOS behavior.

use tauri::{AppHandle, Manager, Emitter};
use tracing::{debug, info, warn, error};
use crate::constants;
use crate::constants::{events, errors::templates};

// Helper function for error formatting - properly handles template substitution
fn format_error(template: &str, context: &str, error: impl std::fmt::Display) -> String {
    template.replacen("{}", context, 1).replacen("{}", &error.to_string(), 1)
}

// macOS-specific imports - only available on macOS
#[cfg(target_os = "macos")]
use {
    cocoa::{
        appkit::{NSWindow, NSWindowCollectionBehavior},
        base::{id as cocoa_id, nil, NO, YES, BOOL},
        foundation::NSRect,
    },
    objc::{
        class, msg_send,
        runtime::{Class, Object, Sel},
        sel, sel_impl,
        declare::ClassDecl
    },
    std::sync::Mutex,
};

/// Apply comprehensive macOS-specific setup for all application windows
pub fn apply_macos_setup(app_handle: &AppHandle) {
    #[cfg(target_os = "macos")]
    {
        info!("Applying macOS specific setup...");

        // Setup floating bar window
        setup_floating_bar_window(app_handle);

        // Setup floating panel window
        setup_floating_panel_window(app_handle);

        // Setup main window
        setup_main_window(app_handle);

        // Setup desktop cursor overlay (pre-created in config as visible:false)
        setup_desktop_cursor_overlay_window(app_handle);

        info!("macOS specific setup completed");
    }

    #[cfg(not(target_os = "macos"))]
    {
        // No-op on non-macOS platforms
        info!("macOS setup skipped on non-macOS platform");
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
                unsafe {
                    // Keep window floating above others
                    ns_window.setLevel_(5);
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

        // Ensure proper window activation
        activate_floating_bar_window(window);
    } else {
        error!("Warning: floating-bar window not found during macOS specific setup.");
    }
}

/// Setup macOS-specific styling and behavior for the floating panel window
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
                    let _: () = msg_send![ns_window, setAccessibilityRole: accessibility_role_string];

                    #[allow(unexpected_cfgs)]
                    let accessibility_label_string: cocoa_id = msg_send![class!(NSString), stringWithUTF8String: "Juno AI Assistant Panel".as_ptr()];
                    let _: () = msg_send![ns_window, setAccessibilityLabel: accessibility_label_string];

                    info!("macOS Setup: Floating panel configured.");
                }
            }
            Err(e) => {
                error!("Error getting NSWindow for styling floating-panel: {}", e);
            }
        }

        // Setup mouse tracking for floating panel
        if let Err(e) = mouse_tracking::setup_tracking_area(&window, app_handle.clone()) {
            error!("Failed to setup mouse tracking area for floating panel: {}", e);
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
///
/// This window shows the AI agent's cursor position during computer-use tasks.
/// It must be:
///   - At NSScreenSaverWindowLevel (1000) so it appears above all app content
///   - Fully click-through (ignoresMouseEvents) so it never blocks user input
///   - Non-activating: must not steal focus from the user's active application
///   - Persistent across spaces and full-screen apps
#[cfg(target_os = "macos")]
fn setup_desktop_cursor_overlay_window(app_handle: &AppHandle) {
    if let Some(window) = app_handle.get_webview_window("desktop-cursor-overlay") {
        info!("Found desktop-cursor-overlay for macOS setup.");

        match window.ns_window() {
            Ok(ns_window_ptr) => {
                let ns_window = ns_window_ptr as cocoa_id;
                unsafe {
                    // NSScreenSaverWindowLevel (1000) — above all app content including full-screen
                    ns_window.setLevel_(1000);

                    // Fully click-through: agent cursor must never intercept user input
                    #[allow(unexpected_cfgs)]
                    let _: BOOL = msg_send![ns_window, setIgnoresMouseEvents: YES];

                    // Visible across all spaces and above full-screen apps
                    ns_window.setCollectionBehavior_(
                        NSWindowCollectionBehavior::NSWindowCollectionBehaviorCanJoinAllSpaces |
                        NSWindowCollectionBehavior::NSWindowCollectionBehaviorStationary |
                        NSWindowCollectionBehavior::NSWindowCollectionBehaviorFullScreenAuxiliary |
                        NSWindowCollectionBehavior::NSWindowCollectionBehaviorIgnoresCycle
                    );

                    // Stay visible when user switches to another app
                    ns_window.setHidesOnDeactivate_(NO);

                    ns_window.setOpaque_(NO);
                    ns_window.setHasShadow_(NO);

                    info!("macOS Setup: Desktop cursor overlay configured at screen-saver level, fully click-through.");
                }
            }
            Err(e) => {
                error!("Error getting NSWindow for styling desktop-cursor-overlay: {}", e);
            }
        }
    } else {
        // Window is pre-created as visible:false in tauri.conf.json, so this should not happen
        // at app startup. Log as info (not error) since it may not be created yet in all builds.
        info!("desktop-cursor-overlay not found during macOS setup — will be configured on first open.");
    }
}

/// Show the floating bar without stealing application focus.
///
/// Overlay windows must never call set_focus()/makeKeyAndOrderFront: — that
/// triggers [NSApp activateIgnoringOtherApps:YES] which yanks keyboard focus
/// away from whatever app the user is currently in.  orderFront: (via show())
/// makes the window visible without changing the active application.
#[cfg(target_os = "macos")]
fn activate_floating_bar_window(window: tauri::WebviewWindow<tauri::Wry>) {
    tauri::async_runtime::spawn(async move {
        // Small delay to ensure window setup is complete
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Only show — never steal focus. Overlays must not activate Juno.
        if let Err(e) = window.show() {
            warn!("{}", format_error(templates::FAILED_TO_PROCESS, "show floating bar window", e));
        } else {
            info!("Floating bar window shown successfully (no focus steal)");
        }
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
    const NS_TRACKING_ACTIVE_ALWAYS: u64 = 0x80;
    const TRACKING_OPTIONS: u64 = NS_TRACKING_MOUSE_ENTERED_AND_EXITED | NS_TRACKING_ACTIVE_ALWAYS;

    // Static storage for the AppHandle, wrapped for thread safety
    static APP_HANDLE: Mutex<Option<AppHandle>> = Mutex::new(None);

    // Store multiple window labels for tracking - HashMap maps delegate pointer to window label
    static TRACKED_WINDOWS: LazyLock<Mutex<HashMap<u64, String>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

    /// Mouse entered event handler - now receives delegate pointer to identify window
    extern "C" fn mouse_entered(this: &Object, _cmd: Sel, _event: cocoa_id) {
        debug!("[Tracking Delegate] Mouse Entered");
        let delegate_ptr = this as *const Object as u64;

        let app_handle = match APP_HANDLE.lock() {
            Ok(handle) => handle.as_ref().cloned(),
            Err(e) => {
                error!("[Tracking Delegate Error] {}", format_error(templates::FAILED_TO_ACCESS, "APP_HANDLE lock", e));
                return;
            }
        };

        if let Some(handle) = app_handle {
            let window_label = match TRACKED_WINDOWS.lock() {
                Ok(tracked) => tracked.get(&delegate_ptr).cloned(),
                Err(e) => {
                    error!("[Tracking Delegate Error] Failed to acquire TRACKED_WINDOWS lock: {}", e);
                    return;
                }
            };

            if let Some(window_label) = window_label {
                if let Some(window) = handle.get_webview_window(&window_label) {
                    let _ = window.emit(events::system::MOUSE_ENTERED_WINDOW, ()); // Emit specific event
                    debug!("[Tracking Delegate] Emitted mouse-entered-window for window: {}", window_label);
                } else {
                    error!("[Tracking Delegate Error] Window '{}' not found for mouse_entered emit.", window_label);
                }
            } else {
                error!("[Tracking Delegate Error] No window label found for delegate pointer: {}", delegate_ptr);
            }
        }
    }

    /// Mouse exited event handler - now receives delegate pointer to identify window
    extern "C" fn mouse_exited(this: &Object, _cmd: Sel, _event: cocoa_id) {
        debug!("[Tracking Delegate] Mouse Exited");
        let delegate_ptr = this as *const Object as u64;

        let app_handle = match APP_HANDLE.lock() {
            Ok(handle) => handle.as_ref().cloned(),
            Err(e) => {
                error!("[Tracking Delegate Error] Failed to acquire APP_HANDLE lock: {}", e);
                return;
            }
        };

        if let Some(handle) = app_handle {
            let window_label = match TRACKED_WINDOWS.lock() {
                Ok(tracked) => tracked.get(&delegate_ptr).cloned(),
                Err(e) => {
                    error!("[Tracking Delegate Error] Failed to acquire TRACKED_WINDOWS lock: {}", e);
                    return;
                }
            };

            if let Some(window_label) = window_label {
                if let Some(window) = handle.get_webview_window(&window_label) {
                    let _ = window.emit(events::system::MOUSE_LEFT_WINDOW, ()); // Emit specific event
                    debug!("[Tracking Delegate] Emitted mouse-left-window for window: {}", window_label);
                } else {
                    error!("[Tracking Delegate Error] Window '{}' not found for mouse_exited emit.", window_label);
                }
            } else {
                error!("[Tracking Delegate Error] No window label found for delegate pointer: {}", delegate_ptr);
            }
        }
    }

    /// Setup mouse tracking area for a window
    pub fn setup_tracking_area(window: &tauri::WebviewWindow<tauri::Wry>, app_handle: AppHandle) -> Result<(), String> {
        let window_label = window.label().to_string();
        debug!("Setting up macOS tracking area for window: {}", window_label);

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
                return Err(format!("Failed to get NSWindow for tracking area setup: {}", e));
            }
        };

        unsafe {
            let view = ns_window.contentView();
            if view == nil {
                error!("Failed to get contentView for tracking area setup.");
                return Err("Failed to get contentView for tracking area setup.".to_string());
            }

            // Create a unique delegate class name for this window
            let delegate_class_name = format!("MouseTrackingDelegate_{}", window_label.replace("-", "_"));
            let mut delegate_class = Class::get(&delegate_class_name);

            // Declare class only if it doesn't exist yet
            if delegate_class.is_none() {
                debug!("Declaring {} class...", delegate_class_name);
                #[allow(unexpected_cfgs)] // Allow cfg from class! macro
                let superclass = class!(NSObject);
                let mut decl = match ClassDecl::new(&delegate_class_name, superclass) {
                    Some(decl) => decl,
                    None => {
                        error!("Failed to create Objective-C class declaration for {}", delegate_class_name);
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
                    debug!("Registered delegate pointer {} for window: {}", delegate_ptr, window_label);
                }
                Err(e) => {
                    error!("Failed to register delegate pointer for window '{}': {}", window_label, e);
                    return Err(format!("Failed to register delegate pointer for window '{}': {}", window_label, e));
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

/// Pure layout math for the notch bar appearance (LAC-3037).
///
/// Shared by the macOS window-positioning code below and unit-testable on any
/// platform. All values are logical points (AppKit coordinates).
pub mod notch_layout {
    use serde::Serialize;

    /// Geometry of the notch (or fallback pill) reported to the frontend so it
    /// can draw a silhouette that lines up with the hardware cutout.
    #[derive(Debug, Clone, Copy, Serialize)]
    pub struct NotchGeometry {
        pub has_notch: bool,
        /// Width of the physical notch cutout, or the fallback pill width on
        /// notch-less displays.
        pub notch_width: f64,
        /// Height of the cutout including the bottom cover fudge, or the menu
        /// bar height on notch-less displays.
        pub notch_height: f64,
        pub menu_bar_height: f64,
        /// Fixed webview canvas size. The window is sized once to the largest
        /// state plus slack and never resized per state — per-state window
        /// resizing makes the bar visibly detach from the bezel mid-animation,
        /// so all state transitions are CSS-only inside this canvas.
        pub canvas_width: f64,
        pub canvas_height: f64,
    }

    /// Idle pill size on notch-less displays — the option degrades gracefully
    /// instead of being gated on hardware.
    pub const FALLBACK_PILL_WIDTH: f64 = 200.0;
    pub const FALLBACK_PILL_HEIGHT: f64 = 30.0;

    /// NSScreen reports the safe area slightly shallower than the real cutout;
    /// extend the cover to avoid a hairline gap at the bottom edge.
    pub const NOTCH_BOTTOM_COVER: f64 = 2.0;

    /// Slack around the largest expanded state so every CSS transition fits
    /// inside the fixed canvas: total horizontal / vertical below the notch.
    const CANVAS_HORIZONTAL_SLACK: f64 = 280.0;
    const CANVAS_VERTICAL_SLACK: f64 = 150.0;

    /// NSMainMenuWindowLevel (24) + 3 — above the menu bar.
    pub const NOTCH_WINDOW_LEVEL: i64 = 27;

    pub fn build_geometry(
        has_notch: bool,
        raw_notch_width: f64,
        safe_area_top: f64,
        menu_bar_height: f64,
    ) -> NotchGeometry {
        let (notch_width, notch_height) = if has_notch {
            // A zero raw width means the auxiliary-area API was unavailable —
            // fall back to a typical cutout width rather than a sliver.
            (
                raw_notch_width.max(FALLBACK_PILL_WIDTH),
                safe_area_top + NOTCH_BOTTOM_COVER,
            )
        } else {
            (
                FALLBACK_PILL_WIDTH,
                menu_bar_height.max(FALLBACK_PILL_HEIGHT),
            )
        };
        NotchGeometry {
            has_notch,
            notch_width,
            notch_height,
            menu_bar_height,
            canvas_width: notch_width + CANVAS_HORIZONTAL_SLACK,
            canvas_height: notch_height + CANVAS_VERTICAL_SLACK,
        }
    }

    /// Top-flush frame centered on the notch, in AppKit screen coordinates
    /// (origin bottom-left, y-up). Returns (x, y, width, height).
    pub fn canvas_frame(
        screen_x: f64,
        screen_y: f64,
        screen_width: f64,
        screen_height: f64,
        geometry: &NotchGeometry,
    ) -> (f64, f64, f64, f64) {
        (
            screen_x + (screen_width - geometry.canvas_width) / 2.0,
            screen_y + screen_height - geometry.canvas_height,
            geometry.canvas_width,
            geometry.canvas_height,
        )
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn notched_screen_uses_reported_cutout_plus_cover() {
            // MacBook Pro 14" style: notch ~200pt wide, safe area 32pt deep
            let geo = build_geometry(true, 210.0, 32.0, 32.0);
            assert!(geo.has_notch);
            assert_eq!(geo.notch_width, 210.0);
            assert_eq!(geo.notch_height, 32.0 + NOTCH_BOTTOM_COVER);
            assert!(geo.canvas_width > geo.notch_width);
            assert!(geo.canvas_height > geo.notch_height);
        }

        #[test]
        fn notch_with_unknown_width_falls_back_to_pill_width() {
            // safeAreaInsets.top > 0 but auxiliary areas unavailable
            let geo = build_geometry(true, 0.0, 32.0, 32.0);
            assert_eq!(geo.notch_width, FALLBACK_PILL_WIDTH);
        }

        #[test]
        fn notchless_screen_gets_centered_pill_from_menu_bar_height() {
            let geo = build_geometry(false, 0.0, 0.0, 30.0);
            assert!(!geo.has_notch);
            assert_eq!(geo.notch_width, FALLBACK_PILL_WIDTH);
            assert_eq!(geo.notch_height, 30.0);
        }

        #[test]
        fn notchless_tiny_menu_bar_clamps_to_minimum_pill_height() {
            let geo = build_geometry(false, 0.0, 0.0, 22.0);
            assert_eq!(geo.notch_height, FALLBACK_PILL_HEIGHT);
        }

        #[test]
        fn canvas_frame_is_top_flush_and_horizontally_centered() {
            let geo = build_geometry(true, 200.0, 32.0, 32.0);
            // Primary screen 1728x1117 at origin (0,0)
            let (x, y, w, h) = canvas_frame(0.0, 0.0, 1728.0, 1117.0, &geo);
            // Centered: equal margins either side
            assert!((x - (1728.0 - w) / 2.0).abs() < f64::EPSILON);
            // Top-flush: frame max-y equals screen max-y (AppKit y-up coords)
            assert!((y + h - 1117.0).abs() < f64::EPSILON);
        }

        #[test]
        fn canvas_frame_respects_secondary_screen_origin() {
            let geo = build_geometry(false, 0.0, 0.0, 24.0);
            let (x, y, _w, h) = canvas_frame(-1920.0, 200.0, 1920.0, 1080.0, &geo);
            assert!(x > -1920.0 && x < 0.0);
            assert!((y + h - (200.0 + 1080.0)).abs() < f64::EPSILON);
        }
    }
}

/// macOS window operations for the notch bar appearance (LAC-3037).
///
/// A prototype confirmed a plain borderless NSWindow at level 27 is NOT pushed
/// below the menu bar by `constrainFrameRect:toScreen:` when positioned
/// programmatically, so no NSPanel conversion or dynamic subclass is needed.
/// Placement is simply re-applied on every mode entry as self-healing.
#[cfg(target_os = "macos")]
pub mod notch {
    use super::*;
    use super::notch_layout::{self, NotchGeometry};
    use cocoa::foundation::{NSPoint, NSSize};

    /// NSEdgeInsets is not defined in the cocoa crate; declare it with the
    /// matching Objective-C type encoding so msg_send! can return it by value.
    #[repr(C)]
    #[derive(Clone, Copy, Debug, Default)]
    struct NSEdgeInsets {
        top: f64,
        left: f64,
        bottom: f64,
        right: f64,
    }

    unsafe impl objc::Encode for NSEdgeInsets {
        fn encode() -> objc::Encoding {
            unsafe { objc::Encoding::from_str("{NSEdgeInsets=dddd}") }
        }
    }

    /// Default floating-bar frame from tauri.conf.json, restored when the user
    /// switches away from the notch appearance.
    const DEFAULT_BAR_WIDTH: f64 = 419.0;
    const DEFAULT_BAR_HEIGHT: f64 = 92.0;
    /// Distance below the top of the screen for the restored floating bar.
    const RESTORE_TOP_OFFSET: f64 = 120.0;
    /// NSFloatingWindowLevel — the floating bar's normal level.
    const FLOATING_WINDOW_LEVEL: i64 = 5;

    /// Read notch geometry and the frame of the screen it anchors to.
    /// Prefers the screen with a notch (the built-in display); otherwise the
    /// primary screen (index 0 — the one that owns the menu bar).
    ///
    /// Must be called on the main thread (AppKit requirement).
    unsafe fn read_geometry() -> (NotchGeometry, NSRect) {
        let screens: cocoa_id = msg_send![class!(NSScreen), screens];
        let count: usize = msg_send![screens, count];
        if count == 0 {
            // Headless / display disconnected — report a fallback pill.
            let geometry = notch_layout::build_geometry(false, 0.0, 0.0, 24.0);
            let zero = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(0.0, 0.0));
            return (geometry, zero);
        }

        let mut target: cocoa_id = msg_send![screens, objectAtIndex: 0usize];
        let mut safe_top = 0.0f64;
        for i in 0..count {
            let screen: cocoa_id = msg_send![screens, objectAtIndex: i];
            // safeAreaInsets requires macOS 12+; older systems take the
            // fallback pill path below.
            let responds: BOOL = msg_send![screen, respondsToSelector: sel!(safeAreaInsets)];
            if responds == YES {
                let insets: NSEdgeInsets = msg_send![screen, safeAreaInsets];
                if insets.top > 0.0 {
                    target = screen;
                    safe_top = insets.top;
                    break;
                }
            }
        }

        let frame: NSRect = msg_send![target, frame];
        let visible: NSRect = msg_send![target, visibleFrame];
        // visibleFrame excludes the menu bar at the top (and the dock at the
        // bottom); only the top edges matter here.
        let menu_bar_height =
            (frame.origin.y + frame.size.height) - (visible.origin.y + visible.size.height);

        let has_notch = safe_top > 0.0;
        let mut raw_width = 0.0f64;
        if has_notch {
            // Notch width = screen width minus the auxiliary areas either side
            // of the cutout. Zero-sized rects mean the API gave us nothing —
            // build_geometry falls back to a typical width.
            let responds: BOOL =
                msg_send![target, respondsToSelector: sel!(auxiliaryTopLeftArea)];
            if responds == YES {
                let left: NSRect = msg_send![target, auxiliaryTopLeftArea];
                let right: NSRect = msg_send![target, auxiliaryTopRightArea];
                if left.size.width > 0.0 && right.size.width > 0.0 {
                    raw_width = frame.size.width - left.size.width - right.size.width;
                }
            }
        }

        (
            notch_layout::build_geometry(has_notch, raw_width, safe_top, menu_bar_height),
            frame,
        )
    }

    /// macOS silently drops CanJoinAllSpaces when a window is re-ordered, so
    /// the overlay behavior must be re-asserted on every mode change.
    unsafe fn reassert_overlay_collection_behavior(ns_window: cocoa_id) {
        ns_window.setCollectionBehavior_(
            NSWindowCollectionBehavior::NSWindowCollectionBehaviorCanJoinAllSpaces
                | NSWindowCollectionBehavior::NSWindowCollectionBehaviorStationary
                | NSWindowCollectionBehavior::NSWindowCollectionBehaviorIgnoresCycle
                | NSWindowCollectionBehavior::NSWindowCollectionBehaviorFullScreenAuxiliary,
        );
    }

    /// Position the floating bar as a notch bar: window level above the menu
    /// bar, fixed canvas top-flush over the notch (or centered pill fallback).
    /// Main thread only.
    pub fn enter_notch_mode(app_handle: &AppHandle) {
        let Some(window) = app_handle.get_webview_window(constants::window_labels::FLOATING_BAR)
        else {
            warn!("Notch mode: floating-bar window not found");
            return;
        };
        match window.ns_window() {
            Ok(ns_window_ptr) => unsafe {
                let ns_window = ns_window_ptr as cocoa_id;
                let (geometry, screen_frame) = read_geometry();
                let (x, y, w, h) = notch_layout::canvas_frame(
                    screen_frame.origin.x,
                    screen_frame.origin.y,
                    screen_frame.size.width,
                    screen_frame.size.height,
                    &geometry,
                );
                ns_window.setLevel_(notch_layout::NOTCH_WINDOW_LEVEL);
                reassert_overlay_collection_behavior(ns_window);
                let rect = NSRect::new(NSPoint::new(x, y), NSSize::new(w, h));
                ns_window.setFrame_display_(rect, YES);
                info!(
                    "Notch mode entered: canvas ({:.0},{:.0}) {:.0}x{:.0}, has_notch={}",
                    x, y, w, h, geometry.has_notch
                );
            },
            Err(e) => error!("Notch mode: failed to get NSWindow: {}", e),
        }
    }

    /// Restore the floating bar's normal level and default frame after leaving
    /// the notch appearance. Main thread only.
    pub fn exit_notch_mode(app_handle: &AppHandle) {
        let Some(window) = app_handle.get_webview_window(constants::window_labels::FLOATING_BAR)
        else {
            warn!("Notch mode exit: floating-bar window not found");
            return;
        };
        match window.ns_window() {
            Ok(ns_window_ptr) => unsafe {
                let ns_window = ns_window_ptr as cocoa_id;
                let (_, screen_frame) = read_geometry();
                let x = screen_frame.origin.x
                    + (screen_frame.size.width - DEFAULT_BAR_WIDTH) / 2.0;
                let y = screen_frame.origin.y + screen_frame.size.height
                    - DEFAULT_BAR_HEIGHT
                    - RESTORE_TOP_OFFSET;
                ns_window.setLevel_(FLOATING_WINDOW_LEVEL);
                reassert_overlay_collection_behavior(ns_window);
                let rect = NSRect::new(
                    NSPoint::new(x, y),
                    NSSize::new(DEFAULT_BAR_WIDTH, DEFAULT_BAR_HEIGHT),
                );
                ns_window.setFrame_display_(rect, YES);
                info!("Notch mode exited: floating bar restored to default frame");
            },
            Err(e) => error!("Notch mode exit: failed to get NSWindow: {}", e),
        }
    }

    async fn run_on_main(app_handle: AppHandle, f: fn(&AppHandle)) -> Result<(), String> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let handle = app_handle.clone();
        app_handle
            .run_on_main_thread(move || {
                f(&handle);
                let _ = tx.send(());
            })
            .map_err(|e| format!("Failed to schedule on main thread: {}", e))?;
        rx.await
            .map_err(|e| format!("Main-thread task dropped: {}", e))
    }

    /// Async wrapper for command / event-loop contexts.
    pub async fn enter_notch_mode_async(app_handle: AppHandle) -> Result<(), String> {
        run_on_main(app_handle, enter_notch_mode).await
    }

    /// Async wrapper for command / event-loop contexts.
    pub async fn exit_notch_mode_async(app_handle: AppHandle) -> Result<(), String> {
        run_on_main(app_handle, exit_notch_mode).await
    }

    /// Read the current notch geometry from any async context.
    pub async fn get_notch_geometry_async(app_handle: AppHandle) -> Result<NotchGeometry, String> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        app_handle
            .run_on_main_thread(move || {
                let geometry = unsafe { read_geometry().0 };
                let _ = tx.send(geometry);
            })
            .map_err(|e| format!("Failed to schedule on main thread: {}", e))?;
        rx.await
            .map_err(|e| format!("Main-thread task dropped: {}", e))
    }
}

/// Non-macOS stubs — the notch appearance is a no-op elsewhere but the option
/// stays selectable, rendering the fallback pill inside the normal window.
#[cfg(not(target_os = "macos"))]
pub mod notch {
    use super::notch_layout::{self, NotchGeometry};
    use tauri::AppHandle;

    pub async fn enter_notch_mode_async(_app_handle: AppHandle) -> Result<(), String> {
        Ok(())
    }

    pub async fn exit_notch_mode_async(_app_handle: AppHandle) -> Result<(), String> {
        Ok(())
    }

    pub async fn get_notch_geometry_async(
        _app_handle: AppHandle,
    ) -> Result<NotchGeometry, String> {
        Ok(notch_layout::build_geometry(false, 0.0, 0.0, 24.0))
    }
}

/// Escape key monitor stub for onboarding.
///
/// During onboarding, the global shortcut handler in `shortcuts.rs` already
/// detects escape presses and emits visual feedback events (via
/// `handle_escape_key_shortcut`). When `is_onboarding_active()` is true,
/// it emits the event and returns early without triggering stop operations.
///
/// A previous implementation used a CGEventTap with `kCGEventTapOptionListenOnly`
/// to observe escape without capturing it (allowing HTML dropdowns and other
/// apps to also receive the key). That approach was removed because
/// `CGEventTapCreate` can segfault on certain macOS versions/permission states,
/// causing a full process crash during onboarding initialization.
///
/// The current approach: escape IS captured by the global shortcut during
/// onboarding (preventing passthrough to other apps), but since the user is
/// in a focused onboarding wizard, this trade-off is acceptable.
pub mod escape_key_monitor {
    use tauri::AppHandle;
    use tracing::info;

    /// No-op: escape detection during onboarding is handled by the global
    /// shortcut handler in `shortcuts.rs`.
    pub fn start(_app_handle: &AppHandle) -> Result<(), String> {
        info!("[EscapeKeyMonitor] Using global shortcut handler for escape detection during onboarding");
        Ok(())
    }

    /// No-op cleanup.
    pub fn stop() {}

    /// Always returns false — no separate monitor running.
    pub fn is_running() -> bool {
        false
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
