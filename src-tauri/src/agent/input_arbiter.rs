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
///
/// # Two cooldowns exist, and they OVERLAP — they do not stack
///
/// Juno paces UI actions with two independent mechanisms. Both are load
/// bearing; neither is redundant, and neither may be lowered.
///
/// * **This arbiter's cooldown** ([`DEFAULT_COOLDOWN`], 500 ms) — measured
///   from the moment the previous guard was *released* (see the `Drop` impl
///   below). Applies only to actions that acquire a guard.
/// * **`enforce_action_cooldown`** in `agent/tools/anthropic_computer_use.rs`
///   (`ACTION_COOLDOWN_MS`, 300 ms) — measured from the previous
///   UI-modifying action's own timestamp. Applies to every UI-modifying
///   action, including AX-grounded ones that never come near this arbiter.
///
/// They run back to back (the 300 ms floor first, then `acquire`) for the
/// actions that take both, which reads like 800 ms of possible sleeping. It
/// is not. Both are sleeps to an *absolute deadline* computed from a past
/// reference point, and sequential deadline-sleeps compose as `max`, never
/// as a sum:
///
/// ```text
///   action N-1:  floor records its timestamp T0 AFTER its own sleep
///                ... action executes ...
///                guard drops at T = T0 + d   (d = execution time, d >= 0)
///
///   action N:    floor sleeps until  T0 + 300ms
///                acquire sleeps until T  + 500ms
///                => starts at max(T0 + 300, T + 500) = T + 500, since T >= T0
/// ```
///
/// So for any action that takes a guard, the 500 ms cooldown dominates and
/// the 300 ms floor contributes exactly zero extra delay. Removing or
/// skipping the floor for those actions would save nothing — and it would
/// *break* the case where the previous action took the AX path: then this
/// arbiter's clock is stale and the 300 ms floor is the only spacing there
/// is. Effective spacing today is ~500 ms between guard-taking actions and
/// ~300 ms between AX-path ones.
///
/// **Do not lower either constant, and do not "simplify" one of them away.**
/// The "clicked too fast" failures they prevent are real macOS behaviour.
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
    ///
    /// `elapsed` is read *here*, after any 300 ms `enforce_action_cooldown`
    /// sleep the caller already did, so that sleep is credited against this
    /// one: this deadline dominates and the two do not stack. See the module
    /// docs above before changing either mechanism.
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
    ///
    /// Because nothing is enforced here, a `try_acquire` caller has satisfied
    /// neither the 500 ms cooldown nor the 300 ms `enforce_action_cooldown`
    /// floor, and must never be treated as if it had. Only [`acquire`] is
    /// authoritative.
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
///
/// Recording on *release* rather than on acquire is what makes this
/// deadline later than `enforce_action_cooldown`'s, and therefore the
/// dominant one — see the module docs.
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

    /// Mutual exclusion must hold with a REAL (non-zero) cooldown too, not
    /// only with the zero-cooldown arbiter the other tests use. macOS has one
    /// hardware pointer; parallel sessions must never overlap (LAC-1432).
    #[tokio::test]
    async fn serializes_concurrent_acquire_with_real_cooldown() {
        let arbiter = Arc::new(InputArbiter::new(Duration::from_millis(15)));
        let counter = Arc::new(AtomicUsize::new(0));
        let observed_max = Arc::new(AtomicUsize::new(0));

        let mut handles = Vec::new();
        for i in 0..4 {
            let arbiter = arbiter.clone();
            let counter = counter.clone();
            let observed_max = observed_max.clone();
            handles.push(tokio::spawn(async move {
                let _guard = arbiter.acquire(Some(&format!("s{i}"))).await;
                let current = counter.fetch_add(1, Ordering::SeqCst) + 1;
                observed_max.fetch_max(current, Ordering::SeqCst);
                tokio::time::sleep(Duration::from_millis(3)).await;
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

    /// The 300 ms `enforce_action_cooldown` floor and this arbiter's cooldown
    /// OVERLAP; they do not stack.
    ///
    /// Both are sleeps to an absolute deadline computed from a past reference
    /// point, and `acquire` reads `elapsed()` *after* the floor has already
    /// slept — so running the floor first and then acquiring costs
    /// `max(floor, cooldown)`, not `floor + cooldown`. This test stands in for
    /// the floor with a worst-case sleep (the full floor duration) and pins
    /// that composition.
    ///
    /// If this ever fails, someone changed a reference point — most likely by
    /// recording a timestamp before the action instead of after it — and the
    /// two waits really have started stacking.
    #[tokio::test]
    async fn action_floor_and_cooldown_overlap_rather_than_stack() {
        let cooldown = Duration::from_millis(60);
        // Stand-in for ACTION_COOLDOWN_MS, scaled down like `cooldown` is.
        let floor = Duration::from_millis(36);
        let arbiter = InputArbiter::new(cooldown);

        // `start` is taken before the first acquire, which on a fresh arbiter
        // returns immediately — so the guard's release time is >= `start` and
        // the bounds below are exact rather than off by a hair.
        let start = Instant::now();

        // Previous action: guard taken and released.
        {
            let _g = arbiter.acquire(None).await;
        }

        // Next action, worst case: the floor sleeps its full duration first...
        tokio::time::sleep(floor).await;
        // ...and only then does the caller acquire the guard.
        {
            let _g = arbiter.acquire(None).await;
        }
        let elapsed = start.elapsed();

        assert!(
            elapsed >= cooldown,
            "physical spacing shrank below the cooldown ({:?} < {:?})",
            elapsed,
            cooldown
        );
        assert!(
            elapsed < cooldown + floor,
            "floor and cooldown stacked ({:?} >= {:?} + {:?}) — sequential \
             deadline sleeps must compose as max(), not sum()",
            elapsed,
            cooldown,
            floor
        );
    }

    /// The arbiter's clock only advances when a guard is taken. An AX-path
    /// action never takes one, so after AX work this arbiter imposes nothing —
    /// which is exactly why the separate 300 ms `enforce_action_cooldown`
    /// floor must stay: it is the only spacing an AX-path action ever gets,
    /// and the only spacing between an AX action and the physical action
    /// after it.
    #[tokio::test]
    async fn arbiter_imposes_nothing_after_ax_only_work() {
        let arbiter = InputArbiter::new(Duration::from_millis(60));

        // No guard has ever been taken (all work went down the AX path).
        let start = Instant::now();
        {
            let _g = arbiter.acquire(None).await;
        }
        assert!(
            start.elapsed() < Duration::from_millis(30),
            "arbiter unexpectedly paced the first guard-taking action; the \
             300 ms action-cooldown floor is what covers AX-path spacing"
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
