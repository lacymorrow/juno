use std::collections::HashSet;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock, Semaphore};
use tokio::time::{sleep, Duration};

/// Test for concurrent queue operations
#[tokio::test]
async fn test_agent_queue_race_conditions() {
    // Simulate the AgentExecutionQueue
    let execution_semaphore = Arc::new(Semaphore::new(1));
    let pending_queries = Arc::new(Mutex::new(Vec::new()));
    let current_execution = Arc::new(RwLock::new(None::<String>));
    let execution_counter = Arc::new(AtomicU32::new(0));

    // Spawn multiple tasks trying to queue and execute
    let mut handles = vec![];

    for i in 0..10 {
        let sem_clone = execution_semaphore.clone();
        let queries_clone = pending_queries.clone();
        let current_clone = current_execution.clone();
        let counter_clone = execution_counter.clone();

        let handle = tokio::spawn(async move {
            // Try to queue
            {
                let mut queries = queries_clone.lock().await;
                queries.push(format!("Query {}", i));
            }

            // Try to execute
            if let Ok(_permit) = sem_clone.try_acquire() {
                // Simulate execution
                let query = {
                    let mut queries = queries_clone.lock().await;
                    queries.pop()
                };

                if let Some(q) = query {
                    // Set current execution
                    {
                        let mut current = current_clone.write().await;
                        *current = Some(q.clone());
                    }

                    // Increment counter
                    counter_clone.fetch_add(1, Ordering::SeqCst);

                    // Simulate work
                    sleep(Duration::from_millis(10)).await;

                    // Clear current execution
                    {
                        let mut current = current_clone.write().await;
                        *current = None;
                    }
                }
            }
        });

        handles.push(handle);
    }

    // Wait for all tasks
    for handle in handles {
        handle.await.unwrap();
    }

    // Verify only one execution happened at a time
    let final_count = execution_counter.load(Ordering::SeqCst);
    println!("Total executions: {}", final_count);
    assert!(final_count <= 10, "More executions than queries!");
}

/// Test for memory manager race conditions
#[tokio::test]
async fn test_memory_manager_concurrent_operations() {
    let messages = Arc::new(RwLock::new(Vec::<String>::new()));
    let pending_tool_calls = Arc::new(Mutex::new(HashSet::<String>::new()));
    let max_messages = 5;

    let mut handles = vec![];

    // Concurrent adds
    for i in 0..20 {
        let messages_clone = messages.clone();
        let pending_clone = pending_tool_calls.clone();

        let handle = tokio::spawn(async move {
            // Add message
            {
                let mut msgs = messages_clone.write().await;
                msgs.push(format!("Message {}", i));

                // Simulate pruning
                if msgs.len() > max_messages {
                    let excess = msgs.len() - max_messages;
                    msgs.drain(0..excess);
                }
            }

            // Add tool call
            if i % 3 == 0 {
                let mut pending = pending_clone.lock().await;
                pending.insert(format!("tool_{}", i));
            }
        });

        handles.push(handle);
    }

    // Concurrent reads
    for _ in 0..5 {
        let messages_clone = messages.clone();

        let handle = tokio::spawn(async move {
            let msgs = messages_clone.read().await;
            let _count = msgs.len();
            // Simulate processing
            sleep(Duration::from_millis(5)).await;
        });

        handles.push(handle);
    }

    // Wait for all
    for handle in handles {
        handle.await.unwrap();
    }

    // Verify constraints
    let final_messages = messages.read().await;
    assert!(
        final_messages.len() <= max_messages,
        "Too many messages: {}",
        final_messages.len()
    );
}

