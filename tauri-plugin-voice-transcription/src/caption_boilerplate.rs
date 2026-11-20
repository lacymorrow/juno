//! Drops the subtitle boilerplate Whisper invents when it hears no speech.
//!
//! Hold the dictation key, say nothing, let go, and Juno would type "Thank
//! you." or "Thanks for watching!" into whatever app was in front. Whisper was
//! trained on captioned video, so a stretch of audio with no words in it is
//! decoded as the thing captioned video ends with. The RMS gate in
//! [`crate::streaming_commit`] already refuses to decode near-silence, but a
//! fan, a breath or a keyboard are comfortably above it, and that audio still
//! reaches the decoder.
//!
//! A phrase list on its own cannot fix this, because every phrase on it is
//! also a thing a person says: dropping every "Thank you." would be a worse
//! bug than the one being fixed. So the text is only half of the test. The
//! other half is the span Whisper itself claims the phrase occupies:
//!
//! 1. The segment's **whole** text, punctuation and case aside, is one of
//!    [`STOCK_PHRASES`]. A stock phrase inside a sentence is speech.
//! 2. The segment covers at least [`MIN_DROP_SECONDS`] of audio.
//! 3. Spread over that span, the phrase is slower than
//!    [`MIN_WORDS_PER_SECOND`] — slower than anyone talks. Boilerplate
//!    invented for eight seconds of room noise is stamped across all eight
//!    seconds; "thank you" actually spoken takes about a second.
//!
//! All three must hold, so a quiet speaker saying "Thank you." keeps it
//! (their segment is short) and so does someone reading a list of thanks
//! (their text is longer than the phrase). Anything this module cannot judge
//! — no timestamps, a zero-length span — falls under the floor and is kept.
//! Only the final pass runs through here; a live partial has no trustworthy
//! timestamps and is overwritten a moment later anyway.

use tracing::info;

/// Words per second below which a stock phrase is not something anyone said.
///
/// Ordinary speech runs 2 to 4 words per second, so this leaves better than a
/// 2x margin under the slowest real talker.
pub const MIN_WORDS_PER_SECOND: f32 = 1.0;

/// No segment shorter than this is ever dropped, whatever its rate.
///
/// The rate on its own is not enough, because a one-word phrase fails it at
/// any span over a second. whisper.cpp routinely starts its first segment at
/// zero and runs its last one to the end of the audio, so a segment covers the
/// thinking pause as well as the word: hold the key, pause a beat, say
/// "Thanks." and the segment is 0..1.5 s for a 0.5 s word, which is 0.67 w/s
/// and would be dropped. That is the exact harm this module exists to avoid.
///
/// With this floor, a drop needs **both** two seconds of audio and a sub-1
/// w/s rate, for every entry on the list regardless of its length. A test
/// asserts that invariant per entry rather than leaving it to this comment,
/// because the comment is what was wrong (LAC-3738, code review of PR #873).
pub const MIN_DROP_SECONDS: f32 = 2.0;

/// The captioned-video sign-offs Whisper reaches for when there is no speech.
///
/// Attested on the English models, in [`normalized`] form so the comparison is
/// one string equality (a test enforces that). Kept deliberately short: every
/// entry is a real thing someone could say, so each one is a small risk paid
/// for by the two rules above, and a long list is a long list of risks.
///
/// `thanks`, `bye`, `goodbye` and `bye bye` were on this list and were taken
/// off. Each is a plausible whole dictation on its own ("Thanks" in a chat
/// reply), and none of them is a dominant Whisper hallucination the way `you`
/// and `thank you` are, so they were costing more than they returned. `you`
/// stays: it is the commonest silence hallucination of all and essentially
/// never a dictation someone meant to send.
pub const STOCK_PHRASES: &[&str] = &[
    "you",
    "thank you",
    "thank you very much",
    "thanks for watching",
    "thanks for watching this video",
    "thank you for watching",
    "thanks for listening",
    "thank you for listening",
    "please subscribe",
    "subscribe to my channel",
    "please subscribe to my channel",
    "dont forget to subscribe",
    "like and subscribe",
    "subtitles by the amaraorg community",
    "transcription by eso",
    "translation by eso",
];

/// One decoded Whisper segment and the span of the recording it claims.
///
/// `start_ms` and `end_ms` come from `full_get_segment_t0`/`t1`, which
/// whisper.cpp reports in hundredths of a second.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimedSegment {
    pub text: String,
    pub start_ms: i64,
    pub end_ms: i64,
}

