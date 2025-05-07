use accessibility::{AXAttribute, AXUIElement};
use super::constants::*;
use super::engine::MacOSEngine;
use super::ffi::AXValueGetValue;
use super::interaction;
use super::utils::macos_role_to_generic_role;
use super::wrappers::ThreadSafeAXUIElement;
use crate::platforms::macos::attributes::parse_ax_attribute_value;
use crate::platforms::tree_search::ElementsCollectorWithWindows;
use crate::UIElementAttributes;
use crate::{element::UIElementImpl, AutomationError, ClickResult, Locator, Selector, UIElement};
use accessibility::{AXUIElementAttributes as AXAttrsTrait};
use anyhow::Result;
use core_foundation::base::TCFType;
use core_foundation::number::CFNumber;
use core_foundation::string::CFString;
use core_graphics::event::{CGEvent, CGEventTapLocation};
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
use core_graphics::geometry::{CGPoint, CGSize};
use objc::{class, msg_send, sel, sel_impl};
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use tracing::{debug, warn};
use core_graphics::event::{CGEventType, CGMouseButton};
use crate::element::ElementTreeNode;
use serde_json;

#[derive(Debug)]
pub struct MacOSUIElement {
    pub(crate) element: ThreadSafeAXUIElement,
    pub(crate) use_background_apps: bool,
    pub(crate) activate_app: bool,
    pub(crate) cached_role: String,
    pub(crate) cached_label: Option<String>,
    pub(crate) cached_description: Option<String>,
    pub(crate) cached_value: Option<String>,
}

impl MacOSUIElement {
    pub(crate) fn generate_stable_id(&self) -> String {
        let mut hasher = DefaultHasher::new();
        let role = self
            .element
            .0
            .role()
            .map(|r| r.to_string())
            .unwrap_or_default();
        let title = self
            .element
            .0
            .title()
            .map(|t| t.to_string())
            .unwrap_or_default();
        let desc = self
            .element
            .0
            .description()
            .map(|d| d.to_string())
            .unwrap_or_default();

        let (_, _, w, h) = self
            .bounds()
            .map(|(x, y, w, h)| {
                (
                    x.round() as i32,
                    y.round() as i32,
                    w.round() as i32,
                    h.round() as i32,
                )
            })
            .unwrap_or((0, 0, 0, 0));

        let count_of_children = self.children().unwrap_or_default().len();

        role.hash(&mut hasher);
        title.hash(&mut hasher);
        desc.hash(&mut hasher);
        w.hash(&mut hasher);
        h.hash(&mut hasher);
        count_of_children.hash(&mut hasher);

        if let Ok(Some(parent)) = self.parent() {
            if let Some(parent_label) = parent.attributes().label {
                parent_label.hash(&mut hasher);
            }
        }

        format!("ax_{:x}", hasher.finish())
    }

    pub(crate) fn bounds(&self) -> Result<(f64, f64, f64, f64), AutomationError> {
        let mut x = 0.0;
        let mut y = 0.0;
        let mut width = 0.0;
        let mut height = 0.0;

        if let Ok(position) = self
            .element
            .0
            .attribute(&AXAttribute::new(&CFString::new("AXPosition")))
        {
            unsafe {
                let value_ref = position.as_CFTypeRef();
                let mut point: CGPoint = CGPoint { x: 0.0, y: 0.0 };
                let point_ptr = &mut point as *mut CGPoint as *mut ::std::os::raw::c_void;
                if AXValueGetValue(value_ref as *const _, K_AXVALUE_CGPOINT_TYPE, point_ptr) != 0 {
                    x = point.x;
                    y = point.y;
                }
            }
        }

        if let Ok(size) = self
            .element
            .0
            .attribute(&AXAttribute::new(&CFString::new("AXSize")))
        {
            unsafe {
                let value_ref = size.as_CFTypeRef();
                let mut cg_size: CGSize = CGSize {
                    width: 0.0,
                    height: 0.0,
                };
                let size_ptr = &mut cg_size as *mut CGSize as *mut ::std::os::raw::c_void;
                if AXValueGetValue(value_ref as *const _, K_AXVALUE_CGSIZE_TYPE, size_ptr) != 0 {
                    width = cg_size.width;
                    height = cg_size.height;
                }
            }
        }
        debug!(
            "Element bounds: x={}, y={}, width={}, height={}",
            x, y, width, height
        );
        Ok((x, y, width, height))
    }

