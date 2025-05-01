// Import the C function for setting attributes
#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    pub(crate) fn AXIsProcessTrustedWithOptions(
        options: core_foundation::dictionary::CFDictionaryRef,
    ) -> bool;
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
