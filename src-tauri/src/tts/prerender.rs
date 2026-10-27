//! A fixed line, rendered before it is needed and kept on disk.
//!
//! The launch greeting is the same words every time, so there is no reason to
//! synthesise it while the person waits. It is rendered once, in the engine,
//! voice and rate in force, and stored under a key made of exactly those
//! things plus the text. A later launch whose key matches plays the file the
//! moment the bar appears: no `say` spawn, no model run, no ElevenLabs call.
//! A change of engine, voice or rate is a different key, so the old file is
//! simply never matched again; it is replaced in the background the next time
//! the cache is refreshed.
//!
//! Engines that render to a file are cached; the rest (Replicate and
//! Chatterbox, slow cloud renders that Juno does not otherwise prefetch) are
//! left to the ordinary speaking path. If the render is not ready when the
//! line is wanted, the caller speaks it the ordinary way.
//!
//! The Mac's voice is rendered by `say -o` with no `-v`, so it is the System
//! Voice (Siri) exactly as `say` would speak it, stored as 16-bit WAV and
//! played through the in-process player (`pcm_player`), which honours the
//! Speaker setting. When the System Voice is not a Siri voice nothing is
//! rendered or played for it (`voices::mac_voice_allowed`). Every other
//! engine's bytes are stored as they came and played the way that engine
//! always plays.

use crate::settings::AudioSettings;
use crate::tts::rate;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex as StdMutex};
use tauri::{AppHandle, Manager};
use tracing::{debug, info, warn};

/// Bump to invalidate every cached file (a change in how they are rendered).
/// 2: the Mac's voice is `say -o` only; version 1 files may hold a non-Siri
/// voice rendered in-process.
const CACHE_VERSION: u32 = 2;

/// Cached files start with this, so stale ones can be cleared without
/// touching anything else in the directory.
const FILE_PREFIX: &str = "line-";

/// What a cached line was rendered with. Two keys are equal exactly when the
/// audio would be the same.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineKey {
    pub engine: String,
    /// Whatever names the voice for that engine (for Supertonic, the server
    /// and its own speed trim too).
    pub voice: String,
    /// The rate the engine will use, to the thousandth.
    pub rate_milli: i64,
    pub text: String,
}

impl LineKey {
    /// The file name, without extension. A hash, so voice names with spaces
    /// and brackets never reach the file system.
    pub fn stem(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(
            format!(
                "v{}|{}|{}|{}|{}",
                CACHE_VERSION, self.engine, self.voice, self.rate_milli, self.text
            )
            .as_bytes(),
        );
        let digest = hasher.finalize();
        let hex: String = digest
            .iter()
            .take(16)
            .map(|byte| format!("{byte:02x}"))
            .collect();
        format!("{FILE_PREFIX}{hex}")
    }

    /// The Mac's voice is cached as PCM for the in-process player.
    pub fn is_pcm(&self) -> bool {
        self.engine == "system"
    }

    pub fn file_name(&self) -> String {
        if self.is_pcm() {
            format!("{}.wav", self.stem())
        } else {
            format!("{}.audio", self.stem())
        }
    }
}

/// The key for `text` under these settings, or `None` when the line is not
/// cached for this engine (speech off, or an engine rendered elsewhere).
///
/// `elevenlabs_voice` is the ElevenLabs voice id, which lives in the
/// environment rather than in the settings.
pub fn key_for(
    audio: &AudioSettings,
    text: &str,
    elevenlabs_voice: Option<&str>,
) -> Option<LineKey> {
    let engine = audio.tts_provider.trim().to_ascii_lowercase();
    let stored_rate = audio.voice_rate;
    let voice = match engine.as_str() {
        // Always the System Voice: there is no Mac voice name to key on.
        "system" => String::new(),
        "kokoro" => crate::tts::voices::resolve_kokoro_voice(audio.voice_for("kokoro")),
        "supertonic" => format!(
            "{}|{}|{:.3}",
            audio.supertonic_server_url, audio.supertonic_voice, audio.supertonic_speed
        ),
        "elevenlabs" => elevenlabs_voice.unwrap_or_default().to_string(),
        _ => return None,
    };
    let effective = rate::effective(&engine, stored_rate).unwrap_or(rate::DEFAULT_RATE);
    Some(LineKey {
        engine,
        voice,
        rate_milli: (effective * 1000.0).round() as i64,
        text: text.to_string(),
    })
}

