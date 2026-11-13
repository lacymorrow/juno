# What starting the microphone actually costs (LAC-7538)

Carved out of LAC-5281 (GH #868, Juno drops the first word of a dictation).
This file exists so the number stops living in a comment: **everything below is
re-derivable from a checkout** with `crates/audio-start-probe`.

## The question

On LAC-5281, press to first audio sample measured 212 ms median over n=35 real
presses on installed release builds (411 ms p90, 1296 ms max, floor 71 ms). Of
that median, **161 ms is one synchronous call** — `AudioOutputUnitStart`,
reached as `WarmInput::resume` → `lend` → `start` → `cpal::Stream::play()` →
`audio_unit.start()`. Every stage Juno itself owns measured 0-4 ms.

A comment claimed the same call costs 29-49 ms in a standalone process on the
same Mac, same device, same cpal. That 3-5x gap decides the fix:

- **real and recoverable** → #868 is fixed by removing it, and nobody spends a
  privacy decision;
- **not recoverable** → every remaining option (pre-roll, modifier pre-arm)
  runs the mic device before the person speaks, lighting the macOS mic
  indicator, and the call is a product decision.

Nothing in the repo could re-derive the 29-49 ms, so step one was to stop
trusting it.

## Where the 29-49 ms came from, and why it stands

It was measured — on **2026-10-28**, on this same Mac (zero, built-in mic, F32
48 kHz), by a standalone crate with a path dependency on this repo's vendored
cpal. That run recorded:

| what | cost |
|---|---|
| `build_input_stream` (patched cpal builds it stopped) | 6-27 ms |
| **`play()` = `AudioOutputUnitStart`** | **28-49 ms** |
| press to first sample callback | 40-58 ms |

and ruled out, by measurement, `default_input_device()` (0.1 ms),
`default_input_config()` (5-7 ms), thread spawn-to-running (0.1 ms), and the
permission check (a cached atomic after first grant). Stream reuse was free:
8 start/stop cycles on one stream cost the same as the first.

**So the premise holds: the gap is real.** The problem was never that the
number was wrong — it is that the crate was built in a scratch directory and
thrown away, leaving the figure in a comment with no way to re-derive it. That
is the hole this file and `crates/audio-start-probe` close.

Note what that run recorded about permissions: *"the agent session's
responsible app already had it, so the probe ran without a TCC prompt."* On
2026-10-28 an agent-launched process on this Mac could open the microphone. It
could not on 2026-11-10 (see that attempt below), and it can again as of
2026-11-12. That capability has now flipped twice without anyone changing
Juno, which is why LAC-7587 tracks it separately.

## The probe

`crates/audio-start-probe` — a workspace member, binary only, nothing depends
on it, so it cannot reach the shipped bundle. It resolves `cpal` through the
workspace `[patch.crates-io]`, so it times the same vendored code the app
starts its microphone with, including the JUNO PATCH that makes an input
stream build *stopped* so `play()` is the only thing that starts the device.

```sh
cargo run -p audio-start-probe --release                     # every factor
cargo run -p audio-start-probe --release -- --only output-baseline
cargo run -p audio-start-probe --release -- --list
```

CI builds it (`Clippy the audio start probe` in `ci.yml`). It needs its own
step: `--manifest-path src-tauri/Cargo.toml` selects the `juno` package alone
and never reaches a sibling member, the same gap that left the voice plugin's
tests unrun for months.

## Result, 2026-11-12: the factorial ran, and idle time is the only variable that moves it

The microphone grant came back (see the 2026-11-10 attempt below), so the whole
factorial executed. M1 MacBook Air (zero), built-in mic, macOS 25.6. 25 cycles
per row, hold 120 ms, **gap 300 ms**. Milliseconds inside
`cpal::Stream::play()`, which is `AudioOutputUnitStart` plus a mutex.

| factor | N | first | min | median | p90 | max | frames |
|---|---|---|---|---|---|---|---|
| `output-baseline` (control, output unit) | 25 | 18.1 | 6.4 | **11.5** | 18.1 | 31.8 | n/a |
| `baseline` | 25 | 35.6 | 31.7 | **37.2** | 47.2 | 85.3 | 148480 |
| `rate-mismatch` | 25 | 166.4 | 30.0 | **34.2** | 51.9 | 166.4 | **0** |
| `output-client` | 25 | 30.0 | 30.0 | **38.7** | 57.2 | 76.9 | 144384 |
| `second-input` | 25 | 0.3 | 0.3 | **0.8** | 7.1 | 10.7 | 148992 |
| `spawned-thread` | 25 | 43.5 | 30.2 | **34.1** | 54.9 | 71.2 | 144384 |
| `hal-crowd` | 25 | 0.9 | 0.6 | **1.0** | 2.7 | 5.1 | 149504 |
| `all-factors` | 25 | 91.3 | 0.6 | **1.4** | 9.7 | 91.3 | **0** |

Two rows are timing starts that produced **no audio at all**: `rate-mismatch`
and `all-factors` report frames = 0 on 25 of 25 cycles, for the reason in its
own section below. Their timings are real measurements of
`AudioOutputUnitStart` returning, but they are not valid samples of a *working*
start. `all-factors`' 1.4 ms median especially must never be quoted as a fast
start; it is a fast failure.

### The baseline reproduces the premise: 37.2 ms

n=25, min 31.7, p90 47.2. The 28-49 ms recorded on 2026-10-28 is confirmed,
this time from a committed crate anyone can re-run. Juno's 161 ms for the same
call is 4.3x it, so the gap this ticket exists to explain is real and was not
an artefact of a lost scratch measurement.

### Idle time before the start is the only variable that moves it, and the 300 ms default hid it

Not one row above reaches 150 ms as a median. Re-run the same rows varying the
one thing the table holds fixed, which is how long the device sits idle
between starts, and they do:

| row | gap 300 ms | gap 30 s | gap 120 s |
|---|---|---|---|
| `baseline` median | 37.2 (n=25) | 72.6 (n=5) | **84.8** (n=5) |
| `baseline` p90 / max | 47.2 / 85.3 | 82.2 / 82.2 | **142.3 / 142.3** |
| `spawned-thread` median | 34.1 (n=25) | **75.6** (n=12) | not run |
| `spawned-thread` p90 / max | 54.9 / 71.2 | **148.0 / 212.4** | not run |

Exactly what produced the three columns, from a checkout:

```sh
cargo build -p audio-start-probe --release
./target/release/audio-start-probe --cycles 25                                      # gap 300 ms (default)
./target/release/audio-start-probe --only baseline,spawned-thread --cycles 5 --gap-ms 30000
./target/release/audio-start-probe --only spawned-thread --cycles 12 --gap-ms 30000
./target/release/audio-start-probe --only baseline --cycles 5 --gap-ms 120000
```

The long-gap runs are slow by construction: a 12-cycle run at a 30 s gap takes
about 6 minutes of mostly waiting, and that waiting *is* the measurement.

Juno's 161 ms median sits between the `spawned-thread` p90 and max at a 30 s
idle. This matters because Juno's warm input stream sits **paused for minutes**
between presses, so the long-gap columns are the regime Juno actually runs in
and the 300 ms column is not.

`--gap-ms` applies only *between* cycles, which hands the measurement a partial
control it was not designed to have: cycle 1 is the one cycle that never sat
idle, and in every long-gap row it comes in **near** the row minimum
(`baseline` at 30 s: first 41.8 = min; at 120 s: first 33.0 = min;
`spawned-thread` at 30 s n=5: first 37.7 = min; n=12: first 42.0 against a min
of 32.6, which is 29% above it rather than at it).

Partial, and the gap matters. Cycle 1 is also the first start after the stream
was built, and the only one not preceded by a `pause()`, so it differs from the
rest in more than idle time and cannot carry the reading on its own. The probe
prints only the per-row summary, not the per-cycle series, so a drift over the
course of a run cannot be ruled out from this output either. Treat cycle 1 as
*consistent with* the idle reading, not as proof of it.

It also lines up with a number LAC-5281 measured but could not account for:
across 35 real presses the floor was **71 ms**, and no press ever came in under
it. `baseline` at a 30 s idle has a median of 72.6 ms.

Read the Verdict before carrying this further. The effect is measured and
dose-responsive, but LAC-5281 also recorded its *fastest* press immediately
after a 3-day gap, which is the opposite of what a simple "longer idle costs
more" rule predicts. Idle-inside-a-live-process and app-unused-for-days are
not established to be the same variable.

### More HAL clients is not a cost. It is the cure.

`second-input` (0.8 ms) and `hal-crowd` (1.0 ms) are about **40x faster** than
baseline, not slower, and this is the most actionable number in the table. A
second *running* input client means the device is already going, and
`AudioOutputUnitStart` against a live device costs ~1 ms. Factor 2 of the
ticket's four hypotheses is not merely absent as a cause; it is inverted.

This is the figure LAC-6121 (pre-roll versus the macOS mic indicator) has been
waiting on. Keeping the device running collapses the start from 37 ms cold, or
~75 ms after a realistic idle, to **0.8 ms**. Whatever the remaining cost is
made of, a start against an already-running device does not pay it.

### A rate-mismatched start succeeds and delivers silence

`rate-mismatch` and `all-factors` report **frames = 0 on 25 of 25 cycles**,
while every rate-matched input row delivered ~145k frames. `play()` returns
`Ok`, the stream runs, and no sample ever arrives. For LAC-5281 that is
first-word loss taken to its limit.

It does **not** explain the 35 measured presses, because `claim_warm_input`
rejects a rate-stale warm stream (`claim_warm_input`, in `devices.rs`). But the guard compares only the
*rate*. Channel count and sample format are unguarded.

One more reading of that row, offered as a loose end and not a result: its
`first` cycle is 166.4 ms against a row median of 34.2 ms. So a mismatch does
not cost 166 ms. Something about that one cycle did, and the idle finding above
is the obvious candidate, but this is a single sample on a row whose starts
delivered no audio whatsoever. It is not evidence of anything by itself.

## The 2026-11-10 attempt, and why it could not run

Kept because the TCC mechanism below is real, is not specific to audio, and
will recur. It is history as of 2026-11-12, not the current state.

### On that day the probe could not measure a microphone start at all

| factor | N | min | median | p90 | max |
|---|---|---|---|---|---|
| `output-baseline` (control, output unit) | 25 | 5.1 | **7.9** | 11.9 | 16.0 |
| `baseline` and every other input row | 0 | — | **unmeasurable** | — | — |

Milliseconds inside `cpal::Stream::play()`.

**The input rows did not come back slow. They did not come back at all**, and
the reason is macOS TCC, not CoreAudio:

- The probe is an unbundled binary, so TCC resolves its microphone access
  against its **responsible process**. For a Paperclip agent run that is
  Homebrew's `node` (`/opt/homebrew/Cellar/node/26.13.0_1/bin/node`) — not
  Juno, not a terminal.
- That identity had **no recorded decision** (`Auth Right: Unknown (None)`), so
  tccd raised a prompt: `AUTHREQ_PROMPTING ... service=kTCCServiceMicrophone`.
- A *pending* prompt is worse than a denial. coreaudiod stops answering the
  client, and every HAL IPC returns after **exactly 30 s** —
  `MACH_RCV_TIMED_OUT` (0x10004003), then `MACH_SEND_TIMED_OUT` (0x10000004).
  The probe never reaches `AudioOutputUnitStart`; it wedges earlier, in
  cpal's `default_input_config` → `audio_unit_from_device` →
  `AudioUnitSetProperty`.

The probe now detects this in one preflight call and prints the diagnosis plus
the `log show` command that confirms it, instead of hanging silently for
minutes. `--only output-baseline` still runs, because an output unit needs no
grant.

### What the control does establish

`AudioOutputUnitStart` is **not intrinsically expensive on this Mac**: 7.9 ms
median to start a device, in the same process, same cpal, same HAL, when no
microphone authorisation sits in the path. The in-Juno figure is 161 ms, about
20x that, and the difference is confined to the input-specific path — which is
exactly where coreaudiod's microphone authorisation lives.

That was a hypothesis, not a measurement, and it has since been tested. The
2026-11-12 run got the grant, started the input device in 37.2 ms, and found
no authorisation cost of that magnitude. So the reasoning in this paragraph,
though sound at the time, pointed at the wrong suspect. See the Verdict.

### Corroboration from Juno's own logs

Juno's `[CaptureTiming]` records `opening`→`live` as "CoreAudio starting the
device" — the same window. Only two presses survive this Mac's week-long log
window (`~/Library/Logs/Juno/juno-2026-11-04.log`), and both are worse than the
LAC-5281 median:

| press | `opening`→`live` | note |
|---|---|---|
| `capture=1` | **1039 ms** | first press after launch |
| `capture=2` | **390 ms** | warm stream, same session |

n=2, quoted as corroboration only; the n=35 dataset lives on LAC-5281. The
first-press-after-launch figure being ~2.7x the next one is consistent with a
cold authorisation path rather than with device hardware.

### Why it worked on 2026-10-28 and not on 2026-11-10

A TCC grant is keyed to the binary's code identity. For an unsigned Homebrew
binary that is a content hash — here `node-55554944e3d173e54c5434e4adeb5d46975f006d`.
Replace the binary and the grant no longer matches anything.

| date | event |
|---|---|
| 2026-10-28 | standalone input probe runs, no TCC prompt, 28-49 ms recorded |
| 2026-11-02 14:08 | `node 26.13.0_1` installed (`INSTALL_RECEIPT.json` mtime) |
| 2026-11-04 21:42 | old keg removed (`/opt/homebrew/Cellar/node/` mtime) |
| 2026-11-10 | same class of probe: `Auth Right: Unknown (None)`, prompt raised |

So the Node CVE patch (LAC-5351) almost certainly revoked the fleet's
microphone access for agent-launched processes as a side effect, silently,
six days before anyone needed it. Stated as the likely cause rather than a
proven one: `TCC.db` needs Full Disk Access to read, which this process does
not have (`kTCCServiceSystemPolicyAllFiles: Denied`), so the old grant record
cannot be inspected directly. The dates, the unchanged method and the changed
outcome are the evidence.

This is not specific to audio: it will silently break any fleet work that
needs a TCC-gated capability from an agent-launched process, and a Homebrew
upgrade will do it again.

## Verdict

**The premise is confirmed by direct measurement.** Standalone
`AudioOutputUnitStart` on an input device is 37.2 ms (n=25, 2026-11-12), inside
the 28-49 ms recorded on 2026-10-28. Juno pays 161 ms for the same call, so the
3-5x gap this ticket exists to explain is real and was not an artefact of a
lost scratch measurement.

**Against the ticket's bar, a row median of >=150 ms, no factor reproduces
it.** That includes the one described below, whose best median is 84.8 ms. Say
that plainly before anything else.

But the four named factors are not merely under the bar, they are *flat*:
`rate-mismatch` 34.2, `spawned-thread` 34.1, `output-client` 38.7 against a
baseline of 37.2, all within run-to-run noise, and the two client-count rows
are 40x **faster**. On its own terms the factorial is conclusive: none of the
four is the cost.

**The one variable that does move the number is idle time before the start.**
Within a single row the cost rises with the dose: `baseline` medians
37.2 -> 72.6 -> 84.8 ms at 0.3 s, 30 s and 120 s, p90 47.2 -> 82.2 -> 142.3;
`spawned-thread` medians 34.1 -> 75.6 ms at 0.3 s and 30 s, p90 54.9 -> 148.0,
with one start reaching 212.4 ms. A 30 s idle is the only configuration in
which this probe starts the microphone in over 150 ms at all.

Three qualifications, because those rows are thin. **Compare only within a
row.** The figures above are quoted per row for that reason: an earlier draft of
this paragraph read "p90 47 -> 148 -> 142", which crossed `baseline` and
`spawned-thread` and so manufactured a fall at 120 s that is not in the data.
**At n=5 a p90 is just the max**, so `baseline`'s 82.2 / 82.2 and 142.3 / 142.3
are one sample each rather than a distribution. And the long-gap rows are n=5
and n=12 against n=25 for the 300 ms column, so this is three points per row,
not a curve.

**That demotes the hardened-runtime hypothesis.** This document previously
reasoned that if no factor reproduced the cost, the explanation was probably
coreaudiod mediating a signed, hardened, TCC-governed bundle, and that no
terminal probe could ever show it. An unbundled, unsigned, terminal-launched
probe now produces 148-212 ms starts. Juno's security context is not *needed*
to explain starts of that magnitude. It is not disproved, just no longer the
leading candidate.

### What this does not settle, and the datapoint that argues against it

Idle time is not established as the explanation for Juno's 161 ms, for two
reasons that should travel with the number:

1. **The residual.** 161 ms in Juno against 84.8 ms in the probe at a 120 s
   idle is still ~1.9x, and the probe's curve is flattening (37 -> 73 -> 85),
   so simply waiting longer does not obviously close it.
2. **LAC-5281's 3-day press.** That dataset recorded the *fastest* of its 35
   presses immediately after a 3-day gap. If idle time were the whole story
   that press should have been the slowest in the set. The honest reading is
   that "paused for 30-120 s inside one live process" and "the app sat unused
   for days" are not the same variable. The first plausibly hits a CoreAudio
   idle-teardown path while the process stays alive, the second does not. So
   the effect measured here is real and reproducible, and generalising it to
   Juno's median is a further step this ticket has not taken.

Remaining candidates for the residual, in order: Juno's stream sits paused for
minutes rather than 120 s; Juno's `Opening -> Live` brackets slightly more than
`play()` alone (LAC-7739); and the hardened-runtime context, demoted but not
eliminated.

## Which factors are even present in Juno today

A code read, not a guess, done while the microphone was unavailable. Two of
the ticket's four hypotheses turn out to be structurally absent on the path
the 35 presses took, which is worth knowing before anyone spends runs on them.

| factor | present? | evidence |
|---|---|---|
| **1. Format mismatch** | **No, guarded** | `claim_warm_input` in `devices.rs`: `let at_rate = fresh \|\| warm.rate_is_current();`. A rate-mismatched warm stream is rejected and the press opens the device directly, so it never reaches `resume()`. Every LAC-5281 press was warm-path, so a mismatch cannot explain them. |
| **2. In-process output HAL client** | **Yes, conditionally** | `src-tauri/src/tts/pcm_player.rs:388-403` builds a cpal output stream on the default output device. (`say` and `afplay` are subprocesses and do not count.) Whether one is live at press time depends on TTS being mid-playback. |
| **3. Spawned caller thread** | **Yes, always** | `controller.rs:587` calls `warm.resume()` inside the `open` closure, which runs on the `thread::spawn` audio worker (`controller.rs:684`). Never the main thread. |
| **4. Two input clients on one device** | **No, on the measured path** | The warm stream is *lent*, not duplicated: `claim_warm_input` takes it out of the slot and `lend()` starts that same stream, so dictation only ever holds one input stream. Two exist only under a wake phrase (`WakeMicHeld`), and there the stream is already running, so a press does **no device start at all**. |

Caveat on 1: only the *rate* is checked, not channel count or sample format.
A format mismatch is unguarded, just much less likely than the rate case the
AirPods log pointed at.

That code read set the factorial's priority order, `spawned-thread` first,
because factor 3 is unconditionally present on every one of the 35 presses
that paid the 161 ms, making it the only hypothesis true every single time.
The 2026-11-12 run honoured that order, and `spawned-thread` came back at
34.1 ms against a 37.2 ms baseline: present on every press, and not the cost.
Being always-true turned out to be the reason to test it first, not evidence
that it was to blame.

## What finished it, and what is left

Options 1 and 2 as this document first listed them are spent: the microphone
grant came back without an agent asking for it (most likely a human answered
the prompt left pending on 2026-11-10), and the probe then ran unattended on
2026-11-12.

1. **Make the idle row first-class in the probe** (LAC-7739). Today the cause
   is visible only to a caller who already knows to pass `--gap-ms`, and the
   default quietly produces the reassuring answer. That is the single change
   that stops this being re-discovered from scratch.
2. **Decide pre-roll versus the mic indicator** (LAC-6121), which now has the
   number it was waiting for, and it is the most robust result here: a start
   against an already-running device costs **0.8 ms** against 37-85 ms for a
   start that has to wake the device. That figure does not depend on which
   explanation for the 161 ms turns out to be right.
3. **Measure inside Juno's own process** via a debug Tauri command, the route
   LAC-7538 pre-authorised. Still worth doing, and now for a sharper reason
   than before: not to find the cause, but to close the ~1.9x residual and to
   settle whether the 3-day-gap press above contradicts the idle effect or
   just measures a different one.

## Traps found while building this

- cpal builds an **output** stream already running (`playing: true`,
  `vendor/cpal/src/host/coreaudio/macos/mod.rs:720`; the JUNO PATCH only
  changed input). `play()` on a fresh one hits cpal's `if !stream.playing`
  guard and returns instantly, so a control row that does not `pause()` first
  reports a confident 0.0 ms that measured nothing.
- cpal changes the device's nominal rate at **build** time
  (`set_sample_rate`, macos/mod.rs:538), not at start. That is what makes the
  format-mismatch factor plausible at all: Juno builds its warm stream once,
  another app moves the device afterwards, and `play()` is the first code to
  find out.
- This Mac's built-in mic accepts 44100/48000/88200/96000 — **not 24000**. The
  24 kHz case in the LAC-5281 logs was AirPods, a device class nobody has
  measured. The probe picks the nearest accepted alternative and says which.
