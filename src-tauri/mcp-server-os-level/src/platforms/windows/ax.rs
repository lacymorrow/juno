//! The Windows half of `ax_elements` and `ax_text`: one window's controls, read
//! through UI Automation, and acting on them without moving anything.
//!
//! Role names are the macOS ones (`AXButton`), because the listing, the role
//! tests and the prompts above this module all speak them. UI Automation's
//! control types map onto them in `ax_role`.
//!
//! Reads ask for everything at once with a cache request. Without one, each
//! property of each element is a round trip into the other process, and a
//! window of a few hundred controls takes seconds.

use super::element::{ensure_com, uia, Uia, WindowsUIElement};
use super::window_list::{
    app_pid, child_of_class, foreground_window, hwnd_from_id, id_from_hwnd, list_window_records,
    process_is_elevated, root_of, window_exists,
};
use crate::ax_elements::{
    clip_value, element_name, is_self_updating_role, short_role, worth_listing, ElementInfo,
};
use crate::ax_text::{insertion_verified, AxTypeReport, AxTypeSkip, TargetInfo, TypeTarget};
use crate::click_verify::ClickKind;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use uiautomation::patterns::{
    UIExpandCollapsePattern, UIInvokePattern, UILegacyIAccessiblePattern, UIPatternType,
    UIScrollPattern, UISelectionItemPattern, UITextPattern, UITogglePattern, UIValuePattern,
};
use uiautomation::types::{
    ControlType, ScrollAmount, TextPatternRangeEndpoint, TreeScope, UIProperty,
};
use uiautomation::variants::Variant;
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    SendMessageTimeoutW, OBJID_CLIENT, SMTO_ABORTIFHUNG, WM_GETOBJECT,
};

/// Same ceiling as the macOS walk.
const MAX_DEPTH_OF_POINT_SEARCH: usize = 50;
/// Nodes read for one fingerprint. It runs twice around every posted click, so
/// it stays smaller than a listing.
const FINGERPRINT_MAX_NODES: usize = 1_500;

/// How long a Chromium window gets to build its tree after being woken. Tree
/// builds of over 300 ms are normal, and past this it isn't coming.
const TREE_WAKE_DEADLINE: Duration = Duration::from_millis(2_000);
/// How long a window has to answer the wake-up message.
const WAKE_MESSAGE_TIMEOUT_MS: u32 = 200;
/// UI Automation's own object id for WM_GETOBJECT.
const UIA_ROOT_OBJECT_ID: i32 = -25;

/// How long a UIA Invoke gets to pull its app forward before the foreground is
/// handed back. Invoke returns before the app has reacted.
const FOREGROUND_WATCH: Duration = Duration::from_millis(200);

/// macOS role for a UI Automation control type.
pub fn ax_role(control_type: ControlType) -> &'static str {
    match control_type {
        ControlType::Button => "AXButton",
        ControlType::CheckBox => "AXCheckBox",
        ControlType::RadioButton => "AXRadioButton",
        ControlType::SplitButton => "AXMenuButton",
        ControlType::ComboBox => "AXComboBox",
        ControlType::Edit => "AXTextField",
        ControlType::Document => "AXTextArea",
        ControlType::Hyperlink => "AXLink",
        ControlType::MenuItem => "AXMenuItem",
        ControlType::Slider => "AXSlider",
        ControlType::Spinner => "AXIncrementor",
        ControlType::ProgressBar => "AXProgressIndicator",
        ControlType::ScrollBar => "AXScrollBar",
        ControlType::Text => "AXStaticText",
        // These read as rows and tabs to the rest of Juno.
        ControlType::ListItem | ControlType::TreeItem | ControlType::DataItem => "AXRow",
        ControlType::TabItem => "AXTab",
        ControlType::Image => "AXImage",
        ControlType::List | ControlType::DataGrid | ControlType::Table => "AXList",
        ControlType::Tree => "AXOutline",
        ControlType::Window => "AXWindow",
        ControlType::Pane => "AXGroup",
        ControlType::Group => "AXGroup",
        ControlType::Menu => "AXMenu",
        ControlType::MenuBar => "AXMenuBar",
        ControlType::ToolBar => "AXToolbar",
        ControlType::TitleBar => "AXGroup",
        ControlType::StatusBar => "AXGroup",
        ControlType::Tab => "AXTabGroup",
        ControlType::Header | ControlType::HeaderItem => "AXGroup",
        _ => "AXUnknown",
    }
}

