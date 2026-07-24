//! Race-condition tests driving the REAL Juno concurrency types (LAC-3056).
//!
//! Every test here exercises `juno_lib` code — the atomic execution coordinator
//! and queue that back `AgentExecutionQueue` in `anthropic.rs`, the
//! `AdvancedMemoryManager`, `AppState`'s execution-state and cancellation
//! paths, and the `ResourcePool` in `utils/resource_manager.rs`. No
//! re-implemented std/tokio stand-ins.

use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::time::{sleep, timeout, Duration};

use juno_lib::agent::core::{Message, Role};
use juno_lib::agent::implementations::memory_manager::{AdvancedMemoryManager, MemoryConfig};
use juno_lib::agent::traits::MemoryManager;
use juno_lib::state::AppState;
use juno_lib::utils::atomic_state::{AtomicExecutionCoordinator, AtomicQueue};
use juno_lib::utils::resource_manager::ResourcePool;

fn user_message(content: impl Into<String>) -> Message {
    Message {
        role: Role::User,
        content: content.into(),
        tool_calls: None,
        tool_call_id: None,
        name: None,
    }
}

/// The real `AgentExecutionQueue` (private in `anthropic.rs`) is a thin wrapper
/// around `AtomicExecutionCoordinator` + `AtomicQueue`. Drive those directly:
/// many tasks race to enqueue and start execution; the coordinator must never
/// allow two executions to overlap, and the queue must never exceed its cap.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_agent_queue_race_conditions() {
    let coordinator = Arc::new(AtomicExecutionCoordinator::new());
    let queue = Arc::new(AtomicQueue::new(10));
    let concurrent = Arc::new(AtomicU32::new(0));
    let started = Arc::new(AtomicU32::new(0));

    let mut handles = vec![];
    for i in 0..10 {
        let coordinator = coordinator.clone();
        let queue = queue.clone();
        let concurrent = concurrent.clone();
        let started = started.clone();

        handles.push(tokio::spawn(async move {
            queue
                .push(format!("query {i}"))
                .await
                .expect("queue accepts up to its configured capacity");

            if let Ok(_guard) = coordinator.try_start_execution(format!("exec_{i}")).await {
                let overlapping = concurrent.fetch_add(1, Ordering::SeqCst);
                assert_eq!(overlapping, 0, "two executions ran concurrently");
                started.fetch_add(1, Ordering::SeqCst);

                assert!(coordinator.is_executing().await);
                let _ = queue.pop().await;
                sleep(Duration::from_millis(10)).await;

                concurrent.fetch_sub(1, Ordering::SeqCst);
            }
        }));
    }

    for handle in handles {
        handle.await.expect("task panicked");
    }

    let total_started = started.load(Ordering::SeqCst);
    assert!(total_started >= 1, "coordinator never granted execution");
    assert!(total_started <= 10, "more executions than submissions");
    // Everything that executed popped its query; the rest stay queued.
    assert_eq!(queue.len().await, (10 - total_started) as usize);
}

/// Real `AdvancedMemoryManager` under concurrent writers and readers, with
/// auto-pruning enabled and a small cap — the configuration `AppState` uses,
/// shrunk so pruning actually triggers during the test.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_memory_manager_concurrent_operations() {
    let config = MemoryConfig {
        max_messages: 5,
        max_tokens: 100_000,
        min_messages_to_keep: 2,
        auto_prune: true,
        enable_summarization: false,
        summarization_batch_size: 5,
        enable_metrics: false,
        enable_summary_cache: false,
    };
    let manager = Arc::new(tokio::sync::Mutex::new(AdvancedMemoryManager::with_config(
        config,
    )));

    let mut handles = vec![];

    for i in 0..20 {
        let manager = manager.clone();
        handles.push(tokio::spawn(async move {
            let mut mgr = manager.lock().await;
            mgr.add_message(user_message(format!("message {i}")))
                .await
                .expect("add_message failed");
        }));
    }

    for _ in 0..5 {
        let manager = manager.clone();
        handles.push(tokio::spawn(async move {
            let mgr = manager.lock().await;
            let msgs = mgr.get_messages().await.expect("get_messages failed");
            // Reads interleaved with pruning writers must still see a bounded view.
            assert!(msgs.len() <= 20, "read more messages than were ever added");
        }));
    }

    for handle in handles {
        handle.await.expect("task panicked");
    }

    let mgr = manager.lock().await;
    let final_messages = mgr.get_messages().await.expect("get_messages failed");
    assert!(
        !final_messages.is_empty(),
        "pruning removed every message despite min_messages_to_keep"
    );
    assert!(
        final_messages.len() <= 5,
        "auto-prune failed to enforce max_messages: {} messages remain",
        final_messages.len()
    );
}

