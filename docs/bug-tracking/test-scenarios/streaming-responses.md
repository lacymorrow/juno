# Streaming Response Test Scenarios

This document outlines critical test scenarios for agent streaming responses to prevent regressions.

## Core Streaming Functionality

### Test 1: Basic Streaming Display

**Objective**: Verify progressive text streaming is visible to users
**Steps**:

1. Open Juno application
2. Activate agent mode (Option+D or voice command)
3. Send a query: "Write a brief explanation of how AI works"
4. **Verify**: Text appears progressively as agent responds
5. **Verify**: Streaming indicators (typing animation, etc.) are visible
6. **Verify**: Text remains readable and properly formatted during streaming

**Expected Behavior**: Users see text appearing word-by-word or in small chunks

### Test 2: Streaming Completion

**Objective**: Ensure proper transition from streaming to final message
**Steps**:

1. Start a streaming response (any agent query)
2. Wait for response to complete
3. **Verify**: Final message is complete and properly formatted
4. **Verify**: No text is missing or duplicated
5. **Verify**: Message state transitions from streaming to complete

### Test 3: Rapid Message Streaming

**Objective**: Test streaming with multiple quick messages
**Steps**:

1. Send first agent query
2. Immediately send second query before first completes
3. **Verify**: Both responses stream independently
4. **Verify**: No interference between streaming messages
5. **Verify**: Both messages complete properly

## Streaming + JSX Integration

### Test 4: JSX Detection After Streaming

**Objective**: Verify JSX components render after streaming completes
**Steps**:

1. Send query that results in JSX/React components: "Create a simple button component"
2. **Verify**: Text streams normally during response
3. **Verify**: After completion, JSX components render properly
4. **Verify**: No JSX parsing errors in console

### Test 5: Streaming with Markdown

**Objective**: Ensure Markdown rendering works with streaming
**Steps**:

1. Send query: "Write a markdown document with headers, lists, and code blocks"
2. **Verify**: Text streams normally
3. **Verify**: Markdown renders properly after completion
4. **Verify**: No formatting issues during or after streaming

### Test 6: Complex Content Streaming

**Objective**: Test streaming with mixed content types
**Steps**:

1. Send query for complex response with code, markdown, and potential JSX
2. **Verify**: Streaming works regardless of final content type
3. **Verify**: Content detection only happens after streaming completes
4. **Verify**: Final rendering is correct for content type detected

## Error Scenarios

### Test 7: Interrupted Streaming

**Objective**: Handle streaming interruptions gracefully
**Steps**:

1. Start a long streaming response
2. Interrupt by sending new query or closing app
3. **Verify**: No hanging states or memory leaks
4. **Verify**: New streaming works normally

### Test 8: Empty or Error Responses

**Objective**: Test streaming with edge case responses
**Steps**:

1. Send query that might result in empty response
2. Send query that might cause agent error
3. **Verify**: Streaming handles edge cases gracefully
4. **Verify**: No UI breaking or console errors

## Performance Tests

### Test 9: Long Streaming Responses

**Objective**: Verify performance with very long responses
**Steps**:

1. Send query for very long response: "Write a detailed 2000-word essay"
2. **Verify**: Streaming remains smooth throughout
3. **Verify**: UI stays responsive during long streaming
4. **Verify**: Memory usage remains reasonable

### Test 10: Streaming with High Frequency Updates

**Objective**: Test rapid streaming updates
**Steps**:

1. Monitor streaming event frequency
2. **Verify**: UI can handle rapid updates without lag
3. **Verify**: No dropped text or display artifacts

## Integration Tests

### Test 11: Voice + Streaming

**Objective**: Test streaming with voice integration
**Steps**:

1. Use voice command to trigger agent
2. **Verify**: Voice transcription completes before streaming starts
3. **Verify**: Streaming works normally after voice input

### Test 12: Command Overlay + Streaming

**Objective**: Verify command overlay doesn't interfere with streaming
**Steps**:

1. Enable command overlay in settings
2. Trigger agent response that shows command execution
3. **Verify**: Both command overlay and streaming work simultaneously
4. **Verify**: No visual conflicts or performance issues

## Regression Prevention Checklist

Before any release involving message rendering or streaming:

- [ ] Run all streaming test scenarios
- [ ] Test streaming + JSX detection interaction
- [ ] Verify streaming indicators work
- [ ] Test with various content types (text, markdown, JSX)
- [ ] Check console for streaming-related errors
- [ ] Test interruption and error scenarios
- [ ] Verify performance with long responses
- [ ] Test integration with voice and other features

## Common Issues to Watch For

1. **JSX Detection During Streaming**: Ensure JSX detection only runs on completed messages
2. **Streaming State Management**: Verify `isStreaming` flag is properly managed
3. **Event Handler Cleanup**: Ensure streaming event listeners are properly cleaned up
4. **Performance Degradation**: Watch for memory leaks or UI lag during streaming
5. **Content Type Conflicts**: Ensure different content types don't interfere with streaming

---

*This document should be updated whenever new streaming features are added or issues are discovered.*
