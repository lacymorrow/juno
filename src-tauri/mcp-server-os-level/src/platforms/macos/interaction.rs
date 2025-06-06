use accessibility::{AXAttribute, AXUIElement};
use accessibility_sys::{AXUIElementSetAttributeValue, AXUIElementRef};
use super::constants::*;
use super::element::MacOSUIElement;
use super::wrappers::ThreadSafeAXUIElement;
use crate::element::UIElementImpl; // Needed for app_attributes in click_auto
use crate::{AutomationError, ClickResult};
use core_foundation::base::{TCFType, CFTypeRef};
use core_foundation::string::{CFString, CFStringRef};
use core_graphics::event::{
    CGEvent, CGEventFlags, CGEventTapLocation, CGEventType, CGKeyCode, CGMouseButton,
};
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
use core_graphics::geometry::CGPoint;
use clipboard_macos::Clipboard; // Import clipboard_macos
use std::collections::HashMap;
use tracing::{debug, warn};
use std::thread;
use std::time::Duration;
 // Import AXValueCreate

// Define key code constants for keyboard shortcuts
const KEYCODE_CMD: CGKeyCode = 55; // Left Command key
const KEYCODE_A: CGKeyCode = 0;    // 'A' key

// --- Moved from element.rs --- //

pub(crate) fn get_application(element: &MacOSUIElement) -> Option<MacOSUIElement> {
    let attr = AXAttribute::new(&CFString::new("AXTopLevelUIElement"));
    match element.element.0.attribute(&attr) {
        Ok(value) => {
            if let Some(app) = value.downcast::<AXUIElement>() {
                Some(MacOSUIElement {
                    element: ThreadSafeAXUIElement::new(app),
                    use_background_apps: element.use_background_apps,
                    activate_app: element.activate_app,
                    cached_role: String::new(),
                    cached_label: None,
                    cached_description: None,
                    cached_value: None,
                })
            } else {
                None
            }
        }
        Err(_) => None,
    }
}

