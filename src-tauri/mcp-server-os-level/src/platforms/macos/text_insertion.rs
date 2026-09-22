//! Clipboard-free text insertion and the hardened pasteboard machinery.
//!
//! The clipboard-free path posts the text as unicode keyboard CGEvents to the
//! process that owns keyboard focus, so the pasteboard is never touched. The
//! paste path (still the dictation default) lives in `interaction::paste_text`
//! but leans on this module for its pasteboard snapshot/restore and for the
//! layout-aware Cmd+V key code.
//!
//! Fallback ladder for `insert_text_clipboard_free`, in order:
//! 1. unicode CGEvent chunks to the focused PID (frontmost app, then HID tap,
//!    when no PID is known),
//! 2. AX insert at the selected range of the focused element,
//! 3. the clipboard paste path,
//! 4. per-character unicode events with a small settle delay.
//!
//! Apps on [`FORCED_PASTE_BUNDLE_IDS`] skip straight to paste: Ghostty, for
//! one, does not reliably accept unicode keystroke events.

use super::ffi;
use super::interaction::{frontmost_app_pid, paste_text, post_cg_event_to_pid};
use super::memory_safety::get_pooled_event_source;
use crate::AutomationError;
use accessibility_sys::{
    AXUIElementCopyAttributeValue, AXUIElementCreateSystemWide, AXUIElementGetPid, AXUIElementRef,
    AXUIElementSetAttributeValue,
};
use core_foundation::base::{CFTypeRef, TCFType};
use core_foundation::string::{CFString, CFStringRef};
use core_foundation_sys::base::CFRelease;
use core_graphics::event::{CGEvent, CGEventTapLocation, EventField};
use objc::runtime::{Object, BOOL, NO};
use objc::{class, msg_send, sel, sel_impl};
use std::collections::HashMap;
use std::ffi::CString;
use std::os::raw::c_void;
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::Duration;
use tracing::{debug, warn};

type Id = *mut Object;

/// Maximum UTF-16 code units carried by a single unicode keyboard event.
const MAX_UTF16_UNITS_PER_EVENT: usize = 200;

/// Settle delay between per-character events on the last-resort typing path.
const PER_CHARACTER_DELAY_MS: u64 = 2;

/// Bundle identifiers that must always take the clipboard paste path because
/// they do not accept unicode keystroke events reliably.
pub(crate) const FORCED_PASTE_BUNDLE_IDS: &[&str] = &["com.mitchellh.ghostty"];

/// Tag a synthesized event so Juno's own key monitors can recognise and
/// ignore it. The marker rides in the event-source user-data field, which
/// survives delivery to NSEvent monitors and event taps.
pub(crate) fn tag_synthesized_event(event: &CGEvent) {
    event.set_integer_value_field(
        EventField::EVENT_SOURCE_USER_DATA,
        crate::SYNTHESIZED_EVENT_MARKER,
    );
}

// ── UTF-16 chunking ──────────────────────────────────────────────────────────

/// Split `text` into UTF-16 runs of at most `max_units` code units, never
/// splitting a surrogate pair: when the last unit of a chunk would be a high
/// surrogate, the boundary backs off one unit so the pair travels together.
fn chunk_utf16(text: &str, max_units: usize) -> Vec<Vec<u16>> {
    let units: Vec<u16> = text.encode_utf16().collect();
    let mut chunks = Vec::new();
    let mut start = 0;
    while start < units.len() {
        let mut end = (start + max_units).min(units.len());
        if end < units.len() && (0xD800..0xDC00).contains(&units[end - 1]) {
            // A high surrogate's partner is the next unit. Back the boundary
            // off one unit so the pair travels together; a degenerate limit
            // that cannot hold a pair carries it oversize instead.
            if end - start > 1 {
                end -= 1;
            } else {
                end += 1;
            }
        }
        chunks.push(units[start..end].to_vec());
        start = end;
    }
    chunks
}

// ── Focus resolution ─────────────────────────────────────────────────────────

