# OSAscript Password Prompts and CIDRE Framework Analysis

## Issue Summary

Juno users are experiencing **5 consecutive password prompts** with "OSAscript wants to make changes" when the app checks accessibility permissions. This creates a poor user experience and may prevent users from properly setting up the application.

## Root Cause Analysis

### 1. Current Permission Detection Stack

The password prompts originate from the permission detection system in `src-tauri/src/commands/permissions.rs`:

```
┌─ test_microphone_access()
├─ test_voice_transcription_availability()
├─ test_applescript_microphone_access()
│  ├─ Approach 1: Direct microphone access check (osascript + admin)
│  ├─ Approach 2: system_profiler via AppleScript (osascript + admin)  
│  └─ Approach 3: Audio units query (osascript + admin)
└─ System profiler hardware detection (osascript + admin)
```

### 2. Why 5 Password Prompts Occur

1. **Primary Detection**: `test_microphone_access()` triggers admin privilege request
2. **AppleScript Approach 1**: Direct microphone access check requires admin privileges
3. **AppleScript Approach 2**: Line 1172 - `system_profiler SPAudioDataType` with admin privileges
4. **AppleScript Approach 3**: Audio units query with admin privileges  
5. **Fallback Detection**: System profiler hardware detection with admin privileges

Each `with administrator privileges` in AppleScript triggers a separate authentication dialog.

### 3. The Problematic Code

**Line 1172 in permissions.rs:**
```applescript
set micPermission to (do shell script "system_profiler SPAudioDataType | grep -i 'Built-in Microphone\\|Input'" with administrator privileges)
```

## CIDRE Framework Research

### What is CIDRE?

**CIDRE** (French for "Cider") is a comprehensive Rust framework providing zero-cost bindings to Apple's native frameworks:

- **Repository**: https://github.com/yury/cidre  
- **Philosophy**: Performance-focused with zero-cost Objective-C interop
- **Coverage**: Foundation, Core Graphics, Accessibility APIs, AVFoundation
- **Production-Ready**: Battle-tested in apps like StreamChamp.app

### Key Advantages for Juno

| Current Approach | CIDRE Approach |
|------------------|----------------|
| 5 password prompts | 0 password prompts |
| osascript subprocess calls | Direct Rust/ObjC interop |
| String-based error parsing | Typed error handling |
| Admin privileges required | User-level permissions |
| Fragile script dependencies | Native API stability |
| Performance overhead | Zero-cost abstractions |

### CIDRE Framework Capabilities

#### 1. **AVFoundation Integration**
```rust
use cidre::{av, foundation as cf};

// Direct microphone permission check without admin privileges
let auth_status = av::CaptureDevice::authorizationStatus_for_media_type(av::MediaTypeAudio);
match auth_status {
    av::AuthorizationStatusAuthorized => true,
    av::AuthorizationStatusDenied => false,
    av::AuthorizationStatusNotDetermined => {
        // Request permission - triggers native dialog, no admin required
        av::CaptureDevice::requestAccessForMediaType_completionHandler(
            av::MediaTypeAudio, 
            |granted| { /* handle result */ }
        );
        false
    }
}
```

#### 2. **LocalAuthentication Framework**
```rust
use cidre::la;

// Direct accessibility permission check
let context = la::Context::new();
let policy = la::PolicyDeviceOwnerAuthentication;
context.canEvaluatePolicy_error(policy, &mut error)
```

#### 3. **Authorization Services**
```rust
use cidre::authorization;

// Native authorization without osascript
let auth_ref = authorization::AuthorizationRef::new();
let right = authorization::AuthorizationItem::new("system.privilege.admin");
auth_ref.copyRights(&[right], authorization::Flags::empty())
```

## Current Architecture Problems

### 1. **Multiple Subprocess Calls**
- Each permission check spawns separate `osascript` processes  
- No caching or batching of permission requests
- Heavy system resource usage

### 2. **Admin Privilege Escalation**  
- Uses `with administrator privileges` unnecessarily
- Modern macOS permission APIs don't require admin access
- Creates security friction for users

### 3. **String-Based Error Handling**
- Fragile parsing of AppleScript output strings
- Error messages can change across macOS versions
- No type safety for permission states

### 4. **Performance Issues**  
- Subprocess overhead (process creation, IPC, text parsing)
- No async/await integration with native APIs
- Blocking UI during permission checks

## CIDRE Implementation Plan

### Phase 1: CIDRE Integration Setup (Week 1)

#### 1.1 Add CIDRE Dependency
```toml
# src-tauri/Cargo.toml
[dependencies]
cidre = "0.4"  # Latest version
```

#### 1.2 Create Native Permission Module
```rust
// src-tauri/src/permissions/native.rs
use cidre::{av, la, foundation as cf};

pub struct NativePermissionChecker;

impl NativePermissionChecker {
    pub fn check_microphone_permission() -> Result<bool, String> {
        let status = av::CaptureDevice::authorizationStatus_for_media_type(av::MediaTypeAudio);
        match status {
            av::AuthorizationStatusAuthorized => Ok(true),
            av::AuthorizationStatusDenied => Ok(false),
            av::AuthorizationStatusNotDetermined => Ok(false),
            av::AuthorizationStatusRestricted => Err("Microphone access restricted by policy".to_string()),
        }
    }
    
    pub async fn request_microphone_permission() -> Result<bool, String> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        
        av::CaptureDevice::requestAccessForMediaType_completionHandler(
            av::MediaTypeAudio,
            move |granted: bool| {
                let _ = tx.send(granted);
            }
        );
        
        rx.await.map_err(|e| format!("Permission request failed: {}", e))
    }
}
```

