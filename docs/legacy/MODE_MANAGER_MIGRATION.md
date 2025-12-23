# Mode Manager Migration Guide

This document outlines how to migrate from the current fragmented mode management to the new centralized Mode Manager architecture.

## Overview

The new Mode Manager provides:
- Single source of truth for app mode state
- Clean state machine transitions
- Simplified event handling
- Better separation of concerns
- Easier debugging and maintenance

## Architecture Changes

### Before: Fragmented State
```rust
// State scattered across multiple locations
AppState.audio_settings.dictation_active
AppState.audio_settings.always_listening_active
AppState.agent_execution.execution_active
UIStateData.voiceMode
UIStateData.isDictationMode
// ...and more
```

### After: Centralized Mode Manager
```rust
// Single mode enum
enum AppMode {
    Idle,      // Nothing active
    Agent,     // AI agent mode
    Dictation, // Text dictation mode
}

// Single manager
ModeManager {
    current_mode: AppMode,
    config: ModeConfig,
}
```

## Integration Steps

### 1. Add Mode Manager Module

Add to `src-tauri/src/lib.rs`:
```rust
mod mode_manager;
mod events {
    pub mod mode_handlers;
    pub mod mode_shortcuts;
    // Keep existing modules
}
```

### 2. Register Mode Commands

In `src-tauri/src/main.rs`, add mode manager commands:
```rust
.invoke_handler(tauri::generate_handler![
    // Existing commands...
    mode_manager::get_current_mode,
    mode_manager::set_mode,
    mode_manager::get_mode_config,
    mode_manager::set_always_listening_enabled,
    mode_manager::get_mode_status,
])
```

### 3. Update Event Listeners

Replace complex event handlers with simplified mode handlers:

```rust
// In setup_event_listeners()
events::mode_handlers::setup_mode_listeners(&app);
```

### 4. Update Keyboard Shortcuts

Replace shortcut handler with mode-aware version:

```rust
// In handle_global_shortcut()
events::mode_shortcuts::handle_mode_shortcut(&app, &shortcut, &event);
```

### 5. Update Frontend Integration

Update frontend to use mode commands:

```typescript
// Before
await invoke('set_dictation_active', { active: true });
await invoke('start_voice_transcription');

// After
await invoke('set_mode', { mode: 'dictation', reason: 'User request' });
```

### 6. Remove Redundant State

Once migrated, remove:
- `was_always_listening_active_before_dictation` tracking
- Complex state restoration logic
- Duplicate mode state fields
- Manual mode coordination code

## Mode Behavior

### Agent Mode
- Triggered by: Keyboard shortcut OR wake word (if always listening enabled)
- Actions: Start voice transcription → Submit to AI agent
- Exit: On completion OR escape key

### Dictation Mode  
- Triggered by: Keyboard shortcut only
- Actions: Start voice transcription → Type at cursor
- Exit: On completion OR escape key

### Always Listening
- Not a mode, but a feature that triggers Agent mode
- Enabled/disabled via settings
- Only active in Idle mode
- Detects wake words → Transitions to Agent mode

## Benefits

1. **Simpler Code**: 
   - Mode transitions in one place
   - No state synchronization issues
   - Clear mode priority rules

2. **Better UX**:
   - Predictable mode behavior
   - No mode conflicts
   - Clean error handling

3. **Easier Debugging**:
   - Mode history tracking
   - Single status endpoint
   - Clear transition logs

4. **Maintainability**:
   - Add new modes easily
   - Modify behavior in one place
   - Test mode transitions independently

## Migration Checklist

- [ ] Add mode_manager.rs module
- [ ] Add mode event handlers
- [ ] Add mode keyboard shortcuts
- [ ] Register Tauri commands
- [ ] Update frontend API calls
- [ ] Remove old state tracking code
- [ ] Update documentation
- [ ] Test all mode transitions
- [ ] Verify always listening behavior
- [ ] Check escape key cancellation

## Testing

After migration, test:

1. **Mode Transitions**:
   - Idle → Agent → Idle
   - Idle → Dictation → Idle
   - Agent → Escape → Idle
   - Dictation → Escape → Idle

2. **Always Listening**:
   - Enable in settings → Idle mode
   - Say wake word → Agent mode
   - Complete agent → Back to Idle
   - Disable → No wake word detection

3. **Edge Cases**:
   - Rapid mode switching
   - Mode conflicts (can't go Agent → Dictation)
   - Error recovery
   - App restart with always listening enabled