/// Real `AppState` execution state: `mark_agent_execution_started_with_steps`
/// writes four fields under one lock. Concurrent writers must never leave a
/// torn state where `execution_id` and `max_steps` come from different writers.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_state_update_atomicity() {
    let state = AppState::new(None);

    let mut handles = vec![];

    for i in 0..10u32 {
        let state = state.clone();
        handles.push(tokio::spawn(async move {
            state
                .mark_agent_execution_started_with_steps(format!("exec_{i}"), (i + 1) * 100)
                .expect("mark_agent_execution_started_with_steps failed");
        }));
    }

    for _ in 0..10 {
        let state = state.clone();
        handles.push(tokio::spawn(async move {
            let snapshot = state
                .agent_execution
                .lock()
                .expect("agent_execution lock poisoned")
                .clone();
            if let (Some(id), Some(max_steps)) = (snapshot.execution_id, snapshot.max_steps) {
                let i: u32 = id
                    .strip_prefix("exec_")
                    .and_then(|s| s.parse().ok())
                    .expect("unexpected execution_id format");
                assert_eq!(
                    max_steps,
                    (i + 1) * 100,
                    "torn state: execution_id {id} paired with max_steps from another writer"
                );
            }
        }));
    }

    for handle in handles {
        handle.await.expect("task panicked");
    }

    // Exactly one writer's full update wins; the state must be internally consistent.
    assert!(state.is_agent_executing());
    assert!(state.get_current_agent_execution_id().is_some());

    state.mark_agent_execution_finished();
    assert!(!state.is_agent_executing());
    assert!(state.get_current_agent_execution_id().is_none());
}

/// Real `AppState` cancellation: an executor honours `signal_cancel()` via the
/// watch channel while a canceller races it, and concurrent signal/reset calls
/// leave the channel in a consistent final state.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_cancellation_token_races() {
    let state = AppState::new(None);
    let completed = Arc::new(AtomicU32::new(0));
    let cancelled = Arc::new(AtomicU32::new(0));

    let exec_state = state.clone();
    let exec_completed = completed.clone();
    let exec_cancelled = cancelled.clone();
    let executor = tokio::spawn(async move {
        let mut cancel_rx = exec_state.cancel_rx.clone();
        tokio::select! {
            _ = async {
                for _ in 0..200 {
                    sleep(Duration::from_millis(5)).await;
                }
                exec_completed.fetch_add(1, Ordering::SeqCst);
            } => {}
            _ = cancel_rx.wait_for(|cancelled| *cancelled) => {
                exec_cancelled.fetch_add(1, Ordering::SeqCst);
            }
        }
    });

    let cancel_state = state.clone();
    let canceller = tokio::spawn(async move {
        sleep(Duration::from_millis(30)).await;
        cancel_state.signal_cancel();
    });

    timeout(Duration::from_secs(5), async {
        executor.await.expect("executor panicked");
        canceller.await.expect("canceller panicked");
    })
    .await
    .expect("cancellation was not observed within timeout");

    assert_eq!(
        cancelled.load(Ordering::SeqCst),
        1,
        "executor did not observe the cancel signal"
    );
    assert_eq!(completed.load(Ordering::SeqCst), 0, "executor ran to completion despite cancel");

    // Racing signal/reset from many tasks must not poison the channel.
    let mut handles = vec![];
    for i in 0..10 {
        let state = state.clone();
        handles.push(tokio::spawn(async move {
            if i % 2 == 0 {
                state.signal_cancel();
            } else {
                state.reset_cancel();
            }
        }));
    }
    for handle in handles {
        handle.await.expect("task panicked");
    }

    state.reset_cancel();
    assert!(!*state.cancel_rx.borrow(), "reset_cancel failed after racing updates");
}

