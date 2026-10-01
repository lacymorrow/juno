//! # Timer Tools Module
//!
//! Advanced scheduling and monitoring tools for delayed agent execution and system monitoring.
//! Enables agents to set timers, monitor screen changes, file system events, and application states.
//!
//! ## Core Capabilities:
//! - Simple time-based delays with context restoration
//! - Screen region monitoring for visual changes
//! - File system monitoring (creation, modification, deletion)
//! - Application state monitoring (launch, focus, termination)
//!
//! ## Usage
//! Used by: Game automation, long-running tasks, system monitoring, waiting for external events
//! Registration: Called via `register_timer_tools()` during agent setup

use crate::agent::implementations::tool_provider::LocalToolProvider;
use crate::constants::{agent, error_messages, events};
use once_cell::sync::Lazy;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter};
use tokio::fs;
use tokio::sync::Mutex;
use tokio::time::{sleep, MissedTickBehavior};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

#[cfg(target_os = "macos")]
use computer_use_ai_sdk::platforms::macos::utils as macos_utils;
#[cfg(target_os = "macos")]
use image::{Rgba, RgbaImage};

// Timer state management

/// Represents a scheduled timer task with associated context and configuration.
///
/// Used by: Timer management system, monitoring tools, agent scheduling
/// Contains all information needed to restore agent state when timer expires.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TimerTask {
    /// Unique identifier for this timer task
    pub id: String,
    /// Unix timestamp in seconds when timer should trigger
    pub trigger_time: u64,
    /// JSON context to restore when timer triggers (game state, conversation history, etc.)
    pub context: Value,
    /// Human-readable description of what this timer is for
    pub description: String,
    /// Unix timestamp when timer was created
    pub created_at: u64,
    /// Type and configuration of the timer (simple, screen monitor, etc.)
    pub timer_type: TimerType,
}

/// Defines the different types of timers and their specific configurations.
///
/// Used by: Timer task creation, monitoring system dispatch, timer execution logic
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum TimerType {
    /// Simple time-based delay timer
    Simple,
    /// Screen monitoring with change detection
    ScreenMonitor {
        /// Optional screen region to monitor (full screen if None)
        region: Option<ScreenRegion>,
        /// Percentage change threshold to trigger (0.0-1.0)
        threshold: f32,
        /// How often to check for changes in seconds
        check_interval_seconds: u64,
    },
    /// File system monitoring for various file events
    FileMonitor {
        /// Path to the file to monitor
        file_path: String,
        /// Type of file event to watch for
        monitor_type: FileMonitorType,
    },
    /// Application state monitoring
    ApplicationMonitor {
        /// Name of the application to monitor
        app_name: String,
        /// State change to watch for
        monitor_state: AppMonitorState,
    },
}

impl TimerType {
    /// Is this a *watch*, something polling the world, rather than a plain
    /// delay?
    ///
    /// A watch is the thing a person needs to be able to stop: it observes the
    /// screen or the filesystem on an interval and can start a turn on its own.
    /// This is what decides whether Juno holds the stop key (see
    /// `TimerManager::reconcile_stop_key`) and what the status command
    /// reports.
    ///
    /// Matched exhaustively on purpose. A new timer kind has to answer this
    /// question at the point it is added, rather than defaulting to "not a
    /// watch" behind a wildcard and quietly shipping something unstoppable.
    pub fn is_monitor(&self) -> bool {
        match self {
            TimerType::Simple => false,
            TimerType::ScreenMonitor { .. }
            | TimerType::FileMonitor { .. }
            | TimerType::ApplicationMonitor { .. } => true,
        }
    }

    /// Short human label, for logs and the status command.
    pub fn label(&self) -> &'static str {
        match self {
            TimerType::Simple => "delay",
            TimerType::ScreenMonitor { .. } => "screen monitor",
            TimerType::FileMonitor { .. } => "file monitor",
            TimerType::ApplicationMonitor { .. } => "application monitor",
        }
    }
}

/// Defines a rectangular screen region for monitoring.
///
/// Used by: Screen monitoring timers, visual change detection
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ScreenRegion {
    /// X coordinate of top-left corner
    pub x: f64,
    /// Y coordinate of top-left corner
    pub y: f64,
    /// Width of the region
    pub width: f64,
    /// Height of the region
    pub height: f64,
}

/// Types of file system events that can be monitored.
///
/// Used by: File monitoring timers, filesystem change detection
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum FileMonitorType {
    /// File was created
    Created,
    /// File content was modified
    Modified,
    /// File was deleted
    Deleted,
    /// File size changed
    SizeChanged,
}

/// Application state changes that can be monitored.
///
/// Used by: Application monitoring timers, app state tracking
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum AppMonitorState {
    /// Application was launched
    Launched,
    /// Application was terminated
    Terminated,
    /// Application gained focus
    BecameFocused,
    /// Application lost focus
    LostFocus,
}

/// Ledger user name under which an armed monitor holds the stop key.
///
/// Registered with `crate::commands::escape_key_coordinator` exactly the way
/// an agent run (`"agent_execution"`), TTS and dictation do, so an armed watch
/// is one more thing Juno "has to stop" and Escape reaches it through the
/// existing coordinated-stop path rather than a parallel mechanism.
pub const MONITOR_STOP_KEY_USER: &str = "timer_monitor";

/// How many turns monitors may wake inside `WAKE_BUDGET_WINDOW`.
///
/// This is the spending guard, and the model cannot opt out of it: the budget
/// is checked when a watch is armed *and* again at the moment a change is
/// detected, so a chain of monitors that each re-arm the next one dies after
/// this many model calls instead of running until someone notices the bill.
///
/// Twenty is the same number `docs/plans/ambient-awareness.md` picks for the
/// real feature's `max_wakes`, so the shipping guard and the planned one do not
/// disagree.
pub const MAX_WAKES_PER_WINDOW: usize = 20;

/// Rolling window the wake budget is counted over.
pub const WAKE_BUDGET_WINDOW: Duration = Duration::from_secs(60 * 60);

/// Rolling-window count of turns that monitors have woken.
///
/// Pure, so the budget is unit-testable without a Tauri app: every method takes
/// `now` rather than reading the clock.
#[derive(Debug, Default)]
pub struct WakeLedger {
    wakes: Vec<Instant>,
}

impl WakeLedger {
    /// Spend one wake, or refuse when the budget is gone.
    pub fn try_consume(&mut self, now: Instant) -> bool {
        self.prune(now);
        if self.wakes.len() >= MAX_WAKES_PER_WINDOW {
            return false;
        }
        self.wakes.push(now);
        true
    }

    /// Wakes still available in the current window.
    pub fn remaining(&mut self, now: Instant) -> usize {
        self.prune(now);
        MAX_WAKES_PER_WINDOW.saturating_sub(self.wakes.len())
    }

    /// Forget every recorded wake.
    pub fn clear(&mut self) {
        self.wakes.clear();
    }

    /// Drop wakes that have aged out of the window.
    ///
    /// `saturating_duration_since` rather than subtraction: an `Instant` that
    /// somehow reads later than `now` must not panic the monitor loop.
    fn prune(&mut self, now: Instant) {
        self.wakes
            .retain(|wake| now.saturating_duration_since(*wake) < WAKE_BUDGET_WINDOW);
    }
}

/// Enhanced timer manager with monitoring capabilities.
///
/// Manages active timers and their associated monitoring tasks.
/// Provides thread-safe access to timer state and cancellation.
///
/// Used by: Agent system for timer lifecycle management, monitoring coordination
#[derive(Debug, Default, Clone)]
pub struct TimerManager {
    /// Map of active timer tasks by ID
    pub active_timers: Arc<Mutex<HashMap<String, TimerTask>>>,
    /// Map of monitoring task handles by timer ID
    pub monitoring_tasks: Arc<Mutex<HashMap<String, tauri::async_runtime::JoinHandle<()>>>>,
    /// Rolling budget of turns monitors have woken.
    wakes: Arc<Mutex<WakeLedger>>,
}

/// The one timer manager for the process.
///
/// It used to live in `AppState`'s type-keyed component map, and the lookup
/// could never have worked: the tools inserted an `Arc<TimerManager>` and read
/// back with `get::<TimerManager>()`, which keys on `TypeId::of::<TimerManager>`
/// and so never matched the `TypeId::of::<Arc<TimerManager>>` that was stored.
/// Every `set_screen_monitor` call therefore built a *fresh* manager, armed its
/// watch inside it and dropped the only reference to it, while `cancel_timer`
/// and `list_timers` both failed with "timer manager not initialized". The
/// watch kept capturing the screen with nothing anywhere able to name it, let
/// alone cancel it.
///
/// A process-global behind `Lazy` is the shape the rest of this codebase
/// already uses for exactly this job (`get_stop_coordinator`,
/// `get_escape_key_coordinator`), and it means the tools, the Escape path and
/// shutdown are all looking at the same timers.
static TIMER_MANAGER: Lazy<TimerManager> = Lazy::new(TimerManager::new);

/// The process-wide timer manager.
pub fn timer_manager() -> &'static TimerManager {
    &TIMER_MANAGER
}

impl TimerManager {
    /// Creates a new empty timer manager.
    ///
    /// Used by: Agent initialization, app state setup
    ///
    /// # Returns
    /// New `TimerManager` instance with empty timer collections
    pub fn new() -> Self {
        Self {
            active_timers: Arc::new(Mutex::new(HashMap::new())),
            monitoring_tasks: Arc::new(Mutex::new(HashMap::new())),
            wakes: Arc::new(Mutex::new(WakeLedger::default())),
        }
    }

    /// Wakes still available in the current budget window.
    pub async fn wakes_remaining(&self) -> usize {
        let mut ledger = self.wakes.lock().await;
        ledger.remaining(Instant::now())
    }

    /// Spend one wake from the budget, or refuse when it is gone.
    pub async fn try_consume_wake(&self) -> bool {
        let mut ledger = self.wakes.lock().await;
        ledger.try_consume(Instant::now())
    }

    /// Every armed watch (screen, file, application), not plain delays.
    ///
    /// This is what the status command reports and what makes an armed watch
    /// visible to anything outside this module.
    pub async fn armed_monitors(&self) -> Vec<TimerTask> {
        let timers = self.active_timers.lock().await;
        timers
            .values()
            .filter(|timer| timer.timer_type.is_monitor())
            .cloned()
            .collect()
    }

    /// Is any watch armed right now?
    ///
    /// Read by `escape_key_coordinator::something_to_stop`, so that Escape
    /// pressed with nothing else running stops the watch instead of being
    /// treated as "nothing to do, dismiss the pane".
    pub async fn has_armed_monitor(&self) -> bool {
        let timers = self.active_timers.lock().await;
        timers.values().any(|timer| timer.timer_type.is_monitor())
    }

    /// Remove one timer and bring the stop-key registration back in line.
    ///
    /// Every exit from a monitor loop goes through here (change detected,
    /// deadline reached, budget spent), so the stop key is never left held by
    /// a watch that is no longer running.
    pub async fn release_timer(
        &self,
        app_handle: Option<&AppHandle>,
        timer_id: &str,
    ) -> Option<TimerTask> {
        // Remove the timer first, so the reconcile below sees the truth.
        let removed = {
            let mut timers = self.active_timers.lock().await;
            timers.remove(timer_id)
        };
        // Take the task handle, but do not abort it yet.
        let handle = {
            let mut tasks = self.monitoring_tasks.lock().await;
            tasks.remove(timer_id)
        };

        self.reconcile_stop_key(app_handle).await;

        // Abort last. A monitor loop calls this on *itself* when it fires or
        // reaches its deadline, and `abort` takes effect at the next await
        // point: aborting before the reconcile above could cancel this very
        // task part way through it and leave the stop key held by a watch that
        // no longer exists.
        if let Some(handle) = handle {
            handle.abort();
            debug!("Cancelled monitoring task for timer: {}", timer_id);
        }

        removed
    }

