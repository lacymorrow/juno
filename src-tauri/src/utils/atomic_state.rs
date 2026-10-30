use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock, Semaphore};
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

#[allow(clippy::new_without_default)]
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
    #[allow(dead_code)]
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
        let permit = self
            .semaphore
            .acquire()
            .await
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
        let item = if items.is_empty() {
            None
        } else {
            Some(items.remove(0))
        };

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

    /// Check if the queue is empty
    pub async fn is_empty(&self) -> bool {
        let items = self.items.lock().await;
        items.is_empty()
    }
}

/// Atomic execution coordinator to prevent concurrent executions
pub struct AtomicExecutionCoordinator {
    current_execution_id: Arc<RwLock<Option<String>>>,
    execution_semaphore: Arc<Semaphore>,
    #[allow(dead_code)]
    state_manager: Arc<AtomicStateManager>,
    /// The ticket of the newest query to ask for a turn. A waiter holding an
    /// older ticket has been replaced and must not run.
    latest_ticket: AtomicU64,
}

/// How [`AtomicExecutionCoordinator::start_latest`] ended.
pub enum StartOutcome {
    /// This query runs now; the guard ends the turn when dropped.
    Started(ExecutionGuard),
    /// A newer query asked for a turn while this one waited. The newer one
    /// runs instead, so this one is dropped on purpose.
    Superseded,
    /// The turn before it never ended within the wait.
    TimedOut,
}

#[allow(clippy::new_without_default)]
impl AtomicExecutionCoordinator {
    pub fn new() -> Self {
        Self {
            current_execution_id: Arc::new(RwLock::new(None)),
            execution_semaphore: Arc::new(Semaphore::new(1)),
            state_manager: Arc::new(AtomicStateManager::new()),
            latest_ticket: AtomicU64::new(0),
        }
    }

    /// Take a ticket for a new query. Taking one replaces every query still
    /// waiting for its turn: the newest message is the one the person wants
    /// answered.
    pub fn claim_latest(&self) -> u64 {
        self.latest_ticket.fetch_add(1, Ordering::SeqCst) + 1
    }

    /// Wait for the running turn to end, then start this one, unless a newer
    /// query took a ticket in the meantime.
    ///
    /// The wait is on the semaphore the running turn holds, so it ends the
    /// moment that turn drops its guard. It used to be a 3 s poll followed by
    /// a single `try_start`, and a turn that took longer than that to honour
    /// its cancel left the new query in a queue nobody read again: the
    /// person's message sat in the chat with no answer.
    pub async fn start_latest(
        &self,
        ticket: u64,
        execution_id: String,
        wait: std::time::Duration,
    ) -> StartOutcome {
        let permit = match tokio::time::timeout(
            wait,
            self.execution_semaphore.clone().acquire_owned(),
        )
        .await
        {
            Ok(Ok(permit)) => permit,
            // The semaphore is never closed; treat it like a turn that never ended.
            Ok(Err(_)) | Err(_) => return StartOutcome::TimedOut,
        };
        if self.latest_ticket.load(Ordering::SeqCst) != ticket {
            return StartOutcome::Superseded;
        }

        // The permit is the exclusion; the id only names the turn. A previous
        // guard clears it only while it still names that guard's own turn.
        *self.current_execution_id.write().await = Some(execution_id.clone());
        info!("Started execution atomically: {}", execution_id);

        StartOutcome::Started(ExecutionGuard {
            execution_id,
            permit: Some(permit),
            current_execution_id: self.current_execution_id.clone(),
        })
    }

