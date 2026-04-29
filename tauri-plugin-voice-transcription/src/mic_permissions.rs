use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tracing::{debug, info, warn};

#[cfg(target_os = "macos")]
use objc::{class, msg_send, sel, sel_impl};
#[cfg(target_os = "macos")]
use objc::runtime::{BOOL, YES};
#[cfg(target_os = "macos")]
use cocoa::foundation::{NSString, NSAutoreleasePool};
#[cfg(target_os = "macos")]
use cocoa::base::{nil, id};

/// Microphone permission status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MicrophonePermissionStatus {
    /// Permission has been granted
    Granted,
    /// Permission has been denied
    Denied,
    /// Permission status is undetermined (not yet requested)
    Undetermined,
    /// Permission check is not applicable (non-macOS)
    NotApplicable,
}

/// Global permission status cache
static PERMISSION_CACHED: AtomicBool = AtomicBool::new(false);
static PERMISSION_GRANTED: AtomicBool = AtomicBool::new(false);

/// Guard against concurrent permission requests
static PERMISSION_REQUEST_IN_FLIGHT: AtomicBool = AtomicBool::new(false);

/// Track whether we already attempted a TCC request this session.
/// If the OS doesn't persist the decision (e.g. unsigned dev builds),
/// we avoid re-prompting and direct the user to System Settings instead.
static PERMISSION_REQUESTED_THIS_SESSION: AtomicBool = AtomicBool::new(false);

/// Check microphone permission status using AVCaptureDevice (macOS TCC-compatible)
#[cfg(target_os = "macos")]
pub fn check_microphone_permission() -> MicrophonePermissionStatus {
    // Fast path: return cached result
    if PERMISSION_CACHED.load(Ordering::SeqCst) {
        let granted = PERMISSION_GRANTED.load(Ordering::SeqCst);
        debug!("Returning cached microphone permission: {}", if granted { "granted" } else { "denied" });
        return if granted {
            MicrophonePermissionStatus::Granted
        } else {
            MicrophonePermissionStatus::Denied
        };
    }

    unsafe {
        let _pool = NSAutoreleasePool::new(nil);

        // AVCaptureDevice.authorizationStatus(for: .audio) — the correct macOS TCC API
        let av_capture_device_class = class!(AVCaptureDevice);
        let media_type: id = NSString::alloc(nil).init_str("soun"); // AVMediaTypeAudio
        let status: i64 = msg_send![av_capture_device_class, authorizationStatusForMediaType: media_type];

        // AVAuthorizationStatus enum:
        //   0 = NotDetermined
        //   1 = Restricted
        //   2 = Denied
        //   3 = Authorized
        match status {
            3 => {
                debug!("Microphone permission is granted (AVCaptureDevice)");
                PERMISSION_GRANTED.store(true, Ordering::SeqCst);
                PERMISSION_CACHED.store(true, Ordering::SeqCst);
                MicrophonePermissionStatus::Granted
            }
            2 | 1 => {
                debug!("Microphone permission is denied/restricted (AVCaptureDevice, status={})", status);
                PERMISSION_GRANTED.store(false, Ordering::SeqCst);
                PERMISSION_CACHED.store(true, Ordering::SeqCst);
                MicrophonePermissionStatus::Denied
            }
            0 => {
                debug!("Microphone permission is not determined (AVCaptureDevice)");
                MicrophonePermissionStatus::Undetermined
            }
            _ => {
                warn!("Unknown AVAuthorizationStatus for microphone: {}", status);
                MicrophonePermissionStatus::Undetermined
            }
        }
    }
}

#[cfg(not(target_os = "macos"))]
pub fn check_microphone_permission() -> MicrophonePermissionStatus {
    MicrophonePermissionStatus::NotApplicable
}

