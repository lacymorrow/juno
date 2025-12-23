# Race Condition Prevention Guide

This guide provides best practices for preventing race conditions in the Juno AI application. Following these patterns will help ensure thread safety and prevent concurrency bugs.

## Common Race Conditions

Race conditions occur when multiple threads or asynchronous operations interact with shared state in an unpredictable order. In Juno, we've identified and fixed several common patterns:

1. **State Manager Race Conditions** - Inconsistent access to shared state
2. **UI State Transition Races** - Multiple async tasks updating UI state
3. **Event Handler Duplication** - Multiple handlers for the same event
4. **Shell Command Execution Races** - Multiple lock acquisitions in tight loops
5. **Tool Provider Refresh Races** - Skipped providers due to try_lock failures

## Best Practices

### 1. State Access Patterns

✅ **DO**:

- Use async versions of state access methods where possible
- Use consistent locking patterns (prefer `.lock().await` over `.try_lock()`)
- Add proper error handling for lock acquisition failures
- Document state access requirements in function docstrings

❌ **DON'T**:

- Mix blocking and non-blocking lock patterns
- Return `None` from `try_lock()` without proper error handling
- Access shared state without synchronization
- Hold locks for longer than necessary

### 2. UI State Transitions

✅ **DO**:

- Use transition IDs to track state changes
- Check if transitions are still valid before applying
- Consolidate state transitions into single methods
- Log state transitions for debugging

```rust
// Good pattern
let transition_id = Uuid::new_v4().to_string();
self.current_transition_id = Some(transition_id.clone());

tokio::spawn(async move {
    sleep(Duration::from_millis(300)).await;
    if let Some(manager) = get_bar_manager(&app_handle).await {
        let mut manager = manager.lock().await;
        
        // Only proceed if this is still the active transition
        if manager.current_transition_id.as_ref() == Some(&transition_id) {
            manager.set_state(BarState::Input).await;
            manager.current_transition_id = None;
        }
    }
});
```

❌ **DON'T**:

- Spawn multiple tasks that modify the same state
- Assume state hasn't changed between task spawn and execution
- Modify state without checking if transitions are still valid

### 3. Event Handlers

✅ **DO**:

- Use event deduplication patterns
- Check for existing handlers before adding new ones
- Use a centralized event manager
- Add debug logs for event processing

❌ **DON'T**:

- Register duplicate event handlers
- Process events in multiple places
- Assume events are processed in a specific order

### 4. Async Task Management

✅ **DO**:

- Use proper cancellation tokens for async tasks
- Clean up resources when tasks complete
- Use structured concurrency patterns
- Limit the number of spawned tasks

❌ **DON'T**:

- Spawn tasks without tracking them
- Assume tasks complete in a specific order
- Leak resources in spawned tasks

### 5. Lock Acquisition

✅ **DO**:

- Consolidate multiple lock acquisitions into single scopes
- Keep critical sections small
- Use read/write locks for appropriate access patterns
- Document lock ordering to prevent deadlocks

```rust
// Good pattern - single lock scope
{
    let mut process = self.process.lock().map_err(|e| format!("Failed to lock process mutex: {}", e))?;
    
    // Read both stdout and stderr in single critical section
    if let Some(stdout) = process.stdout.as_mut() {
        // Read stdout
    }
    
    if let Some(stderr) = process.stderr.as_mut() {
        // Read stderr
    }
}
```

❌ **DON'T**:

- Acquire and release the same lock repeatedly in loops
- Hold locks across await points
- Use blocking locks in async code

### 6. Tool Provider Management

✅ **DO**:

- Use retry mechanisms for `try_lock` failures
- Implement proper error handling for provider access
- Use background tasks for retrying operations
- Log provider state changes

❌ **DON'T**:

- Silently skip providers that fail `try_lock`
- Assume all providers are always available
- Use inconsistent access patterns

## Testing for Race Conditions

1. Use the `scripts/check-race-conditions.sh` script to detect potential issues
2. Run stress tests with multiple concurrent operations
3. Use thread sanitizers when available
4. Log and analyze timing-related bugs

## Real-World Examples

### Fixed: Floating Bar State Race

```rust
// Before: Multiple tasks could race to update state
tokio::spawn(async move {
    sleep(Duration::from_millis(300)).await;
    if let Some(manager) = get_bar_manager(&app_handle).await {
        let mut manager = manager.lock().await;
        manager.set_state(BarState::Input).await;
    }
});

// After: Using transition IDs to prevent races
let transition_id = Uuid::new_v4().to_string();
self.current_transition_id = Some(transition_id.clone());

tokio::spawn(async move {
    sleep(Duration::from_millis(300)).await;
    if let Some(manager) = get_bar_manager(&app_handle).await {
        let mut manager = manager.lock().await;
        
        if manager.current_transition_id.as_ref() == Some(&transition_id) {
            manager.set_state(BarState::Input).await;
            manager.current_transition_id = None;
        }
    }
});
```

### Fixed: Shell Command Race

```rust
// Before: Multiple lock acquisitions could race
{
    let mut process = self.process.lock().map_err(...)?;
    // Read stdout
}

{
    let mut process = self.process.lock().map_err(...)?;  
    // Read stderr
}

// After: Single lock scope prevents races
{
    let mut process = self.process.lock().map_err(...)?;
    
    // Read both stdout and stderr in single critical section
    if let Some(stdout) = process.stdout.as_mut() {
        // Read stdout
    }
    
    if let Some(stderr) = process.stderr.as_mut() {
        // Read stderr
    }
}
```

## Conclusion

Race conditions can be subtle and difficult to debug. By following these patterns and using the provided tools, you can prevent many common concurrency issues in the Juno application.

Remember:

- Use consistent locking patterns
- Track state transitions with IDs
- Consolidate lock acquisitions
- Implement proper retry mechanisms
- Test thoroughly for concurrency issues

For more detailed analysis, run the `scripts/check-race-conditions.sh` script regularly.
