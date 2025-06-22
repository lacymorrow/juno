# Timer-Expired Event Handler Implementation Plan

## 🎯 **Objective**

Implement complete timer-expired event handling to enable agent resumption with context restoration for use cases like chess games, page monitoring, and user interruption recovery.

## 🚨 **Current Gap Analysis**

- ✅ Timer system emits `"timer-expired"` events correctly
- ❌ **No event handlers exist to process timer-expired events**
- ❌ **Agent restart with context restoration not implemented**
- ❌ **No integration with agent orchestration system**

## 📋 **Implementation Plan**

### **Phase 1: Core Event Handler Implementation**

#### **1.1 Create Timer Event Handler Module**

**File**: `src-tauri/src/events/timer_handlers.rs`

**Purpose**: Dedicated module for timer-expired event processing

**Key Components**:

```rust
pub struct TimerEventHandler {
    app_handle: AppHandle,
}

impl TimerEventHandler {
    pub fn new(app_handle: AppHandle) -> Self { ... }
    
    pub async fn handle_timer_expired(&self, timer_data: TimerTask) -> Result<(), AgentError> { ... }
    
    async fn should_restart_agent(&self, timer_data: &TimerTask) -> Result<bool, AgentError> { ... }
    
    async fn restart_agent_with_context(&self, context: &Value, description: &str) -> Result<(), AgentError> { ... }
    
    async fn queue_timer_for_later(&self, timer_data: TimerTask) -> Result<(), AgentError> { ... }
}
```

**Edge Cases Handled**:

- Agent currently running query
- Multiple timer expirations simultaneously
- Invalid or corrupted context data
- Agent system unavailable
- Memory/resource constraints

#### **1.2 Agent State Detection System**

**Purpose**: Determine if agent is currently active before attempting restart

**Implementation**:

```rust
pub enum AgentSystemState {
    Idle,
    ProcessingQuery(String),           // Current query being processed
    WaitingForUserInput,              // Waiting for continuation approval
    ProcessingTimerRestart(String),   // Already processing a timer restart
    SystemError(String),              // Error state
    Shutdown,                         // System shutting down
}

pub async fn get_agent_system_state(app_handle: &AppHandle) -> Result<AgentSystemState, AgentError> {
    // Check AppState for current agent activity
    // Check continuation manager for pending requests
    // Check if anthropic::submit_query is currently running
}
```

#### **1.3 Context Validation and Restoration**

**Purpose**: Ensure timer context is valid and can be safely restored

**Validation Checks**:

- JSON schema validation for context structure
- Required fields presence (conversation_history, task_state, etc.)
- Context size limits (prevent memory exhaustion)
- Timestamp freshness (prevent stale context restoration)
- Security validation (prevent malicious context injection)

**Context Structure**:

```json
{
  "timer_id": "uuid",
  "timer_type": "simple|screen_monitor|file_monitor|app_monitor",
  "conversation_history": [...],
  "task_state": {
    "current_action": "...",
    "completed_steps": [...],
    "next_steps": [...],
    "variables": {...}
  },
  "user_preferences": {...},
  "environment_state": {...},
  "created_at": "timestamp",
  "expires_at": "timestamp"
}
```

### **Phase 2: Agent Orchestration Integration**

#### **2.1 Orchestrator Enhancement**

**File**: `src-tauri/src/commands/orchestrator.rs`

**New Functions**:

```rust
pub async fn handle_timer_based_restart(
    context: Value,
    description: String,
    app_handle: AppHandle,
    state: tauri::State<'_, AppState>
) -> Result<String, AgentError> {
    // Validate context
    // Check system state
    // Queue or execute restart
    // Return execution ID
}

pub async fn queue_delayed_restart(
    timer_data: TimerTask,
    retry_count: u32,
    app_handle: AppHandle,
    state: tauri::State<'_, AppState>
) -> Result<(), AgentError> {
    // Queue timer for later processing when agent becomes available
}
```

#### **2.2 Memory Manager Integration**

**File**: `src-tauri/src/commands/memory.rs`

**Context Restoration**:

- Restore conversation history from timer context
- Merge with existing memory if agent has ongoing context
- Handle memory conflicts (timer context vs current context)
- Apply memory optimization and pruning

### **Phase 3: Event System Integration**

#### **3.1 Event Handler Registration**

**File**: `src-tauri/src/events/handlers.rs`

**Add Timer Event Listener**:

```rust
pub fn setup_event_listeners(app: &AppHandle) {
    setup_voice_transcription_listeners(app);
    setup_dictation_listeners(app);
    setup_timer_event_listeners(app); // NEW
}

fn setup_timer_event_listeners(app: &AppHandle) {
    let app_handle_for_timer = app.clone();
    app.listen("timer-expired", move |event| {
        let app_handle = app_handle_for_timer.clone();
        tauri::async_runtime::spawn(async move {
            handle_timer_expired_event(app_handle, event.payload()).await;
        });
    });
}

async fn handle_timer_expired_event(app_handle: AppHandle, payload: &str) {
    // Parse timer data from payload
    // Create TimerEventHandler
    // Process timer expiration
    // Handle all edge cases
}
```

#### **3.2 Integration with Existing Event System**

**File**: `src-tauri/src/integration.rs`

**Enhanced Integration**:

- Coordinate timer events with voice transcription events
- Handle conflicts between timer restart and user voice input
- Manage event priority (user input > timer expiration)
- Prevent event listener accumulation

### **Phase 4: Edge Case Handling**

#### **4.1 Concurrent Agent Execution Prevention**

**Scenario**: Timer expires while agent is processing user query

**Solution**:

```rust
async fn handle_concurrent_execution(
    timer_data: &TimerTask,
    current_execution_id: &str
) -> Result<TimerHandlingStrategy, AgentError> {
    match timer_data.timer_type {
        TimerType::Simple => {
            // Queue for later - simple delays can wait
            Ok(TimerHandlingStrategy::QueueForLater)
        },
        TimerType::ScreenMonitor { .. } => {
            // High priority - screen changes might be time-sensitive
            Ok(TimerHandlingStrategy::InterruptCurrent)
        },
        TimerType::FileMonitor { .. } => {
            // Medium priority - check file event urgency
            Ok(TimerHandlingStrategy::QueueWithPriority)
        },
        TimerType::ApplicationMonitor { .. } => {
            // Context-dependent priority
            Ok(TimerHandlingStrategy::EvaluateContext)
        }
    }
}

enum TimerHandlingStrategy {
    QueueForLater,
    InterruptCurrent,
    QueueWithPriority,
    EvaluateContext,
    DiscardExpired,
}
```

#### **4.2 Multiple Timer Expiration Handling**

**Scenario**: Multiple timers expire simultaneously

**Solution**:

- Priority queue based on timer type and creation time
- Batch processing with rate limiting
- Context merging for related timers
- Conflict resolution for competing contexts

```rust
pub struct TimerExpirationQueue {
    pending_timers: Arc<Mutex<BinaryHeap<PrioritizedTimer>>>,
    processing_rate_limiter: Arc<Mutex<RateLimiter>>,
}

struct PrioritizedTimer {
    timer: TimerTask,
    priority: u8,
    expires_at: SystemTime,
}
```

#### **4.3 Context Data Corruption Handling**

**Scenario**: Timer context contains invalid or corrupted data

**Validation Pipeline**:

1. **JSON Schema Validation**: Ensure structure integrity
2. **Size Limits**: Prevent memory exhaustion (max 10MB context)
3. **Field Validation**: Required fields present and valid types
4. **Security Scan**: Detect potential injection attempts
5. **Freshness Check**: Reject contexts older than 24 hours
6. **Compatibility Check**: Ensure context matches current agent version

**Fallback Strategy**:

```rust
async fn handle_corrupted_context(
    timer_id: &str,
    error: &ContextValidationError
) -> Result<RecoveryAction, AgentError> {
    match error {
        ContextValidationError::InvalidJson => {
            // Try to repair common JSON issues
            Ok(RecoveryAction::AttemptRepair)
        },
        ContextValidationError::MissingRequired => {
            // Use default context with timer description
            Ok(RecoveryAction::UseMinimalContext)
        },
        ContextValidationError::SecurityViolation => {
            // Discard timer and log security event
            Ok(RecoveryAction::DiscardAndAlert)
        },
        ContextValidationError::TooLarge => {
            // Truncate context to essential data
            Ok(RecoveryAction::TruncateContext)
        }
    }
}
```

#### **4.4 System Resource Management**

**Memory Management**:

- Context size limits per timer (10MB max)
- Total timer context memory limit (100MB max)
- Automatic cleanup of expired contexts
- Memory pressure detection and response

**CPU Management**:

- Rate limiting for timer processing (max 5 concurrent)
- Background processing for non-urgent timers
- Priority-based resource allocation
- Timeout protection for context restoration

#### **4.5 User Interruption Scenarios**

**Scenario 1**: User starts voice input while timer is restarting agent

**Solution**:

```rust
async fn handle_user_interruption_during_timer_restart(
    app_handle: &AppHandle,
    timer_execution_id: &str
) -> Result<(), AgentError> {
    // Cancel timer-based restart
    // Prioritize user input
    // Save timer context for later retry
    // Clean up partial restart state
}
```

**Scenario 2**: User manually stops agent that was started by timer

**Solution**:

- Detect manual stop events
- Clean up timer-related state
- Prevent timer retry loops
- Update timer status to cancelled

#### **4.6 System State Edge Cases**

**Application Shutdown**:

```rust
async fn handle_shutdown_with_pending_timers(
    app_handle: &AppHandle
) -> Result<(), AgentError> {
    // Cancel all active timer processing
    // Save pending timers to persistent storage
    // Clean up resources
    // Prevent new timer processing
}
```

**Low Memory Conditions**:

```rust
async fn handle_low_memory_conditions(
    app_handle: &AppHandle
) -> Result<(), AgentError> {
    // Reduce context sizes
    // Pause non-critical timers
    // Clear expired contexts
    // Emit memory pressure warnings
}
```

### **Phase 5: Testing and Validation**

#### **5.1 Unit Tests**

**Timer Event Handler Tests**:

- Valid timer expiration processing
- Invalid context handling
- Concurrent execution scenarios
- Memory limit enforcement
- Security validation

**Integration Tests**:

- Timer expiration during active agent query
- Multiple simultaneous timer expirations
- Context restoration accuracy
- Memory management under load
- Event listener registration/cleanup

#### **5.2 End-to-End Testing Scenarios**

**Chess Game Scenario**:

1. Start chess game with agent
2. Set 30-second timer with game state context
3. Agent becomes idle
4. Timer expires
5. Verify agent restarts with chess context
6. Verify game state restoration accuracy

**Page Monitoring Scenario**:

1. Set up screen monitor for webpage changes
2. Start unrelated agent task
3. Page changes during agent task
4. Verify screen monitor queues for later
5. Verify agent restarts with monitoring context after task completion

**User Interruption Scenario**:

1. Set timer for agent restart
2. Timer expires and starts agent restart
3. User starts voice input during restart
4. Verify user input takes priority
5. Verify timer context saved for later retry

#### **5.3 Performance Testing**

**Load Testing**:

- 100 simultaneous timer expirations
- Large context sizes (near 10MB limit)
- Memory pressure conditions
- CPU resource contention

**Stress Testing**:

- Malformed context data
- Network interruptions during restart
- Rapid timer creation/cancellation
- System resource exhaustion

### **Phase 6: Error Handling and Monitoring**

#### **6.1 Comprehensive Error Handling**

**Error Categories**:

```rust
#[derive(Debug, thiserror::Error)]
pub enum TimerEventError {
    #[error("Invalid timer context: {0}")]
    InvalidContext(String),
    
    #[error("Agent system unavailable: {0}")]
    AgentUnavailable(String),
    
    #[error("Context validation failed: {0}")]
    ContextValidation(String),
    
    #[error("Resource limit exceeded: {0}")]
    ResourceLimit(String),
    
    #[error("Concurrent execution conflict: {0}")]
    ConcurrencyConflict(String),
    
    #[error("System state error: {0}")]
    SystemState(String),
}
```

**Error Recovery Strategies**:

- Automatic retry with exponential backoff
- Context repair attempts
- Fallback to minimal context
- User notification for critical failures

#### **6.2 Monitoring and Observability**

**Metrics Collection**:

- Timer expiration processing times
- Context restoration success rates
- Memory usage patterns
- Error frequencies by type
- User interruption patterns

**Logging Strategy**:

```rust
// Structured logging for timer events
info!(
    timer_id = %timer.id,
    timer_type = ?timer.timer_type,
    context_size = context.len(),
    processing_time_ms = elapsed.as_millis(),
    "Timer expired and processed successfully"
);

warn!(
    timer_id = %timer.id,
    error = %error,
    retry_count = retry_count,
    "Timer processing failed, scheduling retry"
);
```

**Health Checks**:

- Timer system responsiveness
- Context validation pipeline health
- Memory usage monitoring
- Event listener registration status

### **Phase 7: Documentation and User Experience**

#### **7.1 API Documentation**

**Timer Context Structure Documentation**:

- Required fields and formats
- Size limits and recommendations
- Security considerations
- Best practices for context design

**Tool Usage Examples**:

```rust
// Chess game timer example
set_timer({
    "delay_seconds": 30,
    "context": {
        "conversation_history": [...],
        "game_state": {
            "board": "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR",
            "current_player": "white",
            "move_history": [...],
            "game_phase": "opening"
        },
        "task_state": {
            "current_action": "analyze_position",
            "thinking_time_used": 45,
            "candidate_moves": [...]
        }
    },
    "description": "Resume chess game analysis after opponent's move"
});
```

#### **7.2 Error Messages and User Feedback**

**User-Friendly Error Messages**:

- Clear explanation of what went wrong
- Suggested actions for resolution
- Context about impact on user's workflow

**Progress Indicators**:

- Timer expiration notifications
- Context restoration progress
- Agent restart status updates

### **Phase 8: Security Considerations**

#### **8.1 Context Security Validation**

**Security Checks**:

- Prevent code injection in context data
- Validate file paths in context
- Sanitize user input stored in context
- Encrypt sensitive context data

**Access Control**:

- Verify context ownership
- Prevent cross-user context access
- Audit context access patterns

#### **8.2 Rate Limiting and Abuse Prevention**

**Rate Limits**:

- Max 10 timer expirations per minute per user
- Max 100MB total context storage per user
- Max 5 concurrent timer processing operations

**Abuse Detection**:

- Unusual timer creation patterns
- Excessive context sizes
- Rapid timer expiration cycles
- Resource exhaustion attempts

### **Phase 9: Backward Compatibility**

#### **9.1 Existing Timer System Compatibility**

**Ensure No Breaking Changes**:

- Existing timer tools continue to work
- Timer creation API remains unchanged
- Event emission format stays consistent
- Context structure is backward compatible

#### **9.2 Migration Strategy**

**For Existing Timers**:

- Detect legacy context formats
- Automatically upgrade context structure
- Maintain compatibility mode
- Gradual migration to new format

### **Phase 10: Deployment and Rollout**

#### **10.1 Feature Flags**

**Gradual Rollout**:

```rust
pub struct TimerEventConfig {
    pub enabled: bool,
    pub max_concurrent_processing: u32,
    pub context_size_limit_mb: u32,
    pub enable_context_validation: bool,
    pub enable_security_scanning: bool,
}
```

#### **10.2 Rollback Plan**

**Rollback Triggers**:

- High error rates (>5%)
- Memory usage spikes
- User complaints about interruptions
- System instability

**Rollback Process**:

- Disable timer event processing
- Revert to timer emission only
- Clean up partial state
- Notify users of temporary limitation

## 🔧 **Implementation Order**

1. **Phase 1**: Core event handler (2-3 days)
2. **Phase 4**: Critical edge cases (2-3 days)
3. **Phase 3**: Event system integration (1-2 days)
4. **Phase 2**: Orchestrator integration (1-2 days)
5. **Phase 5**: Testing and validation (2-3 days)
6. **Phase 6**: Error handling and monitoring (1-2 days)
7. **Phase 7-10**: Documentation, security, deployment (2-3 days)

**Total Estimated Time**: 11-17 days

## 🎯 **Success Criteria**

- ✅ Timer-expired events trigger agent restart with context
- ✅ Chess game scenario works end-to-end
- ✅ Page monitoring scenario works end-to-end
- ✅ User interruption handling works correctly
- ✅ No memory leaks or resource exhaustion
- ✅ Error rate < 1% under normal conditions
- ✅ Performance impact < 5% on existing functionality
- ✅ All existing timer functionality continues to work

## 🚨 **Risk Mitigation**

**High-Risk Areas**:

1. **Memory Management**: Implement strict limits and monitoring
2. **Concurrency Issues**: Use proper locking and state management
3. **User Experience**: Ensure timer restarts don't disrupt user workflow
4. **Security**: Validate all context data thoroughly
5. **Performance**: Monitor resource usage and implement rate limiting

**Mitigation Strategies**:

- Extensive testing with edge cases
- Feature flags for gradual rollout
- Comprehensive monitoring and alerting
- Clear rollback procedures
- User feedback collection and response