impl TimedSegment {
    /// From whisper.cpp's native centisecond timestamps.
    pub fn from_centiseconds(text: impl Into<String>, t0: i64, t1: i64) -> Self {
        Self {
            text: text.into(),
            start_ms: t0 * 10,
            end_ms: t1 * 10,
        }
    }

    fn duration_secs(&self) -> f32 {
        (self.end_ms - self.start_ms).max(0) as f32 / 1000.0
    }
}

/// Lowercased, with everything that is not a letter, a digit or a space
/// removed and runs of whitespace collapsed to one.
///
/// Folds away the things that vary between two decodes of the same phrase and
/// nothing else: "Thank you!", "thank you.", "Thank  you" are one phrase.
/// Apostrophes go too, so the curly one Whisper writes in "don't" matches the
/// straight one on the list.
pub fn normalized(text: &str) -> String {
    let mut kept = String::with_capacity(text.len());
    for c in text.chars() {
        if c.is_alphanumeric() {
            kept.extend(c.to_lowercase());
        } else if c.is_whitespace() {
            kept.push(' ');
        }
    }
    kept.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The normalized form of `text`, when the whole of it is a captioned-video
/// sign-off and nothing else. Empty text never matches: no entry is empty.
fn stock_phrase(text: &str) -> Option<String> {
    let phrase = normalized(text);
    STOCK_PHRASES.contains(&phrase.as_str()).then_some(phrase)
}

/// Is this the entire text of a segment, and nothing but a captioned-video
/// sign-off?
pub fn is_stock_phrase(text: &str) -> bool {
    stock_phrase(text).is_some()
}

/// Is this segment boilerplate invented for audio with no speech in it?
///
/// All three rules from the module docs have to hold. A segment whose span is
/// missing, empty or backwards reads as zero seconds, which is under the
/// floor, so it is kept: an unjudgeable segment is not a hallucination.
pub fn is_invented(segment: &TimedSegment) -> bool {
    let Some(phrase) = stock_phrase(&segment.text) else {
        return false;
    };
    let seconds = segment.duration_secs();
    if seconds < MIN_DROP_SECONDS {
        return false;
    }
    phrase.split_whitespace().count() as f32 / seconds < MIN_WORDS_PER_SECOND
}

/// Joins the segments of a final decode, leaving out the invented ones.
///
/// The kept segments are concatenated exactly as whisper.cpp wrote them,
/// leading space and all, so a transcript with nothing to drop comes out
/// byte-for-byte what it was before this module existed.
pub fn keep_spoken(segments: &[TimedSegment]) -> String {
    let mut out = String::new();
    for segment in segments {
        if is_invented(segment) {
            info!(
                "[CaptionBoilerplate] Dropping {:?} stamped across {:.1}s of audio: nobody talks that slowly",
                segment.text.trim(),
                segment.duration_secs()
            );
            continue;
        }
        out.push_str(&segment.text);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Centiseconds, the way whisper.cpp reports them.
    fn seg(text: &str, t0: i64, t1: i64) -> TimedSegment {
        TimedSegment::from_centiseconds(text, t0, t1)
    }

    #[test]
    fn every_listed_phrase_is_already_in_normal_form() {
        // Otherwise it can never match, and the list would silently do nothing.
        for phrase in STOCK_PHRASES {
            assert_eq!(&normalized(phrase), phrase, "{phrase:?} is not normalized");
        }
    }

    #[test]
    fn the_list_has_no_duplicates() {
        let mut seen = STOCK_PHRASES.to_vec();
        seen.sort_unstable();
        let before = seen.len();
        seen.dedup();
        assert_eq!(seen.len(), before);
    }

    #[test]
    fn normalizing_folds_case_punctuation_and_spacing() {
        assert_eq!(normalized(" Thank you! "), "thank you");
        assert_eq!(normalized("thank  you."), "thank you");
        assert_eq!(
            normalized("Don\u{2019}t forget to subscribe"),
            "dont forget to subscribe"
        );
        assert_eq!(
            normalized("Don't forget to subscribe"),
            "dont forget to subscribe"
        );
        assert_eq!(normalized("Amara.org"), "amaraorg");
        assert_eq!(normalized("   "), "");
    }

    #[test]
    fn a_stock_phrase_is_the_whole_segment_or_it_is_speech() {
        assert!(is_stock_phrase(" Thanks for watching! "));
        assert!(is_stock_phrase("Thank you."));
        // The same words inside a sentence someone said.
        assert!(!is_stock_phrase(
            "Thank you for the quick turnaround on this"
        ));
        assert!(!is_stock_phrase("thanks for watching the kids on Friday"));
        assert!(!is_stock_phrase(""));
        assert!(!is_stock_phrase("   "));
    }

    #[test]
    fn boilerplate_stretched_over_a_long_hold_is_dropped() {
        // Eight seconds of room noise, three words.
        assert!(is_invented(&seg(" Thanks for watching!", 0, 800)));
        // Five seconds, two words.
        assert!(is_invented(&seg(" Thank you.", 0, 500)));
        // The one-word case, which is the commonest of all.
        assert!(is_invented(&seg(" you", 120, 400)));
    }

    #[test]
    fn a_phrase_someone_actually_said_is_kept() {
        // "Thanks for watching" spoken takes about 1.2s: 2.5 words/second.
        assert!(!is_invented(&seg(" Thanks for watching!", 0, 120)));
        // A quiet, unhurried "thank you" in under two seconds.
        assert!(!is_invented(&seg(" Thank you.", 300, 480)));
        // Said at the end of a long hold, not stamped across it.
        assert!(!is_invented(&seg(" Thank you.", 740, 820)));
    }

    #[test]
    fn no_entry_can_be_dropped_before_the_floor() {
        // The invariant the module docs claim, asserted per entry instead of
        // described. Before the floor existed the one-word entries broke it:
        // "you" over 1.1s is 0.9 w/s, which dropped a word someone said.
        let just_under = (MIN_DROP_SECONDS * 100.0) as i64 - 1;
        for phrase in STOCK_PHRASES {
            assert!(
                !is_invented(&seg(phrase, 0, just_under)),
                "{phrase:?} dropped at {:.2}s, under the {MIN_DROP_SECONDS}s floor",
                just_under as f32 / 100.0
            );
        }
    }

    #[test]
    fn a_one_word_reply_someone_meant_to_send_is_kept() {
        // Hold the key, pause a beat to think, say it. whisper.cpp starts the
        // first segment at zero, so the segment covers the pause as well as
        // the word: 1.5s for a 0.5s word is 0.67 w/s, which the rate alone
        // would drop.
        assert!(!is_invented(&seg(" you", 0, 150)));
        // Past the floor it is boilerplate again.
        assert!(is_invented(&seg(" you", 0, 300)));
    }

    #[test]
    fn the_words_people_actually_say_alone_are_not_on_the_list() {
        // Taken off deliberately: each is a plausible whole dictation and
        // none is a dominant hallucination.
        for word in ["thanks", "Thanks.", "bye", "Bye!", "goodbye", "bye bye"] {
            assert!(!is_stock_phrase(word), "{word:?} is back on the list");
            assert!(!is_invented(&seg(word, 0, 900)), "{word:?} was dropped");
        }
    }

    #[test]
    fn a_segment_with_no_usable_span_is_kept() {
        assert!(!is_invented(&seg(" Thank you.", 0, 0)));
        assert!(!is_invented(&seg(" Thank you.", 500, 100)));
    }

    #[test]
    fn exactly_the_minimum_rate_is_kept() {
        // Two words over two seconds is 1.0 w/s: not *below* the floor.
        assert!(!is_invented(&seg("thank you", 0, 200)));
        // A hair slower is.
        assert!(is_invented(&seg("thank you", 0, 201)));
    }

    #[test]
    fn a_transcript_with_nothing_to_drop_is_returned_unchanged() {
        let segments = vec![
            seg(" Ship the fix on Friday.", 0, 180),
            seg(" Tell the team in the standup.", 180, 340),
        ];
        assert_eq!(
            keep_spoken(&segments),
            " Ship the fix on Friday. Tell the team in the standup."
        );
        assert_eq!(keep_spoken(&[]), "");
    }

    #[test]
    fn real_speech_survives_boilerplate_appended_to_it() {
        // The case that matters most: a dictation that ended with the person
        // going quiet, so Whisper signed off for them.
        let segments = vec![
            seg(" Let's move the review to Thursday.", 0, 210),
            seg(" Thanks for watching!", 210, 900),
        ];
        assert_eq!(
            keep_spoken(&segments),
            " Let's move the review to Thursday."
        );
    }

    #[test]
    fn a_hold_with_nothing_but_boilerplate_in_it_comes_out_empty() {
        // Callers treat an empty final transcript as "nothing was said": no
        // typing, no clipboard, no query.
        assert_eq!(keep_spoken(&[seg(" Thank you.", 0, 600)]), "");
    }

    #[test]
    fn a_dictation_that_is_only_thank_you_still_types_thank_you() {
        assert_eq!(keep_spoken(&[seg(" Thank you.", 20, 110)]), " Thank you.");
    }
}
