# DotDot Event Architecture Implementation Guide

## Executive Summary

This guide provides step-by-step instructions for implementing the solutions to fix critical race conditions and refactor the event-driven architecture in the DotDot application.

## 🚨 Priority 1: Critical Race Condition Fixes

### Step 1: Install Dependencies

```bash
npm install async-mutex zod recast
npm install --save-dev @types/async-mutex
```

### Step 2: Create Thread-Safe Utilities

Create `/src/lib/thread-safe-utils.ts`:

```typescript
import { Mutex } from 'async-mutex';

export class ThreadSafeMap<K, V> {
  private map = new Map<K, V>();
  private mutex = new Mutex();
  
  async get(key: K): Promise<V | undefined> {
    const release = await this.mutex.acquire();
    try {
      return this.map.get(key);
    } finally {
      release();
    }
  }
  
  async set(key: K, value: V): Promise<void> {
    const release = await this.mutex.acquire();
    try {
      this.map.set(key, value);
    } finally {
      release();
    }
  }
  
  async update(key: K, updater: (current: V | undefined) => V): Promise<V> {
    const release = await this.mutex.acquire();
    try {
      const current = this.map.get(key);
      const updated = updater(current);
      this.map.set(key, updated);
      return updated;
    } finally {
      release();
    }
  }
  
  async delete(key: K): Promise<boolean> {
    const release = await this.mutex.acquire();
    try {
      return this.map.delete(key);
    } finally {
      release();
    }
  }
  
  async clear(): Promise<void> {
    const release = await this.mutex.acquire();
    try {
      this.map.clear();
    } finally {
      release();
    }
  }
}

export class ThreadSafeQueue<T> {
  private queue: T[] = [];
  private mutex = new Mutex();
  
  async enqueue(item: T): Promise<void> {
    const release = await this.mutex.acquire();
    try {
      this.queue.push(item);
    } finally {
      release();
    }
  }
  
  async dequeue(): Promise<T | undefined> {
    const release = await this.mutex.acquire();
    try {
      return this.queue.shift();
    } finally {
      release();
    }
  }
  
  async peek(): Promise<T | undefined> {
    const release = await this.mutex.acquire();
    try {
      return this.queue[0];
    } finally {
      release();
    }
  }
  
  async size(): Promise<number> {
    const release = await this.mutex.acquire();
    try {
      return this.queue.length;
    } finally {
      release();
    }
  }
}
```

### Step 3: Fix useBackendEvents Hook

Update `/src/hooks/useBackendEvents.ts`:

```typescript
import { useRef, useCallback, useState, useEffect } from 'react';
import { ThreadSafeMap } from '@/lib/thread-safe-utils';

interface StreamingMessage {
  text: string;
  chunks: string[];
  sequence: number;
  lastUpdate: number;
  done: boolean;
}

export function useBackendEvents() {
  // Replace unsafe Map with ThreadSafeMap
  const streamingMessages = useRef(new ThreadSafeMap<string, StreamingMessage>());
  const [streamingState, setStreamingState] = useState<Record<string, string>>({});
  
  const handleEvent = useCallback(async (event: string, payload: any) => {
    switch (event) {
      case "agent-stream-start":
        await streamingMessages.current.set(payload.message_id, {
          text: "",
          chunks: [],
          sequence: 0,
          lastUpdate: Date.now(),
          done: false
        });
        
        setStreamingState(prev => ({
          ...prev,
          [payload.message_id]: ""
        }));
        break;
        
      case "agent-text-stream":
        const updated = await streamingMessages.current.update(
          payload.message_id,
          (current) => {
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
            
            // Handle out-of-order chunks
            const newChunks = [...current.chunks];
            if (payload.sequence !== undefined) {
              newChunks[payload.sequence] = payload.chunk;
            } else {
              newChunks.push(payload.chunk);
            }
            
            const text = newChunks.filter(Boolean).join('');
            
            return {
              ...current,
              text,
              chunks: newChunks,
              sequence: payload.sequence || current.sequence + 1,
              lastUpdate: Date.now()
            };
          }
        );
        
        setStreamingState(prev => ({
          ...prev,
          [payload.message_id]: updated.text
        }));
        break;
        
      case "agent-stream-end":
        await streamingMessages.current.update(
          payload.message_id,
          (current) => current ? { ...current, done: true } : current!
        );
        
        // Cleanup after a delay
        setTimeout(async () => {
          await streamingMessages.current.delete(payload.message_id);
          setStreamingState(prev => {
            const { [payload.message_id]: _, ...rest } = prev;
            return rest;
          });
        }, 5000);
        break;
    }
  }, []);
  
  // ... rest of the hook
}
```

