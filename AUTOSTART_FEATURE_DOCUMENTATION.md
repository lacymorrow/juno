# Auto-Launch Feature Documentation

## Overview

The Auto-Launch feature enables Juno to automatically start when users log in to their computer, providing seamless integration with the user's workflow. This feature is implemented using the `tauri-plugin-autostart` plugin and provides a complete cross-platform solution with macOS-specific optimizations.

## ✅ Implementation Status

**COMPLETE** - All components implemented and fully functional:
- ✅ Backend Rust commands and logic
- ✅ Frontend React UI integration
- ✅ Persistent settings storage
- ✅ Error handling and fallbacks
- ✅ macOS LaunchAgent configuration
- ✅ Proper permissions and capabilities
- ✅ Settings synchronization

## 🎯 User Features

### Settings Interface

The auto-launch feature is accessible through the **General Settings** section:

1. **Location**: Settings → General → Startup Behavior
2. **Control**: Toggle switch labeled "Launch at Login"
3. **Description**: "Automatically start Juno when you log in to your computer"
4. **Visual Feedback**: Loading state during toggle operations
5. **Error Handling**: Automatic state reversion on failures

### Behavior

- **Enabled**: Juno starts automatically when the user logs in
- **Disabled**: Manual application launch required
- **Persistent**: Setting persists across app restarts and system reboots
- **Sync**: System and app settings remain synchronized

## 🔧 Technical Implementation

### Backend Architecture

#### Commands (`src-tauri/src/commands/autostart.rs`)

```rust
// Core Commands
enable_autostart()     // Enable auto-launch functionality
disable_autostart()    // Disable auto-launch functionality  
is_autostart_enabled() // Check current auto-launch status
toggle_autostart()     // Toggle current state

// Initialization
init_autostart()       // Sync settings on app startup
```

#### Features

1. **System Integration**: Uses `MacosLauncher::LaunchAgent` for native macOS integration
2. **Local Persistence**: JSON-based settings storage with timestamps
3. **Error Recovery**: Fallback to saved settings if system check fails
4. **Synchronization**: Ensures system and app settings remain in sync

#### Storage Location

Settings stored in: `{app_config_dir}/autostart.json`

```json
{
  "enabled": true,
  "last_updated": "2024-01-15T10:30:45.123Z"
}
```

### Frontend Integration

#### Component (`src/components/settings/sections/GeneralSettings.tsx`)

```typescript
// State Management
const [autoLaunchEnabled, setAutoLaunchEnabled] = useState(false);
const [autoLaunchLoading, setAutoLaunchLoading] = useState(false);

// Load initial status
useEffect(() => {
  const enabled = await invoke<boolean>('is_autostart_enabled');
  setAutoLaunchEnabled(enabled);
}, []);

// Handle state changes
const handleAutoLaunchChange = async (enabled: boolean) => {
  // Loading state, error handling, state reversion
};
```

#### UI Components

- **Switch Control**: Accessible toggle with proper labeling
- **Loading State**: Visual feedback during operations
- **Error Handling**: Console logging and state restoration
- **Responsive Design**: Consistent with app design system

### Dependencies

#### Backend (Rust)
```toml
[dependencies]
tauri-plugin-autostart = "2.3.0"
```

#### Frontend (JavaScript)
```json
{
  "dependencies": {
    "@tauri-apps/plugin-autostart": "^2.3.0"
  }
}
```

### Permissions (`src-tauri/capabilities/default.json`)

```json
{
  "permissions": [
    "autostart:allow-enable",
    "autostart:allow-disable", 
    "autostart:allow-is-enabled"
  ]
}
```

## 🔗 Integration Points

### App Initialization (`src-tauri/src/lib.rs`)

```rust
// Plugin Registration
.plugin(tauri_plugin_autostart::init(
    tauri_plugin_autostart::MacosLauncher::LaunchAgent, 
    None
))

// Command Registration
.invoke_handler(tauri::generate_handler![
    enable_autostart,
    disable_autostart,
    is_autostart_enabled,
    toggle_autostart,
    // ... other commands
])

// Startup Initialization
let app_handle_for_autostart = app.handle().clone();
commands::autostart::init_autostart(&app_handle_for_autostart);
```

