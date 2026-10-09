//! The Mac's voice, spoken from inside Juno instead of from a `say` process.
//!
//! Each `say` costs 0.8 to 2.0 s before its first sound, warm or cold, because
//! the cost is the process (see `docs/research/say-latency.md`). An
//! `AVSpeechSynthesizer` that stays alive in Juno pays about a second once, at
//! launch, and then starts a sentence in 10 to 30 ms.
//!
//! # How a sentence plays
//!
//! 1. The synthesizer renders into buffers (`writeUtterance:toBufferCallback:`)
//!    instead of speaking, so Juno owns the samples. Measured on macOS 26:
//!    mono Float32 at 22,050 Hz, delivered on the main thread, ending with one
//!    zero-length buffer.
//! 2. The samples go into a [`Session`]'s [`PlaybackBuffer`], and a cpal output
//!    stream on the speaker chosen in Settings pulls them out, resampled to the
//!    device's rate. That is how the Speaker setting (#652) still applies:
//!    `AVSpeechSynthesizer.speak` could only use the system output.
//! 3. A stop marks the session stopped (the next audio callback is silence) and
//!    tells the synthesizer to stop at once.
//!
//! # What replaced the `say` pid
//!
//! The pid gave three things, and each now comes from the buffers:
//! - "audio started" ([`crate::turn_timing::Stage::FirstAudio`]): the first
//!   frame the device actually plays, not the spawn;
//! - the echo hold (#720): the audio callback plays silence and does not
//!   advance while [`crate::tts::is_held_silent`] is true, so Juno picks up
//!   mid-word when the microphone closes, as `SIGCONT` did;
//! - Escape: [`stop_all`], reached from the one place every stop goes through
//!   (`kill_audio_processes`).
//!
//! # When `say` still speaks
//!
//! Silently, whenever this path cannot honour the request exactly: before the
//! synthesizer is warm, when the chosen voice is not one AVFoundation can name
//! unambiguously, when the Mac's own voice is a Siri voice (third-party apps
//! cannot use Siri voices), or when no audio arrives in time. `system.rs`
//! keeps the `say` code for exactly that.
//!
//! The decisions (voice matching, the rate mapping, the playback buffer, the
//! session that stop and start race on) are plain Rust behind the [`Synth`]
//! trait, tested without a speaker.

#![cfg_attr(not(target_os = "macos"), allow(dead_code))]

use crate::tts::voices::{voice_base_name, voice_quality, VoiceQuality};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex as StdMutex, MutexGuard};
use std::time::{Duration, Instant, SystemTime};

/// How long the first buffer of a sentence may take before `say` is used
/// instead. A warm synthesizer answers in about 30 ms and a voice it has not
/// loaded yet in about 250 ms; a cold one takes about a second.
pub const FIRST_BUFFER_TIMEOUT: Duration = Duration::from_millis(2_500);

/// A render that stopped sending buffers without its zero-length end marker
/// is treated as finished once everything it sent has played and this long
/// has passed. Rendering runs many times faster than real time, so a real
/// sentence never pauses this long between buffers.
pub const RENDER_STALL: Duration = Duration::from_millis(1_500);

/// How long a render ahead waits for the synthesizer to warm up at launch.
pub const RENDER_WARM_WAIT: Duration = Duration::from_secs(5);

/// How long a whole render ahead may take. A greeting renders in tens of ms.
pub const RENDER_WHOLE_WAIT: Duration = Duration::from_secs(10);

/// The rate AVFoundation speaks at when nothing is set, which renders exactly
/// as `say` does at its own 175 words a minute (measured, see [`av_rate_for`]).
pub const AV_DEFAULT_RATE: f32 = 0.5;

// ---------------------------------------------------------------------------
// The rate
// ---------------------------------------------------------------------------

/// Juno's speed multiplier against `AVSpeechUtterance.rate`.
///
/// `say` takes words per minute (175 x the multiplier). AVFoundation takes
/// 0.0 to 1.0 with 0.5 as normal, and the scale is not linear. Measured on
/// macOS 26 by rendering the same 24-word sentence with Samantha through both
/// and matching durations (`say -r N -o` against `write` at each rate):
///
/// | Juno | `say -r` | seconds | AV rate with the same duration |
/// |---|---|---|---|
/// | 0.75x | 131 | 6.95 | 0.40 to 0.44 |
/// | 1.0x | 175 | 6.32 | 0.48 to 0.50 |
/// | 1.25x | 219 | 5.12 | 0.54 |
/// | 1.5x | 262 | 4.43 | 0.57 to 0.58 |
/// | 1.75x | 306 | 3.97 | 0.60 |
/// | 2.0x | 350 | 3.45 | 0.64 |
///
/// Daniel scales the same way (0.5 to 0.6 is 1.59x for both). Between the
/// rows the rate is interpolated linearly.
const RATE_TABLE: &[(f64, f32)] = &[
    (0.75, 0.42),
    (1.0, 0.50),
    (1.25, 0.54),
    (1.5, 0.575),
    (1.75, 0.60),
    (2.0, 0.64),
];

/// The `AVSpeechUtterance.rate` that speaks as fast as `say -r 175 x rate`.
pub fn av_rate_for(rate: f64) -> f32 {
    let rate = crate::tts::rate::sanitize(rate);
    let mut previous = RATE_TABLE[0];
    if rate <= previous.0 {
        return previous.1;
    }
    for &(multiplier, av) in &RATE_TABLE[1..] {
        if rate <= multiplier {
            let span = multiplier - previous.0;
            let t = if span > 0.0 {
                (rate - previous.0) / span
            } else {
                0.0
            };
            return previous.1 + ((av - previous.1) as f64 * t) as f32;
        }
        previous = (multiplier, av);
    }
    previous.1
}

/// The multiplier a `say -r` value stands for. `None` is normal pace.
pub fn rate_from_words_per_minute(words_per_minute: Option<u32>) -> f64 {
    match words_per_minute {
        Some(wpm) if wpm > 0 => wpm as f64 / crate::tts::rate::SAY_BASE_WPM,
        _ => crate::tts::rate::DEFAULT_RATE,
    }
}

// ---------------------------------------------------------------------------
// The voice
// ---------------------------------------------------------------------------

/// One voice AVFoundation offers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AvVoice {
    /// `com.apple.voice.compact.en-US.Samantha`.
    pub identifier: String,
    /// `Samantha`, with no quality suffix.
    pub name: String,
    pub quality: VoiceQuality,
    /// `en-US`.
    pub language: String,
}

/// `AVSpeechSynthesisVoiceQuality` as the quality `say` puts in a name.
pub fn quality_from_av(raw: isize) -> VoiceQuality {
    match raw {
        3 => VoiceQuality::Premium,
        2 => VoiceQuality::Enhanced,
        _ => VoiceQuality::Compact,
    }
}

/// System Settings > Accessibility > Spoken Content > System Voice, which is
/// what `say` uses with no `-v`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SpokenContent {
    pub voice_id: Option<String>,
    pub voice_name: Option<String>,
    /// The language the choice was made for (`en`), when known.
    pub language: Option<String>,
}

/// How long a reading of the Spoken Content voice is trusted without looking
/// at the preferences file again.
pub const SPOKEN_FRESH_FOR: Duration = Duration::from_secs(5);

/// Whether a reading of the Spoken Content voice must be redone before a
/// sentence is spoken with it. The person can change the System Voice at any
/// time, so a reading is stale when the preferences file has been written since
/// (its modification time moved), or when it is simply old. Pure.
pub fn spoken_is_stale(
    age: Duration,
    mtime_when_read: Option<SystemTime>,
    mtime_now: Option<SystemTime>,
) -> bool {
    age >= SPOKEN_FRESH_FOR || mtime_when_read != mtime_now
}

/// Which voice a sentence is rendered with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VoicePlan {
    /// A voice by AVFoundation identifier.
    Identifier(String),
    /// AVFoundation's default for the Mac's language, which is what `say`
    /// uses when no System Voice was ever chosen (measured: byte-for-byte the
    /// same duration as `say` with no `-v`).
    LanguageDefault,
    /// This path cannot speak with the voice that was asked for.
    UseSay,
}

