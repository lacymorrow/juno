use super::element::MacOSUIElement;
use super::permissions::check_accessibility_permissions;
use super::utils::{
    element_contains_text,
    get_running_application_pids, map_generic_role_to_macos_roles,
};
use super::wrappers::ThreadSafeAXUIElement;
use super::display::{adjust_coordinates_for_display, get_displays_debug_info};
use crate::platforms::tree_search::{
    ElementFinderWithWindows, ElementsCollectorWithWindows, TreeWalkerWithWindows,
};
use crate::platforms::AccessibilityEngine;
use crate::{AutomationError, Selector, UIElement};
use accessibility::{AXAttribute, AXUIElementAttributes, Error as AXError};
use accessibility_sys::{kAXFocusedUIElementAttribute, AXUIElementRef, kAXFrontmostAttribute, AXUIElementGetTypeID, kAXErrorNoValue};
use anyhow::Result;
use core_foundation::base::{TCFType, CFTypeID, CFGetTypeID, CFType};
use core_foundation::boolean::CFBoolean;
use core_foundation::number::CFNumber;
use core_foundation::string::CFString;
use core_graphics::display::CGPoint;
use core_graphics::event::{CGEvent, CGEventTapLocation, CGEventType, CGMouseButton, CGEventFlags};
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
use libc;
use std::collections::BTreeMap;
use tracing::{debug, trace, warn};
// Re-add interaction module for non-mouse actions
use crate::platforms::macos::interaction::{self};
// Import keycode mapping function
use crate::platforms::macos::constants::key_name_to_keycode;

use serde_json::{json, Value as JsonValue};
use crate::element::ElementTreeNode;

pub struct MacOSEngine {
    pub(crate) system_wide: ThreadSafeAXUIElement,
    pub(crate) use_background_apps: bool,
    pub(crate) activate_app: bool,
}

impl MacOSEngine {
    pub fn new(use_background_apps: bool, activate_app: bool) -> Result<Self, AutomationError> {
        check_accessibility_permissions(false)?;

        Ok(Self {
            system_wide: ThreadSafeAXUIElement::system_wide(),
            use_background_apps,
            activate_app,
        })
    }

    pub fn new_with_auto_redirect(use_background_apps: bool, activate_app: bool, auto_open_settings: bool) -> Result<Self, AutomationError> {
        use super::permissions::check_accessibility_permissions_with_auto_redirect;
        check_accessibility_permissions_with_auto_redirect(false, auto_open_settings)?;

        Ok(Self {
            system_wide: ThreadSafeAXUIElement::system_wide(),
            use_background_apps,
            activate_app,
        })
    }

    pub(crate) fn wrap_element(
        &self,
        ax_element: ThreadSafeAXUIElement,
        role: Option<String>,
        label: Option<String>,
        description: Option<String>,
        value: Option<String>,
    ) -> UIElement {
        UIElement::new(Box::new(MacOSUIElement {
            element: ax_element,
            use_background_apps: self.use_background_apps,
            activate_app: self.activate_app,
            cached_role: role.unwrap_or_default(),
            cached_label: label,
            cached_description: description,
            cached_value: value,
        }))
    }

    #[allow(clippy::unexpected_cfg_condition)]
    pub(crate) fn refresh_accessibility_tree(
        &self,
        app_name: Option<&str>,
    ) -> Result<(), AutomationError> {
        if !self.activate_app {
            return Ok(());
        }

        debug!("Refreshing accessibility tree");

        if let Some(name) = app_name {
            unsafe {
                use objc::{class, msg_send, sel, sel_impl};

                let workspace_class = class!(NSWorkspace);
                let shared_workspace: *mut objc::runtime::Object =
                    msg_send![workspace_class, sharedWorkspace];
                let apps: *mut objc::runtime::Object =
                    msg_send![shared_workspace, runningApplications];
                let count: usize = msg_send![apps, count];

                for i in 0..count {
                    let app: *mut objc::runtime::Object = msg_send![apps, objectAtIndex:i];
                    let app_name_obj: *mut objc::runtime::Object = msg_send![app, localizedName];

                    if !app_name_obj.is_null() {
                        let app_name_str: &str = {
                            let nsstring = app_name_obj as *const objc::runtime::Object;
                            let bytes: *const std::os::raw::c_char =
                                msg_send![nsstring, UTF8String];
                            let len: usize = msg_send![nsstring, lengthOfBytesUsingEncoding:4];
                            let bytes_slice = std::slice::from_raw_parts(bytes as *const u8, len);
                            std::str::from_utf8_unchecked(bytes_slice)
                        };

                        if app_name_str.to_lowercase() == name.to_lowercase() {
                            let _: () = msg_send![app, activateWithOptions:1];
                            debug!("Activated application: {}", name);

                            std::thread::sleep(std::time::Duration::from_millis(100));
                            break;
                        }
                    }
                }
            }
        }

        let _ = self.system_wide.0.attribute_names();

        Ok(())
    }

    // This is the primary scroll implementation using interaction module
    fn scroll_at_position(
        &self,
        x: f64,
        y: f64,
        direction: &str,
        amount: f64,
    ) -> Result<(), AutomationError> {
        debug!(
            "scrolling {} by {} at position ({}, {})",
            direction, amount, x, y
        );

        // Use the improved implementation in the interaction module that supports all directions
        interaction::scroll_with_modifiers(x, y, direction, amount, None)
    }

    // Add another method for scrolling with modifiers
    pub fn scroll_at_position_with_modifiers(
        &self,
        x: f64,
        y: f64,
        direction: &str,
        amount: f64,
        modifiers: Option<&str>,
    ) -> Result<(), AutomationError> {
        debug!(
            "scrolling {} by {} at position ({}, {}) with modifiers: {:?}",
            direction, amount, x, y, modifiers
        );

        let parsed_modifiers = match modifiers {
            Some(mods) => {
                let mut flags = CGEventFlags::empty();

                for modifier in mods.split('+') {
                    let modifier_flag = match modifier.trim().to_lowercase().as_str() {
                        "command" | "cmd" => CGEventFlags::CGEventFlagCommand,
                        "shift" => CGEventFlags::CGEventFlagShift,
                        "option" | "alt" => CGEventFlags::CGEventFlagAlternate,
                        "control" | "ctrl" => CGEventFlags::CGEventFlagControl,
                        "fn" => CGEventFlags::CGEventFlagSecondaryFn,
                        _ => {
                            return Err(AutomationError::InvalidArgument(format!(
                                "Unknown modifier: {}. Use standard modifier names.",
                                modifier
                            )))
                        }
                    };
                    flags |= modifier_flag;
                }

                Some(flags)
            },
            None => None,
        };

        // Use the improved implementation in the interaction module that supports modifiers
        interaction::scroll_with_modifiers(x, y, direction, amount, parsed_modifiers)
    }

    fn scroll_at_current_position(
        &self,
        direction: &str,
        amount: f64,
    ) -> Result<(), AutomationError> {
        debug!("getting current mouse location using CGEvent::new with a valid event source");

        let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState).map_err(|_| {
            AutomationError::PlatformError("failed to create event source".to_string())
        })?;
        debug!("created event source successfully");

        let event = CGEvent::new(source).map_err(|_| {
            AutomationError::PlatformError(
                "failed to create event for obtaining current mouse position".to_string(),
            )
        })?;
        debug!("got current event; mouse position: {:?}", event.location());

        let current_pos = event.location();

        self.scroll_at_position(current_pos.x, current_pos.y, direction, amount)
    }

    // Method to get the UI tree, starting from the focused application or a specified one
    pub fn get_ui_tree(&self, app_name: Option<&str>) -> Result<JsonValue, AutomationError> {
        debug!("Getting UI tree for app: {:?}", app_name);
        let root_element = match app_name {
            Some(name) => self.get_application_by_name(name)?,
            None => self.get_focused_element()?, // Get focused element as root if no app specified
        };

        // Use the internal recursive function to build the tree
        self.build_element_tree(&root_element, 0)
    }

    // Recursive helper to build the JSON representation of the UI tree
    fn build_element_tree(&self, element: &UIElement, depth: usize) -> Result<JsonValue, AutomationError> {
        const MAX_DEPTH: usize = 10; // Limit recursion depth
        if depth > MAX_DEPTH {
            return Ok(json!({ "error": "Max recursion depth reached" }));
        }

        let attributes = element.attributes();
        let mut children_json = Vec::new();

        if let Ok(children) = element.children() {
            for child in children {
                match self.build_element_tree(&child, depth + 1) {
                    Ok(child_json) => children_json.push(child_json),
                    Err(e) => {
                        warn!("Error building child tree: {}", e);
                        children_json.push(json!({ "error": format!("Failed to get child attributes: {}", e) }));
                    }
                }
            }
        }

        Ok(json!({
            "attributes": attributes,
            "children": children_json
        }))
    }
}

