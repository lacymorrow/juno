// This file contains the fixed version of critical race condition sections from anthropic.rs
// Copy these implementations to replace the original ones

use crate::utils::atomic_state::{AtomicExecutionCoordinator, AtomicQueue};
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tracing::{info, error, warn};
use uuid::Uuid;
use std::time::Instant;

/// Fixed AgentExecutionQueue with atomic operations
pub struct AgentExecutionQueueFixed {
    coordinator: Arc<AtomicExecutionCoordinator>,
    pending_queries: Arc<AtomicQueue<QueuedQuery>>,
}

impl AgentExecutionQueueFixed {
    pub fn new() -> Self {
        Self {
            coordinator: Arc::new(AtomicExecutionCoordinator::new()),
            pending_queries: Arc::new(AtomicQueue::new(10)), // Max 10 pending queries
        }
    }

    /// Queue a new query atomically
    pub async fn queue_query(
        &self,
        query: String,
        app_handle: tauri::AppHandle,
        state: tauri::State<'_, AppState>,
    ) -> Result<String, String> {
        let query_id = Uuid::new_v4().to_string();
        let queued_query = QueuedQuery {
            id: query_id.clone(),
            query,
            queued_at: Instant::now(),
            app_handle,
        };

        // Cancel current execution if any
        self.cancel_current_execution(state).await;

        // Clear pending queue and add new query atomically
        self.pending_queries.clear().await;
        self.pending_queries.push(queued_query).await?;
        
        info!("Queued new agent query with ID: {}", query_id);
        Ok(query_id)
    }

    /// Cancel current execution with proper cleanup
    pub async fn cancel_current_execution(&self, state: tauri::State<'_, AppState>) {
        if let Some(execution_id) = self.coordinator.get_current_execution_id().await {
            info!("Cancelling current agent execution: {}", execution_id);
            state.signal_cancel();
            info!("Signalled cancellation for existing agent execution");
            
            // Wait for cancellation with timeout
            let start = Instant::now();
            while self.coordinator.is_executing().await && start.elapsed().as_millis() < 500 {
                tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
            }
        }
    }

    /// Execute next query atomically
    pub async fn execute_next_query(&self, state: tauri::State<'_, AppState>) -> Option<QueuedQuery> {
        // Get next query from queue
        let query = self.pending_queries.pop().await?;
        
        // Try to start execution atomically
        let guard = match self.coordinator.try_start_execution(query.id.clone()).await {
            Ok(guard) => guard,
            Err(e) => {
                warn!("Failed to start execution: {}", e);
                // Re-queue the query
                let _ = self.pending_queries.push(query).await;
                return None;
            }
        };

        info!("Starting atomic agent execution for query ID: {}", query.id);

        // Execute the actual agent logic
        let result = execute_agent_internal(
            query.query.clone(),
            state,
            query.app_handle.clone()
        ).await;

        // Guard automatically cleans up on drop
        drop(guard);

        // Handle execution result
        match result {
            Ok(()) => {
                info!("Agent execution completed successfully for query ID: {}", query.id);
            }
            Err(e) => {
                error!("Agent execution failed for query {}: {}", query.id, e);
            }
        }

        Some(query)
    }

    /// Check if execution is in progress
    pub async fn is_executing(&self) -> bool {
        self.coordinator.is_executing().await
    }
}

// Fixed memory manager to prevent race conditions in pruning
use crate::agent::implementations::memory_manager::SimpleMemoryManager;

impl SimpleMemoryManager {
    /// Fixed add_message to prevent race conditions
    pub async fn add_message_fixed(&mut self, message: Message) -> Result<(), AgentError> {
        // Use a single lock acquisition for the entire operation
        let mut messages = self.messages.write().await;
        
        // Add message
        messages.push(message.clone());
        
        // Check if pruning needed while holding lock
        let needs_pruning = messages.len() > self.max_messages;
        
        if needs_pruning {
            // Prune while holding the same lock
            let excess = messages.len() - self.max_messages;
            messages.drain(0..excess);
            info!("Pruned {} messages to maintain limit of {}", excess, self.max_messages);
        }
        
        drop(messages);
        
        // Update metrics after releasing lock
        self.update_metrics().await;
        
        Ok(())
    }
}

// Fixed state management to prevent non-atomic updates
use crate::state::AppState;

