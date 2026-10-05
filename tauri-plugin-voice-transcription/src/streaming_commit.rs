//! Streaming commit: decode the finished part of a dictation while the
//! person is still holding the key, so the release only waits for the tail.
//!
//! The final transcript used to be one high-quality decode of the whole
//! utterance, started on release. Its cost grows with the utterance, and all
//! of it sat between "I let go" and "the words appear". Here the audio is cut
//! into segments at pauses as it arrives; each committed segment is decoded
//! with the same final-quality settings on a background worker while the
//! person keeps talking. On release only the audio after the last cut is
//! decoded, and the texts are joined.
//!
//! Accuracy rules, in the order they matter:
//!
//! 1. A cut only lands inside a real pause: at least [`MIN_PAUSE_MS`] of
//!    audio well below the segment's speech level, with at least half of it
//!    on each side of the cut. Words are never split.
//! 2. A segment is at least [`MIN_SEGMENT_MS`] long, so the decoder always
//!    has a sentence's worth of context, and Whisper is additionally prompted
//!    with the tail of the text already committed.
//! 3. No pause, no cut. Someone who talks straight through gets exactly the
//!    old whole-utterance decode. A failed segment decode does the same.
//!
//! Everything here except [`SegmentCommitter`] is pure, so the cut and join
//! rules are tested without a microphone or a model.

use crate::engine::{TranscriptionEngine, TranscriptionSession};
use std::ops::Range;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Sender};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use tracing::{info, warn};

/// Analysis frame for pause detection.
pub const FRAME_MS: u32 = 20;
/// Shortest segment worth committing. Shorter ones give the decoder too
/// little context to be as accurate as the whole-utterance pass.
pub const MIN_SEGMENT_MS: u32 = 3_000;
/// Shortest pause a cut may land in. Gaps inside words (stop consonants,
/// a breath between syllables) are well under this.
pub const MIN_PAUSE_MS: u32 = 400;
/// A frame counts as pause when it is this far below the segment's speech
/// level (the 90th-percentile frame): -20 dB.
pub const PAUSE_RATIO: f32 = 0.1;
/// Bounds on the pause threshold, so a near-silent segment cannot make the
/// threshold vanish and a loud one cannot make soft speech count as pause.
pub const PAUSE_FLOOR_RMS: f32 = 0.001;
pub const PAUSE_CEIL_RMS: f32 = 0.02;
/// RMS gate before a final-quality decode: audio whose loudest frame is under
/// this (about -50 dBFS) holds no speech and is not decoded at all. Whisper
/// invents stock phrases ("Thank you.") for silence, and decoding nothing
/// costs time on the release path. Deliberately low: typical dictation peaks
/// at 0.02 to 0.1, and a quiet speaker must never be gated out.
pub const NEAR_SILENT_RMS: f32 = 0.003;
/// How much already-committed text Whisper is prompted with, in characters.
pub const PROMPT_TAIL_CHARS: usize = 200;
/// Re-evaluate for a cut at most once per this many new frames (100 ms).
const EVAL_EVERY_FRAMES: usize = 5;

fn rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    (samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32).sqrt()
}

fn frame_len(sample_rate: u32) -> usize {
    ((sample_rate as u64 * FRAME_MS as u64) / 1000).max(1) as usize
}

/// True when `audio` holds nothing louder than [`NEAR_SILENT_RMS`] in any
/// [`FRAME_MS`] frame. Empty audio is silent.
pub fn is_near_silent(audio: &[f32], sample_rate: u32) -> bool {
    audio
        .chunks(frame_len(sample_rate))
        .all(|frame| rms(frame) < NEAR_SILENT_RMS)
}

/// Finds safe cut points in a growing recording.
///
/// Fed the raw capture as it arrives (any chunk size, any rate). Keeps one
/// RMS value per [`FRAME_MS`] frame, so a check costs a scan over the
/// uncommitted frames only, never over samples.
pub struct CommitPlanner {
    frame_len: usize,
    frame_rms: Vec<f32>,
    partial: Vec<f32>,
    committed_frame: usize,
    evaluated_at: usize,
    min_segment_frames: usize,
    half_pause_frames: usize,
}

