//! Where the time goes between the key going down and the microphone's first
//! sample.
//!
//! The goal this exists for: a person who presses the dictation key and starts
//! talking immediately keeps their first word. Every sample the device has not
//! delivered yet is speech the decoder never sees, so the whole of this window
//! is audio thrown away, and one press logs one line with millisecond offsets
//! from the press:
//!
//! ```text
//! [CaptureTiming] capture=4 outcome=first_sample press=0 start_cmd=3 mic_ready=3 engine_ready=3 locked=4 preferred=4 claimed=9 thread=9 opening=10 live=42 first_sample=53 warm=yes rate=48000 device="MacBook Air Microphone"
//! ```
//!
//! A stage the press never reached prints as `-`. Grep `[CaptureTiming]` in the
//! log.
//!
//! This is [`crate`]'s mirror of the app's `turn_timing`, which measures the
//! other half: that one starts at the key *release* and ends at the first
//! spoken audio, this one starts at the key *press* and ends at the first
//! recorded sample. Together they cover a whole voice turn.
//!
//! The critical section is small but not free: the line is formatted under the
//! lock, which is about a dozen allocations and no syscall. Bounded and
//! microseconds, so it is not a dropout risk, but it is more than a field
//! write and worth knowing before anything else is added to it.
//!
//! Reading the gaps matters more than the total, because each one has a
//! different owner:
//!
//! | Gap | Owner |
//! |---|---|
//! | `press` to `start_cmd` | the hold-edge queue and the event hop out of the recognizer |
//! | `start_cmd` to `engine_ready` | permission check, then waiting for a speech engine still loading |
//! | `engine_ready` to `locked` | another dictation still starting or stopping |
//! | `locked` to `preferred` | reading which microphone the person chose (in memory) |
//! | `preferred` to `claimed` | the first CoreAudio call: claiming the warm stream, and the staleness check on it |
//! | `claimed` to `opening` | spawning the audio thread and getting it scheduled |
//! | `opening` to `live` | CoreAudio starting the device |
//! | `live` to `first_sample` | the device's first buffer period |
//!
//! Measured on an M1 MacBook Air (2026-10-28, LAC-5281), the last two gaps
//! together are 40-58 ms with the stream already built and stopped, while the
//! running app took 87-254 ms (median ~125) to reach `live`. Attributed
//! carefully, because the obvious reading is wrong: that figure comes from the
//! one pre-existing log line on this path, which measures from `requested` at
//! the top of `VoiceController::start_dictation`. `Locked` is marked
//! immediately before the call that sets it, so the 87-254 ms is `locked` to
//! `live`, and press to `live` is at least that rather than exactly it.
//!
//! Reading the code accounts for only 40-65 ms of it: the permission check is
//! a cached atomic, the device preference an in-memory read, `claim_warm_input`
//! atomics plus at worst two HAL queries, and `WarmInput::resume` a sink-mutex
//! write plus the device start, with no stream rebuild. So the residual is in a
//! stage that costs more inside Juno than the same call does measured alone,
//! which a standalone probe cannot show: it opens one CoreAudio client, and
//! Juno runs several in one process. The line exists to say which gap holds
//! that difference, because nothing recorded it before: the only timing log on
//! this path fired when the device start returned, so Juno never saw when
//! audio actually began.
//!
//! The tracker is passive. A press opens a capture ([`press`]), each stage
//! records the first time it is reached ([`mark_for`]), and the line goes out
//! once: when the first sample lands, when the press ends without one
//! ([`finish_for`]), or when the next press supersedes it.
//!
//! **Every stage and every close is scoped to the capture it belongs to.**
//! A start carries its capture id as an `Option<u64>` from the press that
//! began it, threaded through the `TRANSCRIPTION_START` payload beside the
//! hold id. `None` means "this start did not come from a dictation key press"
//! and records nothing at all, so `toggle_dictation`, the agent paths and the
//! mode manager stay silent rather than writing their stages into a press's
//! line. `Some(id)` records only while that capture is still open.
//!
//! That scoping is load-bearing, not tidiness. `ENGINE_WAIT` is 15 s, so a
//! press made while the speech engine is still loading sits in
//! `wait_for_engine` long enough for several later presses to open captures of
//! their own; unscoped, its eventual `controller_busy` would close the live
//! one and the press that actually recorded would never log its own first
//! sample. A line reported under the wrong id is worse than no line, because
//! the next change spends these numbers (LAC-6118).
//!
//! Always-listening and the wake word do not share this path at all - they have
//! their own capture loop and call nothing here - so they pay nothing, not even
//! a lock.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use tracing::info;

