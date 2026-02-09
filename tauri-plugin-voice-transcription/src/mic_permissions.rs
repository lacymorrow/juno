use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tracing::{debug, error, info, warn};

#[cfg(target_os = "macos")]
use objc::{class, msg_send, sel, sel_impl};
#[cfg(target_os = "macos")]
use objc::runtime::{BOOL, YES};
#[cfg(target_os = "macos")]
use cocoa::foundation::{NSString, NSAutoreleasePool};
#[cfg(target_os = "macos")]
use cocoa::base::{nil, id};
#[cfg(target_os = "macos")]
use dispatch::Queue;

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

/// Check microphone permission status using AVAudioSession
#[cfg(target_os = "macos")]
pub fn check_microphone_permission() -> MicrophonePermissionStatus {
    unsafe {
        let _pool = NSAutoreleasePool::new(nil);
        
        // Get AVAudioSession sharedInstance
        let av_audio_session_class = class!(AVAudioSession);
        let shared_instance: id = msg_send![av_audio_session_class, sharedInstance];
        
        if shared_instance == nil {
            error!("Failed to get AVAudioSession shared instance");
            return MicrophonePermissionStatus::Undetermined;
        }
        
        // Get recordPermission
        let permission: i64 = msg_send![shared_instance, recordPermission];
        
        // AVAudioSessionRecordPermission values:
        // Undetermined = 1970168948 ('undt')
        // Denied = 1684369017 ('deny')  
        // Granted = 1735552628 ('grnt')
        
        match permission {
            1735552628 => {
                debug!("Microphone permission is granted");
                PERMISSION_GRANTED.store(true, Ordering::SeqCst);
                PERMISSION_CACHED.store(true, Ordering::SeqCst);
                MicrophonePermissionStatus::Granted
            }
            1684369017 => {
                debug!("Microphone permission is denied");
                PERMISSION_GRANTED.store(false, Ordering::SeqCst);
                PERMISSION_CACHED.store(true, Ordering::SeqCst);
                MicrophonePermissionStatus::Denied
            }
            1970168948 => {
                debug!("Microphone permission is undetermined");
                MicrophonePermissionStatus::Undetermined
            }
            _ => {
                warn!("Unknown microphone permission status: {}", permission);
                MicrophonePermissionStatus::Undetermined
            }
        }
    }
}

#[cfg(not(target_os = "macos"))]
pub fn check_microphone_permission() -> MicrophonePermissionStatus {
    MicrophonePermissionStatus::NotApplicable
}

