# Cidre Migration Patterns - Cursor Rules

## Integration with Juno AI Computer Use Agent

This rule set extends the main Juno project rules with specific patterns for safe Apple framework integration using Cidre.

## Rule: Apple Framework Safety Patterns

### MANDATORY for all Apple framework code in src-tauri/mcp-server-os-level/src/platforms/macos/

```rust
// ✅ REQUIRED PATTERN - Conditional Implementation
#[cfg(all(target_os = "macos", feature = "use-cidre"))]
pub(crate) fn apple_operation() -> Result<T, AutomationError> {
    // Use Cidre safe APIs only
    use cidre::{ax, ns, cg, cf};
    
    let result = cidre_framework::safe_operation()
        .ok_or_else(|| AutomationError::PlatformError("Descriptive error".to_string()))?;
    
    Ok(result)
}

#[cfg(all(target_os = "macos", not(feature = "use-cidre")))]
pub(crate) fn apple_operation() -> Result<T, AutomationError> {
    // Fallback using core-foundation (safer than manual FFI)
    use core_foundation::*;
    // Minimize unsafe usage
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn apple_operation() -> Result<T, AutomationError> {
    Err(AutomationError::PlatformError("macOS only feature".to_string()))
}
```

### FORBIDDEN PATTERNS

```rust
// ❌ NEVER DO THIS - Manual FFI
extern "C" {
    fn AXSomeFunction(ptr: *const c_void) -> bool;
}

// ❌ NEVER DO THIS - Manual msg_send!
unsafe {
    let result: bool = msg_send![obj, selector];
}

// ❌ NEVER DO THIS - Manual memory management
let cf_value = CFType::wrap_under_create_rule(raw_ptr);
```

## Rule: Integration with Existing Juno Architecture

### Must use existing AutomationError types
```rust
// ✅ CORRECT - Use existing error types
use crate::AutomationError;

fn cidre_function() -> Result<T, AutomationError> {
    cidre_operation()
        .ok_or_else(|| AutomationError::PlatformError("Clear message".to_string()))
}

// ❌ WRONG - New error types
enum CidreError { /* ... */ }
```

### Must integrate with existing logging
```rust
// ✅ CORRECT - Use existing tracing patterns
use tracing::{debug, warn, error};

debug!("Performing Cidre operation with safe bindings");
if let Err(e) = cidre_operation() {
    error!(error = %e, "Cidre operation failed");
}
```

### Must follow existing module patterns
```rust
// File: src-tauri/mcp-server-os-level/src/platforms/macos/some_feature.rs

// ✅ CORRECT - Follow existing patterns
use super::ffi::ax_is_process_trusted_with_options; // Use our safe wrappers
use crate::element::UIElementImpl;
use crate::AutomationError;

// Import Cidre conditionally
#[cfg(all(target_os = "macos", feature = "use-cidre"))]
use cidre::{ax, ns, cg};
```

## Rule: Testing Patterns for Cidre Code

### MANDATORY test structure
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(target_os = "macos")]
    fn test_with_cidre_feature() {
        // Test actual functionality on macOS
    }

    #[test]
    #[cfg(feature = "use-cidre")]
    fn test_cidre_specific_behavior() {
        // Test Cidre-specific paths
    }

    #[test]
    #[cfg(not(target_os = "macos"))]
    fn test_cross_platform_fallback() {
        let result = function_under_test();
        assert!(result.is_err()); // Should fail gracefully
    }
}
```

## Rule: Cargo.toml Integration

### REQUIRED dependency structure
```toml
# Add to existing features section
[features]
use-cidre = []

# Add to existing macOS dependencies
[target.'cfg(target_os = "macos")'.dependencies]
# Note: Cidre commented out until ready for macOS deployment
# cidre = "0.1.0"  # Uncomment when building on macOS
```

## Rule: Integration with Computer Use Actions

### Must maintain existing Computer Use API compatibility
```rust
// ✅ CORRECT - Maintain existing signatures
impl MacOSEngine {
    fn screenshot(&self) -> Result<String, AutomationError> {
        // Use new Cidre implementation internally
        capture_and_encode_screenshot_cidre()
    }
    
    fn click(&self, x: f64, y: f64) -> Result<(), AutomationError> {
        // Use new Cidre mouse implementation internally
        left_click_cidre(x, y)
    }
}