    /// Cancel every timer this process has armed and return what was cancelled.
    ///
    /// Monitors *and* plain delays: both can start a turn on their own, and a
    /// person pressing Escape means "stop what you are about to do", not "stop
    /// the screen capture but keep the pending restart".
    ///
    /// `app_handle` is `None` only where there is no Tauri application (unit
    /// tests); the stop-key reconciliation is then skipped because there is no
    /// coordinator to reconcile with.
    pub async fn cancel_all_timers(&self, app_handle: Option<&AppHandle>) -> Vec<TimerTask> {
        // Collect the ids under the lock, then release it: `remove_timer` takes
        // the same lock, so holding it here would deadlock.
        let ids: Vec<String> = {
            let timers = self.active_timers.lock().await;
            timers.keys().cloned().collect()
        };

        let mut cancelled = Vec::with_capacity(ids.len());
        for id in ids {
            if let Some(timer) = self.remove_timer(&id).await {
                info!(
                    "[TimerManager] Cancelled {} {} ({})",
                    timer.timer_type.label(),
                    timer.id,
                    timer.description
                );
                cancelled.push(timer);
            }
        }

        self.reconcile_stop_key(app_handle).await;
        cancelled
    }

    /// Hold the stop key while a watch is armed; release it when none is.
    ///
    /// Idempotent in both directions. The coordinator's ledger is a set, so a
    /// repeat register or a stale unregister is a no-op.
    async fn reconcile_stop_key(&self, app_handle: Option<&AppHandle>) {
        let Some(app_handle) = app_handle else {
            return;
        };
        let armed = self.has_armed_monitor().await;
        let coordinator = crate::commands::escape_key_coordinator::get_escape_key_coordinator();
        let outcome = if armed {
            coordinator
                .register_escape_user(app_handle, MONITOR_STOP_KEY_USER)
                .await
        } else {
            coordinator
                .unregister_escape_user(app_handle, MONITOR_STOP_KEY_USER)
                .await
        };
        if let Err(e) = outcome {
            warn!(
                "[TimerManager] Could not {} the stop key for armed monitors: {}",
                if armed { "claim" } else { "release" },
                e
            );
        }
    }

    /// Adds a timer to the active timers collection.
    ///
    /// Used by: Timer creation functions, scheduler setup
    ///
    /// # Arguments
    /// * `timer` - The TimerTask to add to active collection
    pub async fn add_timer(&self, timer: TimerTask) {
        let mut timers = self.active_timers.lock().await;
        timers.insert(timer.id.clone(), timer);
    }

    /// Removes and returns a timer from the active collection.
    ///
    /// Also cancels any associated monitoring task.
    /// Used by: Timer expiration, timer cancellation, cleanup
    ///
    /// # Arguments
    /// * `timer_id` - ID of the timer to remove
    ///
    /// # Returns
    /// The removed `TimerTask` if it existed, None otherwise
    pub async fn remove_timer(&self, timer_id: &str) -> Option<TimerTask> {
        let mut timers = self.active_timers.lock().await;
        let timer = timers.remove(timer_id);

        // Cancel monitoring task if exists
        let mut monitoring_tasks = self.monitoring_tasks.lock().await;
        if let Some(task_handle) = monitoring_tasks.remove(timer_id) {
            task_handle.abort();
            debug!("Cancelled monitoring task for timer: {}", timer_id);
        }

        timer
    }

    /// Adds a monitoring task handle for cleanup management.
    ///
    /// Used by: Monitoring timer creation, background task tracking
    ///
    /// # Arguments
    /// * `timer_id` - ID of the timer this task belongs to
    /// * `task_handle` - Handle to the background monitoring task
    pub async fn add_monitoring_task(
        &self,
        timer_id: String,
        task_handle: tauri::async_runtime::JoinHandle<()>,
    ) {
        let mut monitoring_tasks = self.monitoring_tasks.lock().await;
        monitoring_tasks.insert(timer_id, task_handle);
    }

    /// Retrieves a timer by ID without removing it.
    ///
    /// Used by: Timer status checking, monitoring loops
    ///
    /// # Arguments
    /// * `timer_id` - ID of the timer to retrieve
    ///
    /// # Returns
    /// Cloned `TimerTask` if found, None otherwise
    pub async fn get_timer(&self, timer_id: &str) -> Option<TimerTask> {
        let timers = self.active_timers.lock().await;
        timers.get(timer_id).cloned()
    }

    /// Returns a list of all currently active timers.
    ///
    /// Used by: Timer listing tool, status reporting, debugging
    ///
    /// # Returns
    /// Vector of all active `TimerTask` instances
    pub async fn list_active_timers(&self) -> Vec<TimerTask> {
        let timers = self.active_timers.lock().await;
        timers.values().cloned().collect()
    }

    /// Gets all expired timers from the active timer collection.
    ///
    /// Used by: Timer expiration checking, cleanup processes
    ///
    /// # Returns
    /// Vector of expired `TimerTask` instances
    pub async fn get_expired_timers(&self) -> Vec<TimerTask> {
        let timers = self.active_timers.lock().await;

        // Get current time, or return empty vector if system time error
        let now = match SystemTime::now().duration_since(UNIX_EPOCH) {
            Ok(duration) => duration.as_secs(),
            Err(e) => {
                warn!("System time error in get_expired_timers: {}", e);
                return Vec::new(); // Return empty vector if we can't get current time
            }
        };

        timers
            .values()
            .filter(|timer| {
                match timer.timer_type {
                    TimerType::Simple => timer.trigger_time <= now,
                    _ => false, // Monitoring timers don't expire by time
                }
            })
            .cloned()
            .collect()
    }
}

// Tool implementations
mod timer_tools_impl {
    use super::*;
    use crate::agent::core::ToolDefinition;

    /// Upper bound, in seconds, on any schedule an agent can ask for (30 days).
    ///
    /// The tool schemas declare `"minimum": 1` and no maximum, and a JSON
    /// Schema `minimum` is advisory — nothing on the request path enforced it.
    /// Two silent failures followed from that:
    ///
    /// - `now + delay_seconds` overflowed. Rust's float-to-int casts saturate,
    ///   so `delay_seconds: 1e30` became `u64::MAX` and the addition panicked
    ///   in a debug build or wrapped in a release build — a wrap producing a
    ///   `trigger_time` in the past, so the timer fired at once.
    /// - `check_interval_seconds: 0` reached `tokio::time::interval`, which
    ///   panics on a zero period and killed the monitoring task outright.
    ///
    /// Thirty days is far longer than any plausible agent timer and leaves no
    /// room for either.
    pub(super) const MAX_SCHEDULE_SECONDS: u64 = 30 * 24 * 60 * 60;

    /// Read a count of **seconds** from an agent-supplied `input[key]`.
    ///
    /// Returns `None` only when the key is absent or is not a number, so the
    /// caller can distinguish "not asked for" from "asked for badly".
    ///
    /// Fractional JSON numbers are accepted and rounded. These sites used
    /// `as_u64()`, which returns `None` for `2.5` — so a model that asked for
    /// a 2.5-second interval silently got the default instead, with nothing
    /// said about it. That is the same class of defect as a wrong unit: the
    /// request was ignored and the log looked fine.
    ///
    /// The result is clamped into `1..=MAX_SCHEDULE_SECONDS`. Saturating casts
    /// put negatives and NaN on `0` and absurd values on `u64::MAX`; the clamp
    /// catches both ends, which is what keeps `now.saturating_add(..)` inside
    /// a sane range and keeps a zero period away from `tokio::time::interval`.
    pub(super) fn schedule_seconds(input: &Value, key: &str) -> Option<u64> {
        let raw = input.get(key)?.as_f64()?;
        Some((raw.round() as u64).clamp(1, MAX_SCHEDULE_SECONDS))
    }

    /// A file's modification time, or `None` when the file is absent or the
    /// platform will not report one.
    ///
    /// Wall clock by nature — an mtime is a wall-clock stamp — but the file
    /// monitor only ever compares one reading against another reading of the
    /// same clock, never against "now", so a clock step cannot make a file look
    /// freshly modified or ancient.
    pub(super) async fn read_modified_time(path: &Path) -> Option<SystemTime> {
        fs::metadata(path).await.ok()?.modified().ok()
    }

    /// Has the file changed since the previous tick?
    ///
    /// The baseline is the mtime read on the **previous tick**, not the one
    /// read when the monitor started. Comparing against the start time meant a
    /// `Modified` monitor could only ever fire for a file touched within
    /// `check_interval + 1` seconds of the monitor being set up, and was blind
    /// to every edit after that — which is the opposite of what a monitor is
    /// for.
    ///
    /// `None` on either side is meaningful: it covers a file that did not exist
    /// yet, one that has since gone, and one whose mtime the platform refused
    /// to report. A transition into or out of `None` is a change like any
    /// other, but `current_exists` gates the whole thing so a deletion is left
    /// to `FileMonitorType::Deleted`.
    pub(super) fn modification_detected(
        current_exists: bool,
        current_modified: Option<SystemTime>,
        last_modified: Option<SystemTime>,
    ) -> bool {
        current_exists && current_modified != last_modified
    }

    /// How long a watch runs when the model does not say (30 minutes).
    ///
    /// There used to be no default. `max_duration_seconds` was documented as
    /// optional, and omitting it set `trigger_time` to `u64::MAX` and left the
    /// deadline check switched off entirely, so the watch ran until the process
    /// died. A watch that captures the screen is not something to leave running
    /// by omission.
    pub(super) const DEFAULT_MONITOR_DURATION_SECONDS: u64 = 30 * 60;

    /// The longest a watch may run however much the model asks for (2 hours).
    ///
    /// Separate from, and far below, `MAX_SCHEDULE_SECONDS`: thirty days is a
    /// defensible cap on a one-shot delay and an indefensible one on a loop
    /// that captures the screen every few seconds. Two hours is the outside of
    /// a single sitting at the machine, which is the longest a watch armed for
    /// "wait for this to finish" can still be about something the person is
    /// doing.
    pub(super) const MAX_MONITOR_DURATION_SECONDS: u64 = 2 * 60 * 60;

    /// Fraction of watched pixels that must change by default (10%).
    pub(super) const DEFAULT_CHANGE_THRESHOLD: f64 = 0.1;

    /// Per-channel difference, out of 255, below which two pixels count as the
    /// same.
    ///
    /// Captures are lossless RGBA, so there is no codec noise to absorb, but
    /// subpixel antialiasing and cursor blending move a channel by one or two
    /// without anything on screen having changed.
    #[cfg(target_os = "macos")]
    pub(super) const PIXEL_TOLERANCE: u8 = 8;

    /// Resolve the watch duration a monitor will actually run for.
    ///
    /// Always a bound: absent becomes the default, and anything above the cap
    /// becomes the cap. There is no way to ask for an unbounded watch, and the
    /// tool reports the number it resolved to so the model is not told
    /// otherwise.
    pub(super) fn monitor_duration_seconds(input: &Value) -> u64 {
        schedule_seconds(input, "max_duration_seconds")
            .unwrap_or(DEFAULT_MONITOR_DURATION_SECONDS)
            .min(MAX_MONITOR_DURATION_SECONDS)
    }

