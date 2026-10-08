//! Typing through the accessibility API, into the app the agent is working on,
//! and only when it can be proven to have worked.
//!
//! The old path asked for the *system-wide* focused element and set its
//! `AXValue`. In background mode a click posted to another app deliberately
//! leaves system focus alone, so that element belonged to whatever app was
//! frontmost: often Juno's own chat input. The write "succeeded", nothing
//! appeared where the agent was looking, and the tool reported success.
//!
//! Here the field is the target app's own focused element, it is checked
//! against the target's process and window before anything is written, the
//! write inserts at the caret (`AXSelectedText`) instead of replacing the whole
//! value, and the value is read back. Only a verified write is a success; every
//! other outcome is a reason to use the process-targeted paste instead.
//!
//! The check and the write are linked by type: `insert_text` takes a
//! [`CheckedField`], and the only way to get one is [`check_focused_field`],
//! which runs [`gate`]. Disconnecting the check from the write does not compile.

/// The process (and, when known, the window) a `type` action is meant for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TypeTarget {
    pub pid: i32,
    pub window_id: Option<u32>,
}

/// Why the accessibility path was not used. Each one sends the text down the
/// process-targeted paste path instead.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AxTypeSkip {
    /// No app has been targeted, so there is nothing to check a field against.
    NoTarget,
    /// The target is Juno itself. Juno never types into its own windows.
    OwnProcess,
    /// Terminals ignore `AXValue`/`AXSelectedText` writes on their surface
    /// while reporting success; keystrokes are the only reliable input.
    Terminal(String),
    /// The target app reports no focused element.
    NoFocusedElement,
    /// The focused element belongs to a different process than the target.
    WrongProcess { expected: i32, found: Option<i32> },
    /// The focused element is in another window of the target app.
    WrongWindow { expected: u32, found: Option<u32> },
    /// The focused element does not accept inserted text.
    NotEditable,
    /// The field's value cannot be read, so a write could not be verified.
    Unverifiable,
    /// The app refused the write (AXError code).
    Rejected(i32),
    /// The write returned success but the text is not in the field.
    NotVerified,
    /// No accessibility API on this platform.
    Unsupported,
}

impl AxTypeSkip {
    /// Short machine-readable label for tool results and logs.
    pub fn label(&self) -> &'static str {
        match self {
            AxTypeSkip::NoTarget => "no_target",
            AxTypeSkip::OwnProcess => "own_process",
            AxTypeSkip::Terminal(_) => "terminal",
            AxTypeSkip::NoFocusedElement => "no_focused_element",
            AxTypeSkip::WrongProcess { .. } => "wrong_process",
            AxTypeSkip::WrongWindow { .. } => "wrong_window",
            AxTypeSkip::NotEditable => "not_editable",
            AxTypeSkip::Unverifiable => "unverifiable",
            AxTypeSkip::Rejected(_) => "rejected",
            AxTypeSkip::NotVerified => "not_verified",
            AxTypeSkip::Unsupported => "unsupported",
        }
    }
}

/// What was observed about the focused element, gathered before any write.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldFacts {
    pub pid: Option<i32>,
    pub window_id: Option<u32>,
    pub accepts_inserted_text: bool,
    pub value_readable: bool,
}

/// Bundle ids of terminal emulators. Their text surfaces expose the screen as
/// an AX text area, but writing to it does not reach the shell.
pub const TERMINAL_BUNDLE_IDS: &[&str] = &[
    "com.mitchellh.ghostty",
    "com.apple.Terminal",
    "com.googlecode.iterm2",
    "net.kovidgoyal.kitty",
    "org.alacritty",
    "io.alacritty",
    "dev.warp.Warp-Stable",
    "dev.warp.Warp",
    "com.github.wez.wezterm",
    "co.zeit.hyper",
    "com.raphaelamorim.rio",
    "org.tabby",
];

pub fn is_terminal_bundle(bundle_id: &str) -> bool {
    TERMINAL_BUNDLE_IDS
        .iter()
        .any(|t| t.eq_ignore_ascii_case(bundle_id))
}