    fn children(&self) -> Result<Vec<UIElement>, AutomationError> {
        debug!("Getting children for element: {:?}", self.element.0.role());
        let mut all_children = Vec::new();
        if let Ok(windows) = self.element.0.windows() {
            debug!("Found {} windows", windows.len());
            for window in windows.iter() {
                all_children.push(UIElement::new(Box::new(MacOSUIElement {
                    element: ThreadSafeAXUIElement::new(window.clone()),
                    use_background_apps: self.use_background_apps,
                    activate_app: self.activate_app,
                    cached_role: String::new(),
                    cached_label: None,
                    cached_description: None,
                    cached_value: None,
                })));
            }
        }
        if let Ok(window) = self.element.0.main_window() {
            debug!("Found main window");
            all_children.push(UIElement::new(Box::new(MacOSUIElement {
                element: ThreadSafeAXUIElement::new(window.clone()),
                use_background_apps: self.use_background_apps,
                activate_app: self.activate_app,
                cached_role: String::new(),
                cached_label: None,
                cached_description: None,
                cached_value: None,
            })));
        }
        match self.element.0.children() {
            Ok(children) => {
                for child in children.iter() {
                    all_children.push(UIElement::new(Box::new(MacOSUIElement {
                        element: ThreadSafeAXUIElement::new(child.clone()),
                        use_background_apps: self.use_background_apps,
                        activate_app: self.activate_app,
                        cached_role: String::new(),
                        cached_label: None,
                        cached_description: None,
                        cached_value: None,
                    })));
                }
                Ok(all_children)
            }
            Err(e) => {
                if !all_children.is_empty() {
                    debug!(
                        "Failed to get regular children but returning {} windows",
                        all_children.len()
                    );
                    Ok(all_children)
                } else {
                    Err(AutomationError::PlatformError(format!(
                        "Failed to get children: {}",
                        e
                    )))
                }
            }
        }
    }

    fn parent(&self) -> Result<Option<UIElement>, AutomationError> {
        let attr = AXAttribute::new(&CFString::new("AXParent"));
        match self.element.0.attribute(&attr) {
            Ok(value) => {
                if let Some(parent) = value.downcast::<AXUIElement>() {
                    Ok(Some(UIElement::new(Box::new(MacOSUIElement {
                        element: ThreadSafeAXUIElement::new(parent),
                        use_background_apps: self.use_background_apps,
                        activate_app: self.activate_app,
                        cached_role: String::new(),
                        cached_label: None,
                        cached_description: None,
                        cached_value: None,
                    }))))
                } else {
                    Ok(None)
                }
            }
            Err(_) => Ok(None),
        }
    }
}

/// Gets the focused UI element by first finding the frontmost application via NSWorkspace.
pub fn get_focused_element_ns_workspace(
    use_background_apps: bool,
    activate_app: bool,
) -> Result<UIElement, AutomationError> {
    debug!("Attempting to get focused element via NSWorkspace");
    unsafe {
        // 1. Get NSWorkspace shared instance
        let workspace_class = class!(NSWorkspace);
        let shared_workspace: *mut objc::runtime::Object =
            msg_send![workspace_class, sharedWorkspace];
        if shared_workspace.is_null() {
            return Err(AutomationError::PlatformError(
                "Failed to get shared NSWorkspace instance".to_string(),
            ));
        }

        // 2. Get the frontmost application
        let frontmost_app: *mut objc::runtime::Object = msg_send![shared_workspace, frontmostApplication];
        if frontmost_app.is_null() {
            debug!("NSWorkspace reported no frontmost application.");
            return Err(AutomationError::NoFocusedElement(
                "NSWorkspace reported no frontmost application".to_string(),
            ));
        }

        // 3. Get the PID of the frontmost application
        let pid: i32 = msg_send![frontmost_app, processIdentifier];
        if pid <= 0 {
            return Err(AutomationError::PlatformError(format!(
                "Failed to get PID for frontmost application: {:?}",
                frontmost_app
            )));
        }
        debug!("Frontmost application PID: {}", pid);

        // 4. Create an AXUIElement for the application PID
        let app_element_ref = accessibility::AXUIElement::application(pid);

        // 5. Get the focused UI element attribute from the application element
        let focused_element_attr_name = CFString::new("AXFocusedUIElement");
        let focused_element_attr = AXAttribute::new(&focused_element_attr_name);
        match app_element_ref.attribute(&focused_element_attr) {
            Ok(focused_element_cf) => {
                 if let Some(focused_element) = focused_element_cf.downcast::<AXUIElement>() {
                    debug!("Successfully found focused element via NSWorkspace->App->Focus");
                    Ok(UIElement::new(Box::new(MacOSUIElement {
                        element: ThreadSafeAXUIElement::new(focused_element),
                        use_background_apps,
                        activate_app,
                        cached_role: String::new(),
                        cached_label: None,
                        cached_description: None,
                        cached_value: None,
                    })))
                 } else {
                     debug!("AXFocusedUIElement attribute was not an AXUIElement for PID {}", pid);
                     Err(AutomationError::NoFocusedElement(format!(
                        "Application PID {} is frontmost, but has no focused UI element (or attribute type mismatch)",
                        pid
                    )))
                 }
            }
            Err(e) => {
                let error_msg = format!("Failed to get AXFocusedUIElement attribute for PID {}: {:?}", pid, e);
                 warn!("{}", error_msg);
                Err(AutomationError::NoFocusedElement(error_msg))
            }
        }
    }
}

