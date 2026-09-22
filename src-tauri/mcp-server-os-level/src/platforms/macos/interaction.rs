use super::constants::*;
use super::display::{
    adjust_coordinates_for_display, get_displays_debug_info, get_pid_at_screen_point,
};
use super::element::MacOSUIElement;
use super::ffi;
use super::memory_safety::get_pooled_event_source;
use super::wrappers::ThreadSafeAXUIElement;
use crate::background;
use crate::element::UIElementImpl; // Needed for app_attributes in click_auto
use crate::input_tier::InputOutcome;
use crate::{AutomationError, ClickResult};
use accessibility::{AXAttribute, AXUIElement};
use accessibility_sys::{AXUIElementRef, AXUIElementSetAttributeValue};
use core_foundation::base::{CFTypeRef, TCFType};
use core_foundation::string::{CFString, CFStringRef};
use core_graphics::event::{
    CGEvent, CGEventFlags, CGEventTapLocation, CGEventType, CGKeyCode, CGMouseButton,
};
use core_graphics::geometry::CGPoint;
use foreign_types::ForeignType;
use std::collections::HashMap;
use std::os::raw::c_void;
use std::sync::OnceLock;
use std::thread;
use std::time::Duration;
use tracing::{debug, warn};
// Removed objc imports - now using arboard for clipboard
use arboard::Clipboard;

// Define key code constants for keyboard shortcuts
const KEYCODE_CMD: CGKeyCode = 55; // Left Command key
const KEYCODE_A: CGKeyCode = 0; // 'A' key

// Timing constants for UI automation delays (milliseconds)
const MOUSE_EVENT_DELAY_MS: u64 = 50; // Delay between mouse events (move, down, up)
const KEY_EVENT_DELAY_MS: u64 = 50; // Delay between key press/release events
const MOUSE_MOVE_STEP_DELAY_MS: u64 = 20; // Delay between mouse move interpolation steps
const DRAG_HOLD_DELAY_MS: u64 = 100; // Delay to hold before/after drag operations
const APP_ACTIVATION_DELAY_MS: u64 = 100; // Delay after app activation

/// Helper function to create multi-monitor aware CGPoint
/// Adjusts coordinates for the appropriate display and logs any changes
fn create_adjusted_point(x: f64, y: f64) -> Result<CGPoint, AutomationError> {
    let (adjusted_x, adjusted_y) = adjust_coordinates_for_display(x, y, None)?;

    // Log coordinate adjustment for debugging multi-monitor issues
    if x != adjusted_x || y != adjusted_y {
        debug!(
            "Multi-monitor coordinate adjustment: ({}, {}) → ({}, {})",
            x, y, adjusted_x, adjusted_y
        );
        tracing::trace!("Display info: {}", get_displays_debug_info());
    }

    Ok(CGPoint::new(adjusted_x, adjusted_y))
}

// Native clipboard implementation using arboard (modern, cross-platform)
struct NativeClipboard {
    clipboard: Clipboard,
}

impl NativeClipboard {
    fn new() -> Result<Self, AutomationError> {
        let clipboard = Clipboard::new().map_err(|e| {
            AutomationError::PlatformError(format!("Failed to initialize clipboard: {}", e))
        })?;
        Ok(NativeClipboard { clipboard })
    }

    fn read(&self) -> Result<String, AutomationError> {
        // Create a new clipboard instance for reading since arboard methods take &mut self
        let mut clipboard = Clipboard::new().map_err(|e| {
            AutomationError::PlatformError(format!("Failed to access clipboard for reading: {}", e))
        })?;

        clipboard.get_text().map_err(|e| {
            AutomationError::PlatformError(format!("Failed to read from clipboard: {}", e))
        })
    }

    fn write(&mut self, content: String) -> Result<(), AutomationError> {
        self.clipboard.set_text(content).map_err(|e| {
            AutomationError::PlatformError(format!("Failed to write to clipboard: {}", e))
        })
    }
}

// --- Moved from element.rs --- //

pub(crate) fn get_application(element: &MacOSUIElement) -> Option<MacOSUIElement> {
    let attr = AXAttribute::new(&CFString::new("AXTopLevelUIElement"));
    match element.element.0.attribute(&attr) {
        Ok(value) => value.downcast::<AXUIElement>().map(|app| MacOSUIElement {
            element: ThreadSafeAXUIElement::new(app),
            use_background_apps: element.use_background_apps,
            activate_app: element.activate_app,
            cached_role: String::new(),
            cached_label: None,
            cached_description: None,
            cached_value: None,
        }),
        Err(_) => None,
    }
}

pub(crate) fn click_with_method(element: &MacOSUIElement) -> Result<ClickResult, AutomationError> {
    click_auto(element)
}

pub(crate) fn click_auto(element: &MacOSUIElement) -> Result<ClickResult, AutomationError> {
    if let Some(app) = get_application(element) {
        let app_attributes = UIElementImpl::attributes(&app); // Need UIElementImpl trait here
        let app_name = app_attributes.label.unwrap_or_default().to_lowercase();
        debug!("detected application: {}", app_name);
        if app_name.contains("chrome")
            || app_name.contains("safari")
            || app_name.contains("arc")
            || app_name.contains("firefox")
            || app_name.contains("edge")
            || app_name.contains("brave")
            || app_name.contains("opera")
            || app_name.contains("vivaldi")
            || app_name.contains("microsoft edge")
        {
            // Browsers ignore AXPress on web content, so the click has to be a real
            // event. In background mode that event goes to the browser process
            // (with the Chromium activation primer) instead of the HID tap, which
            // leaves the user's cursor and frontmost app alone.
            if background::is_background_mode() {
                match click_element_no_warp(element) {
                    Ok(result) => return Ok(result),
                    Err(e) => debug!(
                        "browser detected, no-warp click unavailable ({:?}), using mouse simulation",
                        e
                    ),
                }
            }
            debug!("browser detected, using mouse simulation directly");
            return click_mouse_simulation(element);
        }
    }
    match click_press(element) {
        Ok(result) => return Ok(result),
        Err(e) => debug!("AXPress failed: {:?}, trying alternative methods", e),
    }
    match click_accessibility_click(element) {
        Ok(result) => return Ok(result),
        Err(e) => debug!("AXClick failed: {:?}, trying alternative methods", e),
    }
    click_mouse_simulation(element)
}

pub(crate) fn click_press(element: &MacOSUIElement) -> Result<ClickResult, AutomationError> {
    let press_attr = AXAttribute::new(&CFString::new("AXPress"));
    match element.element.0.perform_action(press_attr.as_CFString()) {
        Ok(_) => {
            debug!("Successfully clicked element with AXPress");
            Ok(ClickResult {
                method: "AXPress".to_string(),
                coordinates: None,
                details: "Used accessibility AXPress action".to_string(),
            })
        }
        Err(e) => Err(AutomationError::PlatformError(format!(
            "AXPress click failed: {:?}",
            e
        ))),
    }
}

pub(crate) fn click_accessibility_click(
    element: &MacOSUIElement,
) -> Result<ClickResult, AutomationError> {
    let click_attr = AXAttribute::new(&CFString::new("AXClick"));
    match element.element.0.perform_action(click_attr.as_CFString()) {
        Ok(_) => {
            debug!("Successfully clicked element with AXClick");
            Ok(ClickResult {
                method: "AXClick".to_string(),
                coordinates: None,
                details: "Used accessibility AXClick action".to_string(),
            })
        }
        Err(e) => Err(AutomationError::PlatformError(format!(
            "AXClick click failed: {:?}",
            e
        ))),
    }
}

pub(crate) fn click_mouse_simulation(
    element: &MacOSUIElement,
) -> Result<ClickResult, AutomationError> {
    match element.bounds() {
        Ok((x, y, width, height)) => {
            let center_x = x + width / 2.0;
            let center_y = y + height / 2.0;
            let point = CGPoint::new(center_x, center_y);
            let source = get_pooled_event_source().map_err(|e| {
                AutomationError::PlatformError(format!("Failed to create event source: {}", e))
            })?;

            let mouse_move = CGEvent::new_mouse_event(
                source.clone(),
                CGEventType::MouseMoved,
                point,
                CGMouseButton::Left,
            )
            .map_err(|_| {
                AutomationError::PlatformError("Failed to create mouse move event".to_string())
            })?;
            mouse_move.post(CGEventTapLocation::HID);
            thread::sleep(Duration::from_millis(MOUSE_EVENT_DELAY_MS));

            debug!("Mouse down at ({}, {})", center_x, center_y);
            let mouse_down = CGEvent::new_mouse_event(
                source.clone(),
                CGEventType::LeftMouseDown,
                point,
                CGMouseButton::Left,
            )
            .map_err(|_| {
                AutomationError::PlatformError("Failed to create mouse down event".to_string())
            })?;
            mouse_down.post(CGEventTapLocation::HID);
            thread::sleep(Duration::from_millis(MOUSE_EVENT_DELAY_MS));

            debug!("Mouse up at ({}, {})", center_x, center_y);
            let mouse_up = CGEvent::new_mouse_event(
                source,
                CGEventType::LeftMouseUp,
                point,
                CGMouseButton::Left,
            )
            .map_err(|_| {
                AutomationError::PlatformError("Failed to create mouse up event".to_string())
            })?;
            mouse_up.post(CGEventTapLocation::HID);

            debug!(
                "Performed simulated mouse click at ({}, {})",
                center_x, center_y
            );
            Ok(ClickResult {
                method: "MouseSimulation".to_string(),
                coordinates: Some((center_x, center_y)),
                details: format!(
                    "Used mouse simulation at coordinates ({:.1}, {:.1}), element bounds: ({:.1}, {:.1}, {:.1}, {:.1})",
                    center_x, center_y, x, y, width, height
                ),
            })
        }
        Err(e) => Err(AutomationError::PlatformError(format!(
            "Failed to determine element bounds for click: {}",
            e
        ))),
    }
}

/// Click an element's centre without warping the cursor, for elements that do
/// not answer AXPress (browser web content being the common case).
///
/// The element's own process is preferred over a hit-test of its centre point:
/// the centre may be covered by another window, and the caller reached this
/// element through the accessibility tree, so its owner is the intended target.
pub(crate) fn click_element_no_warp(
    element: &MacOSUIElement,
) -> Result<ClickResult, AutomationError> {
    let (x, y, width, height) = element.bounds()?;
    let center_x = x + width / 2.0;
    let center_y = y + height / 2.0;

    let pid_override = element_pid(element);
    let outcome = left_click_no_warp_inner(
        center_x,
        center_y,
        CGEventType::LeftMouseDown,
        CGEventType::LeftMouseUp,
        CGMouseButton::Left,
        None,
        1,
        pid_override,
        false,
    )?
    .ok_or_else(|| {
        AutomationError::PlatformError(
            "No-warp click needs the physical cursor for this element".to_string(),
        )
    })?;

    Ok(ClickResult {
        method: outcome.method.to_string(),
        coordinates: Some((center_x, center_y)),
        details: format!(
            "Posted click to the target process at ({:.1}, {:.1}) via {}",
            center_x, center_y, outcome.method
        ),
    })
}

/// The process that owns an accessibility element, if it can be determined.
pub(crate) fn element_pid(element: &MacOSUIElement) -> Option<i32> {
    let mut pid: libc::pid_t = 0;
    let err = unsafe {
        accessibility_sys::AXUIElementGetPid(
            element.element.0.as_concrete_TypeRef() as AXUIElementRef,
            &mut pid,
        )
    };
    if err == 0 && pid > 0 {
        Some(pid)
    } else {
        None
    }
}

pub(crate) fn focus(element: &MacOSUIElement) -> Result<(), AutomationError> {
    // In background mode the point is to leave the user's window order alone, so
    // AXRaise is skipped and a miss is reported rather than papered over with a
    // coordinate click that would pull the app forward.
    let background = background::is_background_mode();

    let raised = if background {
        // Keep the app's accessibility tree awake first: Chromium and Electron
        // suspend AX on occluded windows, which would make the focus set below
        // fail for exactly the apps that need it most.
        if let Some(pid) = element_pid(element) {
            note_background_target(pid);
        }
        true
    } else {
        let raise_attr = AXAttribute::new(&CFString::new("AXRaise"));
        let ok = element
            .element
            .0
            .perform_action(raise_attr.as_CFString())
            .is_ok();
        if ok {
            debug!("Successfully raised element");
        }
        ok
    };

    if raised {
        if let Some(app) = get_application(element) {
            unsafe {
                let app_ref = app.element.0.as_concrete_TypeRef();
                let attr_str = CFString::new("AXFocusedUIElement");
                let attr_str_ref = attr_str.as_concrete_TypeRef();
                let elem_ref = element.element.0.as_concrete_TypeRef() as CFTypeRef;
                let result = AXUIElementSetAttributeValue(
                    app_ref as AXUIElementRef,
                    attr_str_ref as CFStringRef,
                    elem_ref,
                );
                if result == 0 {
                    debug!("Successfully set focus to element via AXFocusedUIElement");
                    return Ok(());
                } else {
                    let error = accessibility::Error::Ax(result);
                    debug!("Failed to set AXFocusedUIElement: {:?}", error);
                }
            }
        }
    }

    if background {
        return Err(AutomationError::PlatformError(
            "Could not focus the element without raising its window".to_string(),
        ));
    }

    debug!("Raise action failed or app not found, attempting focus via click");
    click_auto(element).map(|_result| {
        debug!("Focus achieved via click method: {}", _result.method);
    })
}

// --- New Mouse Functions ---

/// Get the current mouse cursor position.
#[allow(dead_code)] // Used through computer_use_ai_sdk interface
pub(crate) fn get_cursor_position() -> Result<(f64, f64), AutomationError> {
    // 1. Create an event source.
    let source = get_pooled_event_source().map_err(|_| {
        AutomationError::PlatformError(
            "Failed to create event source for cursor position".to_string(),
        )
    })?;

    // 2. Create a null mouse event (MouseMoved seems appropriate) using the source.
    //    The specific type and point might not matter if we only need location.
    let event = CGEvent::new_mouse_event(
        source,
        CGEventType::MouseMoved, // Or any other type?
        CGPoint::new(0.0, 0.0),  // Dummy point
        CGMouseButton::Left,     // Dummy button
    )
    .map_err(|_| {
        AutomationError::PlatformError("Failed to create CGEvent for cursor position".to_string())
    })?;

    // 3. Get the location from the created event.
    let location = event.location();
    debug!(
        "Retrieved cursor position: ({}, {})",
        location.x, location.y
    );
    Ok((location.x, location.y))
}

