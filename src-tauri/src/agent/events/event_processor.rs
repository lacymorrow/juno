use std::sync::Arc;
use tokio::sync::RwLock;
use tauri::{AppHandle, Emitter};
use tracing::{debug, error, info, warn};
use serde::Serialize;

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
            let event_type = event.event_type();
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
            JunoAgentEvent::ToolCall { tool_name, id, .. } => {
                debug!("Tool call: {} (id: {})", tool_name, id);
            }
            JunoAgentEvent::ToolResult { tool_call_id, success, execution_time_ms, .. } => {
                debug!("Tool result: {} (success: {}, time: {:?}ms)", 
                       tool_call_id, success, execution_time_ms);
            }
            _ => {
                debug!("Event: {}", event.event_type());
            }
        }
        
        Ok(())
    }
    
    fn event_filter(&self) -> Option<Vec<&'static str>> {
        // Log all events
        None
    }
}