/// A rendered line, ready to play.
#[derive(Debug, Clone)]
pub enum Prerendered {
    /// Mono samples for the in-process player.
    Pcm { samples: Arc<Vec<f32>>, rate: f64 },
    /// An engine's own bytes (WAV, MP3...), base64 for the ordinary player.
    Encoded(String),
}

/// The line in memory, under the stem it was loaded for.
static READY: StdMutex<Option<(String, Prerendered)>> = StdMutex::new(None);

/// One refresh at a time; the next one re-reads the settings, so the last
/// change always wins.
static REFRESH: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

fn ready() -> std::sync::MutexGuard<'static, Option<(String, Prerendered)>> {
    READY
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn elevenlabs_voice() -> Option<String> {
    Some(
        std::env::var("ELEVENLABS_VOICE_ID")
            .unwrap_or_else(|_| crate::tts::elevenlabs::DEFAULT_VOICE_ID.to_string()),
    )
}

pub(crate) async fn current_key(app: &AppHandle, text: &str) -> Option<LineKey> {
    let manager = crate::settings::manager::SettingsManager::new(app.clone()).ok()?;
    let audio = manager.get_audio_settings().await.ok()?;
    key_for(&audio, text, elevenlabs_voice().as_deref())
}

fn cache_dir(app: &AppHandle) -> Option<PathBuf> {
    app.path()
        .app_cache_dir()
        .ok()
        .map(|dir| dir.join("speech-cache"))
}

/// The line, if it is rendered for the settings in force right now, waiting
/// for a render already running rather than letting the caller start a second
/// one. See [`JOIN_WAIT`].
pub async fn ready_for(app: &AppHandle, text: &str) -> Option<Prerendered> {
    let key = current_key(app, text).await?;
    // The System Voice may have changed to a non-Siri voice since this was
    // rendered; then it is not played.
    if key.engine == "system" && !crate::tts::voices::mac_voice_allowed().await {
        return None;
    }
    line_for(&key.stem()).await
}

/// How long to wait for a render already in flight. Long enough for a cold
/// Kokoro, which has to load the model before it can synthesise anything
/// (6.9 s observed from a cold cache), and finite so that an engine which
/// never answers cannot keep the line silent for the whole launch.
const JOIN_WAIT: std::time::Duration = std::time::Duration::from_secs(12);

/// The line rendered for `stem`, waiting for a render in flight rather than
/// answering "not ready" while the line is on its way.
///
/// The refresh and the greeting start together at launch, so on a cold cache
/// the greeting asks for the line while it is still being rendered. Answering
/// "not ready" sends the caller to the ordinary speaking path, which
/// synthesises the same words a second time *beside* the first: the two runs
/// compete for the one model, so each is slower than either alone, and the
/// person waits longer for a line that was already being made for them.
/// Waiting for the render already running is never slower than starting
/// another.
///
/// Taking [`REFRESH`] is the wait: `refresh` holds it for exactly as long as
/// the render takes and sets the line before it lets go, so this returns the
/// moment that render is done, or at once when there is none.
async fn line_for(stem: &str) -> Option<Prerendered> {
    // The behaviour under test, before the fix: a line still being rendered
    // is reported as not ready, and the caller synthesises it again.
    let _unused_until_the_fix = JOIN_WAIT;
    let _unused_until_the_fix = &REFRESH;
    loaded(stem)
}

/// The line held in memory, when what is held was rendered for `stem`.
fn loaded(stem: &str) -> Option<Prerendered> {
    ready()
        .as_ref()
        .filter(|(ready_stem, _)| ready_stem.as_str() == stem)
        .map(|(_, audio)| audio.clone())
}

/// Make sure `text` is rendered for the settings in force, in the background.
/// Called at launch and after any change of engine, voice or rate.
pub fn refresh_in_background(app: &AppHandle, text: &'static str) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(e) = refresh(&app, text).await {
            debug!("[Prerender] Not rendered ahead: {e}");
        }
    });
}

