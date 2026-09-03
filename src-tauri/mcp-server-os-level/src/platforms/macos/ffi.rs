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
    #[allow(dead_code)]
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