impl CommitPlanner {
    pub fn new(sample_rate: u32) -> Self {
        Self {
            frame_len: frame_len(sample_rate),
            frame_rms: Vec::new(),
            partial: Vec::new(),
            committed_frame: 0,
            evaluated_at: 0,
            min_segment_frames: (MIN_SEGMENT_MS / FRAME_MS) as usize,
            half_pause_frames: (MIN_PAUSE_MS / FRAME_MS / 2) as usize,
        }
    }

    pub fn push(&mut self, samples: &[f32]) {
        let mut rest = samples;
        if !self.partial.is_empty() {
            let need = self.frame_len - self.partial.len();
            let take = need.min(rest.len());
            self.partial.extend_from_slice(&rest[..take]);
            rest = &rest[take..];
            if self.partial.len() < self.frame_len {
                return;
            }
            self.frame_rms.push(rms(&self.partial));
            self.partial.clear();
        }
        let mut frames = rest.chunks_exact(self.frame_len);
        for frame in &mut frames {
            self.frame_rms.push(rms(frame));
        }
        self.partial.extend_from_slice(frames.remainder());
    }

    /// Sample offset everything before which has been committed.
    pub fn committed_samples(&self) -> usize {
        self.committed_frame * self.frame_len
    }

    /// The next segment to commit, as a sample range of the recording, or
    /// `None` when there is no safe cut yet. Each range starts where the
    /// previous one ended.
    pub fn next_commit(&mut self) -> Option<Range<usize>> {
        if self.frame_rms.len() < self.evaluated_at + EVAL_EVERY_FRAMES {
            return None;
        }
        self.evaluated_at = self.frame_rms.len();

        let frames = &self.frame_rms[self.committed_frame..];
        let half = self.half_pause_frames;
        if frames.len() < self.min_segment_frames + half {
            return None;
        }

        let threshold = pause_threshold(frames);
        let mut run_start: Option<usize> = None;
        let mut cut = None;
        for (i, &level) in frames.iter().enumerate() {
            if level >= threshold {
                run_start = None;
                continue;
            }
            let start = *run_start.get_or_insert(i);
            // Earliest cut in this pause that leaves half a pause on its left
            // and makes the segment long enough; it also needs half a pause
            // of already-heard quiet on its right.
            let candidate = (start + half).max(self.min_segment_frames);
            if candidate + half <= i + 1 {
                cut = Some(candidate);
                break;
            }
        }

        let cut = cut?;
        let from = self.committed_samples();
        self.committed_frame += cut;
        Some(from..self.committed_samples())
    }
}

/// The level under which a frame counts as pause, relative to this
/// segment's own speech level so it adapts to the microphone and the room.
fn pause_threshold(frames: &[f32]) -> f32 {
    let mut sorted = frames.to_vec();
    let idx = (sorted.len() * 9 / 10).min(sorted.len().saturating_sub(1));
    let (_, p90, _) = sorted.select_nth_unstable_by(idx, |a, b| a.total_cmp(b));
    (*p90 * PAUSE_RATIO).clamp(PAUSE_FLOOR_RMS, PAUSE_CEIL_RMS)
}

fn is_attaching_punctuation(c: char) -> bool {
    matches!(c, ',' | '.' | ';' | ':' | '!' | '?' | ')' | ']' | '%')
}

fn strip_trailing_ellipsis(s: &mut String) {
    loop {
        let trimmed = s.trim_end();
        if let Some(rest) = trimmed.strip_suffix("...") {
            let len = rest.len();
            s.truncate(len);
        } else if let Some(rest) = trimmed.strip_suffix('\u{2026}') {
            let len = rest.len();
            s.truncate(len);
        } else {
            let len = trimmed.len();
            s.truncate(len);
            return;
        }
    }
}