/// Move the mouse cursor to the specified coordinates.
/// This is now a simple atomic move - smooth movement is handled at the command level.
#[allow(dead_code)] // Used through computer_use_ai_sdk interface
pub(crate) fn mouse_move(x: f64, y: f64) -> Result<(), AutomationError> {
    let point = create_adjusted_point(x, y)?;
    debug!("Moving mouse to ({}, {}) [adjusted]", point.x, point.y);
    let source = get_pooled_event_source().map_err(|_| {
        AutomationError::PlatformError("Failed to create event source for mouse move".to_string())
    })?;

    let event =
        CGEvent::new_mouse_event(source, CGEventType::MouseMoved, point, CGMouseButton::Left) // Button doesn't matter for move
            .map_err(|_| {
                AutomationError::PlatformError("Failed to create mouse move event".to_string())
            })?;

    event.post(CGEventTapLocation::HID);
    // Optional: Add a small delay after moving
    // std::thread::sleep(std::time::Duration::from_millis(10));
    Ok(())
}

/// Press down the left mouse button at specified coordinates.
pub(crate) fn left_mouse_down(
    x: f64,
    y: f64,
    modifiers: Option<CGEventFlags>,
) -> Result<(), AutomationError> {
    debug!(
        "Mouse down at ({}, {}) with modifiers: {:?}",
        x, y, modifiers
    );

    // Create an event source for user input
    let source = get_pooled_event_source().map_err(|_| {
        AutomationError::PlatformError("Failed to create event source for mouse down".to_string())
    })?;

    // First, move the cursor to the target position (with multi-monitor support)
    let point = create_adjusted_point(x, y)?;
    let mouse_move = CGEvent::new_mouse_event(
        source.clone(),
        CGEventType::MouseMoved,
        point,
        CGMouseButton::Left,
    )
    .map_err(|_| AutomationError::PlatformError("Failed to create mouse move event".to_string()))?;

    // Apply modifiers if provided
    if let Some(flags) = modifiers {
        mouse_move.set_flags(flags);
    }

    // Post the mouse move event
    mouse_move.post(CGEventTapLocation::HID);

    // Wait a short time for the move to register
    std::thread::sleep(std::time::Duration::from_millis(50));

    // Create the mouse down event
    let mouse_down = CGEvent::new_mouse_event(
        source,
        CGEventType::LeftMouseDown,
        point,
        CGMouseButton::Left,
    )
    .map_err(|_| AutomationError::PlatformError("Failed to create mouse down event".to_string()))?;

    // Apply modifiers if provided
    if let Some(flags) = modifiers {
        mouse_down.set_flags(flags);
    }

    // Post the mouse down event
    mouse_down.post(CGEventTapLocation::HID);

    debug!("Left mouse button down at ({}, {})", x, y);

    Ok(())
}

/// Release the left mouse button at specified coordinates.
pub(crate) fn left_mouse_up(
    x: f64,
    y: f64,
    modifiers: Option<CGEventFlags>,
) -> Result<(), AutomationError> {
    debug!("Mouse up at ({}, {}) with modifiers: {:?}", x, y, modifiers);

    // Create an event source for user input
    let source = get_pooled_event_source().map_err(|_| {
        AutomationError::PlatformError("Failed to create event source for mouse up".to_string())
    })?;

    // Create the point for the current position (with multi-monitor support)
    let point = create_adjusted_point(x, y)?;

    // Create the mouse up event
    let mouse_up =
        CGEvent::new_mouse_event(source, CGEventType::LeftMouseUp, point, CGMouseButton::Left)
            .map_err(|_| {
                AutomationError::PlatformError("Failed to create mouse up event".to_string())
            })?;

    // Apply modifiers if provided
    if let Some(flags) = modifiers {
        mouse_up.set_flags(flags);
    }

    // Post the mouse up event
    mouse_up.post(CGEventTapLocation::HID);

    debug!("Left mouse button up at ({}, {})", x, y);

    Ok(())
}

/// Simulate a standard left click (down + up) at specified coordinates.
/// Optionally apply modifier keys to the click.
pub(crate) fn left_click(
    x: f64,
    y: f64,
    modifiers: Option<CGEventFlags>,
) -> Result<(), AutomationError> {
    debug!(
        "Left click at ({}, {}) with modifiers: {:?}",
        x, y, modifiers
    );

    // Use our improved left_mouse_down and left_mouse_up functions
    left_mouse_down(x, y, modifiers)?;

    // Wait a short time between down and up events
    std::thread::sleep(std::time::Duration::from_millis(30));

    left_mouse_up(x, y, modifiers)?;

    debug!("Completed left click at ({}, {})", x, y);

    Ok(())
}

/// Simulate a right click (down + up) at specified coordinates.
#[allow(dead_code)] // Used through computer_use_ai_sdk interface
pub(crate) fn right_click(x: f64, y: f64) -> Result<(), AutomationError> {
    let point = create_adjusted_point(x, y)?;
    debug!(
        "Performing right click at ({}, {}) [adjusted]",
        point.x, point.y
    );
    mouse_move(x, y)?; // Ensure cursor is at the correct position
    thread::sleep(Duration::from_millis(MOUSE_MOVE_STEP_DELAY_MS));

    let source = get_pooled_event_source().map_err(|_| {
        AutomationError::PlatformError("Failed to create event source for right click".to_string())
    })?;

    let down_event = CGEvent::new_mouse_event(
        source.clone(),
        CGEventType::RightMouseDown,
        point,
        CGMouseButton::Right,
    )
    .map_err(|_| {
        AutomationError::PlatformError("Failed to create right mouse down event".to_string())
    })?;
    down_event.post(CGEventTapLocation::HID);
    std::thread::sleep(std::time::Duration::from_millis(50));

    let up_event = CGEvent::new_mouse_event(
        source,
        CGEventType::RightMouseUp,
        point,
        CGMouseButton::Right,
    )
    .map_err(|_| {
        AutomationError::PlatformError("Failed to create right mouse up event".to_string())
    })?;
    up_event.post(CGEventTapLocation::HID);
    Ok(())
}

/// Simulate a middle click (down + up) at specified coordinates.
#[allow(dead_code)] // Used through computer_use_ai_sdk interface
pub(crate) fn middle_click(x: f64, y: f64) -> Result<(), AutomationError> {
    let point = create_adjusted_point(x, y)?;
    debug!(
        "Performing middle click at ({}, {}) [adjusted]",
        point.x, point.y
    );
    mouse_move(x, y)?; // Ensure cursor is at the correct position
    std::thread::sleep(std::time::Duration::from_millis(20));

    let source = get_pooled_event_source().map_err(|_| {
        AutomationError::PlatformError("Failed to create event source for middle click".to_string())
    })?;

    let down_event = CGEvent::new_mouse_event(
        source.clone(),
        CGEventType::OtherMouseDown,
        point,
        CGMouseButton::Center,
    )
    .map_err(|_| {
        AutomationError::PlatformError("Failed to create middle mouse down event".to_string())
    })?;
    down_event.post(CGEventTapLocation::HID);
    std::thread::sleep(std::time::Duration::from_millis(50));

    let up_event = CGEvent::new_mouse_event(
        source,
        CGEventType::OtherMouseUp,
        point,
        CGMouseButton::Center,
    )
    .map_err(|_| {
        AutomationError::PlatformError("Failed to create middle mouse up event".to_string())
    })?;
    up_event.post(CGEventTapLocation::HID);
    Ok(())
}

/// Simulate a double click at the specified coordinates.
#[allow(dead_code)]
pub(crate) fn double_click(x: f64, y: f64) -> Result<(), AutomationError> {
    let point = create_adjusted_point(x, y)?;
    debug!(
        "Performing double click at ({}, {}) [adjusted]",
        point.x, point.y
    );
    mouse_move(x, y)?;
    std::thread::sleep(std::time::Duration::from_millis(20));

    let source = get_pooled_event_source().map_err(|_| {
        AutomationError::PlatformError("Failed to create event source for double click".to_string())
    })?;

    // First click (down)
    let down1 = CGEvent::new_mouse_event(
        source.clone(),
        CGEventType::LeftMouseDown,
        point,
        CGMouseButton::Left,
    )
    .map_err(|_| {
        AutomationError::PlatformError("Failed to create double click down1 event".to_string())
    })?;
    down1.set_integer_value_field(core_graphics::event::EventField::MOUSE_EVENT_CLICK_STATE, 1);
    down1.post(CGEventTapLocation::HID);
    std::thread::sleep(std::time::Duration::from_millis(50));

    // First click (up)
    let up1 = CGEvent::new_mouse_event(
        source.clone(),
        CGEventType::LeftMouseUp,
        point,
        CGMouseButton::Left,
    )
    .map_err(|_| {
        AutomationError::PlatformError("Failed to create double click up1 event".to_string())
    })?;
    up1.set_integer_value_field(core_graphics::event::EventField::MOUSE_EVENT_CLICK_STATE, 1);
    up1.post(CGEventTapLocation::HID);
    std::thread::sleep(std::time::Duration::from_millis(50)); // Double click interval

    // Second click (down)
    let down2 = CGEvent::new_mouse_event(
        source.clone(),
        CGEventType::LeftMouseDown,
        point,
        CGMouseButton::Left,
    )
    .map_err(|_| {
        AutomationError::PlatformError("Failed to create double click down2 event".to_string())
    })?;
    down2.set_integer_value_field(core_graphics::event::EventField::MOUSE_EVENT_CLICK_STATE, 2);
    down2.post(CGEventTapLocation::HID);
    std::thread::sleep(std::time::Duration::from_millis(50));

    // Second click (up)
    let up2 =
        CGEvent::new_mouse_event(source, CGEventType::LeftMouseUp, point, CGMouseButton::Left)
            .map_err(|_| {
                AutomationError::PlatformError(
                    "Failed to create double click up2 event".to_string(),
                )
            })?;
    up2.set_integer_value_field(core_graphics::event::EventField::MOUSE_EVENT_CLICK_STATE, 2);
    up2.post(CGEventTapLocation::HID);

    Ok(())
}

/// Simulate a triple click at the specified coordinates.
#[allow(dead_code)]
pub(crate) fn triple_click(x: f64, y: f64) -> Result<(), AutomationError> {
    debug!("Performing triple click at ({}, {})", x, y);

    // Instead of calling double_click + left_click, we'll implement the full sequence
    // with proper click state tracking - similar to how double_click is implemented
    let point = create_adjusted_point(x, y)?;
    mouse_move(x, y)?;
    std::thread::sleep(std::time::Duration::from_millis(20));

    let source = get_pooled_event_source().map_err(|_| {
        AutomationError::PlatformError("Failed to create event source for triple click".to_string())
    })?;

    // First click (down)
    let down1 = CGEvent::new_mouse_event(
        source.clone(),
        CGEventType::LeftMouseDown,
        point,
        CGMouseButton::Left,
    )
    .map_err(|_| {
        AutomationError::PlatformError("Failed to create triple click down1 event".to_string())
    })?;
    down1.set_integer_value_field(core_graphics::event::EventField::MOUSE_EVENT_CLICK_STATE, 1);
    down1.post(CGEventTapLocation::HID);
    std::thread::sleep(std::time::Duration::from_millis(50));

    // First click (up)
    let up1 = CGEvent::new_mouse_event(
        source.clone(),
        CGEventType::LeftMouseUp,
        point,
        CGMouseButton::Left,
    )
    .map_err(|_| {
        AutomationError::PlatformError("Failed to create triple click up1 event".to_string())
    })?;
    up1.set_integer_value_field(core_graphics::event::EventField::MOUSE_EVENT_CLICK_STATE, 1);
    up1.post(CGEventTapLocation::HID);
    std::thread::sleep(std::time::Duration::from_millis(50));

    // Second click (down)
    let down2 = CGEvent::new_mouse_event(
        source.clone(),
        CGEventType::LeftMouseDown,
        point,
        CGMouseButton::Left,
    )
    .map_err(|_| {
        AutomationError::PlatformError("Failed to create triple click down2 event".to_string())
    })?;
    down2.set_integer_value_field(core_graphics::event::EventField::MOUSE_EVENT_CLICK_STATE, 2);
    down2.post(CGEventTapLocation::HID);
    std::thread::sleep(std::time::Duration::from_millis(50));

    // Second click (up)
    let up2 = CGEvent::new_mouse_event(
        source.clone(),
        CGEventType::LeftMouseUp,
        point,
        CGMouseButton::Left,
    )
    .map_err(|_| {
        AutomationError::PlatformError("Failed to create triple click up2 event".to_string())
    })?;
    up2.set_integer_value_field(core_graphics::event::EventField::MOUSE_EVENT_CLICK_STATE, 2);
    up2.post(CGEventTapLocation::HID);
    std::thread::sleep(std::time::Duration::from_millis(50));

    // Third click (down)
    let down3 = CGEvent::new_mouse_event(
        source.clone(),
        CGEventType::LeftMouseDown,
        point,
        CGMouseButton::Left,
    )
    .map_err(|_| {
        AutomationError::PlatformError("Failed to create triple click down3 event".to_string())
    })?;
    down3.set_integer_value_field(core_graphics::event::EventField::MOUSE_EVENT_CLICK_STATE, 3);
    down3.post(CGEventTapLocation::HID);
    std::thread::sleep(std::time::Duration::from_millis(50));

    // Third click (up)
    let up3 =
        CGEvent::new_mouse_event(source, CGEventType::LeftMouseUp, point, CGMouseButton::Left)
            .map_err(|_| {
                AutomationError::PlatformError(
                    "Failed to create triple click up3 event".to_string(),
                )
            })?;
    up3.set_integer_value_field(core_graphics::event::EventField::MOUSE_EVENT_CLICK_STATE, 3);
    up3.post(CGEventTapLocation::HID);

    Ok(())
}