/// Roles worth listing on Windows. The shared `is_listed_role` is the macOS
/// answer; a file in Explorer is a row here, and renaming one starts by finding
/// it, so rows and tabs have to be in the list.
fn is_listed(role: &str) -> bool {
    crate::ax_elements::is_listed_role(role) || matches!(role, "AXRow" | "AXTab" | "AXMenuButton")
}

// ── The window a tree is read from ──────────────────────────────────────────

/// Chromium and Electron build their accessibility tree only once something
/// asks, and hand back an empty document until then. So ask, then watch the
/// tree rather than sleep: it's done when the document has children.
fn wake_chromium_tree(uia: &Uia, window: HWND) {
    let Some(host) = child_of_class(window, "Chrome_RenderWidgetHostHWND") else {
        return;
    };
    for object in [OBJID_CLIENT.0, UIA_ROOT_OBJECT_ID] {
        let mut result = 0usize;
        // SAFETY: passes plain values, and `result` outlives the call. The
        // timeout keeps a hung renderer from holding the agent.
        let _ = unsafe {
            SendMessageTimeoutW(
                host,
                WM_GETOBJECT,
                WPARAM(0),
                LPARAM(object as isize),
                SMTO_ABORTIFHUNG,
                WAKE_MESSAGE_TIMEOUT_MS,
                Some(&mut result),
            )
        };
    }
    let Ok(document_type) = uia.automation.create_property_condition(
        UIProperty::ControlType,
        Variant::from(ControlType::Document as i32),
        None,
    ) else {
        return;
    };
    let Ok(root) = uia.automation.element_from_handle(host.into()) else {
        return;
    };
    let Ok(walker) = uia.automation.get_control_view_walker() else {
        return;
    };
    let started = Instant::now();
    while started.elapsed() < TREE_WAKE_DEADLINE {
        let built = root
            .find_first(TreeScope::Subtree, &document_type)
            .ok()
            .is_some_and(|document| walker.get_first_child(&document).is_ok());
        if built {
            return;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    tracing::info!(
        "[AX] Chromium tree of window {:?} still empty after the wake",
        window
    );
}

/// A window's descendants in document order, with what a listing reads already
/// fetched.
fn read_window(uia: &Uia, window: HWND) -> Result<Vec<uiautomation::UIElement>, String> {
    ensure_com();
    if process_is_elevated(app_pid(window)) {
        return Err(crate::AutomationError::ElevatedWindow.to_string());
    }
    wake_chromium_tree(uia, window);
    let root = uia
        .automation
        .element_from_handle(window.into())
        .map_err(|e| format!("Couldn't read that window's controls: {e}"))?;
    let cache = uia
        .automation
        .create_cache_request()
        .map_err(|e| format!("Couldn't read that window's controls: {e}"))?;
    for property in [
        UIProperty::Name,
        UIProperty::ControlType,
        UIProperty::IsEnabled,
        UIProperty::IsOffscreen,
        UIProperty::IsPassword,
        UIProperty::BoundingRectangle,
        UIProperty::HelpText,
        UIProperty::ValueValue,
    ] {
        cache
            .add_property(property)
            .map_err(|e| format!("Couldn't read that window's controls: {e}"))?;
    }
    let _ = cache.add_pattern(UIPatternType::Value);
    let condition = uia
        .automation
        .get_control_view_condition()
        .map_err(|e| format!("Couldn't read that window's controls: {e}"))?;
    root.find_all_build_cache(TreeScope::Descendants, &condition, &cache)
        .map_err(|e| format!("Couldn't read that window's controls: {e}"))
}

fn cached_value(element: &uiautomation::UIElement) -> Option<String> {
    let value: String = element
        .get_cached_property_value(UIProperty::ValueValue)
        .ok()?
        .try_into()
        .ok()?;
    (!value.is_empty()).then_some(value)
}

// ── Element listing ─────────────────────────────────────────────────────────

/// A control in a window, held so it can be acted on later from another thread.
#[derive(Clone, Debug)]
pub struct ElementRef {
    element: WindowsUIElement,
}

impl ElementRef {
    pub(super) fn new(uia: Uia, element: uiautomation::UIElement) -> Self {
        Self {
            element: WindowsUIElement::new(uia, element),
        }
    }
}

/// One window's controls and text, up to `limit`.
pub fn window_elements(
    _pid: i32,
    window_id: u32,
    limit: usize,
) -> Result<Vec<(ElementInfo, ElementRef)>, String> {
    let uia = uia().map_err(|e| e.to_string())?;
    let window = hwnd_from_id(window_id);
    if !window_exists(window) {
        return Err(format!("Window {window_id} is no longer open"));
    }
    let found = read_window(&uia, window)?;
    let mut listed = Vec::new();
    for element in found
        .into_iter()
        .take(crate::ax_elements::MAX_NODES_VISITED)
    {
        if listed.len() >= limit {
            break;
        }
        if element.is_cached_offscreen().unwrap_or(false) {
            continue;
        }
        let Ok(control_type) = element.get_cached_control_type() else {
            continue;
        };
        let role = ax_role(control_type);
        if !is_listed(role) {
            continue;
        }
        let name = element.get_cached_name().ok();
        let help = element.get_cached_help_text().ok();
        let name = element_name(name.as_deref(), help.as_deref(), None);
        // A password field's value is never read, listed or logged.
        let value = if element.is_cached_password().unwrap_or(false) {
            None
        } else {
            cached_value(&element).map(|v| clip_value(&v))
        };
        if !worth_listing(role, name.as_deref(), value.as_deref()) {
            continue;
        }
        let Ok(rect) = element.get_cached_bounding_rectangle() else {
            continue;
        };
        listed.push((
            ElementInfo {
                role: short_role(role),
                name,
                value,
                enabled: element.is_cached_enabled().unwrap_or(true),
                frame: (
                    f64::from(rect.get_left()),
                    f64::from(rect.get_top()),
                    f64::from(rect.get_width()),
                    f64::from(rect.get_height()),
                ),
            },
            ElementRef::new(uia.clone(), element),
        ));
    }
    Ok(listed)
}

/// A hash of what a window shows, to tell whether a posted click did anything.
/// `None` when it can't be read, which means "can't tell", not "unchanged".
pub fn window_fingerprint(_pid: i32, window_id: u32) -> Option<u64> {
    let uia = uia().ok()?;
    let found = read_window(&uia, hwnd_from_id(window_id)).ok()?;
    let mut hasher = DefaultHasher::new();
    let mut nodes = 0usize;
    for element in found.iter().take(FINGERPRINT_MAX_NODES) {
        let Ok(control_type) = element.get_cached_control_type() else {
            continue;
        };
        let role = ax_role(control_type);
        if is_self_updating_role(role) {
            continue;
        }
        role.hash(&mut hasher);
        element
            .get_cached_name()
            .unwrap_or_default()
            .hash(&mut hasher);
        element
            .is_cached_enabled()
            .unwrap_or(true)
            .hash(&mut hasher);
        if !element.is_cached_password().unwrap_or(false) {
            cached_value(element).hash(&mut hasher);
        }
        nodes += 1;
    }
    (nodes > 0).then(|| hasher.finish())
}

// ── Acting on an element ────────────────────────────────────────────────────

/// Press an element where it is, or say why not.
pub fn press(element: &ElementRef) -> Result<(), String> {
    element
        .element
        .press()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// Open the element's context menu without a click.
pub fn show_menu(element: &ElementRef) -> Result<(), String> {
    ensure_com();
    element
        .element
        .element
        .show_context_menu()
        .map_err(|e| format!("That control has no menu to open: {e}"))
}

/// Focus without moving the foreground. UI Automation's `SetFocus` brings the
/// whole app forward, so it isn't used: the field is remembered, and typing that
/// follows goes straight to it.
pub fn focus(element: &ElementRef) -> Result<(), String> {
    remember_field(&element.element);
    Ok(())
}

/// Where a control is now, or `None` once it's gone.
pub fn live_frame(element: &ElementRef) -> Option<(f64, f64, f64, f64)> {
    ensure_com();
    let rect = element.element.element.get_bounding_rectangle().ok()?;
    if rect.get_width() <= 0 || rect.get_height() <= 0 {
        return None;
    }
    Some((
        f64::from(rect.get_left()),
        f64::from(rect.get_top()),
        f64::from(rect.get_width()),
        f64::from(rect.get_height()),
    ))
}

/// Press an element or its nearest ancestor that can be pressed (a row around a
/// title). A double click means "open", which only an open-style action stands
/// in for: a list row's selection isn't one.
pub fn press_nearest(element: &ElementRef, kind: ClickKind) -> Result<&'static str, String> {
    ensure_com();
    let mut current = Some(element.element.clone());
    let mut last_error = String::from("That control can't be pressed");
    for _ in 0..8 {
        let Some(candidate) = current else {
            break;
        };
        let pressed = match kind {
            ClickKind::Double => candidate.open(),
            _ => candidate.press(),
        };
        match pressed {
            Ok(method) => return Ok(method),
            Err(e) => last_error = e.to_string(),
        }
        current = candidate.parent_element();
    }
    Err(last_error)
}

impl WindowsUIElement {
    fn control_type(&self) -> Option<ControlType> {
        self.element.get_control_type().ok()
    }

    /// The element's own window, when it is a classic Win32 button or a
    /// WinForms button (which wraps the same control).
    fn win32_button(&self) -> Option<HWND> {
        let class = self.element.get_classname().ok()?;
        if class != "Button" && !class.starts_with("WindowsForms10.BUTTON.") {
            return None;
        }
        let handle = self.element.get_native_window_handle().ok()?;
        (!handle.is_invalid()).then(|| handle.into())
    }

    fn msaa_default_action(&self) -> bool {
        self.element
            .get_pattern::<UILegacyIAccessiblePattern>()
            .is_ok_and(|legacy| {
                legacy.get_default_action().is_ok_and(|a| !a.is_empty())
                    && legacy.do_default_action().is_ok()
            })
    }

    /// Press with no input event, so nothing moves. The order is the one the
    /// spike proved (docs/changelog/media/928): every UIA pattern press brought
    /// its app to the front on Win32, WinForms, WPF, XAML, Edge and VS Code,
    /// where BM_CLICK on a real button never did and MSAA's default action did
    /// it 0 times in 10 on a covered WPF button.
    pub(super) fn press(&self) -> Result<&'static str, crate::AutomationError> {
        use crate::AutomationError;
        use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, BM_CLICK};
        ensure_com();
        match self.control_type() {
            // Typing is what a click on a field is for, and a click carries no
            // caret position through UI Automation. The next `type` goes here.
            Some(ControlType::Edit | ControlType::Document) => {
                remember_field(self);
                return Ok("Remembered as the field to type in");
            }
            // A click on a row selects it. Its default action is "open", which
            // would launch the file somebody meant to rename.
            Some(
                ControlType::ListItem
                | ControlType::TreeItem
                | ControlType::DataItem
                | ControlType::TabItem,
            ) => {
                if let Ok(item) = self.element.get_pattern::<UISelectionItemPattern>() {
                    if item.select().is_ok() {
                        return Ok("UIA SelectionItem");
                    }
                }
            }
            _ => {}
        }
        if let Some(hwnd) = self.win32_button() {
            // SAFETY: posts a message with no pointers in it; a stale handle
            // only makes the call fail. A window at a higher integrity level
            // refuses the post, which falls through to the next tier.
            if unsafe { PostMessageW(Some(hwnd), BM_CLICK, WPARAM(0), LPARAM(0)) }.is_ok() {
                return Ok("BM_CLICK");
            }
        }
        if self.msaa_default_action() {
            return Ok("MSAA default action");
        }
        let front = foreground_window();
        if let Ok(invoke) = self.element.get_pattern::<UIInvokePattern>() {
            invoke
                .invoke()
                .map_err(|e| AutomationError::PlatformError(format!("Invoke failed: {e}")))?;
            hand_foreground_back(front);
            return Ok("UIA Invoke");
        }
        if let Ok(toggle) = self.element.get_pattern::<UITogglePattern>() {
            toggle
                .toggle()
                .map_err(|e| AutomationError::PlatformError(format!("Toggle failed: {e}")))?;
            hand_foreground_back(front);
            return Ok("UIA Toggle");
        }
        if let Ok(item) = self.element.get_pattern::<UISelectionItemPattern>() {
            item.select()
                .map_err(|e| AutomationError::PlatformError(format!("Select failed: {e}")))?;
            hand_foreground_back(front);
            return Ok("UIA SelectionItem");
        }
        if let Ok(expander) = self.element.get_pattern::<UIExpandCollapsePattern>() {
            expander
                .expand()
                .map_err(|e| AutomationError::PlatformError(format!("Expand failed: {e}")))?;
            hand_foreground_back(front);
            return Ok("UIA ExpandCollapse");
        }
        Err(AutomationError::UnsupportedOperation(format!(
            "'{}' can't be pressed without real input",
            self.element.get_name().unwrap_or_default()
        )))
    }

    /// The action a double click stands for: open it. Row or button alike,
    /// that's the default action, then Invoke.
    pub(super) fn open(&self) -> Result<&'static str, crate::AutomationError> {
        use crate::AutomationError;
        ensure_com();
        if self.msaa_default_action() {
            return Ok("MSAA default action");
        }
        if let Ok(invoke) = self.element.get_pattern::<UIInvokePattern>() {
            let front = foreground_window();
            invoke
                .invoke()
                .map_err(|e| AutomationError::PlatformError(format!("Invoke failed: {e}")))?;
            hand_foreground_back(front);
            return Ok("UIA Invoke");
        }
        Err(AutomationError::UnsupportedOperation(
            "That control has no open action".to_string(),
        ))
    }
}