/// Joins segment texts into one transcript.
///
/// Each part is trimmed and empty parts are skipped. Parts are separated by
/// one space, except that leading punctuation attaches to the previous word.
/// Whisper marks a segment that "trails off" with an ellipsis; when the next
/// segment carries on mid-sentence (starts lowercase or with an ellipsis of
/// its own), those seam ellipses are dropped. Case is never changed.
pub fn join_segments<S: AsRef<str>>(parts: &[S]) -> String {
    let mut out = String::new();
    for part in parts {
        let mut text = part.as_ref().trim();
        if text.is_empty() {
            continue;
        }
        if out.is_empty() {
            out.push_str(text);
            continue;
        }

        let mut continues = false;
        loop {
            if let Some(rest) = text.strip_prefix("...") {
                text = rest.trim_start();
                continues = true;
            } else if let Some(rest) = text.strip_prefix('\u{2026}') {
                text = rest.trim_start();
                continues = true;
            } else {
                break;
            }
        }
        if text.is_empty() {
            continue;
        }
        let first = text.chars().next().unwrap_or(' ');
        if continues || first.is_lowercase() {
            strip_trailing_ellipsis(&mut out);
        }
        if !out.is_empty() && !is_attaching_punctuation(first) {
            out.push(' ');
        }
        out.push_str(text);
    }
    out
}

/// The last [`PROMPT_TAIL_CHARS`] or so of `text`, starting on a word
/// boundary, with NUL bytes removed (whisper-rs panics on them).
pub fn prompt_tail(text: &str) -> String {
    let text = text.trim();
    let chars = text.chars().count();
    let tail = if chars <= PROMPT_TAIL_CHARS {
        text
    } else {
        let skip = text
            .char_indices()
            .nth(chars - PROMPT_TAIL_CHARS)
            .map(|(i, _)| i)
            .unwrap_or(0);
        let rest = &text[skip..];
        match rest.find(char::is_whitespace) {
            Some(ws) => rest[ws..].trim_start(),
            None => rest,
        }
    };
    tail.replace('\0', "")
}

/// Decodes one committed segment, or the tail, at final quality.
///
/// Near-silent audio is not decoded. Text committed so far is passed as
/// context. Shared by the background worker and the release path so both
/// apply the same gate.
pub fn decode_segment(
    session: &mut dyn TranscriptionSession,
    audio_16k: &[f32],
    committed_so_far: &[String],
) -> Result<String, String> {
    if is_near_silent(audio_16k, 16_000) {
        return Ok(String::new());
    }
    if committed_so_far.iter().all(|t| t.trim().is_empty()) {
        return session.transcribe_final(audio_16k);
    }
    let context = prompt_tail(&join_segments(committed_so_far));
    session.transcribe_final_segment(audio_16k, &context)
}

/// Converts captured audio to the 16 kHz the engines take.
pub type Resample = fn(&[f32], u32) -> Result<Vec<f32>, String>;

/// Background worker that decodes committed segments while recording goes on.
///
/// One thread, one session, segments decoded strictly in order, so each one
/// can be prompted with the text before it and no two segment decodes ever
/// overlap. The release path calls [`finish`](Self::finish), which waits for
/// every outstanding segment before the tail is decoded; dropping it instead
/// (a cancel) skips segments not yet started and still waits for the one in
/// flight, so no decode outlives its recording.
pub struct SegmentCommitter {
    jobs: Option<Sender<Vec<f32>>>,
    cancel: Arc<AtomicBool>,
    handle: Option<JoinHandle<Option<Vec<String>>>>,
}

