# WebKit Display Error Analysis and Solutions

## Error Overview

**Error**: `RemoteLayerTreeDrawingAreaProxyMac::scheduleDisplayLink(): page has no displayID`

This is a **WebKit rendering issue specific to macOS** that occurs when WebKit's layer rendering system cannot properly associate a web page with a macOS display ID for scheduling display refresh cycles.

## Root Cause Analysis

### 1. **WebKit Layer Tree Issues**
- WebKit uses a **Remote Layer Tree Drawing Area Proxy** to manage rendering across displays
- The `scheduleDisplayLink()` function requires a valid display ID to sync with the display's refresh rate
- When the display ID is missing or invalid, rendering synchronization fails

### 2. **Juno Project-Specific Factors**

Your Juno project has several components that actively interact with the display system:

#### **Active Display Querying**
```rust
// In src-tauri/mcp-server-os-level/src/platforms/macos/utils.rs
unsafe { CGMainDisplayID() }
find_display_containing_point(cursor_point)
CGDisplayBounds(target_display_id)
```

#### **Hardware Monitoring**
```rust
// In src-tauri/src/cloud/connector.rs
async fn get_screen_resolution() -> Option<String> {
    Command::new("system_profiler").args(&["SPDisplaysDataType"])
}
```

#### **Multiple Window Configuration**
```json
// tauri.conf.json
"windows": [
  {"label": "main", "width": 800, "height": 600},
  {"label": "floating-bar", "transparent": true, "alwaysOnTop": true},
  {"label": "settings", "width": 800, "height": 600}
]
```

## Immediate Solutions

### 1. **Development Environment Fixes**

**Quick Resolution** (try in order):
```bash
# 1. Restart your Mac (surprisingly effective)
sudo reboot

# 2. Update build tools
cargo install wasm-bindgen-cli --force
cargo install tauri-cli --force

# 3. Clear Tauri cache
rm -rf src-tauri/target
```

**Change Dev Server Configuration**:
```json
// In tauri.conf.json, change:
"devUrl": "http://127.0.0.1:1420"  // Instead of localhost
```

**Vite Configuration** (if using Vite):
```js
// vite.config.ts
export default defineConfig({
  server: {
    host: '127.0.0.1',  // Explicit IP instead of localhost
    port: 1420,
    strictPort: true
  }
});
```

### 2. **Display Coordination Improvements**

**Add Display State Management**:
```rust
// In src-tauri/src/state/mod.rs or new file
use std::sync::Mutex;
use core_graphics::display::CGDirectDisplayID;

pub struct DisplayState {
    pub primary_display_id: Mutex<Option<CGDirectDisplayID>>,
    pub available_displays: Mutex<Vec<CGDirectDisplayID>>,
    pub last_display_check: Mutex<Option<std::time::Instant>>,
}

impl DisplayState {
    pub fn new() -> Self {
        Self {
            primary_display_id: Mutex::new(None),
            available_displays: Mutex::new(Vec::new()),
            last_display_check: Mutex::new(None),
        }
    }
    
    pub fn update_display_cache(&self) -> Result<(), String> {
        // Cache display information to reduce CGDisplayBounds calls
        // Update only when displays change
    }
}
```

**Modify Display Operations**:
```rust
// In src-tauri/mcp-server-os-level/src/platforms/macos/utils.rs

// Add rate limiting to display queries
lazy_static::lazy_static! {
    static ref LAST_DISPLAY_CHECK: std::sync::Mutex<Option<std::time::Instant>> = 
        std::sync::Mutex::new(None);
    static ref CACHED_MAIN_DISPLAY: std::sync::Mutex<Option<CGDirectDisplayID>> = 
        std::sync::Mutex::new(None);
}

fn get_cached_main_display() -> CGDirectDisplayID {
    let mut last_check = LAST_DISPLAY_CHECK.lock().unwrap();
    let mut cached_display = CACHED_MAIN_DISPLAY.lock().unwrap();
    
    let should_refresh = last_check
        .map(|last| last.elapsed() > std::time::Duration::from_secs(5))
        .unwrap_or(true);
        
    if should_refresh || cached_display.is_none() {
        let display_id = unsafe { CGMainDisplayID() };
        *cached_display = Some(display_id);
        *last_check = Some(std::time::Instant::now());
        display_id
    } else {
        cached_display.unwrap()
    }
}
```

### 3. **Window Management Improvements**

**Sequential Window Creation**:
```rust
// In your window creation logic
async fn create_windows_sequentially(app: &AppHandle) -> Result<(), String> {
    // Create main window first
    create_main_window(app).await?;
    
    // Wait for main window to be ready
    tokio::time::sleep(Duration::from_millis(500)).await;
    
    // Create floating bar
    create_floating_bar(app).await?;
    
    // Wait before settings window
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    // Create settings window (but keep hidden)
    create_settings_window(app).await?;
    
    Ok(())
}
```