    /// Resolve the change threshold, as a fraction of the watched pixels.
    ///
    /// Clamped into `0.0..=1.0`, with anything non-finite falling back to the
    /// default rather than becoming a `NaN` that compares false against
    /// everything and silently makes the watch unable to fire.
    pub(super) fn change_threshold(input: &Value) -> f32 {
        let raw = input
            .get("threshold")
            .and_then(Value::as_f64)
            .unwrap_or(DEFAULT_CHANGE_THRESHOLD);
        if !raw.is_finite() {
            return DEFAULT_CHANGE_THRESHOLD as f32;
        }
        raw.clamp(0.0, 1.0) as f32
    }

    /// Seconds since the epoch, or `None` if the platform will not say.
    pub(super) fn unix_now() -> Option<u64> {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .ok()
            .map(|d| d.as_secs())
    }

    /// Wall-clock seconds between a timer's creation and now.
    ///
    /// `saturating_sub` so a clock stepped backwards reads as zero elapsed
    /// rather than underflowing. That is the same bug class the monotonic clock
    /// was introduced to fix, which is why this value is never used alone.
    pub(super) fn wall_elapsed_seconds(created_at: u64, now: u64) -> u64 {
        now.saturating_sub(created_at)
    }

    /// Has a watch reached its deadline, by either clock?
    ///
    /// Two clocks, because neither is honest alone on macOS:
    ///
    /// * `Instant` is monotonic and cannot be stepped, which is why the loop
    ///   measures elapsed time with it. But Rust's `Instant` on Apple targets
    ///   is `CLOCK_UPTIME_RAW`, which by definition does not advance while the
    ///   machine is asleep, so a 30 minute watch survives a lunch break and
    ///   resumes capturing the screen afterwards.
    /// * The wall clock does advance across sleep, but it can be stepped
    ///   backwards by NTP or by the person, and a backward step is what used to
    ///   underflow this arithmetic.
    ///
    /// Taking whichever says the watch is over means sleep cannot extend it
    /// (the wall clock catches that) and a backward step cannot end it early
    /// (`wall_elapsed_seconds` saturates to zero and the monotonic clock still
    /// holds the real bound).
    pub(super) fn deadline_reached(
        monotonic_elapsed: u64,
        wall_elapsed: u64,
        max_duration_seconds: u64,
    ) -> bool {
        monotonic_elapsed >= max_duration_seconds || wall_elapsed >= max_duration_seconds
    }

    /// A rectangle of captured pixels, in the captured image's own coordinates.
    #[cfg(target_os = "macos")]
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(super) struct PixelRect {
        pub x: u32,
        pub y: u32,
        pub width: u32,
        pub height: u32,
    }

    /// Turn the model's requested `region` into a rectangle inside the captured
    /// image, or say why it cannot.
    ///
    /// `region` used to be parsed, stored on the task, and then never read:
    /// every capture was the whole screen whatever was asked for. This is the
    /// function that makes it mean something.
    ///
    /// A region that overhangs the captured display is trimmed to fit, because
    /// a slightly-too-large rectangle is plainly a rounding question. A region
    /// that is empty or lies entirely outside the display is an error and
    /// arms nothing: quietly falling back to the full screen would be the same
    /// lie in a new place.
    #[cfg(target_os = "macos")]
    pub(super) fn resolve_rect(
        image_width: u32,
        image_height: u32,
        region: Option<&ScreenRegion>,
    ) -> Result<PixelRect, String> {
        if image_width == 0 || image_height == 0 {
            return Err("The captured display reported no pixels".to_string());
        }

        let Some(region) = region else {
            return Ok(PixelRect {
                x: 0,
                y: 0,
                width: image_width,
                height: image_height,
            });
        };

        // Saturating float-to-int casts put negatives and NaN on 0 and absurd
        // values on u32::MAX; `max(0.0)` also turns NaN into 0.0 first.
        let x = region.x.max(0.0).floor() as u32;
        let y = region.y.max(0.0).floor() as u32;
        let requested_width = region.width.max(0.0).floor() as u32;
        let requested_height = region.height.max(0.0).floor() as u32;

        if requested_width == 0 || requested_height == 0 {
            return Err(format!(
                "Region {}x{} has no area; give a width and height of at least 1 pixel",
                requested_width, requested_height
            ));
        }
        if x >= image_width || y >= image_height {
            return Err(format!(
                "Region origin ({}, {}) is outside the captured display, which is {}x{} pixels",
                x, y, image_width, image_height
            ));
        }

        Ok(PixelRect {
            x,
            y,
            width: requested_width.min(image_width - x),
            height: requested_height.min(image_height - y),
        })
    }

    /// Do two pixels differ by more than the tolerance on any channel?
    #[cfg(target_os = "macos")]
    fn pixel_changed(a: &Rgba<u8>, b: &Rgba<u8>, tolerance: u8) -> bool {
        a.0.iter()
            .zip(b.0.iter())
            .any(|(left, right)| left.abs_diff(*right) > tolerance)
    }

    /// Fraction of pixels that differ between two captures of the same region.
    ///
    /// `None` means the two captures are not comparable: the images are
    /// different sizes, which on this capture path means the pointer moved to
    /// another display or the resolution changed. The caller re-baselines
    /// instead of waking a turn, because "your other monitor is a different
    /// size" is not the change anybody asked to watch for.
    #[cfg(target_os = "macos")]
    pub(super) fn changed_pixel_ratio(
        previous: &RgbaImage,
        current: &RgbaImage,
        tolerance: u8,
    ) -> Option<f64> {
        if previous.dimensions() != current.dimensions() {
            return None;
        }
        let total = u64::from(previous.width()) * u64::from(previous.height());
        if total == 0 {
            return None;
        }
        // `&(a, b)` rather than `(a, b)`: `filter` hands the closure a
        // reference to the zipped tuple, so the plain pattern would bind
        // `&&Rgba<u8>` and function arguments do not auto-deref.
        let changed = previous
            .pixels()
            .zip(current.pixels())
            .filter(|&(a, b)| pixel_changed(a, b, tolerance))
            .count() as u64;
        Some(changed as f64 / total as f64)
    }

    /// Is a measured change big enough to wake a turn?
    ///
    /// The test used to be `current_screenshot != previous_screenshot` on two
    /// base64 PNGs: byte inequality, so a clock digit, a blinking caret or the
    /// mouse pointer counted as "the screen changed" and the `threshold` the
    /// model was told was in force filtered nothing. Now the threshold is the
    /// fraction of watched pixels that must differ.
    ///
    /// `ratio > 0.0` is required as well as `ratio >= threshold`, so a
    /// threshold of 0 means "any change at all" rather than "fire on the first
    /// tick whether or not anything moved".
    pub(super) fn change_detected(ratio: f64, threshold: f32) -> bool {
        ratio > 0.0 && ratio >= f64::from(threshold)
    }

    /// Capture the watched region of the screen.
    ///
    /// Blocking: screen capture is a synchronous system call, so callers run it
    /// on the blocking pool rather than on an async worker.
    ///
    /// Only the watched rectangle is kept, so a small region costs a small
    /// baseline rather than a full-screen buffer held for the life of the
    /// watch.
    #[cfg(target_os = "macos")]
    pub(super) fn capture_region(
        region: Option<&ScreenRegion>,
    ) -> Result<(PixelRect, RgbaImage), String> {
        let frame = macos_utils::capture_screenshot_buffer()
            .map_err(|e| format!("Failed to capture the screen: {}", e))?;
        let rect = resolve_rect(frame.width(), frame.height(), region)?;
        let cropped = RgbaImage::from_fn(rect.width, rect.height, |x, y| {
            *frame.get_pixel(rect.x + x, rect.y + y)
        });
        Ok((rect, cropped))
    }