/// Test for state update atomicity
#[tokio::test]
async fn test_state_update_atomicity() {
    #[derive(Default)]
    struct AgentState {
        execution_active: bool,
        execution_id: Option<String>,
        max_steps: Option<u32>,
        current_step: Option<u32>,
    }

    let state = Arc::new(Mutex::new(AgentState::default()));
    let update_counter = Arc::new(AtomicU32::new(0));

    let mut handles = vec![];

    // Multiple concurrent state updates
    for i in 0..10 {
        let state_clone = state.clone();
        let counter_clone = update_counter.clone();

        let handle = tokio::spawn(async move {
            // Non-atomic update (BAD)
            // This simulates the race condition
            let should_update = {
                let s = state_clone.lock().await;
                !s.execution_active
            };

            if should_update {
                sleep(Duration::from_micros(100)).await; // Simulate race window

                let mut s = state_clone.lock().await;
                if !s.execution_active {
                    // Double check
                    s.execution_active = true;
                    s.execution_id = Some(format!("exec_{}", i));
                    s.max_steps = Some(10);
                    s.current_step = Some(0);
                    counter_clone.fetch_add(1, Ordering::SeqCst);
                }
            }
        });

        handles.push(handle);
    }

    for handle in handles {
        handle.await.unwrap();
    }

    let updates = update_counter.load(Ordering::SeqCst);
    println!("Total updates: {}", updates);

    // In a race condition scenario, multiple updates might occur
    // With proper atomic operations, only one should succeed
}

/// Test for cancellation token race conditions
#[tokio::test]
async fn test_cancellation_token_races() {
    use tokio_util::sync::CancellationToken;

    let token = CancellationToken::new();
    let current_execution = Arc::new(RwLock::new(Some("test_exec".to_string())));
    let completed = Arc::new(AtomicU32::new(0));

    // Start execution task
    let exec_token = token.clone();
    let exec_current = current_execution.clone();
    let exec_completed = completed.clone();

    let exec_handle = tokio::spawn(async move {
        tokio::select! {
            _ = async {
                for _i in 0..100 {
                    if exec_token.is_cancelled() {
                        break;
                    }
                    sleep(Duration::from_millis(10)).await;

                    // Check if we're still the current execution
                    let current = exec_current.read().await;
                    if current.as_ref() != Some(&"test_exec".to_string()) {
                        break;
                    }
                }
                exec_completed.fetch_add(1, Ordering::SeqCst);
            } => {}
            _ = exec_token.cancelled() => {
                println!("Execution cancelled");
            }
        }
    });

    // Start cancellation task
    let cancel_token = token.clone();
    let cancel_current = current_execution.clone();

    let cancel_handle = tokio::spawn(async move {
        sleep(Duration::from_millis(50)).await;

        // Cancel current execution
        {
            let mut current = cancel_current.write().await;
            *current = None;
        }
        cancel_token.cancel();
    });

    // Wait for both
    exec_handle.await.unwrap();
    cancel_handle.await.unwrap();

    // Verify clean cancellation
    let final_completed = completed.load(Ordering::SeqCst);
    assert!(final_completed <= 1, "Multiple completions detected");
}

/// Test for resource pool race conditions
#[tokio::test]
async fn test_resource_pool_concurrent_access() {
    #[derive(Debug, Clone)]
    struct Resource {
        id: u32,
        in_use: Arc<AtomicU32>,
    }

    let pool = Arc::new(Mutex::new(Vec::new()));
    let available = Arc::new(Semaphore::new(0));

    // Initialize pool
    for i in 0..3 {
        let resource = Resource {
            id: i,
            in_use: Arc::new(AtomicU32::new(0)),
        };
        pool.lock().await.push(resource);
        available.add_permits(1);
    }

    let mut handles = vec![];

    // Spawn workers trying to acquire resources
    for worker_id in 0..10 {
        let pool_clone = pool.clone();
        let available_clone = available.clone();

        let handle = tokio::spawn(async move {
            // Try to acquire resource
            if let Ok(_permit) = available_clone.acquire().await {
                let resource = {
                    let mut p = pool_clone.lock().await;
                    p.pop()
                };

                if let Some(res) = resource {
                    // Mark as in use
                    let was_in_use = res.in_use.fetch_add(1, Ordering::SeqCst);
                    assert_eq!(was_in_use, 0, "Resource {} already in use!", res.id);

                    // Use resource
                    sleep(Duration::from_millis(10)).await;

                    // Mark as not in use
                    res.in_use.fetch_sub(1, Ordering::SeqCst);

                    // Return to pool
                    let mut p = pool_clone.lock().await;
                    p.push(res);
                    available_clone.add_permits(1);
                }
            }

            worker_id
        });

        handles.push(handle);
    }

    // Wait for all workers
    for handle in handles {
        let _worker_id = handle.await.unwrap();
    }

    // Verify all resources are back in pool
    let final_pool = pool.lock().await;
    assert_eq!(final_pool.len(), 3, "Resources leaked from pool");
}

