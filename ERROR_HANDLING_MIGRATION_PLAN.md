# Error Handling Migration Plan - Juno Project

## Overview

This document tracks the migration from fragile string-based error detection to structured error handling patterns throughout the Juno codebase.

## Current Status

**Detection Script**: ✅ Created `scripts/detect-string-error-patterns.sh`
**Documentation**: ✅ Created `docs/rules/ERROR_HANDLING_BEST_PRACTICES.md`
**Rules Integration**: ✅ Added to `LLMs.txt` main instructions

## Files Requiring Migration

### 🔴 HIGH PRIORITY - Critical System Functions

#### 1. `src-tauri/src/utils/network.rs`

- **Function**: `is_network_error(error_msg: &str) -> bool`
- **Issue**: String matching for 11+ error patterns
- **Impact**: Network connectivity detection throughout app
- **Dependencies**: Used in `anthropic.rs`, `cloud/connector.rs`
- **Status**: 🔴 NOT STARTED

#### 2. `src-tauri/src/agent/error_recovery.rs`

- **Function**: `determine_error_pattern(&self, error: &AgentError) -> ErrorPattern`
- **Issue**: String matching for 9+ error patterns
- **Impact**: Agent recovery strategies and error handling
- **Dependencies**: Core agent functionality
- **Status**: 🔴 NOT STARTED

#### 3. `src-tauri/src/agent/implementations/tool_provider.rs`

- **Function**: `classify_error(&self, error_msg: &str) -> ErrorClass`
- **Issue**: String matching for 7+ error patterns
- **Impact**: Tool execution error classification
- **Dependencies**: All agent tool operations
- **Status**: 🔴 NOT STARTED

### 🟡 MEDIUM PRIORITY - Application Features

#### 4. `src-tauri/src/startup.rs`

- **Function**: Permission error detection (line 146)
- **Issue**: String matching for accessibility permissions
- **Impact**: App startup and permission validation
- **Status**: 🟡 NOT STARTED

#### 5. `src-tauri/src/anthropic.rs`

- **Function**: Network error detection (line 427)
- **Issue**: Uses `is_network_error()` function
- **Impact**: LLM error handling and user feedback
- **Dependencies**: Depends on network.rs migration
- **Status**: 🟡 BLOCKED (waiting for network.rs)

#### 6. `src-tauri/src/commands/notifications.rs`

- **Function**: Permission string matching (lines 131-132, 158-159)
- **Issue**: String matching for notification permissions
- **Impact**: Notification system functionality
- **Status**: 🟡 NOT STARTED

## Migration Steps

### Phase 1: Foundation (Week 1)

1. **Create Error Type Hierarchy**
   - [ ] Define `NetworkError` enum with proper variants
   - [ ] Define `PermissionError` enum for access control
   - [ ] Define `ToolError` enum for agent operations
   - [ ] Create `ErrorClassifiable` trait

2. **Implement Error Conversions**
   - [ ] `From<reqwest::Error>` for `NetworkError`
   - [ ] `From<std::io::Error>` for appropriate error types
   - [ ] `From<tokio_tungstenite::tungstenite::Error>` for WebSocket errors

### Phase 2: Network Layer (Week 2)

1. **Migrate `src-tauri/src/utils/network.rs`**
   - [ ] Replace `is_network_error()` with `classify_error()`
   - [ ] Implement error source chain analysis
   - [ ] Add comprehensive tests for error classification
   - [ ] Update documentation

2. **Update Dependencies**
   - [ ] Update `src-tauri/src/anthropic.rs` to use new classification
   - [ ] Update `src-tauri/src/cloud/connector.rs` error handling

### Phase 3: Agent Layer (Week 3)

1. **Migrate `src-tauri/src/agent/error_recovery.rs`**
   - [ ] Replace string pattern matching with error type matching
   - [ ] Implement trait-based error classification
   - [ ] Update recovery strategy selection logic
   - [ ] Add comprehensive error recovery tests

2. **Migrate `src-tauri/src/agent/implementations/tool_provider.rs`**
   - [ ] Replace `classify_error()` string matching
   - [ ] Implement structured error handling
   - [ ] Update tool execution error paths

### Phase 4: Application Layer (Week 4)

1. **Migrate Permission Systems**
   - [ ] Update `src-tauri/src/startup.rs` permission detection
   - [ ] Update `src-tauri/src/commands/notifications.rs` permission handling
   - [ ] Implement structured permission error types

2. **Testing and Validation**
   - [ ] Run comprehensive error handling tests
   - [ ] Validate error classification accuracy
   - [ ] Test recovery strategies with structured errors
   - [ ] Performance benchmarking (string vs type matching)

## Implementation Guidelines

### Error Type Structure

```rust
// Example structure for NetworkError
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum NetworkError {
    #[error("Connection timeout")]
    ConnectionTimeout,
    
    #[error("Connection refused")]
    ConnectionRefused,
    
    #[error("DNS resolution failed")]
    DnsResolution,
    
    #[error("TLS handshake failed")]
    TlsHandshake,
    
    #[error("Request timeout")]
    RequestTimeout,
    
    #[error("Host unreachable")]
    Unreachable,
    
    #[error("Unknown network error: {0}")]
    Unknown(String),
}
```

### Error Classification Trait

```rust
pub trait ErrorClassifiable {
    fn classify(&self) -> ErrorClass;
    fn is_recoverable(&self) -> bool;
    fn recovery_strategy(&self) -> RecoveryStrategy;
    fn user_message(&self) -> String;
}
```

### Migration Validation

Each migrated file must:

1. ✅ Remove all string-based error detection
2. ✅ Implement structured error types
3. ✅ Add comprehensive tests
4. ✅ Pass `cargo check` without warnings
5. ✅ Pass `./scripts/detect-string-error-patterns.sh` validation

## Success Metrics

- [ ] Zero string-based error detection patterns detected by script
- [ ] All error handling paths covered by tests
- [ ] Performance improvement in error classification (benchmark)
- [ ] Improved error recovery accuracy
- [ ] Better user error messages and debugging information

## Risk Mitigation

- **Breaking Changes**: Maintain backward compatibility where possible
- **Testing**: Comprehensive test coverage for all error paths
- **Rollback Plan**: Git branches for each migration phase
- **Performance**: Benchmark before/after to ensure no regression

## Completion Checklist

- [ ] Phase 1: Foundation error types created
- [ ] Phase 2: Network layer migrated and tested
- [ ] Phase 3: Agent layer migrated and tested
- [ ] Phase 4: Application layer migrated and tested
- [ ] ✅ All files pass `detect-string-error-patterns.sh` validation
- [ ] ✅ Comprehensive test suite passes
- [ ] ✅ Documentation updated
- [ ] ✅ Performance benchmarks show improvement or no regression

## Notes

- **Priority**: Focus on high-impact files first (network.rs, error_recovery.rs)
- **Dependencies**: Some files depend on others being migrated first
- **Testing**: Each phase should include thorough testing before proceeding
- **Documentation**: Update all affected documentation as part of migration
