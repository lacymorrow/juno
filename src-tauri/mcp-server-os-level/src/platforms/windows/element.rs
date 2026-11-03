//! The UI Automation client, and one element of another app's window.

use super::ax::{ax_role, remember_field};
use super::input;
use crate::element::UIElementImpl;
use crate::ClickResult;
use crate::{AutomationError, ElementTreeNode, Locator, Selector, UIElement, UIElementAttributes};
use serde_json::json;
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::fmt::Debug;
use std::hash::{Hash, Hasher};
use std::sync::OnceLock;
use uiautomation::patterns::{UITextPattern, UIValuePattern};
use uiautomation::types::ControlType;
use uiautomation::UIAutomation;
use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};

/// Deep enough for a WebView2 or Electron page, which sits 20 to 40 levels
/// below its window. Same reasoning as the macOS engine's cap.
pub(super) const MAX_TREE_DEPTH: usize = 60;

/// A tree read stops here, so one huge window can't run for minutes.
pub(super) const MAX_TREE_NODES: usize = 5_000;

pub(super) fn uia_error(context: &str, e: uiautomation::Error) -> AutomationError {
    AutomationError::PlatformError(format!("{context}: {e}"))
}

thread_local! {
    // UI Automation is COM, and COM has to be initialised on every thread that
    // calls it. The result is ignored on purpose: a thread that is already in
    // a single-threaded apartment (a GUI thread) returns RPC_E_CHANGED_MODE,
    // and UI Automation works there too.
    static COM_READY: () = {
        // SAFETY: plain COM initialisation for the calling thread, with no
        // pointers passed in.
        let _ = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
    };
}

pub fn ensure_com() {
    COM_READY.with(|_| {});
}

/// The UI Automation client.
#[derive(Clone)]
pub struct Uia {
    pub(super) automation: UIAutomation,
}

// SAFETY: the CUIAutomation client object and the element proxies it hands out
// are free-threaded: they aggregate the free-threaded marshaler and are meant
// to be called from any thread in the multithreaded apartment. `ensure_com`
// runs before every call that reaches them.
unsafe impl Send for Uia {}
unsafe impl Sync for Uia {}

static UIA: OnceLock<Uia> = OnceLock::new();

/// The process's one client, started on first use. A failure isn't kept: COM
/// can recover between calls, and the next caller gets a fresh try.
pub fn uia() -> Result<Uia, AutomationError> {
    ensure_com();
    if let Some(uia) = UIA.get() {
        return Ok(uia.clone());
    }
    let automation =
        UIAutomation::new_direct().map_err(|e| uia_error("Couldn't start UI Automation", e))?;
    Ok(UIA.get_or_init(|| Uia { automation }).clone())
}

pub(super) fn build_tree(
    element: &WindowsUIElement,
    depth: usize,
    node_count: &mut usize,
) -> ElementTreeNode {
    *node_count += 1;
    let mut children = Vec::new();
    if depth < MAX_TREE_DEPTH {
        for child in element.child_elements() {
            if *node_count >= MAX_TREE_NODES {
                break;
            }
            children.push(build_tree(&child, depth + 1, node_count));
        }
    }
    ElementTreeNode {
        attributes: element.attributes(),
        children,
    }
}

#[derive(Clone)]
pub struct WindowsUIElement {
    pub(super) uia: Uia,
    pub(super) element: uiautomation::UIElement,
}

// SAFETY: see `Uia`. An element is a proxy to a free-threaded COM object.
unsafe impl Send for WindowsUIElement {}
unsafe impl Sync for WindowsUIElement {}

impl Debug for WindowsUIElement {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WindowsUIElement")
            .field("role", &self.role())
            .field("name", &self.element.get_name().unwrap_or_default())
            .finish()
    }
}

impl WindowsUIElement {
    pub(super) fn new(uia: Uia, element: uiautomation::UIElement) -> Self {
        Self { uia, element }
    }

    pub(super) fn child_elements(&self) -> Vec<WindowsUIElement> {
        ensure_com();
        let Ok(walker) = self.uia.automation.get_control_view_walker() else {
            return Vec::new();
        };
        walker
            .get_children(&self.element)
            .unwrap_or_default()
            .into_iter()
            .map(|e| WindowsUIElement::new(self.uia.clone(), e))
            .collect()
    }

    pub(super) fn parent_element(&self) -> Option<WindowsUIElement> {
        ensure_com();
        let walker = self.uia.automation.get_control_view_walker().ok()?;
        walker
            .get_parent(&self.element)
            .ok()
            .map(|e| WindowsUIElement::new(self.uia.clone(), e))
    }

    fn value(&self) -> Option<String> {
        if self.element.is_password().unwrap_or(false) {
            return None;
        }
        self.element
            .get_pattern::<UIValuePattern>()
            .and_then(|p| p.get_value())
            .ok()
            .filter(|v| !v.is_empty())
    }

    /// Where a real click on this element would go.
    fn center(&self) -> Result<(f64, f64), AutomationError> {
        let (x, y, w, h) = self.bounds()?;
        if w <= 0.0 || h <= 0.0 {
            return Err(AutomationError::ZeroElementDimensions {
                role: self.role(),
                label: self.element.get_name().unwrap_or_default(),
                x,
                y,
                width: w,
                height: h,
            });
        }
        Ok((x + w / 2.0, y + h / 2.0))
    }
}

