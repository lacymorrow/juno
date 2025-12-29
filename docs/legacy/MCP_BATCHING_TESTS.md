# MCP Request Batching - Test Scenarios & Validation

## Overview

This document provides comprehensive test scenarios to validate the MCP request batching implementation in Juno AI Computer Use Agent.

## Test Environment Setup

### Prerequisites

- Development build of Juno with debug logging enabled
- `RUST_LOG=debug bun run tauri dev`
- Test applications (browser, text editor, file manager)
- Network monitoring tools (optional)

### Verification Commands

```bash
# Compile and verify implementation
cargo check --manifest-path src-tauri/Cargo.toml --message-format=short 2>&1

# Run with debug logging to observe batching
RUST_LOG=debug bun run tauri dev
```

## Core Test Scenarios

### 1. Sequential Desktop Pattern Detection

**Test Case**: Type → Enter → Screenshot

```
User: "Type 'Hello World' in the text field, press Enter, then take a screenshot"
Expected Batching: [type_text, key_press, screenshot] → Single Batch
```

**Validation Steps**:

1. Open a text editor or web form
2. Issue the command above
3. Monitor logs for batch detection messages
4. Verify execution happens as single operation

**Expected Log Output**:

```
Tool execution plan: 3 tools organized into 1 batch(es)
Executing batch 0: 3 tools (type_text → key_press → screenshot)
```

### 2. Click → Screenshot Pattern

**Test Case**: Navigation with immediate feedback

```
User: "Click the submit button and take a screenshot"
Expected Batching: [click, screenshot] → Single Batch
```

**Validation**:

- Should detect click+screenshot as obvious sequential pair
- No agent reasoning between operations
- Faster execution than individual tool calls

### 3. Complex Form Interaction

**Test Case**: Multi-field form completion

```
User: "Fill out the form - type 'John' in name field, type 'john@email.com' in email field, click submit, take screenshot"
Expected Batching: [type_text, type_text, click, screenshot] → Single Batch (if sequential)
```

**Validation**:

- Complex multi-tool sequences should batch when appropriate
- Form interaction should be smooth and fast

### 4. MCP Tool Chain Detection

**Test Case**: External MCP server tools

```
User: "Use file explorer to navigate to Documents, list files, create new folder"
Expected Batching: MCP tools from same server → MCP Batch Request
```

**Validation**:

- MCP tools from same server should use JSON-RPC batch format
- Different servers should execute separately
- Error in one MCP tool should not break entire batch

### 5. Mixed Tool Types (No Batching)

**Test Case**: Tools requiring individual reasoning

```
User: "Take a screenshot, analyze what you see, then click the most relevant button"
Expected Batching: No batching - requires reasoning between tools
```

**Validation**:

- Should execute sequentially with agent reasoning
- Each tool should have individual approval if required
- Memory updates after each tool

## Performance Test Scenarios

### 6. Latency Comparison

**Setup**: Measure execution time differences

```bash
# Tool A: Execute 5 sequential screenshot operations (non-batched)
# Tool B: Execute equivalent batched operations
```

**Validation**:

- Batched operations should show ~33% time reduction
- Network requests should be consolidated
- Memory updates should be more efficient

### 7. Error Recovery in Batches

**Test Case**: Batch with failing tool

```
User: "Type 'test', press invalid key combo, take screenshot"
Expected: Partial batch execution with error handling
```

**Validation**:

- First tool should execute successfully
- Second tool should fail gracefully
- Third tool should be skipped or cancelled
- Conversation state should remain consistent

### 8. Cancellation During Batch

**Test Case**: User cancels during batch execution

```
1. Start batch operation: "Type long text, press enter, take screenshot"
2. Cancel (Escape key) during typing
```

**Validation**:

- Cancellation should stop entire batch immediately
- No orphaned operations should continue
- Memory state should be consistent
- Tool results should reflect cancellation

## Advanced Scenarios

### 9. Approval System Integration

**Test Case**: Batch requiring user approval

```
User: Enable tool approval mode
Command: "Delete file, empty trash, take screenshot"
Expected: Single approval prompt for entire batch
```

**Validation**:

- Should request approval once for entire batch
- Approval denial should cancel entire batch
- Individual tool approvals should not be required

### 10. Mixed MCP and Local Tools

**Test Case**: Heterogeneous tool batch

