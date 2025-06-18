# Event Listener Rules - Preventing Duplicate Listeners and Race Conditions

## Overview

This document establishes mandatory rules for event listener management to prevent the kind of race conditions and application crashes that occurred with duplicate `voice-transcription:final-result` listeners.

## Critical Issues Identified

- **Duplicate listeners** causing double execution of clipboard and typing operations
- **Race conditions** in state management leading to system instability  
- **Application crashes** due to concurrent access to system resources

## Mandatory Rules

### 1. One Listener Per Event Type

**RULE**: Each event type MUST have exactly ONE listener in the application.

```rust
// ✅ CORRECT - Single listener per event
app.listen("voice-transcription:final-result", move |event| {
    // Handle both dictation and agent modes in one place
});

// ❌ INCORRECT - Multiple listeners for same event
app.listen("voice-transcription:final-result", move |event| { /* Handler 1 */ });
app.listen("voice-transcription:final-result", move |event| { /* Handler 2 */ }); // DUPLICATE!
```

### 2. Event Listener Registry

**RULE**: All event listeners MUST be documented in a central registry with their purpose.

**Location**: `src-tauri/src/lib.rs` - Event Listeners Section

Current registered listeners:

- `voice-transcription:dictation-started` - Plugin dictation start events
- `voice-transcription:partial-result` - Partial transcription updates
- `voice-transcription:final-result` - **SINGLE HANDLER** for both dictation and agent modes
- `voice-transcription:dictation-stopped` - Plugin dictation stop events  
- `voice-transcription:error` - Plugin error handling
- `dictation-transcription-start` - Internal dictation lifecycle
- `dictation-committed` - Dictation threshold reached
- `dictation-transcription-cancel` - Dictation cancelled before threshold
- `dictation-stop` - Normal dictation completion
- `dictation-transcription-force-stop` - Emergency dictation cleanup
- `dictation-transcription-force-cleanup` - Stuck state recovery
- `always-listening:activated` - Wake word detection
- `always-listening:transcription` - Post-wake-word transcription
- `agent-transcription-start` - Agent mode start (hold)
- `agent-stop` - Agent mode normal stop
- `agent-cancel` - Agent mode cancelled
- `agent-force-stop` - Agent mode emergency stop
- `agent-force-cleanup` - Agent stuck state recovery
- `agent-transcription-stop` - Agent transcription completion

### 3. Mode-Based Logic Within Single Handlers

**RULE**: Use conditional logic within one handler rather than multiple handlers.

```rust
// ✅ CORRECT - Single handler with mode detection
app.listen("voice-transcription:final-result", move |event| {
    let app_handle_clone = app_handle_for_listener.clone();
    tauri::async_runtime::spawn(async move {
        let app_state = app_handle_clone.state::<state::AppState>();
        let is_dictation_active = app_state.dictation_active.lock()
            .map(|active| *active)
            .unwrap_or(false);

        if is_dictation_active {
            // Handle dictation mode
            handle_dictation_final_result(&app_handle_clone, event).await;
        } else {
            // Handle agent mode  
            handle_agent_final_result(&app_handle_clone, event).await;
        }
    });
});
```

### 4. Compile-Time Detection

**RULE**: Before any commit, run duplicate listener detection.

```bash
# Check for duplicate listeners
grep -n 'app\.listen("' src-tauri/src/lib.rs | cut -d'"' -f2 | sort | uniq -c | sort -nr

# Expected output: All counts should be 1
#    1 voice-transcription:final-result
#    1 voice-transcription:partial-result
#    etc.
```

### 5. Code Review Checklist

**RULE**: All PRs touching event listeners must pass this checklist:

- [ ] No duplicate event listener names
- [ ] Each listener documented in registry
- [ ] Mode-based logic uses conditionals, not multiple listeners
- [ ] Race condition analysis completed
- [ ] Compilation check passed: `cargo check --manifest-path src-tauri/Cargo.toml`
- [ ] Duplicate detection passed: `grep -n 'app\.listen("' src-tauri/src/lib.rs | cut -d'"' -f2 | sort | uniq -c | sort -nr`

### 6. Event Listener Patterns

**RULE**: Follow these patterns for different event types:

#### Plugin Events (voice-transcription:*)

- Handle in main event setup section
- Use single handlers with mode detection
- Always include error handling

#### Internal Lifecycle Events (dictation-*, agent-*)

- Keep handlers focused on single responsibility
- Use proper async/await patterns
- Include timeout and cleanup mechanisms

#### State Management

- Always use proper locking patterns
- Include fallback error handling
- Reset states after operations

### 7. Testing Requirements

**RULE**: All event listener changes must include:

1. **Unit tests** for individual event handlers
2. **Integration tests** for event sequences
3. **Race condition tests** for concurrent events
4. **Crash recovery tests** for stuck states

### 8. Emergency Procedures

**RULE**: If duplicate listeners are detected:

1. **Immediate action**: Remove duplicate listeners
2. **Root cause analysis**: Determine how duplicates were introduced
3. **Testing**: Verify fix resolves race conditions
4. **Documentation**: Update this rules document

### 9. Monitoring

**RULE**: Include runtime duplicate detection:

```rust
// TODO: Add runtime listener tracking
static REGISTERED_LISTENERS: Lazy<Mutex<HashSet<String>>> = Lazy::new(|| {
    Mutex::new(HashSet::new())
});

fn register_unique_listener(event_name: &str) -> Result<(), String> {
    let mut listeners = REGISTERED_LISTENERS.lock().unwrap();
    if listeners.contains(event_name) {
        return Err(format!("Duplicate listener detected for event: {}", event_name));
    }
    listeners.insert(event_name.to_string());
    Ok(())
}
```

### 10. Documentation Requirements

**RULE**: Every event listener must have:

- **Purpose**: What does this listener handle?
- **Mode dependencies**: Does it depend on app state?
- **Error handling**: How does it handle failures?
- **Cleanup**: How does it reset state?

## Implementation Status

### ✅ Fixed Issues

- Removed duplicate `voice-transcription:final-result` listener
- Verified all current listeners are unique
- Compilation verified successful

### 🔄 Next Steps

1. Implement runtime duplicate detection
2. Add unit tests for all event handlers
3. Create integration tests for event sequences
4. Add monitoring for race conditions

## Violations and Enforcement

### Automatic Detection

```bash
# Add to CI/CD pipeline
#!/bin/bash
echo "Checking for duplicate event listeners..."
DUPLICATES=$(grep -n 'app\.listen("' src-tauri/src/lib.rs | cut -d'"' -f2 | sort | uniq -c | sort -nr | awk '$1 > 1')
if [ ! -z "$DUPLICATES" ]; then
    echo "ERROR: Duplicate event listeners detected:"
    echo "$DUPLICATES"
    exit 1
fi
echo "✅ No duplicate event listeners found"
```

### Manual Review Process

1. **Pre-commit**: Developer runs duplicate detection
2. **PR Review**: Reviewer verifies checklist completion
3. **CI/CD**: Automated duplicate detection in pipeline
4. **Post-merge**: Monitor for runtime issues

## Contact and Updates

- **Owner**: Development Team
- **Last Updated**: 2024-12-13
- **Next Review**: When adding new event listeners

This document must be updated whenever new event types are added or listener patterns change.