/// PID of the application that owns keyboard focus, per the accessibility
/// system-wide element. This can differ from the frontmost app while a focus
/// redirection is active, and it is what dictation should type into.
fn ax_focused_app_pid() -> Option<i32> {
    unsafe {
        let systemwide = AXUIElementCreateSystemWide();
        if systemwide.is_null() {
            return None;
        }
        let attr = CFString::new("AXFocusedApplication");
        let mut value: CFTypeRef = std::ptr::null();
        let err = AXUIElementCopyAttributeValue(
            systemwide,
            attr.as_concrete_TypeRef(),
            &mut value as *mut CFTypeRef,
        );
        CFRelease(systemwide as CFTypeRef);
        if err != 0 || value.is_null() {
            return None;
        }
        let mut pid: libc::pid_t = 0;
        let pid_err = AXUIElementGetPid(value as AXUIElementRef, &mut pid);
        CFRelease(value);
        if pid_err == 0 && pid > 0 {
            Some(pid)
        } else {
            None
        }
    }
}

/// Bundle identifier for a PID via NSRunningApplication, when the process is
/// an ordinary app. Command-line tools have none.
fn bundle_id_for_pid(pid: i32) -> Option<String> {
    objc::rc::autoreleasepool(|| unsafe {
        let app: Id = msg_send![
            class!(NSRunningApplication),
            runningApplicationWithProcessIdentifier: pid
        ];
        if app.is_null() {
            return None;
        }
        let bundle_id: Id = msg_send![app, bundleIdentifier];
        nsstring_to_string(bundle_id)
    })
}

fn requires_forced_paste(pid: i32) -> bool {
    bundle_id_for_pid(pid)
        .map(|bundle| FORCED_PASTE_BUNDLE_IDS.contains(&bundle.as_str()))
        .unwrap_or(false)
}

// ── Unicode event posting ────────────────────────────────────────────────────

/// Post `text` as unicode keyboard events, one keyDown/keyUp pair per chunk,
/// with no delay between chunks. `target_pid` routes the events to a process
/// directly; `None` posts them on the HID tap for the frontmost app.
fn post_unicode_chunks(text: &str, target_pid: Option<i32>) -> Result<(), AutomationError> {
    post_unicode_units(text, target_pid, MAX_UTF16_UNITS_PER_EVENT, None)
}

/// Last-resort path: one code point per event pair with a settle delay, for
/// targets that drop multi-unit unicode payloads.
fn type_text_per_character(text: &str, target_pid: Option<i32>) -> Result<(), AutomationError> {
    post_unicode_units(
        text,
        target_pid,
        1,
        Some(Duration::from_millis(PER_CHARACTER_DELAY_MS)),
    )
}

fn post_unicode_units(
    text: &str,
    target_pid: Option<i32>,
    max_units: usize,
    inter_chunk_delay: Option<Duration>,
) -> Result<(), AutomationError> {
    let source = get_pooled_event_source().map_err(|e| {
        AutomationError::PlatformError(format!(
            "Failed to create event source for unicode insertion: {}",
            e
        ))
    })?;

    for chunk in chunk_utf16(text, max_units) {
        let key_down = CGEvent::new_keyboard_event(source.clone(), 0, true).map_err(|_| {
            AutomationError::PlatformError(
                "Failed to create key down event for unicode insertion".to_string(),
            )
        })?;
        key_down.set_string_from_utf16_unchecked(&chunk);
        tag_synthesized_event(&key_down);

        let key_up = CGEvent::new_keyboard_event(source.clone(), 0, false).map_err(|_| {
            AutomationError::PlatformError(
                "Failed to create key up event for unicode insertion".to_string(),
            )
        })?;
        key_up.set_string_from_utf16_unchecked(&chunk);
        tag_synthesized_event(&key_up);

        match target_pid {
            Some(pid) => {
                post_cg_event_to_pid(pid, &key_down);
                post_cg_event_to_pid(pid, &key_up);
            }
            None => {
                key_down.post(CGEventTapLocation::HID);
                key_up.post(CGEventTapLocation::HID);
            }
        }

        if let Some(delay) = inter_chunk_delay {
            thread::sleep(delay);
        }
    }
    Ok(())
}

// ── AX insert at the selected range ──────────────────────────────────────────