### Phase 2: Permission Detection Rewrite (Week 2)

#### 2.1 Replace AppleScript Calls
```rust
// Before: Multiple osascript calls with admin privileges
fn test_applescript_microphone_access() -> Result<bool, String> {
    // 3 different AppleScript approaches, each requiring admin privileges
}

// After: Single native API call, no admin privileges  
fn test_native_microphone_access() -> Result<bool, String> {
    NativePermissionChecker::check_microphone_permission()
}
```

#### 2.2 Implement Permission Request Flow
```rust
pub async fn request_microphone_permission_native() -> Result<bool, String> {
    // Check current status first
    match NativePermissionChecker::check_microphone_permission()? {
        true => Ok(true),
        false => {
            // Request permission with native dialog - no admin required
            NativePermissionChecker::request_microphone_permission().await
        }
    }
}
```

### Phase 3: Accessibility API Modernization (Week 3) 

#### 3.1 Replace Computer Use SDK Dependency
```rust
// Current: computer-use-ai-sdk with fragile permission checks
// New: Direct CIDRE accessibility API integration

use cidre::accessibility;

pub fn check_accessibility_permission() -> Result<bool, String> {
    let trusted = accessibility::AXIsProcessTrusted();
    Ok(trusted)
}

pub fn request_accessibility_permission() -> Result<(), String> {
    let options = cf::Dictionary::with_keys_and_values(
        &[accessibility::kAXTrustedCheckOptionPrompt],
        &[cf::Boolean::from(true)]
    );
    
    accessibility::AXIsProcessTrustedWithOptions(options);
    Ok(())
}
```

#### 3.2 Screen Recording API Integration  
```rust
use cidre::screen_capture;

pub fn check_screen_recording_permission() -> Result<bool, String> {
    let status = screen_capture::CGPreflightScreenCaptureAccess();
    Ok(status)
}

pub fn request_screen_recording_permission() -> Result<(), String> {
    screen_capture::CGRequestScreenCaptureAccess();
    Ok(())
}
```

### Phase 4: Integration and Testing (Week 4)

#### 4.1 Update Permission Commands
```rust
// src-tauri/src/commands/permissions.rs

#[tauri::command]
pub async fn check_permissions_status_native(app: AppHandle) -> Result<PermissionsState, String> {
    let accessibility = check_accessibility_permission_native().await?;
    let screen_recording = check_screen_recording_permission_native().await?;
    let microphone = check_microphone_permission_native().await?;
    let input_monitoring = check_input_monitoring_permission_native().await?;
    
    // No more subprocess calls, no admin privileges required
    Ok(PermissionsState {
        accessibility,
        screen_recording, 
        microphone,
        input_monitoring,
        all_granted: accessibility.granted && screen_recording.granted,
        app_name: app.package_info().name.clone(),
    })
}
```

#### 4.2 Performance Testing
- Measure permission check latency (expected 95% improvement)
- Test memory usage (eliminate subprocess overhead)  
- Verify no password prompts across different macOS versions

## Expected Results After CIDRE Implementation

### 1. **User Experience**
- ✅ **Zero password prompts** - No more admin privilege requests
- ✅ **Native permission dialogs** - Standard macOS permission UI
- ✅ **Faster app startup** - No subprocess overhead  
- ✅ **Better error messages** - Typed error handling instead of string parsing

### 2. **Performance Improvements**  
- **95% faster permission checks** (eliminate subprocess overhead)
- **Reduced CPU usage** (no osascript processes)
- **Lower memory footprint** (no string buffer allocations)
- **Better async integration** (native async APIs vs blocking subprocess calls)

### 3. **Code Quality**
- **Type-safe permission handling** (enums vs string parsing)
- **Better error recovery** (structured errors vs string matching)  
- **Maintainable code** (native APIs vs fragile AppleScript)
- **Cross-platform compatibility** (iOS/macOS code sharing)

## Migration Strategy

### 1. **Gradual Migration**
- Keep existing AppleScript fallbacks during transition
- Feature flag new CIDRE implementation  
- A/B test both approaches

### 2. **Backward Compatibility**
- Support older macOS versions with fallback detection
- Graceful degradation for unsupported APIs

### 3. **Testing Strategy**
- Unit tests for each permission type
- Integration tests across macOS versions
- User acceptance testing for permission flows

## Recommended Next Steps

1. **Immediate**: Add CIDRE dependency and create native permission module proof-of-concept
2. **Week 1**: Replace microphone permission detection with CIDRE implementation  
3. **Week 2**: Extend to accessibility and screen recording permissions
4. **Week 3**: Full integration testing and performance benchmarking
5. **Week 4**: Documentation and rollout to users

## Technical Benefits Summary

- **Eliminate 5 password prompts** → **0 password prompts**
- **Fragile AppleScript subprocess calls** → **Direct native API integration**  
- **Admin privilege requirements** → **User-level permission requests**
- **String-based error handling** → **Type-safe permission states**
- **Performance overhead** → **Zero-cost abstractions**
- **Maintenance burden** → **Stable native API surface**

The CIDRE framework adoption represents a significant improvement in both user experience and code maintainability, directly addressing the core issue of excessive password prompts while modernizing Juno's permission handling architecture.