use crate::element::UIElementImpl;
use super::utils::{macos_role_to_generic_role, parse_ax_attribute_value};
use crate::platforms::tree_search::ElementsCollectorWithWindows;
use super::engine::MacOSEngine;
use super::ffi::AXValueGetValue;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::collections::hash_map::DefaultHasher;
use accessibility::AXAttribute;
use accessibility::AXUIElement;
use core_foundation::base::CFString;
use core_foundation::boolean::CFBoolean;
use core_foundation::string::CFStringRef;
use core_graphics::event::CGEvent;
use core_graphics::event_source::CGEventSource;
use core_graphics::event_source::CGEventSourceStateID;
use core_graphics::event::CGEventTapLocation;
use core_graphics::event::CGKeyCode;
use core_graphics::geometry::CGPoint;
use core_graphics::geometry::CGSize;
use serde_json;
use crate::element::{UIElement, UIElementAttributes, ClickResult, ClickMethodSelection, AutomationError, Locator, Selector};
use crate::platforms::macos::thread_safe_ax_ui_element::ThreadSafeAXUIElement;
use super::actions::ClickMethodSelection;
use super::constants::*;
use super::ffi::{AXUIElementSetAttributeValue};
use super::wrappers::ThreadSafeAXUIElement;
use crate::{
    AutomationError,
    ClickResult,
    UIElement,
    UIElementAttributes,
    Locator,
    Selector,
    element::UIElementImpl,
};
use crate::platforms::macos::utils::{macos_role_to_generic_role, parse_ax_attribute_value};
use crate::platforms::tree_search::ElementsCollectorWithWindows;
use super::engine::MacOSEngine;
use accessibility::{AXAttribute, AXUIElement};
use core_foundation::base::TCFType;
use core_foundation::string::CFString;
use core_graphics::event::{CGEventType, CGMouseButton, CGEventTapLocation, CGEvent, CGEventFlags, CGKeyCode};
use core_graphics::event_source::{CGEventSourceStateID, CGEventSource};
use core_graphics::geometry::{CGPoint, CGSize};
use tracing::debug;

pub struct MacOSUIElement {
    pub(crate) element: ThreadSafeAXUIElement,
    pub(crate) use_background_apps: bool,
    pub(crate) activate_app: bool,
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