impl SegmentCommitter {
    /// `sample_rate` is the capture rate of the raw segments it will be given.
    /// The worker's session is created on its first segment, so a dictation
    /// that never pauses never pays for one.
    pub fn spawn(
        engine: Arc<dyn TranscriptionEngine>,
        sample_rate: u32,
        resample: Resample,
    ) -> Self {
        let (tx, rx) = channel::<Vec<f32>>();
        let cancel = Arc::new(AtomicBool::new(false));
        let cancel_for_worker = Arc::clone(&cancel);
        let handle = thread::spawn(move || {
            let mut session: Option<Box<dyn TranscriptionSession>> = None;
            let mut texts: Vec<String> = Vec::new();
            while let Ok(raw) = rx.recv() {
                if cancel_for_worker.load(Ordering::SeqCst) {
                    return None;
                }
                if session.is_none() {
                    match engine.create_session() {
                        Ok(s) => session = Some(s),
                        Err(e) => {
                            warn!("[StreamingCommit] No session for segment decodes: {e}");
                            return None;
                        }
                    }
                }
                let session = session.as_mut()?;
                let started = std::time::Instant::now();
                let decoded = resample(&raw, sample_rate)
                    .and_then(|audio| decode_segment(&mut **session, &audio, &texts));
                match decoded {
                    Ok(text) => {
                        info!(
                            "[StreamingCommit] Segment {} ({:.2}s) decoded in {} ms",
                            texts.len(),
                            raw.len() as f32 / sample_rate.max(1) as f32,
                            started.elapsed().as_millis()
                        );
                        texts.push(text);
                    }
                    Err(e) => {
                        // One bad segment poisons the join; the release path
                        // falls back to decoding the whole recording.
                        warn!("[StreamingCommit] Segment decode failed: {e}");
                        return None;
                    }
                }
            }
            Some(texts)
        });
        Self {
            jobs: Some(tx),
            cancel,
            handle: Some(handle),
        }
    }

    /// Queue a committed raw segment. Returns immediately.
    pub fn submit(&self, raw_segment: Vec<f32>) {
        if let Some(jobs) = &self.jobs {
            let _ = jobs.send(raw_segment);
        }
    }

    /// Wait for every submitted segment and return their texts in order, or
    /// `None` if any of them could not be decoded.
    pub fn finish(mut self) -> Option<Vec<String>> {
        self.jobs.take();
        self.handle.take()?.join().ok().flatten()
    }
}

impl Drop for SegmentCommitter {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            self.cancel.store(true, Ordering::SeqCst);
            self.jobs.take();
            let _ = handle.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    const RATE: u32 = 16_000;

    fn speech(ms: u32, rate: u32) -> Vec<f32> {
        let n = (rate as u64 * ms as u64 / 1000) as usize;
        (0..n)
            .map(|i| 0.1 * (i as f32 * 2.0 * std::f32::consts::PI * 220.0 / rate as f32).sin())
            .collect()
    }

    fn quiet(ms: u32, rate: u32) -> Vec<f32> {
        let n = (rate as u64 * ms as u64 / 1000) as usize;
        // Room noise, not digital zero.
        (0..n)
            .map(|i| 0.0005 * (i as f32 * std::f32::consts::PI).cos())
            .collect()
    }

    fn cat(parts: &[Vec<f32>]) -> Vec<f32> {
        parts.concat()
    }

    /// Feeds `audio` in `chunk`-sized pieces, collecting every commit.
    fn feed(planner: &mut CommitPlanner, audio: &[f32], chunk: usize) -> Vec<Range<usize>> {
        let mut commits = Vec::new();
        for piece in audio.chunks(chunk) {
            planner.push(piece);
            while let Some(r) = planner.next_commit() {
                commits.push(r);
            }
        }
        commits
    }

    fn ms_to_samples(ms: u32, rate: u32) -> usize {
        (rate as u64 * ms as u64 / 1000) as usize
    }

    #[test]
    fn a_pause_after_enough_speech_is_cut_inside_the_pause() {
        let audio = cat(&[speech(4_000, RATE), quiet(600, RATE), speech(2_000, RATE)]);
        let mut p = CommitPlanner::new(RATE);
        let commits = feed(&mut p, &audio, 160);
        assert_eq!(commits.len(), 1, "{commits:?}");
        let cut = commits[0].end;
        let pause_start = ms_to_samples(4_000, RATE);
        let pause_end = ms_to_samples(4_600, RATE);
        let half = ms_to_samples(MIN_PAUSE_MS / 2, RATE);
        assert_eq!(commits[0].start, 0);
        assert!(cut >= pause_start + half, "cut {cut} too close to speech");
        assert!(cut + half <= pause_end, "cut {cut} too close to speech");
    }

