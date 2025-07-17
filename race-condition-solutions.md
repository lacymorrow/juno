# Race Condition Solutions Architecture

## Executive Summary

This document provides comprehensive solutions for the critical race conditions and event architecture issues identified in the DotDot application. All solutions prioritize thread safety, data integrity, and maintainability while minimizing performance impact.

## 🔴 Critical Issue Solutions

### 1. Streaming Message Map Race Condition

**Problem**: Concurrent access to `streamingMessages` Map without synchronization
**Location**: `/src/hooks/useBackendEvents.ts`

#### Solution A: Mutex-based Synchronization (Recommended)

```typescript
import { Mutex } from 'async-mutex';

// Create a dedicated mutex for streaming messages
const streamingMessagesMutex = useRef(new Mutex());
const streamingMessages = useRef<Map<string, StreamingMessage>>(new Map());

interface StreamingMessage {
  text: string;
  chunks: string[];
  sequence: number;
  lastUpdate: number;
  done: boolean;
}

// Safe update function
const updateStreamingMessage = async (
  messageId: string, 
  updater: (current: StreamingMessage | undefined) => StreamingMessage
) => {
  const release = await streamingMessagesMutex.current.acquire();
  try {
    const current = streamingMessages.current.get(messageId);
    const updated = updater(current);
    streamingMessages.current.set(messageId, updated);
    
    // Trigger React update
    setStreamingState(prev => ({
      ...prev,
      [messageId]: updated.text
    }));
  } finally {
    release();
  }
};

// Usage in event handlers
case "agent-stream-start":
  await updateStreamingMessage(payload.message_id, () => ({
    text: "",
    chunks: [],
    sequence: 0,
    lastUpdate: Date.now(),
    done: false
  }));
  break;

case "agent-text-stream":
  await updateStreamingMessage(payload.message_id, (current) => {
    if (!current) {
      console.warn(`Received chunk for unknown message: ${payload.message_id}`);
      return {
        text: payload.chunk,
        chunks: [payload.chunk],
        sequence: payload.sequence || 0,
        lastUpdate: Date.now(),
        done: false
      };
    }
    
    // Handle out-of-order chunks using sequence numbers
    const newChunks = [...current.chunks];
    if (payload.sequence !== undefined) {
      newChunks[payload.sequence] = payload.chunk;
    } else {
      newChunks.push(payload.chunk);
    }
    
    return {
      ...current,
      text: newChunks.join(''),
      chunks: newChunks,
      sequence: payload.sequence || current.sequence + 1,
      lastUpdate: Date.now()
    };
  });
  break;
```

#### Solution B: Event Queue Pattern

```typescript
interface StreamEvent {
  type: 'start' | 'chunk' | 'end' | 'error';
  messageId: string;
  payload: any;
  timestamp: number;
  sequence?: number;
}

const eventQueue = useRef<StreamEvent[]>([]);
const processingQueue = useRef(false);

const enqueueEvent = (event: StreamEvent) => {
  eventQueue.current.push(event);
  processQueue();
};

const processQueue = useCallback(async () => {
  if (processingQueue.current) return;
  processingQueue.current = true;
  
  while (eventQueue.current.length > 0) {
    const event = eventQueue.current.shift()!;
    await processStreamEvent(event);
  }
  
  processingQueue.current = false;
}, []);

const processStreamEvent = async (event: StreamEvent) => {
  switch (event.type) {
    case 'start':
      streamingMessages.current.set(event.messageId, {
        text: '',
        chunks: [],
        done: false
      });
      break;
    case 'chunk':
      // Process chunk safely
      break;
  }
};
```

### 2. App State Race Conditions

**Problem**: Multiple state updates from concurrent events
**Location**: `/src/hooks/useAppState.ts`

#### Solution: Reducer Pattern with Action Queue

