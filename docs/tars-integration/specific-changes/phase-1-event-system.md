# Phase 1: Event System Implementation

## Overview

This document provides exact code changes required for implementing the event-driven architecture foundation in Juno, based on patterns learned from TARS while maintaining Juno's production-ready reliability.

## Files to Create

### 1. Event Type Definitions

**File**: `src-tauri/src/agent/events/event_types.rs`

```rust
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Comprehensive event types for Juno's agent system
/// Inspired by TARS's event stream architecture
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type")]
pub enum JunoAgentEvent {
    // Core conversation events
    UserMessage {
        content: String,
        timestamp: u64,
        session_id: Option<String>,
    },
    AssistantMessage {
        content: String,
        timestamp: u64,
        session_id: Option<String>,
    },
    AssistantStreamingMessage {
        content: String,
        is_partial: bool,
        chunk_id: String,
        session_id: Option<String>,
    },
    
    // Tool execution events
    ToolCall {
        tool_name: String,
        args: Value,
        id: String,
        timestamp: u64,
        session_id: Option<String>,
    },
    ToolResult {
        tool_call_id: String,
        result: Value,
        timestamp: u64,
        success: bool,
        execution_time_ms: Option<u64>,
    },
    ToolExecutionStart {
        tool_name: String,
        tool_call_id: String,
        timestamp: u64,
    },
    ToolExecutionEnd {
        tool_name: String,
        tool_call_id: String,
        timestamp: u64,
        success: bool,
    },
    
    // Agent lifecycle events
    AgentRunStart {
        session_id: String,
        agent_type: String,
        max_iterations: u32,
        timestamp: u64,
    },
    AgentRunEnd {
        session_id: String,
        status: String,
        iterations: u32,
        elapsed_ms: u64,
        timestamp: u64,
    },
    AgentIterationStart {
        session_id: String,
        iteration: u32,
        timestamp: u64,
    },
    AgentIterationEnd {
        session_id: String,
        iteration: u32,
        timestamp: u64,
        action_taken: String,
    },
    
    // Voice system events
    VoiceTranscriptionStart {
        session_id: String,
        mode: String, // "agent", "dictation", "always_listening"
        timestamp: u64,
    },
    VoiceTranscriptionChunk {
        content: String,
        is_final: bool,
        confidence: Option<f32>,
        session_id: String,
        timestamp: u64,
    },
    VoiceTranscriptionEnd {
        session_id: String,
        final_text: String,
        total_duration_ms: u64,
        timestamp: u64,
    },
    VoiceTranscriptionError {
        session_id: String,
        error_message: String,
        timestamp: u64,
    },
    
    // TTS events
    TtsStart {
        text: String,
        provider: String,
        session_id: String,
        timestamp: u64,
    },
    TtsEnd {
        session_id: String,
        success: bool,
        duration_ms: Option<u64>,
        timestamp: u64,
    },
    
    // System events
    SystemMessage {
        level: String, // "debug", "info", "warn", "error"
        message: String,
        timestamp: u64,
        category: Option<String>,
    },
    PermissionRequest {
        permission_type: String, // "accessibility", "screen_recording", "microphone"
        status: String, // "requested", "granted", "denied"
        timestamp: u64,
    },
    ErrorOccurred {
        error_type: String,
        message: String,
        recoverable: bool,
        timestamp: u64,
        context: Option<Value>,
    },
    
    // Memory management events
    MemoryPruneStart {
        reason: String,
        messages_before: usize,
        estimated_tokens: usize,
        timestamp: u64,
    },
    MemoryPruneEnd {
        messages_after: usize,
        tokens_saved: usize,
        compression_applied: bool,
        timestamp: u64,
    },
    
    // Browser events
    BrowserStart {
        session_id: String,
        timestamp: u64,
    },
    BrowserEnd {
        session_id: String,
        timestamp: u64,
    },
    BrowserNavigation {
        url: String,
        session_id: String,
        timestamp: u64,
    },
    
    // Configuration events
    ConfigurationChanged {
        key: String,
        old_value: Option<Value>,
        new_value: Value,
        timestamp: u64,
    },
}

impl JunoAgentEvent {
    /// Create a new event with current timestamp
    pub fn with_timestamp(mut self) -> Self {
        let timestamp = chrono::Utc::now().timestamp_millis() as u64;
        
        match &mut self {
            JunoAgentEvent::UserMessage { timestamp: ts, .. } => *ts = timestamp,
            JunoAgentEvent::AssistantMessage { timestamp: ts, .. } => *ts = timestamp,
            JunoAgentEvent::ToolCall { timestamp: ts, .. } => *ts = timestamp,
            JunoAgentEvent::ToolResult { timestamp: ts, .. } => *ts = timestamp,
            JunoAgentEvent::AgentRunStart { timestamp: ts, .. } => *ts = timestamp,
            JunoAgentEvent::AgentRunEnd { timestamp: ts, .. } => *ts = timestamp,
            JunoAgentEvent::SystemMessage { timestamp: ts, .. } => *ts = timestamp,
            JunoAgentEvent::ErrorOccurred { timestamp: ts, .. } => *ts = timestamp,
            _ => {} // Handle other variants as needed
        }
        
        self
    }
    
    /// Get the session ID from events that have one
    pub fn session_id(&self) -> Option<&str> {
        match self {
            JunoAgentEvent::UserMessage { session_id, .. } => session_id.as_deref(),
            JunoAgentEvent::AssistantMessage { session_id, .. } => session_id.as_deref(),
            JunoAgentEvent::AssistantStreamingMessage { session_id, .. } => session_id.as_deref(),
            JunoAgentEvent::ToolCall { session_id, .. } => session_id.as_deref(),
            JunoAgentEvent::AgentRunStart { session_id, .. } => Some(session_id),
            JunoAgentEvent::AgentRunEnd { session_id, .. } => Some(session_id),
            JunoAgentEvent::VoiceTranscriptionStart { session_id, .. } => Some(session_id),
            JunoAgentEvent::VoiceTranscriptionEnd { session_id, .. } => Some(session_id),
            _ => None,
        }
    }
}

/// Event subscription trait for components that want to receive events
#[async_trait::async_trait]
pub trait EventSubscriber: Send + Sync {
    async fn on_event(&self, event: &JunoAgentEvent) -> Result<(), String>;
    
    /// Filter to only receive specific event types
    fn event_filter(&self) -> Option<Vec<&'static str>> {
        None // By default, receive all events
    }
}

/// Event filtering utilities
pub struct EventFilter;

impl EventFilter {
    pub fn matches_filter(event: &JunoAgentEvent, filter: &[&str]) -> bool {
        let event_type = match event {
            JunoAgentEvent::UserMessage { .. } => "user_message",
            JunoAgentEvent::AssistantMessage { .. } => "assistant_message",
            JunoAgentEvent::ToolCall { .. } => "tool_call",
            JunoAgentEvent::ToolResult { .. } => "tool_result",
            JunoAgentEvent::AgentRunStart { .. } => "agent_run_start",
            JunoAgentEvent::AgentRunEnd { .. } => "agent_run_end",
            JunoAgentEvent::SystemMessage { .. } => "system_message",
            JunoAgentEvent::ErrorOccurred { .. } => "error_occurred",
            _ => "unknown",
        };
        
        filter.contains(&event_type)
    }
}
```

