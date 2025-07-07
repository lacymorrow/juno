# TARS Event-Driven Refactor: Implementation Guide

## 🚀 Quick Start Guide

### Prerequisites
- ✅ Phase 1 completed (events are being emitted)
- ✅ Compilation successful (`cargo check` passes)
- ✅ Basic understanding of event-driven patterns

### Implementation Order
**Critical**: Follow this exact order to maintain system stability during refactor.

---

## 🔧 Phase 1.5: Event Bus Foundation (Day 1-2)

### Step 1: Create Event Bus Core
```bash
# Create the event bus infrastructure
touch src-tauri/src/agent/events/event_bus.rs
```

**File: `src-tauri/src/agent/events/event_bus.rs`**
```rust
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use async_trait::async_trait;
use tauri::AppHandle;
use tracing::{debug, error, info, warn};

use super::JunoAgentEvent;

/// Trait for components that handle events
#[async_trait]
pub trait EventHandler: Send + Sync {
    /// Handle an event and optionally return new events to emit
    async fn handle_event(&self, event: &JunoAgentEvent) -> Result<Vec<JunoAgentEvent>, String>;
    
    /// Return the event types this handler cares about
    fn event_types(&self) -> Vec<&'static str>;
    
    /// Handler name for debugging
    fn name(&self) -> &'static str;
}

/// Central event bus for the application
pub struct EventBus {
    /// Event handlers organized by event type
    handlers: Arc<RwLock<HashMap<String, Vec<Arc<dyn EventHandler>>>>>,
    /// Event store for debugging and replay
    event_store: Arc<RwLock<Vec<JunoAgentEvent>>>,
    /// App handle for frontend communication
    app_handle: AppHandle,
}

impl EventBus {
    pub fn new(app_handle: AppHandle) -> Self {
        Self {
            handlers: Arc::new(RwLock::new(HashMap::new())),
            event_store: Arc::new(RwLock::new(Vec::new())),
            app_handle,
        }
    }
    
    /// Register an event handler
    pub async fn register_handler(&self, handler: Arc<dyn EventHandler>) {
        let mut handlers = self.handlers.write().await;
        
        for event_type in handler.event_types() {
            handlers
                .entry(event_type.to_string())
                .or_insert_with(Vec::new)
                .push(handler.clone());
        }
        
        info!("Registered event handler: {}", handler.name());
    }
    
    /// Emit an event and process it through all relevant handlers
    pub async fn emit(&self, event: JunoAgentEvent) -> Result<(), String> {
        // Store event for debugging
        {
            let mut store = self.event_store.write().await;
            store.push(event.clone());
            
            // Keep only last 1000 events
            if store.len() > 1000 {
                store.drain(0..store.len() - 1000);
            }
        }
        
        debug!("Emitting event: {}", event.event_type());
        
        // Get handlers for this event type
        let handlers = {
            let handlers_guard = self.handlers.read().await;
            handlers_guard
                .get(event.event_type())
                .cloned()
                .unwrap_or_default()
        };
        
        // Process event through each handler
        let mut new_events = Vec::new();
        for handler in handlers {
            match handler.handle_event(&event).await {
                Ok(mut events) => {
                    new_events.append(&mut events);
                }
                Err(e) => {
                    error!("Handler '{}' failed to process event: {}", handler.name(), e);
                }
            }
        }
        
        // Emit to frontend (preserve existing behavior)
        if let Err(e) = self.app_handle.emit("agent-event", &event) {
            warn!("Failed to emit event to frontend: {}", e);
        }
        
        // Recursively emit new events
        for new_event in new_events {
            self.emit(new_event).await?;
        }
        
        Ok(())
    }
    
    /// Get recent events for debugging
    pub async fn get_recent_events(&self, limit: usize) -> Vec<JunoAgentEvent> {
        let store = self.event_store.read().await;
        store.iter().rev().take(limit).rev().cloned().collect()
    }
}
```

### Step 2: Create State Machine
```bash
# Create the agent state machine
touch src-tauri/src/agent/state_machine.rs
```