/// Simulate dragging with the left mouse button from a start point to an end point.
#[allow(dead_code)]
pub(crate) fn left_click_drag(
    start_x: f64,
    start_y: f64,
    end_x: f64,
    end_y: f64,
) -> Result<(), AutomationError> {
    let start_point = create_adjusted_point(start_x, start_y)?;
    let end_point = create_adjusted_point(end_x, end_y)?;
    debug!(
        "Performing left click drag from ({}, {}) to ({}, {})",
        start_x, start_y, end_x, end_y
    );

    let source = get_pooled_event_source().map_err(|_| {
        AutomationError::PlatformError("Failed to create event source for drag".to_string())
    })?;

    // 1. Move to start position
    mouse_move(start_x, start_y)?;
    std::thread::sleep(std::time::Duration::from_millis(20));

    // 2. Press left button down
    let down_event = CGEvent::new_mouse_event(
        source.clone(),
        CGEventType::LeftMouseDown,
        start_point,
        CGMouseButton::Left,
    )
    .map_err(|_| AutomationError::PlatformError("Failed to create drag down event".to_string()))?;
    down_event.post(CGEventTapLocation::HID);
    thread::sleep(Duration::from_millis(DRAG_HOLD_DELAY_MS)); // Hold briefly before dragging

    // 3. Move to end position (drag event)
    let drag_event = CGEvent::new_mouse_event(
        source.clone(),
        CGEventType::LeftMouseDragged,
        end_point,
        CGMouseButton::Left,
    )
    .map_err(|_| AutomationError::PlatformError("Failed to create drag move event".to_string()))?;
    drag_event.post(CGEventTapLocation::HID);
    std::thread::sleep(std::time::Duration::from_millis(100)); // Pause at end position

    // 4. Release left button
    let up_event = CGEvent::new_mouse_event(
        source,
        CGEventType::LeftMouseUp,
        end_point,
        CGMouseButton::Left,
    )
    .map_err(|_| AutomationError::PlatformError("Failed to create drag up event".to_string()))?;
    up_event.post(CGEventTapLocation::HID);

    Ok(())
}

// RAII guard to restore clipboard content
struct ClipboardGuard {
    original_content: Option<String>,
}

impl ClipboardGuard {
    fn new() -> Result<Self, AutomationError> {
        let original_content = match NativeClipboard::new() {
            Ok(clipboard) => clipboard.read().ok(), // Ignore read errors, proceed without restore maybe? Or error out? Let's ignore for now.
            Err(_) => None,                         // Ignore errors getting clipboard instance
        };
        Ok(Self { original_content })
    }
}

impl Drop for ClipboardGuard {
    fn drop(&mut self) {
        if let Some(content) = &self.original_content {
            match NativeClipboard::new() {
                Ok(mut clipboard) => {
                    if let Err(e) = clipboard.write(content.clone()) {
                        warn!("Failed to restore clipboard content: {:?}", e);
                    } else {
                        debug!("Successfully restored clipboard content.");
                    }
                }
                Err(e) => {
                    warn!("Failed to get clipboard instance for restoring: {:?}", e);
                }
            }
        }
    }
}

/// Types text into the specified UI element.
///
/// Attempts to set the value directly using AXValue first. If that fails,
/// it falls back to using the clipboard and simulating Cmd+V (Paste).
pub(crate) fn type_text(element: &MacOSUIElement, text: &str) -> Result<(), AutomationError> {
    match focus(element) {
        Ok(_) => debug!("Successfully focused element for typing"),
        Err(e) if background::is_background_mode() => {
            // A coordinate click here would raise the window and steal focus,
            // which is the whole thing background mode exists to avoid. AXValue
            // below often succeeds without focus, so carry on without clicking.
            debug!("Focus failed before typing in background mode: {:?}", e);
        }
        Err(e) => {
            warn!("Focus failed before typing, attempting anyway: {:?}", e);
            // Attempting to click as a fallback focus mechanism
            if let Err(click_err) = click_auto(element) {
                warn!("Fallback click also failed: {:?}", click_err);
                // Decide if we should proceed or error out here.
                // Let's proceed cautiously, AXValue might still work if focus is weird.
            } else {
                // Add a small delay after fallback click before trying to type
                thread::sleep(Duration::from_millis(APP_ACTIVATION_DELAY_MS));
            }
        }
    }

    // --- Try AXValue first ---
    let cf_string = CFString::new(text);
    unsafe {
        let element_ref = element.element.0.as_concrete_TypeRef();
        let attr_str = CFString::new("AXValue");
        let attr_str_ref = attr_str.as_concrete_TypeRef();
        let value_ref = cf_string.as_concrete_TypeRef() as CFTypeRef;
        let result = AXUIElementSetAttributeValue(
            element_ref as AXUIElementRef,
            attr_str_ref as CFStringRef,
            value_ref,
        );

        if result == 0 {
            debug!("Successfully set text value via AXValue");
            return Ok(());
        } else {
            let error = accessibility::Error::Ax(result);
            debug!(
                "Failed to set text via AXValue: {:?}. Falling back to clipboard paste.",
                error
            );
        }
    }

    // --- Fallback to Clipboard Paste ---
    debug!("Attempting clipboard paste for text: '{}'", text);
    let _guard = ClipboardGuard::new()?; // Restore clipboard automatically on scope exit

    // Set clipboard
    match NativeClipboard::new() {
        Ok(mut clipboard) => {
            if let Err(e) = clipboard.write(text.to_string()) {
                return Err(AutomationError::PlatformError(format!(
                    "Failed to write to clipboard before paste: {:?}",
                    e
                )));
            }
            debug!("Successfully set clipboard content.");
        }
        Err(e) => {
            return Err(AutomationError::PlatformError(format!(
                "Failed to access clipboard before paste: {:?}",
                e
            )));
        }
    }

    // Give clipboard time to process
    thread::sleep(Duration::from_millis(100));

    // Simulate Cmd+V
    let source = get_pooled_event_source().map_err(|_| {
        AutomationError::PlatformError("Failed to create event source for paste".to_string())
    })?;

    let key_code_v = KEY_V; // Assuming KEY_V is defined in constants
    let cmd_flag = MODIFIER_COMMAND; // Assuming MODIFIER_COMMAND is defined

    // In background mode the paste goes to the element's own process, otherwise a
    // Cmd+V on the HID tap would land in whatever app the user is looking at.
    let target_pid = if background::is_background_mode() {
        element_pid(element)
    } else {
        None
    };

    if let Some(pid) = target_pid {
        let _focus = prepare_background_keyboard_target(pid);
        post_key_event_with_flags_to_pid(pid, key_code_v, cmd_flag, true)?;
        thread::sleep(Duration::from_millis(KEY_EVENT_DELAY_MS));
        post_key_event_with_flags_to_pid(pid, key_code_v, cmd_flag, false)?;
        thread::sleep(Duration::from_millis(KEY_EVENT_DELAY_MS));
        debug!("Posted Cmd+V paste to PID {}", pid);
    } else {
        // Press Cmd+V
        let key_down =
            CGEvent::new_keyboard_event(source.clone(), key_code_v, true).map_err(|_| {
                AutomationError::PlatformError(
                    "Failed to create key down event for paste".to_string(),
                )
            })?;
        key_down.set_flags(cmd_flag);
        key_down.post(CGEventTapLocation::HID);
        thread::sleep(Duration::from_millis(50));

        // Release Cmd+V
        let key_up = CGEvent::new_keyboard_event(source, key_code_v, false).map_err(|_| {
            AutomationError::PlatformError("Failed to create key up event for paste".to_string())
        })?;
        key_up.set_flags(cmd_flag);
        key_up.post(CGEventTapLocation::HID);
        thread::sleep(Duration::from_millis(50));

        debug!("Successfully simulated Cmd+V paste.");
    }

    // Clipboard restored by ClipboardGuard automatically
    Ok(())
}

/// Types text globally using clipboard paste (Cmd+V).
///
/// This function simulates key presses for Cmd+V after setting the clipboard.
/// It does not require focusing a specific UI element beforehand.
/// How long after Cmd+V the pasted text stays on the clipboard before the previous
/// contents are restored. Electron and JVM apps read the pasteboard well after the key
/// event, so an immediate restore pastes the *old* clipboard into them. The restore
/// only runs when the pasteboard's changeCount is unchanged since the write, so a
/// copy the user makes in this window is never clobbered.
const PASTE_CLIPBOARD_RESTORE_DELAY_MS: u64 = 500;
/// Pasteboard write-to-visible settle time (HID path only; a PID-targeted
/// paste delivers after the write by construction).
const PASTE_CLIPBOARD_SETTLE_MS: u64 = 20;
/// Cmd+V key down/up spacing.
const PASTE_KEY_GAP_MS: u64 = 8;

pub(crate) fn type_text_global(text: &str) -> Result<(), AutomationError> {
    // "Global" means the frontmost app. Dictation types where the user is
    // looking, so this path must stay on the HID tap even in background mode.
    paste_text(text, None, false)
}

/// Put `text` on the clipboard and paste it with Cmd+V.
///
/// `target_pid` decides where the paste lands: `Some(pid)` posts it to that
/// process after pointing input focus at it without raising it, `None` sends it
/// through the HID tap to whatever application is frontmost.
///
/// With `retain_clipboard` false, the previous pasteboard contents — every
/// representation of every item — are snapshotted before the write and
/// restored off the critical path after [`PASTE_CLIPBOARD_RESTORE_DELAY_MS`],
/// guarded by the pasteboard changeCount so a copy the user makes in between
/// wins; the temporary item is marked transient/auto-generated so clipboard
/// managers skip it. With `retain_clipboard` true, the text is written as an
/// ordinary item and stays on the clipboard: no snapshot, no markers, no
/// restore (dictation's "leave the transcript on the clipboard").
pub(crate) fn paste_text(
    text: &str,
    target_pid: Option<i32>,
    retain_clipboard: bool,
) -> Result<(), AutomationError> {
    debug!("Typing text via clipboard paste: {}", text);

    let snapshot = if retain_clipboard {
        None
    } else {
        super::text_insertion::snapshot_pasteboard()
    };
    let change_count_after_write =
        super::text_insertion::write_string_to_pasteboard(text, !retain_clipboard)?;

    // Cmd+V's key code depends on the layout's Command layer
    // (Dvorak-QWERTY-Command differs from the unmodified layout).
    let key_code_v = super::text_insertion::cmd_v_keycode();
    let cmd_flag = MODIFIER_COMMAND;

    if let Some(pid) = target_pid {
        // The write already round-tripped through the pasteboard server, and a
        // PID-targeted paste cannot race the user's typing, so no settle delay.
        let _focus = prepare_background_keyboard_target(pid);
        post_key_event_with_flags_to_pid(pid, key_code_v, cmd_flag, true)?;
        thread::sleep(Duration::from_millis(PASTE_KEY_GAP_MS));
        post_key_event_with_flags_to_pid(pid, key_code_v, cmd_flag, false)?;
        debug!("Posted Cmd+V paste to PID {}", pid);
    } else {
        // Give the pasteboard server a moment to publish the new contents
        thread::sleep(Duration::from_millis(PASTE_CLIPBOARD_SETTLE_MS));

        let source = get_pooled_event_source().map_err(|e| {
            AutomationError::PlatformError(format!(
                "Failed to create event source for global paste: {}",
                e
            ))
        })?;

        // Press Cmd+V
        let key_down =
            CGEvent::new_keyboard_event(source.clone(), key_code_v, true).map_err(|_| {
                AutomationError::PlatformError(
                    "Failed to create key down event for global paste".to_string(),
                )
            })?;
        key_down.set_flags(cmd_flag);
        super::text_insertion::tag_synthesized_event(&key_down);
        key_down.post(CGEventTapLocation::HID);
        thread::sleep(Duration::from_millis(PASTE_KEY_GAP_MS));

        // Release Cmd+V
        let key_up = CGEvent::new_keyboard_event(source, key_code_v, false).map_err(|_| {
            AutomationError::PlatformError(
                "Failed to create key up event for global paste".to_string(),
            )
        })?;
        key_up.set_flags(cmd_flag);
        super::text_insertion::tag_synthesized_event(&key_up);
        key_up.post(CGEventTapLocation::HID);

        debug!("Successfully simulated global Cmd+V paste.");
    }

    // Restore the previous pasteboard on a timer, off the hot path, only if
    // nothing else has claimed the pasteboard since the write. When the
    // clipboard is retained there is no snapshot and nothing to restore.
    super::text_insertion::schedule_snapshot_restore(
        snapshot,
        change_count_after_write,
        Duration::from_millis(PASTE_CLIPBOARD_RESTORE_DELAY_MS),
    );
    Ok(())
}

/// Type text into the process the agent is working on, without disturbing the
/// user's frontmost app.
///
/// Returns `Ok(None)` when there is no such process yet (the agent has not
/// clicked into anything) and `allow_physical` forbids falling back to the HID
/// tap, which would paste into whatever the user is looking at.
pub(crate) fn type_text_no_warp(
    text: &str,
    allow_physical: bool,
) -> Result<Option<InputOutcome>, AutomationError> {
    if let Some(pid) = background::target_pid() {
        // `paste_text` holds the input-focus guard for the duration of the
        // paste, so focus is handed back here without a second guard.
        paste_text(text, Some(pid), false)?;
        return Ok(Some(InputOutcome::process_targeted(post_method_label())));
    }

    if !allow_physical {
        return Ok(None);
    }

    paste_text(text, None, false)?;
    Ok(Some(InputOutcome::physical_cursor("HID")))
}