/// Every check that must pass before a single character is written.
pub fn gate(
    target: &TypeTarget,
    own_pid: i32,
    bundle_id: Option<&str>,
    facts: &FieldFacts,
) -> Result<(), AxTypeSkip> {
    if target.pid <= 0 {
        return Err(AxTypeSkip::NoTarget);
    }
    if target.pid == own_pid || facts.pid == Some(own_pid) {
        return Err(AxTypeSkip::OwnProcess);
    }
    if let Some(bundle) = bundle_id.filter(|b| is_terminal_bundle(b)) {
        return Err(AxTypeSkip::Terminal(bundle.to_string()));
    }
    if facts.pid != Some(target.pid) {
        return Err(AxTypeSkip::WrongProcess {
            expected: target.pid,
            found: facts.pid,
        });
    }
    if let Some(expected) = target.window_id {
        if facts.window_id != Some(expected) {
            return Err(AxTypeSkip::WrongWindow {
                expected,
                found: facts.window_id,
            });
        }
    }
    if !facts.accepts_inserted_text {
        return Err(AxTypeSkip::NotEditable);
    }
    if !facts.value_readable {
        return Err(AxTypeSkip::Unverifiable);
    }
    Ok(())
}

/// Did writing `text` change the field and leave `text` in it?
pub fn insertion_verified(before: &str, after: &str, text: &str) -> bool {
    !text.is_empty() && after != before && after.contains(text)
}

/// A successful, verified accessibility write.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AxTypeReport {
    pub pid: i32,
    pub window_id: Option<u32>,
    pub method: &'static str,
}

/// Who an input action reached, for tool results.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TargetInfo {
    pub app: Option<String>,
    pub bundle_id: Option<String>,
    pub pid: Option<i32>,
    pub window_id: Option<u32>,
    pub window_title: Option<String>,
}

#[cfg(target_os = "macos")]
pub use macos_impl::{bundle_id_for_pid, check_focused_field, describe_target, insert_text};

#[cfg(target_os = "macos")]
pub use macos_impl::{element_owner, CheckedField};

#[cfg(target_os = "macos")]
pub(crate) use macos_impl::window_id_of_window;

#[cfg(not(target_os = "macos"))]
pub fn element_owner(_element: &crate::UIElement) -> (Option<i32>, Option<u32>) {
    (None, None)
}

/// Check then write, in one call. The usual entry point.
#[cfg(target_os = "macos")]
pub fn type_verified(target: &TypeTarget, text: &str) -> Result<AxTypeReport, AxTypeSkip> {
    let own_pid = std::process::id() as i32;
    let field = check_focused_field(target, own_pid)?;
    insert_text(field, text)
}

#[cfg(not(target_os = "macos"))]
pub fn type_verified(_target: &TypeTarget, _text: &str) -> Result<AxTypeReport, AxTypeSkip> {
    Err(AxTypeSkip::Unsupported)
}

#[cfg(not(target_os = "macos"))]
pub fn describe_target(pid: Option<i32>, window_id: Option<u32>) -> TargetInfo {
    TargetInfo {
        pid,
        window_id,
        ..TargetInfo::default()
    }
}

#[cfg(target_os = "macos")]
mod macos_impl {
    use super::*;
    use accessibility::AXUIElement;
    use accessibility_sys::{
        AXUIElementCopyAttributeValue, AXUIElementCreateApplication, AXUIElementGetPid,
        AXUIElementIsAttributeSettable, AXUIElementRef, AXUIElementSetAttributeValue,
    };
    use core_foundation::base::{CFRelease, CFTypeRef, TCFType};
    use core_foundation::string::{CFString, CFStringRef};
    use std::os::raw::c_void;
    use std::sync::OnceLock;

    /// A focused field that passed [`gate`], with its value before the write.
    /// Fields are private: [`check_focused_field`] is the only constructor.
    pub struct CheckedField {
        element: AXUIElement,
        pid: i32,
        window_id: Option<u32>,
        before: String,
    }

    fn copy_attribute(element: AXUIElementRef, name: &str) -> Option<CFTypeRef> {
        let attr = CFString::new(name);
        let mut value: CFTypeRef = std::ptr::null();
        let err = unsafe {
            AXUIElementCopyAttributeValue(element, attr.as_concrete_TypeRef(), &mut value)
        };
        if err == 0 && !value.is_null() {
            Some(value)
        } else {
            None
        }
    }

