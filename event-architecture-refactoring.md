# Event-Driven Architecture Refactoring Guide

## Overview

This document provides a comprehensive refactoring strategy for the DotDot event-driven architecture, addressing the 13 duplicate event types, improving synchronization, and enhancing overall system reliability.

## 🏗️ New Event Architecture Design

### Core Principles

1. **Single Responsibility**: Each event type has one clear purpose
2. **Namespace Organization**: Hierarchical event naming for clarity
3. **Type Safety**: Full TypeScript support with discriminated unions
4. **Backward Compatibility**: Graceful migration from old events
5. **Performance First**: Optimized for high-throughput scenarios

### Event Naming Convention

```typescript
// Format: domain:category:action
// Examples:
// - agent:lifecycle:start
// - dictation:audio:chunk
// - system:metric:update

type EventDomain = 'agent' | 'dictation' | 'system' | 'ui' | 'voice';
type EventCategory = 'lifecycle' | 'stream' | 'audio' | 'metric' | 'error';
type EventAction = string;

type EventType = `${EventDomain}:${EventCategory}:${EventAction}`;
```

## 🔄 Event Consolidation Strategy

### Before: 13 Duplicate Events
```typescript
// Agent stop events (5 duplicates)
'agent-stopping'
'agent-stop-all'
'agent-cancel'
'agent-force-stop'
'agent-force-cleanup'

// Dictation stop events (8 duplicates)
'dictation-stop'
'dictation-cancelled'
'dictation-transcription-stop'
'dictation-transcription-cancel'
'dictation-transcription-force-stop'
'dictation-transcription-force-cleanup'
'voice-transcription:dictation-stopped'
'plugin:voice-transcription:dictation-stopped'
```

### After: Unified Events with Context
```typescript
interface StopEventPayload {
  reason: 'user' | 'error' | 'timeout' | 'system';
  force: boolean;
  scope: 'single' | 'all';
  metadata?: Record<string, any>;
}

// Single event with context
emit('agent:lifecycle:stop', {
  agentId: 'agent-123',
  reason: 'user',
  force: false,
  scope: 'single'
});

emit('dictation:lifecycle:stop', {
  sessionId: 'dictation-456',
  reason: 'user',
  force: false,
  scope: 'single'
});
```

## 🎯 Complete Event Taxonomy

```typescript
export const EVENTS = {
  // Agent Events
  AGENT: {
    LIFECYCLE: {
      START: 'agent:lifecycle:start',
      STOP: 'agent:lifecycle:stop',
      ERROR: 'agent:lifecycle:error',
      STATE_CHANGE: 'agent:lifecycle:state-change',
      READY: 'agent:lifecycle:ready'
    },
    STREAM: {
      START: 'agent:stream:start',
      CHUNK: 'agent:stream:chunk',
      END: 'agent:stream:end',
      ERROR: 'agent:stream:error'
    },
    TOOL: {
      START: 'agent:tool:start',
      COMPLETE: 'agent:tool:complete',
      ERROR: 'agent:tool:error'
    }
  },
  
  // Dictation Events
  DICTATION: {
    LIFECYCLE: {
      START: 'dictation:lifecycle:start',
      STOP: 'dictation:lifecycle:stop',
      ERROR: 'dictation:lifecycle:error'
    },
    AUDIO: {
      CHUNK: 'dictation:audio:chunk',
      LEVEL: 'dictation:audio:level',
      SILENCE: 'dictation:audio:silence'
    },
    TRANSCRIPTION: {
      PARTIAL: 'dictation:transcription:partial',
      FINAL: 'dictation:transcription:final',
      ERROR: 'dictation:transcription:error'
    }
  },
  
  // System Events
  SYSTEM: {
    CONNECTION: {
      CONNECT: 'system:connection:connect',
      DISCONNECT: 'system:connection:disconnect',
      ERROR: 'system:connection:error'
    },
    METRIC: {
      UPDATE: 'system:metric:update',
      THRESHOLD: 'system:metric:threshold'
    },
    ERROR: {
      CRITICAL: 'system:error:critical',
      WARNING: 'system:error:warning',
      INFO: 'system:error:info'
    }
  },
  
  // UI Events
  UI: {
    MODAL: {
      OPEN: 'ui:modal:open',
      CLOSE: 'ui:modal:close'
    },
    NOTIFICATION: {
      SHOW: 'ui:notification:show',
      DISMISS: 'ui:notification:dismiss'
    }
  },
  
  // Voice Events
  VOICE: {
    TTS: {
      START: 'voice:tts:start',
      CHUNK: 'voice:tts:chunk',
      END: 'voice:tts:end',
      ERROR: 'voice:tts:error'
    }
  }
} as const;
```

