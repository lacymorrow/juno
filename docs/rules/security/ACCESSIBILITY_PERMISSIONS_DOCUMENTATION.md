# macOS Accessibility Permissions Documentation

## Overview

Juno AI requires several macOS accessibility permissions to function as a computer use agent. These permissions enable the application to interact with your desktop, take screenshots, process voice commands, and automate tasks on your behalf.

## Required Permissions

### 1. Accessibility Permission ✅ **CRITICAL**

**Required for:** Desktop automation, UI interaction, and computer use capabilities

**System Location:** `System Preferences > Privacy & Security > Accessibility`

#### Functions That Require Accessibility Permission:

##### **Desktop Automation Core**
- **Mouse Control:**
  - `desktop_click` - Click at specific coordinates
  - `left_click`, `right_click`, `double_click`, `triple_click` - Mouse interactions
  - `left_click_drag` - Drag operations
  - `mouse_move` - Move cursor to coordinates
  - `scroll` - Scroll windows and elements

- **Keyboard Control:**
  - `type_text` - Type text into active applications
  - `key` - Press individual keys and combinations
  - `hold_key` - Hold keys for extended periods
  - `press_key` - Key combinations with modifiers (Cmd, Shift, Alt, Ctrl)

- **Application Management:**
  - `open_application` - Launch applications by name
  - `focus_application` - Bring applications to foreground
  - `quit_application` - Close applications
  - `get_running_applications` - List active applications

- **Window Management:**
  - `focus_window` - Switch between windows
  - `get_window_info` - Get window properties
  - `list_windows` - Enumerate all windows
  - `window_operations` - Resize, move, minimize windows

- **Element Interaction:**
  - `get_focused_element_info` - Get details about UI elements
  - `element_interaction` - Interact with specific UI components
  - `accessibility_tree_navigation` - Navigate UI hierarchies

- **System Information:**
  - `get_system_info` - Access system state and properties
  - `manage_audio` - Control system audio settings

#### Permission Check Implementation:
```rust
// Location: src-tauri/src/commands/permissions.rs
use computer_use_ai_sdk::platforms::macos::permissions::check_accessibility_permissions;

let granted = check_accessibility_permissions(false)?;
```

#### Without This Permission:
- All desktop automation fails
- Cannot interact with native applications
- Computer use agent becomes non-functional
- Error: "Desktop automation is not available. Please grant accessibility permissions and restart the app."

---

### 2. Screen Recording Permission 📸 **CRITICAL**

**Required for:** Screenshot capture and visual analysis

**System Location:** `System Preferences > Privacy & Security > Screen Recording`

#### Functions That Require Screen Recording Permission:

##### **Screenshot Operations**
- **Desktop Screenshots:**
  - `capture_screenshot` - Full desktop screenshot
  - `capture_screenshot_command` - Screenshot with processing
  - `computer` tool with `action: "screenshot"` - Anthropic Computer Use API

- **Element Screenshots:**
  - `capture_element_screenshot` - Screenshot focused UI element
  - `capture_element_screenshot_command` - Element capture with processing

- **Browser Screenshots:**
  - `browser_screenshot` - Capture web pages
  - Element-specific browser screenshots

- **Visual Analysis:**
  - AI vision processing of screen content
  - Context understanding for automation decisions
  - Visual verification of actions performed

#### Permission Check Implementation:
```rust
// Location: src-tauri/src/commands/permissions.rs
async fn test_screen_recording_access() -> Result<bool, String> {
    use computer_use_ai_sdk::Desktop;
    
    let desktop = Desktop::new(false, false)?;
    desktop.capture_screenshot_base64()?; // Test actual screenshot capability
}
```

#### Without This Permission:
- Screenshots return empty/black images
- AI cannot see screen content
- Visual verification impossible
- Computer use agent loses "eyes"

---

### 3. Microphone Permission 🎤 **IMPORTANT**

**Required for:** Voice transcription and dictation features

**System Location:** `System Preferences > Privacy & Security > Microphone`

#### Functions That Require Microphone Permission:

##### **Voice Processing**
- **Voice Transcription:**
  - Real-time speech-to-text using Whisper.cpp
  - Voice command recognition
  - Dictation mode for text input

- **Voice Control:**
  - "Always listening" mode with wake words
  - Voice-activated agent commands
  - Hands-free operation

- **Audio Integration:**
  - Voice feedback and TTS coordination
  - Audio cue processing
  - Multi-modal interaction (voice + visual)

#### Permission Check Implementation:
```rust
// Location: src-tauri/src/commands/permissions.rs
async fn test_microphone_access() -> Result<bool, String> {
    // Test using AppleScript
    let output = Command::new("osascript")
        .args(&["-e", "tell application \"System Events\" to return microphone authorization status"])
        .output();
}
```

#### Without This Permission:
- Voice transcription fails
- No voice control capabilities
- Dictation mode unavailable
- Agent becomes keyboard/mouse only

---

### 4. Input Monitoring Permission ⌨️ **ENHANCEMENT**

