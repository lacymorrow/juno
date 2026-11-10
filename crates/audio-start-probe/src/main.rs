//! `audio-start-probe` — what does starting a CoreAudio input stream cost?
//!
//! LAC-7538, carved out of LAC-5281 (Juno drops the first word of a
//! dictation). On that ticket, press-to-first-audio-sample measured 212 ms
//! median over n=35 real presses on installed release builds, and **161 ms of
//! that median is one synchronous call**: `AudioOutputUnitStart`, reached as
//! `WarmInput::resume` -> `lend` -> `start` -> `cpal::Stream::play()` ->
//! `audio_unit.start()`. Every stage Juno itself owns measured 0-4 ms.
//!
//! A comment claimed the same call costs 29-49 ms in a standalone process on
//! the same Mac, same device, same cpal — a 3-5x gap that decides the fix. But
//! that number lived only in a comment and nothing in the repo could re-derive
//! it, so this binary exists to make it a measurement anyone can repeat from a
//! checkout. **If the number is ever quoted again, quote this probe's output.**
//!
//! It times `play()` and nothing else, through the workspace's vendored cpal
//! (`vendor/cpal`, where a JUNO PATCH makes an input stream build stopped so
//! `play()` is the only thing that starts the device). Then it varies one
//! factor at a time, looking for a row whose median reaches 150 ms:
//!
//! | factor | models |
//! |---|---|
//! | `baseline` | the 29-49 ms claim: one stream, device's own config, main thread |
//! | `rate-mismatch` | the device moved off the rate the stream was built for |
//! | `output-client` | TTS holding an output unit open in the same process |
//! | `second-input` | the warm keeper and the lent stream, both on one device |
//! | `spawned-thread` | Juno starts from a worker thread, not main |
//! | `hal-crowd` | output unit and second input together |
//! | `all-factors` | every factor at once, in case none alone is enough |
//!
//! ## Running it
//!
//! ```sh
//! cargo run -p audio-start-probe --release
//! cargo run -p audio-start-probe --release -- --only baseline,rate-mismatch --cycles 40
//! ```
//!
//! A debug build is fine: the timed region is one FFI call, so optimisation
//! level does not move it. `--release` only shortens the build.
//!
//! ## What a clean sweep would and would not prove
//!
//! A terminal-launched probe and Juno are not the same security context. Juno
//! is a signed, hardened-runtime bundle whose microphone access is TCC-governed
//! under its own bundle id; this binary inherits the terminal's grant. If the
//! 161 ms is `coreaudiod` mediating a hardened app's device start, **no
//! terminal probe can reproduce it**, and every row coming back fast would
//! prove the probe cannot model Juno — not that Juno is innocent. In that case
//! the next measurement belongs inside Juno's own process, and the answer is
//! "the probe cannot see it", never "cause unknown".
//!
//! It touches no production code. It is a binary in a crate nothing depends on,
//! so it cannot reach the shipped bundle.

#[cfg(not(target_os = "macos"))]
fn main() -> std::process::ExitCode {
    eprintln!("audio-start-probe times CoreAudio. It only runs on macOS.");
    std::process::ExitCode::FAILURE
}

#[cfg(target_os = "macos")]
mod hal;

#[cfg(target_os = "macos")]
fn main() -> std::process::ExitCode {
    match run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("audio-start-probe: {e}");
            std::process::ExitCode::FAILURE
        }
    }
}

#[cfg(target_os = "macos")]
use {
    cpal::traits::{DeviceTrait, HostTrait, StreamTrait},
    std::sync::atomic::{AtomicUsize, Ordering},
    std::sync::Arc,
    std::time::{Duration, Instant},
};

/// The cost this is hunting for. Below it, a row has not reproduced the
/// in-Juno 161 ms and is not the explanation.
#[cfg(target_os = "macos")]
const REPRODUCES_MS: u128 = 150;

