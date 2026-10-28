//! # The elements of one window, read from the accessibility tree
//!
//! Clicking by pixel means guessing where a button is from a picture of it,
//! and a guess that is half a button off presses the neighbour. The window's
//! accessibility tree already knows every control's role, name and frame in
//! screen points. This module reads that list for one window and presses an
//! element directly, so an agent acts on "button 1" rather than on (321, 649).
//!
//! Text is listed too, read-only, so a result (a calculator's display, a
//! status line) can be read without a screenshot.
//!
//! Nothing here raises the window or moves the pointer: reading attributes and
//! `AXPress` both work on a window that is behind others.

use serde::Serialize;

/// Hard ceilings on one walk, so a huge tree (a long web page, a table with
/// thousands of rows) costs a bounded amount of time and tokens.
pub const MAX_NODES_VISITED: usize = 5_000;
const MAX_DEPTH: usize = 50;
/// Longest value reported for an element, in characters.
const MAX_VALUE_CHARS: usize = 200;
/// Nodes read for one window fingerprint. Smaller than a full listing: it runs
/// twice around every background click.
const FINGERPRINT_MAX_NODES: usize = 1_500;

/// Roles whose value changes by itself (a playback position, a progress bar),
/// which would make an ignored click look like it did something.
pub fn is_self_updating_role(ax_role: &str) -> bool {
    matches!(
        ax_role,
        "AXSlider"
            | "AXProgressIndicator"
            | "AXScrollBar"
            | "AXValueIndicator"
            | "AXLevelIndicator"
    )
}

/// One element of a window, as an agent reads it.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ElementInfo {
    /// Short role: `button`, `textfield`, `statictext`, ...
    pub role: String,
    /// What a person would read on it: title, else description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Its current value, when it has one worth reading. Never read for
    /// secure (password) fields.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    pub enabled: bool,
    /// x, y, width, height in screen points.
    #[serde(skip)]
    pub frame: (f64, f64, f64, f64),
}

/// Roles worth listing: things a person can act on, plus text to read.
pub fn is_listed_role(ax_role: &str) -> bool {
    matches!(
        ax_role,
        "AXButton"
            | "AXCheckBox"
            | "AXRadioButton"
            | "AXPopUpButton"
            | "AXMenuButton"
            | "AXComboBox"
            | "AXTextField"
            | "AXSecureTextField"
            | "AXTextArea"
            | "AXSearchField"
            | "AXSlider"
            | "AXIncrementor"
            | "AXLink"
            | "AXMenuItem"
            | "AXDisclosureTriangle"
            | "AXColorWell"
            | "AXStaticText"
    )
}

/// `AXPopUpButton` -> `popupbutton`.
pub fn short_role(ax_role: &str) -> String {
    ax_role
        .strip_prefix("AX")
        .unwrap_or(ax_role)
        .to_ascii_lowercase()
}

/// The name a person would read on an element: its title, else its
/// description (Calculator's digit buttons have no title, only a description
/// such as "1" or "Add"), else a developer identifier that reads like a word.
pub fn element_name(
    title: Option<&str>,
    description: Option<&str>,
    identifier: Option<&str>,
) -> Option<String> {
    let usable = |s: &&str| !s.trim().is_empty();
    title
        .filter(usable)
        .or(description.filter(usable))
        .or(identifier
            .filter(usable)
            // AppKit's generated ids ("_NS:9") name nothing a person sees.
            .filter(|id| !id.starts_with("_NS:")))
        .map(|s| s.trim().to_string())
}

/// Clip a value to [`MAX_VALUE_CHARS`] characters, on a char boundary.
pub fn clip_value(value: &str) -> String {
    if value.chars().count() <= MAX_VALUE_CHARS {
        value.to_string()
    } else {
        let head: String = value.chars().take(MAX_VALUE_CHARS).collect();
        format!("{head}…")
    }
}

/// Whether an element of this role should be listed with what it holds.
/// Static text with nothing in it is noise; a button is worth listing bare.
pub fn worth_listing(ax_role: &str, name: Option<&str>, value: Option<&str>) -> bool {
    if ax_role == "AXStaticText" {
        return value.is_some_and(|v| !v.trim().is_empty())
            || name.is_some_and(|n| !n.trim().is_empty());
    }
    true
}

#[cfg(target_os = "macos")]
pub use macos_impl::{
    focus, live_frame, press, press_nearest, show_menu, window_elements, window_fingerprint,
    ElementRef,
};

#[cfg(not(target_os = "macos"))]
#[derive(Clone, Debug)]
pub struct ElementRef;

#[cfg(not(target_os = "macos"))]
pub fn window_elements(
    _pid: i32,
    _window_id: u32,
    _limit: usize,
) -> Result<Vec<(ElementInfo, ElementRef)>, String> {
    Err("Reading a window's elements is only available on macOS".to_string())
}