impl UIElementImpl for WindowsUIElement {
    fn object_id(&self) -> usize {
        let mut hasher = DefaultHasher::new();
        self.element
            .get_runtime_id()
            .unwrap_or_default()
            .hash(&mut hasher);
        hasher.finish() as usize
    }

    fn id(&self) -> Option<String> {
        self.element
            .get_automation_id()
            .ok()
            .filter(|id| !id.is_empty())
    }

    fn role(&self) -> String {
        self.element
            .get_control_type()
            .map(ax_role)
            .unwrap_or("AXUnknown")
            .to_string()
    }

    fn attributes(&self) -> UIElementAttributes {
        ensure_com();
        let mut properties = HashMap::new();
        let text = |s: String| (!s.is_empty()).then(|| json!(s));
        properties.insert(
            "automation_id".to_string(),
            self.element.get_automation_id().ok().and_then(text),
        );
        properties.insert(
            "class_name".to_string(),
            self.element.get_classname().ok().and_then(text),
        );
        properties.insert(
            "framework_id".to_string(),
            self.element.get_framework_id().ok().and_then(text),
        );
        properties.insert(
            "process_id".to_string(),
            self.element.get_process_id().ok().map(|pid| json!(pid)),
        );
        properties.insert(
            "bounds".to_string(),
            self.bounds().ok().map(|(x, y, w, h)| json!([x, y, w, h])),
        );
        properties.insert(
            "enabled".to_string(),
            self.element.is_enabled().ok().map(|b| json!(b)),
        );
        properties.insert(
            "offscreen".to_string(),
            self.element.is_offscreen().ok().map(|b| json!(b)),
        );
        UIElementAttributes {
            role: self.role(),
            label: self.element.get_name().ok().filter(|s| !s.is_empty()),
            value: self.value(),
            description: self.element.get_help_text().ok().filter(|s| !s.is_empty()),
            properties,
        }
    }

    fn children(&self) -> Result<Vec<UIElement>, AutomationError> {
        Ok(self
            .child_elements()
            .into_iter()
            .map(|e| UIElement::new(Box::new(e)))
            .collect())
    }

    fn parent(&self) -> Result<Option<UIElement>, AutomationError> {
        Ok(self.parent_element().map(|e| UIElement::new(Box::new(e))))
    }

    fn bounds(&self) -> Result<(f64, f64, f64, f64), AutomationError> {
        let rect = self
            .element
            .get_bounding_rectangle()
            .map_err(|e| uia_error("Couldn't read the bounds", e))?;
        Ok((
            f64::from(rect.get_left()),
            f64::from(rect.get_top()),
            f64::from(rect.get_width()),
            f64::from(rect.get_height()),
        ))
    }

    fn click(&self) -> Result<ClickResult, AutomationError> {
        let method = self.press()?;
        Ok(ClickResult {
            method: method.to_string(),
            coordinates: None,
            details: "Pressed without an input event; the pointer didn't move".to_string(),
        })
    }

    fn double_click(&self) -> Result<ClickResult, AutomationError> {
        let method = self.open()?;
        Ok(ClickResult {
            method: method.to_string(),
            coordinates: None,
            details: "Opened without an input event; the pointer didn't move".to_string(),
        })
    }

    fn right_click(&self) -> Result<(), AutomationError> {
        ensure_com();
        self.element
            .show_context_menu()
            .map_err(|e| uia_error("That control has no menu to open", e))
    }

    fn hover(&self) -> Result<(), AutomationError> {
        Err(AutomationError::UnsupportedOperation(
            "Hovering needs the real cursor, and Juno leaves that with you".to_string(),
        ))
    }

    /// Remembered rather than focused: UI Automation's `SetFocus` brings the
    /// whole app forward (see `ax::focus`).
    fn focus(&self) -> Result<(), AutomationError> {
        ensure_com();
        remember_field(self);
        Ok(())
    }

    fn type_text(&self, text: &str) -> Result<(), AutomationError> {
        ensure_com();
        let pattern = self.element.get_pattern::<UIValuePattern>().map_err(|_| {
            AutomationError::UnsupportedOperation(
                "That control doesn't take typed text".to_string(),
            )
        })?;
        let before = pattern.get_value().unwrap_or_default();
        pattern
            .set_value(&format!("{before}{text}"))
            .map_err(|e| uia_error("Couldn't set the value", e))
    }

    fn press_key(&self, key: &str) -> Result<(), AutomationError> {
        ensure_com();
        remember_field(self);
        input::key_no_warp(&input::parse_key(key)?, false)?.ok_or_else(|| {
            AutomationError::UnsupportedOperation(
                "No window has been targeted for that key".to_string(),
            )
        })?;
        Ok(())
    }

