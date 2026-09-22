//! Recognising Juno's own synthesized keyboard events.
//!
//! Dictation insertion and the agent's typing paths post CGEvents tagged with
//! [`computer_use_ai_sdk::SYNTHESIZED_EVENT_MARKER`] in the event-source
//! user-data field. Juno's passive NSEvent monitors ignore tagged events so a
//! synthesized keystroke can never trip a hotkey, the stop key, or a trigger.

#[cfg(target_os = "macos")]
mod imp {
    use objc::runtime::Object;
    use objc::{msg_send, sel, sel_impl};
    use std::ffi::c_void;

    /// `kCGEventSourceUserData` (CGEventField 42).
    const EVENT_SOURCE_USER_DATA: u32 = 42;

    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGEventGetIntegerValueField(event: *const c_void, field: u32) -> i64;
    }

    /// Whether an NSEvent carries Juno's synthesized-event marker.
    ///
    /// # Safety
    ///
    /// `event` must be null or a live NSEvent (AppKit hands monitors one for
    /// the duration of the handler); `CGEvent` is a plain getter.
    pub unsafe fn is_juno_synthesized_event(event: *mut Object) -> bool {
        if event.is_null() {
            return false;
        }
        let cg_event: *const c_void = msg_send![event, CGEvent];
        if cg_event.is_null() {
            return false;
        }
        CGEventGetIntegerValueField(cg_event, EVENT_SOURCE_USER_DATA)
            == computer_use_ai_sdk::SYNTHESIZED_EVENT_MARKER
    }
}

#[cfg(target_os = "macos")]
pub use imp::is_juno_synthesized_event;