/// Log target, so the line can be raised above `info` with `RUST_LOG`.
///
/// It cannot be *isolated* that way: `init_tracing` adds an unconditional
/// global `info` directive, so this target is always enabled at `info` no
/// matter what `RUST_LOG` selects. Grep `[CaptureTiming]`, which is in the
/// message body rather than the target, so `with_target(false)` does not hide
/// it either.
pub const TARGET: &str = "juno::capture_timing";

/// The stages between the press and the first sample, in the order they
/// normally happen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    /// The plugin's `start_dictation` command began running.
    StartCmd,
    /// Microphone access was confirmed.
    MicReady,
    /// The speech engine was loaded and ready to take a session.
    EngineReady,
    /// The controller lock was taken, so no other recording was in the way.
    Locked,
    /// The microphone choice was read, so the next thing to happen is the
    /// first call that talks to CoreAudio. Split out from [`Stage::Claimed`]
    /// deliberately: that gap used to fuse an in-memory read with a HAL
    /// round trip, and the HAL is the suspect, so fusing them would localise
    /// the cost to a span and still not name the call (LAC-6118).
    Preferred,
    /// A microphone was settled on: the warm stream claimed, or a device
    /// resolved and configured for this press to open itself.
    Claimed,
    /// The audio thread is running.
    Thread,
    /// About to ask the device to start.
    Opening,
    /// The device accepted the start. Audio is coming but none has arrived.
    Live,
    /// The first buffer of audio reached the audio thread. The first moment
    /// anything the person said could be recorded.
    FirstSample,
}

impl Stage {
    pub const ALL: [Stage; 10] = [
        Stage::StartCmd,
        Stage::MicReady,
        Stage::EngineReady,
        Stage::Locked,
        Stage::Preferred,
        Stage::Claimed,
        Stage::Thread,
        Stage::Opening,
        Stage::Live,
        Stage::FirstSample,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Stage::StartCmd => "start_cmd",
            Stage::MicReady => "mic_ready",
            Stage::EngineReady => "engine_ready",
            Stage::Locked => "locked",
            Stage::Preferred => "preferred",
            Stage::Claimed => "claimed",
            Stage::Thread => "thread",
            Stage::Opening => "opening",
            Stage::Live => "live",
            Stage::FirstSample => "first_sample",
        }
    }

    fn index(self) -> usize {
        match self {
            Stage::StartCmd => 0,
            Stage::MicReady => 1,
            Stage::EngineReady => 2,
            Stage::Locked => 3,
            Stage::Preferred => 4,
            Stage::Claimed => 5,
            Stage::Thread => 6,
            Stage::Opening => 7,
            Stage::Live => 8,
            Stage::FirstSample => 9,
        }
    }
}

/// One press's stage offsets. Pure data, so it can be tested without a clock.
#[derive(Debug)]
pub struct CaptureTimer {
    id: u64,
    pressed: Instant,
    marks: [Option<Duration>; Stage::ALL.len()],
    /// Whether the press opened the stream waiting for it, rather than opening
    /// a device itself. Unknown until the microphone is settled on.
    warm: Option<bool>,
    device: Option<String>,
    rate: Option<u32>,
}

impl CaptureTimer {
    pub fn new(id: u64, pressed: Instant) -> Self {
        Self {
            id,
            pressed,
            marks: [None; Stage::ALL.len()],
            warm: None,
            device: None,
            rate: None,
        }
    }

    pub fn id(&self) -> u64 {
        self.id
    }

    /// Record a stage at `at`. The first time wins, so a retry that opens the
    /// device a second time does not overwrite when it first started.
    /// Returns whether this call recorded it.
    pub fn mark_at(&mut self, stage: Stage, at: Instant) -> bool {
        let slot = &mut self.marks[stage.index()];
        if slot.is_some() {
            return false;
        }
        *slot = Some(at.saturating_duration_since(self.pressed));
        true
    }

    pub fn offset(&self, stage: Stage) -> Option<Duration> {
        self.marks[stage.index()]
    }

    /// Which microphone this press settled on, and whether it was already
    /// built and waiting. First wins, like the stages.
    pub fn note_device(&mut self, device: &str, rate: u32, warm: bool) {
        if self.warm.is_none() {
            self.warm = Some(warm);
            self.device = Some(device.to_string());
            self.rate = Some(rate);
        }
    }

