//! macOS cursor scaling via Accessibility preferences.
//!
//! Sets `mouseDriverCursorSize` in `com.apple.universalaccess` — the same
//! preference macOS System Settings → Accessibility → Display → Pointer Size
//! uses. Works on all macOS versions including macOS 16+ where the older
//! CGSSetCursorScale private API was removed.

#[cfg(target_os = "macos")]
mod inner {
    use std::process::Command;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Mutex;
    use tracing::{info, warn};

    static SCALE_REFCOUNT: AtomicUsize = AtomicUsize::new(0);
    static ORIGINAL_SIZE: Mutex<Option<f64>> = Mutex::new(None);

    fn read_cursor_size() -> f64 {
        Command::new("defaults")
            .args(["read", "com.apple.universalaccess", "mouseDriverCursorSize"])
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .and_then(|s| s.trim().parse::<f64>().ok())
            .unwrap_or(1.0)
    }

    fn write_cursor_size(size: f64) {
        let result = Command::new("defaults")
            .args([
                "write",
                "com.apple.universalaccess",
                "mouseDriverCursorSize",
                "-float",
                &format!("{:.2}", size),
            ])
            .output();

        match result {
            Ok(o) if o.status.success() => {
                info!("[CursorScale] Wrote mouseDriverCursorSize={:.2}", size);
            }
            Ok(o) => {
                warn!(
                    "[CursorScale] defaults write failed: {}",
                    String::from_utf8_lossy(&o.stderr)
                );
            }
            Err(e) => {
                warn!("[CursorScale] Failed to run defaults: {}", e);
            }
        }

        // Post Darwin notifications to nudge the accessibility subsystem into
        // picking up the new preference value. Multiple names for cross-version
        // coverage — notifyutil is instant (no compilation unlike swift -e).
        for name in &[
            "com.apple.accessibility.cache.cursor",
            "com.apple.universalaccess.prefChanged",
        ] {
            let _ = Command::new("notifyutil").args(["-p", name]).output();
        }
    }

    pub fn set_cursor_scale(scale: f64) {
        let scale = scale.clamp(1.0, 10.0);

        // Save original size on first call
        if let Ok(mut orig) = ORIGINAL_SIZE.lock() {
            if orig.is_none() {
                let current = read_cursor_size();
                info!("[CursorScale] Saved original cursor size: {:.2}", current);
                *orig = Some(current);
            }
        }

        write_cursor_size(scale);
        SCALE_REFCOUNT.fetch_add(1, Ordering::Release);
        info!(
            "[CursorScale] Set cursor scale to {:.1} (refs: {})",
            scale,
            SCALE_REFCOUNT.load(Ordering::Acquire)
        );
    }

    pub fn restore_cursor_scale() {
        let did_dec = SCALE_REFCOUNT.fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| {
            if n > 0 { Some(n - 1) } else { None }
        });
        match did_dec {
            Ok(1) => {
                let orig = ORIGINAL_SIZE
                    .lock()
                    .ok()
                    .and_then(|mut g| g.take())
                    .unwrap_or(1.0);
                write_cursor_size(orig);
                info!(
                    "[CursorScale] Restored cursor to original size {:.2} (last guard dropped)",
                    orig
                );
            }
            Ok(prev) => {
                info!(
                    "[CursorScale] Guard dropped, {} agent(s) still active",
                    prev - 1
                );
            }
            Err(_) => {
                info!("[CursorScale] restore_cursor_scale called with zero refs — no-op");
            }
        }
    }

    pub fn force_restore_cursor_scale() {
        SCALE_REFCOUNT.store(0, Ordering::Release);
        let orig = ORIGINAL_SIZE
            .lock()
            .ok()
            .and_then(|mut g| g.take())
            .unwrap_or(1.0);
        write_cursor_size(orig);
        info!(
            "[CursorScale] Force-restored cursor to original size {:.2} (user disabled)",
            orig
        );
    }

    pub fn is_cursor_scaled() -> bool {
        SCALE_REFCOUNT.load(Ordering::Acquire) > 0
    }

    pub fn get_system_cursor_size() -> f64 {
        read_cursor_size()
    }
}

#[cfg(not(target_os = "macos"))]
mod inner {
    pub fn set_cursor_scale(_scale: f64) {}
    pub fn restore_cursor_scale() {}
    pub fn force_restore_cursor_scale() {}
    pub fn is_cursor_scaled() -> bool {
        false
    }
    pub fn get_system_cursor_size() -> f64 {
        1.0
    }
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