```typescript
// Define action types
type AppAction = 
  | { type: 'SET_PROCESSING'; payload: boolean }
  | { type: 'SET_SERVER_STATUS'; payload: ServerStatus }
  | { type: 'SET_MODAL'; payload: ModalType }
  | { type: 'BATCH_UPDATE'; payload: Partial<AppState> }
  | { type: 'ATOMIC_UPDATE'; updater: (state: AppState) => AppState };

// Implement thread-safe reducer
const [state, dispatch] = useReducer(appReducer, initialState);

// Action queue for ordered processing
const actionQueue = useRef<AppAction[]>([]);
const actionQueueMutex = useRef(new Mutex());

const queuedDispatch = useCallback(async (action: AppAction) => {
  const release = await actionQueueMutex.current.acquire();
  try {
    actionQueue.current.push(action);
    
    // Process all queued actions
    while (actionQueue.current.length > 0) {
      const nextAction = actionQueue.current.shift()!;
      dispatch(nextAction);
    }
  } finally {
    release();
  }
}, [dispatch]);

// Atomic state updates
const atomicUpdate = useCallback((
  updater: (state: AppState) => Partial<AppState>
) => {
  queuedDispatch({
    type: 'ATOMIC_UPDATE',
    updater: (state) => ({ ...state, ...updater(state) })
  });
}, [queuedDispatch]);

// Batch updates for related state
const batchUpdate = useCallback((updates: Partial<AppState>) => {
  queuedDispatch({
    type: 'BATCH_UPDATE',
    payload: updates
  });
}, [queuedDispatch]);
```

### 3. Audio Playback Race Conditions

**Problem**: Audio element accessed in multiple async contexts
**Location**: `/src/hooks/useAudioPlayback.ts`

#### Solution: State Machine Pattern

```typescript
type AudioState = 
  | { status: 'idle' }
  | { status: 'loading'; url: string; abortController: AbortController }
  | { status: 'playing'; audio: HTMLAudioElement; url: string }
  | { status: 'paused'; audio: HTMLAudioElement; url: string }
  | { status: 'error'; error: Error; lastUrl?: string };

interface AudioAction {
  type: 'LOAD' | 'PLAY' | 'PAUSE' | 'STOP' | 'ERROR' | 'CLEANUP';
  payload?: any;
}

const audioStateMachine = {
  idle: {
    LOAD: async (state: AudioState, url: string): Promise<AudioState> => {
      const abortController = new AbortController();
      return { status: 'loading', url, abortController };
    }
  },
  loading: {
    PLAY: async (state: AudioState): Promise<AudioState> => {
      if (state.status !== 'loading') return state;
      
      try {
        const audio = new Audio(state.url);
        
        // Cleanup handler
        const cleanup = () => {
          audio.pause();
          audio.src = '';
          audio.load();
          URL.revokeObjectURL(state.url);
        };
        
        // Set up event handlers with proper cleanup
        const playPromise = new Promise<void>((resolve, reject) => {
          audio.oncanplaythrough = () => resolve();
          audio.onerror = () => reject(new Error('Audio failed to load'));
          
          state.abortController.signal.addEventListener('abort', () => {
            cleanup();
            reject(new Error('Audio loading aborted'));
          });
        });
        
        await playPromise;
        await audio.play();
        
        return { status: 'playing', audio, url: state.url };
      } catch (error) {
        return { status: 'error', error: error as Error, lastUrl: state.url };
      }
    },
    STOP: async (state: AudioState): Promise<AudioState> => {
      if (state.status === 'loading') {
        state.abortController.abort();
      }
      return { status: 'idle' };
    }
  },
  playing: {
    PAUSE: async (state: AudioState): Promise<AudioState> => {
      if (state.status === 'playing') {
        state.audio.pause();
        return { ...state, status: 'paused' };
      }
      return state;
    },
    STOP: async (state: AudioState): Promise<AudioState> => {
      if (state.status === 'playing') {
        state.audio.pause();
        state.audio.src = '';
        URL.revokeObjectURL(state.url);
        return { status: 'idle' };
      }
      return state;
    }
  }
};

// Usage with mutex for state transitions
const audioStateMutex = useRef(new Mutex());
const [audioState, setAudioState] = useState<AudioState>({ status: 'idle' });

const transitionAudio = async (action: AudioAction) => {
  const release = await audioStateMutex.current.acquire();
  try {
    const currentStatus = audioState.status;
    const transition = audioStateMachine[currentStatus]?.[action.type];
    
    if (transition) {
      const newState = await transition(audioState, action.payload);
      setAudioState(newState);
    }
  } finally {
    release();
  }
};
```