    fn get_text(&self, max_depth: usize) -> Result<String, AutomationError> {
        fn collect(element: &WindowsUIElement, depth: usize, max: usize, out: &mut Vec<String>) {
            if let Ok(name) = element.element.get_name() {
                if !name.is_empty() {
                    out.push(name);
                }
            }
            if let Some(value) = element.value() {
                out.push(value);
            }
            if depth < max {
                for child in element.child_elements() {
                    collect(&child, depth + 1, max, out);
                }
            }
        }
        ensure_com();
        let mut out = Vec::new();
        collect(self, 0, max_depth, &mut out);
        Ok(out.join("\n"))
    }

    fn set_value(&self, value: &str) -> Result<(), AutomationError> {
        ensure_com();
        let pattern = self.element.get_pattern::<UIValuePattern>().map_err(|_| {
            AutomationError::UnsupportedOperation("This element has no Value pattern".to_string())
        })?;
        pattern
            .set_value(value)
            .map_err(|e| uia_error("Couldn't set the value", e))
    }

    fn is_enabled(&self) -> Result<bool, AutomationError> {
        self.element
            .is_enabled()
            .map_err(|e| uia_error("Couldn't read enabled", e))
    }

    fn is_visible(&self) -> Result<bool, AutomationError> {
        self.element
            .is_offscreen()
            .map(|offscreen| !offscreen)
            .map_err(|e| uia_error("Couldn't read visibility", e))
    }

    fn is_focused(&self) -> Result<bool, AutomationError> {
        self.element
            .has_keyboard_focus()
            .map_err(|e| uia_error("Couldn't read focus", e))
    }

    fn perform_action(&self, action: &str) -> Result<(), AutomationError> {
        match action.to_lowercase().as_str() {
            "press" | "invoke" | "click" | "axpress" => self.press().map(|_| ()),
            "axshowmenu" | "showmenu" => self.right_click(),
            "focus" => self.focus(),
            other => Err(AutomationError::UnsupportedOperation(format!(
                "The '{other}' action isn't available on Windows"
            ))),
        }
    }

    fn create_locator(&self, _selector: Selector) -> Result<Locator, AutomationError> {
        Err(AutomationError::UnsupportedOperation(
            "A locator scoped to an element isn't available on Windows yet".to_string(),
        ))
    }

    fn scroll(&self, direction: &str, amount: f64) -> Result<(), AutomationError> {
        let (center_x, center_y) = self.center()?;
        let target = super::window_list::root_of(
            self.element
                .get_native_window_handle()
                .ok()
                .filter(|h| !h.is_invalid())
                .map(Into::into)
                .ok_or_else(|| {
                    AutomationError::UnsupportedOperation(
                        "That control has no window to scroll".to_string(),
                    )
                })?,
        );
        super::ax::scroll_at(target, center_x, center_y, direction, amount)
            .map_err(AutomationError::UnsupportedOperation)
    }

    fn screenshot(&self) -> Result<String, AutomationError> {
        Err(AutomationError::UnsupportedOperation(
            "A picture of one control isn't available on Windows; take one of its window"
                .to_string(),
        ))
    }

    fn select_text(&self) -> Result<(), AutomationError> {
        ensure_com();
        let pattern = self.element.get_pattern::<UITextPattern>().map_err(|_| {
            AutomationError::UnsupportedOperation("That control has no selectable text".to_string())
        })?;
        pattern
            .get_document_range()
            .and_then(|range| range.select())
            .map_err(|e| uia_error("Couldn't select the text", e))
    }

    fn get_all_attributes(&self) -> Result<UIElementAttributes, AutomationError> {
        Ok(self.attributes())
    }

    fn get_tree(&self) -> Result<ElementTreeNode, AutomationError> {
        let mut node_count = 0;
        Ok(build_tree(self, 0, &mut node_count))
    }

    fn clone_box(&self) -> Box<dyn UIElementImpl> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// UI Automation control types for the role names the selectors use. The names
/// are matched without case, so `Button`, `button` and the macOS-style
/// `textfield` all work.
pub(super) fn control_type_from_role(role: &str) -> Option<ControlType> {
    let role = role.to_lowercase();
    let role = role.strip_prefix("ax").unwrap_or(&role);
    Some(match role {
        "button" => ControlType::Button,
        "checkbox" => ControlType::CheckBox,
        "radiobutton" => ControlType::RadioButton,
        "combobox" => ControlType::ComboBox,
        "edit" | "textfield" | "input" => ControlType::Edit,
        "hyperlink" | "link" => ControlType::Hyperlink,
        "list" => ControlType::List,
        "listitem" | "row" => ControlType::ListItem,
        "menu" => ControlType::Menu,
        "menubar" => ControlType::MenuBar,
        "menuitem" => ControlType::MenuItem,
        "tabgroup" => ControlType::Tab,
        "tabitem" | "tab" => ControlType::TabItem,
        "text" | "statictext" => ControlType::Text,
        "tree" | "outline" => ControlType::Tree,
        "treeitem" => ControlType::TreeItem,
        "window" => ControlType::Window,
        "pane" => ControlType::Pane,
        "group" => ControlType::Group,
        "document" | "textarea" => ControlType::Document,
        "dataitem" => ControlType::DataItem,
        "slider" => ControlType::Slider,
        _ => return None,
    })
}