    #[test]
    fn a_short_pause_is_never_cut() {
        let audio = cat(&[speech(4_000, RATE), quiet(250, RATE), speech(4_000, RATE)]);
        let mut p = CommitPlanner::new(RATE);
        assert!(feed(&mut p, &audio, 160).is_empty());
    }

    #[test]
    fn a_pause_too_early_in_the_segment_is_not_cut() {
        let audio = cat(&[speech(1_000, RATE), quiet(600, RATE), speech(1_000, RATE)]);
        let mut p = CommitPlanner::new(RATE);
        assert!(feed(&mut p, &audio, 160).is_empty());
    }

    #[test]
    fn talking_straight_through_commits_nothing_so_the_whole_decode_runs() {
        let audio = speech(20_000, RATE);
        let mut p = CommitPlanner::new(RATE);
        assert!(feed(&mut p, &audio, 160).is_empty());
        assert_eq!(p.committed_samples(), 0);
    }

    #[test]
    fn a_pause_still_going_at_the_end_is_cut_once_long_enough() {
        let audio = cat(&[speech(3_500, RATE), quiet(450, RATE)]);
        let mut p = CommitPlanner::new(RATE);
        let commits = feed(&mut p, &audio, 160);
        assert_eq!(commits.len(), 1);
        assert!(commits[0].end >= ms_to_samples(3_500 + MIN_PAUSE_MS / 2, RATE));
    }

    #[test]
    fn segments_are_contiguous_and_each_long_enough() {
        let mut parts = Vec::new();
        for _ in 0..4 {
            parts.push(speech(3_200, RATE));
            parts.push(quiet(500, RATE));
        }
        parts.push(speech(1_000, RATE));
        let audio = cat(&parts);
        let mut p = CommitPlanner::new(RATE);
        let commits = feed(&mut p, &audio, 333);
        assert_eq!(commits.len(), 4, "{commits:?}");
        for pair in commits.windows(2) {
            assert_eq!(pair[0].end, pair[1].start);
        }
        for c in &commits {
            assert!(c.len() >= ms_to_samples(MIN_SEGMENT_MS, RATE));
        }
        assert_eq!(p.committed_samples(), commits.last().unwrap().end);
    }

    #[test]
    fn the_cut_does_not_depend_on_how_the_audio_was_chunked() {
        let audio = cat(&[speech(4_000, RATE), quiet(700, RATE), speech(2_000, RATE)]);
        let a = feed(&mut CommitPlanner::new(RATE), &audio, 160);
        let b = feed(&mut CommitPlanner::new(RATE), &audio, 1);
        let c = feed(&mut CommitPlanner::new(RATE), &audio, 4_801);
        // Evaluation cadence can shift a cut by a few frames; it must stay in
        // the pause regardless.
        for commits in [&a, &b, &c] {
            assert_eq!(commits.len(), 1);
            let cut = commits[0].end;
            assert!(cut >= ms_to_samples(4_200, RATE) && cut <= ms_to_samples(4_500, RATE));
        }
    }

    #[test]
    fn works_at_a_microphone_rate_that_is_not_16k() {
        for rate in [44_100, 48_000] {
            let audio = cat(&[speech(4_000, rate), quiet(600, rate), speech(2_000, rate)]);
            let commits = feed(&mut CommitPlanner::new(rate), &audio, 480);
            assert_eq!(commits.len(), 1, "rate {rate}");
            let cut = commits[0].end;
            assert!(cut >= ms_to_samples(4_200, rate) && cut <= ms_to_samples(4_400, rate));
        }
    }

    #[test]
    fn quiet_speech_is_not_mistaken_for_a_pause() {
        // A stretch spoken 10 dB softer than the rest, longer than a pause.
        // Only frames 20 dB under the speech level count as pause, so no cut
        // lands inside it.
        let soft: Vec<f32> = speech(800, RATE).iter().map(|s| s * 0.3).collect();
        let audio = cat(&[speech(4_000, RATE), soft, speech(2_000, RATE)]);
        assert!(feed(&mut CommitPlanner::new(RATE), &audio, 160).is_empty());
    }