// Standalone helper function to check if a raw AXUIElement matches the Attributes selector
fn check_ax_element_attributes_match(
    e: &accessibility::AXUIElement,
    attrs: &BTreeMap<String, String>,
) -> bool {
    for (key, value) in attrs.iter() {
        let key_lower = key.to_lowercase();
        let value_lower = value.to_lowercase();

        let match_result = match key_lower.as_str() {
            "focused" => {
                let expected_focus = value_lower == "true";
                let focus_attr = AXAttribute::new(&CFString::new("AXFocused"));
                e.attribute(&focus_attr)
                    .ok()
                    .and_then(|v| v.downcast_into::<CFBoolean>())
                    .map_or(false, |b| b == expected_focus.into())
            }
            "role" => {
                let role_attr = AXAttribute::new(&CFString::new("AXRole"));
                e.attribute(&role_attr)
                    .ok()
                    .and_then(|v| v.downcast_into::<CFString>())
                    .map_or(false, |s| s.to_string().to_lowercase() == value_lower)
            }
            "title" | "label" => {
                // Check AXTitle first
                let title_attr = AXAttribute::new(&CFString::new("AXTitle"));
                let title_match = e
                    .attribute(&title_attr)
                    .ok()
                    .and_then(|v| v.downcast_into::<CFString>())
                    .map_or(false, |s| !s.to_string().is_empty() && s.to_string().to_lowercase() == value_lower);

                if title_match {
                    true
                } else {
                    // Fallback to AXLabel
                    let label_attr = AXAttribute::new(&CFString::new("AXLabel"));
                    e.attribute(&label_attr)
                        .ok()
                        .and_then(|v| v.downcast_into::<CFString>())
                        .map_or(false, |s| s.to_string().to_lowercase() == value_lower)
                }
            }
            "description" => {
                let desc_attr = AXAttribute::new(&CFString::new("AXDescription"));
                e.attribute(&desc_attr)
                    .ok()
                    .and_then(|v| v.downcast_into::<CFString>())
                    .map_or(false, |s| s.to_string().to_lowercase() == value_lower)
            }
            "value" => {
                 let value_attr = AXAttribute::new(&CFString::new("AXValue"));
                 e.attribute(&value_attr)
                     .ok()
                     .map_or(false, |v| {
                         if let Some(s) = v.clone().downcast_into::<CFString>() {
                             s.to_string().to_lowercase() == value_lower
                         } else if let Some(n) = v.clone().downcast_into::<CFNumber>() {
                             // Handle numeric comparison if necessary, for now just stringify
                             if let Some(int_val) = n.to_i64() {
                                 int_val.to_string() == value_lower
                             } else if let Some(float_val) = n.to_f64() {
                                 float_val.to_string() == value_lower
                             } else {
                                 false
                             }
                         } else if let Some(b) = v.clone().downcast_into::<CFBoolean>(){
                            (b == true.into()).to_string() == value_lower
                         } else {
                             false
                         }
                     })
             }
            "id" => {
                 // Check AXIdentifier first
                 let axid_attr = AXAttribute::new(&CFString::new("AXIdentifier"));
                 e.attribute(&axid_attr)
                     .ok()
                     .and_then(|v| v.downcast_into::<CFString>())
                     .map_or(false, |s| s.to_string().to_lowercase() == value_lower)
                // Note: We don't check the generated ID here for efficiency
            }
             // Add more attribute checks here if needed (e.g., AXEnabled)
             _ => {
                 warn!("Unsupported attribute key in selector: {}", key);
                 false // Treat unsupported keys as non-matching for now
             }
        };

        if !match_result {
            // Reduced logging inside predicate for performance
            // trace!("Attribute mismatch for key '{}': expected '{}'", key, value);
            return false; // If any attribute doesn't match, the element doesn't match
        }
    }
    true // All specified attributes matched
}

// Helper function to post mouse events
fn post_mouse_event(
    event_type: CGEventType,
    point: CGPoint,
    button: CGMouseButton,
    click_count: Option<i64>, // For clicks/double/triple
    modifiers: Option<CGEventFlags>, // For modifier keys during click
) -> Result<(), AutomationError> {
    // Multi-monitor support: adjust coordinates for the appropriate display
    let (adjusted_x, adjusted_y) = adjust_coordinates_for_display(point.x, point.y, None)?;
    let adjusted_point = CGPoint::new(adjusted_x, adjusted_y);

    // Log display information for debugging multi-monitor issues
    if point.x != adjusted_x || point.y != adjusted_y {
        debug!("Multi-monitor coordinate adjustment: ({}, {}) → ({}, {})",
            point.x, point.y, adjusted_x, adjusted_y);
        trace!("Display info: {}", get_displays_debug_info());
    }

    let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
        .map_err(|_| AutomationError::PlatformError("Failed to create event source".to_string()))?;

    // Use adjusted point for multi-monitor support
    let event = CGEvent::new_mouse_event(source, event_type, adjusted_point, button)
        .map_err(|e| AutomationError::PlatformError(format!("Failed to create mouse event: {:?}", e)))?;

    if let Some(count) = click_count {
        // Use the correct constant kCGMouseEventClickState - try fully qualified path
        // event.set_integer_value_field(core_graphics::event::CGEventField::MouseEventClickState, count);
        // Fallback to raw field ID 93 if constant lookup fails
        const MOUSE_EVENT_CLICK_STATE_FIELD: u32 = 93;
        event.set_integer_value_field(MOUSE_EVENT_CLICK_STATE_FIELD, count);
    }

    if let Some(flags) = modifiers {
        event.set_flags(flags);
    }


    event.post(CGEventTapLocation::HID);
    // Add a small delay after posting, can improve reliability sometimes
    std::thread::sleep(std::time::Duration::from_millis(20));
    Ok(())
}

// Helper function to parse modifier string into CGEventFlags
fn parse_modifiers(modifier_str: Option<&str>) -> CGEventFlags {
    let mut flags = CGEventFlags::empty();
    if let Some(s) = modifier_str {
        let lower = s.to_lowercase();
        if lower.contains("cmd") || lower.contains("command") || lower.contains("meta") {
            // Use correct flag constants (e.g., CGEventFlags::CGEventFlagCommand)
            flags |= CGEventFlags::CGEventFlagCommand;
        }
        if lower.contains("shift") {
            flags |= CGEventFlags::CGEventFlagShift;
        }
        if lower.contains("option") || lower.contains("alt") {
            flags |= CGEventFlags::CGEventFlagAlternate;
        }
        if lower.contains("ctrl") || lower.contains("control") {
            flags |= CGEventFlags::CGEventFlagControl;
        }
        // Add other modifiers like CAPSLOCK if needed (e.g., CGEventFlags::CGEventFlagAlphaShift)
    }
    flags
}

// Helper function to post keyboard events
#[allow(dead_code)] // Used through computer_use_ai_sdk interface
fn post_keyboard_event(
    key_code: u16,
    flags: CGEventFlags,
    is_down: bool,
) -> Result<(), AutomationError> {
    let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
        .map_err(|_| AutomationError::PlatformError("Failed to create event source".to_string()))?;

    let event = CGEvent::new_keyboard_event(source, key_code, is_down)
        .map_err(|e| AutomationError::PlatformError(format!("Failed to create keyboard event: {:?}", e)))?;

    // Set modifier flags AFTER creating the event
    event.set_flags(flags);

    event.post(CGEventTapLocation::HID);
    // Small delay, especially after key down
    std::thread::sleep(std::time::Duration::from_millis(10));
    Ok(())
}

impl AccessibilityEngine for MacOSEngine {
    fn get_applications(&self) -> Result<Vec<UIElement>, AutomationError> {
        // Get running application PIDs using NSWorkspace
        let pids = get_running_application_pids(self.use_background_apps)?;

        debug!("Found {} running applications", pids.len());

        // Create AXUIElements for each application
        let mut app_elements = Vec::new();
        for pid in pids {
            trace!("Creating AXUIElement for application with PID: {}", pid);
            let app_element = ThreadSafeAXUIElement::application(pid);

            app_elements.push(self.wrap_element(app_element, None, None, None, None));
        }

        Ok(app_elements)
    }
    fn get_root_element(&self) -> UIElement {
        self.wrap_element(self.system_wide.clone(), None, None, None, None)
    }