#[cfg(not(target_os = "macos"))]
pub fn press(_element: &ElementRef) -> Result<(), String> {
    Err("Pressing an element is only available on macOS".to_string())
}

#[cfg(not(target_os = "macos"))]
pub fn show_menu(_element: &ElementRef) -> Result<(), String> {
    Err("Opening an element's menu is only available on macOS".to_string())
}

#[cfg(not(target_os = "macos"))]
pub fn focus(_element: &ElementRef) -> Result<(), String> {
    Err("Focusing an element is only available on macOS".to_string())
}

#[cfg(not(target_os = "macos"))]
pub fn live_frame(_element: &ElementRef) -> Option<(f64, f64, f64, f64)> {
    None
}

#[cfg(not(target_os = "macos"))]
pub fn press_nearest(
    _element: &ElementRef,
    _kind: crate::click_verify::ClickKind,
) -> Result<&'static str, String> {
    Err("Pressing an element is only available on macOS".to_string())
}

#[cfg(not(target_os = "macos"))]
pub fn window_fingerprint(_pid: i32, _window_id: u32) -> Option<u64> {
    None
}

#[cfg(target_os = "macos")]
mod macos_impl {
    use super::*;
    use accessibility::{AXAttribute, AXUIElement};
    use accessibility_sys::{kAXValueTypeCGPoint, kAXValueTypeCGSize, AXValueGetValue, AXValueRef};
    use core_foundation::base::{CFType, TCFType};
    use core_foundation::boolean::CFBoolean;
    use core_foundation::number::CFNumber;
    use core_foundation::string::CFString;
    use core_graphics::geometry::{CGPoint, CGSize};
    use std::collections::VecDeque;
    use std::os::raw::c_void;

    /// A live handle on one element, kept so it can be pressed later.
    ///
    /// SAFETY: an `AXUIElementRef` is an immutable Core Foundation reference
    /// to a remote element; the Accessibility API may be called on it from any
    /// thread. Same reasoning as the crate's `ThreadSafeAXUIElement`.
    #[derive(Clone)]
    pub struct ElementRef(AXUIElement);
    unsafe impl Send for ElementRef {}
    unsafe impl Sync for ElementRef {}