/// Replace the focused element's selected text with `text` (an insert when the
/// selection is empty). Works only where the focused element implements the
/// AX text protocol.
fn ax_insert_at_selection(text: &str) -> Result<(), AutomationError> {
    unsafe {
        let systemwide = AXUIElementCreateSystemWide();
        if systemwide.is_null() {
            return Err(AutomationError::PlatformError(
                "Failed to create system-wide AX element".to_string(),
            ));
        }
        let focused_attr = CFString::new("AXFocusedUIElement");
        let mut focused: CFTypeRef = std::ptr::null();
        let err = AXUIElementCopyAttributeValue(
            systemwide,
            focused_attr.as_concrete_TypeRef(),
            &mut focused as *mut CFTypeRef,
        );
        CFRelease(systemwide as CFTypeRef);
        if err != 0 || focused.is_null() {
            return Err(AutomationError::PlatformError(format!(
                "No AX-focused element to insert into (AXError {})",
                err
            )));
        }

        let selected_text_attr = CFString::new("AXSelectedText");
        let value = CFString::new(text);
        let set_err = AXUIElementSetAttributeValue(
            focused as AXUIElementRef,
            selected_text_attr.as_concrete_TypeRef() as CFStringRef,
            value.as_concrete_TypeRef() as CFTypeRef,
        );
        CFRelease(focused);
        if set_err != 0 {
            return Err(AutomationError::PlatformError(format!(
                "AXSelectedText insert rejected (AXError {})",
                set_err
            )));
        }
        Ok(())
    }
}

// ── Public entry point ───────────────────────────────────────────────────────

/// Insert `text` into the focused app without touching the pasteboard.
///
/// Returns a label naming the path that succeeded, for logging. Apps on the
/// forced-paste list go straight to the paste path (which snapshots and
/// restores the pasteboard, so the user's clipboard still survives).
pub fn insert_text_clipboard_free(text: &str) -> Result<&'static str, AutomationError> {
    if text.is_empty() {
        return Ok("noop");
    }

    let target_pid = ax_focused_app_pid().or_else(frontmost_app_pid);

    if let Some(pid) = target_pid {
        if requires_forced_paste(pid) {
            debug!(
                "Target PID {} is on the forced-paste list; using clipboard paste",
                pid
            );
            paste_text(text, None, false)?;
            return Ok("forced-paste");
        }
    }

    match post_unicode_chunks(text, target_pid) {
        Ok(()) => return Ok("unicode-events"),
        Err(e) => debug!("Unicode event insertion failed: {:?}; trying AX insert", e),
    }

    match ax_insert_at_selection(text) {
        Ok(()) => return Ok("ax-insert"),
        Err(e) => debug!("AX insert failed: {:?}; trying clipboard paste", e),
    }

    match paste_text(text, None, false) {
        Ok(()) => return Ok("paste"),
        Err(e) => debug!("Clipboard paste failed: {:?}; typing per character", e),
    }

    type_text_per_character(text, target_pid)?;
    Ok("per-character")
}

// ── Pasteboard snapshot / restore ────────────────────────────────────────────

/// Every representation of every pasteboard item, so a restore puts back rich
/// content (RTF, images, file URLs), not just the plain string.
pub(crate) struct PasteboardSnapshot {
    items: Vec<Vec<(String, Vec<u8>)>>,
}

unsafe fn general_pasteboard() -> Id {
    msg_send![class!(NSPasteboard), generalPasteboard]
}

unsafe fn nsstring_to_string(ns_string: Id) -> Option<String> {
    if ns_string.is_null() {
        return None;
    }
    let utf8: *const libc::c_char = msg_send![ns_string, UTF8String];
    if utf8.is_null() {
        return None;
    }
    Some(
        std::ffi::CStr::from_ptr(utf8)
            .to_string_lossy()
            .into_owned(),
    )
}

unsafe fn nsstring(text: &str) -> Option<Id> {
    let c_string = CString::new(text).ok()?;
    let ns: Id = msg_send![class!(NSString), stringWithUTF8String: c_string.as_ptr()];
    if ns.is_null() {
        None
    } else {
        Some(ns)
    }
}

/// The pasteboard server's monotonically increasing change counter.
pub(crate) fn pasteboard_change_count() -> i64 {
    objc::rc::autoreleasepool(|| unsafe {
        let pasteboard = general_pasteboard();
        if pasteboard.is_null() {
            return -1;
        }
        msg_send![pasteboard, changeCount]
    })
}

