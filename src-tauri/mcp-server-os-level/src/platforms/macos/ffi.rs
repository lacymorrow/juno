// libc is needed for pid_t in CGEventPostToPid
extern crate libc;

// Import the C function for setting attributes
#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    pub(crate) fn AXIsProcessTrustedWithOptions(
        options: core_foundation::dictionary::CFDictionaryRef,
    ) -> bool;

    /// Convert a PID to a ProcessSerialNumber (deprecated since 10.9 but still present
    /// in macOS 13-15; used only by the SLPSPostEventRecordTo focus-without-raise path).
    pub(crate) fn GetProcessForPID(pid: libc::pid_t, psn: *mut ProcessSerialNumber) -> i32;
}

/// Carbon ProcessSerialNumber — required by SLPSPostEventRecordTo.
/// Layout matches `struct ProcessSerialNumber { UInt32 high; UInt32 low; }`.
#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct ProcessSerialNumber {
    pub high_long_of_psn: u32,
    pub low_long_of_psn: u32,
}

// Screen recording permission APIs from CoreGraphics
#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    /// Check if the app has screen recording permission without prompting
    /// Returns true if permission is granted, false otherwise
    pub fn CGPreflightScreenCaptureAccess() -> bool;

    /// Request screen recording permission, may show a system prompt
    /// Returns true if permission is granted after request
    pub fn CGRequestScreenCaptureAccess() -> bool;

    /// Post a CGEvent to a specific process by PID without moving the system cursor.
    /// The event's position field is metadata for the target process only.
    /// Public API since macOS 10.11. Declared here because the core-graphics crate
    /// does not expose this function.
    pub(crate) fn CGEventPostToPid(pid: libc::pid_t, event: *mut ::std::os::raw::c_void);

    /// Set the location an event reports to the app that receives it. Declared
    /// here because the core-graphics crate exposes only the getter, and a
    /// scroll posted to a process still has to say where it happened.
    pub(crate) fn CGEventSetLocation(
        event: *mut ::std::os::raw::c_void,
        location: core_graphics::geometry::CGPoint,
    );
}

// Add these extern "C" declarations if not already present
extern "C" {
    pub(crate) fn AXValueCreate(
        type_: u32,
        value_ptr: *const ::std::os::raw::c_void,
    ) -> *const ::std::os::raw::c_void; // Returns AXValueRef which is a CFTypeRef

    pub(crate) fn AXValueGetValue(
        value: *const ::std::os::raw::c_void,
        type_: u32,
        out: *mut ::std::os::raw::c_void,
    ) -> i32;
}

// Keyboard layout resolution (Text Input Sources + UCKeyTranslate), used to
// find the key code that produces "v" under the Command modifier in the
// current layout. Dvorak-QWERTY-Command maps the Command layer differently
// from the unmodified layout, so the code cannot be assumed to be ANSI V.
#[link(name = "Carbon", kind = "framework")]
extern "C" {
    /// The keyboard layout the current input source resolves to
    /// (a `TISInputSourceRef`, returned owned per the Copy rule).
    pub(crate) fn TISCopyCurrentKeyboardLayoutInputSource() -> *mut ::std::os::raw::c_void;

    /// Borrowed property of an input source (`CFTypeRef`, NOT owned).
    pub(crate) fn TISGetInputSourceProperty(
        input_source: *mut ::std::os::raw::c_void,
        property_key: core_foundation::string::CFStringRef,
    ) -> *mut ::std::os::raw::c_void;

    /// Property key: `CFDataRef` holding the 'uchr' keyboard layout data.
    pub(crate) static kTISPropertyUnicodeKeyLayoutData: core_foundation::string::CFStringRef;

    /// Property key: `CFStringRef` uniquely identifying the input source.
    pub(crate) static kTISPropertyInputSourceID: core_foundation::string::CFStringRef;

    /// Translate a virtual key code + modifier state to the characters it
    /// would produce under the given 'uchr' layout data.
    pub(crate) fn UCKeyTranslate(
        key_layout_ptr: *const ::std::os::raw::c_void,
        virtual_key_code: u16,
        key_action: u16,
        modifier_key_state: u32,
        keyboard_type: u32,
        key_translate_options: u32,
        dead_key_state: *mut u32,
        max_string_length: libc::c_ulong,
        actual_string_length: *mut libc::c_ulong,
        unicode_string: *mut u16,
    ) -> i32;

    /// The hardware keyboard type UCKeyTranslate expects.
    pub(crate) fn LMGetKbdType() -> u8;
}
