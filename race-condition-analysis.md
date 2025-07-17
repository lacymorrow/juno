# Race Condition Analysis Report for DotDot Event-Driven Architecture

## Summary

This analysis identified several critical race conditions in the event-driven architecture of the DotDot application. The primary concerns involve shared mutable state accessed by multiple event handlers without proper synchronization mechanisms.

## Critical Race Conditions Found

### 1. **useBackendEvents Hook - Streaming Message Map**
**File:** `/src/hooks/useBackendEvents.ts`
**Risk Level:** HIGH 🔴

**Issue:** Uses `streamingMessages.current` Map for concurrent event handling without synchronization
```typescript
const streamingMessages = useRef<Map<string, string>>(new Map());

// Multiple events modify this map concurrently:
case "agent-stream-start":
    streamingMessages.current.set(payload.message_id, "");
case "agent-text-stream":
    const existing = streamingMessages.current.get(payload.message_id) || "";
    const newText = existing + payload.chunk;
    streamingMessages.current.set(payload.message_id, newText);
```

**Race Condition Scenario:**
- Multiple streaming events for the same message ID could arrive out of order
- Concurrent updates to the Map could result in lost chunks or corrupted message state
- No locking mechanism prevents simultaneous read/write operations

### 2. **useAppState Hook - Multiple State Variables**
**File:** `/src/hooks/useAppState.ts`
**Risk Level:** MEDIUM 🟡

**Issue:** Extensive shared state with multiple useState calls that could be updated from various event sources
```typescript
const [isProcessing, setIsProcessing] = useState(false);
const [serverStatus, setServerStatus] = useState<"connected" | "error" | "connecting">("connecting");
const [activeModal, setActiveModal] = useState<ModalType>(null);
// ... many more state variables
```

**Race Condition Scenario:**
- Multiple event handlers could trigger state updates simultaneously
- State updates are not atomic across multiple useState calls
- Timing-dependent bugs when multiple events affect related state

### 3. **useAudioPlayback Hook - Audio Element Management**
**File:** `/src/hooks/useAudioPlayback.ts`
**Risk Level:** MEDIUM 🟡

**Issue:** `currentAudio` state is accessed in multiple async contexts and event handlers
```typescript
const [currentAudio, setCurrentAudio] = useState<HTMLAudioElement | null>(null);

// Accessed in playAudioFromBase64, stopCurrentAudio, event handlers, and cleanup
if (currentAudio) {
    currentAudio.pause();
    currentAudio.currentTime = 0;
    // ... more operations
}
```

**Race Condition Scenario:**
- Audio cleanup could race with new audio starting
- Event handlers (onended, onerror) could fire while state is being updated
- URL cleanup might happen while audio is still being accessed

### 4. **ProductionCloudConnector - Listener Arrays**
**File:** `/src/lib/cloud-connector.ts`
**Risk Level:** LOW-MEDIUM 🟡

**Issue:** Mutable listener arrays without synchronization
```typescript
private statusListeners: ((status: CloudConnectorStatus) => void)[] = [];
private messageListeners: ((message: CloudMessage) => void)[] = [];
```

**Race Condition Scenario:**
- Adding/removing listeners while events are being fired
- Iterating over listeners while array is being modified
- No protection against concurrent modifications

### 5. **Event Handler Timing Dependencies**
**File:** `/src/hooks/useMenuEvents.ts` and `/src/hooks/useEventListener.ts`
**Risk Level:** LOW 🟢

**Issue:** Event handlers depend on execution order but have no coordination
```typescript
// Multiple listeners setup without coordination
unlistenCallbacks.push(
    await listen(EVENTS.MENU_DEVTOOLS_REQUESTED, () => {
        setCurrentView("devtools");
    })
);
```

**Race Condition Scenario:**
- Events could be processed out of order
- State changes from one event might not be visible to another
- No guarantee of event ordering or atomicity

## Recommendations

### 1. **Implement Proper State Management**
- Consider using a reducer pattern (useReducer) for complex state
- Use a state management library (Redux, Zustand) for global state
- Implement atomic state updates for related state variables

### 2. **Add Synchronization Mechanisms**
- Use mutexes or semaphores for critical sections
- Implement message queuing for event processing
- Add sequence numbers to streaming messages

### 3. **Event Ordering and Coordination**
- Implement event queuing with guaranteed ordering
- Add timestamps and sequence numbers to events
- Use async/await patterns consistently

### 4. **Audio Resource Management**
- Implement a proper audio queue system
- Add state machines for audio lifecycle
- Use cleanup functions with proper cancellation tokens

### 5. **Testing Recommendations**
- Add concurrent event testing
- Implement stress tests with rapid event firing
- Test with network delays and out-of-order events

## Example Fix for Streaming Messages

```typescript
// Use a proper concurrent data structure or add locking
import { Mutex } from 'async-mutex';

const streamingMessagesMutex = new Mutex();
const streamingMessages = useRef<Map<string, string>>(new Map());

// In event handler:
const release = await streamingMessagesMutex.acquire();
try {
    const existing = streamingMessages.current.get(payload.message_id) || "";
    const newText = existing + payload.chunk;
    streamingMessages.current.set(payload.message_id, newText);
} finally {
    release();
}
```

## Severity Assessment

- **Critical Issues:** 1 (streaming message handling)
- **High Risk Issues:** 2 (app state, audio management)
- **Medium Risk Issues:** 2 (cloud connector, event ordering)
- **Total Race Conditions Found:** 5

## Next Steps

1. Prioritize fixing the streaming message race condition
2. Implement proper state management patterns
3. Add comprehensive concurrent testing
4. Monitor production for race condition symptoms
5. Consider using event sourcing for critical operations