## 🔧 TypeScript Integration

### Event Type Definitions

```typescript
// Base event interface
interface BaseEvent<T extends EventType, P = unknown> {
  type: T;
  payload: P;
  timestamp: number;
  sequence: number;
  correlationId?: string;
  metadata?: EventMetadata;
}

interface EventMetadata {
  source: string;
  version: string;
  userId?: string;
  sessionId?: string;
  traceId?: string;
}

// Specific event types
type AgentStartEvent = BaseEvent<
  typeof EVENTS.AGENT.LIFECYCLE.START,
  {
    agentId: string;
    config: AgentConfig;
    parentId?: string;
  }
>;

type AgentStopEvent = BaseEvent<
  typeof EVENTS.AGENT.LIFECYCLE.STOP,
  StopEventPayload & { agentId: string }
>;

// Union of all events
type AppEvent = 
  | AgentStartEvent
  | AgentStopEvent
  | DictationStartEvent
  | DictationStopEvent
  // ... all other events

// Type-safe event emitter
class TypedEventEmitter {
  emit<E extends AppEvent>(event: E): void;
  on<E extends AppEvent>(
    type: E['type'],
    handler: (event: E) => void | Promise<void>
  ): () => void;
}
```

### Event Payload Validation

```typescript
import { z } from 'zod';

// Define schemas for event payloads
const StopEventSchema = z.object({
  reason: z.enum(['user', 'error', 'timeout', 'system']),
  force: z.boolean(),
  scope: z.enum(['single', 'all']),
  metadata: z.record(z.any()).optional()
});

const AgentStartSchema = z.object({
  agentId: z.string(),
  config: z.object({
    model: z.string(),
    temperature: z.number().min(0).max(2),
    maxTokens: z.number().positive()
  }),
  parentId: z.string().optional()
});

// Validate events before emission
function validateEvent<T>(schema: z.Schema<T>, payload: unknown): T {
  return schema.parse(payload);
}
```

## 🔄 Migration Strategy

### Phase 1: Compatibility Layer

```typescript
// Create backward compatibility wrapper
class EventMigrationLayer {
  private migrationMap = new Map<string, string>([
    ['agent-stopping', EVENTS.AGENT.LIFECYCLE.STOP],
    ['agent-stop-all', EVENTS.AGENT.LIFECYCLE.STOP],
    ['agent-cancel', EVENTS.AGENT.LIFECYCLE.STOP],
    // ... all mappings
  ]);
  
  private payloadTransformers = new Map<string, (old: any) => any>([
    ['agent-stopping', (payload) => ({
      agentId: payload.agent_id,
      reason: 'user',
      force: false,
      scope: 'single'
    })],
    ['agent-stop-all', (payload) => ({
      agentId: payload.agent_id,
      reason: 'user',
      force: true,
      scope: 'all'
    })]
  ]);
  
  wrap(eventBus: EventBus): EventBus {
    const originalEmit = eventBus.emit.bind(eventBus);
    const originalOn = eventBus.on.bind(eventBus);
    
    eventBus.emit = (event: string, payload: any) => {
      const newEvent = this.migrationMap.get(event);
      if (newEvent) {
        console.warn(`Event '${event}' is deprecated. Using '${newEvent}'`);
        const transformer = this.payloadTransformers.get(event);
        const newPayload = transformer ? transformer(payload) : payload;
        return originalEmit(newEvent, newPayload);
      }
      return originalEmit(event, payload);
    };
    
    eventBus.on = (event: string, handler: Function) => {
      const newEvent = this.migrationMap.get(event);
      if (newEvent) {
        console.warn(`Listening to deprecated event '${event}'. Use '${newEvent}'`);
        return originalOn(newEvent, handler);
      }
      return originalOn(event, handler);
    };
    
    return eventBus;
  }
}
```