/// Real `ResourcePool` from `utils/resource_manager.rs`: workers race `get()`
/// and `add()`; a resource handed out must never be held by two workers, and
/// every resource must end up back in the pool.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_resource_pool_concurrent_access() {
    struct TestResource {
        id: usize,
        in_use: Arc<AtomicU32>,
    }

    let pool = Arc::new(ResourcePool::new(
        "test_pool".to_string(),
        3,
        Duration::from_secs(60),
    ));
    let checkouts = Arc::new(AtomicUsize::new(0));

    let holders: Vec<Arc<AtomicU32>> = (0..3).map(|_| Arc::new(AtomicU32::new(0))).collect();
    for (id, in_use) in holders.iter().enumerate() {
        pool.add(
            TestResource {
                id,
                in_use: in_use.clone(),
            },
            |_| {},
        )
        .await
        .unwrap_or_else(|_| panic!("pool rejected resource {id} during setup"));
    }
    assert_eq!(pool.size().await, 3);

    let mut handles = vec![];
    for worker in 0..10 {
        let pool = pool.clone();
        let checkouts = checkouts.clone();
        handles.push(tokio::spawn(async move {
            // `get()` is non-blocking; retry briefly like a real caller would.
            for _ in 0..50 {
                if let Some(resource) = pool.get().await {
                    let previous = resource.in_use.fetch_add(1, Ordering::SeqCst);
                    assert_eq!(
                        previous, 0,
                        "resource {} checked out by two workers at once",
                        resource.id
                    );
                    checkouts.fetch_add(1, Ordering::SeqCst);

                    sleep(Duration::from_millis(5)).await;

                    resource.in_use.fetch_sub(1, Ordering::SeqCst);
                    let id = resource.id;
                    pool.add(resource, |_| {})
                        .await
                        .unwrap_or_else(|_| panic!("pool rejected returned resource {id}"));
                    return;
                }
                sleep(Duration::from_millis(2)).await;
            }
            panic!("worker {worker} never acquired a resource");
        }));
    }

    for handle in handles {
        handle.await.expect("worker panicked");
    }

    assert_eq!(pool.size().await, 3, "resources leaked from pool");
    assert_eq!(checkouts.load(Ordering::SeqCst), 10, "not every worker got a resource");
}

/// Deadlock check across `AppState`'s real lock landscape: std mutexes
/// (`agent_execution`), tokio mutexes (`memory_manager`,
/// `pending_tool_approvals`), and the cancel watch channel, hammered from
/// multiple threads. The deadlock rules in `src-tauri/CLAUDE.md` (never hold
/// one lock while taking another out of order) must hold — a violation shows
/// up here as the 10s timeout firing. The browser-controller variant of this
/// path needs a live Playwright driver, so it is exercised through the same
/// check-init-recheck locks (`memory_manager` here) rather than a real browser.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_no_deadlocks() {
    let state = AppState::new(None);
    let completed = Arc::new(AtomicU32::new(0));

    let mut handles = vec![];

    for i in 0..4 {
        // Path A: execution-state std mutex.
        let s = state.clone();
        let c = completed.clone();
        handles.push(tokio::spawn(async move {
            for j in 0..10 {
                s.mark_agent_execution_started(format!("exec_{i}_{j}"))
                    .expect("mark started failed");
                sleep(Duration::from_millis(1)).await;
                s.mark_agent_execution_finished();
            }
            c.fetch_add(1, Ordering::SeqCst);
        }));

        // Path B: memory-manager tokio mutex (the lazily-initialised async state
        // that check-init-recheck protects).
        let s = state.clone();
        let c = completed.clone();
        handles.push(tokio::spawn(async move {
            for j in 0..10 {
                let manager = s.get_memory_manager().await;
                let mut mgr = manager.lock().await;
                mgr.add_message(user_message(format!("deadlock probe {i}-{j}")))
                    .await
                    .expect("add_message failed");
            }
            c.fetch_add(1, Ordering::SeqCst);
        }));

        // Path C: cancel watch channel writes racing both lock families.
        let s = state.clone();
        let c = completed.clone();
        handles.push(tokio::spawn(async move {
            for _ in 0..10 {
                s.signal_cancel();
                sleep(Duration::from_millis(1)).await;
                s.reset_cancel();
            }
            c.fetch_add(1, Ordering::SeqCst);
        }));

        // Path D: pending tool approvals tokio mutex.
        let s = state.clone();
        let c = completed.clone();
        handles.push(tokio::spawn(async move {
            for j in 0..10 {
                let key = format!("approval_{i}_{j}");
                {
                    let mut approvals = s.pending_tool_approvals.lock().await;
                    approvals.remove(&key);
                }
                sleep(Duration::from_millis(1)).await;
            }
            c.fetch_add(1, Ordering::SeqCst);
        }));
    }

    let all_done = timeout(Duration::from_secs(10), async {
        for handle in handles {
            handle.await.expect("task panicked");
        }
    })
    .await;

    assert!(all_done.is_ok(), "deadlock: AppState lock paths did not complete in 10s");
    assert_eq!(completed.load(Ordering::SeqCst), 16, "not all lock paths completed");
}