impl UIElementImpl for MacOSUIElement {
    fn object_id(&self) -> usize {
        let stable_id = self.generate_stable_id();
        let mut hasher = DefaultHasher::new();
        stable_id.hash(&mut hasher);
        let id = hasher.finish() as usize;
        id
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn id(&self) -> Option<String> {
        Some(self.object_id().to_string())
    }

    fn role(&self) -> String {
        let mut role_to_use = self.cached_role.clone();
        if role_to_use.is_empty() {
            let role_attr = AXAttribute::new(&CFString::new("AXRole"));
            if let Ok(role_val) = self.element.0.attribute(&role_attr) {
                if let Some(cf_string) = role_val.downcast_into::<CFString>() {
                    role_to_use = cf_string.to_string();
                }
            }
            debug!("Role cache miss, fetched dynamically: {}", role_to_use);
        }

        macos_role_to_generic_role(&role_to_use)
            .first()
            .unwrap_or(&role_to_use)
            .to_string()
    }

    fn attributes(&self) -> UIElementAttributes {
        let properties = HashMap::new();

        let mut attrs = UIElementAttributes {
            role: self.role(),
            label: self.cached_label.clone().or_else(|| {
                let mut fetched_label = None;
                let title_attr = AXAttribute::new(&CFString::new("AXTitle"));
                if let Ok(title_val) = self.element.0.attribute(&title_attr) {
                    if let Some(cf_string) = title_val.downcast_into::<CFString>() {
                        let title_str = cf_string.to_string();
                        if !title_str.is_empty() {
                            fetched_label = Some(title_str);
                        }
                    }
                }
                if fetched_label.is_none() {
                    let label_attr = AXAttribute::new(&CFString::new("AXLabel"));
                    if let Ok(label_val) = self.element.0.attribute(&label_attr) {
                        if let Some(cf_string) = label_val.downcast_into::<CFString>() {
                            fetched_label = Some(cf_string.to_string());
                        }
                    }
                }
                debug!("Label cache miss, fetched dynamically: {:?}", fetched_label);
                fetched_label
            }),
            value: self.cached_value.clone().or_else(|| {
                let mut fetched_value = None;
                let value_attr = AXAttribute::new(&CFString::new("AXValue"));
                if let Ok(value_val) = self.element.0.attribute(&value_attr) {
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
                debug!("Value cache miss, fetched dynamically: {:?}", fetched_value);
                fetched_value
            }),
            description: self.cached_description.clone().or_else(|| {
                let mut fetched_description = None;
                let desc_attr = AXAttribute::new(&CFString::new("AXDescription"));
                if let Ok(desc_val) = self.element.0.attribute(&desc_attr) {
                    if let Some(cf_string) = desc_val.downcast_into::<CFString>() {
                        fetched_description = Some(cf_string.to_string());
                    }
                }
                debug!(
                    "Description cache miss, fetched dynamically: {:?}",
                    fetched_description
                );
                fetched_description
            }),
            properties,
        };

        // Define a list of potentially useful attributes to fetch if available
        let standard_attrs_to_fetch = [
            "AXURL",
            "AXDOMIdentifier",
            "AXEnabled",
            "AXFocused", // Note: AXFocused might be app-level, but check anyway
            "AXParent",
            "AXWindow",
            "AXTopLevelUIElement",
            "AXSelected",
            "AXPlaceholderValue", // Often useful for input fields
            "AXIdentifier",       // Standard UI element identifier
            "AXHelp",
            "AXFilename",  // For document-based apps/windows
            "AXDocument",  // For document URI/path
            "AXMain",      // Is it the main window?
            "AXMinimized", // Is the window minimized?
            "AXPosition",  // Already handled by bounds(), but might be useful raw
            "AXSize",      // Already handled by bounds(), but might be useful raw
        ];

        // Fetch attribute names only once
        if let Ok(attr_names_cf) = self.element.0.attribute_names() {
            // Convert CFStringRef array to Vec<String> for easier comparison
            let attr_names: Vec<String> = attr_names_cf.iter().map(|s| s.to_string()).collect();
            let available_attr_names: std::collections::HashSet<String> =
                attr_names.into_iter().collect();

            for name_str in standard_attrs_to_fetch {
                // Skip core attributes already handled (or attempted via cache)
                if !["AXRole", "AXTitle", "AXLabel", "AXDescription", "AXValue"].contains(&name_str)
                {
                    // Check if the attribute is listed as available by the element
                    if available_attr_names.contains(name_str) {
                        let attr = AXAttribute::new(&CFString::new(name_str));
                        match self.element.0.attribute(&attr) {
                            Ok(value) => {
                                let parsed_value = parse_ax_attribute_value(name_str, value);
                                attrs
                                    .properties
                                    .insert(name_str.to_string(), parsed_value.clone());
                                debug!("Fetched property '{}': {:?}", name_str, parsed_value);
                            }
                            Err(e) => {
                                // Log errors only if they are unexpected (not 'unsupported' or 'no value')
                                if !matches!(
                                    e,
                                    accessibility::Error::Ax(-25212) // attribute unsupported
                                        | accessibility::Error::Ax(-25205) // no value
                                        | accessibility::Error::Ax(-25204) // getting attribute failed (internal error)
                                ) {
                                    debug!(
                                        "Error getting property attribute '{}': {:?}",
                                        name_str, e
                                    );
                                }
                            }
                        }
                    } else {
                        // Optional: Log attributes that were in standard_attrs_to_fetch but not available
                        // trace!("Attribute '{}' from standard list is not available for this element.", name_str);
                    }
                }
            }
        } else {
            debug!("Failed to retrieve attribute names for element.");
        }

        attrs
    }

    fn children(&self) -> Result<Vec<UIElement>, AutomationError> {
        MacOSUIElement::children(self)
    }

    fn parent(&self) -> Result<Option<UIElement>, AutomationError> {
        MacOSUIElement::parent(self)
    }

    fn bounds(&self) -> Result<(f64, f64, f64, f64), AutomationError> {
        MacOSUIElement::bounds(self)
    }

    fn click(&self) -> Result<ClickResult, AutomationError> {
        interaction::click_with_method(self)
    }

    fn double_click(&self) -> Result<ClickResult, AutomationError> {
        let first_click = interaction::click_with_method(self)?;
        match interaction::click_with_method(self) {
            Ok(second_click) => Ok(ClickResult {
                method: second_click.method,
                coordinates: second_click.coordinates,
                details: format!(
                    "Double-click: First click: {}, Second click: {}",
                    first_click.details, second_click.details
                ),
            }),
            Err(e) => Err(e),
        }
    }

    fn right_click(&self) -> Result<(), AutomationError> {
        // Implementation adapted from interaction::click_mouse_simulation
        match self.bounds() {
            Ok((x, y, width, height)) => {
                let center_x = x + width / 2.0;
                let center_y = y + height / 2.0;
                let point = CGPoint::new(center_x, center_y);
                let source =
                    CGEventSource::new(CGEventSourceStateID::HIDSystemState).map_err(|_| {
                        AutomationError::PlatformError("Failed to create event source for right-click".to_string())
                    })?;

                // Optional: Move mouse first
                let mouse_move = CGEvent::new_mouse_event(
                    source.clone(),
                    CGEventType::MouseMoved,
                    point,
                    CGMouseButton::Right, // Button doesn't matter for move
                )
                .map_err(|_| {
                    AutomationError::PlatformError("Failed to create mouse move event for right-click".to_string())
                })?;
                mouse_move.post(CGEventTapLocation::HID);
                std::thread::sleep(std::time::Duration::from_millis(50)); // Small delay

                // Right Mouse Down
                debug!("Right mouse down at ({}, {})", center_x, center_y);
                let mouse_down = CGEvent::new_mouse_event(
                    source.clone(),
                    CGEventType::RightMouseDown,
                    point,
                    CGMouseButton::Right,
                )
                .map_err(|_| {
                    AutomationError::PlatformError("Failed to create right mouse down event".to_string())
                })?;
                mouse_down.post(CGEventTapLocation::HID);
                std::thread::sleep(std::time::Duration::from_millis(50)); // Small delay

                // Right Mouse Up
                debug!("Right mouse up at ({}, {})", center_x, center_y);
                let mouse_up = CGEvent::new_mouse_event(
                    source,
                    CGEventType::RightMouseUp,
                    point,
                    CGMouseButton::Right,
                )
                .map_err(|_| {
                    AutomationError::PlatformError("Failed to create right mouse up event".to_string())
                })?;
                mouse_up.post(CGEventTapLocation::HID);

                debug!(
                    "Performed simulated right mouse click at ({}, {})",
                    center_x, center_y
                );
                Ok(())
            }
            Err(e) => Err(AutomationError::PlatformError(format!(
                "Failed to determine element bounds for right-click: {}",
                e
            ))),
        }
    }

    fn hover(&self) -> Result<(), AutomationError> {
        // Implementation adapted from interaction::click_mouse_simulation
        match self.bounds() {
            Ok((x, y, width, height)) => {
                let center_x = x + width / 2.0;
                let center_y = y + height / 2.0;
                let point = CGPoint::new(center_x, center_y);
                let source =
                    CGEventSource::new(CGEventSourceStateID::HIDSystemState).map_err(|_| {
                        AutomationError::PlatformError("Failed to create event source for hover".to_string())
                    })?;

                // Mouse Move
                let mouse_move = CGEvent::new_mouse_event(
                    source,
                    CGEventType::MouseMoved,
                    point,
                    CGMouseButton::Left, // Button doesn't matter for move
                )
                .map_err(|_| {
                    AutomationError::PlatformError("Failed to create mouse move event for hover".to_string())
                })?;
                mouse_move.post(CGEventTapLocation::HID);
                 // Maybe a small delay is good practice even for hover
                std::thread::sleep(std::time::Duration::from_millis(20));

                debug!("Performed simulated hover at ({}, {})", center_x, center_y);
                Ok(())
            }
             Err(e) => Err(AutomationError::PlatformError(format!(
                "Failed to determine element bounds for hover: {}",
                e
            ))),
        }
    }

    fn focus(&self) -> Result<(), AutomationError> {
        interaction::focus(self)
    }

    fn type_text(&self, text: &str) -> Result<(), AutomationError> {
        interaction::type_text(self, text)
    }

    fn press_key(&self, key_combo: &str) -> Result<(), AutomationError> {
        interaction::press_key(self, key_combo)
    }

    fn get_text(&self, max_depth: usize) -> Result<String, AutomationError> {
        let collector = ElementsCollectorWithWindows::new(&self.element.0, |_| true)
            .with_limits(None, Some(max_depth));
        let elements = collector.find_all();
        let mut all_text: Vec<String> = Vec::new();
        for element in elements {
            for attr_name in &[
                "AXValue",
                "AXTitle",
                "AXDescription",
                "AXHelp",
                "AXLabel",
                "AXText",
            ] {
                let attr = AXAttribute::new(&CFString::new(attr_name));
                if let Ok(value) = element.attribute(&attr) {
                    if let Some(cf_string) = value.downcast_into::<CFString>() {
                        let text = cf_string.to_string();
                        if !text.is_empty() && !all_text.contains(&text) {
                            all_text.push(text);
                        }
                    }
                }
            }
        }
        Ok(all_text.join("\n"))
    }

    fn set_value(&self, value: &str) -> Result<(), AutomationError> {
        interaction::set_value(self, value)
    }

    fn is_enabled(&self) -> Result<bool, AutomationError> {
        let enabled_attr = AXAttribute::new(&CFString::new("AXEnabled"));
        match self.element.0.attribute(&enabled_attr) {
            Ok(value) => value.downcast_into::<core_foundation::boolean::CFBoolean>()
                              .map(|b| b == core_foundation::boolean::CFBoolean::true_value())
                              .ok_or_else(|| AutomationError::PlatformError("AXEnabled attribute was not a boolean".to_string())),
            Err(e) => {
                debug!("Failed to get AXEnabled attribute: {:?}, assuming disabled", e);
                Ok(false) // Often safer to assume disabled if attribute is missing/errors
            }
        }
    }

    fn is_visible(&self) -> Result<bool, AutomationError> {
        match self.bounds() {
            Ok((_, _, width, height)) => Ok(width > 0.0 && height > 0.0),
            Err(_) => Ok(false),
        }
    }

    fn is_focused(&self) -> Result<bool, AutomationError> {
        let focused_attr = AXAttribute::new(&CFString::new("AXFocused"));
         match self.element.0.attribute(&focused_attr) {
            Ok(value) => value.downcast_into::<core_foundation::boolean::CFBoolean>()
                              .map(|b| b == core_foundation::boolean::CFBoolean::true_value())
                              .ok_or_else(|| AutomationError::PlatformError("AXFocused attribute was not a boolean".to_string())),
            Err(e) => {
                debug!("Failed to get AXFocused attribute: {:?}, assuming not focused", e);
                Ok(false)
            }
        }
    }

    fn perform_action(&self, action: &str) -> Result<(), AutomationError> {
        let action_attr = AXAttribute::new(&CFString::new(action));
        self.element
            .0
            .perform_action(&action_attr.as_CFString())
            .map_err(|e| {
                AutomationError::PlatformError(format!(
                    "Failed to perform action {}: {}",
                    action, e
                ))
            })
    }

    fn create_locator(&self, selector: Selector) -> Result<Locator, AutomationError> {
        let engine = MacOSEngine::new(self.use_background_apps, self.activate_app)?;
        if self
            .element
            .0
            .role()
            .map_or(false, |r| r.to_string() == "AXApplication")
        {
            if let Some(app_name) = self.attributes().label {
                engine.refresh_accessibility_tree(Some(&app_name))?;
            }
        }
        let attrs = self.attributes();
        debug!(
            "Creating locator for element: role={}, label={:?}",
            attrs.role, attrs.label
        );
        let self_element = UIElement::new(self.clone_box());
        let locator = Locator::new(std::sync::Arc::new(engine), selector).within(self_element);
        Ok(locator)
    }

    fn clone_box(&self) -> Box<dyn UIElementImpl> {
        Box::new(MacOSUIElement {
            element: self.element.clone(),
            use_background_apps: self.use_background_apps,
            activate_app: self.activate_app,
            cached_role: self.cached_role.clone(),
            cached_label: self.cached_label.clone(),
            cached_description: self.cached_description.clone(),
            cached_value: self.cached_value.clone(),
        })
    }

    fn scroll(&self, direction: &str, amount: f64) -> Result<(), AutomationError> {
        // Use the shared implementation from interaction module
        interaction::scroll(self, direction, amount)
    }

    fn get_all_attributes(&self) -> Result<UIElementAttributes, AutomationError> {
        let mut attrs = self.attributes(); // Start with basic attributes (role, label, value, description)

        // Explicitly fetch key attributes and add to properties
        if let Ok(enabled) = self.is_enabled() {
            attrs.properties.insert("enabled".to_string(), Some(serde_json::Value::Bool(enabled)));
        }
        if let Ok(focused) = self.is_focused() {
             attrs.properties.insert("focused".to_string(), Some(serde_json::Value::Bool(focused)));
        }
        if let Ok((x, y, w, h)) = self.bounds() {
            attrs.properties.insert("bounds_x".to_string(), Some(serde_json::json!(x)));
            attrs.properties.insert("bounds_y".to_string(), Some(serde_json::json!(y)));
            attrs.properties.insert("bounds_width".to_string(), Some(serde_json::json!(w)));
            attrs.properties.insert("bounds_height".to_string(), Some(serde_json::json!(h)));
        }
        if let Some(id) = self.id() { // Use the existing id() method which checks AXIdentifier
             attrs.properties.insert("identifier".to_string(), Some(serde_json::Value::String(id)));
        }

        // Helper function to fetch and parse specific attributes
        let mut fetch_and_insert = |key: &str, ax_attr_name: &str| {
            let attr = AXAttribute::new(&CFString::new(ax_attr_name));
            match self.element.0.attribute(&attr) {
                Ok(value) => {
                    let parsed = parse_ax_attribute_value(ax_attr_name, value);
                     if parsed.is_some() { // Only insert if parsing was successful
                        attrs.properties.insert(key.to_string(), parsed);
                    }
                }
                Err(_) => { /* Ignore errors for optional attributes */ }
            }
        };

        fetch_and_insert("placeholder", "AXPlaceholderValue");
        fetch_and_insert("selected", "AXSelected");
        // Note: AXChecked is often part of AXValue on checkboxes/radio buttons,
        // but we can try fetching it directly too.
        fetch_and_insert("checked", "AXChecked");


        // Fetch remaining attributes dynamically (optional, keep if desired)
        match self.element.0.attribute_names() {
            Ok(attr_names_cf) => {
                let attr_names: Vec<String> = attr_names_cf.iter().map(|s| s.to_string()).collect();
                debug!(element_role = %attrs.role, label = ?attrs.label, "Dynamically fetching {} other attributes", attr_names.len());

                let explicitly_handled = [
                    "AXRole", "AXTitle", "AXLabel", "AXDescription", "AXValue", // Basic handled by self.attributes()
                    "AXPosition", "AXSize", // Handled by self.bounds()
                    "AXEnabled", "AXFocused", "AXIdentifier", // Explicitly handled above
                    "AXPlaceholderValue", "AXSelected", "AXChecked" // Explicitly handled above
                ];

                for name_str in attr_names {
                    // Skip attributes already handled explicitly or by basic fetch
                    let key_to_insert = name_str.strip_prefix("AX").unwrap_or(&name_str).to_string(); // Use cleaner key
                    if !explicitly_handled.contains(&name_str.as_str()) && !attrs.properties.contains_key(&key_to_insert) {
                        let attr = AXAttribute::new(&CFString::new(&name_str));
                        match self.element.0.attribute(&attr) {
                            Ok(value) => {
                                let parsed_value = parse_ax_attribute_value(&name_str, value);
                                attrs.properties.insert(key_to_insert, parsed_value);
                            }
                            Err(e) => {
                                // Log errors only if they are unexpected
                                if !matches!(
                                    e,
                                    accessibility::Error::Ax(-25212) // attribute unsupported
                                        | accessibility::Error::Ax(-25205) // no value
                                        | accessibility::Error::Ax(-25204) // getting attribute failed
                                ) {
                                    debug!("Error getting dynamic property attribute '{}': {:?}", name_str, e);
                                }
                            }
                        }
                    }
                }
            }
            Err(e) => {
                warn!("Failed to retrieve attribute names for dynamic fetch: {:?}", e);
            }
        }

        Ok(attrs)
    }

    fn screenshot(&self) -> Result<String, AutomationError> {
        // Call the utility function
        super::utils::capture_element_screenshot(self)
    }

    fn select_text(&self) -> Result<(), AutomationError> {
        interaction::select_text(self)
    }

    fn get_tree(&self) -> Result<ElementTreeNode, AutomationError> {
        let attributes = self.attributes();
        let children = self.children().unwrap_or_default();
        let child_nodes = children
            .into_iter()
            .filter_map(|child| child.get_tree().ok()) // Recursively get tree for children, ignore errors
            .collect();

        Ok(ElementTreeNode {
            role: attributes.role,
            label: attributes.label,
            description: attributes.description,
            bounds: self.bounds().ok(), // Get bounds, ignore errors for the tree
            children: child_nodes,
        })
    }
}

#[cfg(test)]
mod tests {

    // We will add tests here later.

    // Example placeholder test
    #[test]
    fn test_placeholder() {
        assert_eq!(2 + 2, 4);
    }
}