/// A UIA pattern press can drag its app in front of the person's window. Watch
/// for that a bounded time, and give the foreground back if it happened. This is
/// the one place the engine notices rather than prevents, because Invoke has no
/// quiet form.
fn hand_foreground_back(front_before: Option<HWND>) {
    let Some(before) = front_before else {
        return;
    };
    let started = Instant::now();
    while started.elapsed() < FOREGROUND_WATCH {
        if foreground_window() != Some(before) {
            if window_exists(before) {
                super::window_list::activate_window(before);
                tracing::info!("[AX] A press took the foreground; gave it back");
            }
            return;
        }
        std::thread::sleep(Duration::from_millis(15));
    }
}

/// How long a launched app gets to take the foreground. Slower than a press:
/// the process has to start first.
const LAUNCH_WATCH: Duration = Duration::from_millis(1500);

/// Watch for a launched app taking the foreground from `window_id`, and give it
/// back the moment it does. True if it had to.
pub fn keep_foreground_on(window_id: u32) -> bool {
    let before = hwnd_from_id(window_id);
    let started = Instant::now();
    while started.elapsed() < LAUNCH_WATCH {
        if foreground_window() != Some(before) {
            return window_exists(before) && super::window_list::activate_window(before);
        }
        std::thread::sleep(Duration::from_millis(15));
    }
    false
}