pub(crate) fn click_with_method(
    element: &MacOSUIElement,
) -> Result<ClickResult, AutomationError> {
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
    match element.element.0.perform_action(&press_attr.as_CFString()) {
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
    match element.element.0.perform_action(&click_attr.as_CFString()) {
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
            let source =
                CGEventSource::new(CGEventSourceStateID::HIDSystemState).map_err(|_| {
                    AutomationError::PlatformError("Failed to create event source".to_string())
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
            std::thread::sleep(std::time::Duration::from_millis(50));

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
            std::thread::sleep(std::time::Duration::from_millis(50));

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

pub(crate) fn focus(element: &MacOSUIElement) -> Result<(), AutomationError> {
    let raise_attr = AXAttribute::new(&CFString::new("AXRaise"));
    if element
        .element
        .0
        .perform_action(&raise_attr.as_CFString())
        .is_ok()
    {
        debug!("Successfully raised element");
        if let Some(app) = get_application(element) {
            unsafe {
                let app_ref = app.element.0.as_concrete_TypeRef();
                let attr_str = CFString::new("AXFocusedUIElement");
                let attr_str_ref = attr_str.as_concrete_TypeRef();
                let elem_ref = element.element.0.as_concrete_TypeRef() as CFTypeRef;
                let result = AXUIElementSetAttributeValue(app_ref as AXUIElementRef, attr_str_ref as CFStringRef, elem_ref);
                if result == 0 {
                    debug!("Successfully set focus to element via AXFocusedUIElement");
                    return Ok(());
                } else {
                    let error = accessibility::Error::from(accessibility::Error::Ax(result));
                    debug!("Failed to set AXFocusedUIElement: {:?}", error);
                }
            }
        }
    }
    debug!("Raise action failed or app not found, attempting focus via click");
    click_auto(element).map(|_result| {
        debug!("Focus achieved via click method: {}", _result.method);
        ()
    })
}

// --- New Mouse Functions ---

/// Get the current mouse cursor position.
pub(crate) fn get_cursor_position() -> Result<(f64, f64), AutomationError> {
    // 1. Create an event source.
    let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
        .map_err(|_| AutomationError::PlatformError("Failed to create event source for cursor position".to_string()))?;

    // 2. Create a null mouse event (MouseMoved seems appropriate) using the source.
    //    The specific type and point might not matter if we only need location.
    let event = CGEvent::new_mouse_event(
        source,
        CGEventType::MouseMoved, // Or any other type?
        CGPoint::new(0.0, 0.0), // Dummy point
        CGMouseButton::Left, // Dummy button
    )
    .map_err(|_| AutomationError::PlatformError("Failed to create CGEvent for cursor position".to_string()))?;

    // 3. Get the location from the created event.
    let location = event.location();
    debug!("Retrieved cursor position: ({}, {})", location.x, location.y);
    Ok((location.x, location.y))
}

/// Move the mouse cursor to the specified coordinates.
pub(crate) fn mouse_move(x: f64, y: f64) -> Result<(), AutomationError> {
    let point = CGPoint::new(x, y);
    debug!("Moving mouse to ({}, {})", x, y);
    let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
        .map_err(|_| AutomationError::PlatformError("Failed to create event source for mouse move".to_string()))?;

    let event = CGEvent::new_mouse_event(source, CGEventType::MouseMoved, point, CGMouseButton::Left) // Button doesn't matter for move
        .map_err(|_| AutomationError::PlatformError("Failed to create mouse move event".to_string()))?;

    event.post(CGEventTapLocation::HID);
    // Optional: Add a small delay after moving
    // std::thread::sleep(std::time::Duration::from_millis(10));
    Ok(())
}

/// Press down the left mouse button at specified coordinates.
pub(crate) fn left_mouse_down(x: f64, y: f64, modifiers: Option<CGEventFlags>) -> Result<(), AutomationError> {
    debug!("Mouse down at ({}, {}) with modifiers: {:?}", x, y, modifiers);

    // Create an event source for user input
    let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
        .map_err(|_| AutomationError::PlatformError("Failed to create event source for mouse down".to_string()))?;

    // First, move the cursor to the target position
    let point = CGPoint::new(x, y);
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
pub(crate) fn left_mouse_up(x: f64, y: f64, modifiers: Option<CGEventFlags>) -> Result<(), AutomationError> {
    debug!("Mouse up at ({}, {}) with modifiers: {:?}", x, y, modifiers);

    // Create an event source for user input
    let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
        .map_err(|_| AutomationError::PlatformError("Failed to create event source for mouse up".to_string()))?;

    // Create the point for the current position
    let point = CGPoint::new(x, y);

    // Create the mouse up event
    let mouse_up = CGEvent::new_mouse_event(
        source,
        CGEventType::LeftMouseUp,
        point,
        CGMouseButton::Left,
    )
    .map_err(|_| AutomationError::PlatformError("Failed to create mouse up event".to_string()))?;

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
pub(crate) fn left_click(x: f64, y: f64, modifiers: Option<CGEventFlags>) -> Result<(), AutomationError> {
    debug!("Left click at ({}, {}) with modifiers: {:?}", x, y, modifiers);

    // Use our improved left_mouse_down and left_mouse_up functions
    left_mouse_down(x, y, modifiers)?;

    // Wait a short time between down and up events
    std::thread::sleep(std::time::Duration::from_millis(30));

    left_mouse_up(x, y, modifiers)?;

    debug!("Completed left click at ({}, {})", x, y);

    Ok(())
}

/// Simulate a right click (down + up) at specified coordinates.
pub(crate) fn right_click(x: f64, y: f64) -> Result<(), AutomationError> {
    let point = CGPoint::new(x, y);
    debug!("Performing right click at ({}, {})", x, y);
    mouse_move(x, y)?; // Ensure cursor is at the correct position
    std::thread::sleep(std::time::Duration::from_millis(20));

    let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
        .map_err(|_| AutomationError::PlatformError("Failed to create event source for right click".to_string()))?;

    let down_event = CGEvent::new_mouse_event(source.clone(), CGEventType::RightMouseDown, point, CGMouseButton::Right)
        .map_err(|_| AutomationError::PlatformError("Failed to create right mouse down event".to_string()))?;
    down_event.post(CGEventTapLocation::HID);
    std::thread::sleep(std::time::Duration::from_millis(50));

    let up_event = CGEvent::new_mouse_event(source, CGEventType::RightMouseUp, point, CGMouseButton::Right)
        .map_err(|_| AutomationError::PlatformError("Failed to create right mouse up event".to_string()))?;
    up_event.post(CGEventTapLocation::HID);
    Ok(())
}

/// Simulate a middle click (down + up) at specified coordinates.
pub(crate) fn middle_click(x: f64, y: f64) -> Result<(), AutomationError> {
    let point = CGPoint::new(x, y);
    debug!("Performing middle click at ({}, {})", x, y);
    mouse_move(x, y)?; // Ensure cursor is at the correct position
    std::thread::sleep(std::time::Duration::from_millis(20));

    let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
        .map_err(|_| AutomationError::PlatformError("Failed to create event source for middle click".to_string()))?;

    let down_event = CGEvent::new_mouse_event(source.clone(), CGEventType::OtherMouseDown, point, CGMouseButton::Center)
        .map_err(|_| AutomationError::PlatformError("Failed to create middle mouse down event".to_string()))?;
    down_event.post(CGEventTapLocation::HID);
    std::thread::sleep(std::time::Duration::from_millis(50));

    let up_event = CGEvent::new_mouse_event(source, CGEventType::OtherMouseUp, point, CGMouseButton::Center)
        .map_err(|_| AutomationError::PlatformError("Failed to create middle mouse up event".to_string()))?;
    up_event.post(CGEventTapLocation::HID);
    Ok(())
}

/// Simulate a double click at the specified coordinates.
pub(crate) fn double_click(x: f64, y: f64) -> Result<(), AutomationError> {
    let point = CGPoint::new(x, y);
    debug!("Performing double click at ({}, {})", x, y);
    mouse_move(x, y)?;
    std::thread::sleep(std::time::Duration::from_millis(20));

    let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
        .map_err(|_| AutomationError::PlatformError("Failed to create event source for double click".to_string()))?;

    // First click (down)
    let down1 = CGEvent::new_mouse_event(source.clone(), CGEventType::LeftMouseDown, point, CGMouseButton::Left)
        .map_err(|_| AutomationError::PlatformError("Failed to create double click down1 event".to_string()))?;
    down1.set_integer_value_field(core_graphics::event::EventField::MOUSE_EVENT_CLICK_STATE, 1);
    down1.post(CGEventTapLocation::HID);
    std::thread::sleep(std::time::Duration::from_millis(50));

    // First click (up)
    let up1 = CGEvent::new_mouse_event(source.clone(), CGEventType::LeftMouseUp, point, CGMouseButton::Left)
        .map_err(|_| AutomationError::PlatformError("Failed to create double click up1 event".to_string()))?;
    up1.set_integer_value_field(core_graphics::event::EventField::MOUSE_EVENT_CLICK_STATE, 1);
    up1.post(CGEventTapLocation::HID);
    std::thread::sleep(std::time::Duration::from_millis(50)); // Double click interval

    // Second click (down)
    let down2 = CGEvent::new_mouse_event(source.clone(), CGEventType::LeftMouseDown, point, CGMouseButton::Left)
        .map_err(|_| AutomationError::PlatformError("Failed to create double click down2 event".to_string()))?;
    down2.set_integer_value_field(core_graphics::event::EventField::MOUSE_EVENT_CLICK_STATE, 2);
    down2.post(CGEventTapLocation::HID);
    std::thread::sleep(std::time::Duration::from_millis(50));

    // Second click (up)
    let up2 = CGEvent::new_mouse_event(source, CGEventType::LeftMouseUp, point, CGMouseButton::Left)
        .map_err(|_| AutomationError::PlatformError("Failed to create double click up2 event".to_string()))?;
    up2.set_integer_value_field(core_graphics::event::EventField::MOUSE_EVENT_CLICK_STATE, 2);
    up2.post(CGEventTapLocation::HID);

    Ok(())
}

/// Simulate a triple click at the specified coordinates.
pub(crate) fn triple_click(x: f64, y: f64) -> Result<(), AutomationError> {
    debug!("Performing triple click at ({}, {})", x, y);

    // Instead of calling double_click + left_click, we'll implement the full sequence
    // with proper click state tracking - similar to how double_click is implemented
    let point = CGPoint::new(x, y);
    mouse_move(x, y)?;
    std::thread::sleep(std::time::Duration::from_millis(20));

    let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
        .map_err(|_| AutomationError::PlatformError("Failed to create event source for triple click".to_string()))?;

    // First click (down)
    let down1 = CGEvent::new_mouse_event(source.clone(), CGEventType::LeftMouseDown, point, CGMouseButton::Left)
        .map_err(|_| AutomationError::PlatformError("Failed to create triple click down1 event".to_string()))?;
    down1.set_integer_value_field(core_graphics::event::EventField::MOUSE_EVENT_CLICK_STATE, 1);
    down1.post(CGEventTapLocation::HID);
    std::thread::sleep(std::time::Duration::from_millis(50));

    // First click (up)
    let up1 = CGEvent::new_mouse_event(source.clone(), CGEventType::LeftMouseUp, point, CGMouseButton::Left)
        .map_err(|_| AutomationError::PlatformError("Failed to create triple click up1 event".to_string()))?;
    up1.set_integer_value_field(core_graphics::event::EventField::MOUSE_EVENT_CLICK_STATE, 1);
    up1.post(CGEventTapLocation::HID);
    std::thread::sleep(std::time::Duration::from_millis(50));

    // Second click (down)
    let down2 = CGEvent::new_mouse_event(source.clone(), CGEventType::LeftMouseDown, point, CGMouseButton::Left)
        .map_err(|_| AutomationError::PlatformError("Failed to create triple click down2 event".to_string()))?;
    down2.set_integer_value_field(core_graphics::event::EventField::MOUSE_EVENT_CLICK_STATE, 2);
    down2.post(CGEventTapLocation::HID);
    std::thread::sleep(std::time::Duration::from_millis(50));

    // Second click (up)
    let up2 = CGEvent::new_mouse_event(source.clone(), CGEventType::LeftMouseUp, point, CGMouseButton::Left)
        .map_err(|_| AutomationError::PlatformError("Failed to create triple click up2 event".to_string()))?;
    up2.set_integer_value_field(core_graphics::event::EventField::MOUSE_EVENT_CLICK_STATE, 2);
    up2.post(CGEventTapLocation::HID);
    std::thread::sleep(std::time::Duration::from_millis(50));

    // Third click (down)
    let down3 = CGEvent::new_mouse_event(source.clone(), CGEventType::LeftMouseDown, point, CGMouseButton::Left)
        .map_err(|_| AutomationError::PlatformError("Failed to create triple click down3 event".to_string()))?;
    down3.set_integer_value_field(core_graphics::event::EventField::MOUSE_EVENT_CLICK_STATE, 3);
    down3.post(CGEventTapLocation::HID);
    std::thread::sleep(std::time::Duration::from_millis(50));

    // Third click (up)
    let up3 = CGEvent::new_mouse_event(source, CGEventType::LeftMouseUp, point, CGMouseButton::Left)
        .map_err(|_| AutomationError::PlatformError("Failed to create triple click up3 event".to_string()))?;
    up3.set_integer_value_field(core_graphics::event::EventField::MOUSE_EVENT_CLICK_STATE, 3);
    up3.post(CGEventTapLocation::HID);

    Ok(())
}

/// Simulate dragging with the left mouse button from a start point to an end point.
pub(crate) fn left_click_drag(
    start_x: f64,
    start_y: f64,
    end_x: f64,
    end_y: f64,
) -> Result<(), AutomationError> {
    let start_point = CGPoint::new(start_x, start_y);
    let end_point = CGPoint::new(end_x, end_y);
    debug!(
        "Performing left click drag from ({}, {}) to ({}, {})",
        start_x, start_y, end_x, end_y
    );

    let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
        .map_err(|_| AutomationError::PlatformError("Failed to create event source for drag".to_string()))?;

    // 1. Move to start position
    mouse_move(start_x, start_y)?;
    std::thread::sleep(std::time::Duration::from_millis(20));

    // 2. Press left button down
    let down_event = CGEvent::new_mouse_event(source.clone(), CGEventType::LeftMouseDown, start_point, CGMouseButton::Left)
        .map_err(|_| AutomationError::PlatformError("Failed to create drag down event".to_string()))?;
    down_event.post(CGEventTapLocation::HID);
    std::thread::sleep(std::time::Duration::from_millis(100)); // Hold briefly before dragging

    // 3. Move to end position (drag event)
    let drag_event = CGEvent::new_mouse_event(source.clone(), CGEventType::LeftMouseDragged, end_point, CGMouseButton::Left)
        .map_err(|_| AutomationError::PlatformError("Failed to create drag move event".to_string()))?;
    drag_event.post(CGEventTapLocation::HID);
    std::thread::sleep(std::time::Duration::from_millis(100)); // Pause at end position

    // 4. Release left button
    let up_event = CGEvent::new_mouse_event(source, CGEventType::LeftMouseUp, end_point, CGMouseButton::Left)
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
        let original_content = match Clipboard::new() {
            Ok(clipboard) => clipboard.read().ok(), // Ignore read errors, proceed without restore maybe? Or error out? Let's ignore for now.
            Err(_) => None, // Ignore errors getting clipboard instance
        };
        Ok(Self { original_content })
    }
}

impl Drop for ClipboardGuard {
    fn drop(&mut self) {
        if let Some(content) = &self.original_content {
            match Clipboard::new() {
                Ok(mut clipboard) => {
                    if let Err(e) = clipboard.write(content.clone()) {
                        warn!("Failed to restore clipboard content: {:?}", e);
                    } else {
                        debug!("Successfully restored clipboard content.");
                    }
                },
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
        Err(e) => {
            warn!("Focus failed before typing, attempting anyway: {:?}", e);
            // Attempting to click as a fallback focus mechanism
            if let Err(click_err) = click_auto(element) {
                 warn!("Fallback click also failed: {:?}", click_err);
                 // Decide if we should proceed or error out here.
                 // Let's proceed cautiously, AXValue might still work if focus is weird.
            } else {
                 // Add a small delay after fallback click before trying to type
                 thread::sleep(Duration::from_millis(100));
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
        let result = AXUIElementSetAttributeValue(element_ref as AXUIElementRef, attr_str_ref as CFStringRef, value_ref);

        if result == 0 {
            debug!("Successfully set text value via AXValue");
            return Ok(());
        } else {
            let error = accessibility::Error::from(accessibility::Error::Ax(result));
            debug!("Failed to set text via AXValue: {:?}. Falling back to clipboard paste.", error);
        }
    }

    // --- Fallback to Clipboard Paste ---
    debug!("Attempting clipboard paste for text: '{}'", text);
    let _guard = ClipboardGuard::new()?; // Restore clipboard automatically on scope exit

    // Set clipboard
    match Clipboard::new() {
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
    let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
        .map_err(|_| AutomationError::PlatformError("Failed to create event source for paste".to_string()))?;

    let key_code_v = KEY_V; // Assuming KEY_V is defined in constants
    let cmd_flag = MODIFIER_COMMAND; // Assuming MODIFIER_COMMAND is defined

    // Press Cmd+V
    let key_down = CGEvent::new_keyboard_event(source.clone(), key_code_v, true)
        .map_err(|_| AutomationError::PlatformError("Failed to create key down event for paste".to_string()))?;
    key_down.set_flags(cmd_flag);
    key_down.post(CGEventTapLocation::HID);
    thread::sleep(Duration::from_millis(50));

    // Release Cmd+V
    let key_up = CGEvent::new_keyboard_event(source, key_code_v, false)
        .map_err(|_| AutomationError::PlatformError("Failed to create key up event for paste".to_string()))?;
    key_up.set_flags(cmd_flag);
    key_up.post(CGEventTapLocation::HID);
    thread::sleep(Duration::from_millis(50));

    debug!("Successfully simulated Cmd+V paste.");

    // Clipboard restored by ClipboardGuard automatically
    Ok(())
}

/// Types text globally using clipboard paste (Cmd+V).
///
/// This function simulates key presses for Cmd+V after setting the clipboard.
/// It does not require focusing a specific UI element beforehand.
pub(crate) fn type_text_global(text: &str) -> Result<(), AutomationError> {
    debug!("Typing text globally via clipboard paste: {}", text);

    let _guard = ClipboardGuard::new()?; // Restore clipboard automatically

    // Set clipboard
    match Clipboard::new() {
        Ok(mut clipboard) => {
             if let Err(e) = clipboard.write(text.to_string()) {
                return Err(AutomationError::PlatformError(format!(
                    "Failed to write to clipboard before global paste: {:?}",
                    e
                )));
            }
             debug!("Successfully set clipboard content for global paste.");
        },
        Err(e) => return Err(AutomationError::PlatformError(format!(
            "Failed to access clipboard before global paste: {:?}",
            e
        )))
    }

    // Give clipboard time to process
    thread::sleep(Duration::from_millis(100));

    // Simulate Cmd+V
    let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState).map_err(|_| {
        AutomationError::PlatformError("Failed to create event source for global paste".to_string())
    })?;

    let key_code_v = KEY_V;
    let cmd_flag = MODIFIER_COMMAND;

    // Press Cmd+V
    let key_down = CGEvent::new_keyboard_event(source.clone(), key_code_v, true)
        .map_err(|_| AutomationError::PlatformError("Failed to create key down event for global paste".to_string()))?;
    key_down.set_flags(cmd_flag);
    key_down.post(CGEventTapLocation::HID);
    thread::sleep(Duration::from_millis(50));

    // Release Cmd+V
    let key_up = CGEvent::new_keyboard_event(source, key_code_v, false)
        .map_err(|_| AutomationError::PlatformError("Failed to create key up event for global paste".to_string()))?;
    key_up.set_flags(cmd_flag);
    key_up.post(CGEventTapLocation::HID);
    thread::sleep(Duration::from_millis(50));

    debug!("Successfully simulated global Cmd+V paste.");

    // Clipboard restored by ClipboardGuard automatically
    Ok(())
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
        let c = key_lower.chars().next().unwrap();

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
                _ => return Err(AutomationError::InvalidArgument(format!("Unsupported character: {}", c))),
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
                _ => return Err(AutomationError::InvalidArgument(format!("Unsupported digit: {}", c))),
            };
            return Ok(vk);
        }
    }

    // If we get here, the key wasn't recognized
    Err(AutomationError::InvalidArgument(format!("Unknown key: {}", key)))
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
    let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
        .map_err(|_| AutomationError::PlatformError("Failed to create event source".to_string()))?;

    let key_down = CGEvent::new_keyboard_event(source.clone(), key_code as CGKeyCode, true)
        .map_err(|_| {
            AutomationError::PlatformError("Failed to create key down event".to_string())
        })?;
    if !flags.is_empty() {
        key_down.set_flags(flags);
    }
    key_down.post(CGEventTapLocation::HID);

    std::thread::sleep(std::time::Duration::from_millis(50));

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
        let result = AXUIElementSetAttributeValue(element_ref as AXUIElementRef, attr_str_ref as CFStringRef, value_ref);
        if result != 0 {
            let error = accessibility::Error::from(accessibility::Error::Ax(result));
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

    let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
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

    // First create a move event to position mouse
    let move_event = CGEvent::new_mouse_event(
        source.clone(), // Clone here to avoid move
        CGEventType::MouseMoved,
        CGPoint::new(center_x, center_y),
        CGMouseButton::Left
    ).map_err(|_| AutomationError::PlatformError("Failed to create mouse move event".to_string()))?;

    move_event.post(CGEventTapLocation::HID);
    std::thread::sleep(std::time::Duration::from_millis(50));

    // Create a scroll event manually since new_scroll_wheel_event is not available
    let scroll_event = CGEvent::new(source.clone()) // Clone source again here
        .map_err(|_| AutomationError::PlatformError("Failed to create scroll event".to_string()))?;

    // Constants for scroll wheel event
    const SCROLL_WHEEL_EVENT_TYPE: u32 = 22; // kCGEventScrollWheel
    const SCROLL_WHEEL_EVENT_DELTA_AXIS_1: u32 = 11; // Vertical scroll delta
    const SCROLL_WHEEL_EVENT_DELTA_AXIS_2: u32 = 10; // Horizontal scroll delta
    const SCROLL_WHEEL_EVENT_LINE_SCROLL: i64 = 1 << 0; // Line scroll mode

    // Set event type to scroll wheel - use transmute instead of from_type_id
    scroll_event.set_type(unsafe { std::mem::transmute(SCROLL_WHEEL_EVENT_TYPE) });

    // Set the line scroll bit
    scroll_event.set_integer_value_field(120, SCROLL_WHEEL_EVENT_LINE_SCROLL);

    // Set the delta values
    scroll_event.set_integer_value_field(SCROLL_WHEEL_EVENT_DELTA_AXIS_1, scroll_y as i64);
    scroll_event.set_integer_value_field(SCROLL_WHEEL_EVENT_DELTA_AXIS_2, scroll_x as i64);

    scroll_event.post(CGEventTapLocation::HID);

    Ok(())
}

pub(crate) fn select_text(element: &MacOSUIElement) -> Result<(), AutomationError> {
    debug!("Attempting to select all text in element");

    // First, focus the element to ensure it's active
    element.focus()?;
    thread::sleep(Duration::from_millis(100)); // Give time for focus to register

    // Try using AXSelectText action if available
    let select_attr = AXAttribute::new(&CFString::new("AXSelectText"));
    match element.element.0.perform_action(&select_attr.as_CFString()) {
        Ok(_) => {
            debug!("Successfully selected text using AXSelectText action");
            return Ok(());
        }
        Err(e) => {
            debug!("AXSelectText action failed: {:?}, trying alternative methods", e);
        }
    }

    // If AXSelectText failed, try using Cmd+A shortcut
    debug!("Attempting to select all text with Command+A shortcut");

    // Create event source
    let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
        .map_err(|_| AutomationError::PlatformError("Failed to create event source for select_text".to_string()))?;

    // Key codes for Command+A
    let cmd_down = CGEvent::new_keyboard_event(source.clone(), KEYCODE_CMD, true)
        .map_err(|_| AutomationError::PlatformError("Failed to create command key down event".to_string()))?;
    cmd_down.post(CGEventTapLocation::HID);
    thread::sleep(Duration::from_millis(50));

    let a_down = CGEvent::new_keyboard_event(source.clone(), KEYCODE_A, true)
        .map_err(|_| AutomationError::PlatformError("Failed to create 'A' key down event".to_string()))?;
    a_down.post(CGEventTapLocation::HID);
    thread::sleep(Duration::from_millis(50));

    let a_up = CGEvent::new_keyboard_event(source.clone(), KEYCODE_A, false)
        .map_err(|_| AutomationError::PlatformError("Failed to create 'A' key up event".to_string()))?;
    a_up.post(CGEventTapLocation::HID);
    thread::sleep(Duration::from_millis(50));

    let cmd_up = CGEvent::new_keyboard_event(source, KEYCODE_CMD, false)
        .map_err(|_| AutomationError::PlatformError("Failed to create command key up event".to_string()))?;
    cmd_up.post(CGEventTapLocation::HID);

    debug!("Successfully simulated Command+A for text selection");
    Ok(())
}

/// Gets the current text content from the system clipboard.
pub(crate) fn get_clipboard_contents() -> Result<String, AutomationError> {
    match Clipboard::new() {
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
    match Clipboard::new() {
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
pub(crate) fn hold_key(key_code: CGKeyCode, flags: CGEventFlags, duration_ms: Option<u64>) -> Result<(), AutomationError> {
    debug!("Holding key code {} with flags {:?} for {:?}ms", key_code, flags, duration_ms);

    // Create an event source for user input
    let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
        .map_err(|_| AutomationError::PlatformError("Failed to create event source for key hold".to_string()))?;

    // Create the key down event
    let key_down = CGEvent::new_keyboard_event(source.clone(), key_code, true)
        .map_err(|_| AutomationError::PlatformError("Failed to create key down event".to_string()))?;

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
pub(crate) fn release_key(key_code: CGKeyCode, flags: CGEventFlags) -> Result<(), AutomationError> {
    debug!("Releasing key: code={}, flags={:?}", key_code, flags);
    let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState).map_err(|_|
        AutomationError::PlatformError("Failed to create event source for release_key".to_string())
    )?;

    // Create keyboard event for key up
    let key_up = CGEvent::new_keyboard_event(source, key_code, false)
        .map_err(|_| AutomationError::PlatformError("Failed to create key up event for release_key".to_string()))?;

    // Set the flags for the key up event (usually should have the modifier flag being released)
    key_up.set_flags(flags);
    key_up.post(CGEventTapLocation::HID);

    Ok(())
}

pub(crate) fn press_key_with_modifier(key_code: CGKeyCode, modifier_flags: CGEventFlags) -> Result<(), AutomationError> {
    debug!("Pressing key: {} with modifiers: {:?}", key_code, modifier_flags);
    let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
        .map_err(|_| AutomationError::PlatformError("Failed to create event source".to_string()))?;

    let event_down = CGEvent::new_keyboard_event(source.clone(), key_code, true)
        .map_err(|_| AutomationError::PlatformError("Failed to create key down event".to_string()))?;
    event_down.set_flags(modifier_flags);
    event_down.post(CGEventTapLocation::HID);
    thread::sleep(Duration::from_millis(50));

    let event_up = CGEvent::new_keyboard_event(source, key_code, false)
        .map_err(|_| AutomationError::PlatformError("Failed to create key up event".to_string()))?;
    event_up.set_flags(modifier_flags);
    event_up.post(CGEventTapLocation::HID);
    thread::sleep(Duration::from_millis(50));

    debug!("Key press simulated for key code: {}", key_code);
    Ok(())
}

// --- Old/Internal key sequence logic (if needed for reference, keep private) ---
fn press_key_sequence(keys: &[(CGKeyCode, Option<CGEventFlags>)]) -> Result<(), AutomationError> {
    // Placeholder implementation or keep the original logic if needed internally
    debug!("Internal press_key_sequence called (currently placeholder)");
    // Example: iterate through keys and simulate presses
    for (key_code, modifier_flags_opt) in keys {
        let modifier_flags = modifier_flags_opt.unwrap_or_else(CGEventFlags::empty);
        // Simulate key down
        // Simulate key up
        debug!("Simulating press for key: {} with flags: {:?}", key_code, modifier_flags);
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
                "Failed to get position attribute: {:?}", e
            )));
        }
    };

    // Get size
    let size_attr = AXAttribute::new(&CFString::new("AXSize"));
    let size_cf = match element.attribute(&size_attr) {
        Ok(cf) => cf,
        Err(e) => {
            return Err(AutomationError::PlatformError(format!(
                "Failed to get size attribute: {:?}", e
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
        for part in position_str.split([',', ' ', ':', ')', '(']).collect::<Vec<&str>>() {
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
        for part in size_str.split([',', ' ', ':', ')', '(']).collect::<Vec<&str>>() {
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
        return Err(AutomationError::PlatformError(
            format!("Invalid element dimensions: {}x{}", width, height)
        ));
    }

    Ok((x, y, width, height))
}

pub fn scroll_element(element: &AXUIElement, direction: &str, amount: f64) -> Result<(), AutomationError> {
    let element_frame = match get_element_frame(element) {
        Ok(frame) => frame,
        Err(e) => {
            return Err(AutomationError::PlatformError(format!(
                "Failed to get element frame for scrolling: {}", e
            )))
        }
    };

    // Calculate center of element
    let center_x = element_frame.0 + element_frame.2 / 2.0;
    let center_y = element_frame.1 + element_frame.3 / 2.0;

    // Create event source
    let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState).map_err(|_| {
        AutomationError::PlatformError("Failed to create event source for scrolling".to_string())
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
        CGMouseButton::Left
    ).map_err(|_| AutomationError::PlatformError("Failed to create mouse move event".to_string()))?;

    move_event.post(CGEventTapLocation::HID);
    std::thread::sleep(std::time::Duration::from_millis(50));

    // Create a scroll event manually since new_scroll_wheel_event is not available
    let scroll_event = CGEvent::new(source.clone()).map_err(|_| {
        AutomationError::PlatformError("Failed to create scroll event".to_string())
    })?;

    // Constants for scroll wheel event
    const SCROLL_WHEEL_EVENT_TYPE: u32 = 22; // kCGEventScrollWheel
    const SCROLL_WHEEL_EVENT_DELTA_AXIS_1: u32 = 11; // Vertical scroll delta
    const SCROLL_WHEEL_EVENT_DELTA_AXIS_2: u32 = 10; // Horizontal scroll delta
    const SCROLL_WHEEL_EVENT_LINE_SCROLL: i64 = 1 << 0; // Line scroll mode

    // Set event type to scroll wheel using transmute for safety
    scroll_event.set_type(unsafe { std::mem::transmute(SCROLL_WHEEL_EVENT_TYPE) });

    // Set the line scroll bit
    scroll_event.set_integer_value_field(120, SCROLL_WHEEL_EVENT_LINE_SCROLL);

    // Set the delta values
    scroll_event.set_integer_value_field(SCROLL_WHEEL_EVENT_DELTA_AXIS_1, scroll_y as i64);
    scroll_event.set_integer_value_field(SCROLL_WHEEL_EVENT_DELTA_AXIS_2, scroll_x as i64);

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
    modifiers: Option<CGEventFlags>
) -> Result<(), AutomationError> {
    debug!("scrolling {} by {} at ({}, {}) with modifiers: {:?}", direction, amount, x, y, modifiers);

    // Create an event source for user input
    let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
        .map_err(|_| AutomationError::PlatformError("Failed to create event source for scrolling".to_string()))?;

    // Move the cursor to the target position first
    let point = CGPoint::new(x, y);
    let mouse_move = CGEvent::new_mouse_event(
        source.clone(),
        CGEventType::MouseMoved,
        point,
        CGMouseButton::Left,
    )
    .map_err(|_| {
        AutomationError::PlatformError("Failed to create mouse move event for scrolling".to_string())
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
        "up" => (0, -scroll_units), // Negative for up
        "down" => (0, scroll_units), // Positive for down
        "left" => (scroll_units, 0), // Positive for left (wheel axis)
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
    // Constants for scroll wheel event types
    const SCROLL_WHEEL_EVENT_TYPE: u32 = 22; // kCGEventScrollWheel
    const SCROLL_WHEEL_EVENT_DELTA_AXIS_1: u32 = 11; // Vertical scroll delta (main scroll axis)
    const SCROLL_WHEEL_EVENT_DELTA_AXIS_2: u32 = 10; // Horizontal scroll delta
    const SCROLL_WHEEL_EVENT_LINE_SCROLL: i64 = 1 << 0; // Line scroll (as opposed to pixel scroll)

    // Set event type using CGEventType transmute
    scroll_event.set_type(unsafe { std::mem::transmute(SCROLL_WHEEL_EVENT_TYPE) });

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
pub(crate) fn post_mouse_event(
    event_type: CGEventType,
    location: CGPoint,
    button: CGMouseButton,
    modifiers: Option<CGEventFlags>,
    click_state: Option<i64>
) -> Result<(), AutomationError> {
    let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState).map_err(|_| {
        AutomationError::PlatformError("Failed to create event source".to_string())
    })?;

    let event = CGEvent::new_mouse_event(source, event_type, location, button).map_err(|_| {
        AutomationError::PlatformError("Failed to create mouse event".to_string())
    })?;

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
