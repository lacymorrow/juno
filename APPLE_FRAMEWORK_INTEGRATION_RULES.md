# Apple Framework Integration Rules for Rust Projects

## 📋 Core Principles

### **Rule 1: Safety First**
- **NEVER use manual FFI when safe alternatives exist**
- **ALWAYS prefer Cidre over manual `objc` crate usage**
- **ELIMINATE all `unsafe` blocks for Apple framework interactions**
- **VALIDATE all pointer operations with proper error handling**

### **Rule 2: Cross-Platform Compatibility**
- **ALWAYS provide fallback implementations for non-macOS targets**
- **USE conditional compilation for platform-specific code**
- **MAINTAIN identical public APIs across all platforms**
- **TEST on all target platforms regularly**

### **Rule 3: Feature Flag Management**
- **IMPLEMENT feature flags for optional dependencies**
- **PROVIDE both safe and fallback implementations**
- **DOCUMENT all feature combinations clearly**
- **ENSURE graceful degradation when features are disabled**

## 🏗 **Architecture Patterns**

### **Pattern 1: Conditional Implementation Strategy**

**Template**:
```rust
// Primary implementation (Cidre - safest)
#[cfg(all(target_os = "macos", feature = "use-cidre"))]
pub fn apple_framework_function() -> Result<T, Error> {
    // Use Cidre safe APIs
    cidre::framework::safe_operation()
        .ok_or_else(|| Error::PlatformError("Operation failed".to_string()))
}

// Fallback implementation (core-foundation - safer than manual FFI)
#[cfg(all(target_os = "macos", not(feature = "use-cidre")))]
pub fn apple_framework_function() -> Result<T, Error> {
    // Use core-foundation crate for safety
    use core_foundation::*;
    // Minimize unsafe usage, prefer safe wrappers
}

// Cross-platform stub (non-macOS)
#[cfg(not(target_os = "macos"))]
pub fn apple_framework_function() -> Result<T, Error> {
    Err(Error::PlatformError("Feature only available on macOS".to_string()))
}
```

### **Pattern 2: Error Handling Strategy**

**Template**:
```rust
// Define platform-specific error types
#[derive(Debug, thiserror::Error)]
pub enum AppleFrameworkError {
    #[error("Apple framework operation failed: {0}")]
    OperationFailed(String),
    
    #[error("Permission denied: {0}")]
    PermissionDenied(String),
    
    #[error("Feature not available on this platform")]
    PlatformUnsupported,
    
    #[error("Required framework feature disabled")]
    FeatureDisabled,
}

// Convert Cidre errors to structured errors
impl From<cidre::Error> for AppleFrameworkError {
    fn from(err: cidre::Error) -> Self {
        AppleFrameworkError::OperationFailed(format!("Cidre error: {:?}", err))
    }
}
```

### **Pattern 3: Memory Management Strategy**

**Rules**:
- **NEVER manually manage CFType reference counting**
- **USE Cidre's automatic memory management**
- **AVOID `CFType::wrap_under_create_rule` unless absolutely necessary**
- **PREFER stack-allocated structures when possible**

**Example**:
```rust
// ❌ WRONG - Manual memory management
unsafe {
    let value_ref = AXValueCreate(type_id, ptr);
    let cf_value = CFType::wrap_under_create_rule(value_ref);
    // Risk of leaks and use-after-free
}

// ✅ CORRECT - Automatic memory management
let point = cidre::cf::Point::new(x, y);
let ax_value = cidre::ax::Value::from_point(&point)?;
// Memory automatically managed
```

## 🔧 **Implementation Rules**

### **Rule 4: Dependency Management**

**Cargo.toml Structure**:
```toml
[features]
default = []
use-cidre = []

# Platform-specific dependencies
[target.'cfg(target_os = "macos")'.dependencies]
core-foundation = "0.10.0"
core-graphics = "0.24.0"
accessibility = "0.2.0"

# Conditional Cidre dependency
[target.'cfg(all(target_os = "macos", feature = "use-cidre"))'.dependencies]
cidre = "0.1.0"

# Cross-platform dependencies
[dependencies]
thiserror = "1.0"
tracing = "0.1"
```

### **Rule 5: API Design Consistency**

**Function Signature Rules**:
```rust
// ✅ CORRECT - Consistent across platforms
pub fn check_accessibility_permissions(show_prompt: bool) -> Result<bool, Error>;

// ❌ WRONG - Platform-specific signatures
#[cfg(target_os = "macos")]
pub fn check_accessibility_permissions_macos(show_prompt: bool, options: MacOSOptions) -> Result<bool, Error>;
```

