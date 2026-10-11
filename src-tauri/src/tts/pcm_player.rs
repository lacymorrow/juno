//! Juno's in-process player for audio rendered ahead of time.
//!
//! The launch greeting is rendered before it is needed (`prerender.rs`) and
//! played from here the moment the bar appears. This module only plays
//! samples; it never synthesises speech. The Mac's own voice is always `say`
//! with no `-v` (see `system.rs`), because that is the only way a third-party
//! app can speak with a Siri voice: AVFoundation lists no Siri voices to apps.
//!
//! # How a line plays
//!
//! 1. The samples go into a [`Session`]'s [`PlaybackBuffer`], and a cpal output
//!    stream on the speaker chosen in Settings pulls them out, resampled to the
//!    device's rate. That is how the Speaker setting applies to it.
//! 2. A stop marks the session stopped, and the next audio callback is silence.
//!
//! # What a `say` pid would give, and where it comes from here
//!
//! - "audio started" ([`crate::turn_timing::Stage::FirstAudio`]): the first
//!   frame the device actually plays;
//! - the echo hold (#720): the audio callback plays silence and does not
//!   advance while [`crate::tts::is_held_silent`] is true, so Juno picks up
//!   mid-word when the microphone closes, as `SIGCONT` does for `say`;
//! - Escape: [`stop_all`], reached from the one place every stop goes through
//!   (`kill_audio_processes`).
//!
//! The playback buffer and the session that stop and start race on are plain
//! Rust, tested without a speaker.

#![cfg_attr(not(target_os = "macos"), allow(dead_code))]

use std::collections::VecDeque;
use std::sync::{Arc, Mutex as StdMutex, MutexGuard};
use std::time::{Duration, Instant};

/// A render that stopped sending samples without its end marker is treated
/// as finished once everything it sent has played and this long has passed.
pub const RENDER_STALL: Duration = Duration::from_millis(1_500);

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

/// The samples of one line, between the renderer and the device.
///
/// Stores mono samples at the rendered rate and resamples linearly as the
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

    /// Samples from the render. Ignored once stopped.
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

    /// Nothing more is coming.
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

    /// Everything rendered, at its own rate, for a render nobody
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

/// One line in flight. Shared by whoever fills it, the audio callback (which
/// pulls) and whoever stops it.
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

// ---------------------------------------------------------------------------
// What is playing now
// ---------------------------------------------------------------------------

/// The one line playing now.
///
/// Start and stop race: Escape can land between the caller's stop check and
/// the session becoming current. `stop_speech` sets the stop flag and then
/// takes the current session under this lock; [`Speaker::adopt`] makes its
/// session current under the same lock and then reads the flag. Either the
/// stop sees the session or the session sees the flag.
#[derive(Debug, Default)]
pub struct Speaker {
    current: StdMutex<Option<Arc<Session>>>,
}

impl Speaker {
    pub const fn new() -> Self {
        Self {
            current: StdMutex::new(None),
        }
    }

    fn current(&self) -> MutexGuard<'_, Option<Arc<Session>>> {
        self.current
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Make a filled session current, stopping whatever was playing: only
    /// one thing plays at a time. False when a stop was requested first, and
    /// then nothing plays.
    pub fn adopt(&self, session: Arc<Session>, stop_requested: impl Fn() -> bool) -> bool {
        let previous = self.current().replace(session.clone());
        if let Some(previous) = previous {
            previous.stop();
        }
        if stop_requested() {
            self.end(&session);
            session.stop();
            return false;
        }
        true
    }

    /// Stop whatever is playing. True when something was.
    pub fn stop_all(&self) -> bool {
        match self.current().take() {
            Some(session) => {
                session.stop();
                true
            }
            None => false,
        }
    }

    /// Give up on a line that could not reach the speaker.
    pub fn abandon(&self, session: &Arc<Session>) {
        session.stop();
        self.end(session);
    }

    /// The line is over. Only clears it if it is still the current one.
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

/// What [`play_pcm`] did with a line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Spoken {
    /// Played or stopped: a `TTS_*` status for the caller.
    Status(&'static str),
    /// Could not be played. The reason is for the log only.
    Unplayable(String),
}

// ---------------------------------------------------------------------------
// macOS
// ---------------------------------------------------------------------------

#[cfg(target_os = "macos")]
mod mac {
    use super::*;
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
    use tracing::{info, warn};

    static SPEAKER: Speaker = Speaker::new();

    pub fn stop_all() {
        if SPEAKER.stop_all() {
            info!("[Player] Stopped playing");
        }
    }

    /// What the player thread reports.
    enum PlayerEvent {
        /// The first samples are in the buffer.
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
            warn!("[Player] Speaker {name} is not connected; using the system output");
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
                |e| warn!("[Player] Output stream error: {e}"),
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
                    warn!("[Player] Playback failed: {e}");
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
    /// with the same stop, hold and first-audio handling as live speech.
    pub async fn play_pcm(samples: &[f32], sample_rate: f64, device: Option<&str>) -> Spoken {
        let started = Instant::now();
        let session = Arc::new(Session::new());
        {
            let mut buffer = session.buffer();
            buffer.push(samples, sample_rate, Instant::now());
            buffer.finish_render();
        }
        if !SPEAKER.adopt(session.clone(), crate::tts::is_tts_stop_requested) {
            return Spoken::Status("TTS_STOPPED_BY_USER");
        }
        let rx = match start_player(&session, device) {
            Ok(rx) => rx,
            Err(e) => {
                SPEAKER.abandon(&session);
                return Spoken::Unplayable(e);
            }
        };
        let detail = format!("prerendered_frames={}", samples.len());
        play_out(&session, rx, started, "prerendered", &detail, device).await
    }
}

/// Stop the line playing now. Every stop path reaches this through
/// `kill_audio_processes`.
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
    Spoken::Unplayable("not macOS".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

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

    fn filled() -> Arc<Session> {
        let session = Arc::new(Session::new());
        session.push(&[0.5; 32], 22_050.0);
        session.finish_render();
        session
    }

    #[test]
    fn a_line_is_adopted_and_stopped_like_live_speech() {
        let speaker = Speaker::new();
        let line = filled();
        assert!(speaker.adopt(line.clone(), || false));
        assert!(speaker.is_current(&line));
        assert!(speaker.stop_all());
        assert!(line.is_stopped());
        assert_eq!(line.buffer().state(Instant::now()), PlaybackState::Done);
        assert!(!speaker.stop_all(), "nothing left to stop");
    }

    #[test]
    fn a_new_line_stops_the_one_before() {
        let speaker = Speaker::new();
        let first = filled();
        let second = filled();
        assert!(speaker.adopt(first.clone(), || false));
        assert!(speaker.adopt(second.clone(), || false));
        assert!(first.is_stopped(), "only one thing plays at a time");
        assert!(!second.is_stopped());
        // The first one ending late must not clear the second.
        speaker.end(&first);
        assert!(speaker.is_current(&second));
    }

    #[test]
    fn adopting_after_a_stop_request_plays_nothing() {
        let speaker = Speaker::new();
        let line = filled();
        assert!(!speaker.adopt(line.clone(), || true));
        assert!(line.is_stopped());
        assert!(!speaker.stop_all());
    }

    #[test]
    fn abandoning_silences_the_line() {
        let speaker = Speaker::new();
        let line = filled();
        assert!(speaker.adopt(line.clone(), || false));
        speaker.abandon(&line);
        assert!(line.is_stopped());
        assert!(!speaker.is_current(&line));
        let mut out = [1.0f32; 4];
        assert_eq!(line.buffer().fill(&mut out, 22_050.0, false), 0);
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
}