**Enhanced Window Configuration**:
```json
// tauri.conf.json - Add display-specific settings
{
  "app": {
    "windows": [
      {
        "label": "main",
        "title": "Juno",
        "width": 800,
        "height": 600,
        "visible": true,
        "center": true,
        "skipTaskbar": false,
        "fullscreen": false,
        "focus": true
      },
      {
        "label": "floating-bar",
        "url": "/floating-bar",
        "width": 110,
        "height": 60,
        "decorations": false,
        "transparent": true,
        "alwaysOnTop": true,
        "resizable": false,
        "skipTaskbar": true,
        "visible": false,
        "shadow": false,
        "center": false,
        "focus": false
      }
    ]
  }
}
```

### 4. **Hardware Monitoring Adjustments**

**Reduce Display Polling**:
```rust
// In src-tauri/src/cloud/connector.rs
impl HardwareMonitor {
    async fn get_screen_resolution() -> Option<String> {
        // Add caching to reduce system_profiler calls
        static CACHED_RESOLUTION: std::sync::Mutex<Option<(String, std::time::Instant)>> = 
            std::sync::Mutex::new(None);
            
        let mut cache = CACHED_RESOLUTION.lock().unwrap();
        
        if let Some((resolution, timestamp)) = &*cache {
            if timestamp.elapsed() < Duration::from_secs(30) {
                return Some(resolution.clone());
            }
        }
        
        // Only call system_profiler if cache is stale
        #[cfg(target_os = "macos")]
        {
            match Command::new("system_profiler")
                .args(&["SPDisplaysDataType"])
                .output()
            {
                Ok(output) => {
                    let output_str = String::from_utf8_lossy(&output.stdout);
                    for line in output_str.lines() {
                        if line.trim().starts_with("Resolution:") {
                            if let Some(resolution) = line.split(':').nth(1) {
                                let resolution = resolution.trim().to_string();
                                *cache = Some((resolution.clone(), std::time::Instant::now()));
                                return Some(resolution);
                            }
                        }
                    }
                    None
                },
                Err(e) => {
                    log::warn!("Failed to get screen resolution: {}", e);
                    None
                }
            }
        }
        
        #[cfg(not(target_os = "macos"))]
        None
    }
}
```

## Advanced Debugging

### 1. **Enable WebKit Debugging**
```bash
# Run with WebKit debug flags
WEBKIT_DEBUG=1 RUST_LOG=debug bun run tauri dev

# Or in your terminal before running
export WEBKIT_DISABLE_COMPOSITING_MODE=1
bun run tauri dev
```

### 2. **Monitor Display Events**
```rust
// Add to your main application
use core_graphics::display::CGDisplayRegisterReconfigurationCallback;

extern "C" fn display_reconfiguration_callback(
    display: CGDirectDisplayID,
    flags: u32,
    _user_info: *mut std::ffi::c_void,
) {
    log::warn!("Display {} reconfigured with flags: {}", display, flags);
    // Potentially restart WebKit context here
}

// Register in your app initialization
unsafe {
    CGDisplayRegisterReconfigurationCallback(
        Some(display_reconfiguration_callback),
        std::ptr::null_mut(),
    );
}
```

### 3. **Window State Monitoring**
```rust
// Add window state tracking
#[tauri::command]
async fn monitor_window_state(app: AppHandle) -> Result<String, String> {
    let windows = app.webview_windows();
    let mut status = Vec::new();
    
    for (label, window) in windows.iter() {
        let is_visible = window.is_visible().unwrap_or(false);
        let is_focused = window.is_focused().unwrap_or(false);
        
        status.push(format!("Window '{}': visible={}, focused={}", 
                          label, is_visible, is_focused));
    }
    
    Ok(status.join("; "))
}
```

## Testing Checklist

1. **Basic Functionality Test**:
   ```bash
   cargo check --manifest-path src-tauri/Cargo.toml
   bun run tauri dev
   ```

2. **Multi-Display Test** (if you have multiple monitors):
   - Move windows between displays
   - Test screenshot capture on each display
   - Verify floating bar positioning

3. **Performance Test**:
   - Monitor CPU usage during development
   - Check for excessive CGDisplayBounds calls
   - Verify memory usage with multiple windows

## Long-term Solutions

### 1. **Display Management Service**
Create a dedicated service to coordinate all display operations and cache display information.

### 2. **WebKit Context Isolation**
Consider isolating WebKit contexts per window to prevent display ID conflicts.

### 3. **Alternative Rendering Backend**
Evaluate migration to Tauri v2 with potential alternative backends if the issue persists.

## Related Issues and Resources

- **Tauri Issue**: https://github.com/tauri-apps/tauri/issues/8511
- **WebKit Bug**: Likely related to multi-display layer management
- **macOS Behavior**: System display changes can trigger this error

## Immediate Action Plan

1. **Try the immediate solutions** (restart, update tools, change dev URL)
2. **Implement display state caching** to reduce CGDisplayBounds calls
3. **Add window creation sequencing** to prevent simultaneous window creation
4. **Monitor for improvements** and document any successful workarounds

This error is typically **non-fatal** but indicates underlying display synchronization issues that could affect performance and user experience.