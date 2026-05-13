//! macOS cursor scaling via CoreGraphics private API.
//!
//! Uses `CGSMainConnectionID` and `CGSSetCursorScale` to change the system
//! cursor size at runtime. These private APIs have been stable since macOS 10.6
//! and are the same mechanism macOS uses for the "shake to locate" cursor feature.

#[cfg(target_os = "macos")]
mod inner {
    use std::sync::atomic::{AtomicBool, Ordering};
    use tracing::info;

    static CURSOR_SCALED: AtomicBool = AtomicBool::new(false);

    type CGSConnectionID = u32;

    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGSMainConnectionID() -> CGSConnectionID;
        fn CGSSetCursorScale(cid: CGSConnectionID, scale: f64);
    }

    pub fn set_cursor_scale(scale: f64) {
        let scale = scale.clamp(1.0, 10.0);
        unsafe {
            let cid = CGSMainConnectionID();
            CGSSetCursorScale(cid, scale);
        }
        CURSOR_SCALED.store(scale > 1.0, Ordering::Relaxed);
        info!("[CursorScale] Set cursor scale to {:.1}", scale);
    }

    pub fn restore_cursor_scale() {
        if CURSOR_SCALED.swap(false, Ordering::Relaxed) {
            unsafe {
                let cid = CGSMainConnectionID();
                CGSSetCursorScale(cid, 1.0);
            }
            info!("[CursorScale] Restored cursor scale to 1.0");
        }
    }

    pub fn is_cursor_scaled() -> bool {
        CURSOR_SCALED.load(Ordering::Relaxed)
    }
}

#[cfg(not(target_os = "macos"))]
mod inner {
    pub fn set_cursor_scale(_scale: f64) {}
    pub fn restore_cursor_scale() {}
    pub fn is_cursor_scaled() -> bool { false }
}

pub use inner::*;

/// RAII guard that restores cursor scale when dropped.
/// Ensures the cursor returns to normal size on every exit path —
/// success, error, or panic.
pub struct CursorScaleGuard {
    active: bool,
}

impl CursorScaleGuard {
    pub fn new(scale: f64) -> Self {
        set_cursor_scale(scale);
        Self { active: true }
    }

    pub fn noop() -> Self {
        Self { active: false }
    }
}

impl Drop for CursorScaleGuard {
    fn drop(&mut self) {
        if self.active {
            restore_cursor_scale();
        }
    }
}
