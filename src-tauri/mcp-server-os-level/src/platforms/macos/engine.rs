use super::element::MacOSUIElement;
use super::permissions::check_accessibility_permissions;
use super::utils::{
    element_contains_text, get_pid_for_element, get_running_application_pids,
    map_generic_role_to_macos_roles,
};
use super::wrappers::ThreadSafeAXUIElement;
use crate::platforms::tree_search::{
    ElementFinderWithWindows, ElementsCollectorWithWindows, TreeWalkerWithWindows,
};
use crate::platforms::AccessibilityEngine;
use crate::{AutomationError, Selector, UIElement};
use accessibility::{AXAttribute, AXUIElementAttributes, Error as AXError};
use accessibility_sys::{kAXFocusedUIElementAttribute, AXUIElementRef, kAXFrontmostAttribute, AXUIElementGetTypeID, kAXErrorNoValue};
use anyhow::Result;
use core_foundation::base::{CFType, TCFType, CFTypeID, CFGetTypeID};
use core_foundation::boolean::CFBoolean;
use core_foundation::number::CFNumber;
use core_foundation::string::CFString;
use core_graphics::display::CGPoint;
use core_graphics::event::{CGEvent, CGEventTapLocation, CGEventType, CGMouseButton, CGEventFlags};
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
use libc;
use std::collections::BTreeMap;
use tracing::{debug, trace, warn};
use crate::platforms::macos::interaction::{self};
use crate::platforms::macos::constants::{
    COMMAND_KEYCODE, CONTROL_KEYCODE, OPTION_KEYCODE, SHIFT_KEYCODE, // Key codes
    MODIFIER_COMMAND, MODIFIER_SHIFT, MODIFIER_OPTION, MODIFIER_CONTROL, // Modifier flags
    key_name_to_keycode, modifier_name_to_flags // Mapping functions
};
use serde_json::{json, Value as JsonValue};

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

    pub(crate) fn focus_application_with_cache(
        &self,
        app_name: &str,
        app_cache: Option<&ThreadSafeAXUIElement>,
    ) -> Result<ThreadSafeAXUIElement, AutomationError> {
        debug!("focusing application: {}", app_name);

        if let Some(cached_element) = app_cache {
            debug!("using cached application element");

            match cached_element.0.role() {
                Ok(role) if role.to_string() == "AXApplication" => unsafe {
                    use objc::{class, msg_send, sel, sel_impl};
                    let pid = get_pid_for_element(cached_element);

                    let nsra_class = class!(NSRunningApplication);
                    let app: *mut objc::runtime::Object =
                        msg_send![nsra_class, runningApplicationWithProcessIdentifier:pid];
                    if !app.is_null() {
                        let _: () = msg_send![app, activateWithOptions:1];
                        debug!("Activated application using cached element");

                        return Ok(cached_element.clone());
                    }
                },
                _ => {
                    debug!("Cached element is no longer valid");
                }
            }
        }

        self.refresh_accessibility_tree(Some(app_name))?;

        unsafe {
            use objc::{class, msg_send, sel, sel_impl};

            let workspace_class = class!(NSWorkspace);
            let shared_workspace: *mut objc::runtime::Object =
                msg_send![workspace_class, sharedWorkspace];
            let apps: *mut objc::runtime::Object = msg_send![shared_workspace, runningApplications];
            let count: usize = msg_send![apps, count];

            for i in 0..count {
                let app: *mut objc::runtime::Object = msg_send![apps, objectAtIndex:i];
                let app_name_obj: *mut objc::runtime::Object = msg_send![app, localizedName];

                if !app_name_obj.is_null() {
                    let app_name_str: &str = {
                        let nsstring = app_name_obj as *const objc::runtime::Object;
                        let bytes: *const std::os::raw::c_char = msg_send![nsstring, UTF8String];
                        let len: usize = msg_send![nsstring, lengthOfBytesUsingEncoding:4];
                        let bytes_slice = std::slice::from_raw_parts(bytes as *const u8, len);
                        std::str::from_utf8_unchecked(bytes_slice)
                    };

                    if app_name_str.to_lowercase() == app_name.to_lowercase() {
                        let pid: i32 = msg_send![app, processIdentifier];
                        let ax_element = ThreadSafeAXUIElement::application(pid);

                        return Ok(ax_element);
                    }
                }
            }
        }

        Err(AutomationError::ElementNotFound(format!(
            "Application '{}' not found",
            app_name
        )))
    }

    pub(crate) fn scroll_at_position(
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

        let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState).map_err(|_| {
            AutomationError::PlatformError("Failed to create event source".to_string())
        })?;

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

        let point = CGPoint::new(x, y);
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

        let scroll_event =
            CGEvent::new_scroll_event(source, 0, 1, scroll_y, scroll_x, 0).map_err(|_| {
                AutomationError::PlatformError("Failed to create scroll event".to_string())
            })?;

        scroll_event.post(CGEventTapLocation::HID);

        debug!(
            "scrolled {} by {} at position ({}, {})",
            direction, amount, x, y
        );
        Ok(())
    }

    pub fn scroll_at_current_position(
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
        let policy_attr = accessibility::AXAttribute::<CFType>::new(&CFString::new("AXActivationPolicy"));

        for pid in pids {
            let app_element = accessibility::AXUIElement::application(pid);

            // Optional: Manually skip background-only apps if get_running_application_pids(true) includes them unexpectedly
            // It *shouldn't* based on NSWorkspace docs, but let's be safe.
            // We'll rely on the kAXFrontmost check primarily.
            /*
            if let Ok(policy_val) = app_element.attribute(&policy_attr) {
                if let Some(policy_num) = policy_val.downcast_into::<CFNumber>() {
                    if let Some(policy_int) = policy_num.to_i64() {
                        if policy_int == 2 { // NSApplicationActivationPolicyProhibited
                             trace!("Skipping PID {} due to activation policy 2", pid);
                             continue;
                        }
                    }
                }
            }
            */

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
            Selector::Path(_) => Err(AutomationError::UnsupportedOperation(
                "Path selector not implemented".to_string(),
            )),
            Selector::Chain(selectors) => {
                // For now, only support role -> id pattern
                if selectors.len() != 2 {
                    return Err(AutomationError::UnsupportedOperation(
                        "Only role -> id chains are supported".to_string(),
                    ));
                }

                // Check if it's a role -> id pattern
                if let (Selector::Role { role, name: _ }, Selector::Id(id)) =
                    (&selectors[0], &selectors[1])
                {
                    debug!("processing chain: role '{}' -> id '{}'", role, id);

                    // First find elements matching the role
                    let role_elements = self.find_elements(&selectors[0], root)?;
                    debug!(
                        "found {} elements matching role '{}'",
                        role_elements.len(),
                        role
                    );

                    // Then find the one with matching id
                    for element in role_elements {
                        if let Some(element_id) = element.id() {
                            if element_id == *id {
                                debug!("found matching element with id '{}'", id);
                                return Ok(element);
                            }
                        }
                    }

                    return Err(AutomationError::ElementNotFound(format!(
                        "no element found with role '{}' and id '{}'",
                        role, id
                    )));
                } else {
                    return Err(AutomationError::UnsupportedOperation(
                        "only role -> id chains are supported".to_string(),
                    ));
                }
            }
            Selector::Filter(_) => Err(AutomationError::UnsupportedOperation(
                "Filter selector not implemented for find_elements".to_string(),
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
            Selector::Text(text) => {
                let text_lower = text.to_lowercase();
                let collector = ElementsCollectorWithWindows::new(start_element, move |e| {
                    element_contains_text(e, &text_lower) // Use lower case text
                });
                 let ax_ui_elements = collector.find_all();
                let ui_elements = ax_ui_elements
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
                let ui_elements = ax_ui_elements
                    .into_iter()
                    .map(|e| self.wrap_element(ThreadSafeAXUIElement::new(e), None, None, None, None))
                    .collect();

                 Ok(ui_elements)
            }
            Selector::Path(_) => Err(AutomationError::UnsupportedOperation(
                "Path selector not implemented".to_string(),
            )),
            Selector::Filter(_) => Err(AutomationError::UnsupportedOperation(
                "Filter selector not implemented for find_elements".to_string(),
            )),
            Selector::Chain(_) => Err(AutomationError::UnsupportedOperation(
                "Chain selector not implemented for find_elements".to_string(),
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
        self.scroll_at_position(x, y, direction, amount)
    }

    fn scroll_at_current_position(
        &self,
        direction: &str,
        amount: f64,
    ) -> Result<(), AutomationError> {
        self.scroll_at_current_position(direction, amount)
    }

    fn type_text(&self, text: &str) -> Result<(), AutomationError> {
        // For global typing, we don't have a specific element context
        // We will simulate key presses directly
        interaction::type_text_global(text)
    }

    fn get_clipboard_content(&self) -> Result<String, AutomationError> {
        interaction::get_clipboard_contents()
    }

    fn set_clipboard_content(&self, content: &str) -> Result<(), AutomationError> {
        interaction::set_clipboard_contents(content)
    }

    fn hold_key(&self, key: &str) -> Result<(), AutomationError> {
        let lower_key = key.to_lowercase();
        let (key_code, flags) = match lower_key.as_str() {
            "shift" => (SHIFT_KEYCODE, MODIFIER_SHIFT),
            "cmd" | "command" | "meta" => (COMMAND_KEYCODE, MODIFIER_COMMAND),
            "ctrl" | "control" => (CONTROL_KEYCODE, MODIFIER_CONTROL),
            "alt" | "option" => (OPTION_KEYCODE, MODIFIER_OPTION),
            _ => return Err(AutomationError::InvalidArgument(format!(
                "Unsupported or non-modifier key for hold_key: {}",
                key
            ))),
        };
        interaction::hold_key(key_code, flags)
    }

    fn release_key(&self, key: &str) -> Result<(), AutomationError> {
        let lower_key = key.to_lowercase();
        let (key_code, flags) = match lower_key.as_str() {
             "shift" => (SHIFT_KEYCODE, MODIFIER_SHIFT),
            "cmd" | "command" | "meta" => (COMMAND_KEYCODE, MODIFIER_COMMAND),
            "ctrl" | "control" => (CONTROL_KEYCODE, MODIFIER_CONTROL),
            "alt" | "option" => (OPTION_KEYCODE, MODIFIER_OPTION),
            _ => return Err(AutomationError::InvalidArgument(format!(
                "Unsupported or non-modifier key for release_key: {}",
                key
            ))),
        };
        interaction::release_key(key_code, flags)
    }

    fn wait(&self, duration_ms: u64) -> Result<(), AutomationError> {
        debug!("waiting for {} milliseconds", duration_ms);
        std::thread::sleep(std::time::Duration::from_millis(duration_ms));
        Ok(())
    }

    fn get_ui_tree(&self, app_name: Option<&str>) -> Result<JsonValue, AutomationError> {
        self.get_ui_tree(app_name) // Call the struct's method
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn press_key(&self, key_name: &str, modifier: Option<&str>) -> Result<(), AutomationError> {
        debug!("pressing key: {} with modifier: {:?}", key_name, modifier);

        let key_code = key_name_to_keycode(key_name)
            .ok_or_else(|| AutomationError::InvalidArgument(format!("Invalid key name: {}", key_name)))?;

        let modifier_flags = match modifier {
            Some(mod_name) => modifier_name_to_flags(mod_name)
                .ok_or_else(|| AutomationError::InvalidArgument(format!("Invalid modifier name: {}", mod_name)))?,
            None => CGEventFlags::empty(),
        };

        interaction::press_key_with_modifier(key_code, modifier_flags)
    }
}
