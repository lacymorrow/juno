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

6. Pre-warm and hold the microphone: resolve the device at startup, keep the cpal stream open for a few seconds after each dictation into a ring buffer, and feed the pre-roll into the session so the hold-threshold window loses no speech (`controller.rs start_dictation`).
7. Trigger the mic on the press event itself; keep the 300 ms hold-cancel logic (`events/shortcuts.rs`, `dictation_monitor.rs`).
8. Streaming commit instead of re-decode on release: Parakeet TDT via `parakeet-rs`, or a growing-window decode with text-stability commit for Whisper (`controller.rs process_partial_transcription`).
9. RMS speech gate before the final decode; stock-phrase hallucination filter gated on `no_speech_prob`.
10. Optional: capture app/AX context in parallel with recording and run a cleanup pass (opt-in, "preserve exact wording" bypass).

## Measuring

Record one 10 s fixture utterance. Log timestamps at: shortcut press, first audio frame, key release, final text ready, paste posted. Report the release → paste number; that is the one users feel.