    fn get_focused_element(&self) -> Result<UIElement, AutomationError> {
        tracing::info!("Entering MacOSEngine::get_focused_element (using kAXFrontmostAttribute strategy)");

        // 1. Find the frontmost application by iterating (including accessory apps)
        let pids = get_running_application_pids(true)?; // Include accessory apps initially
        let mut focused_app_element: Option<accessibility::AXUIElement> = None;

        let frontmost_attr_name = CFString::new(kAXFrontmostAttribute);
        let frontmost_attr = accessibility::AXAttribute::<CFType>::new(&frontmost_attr_name);
        // Attribute for checking activation policy manually if needed (though get_pid might be better)
        // let policy_attr = accessibility::AXAttribute::<CFType>::new(&CFString::new("AXActivationPolicy"));

        for pid in pids {
            let app_element = accessibility::AXUIElement::application(pid);

            // Get attribute as CFType, then downcast
            match app_element.attribute(&frontmost_attr) {
                Ok(frontmost_val) => {
                    // Downcast the CFType result to CFBoolean
                    if let Some(is_frontmost) = frontmost_val.downcast_into::<CFBoolean>() {
                        // Compare CFBoolean to true
                        if is_frontmost == true.into() { // Correct comparison
                            debug!("Found frontmost application with PID: {}", pid);
                            // Double check it's not a background-only process before accepting
                             if let Ok(role) = app_element.role(){
                                if role.to_string() == "AXApplication" { // Basic sanity check
                                    focused_app_element = Some(app_element);
                                    break; // Found the focused app
                                }
                            }
                        }
                    } else {
                        trace!("kAXFrontmostAttribute for PID {} was not a CFBoolean", pid);
                    }
                }
                Err(e) => {
                    // Log error but continue checking other apps
                    trace!("Error checking kAXFrontmostAttribute for PID {}: {:?}", pid, e);
                }
            }
        }

        // Ensure we found a focused application
        let focused_app_element = match focused_app_element {
            Some(app) => app,
            None => {
                warn!("Could not find any frontmost application.");
                return Err(AutomationError::ElementNotFound(
                    "No frontmost application found".to_string(),
                ));
            }
        };

        // Log details about the focused application (optional but helpful)
        // ... (logging code can remain the same, using focused_app_element)
        match focused_app_element.title() {
             Ok(title) => debug!("Focused Application Title (found via frontmost): {}", title.to_string()),
             Err(e) => debug!("Error getting focused app title: {:?}", e),
        }

        // 2. Get the focused UI element AS CFType *from the identified frontmost application*
        let focused_ui_attr_name = CFString::new(kAXFocusedUIElementAttribute);
        let focused_ui_attr = accessibility::AXAttribute::<CFType>::new(&focused_ui_attr_name);

        match focused_app_element.attribute(&focused_ui_attr) {
            Ok(focused_element_val) => {
                let focused_element_ref = focused_element_val.as_CFTypeRef();
                if focused_element_ref.is_null() {
                    warn!("kAXFocusedUIElementAttribute returned NULL CFTypeRef.");
                    return Err(AutomationError::PlatformError(
                        "Focused UI element reference from attribute is NULL".to_string(),
                    ));
                }

                // --- Safety Check: Verify CFTypeID before casting ---
                let expected_type_id: CFTypeID = unsafe { AXUIElementGetTypeID() };
                let actual_type_id: CFTypeID = unsafe { CFGetTypeID(focused_element_ref) };
                debug!("Focused Element CFType Check: Expected TypeID: {}, Actual TypeID: {}", expected_type_id, actual_type_id);

                if actual_type_id != expected_type_id {
                    warn!(
                        "Type mismatch for focused element! Expected AXUIElement ({}), got TypeID {}.",
                        expected_type_id, actual_type_id
                    );
                    return Err(AutomationError::PlatformError(format!(
                        "Type mismatch for focused element: expected AXUIElement ({}), got TypeID {}",
                        expected_type_id, actual_type_id
                    )));
                }
                // --- End Safety Check ---

                // Cast CFType to AXUIElement (Now with more confidence)
                let focused_element = unsafe {
                    debug!("Attempting cast from CFTypeRef to AXUIElementRef...");
                    let element_ref = focused_element_ref as *mut libc::c_void as AXUIElementRef;
                    // No null check needed here as we checked focused_element_ref above
                    debug!("Cast successful. Wrapping AXUIElementRef...");
                    let wrapped_element = accessibility::AXUIElement::wrap_under_create_rule(element_ref);
                    debug!("Wrapping successful.");
                    wrapped_element
                };

                // --- Start Fetching Core Attributes Early ---
                let mut fetched_role = String::new();
                let mut fetched_label = None;
                let mut fetched_description = None;
                let mut fetched_value = None;

                let role_attr = AXAttribute::new(&CFString::new("AXRole"));
                if let Ok(role_val) = focused_element.attribute(&role_attr) {
                    if let Some(cf_string) = role_val.downcast_into::<CFString>() {
                        fetched_role = cf_string.to_string();
                    }
                }
                debug!("Pre-fetched Role: {}", fetched_role);

                let title_attr = AXAttribute::new(&CFString::new("AXTitle"));
                if let Ok(title_val) = focused_element.attribute(&title_attr) {
                    if let Some(cf_string) = title_val.downcast_into::<CFString>() {
                        let title_str = cf_string.to_string();
                        if !title_str.is_empty() {
                            fetched_label = Some(title_str);
                        }
                    }
                }
                if fetched_label.is_none() {
                    let label_attr = AXAttribute::new(&CFString::new("AXLabel"));
                    if let Ok(label_val) = focused_element.attribute(&label_attr) {
                        if let Some(cf_string) = label_val.downcast_into::<CFString>() {
                            fetched_label = Some(cf_string.to_string());
                        }
                    }
                }
                debug!("Pre-fetched Label: {:?}", fetched_label);

                let desc_attr = AXAttribute::new(&CFString::new("AXDescription"));
                if let Ok(desc_val) = focused_element.attribute(&desc_attr) {
                    if let Some(cf_string) = desc_val.downcast_into::<CFString>() {
                        fetched_description = Some(cf_string.to_string());
                    }
                }
                debug!("Pre-fetched Description: {:?}", fetched_description);

                let value_attr = AXAttribute::new(&CFString::new("AXValue"));
                if let Ok(value_val) = focused_element.attribute(&value_attr) {
                    if let Some(cf_string) = value_val.clone().downcast_into::<CFString>() {
                        fetched_value = Some(cf_string.to_string());
                    } else if let Some(cf_num) = value_val.clone().downcast_into::<CFNumber>() {
                        if let Some(num) = cf_num.to_i64() {
                            fetched_value = Some(num.to_string());
                        } else if let Some(num) = cf_num.to_f64() {
                            fetched_value = Some(num.to_string());
                        }
                    }
                }
                debug!("Pre-fetched Value: {:?}", fetched_value);
                // --- End Fetching Core Attributes Early ---

                 if !fetched_role.is_empty()
                    || fetched_label.is_some()
                    || fetched_description.is_some()
                {
                    debug!("Returning specific focused element within the frontmost app.");
                    Ok(self.wrap_element(
                        ThreadSafeAXUIElement::new(focused_element),
                        Some(fetched_role),
                        fetched_label,
                        fetched_description,
                        fetched_value,
                    ))
                } else {
                    debug!("Specific focused element seems invalid, returning frontmost app element instead.");
                    Ok(self.wrap_element(ThreadSafeAXUIElement::new(focused_app_element), None, None, None, None))
                }
            }
            Err(e) => {
                // Check if the error is kAXErrorNoValue (-25212)
                if let AXError::Ax(err_num) = e {
                    if err_num == kAXErrorNoValue {
                        warn!(
                            "Frontmost application has no specific focused UI element (kAXErrorNoValue). Returning the application element itself."
                        );
                        // Return the application element we found earlier
                        return Ok(self.wrap_element(ThreadSafeAXUIElement::new(focused_app_element), None, None, None, None));
                    }
                }
                // For any other error, report it as before
                 warn!("Failed to get kAXFocusedUIElementAttribute (as CFType) from frontmost application: {:?}", e);
                 Err(AutomationError::PlatformError(format!(
                    "Failed to get focused UI element from frontmost application: {:?}",
                    e
                 )))
            }
        }
    }

    fn get_application_by_name(&self, name: &str) -> Result<UIElement, AutomationError> {
        // Refresh the accessibility tree before searching
        self.refresh_accessibility_tree(Some(name))?;

        // Get all applications first, then filter by name
        let apps = self.get_applications()?;

        debug!(
            "Searching for application '{}' among {} applications",
            name,
            apps.len()
        );

        // Optimization: Convert target name to lowercase once, outside the loop
        let name_lowercase = name.to_lowercase();

        // Look for an application with a matching name
        for app in apps {
            let app_name = app.attributes().label.unwrap_or_default();
            // debug!("Checking application: '{}'", app_name);
            // std::thread::sleep(std::time::Duration::from_millis(1));

            // Case-insensitive comparison with pre-computed lowercase name
            if app_name.to_lowercase() == name_lowercase {
                debug!("found matching application: '{}'", app_name);
                return Ok(app);
            }
        }

        // No matching application found
        Err(AutomationError::ElementNotFound(format!(
            "Application '{}' not found",
            name
        )))
    }

