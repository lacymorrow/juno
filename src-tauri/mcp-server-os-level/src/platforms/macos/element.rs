use super::actions::ClickMethodSelection;
use super::constants::*;
use super::engine::MacOSEngine;
use super::ffi::AXValueGetValue;
use super::interaction;
use super::utils::macos_role_to_generic_role;
use super::wrappers::ThreadSafeAXUIElement;
use crate::platforms::macos::attributes::get_element_attributes;
use crate::platforms::tree_search::ElementsCollectorWithWindows;
use crate::UIElementAttributes;
use crate::{element::UIElementImpl, AutomationError, ClickResult, Locator, Selector, UIElement};
use accessibility::{AXAttribute, AXUIElement, AXUIElementAttributes as AXAttrsTrait};
use anyhow::Result;
use core_foundation::base::TCFType;
use core_foundation::string::CFString;
use core_graphics::event::{CGEvent, CGEventTapLocation};
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
use core_graphics::geometry::{CGPoint, CGSize};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use tracing::debug;

#[derive(Debug)]
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
                    }))))
                } else {
                    Ok(None)
                }
            }
            Err(_) => Ok(None),
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
        get_element_attributes(self)
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
        interaction::click_with_method(self, ClickMethodSelection::Auto)
    }

    fn double_click(&self) -> Result<ClickResult, AutomationError> {
        let first_click = interaction::click_with_method(self, ClickMethodSelection::Auto)?;
        match interaction::click_with_method(self, ClickMethodSelection::Auto) {
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
        Err(AutomationError::UnsupportedOperation(
            "Right-click not yet implemented for macOS".to_string(),
        ))
    }

    fn hover(&self) -> Result<(), AutomationError> {
        Err(AutomationError::UnsupportedOperation(
            "Hover not yet implemented for macOS".to_string(),
        ))
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
        Err(AutomationError::UnsupportedOperation(
            "is_enabled not yet implemented for macOS".to_string(),
        ))
    }

    fn is_visible(&self) -> Result<bool, AutomationError> {
        match self.bounds() {
            Ok((_, _, width, height)) => Ok(width > 0.0 && height > 0.0),
            Err(_) => Ok(false),
        }
    }

    fn is_focused(&self) -> Result<bool, AutomationError> {
        Err(AutomationError::UnsupportedOperation(
            "is_focused not yet implemented for macOS".to_string(),
        ))
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
        })
    }

    fn scroll(&self, direction: &str, amount: f64) -> Result<(), AutomationError> {
        let _ = self.focus();
        let (x, y, width, height) = self.bounds()?;
        let center_x = x + width / 2.0;
        let center_y = y + height / 2.0;
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
        let scroll_event =
            CGEvent::new_scroll_event(source, 0, 1, scroll_y, scroll_x, 0).map_err(|_| {
                AutomationError::PlatformError("Failed to create scroll event".to_string())
            })?;
        scroll_event.post(CGEventTapLocation::HID);
        debug!(
            "scrolled {} by {} lines at position ({}, {})",
            direction, amount, center_x, center_y
        );
        Ok(())
    }
}
