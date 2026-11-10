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
cannot today, and the next section is why.

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

## Result, 2026-11-10, M1 MacBook Air (zero), built-in mic, macOS 25.6

### The probe cannot measure a microphone start in an agent-launched process

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

That is a hypothesis, not a measurement. Confirming it needs an input start in
a process that *has* a microphone grant.

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

**The premise is confirmed, not killed.** Standalone `AudioOutputUnitStart` on
an input device is 28-49 ms; Juno pays 161 ms for the same call. The 3-5x gap
is real, so the factorial is still the right next step and still worth running.

On *this* run, of the ticket's two honest outcomes, the answer is the second:
**the probe cannot model Juno here**, and it cannot even run. Not "no factor
reproduced it, cause unknown" — the factorial never got to execute, because an
agent-launched process on this Mac no longer has a microphone grant to execute
it with, for the reason above.

## What would finish it

One of these, cheapest first. Both produce the factor table the probe already
knows how to print.

1. **Run the probe from a context that already has Microphone ticked** — a real
   Terminal/iTerm window on this Mac. `cargo run -p audio-start-probe
   --release` and paste the table. No new grant, no new code, and it answers
   the whole factorial in one go.
2. **Restore Microphone for the fleet's `node`**, which puts agent runs back
   where they were on 2026-10-28 and makes this unattended again. It is a
   restoration rather than an expansion, but it still grants every
   agent-spawned process microphone access, so it is the machine owner's
   decision and not one an agent should answer on a prompt. Note it will be
   revoked again by the next `node` upgrade.
3. **Measure in Juno's own process** — a debug Tauri command on a dev build,
   which is the route LAC-7538 pre-authorised for exactly this case. Costs a CI
   build and a manual invocation, and gives up this ticket's "touches no
   production code" property. Worth doing on its own merits eventually: it is
   the only option that measures the real security context Juno ships in.

Until one of them runs, the pre-roll / mic-indicator tradeoff (LAC-6121) stays
undecided, because its input is the number above.

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