    /// Try to start execution atomically
    pub async fn try_start_execution(
        &self,
        execution_id: String,
    ) -> Result<ExecutionGuard, String> {
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
    #[allow(dead_code)]
    permit: Option<tokio::sync::OwnedSemaphorePermit>,
    current_execution_id: Arc<RwLock<Option<String>>>,
}

impl Drop for ExecutionGuard {
    fn drop(&mut self) {
        // Clear execution ID
        let current_id = self.current_execution_id.clone();
        let execution_id = self.execution_id.clone();

        // Clear it now when the lock is free, so the next turn never finds
        // this one's id still set; otherwise clear it async.
        let cleared_now = match current_id.try_write() {
            Ok(mut current) => {
                if current.as_ref() == Some(&execution_id) {
                    *current = None;
                    debug!("Cleared execution ID on guard drop: {}", execution_id);
                }
                true
            }
            Err(_) => false,
        };
        if !cleared_now {
            if let Ok(handle) = tokio::runtime::Handle::try_current() {
                handle.spawn(async move {
                    let mut current = current_id.write().await;
                    if current.as_ref() == Some(&execution_id) {
                        *current = None;
                        debug!("Cleared execution ID on guard drop: {}", execution_id);
                    }
                });
            } else {
                warn!(
                    "Dropped outside Tokio runtime - async cleanup skipped for execution ID: {}",
                    execution_id
                );
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
        let _permit = self
            .available
            .acquire()
            .await
            .map_err(|e| format!("Failed to acquire resource: {}", e))?;

        let mut resources = self.resources.lock().await;
        resources
            .pop()
            .ok_or_else(|| "No resources available".to_string())
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

    const WAIT: std::time::Duration = std::time::Duration::from_secs(5);

    /// A query sent while a slow turn is still stopping waits for that turn
    /// to end and then runs. It used to give up after 3 s and sit in a queue
    /// nobody read again, so the message got no answer.
    #[tokio::test]
    async fn a_new_query_waits_out_a_slow_stop_and_then_runs() {
        let coordinator = Arc::new(AtomicExecutionCoordinator::new());
        let first = coordinator.claim_latest();
        let StartOutcome::Started(running) =
            coordinator.start_latest(first, "a".to_string(), WAIT).await
        else {
            panic!("the first query must start");
        };

        let second = coordinator.claim_latest();
        let waiter = {
            let coordinator = coordinator.clone();
            tokio::spawn(async move {
                coordinator
                    .start_latest(second, "b".to_string(), WAIT)
                    .await
            })
        };
        tokio::task::yield_now().await;
        assert!(!waiter.is_finished(), "must wait while the first turn runs");

        drop(running);
        // Held while it is checked: dropping it ends "b", and the check would
        // race that cleanup (it failed CI that way).
        let Ok(StartOutcome::Started(_b)) = waiter.await else {
            panic!("the waiting query must start once the first ends");
        };
        assert_eq!(
            coordinator.get_current_execution_id().await.as_deref(),
            Some("b")
        );
    }

    /// Two queries waiting behind one turn: only the newer one runs.
    #[tokio::test]
    async fn only_the_newest_waiting_query_runs() {
        let coordinator = AtomicExecutionCoordinator::new();
        let first = coordinator.claim_latest();
        let StartOutcome::Started(running) =
            coordinator.start_latest(first, "a".to_string(), WAIT).await
        else {
            panic!("the first query must start");
        };

        let older = coordinator.claim_latest();
        let newer = coordinator.claim_latest();
        drop(running);

        assert!(matches!(
            coordinator.start_latest(older, "b".to_string(), WAIT).await,
            StartOutcome::Superseded
        ));
        assert!(matches!(
            coordinator.start_latest(newer, "c".to_string(), WAIT).await,
            StartOutcome::Started(_)
        ));
    }

    #[tokio::test]
    async fn a_turn_that_never_ends_times_the_waiter_out() {
        let coordinator = AtomicExecutionCoordinator::new();
        let first = coordinator.claim_latest();
        let _running = coordinator.start_latest(first, "a".to_string(), WAIT).await;
        let second = coordinator.claim_latest();
        assert!(matches!(
            coordinator
                .start_latest(
                    second,
                    "b".to_string(),
                    std::time::Duration::from_millis(20)
                )
                .await,
            StartOutcome::TimedOut
        ));
    }
}