    fn find_element(
        &self,
        selector: &Selector,
        root: Option<&UIElement>,
    ) -> Result<UIElement, AutomationError> {
        // If we have a root element that's an application, refresh the tree for that app
        if let Some(root_elem) = root {
            if let Some(macos_el) = root_elem.as_any().downcast_ref::<MacOSUIElement>() {
                if macos_el
                    .element
                    .0
                    .role()
                    .map_or(false, |r| r.to_string() == "AXApplication")
                {
                    if let Some(app_name) = root_elem.attributes().label {
                        self.refresh_accessibility_tree(Some(&app_name))?;
                    }
                }
            }
        }

        let start_element = root
            .map(|el| {
                if let Some(macos_el) = el.as_any().downcast_ref::<MacOSUIElement>() {
                    &macos_el.element.0
                } else {
                    // Use panic! for now as this indicates a programming error
                    panic!("Root element is not a macOS element")
                }
            })
            .unwrap_or(&self.system_wide.0);

        // Regular element finding logic
        match selector {
            Selector::Role { role, name } => { // Handle optional name here too
                // Get all possible macOS roles for this generic role
                let macos_roles = map_generic_role_to_macos_roles(role);
                let target_name = name.clone(); // Clone name for use in closure

                let collector = ElementFinderWithWindows::new(
                    start_element, // Pass start_element correctly
                    move |e| {
                        let element_role = e.role().unwrap_or(CFString::new("")).to_string();
                        if !macos_roles.contains(&element_role) {
                            return false;
                        }
                        // If name is specified, check it
                        if let Some(ref required_name) = target_name {
                            let element_title = e.title().unwrap_or(CFString::new("")).to_string();
                            if element_title != *required_name {
                                return false;
                            }
                        }
                        true // Role matches, and name matches if specified
                    },
                    None,
                );
                let walker: TreeWalkerWithWindows = TreeWalkerWithWindows::new();

                walker.walk(start_element, &collector);

                let ax_ui_element = match collector.find() {
                    Ok(ax_ui_element) => ax_ui_element,
                    Err(_) => {
                        return Err(AutomationError::ElementNotFound(format!(
                            "Element matching selector '{:?}' not found", // Improved error message
                            selector
                        )))
                    }
                };
                Ok(self.wrap_element(
                    ThreadSafeAXUIElement::new(ax_ui_element),
                    None,
                    None,
                    None,
                    None,
                ))
            }
            Selector::Id(id) => {
                let id_owned = id.clone(); // Create an owned copy
                let collector = ElementFinderWithWindows::new(
                    start_element, // Pass start_element correctly
                    move |e| {
                        // Check AXIdentifier first for a more direct match
                        let axid_attr = AXAttribute::new(&CFString::new("AXIdentifier"));
                        if let Ok(axid_val) = e.attribute(&axid_attr) {
                             if let Some(cf_string) = axid_val.downcast_into::<CFString>() {
                                if cf_string.to_string() == id_owned {
                                    return true;
                                }
                            }
                        }
                        // Fallback: Check generated ID (less reliable but better than nothing)
                        // Need to wrap 'e' to call id() - This is inefficient in a walker predicate!
                        // Consider if ID selector should *only* use AXIdentifier on macOS.
                        // For now, let's comment out the fallback due to inefficiency.
                        /*
                        let wrapped = self.wrap_element(ThreadSafeAXUIElement::new(e.clone()), None, None, None, None);
                        wrapped.id().map_or(false, |gen_id| gen_id == id_owned)
                        */
                        false // Only checking AXIdentifier for now
                    },
                    None,
                );
                let walker: TreeWalkerWithWindows = TreeWalkerWithWindows::new();

                walker.walk(start_element, &collector);

                let ax_ui_element = match collector.find() {
                    Ok(ax_ui_element) => ax_ui_element,
                    Err(_) => {
                        return Err(AutomationError::ElementNotFound(format!(
                            "Element with ID (AXIdentifier) '{}' not found", // Clarify ID type
                            id
                        )))
                    }
                };
                Ok(self.wrap_element(
                    ThreadSafeAXUIElement::new(ax_ui_element),
                    None,
                    None,
                    None,
                    None,
                ))
            }
             Selector::Name(name) => {
                let name_lower = name.to_lowercase(); // Case-insensitive comparison
                let collector = ElementFinderWithWindows::new(
                    start_element, // Pass start_element correctly
                    move |e| {
                        // Check AXTitle first
                        let title_match = e
                            .title()
                            .map_or(false, |t| t.to_string().to_lowercase() == name_lower);
                        if title_match {
                            return true;
                        }
                        // Fallback to AXLabel
                        let label_attr = AXAttribute::new(&CFString::new("AXLabel"));
                         if let Ok(label_val) = e.attribute(&label_attr) {
                             if let Some(cf_string) = label_val.downcast_into::<CFString>() {
                                if cf_string.to_string().to_lowercase() == name_lower {
                                    return true;
                                }
                            }
                        }
                        false
                    },
                    None,
                );
                let walker: TreeWalkerWithWindows = TreeWalkerWithWindows::new();

                walker.walk(start_element, &collector);

                 let ax_ui_element = match collector.find() {
                    Ok(ax_ui_element) => ax_ui_element,
                    Err(_) => {
                        return Err(AutomationError::ElementNotFound(format!(
                            "Element with name '{}' not found",
                            name
                        )))
                    }
                };
                Ok(self.wrap_element(
                    ThreadSafeAXUIElement::new(ax_ui_element),
                    None,
                    None,
                    None,
                    None,
                ))
            }
            Selector::Description(desc) => {
                let desc_lower = desc.to_lowercase();
                let collector = ElementFinderWithWindows::new(
                    start_element, // Pass start_element correctly
                    move |e| {
                        // Check AXDescription
                        let desc_attr = AXAttribute::new(&CFString::new("AXDescription"));
                        if let Ok(desc_val) = e.attribute(&desc_attr) {
                            if let Some(cf_string) = desc_val.downcast_into::<CFString>() {
                                if cf_string.to_string().to_lowercase() == desc_lower {
                                    return true;
                                }
                            }
                        }
                        false
                    },
                    None,
                );
                let walker: TreeWalkerWithWindows = TreeWalkerWithWindows::new();

                walker.walk(start_element, &collector);

                let ax_ui_element = match collector.find() {
                    Ok(ax_ui_element) => ax_ui_element,
                    Err(_) => {
                        return Err(AutomationError::ElementNotFound(format!(
                            "Element with description '{}' not found",
                            desc
                        )))
                    }
                };
                Ok(self.wrap_element(
                    ThreadSafeAXUIElement::new(ax_ui_element),
                    None,
                    None,
                    None,
                    None,
                ))
            }
            Selector::Text(text) => {
                let text_lower = text.to_lowercase(); // Case-insensitive comparison

                let collector = ElementFinderWithWindows::new(
                    start_element, // Pass start_element correctly
                    move |e| {
                        element_contains_text(e, &text_lower) // Use lower case text
                    },
                    None,
                );

                let walker: TreeWalkerWithWindows = TreeWalkerWithWindows::new();

                walker.walk(start_element, &collector);

                let ax_ui_element = match collector.find() {
                    Ok(ax_ui_element) => ax_ui_element,
                    Err(_) => {
                        return Err(AutomationError::ElementNotFound(format!(
                            "Element containing text '{}' not found",
                            text
                        )))
                    }
                };
                 Ok(self.wrap_element(
                    ThreadSafeAXUIElement::new(ax_ui_element),
                    None,
                    None,
                    None,
                    None,
                ))
            }
            Selector::Attributes(attrs) => {
                // Clone attrs for the closure
                let attrs_clone = attrs.clone();
                let collector = ElementFinderWithWindows::new(
                    start_element,
                    move |e| {
                        // Check attributes directly on the raw AXUIElement
                        check_ax_element_attributes_match(e, &attrs_clone)
                    },
                    None,
                );

                let walker: TreeWalkerWithWindows = TreeWalkerWithWindows::new();
                walker.walk(start_element, &collector);

                let ax_ui_element = match collector.find() {
                    Ok(ax_ui_element) => ax_ui_element,
                    Err(_) => {
                        return Err(AutomationError::ElementNotFound(format!(
                            "Element matching attributes '{:?}' not found",
                            attrs
                        )))
                    }
                };
                 Ok(self.wrap_element(
                    ThreadSafeAXUIElement::new(ax_ui_element),
                    None, None, None, None
                ))
            }
            Selector::Path(path) => {
                // Basic Path Implementation: Treat as a chain of Role/Name selectors separated by '/'
                // Example: "/AXApplication[@AXTitle='Finder']/AXWindow/AXButton[@AXIdentifier='save']"
                // This simple implementation only handles simple segments like "Role" or "Name"
                // and assumes '/' separation.
                // A full XPath-like implementation is complex.

                debug!("Processing Path selector: {}", path);
                let segments: Vec<&str> = path.trim_start_matches('/').split('/').collect();
                let mut current_element_opt = root.cloned(); // Start with the provided root or None
                let mut current_root_ref = root; // Keep track of the current root for find_element

                for segment in segments {
                    if segment.is_empty() { continue; }
                    // Very basic parsing: If it contains '[', assume attribute for now (unsupported)
                    // Otherwise, treat as Role or Name
                    if segment.contains('[') || segment.contains('@') {
                        return Err(AutomationError::UnsupportedOperation(
                            format!("Complex path segments with attribute filters ('{}') are not yet supported.", segment)
                        ));
                    }

                    // Convert segment string to a simple Selector (Role or Name)
                    let segment_selector = Selector::from(segment); // Use From<&str> logic

                    // Find the next element within the current one
                    let next_element = self.find_element(&segment_selector, current_root_ref)?;

                    // Update current element and root reference for the next iteration
                    current_element_opt = Some(next_element.clone());
                    current_root_ref = current_element_opt.as_ref(); // Need to store the owned element to reference it
                }

                // Return the final element found, or error if the chain broke
                current_element_opt.ok_or_else(|| AutomationError::ElementNotFound(format!(
                    "Path selector '{}' did not resolve to an element.", path
                )))
            }
            Selector::Chain(selectors) => {
                debug!("Processing Chain selector with {} parts", selectors.len());
                if selectors.is_empty() {
                    return Err(AutomationError::InvalidArgument("Chain selector cannot be empty".to_string()));
                }

                let mut current_element_opt = root.cloned();
                let mut current_root_ref = root; // Reference to the current element for find_element

                for (index, selector) in selectors.iter().enumerate() {
                    debug!("Chain part {}: {:?}", index + 1, selector);
                    let next_element = self.find_element(selector, current_root_ref)?;

                    // Update current element and root reference
                    current_element_opt = Some(next_element.clone());
                    current_root_ref = current_element_opt.as_ref();
                }

                // Return the final element found
                current_element_opt.ok_or_else(|| AutomationError::ElementNotFound(
                    "Chain selector did not resolve to an element (intermediate step failed)".to_string()
                ))
            }
            Selector::Filter(_) => Err(AutomationError::UnsupportedOperation(
                "Filter selector not implemented for find_element".to_string(),
            )),
        }
    }

