//! Which screen-capture API is safe to call on the host this build is running on.
//!
//! `SCScreenshotManager` arrived in macOS 14.0. Juno's `minimumSystemVersion`
//! is 12.3, so the app runs on hosts where that class does not exist, and
//! nothing below this module can discover that at runtime:
//!
//! - The `screencapturekit` crate reaches `SCScreenshotManager` through a Swift
//!   bridge whose entry point is annotated `@available(macOS 14.0, *)` with no
//!   `if #available` inside it. Swift's availability model expects the *caller*
//!   to have checked. Rust has no availability model and cannot check.
//! - `_OBJC_CLASS_$_SCScreenshotManager` is bound `[weak-import]` in the
//!   shipped binary (`dyld_info -fixups` on v0.8.287), so below macOS 14 the
//!   class reference is NULL. An Objective-C message to a NULL class returns
//!   immediately without running anything, so the bridge's completion handler
//!   is never invoked, the Swift continuation never resumes, and the crate's
//!   `SyncCompletion::wait()` — an unbounded `Condvar` wait with no timeout —
//!   blocks the calling thread forever.
//!
//! So "call ScreenCaptureKit and fall back on error" cannot work: on an older
//! host there is no error, only a hang. The version has to be checked *before*
//! the call, which is what this module is for.

use std::ffi::CString;
use std::sync::OnceLock;
use tracing::{debug, warn};

/// The screen-capture API to use for a capture on this host.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureBackend {
    /// `SCScreenshotManager`, macOS 14.0+. Faster, and renders a window on its
    /// own rather than cropping the display.
    ScreenCaptureKit,
    /// `CGWindowListCreateImage` / `CGDisplay::screenshot`. Deprecated in macOS
    /// 15.0 but present on every version Juno supports, so it is the floor.
    CoreGraphics,
}

/// First macOS major version that ships `SCScreenshotManager`.
const SCREENSHOT_MANAGER_MIN_MAJOR: u32 = 14;

/// Darwin kernel major for macOS 14. Only ever used to answer "at least 14?",
/// never to name a release: the +9 offset that turns 23 into 14 stops holding
/// at macOS 26 (Darwin 25). Darwin majors increase monotonically, so the
/// threshold comparison stays correct where the naming does not.
const DARWIN_MAJOR_MACOS_14: u32 = 23;

/// Which backend to use, given the host's macOS major version.
///
/// `None` means the version could not be read, and is treated as too old:
/// CoreGraphics works on every supported host, while guessing the other way
/// hangs the capture thread for good.
pub fn capture_backend_for(macos_major: Option<u32>) -> CaptureBackend {
    match macos_major {
        Some(major) if major >= SCREENSHOT_MANAGER_MIN_MAJOR => CaptureBackend::ScreenCaptureKit,
        _ => CaptureBackend::CoreGraphics,
    }
}

/// Which backend to use on this host. Cached: the answer cannot change while
/// the process is alive.
pub fn capture_backend() -> CaptureBackend {
    capture_backend_for(host_macos_major())
}

/// The host's macOS major version, read once and cached.
///
/// On every host this binary can actually run on, this is the real major
/// version. The one exception is a host whose `kern.osproductversion` is
/// shimmed to "10.16", where the answer is derived from the Darwin major and
/// is only accurate either side of macOS 14 — see `macos_major_from`. Use it
/// to decide about ScreenCaptureKit, not to display a version.
pub fn host_macos_major() -> Option<u32> {
    static MAJOR: OnceLock<Option<u32>> = OnceLock::new();
    *MAJOR.get_or_init(|| {
        let major = macos_major_from(
            sysctl_string("kern.osproductversion").as_deref(),
            sysctl_string("kern.osrelease").as_deref(),
        );
        match major {
            Some(major) => debug!("Host reports macOS major version {major}"),
            None => warn!(
                "Could not read the host's macOS version; \
                 screen capture will use CoreGraphics rather than ScreenCaptureKit"
            ),
        }
        major
    })
}

/// The macOS major version implied by `kern.osproductversion`, with
/// `kern.osrelease` as the tie-breaker.
///
/// `kern.osproductversion` is the direct answer ("14.6.1" -> 14), with one
/// trap: a process built against a pre-11 SDK is told "10.16" by a
/// compatibility shim no matter how new the host is. Juno is built against a
/// current SDK so that should not happen, but a `10.x` reading is treated as
/// "unknown" rather than "ancient" and deferred to the Darwin major, which the
/// shim does not rewrite.
fn macos_major_from(product_version: Option<&str>, darwin_release: Option<&str>) -> Option<u32> {
    if let Some(major) = product_version.and_then(parse_leading_u32) {
        // 11 or newer is the host answering for itself. 10.x is either a
        // genuinely ancient Mac or the shim, and both are handled correctly by
        // deferring to the Darwin major below.
        if major >= 11 {
            return Some(major);
        }
    }

    // Only reached when the product version is missing or shimmed. The number
    // returned here is the gate boundary, **not** a release: it says "at least
    // 14" or "below 14" and nothing finer, because the +9 offset that turns
    // Darwin 23 into macOS 14 stops holding at macOS 26 (Darwin 25).
    let darwin_major = parse_leading_u32(darwin_release?)?;
    if darwin_major >= DARWIN_MAJOR_MACOS_14 {
        Some(SCREENSHOT_MANAGER_MIN_MAJOR)
    } else {
        Some(SCREENSHOT_MANAGER_MIN_MAJOR - 1)
    }
}