    /// The log line, minus the `[CaptureTiming]` prefix.
    pub fn line(&self, outcome: &str) -> String {
        let mut out = format!("capture={} outcome={} press=0", self.id, outcome);
        for stage in Stage::ALL {
            match self.offset(stage) {
                Some(d) => out.push_str(&format!(" {}={}", stage.label(), d.as_millis())),
                None => out.push_str(&format!(" {}=-", stage.label())),
            }
        }
        out.push_str(&format!(
            " warm={} rate={} device={:?}",
            match self.warm {
                Some(true) => "yes",
                Some(false) => "no",
                None => "-",
            },
            self.rate
                .map(|r| r.to_string())
                .unwrap_or_else(|| "-".to_string()),
            self.device.as_deref().unwrap_or("-"),
        ));
        out
    }
}

/// The open capture, if any. Separate from the global so tests can drive it
/// without sharing state between threads.
#[derive(Debug, Default)]
pub struct Tracker {
    current: Option<CaptureTimer>,
}

impl Tracker {
    /// Open a capture for a press at `pressed`. A capture still open is closed
    /// out as `superseded` and its line returned, so a press whose microphone
    /// never opened is never silently lost.
    pub fn press(
        &mut self,
        next_id: impl FnOnce() -> u64,
        pressed: Instant,
    ) -> (u64, Option<String>) {
        let previous = self.current.take().map(|t| t.line("superseded"));
        let id = next_id();
        self.current = Some(CaptureTimer::new(id, pressed));
        (id, previous)
    }

    /// Record a stage. Returns the finished line once the first sample lands,
    /// which is the end of the path this tracker measures.
    pub fn mark_at(&mut self, stage: Stage, at: Instant) -> Option<String> {
        let timer = self.current.as_mut()?;
        let recorded = timer.mark_at(stage, at);
        if recorded && stage == Stage::FirstSample {
            return self.current.take().map(|t| t.line("first_sample"));
        }
        None
    }

    /// Record a stage, but only on the capture `id` names. A start still in
    /// flight from an earlier press reaches here after a later press has
    /// opened its own capture; without the id its stages would be written
    /// into that capture and reported under its number.
    pub fn mark_at_for(&mut self, id: u64, stage: Stage, at: Instant) -> Option<String> {
        if self.current.as_ref()?.id() != id {
            return None;
        }
        self.mark_at(stage, at)
    }

    pub fn note_device(&mut self, device: &str, rate: u32, warm: bool) {
        if let Some(timer) = self.current.as_mut() {
            timer.note_device(device, rate, warm);
        }
    }

    /// Which microphone, but only on the capture `id` names.
    pub fn note_device_for(&mut self, id: u64, device: &str, rate: u32, warm: bool) {
        if self.current.as_ref().is_some_and(|t| t.id() == id) {
            self.note_device(device, rate, warm);
        }
    }

    /// Close the capture without a first sample: a tap that cancelled before
    /// the device opened, or a device that would not start. With `id`, only
    /// that capture, so a late report cannot close the press after it.
    pub fn finish(&mut self, id: Option<u64>, outcome: &str) -> Option<String> {
        let matches = match (self.current.as_ref(), id) {
            (Some(timer), Some(id)) => timer.id() == id,
            (Some(_), None) => true,
            (None, _) => false,
        };
        if !matches {
            return None;
        }
        self.current.take().map(|t| t.line(outcome))
    }

    /// The open capture's id, for [`Tracker::finish`].
    pub fn open_id(&self) -> Option<u64> {
        self.current.as_ref().map(CaptureTimer::id)
    }

    /// A stage off the open capture. Test-only: production code never reads a
    /// stage back, it only reports them.
    #[cfg(test)]
    fn offset_of_open(&self, stage: Stage) -> Option<Duration> {
        self.current.as_ref()?.offset(stage)
    }
}

static TRACKER: Mutex<Tracker> = Mutex::new(Tracker { current: None });
static NEXT_CAPTURE: AtomicU64 = AtomicU64::new(1);

fn with_tracker<T>(f: impl FnOnce(&mut Tracker) -> T) -> T {
    // A panic elsewhere while holding this lock leaves plain data behind;
    // timing is not worth refusing to log over.
    let mut guard = TRACKER.lock().unwrap_or_else(|e| e.into_inner());
    f(&mut guard)
}

fn emit(line: Option<String>) {
    if let Some(line) = line {
        info!(target: TARGET, "[CaptureTiming] {}", line);
    }
}

