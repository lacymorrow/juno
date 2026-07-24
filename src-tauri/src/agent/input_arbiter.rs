use std::sync::{Arc, Mutex as StdMutex};
use std::time::{Duration, Instant};

use tokio::sync::{Mutex as TokioMutex, OwnedMutexGuard};
use tracing::{debug, trace};

/// Serializes coordinate-based physical input across all agent sessions.
///
/// macOS exposes exactly one hardware pointer, so N parallel agents cannot
/// execute CGEvent-based clicks, drags, or typing simultaneously. Every
/// call site that emits a physical input event must acquire a
/// [`PhysicalInputGuard`] first — the guard blocks other sessions until it
/// is dropped, and enforces a small cooldown between actions so we never
/// fire pointer events faster than macOS reliably delivers them.
///
/// AX-grounded actions (`AXPress` via the accessibility API) do NOT go
/// through this arbiter. They do not move the physical pointer, so multiple
/// agents can invoke them concurrently — that is Juno's parallelism moat.
/// Default cooldown between coordinate-based input actions.
///
/// 500 ms gives macOS time to process one event before the next lands.
/// Callers that need tighter pacing can construct an [`InputArbiter`] with
/// a custom [`Duration`], but this constant should be preferred for
/// production agent sessions.
pub const DEFAULT_COOLDOWN: Duration = Duration::from_millis(500);

pub struct InputArbiter {
    inner: Arc<TokioMutex<InputArbiterInner>>,
    /// Observable holder id, kept OUTSIDE the input mutex so observers can
    /// ask "who holds the arbiter?" while a guard is held. Storing it inside
    /// `inner` would deadlock any `held_by()` call made during a hold.
    holder: Arc<StdMutex<Option<String>>>,
    cooldown: Duration,
}

#[derive(Default)]
struct InputArbiterInner {
    last_action_at: Option<Instant>,
}

impl InputArbiter {
    pub fn new(cooldown: Duration) -> Self {
        Self {
            inner: Arc::new(TokioMutex::new(InputArbiterInner::default())),
            holder: Arc::new(StdMutex::new(None)),
            cooldown,
        }
    }

    pub fn cooldown(&self) -> Duration {
        self.cooldown
    }

    fn set_holder(&self, session_id: Option<&str>) -> Option<String> {
        let held = session_id.map(|s| s.to_string());
        *self
            .holder
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = held.clone();
        held
    }

    /// Acquire exclusive access to physical input.
    ///
    /// Blocks until any current holder releases, then sleeps for the
    /// remainder of the cooldown if the previous action was too recent.
    /// The returned guard tracks the caller's session id for observability;
    /// pass `None` for internal/system callers that are not agent-scoped.
    pub async fn acquire(&self, session_id: Option<&str>) -> PhysicalInputGuard {
        let guard = self.inner.clone().lock_owned().await;
        if let Some(last) = guard.last_action_at {
            let elapsed = last.elapsed();
            if elapsed < self.cooldown {
                let sleep_for = self.cooldown - elapsed;
                trace!(
                    "InputArbiter cooldown sleep {:?} for session {:?}",
                    sleep_for,
                    session_id
                );
                tokio::time::sleep(sleep_for).await;
            }
        }
        let held_by = self.set_holder(session_id);
        debug!("InputArbiter acquired by session {:?}", session_id);
        PhysicalInputGuard {
            guard,
            holder: self.holder.clone(),
            held_by,
        }
    }

    /// Try to acquire without blocking. Returns `None` if another session holds it.
    ///
    /// Does NOT enforce the cooldown — callers using try_acquire opt into
    /// firing as soon as they win the lock. Prefer [`acquire`] for normal
    /// agent input paths.
    pub async fn try_acquire(&self, session_id: Option<&str>) -> Option<PhysicalInputGuard> {
        match self.inner.clone().try_lock_owned() {
            Ok(guard) => {
                let held_by = self.set_holder(session_id);
                Some(PhysicalInputGuard {
                    guard,
                    holder: self.holder.clone(),
                    held_by,
                })
            }
            Err(_) => None,
        }
    }