**Return Type Rules**:
- **USE `Result<T, Error>` for all fallible operations**
- **RETURN the same types across all platform implementations**
- **WRAP platform-specific types in generic abstractions**

### **Rule 6: Testing Strategy**

**Test Structure**:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(target_os = "macos")]
    fn test_macos_functionality() {
        // Test actual macOS implementation
        let result = apple_framework_function().unwrap();
        assert!(!result.is_empty());
    }

    #[test]
    #[cfg(feature = "use-cidre")]
    fn test_cidre_implementation() {
        // Test Cidre-specific features
    }

    #[test]
    #[cfg(not(target_os = "macos"))]
    fn test_cross_platform_fallback() {
        // Ensure fallbacks return appropriate errors
        let result = apple_framework_function();
        assert!(result.is_err());
    }

    #[test]
    fn test_api_consistency() {
        // Test that API works the same across platforms
        // (with different expected behaviors)
    }
}
```

## 📚 **Apple Framework Specific Rules**

### **Accessibility Framework (AX)**

**Rules**:
- **ALWAYS check permissions before performing accessibility operations**
- **USE Cidre's `ax::is_process_trusted_with_options` instead of manual FFI**
- **HANDLE permission prompts gracefully**
- **PROVIDE user-friendly error messages for permission issues**

**Example**:
```rust
// ✅ CORRECT
fn perform_accessibility_action() -> Result<(), Error> {
    if !cidre::ax::is_process_trusted_with_options(&Default::default()) {
        return Err(Error::PermissionDenied(
            "Accessibility permissions required. Please grant access in System Settings.".to_string()
        ));
    }
    // Perform action...
}
```

### **NSWorkspace Integration**

**Rules**:
- **USE Cidre's `ns::Workspace::shared()` instead of manual `msg_send!`**
- **FILTER background applications appropriately**
- **HANDLE application enumeration errors gracefully**
- **RESPECT user privacy by only accessing necessary application information**

**Example**:
```rust
// ✅ CORRECT
fn get_running_applications() -> Result<Vec<AppInfo>, Error> {
    let workspace = cidre::ns::Workspace::shared();
    let apps = workspace.running_applications();
    
    let mut result = Vec::new();
    for app in apps.iter() {
        if let (Some(name), Some(bundle_id)) = (app.localized_name(), app.bundle_identifier()) {
            result.push(AppInfo {
                name: name.to_string(),
                bundle_id: bundle_id.to_string(),
                pid: app.process_identifier(),
            });
        }
    }
    Ok(result)
}
```

### **Core Graphics Integration**

**Rules**:
- **USE Cidre's `cg::Display` and `cg::Event` instead of manual Core Graphics FFI**
- **HANDLE display enumeration and bounds checking safely**
- **VALIDATE coordinates and dimensions before use**
- **MANAGE screenshot capture memory efficiently**

**Example**:
```rust
// ✅ CORRECT
fn capture_display_screenshot(display_id: Option<u32>) -> Result<Vec<u8>, Error> {
    let display = match display_id {
        Some(id) => cidre::cg::Display::from_id(id),
        None => cidre::cg::Display::main(),
    };
    
    let bounds = display.bounds();
    let image = display.create_image(&bounds)
        .ok_or_else(|| Error::OperationFailed("Failed to capture screenshot".to_string()))?;
    
    // Process image data safely...
    Ok(image_data)
}
```

## 🚨 **Security Rules**

### **Rule 7: Permission Handling**

- **NEVER bypass system permission checks**
- **ALWAYS inform users why permissions are needed**
- **PROVIDE clear instructions for granting permissions**
- **HANDLE permission denial gracefully**

### **Rule 8: Data Access**

- **ONLY access data necessary for functionality**
- **RESPECT user privacy settings**
- **AVOID caching sensitive information unnecessarily**
- **FOLLOW Apple's privacy guidelines**

### **Rule 9: Code Injection Prevention**

- **NEVER execute dynamic code from untrusted sources**
- **VALIDATE all external inputs thoroughly**
- **USE safe string handling for all operations**
- **AVOID shell command injection**

## 🔄 **Migration Rules**

### **Rule 10: Legacy Code Migration**

**Migration Checklist**:
- [ ] Identify all `unsafe` blocks related to Apple frameworks
- [ ] Replace manual FFI with Cidre equivalents
- [ ] Add conditional compilation directives
- [ ] Implement fallback mechanisms
- [ ] Add comprehensive error handling
- [ ] Write tests for all code paths
- [ ] Update documentation

**Migration Strategy**:
```rust
// Phase 1: Add Cidre implementation alongside existing code
#[cfg(all(target_os = "macos", feature = "use-cidre"))]
fn new_safe_implementation() -> Result<T, Error> { /* ... */ }

