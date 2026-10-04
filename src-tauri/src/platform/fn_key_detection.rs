//! # Is there a globe key to press?
//!
//! Juno's default holds are on the globe (Fn) key. A keyboard without one
//! cannot press them, so when no connected keyboard has the key the defaults
//! move to keys every keyboard has (see [`crate::triggers::settle_for_keyboard`])
//! and move back when one is plugged in. Nobody is asked about their hardware.
//!
//! ## How a keyboard is known to have the key
//!
//! Keyboards are enumerated with an `IOHIDManager` matching Generic Desktop /
//! Keyboard (usage page 0x01, usage 0x06), and a keyboard counts when any of
//! these is true:
//!
//! - Its HID report descriptor has the Apple vendor Fn element: usage page
//!   `0xFF` (`kHIDPage_AppleVendorTopCase`) usage `0x03`
//!   (`kHIDUsage_AV_TopCase_KeyboardFn`), or usage page `0xFF01`
//!   (`kHIDPage_AppleVendorKeyboard`) usage `0x03`
//!   (`kHIDUsage_AppleVendorKeyboard_Function`). These are the usages the
//!   globe key reports, the same ones Karabiner-Elements names
//!   `apple_vendor_top_case_key_code: keyboard_fn` and
//!   `apple_vendor_keyboard_key_code: function` (values from
//!   `AppleHIDUsageTables.h` in Apple's open-source IOHIDFamily; the header is
//!   not in the public SDK). Checked on hardware: the built-in keyboard of an
//!   Apple silicon MacBook exposes `0xFF/0x03`; a Glove80 does not.
//! - Its vendor is Apple (`0x05AC`). Every Apple keyboard has an Fn key.
//! - It is the built-in keyboard (`Built-In`), unless the lid is shut
//!   (`AppleClamshellState` on `IOPMrootDomain`), when nobody can reach it.
//!
//! Only descriptors and registry properties are read. The manager is never
//! opened and no device is seized, so nothing here needs Input Monitoring or
//! raises a permission prompt.
//!
//! Keyboards arriving and leaving are reported by the manager's matching and
//! removal callbacks on the main run loop. The lid has no callback worth the
//! wiring, so the answer is also re-derived every few seconds; that is a scan
//! of a short list and one registry read.
//!
//! ## When in doubt, there is a globe key
//!
//! Until a scan has finished, or if it cannot run at all, the answer is "yes",
//! which is how Juno behaved before this existed. A press of the globe key is
//! proof too ([`note_fn_pressed`]), whatever the scan said.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

use tauri::AppHandle;
use tracing::info;

const UNKNOWN: u8 = 0;
const PRESENT: u8 = 1;
const ABSENT: u8 = 2;

/// The last answer, as one of the three values above.
static DETECTED: AtomicU8 = AtomicU8::new(UNKNOWN);
/// The globe key was pressed since the keyboards last changed.
static PROVEN: AtomicBool = AtomicBool::new(false);

/// Apple's USB vendor id.
pub const APPLE_VENDOR_ID: i64 = 0x05AC;
/// `kHIDPage_AppleVendorTopCase`.
pub const APPLE_VENDOR_TOP_CASE_PAGE: u32 = 0x00FF;
/// `kHIDUsage_AV_TopCase_KeyboardFn`.
pub const TOP_CASE_KEYBOARD_FN: u32 = 0x0003;
/// `kHIDPage_AppleVendorKeyboard`.
pub const APPLE_VENDOR_KEYBOARD_PAGE: u32 = 0xFF01;
/// `kHIDUsage_AppleVendorKeyboard_Function`.
pub const APPLE_VENDOR_KEYBOARD_FUNCTION: u32 = 0x0003;

/// Whether one HID element is the globe key.
pub fn is_fn_element(usage_page: u32, usage: u32) -> bool {
    (usage_page == APPLE_VENDOR_TOP_CASE_PAGE && usage == TOP_CASE_KEYBOARD_FN)
        || (usage_page == APPLE_VENDOR_KEYBOARD_PAGE && usage == APPLE_VENDOR_KEYBOARD_FUNCTION)
}

/// What one connected keyboard says about itself.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct KeyboardInfo {
    /// Its descriptor has the Apple vendor Fn element.
    pub has_fn_element: bool,
    /// Its vendor is Apple.
    pub apple: bool,
    /// It is the Mac's own keyboard.
    pub built_in: bool,
}