### 2. Event Processor Implementation

**File**: `src-tauri/src/agent/events/event_processor.rs`

```rust
use std::sync::Arc;
use tokio::sync::RwLock;
use tauri::{AppHandle, Emitter};
use tracing::{debug, error, info, warn};

use super::event_types::{JunoAgentEvent, EventSubscriber, EventFilter};

/// Central event processing system for Juno
/// Inspired by TARS's event stream architecture with Juno's reliability patterns
pub struct JunoEventStreamProcessor {
    /// Event log for debugging and replay
    events: Arc<RwLock<Vec<JunoAgentEvent>>>,
    /// Subscribers for real-time event processing
    subscribers: Arc<RwLock<Vec<Box<dyn EventSubscriber + Send + Sync>>>>,
    /// Tauri app handle for frontend communication
    app_handle: AppHandle,
    /// Configuration
    config: EventProcessorConfig,
}

#[derive(Debug, Clone)]
pub struct EventProcessorConfig {
    /// Maximum number of events to keep in memory
    pub max_events: usize,
    /// Whether to emit events to frontend
    pub emit_to_frontend: bool,
    /// Whether to persist events to disk
    pub persist_events: bool,
    /// Maximum number of failed emissions before warning
    pub max_emission_failures: usize,
}

impl Default for EventProcessorConfig {
    fn default() -> Self {
        Self {
            max_events: 10000,
            emit_to_frontend: true,
            persist_events: false, // Can be enabled later
            max_emission_failures: 5,
        }
    }
}

impl JunoEventStreamProcessor {
    pub fn new(app_handle: AppHandle, config: Option<EventProcessorConfig>) -> Self {
        Self {
            events: Arc::new(RwLock::new(Vec::new())),
            subscribers: Arc::new(RwLock::new(Vec::new())),
            app_handle,
            config: config.unwrap_or_default(),
        }
    }
    
    /// Send an event through the system
    pub async fn send_event(&self, event: JunoAgentEvent) -> Result<(), String> {
        let event = event.with_timestamp();
        
        debug!("Processing event: {:?}", event);
        
        // Add to event log first
        {
            let mut events = self.events.write().await;
            events.push(event.clone());
            
            // Prune old events if necessary
            if events.len() > self.config.max_events {
                let remove_count = events.len() - self.config.max_events;
                events.drain(0..remove_count);
                debug!("Pruned {} old events from event log", remove_count);
            }
        }
        
        // Notify subscribers
        {
            let subscribers = self.subscribers.read().await;
            for subscriber in subscribers.iter() {
                // Check if subscriber has a filter
                if let Some(filter) = subscriber.event_filter() {
                    if !EventFilter::matches_filter(&event, &filter) {
                        continue;
                    }
                }
                
                if let Err(e) = subscriber.on_event(&event).await {
                    warn!("Event subscriber error: {}", e);
                }
            }
        }
        
        // Emit to frontend if configured
        if self.config.emit_to_frontend {
            if let Err(e) = self.app_handle.emit("agent-event", &event) {
                error!("Failed to emit event to frontend: {}", e);
                return Err(format!("Failed to emit event: {}", e));
            }
        }
        
        Ok(())
    }
    
    /// Add a subscriber to receive events
    pub async fn subscribe(&self, subscriber: Box<dyn EventSubscriber + Send + Sync>) {
        let mut subscribers = self.subscribers.write().await;
        subscribers.push(subscriber);
        info!("Added event subscriber, total: {}", subscribers.len());
    }
    
    /// Get recent events (for debugging or replay)
    pub async fn get_events(&self, limit: Option<usize>) -> Vec<JunoAgentEvent> {
        let events = self.events.read().await;
        
        if let Some(limit) = limit {
            events.iter().rev().take(limit).rev().cloned().collect()
        } else {
            events.clone()
        }
    }
    
    /// Get events for a specific session
    pub async fn get_session_events(&self, session_id: &str) -> Vec<JunoAgentEvent> {
        let events = self.events.read().await;
        events
            .iter()
            .filter(|event| {
                event.session_id().map_or(false, |id| id == session_id)
            })
            .cloned()
            .collect()
    }
    
    /// Clear all events (for testing or reset)
    pub async fn clear_events(&self) {
        let mut events = self.events.write().await;
        events.clear();
        info!("Cleared all events from event processor");
    }
    
    /// Get event statistics
    pub async fn get_stats(&self) -> EventProcessorStats {
        let events = self.events.read().await;
        let subscribers = self.subscribers.read().await;
        
        let mut event_type_counts = std::collections::HashMap::new();
        for event in events.iter() {
            let event_type = match event {
                JunoAgentEvent::UserMessage { .. } => "user_message",
                JunoAgentEvent::AssistantMessage { .. } => "assistant_message",
                JunoAgentEvent::ToolCall { .. } => "tool_call",
                JunoAgentEvent::ToolResult { .. } => "tool_result",
                JunoAgentEvent::AgentRunStart { .. } => "agent_run_start",
                JunoAgentEvent::AgentRunEnd { .. } => "agent_run_end",
                JunoAgentEvent::SystemMessage { .. } => "system_message",
                JunoAgentEvent::ErrorOccurred { .. } => "error_occurred",
                _ => "other",
            };
            
            *event_type_counts.entry(event_type.to_string()).or_insert(0) += 1;
        }
        
        EventProcessorStats {
            total_events: events.len(),
            subscriber_count: subscribers.len(),
            event_type_counts,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct EventProcessorStats {
    pub total_events: usize,
    pub subscriber_count: usize,
    pub event_type_counts: std::collections::HashMap<String, usize>,
}

/// Example subscriber for logging events
pub struct LoggingSubscriber;

#[async_trait::async_trait]
impl EventSubscriber for LoggingSubscriber {
    async fn on_event(&self, event: &JunoAgentEvent) -> Result<(), String> {
        match event {
            JunoAgentEvent::ErrorOccurred { error_type, message, .. } => {
                error!("Agent Error [{}]: {}", error_type, message);
            }
            JunoAgentEvent::SystemMessage { level, message, .. } => {
                match level.as_str() {
                    "error" => error!("System: {}", message),
                    "warn" => warn!("System: {}", message),
                    "info" => info!("System: {}", message),
                    _ => debug!("System: {}", message),
                }
            }
            JunoAgentEvent::AgentRunStart { session_id, agent_type, .. } => {
                info!("Agent started: {} (session: {})", agent_type, session_id);
            }
            JunoAgentEvent::AgentRunEnd { session_id, status, iterations, elapsed_ms, .. } => {
                info!("Agent finished: {} in {}ms ({} iterations, session: {})", 
                      status, elapsed_ms, iterations, session_id);
            }
            _ => {
                debug!("Event: {:?}", event);
            }
        }
        
        Ok(())
    }
    
    fn event_filter(&self) -> Option<Vec<&'static str>> {
        // Log all events
        None
    }
}
```