/// The AVFoundation voice for a stored `say -v` name.
///
/// `say` names a voice "Ava (Premium)" where AVFoundation has name "Ava" and
/// quality premium, so the match is the base name plus the quality. A `say`
/// name can also carry a language ("Eddy (English (US))"); when that leaves
/// more than one candidate, nothing is guessed and `say` keeps the voice.
pub fn match_voice(stored: &str, voices: &[AvVoice]) -> Option<String> {
    let stored = stored.trim();
    if stored.is_empty() {
        return None;
    }
    let base = voice_base_name(stored);
    let quality = voice_quality(stored);
    let mut candidates = voices
        .iter()
        .filter(|voice| voice.name.eq_ignore_ascii_case(base) && voice.quality == quality);
    let first = candidates.next()?;
    if candidates.next().is_some() {
        return None;
    }
    Some(first.identifier.clone())
}

/// Plan the voice for a sentence.
///
/// `stored` is the `-v` value `say` would get (`None` for the Mac's own
/// voice). `spoken_content` is `None` until it has been read once, and then
/// nothing is assumed.
pub fn plan_voice(
    stored: Option<&str>,
    spoken_content: Option<&SpokenContent>,
    voices: &[AvVoice],
) -> VoicePlan {
    if let Some(stored) = stored.map(str::trim).filter(|s| !s.is_empty()) {
        return match match_voice(stored, voices) {
            Some(identifier) => VoicePlan::Identifier(identifier),
            None => VoicePlan::UseSay,
        };
    }
    let Some(spoken) = spoken_content else {
        return VoicePlan::UseSay;
    };
    let by_id = || {
        spoken
            .voice_id
            .as_deref()
            .and_then(|id| voices.iter().find(|voice| voice.identifier == id))
            .map(|voice| VoicePlan::Identifier(voice.identifier.clone()))
    };
    match crate::tts::voices::classify_system_voice(
        spoken.voice_id.as_deref(),
        spoken.voice_name.as_deref(),
    ) {
        crate::tts::voices::SystemVoice::Unset => VoicePlan::LanguageDefault,
        // A voice AVFoundation can build is spoken as chosen. A Siri voice, or
        // one that is not installed, is not buildable here: the best installed
        // voice for the language speaks instead.
        _ => by_id().unwrap_or_else(|| match best_voice(voices, spoken.language.as_deref()) {
            Some(voice) => VoicePlan::Identifier(voice.identifier.clone()),
            None => VoicePlan::LanguageDefault,
        }),
    }
}

/// The best installed voice for a language: Premium, then Enhanced, then
/// Compact; among those the real Apple voices (`com.apple.voice.*`) before
/// novelty ones, American English, Samantha (what a Mac ships with), then the
/// name and identifier so the pick never changes between two reads. With no
/// voice in the language, the best of any. Pure.
pub fn best_voice<'a>(voices: &'a [AvVoice], language: Option<&str>) -> Option<&'a AvVoice> {
    let language = language
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .unwrap_or("en")
        .to_ascii_lowercase();
    let in_language = |voice: &AvVoice| {
        let tag = voice.language.to_ascii_lowercase().replace('_', "-");
        tag == language || tag.starts_with(&format!("{language}-"))
    };
    let rank = |voice: &'a AvVoice| {
        (
            voice.quality,
            voice.identifier.starts_with("com.apple.voice."),
            voice.language.eq_ignore_ascii_case("en-US"),
            voice.name == "Samantha",
            std::cmp::Reverse(voice.name.as_str()),
            std::cmp::Reverse(voice.identifier.as_str()),
        )
    };
    voices
        .iter()
        .filter(|voice| in_language(voice))
        .max_by_key(|voice| rank(voice))
        .or_else(|| voices.iter().max_by_key(|voice| rank(voice)))
}

// ---------------------------------------------------------------------------
// The playback buffer
// ---------------------------------------------------------------------------

/// Where a sentence's playback is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackState {
    /// Nothing has been rendered yet.
    Waiting,
    /// Samples are arriving or playing.
    Playing,
    /// Everything rendered has played, or the sentence was stopped.
    Done,
}

/// The samples of one sentence, between the synthesizer and the device.
///
/// Stores mono samples at the synthesizer's rate and resamples linearly as the
/// device pulls, so nothing needs the device's rate before it is open.
#[derive(Debug)]
pub struct PlaybackBuffer {
    samples: VecDeque<f32>,
    source_rate: f64,
    /// Fractional read position into `samples`.
    position: f64,
    rendered: bool,
    stopped: bool,
    received_any: bool,
    played_any: bool,
    last_push: Option<Instant>,
}

impl Default for PlaybackBuffer {
    fn default() -> Self {
        Self::new()
    }
}

impl PlaybackBuffer {
    pub fn new() -> Self {
        Self {
            samples: VecDeque::new(),
            source_rate: 22_050.0,
            position: 0.0,
            rendered: false,
            stopped: false,
            received_any: false,
            played_any: false,
            last_push: None,
        }
    }

    /// Samples from the synthesizer. Ignored once stopped.
    pub fn push(&mut self, samples: &[f32], source_rate: f64, now: Instant) {
        if self.stopped || samples.is_empty() {
            return;
        }
        if source_rate.is_finite() && source_rate > 0.0 {
            self.source_rate = source_rate;
        }
        self.samples.extend(samples.iter().copied());
        self.received_any = true;
        self.last_push = Some(now);
    }

    /// The synthesizer's zero-length buffer: nothing more is coming.
    pub fn finish_render(&mut self) {
        self.rendered = true;
    }

    /// Escape, a new turn, or a failure: silence from the next callback on.
    pub fn stop(&mut self) {
        self.stopped = true;
        self.samples.clear();
        self.position = 0.0;
    }

    pub fn is_stopped(&self) -> bool {
        self.stopped
    }

    /// The end marker arrived.
    pub fn is_rendered(&self) -> bool {
        self.rendered
    }

    /// Everything rendered, at the synthesizer's rate, for a render nobody
    /// plays. `None` when nothing was rendered.
    pub fn take_all(&mut self) -> Option<(Vec<f32>, f64)> {
        if self.samples.is_empty() {
            return None;
        }
        self.position = 0.0;
        Some((self.samples.drain(..).collect(), self.source_rate))
    }

    pub fn received_any(&self) -> bool {
        self.received_any
    }

    pub fn played_any(&self) -> bool {
        self.played_any
    }

    /// Fill `out` (mono frames at `out_rate`). While `held` the output is
    /// silence and the position does not move, so the sentence resumes from
    /// the same sample. Returns how many frames carried speech.
    pub fn fill(&mut self, out: &mut [f32], out_rate: f64, held: bool) -> usize {
        out.fill(0.0);
        if self.stopped || held || !(out_rate.is_finite() && out_rate > 0.0) {
            return 0;
        }
        let step = self.source_rate / out_rate;
        let mut written = 0;
        for slot in out.iter_mut() {
            let index = self.position.floor() as usize;
            let Some(&current) = self.samples.get(index) else {
                break;
            };
            let next = match self.samples.get(index + 1) {
                Some(&next) => next,
                // The last sample of a finished render fades to zero. One
                // still rendering waits for the next buffer instead.
                None if self.rendered => 0.0,
                None => break,
            };
            let fraction = (self.position - index as f64) as f32;
            *slot = current + (next - current) * fraction;
            self.position += step;
            written += 1;
        }
        let consumed = (self.position.floor() as usize).min(self.samples.len());
        self.samples.drain(..consumed);
        self.position -= consumed as f64;
        if written > 0 {
            self.played_any = true;
        }
        written
    }

    /// Where playback is. A render that went quiet without its end marker
    /// counts as finished after [`RENDER_STALL`], once its samples have played.
    pub fn state(&self, now: Instant) -> PlaybackState {
        if self.stopped {
            return PlaybackState::Done;
        }
        if !self.received_any {
            return if self.rendered {
                PlaybackState::Done
            } else {
                PlaybackState::Waiting
            };
        }
        let stalled = self
            .last_push
            .is_some_and(|at| now.saturating_duration_since(at) >= RENDER_STALL);
        // Without the end marker the last sample waits for a neighbour that
        // is never coming, so a stalled render ends with at most that one left.
        let drained = self.samples.is_empty() && self.rendered;
        let stalled_out = stalled && self.samples.len() <= 1;
        if drained || stalled_out {
            PlaybackState::Done
        } else {
            PlaybackState::Playing
        }
    }
}