pub(crate) fn get_key_code(key: &str) -> Result<u16, AutomationError> {
    // First, check our predefined key map for special keys
    let key_map: HashMap<&str, u16> = [
        ("return", KEY_RETURN),
        ("enter", KEY_RETURN),
        ("tab", KEY_TAB),
        ("space", KEY_SPACE),
        ("delete", KEY_DELETE),
        ("backspace", KEY_DELETE),
        ("esc", KEY_ESCAPE),
        ("escape", KEY_ESCAPE),
        ("left", KEY_ARROW_LEFT),
        ("right", KEY_ARROW_RIGHT),
        ("down", KEY_ARROW_DOWN),
        ("up", KEY_ARROW_UP),
        // Add punctuation marks
        (",", KEY_COMMA),
        ("comma", KEY_COMMA),
        (".", KEY_PERIOD),
        ("period", KEY_PERIOD),
        (";", KEY_SEMICOLON),
        ("semicolon", KEY_SEMICOLON),
        ("'", KEY_QUOTE),
        ("quote", KEY_QUOTE),
        ("apostrophe", KEY_QUOTE),
        ("/", KEY_SLASH),
        ("slash", KEY_SLASH),
        ("\\", KEY_BACKSLASH),
        ("backslash", KEY_BACKSLASH),
        ("[", KEY_BRACKET_LEFT),
        ("bracketleft", KEY_BRACKET_LEFT),
        ("leftbracket", KEY_BRACKET_LEFT),
        ("]", KEY_BRACKET_RIGHT),
        ("bracketright", KEY_BRACKET_RIGHT),
        ("rightbracket", KEY_BRACKET_RIGHT),
        ("-", KEY_MINUS),
        ("minus", KEY_MINUS),
        ("dash", KEY_MINUS),
        ("=", KEY_EQUAL),
        ("equal", KEY_EQUAL),
        ("equals", KEY_EQUAL),
        ("`", KEY_BACKQUOTE),
        ("backquote", KEY_BACKQUOTE),
        ("grave", KEY_BACKQUOTE),
        // Add more special keys here as needed
    ]
    .iter()
    .cloned()
    .collect();

    let key_lower = key.to_lowercase();

    // First check if it's in our predefined map
    if let Some(&code) = key_map.get(key_lower.as_str()) {
        return Ok(code);
    }

    // If not in predefined map, check if it's a single alphanumeric character
    if key_lower.len() == 1 {
        let c = key_lower.chars().next().ok_or_else(|| {
            AutomationError::InvalidArgument(format!("Failed to parse key: {}", key))
        })?;

        // Handle alphabetic keys (a-z)
        if c.is_ascii_alphabetic() {
            // ASCII values: 'a' is 0, 'b' is 11, etc.
            // These are standard macOS virtual key codes
            let vk = match c {
                'a' => 0,
                'b' => 11,
                'c' => 8,
                'd' => 2,
                'e' => 14,
                'f' => 3,
                'g' => 5,
                'h' => 4,
                'i' => 34,
                'j' => 38,
                'k' => 40,
                'l' => 37,
                'm' => 46,
                'n' => 45,
                'o' => 31,
                'p' => 35,
                'q' => 12,
                'r' => 15,
                's' => 1,
                't' => 17,
                'u' => 32,
                'v' => 9,
                'w' => 13,
                'x' => 7,
                'y' => 16,
                'z' => 6,
                _ => {
                    return Err(AutomationError::InvalidArgument(format!(
                        "Unsupported character: {}",
                        c
                    )))
                }
            };
            return Ok(vk);
        }

        // Handle numeric keys (0-9)
        if c.is_ascii_digit() {
            // macOS virtual key codes for digits
            let vk = match c {
                '0' => 29,
                '1' => 18,
                '2' => 19,
                '3' => 20,
                '4' => 21,
                '5' => 23,
                '6' => 22,
                '7' => 26,
                '8' => 28,
                '9' => 25,
                _ => {
                    return Err(AutomationError::InvalidArgument(format!(
                        "Unsupported digit: {}",
                        c
                    )))
                }
            };
            return Ok(vk);
        }
    }

    // If we get here, the key wasn't recognized
    Err(AutomationError::InvalidArgument(format!(
        "Unknown key: {}",
        key
    )))
}

pub(crate) fn parse_key_combination(
    key_combo: &str,
) -> Result<(u16, CGEventFlags), AutomationError> {
    let parts: Vec<String> = key_combo
        .split('+')
        .map(|s| s.trim().to_lowercase())
        .collect();
    if parts.is_empty() {
        return Err(AutomationError::InvalidArgument(
            "Empty key combination".to_string(),
        ));
    }
    let key = &parts[parts.len() - 1];
    let key_code = get_key_code(key)?;
    let mut flags = CGEventFlags::empty();
    for modifier in &parts[0..parts.len() - 1] {
        match modifier.as_str() {
            "cmd" | "command" => flags.insert(MODIFIER_COMMAND),
            "shift" => flags.insert(MODIFIER_SHIFT),
            "alt" | "option" => flags.insert(MODIFIER_OPTION),
            "ctrl" | "control" => flags.insert(MODIFIER_CONTROL),
            "fn" => flags.insert(MODIFIER_FN),
            _ => {
                return Err(AutomationError::InvalidArgument(format!(
                    "Unknown modifier: {}",
                    modifier
                )))
            }
        }
    }
    Ok((key_code, flags))
}

pub(crate) fn press_key(element: &MacOSUIElement, key_combo: &str) -> Result<(), AutomationError> {
    debug!("Pressing key combination: {}", key_combo);
    let element_label = UIElementImpl::attributes(element).label.unwrap_or_default();
    let element_role = element.role(); // Assuming role() is still on MacOSUIElement

    match focus(element) {
        Ok(_) => debug!("successfully focused element for key press"),
        Err(e) => {
            let error_msg = format!(
                "key press aborted - failed to focus {} element '{}' before pressing '{}': {}",
                element_role, element_label, key_combo, e
            );
            debug!("{}", error_msg);
            return Err(AutomationError::PlatformError(error_msg));
        }
    }
    let (key_code, flags) = parse_key_combination(key_combo)?;
    let source = get_pooled_event_source()
        .map_err(|_| AutomationError::PlatformError("Failed to create event source".to_string()))?;

    let key_down = CGEvent::new_keyboard_event(source.clone(), key_code as CGKeyCode, true)
        .map_err(|_| {
            AutomationError::PlatformError("Failed to create key down event".to_string())
        })?;
    if !flags.is_empty() {
        key_down.set_flags(flags);
    }
    key_down.post(CGEventTapLocation::HID);

    thread::sleep(Duration::from_millis(KEY_EVENT_DELAY_MS));

    let key_up = CGEvent::new_keyboard_event(source, key_code as CGKeyCode, false)
        .map_err(|_| AutomationError::PlatformError("Failed to create key up event".to_string()))?;
    if !flags.is_empty() {
        key_up.set_flags(flags);
    }
    key_up.post(CGEventTapLocation::HID);

    debug!("Successfully pressed key combination: {}", key_combo);
    Ok(())
}

pub(crate) fn set_value(element: &MacOSUIElement, value: &str) -> Result<(), AutomationError> {
    let cf_string = CFString::new(value);
    unsafe {
        let element_ref = element.element.0.as_concrete_TypeRef();
        let attr_str = CFString::new("AXValue");
        let attr_str_ref = attr_str.as_concrete_TypeRef();
        let value_ref = cf_string.as_concrete_TypeRef() as CFTypeRef;
        let result = AXUIElementSetAttributeValue(
            element_ref as AXUIElementRef,
            attr_str_ref as CFStringRef,
            value_ref,
        );
        if result != 0 {
            let error = accessibility::Error::Ax(result);
            debug!("Failed to set value via AXValue: {:?}", error);
            return Err(AutomationError::PlatformError(format!(
                "Failed to set value: {:?}",
                error
            )));
        }
    }
    Ok(())
}

pub(crate) fn scroll(
    element: &MacOSUIElement,
    direction: &str,
    amount: f64,
) -> Result<(), AutomationError> {
    element.focus()?; // Ensure the element or its container is focused
    let (x, y, width, height) = element.bounds()?;
    let center_x = x + width / 2.0;
    let center_y = y + height / 2.0;

    let source = get_pooled_event_source()
        .map_err(|_| AutomationError::PlatformError("Failed to create event source".to_string()))?;
    let scroll_amount = amount as i32;

    let (scroll_x, scroll_y) = match direction.to_lowercase().as_str() {
        "up" => (0, -scroll_amount),
        "down" => (0, scroll_amount),
        "left" => (-scroll_amount, 0),
        "right" => (scroll_amount, 0),
        _ => {
            return Err(AutomationError::InvalidArgument(format!(
                "Invalid scroll direction: {}. Must be up, down, left, or right",
                direction
            )))
        }
    };

    // Positioning the real cursor over the element is only needed when the scroll
    // event travels through the HID tap; a process-targeted scroll carries its own
    // location, so in background mode the user's pointer stays put.
    let target_pid = if background::is_background_mode() {
        element_pid(element)
    } else {
        None
    };

    if target_pid.is_none() {
        // First create a move event to position mouse
        let move_event = CGEvent::new_mouse_event(
            source.clone(), // Clone here to avoid move
            CGEventType::MouseMoved,
            CGPoint::new(center_x, center_y),
            CGMouseButton::Left,
        )
        .map_err(|_| {
            AutomationError::PlatformError("Failed to create mouse move event".to_string())
        })?;

        move_event.post(CGEventTapLocation::HID);
        std::thread::sleep(std::time::Duration::from_millis(50));
    }

    // Create a scroll event manually since new_scroll_wheel_event is not available
    let scroll_event =
        CGEvent::new(source.clone()) // Clone source again here
            .map_err(|_| {
                AutomationError::PlatformError("Failed to create scroll event".to_string())
            })?;

    // Constants for scroll wheel event field IDs
    const SCROLL_WHEEL_EVENT_DELTA_AXIS_1: u32 = 11; // Vertical scroll delta
    const SCROLL_WHEEL_EVENT_DELTA_AXIS_2: u32 = 10; // Horizontal scroll delta
    const SCROLL_WHEEL_EVENT_LINE_SCROLL: i64 = 1 << 0; // Line scroll mode

    // Set event type to scroll wheel
    scroll_event.set_type(CGEventType::ScrollWheel);

    // Set the line scroll bit
    scroll_event.set_integer_value_field(120, SCROLL_WHEEL_EVENT_LINE_SCROLL);

    // Set the delta values
    scroll_event.set_integer_value_field(SCROLL_WHEEL_EVENT_DELTA_AXIS_1, scroll_y as i64);
    scroll_event.set_integer_value_field(SCROLL_WHEEL_EVENT_DELTA_AXIS_2, scroll_x as i64);

    match target_pid {
        Some(pid) => {
            set_event_location(&scroll_event, CGPoint::new(center_x, center_y));
            post_cg_event_to_pid(pid, &scroll_event);
        }
        None => scroll_event.post(CGEventTapLocation::HID),
    }

    Ok(())
}

pub(crate) fn select_text(element: &MacOSUIElement) -> Result<(), AutomationError> {
    debug!("Attempting to select all text in element");

    // First, focus the element to ensure it's active
    element.focus()?;
    thread::sleep(Duration::from_millis(100)); // Give time for focus to register

    // Try using AXSelectText action if available
    let select_attr = AXAttribute::new(&CFString::new("AXSelectText"));
    match element.element.0.perform_action(select_attr.as_CFString()) {
        Ok(_) => {
            debug!("Successfully selected text using AXSelectText action");
            return Ok(());
        }
        Err(e) => {
            debug!(
                "AXSelectText action failed: {:?}, trying alternative methods",
                e
            );
        }
    }

    // If AXSelectText failed, try using Cmd+A shortcut
    debug!("Attempting to select all text with Command+A shortcut");

    // Create event source
    let source = get_pooled_event_source().map_err(|_| {
        AutomationError::PlatformError("Failed to create event source for select_text".to_string())
    })?;

    // Key codes for Command+A
    let cmd_down =
        CGEvent::new_keyboard_event(source.clone(), KEYCODE_CMD, true).map_err(|_| {
            AutomationError::PlatformError("Failed to create command key down event".to_string())
        })?;
    cmd_down.post(CGEventTapLocation::HID);
    thread::sleep(Duration::from_millis(50));

    let a_down = CGEvent::new_keyboard_event(source.clone(), KEYCODE_A, true).map_err(|_| {
        AutomationError::PlatformError("Failed to create 'A' key down event".to_string())
    })?;
    a_down.post(CGEventTapLocation::HID);
    thread::sleep(Duration::from_millis(50));

    let a_up = CGEvent::new_keyboard_event(source.clone(), KEYCODE_A, false).map_err(|_| {
        AutomationError::PlatformError("Failed to create 'A' key up event".to_string())
    })?;
    a_up.post(CGEventTapLocation::HID);
    thread::sleep(Duration::from_millis(50));

    let cmd_up = CGEvent::new_keyboard_event(source, KEYCODE_CMD, false).map_err(|_| {
        AutomationError::PlatformError("Failed to create command key up event".to_string())
    })?;
    cmd_up.post(CGEventTapLocation::HID);

    debug!("Successfully simulated Command+A for text selection");
    Ok(())
}

/// Gets the current text content from the system clipboard.
pub(crate) fn get_clipboard_contents() -> Result<String, AutomationError> {
    match NativeClipboard::new() {
        Ok(clipboard) => match clipboard.read() {
            Ok(content) => {
                if content.is_empty() {
                    // Handle empty clipboard content case
                    return Err(AutomationError::PlatformError(
                        "Clipboard content is empty".to_string(),
                    ));
                }
                debug!("Retrieved clipboard content (length: {})", content.len());
                Ok(content)
            }
            Err(e) => Err(AutomationError::PlatformError(format!(
                "Failed to read string from clipboard: {:?}",
                e
            ))),
        },
        Err(e) => Err(AutomationError::PlatformError(format!(
            "Failed to access clipboard: {:?}",
            e
        ))),
    }
}

/// Sets the system clipboard text content.
pub(crate) fn set_clipboard_contents(text: &str) -> Result<(), AutomationError> {
    match NativeClipboard::new() {
        Ok(mut clipboard) => match clipboard.write(text.to_string()) {
            Ok(_) => {
                debug!("Set clipboard content (length: {})", text.len());
                Ok(())
            }
            Err(e) => Err(AutomationError::PlatformError(format!(
                "Failed to write string to clipboard: {:?}",
                e
            ))),
        },
        Err(e) => Err(AutomationError::PlatformError(format!(
            "Failed to access clipboard: {:?}",
            e
        ))),
    }
}

