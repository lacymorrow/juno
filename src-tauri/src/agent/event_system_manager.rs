//! Event System Manager
//! 
//! Centralized manager for initializing and coordinating the event-driven architecture.
//! Registers all event handlers and provides a unified interface for event system management.
//! 
//! TARS Integration Phase 1.6-1.8: Event System Coordination

use std::sync::Arc;
use tracing::{error, info, warn};

use crate::agent::events::EventBus;
use crate::agent::handlers::{UserInputHandler, AgentOrchestrator};
use crate::agent::implementations::event_driven_runner::EventDrivenAgentRunner;
use crate::agent::tools::{ToolCoordinator, EventDrivenToolExecutor};
use crate::agent::implementations::tool_provider::LocalToolProvider;
use crate::state::{AppState, EventDrivenStateManager};
use crate::ui::EventDrivenUIManager;

/// Manages the entire event-driven system lifecycle
pub struct EventSystemManager {
    /// Central event bus
    event_bus: Arc<EventBus>,
    /// App handle for accessing shared resources
    app_handle: tauri::AppHandle,
    /// Application state
    app_state: Arc<AppState>,
}

impl EventSystemManager {
    /// Create a new event system manager
    pub async fn new(app_handle: tauri::AppHandle, app_state: Arc<AppState>) -> Result<Self, String> {
        // Initialize event bus
        let event_bus = app_state.get_event_bus().await?;
        
        Ok(Self {
            event_bus,
            app_handle,
            app_state,
        })
    }
    
    /// Initialize the complete event-driven system
    pub async fn initialize(&self) -> Result<(), String> {
        info!("Initializing event-driven system with all handlers...");
        
        // Register core event handlers in priority order
        self.register_user_input_handler().await?;
        self.register_agent_orchestrator().await?;
        self.register_event_driven_runner().await?;
        self.register_tool_coordinator().await?;
        self.register_event_driven_tool_executor().await?;
        self.register_event_driven_state_manager().await?;
        self.register_event_driven_ui_manager().await?;
        
        info!("Event-driven system initialization completed successfully");
        Ok(())
    }
    
    /// Register the user input handler (highest priority)
    async fn register_user_input_handler(&self) -> Result<(), String> {
        info!("Registering UserInputHandler...");
        
        let handler = Arc::new(UserInputHandler::new());
        self.event_bus.register_handler(handler).await;
        
        info!("UserInputHandler registered successfully");
        Ok(())
    }
    
    /// Register the agent orchestrator
    async fn register_agent_orchestrator(&self) -> Result<(), String> {
        info!("Registering AgentOrchestrator...");
        
        let handler = Arc::new(AgentOrchestrator::new(
            self.app_state.clone(),
            self.app_handle.clone()
        ));
        self.event_bus.register_handler(handler).await;
        
        info!("AgentOrchestrator registered successfully");
        Ok(())
    }
    
    /// Register the event-driven agent runner
    async fn register_event_driven_runner(&self) -> Result<(), String> {
        info!("Registering EventDrivenAgentRunner...");
        
        // Get shared memory manager
        let memory_manager = self.app_state.get_memory_manager().await
            .ok_or("EventMemoryManager not initialized")?;
        
        // Create event-driven runner
        let runner = EventDrivenAgentRunner::new(
            memory_manager,
            self.app_handle.clone(),
            15, // Max iterations
        ).await?;
        
        let handler = Arc::new(runner);
        self.event_bus.register_handler(handler).await;
        
        info!("EventDrivenAgentRunner registered successfully");
        Ok(())
    }
    
    /// Register the tool coordinator
    async fn register_tool_coordinator(&self) -> Result<(), String> {
        info!("Registering ToolCoordinator...");
        
        let mut coordinator = ToolCoordinator::new(self.app_handle.clone());
        
        // Create and register default tool provider
        let mut tool_provider = LocalToolProvider::new();
        
        // Register all tools with the provider
        self.register_all_tools(&mut tool_provider).await?;
        
        // Register the tool provider with the coordinator
        coordinator.register_default_provider(Arc::new(tool_provider));
        
        let handler = Arc::new(coordinator);
        self.event_bus.register_handler(handler).await;
        
        info!("ToolCoordinator registered successfully");
        Ok(())
    }
    