// ---------------------------------------------------------------------------
// The session, and what starts and stops one
// ---------------------------------------------------------------------------

/// One sentence in flight. Shared by the synthesizer's callback (which
/// pushes), the audio callback (which pulls) and whoever stops it.
#[derive(Debug)]
pub struct Session {
    buffer: StdMutex<PlaybackBuffer>,
}

impl Session {
    fn new() -> Self {
        Self {
            buffer: StdMutex::new(PlaybackBuffer::new()),
        }
    }

    /// The buffer. A poisoned lock still holds a usable buffer, and the audio
    /// callback must never panic.
    pub fn buffer(&self) -> MutexGuard<'_, PlaybackBuffer> {
        self.buffer
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub fn push(&self, samples: &[f32], source_rate: f64) {
        self.buffer().push(samples, source_rate, Instant::now());
    }

    pub fn finish_render(&self) {
        self.buffer().finish_render();
    }

    pub fn stop(&self) {
        self.buffer().stop();
    }

    pub fn is_stopped(&self) -> bool {
        self.buffer().is_stopped()
    }
}

/// What one sentence is rendered from.
#[derive(Debug, Clone, PartialEq)]
pub struct Utterance {
    pub text: String,
    pub voice: VoicePlan,
    pub av_rate: f32,
}

/// The synthesizer, as the session logic sees it. The macOS one dispatches to
/// AVFoundation on the main thread; tests use a fake.
pub trait Synth: Send + Sync {
    /// Start rendering into `session`. Must return without waiting for audio.
    fn render(&self, utterance: Utterance, session: Arc<Session>) -> Result<(), String>;
    /// Stop rendering whatever is in progress, immediately.
    fn cancel(&self);
}

/// Why a sentence did not start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StartError {
    /// A stop arrived first. The sentence is not wanted.
    Stopped,
    /// The synthesizer would not take it; `say` should.
    Failed(String),
}

/// The one sentence speaking now, and the synthesizer behind it.
///
/// Start and stop race: Escape can land between the worker's stop check and
/// the session becoming current. `stop_speech` sets the stop flag and then
/// takes the current session under this lock; [`Speaker::begin`] makes its
/// session current under the same lock and then reads the flag. Either the
/// stop sees the session or the session sees the flag.
pub struct Speaker<S: Synth> {
    synth: S,
    current: StdMutex<Option<Arc<Session>>>,
}

impl<S: Synth> Speaker<S> {
    pub const fn new(synth: S) -> Self {
        Self {
            synth,
            current: StdMutex::new(None),
        }
    }

    fn current(&self) -> MutexGuard<'_, Option<Arc<Session>>> {
        self.current
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Start a sentence. A sentence still current is stopped first: only one
    /// plays at a time.
    pub fn begin(
        &self,
        utterance: Utterance,
        stop_requested: impl Fn() -> bool,
    ) -> Result<Arc<Session>, StartError> {
        let session = Arc::new(Session::new());
        let previous = self.current().replace(session.clone());
        if let Some(previous) = previous {
            previous.stop();
            self.synth.cancel();
        }
        if stop_requested() {
            self.end(&session);
            session.stop();
            return Err(StartError::Stopped);
        }
        if let Err(e) = self.synth.render(utterance, session.clone()) {
            self.end(&session);
            session.stop();
            return Err(StartError::Failed(e));
        }
        Ok(session)
    }

    /// Make an already-filled session current (audio rendered ahead), with
    /// the same stop race rule as [`Speaker::begin`]. Nothing is rendered.
    pub fn adopt(
        &self,
        session: Arc<Session>,
        stop_requested: impl Fn() -> bool,
    ) -> Result<(), StartError> {
        let previous = self.current().replace(session.clone());
        if let Some(previous) = previous {
            previous.stop();
            self.synth.cancel();
        }
        if stop_requested() {
            self.end(&session);
            session.stop();
            return Err(StartError::Stopped);
        }
        Ok(())
    }

    /// Stop whatever is speaking. True when something was.
    pub fn stop_all(&self) -> bool {
        let taken = self.current().take();
        match taken {
            Some(session) => {
                session.stop();
                self.synth.cancel();
                true
            }
            None => false,
        }
    }

    /// Give up on a sentence that never produced audio, so `say` can take it.
    pub fn abandon(&self, session: &Arc<Session>) {
        session.stop();
        self.end(session);
        self.synth.cancel();
    }

    /// The sentence is over. Only clears it if it is still the current one.
    pub fn end(&self, session: &Arc<Session>) {
        let mut current = self.current();
        if current
            .as_ref()
            .is_some_and(|now| Arc::ptr_eq(now, session))
        {
            *current = None;
        }
    }

    #[cfg(test)]
    pub fn is_current(&self, session: &Arc<Session>) -> bool {
        self.current()
            .as_ref()
            .is_some_and(|now| Arc::ptr_eq(now, session))
    }
}

/// What [`speak`] did with a sentence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Spoken {
    /// Handled here: a `TTS_*` status for the caller.
    Status(&'static str),
    /// Not handled; speak it with `say`. The reason is for the log only.
    UseSay(String),
}

// ---------------------------------------------------------------------------
// macOS
// ---------------------------------------------------------------------------

#[cfg(target_os = "macos")]
mod mac {
    use super::*;
    use block2::RcBlock;
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
    use objc2::rc::Retained;
    use objc2_avf_audio::{
        AVAudioBuffer, AVAudioCommonFormat, AVAudioPCMBuffer, AVSpeechBoundary,
        AVSpeechSynthesisVoice, AVSpeechSynthesizer, AVSpeechUtterance,
    };
    use objc2_foundation::NSString;
    use std::cell::RefCell;
    use std::ptr::NonNull;
    use std::sync::OnceLock;
    use tauri::{AppHandle, Manager};
    use tracing::{debug, info, warn};

    static APP: OnceLock<AppHandle> = OnceLock::new();

    /// The voices AVFoundation listed at the last refresh. `None` until the
    /// warm-up has run; until then `say` speaks.
    static VOICES: StdMutex<Option<Vec<AvVoice>>> = StdMutex::new(None);

    /// The Spoken Content voice, when it was read, and the modification time
    /// of the preferences file at that moment.
    static SPOKEN: StdMutex<Option<(Instant, Option<SystemTime>, SpokenContent)>> =
        StdMutex::new(None);

    pub(super) static SPEAKER: Speaker<AvSynth> = Speaker::new(AvSynth);

    thread_local! {
        /// The synthesizer. Only ever touched on the main thread, where
        /// AVFoundation delivers its buffers, and kept for the app's life so
        /// the voice stays loaded.
        static SYNTH: RefCell<Option<Retained<AVSpeechSynthesizer>>> = const { RefCell::new(None) };
    }

    /// Main thread only.
    fn with_synth<R>(f: impl FnOnce(&AVSpeechSynthesizer) -> R) -> R {
        SYNTH.with(|cell| {
            let mut slot = cell.borrow_mut();
            // SAFETY: plain `+new` on the main thread.
            let synth = slot.get_or_insert_with(|| unsafe { AVSpeechSynthesizer::new() });
            f(synth)
        })
    }

    fn lock<T>(mutex: &StdMutex<T>) -> MutexGuard<'_, T> {
        mutex
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub(super) struct AvSynth;

    impl Synth for AvSynth {
        fn render(&self, utterance: Utterance, session: Arc<Session>) -> Result<(), String> {
            let app = APP.get().ok_or("the synthesizer is not bound yet")?;
            app.run_on_main_thread(move || render_on_main(utterance, session))
                .map_err(|e| format!("could not reach the main thread: {e}"))
        }

        fn cancel(&self) {
            let Some(app) = APP.get() else { return };
            let result = app.run_on_main_thread(|| {
                with_synth(|synth| {
                    // SAFETY: main thread; stopping when idle is a no-op.
                    unsafe { synth.stopSpeakingAtBoundary(AVSpeechBoundary::Immediate) };
                });
            });
            if let Err(e) = result {
                warn!("[AVSpeech] Could not stop the synthesizer: {e}");
            }
        }
    }

    /// Main thread. Builds the utterance and starts the render; buffers
    /// arrive later, also on the main thread.
    fn render_on_main(utterance: Utterance, session: Arc<Session>) {
        with_synth(|synth| start_render(synth, utterance, session));
    }