// ── The field to type in ────────────────────────────────────────────────────

/// The field the agent last pressed or named, and the top-level window it's in.
struct RememberedField {
    window: u32,
    element: WindowsUIElement,
}

// SAFETY: see `Uia`. An element is a proxy to a free-threaded COM object.
unsafe impl Send for RememberedField {}

static FIELD: Mutex<Option<RememberedField>> = Mutex::new(None);

fn element_window(element: &WindowsUIElement) -> Option<HWND> {
    let mut current = Some(element.clone());
    for _ in 0..MAX_DEPTH_OF_POINT_SEARCH {
        let candidate = current?;
        if let Ok(handle) = candidate.element.get_native_window_handle() {
            if !handle.is_invalid() {
                return Some(handle.into());
            }
        }
        current = candidate.parent_element();
    }
    None
}

pub(super) fn remember_field(element: &WindowsUIElement) {
    let Some(window) = element_window(element) else {
        return;
    };
    if let Ok(mut field) = FIELD.lock() {
        *field = Some(RememberedField {
            window: id_from_hwnd(root_of(window)),
            element: element.clone(),
        });
    }
}

fn remembered_field(root: HWND) -> Option<WindowsUIElement> {
    let field = FIELD.lock().ok()?;
    let field = field.as_ref()?;
    (field.window == id_from_hwnd(root)).then(|| field.element.clone())
}