#[cfg(target_os = "macos")]
const USAGE: &str = "\
audio-start-probe — time AudioOutputUnitStart through Juno's vendored cpal

    --cycles <n>     start/stop cycles per factor (default 25, the ticket asks for >= 20)
    --hold-ms <n>    how long the device stays running after each start (default 120),
                     so the input callback has time to deliver. Not timed.
    --gap-ms <n>     idle time between cycles (default 300). Not timed.
    --preflight-ms <n>
                     how long to wait for the microphone to answer once, before
                     giving up on every input row (default 8000). See the note
                     printed on a timeout: it is usually TCC, not CoreAudio.
    --only <keys>    comma-separated factor keys, in place of all of them
    --list           print the factor keys and exit
    --help           this
";

/// One row of the factorial. Each flag switches on exactly one thing the
/// in-Juno path does that a naive standalone measurement does not.
#[cfg(target_os = "macos")]
#[derive(Clone, Copy)]
struct Factor {
    key: &'static str,
    models: &'static str,
    /// Move the device off the rate the timed stream was built for, before
    /// each start. Juno builds its warm stream once and keeps it across
    /// device and rate changes, so a start can be the first code to notice.
    rate_mismatch: bool,
    /// Hold an output unit open in the same process, as TTS does.
    output_client: bool,
    /// Hold a second *running* input stream open on the same device, as the
    /// warm keeper does while the lent stream records.
    second_input: bool,
    /// Build and start from a worker thread at default QoS, as Juno does
    /// (`juno-mic-ready` builds it; the recording thread starts it).
    spawned_thread: bool,
    /// Time an *output* unit instead of the microphone.
    ///
    /// The control row, and the only row that needs no microphone grant.
    /// `AudioOutputUnitStart` is the same function for both directions, so a
    /// fast output row in a process whose input rows will not run says the
    /// probe's HAL path works and the microphone alone is gated — which is a
    /// different statement from "Juno is slow", and the two are easy to
    /// confuse when the input path simply hangs.
    times_output: bool,
}

#[cfg(target_os = "macos")]
const FACTORS: &[Factor] = &[
    Factor {
        key: "output-baseline",
        models: "control: an output unit, the one start that needs no microphone grant",
        rate_mismatch: false,
        output_client: false,
        second_input: false,
        spawned_thread: false,
        times_output: true,
    },
    Factor {
        key: "baseline",
        models: "the 29-49 ms claim: one stream, device's own config, main thread, idle",
        rate_mismatch: false,
        output_client: false,
        second_input: false,
        spawned_thread: false,
        times_output: false,
    },
    Factor {
        key: "rate-mismatch",
        models: "device moved off the rate the stream was built for",
        rate_mismatch: true,
        output_client: false,
        second_input: false,
        spawned_thread: false,
        times_output: false,
    },
    Factor {
        key: "output-client",
        models: "TTS holding an output unit open in the same process",
        rate_mismatch: false,
        output_client: true,
        second_input: false,
        spawned_thread: false,
        times_output: false,
    },
    Factor {
        key: "second-input",
        models: "warm keeper and lent stream, both running on one device",
        rate_mismatch: false,
        output_client: false,
        second_input: true,
        spawned_thread: false,
        times_output: false,
    },
    Factor {
        key: "spawned-thread",
        models: "built and started off the main thread at default QoS",
        rate_mismatch: false,
        output_client: false,
        second_input: false,
        spawned_thread: true,
        times_output: false,
    },
    Factor {
        key: "hal-crowd",
        models: "output unit and second input together",
        rate_mismatch: false,
        output_client: true,
        second_input: true,
        spawned_thread: false,
        times_output: false,
    },
    Factor {
        key: "all-factors",
        models: "every factor at once, in case none alone is enough",
        rate_mismatch: true,
        output_client: true,
        second_input: true,
        spawned_thread: true,
        times_output: false,
    },
];

#[cfg(target_os = "macos")]
#[derive(Clone)]
struct Opts {
    cycles: usize,
    hold: Duration,
    gap: Duration,
    preflight: Duration,
}

/// What one factor produced. `frames` guards against a row that looks fast
/// because the start silently did nothing.
#[cfg(target_os = "macos")]
struct Measurement {
    factor: Factor,
    samples: Vec<Duration>,
    frames: usize,
    note: String,
}