/// Request microphone permission using AVCaptureDevice (async, macOS TCC-compatible)
#[cfg(target_os = "macos")]
pub async fn request_microphone_permission() -> Result<MicrophonePermissionStatus, String> {
    use std::sync::Mutex;

    // Prevent concurrent permission requests — only one dialog at a time
    if PERMISSION_REQUEST_IN_FLIGHT.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst).is_err() {
        info!("Microphone permission request already in flight, waiting for result");
        // Another request is in progress — poll the cache until it resolves
        let start_time = std::time::Instant::now();
        let timeout = std::time::Duration::from_secs(30);
        loop {
            if PERMISSION_CACHED.load(Ordering::SeqCst) {
                let granted = PERMISSION_GRANTED.load(Ordering::SeqCst);
                return Ok(if granted {
                    MicrophonePermissionStatus::Granted
                } else {
                    MicrophonePermissionStatus::Denied
                });
            }
            if start_time.elapsed() > timeout {
                return Err("Timeout waiting for in-flight microphone permission request".to_string());
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }
    }

    // We won the CAS — we're the one making the request.
    // Mark that we've attempted a TCC dialog this session.
    PERMISSION_REQUESTED_THIS_SESSION.store(true, Ordering::SeqCst);

    // Ensure the in-flight flag is cleared on all exit paths.
    struct InFlightGuard;
    impl Drop for InFlightGuard {
        fn drop(&mut self) {
            PERMISSION_REQUEST_IN_FLIGHT.store(false, Ordering::SeqCst);
        }
    }
    let _guard = InFlightGuard;

    let result: Arc<Mutex<Option<Result<MicrophonePermissionStatus, String>>>> = Arc::new(Mutex::new(None));
    let result_clone = result.clone();

    // AVCaptureDevice.requestAccess(for: .audio) must be called; it handles
    // its own main-thread dispatch internally, so we don't need exec_sync.
    unsafe {
        let _pool = NSAutoreleasePool::new(nil);
        let av_capture_device_class = class!(AVCaptureDevice);
        let media_type: id = NSString::alloc(nil).init_str("soun"); // AVMediaTypeAudio

        let result_for_block = result_clone;
        let block = block::ConcreteBlock::new(move |granted: BOOL| {
            let status = if granted == YES {
                info!("Microphone permission granted by user (AVCaptureDevice)");
                PERMISSION_GRANTED.store(true, Ordering::SeqCst);
                PERMISSION_CACHED.store(true, Ordering::SeqCst);
                MicrophonePermissionStatus::Granted
            } else {
                info!("Microphone permission denied by user (AVCaptureDevice)");
                PERMISSION_GRANTED.store(false, Ordering::SeqCst);
                PERMISSION_CACHED.store(true, Ordering::SeqCst);
                MicrophonePermissionStatus::Denied
            };
            if let Ok(mut res) = result_for_block.lock() {
                *res = Some(Ok(status));
            }
        });
        let block = block.copy();

        let _: () = msg_send![av_capture_device_class, requestAccessForMediaType: media_type completionHandler: block];
    }

    // Poll for result with timeout
    let start_time = std::time::Instant::now();
    let timeout = std::time::Duration::from_secs(60);

    loop {
        if let Ok(res) = result.lock() {
            if let Some(result) = res.as_ref() {
                return result.clone();
            }
        }

        if start_time.elapsed() > timeout {
            return Err("Timeout waiting for microphone permission response".to_string());
        }

        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }
}

#[cfg(not(target_os = "macos"))]
pub async fn request_microphone_permission() -> Result<MicrophonePermissionStatus, String> {
    Ok(MicrophonePermissionStatus::NotApplicable)
}

/// Get cached permission status (fast check)
pub fn get_cached_permission() -> Option<bool> {
    if PERMISSION_CACHED.load(Ordering::SeqCst) {
        Some(PERMISSION_GRANTED.load(Ordering::SeqCst))
    } else {
        None
    }
}

/// Invalidate the permission cache so the next check re-queries the OS.
/// Call this when audio access fails despite the cache saying "granted" —
/// it forces a fresh TCC check on the next `ensure_microphone_ready()`.
pub fn invalidate_permission_cache() {
    info!("Invalidating microphone permission cache — next check will re-query TCC");
    PERMISSION_GRANTED.store(false, Ordering::SeqCst);
    PERMISSION_CACHED.store(false, Ordering::SeqCst);
}