### 4. Event Handler Coordination

**Problem**: Multiple event handlers without coordination
**Solution**: Event Orchestrator Pattern

```typescript
class EventOrchestrator {
  private eventQueue: PriorityQueue<QueuedEvent>;
  private processing: boolean = false;
  private handlers: Map<string, EventHandler[]>;
  private mutex: Mutex;
  
  constructor() {
    this.eventQueue = new PriorityQueue();
    this.handlers = new Map();
    this.mutex = new Mutex();
  }
  
  async emit(event: AppEvent) {
    const release = await this.mutex.acquire();
    try {
      this.eventQueue.enqueue({
        ...event,
        timestamp: Date.now(),
        sequence: this.getNextSequence()
      });
    } finally {
      release();
    }
    
    this.processQueue();
  }
  
  private async processQueue() {
    if (this.processing) return;
    this.processing = true;
    
    while (!this.eventQueue.isEmpty()) {
      const event = this.eventQueue.dequeue();
      await this.processEvent(event);
    }
    
    this.processing = false;
  }
  
  private async processEvent(event: QueuedEvent) {
    const handlers = this.handlers.get(event.type) || [];
    
    // Process handlers in order with error isolation
    for (const handler of handlers) {
      try {
        await handler(event);
      } catch (error) {
        console.error(`Handler error for ${event.type}:`, error);
        // Continue processing other handlers
      }
    }
  }
}
```

## 🟡 Medium Priority Solutions

### 5. Event Name Consolidation

**Problem**: 13 duplicate event types causing confusion
**Solution**: Unified Event Taxonomy

```typescript
// Define clear event namespaces
const EVENT_TYPES = {
  AGENT: {
    LIFECYCLE: {
      START: 'agent:lifecycle:start',
      STOP: 'agent:lifecycle:stop',  // Replaces 5 duplicate stop events
      ERROR: 'agent:lifecycle:error',
      STATE_CHANGE: 'agent:lifecycle:state-change'
    },
    STREAM: {
      START: 'agent:stream:start',
      CHUNK: 'agent:stream:chunk',
      END: 'agent:stream:end'
    }
  },
  DICTATION: {
    START: 'dictation:start',
    STOP: 'dictation:stop',  // Replaces 8 duplicate stop events
    TRANSCRIPTION: 'dictation:transcription',
    ERROR: 'dictation:error'
  },
  SYSTEM: {
    CONNECTION: 'system:connection',
    ERROR: 'system:error',
    METRIC: 'system:metric'
  }
} as const;

// Deprecation wrapper for backward compatibility
const createDeprecatedEventProxy = (oldEvent: string, newEvent: string) => {
  return (handler: Function) => {
    console.warn(`Event '${oldEvent}' is deprecated. Use '${newEvent}' instead.`);
    return on(newEvent, handler);
  };
};

// Map old events to new
const EVENT_MIGRATION_MAP = {
  'agent-stopping': EVENT_TYPES.AGENT.LIFECYCLE.STOP,
  'agent-stop-all': EVENT_TYPES.AGENT.LIFECYCLE.STOP,
  'agent-cancel': EVENT_TYPES.AGENT.LIFECYCLE.STOP,
  'agent-force-stop': EVENT_TYPES.AGENT.LIFECYCLE.STOP,
  'agent-force-cleanup': EVENT_TYPES.AGENT.LIFECYCLE.STOP,
  // ... more mappings
};
```

