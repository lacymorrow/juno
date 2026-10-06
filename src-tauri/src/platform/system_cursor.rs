//! The shape of the system cursor, so the overlay can glow in its outline.
//!
//! When Juno moves the person's real cursor, WindowServer draws that cursor
//! above every window, the overlay included. The overlay therefore draws a
//! soft tinted silhouette of the same shape just behind it, which reads as a
//! halo around an arrow, an I-beam, a hand or a resize cursor alike.
//!
//! Read on the main thread (AppKit), only when Juno has just moved the cursor,
//! and sent only when the shape changed: an unchanged cursor costs one hash.

use serde::Serialize;
use tauri::AppHandle;

/// One cursor image, in points.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct CursorShape {
    /// PNG, as a data URL.
    pub image: String,
    /// The point in the image that is the click point, from its top left.
    pub hotspot_x: f64,
    pub hotspot_y: f64,
    pub width: f64,
    pub height: f64,
}

/// Read the system cursor and send its shape to the overlay if it changed.
pub fn refresh(app: &AppHandle) {
    #[cfg(target_os = "macos")]
    imp::refresh(app);
    #[cfg(not(target_os = "macos"))]
    let _ = app;
}

#[cfg(target_os = "macos")]
mod imp {
    use super::CursorShape;
    use base64::Engine;
    use cocoa::base::{id, nil};
    use cocoa::foundation::{NSPoint, NSSize};
    use objc::{class, msg_send, sel, sel_impl};
    use std::hash::{Hash, Hasher};
    use std::sync::Mutex;
    use tauri::{AppHandle, Emitter};

    /// `NSBitmapImageFileTypePNG`.
    const NS_BITMAP_IMAGE_FILE_TYPE_PNG: usize = 4;

    /// Fingerprint of the last shape sent.
    static LAST_SENT: Mutex<Option<u64>> = Mutex::new(None);

    pub fn refresh(app: &AppHandle) {
        let handle = app.clone();
        if let Err(e) = app.run_on_main_thread(move || {
            let shape =
                crate::utils::resource_manager::with_autorelease_pool(read_if_changed_on_main);
            let Some(shape) = shape else {
                return;
            };
            if let Err(e) = handle.emit(crate::constants::events::ui::AGENT_CURSOR_SHAPE, &shape) {
                tracing::debug!("agent cursor shape emit failed: {}", e);
            }
        }) {
            tracing::debug!("Could not read the system cursor: {}", e);
        }
    }

    /// Main thread only.
    fn read_if_changed_on_main() -> Option<CursorShape> {
        // SAFETY: called on the main thread inside an autorelease pool; every
        // selector is a documented AppKit/Foundation method, and every object
        // is checked for nil before use. The byte slices are copied before
        // the pool drains.
        unsafe {
            let cursor: id = msg_send![class!(NSCursor), currentSystemCursor];
            if cursor == nil {
                return None;
            }
            let image: id = msg_send![cursor, image];
            if image == nil {
                return None;
            }
            let hotspot: NSPoint = msg_send![cursor, hotSpot];
            let size: NSSize = msg_send![image, size];
            let tiff: id = msg_send![image, TIFFRepresentation];
            let tiff_bytes = data_bytes(tiff)?;

            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            tiff_bytes.hash(&mut hasher);
            hotspot.x.to_bits().hash(&mut hasher);
            hotspot.y.to_bits().hash(&mut hasher);
            let fingerprint = hasher.finish();
            if let Ok(last) = LAST_SENT.lock() {
                if *last == Some(fingerprint) {
                    return None;
                }
            }

            let rep: id = msg_send![class!(NSBitmapImageRep), imageRepWithData: tiff];
            if rep == nil {
                return None;
            }
            let props: id = msg_send![class!(NSDictionary), dictionary];
            let png: id = msg_send![rep, representationUsingType: NS_BITMAP_IMAGE_FILE_TYPE_PNG properties: props];
            let png_bytes = data_bytes(png)?;
            let encoded = base64::engine::general_purpose::STANDARD.encode(png_bytes);

            if let Ok(mut last) = LAST_SENT.lock() {
                *last = Some(fingerprint);
            }
            Some(CursorShape {
                image: format!("data:image/png;base64,{encoded}"),
                hotspot_x: hotspot.x,
                hotspot_y: hotspot.y,
                width: size.width,
                height: size.height,
            })
        }
    }

    /// The bytes of an `NSData`, or `None` for nil or empty data.
    ///
    /// # Safety
    /// `data` must be nil or a live `NSData`, and the slice must not outlive it.
    unsafe fn data_bytes<'a>(data: id) -> Option<&'a [u8]> {
        if data == nil {
            return None;
        }
        let len: usize = msg_send![data, length];
        let ptr: *const u8 = msg_send![data, bytes];
        if ptr.is_null() || len == 0 {
            return None;
        }
        Some(std::slice::from_raw_parts(ptr, len))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Field names are the contract with the overlay page.
    #[test]
    fn a_shape_carries_what_the_page_draws_with() {
        let shape = CursorShape {
            image: "data:image/png;base64,AA==".into(),
            hotspot_x: 4.0,
            hotspot_y: 4.0,
            width: 17.0,
            height: 23.0,
        };
        let v = serde_json::to_value(shape).unwrap();
        for key in ["image", "hotspot_x", "hotspot_y", "width", "height"] {
            assert!(v.get(key).is_some(), "shape is missing {key}");
        }
    }
}