async fn refresh(app: &AppHandle, text: &str) -> Result<(), String> {
    let _one = REFRESH.lock().await;
    let Some(key) = current_key(app, text).await else {
        *ready() = None;
        return Ok(());
    };
    if key.engine == "system" && !crate::tts::voices::mac_voice_allowed().await {
        *ready() = None;
        return Err("the System Voice is not a Siri voice".to_string());
    }
    let stem = key.stem();
    if ready().as_ref().is_some_and(|(s, _)| *s == stem) {
        return Ok(());
    }
    let dir = cache_dir(app).ok_or("no cache directory")?;
    let path = dir.join(key.file_name());

    let started = std::time::Instant::now();
    let loaded = if path.exists() {
        load(&path, key.is_pcm()).await
    } else {
        Err("not cached".to_string())
    };
    let audio = match loaded {
        Ok(audio) => {
            debug!(
                "[Prerender] Loaded {} from disk in {} ms",
                key.engine,
                started.elapsed().as_millis()
            );
            audio
        }
        Err(_) => {
            let audio = render(&key).await?;
            store(&dir, &path, &audio).await?;
            clear_others(&dir, &key.file_name()).await;
            info!(
                "[Prerender] Rendered the greeting ahead with {} in {} ms",
                key.engine,
                started.elapsed().as_millis()
            );
            audio
        }
    };
    *ready() = Some((stem, audio));
    Ok(())
}

/// Render the line with the engine the key names.
async fn render(key: &LineKey) -> Result<Prerendered, String> {
    let rate = key.rate_milli as f64 / 1000.0;
    let text = key.text.clone();
    let encoded = match key.engine.as_str() {
        "system" => return render_system(key, rate).await,
        "kokoro" => crate::tts::kokoro::invoke_kokoro_tts(text, key.voice.clone(), rate).await?,
        "elevenlabs" => crate::tts::elevenlabs::invoke_elevenlabs_tts(text, Some(rate)).await?,
        "supertonic" => {
            let mut parts = key.voice.splitn(3, '|');
            let server = parts.next().unwrap_or_default().to_string();
            let voice = parts.next().unwrap_or_default().to_string();
            let trim: f64 = parts
                .next()
                .and_then(|v| v.parse().ok())
                .unwrap_or(crate::tts::supertonic::DEFAULT_SPEED);
            let speed = (trim * rate).clamp(0.5, 2.0);
            crate::tts::supertonic::invoke_supertonic_tts(text, server, voice, speed).await?
        }
        other => return Err(format!("{other} is not rendered ahead")),
    };
    if encoded.starts_with("TTS_") {
        return Err(format!("the engine answered {encoded}"));
    }
    Ok(Prerendered::Encoded(encoded))
}

/// The Mac's voice: `say -o` with no `-v`, which renders the System Voice,
/// Siri included, exactly as `say` would speak it.
async fn render_system(key: &LineKey, rate: f64) -> Result<Prerendered, String> {
    let file = tempfile::Builder::new()
        .prefix("juno-line-")
        .suffix(".wav")
        .tempfile()
        .map_err(|e| format!("no temporary file: {e}"))?;
    let path = file.path().to_path_buf();
    let mut args = vec![
        "-o".to_string(),
        path.to_string_lossy().to_string(),
        "--file-format=WAVE".to_string(),
        "--data-format=LEI16@22050".to_string(),
    ];
    args.extend(crate::tts::system::say_arguments_at(
        &key.text,
        None,
        rate::say_words_per_minute(rate),
    ));
    let status = tokio::process::Command::new("say")
        .args(&args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .await
        .map_err(|e| format!("could not run say: {e}"))?;
    if !status.success() {
        return Err(format!("say -o exited with {status}"));
    }
    let loaded = load(&path, true).await;
    drop(file);
    loaded
}

/// Read a cached file.
async fn load(path: &Path, pcm: bool) -> Result<Prerendered, String> {
    let path = path.to_path_buf();
    tokio::task::spawn_blocking(move || {
        if pcm {
            let (samples, rate) = read_wav(&path)?;
            Ok(Prerendered::Pcm {
                samples: Arc::new(samples),
                rate,
            })
        } else {
            use base64::Engine as _;
            let bytes = std::fs::read(&path).map_err(|e| format!("could not read: {e}"))?;
            if bytes.is_empty() {
                return Err("empty file".to_string());
            }
            Ok(Prerendered::Encoded(
                base64::engine::general_purpose::STANDARD.encode(bytes),
            ))
        }
    })
    .await
    .map_err(|e| format!("load task failed: {e}"))?
}

/// Write a rendered line. Written to a temporary name and renamed, so a
/// half-written file is never found under the real one.
async fn store(dir: &Path, path: &Path, audio: &Prerendered) -> Result<(), String> {
    let dir = dir.to_path_buf();
    let path = path.to_path_buf();
    let audio = audio.clone();
    tokio::task::spawn_blocking(move || {
        std::fs::create_dir_all(&dir).map_err(|e| format!("no cache directory: {e}"))?;
        let partial = path.with_extension("partial");
        match &audio {
            Prerendered::Pcm { samples, rate } => write_wav(&partial, samples, *rate)?,
            Prerendered::Encoded(encoded) => {
                use base64::Engine as _;
                let bytes = base64::engine::general_purpose::STANDARD
                    .decode(encoded)
                    .map_err(|e| format!("not base64: {e}"))?;
                std::fs::write(&partial, bytes).map_err(|e| format!("could not write: {e}"))?;
            }
        }
        std::fs::rename(&partial, &path).map_err(|e| format!("could not rename: {e}"))
    })
    .await
    .map_err(|e| format!("store task failed: {e}"))?
}

/// Remove every cached line except `keep`. Only files this module wrote.
async fn clear_others(dir: &Path, keep: &str) {
    let dir = dir.to_path_buf();
    let keep = keep.to_string();
    let _ = tokio::task::spawn_blocking(move || {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            return;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with(FILE_PREFIX) && name != keep {
                if let Err(e) = std::fs::remove_file(entry.path()) {
                    warn!("[Prerender] Could not remove an old render {name}: {e}");
                }
            }
        }
    })
    .await;
}

