//! How fast Juno speaks.
//!
//! One number, `1.0` meaning the engine's own pace, applied by every engine
//! that can take it. The mapping to each engine's own unit lives here so it can
//! be proved without a speaker:
//!
//! - the Mac's `say` takes words per minute (`-r`). Measured on this Mac, with
//!   no `-r` both Samantha and Daniel render exactly like `-r 175`, so 1.0x is
//!   175 wpm and the rate scales it;
//! - Kokoro takes a speed factor (any-tts reads it from the request);
//! - ElevenLabs takes `voice_settings.speed`, which its API limits to 0.7-1.2;
//! - Supertonic takes `speed` in its OpenAI-style request.
//!
//! Replicate and Chatterbox have no speed control, so for them there is no
//! range: the Audio pane shows no slider rather than one that does nothing.

use serde::Serialize;

/// The slowest rate offered.
pub const MIN_RATE: f64 = 0.75;
/// The fastest rate offered.
pub const MAX_RATE: f64 = 2.0;
/// The engine's own pace.
pub const DEFAULT_RATE: f64 = 1.0;
/// ElevenLabs refuses `speed` outside 0.7-1.2.
const ELEVENLABS_MAX_RATE: f64 = 1.2;
/// Words per minute `say` speaks at with no `-r`.
pub const SAY_BASE_WPM: f64 = 175.0;

/// What the Audio pane needs to draw the speed row for the active engine.
#[derive(Debug, Clone, Copy, Serialize, PartialEq)]
pub struct VoiceSpeed {
    /// The rate in force for this engine, already inside its range.
    pub value: f64,
    pub min: f64,
    pub max: f64,
}

/// A stored rate made safe: finite, and inside the global range.
///
/// A hand-edited or corrupt store must not reach `say -r` or a request body.
pub fn sanitize(rate: f64) -> f64 {
    if rate.is_finite() {
        rate.clamp(MIN_RATE, MAX_RATE)
    } else {
        DEFAULT_RATE
    }
}

/// The range an engine can honour, or `None` when it has no speed control.
pub fn range_for(engine: &str) -> Option<(f64, f64)> {
    match engine.trim().to_ascii_lowercase().as_str() {
        "system" | "kokoro" | "supertonic" => Some((MIN_RATE, MAX_RATE)),
        "elevenlabs" => Some((MIN_RATE, ELEVENLABS_MAX_RATE)),
        _ => None,
    }
}

/// The rate this engine will actually speak at, or `None` when it cannot
/// change speed (and so should be left alone).
pub fn effective(engine: &str, stored: f64) -> Option<f64> {
    let (min, max) = range_for(engine)?;
    Some(sanitize(stored).clamp(min, max))
}

/// The speed row for an engine; `None` hides it.
pub fn speed_for(engine: &str, stored: f64) -> Option<VoiceSpeed> {
    let (min, max) = range_for(engine)?;
    Some(VoiceSpeed {
        value: effective(engine, stored)?,
        min,
        max,
    })
}

/// `say -r` for a rate, or `None` at 1.0x so the Mac's own pace is untouched.
pub fn say_words_per_minute(rate: f64) -> Option<u32> {
    let rate = sanitize(rate);
    if (rate - DEFAULT_RATE).abs() < 0.005 {
        None
    } else {
        Some((SAY_BASE_WPM * rate).round() as u32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normal_pace_leaves_say_alone() {
        assert_eq!(say_words_per_minute(1.0), None);
        assert_eq!(say_words_per_minute(1.001), None);
    }

    #[test]
    fn say_scales_from_175_words_a_minute() {
        assert_eq!(say_words_per_minute(2.0), Some(350));
        assert_eq!(say_words_per_minute(1.5), Some(263));
        assert_eq!(say_words_per_minute(0.75), Some(131));
    }

    #[test]
    fn a_bad_stored_rate_never_reaches_an_engine() {
        assert_eq!(sanitize(f64::NAN), 1.0);
        assert_eq!(sanitize(f64::INFINITY), 1.0);
        assert_eq!(sanitize(9.0), MAX_RATE);
        assert_eq!(sanitize(-3.0), MIN_RATE);
        assert_eq!(say_words_per_minute(500.0), Some(350));
    }

    #[test]
    fn engines_with_a_speed_control_get_a_range() {
        for engine in ["system", "kokoro", "supertonic", "ElevenLabs"] {
            assert!(range_for(engine).is_some(), "{engine}");
        }
    }

    #[test]
    fn engines_without_one_show_no_control() {
        for engine in ["replicate", "chatterbox", "off", ""] {
            assert!(range_for(engine).is_none(), "{engine}");
            assert!(speed_for(engine, 1.5).is_none(), "{engine}");
            assert!(effective(engine, 1.5).is_none(), "{engine}");
        }
    }

    #[test]
    fn elevenlabs_is_held_to_its_own_limit() {
        assert_eq!(effective("elevenlabs", 2.0), Some(1.2));
        let speed = speed_for("elevenlabs", 2.0).unwrap();
        assert_eq!((speed.value, speed.max), (1.2, 1.2));
        assert_eq!(effective("kokoro", 2.0), Some(2.0));
    }
}
