//! How loud Juno's voice is right now, for any surface that wants to move
//! with it (the Avatar's mouth first).
//!
//! One session per audible utterance. A session is opened where the sound
//! actually leaves Juno (the afplay child for engines that hand back audio,
//! the `say` child for the Mac's own voice) and closed when that process
//! exits or is killed, so the mouth can never run ahead of the speaker or
//! keep moving after Escape.
//!
//! - `SPEECH_STARTED` fires when sound begins, not when synthesis starts: the
//!   first frame of the envelope that is above the start threshold.
//! - `SPEECH_LEVEL` is a smoothed 0..1 envelope at about 60 Hz while it plays.
//! - `SPEECH_ENDED` fires once, only for a session that started.
//!
//! Where the samples are known (WAV from Kokoro and Supertonic) the envelope
//! is their RMS, frame by frame, walked on the playback clock. Where they are
//! not (`say`, and compressed audio from the cloud engines), a synthetic
//! syllable rhythm stands in, gated by the same real start and finish.

use serde::Serialize;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

use crate::constants::events::tts as events;

/// Frames per second of the level stream.
pub const LEVEL_HZ: u32 = 60;
/// Normalised level a frame must reach before the speech counts as started.
pub const START_LEVEL: f32 = 0.08;
/// Per-frame smoothing toward a louder target (fast, so a syllable opens the
/// mouth at once) and toward a quieter one (slower, so it closes softly).
pub const ATTACK: f32 = 0.6;
pub const RELEASE: f32 = 0.3;
/// Below this the smoothed level is reported as silence.
pub const SILENCE: f32 = 0.01;
/// How long afplay takes from spawn to its first sample. An estimate: afplay
/// gives no signal when sound begins. Confirm on hardware.
pub const PLAYER_LEAD_MS: u64 = 60;
/// How long `say` takes from spawn to its first sound, measured roughly as a
/// tenth of a second (see `system::speak_directly`).
pub const SAY_LEAD_MS: u64 = 120;
/// A clip whose loudest frame is quieter than this is treated as silence
/// rather than amplified into a moving mouth.
const PEAK_FLOOR: f32 = 1e-3;

// === THE ENVELOPE ===

/// Root-mean-square of each 1/`frame_hz` second of `samples`, normalised so
/// the loudest frame is 1.0. Silence stays 0.0.
pub fn rms_envelope(samples: &[f32], sample_rate: u32, frame_hz: u32) -> Vec<f32> {
    if samples.is_empty() || sample_rate == 0 || frame_hz == 0 {
        return Vec::new();
    }
    let frame_len = ((sample_rate / frame_hz) as usize).max(1);
    let rms: Vec<f32> = samples
        .chunks(frame_len)
        .map(|frame| {
            let sum: f32 = frame.iter().map(|s| s * s).sum();
            (sum / frame.len() as f32).sqrt()
        })
        .collect();
    let peak = rms.iter().cloned().fold(0.0_f32, f32::max);
    if peak < PEAK_FLOOR {
        return vec![0.0; rms.len()];
    }
    rms.into_iter()
        .map(|r| (r / peak).clamp(0.0, 1.0))
        .collect()
}

/// Decode WAV bytes to mono f32 samples and their rate. `None` for anything
/// that is not WAV (MP3, AAC), which then gets the synthetic envelope.
pub fn decode_wav_mono(bytes: &[u8]) -> Option<(Vec<f32>, u32)> {
    let reader = hound::WavReader::new(std::io::Cursor::new(bytes)).ok()?;
    let spec = reader.spec();
    let channels = spec.channels.max(1) as usize;
    let interleaved: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => reader
            .into_samples::<f32>()
            .collect::<Result<Vec<_>, _>>()
            .ok()?,
        hound::SampleFormat::Int => {
            let bits = spec.bits_per_sample.clamp(1, 32) as i32;
            let scale = (1_i64 << (bits - 1)) as f32;
            reader
                .into_samples::<i32>()
                .map(|s| s.map(|v| v as f32 / scale))
                .collect::<Result<Vec<_>, _>>()
                .ok()?
        }
    };
    let mono = interleaved
        .chunks(channels)
        .map(|frame| frame.iter().sum::<f32>() / frame.len() as f32)
        .collect();
    Some((mono, spec.sample_rate))
}