    /// Creates the tool definition for the `set_timer` tool.
    ///
    /// Used by: Tool registration system, agent tool discovery
    /// Creates schema for simple time-based delay timers.
    ///
    /// # Returns
    /// `ToolDefinition` for setting simple delay timers with context
    pub fn set_timer_definition() -> ToolDefinition {
        ToolDefinition {
            name: agent::tool_names::SET_TIMER.to_string(),
            description: "Sets a timer that will restart the agent after a specified delay. Useful for long-running tasks like games where the agent needs to wait for external events or take breaks. The agent will be restarted with the saved context when the timer expires.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "delay_seconds": {
                        "type": "number",
                        "description": "Number of seconds to wait before restarting the agent",
                        "minimum": 1
                    },
                    "context": {
                        "type": "object",
                        "description": "Context data to restore when the timer expires (game state, conversation history, etc.)",
                        "additionalProperties": true
                    },
                    "description": {
                        "type": "string",
                        "description": "Human-readable description of what this timer is for"
                    }
                },
                "required": ["delay_seconds", "context", "description"]
            }),
            api_type: None,
            beta_flag: None,
        }
    }

    /// Executes the `set_timer` tool operation.
    ///
    /// Creates a simple delay timer that will emit a timer-expired event
    /// to restart the agent with saved context after the delay.
    ///
    /// Used by: Game automation, long-running processes, scheduled tasks
    ///
    /// # Arguments
    /// * `input` - JSON with delay_seconds, context, and description
    /// * `app_handle` - Tauri app handle for event emission
    ///
    /// # Returns
    /// Success response with timer details or error message
    pub async fn set_timer_exec(input: Value, app_handle: AppHandle) -> Result<Value, String> {
        let delay_seconds = schedule_seconds(&input, "delay_seconds").ok_or_else(|| {
            error_messages::tool_errors::MISSING_DELAY_SECONDS_PARAMETER.to_string()
        })?;

        let context = input["context"]
            .as_object()
            .ok_or_else(|| error_messages::tool_errors::MISSING_CONTEXT_PARAMETER.to_string())?
            .clone();

        let description = input["description"]
            .as_str()
            .ok_or_else(|| error_messages::tool_errors::MISSING_DESCRIPTION_PARAMETER.to_string())?
            .to_string();

        let timer_id = Uuid::new_v4().to_string();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| {
                error_messages::format_strings::SYSTEM_TIME_ERROR.replace("{}", &e.to_string())
            })?
            .as_secs();
        let trigger_time = now.saturating_add(delay_seconds);

        let timer_task = TimerTask {
            id: timer_id.clone(),
            trigger_time,
            context: Value::Object(context),
            description,
            created_at: now,
            timer_type: TimerType::Simple,
        };

        timer_manager().add_timer(timer_task).await;

        // Start the timer task
        let app_handle_clone = app_handle.clone();
        let timer_id_clone = timer_id.clone();
        tauri::async_runtime::spawn(async move {
            sleep(Duration::from_secs(delay_seconds)).await;

            // Check if timer is still active (might have been cancelled)
            if let Some(expired_timer) = timer_manager()
                .release_timer(Some(&app_handle_clone), &timer_id_clone)
                .await
            {
                info!(
                    "{}",
                    error_messages::format_strings::TIMER_EXPIRED_TRIGGERING_RESTART
                        .replace("{}", &timer_id_clone)
                );

                // Emit event to frontend to restart agent with context
                if let Err(e) = app_handle_clone.emit(events::timer::EXPIRED, &expired_timer) {
                    error!(
                        "{}",
                        error_messages::format_strings::FAILED_TO_EMIT_TIMER_EXPIRED_EVENT
                            .replace("{}", &e.to_string())
                    );
                }
            }
        });

        Ok(json!({
            "success": true,
            "timer_id": timer_id,
            "trigger_time": trigger_time,
            "message": format!("Timer set for {} seconds from now", delay_seconds)
        }))
    }

    /// Creates the tool definition for the `set_screen_monitor` tool.
    ///
    /// Used by: Tool registration system for screen monitoring capabilities
    /// Enables monitoring of screen regions for visual changes.
    ///
    /// # Returns
    /// `ToolDefinition` for screen change monitoring with region and threshold options
    pub fn set_screen_monitor_definition() -> ToolDefinition {
        ToolDefinition {
            name: agent::tool_names::SET_SCREEN_MONITOR.to_string(),
            description: "Watches the screen and starts a new turn once, when enough of the watched pixels have changed. Captures the screen on an interval, so the person is told it is watching and can end it with Escape. The watch is one-shot: it stops as soon as it fires, and it stops on its own at its deadline. Wakes are budgeted across all monitors, so this cannot be used to keep starting turns indefinitely.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "context": {
                        "type": "object",
                        "description": "Context data to restore when screen changes are detected",
                        "additionalProperties": true
                    },
                    "description": {
                        "type": "string",
                        "description": "What this monitor is watching for. Shown to the person while it is armed, so write it for them."
                    },
                    "region": {
                        "type": "object",
                        "description": "Rectangle of pixels to compare, in pixels of the captured display (the display the pointer is on), origin top-left. Omit to compare the whole captured display. A rectangle that overhangs the display is trimmed to fit; one that lies entirely outside it is an error and arms nothing.",
                        "properties": {
                            "x": {"type": "number", "description": "X coordinate of top-left corner"},
                            "y": {"type": "number", "description": "Y coordinate of top-left corner"},
                            "width": {"type": "number", "description": "Width of region"},
                            "height": {"type": "number", "description": "Height of region"}
                        }
                    },
                    "threshold": {
                        "type": "number",
                        "description": "Fraction of the watched pixels that must differ before this fires, 0.0 to 1.0. Default 0.1, meaning 10% of them. 0.0 means any single pixel.",
                        "minimum": 0.0,
                        "maximum": 1.0
                    },
                    "check_interval_seconds": {
                        "type": "number",
                        "description": "How often to compare, in seconds (default 2)",
                        "minimum": 1
                    },
                    "max_duration_seconds": {
                        "type": "number",
                        "description": "How long to watch before giving up, in seconds. Defaults to 1800 (30 minutes) and is capped at 7200 (2 hours); there is no unbounded watch.",
                        "minimum": 1
                    }
                },
                "required": ["context", "description"]
            }),
            api_type: None,
            beta_flag: None,
        }
    }

    /// Executes the `set_screen_monitor` tool operation (macOS only).
    ///
    /// Arms a bounded, stoppable watch: it captures only the watched rectangle,
    /// compares captures by changed-pixel fraction against the threshold it was
    /// actually given, spends from the shared wake budget before it may start a
    /// turn, holds the stop key while it is armed, and tells the person it is
    /// running.
    ///
    /// Used by: Game automation, UI state monitoring, visual change detection
    ///
    /// # Arguments
    /// * `input` - JSON with monitoring configuration
    /// * `app_handle` - Tauri app handle for event emission
    ///
    /// # Returns
    /// Success response with the values that were applied, or an error message
    #[cfg(target_os = "macos")]
    pub async fn set_screen_monitor_exec(
        input: Value,
        app_handle: AppHandle,
    ) -> Result<Value, String> {
        let context = input["context"]
            .as_object()
            .ok_or_else(|| error_messages::tool_errors::MISSING_CONTEXT_PARAMETER.to_string())?
            .clone();

        let description = input["description"]
            .as_str()
            .ok_or_else(|| error_messages::tool_errors::MISSING_DESCRIPTION_PARAMETER.to_string())?
            .to_string();

        let region = input["region"].as_object().map(|r| ScreenRegion {
            x: r["x"].as_f64().unwrap_or(0.0),
            y: r["y"].as_f64().unwrap_or(0.0),
            width: r["width"].as_f64().unwrap_or(f64::MAX),
            height: r["height"].as_f64().unwrap_or(f64::MAX),
        });

        let threshold = change_threshold(&input);
        let check_interval_seconds =
            schedule_seconds(&input, "check_interval_seconds").unwrap_or(2);
        let max_duration_seconds = monitor_duration_seconds(&input);

        let manager = timer_manager();

        // The spending guard, checked before anything is armed. A watch that
        // could never be allowed to wake a turn must not be reported as armed.
        let wakes_remaining = manager.wakes_remaining().await;
        if wakes_remaining == 0 {
            return Err(format!(
                "The wake budget is spent: monitors have already started {} turns in the last hour. Ask the person before watching anything else.",
                MAX_WAKES_PER_WINDOW
            ));
        }

        let now = unix_now().ok_or_else(|| {
            error_messages::format_strings::SYSTEM_TIME_ERROR
                .replace("{}", "the system clock is before the epoch")
        })?;

        // Baseline first, so a region that cannot be captured arms nothing and
        // leaves no timer behind. Capture is a blocking system call.
        let region_for_baseline = region.clone();
        let (rect, baseline) =
            tokio::task::spawn_blocking(move || capture_region(region_for_baseline.as_ref()))
                .await
                .map_err(|e| format!("Screen capture task failed: {}", e))??;

        let timer_task = TimerTask {
            id: Uuid::new_v4().to_string(),
            // Always a deadline. This used to be `u64::MAX` whenever
            // `max_duration_seconds` was omitted, which also switched the
            // deadline check off inside the loop.
            trigger_time: now.saturating_add(max_duration_seconds),
            context: Value::Object(context),
            description: description.clone(),
            created_at: now,
            timer_type: TimerType::ScreenMonitor {
                region: region.clone(),
                threshold,
                check_interval_seconds,
            },
        };
        let timer_id = timer_task.id.clone();

        manager.add_timer(timer_task).await;
        // Claim the stop key before the first capture, so Escape reaches this
        // watch from the moment it exists.
        manager.reconcile_stop_key(Some(&app_handle)).await;

        // Say so. A watch that captures the screen with nothing visible
        // anywhere is the part a person would rightly object to.
        crate::commands::notifications::notify(
            &app_handle,
            "Juno is watching your screen",
            &format!("{}. Press Escape to stop.", description),
        );

        let app_handle_clone = app_handle.clone();
        let timer_id_clone = timer_id.clone();
        let region_for_task = region.clone();
        let created_at = now;

        let monitoring_task = tauri::async_runtime::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(check_interval_seconds));
            // Without this, a stall (system sleep, a long freeze) is repaid as
            // a burst of catch-up ticks, each one a full screen capture.
            interval.set_missed_tick_behavior(MissedTickBehavior::Skip);

            let manager = timer_manager();
            let mut previous = baseline;
            // Monotonic. This value only ever measures elapsed time, so it
            // must not come from the wall clock: `SystemTime` here meant a
            // backward step made `now - start_time` underflow (a panic in a
            // debug build, a wrap in a release build that read as a huge
            // elapsed time and stopped the monitor at once). It is paired with
            // the wall clock in `deadline_reached`, because on Apple targets a
            // monotonic `Instant` does not advance while the machine sleeps.
            let monitor_started_at = Instant::now();

            loop {
                interval.tick().await;

                let monotonic_elapsed = monitor_started_at.elapsed().as_secs();
                let wall_elapsed = unix_now()
                    .map(|now| wall_elapsed_seconds(created_at, now))
                    .unwrap_or(0);
                if deadline_reached(monotonic_elapsed, wall_elapsed, max_duration_seconds) {
                    info!(
                        "Screen monitor {} reached its {}s deadline, stopping",
                        timer_id_clone, max_duration_seconds
                    );
                    manager
                        .release_timer(Some(&app_handle_clone), &timer_id_clone)
                        .await;
                    break;
                }

                // Cancelled from outside: Escape, shutdown, or the model's
                // own `cancel_timer`.
                if manager.get_timer(&timer_id_clone).await.is_none() {
                    debug!("Screen monitor {} was cancelled, stopping", timer_id_clone);
                    break;
                }

                let region_for_tick = region_for_task.clone();
                let captured =
                    tokio::task::spawn_blocking(move || capture_region(region_for_tick.as_ref()))
                        .await;
                let current = match captured {
                    Ok(Ok((_rect, image))) => image,
                    Ok(Err(e)) => {
                        error!(
                            "Failed to capture the screen for monitor {}: {}",
                            timer_id_clone, e
                        );
                        continue;
                    }
                    Err(e) => {
                        error!(
                            "Screen capture task failed for monitor {}: {}",
                            timer_id_clone, e
                        );
                        continue;
                    }
                };

                let Some(ratio) = changed_pixel_ratio(&previous, &current, PIXEL_TOLERANCE) else {
                    debug!(
                        "Screen monitor {} captured {}x{} where its baseline was {}x{} (the pointer moved to another display, or the resolution changed); re-baselining without waking",
                        timer_id_clone,
                        current.width(),
                        current.height(),
                        previous.width(),
                        previous.height()
                    );
                    previous = current;
                    continue;
                };

                if !change_detected(ratio, threshold) {
                    previous = current;
                    continue;
                }

                // The budget is checked again here, not only at arm time: a
                // chain of monitors that each arm the next one has to run out
                // of wakes somewhere, and this is the only place a wake is
                // actually spent.
                if !manager.try_consume_wake().await {
                    warn!(
                        "Screen monitor {} saw its change but the wake budget ({} per hour) is spent; stopping without starting a turn",
                        timer_id_clone, MAX_WAKES_PER_WINDOW
                    );
                    manager
                        .release_timer(Some(&app_handle_clone), &timer_id_clone)
                        .await;
                    break;
                }

                info!(
                    "Screen monitor {}: {:.2}% of the watched pixels changed (threshold {:.2}%), starting a turn",
                    timer_id_clone,
                    ratio * 100.0,
                    f64::from(threshold) * 100.0
                );

                if let Some(expired_timer) = manager
                    .release_timer(Some(&app_handle_clone), &timer_id_clone)
                    .await
                {
                    if let Err(e) = app_handle_clone.emit(events::timer::EXPIRED, &expired_timer) {
                        error!(
                            "{}",
                            error_messages::format_strings::FAILED_TO_EMIT_TIMER_EXPIRED_EVENT
                                .replace("{}", &e.to_string())
                        );
                    }
                }
                break;
            }
        });

        manager
            .add_monitoring_task(timer_id.clone(), monitoring_task)
            .await;

        Ok(json!({
            "success": true,
            "timer_id": timer_id,
            "message": format!(
                "Watching the screen: {}. Fires once when {:.0}% of the watched pixels change, gives up after {}s, and the person can end it with Escape.",
                description,
                f64::from(threshold) * 100.0,
                max_duration_seconds
            ),
            "check_interval_seconds": check_interval_seconds,
            // Everything below is what was *applied*, not what was asked for.
            "threshold": threshold,
            "max_duration_seconds": max_duration_seconds,
            "region": {
                "x": rect.x,
                "y": rect.y,
                "width": rect.width,
                "height": rect.height
            },
            "wakes_remaining": wakes_remaining
        }))
    }

    /// Executes the `set_screen_monitor` tool operation (non-macOS platforms).
    ///
    /// Returns an error indicating screen monitoring is only supported on macOS.
    ///
    /// # Arguments
    /// * `_input` - Unused input (screen monitoring not supported)
    /// * `_app_handle` - Unused app handle
    ///
    /// # Returns
    /// Error message indicating platform limitation
    #[cfg(not(target_os = "macos"))]
    pub async fn set_screen_monitor_exec(
        _input: Value,
        _app_handle: AppHandle,
    ) -> Result<Value, String> {
        Err(error_messages::tool_errors::SCREEN_MONITORING_MACOS_ONLY.to_string())
    }

    /// Creates the tool definition for the `set_file_monitor` tool.
    ///
    /// Used by: Tool registration system for file system monitoring
    /// Enables monitoring of file creation, modification, deletion, and size changes.
    ///
    /// # Returns
    /// `ToolDefinition` for file system event monitoring
    pub fn set_file_monitor_definition() -> ToolDefinition {
        ToolDefinition {
            name: agent::tool_names::SET_FILE_MONITOR.to_string(),
            description: "Watches one file and starts a new turn once, when the named event happens to it. Useful for a download, a log file, or waiting for a build to write its output. The watch is one-shot: it stops as soon as it fires, and it stops on its own at its deadline. Wakes are budgeted across all monitors, and the person can end it with Escape.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "file_path": {
                        "type": "string",
                        "description": "Path to the file to monitor"
                    },
                    "monitor_type": {
                        "type": "string",
                        "enum": ["created", "modified", "deleted", "size_changed"],
                        "description": "Type of file event to monitor for"
                    },
                    "context": {
                        "type": "object",
                        "description": "Context data to restore when file event occurs",
                        "additionalProperties": true
                    },
                    "description": {
                        "type": "string",
                        "description": "Human-readable description of what this monitor is watching for"
                    },
                    "check_interval_seconds": {
                        "type": "number",
                        "description": "How often to check for file changes in seconds (default 5)",
                        "minimum": 1
                    },
                    "max_duration_seconds": {
                        "type": "number",
                        "description": "How long to watch before giving up, in seconds. Defaults to 1800 (30 minutes) and is capped at 7200 (2 hours); there is no unbounded watch.",
                        "minimum": 1
                    }
                },
                "required": ["file_path", "monitor_type", "context", "description"]
            }),
            api_type: None,
            beta_flag: None,
        }
    }

    /// Executes the `set_file_monitor` tool operation.
    ///
    /// Sets up continuous monitoring of a file for specified events
    /// (creation, modification, deletion, size changes).
    ///
    /// Used by: Download monitoring, log file watching, build process tracking
    ///
    /// # Arguments
    /// * `input` - JSON with file path, monitor type, and configuration
    /// * `app_handle` - Tauri app handle for event emission
    ///
    /// # Returns
    /// Success response with monitor details or error message
    pub async fn set_file_monitor_exec(
        input: Value,
        app_handle: AppHandle,
    ) -> Result<Value, String> {
        let file_path = input["file_path"]
            .as_str()
            .ok_or_else(|| "Missing or invalid 'file_path' parameter".to_string())?
            .to_string();

        let monitor_type_str = input["monitor_type"]
            .as_str()
            .ok_or_else(|| "Missing or invalid 'monitor_type' parameter".to_string())?;

        let monitor_type = match monitor_type_str {
            "created" => FileMonitorType::Created,
            "modified" => FileMonitorType::Modified,
            "deleted" => FileMonitorType::Deleted,
            "size_changed" => FileMonitorType::SizeChanged,
            _ => return Err(error_messages::tool_errors::INVALID_MONITOR_TYPE.to_string()),
        };

        let context = input["context"]
            .as_object()
            .ok_or_else(|| error_messages::tool_errors::MISSING_CONTEXT_PARAMETER.to_string())?
            .clone();

        let description = input["description"]
            .as_str()
            .ok_or_else(|| error_messages::tool_errors::MISSING_DESCRIPTION_PARAMETER.to_string())?
            .to_string();

        let check_interval_seconds =
            schedule_seconds(&input, "check_interval_seconds").unwrap_or(5);
        let max_duration_seconds = monitor_duration_seconds(&input);

        let manager = timer_manager();

        // The spending guard, checked before anything is armed.
        let wakes_remaining = manager.wakes_remaining().await;
        if wakes_remaining == 0 {
            return Err(format!(
                "The wake budget is spent: monitors have already started {} turns in the last hour. Ask the person before watching anything else.",
                MAX_WAKES_PER_WINDOW
            ));
        }

        let timer_id = Uuid::new_v4().to_string();
        let now = unix_now().ok_or_else(|| {
            error_messages::format_strings::SYSTEM_TIME_ERROR
                .replace("{}", "the system clock is before the epoch")
        })?;

        let timer_task = TimerTask {
            id: timer_id.clone(),
            // Always a deadline. See `monitor_duration_seconds`.
            trigger_time: now.saturating_add(max_duration_seconds),
            context: Value::Object(context),
            description: description.clone(),
            created_at: now,
            timer_type: TimerType::FileMonitor {
                file_path: file_path.clone(),
                monitor_type: monitor_type.clone(),
            },
        };

        manager.add_timer(timer_task).await;
        // Claim the stop key before the first tick, so Escape reaches this
        // watch from the moment it exists.
        manager.reconcile_stop_key(Some(&app_handle)).await;

        // Start the monitoring task
        let app_handle_clone = app_handle.clone();
        let timer_id_clone = timer_id.clone();
        let created_at = now;
        let file_path_for_async = file_path.clone();

        // Get initial file state
        let path = PathBuf::from(&file_path_for_async);
        let initial_exists = path.exists();
        let initial_size = if initial_exists {
            fs::metadata(&path).await.map(|m| m.len()).unwrap_or(0)
        } else {
            0
        };
        let initial_modified = read_modified_time(&path).await;

        let monitoring_task = tauri::async_runtime::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(check_interval_seconds));
            // Without this, a stall (system sleep, a long freeze) is repaid as
            // a burst of catch-up ticks.
            interval.set_missed_tick_behavior(MissedTickBehavior::Skip);

            let manager = timer_manager();
            let mut last_exists = initial_exists;
            let mut last_size = initial_size;
            // The baseline `Modified` compares against: the mtime as of the
            // previous tick, not as of the monitor's start. Updated at the end
            // of every tick, exactly like `last_size`.
            let mut last_modified = initial_modified;
            // Monotonic, for measuring elapsed time. Mtimes are only ever
            // compared against each other, so a clock step cannot make one look
            // recent or ancient. The deadline is a different question, and
            // `deadline_reached` pairs this with the wall clock because on
            // Apple targets a monotonic `Instant` does not advance while the
            // machine sleeps.
            let monitor_started_at = Instant::now();

            loop {
                interval.tick().await;

                let monotonic_elapsed = monitor_started_at.elapsed().as_secs();
                let wall_elapsed = unix_now()
                    .map(|now| wall_elapsed_seconds(created_at, now))
                    .unwrap_or(0);
                if deadline_reached(monotonic_elapsed, wall_elapsed, max_duration_seconds) {
                    info!(
                        "File monitor {} reached its {}s deadline, stopping",
                        timer_id_clone, max_duration_seconds
                    );
                    manager
                        .release_timer(Some(&app_handle_clone), &timer_id_clone)
                        .await;
                    break;
                }

                // Cancelled from outside: Escape, shutdown, or the model's
                // own `cancel_timer`.
                if manager.get_timer(&timer_id_clone).await.is_none() {
                    debug!("File monitor {} was cancelled, stopping", timer_id_clone);
                    break;
                }

                // Check file state
                let current_exists = path.exists();
                let current_size = if current_exists {
                    fs::metadata(&path).await.map(|m| m.len()).unwrap_or(0)
                } else {
                    0
                };
                let current_modified = read_modified_time(&path).await;

                let event_detected = match monitor_type {
                    FileMonitorType::Created => !last_exists && current_exists,
                    FileMonitorType::Deleted => last_exists && !current_exists,
                    // A modification is an mtime that differs from the one seen
                    // last tick. `Some` -> `Some` with a new value is the normal
                    // case; the `None` transitions cover a file that appeared,
                    // vanished, or whose mtime the platform refused to report.
                    FileMonitorType::Modified => {
                        modification_detected(current_exists, current_modified, last_modified)
                    }
                    FileMonitorType::SizeChanged => current_exists && current_size != last_size,
                };

                if event_detected {
                    // The budget is spent here, not at arm time, so a chain of
                    // monitors each arming the next one runs out of wakes.
                    if !manager.try_consume_wake().await {
                        warn!(
                            "File monitor {} saw its event but the wake budget ({} per hour) is spent; stopping without starting a turn",
                            timer_id_clone, MAX_WAKES_PER_WINDOW
                        );
                        manager
                            .release_timer(Some(&app_handle_clone), &timer_id_clone)
                            .await;
                        break;
                    }

                    info!(
                        "File event detected in monitor {}: {:?} for {}",
                        timer_id_clone, monitor_type, file_path_for_async
                    );

                    if let Some(expired_timer) = manager
                        .release_timer(Some(&app_handle_clone), &timer_id_clone)
                        .await
                    {
                        // Emit event to frontend to restart agent with context
                        if let Err(e) =
                            app_handle_clone.emit(events::timer::EXPIRED, &expired_timer)
                        {
                            error!(
                                "{}",
                                error_messages::format_strings::FAILED_TO_EMIT_TIMER_EXPIRED_EVENT
                                    .replace("{}", &e.to_string())
                            );
                        }
                    }
                    break;
                }

                last_exists = current_exists;
                last_size = current_size;
                last_modified = current_modified;
            }
        });

        manager
            .add_monitoring_task(timer_id.clone(), monitoring_task)
            .await;

        Ok(json!({
            "success": true,
            "timer_id": timer_id,
            "message": format!(
                "Watching {}: {}. Fires once, gives up after {}s, and the person can end it with Escape.",
                file_path, description, max_duration_seconds
            ),
            "file_path": file_path,
            "monitor_type": monitor_type_str,
            "check_interval_seconds": check_interval_seconds,
            // What was applied, not what was asked for.
            "max_duration_seconds": max_duration_seconds,
            "wakes_remaining": wakes_remaining
        }))
    }

    /// Creates the tool definition for the `cancel_timer` tool.
    ///
    /// Used by: Tool registration system for timer cancellation capabilities
    /// Allows agents to cancel previously set timers when conditions change.
    ///
    /// # Returns
    /// `ToolDefinition` for cancelling active timers by ID
    pub fn cancel_timer_definition() -> ToolDefinition {
        ToolDefinition {
            name: agent::tool_names::CANCEL_TIMER.to_string(),
            description: "Cancels a previously set timer by its ID. Useful if conditions change and the agent no longer needs to restart.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "timer_id": {
                        "type": "string",
                        "description": "The ID of the timer to cancel"
                    }
                },
                "required": ["timer_id"]
            }),
            api_type: None,
            beta_flag: None,
        }
    }

    /// Executes the `cancel_timer` tool operation.
    ///
    /// Cancels an active timer by removing it from the manager and stopping
    /// any associated monitoring tasks.
    ///
    /// Used by: Cleanup processes, condition changes, manual timer cancellation
    ///
    /// # Arguments
    /// * `input` - JSON with timer_id to cancel
    /// * `app_handle` - Tauri app handle for state access
    ///
    /// # Returns
    /// Success/failure response with cancellation details
    pub async fn cancel_timer_exec(input: Value, app_handle: AppHandle) -> Result<Value, String> {
        let timer_id = input["timer_id"]
            .as_str()
            .ok_or_else(|| "Missing or invalid 'timer_id' parameter".to_string())?;

        let timer_manager = timer_manager();

        if let Some(cancelled_timer) = timer_manager
            .release_timer(Some(&app_handle), timer_id)
            .await
        {
            Ok(json!({
                "success": true,
                "message": format!("Timer {} cancelled", timer_id),
                "cancelled_timer": {
                    "id": cancelled_timer.id,
                    "description": cancelled_timer.description,
                    "trigger_time": cancelled_timer.trigger_time,
                    "timer_type": cancelled_timer.timer_type
                }
            }))
        } else {
            Ok(json!({
                "success": false,
                "message": format!("Timer {} not found or already expired", timer_id)
            }))
        }
    }

    /// Creates the tool definition for the `list_timers` tool.
    ///
    /// Used by: Tool registration system for timer status inspection
    /// Enables agents to view all currently active timers and their details.
    ///
    /// # Returns
    /// `ToolDefinition` for listing all active timers
    pub fn list_timers_definition() -> ToolDefinition {
        ToolDefinition {
            name: agent::tool_names::LIST_TIMERS.to_string(),
            description: "Lists all active timers that are currently scheduled. Useful for checking what timers are running.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false
            }),
            api_type: None,
            beta_flag: None,
        }
    }

    /// Executes the `list_timers` tool operation.
    ///
    /// Returns a comprehensive list of all active timers with their configurations,
    /// remaining time, and current status.
    ///
    /// Used by: Status reporting, debugging, timer management interfaces
    ///
    /// # Arguments
    /// * `_input` - Unused (no parameters required)
    /// * `app_handle` - Tauri app handle for state access
    ///
    /// # Returns
    /// JSON array of all active timers with details and time remaining
    pub async fn list_timers_exec(_input: Value, _app_handle: AppHandle) -> Result<Value, String> {
        let timer_manager = timer_manager();

        let active_timers = timer_manager.list_active_timers().await;
        let now = unix_now().ok_or_else(|| {
            error_messages::format_strings::SYSTEM_TIME_ERROR
                .replace("{}", "the system clock is before the epoch")
        })?;

        let timer_info: Vec<Value> = active_timers
            .iter()
            .map(|timer| {
                // Every timer has a real deadline now, so this is plain
                // arithmetic. It used to need a `u64::MAX` special case for
                // monitors, which was the unbounded watch showing through.
                let time_remaining = timer.trigger_time.saturating_sub(now);

                json!({
                    "id": timer.id,
                    "description": timer.description,
                    "kind": timer.timer_type.label(),
                    "trigger_time": timer.trigger_time,
                    "time_remaining_seconds": time_remaining,
                    "created_at": timer.created_at,
                    "timer_type": timer.timer_type
                })
            })
            .collect();

        let wakes_remaining = timer_manager.wakes_remaining().await;

        Ok(json!({
            "success": true,
            "active_timers": timer_info,
            "count": active_timers.len(),
            "wakes_remaining": wakes_remaining,
            "max_wakes_per_hour": MAX_WAKES_PER_WINDOW
        }))
    }

    /// Creates the tool definition for the `check_expired_timers` tool.
    ///
    /// Used by: Tool registration system for expired timer checking
    /// Critical for agent startup to detect if previous timers have expired
    /// and need context restoration.
    ///
    /// # Returns
    /// `ToolDefinition` for checking and retrieving expired timer contexts
    pub fn check_expired_timers_definition() -> ToolDefinition {
        ToolDefinition {
            name: agent::tool_names::CHECK_EXPIRED_TIMERS.to_string(),
            description: "Checks for any expired timers and returns their contexts. This is useful during agent startup to see if the agent should resume a previous task.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false
            }),
            api_type: None,
            beta_flag: None,
        }
    }

    /// Executes the `check_expired_timers` tool operation.
    ///
    /// Scans for expired timers and returns their contexts for agent resumption.
    /// Automatically removes expired timers from the active collection.
    ///
    /// Used by: Agent startup, context restoration, expired timer cleanup
    ///
    /// # Arguments
    /// * `_input` - Unused (no parameters required)
    /// * `app_handle` - Tauri app handle for state access
    ///
    /// # Returns
    /// JSON with expired timer details and contexts for restoration
    pub async fn check_expired_timers_exec(
        _input: Value,
        app_handle: AppHandle,
    ) -> Result<Value, String> {
        let timer_manager = timer_manager();

        let expired_timers = timer_manager.get_expired_timers().await;

        // Remove expired timers from active list
        for timer in &expired_timers {
            let _ = timer_manager
                .release_timer(Some(&app_handle), &timer.id)
                .await;
        }

        let expired_info: Vec<Value> = expired_timers
            .iter()
            .map(|timer| {
                json!({
                    "id": timer.id,
                    "description": timer.description,
                    "context": timer.context,
                    "trigger_time": timer.trigger_time,
                    "created_at": timer.created_at,
                    "timer_type": timer.timer_type
                })
            })
            .collect();

        Ok(json!({
            "success": true,
            "expired_timers": expired_info,
            "count": expired_timers.len(),
            "message": if expired_timers.is_empty() {
                error_messages::tool_errors::NO_EXPIRED_TIMERS_FOUND
            } else {
                error_messages::tool_errors::FOUND_EXPIRED_TIMERS_WITH_CONTEXT
            }
        }))
    }
}