    /// Register the event-driven tool executor
    async fn register_event_driven_tool_executor(&self) -> Result<(), String> {
        info!("Registering EventDrivenToolExecutor...");
        
        let mut executor = EventDrivenToolExecutor::new(self.app_handle.clone());
        
        // Create and register default tool provider for executor
        let mut tool_provider = LocalToolProvider::new();
        
        // Register all tools with the executor's provider
        self.register_all_tools(&mut tool_provider).await?;
        
        // Register the tool provider with the executor
        executor.register_default_provider(Arc::new(tool_provider));
        
        let handler = Arc::new(executor);
        self.event_bus.register_handler(handler).await;
        
        info!("EventDrivenToolExecutor registered successfully");
        Ok(())
    }
    
    /// Register the event-driven state manager
    async fn register_event_driven_state_manager(&self) -> Result<(), String> {
        info!("Registering EventDrivenStateManager...");
        
        let state_manager = EventDrivenStateManager::new(
            self.app_state.clone(),
            self.app_handle.clone()
        );
        
        let handler = Arc::new(state_manager);
        self.event_bus.register_handler(handler).await;
        
        info!("EventDrivenStateManager registered successfully");
        Ok(())
    }
    
    /// Register the event-driven UI manager
    async fn register_event_driven_ui_manager(&self) -> Result<(), String> {
        info!("Registering EventDrivenUIManager...");
        
        let ui_manager = EventDrivenUIManager::new(
            self.app_handle.clone(),
            true // Enable legacy events for backwards compatibility
        );
        
        let handler = Arc::new(ui_manager);
        self.event_bus.register_handler(handler).await;
        
        info!("EventDrivenUIManager registered successfully");
        Ok(())
    }
    
    /// Register all tools with a tool provider
    async fn register_all_tools(&self, tool_provider: &mut LocalToolProvider) -> Result<(), String> {
        info!("Registering all tools with tool provider...");
        
        // Register basic tools
        crate::agent::tools::basic_tools::register_basic_tools(tool_provider).await;
        
        // Register computer use tools
        if let Err(e) = crate::agent::providers::factory::BrainFactory::register_computer_use_tools(
            tool_provider,
            self.app_handle.clone()
        ).await {
            warn!("Failed to register computer use tools: {}", e);
        }
        
        // Register browser tools
        for definition in crate::agent::tools::browser_tools::get_browser_tool_definitions() {
            let app_state = self.app_state.clone();
            let executor = move |input: serde_json::Value| {
                let app_state = app_state.clone();
                async move {
                    // Browser tool execution will be implemented later
                    warn!("Browser tool execution not yet implemented in event system: {}", input);
                    Ok(serde_json::json!({"error": "Browser tools not yet implemented in event system"}))
                }
            };
            tool_provider.register_async_tool(definition.clone(), executor).await;
        }
        
        info!("All tools registered successfully with tool provider");
        Ok(())
    }
    
    /// Get the event bus for external access
    pub fn get_event_bus(&self) -> Arc<EventBus> {
        self.event_bus.clone()
    }
    
    /// Emit an event through the system
    pub async fn emit_event(&self, event: crate::agent::events::JunoAgentEvent) -> Result<(), String> {
        self.event_bus.emit(event).await
    }
    
    /// Get event system statistics
    pub async fn get_stats(&self) -> Result<serde_json::Value, String> {
        let stats = self.event_bus.get_stats().await;
        Ok(serde_json::json!({
            "total_events": stats.total_events,
            "total_handlers": stats.total_handlers,
            "event_type_counts": stats.event_type_counts,
            "handler_types": stats.handler_types
        }))
    }
    
    /// Shutdown the event system gracefully
    pub async fn shutdown(&self) -> Result<(), String> {
        info!("Shutting down event-driven system...");
        
        // Clear event store
        self.event_bus.clear_events().await;
        
        info!("Event-driven system shutdown completed");
        Ok(())
    }
}