impl AppState {
    /// Fixed atomic state update for agent execution
    pub fn mark_agent_execution_started_atomic(
        &self,
        execution_id: String,
        max_steps: u32,
    ) -> Result<(), String> {
        // Create update struct first
        let update = AgentExecutionUpdate {
            execution_active: true,
            execution_id: Some(execution_id.clone()),
            max_steps: Some(max_steps),
            current_step: Some(0),
        };
        
        // Apply all updates atomically
        let mut execution_state = self.agent_execution.lock()
            .map_err(|e| format!("Failed to access agent_execution lock: {}", e))?;
        
        // Check version for optimistic locking
        let current_version = execution_state.version;
        
        // Apply updates
        execution_state.execution_active = update.execution_active;
        execution_state.execution_id = update.execution_id;
        execution_state.max_steps = update.max_steps;
        execution_state.current_step = update.current_step;
        execution_state.version = current_version + 1; // Increment version
        
        info!("[AppState] Agent execution started atomically with ID: {} (max steps: {})", 
              execution_id, max_steps);
        Ok(())
    }
}

// Fixed tool provider lifecycle management
use std::sync::Weak;

pub struct ToolProviderPool {
    browser_controllers: Arc<Mutex<Vec<BrowserController>>>,
    playwright_drivers: Arc<Mutex<Vec<Weak<PlaywrightDriver>>>>,
    initialization_mutex: Arc<Mutex<()>>,
}

impl ToolProviderPool {
    pub async fn get_or_init_browser_controller(&self) -> Result<BrowserController, String> {
        // Use initialization mutex to prevent concurrent initialization
        let _init_guard = self.initialization_mutex.lock().await;
        
        // Try to get existing controller
        {
            let mut controllers = self.browser_controllers.lock().await;
            if let Some(controller) = controllers.pop() {
                return Ok(controller);
            }
        }
        
        // Need to create new controller
        info!("Creating new browser controller");
        
        // Get or create playwright driver
        let playwright = self.get_or_init_playwright_driver().await?;
        
        // Create controller
        let controller = BrowserController::new(playwright).await
            .map_err(|e| format!("Failed to create browser controller: {}", e))?;
        
        Ok(controller)
    }
    
    pub async fn release_browser_controller(&self, controller: BrowserController) {
        let mut controllers = self.browser_controllers.lock().await;
        if controllers.len() < 3 { // Keep max 3 controllers in pool
            controllers.push(controller);
        } else {
            // Drop excess controller
            drop(controller);
        }
    }
}

// Fixed cancellation token handling
use tokio_util::sync::CancellationToken;

pub struct CancellationManager {
    token: CancellationToken,
    generation: Arc<std::sync::atomic::AtomicU64>,
}

impl CancellationManager {
    pub fn new() -> Self {
        Self {
            token: CancellationToken::new(),
            generation: Arc::new(std::sync::atomic::AtomicU64::new(0)),
        }
    }
    
    pub fn signal_cancel(&self) {
        // Increment generation to invalidate old tokens
        self.generation.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        self.token.cancel();
    }
    
    pub fn get_token(&self) -> (CancellationToken, u64) {
        let gen = self.generation.load(std::sync::atomic::Ordering::SeqCst);
        (self.token.clone(), gen)
    }
    
    pub fn reset(&self) -> CancellationToken {
        // Create new token and increment generation
        self.generation.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let new_token = CancellationToken::new();
        // Atomic swap
        std::mem::replace(&mut *self.token, new_token).clone()
    }
}

// Fixed resource cleanup during cancellation
pub struct ResourceGuard<T> {
    resource: Option<T>,
    cleanup: Option<Box<dyn FnOnce(T) + Send>>,
}

impl<T> ResourceGuard<T> {
    pub fn new(resource: T, cleanup: impl FnOnce(T) + Send + 'static) -> Self {
        Self {
            resource: Some(resource),
            cleanup: Some(Box::new(cleanup)),
        }
    }
    
    pub fn take(mut self) -> T {
        self.cleanup = None; // Disable cleanup
        self.resource.take().expect("Resource already taken")
    }
}

impl<T> Drop for ResourceGuard<T> {
    fn drop(&mut self) {
        if let (Some(resource), Some(cleanup)) = (self.resource.take(), self.cleanup.take()) {
            cleanup(resource);
        }
    }
}

// Example usage in agent execution
pub async fn execute_agent_with_proper_cleanup(
    query: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    // Create resource guards
    let browser_guard = ResourceGuard::new(
        state.get_or_init_browser_controller().await?,
        |controller| {
            // Cleanup browser on drop
            tokio::spawn(async move {
                let _ = controller.close().await;
            });
        }
    );
    
    // Use select! for proper cancellation
    tokio::select! {
        result = execute_agent_logic(query, browser_guard) => {
            result
        }
        _ = state.cancellation_token.cancelled() => {
            info!("Agent execution cancelled");
            Err("Cancelled".to_string())
        }
    }
}