# Event Synchronization Implementation Guide

## Immediate Actions Required

### 1. Integrate New Event Hooks in App.tsx

Add the following hooks to the main App component:

```typescript
import { useShortcutEvents } from "@/hooks/useShortcutEvents";
import { useDictationStateEvents } from "@/hooks/useDictationStateEvents";

// Inside App component:
useShortcutEvents({
  onAgentModeShortcut: (payload) => {
    // Handle agent mode shortcut events
    if (payload.state === "pressed" && !payload.test_mode) {
      // Update UI state for agent mode
    }
  },
  onDictationInputShortcut: (payload) => {
    // Handle dictation input shortcut events
    if (payload.state === "pressed" && !payload.test_mode) {
      // Update UI state for dictation mode
    }
  },
});

useDictationStateEvents({
  onStateChanged: (event) => {
    // Sync UI with backend dictation state
    console.log("Dictation state changed:", event);
    // Update local state or trigger UI updates
  },
  onForceReset: (reason) => {
    // Handle force reset events
    console.log("Dictation force reset:", reason);
    // Reset UI state
  },
});
```

### 2. Remove Duplicate Event Listeners

Check these components for duplicate shortcut event listeners:
- `/src/components/onboarding/Onboarding.tsx` - Keep for onboarding only
- Any other components listening for shortcut events should use the centralized hook

### 3. Add Missing Event Listeners in useBackendEvents.ts

Add listeners for these critical events:
```typescript
// Agent state events
EVENTS.AGENT_ACTIVE
EVENTS.AGENT_COMMITTED
EVENTS.AGENT_FORCE_STOP
EVENTS.AGENT_FORCE_CLEANUP

// Dictation state events  
EVENTS.DICTATION_ACTIVE
EVENTS.DICTATION_CANCELLED
EVENTS.DICTATION_COMMITTED
```

### 4. Fix Event Emission Consistency

Search and replace in backend code:
- Find all `app.emit("string-literal"` patterns
- Replace with proper constant usage: `app.emit(events::module::CONSTANT`

## Testing Checklist

### Basic Functionality
- [ ] Agent mode shortcut (Alt+D) triggers properly
- [ ] Dictation mode shortcut (Alt+Space) triggers properly
- [ ] Escape key stops all active operations
- [ ] UI reflects correct state during dictation
- [ ] UI reflects correct state during agent execution

### Event Flow Testing
- [ ] Start dictation → Verify DICTATION_STATE_CHANGED event
- [ ] Stop dictation → Verify state change event
- [ ] Force stop → Verify FORCE_RESET event
- [ ] Agent execution → Verify agent state events

### Edge Cases
- [ ] Rapid shortcut pressing doesn't cause race conditions
- [ ] Multiple event listeners don't cause duplicate handling
- [ ] Event cleanup on component unmount works properly

## Code Quality Checklist

- [ ] All event names use constants from generated file
- [ ] No hardcoded event strings in TypeScript or Rust
- [ ] Event listeners are properly cleaned up
- [ ] Error handling for failed event emissions
- [ ] Console logging for debugging (remove in production)

## Architecture Recommendations

### 1. Event Bus Pattern
Consider implementing a centralized event bus:
```typescript
class EventBus {
  private listeners: Map<string, Set<Function>>;
  
  on(event: string, handler: Function) {
    // Add listener
  }
  
  off(event: string, handler: Function) {
    // Remove listener
  }
  
  emit(event: string, data: any) {
    // Emit to all listeners
  }
}
```

### 2. State Machine for Dictation/Agent States
Implement proper state machines to prevent invalid state transitions:
```typescript
enum DictationState {
  Idle = "idle",
  Starting = "starting", 
  Active = "active",
  Stopping = "stopping",
  Error = "error"
}

const validTransitions = {
  [DictationState.Idle]: [DictationState.Starting],
  [DictationState.Starting]: [DictationState.Active, DictationState.Error],
  // etc...
};
```

### 3. Event Documentation
Create comprehensive documentation:
- Event flow diagrams
- Component responsibility matrix
- Event payload schemas
- Testing scenarios

## Performance Considerations

1. **Debounce rapid events**: Use debouncing for events that can fire rapidly
2. **Batch state updates**: Group related state changes to avoid excessive re-renders
3. **Memory leak prevention**: Always clean up event listeners
4. **Event filtering**: Only listen for events relevant to each component

## Next Steps

1. Run full test suite after implementing changes
2. Monitor for any new event synchronization issues
3. Consider adding event replay for debugging
4. Implement event metrics/monitoring in production