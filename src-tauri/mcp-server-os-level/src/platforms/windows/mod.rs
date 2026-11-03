//! Windows engine, built on UI Automation (LAC-6321, docs/plans/windows.md).
//!
//! It works without taking the mouse or the foreground wherever Windows allows
//! that, and says so when it doesn't:
//!
//! - `ax`: a window's controls, read and pressed through UI Automation.
//! - `input`: posted window messages, then `SendInput` as the last resort.
//! - `capture`: pictures of one window, covered or not, and of a display.
//! - `window_list`: windows, displays and processes.
//!
//! An elevated window can't be driven by a process that isn't, and Windows
//! gives no error when input to one is dropped. Every path checks first and
//! fails with `AutomationError::ElevatedWindow`.

pub mod ax;
pub mod capture;
pub mod element;
pub mod input;
pub mod window_list;

use crate::input_tier::InputOutcome;
use crate::platforms::AccessibilityEngine;
use crate::{AutomationError, ElementTreeNode, Selector, UIElement};
use element::{
    build_tree, control_type_from_role, ensure_com, uia, uia_error, Uia, WindowsUIElement,
    MAX_TREE_NODES,
};
use input::Button;
use serde_json::{json, Value as JsonValue};
use std::time::{Duration, Instant};
use uiautomation::core::UICondition;
use uiautomation::types::{Point, TreeScope, UIProperty};
use uiautomation::variants::Variant;
use window_list::{
    foreground_window, hwnd_from_id, list_window_records, root_of, top_window_of_pid,
};
use windows::core::{HSTRING, PCWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::HiDpi::{
    SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
};
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::{
    PostMessageW, SetWindowPos, ShowWindow, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER,
    SW_MAXIMIZE, SW_MINIMIZE, SW_SHOWNOACTIVATE, WM_CLOSE,
};

/// How long a just-launched app gets to put up a window.
const APP_LAUNCH_DEADLINE: Duration = Duration::from_secs(5);

pub struct WindowsEngine {
    uia: Uia,
    root: WindowsUIElement,
}

impl WindowsEngine {
    pub fn new(_use_background_apps: bool, _activate_app: bool) -> Result<Self, AutomationError> {
        // Tao already did this for the app, where this call fails and that's
        // fine. The headless CLI relies on it to see physical pixels.
        // SAFETY: takes a constant by value.
        let _ =
            unsafe { SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };
        let uia = uia()?;
        let root = uia
            .automation
            .get_root_element()
            .map_err(|e| uia_error("Couldn't read the desktop root", e))?;
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
        let (scopes, scope) = self.search_scopes(root);
        scopes
            .iter()
            .find_map(|window| window.find_first(scope, &condition).ok())
            .ok_or_else(|| AutomationError::ElementNotFound(format!("{selector:?}")))
    }

    /// Where a search from `root` runs. A Descendants search from the desktop
    /// root finds nothing (on Windows 11 it missed every Calculator button),
    /// so a search from the desktop runs in each top-level window's subtree
    /// instead, front to back. Any other root is searched directly.
    fn search_scopes(
        &self,
        root: &uiautomation::UIElement,
    ) -> (Vec<uiautomation::UIElement>, TreeScope) {
        let automation = &self.uia.automation;
        let is_desktop = automation
            .compare_elements(root, &self.root.element)
            .unwrap_or(false);
        if !is_desktop {
            return (vec![root.clone()], TreeScope::Descendants);
        }
        let windows = automation
            .get_control_view_walker()
            .ok()
            .and_then(|walker| walker.get_children(root))
            .unwrap_or_default();
        (windows, TreeScope::Subtree)
    }

    /// The top-level window whose title or process contains `name`, or whose
    /// window class is `name`. Front to back, so the one the person sees wins.
    fn top_level_window(&self, name: &str) -> Result<uiautomation::UIElement, AutomationError> {
        let wanted = name.to_lowercase();
        let own = std::process::id() as i32;
        let found = list_window_records().into_iter().find(|window| {
            window.pid != own
                && (window
                    .title
                    .as_deref()
                    .is_some_and(|t| t.to_lowercase().contains(&wanted))
                    || window.owner.to_lowercase().contains(&wanted)
                    || window_list::class_name(hwnd_from_id(window.id)).to_lowercase() == wanted)
        });
        let window = found.ok_or_else(|| {
            AutomationError::ElementNotFound(format!("No window matching '{name}'"))
        })?;
        self.uia
            .automation
            .element_from_handle(hwnd_from_id(window.id).into())
            .map_err(|e| uia_error("Couldn't read that window", e))
    }

    fn foreground(&self) -> Result<HWND, AutomationError> {
        foreground_window()
            .ok_or_else(|| AutomationError::NoFocusedElement("No window is in front".to_string()))
    }

    /// Launch through the shell, without taking the foreground from whoever has
    /// it. The shell resolves app names, paths and URLs alike.
    fn shell_open(target: &str, arguments: Option<&str>) -> Result<(), AutomationError> {
        let verb = HSTRING::from("open");
        let target_wide = HSTRING::from(target);
        let arguments_wide = arguments.map(HSTRING::from);
        let arguments_ptr = arguments_wide
            .as_ref()
            .map(|a| PCWSTR(a.as_ptr()))
            .unwrap_or(PCWSTR::null());
        // SAFETY: every string outlives the call, and the rest are plain values.
        let result = unsafe {
            ShellExecuteW(
                None,
                &verb,
                &target_wide,
                arguments_ptr,
                PCWSTR::null(),
                SW_SHOWNOACTIVATE,
            )
        };
        // ShellExecute returns a value above 32 on success.
        if result.0 as isize > 32 {
            Ok(())
        } else {
            Err(AutomationError::PlatformError(format!(
                "Windows couldn't open '{target}' (error {})",
                result.0 as isize
            )))
        }
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
        let (scopes, scope) = self.search_scopes(&scope_root);
        Ok(scopes
            .iter()
            .flat_map(|window| window.find_all(scope, &condition).unwrap_or_default())
            .map(|e| self.wrap(e))
            .collect())
    }

    fn open_application(&self, app_name: &str) -> Result<UIElement, AutomationError> {
        ensure_com();
        Self::shell_open(app_name, None)?;
        let started = Instant::now();
        loop {
            match self.top_level_window(app_name) {
                Ok(window) => return Ok(self.wrap(window)),
                Err(AutomationError::ElementNotFound(message)) => {
                    if started.elapsed() >= APP_LAUNCH_DEADLINE {
                        return Err(AutomationError::Timeout(message));
                    }
                }
                Err(other) => return Err(other),
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }

    fn open_url(&self, url: &str, browser: Option<&str>) -> Result<UIElement, AutomationError> {
        ensure_com();
        match browser {
            Some(browser) => Self::shell_open(browser, Some(url))?,
            None => Self::shell_open(url, None)?,
        }
        Ok(self.get_root_element())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn element_at_position(&self, x: f64, y: f64) -> Option<UIElement> {
        ensure_com();
        self.uia
            .automation
            .element_from_point(Point::new(x.round() as i32, y.round() as i32))
            .ok()
            .map(|e| self.wrap(e))
    }

    /// Inside one app, whichever of its windows is meant: the pinned one, else
    /// its frontmost that holds the point. Works when the window is covered.
    fn element_at_position_in_app(&self, pid: i32, x: f64, y: f64) -> Option<UIElement> {
        ensure_com();
        let window = match crate::window_target::pinned().filter(|p| p.pid == pid) {
            Some(pin) => hwnd_from_id(pin.window_id),
            None => {
                let records = list_window_records();
                let record = records.iter().find(|w| {
                    w.pid == pid
                        && crate::window_target::app_window_contains(
                            std::slice::from_ref(w),
                            pid,
                            x,
                            y,
                        )
                })?;
                hwnd_from_id(record.id)
            }
        };
        ax::element_in_window_at(&self.uia, window, x, y).map(|e| self.wrap(e))
    }

    fn scroll_at_position(
        &self,
        x: f64,
        y: f64,
        direction: &str,
        amount: f64,
    ) -> Result<(), AutomationError> {
        input::scroll_no_warp(x, y, direction, amount, true).map(|_| ())
    }

    fn scroll_at_current_position(
        &self,
        direction: &str,
        amount: f64,
    ) -> Result<(), AutomationError> {
        input::send_scroll(direction, amount)
    }

    fn type_text(&self, text: &str) -> Result<(), AutomationError> {
        input::send_text(text)
    }

    fn get_clipboard_content(&self) -> Result<String, AutomationError> {
        arboard::Clipboard::new()
            .and_then(|mut clipboard| clipboard.get_text())
            .map_err(|e| {
                AutomationError::PlatformError(format!("Couldn't read the clipboard: {e}"))
            })
    }

    fn set_clipboard_content(&self, content: &str) -> Result<(), AutomationError> {
        arboard::Clipboard::new()
            .and_then(|mut clipboard| clipboard.set_text(content.to_string()))
            .map_err(|e| {
                AutomationError::PlatformError(format!("Couldn't write the clipboard: {e}"))
            })
    }

    fn hold_key(&self, key: &str, duration_ms: Option<u64>) -> Result<(), AutomationError> {
        let combo = input::parse_key(key)?;
        input::send_key_edge(&combo, false)?;
        if let Some(duration) = duration_ms {
            std::thread::sleep(Duration::from_millis(duration));
            input::send_key_edge(&combo, true)?;
        }
        Ok(())
    }

    fn release_key(&self, key: &str) -> Result<(), AutomationError> {
        input::send_key_edge(&input::parse_key(key)?, true)
    }

    fn wait(&self, duration_ms: u64) -> Result<(), AutomationError> {
        std::thread::sleep(Duration::from_millis(duration_ms));
        Ok(())
    }

    fn press_key(&self, key_name: &str, modifier: Option<&str>) -> Result<(), AutomationError> {
        input::send_key_combo(&combo_of(key_name, modifier)?)
    }

    /// The tree of one window, or of the foreground window when no name is
    /// given, with how long the read took.
    fn get_ui_tree(&self, app_name: Option<&str>) -> Result<JsonValue, AutomationError> {
        ensure_com();
        let window = match app_name {
            Some(name) => self.top_level_window(name)?,
            None => self
                .uia
                .automation
                .element_from_handle(self.foreground()?.into())
                .map_err(|e| uia_error("Couldn't read the foreground window", e))?,
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
        window_list::cursor_point()
            .map(|(x, y)| (f64::from(x), f64::from(y)))
            .ok_or_else(|| {
                AutomationError::PlatformError("Couldn't read the cursor position".to_string())
            })
    }

    fn mouse_move(&self, x: f64, y: f64) -> Result<(), AutomationError> {
        // SAFETY: takes values only.
        unsafe {
            windows::Win32::UI::WindowsAndMessaging::SetCursorPos(
                x.round() as i32,
                y.round() as i32,
            )
        }
        .map_err(|e| AutomationError::PlatformError(format!("Couldn't move the cursor: {e}")))
    }

    fn left_mouse_down(&self, x: f64, y: f64) -> Result<(), AutomationError> {
        input::send_mouse_button(Button::Left, x, y, true)
    }

    fn left_mouse_up(&self, x: f64, y: f64) -> Result<(), AutomationError> {
        input::send_mouse_button(Button::Left, x, y, false)
    }

    fn left_click(&self, x: f64, y: f64, modifiers: Option<&str>) -> Result<(), AutomationError> {
        input::send_click(Button::Left, x, y, 1, modifiers)
    }

    fn right_click(&self, x: f64, y: f64, modifiers: Option<&str>) -> Result<(), AutomationError> {
        input::send_click(Button::Right, x, y, 1, modifiers)
    }

    fn middle_click(&self, x: f64, y: f64, modifiers: Option<&str>) -> Result<(), AutomationError> {
        input::send_click(Button::Middle, x, y, 1, modifiers)
    }

    fn double_click(&self, x: f64, y: f64, modifiers: Option<&str>) -> Result<(), AutomationError> {
        input::send_click(Button::Left, x, y, 2, modifiers)
    }

    fn triple_click(&self, x: f64, y: f64, modifiers: Option<&str>) -> Result<(), AutomationError> {
        input::send_click(Button::Left, x, y, 3, modifiers)
    }

    fn left_click_drag(
        &self,
        start_x: f64,
        start_y: f64,
        end_x: f64,
        end_y: f64,
    ) -> Result<(), AutomationError> {
        input::send_drag((start_x, start_y), (end_x, end_y))
    }

    fn left_click_no_warp(
        &self,
        x: f64,
        y: f64,
        modifiers: Option<&str>,
        allow_physical: bool,
    ) -> Result<Option<InputOutcome>, AutomationError> {
        input::click_no_warp(Button::Left, x, y, 1, modifiers, allow_physical)
    }

    fn right_click_no_warp(
        &self,
        x: f64,
        y: f64,
        allow_physical: bool,
    ) -> Result<Option<InputOutcome>, AutomationError> {
        input::click_no_warp(Button::Right, x, y, 1, None, allow_physical)
    }

    fn middle_click_no_warp(
        &self,
        x: f64,
        y: f64,
        modifiers: Option<&str>,
        allow_physical: bool,
    ) -> Result<Option<InputOutcome>, AutomationError> {
        input::click_no_warp(Button::Middle, x, y, 1, modifiers, allow_physical)
    }

    fn double_click_no_warp(
        &self,
        x: f64,
        y: f64,
        modifiers: Option<&str>,
        allow_physical: bool,
    ) -> Result<Option<InputOutcome>, AutomationError> {
        input::click_no_warp(Button::Left, x, y, 2, modifiers, allow_physical)
    }

    fn triple_click_no_warp(
        &self,
        x: f64,
        y: f64,
        modifiers: Option<&str>,
        allow_physical: bool,
    ) -> Result<Option<InputOutcome>, AutomationError> {
        input::click_no_warp(Button::Left, x, y, 3, modifiers, allow_physical)
    }

    fn left_mouse_down_no_warp(
        &self,
        x: f64,
        y: f64,
        allow_physical: bool,
    ) -> Result<Option<InputOutcome>, AutomationError> {
        input::mouse_button_no_warp(x, y, true, allow_physical)
    }

    fn left_mouse_up_no_warp(
        &self,
        x: f64,
        y: f64,
        allow_physical: bool,
    ) -> Result<Option<InputOutcome>, AutomationError> {
        input::mouse_button_no_warp(x, y, false, allow_physical)
    }

    fn left_click_drag_no_warp(
        &self,
        start_x: f64,
        start_y: f64,
        end_x: f64,
        end_y: f64,
        allow_physical: bool,
    ) -> Result<Option<InputOutcome>, AutomationError> {
        input::drag_no_warp((start_x, start_y), (end_x, end_y), allow_physical)
    }

    fn scroll_no_warp(
        &self,
        x: f64,
        y: f64,
        direction: &str,
        amount: f64,
        _modifiers: Option<&str>,
        allow_physical: bool,
    ) -> Result<Option<InputOutcome>, AutomationError> {
        input::scroll_no_warp(x, y, direction, amount, allow_physical)
    }

    fn press_key_no_warp(
        &self,
        key_name: &str,
        modifier: Option<&str>,
        allow_physical: bool,
    ) -> Result<Option<InputOutcome>, AutomationError> {
        input::key_no_warp(&combo_of(key_name, modifier)?, allow_physical)
    }

    fn hold_key_no_warp(
        &self,
        key: &str,
        duration_ms: Option<u64>,
        allow_physical: bool,
    ) -> Result<Option<InputOutcome>, AutomationError> {
        input::hold_key_no_warp(&input::parse_key(key)?, duration_ms, allow_physical)
    }

    fn type_text_no_warp(
        &self,
        text: &str,
        allow_physical: bool,
    ) -> Result<Option<InputOutcome>, AutomationError> {
        input::type_no_warp(text, allow_physical)
    }

    fn release_key_no_warp(
        &self,
        key: &str,
        allow_physical: bool,
    ) -> Result<Option<InputOutcome>, AutomationError> {
        input::release_key_no_warp(&input::parse_key(key)?, allow_physical)
    }

    fn get_window_title(&self) -> Result<String, AutomationError> {
        Ok(window_list::foreground_record()
            .and_then(|w| w.title)
            .unwrap_or_default())
    }

    fn list_windows(&self) -> Result<Vec<UIElement>, AutomationError> {
        self.get_applications()
    }

    fn close_window(&self) -> Result<(), AutomationError> {
        let window = root_of(self.foreground()?);
        // SAFETY: posts plain values; a stale handle only makes the call fail.
        unsafe { PostMessageW(Some(window), WM_CLOSE, WPARAM(0), LPARAM(0)) }
            .map_err(|e| AutomationError::PlatformError(format!("Couldn't close the window: {e}")))
    }

    fn maximize_window(&self) -> Result<(), AutomationError> {
        let window = self.foreground()?;
        // SAFETY: takes values only.
        let _ = unsafe { ShowWindow(window, SW_MAXIMIZE) };
        Ok(())
    }

    fn minimize_window(&self) -> Result<(), AutomationError> {
        let window = self.foreground()?;
        // SAFETY: takes values only.
        let _ = unsafe { ShowWindow(window, SW_MINIMIZE) };
        Ok(())
    }

    fn resize_window(&self, width: f64, height: f64) -> Result<(), AutomationError> {
        let window = self.foreground()?;
        // SAFETY: takes values only.
        unsafe {
            SetWindowPos(
                window,
                None,
                0,
                0,
                width.round() as i32,
                height.round() as i32,
                SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
            )
        }
        .map_err(|e| AutomationError::PlatformError(format!("Couldn't resize the window: {e}")))
    }

    fn move_window(&self, x: f64, y: f64) -> Result<(), AutomationError> {
        let window = self.foreground()?;
        // SAFETY: takes values only.
        unsafe {
            SetWindowPos(
                window,
                None,
                x.round() as i32,
                y.round() as i32,
                0,
                0,
                SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
            )
        }
        .map_err(|e| AutomationError::PlatformError(format!("Couldn't move the window: {e}")))
    }

    fn get_element_tree(&self, element: &UIElement) -> Result<ElementTreeNode, AutomationError> {
        element.get_tree()
    }
}

fn combo_of(key_name: &str, modifier: Option<&str>) -> Result<input::KeyCombo, AutomationError> {
    match modifier {
        Some(modifier) => input::parse_key(&format!("{modifier}+{key_name}")),
        None => input::parse_key(key_name),
    }
}

/// Bring an app's frontmost window forward. Only for the last-resort retry.
pub fn activate_pid(pid: i32) {
    if let Some(window) = top_window_of_pid(pid) {
        window_list::activate_window(window);
    }
}