/// A plausible talking rhythm for audio whose samples Juno cannot see: about
/// four and a half syllables a second, rising and falling over a phrase.
/// Starts closed at t = 0 and stays inside 0..1.
pub fn synthetic_level(t_secs: f32) -> f32 {
    use std::f32::consts::TAU;
    let t = t_secs.max(0.0);
    let syllable = 0.5 - 0.5 * (TAU * 4.5 * t).cos();
    let phrase = 0.7 + 0.3 * (TAU * 1.3 * t + 0.6).sin();
    (syllable * phrase).clamp(0.0, 1.0)
}

/// Where a session's raw level comes from.
#[derive(Debug, Clone)]
pub enum LevelSource {
    /// Measured: one normalised RMS frame per 1/`LEVEL_HZ` second.
    Envelope(Arc<Vec<f32>>),
    /// Samples unseen: the synthetic rhythm, for as long as the process plays.
    Synthetic,
}

impl LevelSource {
    /// The measured envelope when the bytes are WAV, the rhythm otherwise.
    pub fn from_audio_bytes(bytes: &[u8]) -> Self {
        match decode_wav_mono(bytes) {
            Some((samples, rate)) => {
                LevelSource::Envelope(Arc::new(rms_envelope(&samples, rate, LEVEL_HZ)))
            }
            None => LevelSource::Synthetic,
        }
    }

    /// The raw level `elapsed` into playback. Past the end of a measured clip
    /// it is silence, so the mouth closes even if the player lingers.
    pub fn level_at(&self, elapsed: Duration) -> f32 {
        match self {
            LevelSource::Envelope(frames) => {
                let index = (elapsed.as_secs_f64() * LEVEL_HZ as f64) as usize;
                frames.get(index).copied().unwrap_or(0.0)
            }
            LevelSource::Synthetic => synthetic_level(elapsed.as_secs_f32()),
        }
    }
}

// === THE GATE ===

/// What one frame of the gate says to emit.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GateFrame {
    /// Sound began on this frame: emit `SPEECH_STARTED` before the level.
    pub started_now: bool,
    /// The smoothed level to emit, or `None` before the sound begins.
    pub level: Option<f32>,
}

/// Turns raw levels into the event stream: nothing until sound begins, then a
/// smoothed level each frame, and an end only for a session that started.
#[derive(Debug, Default, Clone)]
pub struct SpeechGate {
    started: bool,
    smoothed: f32,
}

impl SpeechGate {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn started(&self) -> bool {
        self.started
    }

    pub fn step(&mut self, raw: f32) -> GateFrame {
        let raw = if raw.is_finite() {
            raw.clamp(0.0, 1.0)
        } else {
            0.0
        };
        let mut started_now = false;
        if !self.started {
            if raw < START_LEVEL {
                return GateFrame {
                    started_now: false,
                    level: None,
                };
            }
            self.started = true;
            started_now = true;
        }
        let k = if raw > self.smoothed { ATTACK } else { RELEASE };
        self.smoothed += (raw - self.smoothed) * k;
        if self.smoothed < SILENCE {
            self.smoothed = 0.0;
        }
        GateFrame {
            started_now,
            level: Some(self.smoothed),
        }
    }

    /// The player stopped. True when `SPEECH_ENDED` should be emitted.
    pub fn finish(&mut self) -> bool {
        let was = self.started;
        self.started = false;
        self.smoothed = 0.0;
        was
    }
}

// === THE STREAM ===

#[derive(Debug, Clone, Serialize)]
pub struct SpeechLevelPayload {
    pub session: u64,
    pub level: f32,
}

#[derive(Debug, Clone, Serialize)]
pub struct SpeechSessionPayload {
    pub session: u64,
}

static APP: OnceLock<AppHandle> = OnceLock::new();
static NEXT_SESSION: AtomicU64 = AtomicU64::new(1);