    /// Read a string attribute. Takes ownership of nothing it is not given.
    fn string_attribute(element: AXUIElementRef, name: &str) -> Option<String> {
        let value = copy_attribute(element, name)?;
        unsafe {
            if core_foundation_sys::base::CFGetTypeID(value)
                != core_foundation_sys::string::CFStringGetTypeID()
            {
                CFRelease(value);
                return None;
            }
            Some(CFString::wrap_under_create_rule(value as CFStringRef).to_string())
        }
    }

    fn is_settable(element: AXUIElementRef, name: &str) -> bool {
        let attr = CFString::new(name);
        let mut settable: u8 = 0;
        let err = unsafe {
            AXUIElementIsAttributeSettable(element, attr.as_concrete_TypeRef(), &mut settable)
        };
        err == 0 && settable != 0
    }

    fn element_pid(element: AXUIElementRef) -> Option<i32> {
        let mut pid: libc::pid_t = 0;
        let err = unsafe { AXUIElementGetPid(element, &mut pid) };
        (err == 0 && pid > 0).then_some(pid)
    }

    type AXGetWindowFn = unsafe extern "C" fn(AXUIElementRef, *mut u32) -> i32;

    /// `_AXUIElementGetWindow`: private but long-stable (yabai, Hammerspoon and
    /// Rectangle all rely on it). Looked up at runtime so a missing symbol
    /// degrades to "window unknown" instead of a link failure.
    fn ax_get_window_fn() -> Option<AXGetWindowFn> {
        static FN: OnceLock<Option<AXGetWindowFn>> = OnceLock::new();
        *FN.get_or_init(|| unsafe {
            let sym = libc::dlsym(libc::RTLD_DEFAULT, c"_AXUIElementGetWindow".as_ptr());
            if sym.is_null() {
                None
            } else {
                Some(std::mem::transmute::<*mut c_void, AXGetWindowFn>(sym))
            }
        })
    }

    pub(crate) fn window_id_of_window(window: AXUIElementRef) -> Option<u32> {
        let get = ax_get_window_fn()?;
        let mut id: u32 = 0;
        let err = unsafe { get(window, &mut id) };
        (err == 0 && id > 0).then_some(id)
    }

    /// The CGWindowID of the window that contains `element`.
    fn element_window_id(element: AXUIElementRef) -> Option<u32> {
        let window = copy_attribute(element, "AXWindow")?;
        let id = window_id_of_window(window as AXUIElementRef);
        unsafe { CFRelease(window) };
        id
    }

    /// The process and window an element belongs to.
    pub fn element_owner(element: &crate::UIElement) -> (Option<i32>, Option<u32>) {
        let Some(mac) = element
            .as_any()
            .downcast_ref::<crate::platforms::macos::MacOSUIElement>()
        else {
            return (None, None);
        };
        let raw = mac.element.0.as_concrete_TypeRef();
        (element_pid(raw), element_window_id(raw))
    }

    /// Bundle identifier of a running process.
    pub fn bundle_id_for_pid(pid: i32) -> Option<String> {
        running_app_string(pid, "bundleIdentifier")
    }

    fn running_app_string(pid: i32, selector: &str) -> Option<String> {
        use objc::{class, msg_send, sel, sel_impl};
        unsafe {
            let app: *mut objc::runtime::Object = msg_send![
                class!(NSRunningApplication),
                runningApplicationWithProcessIdentifier: pid
            ];
            if app.is_null() {
                return None;
            }
            let value: *mut objc::runtime::Object = match selector {
                "bundleIdentifier" => msg_send![app, bundleIdentifier],
                _ => msg_send![app, localizedName],
            };
            if value.is_null() {
                return None;
            }
            let bytes: *const libc::c_char = msg_send![value, UTF8String];
            if bytes.is_null() {
                return None;
            }
            Some(
                std::ffi::CStr::from_ptr(bytes)
                    .to_string_lossy()
                    .into_owned(),
            )
        }
    }

