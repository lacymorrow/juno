# Why `say` takes a second to speak, and what could fix it

Question (2026-10-08): the Mac's own voice runs `say` as a child process per sentence. Can the audio be preloaded so it speaks the moment it is wanted?

Measured on this Mac (Apple Silicon, macOS 26, Samantha and Daniel compact voices, other agents building at the same time, so treat the absolute numbers as noisy and the ratios as the finding). Nothing was played aloud: every run used `say -o file`, and "first sample" is the time until the output file holds more than 4 KB, polled every millisecond. Bench scripts were throwaway and are not committed.

## Numbers

| What | Time to first audio |
|---|---|
| `say "hello"`, default voice, median of 6 | 2,030 ms (min 1,840) |
| `say -v Samantha "hello"`, median of 6 | 1,050 ms |
| `say -v Daniel "hello"`, median of 6 | 810 ms |
| `say -v Samantha` on a 3-sentence reply | 1,070 ms (the same as one word) |
| `say -v Samantha "hello"` after 90 s idle ("cold") | 1,510 ms |
| `say -v Samantha "hello"` right after another `say` ("warm") | 1,470 to 1,770 ms (under load) |
| `AVSpeechSynthesizer.write`, first use in a process | 410 to 500 ms |
| `AVSpeechSynthesizer.write`, same process, second utterance onward | 8 to 14 ms (whole "hello" rendered in 20 ms) |

Findings:

1. The cost is per process, not per sentence and not per system. Cold and warm `say` are indistinguishable; a three-sentence reply starts as slowly as one word. Naming a voice with `-v` is cheaper than leaving it to the Mac's default (1.0 s against 2.0 s here), because the default resolves through the speech daemon first.
2. A synthesizer that stays alive in our own process pays about 450 ms once, then starts in about 10 ms. That is a hundred times faster than spawning `say` for each sentence.
3. `say -r` is words per minute, and with no `-r` Samantha and Daniel render exactly like `-r 175` (6.84 s and 7.32 s for the same 20-word reply at both settings). That is where the speed setting's 1.0x comes from.

## Options

**(a) In-process `AVSpeechSynthesizer` (or `NSSpeechSynthesizer`) through objc2.** The only option that removes the cost. The voice stays loaded, each sentence starts in about 10 ms, and sentences can be queued one by one, so #713's speak-the-first-sentence behaviour carries over unchanged. Not done in this PR, for reasons that are about risk rather than doubt:
   - Output device. `say -a <device>` is how the Speaker setting reaches the Mac's voice today. `AVSpeechSynthesizer.speak` plays to the system output only. Honouring the Speaker setting means using `write(_:toBufferCallback:)` and playing the buffers through an `AVAudioEngine` pinned to the chosen device, which is real work.
   - Echo protection (#720). Holding the mic while Juno speaks works by `SIGSTOP`/`SIGCONT` on registered audio pids. A synthesizer in our own process has no pid to stop; it needs `pauseSpeaking` and `continueSpeaking`, wired into `hold_for_capture` and `release_after_capture`.
   - Escape and the speech level. Stop is a `kill` on the registered pid today, and the mouth animation is synthetic because `say` plays out of its own process. In-process playback would stop with `stopSpeaking` and could drive the level from the real buffers, which is better, and also new code.
   - Voice names. The curated catalogue stores `say -v` names such as "Ava (Premium)". They need checking against `AVSpeechSynthesisVoice.speechVoices()`, which names some voices differently.
   - It cannot be compiled or exercised in this PR's environment (no local cargo). Writing unverified objc2 FFI for a path every Mac user hears is the wrong trade.

**(b) One long-lived `say` reading stdin.** Does not work. `say` with no text reads stdin to EOF before it renders anything: with `--progress` and lines written one and two seconds apart, no progress was reported until stdin was closed (the first output appeared 5.4 s in, after the close at 5.0 s). It cannot stream sentence by sentence.

**(c) Pre-render with `-o` while the model streams.** Renders cost the same one second as speaking does, so this only helps if the render finishes before the sentence is wanted. Juno already speaks the first sentence as soon as `</TTS>` arrives (#713), so there is nothing left to hide the render behind for the sentence that matters most, and every later sentence is already spoken while the previous one plays. It would also trade `say`'s own streaming playback for `afplay` of a file, which cannot use `-a`. No gain where it counts.

## What this PR does about it

Only the cheap part, which is not a latency fix: `say` now receives `-r` for the speed setting, and the voice is always passed by name when one is chosen (the existing behaviour), which is the 2x difference between the default voice and a named one.

## Recommendation

Do (a) as its own PR, with the device routing and echo hold built first and a measured before and after on this table. The expected result is about 450 ms once at engine start (which can happen at launch, behind the same preload Kokoro gets in #678) and about 10 ms per sentence after that.

## Built (feat/avspeech-tts)

Option (a) is in `src-tauri/src/tts/avspeech.rs`. Two more measurements made while building it, with a throwaway Swift probe (rendering only, nothing played):

- `writeUtterance:toBufferCallback:` returns at once and delivers its buffers on the main thread: mono Float32, 22,050 Hz, non-interleaved, ending with one zero-length buffer. A stop at `Immediate` also ends with the zero-length buffer, and the next write works.
- `AVSpeechUtterance.rate` against `say -r`: the same 24-word sentence in Samantha lasts 6.32 s at both `say -r 175` and AV rate 0.5, and the other speeds match at 0.42 (0.75x), 0.54 (1.25x), 0.575 (1.5x), 0.60 (1.75x) and 0.64 (2.0x). The table and its interpolation are in `av_rate_for`.
- With no System Voice ever chosen, AVFoundation's default voice and `say` with no `-v` render the same duration to the sample, so that case runs in-process too. A Siri System Voice cannot be used by another app and stays on `say`.

Each sentence logs `[SpeechTiming] engine=avspeech first_audio_ms=...`, or `engine=say` when it fell back.