/// Capture every type of every item currently on the general pasteboard.
/// Best effort: `None` only when the pasteboard itself is unreachable.
pub(crate) fn snapshot_pasteboard() -> Option<PasteboardSnapshot> {
    objc::rc::autoreleasepool(|| unsafe {
        let pasteboard = general_pasteboard();
        if pasteboard.is_null() {
            return None;
        }
        let items: Id = msg_send![pasteboard, pasteboardItems];
        if items.is_null() {
            return Some(PasteboardSnapshot { items: Vec::new() });
        }
        let count: usize = msg_send![items, count];
        let mut snapshot_items = Vec::with_capacity(count);
        for item_index in 0..count {
            let item: Id = msg_send![items, objectAtIndex: item_index];
            if item.is_null() {
                continue;
            }
            let types: Id = msg_send![item, types];
            if types.is_null() {
                continue;
            }
            let type_count: usize = msg_send![types, count];
            let mut representations = Vec::with_capacity(type_count);
            for type_index in 0..type_count {
                let type_id: Id = msg_send![types, objectAtIndex: type_index];
                let Some(type_name) = nsstring_to_string(type_id) else {
                    continue;
                };
                let data: Id = msg_send![item, dataForType: type_id];
                if data.is_null() {
                    continue;
                }
                let length: usize = msg_send![data, length];
                let bytes: *const u8 = msg_send![data, bytes];
                let mut buffer = vec![0u8; length];
                if length > 0 && !bytes.is_null() {
                    std::ptr::copy_nonoverlapping(bytes, buffer.as_mut_ptr(), length);
                }
                representations.push((type_name, buffer));
            }
            if !representations.is_empty() {
                snapshot_items.push(representations);
            }
        }
        Some(PasteboardSnapshot {
            items: snapshot_items,
        })
    })
}

/// Put a snapshot back on the general pasteboard. Returns false when any part
/// of the write is refused.
fn restore_pasteboard(snapshot: &PasteboardSnapshot) -> bool {
    objc::rc::autoreleasepool(|| unsafe {
        let pasteboard = general_pasteboard();
        if pasteboard.is_null() {
            return false;
        }
        let _: i64 = msg_send![pasteboard, clearContents];
        if snapshot.items.is_empty() {
            return true;
        }
        let array: Id = msg_send![class!(NSMutableArray), arrayWithCapacity: snapshot.items.len()];
        if array.is_null() {
            return false;
        }
        for representations in &snapshot.items {
            let item: Id = msg_send![class!(NSPasteboardItem), new];
            if item.is_null() {
                return false;
            }
            for (type_name, bytes) in representations {
                let Some(type_ns) = nsstring(type_name) else {
                    continue;
                };
                let data: Id = msg_send![
                    class!(NSData),
                    dataWithBytes: bytes.as_ptr() as *const c_void
                    length: bytes.len()
                ];
                if data.is_null() {
                    continue;
                }
                let _: BOOL = msg_send![item, setData: data forType: type_ns];
            }
            let _: () = msg_send![array, addObject: item];
            // The array retains the item; `new` handed us the +1 reference.
            let _: () = msg_send![item, release];
        }
        let ok: BOOL = msg_send![pasteboard, writeObjects: array];
        ok != NO
    })
}