/// Open a capture for the dictation key going down now. Returns its id, for
/// [`finish`].
pub fn press() -> u64 {
    let (id, previous) = with_tracker(|t| {
        t.press(
            || NEXT_CAPTURE.fetch_add(1, Ordering::Relaxed),
            Instant::now(),
        )
    });
    emit(previous);
    id
}

/// Record that the capture `capture` names reached `stage` now.
///
/// `None` records nothing. That is what every start Juno did not begin from a
/// dictation key press passes - `toggle_dictation`, the agent paths, the mode
/// manager, tests - so none of them can write a stage into a press's line.
/// `Some(id)` records only while that capture is still the open one, so a
/// start left behind by an earlier press cannot report under a later one.
pub fn mark_for(capture: Option<u64>, stage: Stage) {
    let Some(id) = capture else { return };
    emit(with_tracker(|t| t.mark_at_for(id, stage, Instant::now())));
}

/// Which microphone the capture `capture` names settled on. `None` records
/// nothing, like [`mark_for`].
pub fn note_device_for(capture: Option<u64>, device: &str, rate: u32, warm: bool) {
    let Some(id) = capture else { return };
    with_tracker(|t| t.note_device_for(id, device, rate, warm));
}

/// Close the capture `capture` names without a first sample, saying why.
/// `None` closes nothing, like [`mark_for`]: a start that was never bound to a
/// press must not be able to end one.
pub fn finish_for(capture: Option<u64>, outcome: &str) {
    let Some(id) = capture else { return };
    emit(with_tracker(|t| t.finish(Some(id), outcome)));
}

/// Close whatever capture is open, whichever press it belongs to.
///
/// One caller, and it wants exactly this: an agent hold opens a microphone of
/// its own, so a dictation press still waiting for one is over however far it
/// had got. Everywhere else, use [`finish_for`].
pub fn finish_any(outcome: &str) {
    emit(with_tracker(|t| t.finish(None, outcome)));
}

