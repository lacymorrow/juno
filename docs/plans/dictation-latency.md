# Dictation latency plan

**Goal:** hotkey → text in well under one second on Apple Silicon for a 10 s utterance, with the same pipeline serving agent mode.

**Source:** 2026-09-04 comparison of five open-source Wispr Flow alternatives (whisper-flow, open-wispr, opentypeless, OpenWhispr, freeflow; all MIT) against Juno's pipeline. Tracked in LAC-3730.

## Where the time went (before this plan)

| Stage | Cost | Cause |
|---|---|---|
| Press → mic open | ~500–700 ms | `system_profiler` subprocess in `mic_permissions.rs` on every press (~400 ms), cold cpal stream open, 50 ms poll loop in `dictation_monitor.rs` |
| During hold | CPU-bound | `whisper-rs` built without the `metal` feature; each 1.5 s partial is a full CPU encoder pass |
| Release → text | seconds | `transcribe_final` re-decodes the whole utterance, beam 5, 4 threads, on CPU |
| Injection | ~200 ms + a race | three pasteboard round-trips, 100/50/50 ms sleeps, clipboard restored immediately on drop (Electron/JVM targets read the old clipboard) |

## Done in `perf/dictation-latency`

1. `metal` feature on `whisper-rs` in both crates; `ggml-metal.metal` shipped as a bundle resource and `GGML_METAL_PATH_RESOURCES` pointed at it before the model loads (`tauri-plugin-voice-transcription/src/lib.rs`).
2. Microphone availability via CoreAudio default device instead of `system_profiler`.
3. Decode parameters: threads = available cores (2–8), beam 2 for the final pass, greedy single-segment no-context partials, `suppress_blank`, entropy 2.8 / logprob −1.25 fallback thresholds.
4. Paste path: 20 ms settle, 8 ms key gap, clipboard restored on a 600 ms timer only if it still holds the pasted text.
5. One-second silent warm-up decode after every engine load or switch.

## Next, in order

6. Pre-warm and hold the microphone: resolve the device at startup, keep the cpal stream open for a few seconds after each dictation into a ring buffer, and feed the pre-roll into the session so the hold-threshold window loses no speech (`controller.rs start_dictation`). **Moved to LAC-5281** with item 7: juno#868 ("open mail" typed as "mail") is the same defect with a user-facing done condition, so it is tracked as a bug and not as a latency item.
7. Trigger the mic on the press event itself; keep the 300 ms hold-cancel logic (`events/shortcuts.rs`, `dictation_monitor.rs`). **Moved to LAC-5281**, see item 6.
8. ~~Streaming commit instead of re-decode on release.~~ Done in `perf/stt-final-without-redecode` (`streaming_commit.rs`): segments cut at pauses (>= 400 ms, >= 3 s since the last cut) are decoded at final quality on a background worker during the hold; release decodes only the tail. No pause, or a failed segment, falls back to the whole-utterance decode. Works for both engines. Still open: Parakeet TDT native streaming, and text-stability commits for people who never pause.
9. ~~RMS speech gate before the final decode~~ (done with 8: near-silent segments and tails are not decoded) and ~~the stock-phrase filter~~ (`caption_boilerplate.rs`). **It is not gated on `no_speech_prob`, and it cannot be.** whisper-rs 0.11.1 does not expose it; neither does whisper-rs on master, whose safe API stops at `full_get_segment_t0`/`t1`. The raw `whisper_full_get_segment_no_speech_prob` symbol only appears in whisper-rs-sys on master, so reaching it means a 0.11 → 0.16 jump (a whole whisper.cpp version with its own Metal build) plus unsafe FFI, for one quality filter. Not worth it. The gate used instead is the span Whisper itself reports: a segment whose entire text is a captioned-video sign-off *and* which is stamped across more audio than anyone could say it in (under 1 word/second) is dropped. Both halves are load-bearing — the phrase list alone would eat a genuine "Thank you.", the rate alone would eat genuinely slow speech. The reach of this depends on Whisper stamping invented boilerplate across the whole speechless window rather than a short span; that is what it does on the clips the bug was reported from, and it is what QA measures. If the timestamps come back short, the follow-up is a real `no_speech_prob`, not a lower threshold.
10. ~~Capture app/AX context in parallel with recording and run a cleanup pass.~~ The cleanup pass shipped as `src-tauri/src/dictation_cleanup.rs` (Apple Foundation Models, opt-in, off by default) and is already parallelised the only way it can be: the model is warmed on key-down, because generation needs the finished transcript and so can never overlap the decode of its own input. Context gathering was dropped: nothing in the dictation path reads app or AX context, and adding a capture to feed a consumer that does not exist is the abstraction-without-need rule.

## Measuring

Record one 10 s fixture utterance. Log timestamps at: shortcut press, first audio frame, key release, final text ready, paste posted. Report the release → paste number; that is the one users feel.