**File: `src-tauri/src/agent/state_machine.rs`**
```rust
use std::sync::Arc;
use tokio::sync::RwLock;
use serde::{Serialize, Deserialize};
use tracing::{debug, info};

use crate::agent::core::AgentError;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AgentState {
    Idle,
    Processing { 
        session_id: String,
        current_step: u32,
        max_steps: u32,
    },
    WaitingForTool { 
        session_id: String, 
        tool_call_id: String,
        current_step: u32,
    },
    Responding { 
        session_id: String,
        current_step: u32,
    },
    Error { 
        session_id: String, 
        error: String,
        current_step: u32,
    },
    Completed {
        session_id: String,
        final_step: u32,
    },
    Cancelled {
        session_id: String,
        final_step: u32,
    },
}

impl AgentState {
    pub fn session_id(&self) -> Option<&str> {
        match self {
            AgentState::Idle => None,
            AgentState::Processing { session_id, .. } => Some(session_id),
            AgentState::WaitingForTool { session_id, .. } => Some(session_id),
            AgentState::Responding { session_id, .. } => Some(session_id),
            AgentState::Error { session_id, .. } => Some(session_id),
            AgentState::Completed { session_id, .. } => Some(session_id),
            AgentState::Cancelled { session_id, .. } => Some(session_id),
        }
    }
    
    pub fn current_step(&self) -> u32 {
        match self {
            AgentState::Idle => 0,
            AgentState::Processing { current_step, .. } => *current_step,
            AgentState::WaitingForTool { current_step, .. } => *current_step,
            AgentState::Responding { current_step, .. } => *current_step,
            AgentState::Error { current_step, .. } => *current_step,
            AgentState::Completed { final_step, .. } => *final_step,
            AgentState::Cancelled { final_step, .. } => *final_step,
        }
    }
}

pub struct AgentStateMachine {
    current_state: Arc<RwLock<AgentState>>,
}

impl AgentStateMachine {
    pub fn new() -> Self {
        Self {
            current_state: Arc::new(RwLock::new(AgentState::Idle)),
        }
    }
    
    pub async fn get_state(&self) -> AgentState {
        self.current_state.read().await.clone()
    }
    
    pub async fn transition_to(&self, new_state: AgentState) -> Result<(), String> {
        let mut state = self.current_state.write().await;
        let old_state = state.clone();
        
        // Validate transition
        if !self.is_valid_transition(&old_state, &new_state) {
            return Err(format!(
                "Invalid state transition from {:?} to {:?}",
                old_state, new_state
            ));
        }
        
        *state = new_state.clone();
        info!("Agent state transition: {:?} -> {:?}", old_state, new_state);
        
        Ok(())
    }
    
    fn is_valid_transition(&self, from: &AgentState, to: &AgentState) -> bool {
        use AgentState::*;
        
        match (from, to) {
            // From Idle
            (Idle, Processing { .. }) => true,
            
            // From Processing
            (Processing { .. }, WaitingForTool { .. }) => true,
            (Processing { .. }, Responding { .. }) => true,
            (Processing { .. }, Error { .. }) => true,
            (Processing { .. }, Cancelled { .. }) => true,
            
            // From WaitingForTool
            (WaitingForTool { .. }, Processing { .. }) => true,
            (WaitingForTool { .. }, Error { .. }) => true,
            (WaitingForTool { .. }, Cancelled { .. }) => true,
            
            // From Responding
            (Responding { .. }, Completed { .. }) => true,
            (Responding { .. }, Error { .. }) => true,
            (Responding { .. }, Cancelled { .. }) => true,
            
            // To Idle (reset)
            (_, Idle) => true,
            
            _ => false,
        }
    }
}
```

### Step 3: Update Module Exports
**File: `src-tauri/src/agent/events/mod.rs`**
```rust
//! Event-driven architecture for Juno's agent system

pub mod event_types;
pub mod event_processor;
pub mod event_bus;  // Add this

pub use event_types::{JunoAgentEvent, EventSubscriber, EventFilter};
pub use event_processor::{JunoEventStreamProcessor, EventProcessorConfig, LoggingSubscriber};
pub use event_bus::{EventBus, EventHandler};  // Add this

// Re-export commonly used types
pub use event_types::JunoAgentEvent as AgentEvent;
pub use event_processor::JunoEventStreamProcessor as EventProcessor;
```

**File: `src-tauri/src/agent/mod.rs`**
```rust
// Add this line
pub mod state_machine;

// Add to existing exports
pub use state_machine::{AgentState, AgentStateMachine};
```

### Step 4: Integration Test
```bash
# Test compilation
cargo check --manifest-path src-tauri/Cargo.toml

# Should compile successfully with new event bus infrastructure
```

---

## 🎯 Phase 1.6: Agent Runner Refactor (Day 3-4)

### Step 1: Create Event-Driven Agent Runner
```bash
mkdir -p src-tauri/src/agent/handlers
touch src-tauri/src/agent/handlers/mod.rs
touch src-tauri/src/agent/handlers/user_input.rs
touch src-tauri/src/agent/implementations/event_driven_runner.rs
```

**Implementation files and detailed steps available in full documentation...**

### Step 2: Update Main Entry Point
**Modify `src-tauri/src/anthropic.rs`** to use event-driven flow:

```rust
// Simplified submit_query that just emits events
#[tauri::command]
pub async fn submit_query(
    query: String,
    state: State<'_, AppState>,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    // Get event bus
    let event_bus = state.get_event_bus().await;
    
    // Emit user message event - everything else happens via events
    let user_message = JunoAgentEvent::UserMessage {
        content: query.trim().to_string(),
        timestamp: chrono::Utc::now().timestamp_millis() as u64,
        session_id: Some(uuid::Uuid::new_v4().to_string()),
    };
    
    event_bus.emit(user_message).await
}
```

---

## 🛠️ Phase 1.7-1.8: Tool System & UI Refactor (Day 5-7)

### Detailed implementation steps for:
- Tool Coordinator creation
- Event-driven tool execution
- UI manager refactor  
- State management conversion

---

## ✅ Testing Checklist

### After Each Phase:
- [ ] `cargo check` passes
- [ ] Basic agent execution still works
- [ ] Events are still emitted to frontend
- [ ] No regressions in functionality

### After Complete Refactor:
- [ ] End-to-end agent execution via events
- [ ] All components isolated and testable
- [ ] Performance equivalent to before
- [ ] Complete event stream debugging

---

## 🚨 Rollback Strategy

Each phase maintains backwards compatibility:

```rust
// Feature flag pattern
#[cfg(feature = "event-driven")]
use crate::agent::implementations::event_driven_runner::EventDrivenAgentRunner;

#[cfg(not(feature = "event-driven"))]
use crate::agent::implementations::agent_runner::DefaultAgentRunner;
```

Add to `Cargo.toml`:
```toml
[features]
default = ["event-driven"]
event-driven = []
```

This allows easy rollback if issues arise during refactor.

---

## 🎯 Success Metrics

1. **✅ Zero Direct Calls**: No component directly calls another
2. **✅ Event Tracing**: Complete execution visible in event stream  
3. **✅ Isolated Testing**: Each component testable independently
4. **✅ Performance**: No regression in execution speed
5. **✅ Reliability**: Error handling through event system

Ready to start with Phase 1.5? The event bus foundation is the critical first step!