/// The inner window of a remembered field, when it has its own (a Win32 edit).
pub(super) fn remembered_field_window(root: HWND) -> Option<HWND> {
    let field = remembered_field(root)?;
    let handle = field.element.get_native_window_handle().ok()?;
    (!handle.is_invalid()).then(|| handle.into())
}

// ── Typing ──────────────────────────────────────────────────────────────────

/// The field `text` should go in: the remembered one, else whichever inner
/// window holds the window's own focus.
fn target_field(uia: &Uia, root: HWND) -> Result<WindowsUIElement, AxTypeSkip> {
    if let Some(field) = remembered_field(root) {
        return Ok(field);
    }
    let focused = super::window_list::focused_child(root);
    uia.automation
        .element_from_handle(focused.into())
        .map(|element| WindowsUIElement::new(uia.clone(), element))
        .map_err(|_| AxTypeSkip::NoFocusedElement)
}

/// How many characters of the value come before the caret, when the field can
/// say. A field that can't is appended to.
fn caret_offset(element: &uiautomation::UIElement, before: &str) -> Option<(usize, usize)> {
    let pattern = element.get_pattern::<UITextPattern>().ok()?;
    let selection = pattern.get_selection().ok()?.into_iter().next()?;
    let lead = pattern.get_document_range().ok()?;
    lead.move_endpoint_by_range(
        TextPatternRangeEndpoint::End,
        &selection,
        TextPatternRangeEndpoint::Start,
    )
    .ok()?;
    let prefix = lead.get_text(-1).ok()?;
    let selected = selection.get_text(-1).ok()?;
    // Rich edits report line ends differently through the two patterns. If the
    // text around the caret doesn't match the value exactly, the offset would
    // be a guess.
    if !before.starts_with(&prefix) {
        return None;
    }
    Some((prefix.chars().count(), selected.chars().count()))
}