/// The open capture's id, if a press is still waiting for its microphone.
pub fn open_id() -> Option<u64> {
    with_tracker(|t| t.open_id())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(base: Instant, ms: u64) -> Instant {
        base + Duration::from_millis(ms)
    }

    #[test]
    fn the_line_reports_every_stage_and_dashes_the_rest() {
        let base = Instant::now();
        let mut timer = CaptureTimer::new(4, base);
        timer.mark_at(Stage::StartCmd, at(base, 3));
        timer.mark_at(Stage::Opening, at(base, 10));
        timer.mark_at(Stage::Live, at(base, 42));
        timer.note_device("MacBook Air Microphone", 48_000, true);

        let line = timer.line("abandoned");
        assert!(line.starts_with("capture=4 outcome=abandoned press=0"));
        assert!(line.contains(" start_cmd=3"));
        assert!(line.contains(" opening=10"));
        assert!(line.contains(" live=42"));
        assert!(
            line.contains(" first_sample=-"),
            "a stage never reached prints as a dash: {line}"
        );
        assert!(line.contains(" mic_ready=-"));
        assert!(line.contains(" warm=yes rate=48000 device=\"MacBook Air Microphone\""));
    }

    #[test]
    fn the_first_report_of_a_stage_wins() {
        let base = Instant::now();
        let mut timer = CaptureTimer::new(1, base);
        assert!(timer.mark_at(Stage::Live, at(base, 40)));
        assert!(
            !timer.mark_at(Stage::Live, at(base, 900)),
            "a device opened twice keeps when it first opened"
        );
        assert_eq!(timer.offset(Stage::Live), Some(Duration::from_millis(40)));
    }

    #[test]
    fn a_stage_before_the_press_reads_as_zero_rather_than_panicking() {
        // Clocks read on two threads can disagree by a hair, so a stage can
        // report a moment just before the press. An offset is never negative
        // and must never panic.
        let base = Instant::now();
        let mut timer = CaptureTimer::new(1, at(base, 10));
        timer.mark_at(Stage::StartCmd, base);
        assert_eq!(timer.offset(Stage::StartCmd), Some(Duration::ZERO));
    }

    #[test]
    fn the_first_sample_closes_the_capture_and_logs_it_once() {
        let base = Instant::now();
        let mut tracker = Tracker::default();
        let (id, previous) = tracker.press(|| 7, base);
        assert_eq!(id, 7);
        assert!(previous.is_none(), "nothing was open to supersede");

        assert!(tracker.mark_at(Stage::Live, at(base, 40)).is_none());
        let line = tracker
            .mark_at(Stage::FirstSample, at(base, 53))
            .expect("the first sample ends the capture");
        assert!(line.contains("outcome=first_sample"));
        assert!(line.contains(" first_sample=53"));

        assert!(
            tracker.mark_at(Stage::FirstSample, at(base, 60)).is_none(),
            "the capture is closed, so a second sample logs nothing"
        );
        assert!(tracker.open_id().is_none());
    }

    #[test]
    fn a_press_whose_microphone_never_opened_is_logged_by_the_next_press() {
        let base = Instant::now();
        let mut tracker = Tracker::default();
        tracker.press(|| 1, base);
        tracker.mark_at(Stage::StartCmd, at(base, 2));

        let (id, previous) = tracker.press(|| 2, at(base, 500));
        assert_eq!(id, 2);
        let previous = previous.expect("the abandoned press is not lost");
        assert!(previous.contains("capture=1 outcome=superseded"));
        assert!(previous.contains(" live=-"));
    }

    #[test]
    fn finish_only_closes_the_capture_it_names() {
        let base = Instant::now();
        let mut tracker = Tracker::default();
        tracker.press(|| 1, base);
        tracker.press(|| 2, at(base, 10));

        assert!(
            tracker.finish(Some(1), "cancelled").is_none(),
            "a late report for capture 1 must not close capture 2"
        );
        assert_eq!(tracker.open_id(), Some(2));

        let line = tracker.finish(Some(2), "cancelled").expect("its own id");
        assert!(line.contains("capture=2 outcome=cancelled"));
    }

    #[test]
    fn a_stalled_start_cannot_mark_or_close_a_later_press() {
        // The LAC-6118 regression. `ENGINE_WAIT` is 15 s, so a press made
        // while the speech engine loads sits in `wait_for_engine` long enough
        // for later presses to open captures of their own. When it finally
        // resumes, every stage it reports and every close it performs must
        // land on nothing, not on the press that is actually recording.
        let base = Instant::now();
        let mut tracker = Tracker::default();
        tracker.press(|| 1, base); // the press that stalls on the engine
        tracker.press(|| 2, at(base, 4_000)); // the press that records

        assert!(
            tracker
                .mark_at_for(1, Stage::Locked, at(base, 15_000))
                .is_none(),
            "the stalled start's stage must not be written into capture 2"
        );
        assert_eq!(
            tracker.offset_of_open(Stage::Locked),
            None,
            "capture 2 never reached `locked`, so it must not claim to have"
        );

        tracker.note_device_for(1, "Some Other Microphone", 44_100, false);

        assert!(
            tracker.finish(Some(1), "controller_busy").is_none(),
            "the stalled start must not close the press that is recording"
        );
        assert_eq!(tracker.open_id(), Some(2));

        // Capture 2 still owns its own line, device included.
        tracker.note_device_for(2, "MacBook Air Microphone", 48_000, true);
        let line = tracker
            .mark_at_for(2, Stage::FirstSample, at(base, 4_050))
            .expect("capture 2 closes on its own first sample");
        assert!(line.contains("capture=2 outcome=first_sample"));
        assert!(line.contains(" locked=-"), "no borrowed stage: {line}");
        assert!(line.contains(" device=\"MacBook Air Microphone\""));
    }

    #[test]
    fn the_preferred_split_is_reported_between_locked_and_claimed() {
        // `locked`->`claimed` used to fuse an in-memory read with the first
        // CoreAudio call. The HAL is the suspect, so the two are separable or
        // the measurement has to be taken twice.
        let base = Instant::now();
        let mut timer = CaptureTimer::new(1, base);
        timer.mark_at(Stage::Locked, at(base, 4));
        timer.mark_at(Stage::Preferred, at(base, 4));
        timer.mark_at(Stage::Claimed, at(base, 61));

        let line = timer.line("first_sample");
        let locked = line.find(" locked=").expect("locked is reported");
        let preferred = line.find(" preferred=").expect("preferred is reported");
        let claimed = line.find(" claimed=").expect("claimed is reported");
        assert!(
            locked < preferred && preferred < claimed,
            "stages read in the order they happen: {line}"
        );
    }

    #[test]
    fn marks_with_no_press_open_are_ignored() {
        let mut tracker = Tracker::default();
        assert!(tracker.mark_at(Stage::Live, Instant::now()).is_none());
        tracker.note_device("Some Microphone", 44_100, false);
        assert!(tracker.finish(None, "cancelled").is_none());
    }
}