impl KeyboardInfo {
    fn has_globe_key(&self) -> bool {
        self.has_fn_element || self.apple || self.built_in
    }
}

/// Whether any of these keyboards has a globe key someone can press.
///
/// With the lid shut the built-in keyboard is still connected, but nobody can
/// reach it, so it does not count.
pub fn any_globe_key(keyboards: &[KeyboardInfo], lid_closed: bool) -> bool {
    keyboards
        .iter()
        .any(|k| k.has_globe_key() && !(k.built_in && lid_closed))
}

/// Whether a connected keyboard has a globe key. `true` until known.
pub fn fn_key_present() -> bool {
    DETECTED.load(Ordering::SeqCst) != ABSENT
}

/// The globe key was just pressed, so there is one, whatever the scan said.
pub fn note_fn_pressed(app: &AppHandle) {
    if PROVEN.swap(true, Ordering::SeqCst) {
        return;
    }
    if DETECTED.load(Ordering::SeqCst) == ABSENT {
        record(app, true);
    }
}

/// Store a new answer and, if it changed, put the triggers on the keys it
/// calls for.
fn record(app: &AppHandle, present: bool) {
    let next = if present { PRESENT } else { ABSENT };
    let before = DETECTED.swap(next, Ordering::SeqCst);
    if before == next {
        return;
    }
    info!(
        "[FnKeyDetection] Globe key {}",
        if present {
            "available"
        } else {
            "not on any connected keyboard"
        }
    );
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        crate::commands::triggers::apply_keyboard_detection(app).await;
    });
}

/// Start watching the connected keyboards. Call once, after the triggers
/// have been loaded.
pub fn start(app: &AppHandle) {
    imp::start(app);
}

