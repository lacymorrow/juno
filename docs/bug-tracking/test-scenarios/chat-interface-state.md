# Chat Interface State Management Test Scenarios

This document outlines critical test scenarios for the chat interface state management system, particularly the disable/enable cycle during agent execution.

## Overview

The chat interface uses an `isProcessing` state to disable input and buttons during agent execution. This state must be properly managed through streaming events to ensure the interface remains usable.

## Critical Test Scenarios

### Scenario 1: Normal Agent Execution Cycle

**Purpose**: Verify interface properly disables and re-enables during normal agent execution

**Steps**:

1. Open Juno chat interface
2. Verify input field and send button are enabled
3. Type a message that will trigger agent execution
4. Click send button
5. **Verify**: Input field becomes disabled immediately
6. **Verify**: Send button becomes disabled/grayed out
7. **Verify**: Processing indicator appears
8. Wait for agent to complete processing
9. **Verify**: Input field re-enables when agent completes
10. **Verify**: Send button re-enables when agent completes
11. **Verify**: Processing indicator disappears
12. Type and send another message to confirm interface is fully functional

**Expected Events**:

- `agent-stream-start` → interface disables
- `agent-text-stream` → streaming content appears
- `agent-stream-end` → interface re-enables

### Scenario 2: Agent Execution with Error

**Purpose**: Verify interface re-enables even when agent execution fails

**Steps**:

1. Open Juno chat interface
2. Send a message that will cause an agent error (e.g., invalid tool request)
3. **Verify**: Interface disables during processing
4. Wait for error to occur
5. **Verify**: Error message appears
6. **Verify**: Interface re-enables despite error
7. **Verify**: User can send another message

### Scenario 3: Multiple Rapid Executions

**Purpose**: Verify state management with rapid successive agent calls

**Steps**:

1. Send first agent message
2. **Verify**: Interface disables
3. Wait for completion
4. **Verify**: Interface re-enables
5. Immediately send second agent message
6. **Verify**: Interface disables again
7. Wait for completion
8. **Verify**: Interface re-enables properly

### Scenario 4: Long-Running Agent Tasks

**Purpose**: Verify interface remains responsive during extended agent execution

**Steps**:

1. Send message triggering a long-running agent task
2. **Verify**: Interface disables immediately
3. **Verify**: Streaming updates appear during execution
4. Wait for full completion (may take 30+ seconds)
5. **Verify**: Interface re-enables when task completes
6. **Verify**: No interface elements remain permanently disabled

### Scenario 5: Browser Refresh During Agent Execution

**Purpose**: Verify interface state recovery after page refresh

**Steps**:

1. Send agent message
2. **Verify**: Interface disables
3. Refresh browser/restart app during agent execution
4. **Verify**: Interface returns to enabled state
5. **Verify**: User can send new messages

## Event Flow Verification

### Required Events

Each agent execution must emit these events in order:

1. **`agent-stream-start`**
   - Payload: `{message_id}`
   - Effect: `isProcessing = true`

2. **`agent-text-stream`** (multiple)
   - Payload: `{chunk, message_id}`
   - Effect: Append chunk to message

3. **`agent-stream-end`**
   - Payload: `{message_id, complete_text}`
   - Effect: `isProcessing = false`

### Event Testing

**Manual Verification**:

1. Open browser dev tools
2. Monitor console for event emissions
3. Verify event names match exactly: `"agent-stream-start"`, `"agent-text-stream"`, `"agent-stream-end"`
4. Verify each event has correct payload structure

**Automated Testing**:

```javascript
// Test that streaming events are emitted correctly
test('agent execution emits correct streaming events', async () => {
  const events = [];
  
  // Listen for streaming events
  listen('agent-stream-start', (event) => events.push({type: 'start', ...event}));
  listen('agent-text-stream', (event) => events.push({type: 'stream', ...event}));
  listen('agent-stream-end', (event) => events.push({type: 'end', ...event}));
  
  // Trigger agent execution
  await sendMessage("test message");
  
  // Verify event sequence
  expect(events[0].type).toBe('start');
  expect(events[events.length - 1].type).toBe('end');
  expect(events.filter(e => e.type === 'stream').length).toBeGreaterThan(0);
});
```

## Regression Prevention

### Code Review Checklist

When modifying streaming-related code, verify:

- [ ] Event names match frontend listeners exactly
- [ ] All code paths call `emit_stream_end()` (success and error cases)
- [ ] Event payloads include required fields
- [ ] Frontend state management responds to all events
- [ ] Interface re-enables in all scenarios (success, error, timeout)

### Files to Monitor

Changes to these files require extra attention to streaming state:

- `src/App.tsx` (streaming event handlers)
- `src-tauri/src/agent/tool_logger.rs` (event emission)
- `src-tauri/src/agent/providers/anthropic.rs` (stream handling)
- `src-tauri/src/anthropic.rs` (error handling)

### Common Failure Patterns

1. **Event Name Mismatch**: Backend emits generic events instead of specific names
2. **Missing Stream End**: Error cases don't call `emit_stream_end()`
3. **Payload Structure Changes**: Frontend expects different payload format
4. **State Race Conditions**: Multiple rapid executions cause state confusion

---

**Created**: 2024-12-20  
**Last Updated**: 2024-12-20  
**Related Regression**: `2024-12-chat-interface-disabled.md`