/// Registers all timer tools with the provider for agent task scheduling and resumption.
///
/// This is the main registration function that makes all timer capabilities available
/// to agents. Includes simple timers, monitoring timers, and timer management tools.
///
/// Used by: Agent initialization in `anthropic.rs`, tool provider setup
///
/// # Arguments
/// * `provider` - Mutable reference to LocalToolProvider for tool registration
/// * `app_handle` - Tauri app handle for state access and event emission
///
/// # Tools Registered
/// - `set_timer`: Simple delay timers with context restoration
/// - `set_screen_monitor`: Screen change monitoring (macOS only)
/// - `set_file_monitor`: File system event monitoring
/// - `cancel_timer`: Timer cancellation by ID
/// - `list_timers`: List all active timers with status
/// - `check_expired_timers`: Check for expired timers needing context restoration
pub async fn register_timer_tools(provider: &mut LocalToolProvider, app_handle: AppHandle) {
    // set_timer
    let set_timer_def = timer_tools_impl::set_timer_definition();
    let app_handle_clone1 = app_handle.clone();
    let set_timer_exec = move |input| {
        let handle = app_handle_clone1.clone();
        async move { timer_tools_impl::set_timer_exec(input, handle).await }
    };
    provider
        .register_async_tool(set_timer_def, set_timer_exec)
        .await;

    // set_screen_monitor
    let set_screen_monitor_def = timer_tools_impl::set_screen_monitor_definition();
    let app_handle_clone2 = app_handle.clone();
    let set_screen_monitor_exec = move |input| {
        let handle = app_handle_clone2.clone();
        async move { timer_tools_impl::set_screen_monitor_exec(input, handle).await }
    };
    provider
        .register_async_tool(set_screen_monitor_def, set_screen_monitor_exec)
        .await;

    // set_file_monitor
    let set_file_monitor_def = timer_tools_impl::set_file_monitor_definition();
    let app_handle_clone3 = app_handle.clone();
    let set_file_monitor_exec = move |input| {
        let handle = app_handle_clone3.clone();
        async move { timer_tools_impl::set_file_monitor_exec(input, handle).await }
    };
    provider
        .register_async_tool(set_file_monitor_def, set_file_monitor_exec)
        .await;

    // cancel_timer
    let cancel_timer_def = timer_tools_impl::cancel_timer_definition();
    let app_handle_clone4 = app_handle.clone();
    let cancel_timer_exec = move |input| {
        let handle = app_handle_clone4.clone();
        async move { timer_tools_impl::cancel_timer_exec(input, handle).await }
    };
    provider
        .register_async_tool(cancel_timer_def, cancel_timer_exec)
        .await;

    // list_timers
    let list_timers_def = timer_tools_impl::list_timers_definition();
    let app_handle_clone5 = app_handle.clone();
    let list_timers_exec = move |input| {
        let handle = app_handle_clone5.clone();
        async move { timer_tools_impl::list_timers_exec(input, handle).await }
    };
    provider
        .register_async_tool(list_timers_def, list_timers_exec)
        .await;

    // check_expired_timers
    let check_expired_def = timer_tools_impl::check_expired_timers_definition();
    let app_handle_clone6 = app_handle.clone();
    let check_expired_exec = move |input| {
        let handle = app_handle_clone6.clone();
        async move { timer_tools_impl::check_expired_timers_exec(input, handle).await }
    };
    provider
        .register_async_tool(check_expired_def, check_expired_exec)
        .await;

    info!("Registered enhanced timer tools: set_timer, set_screen_monitor, set_file_monitor, cancel_timer, list_timers, check_expired_timers");
}

