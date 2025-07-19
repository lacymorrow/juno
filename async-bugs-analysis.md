# Async/Await Bug Analysis Report

## Executive Summary
After analyzing 83 TypeScript/TSX files and 195 Rust files containing async patterns, I've identified several critical areas where async/await bugs and unhandled promise rejections could occur in the dotdot codebase.

## Key Findings

### 1. Missing Error Boundaries for Async Operations

**Issue**: While `AsyncErrorBoundary` component exists, it's not consistently used across all async operations.

**Location**: `/src/components/AsyncErrorBoundary.tsx`
- The component properly handles unhandled promise rejections
- However, many components with async operations don't use this boundary

**Affected Files**:
- `/src/App.tsx` - Multiple invoke() calls without error boundaries
- `/src/components/onboarding/Onboarding.tsx` - Async permission checks
- Various hooks making async calls

### 2. Unhandled Promise Rejections in Event Listeners

**Issue**: Event listeners in `useBackendEvents.ts` have try-catch blocks but some async operations inside callbacks might fail silently.

**Location**: `/src/hooks/useBackendEvents.ts`
```typescript
// Line 72-73: Potential unhandled rejection
if (payload.response.audio_base64) {
  await playAudioFromBase64(payload.response.audio_base64);
}
```

**Risk**: Audio playback failures could leave the UI in an inconsistent state.

### 3. Race Conditions in State Updates

**Issue**: Multiple concurrent state updates without proper synchronization.

**Location**: `/src/hooks/useAppStateSync.ts`
- Lines 123-183: Massive Promise.all() with 30+ concurrent API calls
- No handling for partial failures
- State could be partially updated if some calls fail

**Recommendation**: Implement transaction-like behavior or use atomic state updates.

### 4. Missing Try-Catch in Critical Paths

**Issue**: Several invoke() calls lack proper error handling.

**Locations**:
- `/src/App.tsx` (Line 63): `invoke(COMMANDS.AGENT_NOTIFY_QUERY_SUBMITTED)`
- `/src/App.tsx` (Line 233): Similar pattern in example prompt handler
- Multiple instances where errors are only logged, not handled

### 5. Async Operations in useEffect Without Cleanup

**Issue**: Async operations initiated in useEffect hooks without proper cleanup.

**Example**: `/src/hooks/useBackendEvents.ts`
```typescript
useEffect(() => {
  const setupEventListeners = async () => {
    // Async setup without tracking if component is still mounted
  };
  setupEventListeners();
  return () => {
    // Cleanup doesn't wait for async operations to complete
  };
}, []);
```

### 6. Rust Side: Tokio Spawn Without Join Handles

**Issue**: Fire-and-forget tokio::spawn calls without tracking tasks.

**Location**: `/src-tauri/src/dictation_monitor.rs` (Line 38)
```rust
tokio::spawn(async move {
    monitor.run(rx).await;
});
```

**Risk**: Tasks could continue running after app shutdown, causing resource leaks.

### 7. Event-Driven System Race Conditions

**Issue**: Complex event chains without proper sequencing guarantees.

**Location**: `/src-tauri/src/agent/tools/event_executor.rs`
- Async tool execution emits multiple events
- No guarantee of event ordering
- Potential for out-of-order event processing

### 8. Cache Invalidation Issues

**Issue**: Settings cache with TTL but no invalidation on updates.

**Location**: `/src/hooks/useSettings.ts`
- 30-second cache TTL
- Updates might not be reflected immediately
- Potential for stale data bugs

## Critical Bug Patterns Identified

### Pattern 1: Unguarded Async Calls
```typescript
// BAD: No error handling
await invoke(COMMANDS.SOME_COMMAND, { data });

// GOOD: Proper error handling
try {
  await invoke(COMMANDS.SOME_COMMAND, { data });
} catch (error) {
  handleError(error);
  // Update UI state appropriately
}
```

### Pattern 2: Missing Mounted Checks
```typescript
// BAD: Async operation without mounted check
useEffect(() => {
  async function fetchData() {
    const data = await api.getData();
    setState(data); // Component might be unmounted!
  }
  fetchData();
}, []);

// GOOD: Check if component is still mounted
useEffect(() => {
  let mounted = true;
  async function fetchData() {
    const data = await api.getData();
    if (mounted) setState(data);
  }
  fetchData();
  return () => { mounted = false; };
}, []);
```

### Pattern 3: Promise.all Without Error Boundaries
```typescript
// BAD: All-or-nothing approach
const results = await Promise.all([...many promises...]);

// GOOD: Handle partial failures
const results = await Promise.allSettled([...many promises...]);
results.forEach((result, index) => {
  if (result.status === 'rejected') {
    handleFailure(index, result.reason);
  }
});
```

## Severity Classification

### 🔴 Critical (Immediate Action Required)
1. `useAppStateSync.ts` - Promise.all with 30+ operations
2. Missing error boundaries in App.tsx
3. Unhandled audio playback promises

### 🟡 High (Should Fix Soon)
1. Event listener async operations without error handling
2. Tokio spawn without join handles
3. Cache invalidation issues in settings

### 🟢 Medium (Plan for Future)
1. Missing mounted checks in various hooks
2. Event ordering guarantees in event-driven system
3. Cleanup of async operations in useEffect

## Recommendations

1. **Implement Global Async Error Handler**: Wrap the entire app in AsyncErrorBoundary
2. **Use Promise.allSettled**: Replace Promise.all with Promise.allSettled for better error resilience
3. **Add Mounted Checks**: Implement consistent mounted checks in all hooks with async operations
4. **Track Tokio Tasks**: Use JoinHandle or AbortHandle for all spawned tasks
5. **Implement Event Sequencing**: Add sequence numbers or timestamps to guarantee event ordering
6. **Add Retry Logic**: Implement exponential backoff for failed async operations
7. **Cache Invalidation**: Add event-based cache invalidation for settings

## Next Steps

1. Create unit tests for async error scenarios
2. Implement AsyncErrorBoundary wrapper at app root
3. Audit all invoke() calls and add proper error handling
4. Add ESLint rules to catch unhandled promises
5. Implement proper cleanup in all async hooks

## Files Requiring Immediate Attention

1. `/src/hooks/useAppStateSync.ts` - Critical Promise.all issue
2. `/src/App.tsx` - Multiple unhandled async calls
3. `/src/hooks/useBackendEvents.ts` - Audio playback error handling
4. `/src-tauri/src/dictation_monitor.rs` - Untracked tokio tasks
5. `/src/hooks/useSettings.ts` - Cache invalidation

---

*Report generated by Bug Hunter Agent*
*Total files analyzed: 278*
*Critical issues found: 8*
*Estimated fix time: 2-3 days*