### Module Export (`src-tauri/src/commands/mod.rs`)

```rust
pub mod autostart;
pub use self::autostart::*;
```

## 🧪 Testing & Validation

### Manual Testing

1. **Toggle Test**: Enable/disable through settings UI
2. **Persistence Test**: Restart app, verify setting preserved
3. **System Test**: Log out/in, verify auto-launch behavior
4. **Error Test**: Simulate permission failures

### Automated Testing

```bash
# Compilation Check (REQUIRED)
cargo check --manifest-path src-tauri/Cargo.toml

# Full Test Suite
./run-all-tests.sh

# Rust Unit Tests
cargo test --manifest-path src-tauri/Cargo.toml
```

### Verification Commands

```bash
# Check LaunchAgent (macOS)
ls ~/Library/LaunchAgents/com.juno.app.plist

# Verify settings file
cat {app_config_dir}/autostart.json

# Test Tauri commands
# (Available through dev tools console when app is running)
```

## 🔒 Security Considerations

### Permissions
- Uses standard macOS LaunchAgent mechanism
- No additional permissions required beyond app permissions
- Settings stored in user-scoped directory only

### Privacy
- No data transmission related to auto-launch settings
- Local-only settings storage
- User-controlled enable/disable functionality

## 🚀 Platform Support

### macOS (Primary)
- ✅ LaunchAgent integration
- ✅ Native system integration
- ✅ Proper startup behavior
- ✅ User control through System Preferences

### Future Platform Support
- **Windows**: Registry-based startup entries
- **Linux**: XDG autostart desktop entries
- **Cross-platform**: Unified API through tauri-plugin-autostart

## 📝 Development Guidelines

### Code Patterns

1. **Error Handling**: Always use Result types, never panic
2. **Async Operations**: All commands are async for non-blocking UI
3. **State Management**: Centralized state with proper synchronization
4. **User Feedback**: Provide loading states and error messages

### Maintenance

1. **Dependencies**: Keep plugin version synchronized between Rust and JS
2. **Testing**: Test on actual macOS systems for permission validation
3. **Documentation**: Update this document for any behavioral changes
4. **Compatibility**: Verify with new Tauri versions

## 📋 Troubleshooting

### Common Issues

1. **Permission Denied**: 
   - Check app permissions in System Preferences
   - Verify app signing and entitlements

2. **Setting Not Persisted**:
   - Check app config directory permissions
   - Verify JSON file creation and format

3. **Auto-launch Not Working**:
   - Check LaunchAgent file exists and is valid
   - Verify app bundle identifier consistency

### Debug Commands

```rust
// Check current status
is_autostart_enabled().await

// Force sync settings
init_autostart(&app_handle)

// Manual toggle
toggle_autostart().await
```

### Log Monitoring

Enable debug logging to monitor autostart operations:

```bash
RUST_LOG=debug bun run tauri dev
```

Look for log entries:
- `[Setup] Autostart configuration initialized successfully`
- `Autostart enabled successfully`
- `Autostart disabled successfully`

## 🎯 Future Enhancements

### Potential Improvements

1. **Startup Delay**: Option to delay startup by X seconds
2. **Conditional Launch**: Launch only on specific conditions
3. **Startup Behavior**: Options for minimized/background start
4. **System Integration**: Better integration with system startup managers

### API Extensions

```rust
// Potential future commands
set_autostart_delay(seconds: u32) -> Result<(), String>
set_startup_behavior(behavior: StartupBehavior) -> Result<(), String>
get_autostart_diagnostics() -> Result<AutostartDiagnostics, String>
```

## 📄 Related Documentation

- **[DEVELOPMENT.md](DEVELOPMENT.md)** - General development guidelines
- **[LLMs.txt](LLMs.txt)** - Complete project instructions for AI agents
- **[SYSTEMATIC_IMPROVEMENTS_COMPLETE.md](SYSTEMATIC_IMPROVEMENTS_COMPLETE.md)** - Implementation history

---

**Auto-launch functionality provides seamless user experience with robust implementation and comprehensive error handling.**