/// Write `text` to the general pasteboard as a single item. With `transient`
/// the item is marked `org.nspasteboard.TransientType` and
/// `org.nspasteboard.AutoGeneratedType` so clipboard managers skip it — the
/// paste-and-restore case. Without, it is an ordinary item the user means to
/// keep. Returns the pasteboard change count after the write, which the
/// delayed restore uses as its ownership check.
pub(crate) fn write_string_to_pasteboard(
    text: &str,
    transient: bool,
) -> Result<i64, AutomationError> {
    objc::rc::autoreleasepool(|| unsafe {
        let pasteboard = general_pasteboard();
        if pasteboard.is_null() {
            return Err(AutomationError::PlatformError(
                "General pasteboard unavailable".to_string(),
            ));
        }
        let item: Id = msg_send![class!(NSPasteboardItem), new];
        if item.is_null() {
            return Err(AutomationError::PlatformError(
                "Failed to create pasteboard item".to_string(),
            ));
        }
        let release_item = || {
            let _: () = msg_send![item, release];
        };

        let Some(text_ns) = nsstring(text) else {
            release_item();
            return Err(AutomationError::PlatformError(
                "Transcript contains an interior NUL; cannot write to pasteboard".to_string(),
            ));
        };
        let Some(string_type) = nsstring("public.utf8-plain-text") else {
            release_item();
            return Err(AutomationError::PlatformError(
                "Failed to create pasteboard type string".to_string(),
            ));
        };
        let set_ok: BOOL = msg_send![item, setString: text_ns forType: string_type];
        if set_ok == NO {
            release_item();
            return Err(AutomationError::PlatformError(
                "Pasteboard refused the transcript string".to_string(),
            ));
        }

        if transient {
            let empty_data: Id = msg_send![class!(NSData), data];
            for marker in [
                "org.nspasteboard.TransientType",
                "org.nspasteboard.AutoGeneratedType",
            ] {
                if let Some(marker_type) = nsstring(marker) {
                    let _: BOOL = msg_send![item, setData: empty_data forType: marker_type];
                }
            }
        }

        let _: i64 = msg_send![pasteboard, clearContents];
        let array: Id = msg_send![class!(NSArray), arrayWithObject: item];
        let ok: BOOL = msg_send![pasteboard, writeObjects: array];
        release_item();
        if ok == NO {
            return Err(AutomationError::PlatformError(
                "Pasteboard rejected the transcript item".to_string(),
            ));
        }
        let change_count: i64 = msg_send![pasteboard, changeCount];
        Ok(change_count)
    })
}

/// Restore `snapshot` after `delay`, but only if the pasteboard's change count
/// still equals `change_count_after_write` — i.e. nothing else (the user
/// copying, another app) has claimed the pasteboard since the paste wrote it.
pub(crate) fn schedule_snapshot_restore(
    snapshot: Option<PasteboardSnapshot>,
    change_count_after_write: i64,
    delay: Duration,
) {
    let Some(snapshot) = snapshot else {
        return;
    };
    thread::spawn(move || {
        thread::sleep(delay);
        let current = pasteboard_change_count();
        if current != change_count_after_write {
            debug!(
                "Pasteboard changed since paste (changeCount {} -> {}); leaving it alone",
                change_count_after_write, current
            );
            return;
        }
        if !restore_pasteboard(&snapshot) {
            warn!("Failed to restore pasteboard snapshot after paste");
        }
    });
}

// ── Layout-aware Cmd+V ───────────────────────────────────────────────────────

/// `kUCKeyActionDisplay`: which character the key would produce, ignoring
/// dead-key sequencing.
const UC_KEY_ACTION_DISPLAY: u16 = 3;
/// `kUCKeyTranslateNoDeadKeysMask`.
const UC_KEY_TRANSLATE_NO_DEAD_KEYS_MASK: u32 = 1;
/// Carbon's `cmdKey` event modifier, shifted into UCKeyTranslate's
/// `modifierKeyState` format (`(EventModifiers >> 8) & 0xFF`).
const UC_CMD_MODIFIER_STATE: u32 = 1;

/// The virtual key code that produces "v" under the Command modifier in the
/// current keyboard layout, cached per input source. Dvorak-QWERTY-Command
/// maps the Command layer differently from the unmodified layout, so this is
/// resolved with UCKeyTranslate under Command rather than assumed to be the
/// ANSI V position.
pub(crate) fn cmd_v_keycode() -> u16 {
    static CACHE: OnceLock<Mutex<HashMap<String, u16>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));

    let source_id = current_input_source_id().unwrap_or_default();
    if let Ok(cached) = cache.lock() {
        if let Some(&code) = cached.get(&source_id) {
            return code;
        }
    }

    let code = resolve_cmd_v_keycode().unwrap_or(super::constants::KEY_V);
    if let Ok(mut cached) = cache.lock() {
        cached.insert(source_id, code);
    }
    code
}

fn current_input_source_id() -> Option<String> {
    unsafe {
        let source = ffi::TISCopyCurrentKeyboardLayoutInputSource();
        if source.is_null() {
            return None;
        }
        let id_ref = ffi::TISGetInputSourceProperty(source, ffi::kTISPropertyInputSourceID);
        let result = if id_ref.is_null() {
            None
        } else {
            let cf_string =
                CFString::wrap_under_get_rule(id_ref as core_foundation::string::CFStringRef);
            Some(cf_string.to_string())
        };
        CFRelease(source as CFTypeRef);
        result
    }
}

