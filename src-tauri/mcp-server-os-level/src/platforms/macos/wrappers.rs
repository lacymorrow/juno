use accessibility::AXUIElement;
use std::fmt;
use std::sync::Arc;

// Thread-safe wrapper for AXUIElement
#[derive(Clone)]
pub(crate) struct ThreadSafeAXUIElement(pub(crate) Arc<AXUIElement>); // Make inner field pub(crate)

// SAFETY: We implement Send and Sync for ThreadSafeAXUIElement because:
// 1. AXUIElement is a Core Foundation type (AXUIElementRef / CFTypeRef).
// 2. Core Foundation types are reference-counted and their retain/release
//    operations are atomic (thread-safe), per Apple documentation.
// 3. Apple's Accessibility API (AXUIElement*) functions are documented as
//    safe to call from any thread — they use Mach IPC to the target process.
// 4. The inner value is wrapped in Arc, so cloning is already thread-safe.
// 5. We do NOT mutate the AXUIElement itself — all operations go through
//    immutable AX API calls that return new values or errors.
//
// Risk: If Apple's accessibility API ever becomes thread-unsafe in a future
// macOS version, this would be unsound. This is considered unlikely given
// the Mach IPC architecture.
unsafe impl Send for ThreadSafeAXUIElement {}
unsafe impl Sync for ThreadSafeAXUIElement {}

impl ThreadSafeAXUIElement {
    #[allow(clippy::arc_with_non_send_sync)]
    pub fn new(element: AXUIElement) -> Self {
        Self(Arc::new(element))
    }

    #[allow(clippy::arc_with_non_send_sync)]
    pub fn system_wide() -> Self {
        Self(Arc::new(AXUIElement::system_wide()))
    }

    #[allow(clippy::arc_with_non_send_sync)]
    pub fn application(pid: i32) -> Self {
        Self(Arc::new(AXUIElement::application(pid)))
    }

    pub fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

// Implement Debug
impl fmt::Debug for ThreadSafeAXUIElement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("ThreadSafeAXUIElement")
            .field(&"<AXUIElement>")
            .finish()
    }
}
