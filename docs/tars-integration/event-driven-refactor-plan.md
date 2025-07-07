# Event-Driven Architecture Refactor Plan

## 🎯 Vision: Transform Juno into a Pure Event-Driven System

### Current State vs. Target State

#### Current (Hybrid) Architecture
```
User Input → submit_query() → execute_agent_internal() → AgentRunner.run()
    ↓               ↓                    ↓                     ↓
Events emitted  Events emitted    Events emitted      Events emitted
    ↓               ↓                    ↓                     ↓
Frontend receives events (but core flow is still imperative)
```

#### Target (Pure Event-Driven) Architecture
```
User Input → UserMessage Event
    ↓
Event Bus → Multiple Subscribers React
    ↓
AgentOrchestrator → AgentRunStart Event
    ↓
ToolCoordinator → ToolCall Event
    ↓
ToolExecutor → ToolResult Event
    ↓
ResponseGenerator → AssistantMessage Event
    ↓
All Components Subscribe → Reactive Updates
```

---

## 📋 Refactor Phases

### Phase 1.5: Event Bus Foundation
**Goal**: Create robust central event bus and subscription system

#### Implementation
1. **Enhanced Event Bus** (`src-tauri/src/agent/events/event_bus.rs`)
   ```rust
   pub struct EventBus {
       subscribers: Arc<RwLock<HashMap<String, Vec<Box<dyn EventHandler>>>>>,
       event_store: Arc<RwLock<Vec<JunoAgentEvent>>>,
       app_handle: AppHandle,
   }
   
   pub trait EventHandler: Send + Sync {
       async fn handle_event(&self, event: &JunoAgentEvent) -> Result<Vec<JunoAgentEvent>, String>;
       fn event_types(&self) -> Vec<&'static str>;
   }
   ```

2. **Event-Driven State Machine** (`src-tauri/src/agent/state_machine.rs`)
   ```rust
   pub enum AgentState {
       Idle,
       Processing { session_id: String },
       WaitingForTool { session_id: String, tool_call_id: String },
       Responding { session_id: String },
       Error { session_id: String, error: AgentError },
   }
   
   pub struct AgentStateMachine {
       current_state: Arc<RwLock<AgentState>>,
       event_bus: Arc<EventBus>,
   }
   ```

#### Files to Create/Modify
- ✅ Create: `src-tauri/src/agent/events/event_bus.rs`
- ✅ Create: `src-tauri/src/agent/state_machine.rs`
- 🔄 Modify: `src-tauri/src/agent/events/mod.rs`

### Phase 1.6: Agent Runner Refactor
**Goal**: Convert AgentRunner to pure event-driven state machine

#### Current Flow
```rust
// anthropic.rs
submit_query() → execute_agent_internal() → agent_runner.run()
```

#### Target Flow
```rust
// Event-driven flow
UserInputHandler::handle_query() → Emits UserMessage
AgentOrchestrator::on_user_message() → Emits AgentRunStart  
AgentRunner::on_agent_run_start() → Emits ToolCall
ToolCoordinator::on_tool_call() → Emits ToolResult
AgentRunner::on_tool_result() → Emits AssistantMessage
```

#### Implementation
1. **Event-Driven Agent Runner** (`src-tauri/src/agent/implementations/event_driven_runner.rs`)
   ```rust
   pub struct EventDrivenAgentRunner {
       brain: Arc<dyn AgentBrain>,
       state_machine: Arc<AgentStateMachine>,
       memory_manager: Arc<Mutex<dyn MemoryManager>>,
       event_bus: Arc<EventBus>,
   }
   
   impl EventHandler for EventDrivenAgentRunner {
       async fn handle_event(&self, event: &JunoAgentEvent) -> Result<Vec<JunoAgentEvent>, String> {
           match event {
               JunoAgentEvent::UserMessage { content, session_id, .. } => {
                   self.start_agent_run(content, session_id).await
               }
               JunoAgentEvent::ToolResult { tool_call_id, result, .. } => {
                   self.process_tool_result(tool_call_id, result).await
               }
               _ => Ok(vec![])
           }
       }
   }
   ```

