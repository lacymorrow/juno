//! How many steps a task took, and where the time went.
//!
//! [`turn_timing`](crate::turn_timing) measures the first second of a turn: key
//! release to first sound. This measures the other end, the whole task, however
//! many model round-trips and desktop actions it took. One line per step, then
//! one for the task:
//!
//! ```text
//! [StepTiming] task=3 step=7 model=1840 tools=320 tool_calls=2 screenshots=1
//! [TaskTiming] task=3 outcome=finished steps=7 wall=18240 model=12900 tools=4100 overhead=1240 tool_calls=11 screenshots=4 first_step=980 last_step=2840 concurrent=0
//! ```
//!
//! Grep `[TaskTiming]` for the summary, `[StepTiming]` for the curve.
//!
//! # Why these fields and not others
//!
//! Two published results decide what is worth logging, and both are about the
//! numbers Juno was *not* recording:
//!
//! - On MacAgentBench (676 macOS tasks) the same Claude Opus 4.6 takes **27.1
//!   steps** under a bare screenshot loop and **8.6** under a good scaffold,
//!   scoring 39.2% and 73.7%. Step count is the number that separates a working
//!   scaffold from a broken one, so it is the first number on the line.
//! - On OSWorld-Human, model calls for planning and reflection are **75-94% of
//!   end-to-end latency**, and a step near the end of a task takes about **3x**
//!   as long as one at the start. So `model` and `tools` are reported split
//!   rather than summed, and every step logs its own line: the growth curve is
//!   the finding, and a per-task average would hide it.
//!
//! `overhead` is wall clock minus model minus tools, which is Juno's own cost:
//! memory locks, approval waits, app-liveness checks, speech. It is the part no
//! benchmark measures for us.
//!
//! # What this does not record, and why
//!
//! **Which model answered.** The `[TurnTiming]` line already carries
//! `provider=`, `model=` and `effort=`, and it is written seconds away in the
//! same log, so a task line can be read next to it. Recording it here a second
//! time would mean either teaching [`AgentBrain`](crate::agent::traits::AgentBrain)
//! to name its model, which is five implementations, or forwarding from the five
//! provider call sites that already report to `turn_timing`. Neither is worth it
//! for a field that is constant across a measurement run.
//!
//! **Token counts and cache hit rates.** They belong to the request, not the
//! step, and the provider is where they are known. Step count and wall clock are
//! what this exists to establish a baseline for.
//!
//! # Nesting, and concurrent sessions
//!
//! A task is what the person waited for, so a specialist the orchestrator
//! delegated to does not open a task of its own: its steps fold into the task
//! that opened it. [`Tracker::begin`] returns the open task's id and counts
//! depth, and the line goes out when the outermost run ends.
//!
//! Parallel background sessions (LAC-1432) are genuinely two tasks, and this
//! tracker holds one. Rather than silently sum them into a meaningless line, a
//! `begin` from a different session than the one that opened the task sets
//! `concurrent=1` on the line, which marks it as not to be trusted for a
//! baseline. Measuring parallel sessions separately would mean keying the
//! tracker by session; nothing needs that yet.
//!
//! The tracker is passive. Nothing here changes what Juno does.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use tracing::info;

/// Log target, so both lines can be filtered on their own with `RUST_LOG`.
pub const TARGET: &str = "juno::task_timing";

/// One task's step count and time split. Pure data, and every method takes the
/// clock as an argument, so the whole thing tests without waiting for time to
/// pass.
#[derive(Debug)]
pub struct TaskTimer {
    id: u64,
    started: Instant,
    /// How many runs are live. The outermost is 1; a delegated specialist makes
    /// it 2. The line goes out when it returns to 0.
    depth: u32,
    /// The session that opened this task, when it had one. A `begin` from any
    /// other session is a concurrent task, not a nested run.
    owner: Option<String>,
    /// Whether a second session ran while this task was open, which makes every
    /// total on the line a sum of two tasks.
    concurrent: bool,
    steps: u32,
    model: Duration,
    tools: Duration,
    tool_calls: u32,
    screenshots: u32,
    first_step: Option<Duration>,
    last_step: Option<Duration>,
    /// Set from the agent state machine's terminal transitions. Last wins: a
    /// specialist that failed inside a task the orchestrator went on to finish
    /// did not fail the task.
    outcome: Option<String>,
    /// Whether a step is open and still accumulating.
    step_open: bool,
    step_model: Duration,
    step_tools: Duration,
    step_tool_calls: u32,
    step_screenshots: u32,
}