#[cfg(target_os = "macos")]
fn run() -> Result<(), String> {
    let mut opts = Opts {
        cycles: 25,
        hold: Duration::from_millis(120),
        gap: Duration::from_millis(300),
        preflight: Duration::from_millis(8000),
    };
    let mut only: Option<Vec<String>> = None;

    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < args.len() {
        let flag = args[i].clone();
        // Set by every arm that does not return, then applied after the match,
        // so `value` has released its borrow of `i` before `i` moves.
        let consumed: usize;
        let value = |label: &str| -> Result<String, String> {
            args.get(i + 1)
                .cloned()
                .ok_or_else(|| format!("{label} needs a value"))
        };
        match flag.as_str() {
            "--help" | "-h" => {
                print!("{USAGE}");
                return Ok(());
            }
            "--list" => {
                for factor in FACTORS {
                    println!("{:<16} {}", factor.key, factor.models);
                }
                return Ok(());
            }
            "--cycles" => {
                opts.cycles = parse_usize(&value("--cycles")?, "--cycles")?;
                consumed = 2;
            }
            "--hold-ms" => {
                opts.hold = Duration::from_millis(parse_u64(&value("--hold-ms")?, "--hold-ms")?);
                consumed = 2;
            }
            "--gap-ms" => {
                opts.gap = Duration::from_millis(parse_u64(&value("--gap-ms")?, "--gap-ms")?);
                consumed = 2;
            }
            "--preflight-ms" => {
                opts.preflight =
                    Duration::from_millis(parse_u64(&value("--preflight-ms")?, "--preflight-ms")?);
                consumed = 2;
            }
            "--only" => {
                only = Some(
                    value("--only")?
                        .split(',')
                        .map(|key| key.trim().to_string())
                        .filter(|key| !key.is_empty())
                        .collect(),
                );
                consumed = 2;
            }
            other => return Err(format!("unknown argument {other}\n\n{USAGE}")),
        }
        i += consumed;
    }

    if opts.cycles == 0 {
        return Err("--cycles must be at least 1".to_string());
    }

    let chosen: Vec<Factor> = match &only {
        None => FACTORS.to_vec(),
        Some(keys) => {
            for key in keys {
                if !FACTORS.iter().any(|factor| factor.key == key) {
                    return Err(format!("no factor called {key}; --list shows them all"));
                }
            }
            FACTORS
                .iter()
                .filter(|factor| keys.iter().any(|key| key == factor.key))
                .copied()
                .collect()
        }
    };

    print_environment(&opts)?;

    // A microphone this process cannot reach is a result, not a reason to
    // abandon the run: the control row needs no grant, and in a blocked
    // context it is the most useful thing the probe can say. So the input rows
    // are dropped and named, and the table still prints.
    let mut blocked: Option<String> = None;
    if chosen.iter().any(|factor| !factor.times_output) {
        if let Err(e) = preflight(opts.preflight) {
            // One line here; the whole explanation belongs with the table, so
            // that pasting the report carries the reason with it.
            eprintln!("  microphone unreachable: skipping every input row\n");
            blocked = Some(e);
        }
    }
    let runnable: Vec<Factor> = if blocked.is_some() {
        chosen.iter().filter(|f| f.times_output).copied().collect()
    } else {
        chosen
    };

    let mut rows = Vec::with_capacity(runnable.len());
    for factor in &runnable {
        eprintln!("  running {} ...", factor.key);
        rows.push(measure(factor, &opts)?);
    }

    report(&rows, blocked.as_deref());
    if blocked.is_some() {
        // Non-zero: an input row was asked for and could not be timed. A caller
        // in CI or a script must not read this run as a clean sweep.
        return Err("no input row could be timed; see the note above".to_string());
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn parse_usize(raw: &str, label: &str) -> Result<usize, String> {
    raw.parse::<usize>()
        .map_err(|_| format!("{label} wants a whole number, got {raw}"))
}

#[cfg(target_os = "macos")]
fn parse_u64(raw: &str, label: &str) -> Result<u64, String> {
    raw.parse::<u64>()
        .map_err(|_| format!("{label} wants a whole number, got {raw}"))
}

#[cfg(target_os = "macos")]
fn print_environment(opts: &Opts) -> Result<(), String> {
    let host = cpal::default_host();
    let device = host
        .default_input_device()
        .ok_or("this Mac reports no default input device")?;
    let name = device.name().unwrap_or_else(|_| "<unnamed>".to_string());
    let id = hal::device_id(&device);
    let rate = hal::nominal_rate(id)?;
    let rates = hal::available_rates(id)?;

    println!("## audio-start-probe");
    println!();
    println!("- device: {name} (CoreAudio id {id})");
    println!("- rate now: {rate} Hz");
    println!(
        "- rates it accepts: {}",
        if rates.is_empty() {
            "none reported as a discrete rate".to_string()
        } else {
            rates
                .iter()
                .map(|r| format!("{r}"))
                .collect::<Vec<_>>()
                .join(", ")
        }
    );
    println!(
        "- cycles per factor: {}, hold {} ms, gap {} ms",
        opts.cycles,
        opts.hold.as_millis(),
        opts.gap.as_millis()
    );
    // Deliberately does not claim *which* grant applies: TCC resolves this
    // against the responsible process, which for an agent- or daemon-launched
    // run is the thing that spawned the session, not this binary and not the
    // terminal. Measured on 2026-11-10: a Paperclip agent run attributed to
    // Homebrew's `node`. The preflight note says how to read it off the log.
    println!(
        "- context: unbundled binary, microphone grant resolved against its TCC \
         responsible process (NOT Juno's bundle id)"
    );
    println!();
    Ok(())
}

/// What a preflight timeout almost always means, and how to confirm it.
#[cfg(target_os = "macos")]
const TCC_HINT: &str = r#"
This is almost always macOS TCC rather than CoreAudio. A process whose
*responsible* process has no recorded microphone decision gets a pending
prompt, not a denial, and coreaudiod stops answering it: every HAL call then
returns after exactly 30 s with MACH_RCV_TIMED_OUT (0x10004003). Confirm it:

    log show --last 5m --predicate 'process == "tccd"' --style compact \
      | grep -E 'AUTHREQ_PROMPTING|AUTHREQ_ATTRIBUTION'

Read the `responsible=` path on the attribution line. That is the identity
that needs Microphone ticked in System Settings > Privacy & Security >
Microphone, and it is often NOT this binary: launched from an agent or a
daemon it is the parent that spawned the session, so granting it is a real
security decision for whoever owns the machine.

The control row needs no microphone grant and still works:

    audio-start-probe --only output-baseline
"#;

/// Can this process reach the microphone at all?
///
/// A process whose TCC *responsible* process has no recorded microphone
/// decision does not get a denial — it gets a pending prompt, and coreaudiod
/// then stops answering it. Every HAL call returns after exactly 30 s with
/// `MACH_RCV_TIMED_OUT` (0x10004003), and cpal's `default_input_config` is the
/// first call to hit it, so without this the probe sits for minutes printing
/// nothing at all. The timing thread is abandoned rather than joined: it is
/// blocked in a synchronous Mach call that cannot be cancelled, and the
/// process is about to exit.
#[cfg(target_os = "macos")]
fn preflight(timeout: Duration) -> Result<(), String> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::Builder::new()
        .name("probe-preflight".to_string())
        .spawn(move || {
            let outcome = cpal::default_host()
                .default_input_device()
                .ok_or_else(|| "this Mac reports no default input device".to_string())
                .and_then(|device| {
                    device
                        .default_input_config()
                        .map(|_| ())
                        .map_err(|e| format!("default_input_config: {e}"))
                });
            let _ = tx.send(outcome);
        })
        .map_err(|e| format!("could not spawn the preflight thread: {e}"))?;

    match rx.recv_timeout(timeout) {
        Ok(outcome) => outcome,
        Err(_) => Err(format!(
            "the microphone did not answer within {} ms, so no input row can be timed.\n{TCC_HINT}",
            timeout.as_millis()
        )),
    }
}

/// Run one factor. A spawned-thread row runs the whole body on a worker
/// thread, because `cpal::Stream` is not `Send` on macOS and because Juno
/// both builds and starts off the main thread anyway.
#[cfg(target_os = "macos")]
fn measure(factor: &Factor, opts: &Opts) -> Result<Measurement, String> {
    if factor.spawned_thread {
        let factor = *factor;
        let opts = opts.clone();
        std::thread::Builder::new()
            .name("probe-start".to_string())
            .spawn(move || measure_here(&factor, &opts))
            .map_err(|e| format!("could not spawn the probe thread: {e}"))?
            .join()
            .map_err(|_| "the probe thread panicked".to_string())?
    } else {
        measure_here(factor, opts)
    }
}

#[cfg(target_os = "macos")]
fn measure_here(factor: &Factor, opts: &Opts) -> Result<Measurement, String> {
    if factor.times_output {
        return measure_output(factor, opts);
    }

    let host = cpal::default_host();
    let device = host
        .default_input_device()
        .ok_or("this Mac reports no default input device")?;
    let name = device.name().unwrap_or_else(|_| "<unnamed>".to_string());
    let id = hal::device_id(&device);

    // The device's own format, which is what the shipped `input_config` picks.
    let native = device
        .default_input_config()
        .map_err(|e| format!("default_input_config on {name}: {e}"))?;
    let config = native.config();
    let format = native.sample_format();
    let built_for = f64::from(config.sample_rate.0);
    let original_rate = hal::nominal_rate(id)?;

    let mut notes: Vec<String> = Vec::new();

    // Co-tenants first, so they are already running before anything is timed.
    let output = if factor.output_client {
        match start_output() {
            Ok(stream) => Some(stream),
            Err(e) => {
                notes.push(format!("no output unit ({e})"));
                None
            }
        }
    } else {
        None
    };

    let frames = Arc::new(AtomicUsize::new(0));
    let stream = build_input(&device, &config, format, Some(Arc::clone(&frames)))?;

    let keeper = if factor.second_input {
        let keeper = build_input(&device, &config, format, None)?;
        keeper
            .play()
            .map_err(|e| format!("could not start the second input stream: {e}"))?;
        Some(keeper)
    } else {
        None
    };

    // A rate the device accepts and is not already on. 24 kHz first: the
    // AirPods case in the LAC-5281 logs warmed at 24 kHz.
    let alternate = if factor.rate_mismatch {
        let accepted = hal::available_rates(id)?;
        let pick = [24000.0_f64, 16000.0, 44100.0, 48000.0]
            .into_iter()
            .find(|candidate| (candidate - built_for).abs() > 0.5 && accepted.contains(candidate))
            .or_else(|| {
                accepted
                    .iter()
                    .copied()
                    .find(|candidate| (candidate - built_for).abs() > 0.5)
            });
        match pick {
            Some(rate) => Some(rate),
            None => {
                notes.push(format!(
                    "NO MISMATCH POSSIBLE: {name} accepts only {built_for} Hz"
                ));
                None
            }
        }
    } else {
        None
    };

    let mut samples = Vec::with_capacity(opts.cycles);
    let mut confirmed_mismatches = 0usize;

    for _ in 0..opts.cycles {
        if let Some(alternate) = alternate {
            // Move the device off the stream's rate the way another app does
            // while Juno's warm stream waits, then check it actually moved.
            let observed = hal::set_nominal_rate(id, alternate)?;
            if (observed - built_for).abs() > 0.5 {
                confirmed_mismatches += 1;
            }
        }

        let started_at = Instant::now();
        let outcome = stream.play();
        let elapsed = started_at.elapsed();
        outcome.map_err(|e| format!("could not start the input stream on {name}: {e}"))?;
        samples.push(elapsed);

        std::thread::sleep(opts.hold);
        stream
            .pause()
            .map_err(|e| format!("could not stop the input stream on {name}: {e}"))?;
        std::thread::sleep(opts.gap);
    }

    if let Some(alternate) = alternate {
        notes.push(format!(
            "device forced to {alternate} Hz against a stream built for {built_for} Hz, confirmed on {confirmed_mismatches}/{} cycles",
            opts.cycles
        ));
    }

    // Leave the Mac's microphone the way it was found. Stopping is explicit:
    // a cpal macOS stream keeps its device running when merely dropped, which
    // is why `WarmInput`'s own `Drop` halts first. A co-tenant left running
    // here would quietly become part of every later row.
    stop(keeper.as_ref(), "the second input stream");
    stop(output.as_ref(), "the output stream");
    drop(keeper);
    drop(output);
    drop(stream);
    if (hal::nominal_rate(id)? - original_rate).abs() > 0.5 {
        hal::set_nominal_rate(id, original_rate)?;
    }

    let frames = frames.load(Ordering::SeqCst);
    if frames == 0 {
        notes.push("NO AUDIO DELIVERED: these starts did not produce samples".to_string());
    }

    Ok(Measurement {
        factor: *factor,
        samples,
        frames,
        note: notes.join("; "),
    })
}

/// The control row: time `AudioOutputUnitStart` on an output unit.
///
/// Needs no microphone grant, so it runs in a process whose input rows cannot.
/// Note the `pause()` before the loop: the vendored cpal builds an *output*
/// stream already running (`playing: true`, macos/mod.rs:720, upstream
/// behaviour the JUNO PATCH only changed for input), so without it every
/// `play()` would hit cpal's `if !stream.playing` guard, return instantly, and
/// report a 0.0 ms start that measured nothing.
#[cfg(target_os = "macos")]
fn measure_output(factor: &Factor, opts: &Opts) -> Result<Measurement, String> {
    let stream = start_output()?;
    stream
        .pause()
        .map_err(|e| format!("could not stop the output stream: {e}"))?;
    std::thread::sleep(opts.gap);

    let mut samples = Vec::with_capacity(opts.cycles);
    for _ in 0..opts.cycles {
        let started_at = Instant::now();
        let outcome = stream.play();
        let elapsed = started_at.elapsed();
        outcome.map_err(|e| format!("could not start the output stream: {e}"))?;
        samples.push(elapsed);

        std::thread::sleep(opts.hold);
        stream
            .pause()
            .map_err(|e| format!("could not stop the output stream: {e}"))?;
        std::thread::sleep(opts.gap);
    }

    Ok(Measurement {
        factor: *factor,
        samples,
        frames: 0,
        note: "output unit, not the microphone; `frames` does not apply".to_string(),
    })
}

/// Stop a co-tenant stream, reporting rather than failing: cleanup must not
/// lose a measurement that already succeeded.
#[cfg(target_os = "macos")]
fn stop(stream: Option<&cpal::Stream>, what: &str) {
    if let Some(stream) = stream {
        if let Err(e) = stream.pause() {
            eprintln!("  (could not stop {what}: {e})");
        }
    }
}

/// A stopped input stream on `device`, built the way `devices::build_stream`
/// builds Juno's: the device's own config, default buffer size, no timeout.
#[cfg(target_os = "macos")]
fn build_input(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    format: cpal::SampleFormat,
    count_into: Option<Arc<AtomicUsize>>,
) -> Result<cpal::Stream, String> {
    let on_error = |e: cpal::StreamError| eprintln!("  (input stream error: {e})");
    let built = match format {
        cpal::SampleFormat::F32 => device.build_input_stream(
            config,
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                if let Some(count) = &count_into {
                    count.fetch_add(data.len(), Ordering::Relaxed);
                }
            },
            on_error,
            None,
        ),
        cpal::SampleFormat::I16 => device.build_input_stream(
            config,
            move |data: &[i16], _: &cpal::InputCallbackInfo| {
                if let Some(count) = &count_into {
                    count.fetch_add(data.len(), Ordering::Relaxed);
                }
            },
            on_error,
            None,
        ),
        other => {
            return Err(format!(
                "the device records in {other:?}, which the probe does not read"
            ))
        }
    };
    built.map_err(|e| format!("could not build an input stream: {e}"))
}