    /// Main thread. Start `synth` rendering `utterance` into `session`.
    fn start_render(synth: &AVSpeechSynthesizer, utterance: Utterance, session: Arc<Session>) {
        if session.is_stopped() {
            return;
        }
        // SAFETY: every call below is a plain AVFoundation call on the main
        // thread with live objects.
        unsafe {
            let text = NSString::from_str(&utterance.text);
            let av = AVSpeechUtterance::speechUtteranceWithString(&text);
            match &utterance.voice {
                VoicePlan::Identifier(identifier) => {
                    let id = NSString::from_str(identifier);
                    match AVSpeechSynthesisVoice::voiceWithIdentifier(&id) {
                        Some(voice) => av.setVoice(Some(&voice)),
                        None => {
                            // Removed since the list was read. Nothing has
                            // played, so the waiter hands it to `say`.
                            warn!("[AVSpeech] Voice {identifier} is gone");
                            session.stop();
                            return;
                        }
                    }
                }
                VoicePlan::LanguageDefault | VoicePlan::UseSay => {}
            }
            av.setRate(utterance.av_rate);

            let sink = session.clone();
            let block: RcBlock<dyn Fn(NonNull<AVAudioBuffer>)> =
                RcBlock::new(move |buffer: NonNull<AVAudioBuffer>| {
                    // SAFETY: AVFoundation passes a live buffer for the call.
                    deliver(&sink, buffer.as_ref());
                });
            synth.writeUtterance_toBufferCallback(&av, RcBlock::as_ptr(&block));
        }
    }

    /// One buffer from the synthesizer. Never panics: it runs inside an
    /// Objective-C block.
    fn deliver(session: &Session, buffer: &AVAudioBuffer) {
        let Some(pcm) = buffer.downcast_ref::<AVAudioPCMBuffer>() else {
            return;
        };
        // SAFETY: reading a live PCM buffer's own description and samples,
        // within the frame count it reports.
        unsafe {
            let frames = pcm.frameLength() as usize;
            if frames == 0 {
                session.finish_render();
                return;
            }
            let format = pcm.format();
            let rate = format.sampleRate();
            let stride = pcm.stride().max(1);
            let common = format.commonFormat();
            let mut mono = Vec::with_capacity(frames);
            if common == AVAudioCommonFormat::PCMFormatFloat32 {
                let channels = pcm.floatChannelData();
                if channels.is_null() {
                    return;
                }
                let first = (*channels).as_ptr();
                let data = std::slice::from_raw_parts(first, frames * stride);
                mono.extend(data.iter().step_by(stride).copied());
            } else if common == AVAudioCommonFormat::PCMFormatInt16 {
                let channels = pcm.int16ChannelData();
                if channels.is_null() {
                    return;
                }
                let first = (*channels).as_ptr();
                let data = std::slice::from_raw_parts(first, frames * stride);
                mono.extend(
                    data.iter()
                        .step_by(stride)
                        .map(|&s| s as f32 / i16::MAX as f32),
                );
            } else {
                return;
            }
            session.push(&mono, rate);
        }
    }

    /// Main thread. The voices AVFoundation has.
    fn list_voices() -> Vec<AvVoice> {
        // SAFETY: class method returning an array of live voices.
        unsafe {
            AVSpeechSynthesisVoice::speechVoices()
                .iter()
                .map(|voice| AvVoice {
                    identifier: voice.identifier().to_string(),
                    name: voice.name().to_string(),
                    quality: quality_from_av(voice.quality().0),
                    language: voice.language().to_string(),
                })
                .collect()
        }
    }

    /// Re-read the voice list on the main thread, in the background.
    fn refresh_voices() {
        let Some(app) = APP.get() else { return };
        let _ = app.run_on_main_thread(|| {
            let voices = list_voices();
            debug!("[AVSpeech] {} voices available", voices.len());
            *lock(&VOICES) = Some(voices);
        });
    }

    fn read_spoken_content() -> SpokenContent {
        crate::tts::voices::read_spoken_content_blocking()
    }

    /// When the System Voice preferences were last written: the newest of the
    /// Accessibility plist (macOS 26) and the old Speech one.
    fn spoken_prefs_mtime() -> Option<SystemTime> {
        crate::tts::voices::spoken_prefs_files()
            .iter()
            .filter_map(|path| std::fs::metadata(path).ok()?.modified().ok())
            .max()
    }

    /// Forget the reading, so the next sentence reads the System Voice again.
    pub fn forget_spoken_content() {
        *lock(&SPOKEN) = None;
    }

    /// The Spoken Content voice as it is now. A reading is reused only while
    /// the preferences file has not been written and it is younger than
    /// [`SPOKEN_FRESH_FOR`]; otherwise it is read again before the sentence
    /// speaks, so a System Voice the person just changed is the one heard.
    async fn current_spoken_content() -> Option<SpokenContent> {
        let mtime = spoken_prefs_mtime();
        let cached = lock(&SPOKEN).clone();
        if let Some((at, seen, spoken)) = &cached {
            if !spoken_is_stale(at.elapsed(), *seen, mtime) {
                return Some(spoken.clone());
            }
        }
        match tokio::task::spawn_blocking(read_spoken_content).await {
            Ok(spoken) => {
                *lock(&SPOKEN) = Some((Instant::now(), mtime, spoken.clone()));
                Some(spoken)
            }
            Err(_) => cached.map(|(_, _, spoken)| spoken),
        }
    }

    /// Keep the app handle, read the voices and load the synthesizer, off
    /// the critical path. The first render in a process costs about a second
    /// (measured 0.7 to 1.1 s), so it happens here, silently, rather than on
    /// the first sentence.
    pub fn bind(app: AppHandle) {
        if APP.set(app.clone()).is_err() {
            return;
        }
        tauri::async_runtime::spawn(async {
            let _ = current_spoken_content().await;
        });
        let stored_voice = app
            .try_state::<crate::state::AppState>()
            .and_then(|state| state.get_system_voice().ok().flatten());
        let result = app.run_on_main_thread(move || {
            let started = Instant::now();
            let voices = list_voices();
            let warm_voice = stored_voice
                .as_deref()
                .and_then(|stored| match_voice(stored, &voices));
            *lock(&VOICES) = Some(voices);
            // A render nobody plays: loads the synthesizer and the voice.
            let session = Arc::new(Session::new());
            render_on_main(
                Utterance {
                    text: "Ready.".to_string(),
                    voice: warm_voice
                        .map(VoicePlan::Identifier)
                        .unwrap_or(VoicePlan::LanguageDefault),
                    av_rate: AV_DEFAULT_RATE,
                },
                session,
            );
            info!(
                "[AVSpeech] Synthesizer started ({} ms on the main thread)",
                started.elapsed().as_millis()
            );
        });
        if let Err(e) = result {
            warn!("[AVSpeech] Could not start the synthesizer: {e}; the Mac's voice uses say");
        }
    }

    pub fn stop_all() {
        if SPEAKER.stop_all() {
            info!("[AVSpeech] Stopped speaking");
        }
    }

    /// What the player thread reports.
    enum PlayerEvent {
        /// The synthesizer's first buffer arrived.
        Rendering,
        /// The device played the first frame.
        Audible {
            device_open_ms: u128,
        },
        /// Done, stopped, or never started.
        Finished,
        Failed(String),
    }

    /// The speaker chosen in Settings, or the system output when none was
    /// chosen or the chosen one is not connected.
    fn output_device(name: Option<&str>) -> Option<cpal::Device> {
        let host = cpal::default_host();
        if let Some(name) = name.map(str::trim).filter(|n| !n.is_empty()) {
            if let Ok(devices) = host.output_devices() {
                for device in devices {
                    if device.name().ok().as_deref() == Some(name) {
                        return Some(device);
                    }
                }
            }
            warn!("[AVSpeech] Speaker {name} is not connected; using the system output");
        }
        host.default_output_device()
    }

