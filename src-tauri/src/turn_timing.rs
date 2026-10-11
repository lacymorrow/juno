//! Where the time goes in one turn, spoken or typed.
//!
//! The goal this exists for: the person lets go of the hold key and hears Juno
//! start answering in under a second. Nobody can work toward that without
//! knowing which stage eats the time, so every voice turn logs one line with
//! millisecond offsets from the key release:
//!
//! ```text
//! [TurnTiming] turn=7 outcome=first_audio released=0 transcript_final=412 llm_request_sent=455 llm_first_token=1210 first_tts_text=1388 first_audio=1702 provider=claude_cli/persistent-warm model=sonnet effort=medium tts=kokoro source=voice
//! ```
//!
//! A stage the turn never reached prints as `-`. Grep `[TurnTiming]` in the log.
//!
//! A typed query opens a turn too ([`begin_typed`]), timed from the submit, so
//! `released=0` is the moment it was sent and `transcript_final` is `-`.
//! `source=` tells the two apart. `effort=` is the Claude CLI effort the turn
//! ran at (`-` for other providers), so voice and typed effort can be compared.
//!
//! The tracker is deliberately passive. A turn opens on a hold-key release
//! ([`begin`]), each stage records the first time it is reached ([`mark`]), and
//! the line goes out once: when audio starts, when the turn ends without any
//! ([`finish`]), or when the next release supersedes it. Marks with no open turn
//! (typed queries, previews) are ignored, so nothing outside a voice turn pays
//! more than one uncontended lock. Nothing here changes what Juno does, with
//! one read-only exception: [`open_turn_is_voice`] is the only place that knows
//! a query was spoken, and the Claude CLI provider asks it to pick the effort.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use tracing::info;

/// Log target, so the line can be filtered on its own with `RUST_LOG`.
pub const TARGET: &str = "juno::turn_timing";

/// How long a turn stays open waiting for its audio before it is logged
/// incomplete. Long enough for a tool-heavy turn to finish speaking.
pub const TURN_TIMEOUT: Duration = Duration::from_secs(120);

/// A second release this soon after the open turn's, before that turn reached
/// any stage, is the same key release reported twice, not a new turn.
pub const DUPLICATE_RELEASE_WINDOW: Duration = Duration::from_millis(50);

const SOURCE_VOICE: &str = "voice";
const SOURCE_TYPED: &str = "typed";

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
    /// The voice session whose release opened this turn, when known.
    voice_session: Option<u64>,
    released: Instant,
    marks: [Option<Duration>; 5],
    provider: Option<String>,
    model: Option<String>,
    /// The Claude CLI `--effort` the turn ran at.
    effort: Option<String>,
    tts_engine: Option<String>,
    /// `voice` for a hold-key release, `typed` for a query sent from the composer.
    source: &'static str,
}

impl TurnTimer {
    pub fn new(id: u64, released: Instant) -> Self {
        Self {
            id,
            voice_session: None,
            released,
            marks: [None; 5],
            provider: None,
            model: None,
            effort: None,
            tts_engine: None,
            source: SOURCE_VOICE,
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

    /// Whether a release at `released` for `voice_session` is this turn's own
    /// release reported a second time.
    ///
    /// Same voice session: one session is one release, however many paths
    /// report it. No session to compare: a release within
    /// [`DUPLICATE_RELEASE_WINDOW`] of this one, before anything has happened,
    /// is the same press.
    fn is_same_release(&self, released: Instant, voice_session: Option<u64>) -> bool {
        if let (Some(open), Some(new)) = (self.voice_session, voice_session) {
            return open == new;
        }
        let untouched = self.marks.iter().all(Option::is_none);
        untouched && released.saturating_duration_since(self.released) < DUPLICATE_RELEASE_WINDOW
    }

    /// Which model answered. First wins, like the stages.
    pub fn note_llm(&mut self, provider: &str, model: &str) {
        if self.provider.is_none() {
            self.provider = Some(provider.to_string());
            self.model = Some(model.to_string());
        }
    }

    /// Which effort the model ran at. First wins.
    pub fn note_effort(&mut self, effort: &str) {
        if self.effort.is_none() {
            self.effort = Some(effort.to_string());
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
            " provider={} model={} effort={} tts={} source={}",
            field(&self.provider),
            field(&self.model),
            field(&self.effort),
            field(&self.tts_engine),
            self.source
        ));
        out
    }
}

/// What [`Tracker::begin`] did with a release.
#[derive(Debug, PartialEq, Eq)]
pub enum Begin {
    /// A new turn, `id`, is open. Carries the line of the turn it superseded,
    /// if any.
    Opened { id: u64, previous: Option<String> },
    /// The release was the open turn's own, reported again. Nothing changed;
    /// carries the open turn's id.
    Duplicate(u64),
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
        match self.begin_release(|| id, released, None) {
            Begin::Opened { previous, .. } => previous,
            Begin::Duplicate(_) => None,
        }
    }