```
User: "Take screenshot (local), upload to cloud storage (MCP), send notification (local)"
Expected: Smart batching with appropriate grouping
```

**Validation**:

- Local tools should batch together where appropriate
- MCP tools should use their own batching mechanism
- Execution should be optimized across tool types

### 11. Pattern Detection Edge Cases

**Test Case**: Similar but non-batchable patterns

```
User: "Take screenshot, wait 5 seconds, take another screenshot"
Expected: No batching due to timing dependency
```

**Validation**:

- Time-dependent operations should not batch
- Tool analyzer should correctly identify dependencies
- Sequential execution should be used

## Integration Test Scenarios

### 12. Voice Command Batching

**Test Case**: Voice input with batch-suitable commands

```
Voice: "Type my name, press enter, and screenshot the result"
Expected: Voice → Text → Batch execution
```

**Validation**:

- Voice transcription should work normally
- Resulting text command should trigger batching
- End-to-end voice-to-batch should work seamlessly

### 13. Always Listening Mode

**Test Case**: Background listening with batch commands

```
Always Listening: Active
Voice: "Type 'meeting notes', press enter, screenshot"
Expected: Background processing with batching
```

**Validation**:

- Always listening should detect batch-suitable commands
- Background execution should use batching optimizations
- System responsiveness should be maintained

### 14. Timer System Integration

**Test Case**: Scheduled commands with batching

```
User: Set timer for batch operation
Timer executes: "Open calculator, type '2+2', press equals, screenshot"
Expected: Timer-triggered batch execution
```

**Validation**:

- Timer system should support batch commands
- Scheduled batches should execute correctly
- Context resumption should work with batching

## Monitoring & Validation

### Log Analysis

Monitor these key log messages:

```
Tool execution plan: X tools organized into Y batch(es)
Executing batch N: X tools (tool1 → tool2 → ...)
MCP batch execution: server_id with X tools
Batch execution completed: success_count/total_count
```

### Performance Metrics

Track these indicators:

- Total execution time per command
- Number of network requests
- Memory allocation patterns
- User-perceived responsiveness

### Error Conditions

Validate proper handling of:

- Network timeouts during batch execution
- Individual tool failures within batches
- Memory pressure during large batches
- Concurrent batch requests

## Success Criteria

### ✅ Implementation Verification

- [ ] Compilation passes without errors
- [ ] All test scenarios execute correctly
- [ ] Log messages confirm batching behavior
- [ ] No regression in existing functionality

### ✅ Performance Improvements

- [ ] Measurable latency reduction in batch-suitable scenarios
- [ ] Reduced network overhead for MCP tools
- [ ] Improved user experience for common patterns
- [ ] Maintained system stability under load

### ✅ Error Handling

- [ ] Graceful degradation when batching fails
- [ ] Consistent conversation state during errors
- [ ] Proper cancellation support
- [ ] User feedback for batch operations

## Test Execution Checklist

1. **Environment Setup**
   - [ ] Development build compiled successfully
   - [ ] Debug logging enabled
   - [ ] Test applications available

2. **Core Functionality**
   - [ ] Sequential pattern detection works
   - [ ] MCP batch execution functional
   - [ ] Mixed tool handling appropriate
   - [ ] Error recovery operational

3. **Performance Validation**
   - [ ] Latency improvements measured
   - [ ] Resource usage optimized
   - [ ] User experience enhanced

4. **Integration Testing**
   - [ ] Voice system compatibility
   - [ ] Timer system integration
   - [ ] Always listening mode support

5. **Edge Case Handling**
   - [ ] Cancellation scenarios tested
   - [ ] Error conditions validated
   - [ ] Approval system integrated
   - [ ] Memory consistency maintained

## Troubleshooting

### Common Issues

- **No batching detected**: Check tool pattern matching logic
- **MCP batch failures**: Verify JSON-RPC 2.0 format compliance
- **Performance regression**: Monitor memory allocation and network usage
- **Inconsistent state**: Review conversation memory management

### Debug Commands

```bash
# Enable detailed batching logs
RUST_LOG=debug,juno::agent::implementations::agent_runner=trace bun run tauri dev

# Test specific patterns
cargo test test_batch_pattern_detection --manifest-path src-tauri/Cargo.toml

# Validate MCP integration
cargo test mcp_batch_execution --manifest-path src-tauri/Cargo.toml
```