/// Holds down a specified modifier key.
/// If duration_ms is provided, the key will be released after that duration.
pub(crate) fn hold_key(
    key_code: CGKeyCode,
    flags: CGEventFlags,
    duration_ms: Option<u64>,
) -> Result<(), AutomationError> {
    debug!(
        "Holding key code {} with flags {:?} for {:?}ms",
        key_code, flags, duration_ms
    );

    // Create an event source for user input
    let source = get_pooled_event_source().map_err(|_| {
        AutomationError::PlatformError("Failed to create event source for key hold".to_string())
    })?;

    // Create the key down event
    let key_down = CGEvent::new_keyboard_event(source.clone(), key_code, true).map_err(|_| {
        AutomationError::PlatformError("Failed to create key down event".to_string())
    })?;

    // Set modifier flags
    key_down.set_flags(flags);

    // Post the key down event
    key_down.post(CGEventTapLocation::HID);

    // Determine the duration to hold the key
    let hold_duration = duration_ms.unwrap_or(100); // Default to 100ms if not specified

    // Sleep for the specified duration
    std::thread::sleep(std::time::Duration::from_millis(hold_duration));

    // Create the key up event
    let key_up = CGEvent::new_keyboard_event(source, key_code, false)
        .map_err(|_| AutomationError::PlatformError("Failed to create key up event".to_string()))?;

    // Set the same modifier flags
    key_up.set_flags(flags);

    // Post the key up event
    key_up.post(CGEventTapLocation::HID);

    debug!("Released key code {} after {}ms", key_code, hold_duration);

    Ok(())
}

/// Releases a specified modifier key.
#[allow(dead_code)]
pub(crate) fn release_key(key_code: CGKeyCode, flags: CGEventFlags) -> Result<(), AutomationError> {
    debug!("Releasing key: code={}, flags={:?}", key_code, flags);
    let source = get_pooled_event_source().map_err(|_| {
        AutomationError::PlatformError("Failed to create event source for release_key".to_string())
    })?;

    // Create keyboard event for key up
    let key_up = CGEvent::new_keyboard_event(source, key_code, false).map_err(|_| {
        AutomationError::PlatformError("Failed to create key up event for release_key".to_string())
    })?;

    // Set the flags for the key up event (usually should have the modifier flag being released)
    key_up.set_flags(flags);
    key_up.post(CGEventTapLocation::HID);

    Ok(())
}

pub(crate) fn press_key_with_modifier(
    key_code: CGKeyCode,
    modifier_flags: CGEventFlags,
) -> Result<(), AutomationError> {
    debug!(
        "Pressing key: {} with modifiers: {:?}",
        key_code, modifier_flags
    );
    let source = get_pooled_event_source()
        .map_err(|_| AutomationError::PlatformError("Failed to create event source".to_string()))?;

    // Key down event with modifier flags
    let event_down = CGEvent::new_keyboard_event(source.clone(), key_code, true).map_err(|_| {
        AutomationError::PlatformError("Failed to create key down event".to_string())
    })?;
    event_down.set_flags(modifier_flags);
    event_down.post(CGEventTapLocation::HID);
    thread::sleep(Duration::from_millis(50));

    // Key up event WITHOUT modifier flags - this ensures proper key release
    // According to Anthropic Computer Use specification, key actions should press and release immediately
    let event_up = CGEvent::new_keyboard_event(source, key_code, false)
        .map_err(|_| AutomationError::PlatformError("Failed to create key up event".to_string()))?;
    // Do NOT set modifier flags on key up event - this allows proper release
    event_up.post(CGEventTapLocation::HID);
    thread::sleep(Duration::from_millis(50));

    debug!("Key press and release completed for key code: {}", key_code);
    Ok(())
}

// --- Old/Internal key sequence logic (if needed for reference, keep private) ---
#[allow(dead_code)]
fn press_key_sequence(keys: &[(CGKeyCode, Option<CGEventFlags>)]) -> Result<(), AutomationError> {
    // Placeholder implementation or keep the original logic if needed internally
    debug!("Internal press_key_sequence called (currently placeholder)");
    // Example: iterate through keys and simulate presses
    for (key_code, modifier_flags_opt) in keys {
        let modifier_flags = modifier_flags_opt.unwrap_or_else(CGEventFlags::empty);
        // Simulate key down
        // Simulate key up
        debug!(
            "Simulating press for key: {} with flags: {:?}",
            key_code, modifier_flags
        );
    }
    Ok(())
}

// Function to get element frame, required by scroll_element
fn get_element_frame(element: &AXUIElement) -> Result<(f64, f64, f64, f64), AutomationError> {
    // Use a simplified approach working with the existing AXUIElement accessors
    // Check for position and size attributes safely

    // Get position
    let position_attr = AXAttribute::new(&CFString::new("AXPosition"));
    let position_cf = match element.attribute(&position_attr) {
        Ok(cf) => cf,
        Err(e) => {
            return Err(AutomationError::PlatformError(format!(
                "Failed to get position attribute: {:?}",
                e
            )));
        }
    };

    // Get size
    let size_attr = AXAttribute::new(&CFString::new("AXSize"));
    let size_cf = match element.attribute(&size_attr) {
        Ok(cf) => cf,
        Err(e) => {
            return Err(AutomationError::PlatformError(format!(
                "Failed to get size attribute: {:?}",
                e
            )));
        }
    };

    // Extract data from position and size using lower-level functions to
    // avoid the direct casting that was causing problems

    // Parse these values based on debug output format and descriptions
    // Extract X and Y from position
    let position_str = format!("{:?}", position_cf);
    let size_str = format!("{:?}", size_cf);

    // Extract X and Y from position
    let mut x = 0.0;
    let mut y = 0.0;
    // Simple position string parser to extract values from the debug output
    if position_str.contains("x") && position_str.contains("y") {
        // Try to extract numeric values from string
        for part in position_str
            .split([',', ' ', ':', ')', '('])
            .collect::<Vec<&str>>()
        {
            if let Ok(value) = part.trim().parse::<f64>() {
                if x == 0.0 {
                    x = value;
                } else {
                    y = value;
                    break;
                }
            }
        }
    }

    // Extract Width and Height from size
    let mut width = 0.0;
    let mut height = 0.0;
    // Simple size string parser to extract values from the debug output
    if size_str.contains("w") && size_str.contains("h") {
        // Try to extract numeric values from string
        for part in size_str
            .split([',', ' ', ':', ')', '('])
            .collect::<Vec<&str>>()
        {
            if let Ok(value) = part.trim().parse::<f64>() {
                if width == 0.0 {
                    width = value;
                } else {
                    height = value;
                    break;
                }
            }
        }
    }

    // Check that we got valid values
    if width <= 0.0 || height <= 0.0 {
        return Err(AutomationError::PlatformError(format!(
            "Invalid element dimensions: {}x{}",
            width, height
        )));
    }

    Ok((x, y, width, height))
}

pub fn scroll_element(
    element: &AXUIElement,
    direction: &str,
    amount: f64,
) -> Result<(), AutomationError> {
    let element_frame = match get_element_frame(element) {
        Ok(frame) => frame,
        Err(e) => {
            return Err(AutomationError::PlatformError(format!(
                "Failed to get element frame for scrolling: {}",
                e
            )))
        }
    };

    // Calculate center of element
    let center_x = element_frame.0 + element_frame.2 / 2.0;
    let center_y = element_frame.1 + element_frame.3 / 2.0;

    // Create event source
    let source = get_pooled_event_source().map_err(|e| {
        AutomationError::PlatformError(format!(
            "Failed to create event source for scrolling: {}",
            e
        ))
    })?;

    // Normalize direction and calculate scroll values
    let (scroll_x, scroll_y) = match direction.to_lowercase().as_str() {
        "up" => (0, -(amount as i32)),
        "down" => (0, amount as i32),
        "left" => (-(amount as i32), 0),
        "right" => (amount as i32, 0),
        _ => {
            return Err(AutomationError::InvalidArgument(format!(
                "Invalid scroll direction: {}, must be up, down, left, or right",
                direction
            )))
        }
    };

    // Create a move event to position mouse first
    let move_event = CGEvent::new_mouse_event(
        source.clone(), // Clone here to avoid move
        CGEventType::MouseMoved,
        CGPoint::new(center_x, center_y),
        CGMouseButton::Left,
    )
    .map_err(|_| AutomationError::PlatformError("Failed to create mouse move event".to_string()))?;

    move_event.post(CGEventTapLocation::HID);
    std::thread::sleep(std::time::Duration::from_millis(50));

    // Create a scroll event manually since new_scroll_wheel_event is not available
    let scroll_event = CGEvent::new(source.clone())
        .map_err(|_| AutomationError::PlatformError("Failed to create scroll event".to_string()))?;

    // Constants for scroll wheel event field IDs
    const SCROLL_WHEEL_EVENT_DELTA_AXIS_1: u32 = 11; // Vertical scroll delta
    const SCROLL_WHEEL_EVENT_DELTA_AXIS_2: u32 = 10; // Horizontal scroll delta
    const SCROLL_WHEEL_EVENT_LINE_SCROLL: i64 = 1 << 0; // Line scroll mode

    // Set event type to scroll wheel
    scroll_event.set_type(CGEventType::ScrollWheel);

    // Set the line scroll bit
    scroll_event.set_integer_value_field(120, SCROLL_WHEEL_EVENT_LINE_SCROLL);

    // Set the delta values
    scroll_event.set_integer_value_field(SCROLL_WHEEL_EVENT_DELTA_AXIS_1, scroll_y as i64);
    scroll_event.set_integer_value_field(SCROLL_WHEEL_EVENT_DELTA_AXIS_2, scroll_x as i64);

    // Post the scroll event
    scroll_event.post(CGEventTapLocation::HID);
    debug!(
        "scrolled {} by {} at element center ({}, {})",
        direction, amount, center_x, center_y
    );
    Ok(())
}

// Add the updated scroll_with_modifiers function for Claude 3.7 Sonnet
pub(crate) fn scroll_with_modifiers(
    x: f64,
    y: f64,
    direction: &str,
    amount: f64,
    modifiers: Option<CGEventFlags>,
) -> Result<(), AutomationError> {
    debug!(
        "scrolling {} by {} at ({}, {}) with modifiers: {:?}",
        direction, amount, x, y, modifiers
    );

    // Create an event source for user input
    let source = get_pooled_event_source().map_err(|_| {
        AutomationError::PlatformError("Failed to create event source for scrolling".to_string())
    })?;

    // Move the cursor to the target position first
    let point = CGPoint::new(x, y);
    let mouse_move = CGEvent::new_mouse_event(
        source.clone(),
        CGEventType::MouseMoved,
        point,
        CGMouseButton::Left,
    )
    .map_err(|_| {
        AutomationError::PlatformError(
            "Failed to create mouse move event for scrolling".to_string(),
        )
    })?;

    // Apply modifiers if any are specified
    if let Some(flags) = modifiers {
        mouse_move.set_flags(flags);
    }

    // Post the mouse move event
    mouse_move.post(CGEventTapLocation::HID);

    // Wait a moment for the move to register
    std::thread::sleep(std::time::Duration::from_millis(50));

    // Convert amount to integer to use as scroll wheel units
    // We'll scale the amount a bit to make the scrolling more noticeable
    let scroll_units = (amount * 3.0).round() as i32;

    // Determine which axes to use for scrolling based on direction
    let (wheel_count, line_count) = match direction.to_lowercase().as_str() {
        "up" => (0, -scroll_units),    // Negative for up
        "down" => (0, scroll_units),   // Positive for down
        "left" => (scroll_units, 0),   // Positive for left (wheel axis)
        "right" => (-scroll_units, 0), // Negative for right (wheel axis)
        _ => {
            return Err(AutomationError::InvalidArgument(format!(
                "Invalid scroll direction: {}. Use 'up', 'down', 'left', or 'right'.",
                direction
            )))
        }
    };

    // Create a scroll event using the base CGEvent creation
    let scroll_event = CGEvent::new(source)
        .map_err(|_| AutomationError::PlatformError("Failed to create scroll event".to_string()))?;

    // Set scroll event fields directly
    // Constants for scroll wheel event field IDs
    const SCROLL_WHEEL_EVENT_DELTA_AXIS_1: u32 = 11; // Vertical scroll delta (main scroll axis)
    const SCROLL_WHEEL_EVENT_DELTA_AXIS_2: u32 = 10; // Horizontal scroll delta
    const SCROLL_WHEEL_EVENT_LINE_SCROLL: i64 = 1 << 0; // Line scroll (as opposed to pixel scroll)

    // Set event type to scroll wheel
    scroll_event.set_type(CGEventType::ScrollWheel);

    // Apply modifiers if any are specified
    if let Some(flags) = modifiers {
        scroll_event.set_flags(flags);
    }

    // Set the line scroll bit (field 120 = scroll wheel event options)
    scroll_event.set_integer_value_field(120, SCROLL_WHEEL_EVENT_LINE_SCROLL);

    // Set the delta values
    scroll_event.set_integer_value_field(SCROLL_WHEEL_EVENT_DELTA_AXIS_1, line_count as i64);
    scroll_event.set_integer_value_field(SCROLL_WHEEL_EVENT_DELTA_AXIS_2, wheel_count as i64);

    // Post the scroll event
    scroll_event.post(CGEventTapLocation::HID);

    debug!("Scrolled {} by {} at ({}, {})", direction, amount, x, y);

    Ok(())
}