2. **User Input Handler** (`src-tauri/src/agent/handlers/user_input.rs`)
   ```rust
   pub struct UserInputHandler {
       event_bus: Arc<EventBus>,
   }
   
   impl UserInputHandler {
       pub async fn handle_query(&self, query: String) -> Result<(), String> {
           let event = JunoAgentEvent::UserMessage {
               content: query,
               timestamp: now(),
               session_id: Some(generate_session_id()),
           };
           self.event_bus.emit(event).await
       }
   }
   ```

#### Files to Create/Modify
- ✅ Create: `src-tauri/src/agent/implementations/event_driven_runner.rs`
- ✅ Create: `src-tauri/src/agent/handlers/user_input.rs`
- ✅ Create: `src-tauri/src/agent/handlers/mod.rs`
- 🔄 Modify: `src-tauri/src/anthropic.rs` (simplified to just event emission)

### Phase 1.7: Tool System Refactor
**Goal**: Convert tool system to pure event-driven model

#### Current Flow
```rust
tool_provider.execute_tool(tool_call) → Result<ToolResult>
```

#### Target Flow
```rust
ToolCoordinator::on_tool_call() → Emits ToolExecutionStart
ToolExecutor::on_tool_execution_start() → Executes → Emits ToolResult
```

#### Implementation
1. **Tool Coordinator** (`src-tauri/src/agent/tools/coordinator.rs`)
   ```rust
   pub struct ToolCoordinator {
       tool_providers: HashMap<String, Arc<dyn ToolProvider>>,
       event_bus: Arc<EventBus>,
   }
   
   impl EventHandler for ToolCoordinator {
       async fn handle_event(&self, event: &JunoAgentEvent) -> Result<Vec<JunoAgentEvent>, String> {
           match event {
               JunoAgentEvent::ToolCall { tool_name, args, id, .. } => {
                   vec![JunoAgentEvent::ToolExecutionStart {
                       tool_name: tool_name.clone(),
                       tool_call_id: id.clone(),
                       timestamp: now(),
                   }]
               }
               _ => Ok(vec![])
           }
       }
   }
   ```

2. **Event-Driven Tool Executor** (`src-tauri/src/agent/tools/event_executor.rs`)
   ```rust
   pub struct EventDrivenToolExecutor {
       providers: HashMap<String, Arc<dyn ToolProvider>>,
       event_bus: Arc<EventBus>,
   }
   
   impl EventHandler for EventDrivenToolExecutor {
       async fn handle_event(&self, event: &JunoAgentEvent) -> Result<Vec<JunoAgentEvent>, String> {
           match event {
               JunoAgentEvent::ToolExecutionStart { tool_name, tool_call_id, .. } => {
                   let result = self.execute_tool_async(tool_name, tool_call_id).await;
                   vec![JunoAgentEvent::ToolResult { /* result */ }]
               }
               _ => Ok(vec![])
           }
       }
   }
   ```

#### Files to Create/Modify
- ✅ Create: `src-tauri/src/agent/tools/coordinator.rs`
- ✅ Create: `src-tauri/src/agent/tools/event_executor.rs`
- 🔄 Modify: `src-tauri/src/agent/implementations/tool_provider.rs` (remove direct execution)

### Phase 1.8: UI and State Management Refactor
**Goal**: Convert UI updates and state management to pure event subscription

#### Current Flow
```rust
// Direct state mutations
state.mark_agent_execution_started()
app_handle.emit("agent-active", true)
```

#### Target Flow
```rust
// Event-driven state updates
StateManager::on_agent_run_start() → Updates internal state
UIManager::on_agent_run_start() → Updates frontend
```