/// Give the stream somewhere to go. Called once at startup; before it, and in
/// the CLI, sessions are silent no-ops.
pub fn bind(app: AppHandle) {
    let _ = APP.set(app);
}

/// An open session. Dropping it (the player exited, was killed, or the call
/// errored) ends the stream.
#[derive(Debug)]
pub struct SpeechLevelSession {
    stop: Arc<AtomicBool>,
}

impl Drop for SpeechLevelSession {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
    }
}

/// Start streaming levels for one utterance whose player was just spawned.
/// `lead` is how long that player takes to make its first sound.
pub fn begin(source: LevelSource, lead: Duration) -> SpeechLevelSession {
    let stop = Arc::new(AtomicBool::new(false));
    let session = SpeechLevelSession { stop: stop.clone() };
    let Some(app) = APP.get().cloned() else {
        return session;
    };
    let id = NEXT_SESSION.fetch_add(1, Ordering::Relaxed);
    let audible_at = Instant::now() + lead;
    tauri::async_runtime::spawn(async move {
        let mut gate = SpeechGate::new();
        let mut ticker = tokio::time::interval(Duration::from_micros(1_000_000 / LEVEL_HZ as u64));
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let mut last_sent: Option<f32> = None;
        loop {
            ticker.tick().await;
            if stop.load(Ordering::SeqCst) {
                break;
            }
            let now = Instant::now();
            if now < audible_at {
                continue;
            }
            let frame = gate.step(source.level_at(now - audible_at));
            if frame.started_now {
                let _ = app.emit(events::SPEECH_STARTED, SpeechSessionPayload { session: id });
            }
            if let Some(level) = frame.level {
                // Silence held is sent once, not sixty times a second.
                let held_silence = level == 0.0 && last_sent == Some(0.0);
                if !held_silence {
                    let _ = app.emit(
                        events::SPEECH_LEVEL,
                        SpeechLevelPayload { session: id, level },
                    );
                    last_sent = Some(level);
                }
            }
        }
        if gate.finish() {
            let _ = app.emit(
                events::SPEECH_LEVEL,
                SpeechLevelPayload {
                    session: id,
                    level: 0.0,
                },
            );
            let _ = app.emit(events::SPEECH_ENDED, SpeechSessionPayload { session: id });
        }
    });
    session
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(freq: f32, amp: f32, rate: u32, secs: f32) -> Vec<f32> {
        let n = (rate as f32 * secs) as usize;
        (0..n)
            .map(|i| amp * (std::f32::consts::TAU * freq * i as f32 / rate as f32).sin())
            .collect()
    }

    #[test]
    fn silence_has_a_flat_closed_envelope() {
        let env = rms_envelope(&sine(220.0, 0.0, 24_000, 1.0), 24_000, LEVEL_HZ);
        assert_eq!(env.len(), 60);
        assert!(env.iter().all(|&v| v == 0.0));
    }

    #[test]
    fn one_frame_per_sixtieth_of_a_second() {
        let env = rms_envelope(&sine(220.0, 0.5, 24_000, 0.5), 24_000, LEVEL_HZ);
        assert_eq!(env.len(), 30);
    }

    #[test]
    fn the_loudest_frame_is_one_and_quiet_frames_are_lower() {
        let mut samples = sine(220.0, 0.1, 24_000, 0.25);
        samples.extend(sine(220.0, 0.8, 24_000, 0.25));
        let env = rms_envelope(&samples, 24_000, LEVEL_HZ);
        let peak = env.iter().cloned().fold(0.0, f32::max);
        assert!((peak - 1.0).abs() < 1e-4);
        assert!(env[2] < 0.2, "quiet half {:?}", env[2]);
        assert!(env[20] > 0.9, "loud half {:?}", env[20]);
        assert!(env.iter().all(|&v| (0.0..=1.0).contains(&v)));
    }

    #[test]
    fn near_silent_noise_is_not_amplified_into_speech() {
        let env = rms_envelope(&sine(220.0, 1e-5, 24_000, 0.1), 24_000, LEVEL_HZ);
        assert!(env.iter().all(|&v| v == 0.0));
    }

    #[test]
    fn wav_bytes_decode_to_mono_and_other_bytes_do_not() {
        let spec = hound::WavSpec {
            channels: 2,
            sample_rate: 16_000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut buf = Vec::new();
        {
            let mut writer = hound::WavWriter::new(std::io::Cursor::new(&mut buf), spec).unwrap();
            for _ in 0..1600 {
                writer.write_sample(i16::MAX / 2).unwrap();
                writer.write_sample(0_i16).unwrap();
            }
            writer.finalize().unwrap();
        }
        let (mono, rate) = decode_wav_mono(&buf).expect("WAV decodes");
        assert_eq!(rate, 16_000);
        assert_eq!(mono.len(), 1600);
        assert!(
            (mono[0] - 0.25).abs() < 0.01,
            "averaged channels: {}",
            mono[0]
        );

        assert!(decode_wav_mono(b"ID3\x03\x00 not a wav").is_none());
        assert!(matches!(
            LevelSource::from_audio_bytes(b"\xff\xfb mp3 frame"),
            LevelSource::Synthetic
        ));
        assert!(matches!(
            LevelSource::from_audio_bytes(&buf),
            LevelSource::Envelope(_)
        ));
    }

    #[test]
    fn the_synthetic_rhythm_starts_closed_moves_and_stays_in_range() {
        assert!(synthetic_level(0.0) < 1e-6);
        let samples: Vec<f32> = (0..240).map(|i| synthetic_level(i as f32 / 60.0)).collect();
        assert!(samples.iter().all(|v| (0.0..=1.0).contains(v)));
        let max = samples.iter().cloned().fold(0.0, f32::max);
        let min = samples.iter().cloned().fold(1.0, f32::min);
        assert!(max > 0.6 && min < 0.05, "max {max} min {min}");
    }

    #[test]
    fn a_measured_clip_is_silent_past_its_end() {
        let source = LevelSource::Envelope(Arc::new(vec![0.5, 1.0]));
        assert_eq!(source.level_at(Duration::from_millis(0)), 0.5);
        assert_eq!(source.level_at(Duration::from_millis(20)), 1.0);
        assert_eq!(source.level_at(Duration::from_secs(5)), 0.0);
    }

    #[test]
    fn nothing_is_emitted_before_sound_begins() {
        let mut gate = SpeechGate::new();
        for _ in 0..10 {
            let f = gate.step(0.02);
            assert!(!f.started_now);
            assert_eq!(f.level, None);
        }
        assert!(!gate.started());
        // A player killed before it made a sound ends nothing.
        assert!(!gate.finish());
    }

    #[test]
    fn speech_starts_once_on_the_first_audible_frame() {
        let mut gate = SpeechGate::new();
        gate.step(0.0);
        let first = gate.step(0.5);
        assert!(first.started_now);
        assert!(first.level.unwrap() > 0.0);
        let second = gate.step(0.5);
        assert!(!second.started_now);
        // Quiet frames after the start still report, so the mouth can close.
        let quiet = gate.step(0.0);
        assert!(quiet.level.is_some());
    }

    #[test]
    fn the_level_rises_fast_falls_slower_and_settles_closed() {
        let mut gate = SpeechGate::new();
        let up = gate.step(1.0).level.unwrap();
        assert!(up >= ATTACK - 1e-6);
        let a = gate.step(0.0).level.unwrap();
        assert!(a < up && a > 0.0, "release is gradual: {a}");
        let mut last = a;
        for _ in 0..30 {
            last = gate.step(0.0).level.unwrap();
        }
        assert_eq!(last, 0.0, "silence closes all the way");
        for _ in 0..30 {
            let v = gate.step(2.0).level.unwrap();
            assert!((0.0..=1.0).contains(&v));
        }
        assert_eq!(gate.step(f32::NAN).level.map(|v| v <= 1.0), Some(true));
    }

    #[test]
    fn a_started_session_ends_exactly_once() {
        let mut gate = SpeechGate::new();
        gate.step(0.9);
        assert!(gate.finish());
        assert!(!gate.finish());
    }
}