**Required for:** Global keyboard shortcuts and advanced input monitoring

**System Location:** `System Preferences > Privacy & Security > Input Monitoring`

#### Functions That Require Input Monitoring Permission:

##### **Advanced Input Features**
- **Global Shortcuts:**
  - System-wide hotkey detection
  - Background keypress monitoring
  - Context-aware input handling

- **Input Analysis:**
  - Keystroke pattern analysis
  - Advanced input monitoring for automation
  - System-level input event handling

#### Permission Check Implementation:
```rust
// Location: src-tauri/src/commands/permissions.rs
async fn test_input_monitoring_access() -> bool {
    // Test using ioreg to check HID event access
    let output = Command::new("ioreg")
        .args(&["-c", "IOHIDEventDriver"])
        .output();
}
```

#### Without This Permission:
- No global keyboard shortcuts
- Limited input monitoring capabilities
- Reduced automation context awareness

---

## Permission Architecture

### Multi-layered Detection System

1. **Primary Check:** `computer_use_ai_sdk` permission APIs
2. **Functional Test:** Actual capability verification (e.g., taking screenshot)
3. **Fallback Detection:** System command validation

### Permission Request Flow

```rust
// Enhanced permission request with auto-redirect
pub async fn request_accessibility_permission_with_auto_redirect(auto_open_settings: bool) -> Result<bool, String> {
    // 1. Check current status
    let granted = check_accessibility_permissions(false)?;
    
    if !granted {
        // 2. Show system prompt
        check_accessibility_permissions(true)?;
        
        // 3. Auto-open System Settings if enabled
        if auto_open_settings {
            open_system_settings_for_permission("accessibility")?;
        }
    }
    
    granted
}
```

### System Settings Integration

**Modern macOS (13+):**
```bash
open "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility"
open "x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture"
open "x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone"
open "x-apple.systempreferences:com.apple.preference.security?Privacy_ListenEvent"
```

**Legacy macOS:**
```bash
open -b com.apple.systempreferences /System/Library/PreferencePanes/Security.prefPane
```

## Implementation Details

### Entitlements Required

**File:** `src-tauri/juno.entitlements`
```xml
<key>com.apple.security.automation.apple-events</key>
<true/>
<key>com.apple.security.cs.allow-unsigned-executable-memory</key>
<true/>
<key>com.apple.security.cs.disable-library-validation</key>
<true/>
```

### Usage Descriptions

**File:** `src-tauri/Info.plist`
```xml
<key>NSAccessibilityUsageDescription</key>
<string>Juno requires accessibility permissions to automate desktop tasks and interact with applications on your behalf.</string>

<key>NSMicrophoneUsageDescription</key>
<string>Juno uses the microphone for voice transcription and voice commands.</string>

<key>NSAppleEventsUsageDescription</key>
<string>Juno uses Apple Events to control and automate applications.</string>
```

### Bundle Configuration

**File:** `src-tauri/tauri.conf.json`
```json
{
  "bundle": {
    "resources": ["Info.plist"],
    "macOS": {
      "entitlements": "juno.entitlements",
      "files": {"Info.plist": "Info.plist"}
    }
  }
}
```

## Critical Notes

### **Always Test Built Apps**
- Permission behavior differs between development and built applications
- Built apps have different bundle identifiers and security contexts
- Use `cargo tauri build` and test the generated .app bundle

### **Application Restart Required**
- macOS requires app restart after granting accessibility permissions
- Permission changes don't take effect until restart
- Juno automatically detects this and prompts for restart

### **Permission Verification**
- Real functional testing (e.g., taking actual screenshots)
- Not just checking system permission flags
- Ensures permissions actually work in practice

## Troubleshooting

### Common Issues

1. **"Permission granted but not working"**
   - Restart the application
   - Check bundle identifier matches in System Preferences
   - Verify entitlements are properly configured

2. **"Built app shows different permission status"**
   - This is expected - dev and built apps have different bundle IDs
   - Always test with built applications for production

3. **"Permission dialogs not appearing"**
   - System may have cached previous denial
   - Reset permissions: `tccutil reset All com.juno.app`
   - Manually add app in System Preferences

### Debugging Commands

```bash
# Check accessibility permissions
cargo check --manifest-path src-tauri/Cargo.toml

# Reset TCC database (requires restart)
sudo tccutil reset All

# Check bundle ID being used
echo $TAURI_BUNDLE_IDENTIFIER
```

## Security Considerations

- **Principle of Least Privilege:** Only request permissions actually needed
- **Graceful Degradation:** Continue operation with reduced capabilities when possible
- **User Education:** Clear explanations of why each permission is required
- **Transparent Operation:** Log all permission checks and usage

## Future Enhancements

- **Granular Permissions:** Request only specific capabilities needed
- **Runtime Permission Requests:** Ask for permissions as features are used
- **Permission Health Monitoring:** Continuous verification of permission status
- **Enhanced User Experience:** Better onboarding and permission explanation flow