/// An output unit writing silence, standing in for TTS.
#[cfg(target_os = "macos")]
fn start_output() -> Result<cpal::Stream, String> {
    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .ok_or("this Mac reports no default output device")?;
    let native = device
        .default_output_config()
        .map_err(|e| format!("default_output_config: {e}"))?;
    let config = native.config();
    let on_error = |e: cpal::StreamError| eprintln!("  (output stream error: {e})");
    let stream = match native.sample_format() {
        cpal::SampleFormat::F32 => device.build_output_stream(
            &config,
            |data: &mut [f32], _: &cpal::OutputCallbackInfo| data.fill(0.0),
            on_error,
            None,
        ),
        cpal::SampleFormat::I16 => device.build_output_stream(
            &config,
            |data: &mut [i16], _: &cpal::OutputCallbackInfo| data.fill(0),
            on_error,
            None,
        ),
        other => {
            return Err(format!(
                "the output plays {other:?}, which the probe does not write"
            ))
        }
    }
    .map_err(|e| format!("could not build an output stream: {e}"))?;
    // Already running: cpal starts an output stream inside the build. `play()`
    // is a no-op here and is kept only so the state is explicit at the call
    // site, which is why `measure_output` has to pause before it times one.
    stream
        .play()
        .map_err(|e| format!("could not start the output stream: {e}"))?;
    Ok(stream)
}

