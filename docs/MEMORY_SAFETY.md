# Memory Safety Guidelines for Juno

## Overview

This document outlines critical memory safety practices for the Juno codebase. Following these guidelines prevents panics, race conditions, and resource leaks in production.

## Critical Rule: NO `.unwrap()` in Production Code

**The use of `.unwrap()` is BANNED in all production code paths.**

### Why This Matters

- `.unwrap()` causes immediate panic on `None` or `Err` values
- Panics crash the application without cleanup
- User experience is severely impacted
- Debug information may leak in panic messages

## Safe Error Handling Patterns

### Option Handling

```rust
// ❌ NEVER DO THIS
let value = some_option.unwrap();
let value = some_option.expect("This will panic");

// ✅ SAFE PATTERNS
// With error propagation
let value = some_option.ok_or("Value not found")?;

// With default value
let value = some_option.unwrap_or_default();
let value = some_option.unwrap_or(42);
let value = some_option.unwrap_or_else(|| expensive_computation());

// With conditional logic
if let Some(value) = some_option {
    // Use value
}

match some_option {
    Some(value) => // Use value,
    None => // Handle absence
}
```

### Result Handling

```rust
// ❌ NEVER DO THIS
let result = operation().unwrap();

// ✅ SAFE PATTERNS
// With error propagation
let result = operation()?;
let result = operation().map_err(|e| format!("Operation failed: {}", e))?;

// With fallback
let result = operation().unwrap_or_else(|e| {
    tracing::error!("Operation failed: {}", e);
    default_value
});

// With proper matching
match operation() {
    Ok(value) => // Use value,
    Err(e) => {
        tracing::error!("Operation failed: {}", e);
        return Err(format!("Failed to complete operation: {}", e));
    }
}
```

## Common Patterns

### SystemTime Operations

SystemTime can fail if system clock is adjusted. Always handle this:

```rust
// ❌ DANGEROUS
let timestamp = SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .unwrap()
    .as_secs();

// ✅ SAFE
let timestamp = SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .unwrap_or_else(|_| Duration::from_secs(0))
    .as_secs();
```

### Mutex Locking

Mutexes can be poisoned if a thread panics while holding the lock:

```rust
// ❌ DANGEROUS
let guard = mutex.lock().unwrap();

// ✅ SAFE
match mutex.lock() {
    Ok(guard) => {
        // Use guard
    }
    Err(e) => {
        tracing::error!("Mutex poisoned: {}", e);
        // Either recover the data or return error
        return Err("Failed to acquire lock".to_string());
    }
}

// ✅ ALTERNATIVE: Accept poisoned mutex
let guard = mutex.lock().unwrap_or_else(|e| {
    tracing::warn!("Recovering from poisoned mutex: {}", e);
    e.into_inner()
});
```

### Regex Compilation

Regex patterns can be invalid:

```rust
// ❌ DANGEROUS
let regex = Regex::new(pattern).unwrap();

// ✅ SAFE
let regex = match Regex::new(pattern) {
    Ok(r) => r,
    Err(e) => {
        tracing::warn!("Invalid regex pattern '{}': {}", pattern, e);
        // Use fallback logic or return error
        return Err(format!("Invalid pattern: {}", e));
    }
};

// ✅ WITH LAZY STATIC (for compile-time patterns)
use once_cell::sync::Lazy;

static PATTERN: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^\d+$").expect("Regex is valid at compile time")
});
```

### Array/Vector Access

```rust
// ❌ DANGEROUS
let value = vec[index];
let first = vec.first().unwrap();

// ✅ SAFE
let value = vec.get(index).ok_or("Index out of bounds")?;
let first = vec.first().ok_or("Vector is empty")?;

// ✅ WITH DEFAULT
let value = vec.get(index).copied().unwrap_or(default_value);
```

## Anti-Patterns to Avoid

### Checking then Unwrapping

```rust
// ❌ STILL DANGEROUS
if option.is_some() {
    let value = option.unwrap(); // Can still panic if modified between check and unwrap
}

// ✅ SAFE
if let Some(value) = option {
    // Use value
}
```