/// Check the field and write, reading it back. Only a verified write is a
/// success; anything else sends the text down the posted path.
pub fn type_verified(target: &TypeTarget, text: &str) -> Result<AxTypeReport, AxTypeSkip> {
    if target.pid == std::process::id() as i32 {
        return Err(AxTypeSkip::OwnProcess);
    }
    let root = match target.window_id {
        Some(id) => hwnd_from_id(id),
        None => super::window_list::top_window_of_pid(target.pid).ok_or(AxTypeSkip::NoTarget)?,
    };
    let class = super::window_list::class_name(root);
    if matches!(
        class.as_str(),
        "CASCADIA_HOSTING_WINDOW_CLASS" | "ConsoleWindowClass"
    ) {
        return Err(AxTypeSkip::Terminal(class));
    }
    let uia = uia().map_err(|_| AxTypeSkip::Unsupported)?;
    let field = target_field(&uia, root)?;
    let element = &field.element;
    if element.is_password().unwrap_or(false) {
        return Err(AxTypeSkip::Unverifiable);
    }
    let Ok(value) = element.get_pattern::<UIValuePattern>() else {
        return Err(AxTypeSkip::NotEditable);
    };
    if value.is_readonly().unwrap_or(true) {
        return Err(AxTypeSkip::NotEditable);
    }
    let before = value.get_value().map_err(|_| AxTypeSkip::Unverifiable)?;
    let front = foreground_window();
    let written = match caret_offset(element, &before) {
        Some((offset, selected)) => {
            let chars: Vec<char> = before.chars().collect();
            let head: String = chars.iter().take(offset).collect();
            let tail: String = chars.iter().skip(offset + selected).collect();
            format!("{head}{text}{tail}")
        }
        None => format!("{before}{text}"),
    };
    value
        .set_value(&written)
        .map_err(|e| AxTypeSkip::Rejected(e.code()))?;
    hand_foreground_back(front);
    let after = value.get_value().map_err(|_| AxTypeSkip::NotVerified)?;
    if !insertion_verified(&before, &after, text) {
        return Err(AxTypeSkip::NotVerified);
    }
    Ok(AxTypeReport {
        pid: target.pid,
        window_id: Some(id_from_hwnd(root)),
        method: "UIA ValuePattern",
    })
}