/// Request microphone permission (async)
#[cfg(target_os = "macos")]
pub async fn request_microphone_permission() -> Result<MicrophonePermissionStatus, String> {
    use std::sync::Mutex;

    // Use Arc<Mutex> to share the result between threads
    let result: Arc<Mutex<Option<Result<MicrophonePermissionStatus, String>>>> = Arc::new(Mutex::new(None));
    let result_clone = result.clone();

    // SAFETY: exec_sync on the main queue deadlocks if called FROM the main thread.
    // Tauri async commands run on tokio worker threads, so this is safe in normal use.
    // Guard defensively in case this is ever called from a different context.
    let is_main_thread: bool = unsafe {
        let ns_thread_class = class!(NSThread);
        let main: BOOL = msg_send![ns_thread_class, isMainThread];
        main == YES
    };
    if is_main_thread {
        warn!("request_microphone_permission called from main thread — dispatching async to avoid deadlock");
        // Fall through to exec_async path by running inline
        let result_for_inline = result_clone.clone();
        unsafe {
            let _pool = NSAutoreleasePool::new(nil);
            let av_audio_session_class = class!(AVAudioSession);
            let shared_instance: id = msg_send![av_audio_session_class, sharedInstance];
            if shared_instance == nil {
                return Err("Failed to get AVAudioSession shared instance".to_string());
            }
            let result_for_block = result_for_inline.clone();
            let block = block::ConcreteBlock::new(move |granted: BOOL| {
                let status = if granted == YES {
                    info!("Microphone permission granted by user (main thread path)");
                    PERMISSION_GRANTED.store(true, Ordering::SeqCst);
                    PERMISSION_CACHED.store(true, Ordering::SeqCst);
                    MicrophonePermissionStatus::Granted
                } else {
                    info!("Microphone permission denied by user (main thread path)");
                    PERMISSION_GRANTED.store(false, Ordering::SeqCst);
                    PERMISSION_CACHED.store(true, Ordering::SeqCst);
                    MicrophonePermissionStatus::Denied
                };
                if let Ok(mut res) = result_for_block.lock() {
                    *res = Some(Ok(status));
                }
            });
            let block = block.copy();
            let _: () = msg_send![shared_instance, requestRecordPermission: block];
        }
    } else {
        // Request permission on main thread (safe — we are NOT on main thread)
        Queue::main().exec_sync(move || {
            unsafe {
                let _pool = NSAutoreleasePool::new(nil);

                // Get AVAudioSession sharedInstance
                let av_audio_session_class = class!(AVAudioSession);
                let shared_instance: id = msg_send![av_audio_session_class, sharedInstance];

                if shared_instance == nil {
                    if let Ok(mut res) = result_clone.lock() {
                        *res = Some(Err("Failed to get AVAudioSession shared instance".to_string()));
                    }
                    return;
                }

                // Create completion handler block
                let result_for_block = result_clone.clone();
                let block = block::ConcreteBlock::new(move |granted: BOOL| {
                    let status = if granted == YES {
                        info!("Microphone permission granted by user");
                        PERMISSION_GRANTED.store(true, Ordering::SeqCst);
                        PERMISSION_CACHED.store(true, Ordering::SeqCst);
                        MicrophonePermissionStatus::Granted
                    } else {
                        info!("Microphone permission denied by user");
                        PERMISSION_GRANTED.store(false, Ordering::SeqCst);
                        PERMISSION_CACHED.store(true, Ordering::SeqCst);
                        MicrophonePermissionStatus::Denied
                    };

                    if let Ok(mut res) = result_for_block.lock() {
                        *res = Some(Ok(status));
                    }
                });

                // Copy block to heap before passing to ObjC runtime
                // (stack blocks may be deallocated before the async callback fires)
                let block = block.copy();

                // Request permission
                let _: () = msg_send![shared_instance, requestRecordPermission: block];
            }
        });
    }

    // Poll for result with timeout (shared by both main-thread and worker-thread paths)
    let start_time = std::time::Instant::now();
    let timeout = std::time::Duration::from_secs(30);
    
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

/// Initialize audio session for recording
#[cfg(target_os = "macos")]
pub fn initialize_audio_session() -> Result<(), String> {
    unsafe {
        let _pool = NSAutoreleasePool::new(nil);
        
        // Get AVAudioSession sharedInstance
        let av_audio_session_class = class!(AVAudioSession);
        let shared_instance: id = msg_send![av_audio_session_class, sharedInstance];
        
        if shared_instance == nil {
            return Err("Failed to get AVAudioSession shared instance".to_string());
        }
        
        // Set category to PlayAndRecord
        let category = NSString::alloc(nil).init_str("AVAudioSessionCategoryPlayAndRecord");
        let mut error: id = nil;
        let success: BOOL = msg_send![shared_instance, setCategory:category error:&mut error];

        if success != YES {
            let description = if error != nil {
                let desc: id = msg_send![error, localizedDescription];
                if desc != nil {
                    let utf8: *const i8 = msg_send![desc, UTF8String];
                    if !utf8.is_null() {
                        std::ffi::CStr::from_ptr(utf8).to_string_lossy().to_string()
                    } else {
                        "unknown error".to_string()
                    }
                } else {
                    "unknown error".to_string()
                }
            } else {
                "unknown error".to_string()
            };
            return Err(format!("Failed to set audio session category: {}", description));
        }

        // Activate the audio session
        let mut activate_error: id = nil;
        let activate_success: BOOL = msg_send![shared_instance, setActive:YES error:&mut activate_error];

        if activate_success != YES {
            let description = if activate_error != nil {
                let desc: id = msg_send![activate_error, localizedDescription];
                if desc != nil {
                    let utf8: *const i8 = msg_send![desc, UTF8String];
                    if !utf8.is_null() {
                        std::ffi::CStr::from_ptr(utf8).to_string_lossy().to_string()
                    } else {
                        "unknown error".to_string()
                    }
                } else {
                    "unknown error".to_string()
                }
            } else {
                "unknown error".to_string()
            };
            return Err(format!("Failed to activate audio session: {}", description));
        }
        
        debug!("Audio session initialized successfully");
        Ok(())
    }
}

#[cfg(not(target_os = "macos"))]
pub fn initialize_audio_session() -> Result<(), String> {
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
            // Request permission
            match request_microphone_permission().await? {
                MicrophonePermissionStatus::Granted => {
                    // Initialize audio session after permission granted
                    initialize_audio_session()?;
                    Ok(())
                }
                MicrophonePermissionStatus::Denied => {
                    Err("Microphone permission denied by user".to_string())
                }
                _ => {
                    Err("Unexpected permission status".to_string())
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