# Escape Key Capture Fix

## Problem

The escape key was being captured globally at application startup and remained active throughout the entire application lifetime. This caused the escape key to be "eaten" by the Juno application even when the AI agent was not running, preventing other applications from receiving escape key presses.

## Root Cause

In `src-tauri/src/lib.rs`, the escape key was registered as a global shortcut during app initialization:

```rust
// Register Escape shortcut
if let Err(e) = app_handle_shortcuts.global_shortcut().register("Escape") {
    eprintln!("[GlobalShortcut Error] Failed to register Escape shortcut: {}", e);
}
```

This registration happened once at startup and remained active for the entire application lifecycle, capturing all escape key presses regardless of whether the AI agent was actively running.

## Solution

Implemented dynamic escape key registration that only captures the escape key when the AI agent is actually executing:

### 1. Removed Global Registration
- Removed the escape key registration from the app startup in `src-tauri/src/lib.rs`
- Kept the dictation toggle shortcut (Alt+D/Option+D) as it should remain globally active

### 2. Added Helper Functions
Added two helper functions in `src-tauri/src/lib.rs`:

```rust
pub fn register_escape_key_shortcut(app_handle: &AppHandle) {
    info!("[GlobalShortcut] Registering escape key for agent execution");
    if let Err(e) = app_handle.global_shortcut().register("Escape") {
        eprintln!("[GlobalShortcut Error] Failed to register Escape shortcut: {}", e);
    } else {
        info!("[GlobalShortcut] Escape key registered successfully");
    }
}

pub fn unregister_escape_key_shortcut(app_handle: &AppHandle) {
    info!("[GlobalShortcut] Unregistering escape key shortcut");
    if let Err(e) = app_handle.global_shortcut().unregister("Escape") {
        eprintln!("[GlobalShortcut Error] Failed to unregister Escape shortcut: {}", e);
    } else {
        info!("[GlobalShortcut] Escape key unregistered successfully");
    }
}
```

### 3. Dynamic Registration in Agent Execution
Modified both agent execution entry points to register/unregister the escape key:

#### In `src-tauri/src/anthropic.rs` (`submit_query` function):
```rust
// Register escape key shortcut for agent execution (only when agent is actually running)
crate::register_escape_key_shortcut(&app_handle);

info!("Starting agent run...");
let agent_result = agent_runner.run(query.clone(), cancel_rx).await;

// Always unregister escape key shortcut when agent finishes (regardless of success/failure)
crate::unregister_escape_key_shortcut(&app_handle);
```

#### In `src-tauri/src/commands/orchestrator.rs` (`submit_orchestrated_query` function):
```rust
// Register escape key shortcut for orchestrator execution
crate::register_escape_key_shortcut(&app_handle);

let orchestrator = get_orchestrator().await?;
let orchestrator_guard = orchestrator.lock().await;

let result = orchestrator_guard.process_command(query).await
    .map_err(|e| format!("Orchestrator error: {}", e));

// Unregister escape key shortcut when orchestrator finishes
crate::unregister_escape_key_shortcut(&app_handle);
```

## Benefits

1. **No More Key Eating**: The escape key is only captured when the AI agent is actually running
2. **Normal Application Behavior**: Other applications can receive escape key presses when Juno's AI is not active
3. **Maintained Functionality**: The escape key still works to cancel AI agent execution when needed
4. **Error Safety**: The escape key is always unregistered when agent execution finishes, regardless of success or failure
5. **Minimal Impact**: The dictation toggle shortcut and all other functionality remains unchanged

## Technical Details

- Uses Tauri's `global_shortcut().register()` and `global_shortcut().unregister()` methods
- Registration happens just before agent execution begins
- Unregistration happens immediately after agent execution completes
- Handles both success and error cases to ensure cleanup
- Covers both single-agent (`submit_query`) and multi-agent (`submit_orchestrated_query`) execution paths

## Testing Verification

To verify the fix works:

1. **When AI is not running**: Press escape in other applications - it should work normally
2. **When AI is running**: Press escape - it should cancel the AI execution
3. **After AI finishes**: Press escape in other applications - it should work normally again

## Files Modified

- `src-tauri/src/lib.rs`: Removed global registration, added helper functions
- `src-tauri/src/anthropic.rs`: Added dynamic registration to `submit_query`
- `src-tauri/src/commands/orchestrator.rs`: Added dynamic registration to `submit_orchestrated_query` 