### 3. Module Definition

**File**: `src-tauri/src/agent/events/mod.rs`

```rust
//! Event-driven architecture for Juno's agent system
//! 
//! This module implements a comprehensive event system inspired by TARS's
//! event stream architecture while maintaining Juno's production-ready
//! reliability patterns.

pub mod event_types;
pub mod event_processor;

pub use event_types::{JunoAgentEvent, EventSubscriber, EventFilter};
pub use event_processor::{JunoEventStreamProcessor, EventProcessorConfig, LoggingSubscriber};

// Re-export commonly used types
pub use event_types::JunoAgentEvent as AgentEvent;
pub use event_processor::JunoEventStreamProcessor as EventProcessor;
```

## Files to Modify

### 1. AppState Integration

**File**: `src-tauri/src/state.rs`

**Add to imports:**
```rust
use crate::agent::events::{EventProcessor, EventProcessorConfig, JunoAgentEvent, LoggingSubscriber};
```

**Add to AppState struct:**
```rust
impl AppState {
    // Add this field to the AppState struct
    pub event_processor: Arc<TokioMutex<EventProcessor>>,
}
```

**Add to AppState implementation:**
```rust
impl AppState {
    // Add this method to emit events
    pub async fn emit_agent_event(&self, event: JunoAgentEvent) -> Result<(), String> {
        let processor = self.event_processor.lock().await;
        processor.send_event(event).await
    }
    
    // Add this method to get the event processor
    pub async fn get_event_processor(&self) -> Arc<TokioMutex<EventProcessor>> {
        self.event_processor.clone()
    }
    
    // Add this method to subscribe to events
    pub async fn subscribe_to_events(&self, subscriber: Box<dyn EventSubscriber + Send + Sync>) {
        let processor = self.event_processor.lock().await;
        processor.subscribe(subscriber).await;
    }
}
```