impl TaskTimer {
    pub fn new(id: u64, started: Instant, owner: Option<String>) -> Self {
        Self {
            id,
            started,
            depth: 1,
            owner,
            concurrent: false,
            steps: 0,
            model: Duration::ZERO,
            tools: Duration::ZERO,
            tool_calls: 0,
            screenshots: 0,
            first_step: None,
            last_step: None,
            outcome: None,
            step_open: false,
            step_model: Duration::ZERO,
            step_tools: Duration::ZERO,
            step_tool_calls: 0,
            step_screenshots: 0,
        }
    }

    pub fn id(&self) -> u64 {
        self.id
    }

    /// Open a step, folding the one before it into the totals. Returns that
    /// step's line, so stepping is the only hook the agent loop needs: the last
    /// step is folded by [`Self::close_step`] when the task ends.
    pub fn begin_step(&mut self) -> Option<String> {
        let previous = self.close_step();
        self.steps = self.steps.saturating_add(1);
        self.step_open = true;
        previous
    }

    /// Fold the open step into the totals and return its line.
    pub fn close_step(&mut self) -> Option<String> {
        if !self.step_open {
            return None;
        }
        self.step_open = false;

        let elapsed = self.step_model.saturating_add(self.step_tools);
        if self.first_step.is_none() {
            self.first_step = Some(elapsed);
        }
        self.last_step = Some(elapsed);

        self.model = self.model.saturating_add(self.step_model);
        self.tools = self.tools.saturating_add(self.step_tools);
        self.tool_calls = self.tool_calls.saturating_add(self.step_tool_calls);
        self.screenshots = self.screenshots.saturating_add(self.step_screenshots);

        let line = format!(
            "task={} step={} model={} tools={} tool_calls={} screenshots={}",
            self.id,
            self.steps,
            self.step_model.as_millis(),
            self.step_tools.as_millis(),
            self.step_tool_calls,
            self.step_screenshots
        );

        self.step_model = Duration::ZERO;
        self.step_tools = Duration::ZERO;
        self.step_tool_calls = 0;
        self.step_screenshots = 0;

        Some(line)
    }

    /// Time spent waiting on the model: planning and reflection, the 75-94% of
    /// latency OSWorld-Human measured.
    pub fn add_model(&mut self, elapsed: Duration) {
        self.step_model = self.step_model.saturating_add(elapsed);
    }

    /// Time spent actually driving the desktop, and how many calls it took.
    pub fn add_tools(&mut self, elapsed: Duration, calls: u32) {
        self.step_tools = self.step_tools.saturating_add(elapsed);
        self.step_tool_calls = self.step_tool_calls.saturating_add(calls);
    }

    /// A tool result came back carrying a screenshot.
    pub fn add_screenshot(&mut self) {
        self.step_screenshots = self.step_screenshots.saturating_add(1);
    }

    /// Record how the task ended. Last wins.
    pub fn note_outcome(&mut self, outcome: &str) {
        self.outcome = Some(outcome.to_string());
    }

    /// Juno's own cost: wall clock the model and the desktop did not account
    /// for. Saturating, because the two are measured independently and a
    /// rounding overlap must not underflow.
    fn overhead(&self, wall: Duration) -> Duration {
        wall.saturating_sub(self.model).saturating_sub(self.tools)
    }

    /// The task line, minus the `[TaskTiming]` prefix.
    pub fn line(&self, at: Instant) -> String {
        let wall = at.saturating_duration_since(self.started);
        let ms = |d: Option<Duration>| match d {
            Some(d) => d.as_millis().to_string(),
            None => "-".to_string(),
        };
        format!(
            "task={} outcome={} steps={} wall={} model={} tools={} overhead={} tool_calls={} screenshots={} first_step={} last_step={} concurrent={}",
            self.id,
            self.outcome.as_deref().unwrap_or("-"),
            self.steps,
            wall.as_millis(),
            self.model.as_millis(),
            self.tools.as_millis(),
            self.overhead(wall).as_millis(),
            self.tool_calls,
            self.screenshots,
            ms(self.first_step),
            ms(self.last_step),
            u8::from(self.concurrent),
        )
    }
}

/// The open task, if any. Separate from the global so tests drive it without
/// sharing state between threads.
#[derive(Debug, Default)]
pub struct Tracker {
    current: Option<TaskTimer>,
}

