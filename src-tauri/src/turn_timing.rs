//! Where the time goes in one voice turn.
//!
//! The goal this exists for: the person lets go of the hold key and hears Juno
//! start answering in under a second. Nobody can work toward that without
//! knowing which stage eats the time, so every voice turn logs one line with
//! millisecond offsets from the key release:
//!
//! ```text
//! [TurnTiming] turn=7 outcome=first_audio released=0 transcript_final=412 llm_request_sent=455 llm_first_token=1210 first_tts_text=1388 first_audio=1702 provider=claude_cli/persistent-warm model=sonnet tts=kokoro
//! ```
//!
//! A stage the turn never reached prints as `-`. Grep `[TurnTiming]` in the log.
//!
//! The tracker is deliberately passive. A turn opens on a hold-key release
//! ([`begin`]), each stage records the first time it is reached ([`mark`]), and
//! the line goes out once: when audio starts, when the turn ends without any
//! ([`finish`]), or when the next release supersedes it. Marks with no open turn
//! (typed queries, previews) are ignored, so nothing outside a voice turn pays
//! more than one uncontended lock. Nothing here changes what Juno does.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use tracing::info;

/// Log target, so the line can be filtered on its own with `RUST_LOG`.
pub const TARGET: &str = "juno::turn_timing";

/// How long a turn stays open waiting for its audio before it is logged
/// incomplete. Long enough for a tool-heavy turn to finish speaking.
pub const TURN_TIMEOUT: Duration = Duration::from_secs(120);

/// The stages of a voice turn after the key release, in the order they
/// normally happen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    /// Speech-to-text has produced the final transcript.
    TranscriptFinal,
    /// The first request to the model left Juno (HTTP send, CLI spawn, or a
    /// message written to a live CLI process).
    LlmRequestSent,
    /// The first streamed content came back from the model.
    LlmFirstToken,
    /// The first text was handed to text-to-speech.
    FirstTtsText,
    /// Audio playback actually started.
    FirstAudio,
}

impl Stage {
    pub const ALL: [Stage; 5] = [
        Stage::TranscriptFinal,
        Stage::LlmRequestSent,
        Stage::LlmFirstToken,
        Stage::FirstTtsText,
        Stage::FirstAudio,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Stage::TranscriptFinal => "transcript_final",
            Stage::LlmRequestSent => "llm_request_sent",
            Stage::LlmFirstToken => "llm_first_token",
            Stage::FirstTtsText => "first_tts_text",
            Stage::FirstAudio => "first_audio",
        }
    }

    fn index(self) -> usize {
        match self {
            Stage::TranscriptFinal => 0,
            Stage::LlmRequestSent => 1,
            Stage::LlmFirstToken => 2,
            Stage::FirstTtsText => 3,
            Stage::FirstAudio => 4,
        }
    }
}

/// One turn's stage offsets. Pure data, so it can be tested without a clock.
#[derive(Debug)]
pub struct TurnTimer {
    id: u64,
    released: Instant,
    marks: [Option<Duration>; 5],
    provider: Option<String>,
    model: Option<String>,
    tts_engine: Option<String>,
}

impl TurnTimer {
    pub fn new(id: u64, released: Instant) -> Self {
        Self {
            id,
            released,
            marks: [None; 5],
            provider: None,
            model: None,
            tts_engine: None,
        }
    }

    pub fn id(&self) -> u64 {
        self.id
    }

    /// Record a stage at `at`. The first time wins: a tool loop sends several
    /// requests and speaks several blocks, and only the first of each is on
    /// the path to the first sound. Returns whether this call recorded it.
    pub fn mark_at(&mut self, stage: Stage, at: Instant) -> bool {
        let slot = &mut self.marks[stage.index()];
        if slot.is_some() {
            return false;
        }
        *slot = Some(at.saturating_duration_since(self.released));
        true
    }

    pub fn offset(&self, stage: Stage) -> Option<Duration> {
        self.marks[stage.index()]
    }

    /// Which model answered. First wins, like the stages.
    pub fn note_llm(&mut self, provider: &str, model: &str) {
        if self.provider.is_none() {
            self.provider = Some(provider.to_string());
            self.model = Some(model.to_string());
        }
    }

    /// Which engine spoke. First wins.
    pub fn note_tts_engine(&mut self, engine: &str) {
        if self.tts_engine.is_none() {
            self.tts_engine = Some(engine.to_string());
        }
    }