### Phase 2: Gradual Migration

```typescript
// Feature flag for new event system
const useNewEventSystem = getFeatureFlag('new-event-system');

// Conditional event emission
function emitStopEvent(agentId: string, reason: string) {
  if (useNewEventSystem) {
    emit(EVENTS.AGENT.LIFECYCLE.STOP, {
      agentId,
      reason: reason as any,
      force: false,
      scope: 'single'
    });
  } else {
    emit('agent-stopping', { agent_id: agentId });
  }
}

// Deprecation warnings with timeline
function checkDeprecatedEvents() {
  const deprecationDate = new Date('2025-03-01');
  const now = new Date();
  
  if (now < deprecationDate) {
    console.warn(
      `Deprecated events will be removed on ${deprecationDate.toDateString()}`
    );
  } else {
    throw new Error('Deprecated events have been removed. Please update your code.');
  }
}
```

### Phase 3: Automated Migration Tools

```typescript
// Code modification tool
import { parse, visit, print } from 'recast';
import * as fs from 'fs/promises';

async function migrateEventUsage(filePath: string) {
  const code = await fs.readFile(filePath, 'utf-8');
  const ast = parse(code);
  
  visit(ast, {
    visitCallExpression(path) {
      const { callee, arguments: args } = path.node;
      
      // Check for emit() or on() calls
      if (
        callee.type === 'Identifier' && 
        (callee.name === 'emit' || callee.name === 'on') &&
        args[0]?.type === 'StringLiteral'
      ) {
        const oldEvent = args[0].value;
        const newEvent = migrationMap.get(oldEvent);
        
        if (newEvent) {
          // Replace old event with new
          args[0].value = newEvent;
          
          // Add migration comment
          path.insertBefore(
            `// TODO: Migrated from '${oldEvent}' - verify payload`
          );
        }
      }
      
      this.traverse(path);
    }
  });
  
  const modifiedCode = print(ast).code;
  await fs.writeFile(filePath, modifiedCode);
}
```

## 📊 Performance Optimizations

### Event Batching

```typescript
class BatchedEventEmitter extends EventEmitter {
  private batch: AppEvent[] = [];
  private batchTimeout: NodeJS.Timeout | null = null;
  private batchSize = 100;
  private batchDelay = 10; // ms
  
  emit(event: AppEvent) {
    this.batch.push(event);
    
    if (this.batch.length >= this.batchSize) {
      this.flush();
    } else if (!this.batchTimeout) {
      this.batchTimeout = setTimeout(() => this.flush(), this.batchDelay);
    }
  }
  
  private flush() {
    if (this.batchTimeout) {
      clearTimeout(this.batchTimeout);
      this.batchTimeout = null;
    }
    
    if (this.batch.length === 0) return;
    
    const events = [...this.batch];
    this.batch = [];
    
    // Process batch
    super.emit('batch', events);
  }
}
```

### Event Priority Queue

```typescript
interface PrioritizedEvent extends AppEvent {
  priority: 'low' | 'normal' | 'high' | 'critical';
}

class PriorityEventQueue {
  private queues = {
    critical: [] as PrioritizedEvent[],
    high: [] as PrioritizedEvent[],
    normal: [] as PrioritizedEvent[],
    low: [] as PrioritizedEvent[]
  };
  
  enqueue(event: PrioritizedEvent) {
    this.queues[event.priority].push(event);
  }
  
  dequeue(): PrioritizedEvent | null {
    for (const priority of ['critical', 'high', 'normal', 'low'] as const) {
      if (this.queues[priority].length > 0) {
        return this.queues[priority].shift()!;
      }
    }
    return null;
  }
  
