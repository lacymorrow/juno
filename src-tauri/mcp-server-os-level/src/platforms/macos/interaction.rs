use super::actions::ClickMethodSelection;
use super::constants::*;
use super::element::MacOSUIElement;
use super::wrappers::ThreadSafeAXUIElement;
use crate::element::UIElementImpl; // Needed for app_attributes in click_auto
use crate::{AutomationError, ClickResult};
use accessibility::{AXAttribute, AXUIElement};
use accessibility_sys::{AXUIElementSetAttributeValue, AXUIElementRef};
use core_foundation::base::{TCFType, CFTypeRef};
use core_foundation::string::{CFString, CFStringRef};
use core_graphics::event::{
    CGEvent, CGEventFlags, CGEventTapLocation, CGEventType, CGKeyCode, CGMouseButton,
};
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
use core_graphics::geometry::CGPoint;
use clipboard_macos::Clipboard; // Import clipboard_macos
// use pasteboard_macos::Pasteboard; // Ensure this import is removed or commented out
use std::collections::HashMap;
use tracing::{debug, warn};
use std::thread;
use std::time::Duration;

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
    method: ClickMethodSelection,
) -> Result<ClickResult, AutomationError> {
    match method {
        ClickMethodSelection::Auto => click_auto(element),
        ClickMethodSelection::AXPress => click_press(element),
        ClickMethodSelection::AXClick => click_accessibility_click(element),
        ClickMethodSelection::MouseSimulation => click_mouse_simulation(element),
    }
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

fn get_key_code(key: &str) -> Result<u16, AutomationError> {
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
    ]
    .iter()
    .cloned()
    .collect();
    key_map
        .get(key.to_lowercase().as_str())
        .copied()
        .ok_or_else(|| AutomationError::InvalidArgument(format!("Unknown key: {}", key)))
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

    let scroll_event = CGEvent::new_scroll_event(source, 0, 1, scroll_y, scroll_x, 0)
        .map_err(|_| AutomationError::PlatformError("Failed to create scroll event".to_string()))?;

    scroll_event.post(CGEventTapLocation::HID);
    debug!(
        "scrolled {} by {} at element center ({}, {})",
        direction, amount, center_x, center_y
    );
    Ok(())
}

pub(crate) fn select_text(_element: &MacOSUIElement) -> Result<(), AutomationError> {
    // TODO: Implement select_text using AXSelectText action or Cmd+A simulation
    warn!("select_text function is not yet implemented for macOS");
    Ok(())
}

/// Gets the current text content from the system clipboard.
pub(crate) fn get_clipboard_contents() -> Result<String, AutomationError> {
    // clipboard_macos likely uses a static method or a context struct
    // Based on docs.rs, it seems to use a Clipboard struct
    match Clipboard::new() {
        Ok(clipboard) => match clipboard.read() {
            Ok(content) => {
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
pub(crate) fn hold_key(key_code: CGKeyCode, flags: CGEventFlags) -> Result<(), AutomationError> {
    debug!("Holding key: code={}, flags={:?}", key_code, flags);
    let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState).map_err(|_|
        AutomationError::PlatformError("Failed to create event source for hold_key".to_string())
    )?;

    // Create keyboard event for key down
    let key_down = CGEvent::new_keyboard_event(source.clone(), key_code, true)
        .map_err(|_| AutomationError::PlatformError("Failed to create key down event for hold_key".to_string()))?;

    // Set the appropriate flags for the modifier key itself
    key_down.set_flags(flags);
    key_down.post(CGEventTapLocation::HID);

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
