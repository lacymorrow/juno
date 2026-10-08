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
use std::time::{Duration, Instant};

/// How long the first buffer of a sentence may take before `say` is used
/// instead. A warm synthesizer answers in about 30 ms and a voice it has not
/// loaded yet in about 250 ms; a cold one takes about a second.
pub const FIRST_BUFFER_TIMEOUT: Duration = Duration::from_millis(2_500);

/// A render that stopped sending buffers without its zero-length end marker
/// is treated as finished once everything it sent has played and this long
/// has passed. Rendering runs many times faster than real time, so a real
/// sentence never pauses this long between buffers.
pub const RENDER_STALL: Duration = Duration::from_millis(1_500);

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
    match crate::tts::voices::classify_system_voice(
        spoken.voice_id.as_deref(),
        spoken.voice_name.as_deref(),
    ) {
        crate::tts::voices::SystemVoice::Unset => VoicePlan::LanguageDefault,
        crate::tts::voices::SystemVoice::Siri => VoicePlan::UseSay,
        crate::tts::voices::SystemVoice::Other => {
            let by_id = spoken
                .voice_id
                .as_deref()
                .and_then(|id| voices.iter().find(|voice| voice.identifier == id));
            match by_id {
                Some(voice) => VoicePlan::Identifier(voice.identifier.clone()),
                None => VoicePlan::UseSay,
            }
        }
    }
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

    /// The Spoken Content voice, and when it was read.
    static SPOKEN: StdMutex<Option<(Instant, SpokenContent)>> = StdMutex::new(None);
    const SPOKEN_TTL: Duration = Duration::from_secs(60);

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
            with_synth(|synth| {
                synth.writeUtterance_toBufferCallback(&av, RcBlock::as_ptr(&block));
            });
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
        fn read(key: &str) -> Option<String> {
            let output = std::process::Command::new("defaults")
                .args(["read", "com.apple.speech.voice.prefs", key])
                .output()
                .ok()?;
            output
                .status
                .success()
                .then(|| String::from_utf8_lossy(&output.stdout).trim().to_string())
                .filter(|v| !v.is_empty())
        }
        SpokenContent {
            voice_id: read("SelectedVoiceID"),
            voice_name: read("SelectedVoiceName"),
        }
    }

    /// The Spoken Content voice as last read. A stale value is used as is and
    /// refreshed in the background, so no sentence waits on `defaults`.
    fn spoken_content() -> Option<SpokenContent> {
        let cached = lock(&SPOKEN).clone();
        let stale = cached
            .as_ref()
            .is_none_or(|(at, _)| at.elapsed() >= SPOKEN_TTL);
        if stale {
            tauri::async_runtime::spawn(async {
                if let Ok(spoken) = tokio::task::spawn_blocking(read_spoken_content).await {
                    *lock(&SPOKEN) = Some((Instant::now(), spoken));
                }
            });
        }
        cached.map(|(_, spoken)| spoken)
    }

    /// Keep the app handle, read the voices and load the synthesizer, off
    /// the critical path. The first render in a process costs about a second
    /// (measured 0.7 to 1.1 s), so it happens here, silently, rather than on
    /// the first sentence.
    pub fn bind(app: AppHandle) {
        if APP.set(app.clone()).is_err() {
            return;
        }
        let _ = spoken_content();
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
        let plan = plan_voice(stored_voice, spoken_content().as_ref(), &voices);
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

        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let player_session = session.clone();
        let device_name = device.map(str::to_string);
        if let Err(e) = std::thread::Builder::new()
            .name("juno-voice-out".to_string())
            .spawn(move || run_player(player_session, device_name, tx))
        {
            SPEAKER.abandon(&session);
            return Spoken::UseSay(format!("could not start the player: {e}"));
        }

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
                        "[SpeechTiming] engine=avspeech first_audio_ms={} first_buffer_ms={} device_open_ms={} chars={} voice={} rate={:.2} device={}",
                        started.elapsed().as_millis(),
                        render_ms,
                        device_open_ms,
                        chars,
                        match &plan {
                            VoicePlan::Identifier(id) => id.as_str(),
                            _ => "default",
                        },
                        rate,
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
        SPEAKER.end(&session);
        if stopped || crate::tts::is_tts_stop_requested() {
            Spoken::Status("TTS_STOPPED_BY_USER")
        } else {
            Spoken::Status("TTS_COMPLETED")
        }
    }
}

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
    fn a_siri_system_voice_stays_with_say() {
        let spoken = SpokenContent {
            voice_id: Some("com.apple.speech.synthesis.voice.custom.siri.aaron".to_string()),
            voice_name: Some("Siri Voice 2".to_string()),
        };
        assert_eq!(
            plan_voice(None, Some(&spoken), &installed()),
            VoicePlan::UseSay
        );
    }

    #[test]
    fn a_named_system_voice_is_used_by_identifier() {
        let spoken = SpokenContent {
            voice_id: Some("com.apple.voice.premium.en-US.Ava".to_string()),
            voice_name: Some("Ava".to_string()),
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