/// Read a `sysctl` whose value is a NUL-terminated string.
///
/// `sysctl` rather than `NSProcessInfo`: this crate has no Objective-C
/// Foundation binding, and a `sysctl` read is a plain syscall with no
/// main-thread requirement, so it is safe from any worker.
fn sysctl_string(name: &str) -> Option<String> {
    let c_name = CString::new(name).ok()?;

    // A null `oldp` asks only how many bytes the value needs.
    let mut size: libc::size_t = 0;
    let sized = unsafe {
        libc::sysctlbyname(
            c_name.as_ptr(),
            std::ptr::null_mut(),
            &mut size,
            std::ptr::null_mut(),
            0,
        )
    };
    if sized != 0 || size == 0 {
        return None;
    }

    let mut buf = vec![0u8; size];
    let read = unsafe {
        libc::sysctlbyname(
            c_name.as_ptr(),
            buf.as_mut_ptr().cast(),
            &mut size,
            std::ptr::null_mut(),
            0,
        )
    };
    if read != 0 {
        return None;
    }

    // `size` is now the byte count written, and the value is a C string, so
    // stop at the terminator rather than carrying a trailing NUL into the
    // parse.
    buf.truncate(size);
    let end = buf.iter().position(|&byte| byte == 0).unwrap_or(buf.len());
    buf.truncate(end);
    String::from_utf8(buf).ok()
}

/// The leading run of ASCII digits in `value`, as a number.
fn parse_leading_u32(value: &str) -> Option<u32> {
    let digits: String = value
        .trim()
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    if digits.is_empty() {
        None
    } else {
        digits.parse().ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn macos_14_and_newer_get_screencapturekit() {
        assert_eq!(
            capture_backend_for(Some(14)),
            CaptureBackend::ScreenCaptureKit
        );
        assert_eq!(
            capture_backend_for(Some(15)),
            CaptureBackend::ScreenCaptureKit
        );
        assert_eq!(
            capture_backend_for(Some(26)),
            CaptureBackend::ScreenCaptureKit
        );
    }

    #[test]
    fn hosts_below_macos_14_get_coregraphics() {
        // 12.3 is Juno's declared floor; 13 is the other version in range.
        assert_eq!(capture_backend_for(Some(12)), CaptureBackend::CoreGraphics);
        assert_eq!(capture_backend_for(Some(13)), CaptureBackend::CoreGraphics);
    }

    #[test]
    fn an_unreadable_version_gets_coregraphics() {
        // Guessing ScreenCaptureKit here hangs the capture thread forever;
        // guessing CoreGraphics only costs speed.
        assert_eq!(capture_backend_for(None), CaptureBackend::CoreGraphics);
    }

    #[test]
    fn the_product_version_is_the_answer_when_it_is_believable() {
        assert_eq!(macos_major_from(Some("14.0"), Some("23.0.0")), Some(14));
        assert_eq!(macos_major_from(Some("13.7.2"), Some("22.6.0")), Some(13));
        assert_eq!(macos_major_from(Some("12.3"), Some("21.4.0")), Some(12));
        assert_eq!(macos_major_from(Some("26.1"), Some("25.0.0")), Some(26));
    }

    #[test]
    fn a_10_16_shim_reading_defers_to_the_darwin_major() {
        // "10.16" is what the pre-11-SDK compatibility shim reports on any
        // modern host, so it must not be read as "older than 14".
        assert!(macos_major_from(Some("10.16"), Some("23.6.0"))
            .is_some_and(|major| major >= SCREENSHOT_MANAGER_MIN_MAJOR));
        assert!(macos_major_from(Some("10.16"), Some("22.6.0"))
            .is_some_and(|major| major < SCREENSHOT_MANAGER_MIN_MAJOR));
    }

    #[test]
    fn nothing_readable_is_no_answer_rather_than_a_guess() {
        assert_eq!(macos_major_from(None, None), None);
        assert_eq!(macos_major_from(Some(""), Some("")), None);
        assert_eq!(macos_major_from(Some("sonoma"), Some("sonoma")), None);
    }

    #[test]
    fn the_darwin_major_alone_is_enough() {
        assert!(macos_major_from(None, Some("23.0.0"))
            .is_some_and(|major| major >= SCREENSHOT_MANAGER_MIN_MAJOR));
        assert!(macos_major_from(None, Some("21.4.0"))
            .is_some_and(|major| major < SCREENSHOT_MANAGER_MIN_MAJOR));
    }

    /// A wrong sysctl name, or a parse that quietly stops working, would leave
    /// `host_macos_major()` at `None` forever. That is not an error anywhere —
    /// it just permanently picks the slow backend on every Mac. This is the
    /// only test that would notice.
    #[test]
    fn the_host_reports_its_own_macos_major() {
        let major = host_macos_major().expect("the host must report a macOS major version");
        assert!(
            major >= 12,
            "macOS major read as {major}, which is below Juno's own floor of 12.3"
        );
    }

    #[test]
    fn leading_digits_parse_and_everything_else_does_not() {
        assert_eq!(parse_leading_u32("14.6.1"), Some(14));
        assert_eq!(parse_leading_u32(" 23.5.0\n"), Some(23));
        assert_eq!(parse_leading_u32("14"), Some(14));
        assert_eq!(parse_leading_u32(""), None);
        assert_eq!(parse_leading_u32("v14"), None);
    }
}