### Step 4: Create State Management Solution

Create `/src/hooks/useAtomicState.ts`:

```typescript
import { useReducer, useCallback, useRef } from 'react';
import { Mutex } from 'async-mutex';

interface AtomicStateOptions<T> {
  onUpdate?: (newState: T, oldState: T) => void;
  debugName?: string;
}

export function useAtomicState<T>(
  initialState: T,
  options: AtomicStateOptions<T> = {}
) {
  const mutex = useRef(new Mutex());
  const [state, dispatch] = useReducer(
    (state: T, action: (state: T) => T) => {
      const newState = action(state);
      if (options.onUpdate && newState !== state) {
        options.onUpdate(newState, state);
      }
      return newState;
    },
    initialState
  );
  
  const atomicUpdate = useCallback(async (
    updater: (state: T) => T | Promise<T>
  ): Promise<T> => {
    const release = await mutex.current.acquire();
    try {
      return new Promise<T>((resolve) => {
        dispatch(async (currentState) => {
          const newState = await updater(currentState);
          resolve(newState);
          return newState;
        });
      });
    } finally {
      release();
    }
  }, []);
  
  const batchUpdate = useCallback(async (
    updates: Array<(state: T) => T | Promise<T>>
  ): Promise<T> => {
    const release = await mutex.current.acquire();
    try {
      let currentState = state;
      
      for (const update of updates) {
        currentState = await update(currentState);
      }
      
      dispatch(() => currentState);
      return currentState;
    } finally {
      release();
    }
  }, [state]);
  
  return {
    state,
    atomicUpdate,
    batchUpdate,
    isUpdating: mutex.current.isLocked()
  };
}
```

### Step 5: Implement Audio State Machine

Create `/src/hooks/useAudioStateMachine.ts`:

```typescript
import { useState, useCallback, useRef } from 'react';
import { Mutex } from 'async-mutex';

type AudioState = 
  | { status: 'idle' }
  | { status: 'loading'; url: string; abortController: AbortController }
  | { status: 'playing'; audio: HTMLAudioElement; url: string }
  | { status: 'paused'; audio: HTMLAudioElement; url: string }
  | { status: 'error'; error: Error; lastUrl?: string };

export function useAudioStateMachine() {
  const [state, setState] = useState<AudioState>({ status: 'idle' });
  const mutex = useRef(new Mutex());
  
  const transition = useCallback(async (
    action: 'LOAD' | 'PLAY' | 'PAUSE' | 'STOP' | 'ERROR',
    payload?: any
  ): Promise<void> => {
    const release = await mutex.current.acquire();
    try {
      switch (state.status) {
        case 'idle':
          if (action === 'LOAD' && payload?.url) {
            const abortController = new AbortController();
            setState({ status: 'loading', url: payload.url, abortController });
            
            // Start loading audio
            const audio = new Audio(payload.url);
            
            audio.addEventListener('canplaythrough', async () => {
              if (!abortController.signal.aborted) {
                await transition('PLAY', { audio });
              }
            });
            
            audio.addEventListener('error', async (e) => {
              if (!abortController.signal.aborted) {
                await transition('ERROR', { error: new Error('Audio load failed') });
              }
            });
          }
          break;
          
        case 'loading':
          if (action === 'PLAY' && payload?.audio) {
            try {
              await payload.audio.play();
              setState({ 
                status: 'playing', 
                audio: payload.audio, 
                url: state.url 
              });
            } catch (error) {
              setState({ 
                status: 'error', 
                error: error as Error, 
                lastUrl: state.url 
              });
            }
          } else if (action === 'STOP') {
            state.abortController.abort();
            setState({ status: 'idle' });
          }
          break;
          
        case 'playing':
          if (action === 'PAUSE') {
            state.audio.pause();
            setState({ ...state, status: 'paused' });
          } else if (action === 'STOP') {
            state.audio.pause();
            state.audio.src = '';
            URL.revokeObjectURL(state.url);
            setState({ status: 'idle' });
          }
          break;
          
        case 'paused':
          if (action === 'PLAY') {
            await state.audio.play();
            setState({ ...state, status: 'playing' });
          } else if (action === 'STOP') {
            state.audio.src = '';
            URL.revokeObjectURL(state.url);
            setState({ status: 'idle' });
          }
          break;
          
        case 'error':
          if (action === 'LOAD' && payload?.url) {
            setState({ status: 'idle' });
            await transition('LOAD', payload);
          }
          break;
      }
    } finally {
      release();
    }
  }, [state]);
  
  return {
    state,
    load: (url: string) => transition('LOAD', { url }),
    play: () => transition('PLAY'),
    pause: () => transition('PAUSE'),
    stop: () => transition('STOP')
  };
}
```

## 🔄 Priority 2: Event System Refactoring

### Step 1: Create New Event System

Create `/src/events/event-system.ts`:

```typescript
export const EVENTS = {
  AGENT: {
    LIFECYCLE: {
      START: 'agent:lifecycle:start',
      STOP: 'agent:lifecycle:stop',
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
    LIFECYCLE: {
      START: 'dictation:lifecycle:start',
      STOP: 'dictation:lifecycle:stop',
      ERROR: 'dictation:lifecycle:error'
    },
    TRANSCRIPTION: {
      PARTIAL: 'dictation:transcription:partial',
      FINAL: 'dictation:transcription:final'
    }
  },
  SYSTEM: {
    CONNECTION: {
      CONNECT: 'system:connection:connect',
      DISCONNECT: 'system:connection:disconnect',
      ERROR: 'system:connection:error'
    }
  }
} as const;

// Migration map for backward compatibility
export const EVENT_MIGRATION_MAP = new Map([
  ['agent-stopping', EVENTS.AGENT.LIFECYCLE.STOP],
  ['agent-stop-all', EVENTS.AGENT.LIFECYCLE.STOP],
  ['agent-cancel', EVENTS.AGENT.LIFECYCLE.STOP],
  ['agent-force-stop', EVENTS.AGENT.LIFECYCLE.STOP],
  ['agent-force-cleanup', EVENTS.AGENT.LIFECYCLE.STOP],
  ['dictation-stop', EVENTS.DICTATION.LIFECYCLE.STOP],
  ['dictation-cancelled', EVENTS.DICTATION.LIFECYCLE.STOP],
  ['dictation-transcription-stop', EVENTS.DICTATION.LIFECYCLE.STOP],
  ['dictation-transcription-cancel', EVENTS.DICTATION.LIFECYCLE.STOP]
]);
```

### Step 2: Create Compatibility Layer

Create `/src/events/compatibility.ts`:

```typescript
import { EVENT_MIGRATION_MAP } from './event-system';

export function wrapEventEmitter(emitter: any) {
  const originalEmit = emitter.emit?.bind(emitter) || emitter.trigger?.bind(emitter);
  const originalOn = emitter.on?.bind(emitter) || emitter.listen?.bind(emitter);
  
  if (originalEmit) {
    emitter.emit = emitter.trigger = (event: string, ...args: any[]) => {
      const newEvent = EVENT_MIGRATION_MAP.get(event);
      if (newEvent) {
        console.warn(
          `⚠️ Event '${event}' is deprecated. Use '${newEvent}' instead.`,
          '\nThis event will be removed in the next major version.'
        );
        
        // Transform payload if needed
        const payload = transformPayload(event, args[0]);
        return originalEmit(newEvent, payload, ...args.slice(1));
      }
      return originalEmit(event, ...args);
    };
  }
  
  if (originalOn) {
    emitter.on = emitter.listen = (event: string, handler: Function) => {
      const newEvent = EVENT_MIGRATION_MAP.get(event);
      if (newEvent) {
        console.warn(
          `⚠️ Listening to deprecated event '${event}'. Use '${newEvent}' instead.`
        );
        return originalOn(newEvent, handler);
      }
      return originalOn(event, handler);
    };
  }
  
  return emitter;
}

function transformPayload(oldEvent: string, payload: any): any {
  switch (oldEvent) {
    case 'agent-stopping':
      return {
        agentId: payload.agent_id,
        reason: 'user',
        force: false,
        scope: 'single'
      };
    case 'agent-stop-all':
      return {
        agentId: payload.agent_id,
        reason: 'user',
        force: true,
        scope: 'all'
      };
    case 'agent-force-stop':
      return {
        agentId: payload.agent_id,
        reason: 'system',
        force: true,
        scope: 'single'
      };
    default:
      return payload;
  }
}
```

### Step 3: Update Constants File

Update `/src-tauri/src/constants/events.rs`:

```rust
// New event constants with clear naming
pub mod events {
    // Agent lifecycle events
    pub const AGENT_LIFECYCLE_START: &str = "agent:lifecycle:start";
    pub const AGENT_LIFECYCLE_STOP: &str = "agent:lifecycle:stop";
    pub const AGENT_LIFECYCLE_ERROR: &str = "agent:lifecycle:error";
    pub const AGENT_LIFECYCLE_STATE_CHANGE: &str = "agent:lifecycle:state-change";
    
    // Agent stream events
    pub const AGENT_STREAM_START: &str = "agent:stream:start";
    pub const AGENT_STREAM_CHUNK: &str = "agent:stream:chunk";
    pub const AGENT_STREAM_END: &str = "agent:stream:end";
    
    // Dictation events
    pub const DICTATION_LIFECYCLE_START: &str = "dictation:lifecycle:start";
    pub const DICTATION_LIFECYCLE_STOP: &str = "dictation:lifecycle:stop";
    pub const DICTATION_TRANSCRIPTION_PARTIAL: &str = "dictation:transcription:partial";
    pub const DICTATION_TRANSCRIPTION_FINAL: &str = "dictation:transcription:final";
    
    // Deprecated events (to be removed)
    #[deprecated(since = "2.0.0", note = "Use AGENT_LIFECYCLE_STOP instead")]
    pub const AGENT_STOPPING: &str = "agent-stopping";
    
    #[deprecated(since = "2.0.0", note = "Use AGENT_LIFECYCLE_STOP instead")]
    pub const AGENT_STOP_ALL: &str = "agent-stop-all";
}
```

## 📋 Implementation Checklist

### Week 1: Critical Fixes
- [ ] Install required dependencies
- [ ] Implement ThreadSafeMap and ThreadSafeQueue
- [ ] Fix useBackendEvents streaming message race condition
- [ ] Implement atomic state management
- [ ] Create audio state machine
- [ ] Add mutex protection to cloud connector

### Week 2: Event System
- [ ] Define new event taxonomy
- [ ] Create compatibility layer
- [ ] Update Rust event constants
- [ ] Implement event migration warnings
- [ ] Create automated migration script

### Week 3: Testing
- [ ] Write race condition tests
- [ ] Create event migration tests
- [ ] Add performance benchmarks
- [ ] Implement stress testing
- [ ] Document breaking changes