    fn find_elements(
        &self,
        selector: &Selector,
        root: Option<&UIElement>,
    ) -> Result<Vec<UIElement>, AutomationError> {
        // Get the start element from the provided root or fall back to system_wide
         let start_element = root
            .map(|el| {
                if let Some(macos_el) = el.as_any().downcast_ref::<MacOSUIElement>() {
                    &macos_el.element.0
                } else {
                     panic!("Root element is not a macOS element") // Indicate programming error
                }
            })
            .unwrap_or(&self.system_wide.0);


        match selector {
            Selector::Role { role, name } => { // Handle optional name
                let macos_roles = map_generic_role_to_macos_roles(role);
                 let target_name = name.clone(); // Clone name for use in closure

                let collector = ElementsCollectorWithWindows::new(start_element, move |e| {
                    let element_role = e.role().unwrap_or(CFString::new("")).to_string();
                     if !macos_roles.contains(&element_role) {
                         return false;
                     }
                     // If name is specified, check it
                     if let Some(ref required_name) = target_name {
                         let element_title = e.title().unwrap_or(CFString::new("")).to_string();
                         if element_title != *required_name {
                             return false;
                         }
                     }
                     true
                });

                let ax_ui_elements = collector.find_all();

                // Convert AXUIElements to UIElements
                let ui_elements = ax_ui_elements
                    .into_iter()
                    .map(|e| {
                        self.wrap_element(ThreadSafeAXUIElement::new(e), None, None, None, None)
                    })
                    .collect();

                Ok(ui_elements)
            }
             Selector::Id(id) => {
                let id_owned = id.clone();
                let collector = ElementsCollectorWithWindows::new(start_element, move |e| {
                    // Check AXIdentifier first
                     let axid_attr = AXAttribute::new(&CFString::new("AXIdentifier"));
                     if let Ok(axid_val) = e.attribute(&axid_attr) {
                         if let Some(cf_string) = axid_val.downcast_into::<CFString>() {
                             if cf_string.to_string() == id_owned {
                                 return true;
                             }
                         }
                     }
                     // Fallback: generated ID (commented out due to inefficiency)
                     /*
                    let wrapped = self.wrap_element(ThreadSafeAXUIElement::new(e.clone()), None, None, None, None);
                    wrapped.id().map_or(false, |gen_id| gen_id == id_owned)
                    */
                     false
                });
                let ax_ui_elements = collector.find_all();
                let ui_elements = ax_ui_elements
                    .into_iter()
                    .map(|e| self.wrap_element(ThreadSafeAXUIElement::new(e), None, None, None, None))
                    .collect();
                Ok(ui_elements)
            }
            Selector::Name(name) => {
                let name_lower = name.to_lowercase();
                let collector = ElementsCollectorWithWindows::new(start_element, move |e| {
                     // Check AXTitle first
                     let title_match = e
                         .title()
                         .map_or(false, |t| t.to_string().to_lowercase() == name_lower);
                     if title_match {
                         return true;
                     }
                     // Fallback to AXLabel
                    let label_attr = AXAttribute::new(&CFString::new("AXLabel"));
                     if let Ok(label_val) = e.attribute(&label_attr) {
                         if let Some(cf_string) = label_val.downcast_into::<CFString>() {
                            if cf_string.to_string().to_lowercase() == name_lower {
                                return true;
                            }
                        }
                    }
                    false
                });
                 let ax_ui_elements = collector.find_all();
                let ui_elements = ax_ui_elements
                    .into_iter()
                    .map(|e| self.wrap_element(ThreadSafeAXUIElement::new(e), None, None, None, None))
                    .collect();
                Ok(ui_elements)
            }
            Selector::Description(desc) => {
                let desc_lower = desc.to_lowercase();
                let collector = ElementsCollectorWithWindows::new(start_element, move |e| {
                    // Check AXDescription
                    let desc_attr = AXAttribute::new(&CFString::new("AXDescription"));
                    if let Ok(desc_val) = e.attribute(&desc_attr) {
                        if let Some(cf_string) = desc_val.downcast_into::<CFString>() {
                            if cf_string.to_string().to_lowercase() == desc_lower {
                                return true;
                            }
                        }
                    }
                    false
                });
                let ax_ui_elements = collector.find_all();
                let ui_elements: Vec<UIElement> = ax_ui_elements
                    .into_iter()
                    .map(|e| self.wrap_element(ThreadSafeAXUIElement::new(e), None, None, None, None))
                    .collect();
                Ok(ui_elements)
            }
            Selector::Text(text) => {
                let text_lower = text.to_lowercase();
                let collector = ElementsCollectorWithWindows::new(start_element, move |e| {
                    element_contains_text(e, &text_lower) // Use lower case text
                });
                 let ax_ui_elements = collector.find_all();
                let ui_elements: Vec<UIElement> = ax_ui_elements
                    .into_iter()
                    .map(|e| self.wrap_element(ThreadSafeAXUIElement::new(e), None, None, None, None))
                    .collect();
                Ok(ui_elements)
            }
             Selector::Attributes(attrs) => {
                let attrs_clone = attrs.clone();
                // Use ElementsCollectorWithWindows which handles traversal
                let collector = ElementsCollectorWithWindows::new(start_element, move |e| {
                    // Check attributes directly on the raw AXUIElement
                    check_ax_element_attributes_match(e, &attrs_clone)
                });

                 let ax_ui_elements = collector.find_all(); // Get all matching AXUIElements

                 // Convert AXUIElements to UIElements
                let ui_elements: Vec<UIElement> = ax_ui_elements
                    .into_iter()
                    .map(|e| self.wrap_element(ThreadSafeAXUIElement::new(e), None, None, None, None))
                    .collect();

                 Ok(ui_elements)
            }
            Selector::Path(_) => Err(AutomationError::UnsupportedOperation(
                "Path selector is not supported for find_elements due to complexity.".to_string(),
            )),
            Selector::Filter(_) => Err(AutomationError::UnsupportedOperation(
                "Filter selector not implemented for find_elements".to_string(),
            )),
            Selector::Chain(_) => Err(AutomationError::UnsupportedOperation(
                "Chain selector is not supported for find_elements due to complexity.".to_string(),
            )),
        }
    }

    fn open_application(&self, app_name: &str) -> Result<UIElement, AutomationError> {
        debug!("opening application: {}", app_name);

        // Launch the application
        let status = std::process::Command::new("open")
            .args(["-a", app_name])
            .status()
            .map_err(|e| {
                AutomationError::PlatformError(format!("failed to execute 'open' command: {}", e))
            })?;

        if !status.success() {
            return Err(AutomationError::PlatformError(format!(
                "failed to open application '{}': exit code {:?}",
                app_name,
                status.code()
            )));
        }

        // Use a more efficient approach - directly get the app PID without full system scan
        let mut retry_count = 0;
        let max_retries = 10;
        let retry_delay = std::time::Duration::from_millis(100);

        // Retry loop with targeted scanning
        while retry_count < max_retries {
            debug!(
                "looking for newly launched app '{}', attempt {}/{}",
                app_name,
                retry_count + 1,
                max_retries
            );

            // Try to find the app directly without full refresh
            unsafe {
                use objc::{class, msg_send, sel, sel_impl};

                let workspace_class = class!(NSWorkspace);
                let shared_workspace: *mut objc::runtime::Object =
                    msg_send![workspace_class, sharedWorkspace];
                let apps: *mut objc::runtime::Object =
                    msg_send![shared_workspace, runningApplications];
                let count: usize = msg_send![apps, count];

                for i in 0..count {
                    let app: *mut objc::runtime::Object = msg_send![apps, objectAtIndex:i];
                    let app_name_obj: *mut objc::runtime::Object = msg_send![app, localizedName];

                    if !app_name_obj.is_null() {
                        let found_name: &str = {
                            let nsstring = app_name_obj as *const objc::runtime::Object;
                            let bytes: *const std::os::raw::c_char =
                                msg_send![nsstring, UTF8String];
                            let len: usize = msg_send![nsstring, lengthOfBytesUsingEncoding:4];
                            let bytes_slice = std::slice::from_raw_parts(bytes as *const u8, len);
                            std::str::from_utf8_unchecked(bytes_slice)
                        };

                        if found_name.to_lowercase() == app_name.to_lowercase() {
                            // Found the app, get its PID and create element directly
                            let pid: i32 = msg_send![app, processIdentifier];
                            debug!("found newly launched app '{}' with pid {}", app_name, pid);

                            // Create element directly instead of full scan
                            let app_element = ThreadSafeAXUIElement::application(pid);
                            return Ok(self.wrap_element(app_element, None, None, None, None));
                        }
                    }
                }
            }

            // App not found yet, sleep and retry
            std::thread::sleep(retry_delay);
            retry_count += 1;
        }

        // Fallback to existing approach if retries fail
        debug!("retries exceeded, falling back to standard method");
        self.refresh_accessibility_tree(Some(app_name))?;
        self.get_application_by_name(app_name)
    }