/// The process and top-level window an element is in.
pub fn element_owner(element: &crate::UIElement) -> (Option<i32>, Option<u32>) {
    let Some(element) = element.as_any().downcast_ref::<WindowsUIElement>() else {
        return (None, None);
    };
    match element_window(element) {
        Some(window) => {
            let root = root_of(window);
            (Some(app_pid(root)), Some(id_from_hwnd(root)))
        }
        None => (None, None),
    }
}

/// Who an input action reached, for tool results.
pub fn describe_target(pid: Option<i32>, window_id: Option<u32>) -> TargetInfo {
    let records = list_window_records();
    let window = window_id.and_then(|id| records.iter().find(|w| w.id == id));
    TargetInfo {
        app: window
            .map(|w| w.owner.clone())
            .or_else(|| pid.and_then(super::window_list::process_name)),
        bundle_id: None,
        pid,
        window_id,
        window_title: window.and_then(|w| w.title.clone()),
    }
}

// ── Pointing into a window that may be covered ──────────────────────────────

fn contains(rect: &uiautomation::types::Rect, x: i32, y: i32) -> bool {
    x >= rect.get_left()
        && y >= rect.get_top()
        && x < rect.get_left() + rect.get_width()
        && y < rect.get_top() + rect.get_height()
}

/// The deepest element of `window` under a point. UI Automation's own hit test
/// answers for whatever is on top, which is the wrong window when this one is
/// covered, so this walks down from the window instead.
pub fn element_in_window_at(
    uia: &Uia,
    window: HWND,
    x: f64,
    y: f64,
) -> Option<uiautomation::UIElement> {
    ensure_com();
    let (x, y) = (x.round() as i32, y.round() as i32);
    let walker = uia.automation.get_control_view_walker().ok()?;
    let mut current = uia.automation.element_from_handle(window.into()).ok()?;
    for _ in 0..MAX_DEPTH_OF_POINT_SEARCH {
        let Some(children) = walker.get_children(&current) else {
            break;
        };
        // The last sibling drawn is on top, and the smallest that fits is the
        // most specific.
        let next = children
            .into_iter()
            .rev()
            .filter_map(|child| {
                let rect = child.get_bounding_rectangle().ok()?;
                contains(&rect, x, y).then(|| {
                    let area = i64::from(rect.get_width()) * i64::from(rect.get_height());
                    (child, area)
                })
            })
            .min_by_key(|(_, area)| *area);
        match next {
            Some((child, _)) => current = child,
            None => break,
        }
    }
    Some(current)
}

/// Scroll the nearest scrollable thing under a point. Err when there is none,
/// and the caller posts a wheel message instead.
pub fn scroll_at(window: HWND, x: f64, y: f64, direction: &str, amount: f64) -> Result<(), String> {
    let uia = uia().map_err(|e| e.to_string())?;
    let element = element_in_window_at(&uia, window, x, y)
        .ok_or_else(|| "Nothing under that point".to_string())?;
    let (horizontal, vertical) = match direction.to_lowercase().as_str() {
        "up" => (ScrollAmount::NoAmount, ScrollAmount::SmallDecrement),
        "down" => (ScrollAmount::NoAmount, ScrollAmount::SmallIncrement),
        "left" => (ScrollAmount::SmallDecrement, ScrollAmount::NoAmount),
        "right" => (ScrollAmount::SmallIncrement, ScrollAmount::NoAmount),
        other => return Err(format!("'{other}' isn't a scroll direction")),
    };
    let walker = uia
        .automation
        .get_control_view_walker()
        .map_err(|e| e.to_string())?;
    let mut current = Some(element);
    for _ in 0..MAX_DEPTH_OF_POINT_SEARCH {
        let Some(candidate) = current else {
            break;
        };
        if let Ok(pattern) = candidate.get_pattern::<UIScrollPattern>() {
            let can_scroll = match vertical {
                ScrollAmount::NoAmount => pattern.is_horizontally_scrollable().unwrap_or(false),
                _ => pattern.is_vertically_scrollable().unwrap_or(false),
            };
            if can_scroll {
                let notches = (amount.abs().round() as u32).clamp(1, 50);
                for _ in 0..notches {
                    pattern
                        .scroll(horizontal, vertical)
                        .map_err(|e| e.to_string())?;
                }
                return Ok(());
            }
        }
        current = walker.get_parent(&candidate).ok();
    }
    Err("Nothing scrollable under that point".to_string())
}