    /// Find the target app's own focused element and run every check on it.
    ///
    /// Never consults the system-wide focused element: that one belongs to the
    /// frontmost app, which in background mode is by design not the target.
    pub fn check_focused_field(
        target: &TypeTarget,
        own_pid: i32,
    ) -> Result<CheckedField, AxTypeSkip> {
        if target.pid <= 0 {
            return Err(AxTypeSkip::NoTarget);
        }
        if target.pid == own_pid {
            return Err(AxTypeSkip::OwnProcess);
        }
        let bundle = bundle_id_for_pid(target.pid);
        if let Some(b) = bundle.as_deref().filter(|b| is_terminal_bundle(b)) {
            return Err(AxTypeSkip::Terminal(b.to_string()));
        }

        let app = unsafe { AXUIElementCreateApplication(target.pid) };
        if app.is_null() {
            return Err(AxTypeSkip::NoFocusedElement);
        }
        let app = unsafe { AXUIElement::wrap_under_create_rule(app) };
        let focused = copy_attribute(app.as_concrete_TypeRef(), "AXFocusedUIElement")
            .ok_or(AxTypeSkip::NoFocusedElement)?;
        // Owned from here on, released on every path by the wrapper.
        let element = unsafe { AXUIElement::wrap_under_create_rule(focused as AXUIElementRef) };
        let raw = element.as_concrete_TypeRef();

        let before = string_attribute(raw, "AXValue");
        let facts = FieldFacts {
            pid: element_pid(raw),
            window_id: element_window_id(raw),
            accepts_inserted_text: is_settable(raw, "AXSelectedText"),
            value_readable: before.is_some(),
        };
        gate(target, own_pid, bundle.as_deref(), &facts)?;

        Ok(CheckedField {
            element,
            pid: target.pid,
            window_id: facts.window_id,
            before: before.unwrap_or_default(),
        })
    }

    /// Insert at the caret of a checked field and confirm the text landed.
    pub fn insert_text(field: CheckedField, text: &str) -> Result<AxTypeReport, AxTypeSkip> {
        let raw = field.element.as_concrete_TypeRef();
        let attr = CFString::new("AXSelectedText");
        let value = CFString::new(text);
        let err = unsafe {
            AXUIElementSetAttributeValue(
                raw,
                attr.as_concrete_TypeRef(),
                value.as_concrete_TypeRef() as CFTypeRef,
            )
        };
        if err != 0 {
            return Err(AxTypeSkip::Rejected(err));
        }

        // Most apps apply the write synchronously; web content can take a beat.
        for delay_ms in [0u64, 60] {
            if delay_ms > 0 {
                std::thread::sleep(std::time::Duration::from_millis(delay_ms));
            }
            let after = string_attribute(raw, "AXValue").unwrap_or_default();
            if insertion_verified(&field.before, &after, text) {
                return Ok(AxTypeReport {
                    pid: field.pid,
                    window_id: field.window_id,
                    method: "AXSelectedText",
                });
            }
        }
        Err(AxTypeSkip::NotVerified)
    }