/// Every number of seconds these tools take comes from a model, so the reader
/// of `schedule_seconds` is an adversary by default: fractional, negative,
/// zero, absurd and non-numeric all have to land somewhere safe.
#[cfg(test)]
mod schedule_seconds_tests {
    use super::timer_tools_impl::{schedule_seconds, MAX_SCHEDULE_SECONDS};
    use serde_json::json;

    #[test]
    fn a_whole_number_of_seconds_passes_through() {
        assert_eq!(schedule_seconds(&json!({ "s": 30 }), "s"), Some(30));
    }

    #[test]
    fn fractional_seconds_are_rounded_not_discarded() {
        // `as_u64()` returned None for these, so the caller silently fell back
        // to its default and the model's request vanished without a word.
        assert_eq!(schedule_seconds(&json!({ "s": 2.4 }), "s"), Some(2));
        assert_eq!(schedule_seconds(&json!({ "s": 2.6 }), "s"), Some(3));
    }

    #[test]
    fn zero_becomes_one_second() {
        // A zero period panics `tokio::time::interval`, which killed the
        // monitoring task outright.
        assert_eq!(schedule_seconds(&json!({ "s": 0 }), "s"), Some(1));
        assert_eq!(schedule_seconds(&json!({ "s": 0.2 }), "s"), Some(1));
    }

    #[test]
    fn negative_values_become_one_second() {
        assert_eq!(schedule_seconds(&json!({ "s": -5 }), "s"), Some(1));
        assert_eq!(schedule_seconds(&json!({ "s": -1e30 }), "s"), Some(1));
    }

    #[test]
    fn absurd_values_are_capped_rather_than_overflowing_the_deadline() {
        // The saturating float cast puts these on u64::MAX; without the clamp
        // `now + seconds` overflows, panicking in debug and wrapping in
        // release to a deadline in the past.
        assert_eq!(
            schedule_seconds(&json!({ "s": 1e30 }), "s"),
            Some(MAX_SCHEDULE_SECONDS)
        );
        assert_eq!(
            schedule_seconds(&json!({ "s": u64::MAX }), "s"),
            Some(MAX_SCHEDULE_SECONDS)
        );
        assert_eq!(
            schedule_seconds(&json!({ "s": MAX_SCHEDULE_SECONDS + 1 }), "s"),
            Some(MAX_SCHEDULE_SECONDS)
        );
    }

    #[test]
    fn nan_lands_at_the_floor_rather_than_panicking() {
        // serde_json cannot hold NaN, so this arrives as a very large finite
        // number or not at all; the guard is that nothing here can panic.
        assert_eq!(
            schedule_seconds(&json!({ "s": f64::MAX }), "s"),
            Some(MAX_SCHEDULE_SECONDS)
        );
    }

    #[test]
    fn a_missing_or_non_numeric_key_is_none_not_a_default() {
        // `None` has to stay distinguishable from a clamped value, because the
        // caller uses it to tell "not asked for" from "asked for badly".
        assert_eq!(schedule_seconds(&json!({}), "s"), None);
        assert_eq!(schedule_seconds(&json!({ "s": "30" }), "s"), None);
        assert_eq!(schedule_seconds(&json!({ "s": null }), "s"), None);
    }

    #[test]
    fn the_cap_is_expressed_in_the_unit_it_claims() {
        // Thirty days, stated in seconds. If this ever disagrees with the doc
        // comment, that is the `wait` bug again.
        assert_eq!(MAX_SCHEDULE_SECONDS, 2_592_000);
    }
}

#[cfg(test)]
mod file_monitor_tests {
    use super::timer_tools_impl::{modification_detected, read_modified_time};
    use std::time::{Duration, SystemTime};

    fn stamp(secs_after_epoch: u64) -> Option<SystemTime> {
        Some(SystemTime::UNIX_EPOCH + Duration::from_secs(secs_after_epoch))
    }