    fn open_url(&self, url: &str, browser: Option<&str>) -> Result<UIElement, AutomationError> {
        debug!("opening url: {} in browser: {:?}", url, browser);

        let status = match browser {
            Some(browser_name) => {
                // Open URL in the specified browser
                std::process::Command::new("open")
                    .args(["-a", browser_name, url])
                    .status()
                    .map_err(|e| {
                        AutomationError::PlatformError(format!(
                            "failed to execute 'open' command: {}",
                            e
                        ))
                    })?
            }
            None => {
                // Open URL in the default browser
                std::process::Command::new("open")
                    .arg(url)
                    .status()
                    .map_err(|e| {
                        AutomationError::PlatformError(format!(
                            "failed to execute 'open' command: {}",
                            e
                        ))
                    })?
            }
        };

        if !status.success() {
            return Err(AutomationError::PlatformError(format!(
                "failed to open url '{}': exit code {:?}",
                url,
                status.code()
            )));
        }

        // Give the browser a moment to launch
        std::thread::sleep(std::time::Duration::from_millis(1000));

        // If a specific browser was requested, try to get its UI element
        if let Some(browser_name) = browser {
            // Refresh accessibility tree with the browser
            self.refresh_accessibility_tree(Some(browser_name))?;

            // Get the browser application element
            self.get_application_by_name(browser_name)
        } else {
            // Without a specific browser name, we can't reliably return the browser element
            // Just return the system-wide element
            Ok(self.get_root_element())
        }
    }

    fn scroll_at_position(
        &self,
        x: f64,
        y: f64,
        direction: &str,
        amount: f64,
    ) -> Result<(), AutomationError> {
        // Call the non-trait method MacOSEngine::scroll_at_position
        MacOSEngine::scroll_at_position(self, x, y, direction, amount)
    }

    fn scroll_at_current_position(
        &self,
        direction: &str,
        amount: f64,
    ) -> Result<(), AutomationError> {
        // Call the non-trait method MacOSEngine::scroll_at_current_position
        MacOSEngine::scroll_at_current_position(self, direction, amount)
    }

    fn type_text(&self, text: &str) -> Result<(), AutomationError> {
        interaction::type_text_global(text)
    }

    fn get_clipboard_content(&self) -> Result<String, AutomationError> {
        // Keep using interaction module for this
        interaction::get_clipboard_contents()
    }

    fn set_clipboard_content(&self, content: &str) -> Result<(), AutomationError> {
        // Keep using interaction module for this
        interaction::set_clipboard_contents(content)
    }

    fn hold_key(&self, key: &str, duration_ms: Option<u64>) -> Result<(), AutomationError> {
        debug!("holding key {} for {:?}ms", key, duration_ms);

        let mut actual_key_name = key;
        let mut flags = CGEventFlags::empty();

        if key.contains('+') {
            let parts: Vec<&str> = key.split('+').collect();
            actual_key_name = parts.last().unwrap_or(&key); // Get the last part as the key
            for part in &parts[..parts.len() - 1] {
                let modifier_flag = match part.to_lowercase().as_str() {
                    "command" | "cmd" => CGEventFlags::CGEventFlagCommand,
                    "shift" => CGEventFlags::CGEventFlagShift,
                    "option" | "alt" => CGEventFlags::CGEventFlagAlternate,
                    "control" | "ctrl" => CGEventFlags::CGEventFlagControl,
                    "fn" => CGEventFlags::CGEventFlagSecondaryFn,
                    _ => {
                        return Err(AutomationError::InvalidArgument(format!(
                            "Unknown modifier: {}. Use standard modifier names.",
                            part
                        )))
                    }
                };
                flags |= modifier_flag;
            }
        }

        let key_code = match key_name_to_keycode(actual_key_name) {
            Some(code) => code,
            None => interaction::get_key_code(actual_key_name)?
        };

        interaction::hold_key(key_code, flags, duration_ms)
    }

    fn release_key(&self, key: &str) -> Result<(), AutomationError> {
        debug!("releasing key {}", key);

        let mut actual_key_name = key;
        let mut flags = CGEventFlags::empty();

        if key.contains('+') {
            let parts: Vec<&str> = key.split('+').collect();
            actual_key_name = parts.last().unwrap_or(&key);
            for part in &parts[..parts.len() - 1] {
                let modifier_flag = match part.to_lowercase().as_str() {
                    "command" | "cmd" => CGEventFlags::CGEventFlagCommand,
                    "shift" => CGEventFlags::CGEventFlagShift,
                    "option" | "alt" => CGEventFlags::CGEventFlagAlternate,
                    "control" | "ctrl" => CGEventFlags::CGEventFlagControl,
                    "fn" => CGEventFlags::CGEventFlagSecondaryFn,
                    _ => {
                        return Err(AutomationError::InvalidArgument(format!(
                            "Unknown modifier: {}. Use standard modifier names.",
                            part
                        )))
                    }
                };
                flags |= modifier_flag;
            }
        }

        let key_code = match key_name_to_keycode(actual_key_name) {
            Some(code) => code,
            None => interaction::get_key_code(actual_key_name)?
        };

        let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState).map_err(|_| {
            AutomationError::PlatformError("Failed to create event source for key release".to_string())
        })?;

        let key_up = CGEvent::new_keyboard_event(source, key_code, false).map_err(|_| {
            AutomationError::PlatformError(format!("Failed to create key up event for {}", actual_key_name))
        })?;

        if flags != CGEventFlags::empty() {
            key_up.set_flags(flags);
        }

        key_up.post(CGEventTapLocation::HID);

