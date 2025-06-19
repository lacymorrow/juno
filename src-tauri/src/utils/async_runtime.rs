use std::future::Future;
use tracing::error;

/// Safely spawn async tasks within event listeners to prevent "no reactor running" panics.
///
/// This function handles the complexity of Tauri's runtime context by:
/// 1. First checking if we're already in a Tokio runtime context
/// 2. Using tokio::spawn directly if we are (most efficient)
/// 3. Falling back to tauri's async runtime if not
///
/// This prevents the common "no reactor running" panic that occurs when
/// `tauri::async_runtime::spawn()` is called from event listeners.
pub fn safe_spawn_async_task<F, Fut>(task: F)
where
    F: FnOnce() -> Fut + Send + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    // Check if we're already in a Tokio runtime context
    if let Ok(handle) = tokio::runtime::Handle::try_current() {
        // We're in a runtime context, use tokio::spawn directly (most efficient)
        let _ = handle.spawn(task());
    } else {
        // We're not in a runtime context, use tauri's async runtime
        // Note: tauri::async_runtime::spawn returns a JoinHandle, not a Result
        let _ = tauri::async_runtime::spawn(task());
    }
}

/// Spawn an async task with a timeout to prevent hanging operations.
///
/// This is useful for operations that might deadlock or hang indefinitely.
/// If the timeout is reached, the task is cancelled and an error is logged.
pub fn safe_spawn_async_task_with_timeout<F, Fut>(
    task: F,
    timeout_duration: std::time::Duration,
    operation_name: &'static str,
)
where
    F: FnOnce() -> Fut + Send + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    safe_spawn_async_task(move || async move {
        match tokio::time::timeout(timeout_duration, task()).await {
            Ok(_) => {
                // Task completed within timeout
            }
            Err(_) => {
                error!("Async task '{}' timed out after {:?}", operation_name, timeout_duration);
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tokio::sync::mpsc;

    #[tokio::test]
    async fn test_safe_spawn_in_runtime_context() {
        let (tx, mut rx) = mpsc::channel(1);

        safe_spawn_async_task(move || async move {
            tx.send("test").await.unwrap();
        });

        // Wait for the message to be sent
        assert_eq!(rx.recv().await, Some("test"));
    }

    #[test]
    fn test_safe_spawn_without_runtime_context() {
        // This test runs outside of tokio context to test the fallback
        let (tx, rx) = std::sync::mpsc::channel();

        safe_spawn_async_task(move || async move {
            tx.send("test").unwrap();
        });

        // Give some time for the spawned task to complete
        std::thread::sleep(Duration::from_millis(100));

        assert_eq!(rx.try_recv().unwrap(), "test");
    }

    #[tokio::test]
    async fn test_safe_spawn_with_timeout() {
        let (tx, mut rx) = mpsc::channel(1);

        // Test successful completion within timeout
        safe_spawn_async_task_with_timeout(
            move || async move {
                tokio::time::sleep(Duration::from_millis(10)).await;
                tx.send("completed").await.unwrap();
            },
            Duration::from_millis(100),
            "test_operation"
        );

        assert_eq!(rx.recv().await, Some("completed"));
    }
}
