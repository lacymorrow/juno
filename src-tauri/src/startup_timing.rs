//! # Launch timing
//!
//! One `[Startup]` log line per milestone on the way to the bar, in
//! milliseconds since `run()` began (a few milliseconds after the process
//! started; nothing earlier is ours to measure). Read a launch with:
//!
//! ```text
//! grep '\[Startup\]' ~/Library/Logs/Juno/juno-$(date +%F).log
//! ```
//!
//! Milestones that can recur later in a session (the bar is shown again after
//! onboarding, a reveal is replayed) are logged once, the first time, because
//! only the first one is part of a launch.

use std::collections::HashSet;
use std::sync::{LazyLock, Mutex};
use std::time::Instant;

use tracing::info;

static START: LazyLock<Instant> = LazyLock::new(Instant::now);
static SEEN: LazyLock<Mutex<HashSet<&'static str>>> = LazyLock::new(|| Mutex::new(HashSet::new()));

/// Start the clock. Called first thing in `run()`.
pub fn begin() {
    LazyLock::force(&START);
}

/// Milliseconds since [`begin`].
pub fn elapsed_ms() -> u128 {
    START.elapsed().as_millis()
}

/// Log a milestone, every time it happens.
pub fn mark(what: &str) {
    info!("[Startup] {} at {} ms", what, elapsed_ms());
}

/// Log a milestone the first time it happens, and never again this session.
/// True when this call was the first.
pub fn mark_once(what: &'static str) -> bool {
    let first = match SEEN.lock() {
        Ok(mut seen) => seen.insert(what),
        Err(poisoned) => poisoned.into_inner().insert(what),
    };
    if first {
        mark(what);
    }
    first
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_milestone_is_logged_once() {
        assert!(mark_once("test milestone"));
        assert!(!mark_once("test milestone"));
    }

    #[test]
    fn the_clock_only_moves_forward() {
        begin();
        let a = elapsed_ms();
        let b = elapsed_ms();
        assert!(b >= a);
    }
}