impl Tracker {
    /// Open a task, or join the one already open.
    ///
    /// A specialist the orchestrator delegated to joins: same id, depth one
    /// deeper, its steps counted against the task the person is waiting for. A
    /// run from a *different* session is a concurrent task rather than a nested
    /// run, so it joins but marks the task `concurrent`. `next_id` is only
    /// called when a task actually opens.
    pub fn begin(
        &mut self,
        next_id: impl FnOnce() -> u64,
        at: Instant,
        session: Option<String>,
    ) -> u64 {
        if let Some(open) = self.current.as_mut() {
            if let (Some(owner), Some(joining)) = (open.owner.as_deref(), session.as_deref()) {
                if owner != joining {
                    open.concurrent = true;
                }
            }
            open.depth = open.depth.saturating_add(1);
            return open.id();
        }
        let id = next_id();
        self.current = Some(TaskTimer::new(id, at, session));
        id
    }

    /// Close one run of `id`. Returns the step line and the task line once the
    /// outermost run ends; a nested run returns neither.
    ///
    /// An `id` that is not the open task's is a stale finish (the task it
    /// belonged to is already logged) and does nothing.
    pub fn finish(&mut self, id: u64, at: Instant) -> (Option<String>, Option<String>) {
        let Some(open) = self.current.as_mut() else {
            return (None, None);
        };
        if open.id() != id {
            return (None, None);
        }
        open.depth = open.depth.saturating_sub(1);
        if open.depth > 0 {
            return (None, None);
        }
        let step = open.close_step();
        let task = open.line(at);
        self.current = None;
        (step, Some(task))
    }

    /// Open a step on the open task.
    pub fn begin_step(&mut self) -> Option<String> {
        self.current.as_mut()?.begin_step()
    }

    pub fn add_model(&mut self, elapsed: Duration) {
        if let Some(open) = self.current.as_mut() {
            open.add_model(elapsed);
        }
    }

    pub fn add_tools(&mut self, elapsed: Duration, calls: u32) {
        if let Some(open) = self.current.as_mut() {
            open.add_tools(elapsed, calls);
        }
    }

    pub fn add_screenshot(&mut self) {
        if let Some(open) = self.current.as_mut() {
            open.add_screenshot();
        }
    }

    pub fn note_outcome(&mut self, outcome: &str) {
        if let Some(open) = self.current.as_mut() {
            open.note_outcome(outcome);
        }
    }
}

static TRACKER: Mutex<Tracker> = Mutex::new(Tracker { current: None });
static NEXT_TASK: AtomicU64 = AtomicU64::new(1);

fn with_tracker<T>(f: impl FnOnce(&mut Tracker) -> T) -> T {
    // A panic elsewhere while holding this lock leaves plain data behind;
    // timing is not worth refusing to log over.
    let mut guard = TRACKER.lock().unwrap_or_else(|e| e.into_inner());
    f(&mut guard)
}

fn emit_step(line: Option<String>) {
    if let Some(line) = line {
        info!(target: TARGET, "[StepTiming] {}", line);
    }
}

/// Open a task, or join the one already open. Returns the id to hand back to
/// [`finish`].
pub fn begin(session: Option<String>) -> u64 {
    with_tracker(|t| {
        t.begin(
            || NEXT_TASK.fetch_add(1, Ordering::Relaxed),
            Instant::now(),
            session,
        )
    })
}

/// Close one run of `id`, logging the task once its outermost run ends.
pub fn finish(id: u64) {
    let (step, task) = with_tracker(|t| t.finish(id, Instant::now()));
    emit_step(step);
    if let Some(task) = task {
        info!(target: TARGET, "[TaskTiming] {}", task);
    }
}

/// Open a step, logging the one before it.
pub fn begin_step() {
    emit_step(with_tracker(|t| t.begin_step()));
}

/// One run of the agent loop, closed when it returns.
///
/// Held by [`AgentRunnable::run`](crate::agent::traits::AgentRunnable::run) for
/// the length of the run. The loop has six exits (finished, cancelled, three
/// flavours of running out of steps, and a tool error), and a task that is never
/// closed would swallow the next one into its totals. So the close is a `Drop`
/// rather than a line on each exit: it also covers `?` propagation and a panic,
/// and there is no seventh exit to forget.
pub struct TaskRun(u64);

impl TaskRun {
    /// Open a task for a run belonging to `session`, or join the task already
    /// open (a delegated specialist).
    pub fn begin(session: Option<String>) -> Self {
        TaskRun(begin(session))
    }
}

impl Drop for TaskRun {
    fn drop(&mut self) {
        finish(self.0);
    }
}

/// Time the open step spent waiting on the model.
pub fn add_model(elapsed: Duration) {
    with_tracker(|t| t.add_model(elapsed));
}

