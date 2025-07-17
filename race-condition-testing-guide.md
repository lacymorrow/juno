# Race Condition Testing Quick Guide

## Running the Tests

### 1. Unit Tests (Fast)
```bash
# Run all race condition tests
npm test -- --testPathPattern=race-condition-tests.spec.ts

# Run with coverage
npm test -- --coverage --testPathPattern=race-condition-tests.spec.ts

# Run specific test suite
npm test -- --testNamePattern="Streaming Message Race Conditions"
```

### 2. Stress Tests (Longer)
```bash
# Run extended stress tests
npm run test:stress

# Run with memory profiling
npm run test:memory

# Run with performance monitoring
npm run test:performance
```

### 3. Integration Tests
```bash
# Run full integration suite
npm run test:integration

# Run with real Tauri backend
npm run test:e2e
```

## Key Test Scenarios

### Critical Race Conditions to Test

1. **Streaming Message Corruption**
   - Multiple chunks arriving out of order
   - Concurrent updates to same message ID
   - Stream end arriving before chunks

2. **State Synchronization Issues**
   - Rapid state transitions
   - Concurrent state updates
   - Event handler collisions

3. **Audio Resource Leaks**
   - Rapid play/stop cycles
   - Concurrent audio requests
   - Cleanup during playback

4. **Memory Leaks**
   - Long-running event streams
   - Accumulating event listeners
   - Unreleased blob URLs

## Monitoring During Tests

### Performance Metrics to Track
```javascript
// In your tests
const metrics = {
  eventProcessingTime: [],
  memoryUsage: [],
  concurrentEvents: 0,
  droppedEvents: 0,
  errorCount: 0
};
```

### Warning Signs
- Memory growth > 50MB/hour
- Event processing time > 100ms (p95)
- Dropped events > 0.1%
- Unhandled promise rejections

## CI/CD Integration

### GitHub Actions Workflow
```yaml
name: Race Condition Tests
on: [push, pull_request]

jobs:
  race-tests:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: actions/setup-node@v3
      - run: npm ci
      - run: npm run test:race-conditions
      - run: npm run test:stress -- --maxWorkers=1
      
  memory-tests:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: actions/setup-node@v3
      - run: npm ci
      - run: npm run test:memory-leaks
      - uses: actions/upload-artifact@v3
        if: failure()
        with:
          name: memory-profiles
          path: coverage/memory-*.json
```

## Test Development Tips

### 1. Simulating Race Conditions
```typescript
// Create timing variations
const randomDelay = () => Math.random() * 100;

// Concurrent execution
await Promise.all(events.map(e => 
  simulateEvent(e.type, e.payload, randomDelay())
));
```

### 2. Detecting Race Conditions
```typescript
// Track operation order
const operationLog = [];

// Verify expected order
expect(operationLog).toEqual(['start', 'process', 'complete']);
```

### 3. Stress Testing Patterns
```typescript
// High-frequency events
const interval = setInterval(() => {
  emit('test-event', { timestamp: Date.now() });
}, 1); // 1ms interval

// Cleanup
setTimeout(() => clearInterval(interval), testDuration);
```

## Common Issues and Solutions

### Issue: Tests Pass Individually but Fail Together
**Solution**: Tests may share state. Ensure proper cleanup:
```typescript
afterEach(() => {
  jest.clearAllMocks();
  cleanup(); // React Testing Library cleanup
  // Reset any global state
});
```

### Issue: Intermittent Test Failures
**Solution**: Add retry logic and increase timeouts:
```typescript
jest.retryTimes(3);
jest.setTimeout(30000);

// Use waitFor with longer timeout
await waitFor(() => {
  expect(condition).toBe(true);
}, { timeout: 5000 });
```

### Issue: Cannot Reproduce Production Race Conditions
**Solution**: Use production-like conditions:
```typescript
// Simulate network delays
const networkDelay = () => new Promise(r => 
  setTimeout(r, 50 + Math.random() * 200)
);

// Add CPU load
const cpuLoad = () => {
  const start = Date.now();
  while (Date.now() - start < 10) {
    // Busy wait
  }
};
```

## Debugging Race Conditions

### 1. Enable Detailed Logging
```typescript
// Add timestamps to all operations
console.log(`[${Date.now()}] Event: ${type}`, payload);
```

### 2. Use Chrome DevTools
- Performance tab for timing analysis
- Memory tab for leak detection
- Network tab for request ordering

### 3. Record and Replay
```typescript
// Record event sequence
const eventLog = [];
events.forEach(e => eventLog.push({ ...e, timestamp: Date.now() }));

// Save for analysis
fs.writeFileSync('event-log.json', JSON.stringify(eventLog));
```

## Production Monitoring

### Add Telemetry
```typescript
// Track race condition indicators
telemetry.track('event_collision', {
  eventType,
  concurrentCount,
  processingTime
});
```

### Set Up Alerts
- Alert on memory growth > threshold
- Alert on event processing delays
- Alert on increased error rates

## Next Steps

1. **Implement Missing Tests**
   - Add tests for identified race conditions
   - Cover edge cases not yet tested
   - Add chaos testing scenarios

2. **Improve Test Infrastructure**
   - Better event simulation tools
   - Automated performance regression detection
   - Visual test result dashboards

3. **Continuous Improvement**
   - Regular test review sessions
   - Update tests based on production issues
   - Share learnings with team

Remember: Race conditions are often intermittent. Run tests multiple times and under different system loads to increase confidence in your fixes.