### 6. Memory Leak Prevention

**Problem**: Event listeners without cleanup
**Solution**: Automatic Cleanup Manager

```typescript
class EventCleanupManager {
  private cleanupFunctions: Map<string, Set<() => void>> = new Map();
  
  register(componentId: string, cleanup: () => void) {
    if (!this.cleanupFunctions.has(componentId)) {
      this.cleanupFunctions.set(componentId, new Set());
    }
    this.cleanupFunctions.get(componentId)!.add(cleanup);
  }
  
  cleanup(componentId: string) {
    const cleanups = this.cleanupFunctions.get(componentId);
    if (cleanups) {
      cleanups.forEach(cleanup => cleanup());
      this.cleanupFunctions.delete(componentId);
    }
  }
  
  cleanupAll() {
    this.cleanupFunctions.forEach((cleanups, componentId) => {
      this.cleanup(componentId);
    });
  }
}

// Usage in React components
const useEventWithCleanup = (
  event: string, 
  handler: Function,
  deps: any[] = []
) => {
  const cleanupManager = useContext(CleanupContext);
  const componentId = useId();
  
  useEffect(() => {
    const unsubscribe = on(event, handler);
    cleanupManager.register(componentId, unsubscribe);
    
    return () => {
      unsubscribe();
      cleanupManager.cleanup(componentId);
    };
  }, deps);
};
```

## 🚀 Implementation Roadmap

### Phase 1: Critical Fixes (Week 1)
1. Implement mutex for streaming messages
2. Add sequence numbers to streaming events
3. Create atomic state update mechanism
4. Fix audio state machine

### Phase 2: Architecture Improvements (Week 2)
1. Consolidate duplicate events
2. Implement event queue pattern
3. Add automatic cleanup manager
4. Create migration guides

### Phase 3: Testing & Monitoring (Week 3)
1. Add race condition tests
2. Implement performance monitoring
3. Create stress test suite
4. Document new patterns

### Phase 4: Deployment (Week 4)
1. Gradual rollout with feature flags
2. Monitor production metrics
3. Collect performance data
4. Address any issues

## Testing Strategy

### Race Condition Tests

```typescript
describe('Streaming Message Race Conditions', () => {
  it('should handle concurrent chunk updates', async () => {
    const messageId = 'test-123';
    const chunks = Array.from({ length: 100 }, (_, i) => `chunk-${i}`);
    
    // Fire all chunks concurrently
    await Promise.all(
      chunks.map((chunk, i) => 
        eventBus.emit('agent-text-stream', {
          message_id: messageId,
          chunk,
          sequence: i
        })
      )
    );
    
    // Verify all chunks were processed in order
    const result = getStreamingMessage(messageId);
    expect(result.text).toBe(chunks.join(''));
  });
  
  it('should handle interleaved start/chunk/end events', async () => {
    // Test complex event ordering scenarios
  });
});
```

## Monitoring & Metrics

```typescript
interface RaceConditionMetrics {
  eventQueueDepth: number;
  concurrentEventCount: number;
  mutexWaitTime: number;
  eventProcessingTime: number;
  outOfOrderEvents: number;
  droppedEvents: number;
}

const metricsCollector = new MetricsCollector<RaceConditionMetrics>();

// Instrument critical sections
const instrumentedMutexAcquire = async (mutex: Mutex, name: string) => {
  const startTime = Date.now();
  const release = await mutex.acquire();
  const waitTime = Date.now() - startTime;
  
  metricsCollector.record('mutexWaitTime', waitTime);
  
  return release;
};
```

## Conclusion

These solutions address all critical race conditions while maintaining performance and code clarity. The phased implementation approach ensures minimal disruption to existing functionality while providing immediate safety improvements for the most critical issues.