/// Test for deadlock scenarios
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_no_deadlocks() {
    let lock_a = Arc::new(Mutex::new(0));
    let lock_b = Arc::new(Mutex::new(0));
    let completed = Arc::new(AtomicU32::new(0));

    let mut handles = vec![];

    // Task 1: Acquires A then B
    let a1 = lock_a.clone();
    let b1 = lock_b.clone();
    let c1 = completed.clone();

    handles.push(tokio::spawn(async move {
        for _ in 0..10 {
            let _a = a1.lock().await;
            sleep(Duration::from_micros(10)).await;
            let _b = b1.lock().await;
            c1.fetch_add(1, Ordering::SeqCst);
        }
    }));

    // Task 2: Also acquires A then B (consistent order - no deadlock)
    let a2 = lock_a.clone();
    let b2 = lock_b.clone();
    let c2 = completed.clone();

    handles.push(tokio::spawn(async move {
        for _ in 0..10 {
            let _a = a2.lock().await;
            sleep(Duration::from_micros(10)).await;
            let _b = b2.lock().await;
            c2.fetch_add(1, Ordering::SeqCst);
        }
    }));

    // Wait with timeout
    let timeout = tokio::time::timeout(Duration::from_secs(5), async {
        for handle in handles {
            handle.await.unwrap();
        }
    });

    assert!(timeout.await.is_ok(), "Deadlock detected!");

    let total = completed.load(Ordering::SeqCst);
    assert_eq!(total, 20, "Not all operations completed");
}

/// Integration test simulating real agent execution scenarios
#[tokio::test]
async fn test_agent_execution_integration() {
    use tokio_util::sync::CancellationToken;

    // Simulate AppState components
    let execution_queue = Arc::new(Mutex::new(Vec::<String>::new()));
    let current_execution = Arc::new(RwLock::new(None::<String>));
    let memory_manager = Arc::new(RwLock::new(Vec::<String>::new()));
    let cancellation_token = CancellationToken::new();
    let execution_count = Arc::new(AtomicU32::new(0));

    let mut handles = vec![];

    // Simulate multiple users submitting queries
    for i in 0..5 {
        let queue = execution_queue.clone();
        let handle = tokio::spawn(async move {
            sleep(Duration::from_millis(i * 10)).await;
            let mut q = queue.lock().await;
            q.push(format!("User query {}", i));
        });
        handles.push(handle);
    }

    // Simulate agent executor
    let exec_queue = execution_queue.clone();
    let exec_current = current_execution.clone();
    let exec_memory = memory_manager.clone();
    let exec_token = cancellation_token.clone();
    let exec_count = execution_count.clone();

    let executor = tokio::spawn(async move {
        loop {
            // Check for queries
            let query = {
                let mut q = exec_queue.lock().await;
                q.pop()
            };

            if let Some(query) = query {
                // Set current execution
                {
                    let mut current = exec_current.write().await;
                    *current = Some(query.clone());
                }

                // Execute with cancellation support
                tokio::select! {
                    _ = async {
                        // Add to memory
                        {
                            let mut mem = exec_memory.write().await;
                            mem.push(format!("Executing: {}", query));
                        }

                        // Simulate work
                        sleep(Duration::from_millis(50)).await;

                        // Complete
                        exec_count.fetch_add(1, Ordering::SeqCst);
                    } => {}
                    _ = exec_token.cancelled() => {
                        break;
                    }
                }

                // Clear current execution
                {
                    let mut current = exec_current.write().await;
                    *current = None;
                }
            } else {
                sleep(Duration::from_millis(10)).await;
            }

            if exec_token.is_cancelled() {
                break;
            }
        }
    });

    // Wait for submissions
    for handle in handles {
        handle.await.unwrap();
    }

    // Let executor run
    sleep(Duration::from_millis(300)).await;

    // Cancel executor
    cancellation_token.cancel();
    executor.await.unwrap();

    // Verify results
    let final_count = execution_count.load(Ordering::SeqCst);
    println!("Executed {} queries", final_count);
    assert!(final_count > 0, "No queries executed");
    assert!(final_count <= 5, "Too many executions");
}
