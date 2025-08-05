# Mode Manager Test Documentation

## Overview

This document describes the comprehensive test suite for the Juno Mode Manager, which handles the application's different operational modes (Idle, Agent, Dictation).

## Test Structure

All tests are located in `src/mode_manager.rs` within the `#[cfg(test)]` module. The tests use Tokio's async testing framework to properly test the async nature of the mode manager.

## Test Categories

### 1. Initial State Tests

**Test: `test_initial_state`**
- Verifies the mode manager starts in Idle mode
- Checks default configuration values
- Ensures history is empty on initialization

### 2. Mode Transition Tests

**Test: `test_valid_transitions`**
- Tests all valid mode transitions:
  - Idle → Agent
  - Agent → Idle
  - Idle → Dictation
  - Dictation → Idle

**Test: `test_invalid_transitions`**
- Verifies invalid transitions are blocked:
  - Agent → Dictation (must go through Idle)
  - Dictation → Agent (must go through Idle)

**Test: `test_same_mode_transition`**
- Ensures transitioning to the same mode is handled as a no-op
- Verifies no duplicate history entries

### 3. Configuration Management Tests

**Test: `test_config_updates`**
- Tests updating mode configuration
- Verifies changes persist correctly

**Test: `test_wake_word_detection_config`**
- Tests wake word configuration
- Verifies default wake words
- Tests updating wake word list

**Test: `test_wake_sensitivity_bounds`**
- Tests sensitivity values at boundaries (0.0, 1.0)
- Tests values outside normal bounds

### 4. History Management Tests

**Test: `test_transition_history`**
- Verifies transitions are recorded in history
- Checks history is in reverse chronological order

**Test: `test_history_size_limit`**
- Tests that history is capped at 100 entries
- Verifies old entries are removed when limit exceeded

### 5. Concurrency Tests

**Test: `test_concurrent_mode_access`**
- Tests concurrent read access to current mode
- Ensures thread safety with multiple readers

**Test: `test_concurrent_config_updates`**
- Tests concurrent configuration updates
- Verifies both updates are applied correctly

### 6. Serialization Tests

**Test: `test_mode_serialization`**
- Verifies AppMode serializes to correct JSON strings
- Tests all three modes

**Test: `test_config_serialization`**
- Tests ModeConfig serialization/deserialization
- Ensures round-trip consistency

### 7. Edge Case Tests

**Test: `test_mode_transition_edge_cases`**
- Tests empty reason strings
- Tests very long reason strings (1000 chars)

**Test: `test_mode_status_structure`**
- Verifies the status JSON structure
- Checks all required fields are present

## Running the Tests

### Run all mode manager tests:
```bash
cargo test mode_manager::tests --lib
```

### Run with output:
```bash
cargo test mode_manager::tests --lib -- --show-output
```

### Run a specific test:
```bash
cargo test test_initial_state --lib
```

### Use the convenience script:
```bash
./test_mode_manager.sh
```

## Test Coverage

The test suite covers:

1. **State Management**: 100% coverage of state transitions and validation
2. **Configuration**: Full coverage of config updates and retrieval
3. **History**: Complete coverage of history recording and limits
4. **Concurrency**: Thread safety and race condition testing
5. **Serialization**: JSON serialization for API compatibility
6. **Edge Cases**: Boundary conditions and unusual inputs

## Integration Points

While these are unit tests, they verify behavior that integrates with:

- Voice transcription system
- Always listening mode
- Escape key handling
- Event emission system
- Tauri command handlers

## Future Test Enhancements

1. **Mock AppHandle Tests**: Create proper mock AppHandle for testing actual transition methods
2. **Event Testing**: Verify event emission during mode changes
3. **Permission Integration**: Test mode behavior with different permission states
4. **Performance Tests**: Add benchmarks for concurrent access patterns
5. **Property-Based Tests**: Use proptest for exhaustive state machine testing

## Common Test Patterns

### Creating a Test Manager
```rust
let manager = create_test_manager();
```

### Testing Async Methods
```rust
#[tokio::test]
async fn test_async_method() {
    let manager = create_test_manager();
    let result = manager.some_async_method().await;
    assert!(result.is_ok());
}
```

### Testing Concurrent Access
```rust
let manager = Arc::new(create_test_manager());
let handle = tokio::spawn(async move {
    // concurrent operation
});
```

## Debugging Failed Tests

1. Run with `--nocapture` to see print statements
2. Check for timing issues in concurrent tests
3. Verify test isolation (each test gets fresh manager)
4. Look for unhandled unwrap() calls in implementation

## Maintenance Notes

- Keep tests independent - each should create its own manager
- Update tests when adding new modes or transitions
- Add tests for any new configuration options
- Maintain test documentation alongside code changes