    #[test]
    fn the_rms_gate_skips_silence_and_keeps_quiet_speech() {
        assert!(is_near_silent(&[], RATE));
        assert!(is_near_silent(&quiet(2_000, RATE), RATE));
        let whisper_quiet: Vec<f32> = speech(500, RATE).iter().map(|s| s * 0.1).collect();
        assert!(!is_near_silent(&whisper_quiet, RATE), "0.007 RMS is speech");
        let mut mostly_silent = quiet(3_000, RATE);
        mostly_silent.extend(speech(100, RATE));
        assert!(!is_near_silent(&mostly_silent, RATE), "one word is enough");
    }

    #[test]
    fn join_puts_one_space_between_segments_and_skips_empty_ones() {
        assert_eq!(
            join_segments(&[" Hello there.", "", "  ", " How are you? "]),
            "Hello there. How are you?"
        );
        assert_eq!(join_segments::<&str>(&[]), "");
        assert_eq!(join_segments(&["", "Only"]), "Only");
    }

    #[test]
    fn join_attaches_leading_punctuation() {
        assert_eq!(
            join_segments(&["I said yes", ", then left."]),
            "I said yes, then left."
        );
        assert_eq!(join_segments(&["Done", "."]), "Done.");
    }

    #[test]
    fn join_drops_seam_ellipses_when_the_sentence_carries_on() {
        assert_eq!(
            join_segments(&["I went to the store...", "and bought milk."]),
            "I went to the store and bought milk."
        );
        assert_eq!(
            join_segments(&["I went to the store\u{2026}", "\u{2026}and bought milk."]),
            "I went to the store and bought milk."
        );
        assert_eq!(join_segments(&["Wait...", "...what?"]), "Wait what?");
    }

    #[test]
    fn join_keeps_an_ellipsis_before_a_new_sentence_and_never_changes_case() {
        assert_eq!(
            join_segments(&["Let me think...", "Okay, yes."]),
            "Let me think... Okay, yes."
        );
        assert_eq!(join_segments(&["Hello.", "World."]), "Hello. World.");
    }

    #[test]
    fn prompt_tail_is_short_starts_on_a_word_and_has_no_nul() {
        let long = "word ".repeat(100);
        let tail = prompt_tail(&long);
        assert!(tail.chars().count() <= PROMPT_TAIL_CHARS);
        assert!(tail.starts_with("word"));
        assert_eq!(prompt_tail("short\0 text"), "short text");
        // Multi-byte text is cut on a character boundary, not a byte.
        let accents = "\u{e9}t\u{e9} ".repeat(80);
        assert!(prompt_tail(&accents).starts_with('\u{e9}'));
    }

    // ── The worker ──

    #[derive(Default)]
    struct Log {
        calls: Mutex<Vec<(usize, Option<String>)>>,
        fail_on: Option<usize>,
    }

    struct FakeEngine(Arc<Log>);
    struct FakeSession(Arc<Log>);

    impl FakeSession {
        fn record(&self, audio: &[f32], ctx: Option<&str>) -> Result<String, String> {
            let mut calls = self.0.calls.lock().unwrap();
            let n = calls.len();
            calls.push((audio.len(), ctx.map(str::to_string)));
            if self.0.fail_on == Some(n) {
                return Err("boom".into());
            }
            Ok(format!("seg{n}."))
        }
    }

    impl TranscriptionSession for FakeSession {
        fn transcribe_partial(&mut self, _audio: &[f32]) -> Result<Option<String>, String> {
            Ok(None)
        }
        fn transcribe_final(&mut self, audio: &[f32]) -> Result<String, String> {
            self.record(audio, None)
        }
        fn transcribe_final_segment(
            &mut self,
            audio: &[f32],
            context: &str,
        ) -> Result<String, String> {
            self.record(audio, Some(context))
        }
    }