    /// Session id currently holding the arbiter, if any. For observability
    /// only. Safe to call while a guard is held — the holder id lives
    /// outside the input mutex.
    pub fn held_by(&self) -> Option<String> {
        self.holder
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }
}

impl Default for InputArbiter {
    fn default() -> Self {
        Self::new(DEFAULT_COOLDOWN)
    }
}

/// RAII guard for exclusive physical input access.
///
/// Records the release time on drop so the cooldown applies to the next
/// caller regardless of exit path (success, error via `?`, panic).
pub struct PhysicalInputGuard {
    guard: OwnedMutexGuard<InputArbiterInner>,
    holder: Arc<StdMutex<Option<String>>>,
    held_by: Option<String>,
}

impl PhysicalInputGuard {
    pub fn held_by(&self) -> Option<&str> {
        self.held_by.as_deref()
    }
}

impl Drop for PhysicalInputGuard {
    fn drop(&mut self) {
        self.guard.last_action_at = Some(Instant::now());
        *self
            .holder
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    #[tokio::test]
    async fn serializes_concurrent_acquire() {
        let arbiter = Arc::new(InputArbiter::new(Duration::from_millis(0)));
        let counter = Arc::new(AtomicUsize::new(0));
        let observed_max = Arc::new(AtomicUsize::new(0));

        let mut handles = Vec::new();
        for i in 0..8 {
            let arbiter = arbiter.clone();
            let counter = counter.clone();
            let observed_max = observed_max.clone();
            handles.push(tokio::spawn(async move {
                let _guard = arbiter.acquire(Some(&format!("s{i}"))).await;
                let current = counter.fetch_add(1, Ordering::SeqCst) + 1;
                observed_max.fetch_max(current, Ordering::SeqCst);
                tokio::time::sleep(Duration::from_millis(2)).await;
                counter.fetch_sub(1, Ordering::SeqCst);
            }));
        }
        for h in handles {
            h.await.expect("task ok");
        }

        assert_eq!(
            observed_max.load(Ordering::SeqCst),
            1,
            "arbiter must serialize physical input across sessions"
        );
    }

    #[tokio::test]
    async fn enforces_cooldown_between_actions() {
        let cooldown = Duration::from_millis(30);
        let arbiter = InputArbiter::new(cooldown);

        {
            let _g = arbiter.acquire(None).await;
        }
        let start = Instant::now();
        {
            let _g = arbiter.acquire(None).await;
        }
        let elapsed = start.elapsed();
        assert!(
            elapsed >= cooldown,
            "second acquire completed too fast ({:?}, cooldown {:?})",
            elapsed,
            cooldown
        );
    }

    #[tokio::test]
    async fn try_acquire_returns_none_while_held() {
        let arbiter = Arc::new(InputArbiter::new(Duration::from_millis(0)));
        let _held = arbiter.acquire(Some("holder")).await;
        assert!(arbiter.try_acquire(Some("other")).await.is_none());
        // held_by() must not deadlock while the guard is held — the holder
        // id lives outside the input mutex precisely for this.
        assert_eq!(arbiter.held_by().as_deref(), Some("holder"));
    }

    #[tokio::test]
    async fn guard_drop_releases_and_clears_holder() {
        let arbiter = Arc::new(InputArbiter::new(Duration::from_millis(0)));
        {
            let _g = arbiter.acquire(Some("first")).await;
            assert_eq!(arbiter.held_by().as_deref(), Some("first"));
        }
        // Holder is cleared once the guard drops.
        assert_eq!(arbiter.held_by(), None);
        // Second caller wins immediately, and holder is reset.
        let second = arbiter.acquire(Some("second")).await;
        assert_eq!(second.held_by(), Some("second"));
    }
}