/// Nearest-rank percentile over an already sorted slice.
#[cfg(target_os = "macos")]
fn percentile(sorted: &[Duration], fraction: f64) -> Duration {
    if sorted.is_empty() {
        return Duration::ZERO;
    }
    let last = sorted.len() - 1;
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let index = ((sorted.len() as f64 - 1.0) * fraction).round() as usize;
    sorted[index.min(last)]
}

#[cfg(target_os = "macos")]
fn ms(duration: Duration) -> String {
    format!("{:.1}", duration.as_secs_f64() * 1000.0)
}

#[cfg(target_os = "macos")]
fn report(rows: &[Measurement], blocked: Option<&str>) {
    if rows.is_empty() {
        println!("No row could be timed.");
        if let Some(blocked) = blocked {
            println!();
            println!("{blocked}");
        }
        return;
    }
    println!("| factor | N | first | min | median | p90 | max | frames |");
    println!("|---|---|---|---|---|---|---|---|");

    let mut reproduced: Vec<(&'static str, Duration)> = Vec::new();

    for row in rows {
        let first = row.samples.first().copied().unwrap_or(Duration::ZERO);
        let mut sorted = row.samples.clone();
        sorted.sort_unstable();
        let median = percentile(&sorted, 0.5);
        if median.as_millis() >= REPRODUCES_MS {
            reproduced.push((row.factor.key, median));
        }
        println!(
            "| `{}` | {} | {} | {} | **{}** | {} | {} | {} |",
            row.factor.key,
            row.samples.len(),
            ms(first),
            ms(percentile(&sorted, 0.0)),
            ms(median),
            ms(percentile(&sorted, 0.9)),
            ms(percentile(&sorted, 1.0)),
            row.frames,
        );
    }

    println!();
    println!("All figures are milliseconds spent inside `cpal::Stream::play()`, which is");
    println!("`AudioOutputUnitStart` plus a mutex. In-Juno median for the same call: 161 ms.");
    println!();

    for row in rows {
        if !row.note.is_empty() {
            println!("- `{}`: {}", row.factor.key, row.note);
        }
    }

    println!();
    if let Some(blocked) = blocked {
        println!(
            "**Every microphone row was skipped: this process cannot reach the microphone.** \
             Whatever rows are above timed an output unit, which needs no grant, so they say the \
             HAL path works here — they say nothing about Juno's 161 ms."
        );
        println!();
        println!("{blocked}");
        return;
    }
    if reproduced.is_empty() {
        println!(
            "**No factor reached {REPRODUCES_MS} ms.** That does not make Juno innocent: this is \
             an unbundled process under its responsible process's microphone grant, not a signed \
             hardened-runtime bundle under its own TCC identity. If the cost is coreaudiod \
             mediating a hardened app's device start, no probe launched this way can reproduce it. \
             The next measurement is the same timing inside Juno's own process."
        );
    } else {
        println!("**Reproduced at or above {REPRODUCES_MS} ms:**");
        for (key, median) in reproduced {
            println!("- `{key}` at {} ms median", ms(median));
        }
        println!();
        println!(
            "Next: check whether that factor is actually present in Juno today — a code read,"
        );
        println!("not a guess.");
    }
}