// ❌ WRONG - Changing existing APIs
fn screenshot_with_cidre_options(&self, cidre_opts: CidreOptions) -> Result<String, AutomationError>
```

## Rule: Integration with Agent System

### Must work with existing orchestrator
```rust
// ✅ CORRECT - Agent calls work transparently
// The agent system should not know about Cidre vs manual implementation
// All improvements are internal to the platform layer

pub(crate) fn execute_computer_use_action(action: ComputerUseAction) -> Result<String, AutomationError> {
    match action {
        ComputerUseAction::Screenshot => {
            // Internally uses Cidre when available, fallback otherwise
            capture_and_encode_screenshot()
        }
        // Other actions work the same way
    }
}
```

## Rule: Permission Integration

### Must use existing permission system
```rust
// ✅ CORRECT - Integrate with existing permission checks
use super::permissions::{check_accessibility_permissions, check_accessibility_permissions_with_auto_redirect};

pub(crate) fn cidre_accessibility_operation() -> Result<(), AutomationError> {
    // Use existing permission checking
    check_accessibility_permissions(false)?;
    
    // Then use Cidre for the actual operation
    #[cfg(feature = "use-cidre")]
    {
        use cidre::ax;
        ax::perform_operation()
            .ok_or_else(|| AutomationError::PlatformError("AX operation failed".to_string()))
    }
    
    #[cfg(not(feature = "use-cidre"))]
    {
        // Fallback implementation
        fallback_ax_operation()
    }
}
```

## Rule: Development Workflow

### Local development (Linux/Windows)
```bash
# ✅ CORRECT - Works without Cidre
cargo check
cargo test
```

### macOS development
```bash
# ✅ CORRECT - Enable Cidre when available
cargo check --features use-cidre
cargo test --features use-cidre
```

### Production deployment
```bash
# ✅ CORRECT - Use Cidre in production on macOS
cargo build --release --features use-cidre
```

## Rule: Code Review Requirements

### MANDATORY checks for Apple framework PRs:
- [ ] No unsafe blocks for Apple framework interactions
- [ ] Conditional compilation for all platform-specific code  
- [ ] Fallback implementations for non-macOS platforms
- [ ] Feature flag integration (`use-cidre`)
- [ ] Tests for all code paths
- [ ] Integration with existing AutomationError types
- [ ] Maintains existing public API compatibility
- [ ] Uses existing logging patterns
- [ ] No manual memory management for CF types

### Integration with existing Juno rules:
- [ ] MUST compile with `cargo check --manifest-path src-tauri/Cargo.toml`
- [ ] MUST follow existing error handling patterns
- [ ] MUST integrate with existing Agent architecture
- [ ] MUST work with existing Computer Use action system
- [ ] MUST maintain existing permission checking flow

## Quick Reference Commands

```bash
# Check compilation (works on all platforms)
cargo check --manifest-path src-tauri/mcp-server-os-level/Cargo.toml

# Test with Cidre feature (macOS only)
cargo test --manifest-path src-tauri/mcp-server-os-level/Cargo.toml --features use-cidre

# Integration test with main Juno system
cargo check --manifest-path src-tauri/Cargo.toml
```

## File-Specific Rules

### src-tauri/mcp-server-os-level/src/platforms/macos/ffi.rs
- Contains ONLY safe wrapper functions
- All manual FFI replaced with Cidre or safe core-foundation
- Conditional compilation for feature flags

### src-tauri/mcp-server-os-level/src/platforms/macos/utils.rs  
- NSWorkspace operations use Cidre ns::Workspace
- Core Graphics operations use Cidre cg:: module
- Screenshot capture fully migrated to safe APIs

### src-tauri/mcp-server-os-level/src/platforms/macos/permissions.rs
- Uses safe wrapper functions from ffi.rs
- Maintains existing permission checking logic
- Integrates with existing error handling

### src-tauri/mcp-server-os-level/src/platforms/macos/engine.rs
- Application activation uses safe Cidre APIs
- Window manipulation uses safe AXValue creation
- Maintains all existing Computer Use compatibility

---

**These rules ensure the Cidre migration integrates seamlessly with the existing Juno AI Computer Use Agent architecture while providing the safety and maintainability benefits of safe Apple framework bindings.**