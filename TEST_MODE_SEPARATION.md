# Test Plan: Mode Separation

This document outlines how to test that dictation mode and agent mode are properly separated.

## Setup
1. Make sure the app is built with the latest changes
2. Configure keyboard shortcuts (defaults: Option+D for Agent, Option+Space for Dictation)

## Test Cases

### Test 1: Dictation Mode (Option+Space)
1. Press Option+Space (or your configured dictation shortcut)
2. Speak some text
3. **Expected**: Text should be typed at your cursor position
4. **Expected**: NO agent processing should occur
5. **Expected**: Console should show "[Dictation Mode]" logs, not "[Agent Mode]"

### Test 2: Agent Mode (Option+D)  
1. Press Option+D (or your configured agent shortcut)
2. Speak a command (e.g., "open calculator")
3. **Expected**: Agent should process the command
4. **Expected**: Text should NOT be typed at cursor
5. **Expected**: Console should show "[Agent Mode]" logs

### Test 3: Always Listening → Agent Mode
1. Enable Always Listening in settings
2. Say wake word (e.g., "Hey Juno")
3. Then say a command
4. **Expected**: Agent should process the command
5. **Expected**: Text should NOT be typed at cursor

### Test 4: Mode Isolation
1. Start Dictation Mode (Option+Space)
2. While dictating, try pressing Agent Mode (Option+D)
3. **Expected**: Modes should not interfere with each other
4. **Expected**: Each mode should handle its own transcription

## Console Log Verification

### Dictation Mode Logs:
```
[Dictation Input Shortcut] Tap mode - starting dictation mode transcription
[Dictation Tap Mode] Emitted dictation start event for handler processing
[Event] Received dictation-transcription-start event
[Dictation Mode] Started immediate transcription successfully
[Event] Processing final result for Dictation Mode
[Dictation Mode] Successfully typed text: 'your text here'
```

### Agent Mode Logs:
```
[Agent Mode Shortcut] Tap mode - starting agent mode transcription  
[Agent Mode] Emitted agent start event for handler processing
[Event] Received agent-transcription-start event
[Event] Processing final result for AI Agent Mode
[Agent Mode] Query submitted: 'your command here'
```

## Key Changes Made

1. **Fixed Event Emission**:
   - Agent mode now emits `agent-transcription-start` instead of `app-dictation-started`
   - Dictation mode emits `dictation-transcription-start`

2. **Proper Event Routing**:
   - `app-dictation-finished` event only triggers agent mode when appropriate
   - Dictation mode results are handled separately in `handle_dictation_mode_result()`

3. **Always Listening Integration**:
   - Always Listening still properly pauses during dictation
   - Resumes after dictation completes

## Debugging

If modes are still mixed up, check:
1. Console logs to see which events are being emitted
2. Look for "[Dictation Mode]" vs "[Agent Mode]" prefixes
3. Check if `app-dictation-finished` is being triggered incorrectly
4. Verify the event constants in `constants/events.rs` match expectations