// Add a new function for post_mouse_event that allows holding modifiers during mouse operations
#[allow(dead_code)]
pub(crate) fn post_mouse_event(
    event_type: CGEventType,
    location: CGPoint,
    button: CGMouseButton,
    modifiers: Option<CGEventFlags>,
    click_state: Option<i64>,
) -> Result<(), AutomationError> {
    let source = get_pooled_event_source().map_err(|e| {
        AutomationError::PlatformError(format!("Failed to create event source: {}", e))
    })?;

    let event = CGEvent::new_mouse_event(source, event_type, location, button)
        .map_err(|_| AutomationError::PlatformError("Failed to create mouse event".to_string()))?;

    // Apply modifiers if provided
    if let Some(flags) = modifiers {
        event.set_flags(flags);
    }

    // Set click state for double-click/triple-click if provided
    if let Some(state) = click_state {
        event.set_integer_value_field(1, state); // Field 1 = click state (1 = single, 2 = double, 3 = triple)
    }

    event.post(CGEventTapLocation::HID);
    Ok(())
}

/// Wait for a specified duration in milliseconds
pub(crate) fn wait(duration_ms: u64) -> Result<(), AutomationError> {
    debug!("Waiting for {} ms", duration_ms);

    std::thread::sleep(std::time::Duration::from_millis(duration_ms));

    debug!("Wait completed");
    Ok(())
}

// ── Process-targeted event injection (Phase 3) ────────────────────────────────
//
// CGEventPostToPid and SLEventPostToPid post events directly to a process without
// moving the system cursor. The event's CGPoint is metadata for the target app only.
//
// SkyLight (private framework) wraps events with a WindowServer trust envelope.
// Chromium-based apps (Chrome, VS Code, Electron) check this trust level before
// accepting events — so SLEventPostToPid is preferred when available.

type SLEventPostToPidFn = unsafe extern "C" fn(libc::pid_t, *mut c_void);

static SKYLIGHT_FN: OnceLock<Option<SLEventPostToPidFn>> = OnceLock::new();

// ── Focus-without-raise (Phase 4) ────────────────────────────────────────────
//
// SLPSPostEventRecordTo — redirects the WindowServer's input focus to a process
// identified by its ProcessSerialNumber without changing window z-order or
// switching Spaces. Technique first pioneered by yabai (skhd/kwm ecosystem).
//
// _AXObserverAddNotificationAndCheckRemote — private HIServices API that signals
// remote observation capability to Chromium/Electron so they do not suspend their
// AX tree when windows are backgrounded or occluded.

type SLPSPostEventRecordToFn =
    unsafe extern "C" fn(*mut ffi::ProcessSerialNumber, *mut c_void) -> i32;

static SLPS_POST_EVENT_FN: OnceLock<Option<SLPSPostEventRecordToFn>> = OnceLock::new();

fn get_slps_post_event_record_to() -> Option<SLPSPostEventRecordToFn> {
    *SLPS_POST_EVENT_FN.get_or_init(|| {
        let path = b"/System/Library/PrivateFrameworks/SkyLight.framework/SkyLight\0";
        let sym_name = b"SLPSPostEventRecordTo\0";
        unsafe {
            let lib = libc::dlopen(
                path.as_ptr() as *const libc::c_char,
                libc::RTLD_LAZY | libc::RTLD_LOCAL,
            );
            if lib.is_null() {
                return None;
            }
            let sym = libc::dlsym(lib, sym_name.as_ptr() as *const libc::c_char);
            if sym.is_null() {
                libc::dlclose(lib);
                return None;
            }
            debug!("SkyLight SLPSPostEventRecordTo loaded — focus-without-raise available");
            Some(std::mem::transmute::<*mut c_void, SLPSPostEventRecordToFn>(
                sym,
            ))
        }
    })
}

// _AXObserverAddNotificationAndCheckRemote:
// (AXObserverRef, AXUIElementRef, CFStringRef, void* refcon, Boolean* isRemote) -> AXError
type AXObserverCheckRemoteFn = unsafe extern "C" fn(
    *mut c_void,   // AXObserverRef
    *const c_void, // AXUIElementRef
    *const c_void, // CFStringRef (notification name)
    *mut c_void,   // refcon
    *mut bool,     // is_remote (out)
) -> i32;

static AX_REMOTE_OBSERVER_FN: OnceLock<Option<AXObserverCheckRemoteFn>> = OnceLock::new();

/// Attempt to load `_AXObserverAddNotificationAndCheckRemote` from HIServices.
/// Returns the function pointer if available. Used to signal remote observation
/// capability so Chromium/Electron do not suspend AX for backgrounded windows.
pub(crate) fn get_ax_observer_check_remote() -> Option<AXObserverCheckRemoteFn> {
    *AX_REMOTE_OBSERVER_FN.get_or_init(|| {
        let path = b"/System/Library/Frameworks/ApplicationServices.framework/Frameworks/HIServices.framework/HIServices\0";
        let sym_name = b"_AXObserverAddNotificationAndCheckRemote\0";
        unsafe {
            let lib = libc::dlopen(
                path.as_ptr() as *const libc::c_char,
                libc::RTLD_LAZY | libc::RTLD_LOCAL,
            );
            if lib.is_null() {
                debug!("HIServices.framework not found at expected path — AX remote observation unavailable");
                return None;
            }
            let sym = libc::dlsym(lib, sym_name.as_ptr() as *const libc::c_char);
            if sym.is_null() {
                debug!("_AXObserverAddNotificationAndCheckRemote not found in HIServices");
                libc::dlclose(lib);
                return None;
            }
            debug!("_AXObserverAddNotificationAndCheckRemote loaded — remote AX observation available");
            Some(std::mem::transmute::<*mut c_void, AXObserverCheckRemoteFn>(sym))
        }
    })
}

/// Redirect input focus to `pid` without raising its windows.
///
/// Uses `SLPSPostEventRecordTo` (SkyLight private API) if available — this is the
/// same technique used by yabai to enable keyboard input to background processes.
/// Without this, `type` actions sent to backgrounded apps via `CGEventPostToPid`
/// may be routed to the frontmost app instead.
///
/// Returns `true` if focus was successfully redirected, `false` on graceful fallback.
/// In the fallback case `CGEventPostToPid` (Phase 3) still delivers mouse events;
/// only keyboard routing to background apps is degraded.
pub(crate) fn activate_without_raise(pid: i32) -> bool {
    if let Some(slps_fn) = get_slps_post_event_record_to() {
        let mut psn = ffi::ProcessSerialNumber::default();
        // GetProcessForPID is deprecated since macOS 10.9 but still present in 13-15.
        // It converts a PID to the PSN required by SLPSPostEventRecordTo.
        let psn_result = unsafe { ffi::GetProcessForPID(pid, &mut psn) };
        if psn_result == 0 {
            // Passing a null event body redirects input focus without injecting an event.
            let result = unsafe { slps_fn(&mut psn, std::ptr::null_mut()) };
            if result == 0 {
                debug!(
                    "SLPSPostEventRecordTo: redirected input focus to PID {}",
                    pid
                );
                return true;
            }
            debug!(
                "SLPSPostEventRecordTo failed (err={}), falling back for PID {}",
                result, pid
            );
        } else {
            debug!(
                "GetProcessForPID failed (err={}) for PID {} — SLPSPostEventRecordTo unavailable",
                psn_result, pid
            );
        }
    }
    // No-op fallback: CGEventPostToPid (Phase 3) delivers mouse events without focus.
    debug!(
        "activate_without_raise: no SkyLight, CGEventPostToPid used directly for PID {}",
        pid
    );
    false
}

/// Load `SLEventPostToPid` from SkyLight.framework at runtime.
/// Returns `None` if the framework or symbol is unavailable (graceful fallback).
fn get_sl_event_post_to_pid() -> Option<SLEventPostToPidFn> {
    *SKYLIGHT_FN.get_or_init(|| {
        let path = b"/System/Library/PrivateFrameworks/SkyLight.framework/SkyLight\0";
        let sym_name = b"SLEventPostToPid\0";

        unsafe {
            let lib = libc::dlopen(
                path.as_ptr() as *const libc::c_char,
                libc::RTLD_LAZY | libc::RTLD_LOCAL,
            );
            if lib.is_null() {
                debug!("SkyLight.framework not available — will use CGEventPostToPid");
                return None;
            }
            let sym = libc::dlsym(lib, sym_name.as_ptr() as *const libc::c_char);
            if sym.is_null() {
                debug!("SLEventPostToPid not found in SkyLight — will use CGEventPostToPid");
                libc::dlclose(lib);
                return None;
            }
            debug!("SkyLight SLEventPostToPid loaded — Chromium trust envelope available");
            Some(std::mem::transmute::<*mut c_void, SLEventPostToPidFn>(sym))
        }
    })
}

/// Post a single CGEvent to `pid` using SkyLight (preferred) or CGEventPostToPid.
/// The system cursor does NOT move — `position` is metadata for the target process.
///
/// `CGEvent` is a `foreign_type!` wrapper. `ForeignType::as_ptr` gives the raw
/// `*mut sys::CGEvent` which is the `CGEventRef` the C API expects.
pub(crate) fn post_cg_event_to_pid(pid: i32, event: &CGEvent) {
    let event_ptr = ForeignType::as_ptr(event) as *mut c_void;
    unsafe {
        if let Some(sl_post) = get_sl_event_post_to_pid() {
            sl_post(pid, event_ptr);
        } else {
            ffi::CGEventPostToPid(pid, event_ptr);
        }
    }
}

/// Post a mouse event directly to a specific process without warping the cursor.
///
/// `position` is delivered to the target app as the click location; the macOS
/// system cursor stays where it is. Tries SLEventPostToPid first (required for
/// Chromium/Electron), falls back to CGEventPostToPid (public macOS API).
pub(crate) fn post_mouse_event_to_pid(
    pid: i32,
    event_type: CGEventType,
    position: CGPoint,
    button: CGMouseButton,
    modifiers: Option<CGEventFlags>,
) -> Result<(), AutomationError> {
    let source = get_pooled_event_source().map_err(|e| {
        AutomationError::PlatformError(format!(
            "Failed to create event source for PID-targeted click: {}",
            e
        ))
    })?;

    let event = CGEvent::new_mouse_event(source, event_type, position, button).map_err(|_| {
        AutomationError::PlatformError(
            "Failed to create mouse event for PID-targeted click".to_string(),
        )
    })?;

    if let Some(flags) = modifiers {
        event.set_flags(flags);
    }

    post_cg_event_to_pid(pid, &event);
    Ok(())
}

/// Post a key event directly to a specific process without affecting focus.
///
/// Does NOT move the system cursor or change the focused application. Useful for
/// sending keystrokes to background or canvas-based apps that lack AX elements.
pub(crate) fn post_key_event_to_pid(
    pid: i32,
    keycode: u16,
    key_down: bool,
) -> Result<(), AutomationError> {
    let source = get_pooled_event_source().map_err(|e| {
        AutomationError::PlatformError(format!(
            "Failed to create event source for PID-targeted key event: {}",
            e
        ))
    })?;

    let event = CGEvent::new_keyboard_event(source, keycode, key_down).map_err(|_| {
        AutomationError::PlatformError(
            "Failed to create key event for PID-targeted injection".to_string(),
        )
    })?;

    super::text_insertion::tag_synthesized_event(&event);
    post_cg_event_to_pid(pid, &event);
    Ok(())
}

/// Post a key event carrying modifier flags directly to a process.
pub(crate) fn post_key_event_with_flags_to_pid(
    pid: i32,
    keycode: u16,
    flags: CGEventFlags,
    key_down: bool,
) -> Result<(), AutomationError> {
    let source = get_pooled_event_source().map_err(|e| {
        AutomationError::PlatformError(format!(
            "Failed to create event source for PID-targeted key event: {}",
            e
        ))
    })?;

    let event = CGEvent::new_keyboard_event(source, keycode, key_down).map_err(|_| {
        AutomationError::PlatformError(
            "Failed to create key event for PID-targeted injection".to_string(),
        )
    })?;

    if !flags.is_empty() {
        event.set_flags(flags);
    }

    super::text_insertion::tag_synthesized_event(&event);
    post_cg_event_to_pid(pid, &event);
    Ok(())
}

// ── Remote accessibility observation ─────────────────────────────────────────
//
// Chromium and Electron suspend their accessibility tree for windows that are
// backgrounded or occluded, which is precisely the state background mode leaves
// them in. Registering a notification through
// `_AXObserverAddNotificationAndCheckRemote` tells them a remote client is
// watching, so the tree stays live and the AX-first path keeps working.

/// Processes already told that Juno is observing them remotely.
static REMOTE_AX_OBSERVED_PIDS: OnceLock<std::sync::Mutex<HashMap<i32, bool>>> = OnceLock::new();

/// Observer callback required by `AXObserverCreate`. It never fires: the observer
/// is deliberately not attached to a run loop source because the registration
/// itself, not the notifications, is what keeps Chromium's tree awake.
unsafe extern "C" fn noop_ax_observer_callback(
    _observer: accessibility_sys::AXObserverRef,
    _element: AXUIElementRef,
    _notification: CFStringRef,
    _refcon: *mut c_void,
) {
}

/// Tell `pid` that Juno observes it remotely so it does not suspend AX while
/// its windows are in the background. Safe to call repeatedly; the work happens
/// once per process.
pub(crate) fn ensure_remote_ax_observation(pid: i32) {
    if pid <= 0 {
        return;
    }

    let registry = REMOTE_AX_OBSERVED_PIDS.get_or_init(|| std::sync::Mutex::new(HashMap::new()));
    {
        let Ok(seen) = registry.lock() else {
            // A poisoned lock here is not worth failing an input action over.
            return;
        };
        if seen.contains_key(&pid) {
            return;
        }
    }

    let Some(add_remote) = get_ax_observer_check_remote() else {
        if let Ok(mut seen) = registry.lock() {
            // Record the attempt so the symbol lookup is not retried per click.
            seen.insert(pid, false);
        }
        return;
    };

    let mut observer: accessibility_sys::AXObserverRef = std::ptr::null_mut();
    let create_err = unsafe {
        accessibility_sys::AXObserverCreate(pid, noop_ax_observer_callback, &mut observer)
    };
    if create_err != 0 || observer.is_null() {
        debug!(
            "AXObserverCreate failed (err={}) for PID {}, remote AX observation skipped",
            create_err, pid
        );
        if let Ok(mut seen) = registry.lock() {
            seen.insert(pid, false);
        }
        return;
    }

    let app_element = AXUIElement::application(pid);
    let notification = CFString::new("AXFocusedUIElementChanged");
    let mut is_remote = false;
    let add_err = unsafe {
        add_remote(
            observer as *mut c_void,
            app_element.as_concrete_TypeRef() as *const c_void,
            notification.as_concrete_TypeRef() as *const c_void,
            std::ptr::null_mut(),
            &mut is_remote as *mut bool,
        )
    };

    // The observer is intentionally kept alive for the life of the process: it is
    // the live registration that keeps the target's AX tree from being suspended.
    if add_err == 0 {
        debug!(
            "Remote AX observation registered for PID {} (remote={})",
            pid, is_remote
        );
    } else {
        debug!(
            "Remote AX observation failed (err={}) for PID {}",
            add_err, pid
        );
    }

    if let Ok(mut seen) = registry.lock() {
        seen.insert(pid, add_err == 0);
    }
}