    fn build_stream<T>(
        device: &cpal::Device,
        config: &cpal::StreamConfig,
        session: Arc<Session>,
    ) -> Result<cpal::Stream, String>
    where
        T: cpal::SizedSample + cpal::FromSample<f32>,
    {
        let channels = (config.channels as usize).max(1);
        let out_rate = config.sample_rate.0 as f64;
        let mut mono: Vec<f32> = Vec::new();
        device
            .build_output_stream(
                config,
                move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
                    let frames = data.len() / channels;
                    if mono.len() < frames {
                        mono.resize(frames, 0.0);
                    }
                    let held = crate::tts::is_held_silent();
                    session.buffer().fill(&mut mono[..frames], out_rate, held);
                    for (frame, &sample) in data.chunks_mut(channels).zip(mono.iter()) {
                        for out in frame {
                            *out = T::from_sample_(sample);
                        }
                    }
                },
                |e| warn!("[AVSpeech] Output stream error: {e}"),
                None,
            )
            .map_err(|e| format!("could not open the speaker: {e}"))
    }

    /// One sentence's playback, on its own thread: a cpal stream is not
    /// `Send`, so it is opened, watched and closed here.
    fn run_player(
        session: Arc<Session>,
        device_name: Option<String>,
        events: tokio::sync::mpsc::UnboundedSender<PlayerEvent>,
    ) {
        let opened = Instant::now();
        let Some(device) = output_device(device_name.as_deref()) else {
            let _ = events.send(PlayerEvent::Failed("no output device".to_string()));
            return;
        };
        let supported = match device.default_output_config() {
            Ok(config) => config,
            Err(e) => {
                let _ = events.send(PlayerEvent::Failed(format!("no output format: {e}")));
                return;
            }
        };
        let config = supported.config();
        let stream = match supported.sample_format() {
            cpal::SampleFormat::F32 => build_stream::<f32>(&device, &config, session.clone()),
            cpal::SampleFormat::I16 => build_stream::<i16>(&device, &config, session.clone()),
            cpal::SampleFormat::U16 => build_stream::<u16>(&device, &config, session.clone()),
            cpal::SampleFormat::I32 => build_stream::<i32>(&device, &config, session.clone()),
            other => Err(format!("unsupported output format {other:?}")),
        };
        let stream = match stream {
            Ok(stream) => stream,
            Err(e) => {
                let _ = events.send(PlayerEvent::Failed(e));
                return;
            }
        };
        if let Err(e) = stream.play() {
            let _ = events.send(PlayerEvent::Failed(format!(
                "could not start the speaker: {e}"
            )));
            return;
        }
        let device_open_ms = opened.elapsed().as_millis();

        let mut told_rendering = false;
        let mut told_audible = false;
        loop {
            let (state, received, played) = {
                let buffer = session.buffer();
                (
                    buffer.state(Instant::now()),
                    buffer.received_any(),
                    buffer.played_any(),
                )
            };
            if received && !told_rendering {
                told_rendering = true;
                let _ = events.send(PlayerEvent::Rendering);
            }
            if played && !told_audible {
                told_audible = true;
                let _ = events.send(PlayerEvent::Audible { device_open_ms });
            }
            if state == PlaybackState::Done {
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        // Let the last callback's samples leave the device before closing.
        if played_and_not_stopped(&session) {
            std::thread::sleep(Duration::from_millis(40));
        }
        // Pause before dropping: a CoreAudio stream on a non-default device
        // has been seen to keep running after drop (#708).
        let _ = stream.pause();
        drop(stream);
        let _ = events.send(PlayerEvent::Finished);
    }

    fn played_and_not_stopped(session: &Session) -> bool {
        let buffer = session.buffer();
        buffer.played_any() && !buffer.is_stopped()
    }

    /// Speak one sentence and wait until it has finished, or hand it back for
    /// `say`. See the module docs for what falls back.
    pub async fn speak(
        text: &str,
        stored_voice: Option<&str>,
        device: Option<&str>,
        rate: f64,
    ) -> Spoken {
        if APP.get().is_none() {
            return Spoken::UseSay("not bound".to_string());
        }
        let voices = lock(&VOICES).clone();
        let Some(voices) = voices else {
            return Spoken::UseSay("still warming up".to_string());
        };
        let spoken = current_spoken_content().await;
        let plan = plan_voice(stored_voice, spoken.as_ref(), &voices);
        if plan == VoicePlan::UseSay {
            // A voice installed since the last listing is found next time.
            refresh_voices();
            return Spoken::UseSay(format!(
                "no AVFoundation voice for {}",
                stored_voice.unwrap_or("the Mac's own voice")
            ));
        }

        let started = Instant::now();
        let chars = text.chars().count();
        let utterance = Utterance {
            text: text.to_string(),
            voice: plan.clone(),
            av_rate: av_rate_for(rate),
        };
        let session = match SPEAKER.begin(utterance, crate::tts::is_tts_stop_requested) {
            Ok(session) => session,
            Err(StartError::Stopped) => return Spoken::Status("TTS_STOPPED_BY_USER"),
            Err(StartError::Failed(e)) => return Spoken::UseSay(e),
        };

        let mut rx = match start_player(&session, device) {
            Ok(rx) => rx,
            Err(e) => {
                SPEAKER.abandon(&session);
                return Spoken::UseSay(e);
            }
        };

        // Until the first buffer arrives the sentence can still go to `say`
        // without anything having been heard twice.
        let first = tokio::time::timeout(FIRST_BUFFER_TIMEOUT, rx.recv()).await;
        let render_ms = started.elapsed().as_millis();
        match first {
            Ok(Some(PlayerEvent::Rendering)) | Ok(Some(PlayerEvent::Audible { .. })) => {}
            Ok(Some(PlayerEvent::Finished)) | Ok(None) => {
                let stopped = session.is_stopped();
                SPEAKER.end(&session);
                if crate::tts::is_tts_stop_requested() {
                    return Spoken::Status("TTS_STOPPED_BY_USER");
                }
                if stopped {
                    // Stopped without a stop request: the voice went missing.
                    return Spoken::UseSay("the voice could not be loaded".to_string());
                }
                return Spoken::Status("TTS_COMPLETED");
            }
            Ok(Some(PlayerEvent::Failed(e))) => {
                SPEAKER.abandon(&session);
                return Spoken::UseSay(e);
            }
            Err(_) => {
                SPEAKER.abandon(&session);
                if crate::tts::is_tts_stop_requested() {
                    return Spoken::Status("TTS_STOPPED_BY_USER");
                }
                return Spoken::UseSay(format!(
                    "no audio after {} ms",
                    FIRST_BUFFER_TIMEOUT.as_millis()
                ));
            }
        }

        // The synthesizer is producing: from here this path owns the sentence.
        let detail = format!(
            "first_buffer_ms={} chars={} voice={} rate={:.2}",
            render_ms,
            chars,
            match &plan {
                VoicePlan::Identifier(id) => id.as_str(),
                _ => "default",
            },
            rate,
        );
        play_out(&session, rx, started, "avspeech", &detail, device).await
    }

    /// Everything after the first buffer: mark first audio when the device
    /// plays it, drive the mouth, and wait until the sentence ends or stops.
    async fn play_out(
        session: &Arc<Session>,
        mut rx: tokio::sync::mpsc::UnboundedReceiver<PlayerEvent>,
        started: Instant,
        engine: &str,
        detail: &str,
        device: Option<&str>,
    ) -> Spoken {
        crate::tts::release_stale_hold();
        let mut level: Option<crate::tts::speech_level::SpeechLevelSession> = None;
        while let Some(event) = rx.recv().await {
            match event {
                PlayerEvent::Rendering => {}
                PlayerEvent::Audible { device_open_ms } => {
                    crate::turn_timing::mark(crate::turn_timing::Stage::FirstAudio);
                    level = Some(crate::tts::speech_level::begin(
                        crate::tts::speech_level::LevelSource::Synthetic,
                        Duration::ZERO,
                    ));
                    info!(
                        "[SpeechTiming] engine={} first_audio_ms={} device_open_ms={} {} device={}",
                        engine,
                        started.elapsed().as_millis(),
                        device_open_ms,
                        detail,
                        device.unwrap_or("system"),
                    );
                }
                PlayerEvent::Finished => break,
                PlayerEvent::Failed(e) => {
                    warn!("[AVSpeech] Playback failed: {e}");
                    break;
                }
            }
        }
        drop(level);
        let stopped = session.is_stopped();
        SPEAKER.end(session);
        if stopped || crate::tts::is_tts_stop_requested() {
            Spoken::Status("TTS_STOPPED_BY_USER")
        } else {
            Spoken::Status("TTS_COMPLETED")
        }
    }

    /// Spawn the player thread for a session.
    fn start_player(
        session: &Arc<Session>,
        device: Option<&str>,
    ) -> Result<tokio::sync::mpsc::UnboundedReceiver<PlayerEvent>, String> {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let player_session = session.clone();
        let device_name = device.map(str::to_string);
        std::thread::Builder::new()
            .name("juno-voice-out".to_string())
            .spawn(move || run_player(player_session, device_name, tx))
            .map(|_| rx)
            .map_err(|e| format!("could not start the player: {e}"))
    }

    /// Play audio rendered ahead of time (the greeting) on the chosen speaker,
    /// with the same stop, hold and first-audio handling as a live sentence.
    pub async fn play_pcm(samples: &[f32], sample_rate: f64, device: Option<&str>) -> Spoken {
        let started = Instant::now();
        let session = Arc::new(Session::new());
        {
            let mut buffer = session.buffer();
            buffer.push(samples, sample_rate, Instant::now());
            buffer.finish_render();
        }
        match SPEAKER.adopt(session.clone(), crate::tts::is_tts_stop_requested) {
            Ok(()) => {}
            Err(StartError::Stopped) => return Spoken::Status("TTS_STOPPED_BY_USER"),
            Err(StartError::Failed(e)) => return Spoken::UseSay(e),
        }
        let rx = match start_player(&session, device) {
            Ok(rx) => rx,
            Err(e) => {
                SPEAKER.abandon(&session);
                return Spoken::UseSay(e);
            }
        };
        let detail = format!("prerendered_frames={}", samples.len());
        play_out(&session, rx, started, "prerendered", &detail, device).await
    }

    thread_local! {
        /// A second synthesizer for rendering ahead, so a stop aimed at live
        /// speech never cuts a render that will be cached. Main thread only.
        static RENDER_SYNTH: RefCell<Option<Retained<AVSpeechSynthesizer>>> = const { RefCell::new(None) };
    }

    /// Render a whole line to samples without playing it. `Err` when this
    /// path cannot honour the voice exactly (the caller uses `say -o`).
    pub async fn render_pcm(
        text: &str,
        stored_voice: Option<&str>,
        rate: f64,
    ) -> Result<(Vec<f32>, f64), String> {
        let app = APP.get().ok_or("not bound")?.clone();
        // The voice list arrives with the warm-up, a moment after launch.
        let deadline = Instant::now() + RENDER_WARM_WAIT;
        let voices = loop {
            if let Some(voices) = lock(&VOICES).clone() {
                break voices;
            }
            if Instant::now() >= deadline {
                return Err("the synthesizer did not warm up".to_string());
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        };
        let spoken = loop {
            if let Some(spoken) = current_spoken_content().await {
                break Some(spoken);
            }
            if Instant::now() >= deadline {
                break None;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        };
        let plan = plan_voice(stored_voice, spoken.as_ref(), &voices);
        if plan == VoicePlan::UseSay {
            return Err("no AVFoundation voice for this choice".to_string());
        }
        let session = Arc::new(Session::new());
        let utterance = Utterance {
            text: text.to_string(),
            voice: plan,
            av_rate: av_rate_for(rate),
        };
        let target = session.clone();
        app.run_on_main_thread(move || {
            RENDER_SYNTH.with(|cell| {
                let mut slot = cell.borrow_mut();
                // SAFETY: plain `+new` on the main thread.
                let synth = slot.get_or_insert_with(|| unsafe { AVSpeechSynthesizer::new() });
                start_render(synth, utterance, target);
            });
        })
        .map_err(|e| format!("could not reach the main thread: {e}"))?;

        let deadline = Instant::now() + RENDER_WHOLE_WAIT;
        loop {
            {
                let mut buffer = session.buffer();
                if buffer.is_stopped() {
                    return Err("the voice could not be loaded".to_string());
                }
                if buffer.is_rendered() {
                    return buffer
                        .take_all()
                        .ok_or_else(|| "the render was empty".to_string());
                }
            }
            if Instant::now() >= deadline {
                return Err("the render did not finish".to_string());
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }
}

/// The System Voice may have changed: read it again before the next sentence.
#[cfg(target_os = "macos")]
pub fn forget_spoken_content() {
    mac::forget_spoken_content();
}

#[cfg(not(target_os = "macos"))]
pub fn forget_spoken_content() {}

/// Keep the app handle and warm the synthesizer. Called once from setup.
#[cfg(target_os = "macos")]
pub fn bind(app: tauri::AppHandle) {
    mac::bind(app);
}

#[cfg(not(target_os = "macos"))]
pub fn bind(_app: tauri::AppHandle) {}

/// Stop the sentence speaking now and the render behind it. Every stop path
/// reaches this through `kill_audio_processes`.
#[cfg(target_os = "macos")]
pub fn stop_all() {
    mac::stop_all();
}

#[cfg(not(target_os = "macos"))]
pub fn stop_all() {}

/// Play samples rendered ahead on the chosen speaker and wait for them.
#[cfg(target_os = "macos")]
pub async fn play_pcm(samples: &[f32], sample_rate: f64, device: Option<&str>) -> Spoken {
    mac::play_pcm(samples, sample_rate, device).await
}

#[cfg(not(target_os = "macos"))]
pub async fn play_pcm(_samples: &[f32], _sample_rate: f64, _device: Option<&str>) -> Spoken {
    Spoken::UseSay("not macOS".to_string())
}

/// Render a line to samples without playing it.
#[cfg(target_os = "macos")]
pub async fn render_pcm(
    text: &str,
    voice: Option<&str>,
    rate: f64,
) -> Result<(Vec<f32>, f64), String> {
    mac::render_pcm(text, voice, rate).await
}

#[cfg(not(target_os = "macos"))]
pub async fn render_pcm(
    _text: &str,
    _voice: Option<&str>,
    _rate: f64,
) -> Result<(Vec<f32>, f64), String> {
    Err("not macOS".to_string())
}

/// Speak one sentence in-process and wait for it, or say why `say` should.
#[cfg(target_os = "macos")]
pub async fn speak(text: &str, voice: Option<&str>, device: Option<&str>, rate: f64) -> Spoken {
    mac::speak(text, voice, device, rate).await
}

#[cfg(not(target_os = "macos"))]
pub async fn speak(_text: &str, _voice: Option<&str>, _device: Option<&str>, _rate: f64) -> Spoken {
    Spoken::UseSay("not macOS".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

    fn voice(identifier: &str, name: &str, quality: VoiceQuality, language: &str) -> AvVoice {
        AvVoice {
            identifier: identifier.to_string(),
            name: name.to_string(),
            quality,
            language: language.to_string(),
        }
    }

    fn installed() -> Vec<AvVoice> {
        vec![
            voice(
                "com.apple.voice.compact.en-US.Samantha",
                "Samantha",
                VoiceQuality::Compact,
                "en-US",
            ),
            voice(
                "com.apple.voice.premium.en-US.Ava",
                "Ava",
                VoiceQuality::Premium,
                "en-US",
            ),
            voice(
                "com.apple.voice.enhanced.en-US.Ava",
                "Ava",
                VoiceQuality::Enhanced,
                "en-US",
            ),
            voice(
                "com.apple.eloquence.en-US.Eddy",
                "Eddy",
                VoiceQuality::Compact,
                "en-US",
            ),
            voice(
                "com.apple.eloquence.en-GB.Eddy",
                "Eddy",
                VoiceQuality::Compact,
                "en-GB",
            ),
        ]
    }

    // --- rate ---

    #[test]
    fn a_changed_system_voice_is_read_again() {
        let t0 = SystemTime::UNIX_EPOCH + Duration::from_secs(1000);
        let t1 = t0 + Duration::from_secs(2);
        let young = Duration::from_secs(1);
        // Nothing moved and the reading is young: reuse it.
        assert!(!spoken_is_stale(young, Some(t0), Some(t0)));
        assert!(!spoken_is_stale(young, None, None));
        // The preferences file was written since: read again, however young.
        assert!(spoken_is_stale(young, Some(t0), Some(t1)));
        assert!(spoken_is_stale(young, None, Some(t1)));
        assert!(spoken_is_stale(young, Some(t0), None));
        // An old reading is read again even if the file looks unchanged.
        assert!(spoken_is_stale(SPOKEN_FRESH_FOR, Some(t0), Some(t0)));
    }

    #[test]
    fn a_changed_system_voice_changes_the_plan() {
        let voices = installed();
        let siri = SpokenContent {
            voice_id: Some("com.apple.speech.synthesis.voice.custom.siri.nicky".into()),
            voice_name: None,
            language: Some("en".into()),
        };
        // Not buildable here: the best installed voice (Ava Premium) speaks.
        assert_eq!(
            plan_voice(None, Some(&siri), &voices),
            VoicePlan::Identifier("com.apple.voice.premium.en-US.Ava".to_string())
        );
        // The person picks another voice: the same call now follows it.
        let named = voices.first().expect("a voice").clone();
        let chosen = SpokenContent {
            voice_id: Some(named.identifier.clone()),
            voice_name: Some(named.name.clone()),
            language: Some("en".into()),
        };
        assert_eq!(
            plan_voice(None, Some(&chosen), &voices),
            VoicePlan::Identifier(named.identifier)
        );
    }

    #[test]
    fn normal_pace_is_avfoundations_default() {
        assert!((av_rate_for(1.0) - AV_DEFAULT_RATE).abs() < 1e-6);
    }

    #[test]
    fn the_measured_points_are_hit_exactly() {
        for &(multiplier, av) in RATE_TABLE {
            assert!((av_rate_for(multiplier) - av).abs() < 1e-6, "{multiplier}");
        }
    }

    #[test]
    fn faster_is_never_slower() {
        let mut last = 0.0f32;
        let mut rate = 0.5;
        while rate <= 2.5 {
            let av = av_rate_for(rate);
            assert!(av >= last, "{rate}");
            assert!((0.0..=1.0).contains(&av));
            last = av;
            rate += 0.01;
        }
    }

    #[test]
    fn a_corrupt_rate_speaks_at_normal_pace() {
        assert!((av_rate_for(f64::NAN) - AV_DEFAULT_RATE).abs() < 1e-6);
    }

    #[test]
    fn say_words_per_minute_map_back_to_the_multiplier() {
        assert!((rate_from_words_per_minute(None) - 1.0).abs() < 1e-9);
        assert!((rate_from_words_per_minute(Some(350)) - 2.0).abs() < 1e-9);
        assert!((rate_from_words_per_minute(Some(263)) - 1.5).abs() < 0.01);
    }

    // --- voice ---

    #[test]
    fn a_plain_name_finds_its_compact_voice() {
        assert_eq!(
            match_voice("Samantha", &installed()).as_deref(),
            Some("com.apple.voice.compact.en-US.Samantha")
        );
    }

    #[test]
    fn the_quality_in_the_name_picks_the_version() {
        assert_eq!(
            match_voice("Ava (Premium)", &installed()).as_deref(),
            Some("com.apple.voice.premium.en-US.Ava")
        );
        assert_eq!(
            match_voice("Ava (Enhanced)", &installed()).as_deref(),
            Some("com.apple.voice.enhanced.en-US.Ava")
        );
    }

    #[test]
    fn a_version_that_is_not_installed_is_not_substituted() {
        assert_eq!(match_voice("Samantha (Premium)", &installed()), None);
    }

    #[test]
    fn an_ambiguous_name_is_left_to_say() {
        assert_eq!(match_voice("Eddy (English (US))", &installed()), None);
    }

    #[test]
    fn a_missing_voice_goes_to_say() {
        assert_eq!(
            plan_voice(Some("Zarvox"), None, &installed()),
            VoicePlan::UseSay
        );
    }

    #[test]
    fn the_macs_own_voice_unset_is_the_language_default() {
        let spoken = SpokenContent::default();
        assert_eq!(
            plan_voice(None, Some(&spoken), &installed()),
            VoicePlan::LanguageDefault
        );
    }

    #[test]
    fn a_siri_system_voice_speaks_with_the_best_installed_voice() {
        let spoken = SpokenContent {
            voice_id: Some("com.apple.siri.natural.Simone".to_string()),
            voice_name: None,
            language: Some("en".to_string()),
        };
        // `installed()` has Ava Premium: it outranks the compact voices.
        assert_eq!(
            plan_voice(None, Some(&spoken), &installed()),
            VoicePlan::Identifier("com.apple.voice.premium.en-US.Ava".to_string())
        );
    }

    #[test]
    fn the_fallback_ranks_premium_then_enhanced_then_compact() {
        let compact = voice(
            "com.apple.voice.compact.en-US.Samantha",
            "Samantha",
            VoiceQuality::Compact,
            "en-US",
        );
        let enhanced = voice(
            "com.apple.voice.enhanced.en-US.Zoe",
            "Zoe",
            VoiceQuality::Enhanced,
            "en-US",
        );
        let premium = voice(
            "com.apple.voice.premium.en-GB.Serena",
            "Serena",
            VoiceQuality::Premium,
            "en-GB",
        );
        let french = voice(
            "com.apple.voice.premium.fr-FR.Amelie",
            "Amelie",
            VoiceQuality::Premium,
            "fr-FR",
        );
        let novelty = voice(
            "com.apple.speech.synthesis.voice.Albert",
            "Albert",
            VoiceQuality::Compact,
            "en-US",
        );
        let mut all = vec![
            novelty.clone(),
            compact.clone(),
            french.clone(),
            enhanced.clone(),
        ];
        let pick = |all: &[AvVoice]| best_voice(all, Some("en")).map(|v| v.identifier.clone());
        assert_eq!(pick(&all), Some(enhanced.identifier.clone()));
        all.push(premium.clone());
        assert_eq!(pick(&all), Some(premium.identifier.clone()));
        // Only compact left: Samantha beats a novelty voice, in any order.
        let compacts = vec![novelty.clone(), compact.clone()];
        assert_eq!(pick(&compacts), Some(compact.identifier.clone()));
        let reversed = vec![compact.clone(), novelty.clone()];
        assert_eq!(pick(&reversed), Some(compact.identifier.clone()));
        // Another language ranks within itself, and falls back to any voice.
        assert_eq!(
            best_voice(&all, Some("fr")).map(|v| v.identifier.clone()),
            Some(french.identifier.clone())
        );
        assert_eq!(
            best_voice(&compacts, Some("de")).map(|v| v.identifier.clone()),
            Some(compact.identifier.clone())
        );
        assert!(best_voice(&[], Some("en")).is_none());
    }

    #[test]
    fn a_siri_system_voice_with_no_voices_is_the_language_default() {
        let spoken = SpokenContent {
            voice_id: Some("com.apple.siri.natural.Simone".to_string()),
            ..SpokenContent::default()
        };
        assert_eq!(
            plan_voice(None, Some(&spoken), &[]),
            VoicePlan::LanguageDefault
        );
    }

    #[test]
    fn a_named_system_voice_is_used_by_identifier() {
        let spoken = SpokenContent {
            voice_id: Some("com.apple.voice.premium.en-US.Ava".to_string()),
            voice_name: Some("Ava".to_string()),
            language: Some("en".to_string()),
        };
        assert_eq!(
            plan_voice(None, Some(&spoken), &installed()),
            VoicePlan::Identifier("com.apple.voice.premium.en-US.Ava".to_string())
        );
    }

    #[test]
    fn an_unread_system_voice_assumes_nothing() {
        assert_eq!(plan_voice(None, None, &installed()), VoicePlan::UseSay);
    }

    // --- playback buffer ---

    fn now() -> Instant {
        Instant::now()
    }

    #[test]
    fn nothing_rendered_is_waiting_and_silent() {
        let mut buffer = PlaybackBuffer::new();
        let mut out = [1.0f32; 8];
        assert_eq!(buffer.fill(&mut out, 22_050.0, false), 0);
        assert!(out.iter().all(|&s| s == 0.0));
        assert_eq!(buffer.state(now()), PlaybackState::Waiting);
    }

    #[test]
    fn samples_play_in_order_at_the_same_rate() {
        let mut buffer = PlaybackBuffer::new();
        buffer.push(&[0.1, 0.2, 0.3, 0.4], 22_050.0, now());
        buffer.finish_render();
        let mut out = [0.0f32; 4];
        assert_eq!(buffer.fill(&mut out, 22_050.0, false), 4);
        assert_eq!(out, [0.1, 0.2, 0.3, 0.4]);
        assert_eq!(buffer.state(now()), PlaybackState::Done);
    }

    #[test]
    fn a_faster_device_gets_more_frames() {
        let mut buffer = PlaybackBuffer::new();
        buffer.push(&vec![0.5; 2_205], 22_050.0, now());
        buffer.finish_render();
        let mut out = vec![0.0f32; 10_000];
        let written = buffer.fill(&mut out, 48_000.0, false);
        // 0.1 s of speech is 4,800 frames at 48 kHz.
        assert!((4_799..=4_801).contains(&written), "{written}");
    }

    #[test]
    fn held_is_silent_and_resumes_from_the_same_sample() {
        let mut buffer = PlaybackBuffer::new();
        buffer.push(&[0.1, 0.2, 0.3, 0.4], 22_050.0, now());
        buffer.finish_render();
        let mut out = [0.0f32; 2];
        buffer.fill(&mut out, 22_050.0, false);
        assert_eq!(out, [0.1, 0.2]);

        let mut held = [1.0f32; 2];
        assert_eq!(buffer.fill(&mut held, 22_050.0, true), 0);
        assert_eq!(held, [0.0, 0.0]);
        assert_eq!(buffer.state(now()), PlaybackState::Playing);

        buffer.fill(&mut out, 22_050.0, false);
        assert_eq!(out, [0.3, 0.4]);
    }

    #[test]
    fn a_stop_silences_the_next_callback_and_ends_playback() {
        let mut buffer = PlaybackBuffer::new();
        buffer.push(&[0.5; 100], 22_050.0, now());
        buffer.stop();
        let mut out = [1.0f32; 10];
        assert_eq!(buffer.fill(&mut out, 22_050.0, false), 0);
        assert!(out.iter().all(|&s| s == 0.0));
        assert_eq!(buffer.state(now()), PlaybackState::Done);
        // A late buffer from the cancelled render is not played.
        buffer.push(&[0.5; 10], 22_050.0, now());
        assert_eq!(buffer.fill(&mut out, 22_050.0, false), 0);
    }

    #[test]
    fn a_render_still_running_waits_rather_than_ending() {
        let mut buffer = PlaybackBuffer::new();
        buffer.push(&[0.1, 0.2], 22_050.0, now());
        let mut out = [0.0f32; 8];
        buffer.fill(&mut out, 22_050.0, false);
        assert_eq!(buffer.state(now()), PlaybackState::Playing);
        buffer.push(&[0.3], 22_050.0, now());
        buffer.finish_render();
        buffer.fill(&mut out, 22_050.0, false);
        assert_eq!(buffer.state(now()), PlaybackState::Done);
    }

    #[test]
    fn a_render_that_goes_quiet_without_its_end_marker_still_ends() {
        let mut buffer = PlaybackBuffer::new();
        let pushed = now();
        buffer.push(&[0.1], 22_050.0, pushed);
        let mut out = [0.0f32; 4];
        buffer.fill(&mut out, 22_050.0, false);
        assert_eq!(buffer.state(pushed), PlaybackState::Playing);
        assert_eq!(buffer.state(pushed + RENDER_STALL), PlaybackState::Done);
    }

    // --- sessions ---

    #[derive(Default)]
    struct FakeSynth {
        renders: StdMutex<Vec<String>>,
        cancels: AtomicU64,
        refuse: AtomicBool,
    }

    impl Synth for FakeSynth {
        fn render(&self, utterance: Utterance, session: Arc<Session>) -> Result<(), String> {
            if self.refuse.load(Ordering::SeqCst) {
                return Err("refused".to_string());
            }
            self.renders
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .push(utterance.text);
            session.push(&[0.25; 64], 22_050.0);
            session.finish_render();
            Ok(())
        }

        fn cancel(&self) {
            self.cancels.fetch_add(1, Ordering::SeqCst);
        }
    }

    fn utterance(text: &str) -> Utterance {
        Utterance {
            text: text.to_string(),
            voice: VoicePlan::LanguageDefault,
            av_rate: AV_DEFAULT_RATE,
        }
    }

    #[test]
    fn a_sentence_renders_and_becomes_current() {
        let speaker = Speaker::new(FakeSynth::default());
        let session = speaker
            .begin(utterance("Hello."), || false)
            .expect("starts");
        assert!(speaker.is_current(&session));
        assert_eq!(
            session.buffer().state(Instant::now()),
            PlaybackState::Playing
        );
        speaker.end(&session);
        assert!(!speaker.is_current(&session));
    }

    #[test]
    fn escape_stops_the_sentence_and_the_render() {
        let speaker = Speaker::new(FakeSynth::default());
        let session = speaker.begin(utterance("Long."), || false).expect("starts");
        assert!(speaker.stop_all());
        assert!(session.is_stopped());
        assert_eq!(session.buffer().state(Instant::now()), PlaybackState::Done);
        assert_eq!(speaker.synth.cancels.load(Ordering::SeqCst), 1);
        assert!(!speaker.stop_all(), "nothing left to stop");
    }

    #[test]
    fn a_stop_that_lands_before_the_start_wins() {
        let speaker = Speaker::new(FakeSynth::default());
        let result = speaker.begin(utterance("Too late."), || true);
        assert_eq!(result.err(), Some(StartError::Stopped));
        assert!(speaker
            .synth
            .renders
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .is_empty());
        assert!(!speaker.stop_all());
    }

    #[test]
    fn a_new_sentence_stops_the_one_before() {
        let speaker = Speaker::new(FakeSynth::default());
        let first = speaker.begin(utterance("One."), || false).expect("starts");
        let second = speaker.begin(utterance("Two."), || false).expect("starts");
        assert!(first.is_stopped());
        assert!(!second.is_stopped());
        assert!(speaker.is_current(&second));
        // The first one ending late must not clear the second.
        speaker.end(&first);
        assert!(speaker.is_current(&second));
    }

    #[test]
    fn a_refused_render_leaves_nothing_current() {
        let speaker = Speaker::new(FakeSynth::default());
        speaker.synth.refuse.store(true, Ordering::SeqCst);
        let result = speaker.begin(utterance("Hello."), || false);
        assert!(matches!(result, Err(StartError::Failed(_))));
        assert!(!speaker.stop_all());
    }

    #[test]
    fn audio_rendered_ahead_is_adopted_and_stopped_like_live_speech() {
        let speaker = Speaker::new(FakeSynth::default());
        let live = speaker.begin(utterance("Live."), || false).expect("starts");
        let ahead = Arc::new(Session::new());
        ahead.push(&[0.5; 32], 22_050.0);
        ahead.finish_render();
        speaker.adopt(ahead.clone(), || false).expect("adopts");
        assert!(live.is_stopped(), "only one thing speaks at a time");
        assert!(speaker.is_current(&ahead));
        assert!(speaker.stop_all());
        assert!(ahead.is_stopped());
        assert!(speaker
            .synth
            .renders
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .iter()
            .all(|text| text == "Live."));
    }

    #[test]
    fn adopting_after_a_stop_request_plays_nothing() {
        let speaker = Speaker::new(FakeSynth::default());
        let ahead = Arc::new(Session::new());
        assert_eq!(
            speaker.adopt(ahead.clone(), || true),
            Err(StartError::Stopped)
        );
        assert!(ahead.is_stopped());
        assert!(!speaker.stop_all());
    }

    #[test]
    fn a_whole_render_can_be_taken_for_the_cache() {
        let mut buffer = PlaybackBuffer::new();
        assert_eq!(buffer.take_all(), None);
        buffer.push(&[0.1, 0.2], 22_050.0, now());
        buffer.push(&[0.3], 22_050.0, now());
        buffer.finish_render();
        assert!(buffer.is_rendered());
        assert_eq!(buffer.take_all(), Some((vec![0.1, 0.2, 0.3], 22_050.0)));
    }

    #[test]
    fn abandoning_hands_the_sentence_back_silently() {
        let speaker = Speaker::new(FakeSynth::default());
        let session = speaker
            .begin(utterance("Hello."), || false)
            .expect("starts");
        speaker.abandon(&session);
        assert!(session.is_stopped());
        assert!(!speaker.is_current(&session));
        let mut out = [1.0f32; 4];
        assert_eq!(session.buffer().fill(&mut out, 22_050.0, false), 0);
    }
}