fn write_wav(path: &Path, samples: &[f32], rate: f64) -> Result<(), String> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: rate.round().clamp(8_000.0, 192_000.0) as u32,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer =
        hound::WavWriter::create(path, spec).map_err(|e| format!("could not write: {e}"))?;
    for &sample in samples {
        let value = (sample.clamp(-1.0, 1.0) * i16::MAX as f32).round() as i16;
        writer
            .write_sample(value)
            .map_err(|e| format!("could not write: {e}"))?;
    }
    writer
        .finalize()
        .map_err(|e| format!("could not finish: {e}"))
}

/// Mono samples from a 16-bit WAV (the first channel when there are more).
fn read_wav(path: &Path) -> Result<(Vec<f32>, f64), String> {
    let mut reader = hound::WavReader::open(path).map_err(|e| format!("not a WAV: {e}"))?;
    let spec = reader.spec();
    if spec.sample_format != hound::SampleFormat::Int || spec.bits_per_sample != 16 {
        return Err("not 16-bit PCM".to_string());
    }
    let channels = usize::from(spec.channels.max(1));
    let samples: Vec<f32> = reader
        .samples::<i16>()
        .step_by(channels)
        .filter_map(Result::ok)
        .map(|s| s as f32 / i16::MAX as f32)
        .collect();
    if samples.is_empty() {
        return Err("no samples".to_string());
    }
    Ok((samples, f64::from(spec.sample_rate)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn audio(engine: &str) -> AudioSettings {
        AudioSettings {
            tts_provider: engine.to_string(),
            ..AudioSettings::default()
        }
    }

    const LINE: &str = "Juno here. Welcome back.";

    #[test]
    fn the_same_settings_find_the_same_file() {
        let a = key_for(&audio("system"), LINE, None).expect("cached");
        let b = key_for(&audio("system"), LINE, None).expect("cached");
        assert_eq!(a.stem(), b.stem());
        assert!(a.file_name().ends_with(".wav"));
    }

    #[test]
    fn a_new_voice_is_a_new_file() {
        let mut settings = audio("supertonic");
        let before = key_for(&settings, LINE, None).expect("cached").stem();
        settings.supertonic_voice = "F1".to_string();
        let after = key_for(&settings, LINE, None).expect("cached").stem();
        assert_ne!(before, after);
    }

    /// A Mac voice name an older build stored never reaches the render: the
    /// Mac's voice is always the System Voice, so the key ignores it.
    #[test]
    fn a_stored_mac_voice_name_is_not_rendered() {
        let mut named = audio("system");
        named.system_voice = Some("Samantha".to_string());
        let key = key_for(&named, LINE, None).expect("cached");
        assert_eq!(key.voice, "");
        assert_eq!(
            key.stem(),
            key_for(&audio("system"), LINE, None)
                .expect("cached")
                .stem()
        );
    }

    #[test]
    fn a_new_rate_is_a_new_file() {
        let mut settings = audio("system");
        let before = key_for(&settings, LINE, None).expect("cached").stem();
        settings.voice_rate = 1.5;
        let after = key_for(&settings, LINE, None).expect("cached").stem();
        assert_ne!(before, after);
    }

    #[test]
    fn a_new_engine_is_a_new_file() {
        let system = key_for(&audio("system"), LINE, None).expect("cached");
        let kokoro = key_for(&audio("kokoro"), LINE, None).expect("cached");
        assert_ne!(system.stem(), kokoro.stem());
        assert!(kokoro.file_name().ends_with(".audio"));
    }

    #[test]
    fn new_words_are_a_new_file() {
        let a = key_for(&audio("system"), LINE, None).expect("cached");
        let b = key_for(&audio("system"), "Hello.", None).expect("cached");
        assert_ne!(a.stem(), b.stem());
    }

    #[test]
    fn the_macs_own_voice_and_no_voice_are_the_same_render() {
        let mut own = audio("system");
        own.system_voice = Some(crate::tts::voices::SYSTEM_DEFAULT_ID.to_string());
        let mut none = audio("system");
        none.system_voice = None;
        assert_eq!(
            key_for(&own, LINE, None).expect("cached").stem(),
            key_for(&none, LINE, None).expect("cached").stem()
        );
    }

    #[test]
    fn an_elevenlabs_voice_change_is_a_new_file() {
        let a = key_for(&audio("elevenlabs"), LINE, Some("voice-a")).expect("cached");
        let b = key_for(&audio("elevenlabs"), LINE, Some("voice-b")).expect("cached");
        assert_ne!(a.stem(), b.stem());
    }

    #[test]
    fn silence_and_cloud_only_engines_are_not_cached() {
        assert_eq!(key_for(&audio("off"), LINE, None), None);
        assert_eq!(key_for(&audio(""), LINE, None), None);
        assert_eq!(key_for(&audio("replicate"), LINE, None), None);
        assert_eq!(key_for(&audio("chatterbox"), LINE, None), None);
    }

    /// The launch race: the refresh and the greeting start together, so on a
    /// cold cache the greeting asks for the line while it is still being
    /// rendered. It used to be told "not ready" and synthesise the same words
    /// a second time beside the first, both competing for the one model.
    ///
    /// One test covering three cases rather than three tests, and the only
    /// test here that touches the module's statics: `READY` and `REFRESH` are
    /// process-global and tests in a binary run in parallel, so three tests
    /// sharing them would race each other instead of testing anything.
    #[tokio::test]
    async fn the_greeting_waits_for_the_render_already_running() {
        const STEM: &str = "line-under-test";
        *ready() = None;

        // Nothing is being rendered: the caller is told so at once, and
        // speaks the line the ordinary way rather than standing silent.
        let started = std::time::Instant::now();
        assert!(line_for(STEM).await.is_none());
        assert!(
            started.elapsed() < std::time::Duration::from_secs(1),
            "waited {:?} with no render running",
            started.elapsed()
        );

        // A render in flight for this line, held the way `refresh` holds it.
        let rendering = REFRESH.lock().await;
        let joining = tokio::spawn(async { line_for(STEM).await });
        // Let the caller reach the wait. The assertion below does not depend
        // on it getting there: late, it takes the freed lock and still finds
        // the line. The pause is what makes this exercise the waiting.
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        *ready() = Some((
            STEM.to_string(),
            Prerendered::Encoded("the-rendered-line".to_string()),
        ));
        drop(rendering);

        match joining.await.expect("the waiting caller finished") {
            Some(Prerendered::Encoded(audio)) => assert_eq!(audio, "the-rendered-line"),
            other => panic!("expected the line the render made, got {other:?}"),
        }

        // What is held was rendered for some other line (the engine, voice or
        // rate changed under it), so it is not this one.
        assert!(line_for("line-something-else").await.is_none());
        *ready() = None;
    }

    #[test]
    fn a_wav_round_trips() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("line.wav");
        let samples = vec![0.0, 0.5, -0.5, 0.25];
        write_wav(&path, &samples, 22_050.0).expect("writes");
        let (read, rate) = read_wav(&path).expect("reads");
        assert!((rate - 22_050.0).abs() < f64::EPSILON);
        assert_eq!(read.len(), samples.len());
        for (a, b) in read.iter().zip(samples.iter()) {
            assert!((a - b).abs() < 1e-3);
        }
    }
}
