//! Windows engine, built on UI Automation.
//!
//! This is the spike for LAC-6321 (docs/plans/windows.md). It does enough to
//! find an element in a window that isn't in front, press it with the Invoke
//! pattern, and read values back, all without moving the cursor or changing
//! the foreground window. Everything that would need real input (mouse,
//! keyboard, scrolling) returns `UnsupportedPlatform` until the port gets there.

use crate::element::UIElementImpl;
use crate::platforms::AccessibilityEngine;
use crate::ClickResult;
use crate::{AutomationError, ElementTreeNode, Locator, Selector, UIElement, UIElementAttributes};
use serde_json::{json, Value as JsonValue};
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::fmt::Debug;
use std::hash::{Hash, Hasher};
use std::time::{Duration, Instant};
use uiautomation::core::UICondition;
use uiautomation::patterns::{
    UIInvokePattern, UISelectionItemPattern, UITogglePattern, UIValuePattern,
};
use uiautomation::types::{ControlType, TreeScope, UIProperty};
use uiautomation::variants::Variant;
use uiautomation::UIAutomation;
use windows::Win32::Foundation::POINT;
use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};
use windows::Win32::UI::WindowsAndMessaging::{GetCursorPos, GetForegroundWindow, GetWindowTextW};

/// Deep enough for a WebView2 or Electron page, which sits 20 to 40 levels
/// below its window. Same reasoning as the macOS engine's cap.
const MAX_TREE_DEPTH: usize = 60;

/// A tree read stops here, so one huge window can't run for minutes.
const MAX_TREE_NODES: usize = 5_000;

fn unsupported<T>(what: &str) -> Result<T, AutomationError> {
    Err(AutomationError::UnsupportedPlatform(format!(
        "{what} isn't implemented on Windows yet"
    )))
}