    impl std::fmt::Debug for ElementRef {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("ElementRef")
        }
    }

    fn string_attr(element: &AXUIElement, attr: AXAttribute<CFString>) -> Option<String> {
        element.attribute(&attr).ok().map(|s| s.to_string())
    }

    /// A value as text: strings as-is, numbers formatted, booleans as 0/1
    /// (what checkboxes report).
    fn value_text(element: &AXUIElement) -> Option<String> {
        let value: CFType = element.attribute(&AXAttribute::value()).ok()?;
        if let Some(s) = value.downcast::<CFString>() {
            return Some(s.to_string());
        }
        if let Some(n) = value.downcast::<CFNumber>() {
            if let Some(i) = n.to_i64() {
                return Some(i.to_string());
            }
            return n.to_f64().map(|f| f.to_string());
        }
        if let Some(b) = value.downcast::<CFBoolean>() {
            return Some(if bool::from(b) { "1" } else { "0" }.to_string());
        }
        None
    }

    /// Read an `AXValue` attribute (a point or a size) into `out`.
    fn read_ax_value<T>(element: &AXUIElement, name: &str, kind: u32, out: &mut T) -> bool {
        let attr = AXAttribute::<CFType>::new(&CFString::new(name));
        let Ok(value) = element.attribute(&attr) else {
            return false;
        };
        unsafe {
            AXValueGetValue(
                value.as_CFTypeRef() as AXValueRef,
                kind,
                out as *mut T as *mut c_void,
            )
        }
    }

    fn frame(element: &AXUIElement) -> Option<(f64, f64, f64, f64)> {
        let mut origin = CGPoint::new(0.0, 0.0);
        let mut size = CGSize::new(0.0, 0.0);
        let read = read_ax_value(element, "AXPosition", kAXValueTypeCGPoint, &mut origin)
            && read_ax_value(element, "AXSize", kAXValueTypeCGSize, &mut size);
        read.then_some((origin.x, origin.y, size.width, size.height))
    }

    fn find_window(pid: i32, window_id: u32) -> Result<AXUIElement, String> {
        let app = AXUIElement::application(pid);
        // A hung app must not hang the agent with it.
        let _ = app.set_messaging_timeout(1.0);
        let windows = app
            .attribute(&AXAttribute::windows())
            .map_err(|e| format!("Could not read the app's windows: {e}"))?;
        windows
            .iter()
            .find(|w| {
                crate::ax_text::window_id_of_window(w.as_concrete_TypeRef()) == Some(window_id)
            })
            .map(|w| (*w).clone())
            .ok_or_else(|| format!("Window {window_id} is not one of the app's accessible windows"))
    }

    /// The listed elements of one window, in reading order (breadth first),
    /// at most `limit` of them.
    pub fn window_elements(
        pid: i32,
        window_id: u32,
        limit: usize,
    ) -> Result<Vec<(ElementInfo, ElementRef)>, String> {
        let window = find_window(pid, window_id)?;
        let mut found = Vec::new();
        let mut queue = VecDeque::from([(window, 0usize)]);
        let mut visited = 0usize;

        while let Some((element, depth)) = queue.pop_front() {
            visited += 1;
            if visited > MAX_NODES_VISITED || found.len() >= limit {
                break;
            }
            let role = string_attr(&element, AXAttribute::role()).unwrap_or_default();
            if is_listed_role(&role) {
                if let Some(frame) = frame(&element).filter(|f| f.2 > 0.0 && f.3 > 0.0) {
                    let title = string_attr(&element, AXAttribute::title());
                    let description = string_attr(&element, AXAttribute::description());
                    let identifier = string_attr(&element, AXAttribute::identifier());
                    let name = element_name(
                        title.as_deref(),
                        description.as_deref(),
                        identifier.as_deref(),
                    );
                    let value = if role == "AXSecureTextField" {
                        None
                    } else {
                        value_text(&element)
                            .filter(|v| !v.is_empty())
                            .map(|v| clip_value(&v))
                    };
                    if worth_listing(&role, name.as_deref(), value.as_deref()) {
                        let enabled = element
                            .attribute(&AXAttribute::enabled())
                            .map(bool::from)
                            .unwrap_or(true);
                        found.push((
                            ElementInfo {
                                role: short_role(&role),
                                name,
                                value,
                                enabled,
                                frame,
                            },
                            ElementRef(element.clone()),
                        ));
                    }
                }
            }
            if depth < MAX_DEPTH {
                if let Ok(children) = element.attribute(&AXAttribute::children()) {
                    for child in children.iter() {
                        queue.push_back(((*child).clone(), depth + 1));
                    }
                }
            }
        }
        Ok(found)
    }

    /// Press an element with `AXPress`: no pointer, no focus change.
    pub fn press(element: &ElementRef) -> Result<(), String> {
        element
            .0
            .perform_action(&CFString::new("AXPress"))
            .map_err(|e| format!("The element did not accept a press: {e}"))
    }

    /// Open an element's context menu with `AXShowMenu`.
    pub fn show_menu(element: &ElementRef) -> Result<(), String> {
        element
            .0
            .perform_action(&CFString::new("AXShowMenu"))
            .map_err(|e| format!("The element has no menu to open: {e}"))
    }

    /// Run `action` on the element, or on the nearest ancestor that offers an
    /// action fit for this click (a row or cell around a title), and say which
    /// action ran. No pointer, no focus change.
    pub fn press_nearest(
        element: &ElementRef,
        kind: crate::click_verify::ClickKind,
    ) -> Result<&'static str, String> {
        let mut current = element.0.clone();
        for _ in 0..=crate::click_verify::MAX_ANCESTORS {
            let actions: Vec<String> = current
                .action_names()
                .map(|names| names.iter().map(|n| n.to_string()).collect())
                .unwrap_or_default();
            if let Some(action) = crate::click_verify::pick_ax_action(kind, &actions) {
                current
                    .perform_action(&CFString::new(action))
                    .map_err(|e| format!("The element did not accept {action}: {e}"))?;
                return Ok(action);
            }
            match current.attribute(&AXAttribute::parent()) {
                Ok(parent) => current = parent,
                Err(_) => break,
            }
        }
        Err("No action on the element or its parents fits this click".to_string())
    }

    /// A cheap hash of everything about one window a click could change:
    /// which windows the app has, which element has focus, and each listed
    /// element's role, name, value, enabled and selected state.
    ///
    /// Values that tick on their own (sliders, progress bars) are left out, so
    /// a playing track does not make every click look effective. `None` when
    /// the window cannot be read.
    pub fn window_fingerprint(pid: i32, window_id: u32) -> Option<u64> {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();

        let app = AXUIElement::application(pid);
        let _ = app.set_messaging_timeout(1.0);
        let windows = app.attribute(&AXAttribute::windows()).ok()?;
        let mut window = None;
        for w in windows.iter() {
            let id = crate::ax_text::window_id_of_window(w.as_concrete_TypeRef());
            id.hash(&mut hasher);
            if id == Some(window_id) {
                window = Some((*w).clone());
            }
        }
        let window = window?;

        if let Some(focused) = app
            .attribute(&AXAttribute::<CFType>::new(&CFString::new(
                "AXFocusedUIElement",
            )))
            .ok()
            .and_then(|value| value.downcast::<AXUIElement>())
        {
            string_attr(&focused, AXAttribute::role()).hash(&mut hasher);
            string_attr(&focused, AXAttribute::title()).hash(&mut hasher);
            value_text(&focused).hash(&mut hasher);
        }

        let selected = AXAttribute::<CFType>::new(&CFString::new("AXSelected"));
        let mut queue = VecDeque::from([(window, 0usize)]);
        let mut visited = 0usize;
        while let Some((element, depth)) = queue.pop_front() {
            visited += 1;
            if visited > FINGERPRINT_MAX_NODES {
                break;
            }
            let role = string_attr(&element, AXAttribute::role()).unwrap_or_default();
            role.hash(&mut hasher);
            string_attr(&element, AXAttribute::title()).hash(&mut hasher);
            string_attr(&element, AXAttribute::description()).hash(&mut hasher);
            if !is_self_updating_role(&role) && role != "AXSecureTextField" {
                value_text(&element).hash(&mut hasher);
            }
            element
                .attribute(&AXAttribute::enabled())
                .map(bool::from)
                .ok()
                .hash(&mut hasher);
            element
                .attribute(&selected)
                .ok()
                .and_then(|v| v.downcast::<CFBoolean>())
                .map(bool::from)
                .hash(&mut hasher);
            if depth < MAX_DEPTH {
                if let Ok(children) = element.attribute(&AXAttribute::children()) {
                    for child in children.iter() {
                        queue.push_back(((*child).clone(), depth + 1));
                    }
                }
            }
        }
        Some(hasher.finish())
    }

    /// Where an element is now, in screen points, or `None` once it is gone
    /// (its window closed, or the app rebuilt that part of its UI). A listing's
    /// frame is only true when it was read; this is read at the moment of use.
    pub fn live_frame(element: &ElementRef) -> Option<(f64, f64, f64, f64)> {
        frame(&element.0).filter(|f| f.2 > 0.0 && f.3 > 0.0)
    }

    /// Give an element keyboard focus inside its app, without raising the
    /// window, so typing that follows lands in it.
    pub fn focus(element: &ElementRef) -> Result<(), String> {
        element
            .0
            .set_attribute(&AXAttribute::focused(), CFBoolean::true_value())
            .map_err(|e| format!("The element cannot take focus: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calculator_digits_are_named_by_their_description() {
        assert_eq!(element_name(None, Some("1"), None), Some("1".to_string()));
        assert_eq!(
            element_name(Some(""), Some("Add"), Some("_NS:42")),
            Some("Add".to_string())
        );
    }

    #[test]
    fn a_title_wins_over_a_description() {
        assert_eq!(
            element_name(Some("Save"), Some("Saves the file"), None),
            Some("Save".to_string())
        );
    }

    #[test]
    fn generated_appkit_ids_are_not_names() {
        assert_eq!(element_name(None, None, Some("_NS:9")), None);
        assert_eq!(
            element_name(None, None, Some("clearButton")),
            Some("clearButton".to_string())
        );
    }

    #[test]
    fn ticking_controls_do_not_count_as_a_reaction() {
        assert!(is_self_updating_role("AXSlider"));
        assert!(is_self_updating_role("AXProgressIndicator"));
        assert!(!is_self_updating_role("AXButton"));
        assert!(!is_self_updating_role("AXRow"));
    }

    #[test]
    fn roles_read_short() {
        assert_eq!(short_role("AXPopUpButton"), "popupbutton");
        assert_eq!(short_role("AXButton"), "button");
        assert_eq!(short_role("custom"), "custom");
    }

    #[test]
    fn empty_static_text_is_not_listed_but_a_bare_button_is() {
        assert!(!worth_listing("AXStaticText", None, Some("  ")));
        assert!(worth_listing("AXStaticText", None, Some("1,035")));
        assert!(worth_listing("AXButton", None, None));
    }

    #[test]
    fn long_values_are_clipped_on_a_char_boundary() {
        let long = "界".repeat(500);
        let clipped = clip_value(&long);
        assert_eq!(clipped.chars().count(), MAX_VALUE_CHARS + 1);
        assert!(clipped.ends_with('…'));
        assert_eq!(clip_value("short"), "short");
    }

    #[test]
    fn listed_roles_cover_controls_and_text_but_not_containers() {
        assert!(is_listed_role("AXButton"));
        assert!(is_listed_role("AXStaticText"));
        assert!(!is_listed_role("AXGroup"));
        assert!(!is_listed_role("AXWindow"));
    }
}