    /// The log line, minus the `[TurnTiming]` prefix.
    pub fn line(&self, outcome: &str) -> String {
        let mut out = format!("turn={} outcome={} released=0", self.id, outcome);
        for stage in Stage::ALL {
            match self.offset(stage) {
                Some(d) => out.push_str(&format!(" {}={}", stage.label(), d.as_millis())),
                None => out.push_str(&format!(" {}=-", stage.label())),
            }
        }
        let field = |v: &Option<String>| v.clone().unwrap_or_else(|| "-".to_string());
        out.push_str(&format!(
            " provider={} model={} tts={}",
            field(&self.provider),
            field(&self.model),
            field(&self.tts_engine)
        ));
        out
    }
}

/// The open turn, if any, and the rules for when it is logged. Separate from
/// the global so tests can drive it without sharing state between threads.
#[derive(Debug, Default)]
pub struct Tracker {
    current: Option<TurnTimer>,
}

impl Tracker {
    /// Open a turn. A turn still open is closed out as `superseded` and its
    /// line returned, so it is never silently lost.
    pub fn begin(&mut self, id: u64, released: Instant) -> Option<String> {
        let previous = self.current.take().map(|t| t.line("superseded"));
        self.current = Some(TurnTimer::new(id, released));
        previous
    }

    /// Record a stage. Returns the finished line when the stage is first
    /// audio, the end of the path this tracker measures.
    pub fn mark_at(&mut self, stage: Stage, at: Instant) -> Option<String> {
        let timer = self.current.as_mut()?;
        let recorded = timer.mark_at(stage, at);
        if recorded && stage == Stage::FirstAudio {
            return self.current.take().map(|t| t.line("first_audio"));
        }
        None
    }

    pub fn note_llm(&mut self, provider: &str, model: &str) {
        if let Some(timer) = self.current.as_mut() {
            timer.note_llm(provider, model);
        }
    }

    pub fn note_tts_engine(&mut self, engine: &str) {
        if let Some(timer) = self.current.as_mut() {
            timer.note_tts_engine(engine);
        }
    }

    /// Close the turn without audio. With `id`, only that turn: a timeout
    /// armed for an old turn must not close the one after it.
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
}

static TRACKER: Mutex<Tracker> = Mutex::new(Tracker { current: None });
static NEXT_TURN: AtomicU64 = AtomicU64::new(1);

fn with_tracker<T>(f: impl FnOnce(&mut Tracker) -> T) -> T {
    // A panic elsewhere while holding this lock leaves plain data behind;
    // timing is not worth refusing to log over.
    let mut guard = TRACKER.lock().unwrap_or_else(|e| e.into_inner());
    f(&mut guard)
}

fn emit(line: Option<String>) {
    if let Some(line) = line {
        info!(target: TARGET, "[TurnTiming] {}", line);
    }
}

/// Open a voice turn at the moment the hold key was released. Returns the
/// turn id, for [`finish`].
pub fn begin(released: Instant) -> u64 {
    let id = NEXT_TURN.fetch_add(1, Ordering::Relaxed);
    let previous = with_tracker(|t| t.begin(id, released));
    emit(previous);
    id
}

/// Record that the open turn reached `stage` now. A no-op with no open turn.
pub fn mark(stage: Stage) {
    let now = Instant::now();
    let line = with_tracker(|t| t.mark_at(stage, now));
    emit(line);
}

/// Record which provider and model the open turn went to.
pub fn note_llm(provider: &str, model: &str) {
    with_tracker(|t| t.note_llm(provider, model));
}

/// Record which TTS engine the open turn spoke with.
pub fn note_tts_engine(engine: &str) {
    with_tracker(|t| t.note_tts_engine(engine));
}

