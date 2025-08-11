use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock, Semaphore, OwnedSemaphorePermit};
use tracing::{debug, info, warn};

/// Atomic state manager for preventing race conditions
pub struct AtomicStateManager {
    /// Counter for generating unique IDs atomically
    id_counter: AtomicU64,
    
    /// Flag for tracking if an operation is in progress
    operation_in_progress: AtomicBool,
    
    /// Version counter for optimistic locking
    version: AtomicU64,
}

impl AtomicStateManager {
    pub fn new() -> Self {
        Self {
            id_counter: AtomicU64::new(0),
            operation_in_progress: AtomicBool::new(false),
            version: AtomicU64::new(0),
        }
    }
    
    /// Generate a unique ID atomically
    pub fn generate_id(&self) -> u64 {
        self.id_counter.fetch_add(1, Ordering::SeqCst)
    }
    
    /// Try to start an operation atomically
    pub fn try_start_operation(&self) -> bool {
        self.operation_in_progress
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::Relaxed)
            .is_ok()
    }
    
    /// End an operation atomically
    pub fn end_operation(&self) {
        self.operation_in_progress.store(false, Ordering::SeqCst);
    }
    
    /// Get current version for optimistic locking
    pub fn get_version(&self) -> u64 {
        self.version.load(Ordering::SeqCst)
    }
    
    /// Increment version after successful update
    pub fn increment_version(&self) {
        self.version.fetch_add(1, Ordering::SeqCst);
    }
}

/// Thread-safe queue with atomic operations
pub struct AtomicQueue<T> {
    items: Arc<Mutex<Vec<T>>>,
    semaphore: Arc<Semaphore>,
    max_size: usize,
}

impl<T> AtomicQueue<T> {
    pub fn new(max_size: usize) -> Self {
        Self {
            items: Arc::new(Mutex::new(Vec::new())),
            semaphore: Arc::new(Semaphore::new(max_size)),
            max_size,
        }
    }
    
    /// Add item to queue atomically
    pub async fn push(&self, item: T) -> Result<(), String> {
        // Acquire permit first to ensure we don't exceed max size
        let permit = self.semaphore.acquire().await
            .map_err(|e| format!("Failed to acquire queue permit: {}", e))?;
        
        let mut items = self.items.lock().await;
        if items.len() >= self.max_size {
            drop(permit); // Release permit if queue is full
            return Err("Queue is full".to_string());
        }
        
        items.push(item);
        drop(items);
        drop(permit);
        Ok(())
    }
    
    /// Remove and return first item atomically
    pub async fn pop(&self) -> Option<T> {
        let mut items = self.items.lock().await;
        let item = if items.is_empty() { None } else { Some(items.remove(0)) };
        
        // Release permit if we removed an item
        if item.is_some() {
            self.semaphore.add_permits(1);
        }
        
        item
    }
    
    /// Clear all items atomically
    pub async fn clear(&self) {
        let mut items = self.items.lock().await;
        let count = items.len();
        items.clear();
        
        // Release all permits
        if count > 0 {
            self.semaphore.add_permits(count);
        }
    }
    
    /// Get current size
    pub async fn len(&self) -> usize {
        let items = self.items.lock().await;
        items.len()
    }
}

/// Atomic execution coordinator to prevent concurrent executions
pub struct AtomicExecutionCoordinator {
    current_execution_id: Arc<RwLock<Option<String>>>,
    execution_semaphore: Arc<Semaphore>,
    state_manager: Arc<AtomicStateManager>,
}

impl AtomicExecutionCoordinator {
    pub fn new() -> Self {
        Self {
            current_execution_id: Arc::new(RwLock::new(None)),
            execution_semaphore: Arc::new(Semaphore::new(1)),
            state_manager: Arc::new(AtomicStateManager::new()),
        }
    }
    
    /// Try to start execution atomically
    pub async fn try_start_execution(&self, execution_id: String) -> Result<ExecutionGuard, String> {
        // Try to acquire semaphore as owned permit
        let permit = match self.execution_semaphore.clone().try_acquire_owned() {
            Ok(permit) => permit,
            Err(_) => return Err("Another execution is already in progress".to_string()),
        };
        
        // Atomically set current execution ID
        let mut current = self.current_execution_id.write().await;
        if current.is_some() {
            drop(permit);
            return Err("Execution ID already set - possible race condition".to_string());
        }
        
        *current = Some(execution_id.clone());
        drop(current);
        
        info!("Started execution atomically: {}", execution_id);
        
        Ok(ExecutionGuard {
            execution_id,
            permit: Some(permit),
            current_execution_id: self.current_execution_id.clone(),
        })
    }
    
