//! Screen capture must not call a macOS 14 API on a macOS 12.3 host.
//!
//! `bundle.macOS.minimumSystemVersion` is 12.3 and `SCScreenshotManager` is
//! macOS 14.0+, so these two facts have to be reconciled somewhere. They are
//! reconciled in `platforms::macos::capture_backend`, and this file is what
//! holds that in place.
//!
//! **Why these live in the `juno` package and not next to the code.** CI runs
//! `cargo test --manifest-path src-tauri/Cargo.toml`, which selects the `juno`
//! package alone. `computer-use-ai-sdk` is a workspace member reached only as
//! a dependency, so its own `#[cfg(test)]` modules are never compiled and its
//! tests run nowhere — the same gap the "Test the voice plugin" step in
//! `ci.yml` exists to close, one crate over. The unit tests next to the code
//! are the real ones; these are the subset CI actually enforces today, through
//! the crate's public surface. Closing the gap for the whole crate (~60 tests
//! that have never run anywhere) is LAC-4829, already filed and owned; when it
//! lands, these become a duplicate of the unit tests and can go.

#![cfg(target_os = "macos")]

use computer_use_ai_sdk::platforms::macos::capture_backend::{
    capture_backend, capture_backend_for, host_macos_major, CaptureBackend,
};
use computer_use_ai_sdk::platforms::macos::utils::capture_window_via_coregraphics;

/// A window id that no window server will ever hand out.
const NO_SUCH_WINDOW: u32 = u32::MAX;

#[test]
fn a_host_below_macos_14_does_not_get_screencapturekit() {
    // 12.3 is Juno's declared floor, 13 the other version in range. On either,
    // `SCScreenshotManager`'s class reference is NULL and the call hangs the
    // calling thread forever rather than returning an error.
    assert_eq!(capture_backend_for(Some(12)), CaptureBackend::CoreGraphics);
    assert_eq!(capture_backend_for(Some(13)), CaptureBackend::CoreGraphics);
}

#[test]
fn an_unknown_host_version_does_not_get_screencapturekit() {
    assert_eq!(capture_backend_for(None), CaptureBackend::CoreGraphics);
}

#[test]
fn macos_14_and_newer_still_get_the_fast_path() {
    assert_eq!(
        capture_backend_for(Some(14)),
        CaptureBackend::ScreenCaptureKit
    );
    assert_eq!(
        capture_backend_for(Some(26)),
        CaptureBackend::ScreenCaptureKit
    );
}

/// If the host version read breaks, `capture_backend_for(None)` sends every
/// Mac down the slow path. That is not an error anywhere and nothing else
/// would notice.
#[test]
fn this_host_reads_its_own_macos_version() {
    let major = host_macos_major().expect("the host must report a macOS major version");
    assert!(
        major >= 12,
        "macOS major read as {major}, below Juno's own floor of 12.3"
    );
    assert_eq!(capture_backend(), capture_backend_for(Some(major)));
}

/// The point of the whole change: there is a window-capture path that does not
/// need ScreenCaptureKit. Before it existed, window capture on an older host
/// had nothing to fall back to.
#[test]
fn window_capture_has_a_path_that_does_not_need_screencapturekit() {
    let error = capture_window_via_coregraphics(NO_SUCH_WINDOW)
        .expect_err("u32::MAX is never a live window id");
    let message = error.to_string();
    assert!(
        !message.contains("ScreenCaptureKit"),
        "the CoreGraphics path must not refer the caller back to ScreenCaptureKit: {message}"
    );
    assert!(
        message.contains(&NO_SUCH_WINDOW.to_string()),
        "the failure should name the window it could not capture: {message}"
    );
}
