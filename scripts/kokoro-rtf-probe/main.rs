//! Kokoro real-time factor, Metal against CPU (LAC-9112).
//!
//! `kokoro-rtf-probe <rounds> <device>...` where a device is `metal` or `cpu`.
//! Models are configured the way `tts/kokoro.rs` configures them. With two
//! devices both stay loaded and take turns on each sentence, swapping who goes
//! first, so both are measured under the same machine load.
//!
//! One CSV line per synthesis: device, round, sentence, audio seconds, wall
//! milliseconds, RTF, process CPU milliseconds, 1-minute load average.

use std::time::Instant;

const VOICE: &str = "af_heart";

const SENTENCES: [&str; 4] = [
    "Your meeting starts soon.",
    "I found three files that match what you asked for.",
    "The download finished a few minutes ago, and everything looks fine.",
    "I opened the settings window and turned on the option you wanted, so the next \
     time you start the app it will remember your choice and use it right away.",
];

type Model = Box<dyn any_tts::TtsModel>;

fn load(device: &str) -> Result<Model, String> {
    let config = any_tts::TtsConfig::new(any_tts::ModelType::Kokoro);
    let config = match device {
        "metal" => config.with_preferred_runtime(),
        "cpu" => config
            .with_device(any_tts::DeviceSelection::Cpu)
            .with_dtype(any_tts::DType::F32),
        other => return Err(format!("unknown device {other}")),
    };
    let runtime = format!("{:?}/{:?}", config.device, config.dtype);
    let started = Instant::now();
    let model = any_tts::load_model(config).map_err(|e| e.to_string())?;
    eprintln!(
        "{device}: loaded on {runtime} in {}ms",
        started.elapsed().as_millis()
    );
    Ok(model)
}

fn cpu_ms() -> u128 {
    // SAFETY: getrusage fills the zeroed struct it is handed.
    let usage = unsafe {
        let mut usage: libc::rusage = std::mem::zeroed();
        libc::getrusage(libc::RUSAGE_SELF, &mut usage);
        usage
    };
    let ms = |t: libc::timeval| t.tv_sec as u128 * 1000 + t.tv_usec as u128 / 1000;
    ms(usage.ru_utime) + ms(usage.ru_stime)
}

fn load_average() -> f64 {
    let mut load = [0.0f64; 1];
    // SAFETY: the buffer holds the one sample asked for.
    unsafe { libc::getloadavg(load.as_mut_ptr(), 1) };
    load[0]
}

fn speak(model: &Model, text: &str) -> Result<f32, String> {
    let request = any_tts::SynthesisRequest::new(text)
        .with_voice(VOICE)
        .with_temperature(1.0);
    model
        .synthesize(&request)
        .map(|audio| audio.duration_secs())
        .map_err(|e| e.to_string())
}

fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let rounds: usize = args
        .next()
        .and_then(|r| r.parse().ok())
        .ok_or("usage: kokoro-rtf-probe <rounds> <metal|cpu>...")?;
    let mut models = Vec::new();
    for device in args {
        let model = load(&device)?;
        // The first synthesis compiles the GPU kernels; every sentence is
        // warmed once so no measured run pays for a new shape.
        for text in SENTENCES {
            speak(&model, text)?;
        }
        models.push((device, model));
    }
    println!("device,round,sentence,audio_s,wall_ms,rtf,cpu_ms,load1");
    for round in 0..rounds {
        for (index, text) in SENTENCES.iter().enumerate() {
            for turn in 0..models.len() {
                let (device, model) = &models[(turn + round + index) % models.len()];
                let cpu_before = cpu_ms();
                let started = Instant::now();
                let audio = speak(model, text)?;
                let wall = started.elapsed().as_millis();
                println!(
                    "{device},{round},{index},{audio:.2},{wall},{:.3},{},{:.1}",
                    wall as f32 / 1000.0 / audio,
                    cpu_ms() - cpu_before,
                    load_average()
                );
            }
        }
    }
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