#[cfg(all(target_os = "macos", not(feature = "use-cidre")))]
fn legacy_implementation() -> Result<T, Error> { /* existing code */ }

// Phase 2: Test extensively with feature flags
// Phase 3: Remove legacy implementation when confident
```

### **Rule 11: Backwards Compatibility**

- **MAINTAIN existing public APIs during migration**
- **USE feature flags to enable gradual rollout**
- **PROVIDE migration guides for breaking changes**
- **SUPPORT multiple implementation strategies simultaneously**

## 📖 **Documentation Rules**

### **Rule 12: Code Documentation**

**Function Documentation Template**:
```rust
/// Brief description of what the function does
/// 
/// # Platform Support
/// - macOS: Full support via Cidre (with `use-cidre` feature) or core-foundation fallback
/// - Other platforms: Returns `PlatformUnsupported` error
/// 
/// # Permissions Required
/// - Accessibility permissions (for AX operations)
/// - Screen recording permissions (for screenshot capture)
/// 
/// # Examples
/// ```rust
/// let result = apple_framework_function()?;
/// ```
/// 
/// # Errors
/// - `PermissionDenied`: Required permissions not granted
/// - `OperationFailed`: Apple framework operation failed
/// - `PlatformUnsupported`: Feature not available on this platform
pub fn apple_framework_function() -> Result<T, Error> {
    // Implementation...
}
```

### **Rule 13: Feature Documentation**

- **DOCUMENT all feature flags and their effects**
- **PROVIDE examples for each supported configuration**
- **EXPLAIN platform-specific behaviors clearly**
- **INCLUDE troubleshooting guides for common issues**

## ⚡ **Performance Rules**

### **Rule 14: Optimization Guidelines**

- **MINIMIZE Apple framework calls in tight loops**
- **CACHE expensive operations when appropriate**
- **USE async operations for potentially blocking calls**
- **PROFILE actual performance impact of Cidre vs manual FFI**

### **Rule 15: Resource Management**

- **RELEASE resources promptly**
- **AVOID holding references to large objects unnecessarily**
- **USE appropriate data structures for the use case**
- **MONITOR memory usage in long-running applications**

## 🔍 **Debugging Rules**

### **Rule 16: Logging Strategy**

```rust
// Use structured logging for Apple framework operations
tracing::debug!(
    operation = "accessibility_check",
    show_prompt = %show_prompt,
    "Checking accessibility permissions"
);

// Log errors with context
tracing::error!(
    error = %err,
    operation = "screenshot_capture",
    display_id = display_id,
    "Failed to capture screenshot"
);
```

### **Rule 17: Error Context**

- **PROVIDE sufficient context in error messages**
- **INCLUDE relevant system state information**
- **SUGGEST concrete remediation steps**
- **AVOID exposing internal implementation details to end users**

## 📋 **Checklist for New Apple Framework Integration**

- [ ] **Safety**: No unsafe blocks, use Cidre when available
- [ ] **Compatibility**: Works on all target platforms with appropriate fallbacks
- [ ] **Features**: Conditional compilation with feature flags
- [ ] **Errors**: Structured error handling with descriptive messages
- [ ] **Memory**: Automatic memory management, no manual CFType handling
- [ ] **Permissions**: Proper permission checking and user guidance
- [ ] **Testing**: Comprehensive tests for all platforms and feature combinations
- [ ] **Documentation**: Clear documentation with examples and platform notes
- [ ] **Performance**: Efficient implementation without unnecessary overhead
- [ ] **Security**: Respects user privacy and system security policies

## 🎯 **Success Metrics**

- **Zero unsafe blocks** for Apple framework interactions
- **100% test coverage** across all platform/feature combinations
- **Consistent API** across all platforms
- **Clear error messages** with actionable guidance
- **Performance parity** with manual implementations
- **Comprehensive documentation** for all features

---

**These rules ensure safe, maintainable, and cross-platform Apple framework integration in Rust projects while leveraging the safety and ergonomics of the Cidre crate.**