    #[test]
    fn an_unchanged_mtime_is_not_a_modification() {
        assert!(!modification_detected(true, stamp(1_000), stamp(1_000)));
    }

    #[test]
    fn a_changed_mtime_is_a_modification() {
        assert!(modification_detected(true, stamp(1_001), stamp(1_000)));
    }

    #[test]
    fn an_edit_long_after_the_monitor_started_is_still_detected() {
        // The regression this fixes. The old check asked "was this file
        // modified within check_interval + 1 seconds of the monitor starting?",
        // so an edit an hour in was invisible. Comparing tick to tick, the age
        // of the monitor is irrelevant.
        let monitor_started = 1_000;
        let previous_tick = stamp(monitor_started);
        let edited_an_hour_later = stamp(monitor_started + 3_600);
        assert!(modification_detected(
            true,
            edited_an_hour_later,
            previous_tick
        ));
    }

    #[test]
    fn a_file_that_is_gone_is_never_a_modification() {
        // Deletion belongs to FileMonitorType::Deleted, not here.
        assert!(!modification_detected(false, None, stamp(1_000)));
    }

    #[test]
    fn a_file_that_appeared_since_the_last_tick_counts_as_changed() {
        assert!(modification_detected(true, stamp(1_000), None));
    }

    #[tokio::test]
    async fn a_missing_path_has_no_modification_time() {
        let missing = std::env::temp_dir().join("juno-file-monitor-does-not-exist-4200");
        assert_eq!(read_modified_time(&missing).await, None);
    }

    #[tokio::test]
    async fn writing_to_a_file_moves_its_modification_time() {
        let path = std::env::temp_dir().join(format!(
            "juno-file-monitor-{}-{}",
            std::process::id(),
            "mtime"
        ));
        let _ = tokio::fs::remove_file(&path).await;
        tokio::fs::write(&path, b"first")
            .await
            .expect("temp file should be writable");
        let first = read_modified_time(&path).await;
        assert!(first.is_some());

        // Filesystem mtime granularity can be coarse, so give the second write
        // a clear gap rather than racing it.
        tokio::time::sleep(Duration::from_millis(1_100)).await;
        tokio::fs::write(&path, b"second")
            .await
            .expect("temp file should still be writable");
        let second = read_modified_time(&path).await;

        assert!(modification_detected(true, second, first));
        let _ = tokio::fs::remove_file(&path).await;
    }
}

/// A watch that nothing outside this module can stop is the defect these pin.
///
/// The Escape path is `stop_coordinator::perform_coordinated_cleanup`, which
/// calls `TimerManager::cancel_all_timers`; app shutdown
/// (`cleanup::cleanup_application`) calls the same function. So these exercise
/// the real stop path, with `None` for the app handle because a unit test has
/// no Tauri application to register a stop key with.
#[cfg(test)]
mod stoppability_tests {
    use super::*;
    use serde_json::json;

    fn task(id: &str, timer_type: TimerType) -> TimerTask {
        TimerTask {
            id: id.to_string(),
            trigger_time: 2_000,
            context: json!({}),
            description: format!("test {}", id),
            created_at: 1_000,
            timer_type,
        }
    }

    fn screen_monitor(id: &str) -> TimerTask {
        task(
            id,
            TimerType::ScreenMonitor {
                region: None,
                threshold: 0.1,
                check_interval_seconds: 2,
            },
        )
    }

    fn file_monitor(id: &str) -> TimerTask {
        task(
            id,
            TimerType::FileMonitor {
                file_path: "/tmp/does-not-matter".to_string(),
                monitor_type: FileMonitorType::Modified,
            },
        )
    }

    #[tokio::test]
    async fn the_stop_path_cancels_every_armed_watch() {
        let manager = TimerManager::new();
        manager.add_timer(screen_monitor("screen")).await;
        manager.add_timer(file_monitor("file")).await;
        manager.add_timer(task("delay", TimerType::Simple)).await;

        assert!(manager.has_armed_monitor().await);
        assert_eq!(manager.armed_monitors().await.len(), 2);

        let cancelled = manager.cancel_all_timers(None).await;

        // Monitors and plain delays alike: all three can start a turn on their
        // own, and Escape means stop all of it.
        assert_eq!(cancelled.len(), 3);
        assert!(!manager.has_armed_monitor().await);
        assert!(manager.list_active_timers().await.is_empty());

        // `get_timer(..).is_none()` is the condition each monitor loop checks
        // every tick to decide whether it has been cancelled. If that stopped
        // being true after a cancel, the loop would keep capturing the screen.
        assert!(manager.get_timer("screen").await.is_none());
        assert!(manager.get_timer("file").await.is_none());
    }

    #[tokio::test]
    async fn cancelling_is_idempotent_and_safe_with_nothing_armed() {
        let manager = TimerManager::new();
        assert!(manager.cancel_all_timers(None).await.is_empty());
        manager.add_timer(screen_monitor("screen")).await;
        assert_eq!(manager.cancel_all_timers(None).await.len(), 1);
        assert!(manager.cancel_all_timers(None).await.is_empty());
    }

    #[tokio::test]
    async fn releasing_one_watch_leaves_the_others_armed() {
        let manager = TimerManager::new();
        manager.add_timer(screen_monitor("a")).await;
        manager.add_timer(screen_monitor("b")).await;

        assert!(manager.release_timer(None, "a").await.is_some());
        assert!(manager.has_armed_monitor().await);
        assert!(manager.release_timer(None, "b").await.is_some());
        assert!(!manager.has_armed_monitor().await);
        // A stale release is a no-op, not a panic and not a second removal.
        assert!(manager.release_timer(None, "a").await.is_none());
    }

    #[test]
    fn a_plain_delay_is_not_a_watch_and_every_monitor_is() {
        assert!(!TimerType::Simple.is_monitor());
        assert!(TimerType::ScreenMonitor {
            region: None,
            threshold: 0.1,
            check_interval_seconds: 2
        }
        .is_monitor());
        assert!(TimerType::FileMonitor {
            file_path: "/tmp/x".to_string(),
            monitor_type: FileMonitorType::Created
        }
        .is_monitor());
        assert!(TimerType::ApplicationMonitor {
            app_name: "Safari".to_string(),
            monitor_state: AppMonitorState::Launched
        }
        .is_monitor());
    }
}

/// The wake budget is the spending guard, so it gets tested as one.
#[cfg(test)]
mod wake_budget_tests {
    use super::*;

    // Every test below moves time *forwards* from `Instant::now()` rather than
    // subtracting from it. On macOS an `Instant` is time since boot, so
    // `now - one hour` fails on a machine that has been up for less than an
    // hour, which a fresh CI runner usually has, and silently falls back to
    // `now`, turning a window test into a test of nothing.

    #[test]
    fn the_budget_runs_out_and_then_refuses() {
        let mut ledger = WakeLedger::default();
        let now = Instant::now();
        for spent in 0..MAX_WAKES_PER_WINDOW {
            assert_eq!(ledger.remaining(now), MAX_WAKES_PER_WINDOW - spent);
            assert!(ledger.try_consume(now), "wake {} should be allowed", spent);
        }
        assert_eq!(ledger.remaining(now), 0);
        assert!(!ledger.try_consume(now));
        // Still refused on a later call: a refusal does not reset anything.
        assert!(!ledger.try_consume(now));
    }

    #[test]
    fn wakes_age_out_of_the_window() {
        let mut ledger = WakeLedger::default();
        let now = Instant::now();
        for _ in 0..MAX_WAKES_PER_WINDOW {
            assert!(ledger.try_consume(now));
        }
        assert_eq!(ledger.remaining(now), 0);

        // An hour on, the budget is clear again, which is what makes this a
        // rolling rate limit rather than a once-per-process allowance.
        let past_the_window = now + WAKE_BUDGET_WINDOW + Duration::from_secs(60);
        assert_eq!(ledger.remaining(past_the_window), MAX_WAKES_PER_WINDOW);
        assert!(ledger.try_consume(past_the_window));
    }

    #[test]
    fn a_wake_just_inside_the_window_still_counts() {
        let mut ledger = WakeLedger::default();
        let now = Instant::now();
        assert!(ledger.try_consume(now));
        let half_way_through = now + WAKE_BUDGET_WINDOW / 2;
        assert_eq!(ledger.remaining(half_way_through), MAX_WAKES_PER_WINDOW - 1);
    }

    #[test]
    fn clearing_the_ledger_restores_the_whole_budget() {
        let mut ledger = WakeLedger::default();
        let now = Instant::now();
        assert!(ledger.try_consume(now));
        ledger.clear();
        assert_eq!(ledger.remaining(now), MAX_WAKES_PER_WINDOW);
    }

    #[tokio::test]
    async fn the_manager_spends_from_one_shared_budget() {
        // Shared on purpose: the thing being bounded is how many turns monitors
        // start, not how many any single monitor starts, because each monitor
        // is one-shot and a chain of them is the runaway case.
        let manager = TimerManager::new();
        let before = manager.wakes_remaining().await;
        assert_eq!(before, MAX_WAKES_PER_WINDOW);
        assert!(manager.try_consume_wake().await);
        assert_eq!(manager.wakes_remaining().await, MAX_WAKES_PER_WINDOW - 1);
    }
}

/// What the tool schema promises has to be what the code does. This codebase
/// calls the opposite the dead control pattern, and `threshold` was a textbook
/// case: accepted, echoed back to the model, and read by nothing.
#[cfg(test)]
mod advertised_parameter_tests {
    use super::timer_tools_impl::{
        change_detected, change_threshold, monitor_duration_seconds, set_file_monitor_definition,
        set_screen_monitor_definition, DEFAULT_CHANGE_THRESHOLD, DEFAULT_MONITOR_DURATION_SECONDS,
        MAX_MONITOR_DURATION_SECONDS,
    };
    use serde_json::json;

    fn property_names(schema: &serde_json::Value) -> Vec<String> {
        let mut names: Vec<String> = schema["properties"]
            .as_object()
            .map(|props| props.keys().cloned().collect())
            .unwrap_or_default();
        names.sort();
        names
    }

    #[test]
    fn the_screen_monitor_advertises_exactly_the_parameters_it_applies() {
        // Adding a parameter to the schema without wiring it up fails here,
        // which is the whole point: a control the model is offered and the code
        // ignores is worse than no control.
        assert_eq!(
            property_names(&set_screen_monitor_definition().input_schema),
            vec![
                "check_interval_seconds",
                "context",
                "description",
                "max_duration_seconds",
                "region",
                "threshold"
            ]
        );
    }

    #[test]
    fn the_file_monitor_advertises_exactly_the_parameters_it_applies() {
        assert_eq!(
            property_names(&set_file_monitor_definition().input_schema),
            vec![
                "check_interval_seconds",
                "context",
                "description",
                "file_path",
                "max_duration_seconds",
                "monitor_type"
            ]
        );
    }

    #[test]
    fn the_advertised_threshold_default_is_the_one_applied() {
        let definition = set_screen_monitor_definition();
        let described = definition.input_schema["properties"]["threshold"]["description"]
            .as_str()
            .unwrap_or_default();
        assert!(
            described.contains("0.1"),
            "the schema should state the default it actually uses, got: {}",
            described
        );
        assert_eq!(
            change_threshold(&json!({})),
            DEFAULT_CHANGE_THRESHOLD as f32
        );
    }