    /// [`begin`](Self::begin) for a release from `voice_session`. A second
    /// report of the open turn's own release is a no-op rather than a new turn
    /// that logs the real one as `superseded`. `next_id` is only called when a
    /// turn actually opens.
    pub fn begin_release(
        &mut self,
        next_id: impl FnOnce() -> u64,
        released: Instant,
        voice_session: Option<u64>,
    ) -> Begin {
        if let Some(open) = self.current.as_ref() {
            if open.is_same_release(released, voice_session) {
                return Begin::Duplicate(open.id());
            }
        }
        let previous = self.current.take().map(|t| t.line("superseded"));
        let id = next_id();
        let mut timer = TurnTimer::new(id, released);
        timer.voice_session = voice_session;
        self.current = Some(timer);
        Begin::Opened { id, previous }
    }

    /// Open a turn for a typed query submitted at `at`. Every query, spoken or
    /// typed, reaches the submit path, so a voice turn still waiting for its
    /// first request owns this submit: `None`, nothing changed.
    pub fn begin_typed(&mut self, next_id: impl FnOnce() -> u64, at: Instant) -> Option<Begin> {
        if let Some(open) = self.current.as_ref() {
            if open.source == SOURCE_VOICE && open.offset(Stage::LlmRequestSent).is_none() {
                return None;
            }
        }
        let previous = self.current.take().map(|t| t.line("superseded"));
        let id = next_id();
        let mut timer = TurnTimer::new(id, at);
        timer.source = SOURCE_TYPED;
        self.current = Some(timer);
        Some(Begin::Opened { id, previous })
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

    pub fn note_effort(&mut self, effort: &str) {
        if let Some(timer) = self.current.as_mut() {
            timer.note_effort(effort);
        }
    }

    /// Whether the open turn began as a hold-key release.
    pub fn open_turn_is_voice(&self) -> bool {
        self.current
            .as_ref()
            .is_some_and(|timer| timer.source == SOURCE_VOICE)
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
///
/// `voice_session` is the session the release committed. A second report of
/// the same release (same session, or within [`DUPLICATE_RELEASE_WINDOW`]
/// with nothing recorded yet) returns the open turn's id and changes nothing.
pub fn begin(released: Instant, voice_session: Option<u64>) -> u64 {
    let begun = with_tracker(|t| {
        t.begin_release(
            || NEXT_TURN.fetch_add(1, Ordering::Relaxed),
            released,
            voice_session,
        )
    });
    match begun {
        Begin::Opened { id, previous } => {
            emit(previous);
            id
        }
        Begin::Duplicate(id) => id,
    }
}

/// Open a turn for a typed query sent now, closed by audio or after
/// [`TURN_TIMEOUT`]. A no-op when the submit belongs to an open voice turn.
pub fn begin_typed() {
    let opened = with_tracker(|t| {
        t.begin_typed(|| NEXT_TURN.fetch_add(1, Ordering::Relaxed), Instant::now())
    });
    if let Some(Begin::Opened { id, previous }) = opened {
        emit(previous);
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(TURN_TIMEOUT).await;
            finish(Some(id), "timeout");
        });
    }
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

/// Record which Claude CLI effort the open turn ran at.
pub fn note_effort(effort: &str) {
    with_tracker(|t| t.note_effort(effort));
}

/// Whether the query being answered now was spoken (a hold-key release opened
/// the turn) rather than typed. Every query passes [`begin_typed`] on submit,
/// which leaves a voice turn waiting for its first request in place and opens
/// a typed turn otherwise, so at request time the open turn names the source.
pub fn open_turn_is_voice() -> bool {
    with_tracker(|t| t.open_turn_is_voice())
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
             provider=claude_cli/persistent-warm model=sonnet effort=- tts=kokoro source=voice"
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
             llm_first_token=- first_tts_text=- first_audio=- provider=- model=- effort=- tts=- source=voice"
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
        // A real second press, past the duplicate-release window.
        tracker.begin(9, t0 + ms(1000));
        assert!(tracker.finish(Some(8), "timeout").is_none());
        assert!(tracker.finish(Some(9), "timeout").is_some());
    }

    #[test]
    fn a_voice_turn_owns_its_own_submit() {
        let t0 = Instant::now();
        let mut tracker = Tracker::default();
        tracker.begin(1, t0);
        tracker.mark_at(Stage::TranscriptFinal, t0 + ms(300));
        // The transcript reaches the submit path: not a second turn.
        assert!(tracker.begin_typed(|| 2, t0 + ms(310)).is_none());
        tracker.mark_at(Stage::LlmRequestSent, t0 + ms(320));
        // Once the voice turn has sent its request, a submit is a new typed turn.
        match tracker.begin_typed(|| 3, t0 + ms(5000)) {
            Some(Begin::Opened { id, previous }) => {
                assert_eq!(id, 3);
                assert!(previous.unwrap_or_default().contains("source=voice"));
            }
            other => panic!("expected a typed turn, got {other:?}"),
        }
    }

    #[test]
    fn a_typed_turn_is_timed_from_the_submit() {
        let t0 = Instant::now();
        let mut tracker = Tracker::default();
        assert!(tracker.begin_typed(|| 4, t0).is_some());
        tracker.mark_at(Stage::LlmRequestSent, t0 + ms(20));
        tracker.mark_at(Stage::LlmFirstToken, t0 + ms(900));
        let line = tracker
            .mark_at(Stage::FirstAudio, t0 + ms(1200))
            .unwrap_or_default();
        assert!(
            line.contains("transcript_final=- llm_request_sent=20"),
            "{line}"
        );
        assert!(line.ends_with("source=typed"), "{line}");
    }

    #[test]
    fn the_open_turn_names_the_query_source_and_effort() {
        let t0 = Instant::now();
        let mut tracker = Tracker::default();
        assert!(!tracker.open_turn_is_voice(), "no turn, not voice");
        tracker.begin(1, t0);
        assert!(tracker.open_turn_is_voice());
        // The spoken query reaches the submit path: still the voice turn.
        assert!(tracker.begin_typed(|| 2, t0 + ms(400)).is_none());
        assert!(tracker.open_turn_is_voice());
        tracker.note_effort("medium");
        tracker.note_effort("high");
        tracker.mark_at(Stage::LlmRequestSent, t0 + ms(450));
        let line = tracker.finish(None, "x").unwrap_or_default();
        assert!(line.contains("model=- effort=medium tts=-"), "{line}");
        // A typed query after it is typed.
        assert!(tracker.begin_typed(|| 3, t0 + ms(900)).is_some());
        assert!(!tracker.open_turn_is_voice());
    }

    #[test]
    fn the_same_release_reported_twice_is_one_turn() {
        // 2026-10-06 log: one key release reached `begin` from two paths and
        // the second logged the real turn as `superseded` at the same instant.
        let t0 = Instant::now();
        let mut tracker = Tracker::default();
        let mut ids = 10..;
        let mut next = || ids.next().unwrap_or_default();
        assert_eq!(
            tracker.begin_release(&mut next, t0, Some(5)),
            Begin::Opened {
                id: 10,
                previous: None
            }
        );
        // Same voice session, any time later: a no-op.
        assert_eq!(
            tracker.begin_release(&mut next, t0 + ms(400), Some(5)),
            Begin::Duplicate(10)
        );
        // The open turn kept its clock and its id.
        tracker.mark_at(Stage::TranscriptFinal, t0 + ms(300));
        let line = tracker.finish(None, "done").expect("still open");
        assert!(line.starts_with("turn=10 "));
        assert!(line.contains("transcript_final=300"));
    }

    #[test]
    fn a_double_report_with_no_session_is_caught_by_time() {
        let t0 = Instant::now();
        let mut tracker = Tracker::default();
        tracker.begin(1, t0);
        // Within the window and nothing recorded yet: the same release.
        assert_eq!(
            tracker.begin_release(|| 2, t0 + ms(10), None),
            Begin::Duplicate(1)
        );
        // Once a stage has been reached, a release is a new turn.
        tracker.mark_at(Stage::TranscriptFinal, t0 + ms(20));
        match tracker.begin_release(|| 3, t0 + ms(30), None) {
            Begin::Opened {
                previous: Some(previous),
                ..
            } => assert!(previous.starts_with("turn=1 ")),
            other => panic!("expected a new turn, got {other:?}"),
        }
    }

    #[test]
    fn a_new_voice_session_is_a_new_turn_however_soon() {
        let t0 = Instant::now();
        let mut tracker = Tracker::default();
        tracker.begin_release(|| 1, t0, Some(4));
        match tracker.begin_release(|| 2, t0 + ms(5), Some(5)) {
            Begin::Opened {
                previous: Some(previous),
                ..
            } => {
                assert!(previous.starts_with("turn=1 outcome=superseded"))
            }
            other => panic!("expected a new turn, got {other:?}"),
        }
        // And a release outside the window with no session is new too.
        match tracker.begin_release(|| 3, t0 + ms(500), None) {
            Begin::Opened {
                previous: Some(previous),
                ..
            } => assert!(previous.starts_with("turn=2 ")),
            other => panic!("expected a new turn, got {other:?}"),
        }
    }

    #[test]
    fn a_mark_before_the_release_clamps_to_zero() {
        let t0 = Instant::now();
        let mut timer = TurnTimer::new(1, t0 + ms(50));
        timer.mark_at(Stage::TranscriptFinal, t0);
        assert_eq!(timer.offset(Stage::TranscriptFinal), Some(Duration::ZERO));
    }
}