        Ok(())
    }

    fn wait(&self, duration_ms: u64) -> Result<(), AutomationError> {
        debug!("Engine calling wait for {} ms", duration_ms);
        interaction::wait(duration_ms)
    }

    fn get_ui_tree(&self, app_name: Option<&str>) -> Result<JsonValue, AutomationError> {
        // Call the non-trait method MacOSEngine::get_ui_tree
        MacOSEngine::get_ui_tree(self, app_name)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn press_key(&self, key_name: &str, modifier: Option<&str>) -> Result<(), AutomationError> {
        debug!("Pressing key: '{}' with modifier: {:?} using CGEvent", key_name, modifier);

        // Check if key_name contains a modifier already (like "ctrl+n" or "cmd+shift+a")
        if key_name.contains('+') {
            // Use the existing parse_key_combination function which handles multiple modifiers
            let (key_code, flags) = interaction::parse_key_combination(key_name)?;
            return interaction::press_key_with_modifier(key_code, flags);
        }

        // Normal case - separate key and modifier
        // First try key_name_to_keycode, then fall back to interaction::get_key_code
        let key_code = match key_name_to_keycode(key_name) {
            Some(code) => code,
            None => interaction::get_key_code(key_name)? // Fall back to interaction::get_key_code
        };

        let modifier_flags = if let Some(mod_name) = modifier {
            super::constants::modifier_name_to_flags(mod_name)
                .ok_or_else(|| AutomationError::InvalidArgument(format!("Invalid modifier name: {}", mod_name)))?
        } else {
            CGEventFlags::empty()
        };

        interaction::press_key_with_modifier(key_code, modifier_flags)
    }

    /// Get the current mouse cursor position.
    fn cursor_position(&self) -> Result<(f64, f64), AutomationError> {
        let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
            .map_err(|_| AutomationError::PlatformError("Failed to create event source".to_string()))?;
        let event = CGEvent::new(source) // Create a null event just to get location
             .map_err(|_| AutomationError::PlatformError("Failed to create null event for location".to_string()))?;
        let point = event.location();
        Ok((point.x, point.y))
    }

    fn mouse_move(&self, x: f64, y: f64) -> Result<(), AutomationError> {
        debug!("Moving mouse to ({}, {}) using CGEvent", x, y);
        let point = CGPoint::new(x, y);
        post_mouse_event(CGEventType::MouseMoved, point, CGMouseButton::Left, None, None) // Button doesn't matter for move
    }

    fn left_mouse_down(&self, x: f64, y: f64) -> Result<(), AutomationError> {
        debug!("Engine calling left_mouse_down at ({}, {})", x, y);
        // Use the updated implementation with None for modifiers
        interaction::left_mouse_down(x, y, None)
    }

    fn left_mouse_up(&self, x: f64, y: f64) -> Result<(), AutomationError> {
        debug!("Engine calling left_mouse_up at ({}, {})", x, y);
        // Use the updated implementation with None for modifiers
        interaction::left_mouse_up(x, y, None)
    }

    fn left_click(&self, x: f64, y: f64, modifiers: Option<&str>) -> Result<(), AutomationError> {
        debug!("Engine calling left_click at ({}, {}) with modifiers: {:?}", x, y, modifiers);

        let parsed_modifiers = modifiers.map(|m| parse_modifiers(Some(m)));
        interaction::left_click(x, y, parsed_modifiers)
    }

    fn right_click(&self, x: f64, y: f64, modifiers: Option<&str>) -> Result<(), AutomationError> {
        debug!("Right click at ({}, {}) with modifiers {:?} using CGEvent", x, y, modifiers);
        let point = CGPoint::new(x, y);
         let flags = parse_modifiers(modifiers);
        // Simulate down then up
        post_mouse_event(CGEventType::RightMouseDown, point, CGMouseButton::Right, Some(1), Some(flags))?;
        post_mouse_event(CGEventType::RightMouseUp, point, CGMouseButton::Right, Some(1), Some(flags))
    }

    fn middle_click(&self, x: f64, y: f64, modifiers: Option<&str>) -> Result<(), AutomationError> {
        debug!("Middle click at ({}, {}) with modifiers {:?} using CGEvent", x, y, modifiers);
        let point = CGPoint::new(x, y);
         let flags = parse_modifiers(modifiers);
        // Simulate down then up
        post_mouse_event(CGEventType::OtherMouseDown, point, CGMouseButton::Center, Some(1), Some(flags))?;
        post_mouse_event(CGEventType::OtherMouseUp, point, CGMouseButton::Center, Some(1), Some(flags))
    }

    fn double_click(&self, x: f64, y: f64, modifiers: Option<&str>) -> Result<(), AutomationError> {
         debug!("Double click at ({}, {}) with modifiers {:?} using CGEvent", x, y, modifiers);
        let point = CGPoint::new(x, y);
         let flags = parse_modifiers(modifiers);
        // Simulate two clicks (down, up, down, up) with click count 2
        post_mouse_event(CGEventType::LeftMouseDown, point, CGMouseButton::Left, Some(1), Some(flags))?; // Click 1 down
        post_mouse_event(CGEventType::LeftMouseUp, point, CGMouseButton::Left, Some(1), Some(flags))?;   // Click 1 up
        post_mouse_event(CGEventType::LeftMouseDown, point, CGMouseButton::Left, Some(2), Some(flags))?; // Click 2 down (state=2)
        post_mouse_event(CGEventType::LeftMouseUp, point, CGMouseButton::Left, Some(2), Some(flags))     // Click 2 up (state=2)
    }

    fn triple_click(&self, x: f64, y: f64, modifiers: Option<&str>) -> Result<(), AutomationError> {
         debug!("Triple click at ({}, {}) with modifiers {:?} using CGEvent", x, y, modifiers);
        let point = CGPoint::new(x, y);
        let flags = parse_modifiers(modifiers);
        // Simulate three clicks
        post_mouse_event(CGEventType::LeftMouseDown, point, CGMouseButton::Left, Some(1), Some(flags))?;
        post_mouse_event(CGEventType::LeftMouseUp, point, CGMouseButton::Left, Some(1), Some(flags))?;
        post_mouse_event(CGEventType::LeftMouseDown, point, CGMouseButton::Left, Some(2), Some(flags))?;
        post_mouse_event(CGEventType::LeftMouseUp, point, CGMouseButton::Left, Some(2), Some(flags))?;
        post_mouse_event(CGEventType::LeftMouseDown, point, CGMouseButton::Left, Some(3), Some(flags))?;
        post_mouse_event(CGEventType::LeftMouseUp, point, CGMouseButton::Left, Some(3), Some(flags))
    }

    fn left_click_drag(
        &self,
        start_x: f64,
        start_y: f64,
        end_x: f64,
        end_y: f64,
    ) -> Result<(), AutomationError> {
         debug!("Left click drag from ({}, {}) to ({}, {}) using CGEvent", start_x, start_y, end_x, end_y);
        let start_point = CGPoint::new(start_x, start_y);
        let end_point = CGPoint::new(end_x, end_y);

        // Mouse down at start
        post_mouse_event(CGEventType::LeftMouseDown, start_point, CGMouseButton::Left, Some(1), None)?;
        // Small delay before dragging
        std::thread::sleep(std::time::Duration::from_millis(50));
        // Drag to end
        post_mouse_event(CGEventType::LeftMouseDragged, end_point, CGMouseButton::Left, None, None)?; // Button indicates drag type
        // Small delay at end
        std::thread::sleep(std::time::Duration::from_millis(50));
        // Mouse up at end
        post_mouse_event(CGEventType::LeftMouseUp, end_point, CGMouseButton::Left, Some(1), None)
    }

    fn get_window_title(&self) -> Result<String, AutomationError> {
        // Implementation Note: Get focused element, check if it's a window, get AXTitle.
        // Need robust error handling if no focus or not a window.

        debug!("Getting window title");

        let focused_element = self.get_focused_element()?;

        if let Some(macos_element) = focused_element.as_any().downcast_ref::<MacOSUIElement>() {
            let ax_element = &macos_element.element.0;

            // Check if the focused element is a window
            let role = ax_element.role().map_or(String::new(), |r| r.to_string());
            if role == "AXWindow" {
                // Get the AXTitle attribute
                match ax_element.title() {
                    Ok(title_cf) => {
                        let title = title_cf.to_string();
                        debug!("Found window title: {}", title);
                        Ok(title)
                    }
                    Err(e) => {
                        warn!("Focused window element does not have a title: {:?}", e);
                        Err(AutomationError::PlatformError(format!(
                            "Failed to get title for focused window: {:?}",
                            e
                        )))
                    }
                }
            } else {
                warn!(
                    "Focused element is not a window (role: {}), cannot get title.",
                    role
                );
                Err(AutomationError::UnsupportedOperation(
                    "Cannot get window title from a non-window element".to_string(),
                ))
            }
        } else {
            // This should ideally not happen if get_focused_element works correctly
            warn!("Could not downcast focused element to MacOSUIElement");
            Err(AutomationError::PlatformError(
                "Failed to interpret focused element as a macOS element".to_string(),
            ))
        }
    }

    fn list_windows(&self) -> Result<Vec<UIElement>, AutomationError> {
        // Implementation Note: Iterate through applications from get_applications(),
        // then get AXWindows for each. Or use a system-level API if available.

        debug!("Listing all windows");
        let mut all_windows = Vec::new();
        let apps = self.get_applications()?;

        for app_ui_element in apps {
            if let Some(macos_app_element) = app_ui_element.as_any().downcast_ref::<MacOSUIElement>() {
                let ax_app_element = &macos_app_element.element.0;

                match ax_app_element.windows() {
                    Ok(windows) => {
                        debug!(
                            "Found {} windows for app {:?}",
                            windows.len(),
                            macos_app_element.cached_label
                        );
                        for window_ax_element in &windows {
                            let role = window_ax_element.role().ok().map(|s| s.to_string());
                            let title = window_ax_element.title().ok().map(|s| s.to_string());
                            let desc = window_ax_element.description().ok().map(|s| s.to_string());

                            all_windows.push(self.wrap_element(
                                ThreadSafeAXUIElement::new(window_ax_element.clone()),
                                role,
                                title,
                                desc,
                                None, // Value cache - windows typically don't have a simple value
                            ));
                        }
                    }
                    Err(e) => {
                        warn!(
                            "Failed to get windows for app {:?}: {:?}",
                            macos_app_element.cached_label, e
                        );
                    }
                }
            } else {
                warn!("Could not downcast application UIElement to MacOSUIElement");
            }
        }

        debug!("Found a total of {} windows", all_windows.len());
        Ok(all_windows)
    }

    fn close_window(&self) -> Result<(), AutomationError> {
        // Implementation Note: Get focused element, check if it's a window,
        // find the close button (AXCloseButton) and click it.
        // Alternatively, use AXPerformAction kAXPressAction on the close button.

        debug!("Attempting to close the focused window");

        let focused_element = self.get_focused_element()?;

        if let Some(macos_element) = focused_element.as_any().downcast_ref::<MacOSUIElement>() {
            let ax_window = &macos_element.element.0;

            // 1. Verify it's a window
            let role = ax_window.role().map_or(String::new(), |r| r.to_string());
            if role != "AXWindow" {
                warn!("Focused element is not a window (role: {}), cannot close.", role);
                return Err(AutomationError::UnsupportedOperation(
                    "Cannot close a non-window element".to_string(),
                ));
            }

            // 2. Find the close button (AXCloseButton)
            // We can use the AXAttribute directly to search for the button
            let close_button_attr = AXAttribute::new(&CFString::new("AXCloseButton"));
            match ax_window.attribute(&close_button_attr) {
                Ok(button_val) => {
                    if let Some(close_button) = button_val.downcast::<accessibility::AXUIElement>() {
                        // 3. Perform the press action
                        let press_action = CFString::new("AXPress"); // kAXPressAction
                        match close_button.perform_action(&press_action) {
                            Ok(_) => {
                                debug!("Successfully performed AXPress on the close button");
                                Ok(())
                            }
                            Err(e) => {
                                warn!("Failed to perform AXPress on close button: {:?}", e);
                                Err(AutomationError::PlatformError(format!(
                                    "Failed to press the close button: {:?}",
                                    e
                                )))
                            }
                        }
                    } else {
                        warn!("AXCloseButton attribute did not return a valid UI element.");
                        Err(AutomationError::ElementNotFound(
                            "Could not find the close button element within the window".to_string(),
                        ))
                    }
                }
                Err(e) => {
                    warn!("Failed to get AXCloseButton attribute: {:?}", e);
                    Err(AutomationError::ElementNotFound(format!(
                        "Could not find the close button attribute: {:?}",
                        e
                    )))
                }
            }
        } else {
            warn!("Could not downcast focused element to MacOSUIElement");
            Err(AutomationError::PlatformError(
                "Failed to interpret focused element as a macOS element for closing".to_string(),
            ))
        }
    }

    fn maximize_window(&self) -> Result<(), AutomationError> {
        // Implementation Note: Get focused element, check if window,
        // find maximize button (AXZoomButton) and click it.
        debug!("Attempting to maximize the focused window");

        let focused_element = self.get_focused_element()?;

        if let Some(macos_element) = focused_element.as_any().downcast_ref::<MacOSUIElement>() {
            let ax_window = &macos_element.element.0;

            // 1. Verify it's a window
            let role = ax_window.role().map_or(String::new(), |r| r.to_string());
            if role != "AXWindow" {
                warn!(
                    "Focused element is not a window (role: {}), cannot maximize.",
                    role
                );
                return Err(AutomationError::UnsupportedOperation(
                    "Cannot maximize a non-window element".to_string(),
                ));
            }

            // 2. Find the maximize/zoom button (AXZoomButton)
            let zoom_button_attr = AXAttribute::new(&CFString::new("AXZoomButton"));
            match ax_window.attribute(&zoom_button_attr) {
                Ok(button_val) => {
                    if let Some(zoom_button) = button_val.downcast::<accessibility::AXUIElement>() {
                        // 3. Perform the press action
                        let press_action = CFString::new("AXPress");
                        match zoom_button.perform_action(&press_action) {
                            Ok(_) => {
                                debug!("Successfully performed AXPress on the zoom button");
                                Ok(())
                            }
                            Err(e) => {
                                warn!("Failed to perform AXPress on zoom button: {:?}", e);
                                Err(AutomationError::PlatformError(format!(
                                    "Failed to press the zoom button: {:?}",
                                    e
                                )))
                            }
                        }
                    } else {
                        warn!("AXZoomButton attribute did not return a valid UI element.");
                        Err(AutomationError::ElementNotFound(
                            "Could not find the zoom button element within the window".to_string(),
                        ))
                    }
                }
                Err(e) => {
                    warn!("Failed to get AXZoomButton attribute: {:?}", e);
                    Err(AutomationError::ElementNotFound(format!(
                        "Could not find the zoom button attribute: {:?}",
                        e
                    )))
                }
            }
        } else {
            warn!("Could not downcast focused element to MacOSUIElement");
            Err(AutomationError::PlatformError(
                "Failed to interpret focused element as a macOS element for maximizing".to_string(),
            ))
        }
    }

    fn minimize_window(&self) -> Result<(), AutomationError> {
        // Implementation Note: Get focused element, check if window,
        // find minimize button (AXMinimizeButton) and click it.
        // Or set AXMinimized attribute to true.

        debug!("Attempting to minimize the focused window");

        let focused_element = self.get_focused_element()?;

        if let Some(macos_element) = focused_element.as_any().downcast_ref::<MacOSUIElement>() {
            let ax_window = &macos_element.element.0;

            // 1. Verify it's a window
            let role = ax_window.role().map_or(String::new(), |r| r.to_string());
            if role != "AXWindow" {
                warn!(
                    "Focused element is not a window (role: {}), cannot minimize.",
                    role
                );
                return Err(AutomationError::UnsupportedOperation(
                    "Cannot minimize a non-window element".to_string(),
                ));
            }

            // 2. Set the AXMinimized attribute to true
            let minimized_attr = AXAttribute::new(&CFString::new("AXMinimized"));
            let value_to_set = CFBoolean::true_value();

            match ax_window.set_attribute(&minimized_attr, value_to_set.as_CFType()) {
                Ok(_) => {
                    debug!("Successfully set AXMinimized attribute to true");
                    Ok(())
                }
                Err(e) => {
                    warn!("Failed to set AXMinimized attribute: {:?}", e);
                    // Consider if pressing the button is a fallback?
                    // For now, report the error directly.
                    Err(AutomationError::PlatformError(format!(
                        "Failed to set the minimized attribute: {:?}",
                        e
                    )))
                }
            }
        } else {
            warn!("Could not downcast focused element to MacOSUIElement");
            Err(AutomationError::PlatformError(
                "Failed to interpret focused element as a macOS element for minimizing".to_string(),
            ))
        }
    }

    fn resize_window(&self, width: f64, height: f64) -> Result<(), AutomationError> {
        // Implementation Note: Get focused element, check if window,
        // set AXSize attribute.

        debug!("Attempting to resize the focused window to width={}, height={}", width, height);

        let focused_element = self.get_focused_element()?;

        if let Some(macos_element) = focused_element.as_any().downcast_ref::<MacOSUIElement>() {
            let ax_window = &macos_element.element.0;

            // 1. Verify it's a window
            let role = ax_window.role().map_or(String::new(), |r| r.to_string());
            if role != "AXWindow" {
                warn!(
                    "Focused element is not a window (role: {}), cannot resize.",
                    role
                );
                return Err(AutomationError::UnsupportedOperation(
                    "Cannot resize a non-window element".to_string(),
                ));
            }

            // 2. Create CGSize and AXValue
            let size_attr = AXAttribute::new(&CFString::new("AXSize"));
            let mut cg_size = core_graphics::geometry::CGSize::new(width, height);
            let size_ptr = &mut cg_size as *mut _ as *const std::ffi::c_void;

            unsafe {
                // Use AXValueCreate from ffi or accessibility_sys if available
                // Assuming K_AXVALUE_CGSIZE_TYPE is defined similarly to K_AXVALUE_CGPOINT_TYPE
                let value_ref = super::ffi::AXValueCreate(super::constants::K_AXVALUE_CGSIZE_TYPE, size_ptr);
                if value_ref.is_null() {
                    warn!("Failed to create AXValueRef for CGSize");
                    return Err(AutomationError::PlatformError(
                        "Could not create AXValue for size".to_string(),
                    ));
                }

                // 3. Set the AXSize attribute
                // Need to wrap value_ref appropriately for set_attribute
                // TCFType::wrap_under_create_rule might work if AXValueRef is a CFTypeRef
                let value_to_set = CFType::wrap_under_create_rule(value_ref);

                match ax_window.set_attribute(&size_attr, value_to_set) {
                    Ok(_) => {
                        debug!("Successfully set AXSize attribute");
                        Ok(())
                    }
                    Err(e) => {
                        warn!("Failed to set AXSize attribute: {:?}", e);
                        Err(AutomationError::PlatformError(format!(
                            "Failed to set the size attribute: {:?}",
                            e
                        )))
                    }
                    // Ensure the created CFTypeRef (AXValueRef) is released if wrap_under_create_rule doesn't handle it
                    // However, CFType wrapper should manage the retain count.
                }
            }
        } else {
            warn!("Could not downcast focused element to MacOSUIElement");
            Err(AutomationError::PlatformError(
                "Failed to interpret focused element as a macOS element for resizing".to_string(),
            ))
        }
    }

    fn move_window(&self, x: f64, y: f64) -> Result<(), AutomationError> {
        // Implementation Note: Get focused element, check if window,
        // set AXPosition attribute.

        debug!("Attempting to move the focused window to x={}, y={}", x, y);

        let focused_element = self.get_focused_element()?;

        if let Some(macos_element) = focused_element.as_any().downcast_ref::<MacOSUIElement>() {
            let ax_window = &macos_element.element.0;

            // 1. Verify it's a window
            let role = ax_window.role().map_or(String::new(), |r| r.to_string());
            if role != "AXWindow" {
                warn!("Focused element is not a window (role: {}), cannot move.", role);
                return Err(AutomationError::UnsupportedOperation(
                    "Cannot move a non-window element".to_string(),
                ));
            }

            // 2. Create CGPoint and AXValue
            let position_attr = AXAttribute::new(&CFString::new("AXPosition"));
            let mut cg_point = core_graphics::geometry::CGPoint::new(x, y);
            let point_ptr = &mut cg_point as *mut _ as *const std::ffi::c_void;

            unsafe {
                let value_ref =
                    super::ffi::AXValueCreate(super::constants::K_AXVALUE_CGPOINT_TYPE, point_ptr);
                if value_ref.is_null() {
                    warn!("Failed to create AXValueRef for CGPoint");
                    return Err(AutomationError::PlatformError(
                        "Could not create AXValue for position".to_string(),
                    ));
                }

                // 3. Set the AXPosition attribute
                let value_to_set = CFType::wrap_under_create_rule(value_ref);

                match ax_window.set_attribute(&position_attr, value_to_set) {
                    Ok(_) => {
                        debug!("Successfully set AXPosition attribute");
                        Ok(())
                    }
                    Err(e) => {
                        warn!("Failed to set AXPosition attribute: {:?}", e);
                        Err(AutomationError::PlatformError(format!(
                            "Failed to set the position attribute: {:?}",
                            e
                        )))
                    }
                }
            }
        } else {
            warn!("Could not downcast focused element to MacOSUIElement");
            Err(AutomationError::PlatformError(
                "Failed to interpret focused element as a macOS element for moving".to_string(),
            ))
        }
    }

    fn get_element_tree(&self, element: &UIElement) -> Result<ElementTreeNode, AutomationError> {
        element.get_tree() // Delegate to the UIElement's get_tree method
    }
}
