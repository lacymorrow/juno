// libc is needed for pid_t in CGEventPostToPid
extern crate libc;

// Import the C function for setting attributes
#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    pub(crate) fn AXIsProcessTrustedWithOptions(
        options: core_foundation::dictionary::CFDictionaryRef,
    ) -> bool;
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