**Add to AppState initialization (in main.rs or wherever AppState is created):**
```rust
// In the AppState creation code, add:
let event_processor = {
    let config = EventProcessorConfig {
        max_events: 10000,
        emit_to_frontend: true,
        persist_events: false,
        max_emission_failures: 5,
    };
    
    let processor = EventProcessor::new(app_handle.clone(), Some(config));
    
    // Add logging subscriber
    processor.subscribe(Box::new(LoggingSubscriber)).await;
    
    Arc::new(TokioMutex::new(processor))
};

// Add event_processor to AppState construction
```

### 2. Main Orchestrator Integration

**File**: `src-tauri/src/anthropic.rs`

**Add to imports:**
```rust
use crate::agent::events::JunoAgentEvent;
```

**Modify the `execute_agent_internal` function:**

```rust
async fn execute_agent_internal(
    query: String,
    state: tauri::State<'_, AppState>,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    // Generate a unique execution ID for this agent run
    let execution_id = uuid::Uuid::new_v4().to_string();
    let start_time = std::time::Instant::now();

    // Emit agent run start event
    state.emit_agent_event(JunoAgentEvent::AgentRunStart {
        session_id: execution_id.clone(),
        agent_type: "orchestrator".to_string(),
        max_iterations: agent::config::MAX_ITERATIONS,
        timestamp: chrono::Utc::now().timestamp_millis() as u64,
    }).await.map_err(|e| format!("Failed to emit agent start event: {}", e))?;

    // Emit user message event
    state.emit_agent_event(JunoAgentEvent::UserMessage {
        content: query.clone(),
        timestamp: chrono::Utc::now().timestamp_millis() as u64,
        session_id: Some(execution_id.clone()),
    }).await.map_err(|e| format!("Failed to emit user message event: {}", e))?;

    // Mark agent execution as started with max iterations (both modes use 15)
    let _ = state.mark_agent_execution_started_with_steps(
        execution_id.clone(),
        agent::config::MAX_ITERATIONS,
    );
    info!(
        "Starting new agent execution with ID: {} (max steps: {})",
        execution_id,
        agent::config::MAX_ITERATIONS
    );

    // --- FIXED: Notify Floating Bar Manager that Agent Started ---
    // This ensures the floating bar shows agent activity regardless of trigger source
    let app_handle_for_bar_start = app_handle.clone();
    tauri::async_runtime::spawn(async move {
        crate::commands::ui_commands::handle_agent_started(&app_handle_for_bar_start).await;
    });

    // Register escape key for cancellation during agent execution
    if let Err(e) =
        crate::commands::shortcuts::register_escape_key_handler(app_handle.clone()).await
    {
        warn!("Failed to configure escape key for agent execution: {} - continuing without escape key cancellation", e);
        
        // Emit system warning event
        state.emit_agent_event(JunoAgentEvent::SystemMessage {
            level: "warn".to_string(),
            message: format!("Failed to configure escape key: {}", e),
            timestamp: chrono::Utc::now().timestamp_millis() as u64,
            category: Some("input_handling".to_string()),
        }).await.ok(); // Don't fail the whole operation for this
    }

    // Reset cancellation signal for the new agent
    state.reset_cancel();
    info!("Reset cancellation signal for new agent execution");

    let trimmed_query = query.trim();

    // ... existing agent execution logic ...

    // At the end of execution, emit completion events
    let elapsed_ms = start_time.elapsed().as_millis() as u64;
    
    match agent_result {
        Ok(message) => {
            // Emit assistant message event
            state.emit_agent_event(JunoAgentEvent::AssistantMessage {
                content: message.clone(),
                timestamp: chrono::Utc::now().timestamp_millis() as u64,
                session_id: Some(execution_id.clone()),
            }).await.map_err(|e| format!("Failed to emit assistant message event: {}", e))?;
            
            // Emit successful completion event
            state.emit_agent_event(JunoAgentEvent::AgentRunEnd {
                session_id: execution_id.clone(),
                status: "completed".to_string(),
                iterations: current_iteration, // You'll need to track this
                elapsed_ms,
                timestamp: chrono::Utc::now().timestamp_millis() as u64,
            }).await.map_err(|e| format!("Failed to emit agent end event: {}", e))?;
            
            info!("Agent execution completed successfully");
        }
        Err(e) => {
            // Emit error event
            state.emit_agent_event(JunoAgentEvent::ErrorOccurred {
                error_type: "agent_execution".to_string(),
                message: e.to_string(),
                recoverable: matches!(e, AgentError::Terminated),
                timestamp: chrono::Utc::now().timestamp_millis() as u64,
                context: Some(serde_json::json!({
                    "session_id": execution_id,
                    "query": trimmed_query,
                    "elapsed_ms": elapsed_ms
                })),
            }).await.map_err(|e| format!("Failed to emit error event: {}", e))?;
            
            // Emit failed completion event
            let status = match e {
                AgentError::Terminated => "cancelled",
                AgentError::MaxStepsReached => "max_steps_reached",
                _ => "failed",
            };
            
            state.emit_agent_event(JunoAgentEvent::AgentRunEnd {
                session_id: execution_id.clone(),
                status: status.to_string(),
                iterations: current_iteration, // You'll need to track this
                elapsed_ms,
                timestamp: chrono::Utc::now().timestamp_millis() as u64,
            }).await.map_err(|e| format!("Failed to emit agent end event: {}", e))?;
            
            error!("Agent execution failed: {}", e);
        }
    }

    // ... rest of existing cleanup logic ...
    
    Ok(())
}
```