    /// Name the app and window an action reached. With no window id, the app's
    /// focused window is reported, since that is where keystrokes land.
    pub fn describe_target(pid: Option<i32>, window_id: Option<u32>) -> TargetInfo {
        let Some(pid) = pid.filter(|p| *p > 0) else {
            return TargetInfo {
                window_id,
                ..TargetInfo::default()
            };
        };
        let mut info = TargetInfo {
            app: running_app_string(pid, "localizedName"),
            bundle_id: bundle_id_for_pid(pid),
            pid: Some(pid),
            window_id,
            window_title: None,
        };

        if let Some(id) = window_id {
            info.window_title = crate::platforms::macos::display::list_window_records()
                .into_iter()
                .find(|w| w.id == id)
                .and_then(|w| w.title);
            return info;
        }

        let app = unsafe { AXUIElementCreateApplication(pid) };
        if app.is_null() {
            return info;
        }
        let app = unsafe { AXUIElement::wrap_under_create_rule(app) };
        if let Some(window) = copy_attribute(app.as_concrete_TypeRef(), "AXFocusedWindow") {
            let window = unsafe { AXUIElement::wrap_under_create_rule(window as AXUIElementRef) };
            info.window_id = window_id_of_window(window.as_concrete_TypeRef());
            info.window_title = string_attribute(window.as_concrete_TypeRef(), "AXTitle");
        }
        info
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const JUNO: i32 = 100;
    const GHOSTTY: i32 = 200;
    const EDITOR: i32 = 300;

    fn facts(pid: i32, window: Option<u32>) -> FieldFacts {
        FieldFacts {
            pid: Some(pid),
            window_id: window,
            accepts_inserted_text: true,
            value_readable: true,
        }
    }

    fn target(pid: i32, window: Option<u32>) -> TypeTarget {
        TypeTarget {
            pid,
            window_id: window,
        }
    }

    #[test]
    fn a_matching_editable_field_passes() {
        assert_eq!(
            gate(
                &target(EDITOR, Some(7)),
                JUNO,
                Some("com.apple.TextEdit"),
                &facts(EDITOR, Some(7))
            ),
            Ok(())
        );
    }

    #[test]
    fn junos_own_chat_input_is_never_written_even_if_it_holds_focus() {
        // The observed bug: Juno frontmost, its chat input the focused element.
        assert_eq!(
            gate(&target(EDITOR, None), JUNO, None, &facts(JUNO, None)),
            Err(AxTypeSkip::OwnProcess)
        );
        assert_eq!(
            gate(&target(JUNO, None), JUNO, None, &facts(JUNO, None)),
            Err(AxTypeSkip::OwnProcess)
        );
    }

    #[test]
    fn a_field_in_another_process_is_refused() {
        assert_eq!(
            gate(&target(EDITOR, None), JUNO, None, &facts(GHOSTTY, None)),
            Err(AxTypeSkip::WrongProcess {
                expected: EDITOR,
                found: Some(GHOSTTY)
            })
        );
    }

    #[test]
    fn a_field_in_another_window_of_the_same_app_is_refused() {
        assert_eq!(
            gate(
                &target(EDITOR, Some(7)),
                JUNO,
                None,
                &facts(EDITOR, Some(8))
            ),
            Err(AxTypeSkip::WrongWindow {
                expected: 7,
                found: Some(8)
            })
        );
        // Unknown window counts as a mismatch, never as a pass.
        assert_eq!(
            gate(&target(EDITOR, Some(7)), JUNO, None, &facts(EDITOR, None)),
            Err(AxTypeSkip::WrongWindow {
                expected: 7,
                found: None
            })
        );
    }

    #[test]
    fn terminals_skip_the_accessibility_path() {
        for bundle in [
            "com.mitchellh.ghostty",
            "com.apple.Terminal",
            "com.googlecode.iterm2",
        ] {
            assert_eq!(
                gate(
                    &target(GHOSTTY, None),
                    JUNO,
                    Some(bundle),
                    &facts(GHOSTTY, None)
                ),
                Err(AxTypeSkip::Terminal(bundle.to_string()))
            );
        }
    }

    #[test]
    fn unwritable_or_unreadable_fields_are_refused() {
        let mut f = facts(EDITOR, None);
        f.accepts_inserted_text = false;
        assert_eq!(
            gate(&target(EDITOR, None), JUNO, None, &f),
            Err(AxTypeSkip::NotEditable)
        );
        let mut f = facts(EDITOR, None);
        f.value_readable = false;
        assert_eq!(
            gate(&target(EDITOR, None), JUNO, None, &f),
            Err(AxTypeSkip::Unverifiable)
        );
    }

    #[test]
    fn no_target_means_no_accessibility_write() {
        assert_eq!(
            gate(&target(0, None), JUNO, None, &facts(EDITOR, None)),
            Err(AxTypeSkip::NoTarget)
        );
    }

    #[test]
    fn verification_needs_the_text_to_actually_appear() {
        assert!(insertion_verified("hello ", "hello world", "world"));
        assert!(insertion_verified("", "world", "world"));
        // Reported success, value unchanged: the Ghostty case.
        assert!(!insertion_verified("$ ", "$ ", "ls"));
        // Value changed but not to include the text.
        assert!(!insertion_verified("abc", "ab", "xyz"));
        // Text was already there and nothing changed.
        assert!(!insertion_verified("ls", "ls", "ls"));
        assert!(!insertion_verified("a", "ab", ""));
    }

    #[test]
    fn skip_labels_are_stable() {
        assert_eq!(AxTypeSkip::OwnProcess.label(), "own_process");
        assert_eq!(AxTypeSkip::Terminal(String::new()).label(), "terminal");
        assert_eq!(AxTypeSkip::NotVerified.label(), "not_verified");
    }
}