/// Close the open turn without audio, logging what it reached. With `id`,
/// only if that turn is still the open one.
pub fn finish(id: Option<u64>, outcome: &str) {
    let line = with_tracker(|t| t.finish(id, outcome));
    emit(line);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    #[test]
    fn a_full_turn_logs_every_stage_once_on_first_audio() {
        let t0 = Instant::now();
        let mut tracker = Tracker::default();
        assert!(tracker.begin(1, t0).is_none());
        tracker.note_llm("claude_cli/persistent-warm", "sonnet");
        tracker.note_tts_engine("kokoro");
        assert!(tracker
            .mark_at(Stage::TranscriptFinal, t0 + ms(400))
            .is_none());
        assert!(tracker
            .mark_at(Stage::LlmRequestSent, t0 + ms(450))
            .is_none());
        assert!(tracker
            .mark_at(Stage::LlmFirstToken, t0 + ms(1200))
            .is_none());
        assert!(tracker
            .mark_at(Stage::FirstTtsText, t0 + ms(1380))
            .is_none());
        let line = tracker
            .mark_at(Stage::FirstAudio, t0 + ms(1700))
            .expect("first audio closes the turn");
        assert_eq!(
            line,
            "turn=1 outcome=first_audio released=0 transcript_final=400 llm_request_sent=450 \
             llm_first_token=1200 first_tts_text=1380 first_audio=1700 \
             provider=claude_cli/persistent-warm model=sonnet tts=kokoro"
        );
        // Emitted once: the turn is gone, later marks do nothing.
        assert!(tracker.mark_at(Stage::FirstAudio, t0 + ms(2000)).is_none());
        assert!(tracker.finish(None, "late").is_none());
    }

    #[test]
    fn the_first_time_a_stage_is_reached_wins() {
        let t0 = Instant::now();
        let mut timer = TurnTimer::new(3, t0);
        assert!(timer.mark_at(Stage::LlmRequestSent, t0 + ms(100)));
        // A tool loop's second request is not on the path to first sound.
        assert!(!timer.mark_at(Stage::LlmRequestSent, t0 + ms(900)));
        assert_eq!(timer.offset(Stage::LlmRequestSent), Some(ms(100)));
        timer.note_llm("anthropic", "a");
        timer.note_llm("claude_cli/oneshot", "b");
        assert!(timer.line("x").contains("provider=anthropic model=a"));
    }

    #[test]
    fn missing_stages_print_as_absent() {
        let t0 = Instant::now();
        let mut tracker = Tracker::default();
        tracker.begin(2, t0);
        tracker.mark_at(Stage::TranscriptFinal, t0 + ms(300));
        let line = tracker
            .finish(Some(2), "timeout")
            .expect("open turn closes");
        assert_eq!(
            line,
            "turn=2 outcome=timeout released=0 transcript_final=300 llm_request_sent=- \
             llm_first_token=- first_tts_text=- first_audio=- provider=- model=- tts=-"
        );
    }

    #[test]
    fn marks_without_an_open_turn_are_ignored() {
        let mut tracker = Tracker::default();
        let now = Instant::now();
        assert!(tracker.mark_at(Stage::FirstAudio, now).is_none());
        tracker.note_llm("anthropic", "m");
        tracker.note_tts_engine("system");
        assert!(tracker.finish(None, "x").is_none());
    }

    #[test]
    fn a_new_release_closes_the_turn_it_supersedes() {
        let t0 = Instant::now();
        let mut tracker = Tracker::default();
        tracker.begin(5, t0);
        tracker.mark_at(Stage::TranscriptFinal, t0 + ms(250));
        let previous = tracker.begin(6, t0 + ms(5000)).expect("old turn logged");
        assert!(previous.starts_with("turn=5 outcome=superseded"));
        assert!(previous.contains("transcript_final=250"));
        // The new turn measures from its own release.
        tracker.mark_at(Stage::TranscriptFinal, t0 + ms(5100));
        let line = tracker.finish(None, "done").expect("new turn open");
        assert!(line.starts_with("turn=6 "));
        assert!(line.contains("transcript_final=100"));
    }

    #[test]
    fn a_stale_timeout_does_not_close_a_newer_turn() {
        let t0 = Instant::now();
        let mut tracker = Tracker::default();
        tracker.begin(8, t0);
        tracker.begin(9, t0);
        assert!(tracker.finish(Some(8), "timeout").is_none());
        assert!(tracker.finish(Some(9), "timeout").is_some());
    }

    #[test]
    fn a_mark_before_the_release_clamps_to_zero() {
        let t0 = Instant::now();
        let mut timer = TurnTimer::new(1, t0 + ms(50));
        timer.mark_at(Stage::TranscriptFinal, t0);
        assert_eq!(timer.offset(Stage::TranscriptFinal), Some(Duration::ZERO));
    }
}
