# Error Handling Best Practices for Juno

## Problem: String-Based Error Detection Anti-Pattern

The current codebase contains multiple instances of fragile string-based error detection:

```rust
// ❌ ANTI-PATTERN: Fragile string matching
if error_message.to_lowercase().contains("network") || 
   error_message.to_lowercase().contains("connection") {
    return ErrorClass::NetworkConnectivity;
}
```

### Issues with String-Based Detection

1. **Fragile**: Error messages change across library versions
2. **Localization**: Messages may be in different languages
3. **Incomplete**: New error types won't be caught
4. **Brittle**: Formatting changes break detection
5. **Unmaintainable**: Hard to test and update
6. **Performance**: String operations are slower than type matching

## Solution: Structured Error Handling

### 1. Use Structured Error Types

```rust
// ✅ PREFERRED: Structured error types
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NetworkError {
    ConnectionTimeout,
    ConnectionRefused,
    DnsResolution,
    TlsHandshake,
    RequestTimeout,
    Unreachable,
    Unknown(String),
}

impl From<reqwest::Error> for NetworkError {
    fn from(error: reqwest::Error) -> Self {
        if error.is_timeout() {
            NetworkError::RequestTimeout
        } else if error.is_connect() {
            NetworkError::ConnectionRefused
        } else if error.is_dns_resolution() {
            NetworkError::DnsResolution
        } else {
            NetworkError::Unknown(error.to_string())
        }
    }
}
```

### 2. Error Classification Trait

```rust
// ✅ PREFERRED: Trait-based classification
pub trait ErrorClassifiable {
    fn classify(&self) -> ErrorClass;
    fn is_recoverable(&self) -> bool;
    fn recovery_strategy(&self) -> RecoveryStrategy;
}

impl ErrorClassifiable for NetworkError {
    fn classify(&self) -> ErrorClass {
        match self {
            NetworkError::ConnectionTimeout | NetworkError::RequestTimeout => ErrorClass::Timeout,
            NetworkError::ConnectionRefused | NetworkError::Unreachable => ErrorClass::NetworkConnectivity,
            NetworkError::DnsResolution => ErrorClass::DnsError,
            NetworkError::TlsHandshake => ErrorClass::SecurityError,
            NetworkError::Unknown(_) => ErrorClass::Unknown,
        }
    }
    
    fn is_recoverable(&self) -> bool {
        !matches!(self, NetworkError::TlsHandshake)
    }
}
```

### 3. Error Source Chain Analysis

```rust
// ✅ PREFERRED: Analyze error source chain
pub fn classify_error_by_source(error: &dyn std::error::Error) -> ErrorClass {
    let mut current = Some(error);
    
    while let Some(err) = current {
        // Check specific error types in the chain
        if let Some(io_err) = err.downcast_ref::<std::io::Error>() {
            return classify_io_error(io_err);
        }
        
        if let Some(reqwest_err) = err.downcast_ref::<reqwest::Error>() {
            return classify_reqwest_error(reqwest_err);
        }
        
        if let Some(tungstenite_err) = err.downcast_ref::<tokio_tungstenite::tungstenite::Error>() {
            return classify_websocket_error(tungstenite_err);
        }
        
        current = err.source();
    }
    
    ErrorClass::Unknown
}

fn classify_io_error(error: &std::io::Error) -> ErrorClass {
    match error.kind() {
        std::io::ErrorKind::TimedOut => ErrorClass::Timeout,
        std::io::ErrorKind::ConnectionRefused => ErrorClass::NetworkConnectivity,
        std::io::ErrorKind::ConnectionReset => ErrorClass::NetworkConnectivity,
        std::io::ErrorKind::PermissionDenied => ErrorClass::PermissionDenied,
        std::io::ErrorKind::NotFound => ErrorClass::ResourceNotFound,
        _ => ErrorClass::Unknown,
    }
}
```

### 4. Error Context Preservation

```rust
// ✅ PREFERRED: Rich error context
#[derive(Debug, thiserror::Error)]
pub enum JunoError {
    #[error("Network operation failed")]
    Network(#[from] NetworkError),
    
    #[error("Tool execution failed: {tool_name}")]
    ToolExecution {
        tool_name: String,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },
    
    #[error("Permission denied for operation: {operation}")]
    PermissionDenied { operation: String },
    
    #[error("Configuration error: {details}")]
    Configuration { details: String },
}
```

## Migration Strategy

### Phase 1: Create Error Type Hierarchy

1. Define structured error enums for each domain (Network, Agent, Tool, etc.)
2. Implement `From` traits for library errors
3. Add error classification traits

### Phase 2: Replace String Detection

1. Update `utils/network.rs` to use structured errors
2. Replace `is_network_error()` with trait-based classification
3. Update error recovery to use error types instead of strings

### Phase 3: Enhanced Error Context

1. Add error context preservation throughout the call stack
2. Implement proper error chaining
3. Add structured logging for errors

### Phase 4: Testing and Validation

1. Add comprehensive error handling tests
2. Test error classification accuracy
3. Validate recovery strategies

## Implementation Guidelines

### DO ✅

- Use structured error types with enums
- Implement `std::error::Error` trait
- Use `thiserror` for error boilerplate
- Analyze error source chains
- Preserve error context
- Use trait-based classification
- Test error handling paths

### DON'T ❌

- Use string matching for error detection
- Ignore error source chains
- Lose error context information
- Use generic "unknown error" everywhere
- Skip error handling tests
- Mix error types without proper conversion

## Files Requiring Refactoring

Current files with string-based error detection:

- `src-tauri/src/utils/network.rs` - `is_network_error()`
- `src-tauri/src/agent/error_recovery.rs` - `determine_error_pattern()`
- `src-tauri/src/agent/implementations/tool_provider.rs` - `classify_error()`
- `src-tauri/src/startup.rs` - Permission error detection
- `src-tauri/src/anthropic.rs` - Network error detection
- `src-tauri/src/commands/notifications.rs` - Permission string matching

## Example Migration

### Before (String-Based)

```rust
fn is_network_error(error_msg: &str) -> bool {
    let error_lower = error_msg.to_lowercase();
    error_lower.contains("network") || 
    error_lower.contains("connection") || 
    error_lower.contains("timeout")
}
```

### After (Structured)

```rust
fn classify_error(error: &dyn std::error::Error) -> ErrorClass {
    classify_error_by_source(error)
}

impl ErrorClassifiable for reqwest::Error {
    fn classify(&self) -> ErrorClass {
        if self.is_timeout() { ErrorClass::Timeout }
        else if self.is_connect() { ErrorClass::NetworkConnectivity }
        else { ErrorClass::Unknown }
    }
}
```

This structured approach provides:

- Type safety at compile time
- Better error handling and recovery
- Maintainable and testable code
- Performance improvements
- Future-proof error classification