  get size(): number {
    return Object.values(this.queues)
      .reduce((sum, queue) => sum + queue.length, 0);
  }
}
```

## 🧪 Testing Strategy

### Event Contract Tests

```typescript
describe('Event Contracts', () => {
  it('should validate agent stop event payload', () => {
    const validPayload = {
      agentId: 'test-123',
      reason: 'user',
      force: false,
      scope: 'single'
    };
    
    expect(() => StopEventSchema.parse(validPayload)).not.toThrow();
  });
  
  it('should reject invalid stop reason', () => {
    const invalidPayload = {
      agentId: 'test-123',
      reason: 'invalid-reason', // Should be enum value
      force: false,
      scope: 'single'
    };
    
    expect(() => StopEventSchema.parse(invalidPayload)).toThrow();
  });
});
```

### Event Flow Integration Tests

```typescript
describe('Event Flow', () => {
  it('should handle agent lifecycle correctly', async () => {
    const events: AppEvent[] = [];
    const eventBus = new TypedEventEmitter();
    
    eventBus.on('*', (event) => events.push(event));
    
    // Start agent
    eventBus.emit({
      type: EVENTS.AGENT.LIFECYCLE.START,
      payload: { agentId: 'test-123', config: testConfig },
      timestamp: Date.now(),
      sequence: 1
    });
    
    // Stop agent
    eventBus.emit({
      type: EVENTS.AGENT.LIFECYCLE.STOP,
      payload: { agentId: 'test-123', reason: 'user', force: false, scope: 'single' },
      timestamp: Date.now(),
      sequence: 2
    });
    
    expect(events).toHaveLength(2);
    expect(events[0].type).toBe(EVENTS.AGENT.LIFECYCLE.START);
    expect(events[1].type).toBe(EVENTS.AGENT.LIFECYCLE.STOP);
  });
});
```

## 📈 Monitoring & Observability

### Event Metrics

```typescript
interface EventMetrics {
  eventType: string;
  count: number;
  avgProcessingTime: number;
  errors: number;
  lastEmitted: number;
}

class EventMetricsCollector {
  private metrics = new Map<string, EventMetrics>();
  
  recordEvent(event: AppEvent, processingTime: number, error?: Error) {
    const metric = this.metrics.get(event.type) || {
      eventType: event.type,
      count: 0,
      avgProcessingTime: 0,
      errors: 0,
      lastEmitted: 0
    };
    
    metric.count++;
    metric.avgProcessingTime = 
      (metric.avgProcessingTime * (metric.count - 1) + processingTime) / metric.count;
    if (error) metric.errors++;
    metric.lastEmitted = Date.now();
    
    this.metrics.set(event.type, metric);
  }
  
  getMetrics(): EventMetrics[] {
    return Array.from(this.metrics.values());
  }
  
  getDeprecatedEventUsage(): EventMetrics[] {
    return this.getMetrics()
      .filter(m => deprecatedEvents.includes(m.eventType))
      .sort((a, b) => b.count - a.count);
  }
}
```

## 🚀 Implementation Timeline

### Week 1: Foundation
- [ ] Define new event taxonomy
- [ ] Create TypeScript types
- [ ] Implement validation schemas
- [ ] Set up compatibility layer

### Week 2: Migration Tools
- [ ] Build automated migration scripts
- [ ] Create deprecation warnings
- [ ] Update documentation
- [ ] Test compatibility layer

### Week 3: Gradual Rollout
- [ ] Deploy behind feature flag
- [ ] Monitor deprecated event usage
- [ ] Update high-traffic components
- [ ] Collect performance metrics

### Week 4: Completion
- [ ] Complete migration of all components
- [ ] Remove deprecated events
- [ ] Update all tests
- [ ] Final performance optimization

## Conclusion

This refactoring addresses all identified issues while maintaining backward compatibility. The new architecture provides:

1. **Clear event semantics** - No more confusion about which event to use
2. **Type safety** - Full TypeScript support with validation
3. **Performance** - Optimized for high-throughput scenarios
4. **Observability** - Built-in metrics and monitoring
5. **Migration path** - Smooth transition from old to new system