/// Initialize audio session for recording.
///
/// On macOS, CoreAudio (used by `cpal`) manages its own audio session via the
/// Audio HAL. `AVAudioSession` is an iOS concept that was ported to macOS 12+
/// but interferes with CoreAudio's session management and causes permission
/// dialogs to appear without actually granting access to the HAL. We therefore
/// treat this as a no-op on macOS — `cpal` will handle session setup when it
/// opens the input stream.
pub fn initialize_audio_session() -> Result<(), String> {
    debug!("Audio session initialization: no-op (CoreAudio manages its own session)");
    Ok(())
}

/// Check if microphone hardware is available
pub fn is_microphone_available() -> bool {
    #[cfg(target_os = "macos")]
    {
        // Quick hardware check using system_profiler
        if let Ok(output) = std::process::Command::new("system_profiler")
            .args(["SPAudioDataType", "-detailLevel", "mini"])
            .output()
        {
            if output.status.success() {
                let result = String::from_utf8_lossy(&output.stdout);
                return result.contains("Input") ||
                       result.contains("Microphone") ||
                       result.contains("Built-in Microphone");
            }
        }

        // Fallback: assume microphone is available on modern Macs
        true
    }

    #[cfg(not(target_os = "macos"))]
    {
        true
    }
}

/// Combined permission and hardware check
pub async fn ensure_microphone_ready() -> Result<(), String> {
    // First check if microphone hardware is available
    if !is_microphone_available() {
        return Err("No microphone hardware detected".to_string());
    }

    // Check current permission status
    let status = check_microphone_permission();

    match status {
        MicrophonePermissionStatus::Granted => {
            // Initialize audio session
            initialize_audio_session()?;
            Ok(())
        }
        MicrophonePermissionStatus::Denied => {
            Err("Microphone permission denied. Please grant permission in System Settings > Privacy & Security > Microphone".to_string())
        }
        MicrophonePermissionStatus::Undetermined => {
            // If we already attempted the TCC dialog this session and the OS
            // still reports Undetermined, the decision isn't being persisted
            // (common in unsigned dev builds). Don't re-prompt — direct to
            // System Settings instead.
            if PERMISSION_REQUESTED_THIS_SESSION.load(Ordering::SeqCst) {
                warn!("Microphone permission still undetermined after previous request this session — TCC decision may not be persisting");
                return Err(
                    "Microphone permission could not be obtained. Please grant access manually in System Settings > Privacy & Security > Microphone".to_string()
                );
            }

            // Request permission (first attempt this session)
            match request_microphone_permission().await? {
                MicrophonePermissionStatus::Granted => {
                    // Initialize audio session after permission granted
                    initialize_audio_session()?;
                    Ok(())
                }
                MicrophonePermissionStatus::Denied => {
                    Err("Microphone permission denied by user. Please grant access in System Settings > Privacy & Security > Microphone".to_string())
                }
                _ => {
                    Err("Microphone permission could not be obtained. Please grant access in System Settings > Privacy & Security > Microphone".to_string())
                }
            }
        }
        MicrophonePermissionStatus::NotApplicable => {
            // Non-macOS platform, assume permission granted
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_permission_cache() {
        // Test that cache starts empty
        assert_eq!(get_cached_permission(), None);

        // Test cache after setting
        PERMISSION_GRANTED.store(true, Ordering::SeqCst);
        PERMISSION_CACHED.store(true, Ordering::SeqCst);
        assert_eq!(get_cached_permission(), Some(true));

        // Reset cache for other tests
        PERMISSION_CACHED.store(false, Ordering::SeqCst);
    }

    #[test]
    fn test_hardware_check() {
        // Hardware check should not panic
        let available = is_microphone_available();
        // On development machines, this should typically be true
        assert!(available || !available); // Always passes, just checking it doesn't panic
    }
}