    /// Check if execution is in progress
    pub async fn is_executing(&self) -> bool {
        let current = self.current_execution_id.read().await;
        current.is_some()
    }
    
    /// Get current execution ID
    pub async fn get_current_execution_id(&self) -> Option<String> {
        let current = self.current_execution_id.read().await;
        current.clone()
    }
}

/// RAII guard for execution cleanup
pub struct ExecutionGuard {
    execution_id: String,
    permit: Option<tokio::sync::OwnedSemaphorePermit>,
    current_execution_id: Arc<RwLock<Option<String>>>,
}

impl Drop for ExecutionGuard {
    fn drop(&mut self) {
        // Clear execution ID
        let current_id = self.current_execution_id.clone();
        let execution_id = self.execution_id.clone();
        
        // Check if we're in a Tokio runtime before spawning
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(async move {
                let mut current = current_id.write().await;
                if current.as_ref() == Some(&execution_id) {
                    *current = None;
                    debug!("Cleared execution ID on guard drop: {}", execution_id);
                }
            });
        } else {
            // Try synchronous cleanup with try_write
            if let Ok(mut current) = current_id.try_write() {
                if current.as_ref() == Some(&execution_id) {
                    *current = None;
                    debug!("Cleared execution ID on guard drop (sync): {}", execution_id);
                }
            } else {
                warn!("Dropped outside Tokio runtime - async cleanup skipped for execution ID: {}", execution_id);
            }
        }
        
        // Permit drops automatically, releasing semaphore
        info!("Execution guard dropped for: {}", self.execution_id);
    }
}

/// Thread-safe resource pool with proper cleanup
pub struct ResourcePool<T> {
    resources: Arc<Mutex<Vec<T>>>,
    available: Arc<Semaphore>,
    max_resources: usize,
}

impl<T> ResourcePool<T> {
    pub fn new(max_resources: usize) -> Self {
        Self {
            resources: Arc::new(Mutex::new(Vec::new())),
            available: Arc::new(Semaphore::new(0)),
            max_resources,
        }
    }
    
    /// Add resource to pool
    pub async fn add(&self, resource: T) -> Result<(), String> {
        let mut resources = self.resources.lock().await;
        if resources.len() >= self.max_resources {
            return Err("Resource pool is full".to_string());
        }
        
        resources.push(resource);
        self.available.add_permits(1);
        Ok(())
    }
    
    /// Acquire resource from pool
    pub async fn acquire(&self) -> Result<T, String> {
        // Wait for available resource
        let _permit = self.available.acquire().await
            .map_err(|e| format!("Failed to acquire resource: {}", e))?;
        
        let mut resources = self.resources.lock().await;
        resources.pop().ok_or_else(|| "No resources available".to_string())
    }
    
    /// Return resource to pool
    pub async fn release(&self, resource: T) {
        let mut resources = self.resources.lock().await;
        if resources.len() < self.max_resources {
            resources.push(resource);
            self.available.add_permits(1);
        } else {
            warn!("Resource pool overflow, dropping resource");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_atomic_state_manager() {
        let manager = AtomicStateManager::new();
        
        // Test ID generation
        let id1 = manager.generate_id();
        let id2 = manager.generate_id();
        assert_eq!(id1, 0);
        assert_eq!(id2, 1);
        
        // Test operation atomicity
        assert!(manager.try_start_operation());
        assert!(!manager.try_start_operation()); // Should fail
        manager.end_operation();
        assert!(manager.try_start_operation()); // Should succeed again
    }
    
    #[tokio::test]
    async fn test_atomic_queue() {
        let queue = AtomicQueue::new(2);
        
        // Test push and pop
        assert!(queue.push(1).await.is_ok());
        assert!(queue.push(2).await.is_ok());
        
        assert_eq!(queue.pop().await, Some(1));
        assert_eq!(queue.pop().await, Some(2));
        assert_eq!(queue.pop().await, None);
    }
    
    #[tokio::test]
    async fn test_execution_coordinator() {
        let coordinator = AtomicExecutionCoordinator::new();
        
        // Test exclusive execution
        let guard1 = coordinator.try_start_execution("exec1".to_string()).await;
        assert!(guard1.is_ok());
        
        let guard2 = coordinator.try_start_execution("exec2".to_string()).await;
        assert!(guard2.is_err());
        
        drop(guard1);
        
        // Small delay to allow async cleanup
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        
        let guard3 = coordinator.try_start_execution("exec3".to_string()).await;
        assert!(guard3.is_ok());
    }
}