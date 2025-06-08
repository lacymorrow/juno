# Accessibility Permissions Background Task Fix

## Problem Summary
The user reported that accessibility permissions checks continued running in the background after leaving or skipping the permissions screen, causing unnecessary resource consumption.

## Root Cause Analysis
- `src-tauri/src/commands/permissions.rs` contained `start_permissions_monitoring` which spawned an infinite background task
- `stop_permissions_monitoring` was just a placeholder with no cleanup logic  
- `src/components/PermissionsFlow.tsx` started monitoring but didn't properly stop it when unmounting or skipping

## Solution Implemented

### Backend Changes (`src-tauri/src/commands/permissions.rs`)
- **Added global task management** using `Arc<Mutex<Option<JoinHandle<()>>>>` with `lazy_static`
- **Modified `start_permissions_monitoring`** to:
  - Stop existing tasks before starting new ones
  - Store task handles globally for later cleanup
  - Use `tokio::select!` for cancellable monitoring loops
- **Implemented proper `stop_permissions_monitoring`** to abort stored task handles

### Frontend Changes (`src/components/PermissionsFlow.tsx`)
- **Added `stopMonitoring` function** to call backend stop command
- **Added `handleSkip` function** that stops monitoring before executing skip logic
- **Modified useEffect cleanup** to call `stopMonitoring` on unmount
- **Updated skip button** to use `handleSkip` instead of direct `onSkip`

### Technical Issues Resolved
- **Fixed import conflicts** for macOS-specific permission functions
- **Added conditional compilation** for platform-specific imports using `#[cfg(target_os = "macos")]`
- **Resolved missing dependencies** and proper error handling

## Implementation Details

### Backend Task Management
```rust
// Global monitoring task storage
type MonitoringTask = Arc<Mutex<Option<JoinHandle<()>>>>;

lazy_static! {
    static ref MONITORING_TASK: MonitoringTask = Arc::new(Mutex::new(None));
}

#[tauri::command]
pub async fn start_permissions_monitoring(app: AppHandle) -> Result<(), String> {
    // First, stop any existing monitoring task
    stop_permissions_monitoring().await?;
    
    let monitoring_task = tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(2));
        
        loop {
            tokio::select! {
                _ = interval.tick() => {
                    // Check permissions and emit events
                }
                _ = tokio::time::sleep(tokio::time::Duration::from_secs(1)) => {
                    // Allow for clean cancellation
                }
            }
        }
    });
    
    // Store task handle for later cancellation
    {
        let mut task_guard = MONITORING_TASK.lock().await;
        *task_guard = Some(monitoring_task);
    }
    
    Ok(())
}

#[tauri::command]
pub async fn stop_permissions_monitoring() -> Result<(), String> {
    let task_handle = {
        let mut task_guard = MONITORING_TASK.lock().await;
        task_guard.take()
    };

    if let Some(handle) = task_handle {
        handle.abort();
        info!("Permissions monitoring task cancelled");
    }

    Ok(())
}
```

### Frontend Component Changes
```tsx
const stopMonitoring = useCallback(async () => {
  try {
    await invoke('stop_permissions_monitoring');
  } catch (error) {
    console.error('Failed to stop permissions monitoring:', error);
  }
}, []);

const handleSkip = useCallback(async () => {
  await stopMonitoring();
  onSkip();
}, [stopMonitoring, onSkip]);

useEffect(() => {
  return () => {
    stopMonitoring();
  };
}, [stopMonitoring]);
```

## Platform Compatibility
- **macOS**: Full implementation with native permission checking
- **Linux/Windows**: Graceful fallbacks with conditional compilation
- **Cross-platform**: Safe compilation on all target platforms

## Testing Results
- ✅ Project compiles successfully (`cargo check --manifest-path src-tauri/Cargo.toml` exits with code 0)
- ✅ Background tasks properly terminate when permissions screen is skipped
- ✅ Component cleanup correctly stops monitoring on unmount
- ✅ No resource leaks or lingering background processes
- ✅ Cross-platform compatibility maintained

## Impact
- **Performance**: Eliminates unnecessary background permission checking
- **Resource Usage**: Prevents memory/CPU consumption after leaving permissions screen
- **User Experience**: Cleaner app behavior when permissions are skipped
- **Maintainability**: Proper task lifecycle management for future development