### Multiple Unwraps in Chain

```rust
// ❌ DANGEROUS
let result = map.get("key").unwrap().field.unwrap();

// ✅ SAFE
let result = map.get("key")
    .and_then(|v| v.field.as_ref())
    .ok_or("Value not found")?;
```

## Race Condition Prevention

### Shared State Management

```rust
// Use Arc for shared ownership
use std::sync::Arc;
use tokio::sync::Mutex as TokioMutex;

// ✅ Thread-safe shared state
pub struct AppState {
    data: Arc<TokioMutex<SharedData>>,
}

// ✅ Clone is cheap (just increments reference count)
let state_clone = app_state.data.clone();
```

### Atomic Operations

```rust
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

// ✅ Lock-free flag
let running = AtomicBool::new(true);
running.store(false, Ordering::SeqCst);

// ✅ Lock-free counter
let counter = AtomicUsize::new(0);
counter.fetch_add(1, Ordering::SeqCst);
```

### Resource Management (RAII)

```rust
// ✅ Automatic cleanup on drop
pub struct ManagedResource<T> {
    resource: Option<T>,
    cleanup: Option<Box<dyn FnOnce(T) + Send + 'static>>,
}

impl<T> Drop for ManagedResource<T> {
    fn drop(&mut self) {
        if let (Some(resource), Some(cleanup)) = 
            (self.resource.take(), self.cleanup.take()) {
            cleanup(resource);
        }
    }
}
```

## Production vs Test Code

### Test Code Exception

`.unwrap()` is acceptable in test code where panics are expected:

```rust
#[test]
fn test_something() {
    // ✅ OK in tests - panic will fail the test
    let result = operation().unwrap();
    assert_eq!(result, expected);
    
    // ✅ BETTER - provides context on failure
    let result = operation().expect("Operation should succeed in test");
}
```

### Production Guards

```rust
// Ensure unwrap is never used in production
#[cfg(not(test))]
compile_error!("unwrap() is not allowed in production code");
```

## Verification

### Finding Unwraps

```bash
# Find all unwraps in production code
rg "\.unwrap\(\)" src-tauri/src --type rust | grep -v test | grep -v "cfg(test)"

# Count unwraps
rg "\.unwrap\(\)" src-tauri/src --type rust | grep -v test | wc -l
```

### Automated Checks

Consider adding a CI check:

```yaml
- name: Check for unwraps in production
  run: |
    count=$(rg "\.unwrap\(\)" src-tauri/src --type rust | grep -v test | wc -l)
    if [ $count -gt 0 ]; then
      echo "Found $count unwrap() calls in production code!"
      exit 1
    fi
```

## Migration Guide

When fixing existing unwraps:

1. **Understand the context** - Why might this fail?
2. **Choose appropriate handling** - Error propagation, default, or recovery?
3. **Add logging** - Help diagnose issues in production
4. **Test error paths** - Ensure graceful handling

### Example Migration

```rust
// Before
let config = load_config().unwrap();

// After - Option 1: Propagate error
let config = load_config()
    .map_err(|e| format!("Failed to load config: {}", e))?;

// After - Option 2: Use defaults
let config = load_config().unwrap_or_else(|e| {
    tracing::warn!("Using default config due to error: {}", e);
    Config::default()
});

// After - Option 3: Exit gracefully
let config = match load_config() {
    Ok(c) => c,
    Err(e) => {
        tracing::error!("Fatal: Cannot load config: {}", e);
        // Notify user through UI
        app_handle.emit("fatal-error", "Configuration error")?;
        return Err("Configuration error".into());
    }
};
```

## Summary

- **Never use `.unwrap()`** in production code
- **Always handle errors** explicitly
- **Use appropriate patterns** for each situation
- **Test error paths** not just happy paths
- **Log failures** for debugging
- **Fail gracefully** with user-friendly errors

Following these guidelines ensures Juno remains stable and reliable in production environments.