    fn bounds(&self) -> Result<(f64, f64, f64, f64), AutomationError> {
        let mut x = 0.0;
        let mut y = 0.0;
        let mut width = 0.0;
        let mut height = 0.0;

        if let Ok(position) = self.element.0.attribute(&AXAttribute::new(&CFString::new("AXPosition"))) {
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

        if let Ok(size) = self.element.0.attribute(&AXAttribute::new(&CFString::new("AXSize"))) {
            unsafe {
                let value_ref = size.as_CFTypeRef();
                let mut cg_size: CGSize = CGSize { width: 0.0, height: 0.0 };
                let size_ptr = &mut cg_size as *mut CGSize as *mut ::std::os::raw::c_void;
                if AXValueGetValue(value_ref as *const _, K_AXVALUE_CGSIZE_TYPE, size_ptr) != 0 {
                    width = cg_size.width;
                    height = cg_size.height;
                }
            }
        }
        debug!("Element bounds: x={}, y={}, width={}, height={}", x, y, width, height);
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
                })));
            }
        }
        if let Ok(window) = self.element.0.main_window() {
            debug!("Found main window");
            all_children.push(UIElement::new(Box::new(MacOSUIElement {
                element: ThreadSafeAXUIElement::new(window.clone()),
                use_background_apps: self.use_background_apps,
                activate_app: self.activate_app,
            })));
        }
        match self.element.0.children() {
            Ok(children) => {
                for child in children.iter() {
                    all_children.push(UIElement::new(Box::new(MacOSUIElement {
                        element: ThreadSafeAXUIElement::new(child.clone()),
                        use_background_apps: self.use_background_apps,
                        activate_app: self.activate_app,
                    })));
                }
                Ok(all_children)
            }
            Err(e) => {
                if !all_children.is_empty() {
                    debug!("Failed to get regular children but returning {} windows", all_children.len());
                    Ok(all_children)
                } else {
                    Err(AutomationError::PlatformError(format!("Failed to get children: {}", e)))
                }
            }
        }
    }

    fn parent(&self) -> Result<Option<UIElement>, AutomationError>{
        let attr = AXAttribute::new(&CFString::new("AXParent"));
        match self.element.0.attribute(&attr) {
            Ok(value) => {
                if let Some(parent) = value.downcast::<AXUIElement>() {
                    Ok(Some(UIElement::new(Box::new(MacOSUIElement {
                        element: ThreadSafeAXUIElement::new(parent),
                        use_background_apps: self.use_background_apps,
                        activate_app: self.activate_app,
                    }))))
                } else {
                    Ok(None)
                }
            }
            Err(_) => Ok(None),
        }
    }

    pub(crate) fn get_application(&self) -> Option<MacOSUIElement> {
        let attr = AXAttribute::new(&CFString::new("AXTopLevelUIElement"));
        match self.element.0.attribute(&attr) {
            Ok(value) => {
                if let Some(app) = value.downcast::<AXUIElement>() {
                    Some(MacOSUIElement {
                        element: ThreadSafeAXUIElement::new(app),
                        use_background_apps: self.use_background_apps,
                        activate_app: self.activate_app,
                    })
                } else {
                    None
                }
            }
            Err(_) => None,
        }
    }

    pub(crate) fn click_with_method(
        &self,
        method: ClickMethodSelection,
    ) -> Result<ClickResult, AutomationError> {
        match method {
            ClickMethodSelection::Auto => self.click_auto(),
            ClickMethodSelection::AXPress => self.click_press(),
            ClickMethodSelection::AXClick => self.click_accessibility_click(),
            ClickMethodSelection::MouseSimulation => self.click_mouse_simulation(),
        }
    }

    fn click_auto(&self) -> Result<ClickResult, AutomationError> {
        if let Some(app) = self.get_application() {
            let app_attributes = app.attributes();
            let app_name = app_attributes.label.unwrap_or_default().to_lowercase();
            debug!("detected application: {}", app_name);
            if app_name.contains("chrome") || app_name.contains("safari") || app_name.contains("arc") ||
               app_name.contains("firefox") || app_name.contains("edge") || app_name.contains("brave") ||
               app_name.contains("opera") || app_name.contains("vivaldi") || app_name.contains("microsoft edge") {
                debug!("browser detected, using mouse simulation directly");
                return self.click_mouse_simulation();
            }
        }
        match self.click_press() {
            Ok(result) => return Ok(result),
            Err(e) => debug!("AXPress failed: {:?}, trying alternative methods", e),
        }
        match self.click_accessibility_click() {
            Ok(result) => return Ok(result),
            Err(e) => debug!("AXClick failed: {:?}, trying alternative methods", e),
        }
        self.click_mouse_simulation()
    }

    fn click_press(&self) -> Result<ClickResult, AutomationError> {
        let press_attr = AXAttribute::new(&CFString::new("AXPress"));
        match self.element.0.perform_action(&press_attr.as_CFString()) {
            Ok(_) => {
                debug!("Successfully clicked element with AXPress");
                Ok(ClickResult {
                    method: "AXPress".to_string(),
                    coordinates: None,
                    details: "Used accessibility AXPress action".to_string(),
                })
            }
            Err(e) => Err(AutomationError::PlatformError(format!("AXPress click failed: {:?}", e))),
        }
    }

    fn click_accessibility_click(&self) -> Result<ClickResult, AutomationError> {
        let click_attr = AXAttribute::new(&CFString::new("AXClick"));
        match self.element.0.perform_action(&click_attr.as_CFString()) {
            Ok(_) => {
                debug!("Successfully clicked element with AXClick");
                Ok(ClickResult {
                    method: "AXClick".to_string(),
                    coordinates: None,
                    details: "Used accessibility AXClick action".to_string(),
                })
            }
            Err(e) => Err(AutomationError::PlatformError(format!("AXClick click failed: {:?}", e))),
        }
    }

    fn click_mouse_simulation(&self) -> Result<ClickResult, AutomationError> {
        match self.bounds() {
            Ok((x, y, width, height)) => {
                let center_x = x + width / 2.0;
                let center_y = y + height / 2.0;
                let point = CGPoint::new(center_x, center_y);
                let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
                    .map_err(|_| AutomationError::PlatformError("Failed to create event source".to_string()))?;

                let mouse_move = CGEvent::new_mouse_event(source.clone(), CGEventType::MouseMoved, point, CGMouseButton::Left)
                    .map_err(|_| AutomationError::PlatformError("Failed to create mouse move event".to_string()))?;
                mouse_move.post(CGEventTapLocation::HID);
                std::thread::sleep(std::time::Duration::from_millis(50));

                debug!("Mouse down at ({}, {})", center_x, center_y);
                let mouse_down = CGEvent::new_mouse_event(source.clone(), CGEventType::LeftMouseDown, point, CGMouseButton::Left)
                    .map_err(|_| AutomationError::PlatformError("Failed to create mouse down event".to_string()))?;
                mouse_down.post(CGEventTapLocation::HID);
                std::thread::sleep(std::time::Duration::from_millis(50));

                debug!("Mouse up at ({}, {})", center_x, center_y);
                let mouse_up = CGEvent::new_mouse_event(source, CGEventType::LeftMouseUp, point, CGMouseButton::Left)
                    .map_err(|_| AutomationError::PlatformError("Failed to create mouse up event".to_string()))?;
                mouse_up.post(CGEventTapLocation::HID);

                debug!("Performed simulated mouse click at ({}, {})", center_x, center_y);
                Ok(ClickResult {
                    method: "MouseSimulation".to_string(),
                    coordinates: Some((center_x, center_y)),
                    details: format!("Used mouse simulation at coordinates ({:.1}, {:.1}), element bounds: ({:.1}, {:.1}, {:.1}, {:.1})", center_x, center_y, x, y, width, height),
                })
            }
            Err(e) => Err(AutomationError::PlatformError(format!("Failed to determine element bounds for click: {}", e))),
        }
    }

    fn get_key_code(&self, key: &str) -> Result<u16, AutomationError> {
        let key_map: HashMap<&str, u16> = [
            ("return", KEY_RETURN), ("enter", KEY_RETURN), ("tab", KEY_TAB), ("space", KEY_SPACE),
            ("delete", KEY_DELETE), ("backspace", KEY_DELETE), ("esc", KEY_ESCAPE), ("escape", KEY_ESCAPE),
            ("left", KEY_ARROW_LEFT), ("right", KEY_ARROW_RIGHT), ("down", KEY_ARROW_DOWN), ("up", KEY_ARROW_UP),
        ].iter().cloned().collect();
        key_map.get(key.to_lowercase().as_str()).copied()
            .ok_or_else(|| AutomationError::InvalidArgument(format!("Unknown key: {}", key)))
    }

    pub(crate) fn parse_key_combination(&self, key_combo: &str) -> Result<(u16, CGEventFlags), AutomationError> {
        let parts: Vec<String> = key_combo.split('+').map(|s| s.trim().to_lowercase()).collect();
        if parts.is_empty() {
            return Err(AutomationError::InvalidArgument("Empty key combination".to_string()));
        }
        let key = &parts[parts.len() - 1];
        let key_code = self.get_key_code(key)?;
        let mut flags = CGEventFlags::empty();
        for modifier in &parts[0..parts.len() - 1] {
            match modifier.as_str() {
                "cmd" | "command" => flags.insert(MODIFIER_COMMAND),
                "shift" => flags.insert(MODIFIER_SHIFT),
                "alt" | "option" => flags.insert(MODIFIER_OPTION),
                "ctrl" | "control" => flags.insert(MODIFIER_CONTROL),
                "fn" => flags.insert(MODIFIER_FN),
                _ => return Err(AutomationError::InvalidArgument(format!("Unknown modifier: {}", modifier)))
            }
        }
        Ok((key_code, flags))
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
        let role = self
            .element
            .0
            .role()
            .map(|r| r.to_string())
            .unwrap_or_default();

        macos_role_to_generic_role(&role)
            .first()
            .unwrap_or(&role)
            .to_string()
    }

    fn attributes(&self) -> UIElementAttributes {
        let mut properties = HashMap::new();
        let is_window = self.element.0.role().map_or(false, |r| r.to_string() == "AXWindow");

        if is_window {
            let mut attrs = UIElementAttributes {
                role: "window".to_string(),
                label: None,
                value: None,
                description: None,
                properties,
            };

            let title_attrs = [
                "AXTitle",
                "AXTitleUIElement",
                "AXDocument",
                "AXFilename",
                "AXName",
            ];

            for title_attr_name in title_attrs {
                let title_attr = AXAttribute::new(&CFString::new(title_attr_name));
                if let Ok(value) = self.element.0.attribute(&title_attr) {
                    if let Some(cf_string) = value.downcast_into::<CFString>() {
                        attrs.label = Some(cf_string.to_string());
                        break;
                    }
                }
            }

            let pos_attr = AXAttribute::new(&CFString::new("AXPosition"));
            if let Ok(_) = self.element.0.attribute(&pos_attr) {
            }

            let std_attrs = ["AXMinimized", "AXMain", "AXFocused"];

            for attr_name in std_attrs {
                let attr = AXAttribute::new(&CFString::new(attr_name));
                if let Ok(value) = self.element.0.attribute(&attr) {
                    if let Some(cf_bool) = value.downcast_into::<core_foundation::boolean::CFBoolean>() {
                        attrs.properties.insert(
                            attr_name.to_string(),
                            Some(serde_json::Value::String(format!("{:?}", cf_bool))),
                        );
                    }
                }
            }

            return attrs;
        }

        let mut attrs = UIElementAttributes {
            role: self.role(),
            label: None,
            value: None,
            description: None,
            properties,
        };

        let label_attr = AXAttribute::new(&CFString::new("AXTitle"));
        match self.element.0.attribute(&label_attr) {
            Ok(value) => {
                if let Some(cf_string) = value.downcast_into::<CFString>() {
                    attrs.label = Some(cf_string.to_string());
                }
            }
            Err(_e) => {
                let alt_label_attr = AXAttribute::new(&CFString::new("AXLabel"));
                if let Ok(value) = self.element.0.attribute(&alt_label_attr) {
                    if let Some(cf_string) = value.downcast_into::<CFString>() {
                        attrs.label = Some(cf_string.to_string());
                    }
                }
            }
        }

        let desc_attr = AXAttribute::new(&CFString::new("AXDescription"));
        match self.element.0.attribute(&desc_attr) {
            Ok(value) => {
                if let Some(cf_string) = value.downcast_into::<CFString>() {
                    attrs.description = Some(cf_string.to_string());
                }
            }
            Err(_e) => {
            }
        }

        if let Ok(attr_names) = self.element.0.attribute_names() {
            for name in attr_names.iter() {
                let attr = AXAttribute::new(&name);
                match self.element.0.attribute(&attr) {
                    Ok(value) => {
                        let parsed_value = parse_ax_attribute_value(&name.to_string(), value);
                        attrs.properties.insert(name.to_string(), parsed_value);
                    }
                    Err(e) => {
                        if !matches!(
                            e,
                            accessibility::Error::Ax(-25212)
                                | accessibility::Error::Ax(-25205)
                                | accessibility::Error::Ax(-25204)
                        ) {
                        }
                    }
                }
            }
        } else {
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
        self.click_with_method(ClickMethodSelection::Auto)
    }

    fn double_click(&self) -> Result<ClickResult, AutomationError> {
        let first_click = self.click()?;
        match self.click() {
            Ok(second_click) => Ok(ClickResult { method: second_click.method, coordinates: second_click.coordinates, details: format!("Double-click: First click: {}, Second click: {}", first_click.details, second_click.details) }),
            Err(e) => Err(e),
        }
    }

    fn right_click(&self) -> Result<(), AutomationError> {
        Err(AutomationError::UnsupportedOperation("Right-click not yet implemented for macOS".to_string()))
    }

    fn hover(&self) -> Result<(), AutomationError> {
        Err(AutomationError::UnsupportedOperation("Hover not yet implemented for macOS".to_string()))
    }

    fn focus(&self) -> Result<(), AutomationError> {
        let raise_attr = AXAttribute::new(&CFString::new("AXRaise"));
        if let Ok(_) = self.element.0.perform_action(&raise_attr.as_CFString()) {
            debug!("Successfully raised element");
            if let Some(app) = self.get_application() {
                unsafe {
                    let app_ref = app.element.0.as_concrete_TypeRef() as *mut ::std::os::raw::c_void;
                    let attr_str = CFString::new("AXFocusedUIElement");
                    let attr_str_ref = attr_str.as_concrete_TypeRef() as *const ::std::os::raw::c_void;
                    let elem_ref = self.element.0.as_concrete_TypeRef() as *const ::std::os::raw::c_void;
                    let result = AXUIElementSetAttributeValue(app_ref, attr_str_ref, elem_ref);
                    if result == 0 { debug!("Successfully set focus to element"); return Ok(()); }
                    else { debug!("Failed to set element as focused: error code {}", result); }
                }
            }
        }
        debug!("Attempting to focus by clicking the element");
        self.click().map(|_result| { debug!("Focus achieved via click method: {}", _result.method); () })
    }

    fn type_text(&self, text: &str) -> Result<(), AutomationError> {
        match self.focus() {
            Ok(_) => debug!("Successfully focused element for typing"),
            Err(e) => {
                debug!("Focus failed, but continuing with type_text: {:?}", e);
                if let Err(click_err) = self.click() { debug!("Click also failed: {:?}", click_err); }
            }
        }
        let is_web_input = { let role = self.role().to_lowercase(); role.contains("web") || role.contains("generic") };
        if is_web_input {
            debug!("Detected web input, using specialized handling");
            for attr_name in &["AXValue", "AXValueAttribute", "AXText"] {
                let cf_string = CFString::new(text);
                unsafe {
                    let element_ref = self.element.0.as_concrete_TypeRef() as *mut ::std::os::raw::c_void;
                    let attr_str = CFString::new(attr_name);
                    let attr_str_ref = attr_str.as_concrete_TypeRef() as *const ::std::os::raw::c_void;
                    let value_ref = cf_string.as_concrete_TypeRef() as *const ::std::os::raw::c_void;
                    let result = AXUIElementSetAttributeValue(element_ref, attr_str_ref, value_ref);
                    if result == 0 { debug!("Successfully set text using {}", attr_name); return Ok(()); }
                }
            }
        }
        let cf_string = CFString::new(text);
        unsafe {
            let element_ref = self.element.0.as_concrete_TypeRef() as *mut ::std::os::raw::c_void;
            let attr_str = CFString::new("AXValue");
            let attr_str_ref = attr_str.as_concrete_TypeRef() as *const ::std::os::raw::c_void;
            let value_ref = cf_string.as_concrete_TypeRef() as *const ::std::os::raw::c_void;
            let result = AXUIElementSetAttributeValue(element_ref, attr_str_ref, value_ref);
            if result != 0 {
                debug!("Failed to set text value via AXValue: error code {}", result);
                return Err(AutomationError::PlatformError(format!("Failed to set text: error code {}", result)));
            }
            debug!("Successfully set text value via AXValue (standard approach)");
        }
        Ok(())
    }

    fn press_key(&self, key_combo: &str) -> Result<(), AutomationError> {
        debug!("Pressing key combination: {}", key_combo);
        let element_role = self.role();
        let element_label = self.attributes().label.unwrap_or_default();
        match self.focus() {
            Ok(_) => debug!("successfully focused element for key press"),
            Err(e) => {
                let error_msg = format!("key press aborted - failed to focus {} element '{}' before pressing '{}': {}", element_role, element_label, key_combo, e);
                debug!("{}", error_msg);
                return Err(AutomationError::PlatformError(error_msg));
            }
        }
        let (key_code, flags) = self.parse_key_combination(key_combo)?;
        let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
            .map_err(|_| AutomationError::PlatformError("Failed to create event source".to_string()))?;

        let key_down = CGEvent::new_keyboard_event(source.clone(), key_code as CGKeyCode, true)
            .map_err(|_| AutomationError::PlatformError("Failed to create key down event".to_string()))?;
        if !flags.is_empty() { key_down.set_flags(flags); }
        key_down.post(CGEventTapLocation::HID);

        std::thread::sleep(std::time::Duration::from_millis(50));

        let key_up = CGEvent::new_keyboard_event(source, key_code as CGKeyCode, false)
            .map_err(|_| AutomationError::PlatformError("Failed to create key up event".to_string()))?;
        if !flags.is_empty() { key_up.set_flags(flags); }
        key_up.post(CGEventTapLocation::HID);

        debug!("Successfully pressed key combination: {}", key_combo);
        Ok(())
    }

    fn get_text(&self, max_depth: usize) -> Result<String, AutomationError> {
        let collector = ElementsCollectorWithWindows::new(&self.element.0, |_| true).with_limits(None, Some(max_depth));
        let elements = collector.find_all();
        let mut all_text: Vec<String> = Vec::new();
        for element in elements {
            for attr_name in &["AXValue", "AXTitle", "AXDescription", "AXHelp", "AXLabel", "AXText"] {
                let attr = AXAttribute::new(&CFString::new(attr_name));
                if let Ok(value) = element.attribute(&attr) {
                    if let Some(cf_string) = value.downcast_into::<CFString>() {
                        let text = cf_string.to_string();
                        if !text.is_empty() && !all_text.contains(&text) { all_text.push(text); }
                    }
                }
            }
        }
        Ok(all_text.join("\n"))
    }

    fn set_value(&self, value: &str) -> Result<(), AutomationError> {
        let cf_string = CFString::new(value);
        unsafe {
            let element_ref = self.element.0.as_concrete_TypeRef() as *mut ::std::os::raw::c_void;
            let attr_str = CFString::new("AXValue");
            let attr_str_ref = attr_str.as_concrete_TypeRef() as *const ::std::os::raw::c_void;
            let value_ref = cf_string.as_concrete_TypeRef() as *const ::std::os::raw::c_void;
            let result = AXUIElementSetAttributeValue(element_ref, attr_str_ref, value_ref);
            if result != 0 {
                debug!("Failed to set value via AXValue: error code {}", result);
                return Err(AutomationError::PlatformError(format!("Failed to set value: error code {}", result)));
            }
        }
        Ok(())
    }

    fn is_enabled(&self) -> Result<bool, AutomationError> {
        Err(AutomationError::UnsupportedOperation("is_enabled not yet implemented for macOS".to_string()))
    }

    fn is_visible(&self) -> Result<bool, AutomationError> {
        match self.bounds() {
            Ok((_, _, width, height)) => Ok(width > 0.0 && height > 0.0),
            Err(_) => Ok(false),
        }
    }

    fn is_focused(&self) -> Result<bool, AutomationError> {
        Err(AutomationError::UnsupportedOperation("is_focused not yet implemented for macOS".to_string()))
    }

    fn perform_action(&self, action: &str) -> Result<(), AutomationError> {
        let action_attr = AXAttribute::new(&CFString::new(action));
        self.element.0.perform_action(&action_attr.as_CFString())
            .map_err(|e| AutomationError::PlatformError(format!("Failed to perform action {}: {}", action, e)))
    }

    fn create_locator(&self, selector: Selector) -> Result<Locator, AutomationError> {
        let engine = MacOSEngine::new(self.use_background_apps, self.activate_app)?;
        if self.element.0.role().map_or(false, |r| r.to_string() == "AXApplication") {
            if let Some(app_name) = self.attributes().label {
                engine.refresh_accessibility_tree(Some(&app_name))?;
            }
        }
        let attrs = self.attributes();
        debug!("Creating locator for element: role={}, label={:?}", attrs.role, attrs.label);
        let self_element = UIElement::new(self.clone_box());
        let locator = Locator::new(std::sync::Arc::new(engine), selector).within(self_element);
        Ok(locator)
    }

    fn clone_box(&self) -> Box<dyn UIElementImpl> {
        Box::new(MacOSUIElement {
            element: self.element.clone(),
            use_background_apps: self.use_background_apps,
            activate_app: self.activate_app,
        })
    }

    fn scroll(&self, direction: &str, amount: f64) -> Result<(), AutomationError> {
        let _ = self.focus();
        let (x, y, width, height) = self.bounds()?;
        let center_x = x + width / 2.0; let center_y = y + height / 2.0;
        let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
            .map_err(|_| AutomationError::PlatformError("Failed to create event source".to_string()))?;
        let scroll_amount = amount as i32;
        let (scroll_x, scroll_y) = match direction.to_lowercase().as_str() {
            "up" => (0, -scroll_amount), "down" => (0, scroll_amount),
            "left" => (-scroll_amount, 0), "right" => (scroll_amount, 0),
            _ => return Err(AutomationError::InvalidArgument(format!("Invalid scroll direction: {}. Must be up, down, left, or right", direction)))
        };
        let scroll_event = CGEvent::new_scroll_event(source, 0, 1, scroll_y, scroll_x, 0)
            .map_err(|_| AutomationError::PlatformError("Failed to create scroll event".to_string()))?;
        scroll_event.post(CGEventTapLocation::HID);
        debug!("scrolled {} by {} lines at position ({}, {})", direction, amount, center_x, center_y);
        Ok(())
    }
}