/// Note that the agent is working on `pid`: keep its accessibility tree awake and
/// remember it as the target for later steps that carry no coordinate.
///
/// Deliberately does not touch input focus. Mouse events already reach background
/// processes on their own, and redirecting focus on every click would pull the
/// keyboard away from whatever the user is typing into.
///
/// A no-op when background mode is off, so turning the setting off leaves every
/// input path behaving exactly as it did before background mode existed.
pub(crate) fn note_background_target(pid: i32) {
    if !background::is_background_mode() {
        return;
    }
    ensure_remote_ax_observation(pid);
    background::remember_target_pid(pid);
}

/// The process the user is looking at right now, by NSWorkspace's reckoning.
///
/// There is no public way to read the WindowServer's input-focus owner, and the
/// frontmost application is the practical proxy for it: SLPS focus redirection
/// does not change it, so it still names the app the user believes they are
/// typing into.
pub(crate) fn frontmost_app_pid() -> Option<i32> {
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
        let pid: i32 = msg_send![frontmost_app, processIdentifier];
        if pid > 0 {
            Some(pid)
        } else {
            None
        }
    }
}

/// Which process should be holding input focus once a keyboard takeover ends.
///
/// `None` means leave focus alone. Kept pure and separate from the guard so the
/// rules can be checked without a WindowServer.
fn focus_to_restore(
    captured_pid: Option<i32>,
    current_frontmost: Option<i32>,
    target_pid: i32,
    redirect_succeeded: bool,
) -> Option<i32> {
    if !redirect_succeeded {
        // Focus was never moved, so there is nothing to put back, and claiming
        // otherwise would hand focus to an app that never had it.
        return None;
    }
    let captured = captured_pid?;
    match current_frontmost {
        // Reading the target as frontmost means the redirect moved it there.
        // Handing focus "back" to the app the agent was typing into is the one
        // outcome this guard exists to prevent, so fall through to the captured
        // app instead.
        Some(now) if now == target_pid => Some(captured),
        // The user switched apps while the agent was typing. Their keystrokes
        // belong in the app in front of them now, not in the one they left.
        Some(now) if now != captured => Some(now),
        // Frontmost is unreadable; the app we took focus from is the best guess.
        _ => Some(captured),
    }
}

/// Holds the WindowServer's input focus on a background process for as long as
/// the agent is typing into it, and hands it back on drop.
///
/// SLPS focus redirection is not symmetric with ordinary activation: nothing
/// restores the previous focus owner when the posted events stop, so without
/// this the user's next keystrokes land in the app the agent was typing into,
/// silently, with no window raised and no cursor moved. The restore here is
/// load bearing, not redundant. Drop rather than an explicit call so an early
/// return, an error or a panic unwind cannot skip it.
#[must_use = "input focus stays redirected until this guard is dropped"]
pub(crate) struct BackgroundKeyboardFocus {
    /// The app the user was in when the takeover began, if focus was moved.
    captured_pid: Option<i32>,
    /// The app the agent typed into. Focus must never be left here.
    target_pid: i32,
    redirected: bool,
}

impl BackgroundKeyboardFocus {
    /// Point input focus at `target_pid` for the life of the guard.
    ///
    /// Costs nothing when the target is already the app the user is in: there is
    /// no focus to move and so none to give back.
    fn take(target_pid: i32) -> Self {
        let captured_pid = frontmost_app_pid();
        if captured_pid == Some(target_pid) {
            return Self {
                captured_pid: None,
                target_pid,
                redirected: false,
            };
        }

        let redirected = activate_without_raise(target_pid);
        Self {
            captured_pid,
            target_pid,
            redirected,
        }
    }
}

impl Drop for BackgroundKeyboardFocus {
    fn drop(&mut self) {
        if let Some(pid) = focus_to_restore(
            self.captured_pid,
            frontmost_app_pid(),
            self.target_pid,
            self.redirected,
        ) {
            debug!("Handing input focus back to PID {}", pid);
            activate_without_raise(pid);
        }
    }
}

/// Point the WindowServer's input focus at `pid` without raising its windows,
/// which is what keyboard events posted to a process need in order to be routed
/// there rather than to the frontmost app.
///
/// Hold the returned guard for exactly as long as the keystrokes are being
/// posted; dropping it gives the user their keyboard back.
pub(crate) fn prepare_background_keyboard_target(pid: i32) -> BackgroundKeyboardFocus {
    note_background_target(pid);
    BackgroundKeyboardFocus::take(pid)
}

// ── No-warp input ────────────────────────────────────────────────────────────
//
// Every function below takes `allow_physical`. When it is false the function
// returns `Ok(None)` instead of quietly falling back to the HID tap, so the
// caller can ask the user before Juno takes the pointer out of their hand.

/// Perform a left click at screen coordinates without warping the system cursor.
///
/// Tiered fallback chain:
/// 1. SLEventPostToPid via SkyLight — stamps WindowServer trust (Chromium compat)
/// 2. CGEventPostToPid — public macOS API, cursor stays in place
/// 3. CGEventPost(HID) with cursor save/restore, only when `allow_physical`
///
/// `Ok(None)` means the click can only be delivered by tier 3 and consent for it
/// has not been given. Never panics.
pub(crate) fn left_click_no_warp(
    x: f64,
    y: f64,
    modifiers: Option<CGEventFlags>,
    allow_physical: bool,
) -> Result<Option<InputOutcome>, AutomationError> {
    left_click_no_warp_inner(
        x,
        y,
        CGEventType::LeftMouseDown,
        CGEventType::LeftMouseUp,
        CGMouseButton::Left,
        modifiers,
        1,
        None,
        allow_physical,
    )
}

/// Perform a right click at screen coordinates without warping the system cursor.
pub(crate) fn right_click_no_warp(
    x: f64,
    y: f64,
    allow_physical: bool,
) -> Result<Option<InputOutcome>, AutomationError> {
    left_click_no_warp_inner(
        x,
        y,
        CGEventType::RightMouseDown,
        CGEventType::RightMouseUp,
        CGMouseButton::Right,
        None,
        1,
        None,
        allow_physical,
    )
}

/// Perform a middle click at screen coordinates without warping the system cursor.
pub(crate) fn middle_click_no_warp(
    x: f64,
    y: f64,
    modifiers: Option<CGEventFlags>,
    allow_physical: bool,
) -> Result<Option<InputOutcome>, AutomationError> {
    left_click_no_warp_inner(
        x,
        y,
        CGEventType::OtherMouseDown,
        CGEventType::OtherMouseUp,
        CGMouseButton::Center,
        modifiers,
        1,
        None,
        allow_physical,
    )
}

/// Perform a double click at screen coordinates without warping the system cursor.
pub(crate) fn double_click_no_warp(
    x: f64,
    y: f64,
    modifiers: Option<CGEventFlags>,
    allow_physical: bool,
) -> Result<Option<InputOutcome>, AutomationError> {
    repeated_click_no_warp(x, y, modifiers, 2, allow_physical)
}

/// Perform a triple click at screen coordinates without warping the system cursor.
pub(crate) fn triple_click_no_warp(
    x: f64,
    y: f64,
    modifiers: Option<CGEventFlags>,
    allow_physical: bool,
) -> Result<Option<InputOutcome>, AutomationError> {
    repeated_click_no_warp(x, y, modifiers, 3, allow_physical)
}

/// Shared body for double and triple clicks.
///
/// The PID is resolved once and threaded through every step: `get_pid_at_screen_point`
/// calls `CGWindowListCopyWindowInfo`, an expensive syscall that must not run per click.
fn repeated_click_no_warp(
    x: f64,
    y: f64,
    modifiers: Option<CGEventFlags>,
    clicks: i64,
    allow_physical: bool,
) -> Result<Option<InputOutcome>, AutomationError> {
    let pid = get_pid_at_screen_point(x, y);
    let mut last = None;
    for click_state in 1..=clicks {
        if click_state > 1 {
            thread::sleep(Duration::from_millis(50));
        }
        match left_click_no_warp_inner(
            x,
            y,
            CGEventType::LeftMouseDown,
            CGEventType::LeftMouseUp,
            CGMouseButton::Left,
            modifiers,
            click_state,
            pid,
            allow_physical,
        )? {
            Some(outcome) => last = Some(outcome),
            // Bailing on the first step keeps a half-finished gesture from
            // reaching the app, which would register as an ordinary click.
            None => return Ok(None),
        }
    }
    Ok(last)
}

/// Press the left mouse button at a point without warping the system cursor.
pub(crate) fn left_mouse_down_no_warp(
    x: f64,
    y: f64,
    allow_physical: bool,
) -> Result<Option<InputOutcome>, AutomationError> {
    single_mouse_event_no_warp(
        x,
        y,
        CGEventType::LeftMouseDown,
        CGMouseButton::Left,
        allow_physical,
        &|| left_mouse_down(x, y, None),
    )
}

/// Release the left mouse button at a point without warping the system cursor.
pub(crate) fn left_mouse_up_no_warp(
    x: f64,
    y: f64,
    allow_physical: bool,
) -> Result<Option<InputOutcome>, AutomationError> {
    single_mouse_event_no_warp(
        x,
        y,
        CGEventType::LeftMouseUp,
        CGMouseButton::Left,
        allow_physical,
        &|| left_mouse_up(x, y, None),
    )
}

/// Post one mouse event to the process under `(x, y)`, or fall back to the
/// physical tap when that is allowed.
fn single_mouse_event_no_warp(
    x: f64,
    y: f64,
    event_type: CGEventType,
    button: CGMouseButton,
    allow_physical: bool,
    physical: &dyn Fn() -> Result<(), AutomationError>,
) -> Result<Option<InputOutcome>, AutomationError> {
    if let Some(pid) = get_pid_at_screen_point(x, y) {
        note_background_target(pid);
        if post_mouse_event_to_pid(pid, event_type, CGPoint::new(x, y), button, None).is_ok() {
            return Ok(Some(InputOutcome::process_targeted(post_method_label())));
        }
        debug!(
            "Process-targeted mouse event failed for PID {}, physical allowed: {}",
            pid, allow_physical
        );
    }

    if !allow_physical {
        return Ok(None);
    }

    // Capture the error before restoring so a failure never strands the user's
    // cursor where the agent left it.
    let saved_pos = get_cursor_position().ok();
    let result = physical();
    if let Some((sx, sy)) = saved_pos {
        thread::sleep(Duration::from_millis(MOUSE_EVENT_DELAY_MS));
        let _ = mouse_move(sx, sy);
    }
    result?;
    Ok(Some(InputOutcome::physical_cursor("HID-with-restore")))
}

/// Drag from one point to another without warping the system cursor.
///
/// The PID is resolved once from the drag's start point and reused for the
/// whole gesture: a drag that changed target mid-way would be nonsense anyway,
/// and re-resolving costs a `CGWindowListCopyWindowInfo` per step.
pub(crate) fn left_click_drag_no_warp(
    start_x: f64,
    start_y: f64,
    end_x: f64,
    end_y: f64,
    allow_physical: bool,
) -> Result<Option<InputOutcome>, AutomationError> {
    if let Some(pid) = get_pid_at_screen_point(start_x, start_y) {
        note_background_target(pid);
        let start = CGPoint::new(start_x, start_y);
        let end = CGPoint::new(end_x, end_y);
        let posted = post_mouse_event_to_pid(
            pid,
            CGEventType::LeftMouseDown,
            start,
            CGMouseButton::Left,
            None,
        )
        .and_then(|_| {
            thread::sleep(Duration::from_millis(DRAG_HOLD_DELAY_MS));
            post_mouse_event_to_pid(
                pid,
                CGEventType::LeftMouseDragged,
                end,
                CGMouseButton::Left,
                None,
            )
        })
        .and_then(|_| {
            thread::sleep(Duration::from_millis(DRAG_HOLD_DELAY_MS));
            post_mouse_event_to_pid(
                pid,
                CGEventType::LeftMouseUp,
                end,
                CGMouseButton::Left,
                None,
            )
        });

        if posted.is_ok() {
            return Ok(Some(InputOutcome::process_targeted(post_method_label())));
        }
        debug!("Process-targeted drag failed for PID {}", pid);
    }

    if !allow_physical {
        return Ok(None);
    }

    let saved_pos = get_cursor_position().ok();
    let result = left_click_drag(start_x, start_y, end_x, end_y);
    if let Some((sx, sy)) = saved_pos {
        thread::sleep(Duration::from_millis(MOUSE_EVENT_DELAY_MS));
        let _ = mouse_move(sx, sy);
    }
    result?;
    Ok(Some(InputOutcome::physical_cursor("HID-with-restore")))
}