/// End-to-end: the real submission pipeline shape — queries flow through
/// `AtomicQueue`, execution is serialised by `AtomicExecutionCoordinator`,
/// results land in `AppState`'s real memory manager, and `signal_cancel()`
/// stops the executor.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_agent_execution_integration() {
    let state = AppState::new(None);
    let queue = Arc::new(AtomicQueue::new(10));
    let coordinator = Arc::new(AtomicExecutionCoordinator::new());
    let executed = Arc::new(AtomicU32::new(0));

    // Producers: users submitting queries.
    let mut producers = vec![];
    for i in 0..5u64 {
        let queue = queue.clone();
        producers.push(tokio::spawn(async move {
            sleep(Duration::from_millis(i * 10)).await;
            queue
                .push(format!("user query {i}"))
                .await
                .expect("queue rejected query under capacity");
        }));
    }

    // Executor: drains the queue exactly like `AgentExecutionQueue` does —
    // one execution at a time via the coordinator, memory written per query,
    // cancellation honoured via AppState's watch channel.
    let exec_state = state.clone();
    let exec_queue = queue.clone();
    let exec_coordinator = coordinator.clone();
    let exec_executed = executed.clone();
    let executor = tokio::spawn(async move {
        let mut cancel_rx = exec_state.cancel_rx.clone();
        loop {
            if *cancel_rx.borrow() {
                break;
            }

            let Some(query) = exec_queue.pop().await else {
                tokio::select! {
                    _ = sleep(Duration::from_millis(5)) => continue,
                    _ = cancel_rx.wait_for(|c| *c) => break,
                }
            };

            let guard = exec_coordinator
                .try_start_execution(format!("exec_for_{query}"))
                .await;
            let Ok(_guard) = guard else {
                continue;
            };
            assert!(exec_coordinator.is_executing().await);

            let manager = exec_state.get_memory_manager().await;
            {
                let mut mgr = manager.lock().await;
                mgr.add_message(user_message(format!("Executing: {query}")))
                    .await
                    .expect("add_message failed");
            }
            sleep(Duration::from_millis(20)).await;
            exec_executed.fetch_add(1, Ordering::SeqCst);
        }
    });

    for producer in producers {
        producer.await.expect("producer panicked");
    }

    // Give the executor time to drain, then cancel through the real AppState path.
    sleep(Duration::from_millis(400)).await;
    state.signal_cancel();

    timeout(Duration::from_secs(5), executor)
        .await
        .expect("executor ignored cancel signal")
        .expect("executor panicked");

    let final_count = executed.load(Ordering::SeqCst);
    assert!(final_count > 0, "no queries executed");
    assert!(final_count <= 5, "executed more queries than were submitted");

    let manager = state.get_memory_manager().await;
    let mgr = manager.lock().await;
    let messages = mgr.get_messages().await.expect("get_messages failed");
    let executed_messages = messages
        .iter()
        .filter(|m| m.content.starts_with("Executing:"))
        .count();
    assert_eq!(
        executed_messages as u32, final_count,
        "memory manager records do not match executed count"
    );
}
