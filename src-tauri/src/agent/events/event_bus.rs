use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use async_trait::async_trait;
use tauri::{AppHandle, Emitter};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use super::JunoAgentEvent;

/// Trait for components that handle events in the event-driven architecture
#[async_trait]
pub trait EventHandler: Send + Sync {
    /// Handle an event and optionally return new events to emit
    async fn handle_event(&self, event: &JunoAgentEvent) -> Result<Vec<JunoAgentEvent>, String>;
    
    /// Return the event types this handler cares about
    fn event_types(&self) -> Vec<&'static str>;
    
    /// Handler name for debugging and logging
    fn name(&self) -> &'static str;
    
    /// Priority for handler execution (higher numbers execute first)
    fn priority(&self) -> u8 { 50 }
}

/// Central event bus for the application - the heart of our event-driven architecture
pub struct EventBus {
    /// Event handlers organized by event type, sorted by priority
    handlers: Arc<RwLock<HashMap<String, Vec<Arc<dyn EventHandler>>>>>,
    /// Event store for debugging, replay, and analytics
    event_store: Arc<RwLock<Vec<JunoAgentEvent>>>,
    /// App handle for frontend communication
    app_handle: AppHandle,
    /// Configuration for the event bus
    config: EventBusConfig,
}

#[derive(Debug, Clone)]
pub struct EventBusConfig {
    /// Maximum number of events to store
    pub max_stored_events: usize,
    /// Whether to emit events to frontend
    pub emit_to_frontend: bool,
    /// Maximum recursion depth for event chains
    pub max_recursion_depth: u8,
    /// Whether to log all events for debugging
    pub debug_logging: bool,
}

impl Default for EventBusConfig {
    fn default() -> Self {
        Self {
            max_stored_events: 1000,
            emit_to_frontend: true,
            max_recursion_depth: 10,
            debug_logging: cfg!(debug_assertions),
        }
    }
}

impl EventBus {
    pub fn new(app_handle: AppHandle) -> Self {
        Self::with_config(app_handle, EventBusConfig::default())
    }
    
    pub fn with_config(app_handle: AppHandle, config: EventBusConfig) -> Self {
        info!("Initializing EventBus with config: {:?}", config);
        Self {
            handlers: Arc::new(RwLock::new(HashMap::new())),
            event_store: Arc::new(RwLock::new(Vec::new())),
            app_handle,
            config,
        }
    }
    
    /// Register an event handler - handlers are sorted by priority
    pub async fn register_handler(&self, handler: Arc<dyn EventHandler>) {
        let mut handlers = self.handlers.write().await;
        
        for event_type in handler.event_types() {
            let entry = handlers
                .entry(event_type.to_string())
                .or_insert_with(Vec::new);
            
            // Insert handler in priority order (higher priority first)
            let insert_pos = entry
                .iter()
                .position(|h| h.priority() < handler.priority())
                .unwrap_or(entry.len());
            
            entry.insert(insert_pos, handler.clone());
        }
        
        info!("Registered event handler '{}' for types: {:?}", 
              handler.name(), handler.event_types());
    }
    
    /// Emit an event and process it through all relevant handlers
    pub async fn emit(&self, event: JunoAgentEvent) -> Result<(), String> {
        self.emit_with_depth(event, 0).await
    }
    
    /// Internal emit with recursion depth tracking
    fn emit_with_depth(&self, event: JunoAgentEvent, depth: u8) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), String>> + Send + '_>> {
        Box::pin(async move {
            // Prevent infinite recursion
            if depth > self.config.max_recursion_depth {
                error!("Event recursion depth exceeded for event: {}", event.event_type());
                return Err("Event recursion depth exceeded".to_string());
            }
            
            // Store event for debugging and analytics
            {
                let mut store = self.event_store.write().await;
                store.push(event.clone());
                
                // Prune old events if necessary
                if store.len() > self.config.max_stored_events {
                    let drain_count = store.len() - self.config.max_stored_events;
                    store.drain(0..drain_count);
                }
            }
            
            if self.config.debug_logging {
                debug!("Emitting event [depth={}]: {} - {:?}", 
                       depth, event.event_type(), event);
            }
            
            // Get handlers for this event type
            let handlers = {
                let handlers_guard = self.handlers.read().await;
                handlers_guard
                    .get(event.event_type())
                    .cloned()
                    .unwrap_or_default()
            };
            
            // Process event through each handler in priority order
            let mut new_events = Vec::new();
            for handler in handlers {
                match handler.handle_event(&event).await {
                    Ok(mut events) => {
                        if self.config.debug_logging && !events.is_empty() {
                            debug!("Handler '{}' produced {} new events", 
                                   handler.name(), events.len());
                        }
                        new_events.append(&mut events);
                    }
                    Err(e) => {
                        error!("Handler '{}' failed to process event '{}': {}", 
                               handler.name(), event.event_type(), e);
                        
                        // Emit error event for failed handler
                        let error_event = JunoAgentEvent::ErrorOccurred {
                            error_type: "handler_error".to_string(),
                            message: format!("Handler '{}' failed: {}", handler.name(), e),
                            recoverable: true,
                            timestamp: chrono::Utc::now().timestamp_millis() as u64,
                            context: Some(serde_json::json!({
                                "handler": handler.name(),
                                "original_event": event.event_type()
                            })),
                        };
                        new_events.push(error_event);
                    }
                }
            }
            
            // Emit to frontend (if enabled)
            if self.config.emit_to_frontend {
                if let Err(e) = self.app_handle.emit("agent-event", &event) {
                    warn!("Failed to emit event to frontend: {}", e);
                }
            }
            
            // Recursively emit new events
            for new_event in new_events {
                self.emit_with_depth(new_event, depth + 1).await?;
            }
            
            Ok(())
        })
    }
    
    /// Get recent events for debugging and analytics
    pub async fn get_recent_events(&self, limit: usize) -> Vec<JunoAgentEvent> {
        let store = self.event_store.read().await;
        store.iter().rev().take(limit).rev().cloned().collect()
    }
    
    /// Get events for a specific session
    pub async fn get_session_events(&self, session_id: &str) -> Vec<JunoAgentEvent> {
        let store = self.event_store.read().await;
        store
            .iter()
            .filter(|event| {
                event.session_id().map_or(false, |id| id == session_id)
            })
            .cloned()
            .collect()
    }
    
    /// Clear all stored events
    pub async fn clear_events(&self) {
        let mut store = self.event_store.write().await;
        store.clear();
        info!("Cleared all stored events");
    }
    
    /// Get statistics about the event bus
    pub async fn get_stats(&self) -> EventBusStats {
        let store = self.event_store.read().await;
        let handlers = self.handlers.read().await;
        
        let mut event_type_counts = HashMap::new();
        for event in store.iter() {
            *event_type_counts.entry(event.event_type().to_string()).or_insert(0) += 1;
        }
        
        let total_handlers = handlers.values().map(|v| v.len()).sum();
        
        EventBusStats {
            total_events: store.len(),
            total_handlers,
            event_type_counts,
            handler_types: handlers.keys().cloned().collect(),
        }
    }
}

#[derive(Debug)]
pub struct EventBusStats {
    pub total_events: usize,
    pub total_handlers: usize,
    pub event_type_counts: HashMap<String, usize>,
    pub handler_types: Vec<String>,
}

/// Utility for creating session IDs
pub fn generate_session_id() -> String {
    Uuid::new_v4().to_string()
}

/// Utility for getting current timestamp
pub fn now() -> u64 {
    chrono::Utc::now().timestamp_millis() as u64
}