    #[test]
    fn a_threshold_the_model_asks_for_is_the_threshold_that_filters() {
        assert_eq!(change_threshold(&json!({ "threshold": 0.25 })), 0.25);
        // Out of range is clamped rather than accepted and ignored.
        assert_eq!(change_threshold(&json!({ "threshold": 5.0 })), 1.0);
        assert_eq!(change_threshold(&json!({ "threshold": -1.0 })), 0.0);
        // Non-numeric and non-finite fall back to the stated default instead of
        // becoming a NaN that compares false against everything, which would
        // leave the watch unable to ever fire.
        assert_eq!(
            change_threshold(&json!({ "threshold": "lots" })),
            DEFAULT_CHANGE_THRESHOLD as f32
        );
        assert_eq!(
            change_threshold(&json!({ "threshold": f64::MAX })),
            1.0,
            "a finite but absurd threshold clamps to 1.0"
        );
    }

    #[test]
    fn the_threshold_decides_whether_a_change_fires() {
        // 5% of pixels moved, 10% asked for: not a change.
        assert!(!change_detected(0.05, 0.1));
        assert!(change_detected(0.1, 0.1));
        assert!(change_detected(0.9, 0.1));
        // A threshold of zero means any change at all, but still a change: a
        // perfectly still screen must not fire, which is what the old
        // byte-inequality test got wrong in the other direction.
        assert!(change_detected(0.000_01, 0.0));
        assert!(!change_detected(0.0, 0.0));
        // A threshold of one is reachable: every watched pixel differing.
        assert!(change_detected(1.0, 1.0));
        assert!(!change_detected(0.99, 1.0));
    }

    #[test]
    fn the_advertised_duration_bound_is_the_one_applied() {
        for definition in [
            set_screen_monitor_definition(),
            set_file_monitor_definition(),
        ] {
            let described = definition.input_schema["properties"]["max_duration_seconds"]
                ["description"]
                .as_str()
                .unwrap_or_default();
            assert!(
                described.contains(&DEFAULT_MONITOR_DURATION_SECONDS.to_string()),
                "the schema should state the default it uses, got: {}",
                described
            );
            assert!(
                described.contains(&MAX_MONITOR_DURATION_SECONDS.to_string()),
                "the schema should state the cap it enforces, got: {}",
                described
            );
            assert!(
                !described.contains("optional"),
                "the duration is not optional any more, got: {}",
                described
            );
        }
    }

    #[test]
    fn a_monitor_cannot_be_armed_without_a_bound() {
        // Every way of declining to give a duration still yields one.
        for input in [
            json!({}),
            json!({ "max_duration_seconds": null }),
            json!({ "max_duration_seconds": "forever" }),
        ] {
            assert_eq!(
                monitor_duration_seconds(&input),
                DEFAULT_MONITOR_DURATION_SECONDS,
                "omitting the duration must not mean unbounded"
            );
        }

        // And no way of asking for more than the cap.
        for input in [
            json!({ "max_duration_seconds": MAX_MONITOR_DURATION_SECONDS + 1 }),
            json!({ "max_duration_seconds": 1e30 }),
            json!({ "max_duration_seconds": u64::MAX }),
        ] {
            assert_eq!(
                monitor_duration_seconds(&input),
                MAX_MONITOR_DURATION_SECONDS
            );
        }

        // A sane request is honoured exactly.
        assert_eq!(
            monitor_duration_seconds(&json!({ "max_duration_seconds": 45 })),
            45
        );
        // Zero and negative land on the one-second floor, never on "no bound".
        assert_eq!(
            monitor_duration_seconds(&json!({ "max_duration_seconds": 0 })),
            1
        );
        assert_eq!(
            monitor_duration_seconds(&json!({ "max_duration_seconds": -9 })),
            1
        );
    }

    #[test]
    fn the_bound_constants_say_what_they_mean() {
        assert_eq!(DEFAULT_MONITOR_DURATION_SECONDS, 1_800);
        assert_eq!(MAX_MONITOR_DURATION_SECONDS, 7_200);
        // A const block, so a default above the cap fails the build rather
        // than this test.
        const { assert!(DEFAULT_MONITOR_DURATION_SECONDS <= MAX_MONITOR_DURATION_SECONDS) };
    }
}

/// The deadline has to survive a closed lid, and a stepped clock must not end a
/// watch early.
#[cfg(test)]
mod deadline_tests {
    use super::timer_tools_impl::{deadline_reached, wall_elapsed_seconds};

    #[test]
    fn a_watch_inside_its_bound_keeps_running() {
        assert!(!deadline_reached(100, 100, 1_800));
    }

    #[test]
    fn the_monotonic_clock_ends_a_watch_that_ran_its_course() {
        assert!(deadline_reached(1_800, 1_800, 1_800));
        assert!(deadline_reached(1_801, 0, 1_800));
    }

    #[test]
    fn the_wall_clock_ends_a_watch_that_slept_through_its_deadline() {
        // This is the sleep case. Rust's `Instant` on Apple targets is
        // CLOCK_UPTIME_RAW, which does not advance while the machine is
        // asleep, so after a lunch break the monotonic clock still reads two
        // minutes and the watch would resume capturing the screen.
        assert!(deadline_reached(120, 7_000, 1_800));
    }

    #[test]
    fn a_backward_clock_step_does_not_end_a_watch_early() {
        // Wall elapsed saturates to zero rather than underflowing, and the
        // monotonic clock still holds the real bound.
        assert_eq!(wall_elapsed_seconds(2_000, 1_000), 0);
        assert!(!deadline_reached(
            10,
            wall_elapsed_seconds(2_000, 1_000),
            1_800
        ));
    }

    #[test]
    fn wall_elapsed_is_plain_subtraction_going_forwards() {
        assert_eq!(wall_elapsed_seconds(1_000, 1_450), 450);
        assert_eq!(wall_elapsed_seconds(1_000, 1_000), 0);
    }
}

/// `region` and `threshold` were accepted, stored, echoed back to the model and
/// read by nothing: every capture was the whole screen and the change test was
/// byte inequality on two base64 PNGs. These pin the code that makes both mean
/// something.
#[cfg(all(test, target_os = "macos"))]
mod screen_comparison_tests {
    use super::timer_tools_impl::{changed_pixel_ratio, resolve_rect, PixelRect, PIXEL_TOLERANCE};
    use super::ScreenRegion;
    use image::{Rgba, RgbaImage};

    fn region(x: f64, y: f64, width: f64, height: f64) -> ScreenRegion {
        ScreenRegion {
            x,
            y,
            width,
            height,
        }
    }

    fn solid(width: u32, height: u32, value: u8) -> RgbaImage {
        RgbaImage::from_pixel(width, height, Rgba([value, value, value, 255]))
    }

    #[test]
    fn no_region_watches_the_whole_capture() {
        assert_eq!(
            resolve_rect(1920, 1080, None),
            Ok(PixelRect {
                x: 0,
                y: 0,
                width: 1920,
                height: 1080
            })
        );
    }

    #[test]
    fn a_region_inside_the_display_is_used_exactly() {
        assert_eq!(
            resolve_rect(1920, 1080, Some(&region(100.0, 200.0, 300.0, 400.0))),
            Ok(PixelRect {
                x: 100,
                y: 200,
                width: 300,
                height: 400
            })
        );
    }

    #[test]
    fn a_region_that_overhangs_the_display_is_trimmed() {
        assert_eq!(
            resolve_rect(800, 600, Some(&region(700.0, 500.0, 400.0, 400.0))),
            Ok(PixelRect {
                x: 700,
                y: 500,
                width: 100,
                height: 100
            })
        );
    }

    #[test]
    fn negative_and_fractional_coordinates_land_somewhere_sane() {
        assert_eq!(
            resolve_rect(800, 600, Some(&region(-50.0, -50.0, 100.9, 100.9))),
            Ok(PixelRect {
                x: 0,
                y: 0,
                width: 100,
                height: 100
            })
        );
    }

    #[test]
    fn a_region_outside_the_display_arms_nothing() {
        // Deliberately an error rather than a quiet fall back to the full
        // screen: falling back would be the same lie in a new place.
        assert!(resolve_rect(800, 600, Some(&region(900.0, 10.0, 100.0, 100.0))).is_err());
        assert!(resolve_rect(800, 600, Some(&region(10.0, 700.0, 100.0, 100.0))).is_err());
    }

    #[test]
    fn an_empty_region_arms_nothing() {
        assert!(resolve_rect(800, 600, Some(&region(10.0, 10.0, 0.0, 50.0))).is_err());
        assert!(resolve_rect(800, 600, Some(&region(10.0, 10.0, 50.0, 0.0))).is_err());
        assert!(resolve_rect(800, 600, Some(&region(10.0, 10.0, -5.0, 50.0))).is_err());
    }

    #[test]
    fn a_display_with_no_pixels_arms_nothing() {
        assert!(resolve_rect(0, 0, None).is_err());
    }

    #[test]
    fn an_unchanged_capture_measures_as_no_change() {
        let a = solid(4, 4, 10);
        let b = solid(4, 4, 10);
        assert_eq!(changed_pixel_ratio(&a, &b, PIXEL_TOLERANCE), Some(0.0));
    }

    #[test]
    fn the_ratio_is_the_fraction_of_watched_pixels_that_moved() {
        let before = solid(2, 2, 0);
        let mut after = before.clone();
        after.put_pixel(0, 0, Rgba([255, 255, 255, 255]));
        assert_eq!(
            changed_pixel_ratio(&before, &after, PIXEL_TOLERANCE),
            Some(0.25)
        );

        after.put_pixel(1, 1, Rgba([255, 255, 255, 255]));
        assert_eq!(
            changed_pixel_ratio(&before, &after, PIXEL_TOLERANCE),
            Some(0.5)
        );
    }

    #[test]
    fn a_whole_screen_repaint_measures_as_everything() {
        let before = solid(8, 8, 0);
        let after = solid(8, 8, 255);
        assert_eq!(
            changed_pixel_ratio(&before, &after, PIXEL_TOLERANCE),
            Some(1.0)
        );
    }

    #[test]
    fn a_difference_inside_the_tolerance_is_not_a_change() {
        // Antialiasing and cursor blending move a channel by a point or two
        // without anything on screen having happened.
        let before = solid(4, 4, 100);
        let after = solid(4, 4, 100 + PIXEL_TOLERANCE);
        assert_eq!(
            changed_pixel_ratio(&before, &after, PIXEL_TOLERANCE),
            Some(0.0)
        );

        let past_tolerance = solid(4, 4, 100 + PIXEL_TOLERANCE + 1);
        assert_eq!(
            changed_pixel_ratio(&before, &past_tolerance, PIXEL_TOLERANCE),
            Some(1.0)
        );
    }

    #[test]
    fn the_alpha_channel_counts_too() {
        let before = RgbaImage::from_pixel(1, 1, Rgba([10, 10, 10, 255]));
        let after = RgbaImage::from_pixel(1, 1, Rgba([10, 10, 10, 0]));
        assert_eq!(
            changed_pixel_ratio(&before, &after, PIXEL_TOLERANCE),
            Some(1.0)
        );
    }

    #[test]
    fn captures_of_different_sizes_are_not_comparable() {
        // The capture follows the pointer across displays, so this happens when
        // the pointer wanders or the resolution changes. `None` tells the loop
        // to re-baseline rather than wake a turn, because "your other monitor
        // is a different size" is not the change anyone asked to watch for.
        let a = solid(4, 4, 0);
        let b = solid(8, 8, 0);
        assert_eq!(changed_pixel_ratio(&a, &b, PIXEL_TOLERANCE), None);
    }

    #[test]
    fn an_empty_capture_is_not_comparable() {
        let a = RgbaImage::new(0, 0);
        let b = RgbaImage::new(0, 0);
        assert_eq!(changed_pixel_ratio(&a, &b, PIXEL_TOLERANCE), None);
    }
}