### 3. Voice System Integration

**File**: Voice transcription components (you'll need to identify the specific files)

**Add event emission for voice events:**

```rust
// In voice transcription start
state.emit_agent_event(JunoAgentEvent::VoiceTranscriptionStart {
    session_id: session_id.clone(),
    mode: "agent".to_string(), // or "dictation" or "always_listening"
    timestamp: chrono::Utc::now().timestamp_millis() as u64,
}).await.ok();

// In voice transcription chunk processing
state.emit_agent_event(JunoAgentEvent::VoiceTranscriptionChunk {
    content: transcribed_text.clone(),
    is_final: is_final_chunk,
    confidence: Some(confidence_score),
    session_id: session_id.clone(),
    timestamp: chrono::Utc::now().timestamp_millis() as u64,
}).await.ok();

// In voice transcription end
state.emit_agent_event(JunoAgentEvent::VoiceTranscriptionEnd {
    session_id: session_id.clone(),
    final_text: final_transcription.clone(),
    total_duration_ms: duration.as_millis() as u64,
    timestamp: chrono::Utc::now().timestamp_millis() as u64,
}).await.ok();
```

## Testing and Validation

### 1. Event Flow Testing

**File**: `src-tauri/src/agent/events/tests.rs`

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tokio_test;
    
    #[tokio::test]
    async fn test_event_emission() {
        // Create test app handle (you'll need to mock this)
        let app_handle = create_test_app_handle();
        let processor = EventProcessor::new(app_handle, None);
        
        let event = JunoAgentEvent::UserMessage {
            content: "Test message".to_string(),
            timestamp: 0, // Will be set by with_timestamp()
            session_id: Some("test-session".to_string()),
        };
        
        let result = processor.send_event(event).await;
        assert!(result.is_ok());
        
        let events = processor.get_events(None).await;
        assert_eq!(events.len(), 1);
        assert!(events[0].timestamp > 0);
    }
    
    #[tokio::test]
    async fn test_event_filtering() {
        let app_handle = create_test_app_handle();
        let processor = EventProcessor::new(app_handle, None);
        
        // Create test subscriber that only wants tool events
        struct ToolEventSubscriber {
            pub received_events: Arc<Mutex<Vec<String>>>,
        }
        
        #[async_trait::async_trait]
        impl EventSubscriber for ToolEventSubscriber {
            async fn on_event(&self, event: &JunoAgentEvent) -> Result<(), String> {
                let mut events = self.received_events.lock().await;
                match event {
                    JunoAgentEvent::ToolCall { tool_name, .. } => {
                        events.push(format!("tool_call:{}", tool_name));
                    }
                    JunoAgentEvent::ToolResult { tool_call_id, .. } => {
                        events.push(format!("tool_result:{}", tool_call_id));
                    }
                    _ => {}
                }
                Ok(())
            }
            
            fn event_filter(&self) -> Option<Vec<&'static str>> {
                Some(vec!["tool_call", "tool_result"])
            }
        }
        
        let received_events = Arc::new(Mutex::new(Vec::new()));
        let subscriber = ToolEventSubscriber {
            received_events: received_events.clone(),
        };
        
        processor.subscribe(Box::new(subscriber)).await;
        
        // Send various events
        processor.send_event(JunoAgentEvent::UserMessage {
            content: "test".to_string(),
            timestamp: 0,
            session_id: None,
        }).await.unwrap();
        
        processor.send_event(JunoAgentEvent::ToolCall {
            tool_name: "test_tool".to_string(),
            args: serde_json::json!({}),
            id: "call-123".to_string(),
            timestamp: 0,
            session_id: None,
        }).await.unwrap();
        
        // Check that only tool events were received
        let events = received_events.lock().await;
        assert_eq!(events.len(), 1);
        assert_eq!(events[0], "tool_call:test_tool");
    }
}
```

### 2. Integration Testing

Create integration tests to verify that events are properly emitted during actual agent execution.

### 3. Performance Testing

Measure the performance impact of event emission to ensure it doesn't affect response times significantly.

## Frontend Integration

### Event Handling Component

**File**: Frontend component for handling events

```typescript
// Frontend event handling (React/TypeScript)
interface AgentEvent {
  type: string;
  [key: string]: any;
}