#### Implementation
1. **Event-Driven State Manager** (`src-tauri/src/state/event_driven_state.rs`)
   ```rust
   pub struct EventDrivenStateManager {
       state: Arc<RwLock<ApplicationState>>,
       event_bus: Arc<EventBus>,
   }
   
   impl EventHandler for EventDrivenStateManager {
       async fn handle_event(&self, event: &JunoAgentEvent) -> Result<Vec<JunoAgentEvent>, String> {
           match event {
               JunoAgentEvent::AgentRunStart { session_id, .. } => {
                   let mut state = self.state.write().await;
                   state.current_session = Some(session_id.clone());
                   state.agent_executing = true;
                   Ok(vec![])
               }
               _ => Ok(vec![])
           }
       }
   }
   ```

2. **Event-Driven UI Manager** (`src-tauri/src/ui/event_manager.rs`)
   ```rust
   pub struct EventDrivenUIManager {
       app_handle: AppHandle,
       event_bus: Arc<EventBus>,
   }
   
   impl EventHandler for EventDrivenUIManager {
       async fn handle_event(&self, event: &JunoAgentEvent) -> Result<Vec<JunoAgentEvent>, String> {
           // Forward all events to frontend
           self.app_handle.emit("agent-event", event)?;
           
           // Emit legacy events for backwards compatibility
           match event {
               JunoAgentEvent::AgentRunStart { .. } => {
                   self.app_handle.emit("agent-active", true)?;
               }
               _ => {}
           }
           
           Ok(vec![])
       }
   }
   ```

#### Files to Create/Modify
- ✅ Create: `src-tauri/src/state/event_driven_state.rs`
- ✅ Create: `src-tauri/src/ui/event_manager.rs`
- 🔄 Modify: `src-tauri/src/state.rs` (integrate event-driven state)

---

## 🚀 Implementation Timeline

### Week 1: Foundation (Phase 1.5)
- **Day 1**: Event Bus and State Machine
- **Day 2**: Testing and Integration

### Week 2: Core Refactor (Phase 1.6-1.7)
- **Day 3**: Agent Runner Refactor
- **Day 4**: Tool System Refactor
- **Day 5**: Integration Testing

### Week 3: UI and Finalization (Phase 1.8)
- **Day 6**: UI and State Management Refactor
- **Day 7**: End-to-End Testing and Bug Fixes

---

## 🧪 Testing Strategy

### Unit Tests
- Event Bus functionality
- State machine transitions
- Event handler isolation
- Error propagation

### Integration Tests
- Complete agent execution flow via events
- Tool execution via events
- UI updates via events
- Error recovery via events

### Performance Tests
- Event throughput
- Memory usage of event store
- Event handler performance

---

## 🎯 Success Criteria

1. **✅ Complete Agent Flow**: User query → Agent response entirely via events
2. **✅ Zero Direct Calls**: No component directly calls another (only via events)
3. **✅ Reactive UI**: Frontend updates purely via event subscription
4. **✅ Testable Components**: Each component can be tested in isolation
5. **✅ Performance**: No regression in agent execution speed
6. **✅ Reliability**: Error handling through event system

---

## 🔄 Migration Strategy

### Gradual Migration
1. **Phase 1.5**: Add event bus alongside existing system
2. **Phase 1.6**: Convert agent runner, keep tool system imperative
3. **Phase 1.7**: Convert tool system, keep direct state updates
4. **Phase 1.8**: Convert all state management to events

### Rollback Plan
- Keep existing code paths until new system is fully tested
- Feature flags to switch between imperative and event-driven modes
- Gradual removal of old code paths after confidence is built

### Backwards Compatibility
- Maintain existing Tauri command interfaces
- Keep legacy event emissions for frontend compatibility
- Gradual deprecation of old patterns

---

## 💡 Benefits Realization

### Immediate Benefits (After Phase 1.5)
- Better debugging through complete event traces
- Easier testing of individual components
- Foundation for real-time UI updates

### Medium-term Benefits (After Phase 1.6-1.7)
- Cleaner agent execution flow
- Easier to add new tool types
- Better error isolation and recovery

### Long-term Benefits (After Phase 1.8)
- Complete system observability
- Easy to add new features via event subscription
- Foundation for advanced analytics and monitoring
- Perfect setup for TARS Phases 2-6

This refactor will transform Juno from a "hybrid event system" into a "true event-driven architecture" that fully embodies TARS's design philosophy.