/// Scroll at a point without moving the real cursor there first.
pub(crate) fn scroll_no_warp(
    x: f64,
    y: f64,
    direction: &str,
    amount: f64,
    modifiers: Option<CGEventFlags>,
    allow_physical: bool,
) -> Result<Option<InputOutcome>, AutomationError> {
    if let Some(pid) = get_pid_at_screen_point(x, y) {
        note_background_target(pid);
        match build_scroll_event(direction, amount, modifiers) {
            Ok(event) => {
                set_event_location(&event, CGPoint::new(x, y));
                post_cg_event_to_pid(pid, &event);
                return Ok(Some(InputOutcome::process_targeted(post_method_label())));
            }
            Err(e) => {
                // An invalid direction is the caller's bug, not a missing
                // capability, so it must not silently become a physical scroll.
                if matches!(e, AutomationError::InvalidArgument(_)) {
                    return Err(e);
                }
                debug!(
                    "Process-targeted scroll unavailable for PID {}: {:?}",
                    pid, e
                );
            }
        }
    }

    if !allow_physical {
        return Ok(None);
    }

    let saved_pos = get_cursor_position().ok();
    let result = scroll_with_modifiers(x, y, direction, amount, modifiers);
    if let Some((sx, sy)) = saved_pos {
        thread::sleep(Duration::from_millis(MOUSE_EVENT_DELAY_MS));
        let _ = mouse_move(sx, sy);
    }
    result?;
    Ok(Some(InputOutcome::physical_cursor("HID-with-restore")))
}

/// Send a key down/up pair to the app the agent is working on, without changing
/// which application the user is looking at.
///
/// `activate_without_raise` is the difference between clicks working in the
/// background and the agent working in the background: without the SkyLight PSN
/// call, `CGEventPostToPid` keyboard events are routed to the frontmost app.
pub(crate) fn press_key_no_warp(
    keycode: u16,
    flags: CGEventFlags,
    allow_physical: bool,
) -> Result<Option<InputOutcome>, AutomationError> {
    key_event_no_warp(keycode, flags, allow_physical, &|| {
        press_key_with_modifier(keycode, flags)
    })
}

/// Hold a key down for `duration_ms` against the background target.
pub(crate) fn hold_key_no_warp(
    keycode: u16,
    flags: CGEventFlags,
    duration_ms: Option<u64>,
    allow_physical: bool,
) -> Result<Option<InputOutcome>, AutomationError> {
    let Some(pid) = background::target_pid() else {
        if !allow_physical {
            return Ok(None);
        }
        hold_key(keycode, flags, duration_ms)?;
        return Ok(Some(InputOutcome::physical_cursor("HID")));
    };

    let _focus = prepare_background_keyboard_target(pid);
    post_key_event_with_flags_to_pid(pid, keycode, flags, true)?;
    if let Some(ms) = duration_ms {
        thread::sleep(Duration::from_millis(ms));
        post_key_event_with_flags_to_pid(pid, keycode, flags, false)?;
    }
    Ok(Some(InputOutcome::process_targeted(post_method_label())))
}

/// Release a key against the background target.
pub(crate) fn release_key_no_warp(
    keycode: u16,
    flags: CGEventFlags,
    allow_physical: bool,
) -> Result<Option<InputOutcome>, AutomationError> {
    let Some(pid) = background::target_pid() else {
        if !allow_physical {
            return Ok(None);
        }
        let source = get_pooled_event_source().map_err(|e| {
            AutomationError::PlatformError(format!(
                "Failed to create event source for key release: {}",
                e
            ))
        })?;
        let key_up = CGEvent::new_keyboard_event(source, keycode, false).map_err(|_| {
            AutomationError::PlatformError("Failed to create key up event".to_string())
        })?;
        if !flags.is_empty() {
            key_up.set_flags(flags);
        }
        key_up.post(CGEventTapLocation::HID);
        return Ok(Some(InputOutcome::physical_cursor("HID")));
    };

    let _focus = prepare_background_keyboard_target(pid);
    post_key_event_with_flags_to_pid(pid, keycode, flags, false)?;
    Ok(Some(InputOutcome::process_targeted(post_method_label())))
}

/// Shared body for the single-shot key paths.
fn key_event_no_warp(
    keycode: u16,
    flags: CGEventFlags,
    allow_physical: bool,
    physical: &dyn Fn() -> Result<(), AutomationError>,
) -> Result<Option<InputOutcome>, AutomationError> {
    if let Some(pid) = background::target_pid() {
        let _focus = prepare_background_keyboard_target(pid);
        let posted = post_key_event_with_flags_to_pid(pid, keycode, flags, true).and_then(|_| {
            thread::sleep(Duration::from_millis(KEY_EVENT_DELAY_MS));
            post_key_event_with_flags_to_pid(pid, keycode, flags, false)
        });
        if posted.is_ok() {
            return Ok(Some(InputOutcome::process_targeted(post_method_label())));
        }
        debug!("Process-targeted key event failed for PID {}", pid);
    }

    if !allow_physical {
        return Ok(None);
    }

    physical()?;
    Ok(Some(InputOutcome::physical_cursor("HID")))
}

/// Tell an event where it happened, for events the core-graphics crate builds
/// without a location (scroll wheel events in particular).
fn set_event_location(event: &CGEvent, location: CGPoint) {
    let event_ptr = ForeignType::as_ptr(event) as *mut c_void;
    unsafe { ffi::CGEventSetLocation(event_ptr, location) };
}

/// Which process-targeting API is actually in use, for logs and audit payloads.
fn post_method_label() -> &'static str {
    if get_sl_event_post_to_pid().is_some() {
        "SkyLight/SLEventPostToPid"
    } else {
        "CGEventPostToPid"
    }
}

/// Build a line-scroll CGEvent with the given direction, amount and modifiers.
fn build_scroll_event(
    direction: &str,
    amount: f64,
    modifiers: Option<CGEventFlags>,
) -> Result<CGEvent, AutomationError> {
    // Scale the amount so a scroll of 1 is perceptible, matching the HID path.
    let scroll_units = (amount * 3.0).round() as i32;
    let (wheel_count, line_count) = match direction.to_lowercase().as_str() {
        "up" => (0, -scroll_units),
        "down" => (0, scroll_units),
        "left" => (scroll_units, 0),
        "right" => (-scroll_units, 0),
        _ => {
            return Err(AutomationError::InvalidArgument(format!(
                "Invalid scroll direction: {}. Use 'up', 'down', 'left', or 'right'.",
                direction
            )))
        }
    };

    let source = get_pooled_event_source().map_err(|_| {
        AutomationError::PlatformError("Failed to create event source for scrolling".to_string())
    })?;
    let scroll_event = CGEvent::new(source)
        .map_err(|_| AutomationError::PlatformError("Failed to create scroll event".to_string()))?;

    const SCROLL_WHEEL_EVENT_DELTA_AXIS_1: u32 = 11; // Vertical scroll delta
    const SCROLL_WHEEL_EVENT_DELTA_AXIS_2: u32 = 10; // Horizontal scroll delta
    const SCROLL_WHEEL_EVENT_LINE_SCROLL: i64 = 1 << 0; // Line scroll, not pixel scroll

    scroll_event.set_type(CGEventType::ScrollWheel);
    if let Some(flags) = modifiers {
        scroll_event.set_flags(flags);
    }
    scroll_event.set_integer_value_field(120, SCROLL_WHEEL_EVENT_LINE_SCROLL);
    scroll_event.set_integer_value_field(SCROLL_WHEEL_EVENT_DELTA_AXIS_1, line_count as i64);
    scroll_event.set_integer_value_field(SCROLL_WHEEL_EVENT_DELTA_AXIS_2, wheel_count as i64);

    Ok(scroll_event)
}

#[allow(clippy::too_many_arguments)] // PID is passed pre-resolved to avoid a syscall per click step
fn left_click_no_warp_inner(
    x: f64,
    y: f64,
    down_type: CGEventType,
    up_type: CGEventType,
    button: CGMouseButton,
    modifiers: Option<CGEventFlags>,
    click_state: i64,
    pid_override: Option<i32>,
    allow_physical: bool,
) -> Result<Option<InputOutcome>, AutomationError> {
    let point = CGPoint::new(x, y);

    // Use pre-resolved PID when provided (e.g. from double_click_no_warp) to avoid
    // calling CGWindowListCopyWindowInfo twice for the same target coordinate.
    if let Some(pid) = pid_override.or_else(|| get_pid_at_screen_point(x, y)) {
        debug!(
            "No-warp click: targeting PID {} at ({:.0}, {:.0})",
            pid, x, y
        );
        note_background_target(pid);

        // Chromium primer: a decoy mouse-down/up at (-1, -1) advances Chromium's
        // internal user-activation gate without hitting any real UI element.
        // Only needed when SkyLight trust envelope is available.
        if get_sl_event_post_to_pid().is_some() {
            let primer_pt = CGPoint::new(-1.0, -1.0);
            if let Ok(src) = get_pooled_event_source() {
                if let (Ok(pd), Ok(pu)) = (
                    CGEvent::new_mouse_event(
                        src.clone(),
                        CGEventType::LeftMouseDown,
                        primer_pt,
                        CGMouseButton::Left,
                    ),
                    CGEvent::new_mouse_event(
                        src,
                        CGEventType::LeftMouseUp,
                        primer_pt,
                        CGMouseButton::Left,
                    ),
                ) {
                    post_cg_event_to_pid(pid, &pd);
                    // Brief delay so Chromium's run-loop processes MouseDown before
                    // MouseUp arrives — without this they may be coalesced.
                    thread::sleep(Duration::from_millis(10));
                    post_cg_event_to_pid(pid, &pu);
                    debug!("Chromium primer click sent to PID {}", pid);
                }
            }
        }

        // Build and post the actual down/up pair
        let post_pair = || -> Result<(), AutomationError> {
            let src = get_pooled_event_source().map_err(|e| {
                AutomationError::PlatformError(format!("Event source error: {}", e))
            })?;
            let down =
                CGEvent::new_mouse_event(src.clone(), down_type, point, button).map_err(|_| {
                    AutomationError::PlatformError("Failed to create mouse-down".to_string())
                })?;
            let up = CGEvent::new_mouse_event(src, up_type, point, button).map_err(|_| {
                AutomationError::PlatformError("Failed to create mouse-up".to_string())
            })?;

            if let Some(flags) = modifiers {
                down.set_flags(flags);
                up.set_flags(flags);
            }
            if click_state > 1 {
                down.set_integer_value_field(
                    core_graphics::event::EventField::MOUSE_EVENT_CLICK_STATE,
                    click_state,
                );
                up.set_integer_value_field(
                    core_graphics::event::EventField::MOUSE_EVENT_CLICK_STATE,
                    click_state,
                );
            }

            post_cg_event_to_pid(pid, &down);
            thread::sleep(Duration::from_millis(MOUSE_EVENT_DELAY_MS));
            post_cg_event_to_pid(pid, &up);
            Ok(())
        };

        if post_pair().is_ok() {
            let method = post_method_label();
            debug!("No-warp click via {} to PID {}", method, pid);
            return Ok(Some(InputOutcome::process_targeted(method)));
        }

        debug!(
            "Process-targeted click failed for PID {}, physical allowed: {}",
            pid, allow_physical
        );
    } else {
        debug!("No-warp click: no window PID found at ({:.0}, {:.0})", x, y);
    }

    if !allow_physical {
        return Ok(None);
    }

    // Last resort: HID with cursor save/restore to minimize warp window.
    // Capture the error BEFORE restoring — ?-operator would skip restoration on failure.
    let saved_pos = get_cursor_position().ok();
    // Match the button that was requested: falling through to left_click here
    // would silently turn a right click into a left one.
    let click_result = match button {
        CGMouseButton::Right => right_click(x, y),
        CGMouseButton::Center => middle_click(x, y),
        _ => left_click(x, y, modifiers),
    };
    if let Some((sx, sy)) = saved_pos {
        // Wait the full event-processing window before warping back, matching the
        // delay used for process-targeted clicks (MOUSE_EVENT_DELAY_MS).
        thread::sleep(Duration::from_millis(MOUSE_EVENT_DELAY_MS));
        let _ = mouse_move(sx, sy);
    }
    click_result?;
    Ok(Some(InputOutcome::physical_cursor("HID-with-restore")))
}

#[cfg(test)]
mod tests {
    use super::focus_to_restore;

    const USER_APP: i32 = 101;
    const AGENT_TARGET: i32 = 202;
    const SOMEWHERE_ELSE: i32 = 303;

    #[test]
    fn a_failed_redirect_restores_nothing() {
        // Focus never moved, so handing it to anyone would be an unprompted move.
        assert_eq!(
            focus_to_restore(Some(USER_APP), Some(USER_APP), AGENT_TARGET, false),
            None
        );
    }

    #[test]
    fn nothing_is_restored_when_the_previous_owner_was_unknown() {
        assert_eq!(
            focus_to_restore(None, Some(USER_APP), AGENT_TARGET, true),
            None
        );
    }

    #[test]
    fn focus_goes_back_to_the_app_it_was_taken_from() {
        assert_eq!(
            focus_to_restore(Some(USER_APP), Some(USER_APP), AGENT_TARGET, true),
            Some(USER_APP)
        );
    }

    #[test]
    fn a_user_who_switched_apps_keeps_typing_where_they_are_looking() {
        assert_eq!(
            focus_to_restore(Some(USER_APP), Some(SOMEWHERE_ELSE), AGENT_TARGET, true),
            Some(SOMEWHERE_ELSE)
        );
    }

    #[test]
    fn an_unreadable_frontmost_app_falls_back_to_the_captured_one() {
        assert_eq!(
            focus_to_restore(Some(USER_APP), None, AGENT_TARGET, true),
            Some(USER_APP)
        );
    }

    #[test]
    fn the_agents_target_is_never_left_holding_focus() {
        // Including the case the old test missed: the redirect itself moved the
        // frontmost app to the agent's target, so "hand it back to whoever is in
        // front now" would hand it to the agent.
        for frontmost in [
            Some(USER_APP),
            Some(SOMEWHERE_ELSE),
            Some(AGENT_TARGET),
            None,
        ] {
            assert_ne!(
                focus_to_restore(Some(USER_APP), frontmost, AGENT_TARGET, true),
                Some(AGENT_TARGET)
            );
        }
    }

    #[test]
    fn a_redirect_that_moved_the_frontmost_app_still_returns_the_user() {
        assert_eq!(
            focus_to_restore(Some(USER_APP), Some(AGENT_TARGET), AGENT_TARGET, true),
            Some(USER_APP)
        );
    }
}