    impl TranscriptionEngine for FakeEngine {
        fn name(&self) -> &'static str {
            "fake"
        }
        fn supports_streaming(&self) -> bool {
            false
        }
        fn is_initialized(&self) -> bool {
            true
        }
        fn create_session(&self) -> Result<Box<dyn TranscriptionSession>, String> {
            Ok(Box::new(FakeSession(Arc::clone(&self.0))))
        }
    }

    fn identity(audio: &[f32], _rate: u32) -> Result<Vec<f32>, String> {
        Ok(audio.to_vec())
    }

    #[test]
    fn segments_are_decoded_in_order_with_the_text_before_them_as_context() {
        let log = Arc::new(Log::default());
        let c = SegmentCommitter::spawn(Arc::new(FakeEngine(Arc::clone(&log))), RATE, identity);
        c.submit(speech(3_000, RATE));
        c.submit(speech(3_100, RATE));
        c.submit(quiet(3_000, RATE));
        c.submit(speech(3_200, RATE));
        let texts = c.finish().expect("all decoded");
        assert_eq!(texts, vec!["seg0.", "seg1.", "", "seg2."]);
        let calls = log.calls.lock().unwrap();
        assert_eq!(calls.len(), 3, "the silent segment was never decoded");
        assert_eq!(calls[0], (48_000, None));
        assert_eq!(calls[1], (49_600, Some("seg0.".to_string())));
        assert_eq!(calls[2], (51_200, Some("seg0. seg1.".to_string())));
    }

    #[test]
    fn a_failed_segment_makes_finish_report_failure_for_a_full_decode() {
        let log = Arc::new(Log {
            fail_on: Some(1),
            ..Default::default()
        });
        let c = SegmentCommitter::spawn(Arc::new(FakeEngine(Arc::clone(&log))), RATE, identity);
        c.submit(speech(3_000, RATE));
        c.submit(speech(3_000, RATE));
        c.submit(speech(3_000, RATE));
        assert!(c.finish().is_none());
    }

    #[test]
    fn finish_with_nothing_submitted_never_creates_a_session() {
        let log = Arc::new(Log::default());
        let c = SegmentCommitter::spawn(Arc::new(FakeEngine(Arc::clone(&log))), RATE, identity);
        assert_eq!(c.finish(), Some(Vec::new()));
        assert!(log.calls.lock().unwrap().is_empty());
    }

    #[test]
    fn dropping_the_committer_waits_for_the_worker() {
        let log = Arc::new(Log::default());
        let c = SegmentCommitter::spawn(Arc::new(FakeEngine(Arc::clone(&log))), RATE, identity);
        for _ in 0..5 {
            c.submit(speech(3_000, RATE));
        }
        drop(c);
        // Joined: the count can no longer change.
        let n = log.calls.lock().unwrap().len();
        thread::sleep(std::time::Duration::from_millis(50));
        assert_eq!(log.calls.lock().unwrap().len(), n);
        assert!(n <= 5);
    }

    #[test]
    fn the_tail_is_gated_and_prompted_like_a_segment() {
        let log = Arc::new(Log::default());
        let mut s = FakeSession(Arc::clone(&log));
        assert_eq!(
            decode_segment(&mut s, &quiet(1_000, RATE), &["a".into()]).unwrap(),
            ""
        );
        assert!(
            log.calls.lock().unwrap().is_empty(),
            "silent tail not decoded"
        );
        decode_segment(&mut s, &speech(1_000, RATE), &[]).unwrap();
        decode_segment(&mut s, &speech(1_000, RATE), &["".into(), "".into()]).unwrap();
        decode_segment(
            &mut s,
            &speech(1_000, RATE),
            &["One.".into(), "Two.".into()],
        )
        .unwrap();
        let calls = log.calls.lock().unwrap();
        assert_eq!(calls[0].1, None, "no committed text: plain final decode");
        assert_eq!(calls[1].1, None, "only silent segments: plain final decode");
        assert_eq!(calls[2].1.as_deref(), Some("One. Two."));
    }
}