fn uia_error(context: &str, e: uiautomation::Error) -> AutomationError {
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

fn ensure_com() {
    COM_READY.with(|_| {});
}

/// The UI Automation client and the desktop root it was opened on.
#[derive(Clone)]
struct Uia {
    automation: UIAutomation,
}

// SAFETY: the CUIAutomation client object and the element proxies it hands out
// are free-threaded: they aggregate the free-threaded marshaler and are meant
// to be called from any thread in the multithreaded apartment. `ensure_com`
// runs before every call that reaches them.
unsafe impl Send for Uia {}
unsafe impl Sync for Uia {}

pub struct WindowsEngine {
    uia: Uia,
    root: WindowsUIElement,
}

impl WindowsEngine {
    pub fn new(_use_background_apps: bool, _activate_app: bool) -> Result<Self, AutomationError> {
        ensure_com();
        let automation =
            UIAutomation::new_direct().map_err(|e| uia_error("Couldn't start UI Automation", e))?;
        let root = automation
            .get_root_element()
            .map_err(|e| uia_error("Couldn't read the desktop root", e))?;
        let uia = Uia { automation };
        Ok(Self {
            root: WindowsUIElement::new(uia.clone(), root),
            uia,
        })
    }

    fn wrap(&self, element: uiautomation::UIElement) -> UIElement {
        UIElement::new(Box::new(WindowsUIElement::new(self.uia.clone(), element)))
    }

    fn inner<'a>(&'a self, element: Option<&'a UIElement>) -> &'a uiautomation::UIElement {
        element
            .and_then(|e| e.as_any().downcast_ref::<WindowsUIElement>())
            .map(|e| &e.element)
            .unwrap_or(&self.root.element)
    }

    /// The UI Automation condition for one selector. A chain is handled by the
    /// callers, one link at a time.
    fn condition(&self, selector: &Selector) -> Result<UICondition, AutomationError> {
        let automation = &self.uia.automation;
        let property = |property: UIProperty, value: Variant| {
            automation
                .create_property_condition(property, value, None)
                .map_err(|e| uia_error("Couldn't build a search condition", e))
        };
        match selector {
            Selector::Id(id) => property(UIProperty::AutomationId, Variant::from(id.as_str())),
            // UI Automation has no separate text property: a label is the Name.
            Selector::Name(name) | Selector::Text(name) => {
                property(UIProperty::Name, Variant::from(name.as_str()))
            }
            Selector::Role { role, name } => {
                let control_type = control_type_from_role(role).ok_or_else(|| {
                    AutomationError::UnsupportedSelector(format!(
                        "No UI Automation control type for role '{role}'"
                    ))
                })?;
                let by_type =
                    property(UIProperty::ControlType, Variant::from(control_type as i32))?;
                match name {
                    Some(name) => {
                        let by_name = property(UIProperty::Name, Variant::from(name.as_str()))?;
                        automation
                            .create_and_condition(by_type, by_name)
                            .map_err(|e| uia_error("Couldn't build a search condition", e))
                    }
                    None => Ok(by_type),
                }
            }
            other => Err(AutomationError::UnsupportedSelector(format!(
                "{other:?} isn't supported on Windows yet"
            ))),
        }
    }

    fn find_first_in(
        &self,
        selector: &Selector,
        root: &uiautomation::UIElement,
    ) -> Result<uiautomation::UIElement, AutomationError> {
        if let Selector::Chain(links) = selector {
            let mut current = root.clone();
            for link in links {
                current = self.find_first_in(link, &current)?;
            }
            return Ok(current);
        }
        let condition = self.condition(selector)?;
        root.find_first(TreeScope::Descendants, &condition)
            .map_err(|_| AutomationError::ElementNotFound(format!("{selector:?}")))
    }

    /// The top-level window whose title contains `name`, or whose window class
    /// is `name`. Matching by title is enough for the spike: Explorer's title
    /// is the folder name and its class is `CabinetWClass`.
    fn top_level_window(&self, name: &str) -> Result<uiautomation::UIElement, AutomationError> {
        let wanted = name.to_lowercase();
        let walker = self
            .uia
            .automation
            .get_control_view_walker()
            .map_err(|e| uia_error("Couldn't walk the desktop", e))?;
        let windows = walker.get_children(&self.root.element).unwrap_or_default();
        windows
            .into_iter()
            .find(|window| {
                let title = window.get_name().unwrap_or_default().to_lowercase();
                let class = window.get_classname().unwrap_or_default().to_lowercase();
                title.contains(&wanted) || class == wanted
            })
            .ok_or_else(|| AutomationError::ElementNotFound(format!("No window matching '{name}'")))
    }
}

impl AccessibilityEngine for WindowsEngine {
    fn get_root_element(&self) -> UIElement {
        UIElement::new(Box::new(self.root.clone()))
    }

    fn get_element_by_id(&self, id: &str) -> Result<UIElement, AutomationError> {
        ensure_com();
        self.find_first_in(&Selector::Id(id.to_string()), &self.root.element)
            .map(|e| self.wrap(e))
    }

    fn get_focused_element(&self) -> Result<UIElement, AutomationError> {
        ensure_com();
        self.uia
            .automation
            .get_focused_element()
            .map(|e| self.wrap(e))
            .map_err(|e| AutomationError::NoFocusedElement(e.to_string()))
    }

    fn get_applications(&self) -> Result<Vec<UIElement>, AutomationError> {
        ensure_com();
        let walker = self
            .uia
            .automation
            .get_control_view_walker()
            .map_err(|e| uia_error("Couldn't walk the desktop", e))?;
        Ok(walker
            .get_children(&self.root.element)
            .unwrap_or_default()
            .into_iter()
            .map(|e| self.wrap(e))
            .collect())
    }

    fn get_application_by_name(&self, name: &str) -> Result<UIElement, AutomationError> {
        ensure_com();
        self.top_level_window(name).map(|e| self.wrap(e))
    }

    fn find_element(
        &self,
        selector: &Selector,
        root: Option<&UIElement>,
    ) -> Result<UIElement, AutomationError> {
        ensure_com();
        self.find_first_in(selector, self.inner(root))
            .map(|e| self.wrap(e))
    }

    fn find_elements(
        &self,
        selector: &Selector,
        root: Option<&UIElement>,
    ) -> Result<Vec<UIElement>, AutomationError> {
        ensure_com();
        let (scope_root, last) = match selector {
            Selector::Chain(links) => match links.split_last() {
                Some((last, rest)) => (
                    self.find_first_in(&Selector::Chain(rest.to_vec()), self.inner(root))?,
                    last,
                ),
                None => return Ok(Vec::new()),
            },
            _ => (self.inner(root).clone(), selector),
        };
        let condition = self.condition(last)?;
        let found = scope_root
            .find_all(TreeScope::Descendants, &condition)
            .unwrap_or_default();
        Ok(found.into_iter().map(|e| self.wrap(e)).collect())
    }

    fn open_application(&self, _app_name: &str) -> Result<UIElement, AutomationError> {
        unsupported("Opening an application")
    }

    fn open_url(&self, _url: &str, _browser: Option<&str>) -> Result<UIElement, AutomationError> {
        unsupported("Opening a URL")
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn scroll_at_position(
        &self,
        _x: f64,
        _y: f64,
        _direction: &str,
        _amount: f64,
    ) -> Result<(), AutomationError> {
        unsupported("Scrolling")
    }

    fn scroll_at_current_position(
        &self,
        _direction: &str,
        _amount: f64,
    ) -> Result<(), AutomationError> {
        unsupported("Scrolling")
    }

    fn type_text(&self, _text: &str) -> Result<(), AutomationError> {
        unsupported("Typing")
    }

    fn get_clipboard_content(&self) -> Result<String, AutomationError> {
        unsupported("Reading the clipboard")
    }

    fn set_clipboard_content(&self, _content: &str) -> Result<(), AutomationError> {
        unsupported("Writing the clipboard")
    }

    fn hold_key(&self, _key: &str, _duration_ms: Option<u64>) -> Result<(), AutomationError> {
        unsupported("Holding a key")
    }

    fn release_key(&self, _key: &str) -> Result<(), AutomationError> {
        unsupported("Releasing a key")
    }

    fn wait(&self, duration_ms: u64) -> Result<(), AutomationError> {
        std::thread::sleep(Duration::from_millis(duration_ms));
        Ok(())
    }

    fn press_key(&self, _key_name: &str, _modifier: Option<&str>) -> Result<(), AutomationError> {
        unsupported("Pressing a key")
    }

    /// The tree of one window, or of the foreground window when no name is
    /// given, with how long the read took. The timing is what the spike
    /// measures for Explorer and Electron.
    fn get_ui_tree(&self, app_name: Option<&str>) -> Result<JsonValue, AutomationError> {
        ensure_com();
        let window = match app_name {
            Some(name) => self.top_level_window(name)?,
            None => {
                // SAFETY: no arguments; returns a null handle when nothing is
                // in front, which UI Automation rejects with an error.
                let hwnd = unsafe { GetForegroundWindow() };
                self.uia
                    .automation
                    .element_from_handle(hwnd.into())
                    .map_err(|e| uia_error("Couldn't read the foreground window", e))?
            }
        };
        let started = Instant::now();
        let mut node_count = 0;
        let tree = build_tree(
            &WindowsUIElement::new(self.uia.clone(), window),
            0,
            &mut node_count,
        );
        let elapsed_ms = started.elapsed().as_millis();
        let tree = serde_json::to_value(tree)
            .map_err(|e| AutomationError::Internal(format!("Couldn't serialise the tree: {e}")))?;
        Ok(json!({
            "elapsed_ms": elapsed_ms,
            "node_count": node_count,
            "truncated": node_count >= MAX_TREE_NODES,
            "tree": tree,
        }))
    }

    fn cursor_position(&self) -> Result<(f64, f64), AutomationError> {
        let mut point = POINT::default();
        // SAFETY: `point` is a valid, writable POINT for the duration of the call.
        unsafe { GetCursorPos(&mut point) }.map_err(|e| {
            AutomationError::PlatformError(format!("Couldn't read the cursor position: {e}"))
        })?;
        Ok((f64::from(point.x), f64::from(point.y)))
    }

    fn mouse_move(&self, _x: f64, _y: f64) -> Result<(), AutomationError> {
        unsupported("Moving the mouse")
    }

    fn left_mouse_down(&self, _x: f64, _y: f64) -> Result<(), AutomationError> {
        unsupported("Mouse input")
    }

    fn left_mouse_up(&self, _x: f64, _y: f64) -> Result<(), AutomationError> {
        unsupported("Mouse input")
    }

    fn left_click(
        &self,
        _x: f64,
        _y: f64,
        _modifiers: Option<&str>,
    ) -> Result<(), AutomationError> {
        unsupported("Clicking at a point")
    }

    fn right_click(
        &self,
        _x: f64,
        _y: f64,
        _modifiers: Option<&str>,
    ) -> Result<(), AutomationError> {
        unsupported("Clicking at a point")
    }

    fn middle_click(
        &self,
        _x: f64,
        _y: f64,
        _modifiers: Option<&str>,
    ) -> Result<(), AutomationError> {
        unsupported("Clicking at a point")
    }

    fn double_click(
        &self,
        _x: f64,
        _y: f64,
        _modifiers: Option<&str>,
    ) -> Result<(), AutomationError> {
        unsupported("Clicking at a point")
    }

    fn triple_click(
        &self,
        _x: f64,
        _y: f64,
        _modifiers: Option<&str>,
    ) -> Result<(), AutomationError> {
        unsupported("Clicking at a point")
    }

    fn left_click_drag(
        &self,
        _start_x: f64,
        _start_y: f64,
        _end_x: f64,
        _end_y: f64,
    ) -> Result<(), AutomationError> {
        unsupported("Dragging")
    }

    fn get_window_title(&self) -> Result<String, AutomationError> {
        let mut buffer = [0u16; 512];
        // SAFETY: `GetForegroundWindow` takes no arguments, and `buffer` is a
        // valid, writable slice whose length is passed with it.
        let length = unsafe { GetWindowTextW(GetForegroundWindow(), &mut buffer) };
        let length = usize::try_from(length).unwrap_or(0).min(buffer.len());
        Ok(String::from_utf16_lossy(&buffer[..length]))
    }

    fn list_windows(&self) -> Result<Vec<UIElement>, AutomationError> {
        self.get_applications()
    }

    fn close_window(&self) -> Result<(), AutomationError> {
        unsupported("Closing a window")
    }

    fn maximize_window(&self) -> Result<(), AutomationError> {
        unsupported("Maximizing a window")
    }

    fn minimize_window(&self) -> Result<(), AutomationError> {
        unsupported("Minimizing a window")
    }

    fn resize_window(&self, _width: f64, _height: f64) -> Result<(), AutomationError> {
        unsupported("Resizing a window")
    }

    fn move_window(&self, _x: f64, _y: f64) -> Result<(), AutomationError> {
        unsupported("Moving a window")
    }

    fn get_element_tree(&self, element: &UIElement) -> Result<ElementTreeNode, AutomationError> {
        element.get_tree()
    }
}

/// UI Automation control types for the role names the selectors use. The
/// names are matched without case, so `Button`, `button` and the macOS-style
/// `textfield` all work.
fn control_type_from_role(role: &str) -> Option<ControlType> {
    let role = role.to_lowercase();
    let role = role.strip_prefix("ax").unwrap_or(&role);
    Some(match role {
        "button" => ControlType::Button,
        "checkbox" => ControlType::CheckBox,
        "combobox" => ControlType::ComboBox,
        "edit" | "textfield" | "input" | "textarea" => ControlType::Edit,
        "hyperlink" | "link" => ControlType::Hyperlink,
        "list" => ControlType::List,
        "listitem" => ControlType::ListItem,
        "menu" => ControlType::Menu,
        "menubar" => ControlType::MenuBar,
        "menuitem" => ControlType::MenuItem,
        "tab" => ControlType::Tab,
        "tabitem" => ControlType::TabItem,
        "text" | "statictext" => ControlType::Text,
        "tree" => ControlType::Tree,
        "treeitem" => ControlType::TreeItem,
        "window" => ControlType::Window,
        "pane" => ControlType::Pane,
        "group" => ControlType::Group,
        "document" => ControlType::Document,
        "dataitem" => ControlType::DataItem,
        _ => return None,
    })
}

fn build_tree(element: &WindowsUIElement, depth: usize, node_count: &mut usize) -> ElementTreeNode {
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
    uia: Uia,
    element: uiautomation::UIElement,
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
    fn new(uia: Uia, element: uiautomation::UIElement) -> Self {
        Self { uia, element }
    }

    fn child_elements(&self) -> Vec<WindowsUIElement> {
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

    fn value(&self) -> Option<String> {
        self.element
            .get_pattern::<UIValuePattern>()
            .and_then(|p| p.get_value())
            .ok()
            .filter(|v| !v.is_empty())
    }

    /// Press the element the way a screen reader would, through a control
    /// pattern, so no input event is generated and nothing moves.
    fn press(&self) -> Result<&'static str, AutomationError> {
        ensure_com();
        if let Ok(invoke) = self.element.get_pattern::<UIInvokePattern>() {
            invoke.invoke().map_err(|e| uia_error("Invoke failed", e))?;
            return Ok("UIA Invoke");
        }
        if let Ok(toggle) = self.element.get_pattern::<UITogglePattern>() {
            toggle.toggle().map_err(|e| uia_error("Toggle failed", e))?;
            return Ok("UIA Toggle");
        }
        if let Ok(item) = self.element.get_pattern::<UISelectionItemPattern>() {
            item.select().map_err(|e| uia_error("Select failed", e))?;
            return Ok("UIA SelectionItem");
        }
        Err(AutomationError::UnsupportedOperation(format!(
            "'{}' has no Invoke, Toggle or SelectionItem pattern, and pressing it would need real input",
            self.element.get_name().unwrap_or_default()
        )))
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
            .map(|t| format!("{t:?}"))
            .unwrap_or_else(|_| "Unknown".to_string())
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
        ensure_com();
        let walker = self
            .uia
            .automation
            .get_control_view_walker()
            .map_err(|e| uia_error("Couldn't walk the tree", e))?;
        Ok(walker
            .get_parent(&self.element)
            .ok()
            .map(|e| UIElement::new(Box::new(WindowsUIElement::new(self.uia.clone(), e)))))
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
            details: "Pressed through UI Automation; no input event was sent".to_string(),
        })
    }

    fn double_click(&self) -> Result<ClickResult, AutomationError> {
        unsupported("Double-clicking an element")
    }

    fn right_click(&self) -> Result<(), AutomationError> {
        unsupported("Right-clicking an element")
    }

    fn hover(&self) -> Result<(), AutomationError> {
        unsupported("Hovering")
    }

    fn focus(&self) -> Result<(), AutomationError> {
        ensure_com();
        self.element
            .set_focus()
            .map_err(|e| uia_error("Couldn't focus the element", e))
    }

    fn type_text(&self, _text: &str) -> Result<(), AutomationError> {
        unsupported("Typing into an element")
    }

    fn press_key(&self, _key: &str) -> Result<(), AutomationError> {
        unsupported("Pressing a key on an element")
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
            "focus" => self.focus(),
            other => unsupported(&format!("The '{other}' action")),
        }
    }

    fn create_locator(&self, _selector: Selector) -> Result<Locator, AutomationError> {
        unsupported("A locator scoped to an element")
    }

    fn scroll(&self, _direction: &str, _amount: f64) -> Result<(), AutomationError> {
        unsupported("Scrolling an element")
    }

    fn screenshot(&self) -> Result<String, AutomationError> {
        unsupported("An element screenshot")
    }

    fn select_text(&self) -> Result<(), AutomationError> {
        unsupported("Selecting text")
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