/// Time the open step spent running `calls` tools.
pub fn add_tools(elapsed: Duration, calls: u32) {
    with_tracker(|t| t.add_tools(elapsed, calls));
}

/// A tool result carried a screenshot.
pub fn add_screenshot() {
    with_tracker(|t| t.add_screenshot());
}

/// Record how the task ended.
pub fn note_outcome(outcome: &str) {
    with_tracker(|t| t.note_outcome(outcome));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A clock that only moves when a test moves it.
    fn at(base: Instant, ms: u64) -> Instant {
        base + Duration::from_millis(ms)
    }

    fn open(tracker: &mut Tracker, base: Instant) -> u64 {
        tracker.begin(|| 1, base, None)
    }

    #[test]
    fn a_task_reports_its_steps_and_the_split_between_model_and_tools() {
        let base = Instant::now();
        let mut t = Tracker::default();
        let id = open(&mut t, base);

        t.begin_step();
        t.add_model(Duration::from_millis(1000));
        t.add_tools(Duration::from_millis(200), 2);

        t.begin_step();
        t.add_model(Duration::from_millis(1500));
        t.add_tools(Duration::from_millis(300), 1);

        t.note_outcome("finished");
        let (_, task) = t.finish(id, at(base, 4000));
        let line = task.unwrap_or_default();

        assert!(line.contains("steps=2"), "{line}");
        assert!(line.contains("wall=4000"), "{line}");
        assert!(line.contains("model=2500"), "{line}");
        assert!(line.contains("tools=500"), "{line}");
        assert!(line.contains("tool_calls=3"), "{line}");
        assert!(line.contains("outcome=finished"), "{line}");
    }

    /// The whole point of splitting the two: what is left over is Juno's own
    /// cost, and it is the only part no benchmark reports.
    #[test]
    fn overhead_is_the_wall_clock_neither_the_model_nor_the_desktop_explains() {
        let base = Instant::now();
        let mut t = Tracker::default();
        let id = open(&mut t, base);
        t.begin_step();
        t.add_model(Duration::from_millis(600));
        t.add_tools(Duration::from_millis(400), 1);
        let (_, task) = t.finish(id, at(base, 2500));
        assert!(task.unwrap_or_default().contains("overhead=1500"));
    }

    /// Measured independently, so a rounding overlap must not underflow a
    /// u128 of milliseconds into a nonsense number.
    #[test]
    fn overhead_floors_at_zero_when_the_parts_exceed_the_wall_clock() {
        let base = Instant::now();
        let mut t = Tracker::default();
        let id = open(&mut t, base);
        t.begin_step();
        t.add_model(Duration::from_millis(900));
        t.add_tools(Duration::from_millis(900), 1);
        let (_, task) = t.finish(id, at(base, 1000));
        assert!(task.unwrap_or_default().contains("overhead=0"));
    }

    /// OSWorld-Human's finding is that the last step costs ~3x the first, so
    /// the line has to carry both ends rather than an average that hides it.
    #[test]
    fn the_first_and_last_step_are_reported_so_the_growth_curve_is_visible() {
        let base = Instant::now();
        let mut t = Tracker::default();
        let id = open(&mut t, base);

        t.begin_step();
        t.add_model(Duration::from_millis(500));

        t.begin_step();
        t.add_model(Duration::from_millis(900));

        t.begin_step();
        t.add_model(Duration::from_millis(1500));

        let (_, task) = t.finish(id, at(base, 3000));
        let line = task.unwrap_or_default();
        assert!(line.contains("first_step=500"), "{line}");
        assert!(line.contains("last_step=1500"), "{line}");
    }

    /// Stepping is the only hook the loop needs, so the step that was open when
    /// the task ended has to be folded in by the finish rather than dropped.
    #[test]
    fn the_last_step_is_counted_even_though_nothing_opened_a_step_after_it() {
        let base = Instant::now();
        let mut t = Tracker::default();
        let id = open(&mut t, base);
        t.begin_step();
        t.add_model(Duration::from_millis(700));
        t.add_tools(Duration::from_millis(100), 1);

        let (step, task) = t.finish(id, at(base, 800));
        let step = step.unwrap_or_default();
        assert!(step.contains("step=1"), "{step}");
        assert!(step.contains("model=700"), "{step}");

        let task = task.unwrap_or_default();
        assert!(task.contains("steps=1"), "{task}");
        assert!(task.contains("model=700"), "{task}");
        assert!(task.contains("tool_calls=1"), "{task}");
    }

    #[test]
    fn each_step_logs_its_own_line_as_the_next_one_opens() {
        let base = Instant::now();
        let mut t = Tracker::default();
        open(&mut t, base);

        assert_eq!(t.begin_step(), None, "nothing precedes the first step");

        t.add_model(Duration::from_millis(400));
        t.add_screenshot();
        let first = t.begin_step().unwrap_or_default();
        assert!(first.contains("step=1"), "{first}");
        assert!(first.contains("model=400"), "{first}");
        assert!(first.contains("screenshots=1"), "{first}");
    }

    /// A specialist is part of the task the person is waiting for, not a task of
    /// its own, so its steps count against the same line and only the outermost
    /// run logs it.
    #[test]
    fn a_delegated_run_folds_into_the_task_that_opened_it() {
        let base = Instant::now();
        let mut t = Tracker::default();
        let outer = t.begin(|| 1, base, Some("session-a".to_string()));

        t.begin_step();
        t.add_model(Duration::from_millis(300));

        let inner = t.begin(|| 99, at(base, 400), None);
        assert_eq!(inner, outer, "a nested run joins the open task");

        t.begin_step();
        t.add_model(Duration::from_millis(500));

        let (_, task) = t.finish(inner, at(base, 1000));
        assert!(task.is_none(), "the nested run must not log the task");

        let (_, task) = t.finish(outer, at(base, 1200));
        let line = task.unwrap_or_default();
        assert!(line.contains("steps=2"), "{line}");
        assert!(line.contains("model=800"), "{line}");
        assert!(line.contains("concurrent=0"), "{line}");
    }

    /// Two parallel sessions are two tasks, and this tracker holds one. The
    /// totals are then a sum of both, so the line says so instead of passing
    /// for a baseline.
    #[test]
    fn a_second_session_marks_the_task_as_not_a_clean_measurement() {
        let base = Instant::now();
        let mut t = Tracker::default();
        let first = t.begin(|| 1, base, Some("session-a".to_string()));
        t.begin(|| 2, at(base, 10), Some("session-b".to_string()));

        t.begin_step();
        let (_, task) = t.finish(first, at(base, 50));
        // Still depth 1 from session-b's begin, so nothing is logged yet.
        assert!(task.is_none(), "the second session's run is still open");

        let (_, task) = t.finish(first, at(base, 100));
        assert!(task.unwrap_or_default().contains("concurrent=1"));
    }

    /// A specialist that failed inside a task the orchestrator recovered from
    /// did not fail the task.
    #[test]
    fn the_last_outcome_wins_so_a_recovered_failure_is_not_the_task_outcome() {
        let base = Instant::now();
        let mut t = Tracker::default();
        let id = open(&mut t, base);
        t.begin_step();
        t.note_outcome("failed");
        t.note_outcome("finished");
        let (_, task) = t.finish(id, at(base, 100));
        assert!(task.unwrap_or_default().contains("outcome=finished"));
    }

    #[test]
    fn a_task_that_never_reached_a_terminal_state_says_so_rather_than_guessing() {
        let base = Instant::now();
        let mut t = Tracker::default();
        let id = open(&mut t, base);
        t.begin_step();
        let (_, task) = t.finish(id, at(base, 100));
        assert!(task.unwrap_or_default().contains("outcome=-"));
    }

    /// A finish for a task already logged must not reach into the next one.
    #[test]
    fn a_stale_finish_leaves_the_task_after_it_alone() {
        let base = Instant::now();
        let mut t = Tracker::default();
        let first = t.begin(|| 1, base, None);
        t.begin_step();
        t.finish(first, at(base, 100));

        let second = t.begin(|| 2, at(base, 200), None);
        t.begin_step();
        t.add_model(Duration::from_millis(50));

        let (_, task) = t.finish(first, at(base, 300));
        assert!(task.is_none(), "a stale id must not close the open task");

        let (_, task) = t.finish(second, at(base, 400));
        assert!(task.unwrap_or_default().contains("task=2"));
    }

    /// Marks outside a task are ignored rather than panicking: the agent loop
    /// is not the only caller of the paths these hook into.
    #[test]
    fn recording_outside_a_task_is_a_no_op() {
        let mut t = Tracker::default();
        assert_eq!(t.begin_step(), None);
        t.add_model(Duration::from_millis(100));
        t.add_tools(Duration::from_millis(100), 1);
        t.add_screenshot();
        t.note_outcome("finished");
        assert!(t.current.is_none());
    }
}
