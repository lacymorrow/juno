/// Memory safety utilities for macOS API usage
/// Provides autorelease pool management and safe resource handling
use objc::{class, msg_send, sel, sel_impl};
#[allow(unused_imports)]
use std::ffi::c_void;

/// Wrapper for NSAutoreleasePool to ensure proper memory management
pub struct NSAutoreleasePool {
    pool: *mut objc::runtime::Object,
}

impl NSAutoreleasePool {
    /// Create a new autorelease pool
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        unsafe {
            let pool: *mut objc::runtime::Object = msg_send![class!(NSAutoreleasePool), new];
            Self { pool }
        }
    }
}

impl Drop for NSAutoreleasePool {
    fn drop(&mut self) {
        unsafe {
            let _: () = msg_send![self.pool, drain];
        }
    }
}

/// Execute a closure within an autorelease pool
pub fn with_autorelease_pool<F, R>(f: F) -> R
where
    F: FnOnce() -> R,
{
    let _pool = NSAutoreleasePool::new();
    f()
}

/// Safely retain an Objective-C object
///
/// # Safety
/// Caller must ensure `obj` is a valid Objective-C object pointer or null.
pub unsafe fn retain_object(obj: *mut objc::runtime::Object) -> *mut objc::runtime::Object {
    if !obj.is_null() {
        let _: *mut objc::runtime::Object = msg_send![obj, retain];
    }
    obj
}

/// Safely release an Objective-C object
///
/// # Safety
/// Caller must ensure `obj` is a valid Objective-C object pointer or null,
/// and that the object has a positive retain count.
pub unsafe fn release_object(obj: *mut objc::runtime::Object) {
    if !obj.is_null() {
        let _: () = msg_send![obj, release];
    }
}

/// Safely autorelease an Objective-C object
///
/// # Safety
/// Caller must ensure `obj` is a valid Objective-C object pointer or null,
/// and that an autorelease pool is active on the current thread.
pub unsafe fn autorelease_object(obj: *mut objc::runtime::Object) -> *mut objc::runtime::Object {
    if !obj.is_null() {
        let _: *mut objc::runtime::Object = msg_send![obj, autorelease];
    }
    obj
}

/// RAII wrapper for Core Foundation objects
pub struct CFGuard<T> {
    ptr: *mut T,
    release: unsafe extern "C" fn(*mut T),
}

impl<T> CFGuard<T> {
    pub fn new(ptr: *mut T, release: unsafe extern "C" fn(*mut T)) -> Option<Self> {
        if ptr.is_null() {
            None
        } else {
            Some(Self { ptr, release })
        }
    }

    pub fn as_ptr(&self) -> *mut T {
        self.ptr
    }
}

impl<T> Drop for CFGuard<T> {
    fn drop(&mut self) {
        unsafe {
            (self.release)(self.ptr);
        }
    }
}

/// Safe wrapper for CGImage
pub struct CGImageGuard {
    image: core_graphics::image::CGImage,
}

impl CGImageGuard {
    pub fn new(image: core_graphics::image::CGImage) -> Self {
        Self { image }
    }

    pub fn inner_ref(&self) -> &core_graphics::image::CGImage {
        &self.image
    }

    pub fn into_inner(self) -> core_graphics::image::CGImage {
        self.image
    }
}

// CGImage already implements Drop in core-graphics crate, so we don't need to implement it

/// Safe wrapper for CGEventSource
pub struct CGEventSourceGuard {
    source: Option<core_graphics::event_source::CGEventSource>,
}

impl CGEventSourceGuard {
    pub fn new() -> Result<Self, String> {
        use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};

        match CGEventSource::new(CGEventSourceStateID::HIDSystemState) {
            Ok(source) => Ok(Self {
                source: Some(source),
            }),
            Err(_) => Err("Failed to create CGEventSource".to_string()),
        }
    }

    pub fn get(&self) -> Option<&core_graphics::event_source::CGEventSource> {
        self.source.as_ref()
    }

    pub fn take(&mut self) -> Option<core_graphics::event_source::CGEventSource> {
        self.source.take()
    }
}

/// Create a new CGEventSource safely
/// Since CGEventSource doesn't implement Send/Sync, we can't pool them globally
/// Instead, we create new instances with proper error handling
pub fn get_pooled_event_source() -> Result<core_graphics::event_source::CGEventSource, String> {
    use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};

    CGEventSource::new(CGEventSourceStateID::HIDSystemState)
        .map_err(|_| "Failed to create CGEventSource".to_string())
}

/// No-op for release since we're not pooling
pub fn release_event_source(_source: core_graphics::event_source::CGEventSource) {
    // Source will be dropped automatically
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_autorelease_pool() {
        let result = with_autorelease_pool(|| {
            // Simulated work that would create autoreleased objects
            42
        });
        assert_eq!(result, 42);
    }

    #[test]
    fn test_event_source_creation() {
        // Test that we can create event sources
        let source1 = get_pooled_event_source();
        assert!(source1.is_ok(), "Should be able to create event source");

        let source2 = get_pooled_event_source();
        assert!(
            source2.is_ok(),
            "Should be able to create another event source"
        );

        // Release (no-op but should not panic)
        if let Ok(s1) = source1 {
            release_event_source(s1);
        }
        if let Ok(s2) = source2 {
            release_event_source(s2);
        }
    }
}