### Week 4: Deployment
- [ ] Enable feature flags
- [ ] Deploy to staging
- [ ] Monitor deprecated event usage
- [ ] Collect performance metrics
- [ ] Plan production rollout

## 🧪 Testing Examples

### Race Condition Test

```typescript
import { renderHook, act } from '@testing-library/react-hooks';
import { useBackendEvents } from '@/hooks/useBackendEvents';

describe('useBackendEvents race conditions', () => {
  it('should handle concurrent stream chunks', async () => {
    const { result } = renderHook(() => useBackendEvents());
    
    // Simulate 100 concurrent chunks
    const promises = Array.from({ length: 100 }, (_, i) => 
      act(async () => {
        await result.current.handleEvent('agent-text-stream', {
          message_id: 'test-123',
          chunk: `chunk-${i}`,
          sequence: i
        });
      })
    );
    
    await Promise.all(promises);
    
    // Verify all chunks were processed in order
    const state = result.current.streamingState;
    expect(state['test-123']).toBe(
      Array.from({ length: 100 }, (_, i) => `chunk-${i}`).join('')
    );
  });
});
```

## 🚀 Deployment Strategy

### Feature Flags

```typescript
// feature-flags.ts
export const FEATURES = {
  NEW_EVENT_SYSTEM: 'new-event-system',
  THREAD_SAFE_HOOKS: 'thread-safe-hooks',
  AUDIO_STATE_MACHINE: 'audio-state-machine'
} as const;

export function isFeatureEnabled(feature: string): boolean {
  // Check environment variable
  const envFlag = process.env[`REACT_APP_FEATURE_${feature.toUpperCase()}`];
  if (envFlag !== undefined) {
    return envFlag === 'true';
  }
  
  // Check localStorage for development
  if (typeof window !== 'undefined') {
    const localFlag = localStorage.getItem(`feature:${feature}`);
    if (localFlag !== null) {
      return localFlag === 'true';
    }
  }
  
  // Default values
  const defaults: Record<string, boolean> = {
    [FEATURES.NEW_EVENT_SYSTEM]: false,
    [FEATURES.THREAD_SAFE_HOOKS]: true, // Enable critical fixes by default
    [FEATURES.AUDIO_STATE_MACHINE]: true
  };
  
  return defaults[feature] ?? false;
}
```

## 📊 Monitoring Dashboard

Create a simple monitoring dashboard to track the fixes:

```typescript
// monitoring-dashboard.tsx
export function MonitoringDashboard() {
  const metrics = useEventMetrics();
  
  return (
    <div className="monitoring-dashboard">
      <h2>Event System Health</h2>
      
      <section>
        <h3>Deprecated Event Usage</h3>
        {metrics.deprecatedEvents.map(event => (
          <div key={event.name}>
            {event.name}: {event.count} calls
            <progress value={event.count} max={1000} />
          </div>
        ))}
      </section>
      
      <section>
        <h3>Race Condition Metrics</h3>
        <div>Mutex wait time: {metrics.avgMutexWaitTime}ms</div>
        <div>Out-of-order events: {metrics.outOfOrderEvents}</div>
        <div>Event queue depth: {metrics.eventQueueDepth}</div>
      </section>
      
      <section>
        <h3>Performance</h3>
        <div>Events/second: {metrics.eventsPerSecond}</div>
        <div>Processing time: {metrics.avgProcessingTime}ms</div>
      </section>
    </div>
  );
}
```

## Conclusion

This implementation guide provides a complete roadmap for fixing the critical issues in the DotDot event-driven architecture. By following these steps, you'll eliminate race conditions, consolidate duplicate events, and create a more maintainable and reliable system.

Remember to:
1. Test thoroughly at each step
2. Monitor performance impacts
3. Communicate changes to the team
4. Keep backward compatibility during migration
5. Document any breaking changes