export function useAgentEvents() {
  const [events, setEvents] = useState<AgentEvent[]>([]);
  const [currentSession, setCurrentSession] = useState<string | null>(null);
  
  useEffect(() => {
    const unlisten = listen('agent-event', (event: any) => {
      const agentEvent = event.payload as AgentEvent;
      
      setEvents(prev => [...prev, agentEvent]);
      
      // Track current session
      if (agentEvent.type === 'AgentRunStart') {
        setCurrentSession(agentEvent.session_id);
      } else if (agentEvent.type === 'AgentRunEnd') {
        // Keep session for a bit to see final events
        setTimeout(() => setCurrentSession(null), 1000);
      }
    });
    
    return () => {
      unlisten.then(fn => fn());
    };
  }, []);
  
  return {
    events,
    currentSession,
    sessionEvents: events.filter(e => e.session_id === currentSession),
  };
}
```

## Rollback Strategy

If issues arise with the event system:

1. **Feature Flag**: Add a feature flag to disable event emission
2. **Fallback Mode**: Maintain existing direct UI update paths
3. **Gradual Rollout**: Enable events for specific event types first
4. **Performance Monitoring**: Monitor for performance regression

## Validation Checklist

- [ ] Event system compiles without errors
- [ ] Events are properly emitted during agent execution
- [ ] Frontend receives events in real-time
- [ ] No performance regression in agent response times
- [ ] Existing functionality continues to work
- [ ] Event filtering works correctly
- [ ] Memory usage remains stable with event logging
- [ ] Error handling works properly when event emission fails

This completes the Phase 1 implementation for the event-driven architecture foundation. The next phase will build upon this foundation to implement multiple tool call strategies.