#[cfg(target_os = "macos")]
mod imp {
    use super::{any_globe_key, is_fn_element, record, KeyboardInfo, APPLE_VENDOR_ID, PROVEN};
    use std::ffi::{c_char, c_void, CStr};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Mutex, OnceLock};
    use std::time::Duration;
    use tauri::AppHandle;
    use tracing::{debug, warn};

    type CFTypeRef = *const c_void;
    type CFStringRef = *const c_void;
    type CFAllocatorRef = *const c_void;
    type CFDictionaryRef = *const c_void;
    type CFArrayRef = *const c_void;
    type CFRunLoopRef = *const c_void;
    type IOHIDManagerRef = *mut c_void;
    type IOHIDDeviceRef = *mut c_void;
    type IOHIDElementRef = *mut c_void;
    type IOHIDDeviceCallback = extern "C" fn(
        context: *mut c_void,
        result: i32,
        sender: *mut c_void,
        device: IOHIDDeviceRef,
    );

    const K_CF_STRING_ENCODING_UTF8: u32 = 0x0800_0100;
    const K_CF_NUMBER_SINT32_TYPE: isize = 3;
    const K_CF_NUMBER_SINT64_TYPE: isize = 4;
    const K_IOHID_OPTIONS_TYPE_NONE: u32 = 0;
    /// `kIOMainPortDefault`.
    const K_IO_MAIN_PORT_DEFAULT: u32 = 0;

    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        static kCFAllocatorDefault: CFAllocatorRef;
        static kCFBooleanTrue: CFTypeRef;
        static kCFRunLoopCommonModes: CFStringRef;
        // `CFDictionaryKeyCallBacks` is six pointer-sized fields and
        // `CFDictionaryValueCallBacks` five. Only their addresses are used.
        static kCFTypeDictionaryKeyCallBacks: [usize; 6];
        static kCFTypeDictionaryValueCallBacks: [usize; 5];
        fn CFRunLoopGetMain() -> CFRunLoopRef;
        fn CFStringCreateWithCString(
            alloc: CFAllocatorRef,
            c_str: *const c_char,
            encoding: u32,
        ) -> CFStringRef;
        fn CFNumberCreate(
            alloc: CFAllocatorRef,
            the_type: isize,
            value_ptr: *const c_void,
        ) -> CFTypeRef;
        fn CFNumberGetValue(number: CFTypeRef, the_type: isize, value_ptr: *mut c_void) -> u8;
        fn CFNumberGetTypeID() -> usize;
        fn CFGetTypeID(cf: CFTypeRef) -> usize;
        fn CFDictionaryCreate(
            alloc: CFAllocatorRef,
            keys: *const CFTypeRef,
            values: *const CFTypeRef,
            num_values: isize,
            key_callbacks: *const c_void,
            value_callbacks: *const c_void,
        ) -> CFDictionaryRef;
        fn CFArrayGetCount(array: CFArrayRef) -> isize;
        fn CFArrayGetValueAtIndex(array: CFArrayRef, index: isize) -> CFTypeRef;
        fn CFRelease(cf: CFTypeRef);
    }

    #[link(name = "IOKit", kind = "framework")]
    extern "C" {
        fn IOHIDManagerCreate(alloc: CFAllocatorRef, options: u32) -> IOHIDManagerRef;
        fn IOHIDManagerSetDeviceMatching(manager: IOHIDManagerRef, matching: CFDictionaryRef);
        fn IOHIDManagerRegisterDeviceMatchingCallback(
            manager: IOHIDManagerRef,
            callback: IOHIDDeviceCallback,
            context: *mut c_void,
        );
        fn IOHIDManagerRegisterDeviceRemovalCallback(
            manager: IOHIDManagerRef,
            callback: IOHIDDeviceCallback,
            context: *mut c_void,
        );
        fn IOHIDManagerScheduleWithRunLoop(
            manager: IOHIDManagerRef,
            run_loop: CFRunLoopRef,
            mode: CFStringRef,
        );
        fn IOHIDDeviceGetProperty(device: IOHIDDeviceRef, key: CFStringRef) -> CFTypeRef;
        fn IOHIDDeviceCopyMatchingElements(
            device: IOHIDDeviceRef,
            matching: CFDictionaryRef,
            options: u32,
        ) -> CFArrayRef;
        fn IOHIDElementGetUsagePage(element: IOHIDElementRef) -> u32;
        fn IOHIDElementGetUsage(element: IOHIDElementRef) -> u32;
        fn IOServiceMatching(name: *const c_char) -> CFDictionaryRef;
        fn IOServiceGetMatchingService(main_port: u32, matching: CFDictionaryRef) -> u32;
        fn IORegistryEntryCreateCFProperty(
            entry: u32,
            key: CFStringRef,
            allocator: CFAllocatorRef,
            options: u32,
        ) -> CFTypeRef;
        fn IOObjectRelease(object: u32) -> i32;
    }

    static APP: OnceLock<AppHandle> = OnceLock::new();
    /// The manager is up. Until then nothing is concluded, so a scan that
    /// never started cannot report "no keyboard".
    static WATCHING: AtomicBool = AtomicBool::new(false);
    /// Every keyboard connected right now, by device reference.
    static KEYBOARDS: Mutex<Vec<(usize, KeyboardInfo)>> = Mutex::new(Vec::new());
    /// How often the answer is re-derived, for the lid.
    const RECHECK: Duration = Duration::from_secs(5);
    /// Keyboards are reported one at a time; let a burst land first.
    const SETTLE: Duration = Duration::from_millis(400);

    /// A CFString, or null. Caller releases it.
    fn cf_string(text: &CStr) -> CFStringRef {
        // SAFETY: `text` is a valid NUL-terminated string for the call.
        unsafe {
            CFStringCreateWithCString(
                kCFAllocatorDefault,
                text.as_ptr(),
                K_CF_STRING_ENCODING_UTF8,
            )
        }
    }

    /// SAFETY: `device` must be a live IOHIDDeviceRef.
    unsafe fn read_keyboard(device: IOHIDDeviceRef) -> KeyboardInfo {
        let mut info = KeyboardInfo::default();

        let vendor_key = cf_string(c"VendorID");
        if !vendor_key.is_null() {
            let vendor = IOHIDDeviceGetProperty(device, vendor_key);
            if !vendor.is_null() && CFGetTypeID(vendor) == CFNumberGetTypeID() {
                let mut value: i64 = 0;
                if CFNumberGetValue(
                    vendor,
                    K_CF_NUMBER_SINT64_TYPE,
                    &mut value as *mut i64 as *mut c_void,
                ) != 0
                {
                    info.apple = value == APPLE_VENDOR_ID;
                }
            }
            CFRelease(vendor_key);
        }

        let built_in_key = cf_string(c"Built-In");
        if !built_in_key.is_null() {
            let built_in = IOHIDDeviceGetProperty(device, built_in_key);
            info.built_in = !built_in.is_null() && built_in == kCFBooleanTrue;
            CFRelease(built_in_key);
        }

        let elements =
            IOHIDDeviceCopyMatchingElements(device, std::ptr::null(), K_IOHID_OPTIONS_TYPE_NONE);
        if !elements.is_null() {
            for i in 0..CFArrayGetCount(elements) {
                let element = CFArrayGetValueAtIndex(elements, i) as IOHIDElementRef;
                if element.is_null() {
                    continue;
                }
                if is_fn_element(
                    IOHIDElementGetUsagePage(element),
                    IOHIDElementGetUsage(element),
                ) {
                    info.has_fn_element = true;
                    break;
                }
            }
            CFRelease(elements);
        }
        info
    }

    /// Whether the lid is shut. Unknown reads as open.
    fn lid_closed() -> bool {
        // SAFETY: plain IOKit registry calls. `IOServiceGetMatchingService`
        // consumes the matching dictionary; the service, the key and the
        // property are each released once.
        unsafe {
            let matching = IOServiceMatching(c"IOPMrootDomain".as_ptr());
            if matching.is_null() {
                return false;
            }
            let service = IOServiceGetMatchingService(K_IO_MAIN_PORT_DEFAULT, matching);
            if service == 0 {
                return false;
            }
            let key = cf_string(c"AppleClamshellState");
            let mut closed = false;
            if !key.is_null() {
                let value = IORegistryEntryCreateCFProperty(service, key, kCFAllocatorDefault, 0);
                if !value.is_null() {
                    closed = value == kCFBooleanTrue;
                    CFRelease(value);
                }
                CFRelease(key);
            }
            IOObjectRelease(service);
            closed
        }
    }

    fn evaluate() -> bool {
        if !WATCHING.load(Ordering::SeqCst) || PROVEN.load(Ordering::SeqCst) {
            return true;
        }
        let keyboards: Vec<KeyboardInfo> = match KEYBOARDS.lock() {
            Ok(g) => g.iter().map(|(_, k)| *k).collect(),
            Err(poisoned) => poisoned.into_inner().iter().map(|(_, k)| *k).collect(),
        };
        any_globe_key(&keyboards, lid_closed())
    }

    fn settle_soon() {
        let Some(app) = APP.get().cloned() else {
            return;
        };
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(SETTLE).await;
            record(&app, evaluate());
        });
    }

    extern "C" fn on_matched(
        _context: *mut c_void,
        _result: i32,
        _sender: *mut c_void,
        device: IOHIDDeviceRef,
    ) {
        if device.is_null() {
            return;
        }
        // SAFETY: the manager hands over a live device for the callback.
        let info = unsafe { read_keyboard(device) };
        debug!("[FnKeyDetection] Keyboard connected: {info:?}");
        let mut keyboards = match KEYBOARDS.lock() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        };
        let id = device as usize;
        keyboards.retain(|(known, _)| *known != id);
        keyboards.push((id, info));
        drop(keyboards);
        settle_soon();
    }

    extern "C" fn on_removed(
        _context: *mut c_void,
        _result: i32,
        _sender: *mut c_void,
        device: IOHIDDeviceRef,
    ) {
        let id = device as usize;
        match KEYBOARDS.lock() {
            Ok(mut g) => g.retain(|(known, _)| *known != id),
            Err(poisoned) => poisoned.into_inner().retain(|(known, _)| *known != id),
        }
        // The press that proved a globe key may have come from this one.
        PROVEN.store(false, Ordering::SeqCst);
        settle_soon();
    }

    /// Create the manager on the main thread and schedule it on the main run
    /// loop. It lives for the rest of the run.
    fn install() {
        // SAFETY: called on the main thread. Every CF object created here is
        // released once, except the manager, which is kept for the life of
        // the process on purpose.
        unsafe {
            let manager = IOHIDManagerCreate(kCFAllocatorDefault, K_IOHID_OPTIONS_TYPE_NONE);
            if manager.is_null() {
                warn!("[FnKeyDetection] Could not create the HID manager; assuming a globe key");
                return;
            }
            let page_key = cf_string(c"DeviceUsagePage");
            let usage_key = cf_string(c"DeviceUsage");
            let page: i32 = 0x01;
            let usage: i32 = 0x06;
            let page_value = CFNumberCreate(
                kCFAllocatorDefault,
                K_CF_NUMBER_SINT32_TYPE,
                &page as *const i32 as *const c_void,
            );
            let usage_value = CFNumberCreate(
                kCFAllocatorDefault,
                K_CF_NUMBER_SINT32_TYPE,
                &usage as *const i32 as *const c_void,
            );
            let parts = [page_key, usage_key, page_value, usage_value];
            if parts.iter().any(|p| p.is_null()) {
                for p in parts.into_iter().filter(|p| !p.is_null()) {
                    CFRelease(p);
                }
                warn!("[FnKeyDetection] Could not build the keyboard match; assuming a globe key");
                return;
            }
            let keys = [page_key, usage_key];
            let values = [page_value, usage_value];
            let matching = CFDictionaryCreate(
                kCFAllocatorDefault,
                keys.as_ptr(),
                values.as_ptr(),
                2,
                std::ptr::addr_of!(kCFTypeDictionaryKeyCallBacks) as *const c_void,
                std::ptr::addr_of!(kCFTypeDictionaryValueCallBacks) as *const c_void,
            );
            for p in parts {
                CFRelease(p);
            }
            if matching.is_null() {
                warn!("[FnKeyDetection] Could not build the keyboard match; assuming a globe key");
                return;
            }
            IOHIDManagerSetDeviceMatching(manager, matching);
            CFRelease(matching);
            IOHIDManagerRegisterDeviceMatchingCallback(manager, on_matched, std::ptr::null_mut());
            IOHIDManagerRegisterDeviceRemovalCallback(manager, on_removed, std::ptr::null_mut());
            IOHIDManagerScheduleWithRunLoop(manager, CFRunLoopGetMain(), kCFRunLoopCommonModes);
        }
        WATCHING.store(true, Ordering::SeqCst);
        debug!("[FnKeyDetection] Watching keyboards");
    }

    pub fn start(app: &AppHandle) {
        if APP.set(app.clone()).is_err() {
            return; // already started
        }
        if let Err(e) = app.run_on_main_thread(install) {
            warn!("[FnKeyDetection] Could not start on the main thread: {e}; assuming a globe key");
            return;
        }
        // The lid has no callback here, so look again every few seconds.
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            loop {
                tokio::time::sleep(RECHECK).await;
                record(&app, evaluate());
            }
        });
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    use tauri::AppHandle;

    pub fn start(_app: &AppHandle) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    const BUILT_IN: KeyboardInfo = KeyboardInfo {
        has_fn_element: true,
        apple: true,
        built_in: true,
    };
    const THIRD_PARTY: KeyboardInfo = KeyboardInfo {
        has_fn_element: false,
        apple: false,
        built_in: false,
    };

    #[test]
    fn the_globe_key_elements_are_the_apple_vendor_fn_usages() {
        assert!(is_fn_element(0xFF, 0x03));
        assert!(is_fn_element(0xFF01, 0x03));
        assert!(
            !is_fn_element(0x07, 0x03),
            "keyboard page usage 3 is not Fn"
        );
        assert!(!is_fn_element(0xFF, 0x04));
    }

    #[test]
    fn a_laptop_keyboard_has_the_globe_key() {
        assert!(any_globe_key(&[BUILT_IN], false));
    }

    #[test]
    fn a_third_party_keyboard_alone_does_not() {
        assert!(!any_globe_key(&[THIRD_PARTY], false));
        assert!(!any_globe_key(&[], false), "no keyboard at all");
    }

    #[test]
    fn a_third_party_keyboard_that_declares_the_element_does() {
        let declares = KeyboardInfo {
            has_fn_element: true,
            ..THIRD_PARTY
        };
        assert!(any_globe_key(&[declares], false));
    }

    #[test]
    fn any_apple_keyboard_does() {
        let magic = KeyboardInfo {
            apple: true,
            ..THIRD_PARTY
        };
        assert!(any_globe_key(&[THIRD_PARTY, magic], false));
    }

    #[test]
    fn a_shut_lid_takes_the_built_in_keyboard_out_of_reach() {
        assert!(!any_globe_key(&[BUILT_IN, THIRD_PARTY], true));
        assert!(any_globe_key(&[BUILT_IN, THIRD_PARTY], false));
        let magic = KeyboardInfo {
            apple: true,
            ..THIRD_PARTY
        };
        assert!(any_globe_key(&[BUILT_IN, magic], true));
    }

    #[test]
    fn until_a_scan_finishes_there_is_a_globe_key() {
        // Nothing in this test binary starts a scan, so the answer is the
        // starting one, which is the behaviour from before detection existed.
        assert!(fn_key_present());
    }
}