fn resolve_cmd_v_keycode() -> Option<u16> {
    unsafe {
        let source = ffi::TISCopyCurrentKeyboardLayoutInputSource();
        if source.is_null() {
            return None;
        }
        let layout_data =
            ffi::TISGetInputSourceProperty(source, ffi::kTISPropertyUnicodeKeyLayoutData);
        if layout_data.is_null() {
            CFRelease(source as CFTypeRef);
            return None;
        }
        let layout_ptr =
            core_foundation_sys::data::CFDataGetBytePtr(layout_data as *const _) as *const c_void;
        if layout_ptr.is_null() {
            CFRelease(source as CFTypeRef);
            return None;
        }

        let keyboard_type = ffi::LMGetKbdType() as u32;
        let mut found = None;
        for keycode in 0u16..=127 {
            let mut dead_key_state: u32 = 0;
            let mut chars = [0u16; 4];
            let mut actual_length: libc::c_ulong = 0;
            let status = ffi::UCKeyTranslate(
                layout_ptr,
                keycode,
                UC_KEY_ACTION_DISPLAY,
                UC_CMD_MODIFIER_STATE,
                keyboard_type,
                UC_KEY_TRANSLATE_NO_DEAD_KEYS_MASK,
                &mut dead_key_state,
                chars.len() as libc::c_ulong,
                &mut actual_length,
                chars.as_mut_ptr(),
            );
            if status == 0 && actual_length == 1 && (chars[0] == u16::from(b'v')) {
                found = Some(keycode);
                break;
            }
        }
        CFRelease(source as CFTypeRef);
        found
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn units(text: &str) -> usize {
        text.encode_utf16().count()
    }

    #[test]
    fn chunking_empty_string_yields_no_chunks() {
        assert!(chunk_utf16("", 200).is_empty());
    }

    #[test]
    fn chunking_short_string_is_one_chunk() {
        let chunks = chunk_utf16("hello world", 200);
        assert_eq!(chunks.len(), 1);
        assert_eq!(String::from_utf16(&chunks[0]).unwrap(), "hello world");
    }

    #[test]
    fn chunking_splits_at_exact_boundary() {
        let text = "a".repeat(400);
        let chunks = chunk_utf16(&text, 200);
        assert_eq!(chunks.len(), 2);
        assert!(chunks.iter().all(|c| c.len() == 200));
    }

    #[test]
    fn chunking_never_splits_a_surrogate_pair() {
        // 199 ASCII units, then an emoji (2 UTF-16 units): a naive split at
        // 200 would cut the pair in half.
        let text = format!("{}😀rest", "a".repeat(199));
        let chunks = chunk_utf16(&text, 200);
        assert_eq!(chunks[0].len(), 199, "boundary must back off one unit");
        for chunk in &chunks {
            assert!(
                String::from_utf16(chunk).is_ok(),
                "every chunk must be valid UTF-16"
            );
        }
        let rejoined: Vec<u16> = chunks.into_iter().flatten().collect();
        assert_eq!(String::from_utf16(&rejoined).unwrap(), text);
    }

    #[test]
    fn chunking_handles_all_surrogate_text() {
        let text = "😀".repeat(300); // 600 UTF-16 units, all pairs
        let chunks = chunk_utf16(&text, 200);
        let total: usize = chunks.iter().map(|c| c.len()).sum();
        assert_eq!(total, units(&text));
        for chunk in &chunks {
            assert!(String::from_utf16(chunk).is_ok());
            assert!(chunk.len() <= 200);
        }
    }

    #[test]
    fn chunking_max_units_one_still_terminates_on_pairs() {
        // Degenerate limit: a pair cannot fit in one unit, so it travels as
        // an oversize chunk rather than looping forever or splitting.
        let chunks = chunk_utf16("😀", 1);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].len(), 2);
    }

    #[test]
    fn ghostty_is_on_the_forced_paste_list() {
        assert!(FORCED_PASTE_BUNDLE_IDS.contains(&"com.mitchellh.ghostty"));
    }
}
