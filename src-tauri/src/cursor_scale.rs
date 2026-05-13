//! macOS cursor scaling via CoreGraphics private API.
//!
//! Uses `CGSMainConnectionID` and `CGSSetCursorScale` to change the system
//! cursor size at runtime. These private APIs have been stable since macOS 10.6
//! and are the same mechanism macOS uses for the "shake to locate" cursor feature.

#[cfg(target_os = "macos")]
mod inner {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tracing::info;
    use core_graphics::event::{CGEvent, CGEventTapLocation, CGEventType, CGMouseButton};
    use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};

    /// Ref-count of active cursor-scale guards. Only restores to 1.0
    /// when the last guard drops, so concurrent agents don't fight.
    static SCALE_REFCOUNT: AtomicUsize = AtomicUsize::new(0);

    type CGSConnectionID = u32;

    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGSMainConnectionID() -> CGSConnectionID;
        fn CGSSetCursorScale(cid: CGSConnectionID, scale: f64);
    }

    /// Post a synthetic mouse-moved event at the current cursor position
    /// to force macOS to redraw the cursor at the new scale.
    fn refresh_cursor() {
        if let Ok(source) = CGEventSource::new(CGEventSourceStateID::HIDSystemState) {
            if let Ok(dummy) = CGEvent::new(source.clone()) {
                let pos = dummy.location();
                if let Ok(ev) = CGEvent::new_mouse_event(
                    source,
                    CGEventType::MouseMoved,
                    pos,
                    CGMouseButton::Left,
                ) {
                    ev.post(CGEventTapLocation::HID);
                }
            }
        }
    }

    pub fn set_cursor_scale(scale: f64) {
        let scale = scale.clamp(1.0, 10.0);
        unsafe {
            let cid = CGSMainConnectionID();
            CGSSetCursorScale(cid, scale);
        }
        SCALE_REFCOUNT.fetch_add(1, Ordering::Release);
        refresh_cursor();
        info!("[CursorScale] Set cursor scale to {:.1} (refs: {})", scale, SCALE_REFCOUNT.load(Ordering::Acquire));
    }

    pub fn restore_cursor_scale() {
        let did_dec = SCALE_REFCOUNT.fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| {
            if n > 0 { Some(n - 1) } else { None }
        });
        match did_dec {
            Ok(1) => {
                unsafe {
                    let cid = CGSMainConnectionID();
                    CGSSetCursorScale(cid, 1.0);
                }
                refresh_cursor();
                info!("[CursorScale] Restored cursor scale to 1.0 (last guard dropped)");
            }
            Ok(prev) => {
                info!("[CursorScale] Guard dropped, {} agent(s) still active", prev - 1);
            }
            Err(_) => {
                info!("[CursorScale] restore_cursor_scale called with zero refs — no-op");
            }
        }
    }

    /// Force-reset cursor to normal size, clearing the ref-count.
    /// Used when the user explicitly disables big cursor via settings.
    pub fn force_restore_cursor_scale() {
        SCALE_REFCOUNT.store(0, Ordering::Release);
        unsafe {
            let cid = CGSMainConnectionID();
            CGSSetCursorScale(cid, 1.0);
        }
        refresh_cursor();
        info!("[CursorScale] Force-restored cursor scale to 1.0 (user disabled)");
    }

    pub fn is_cursor_scaled() -> bool {
        SCALE_REFCOUNT.load(Ordering::Acquire) > 0
    }
}

#[cfg(not(target_os = "macos"))]
mod inner {
    pub fn set_cursor_scale(_scale: f64) {}
    pub fn restore_cursor_scale() {}
    pub fn force_restore_cursor_scale() {}
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
