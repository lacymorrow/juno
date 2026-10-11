//! Juno's voice: the voices the engine that is speaking actually has.
//!
//! The list is a function of the active engine, computed here. Rust owns which
//! voices exist, which one is active, whether it is valid for the current
//! engine and what the default is. React draws that. A control that names a
//! behaviour it is not wired to is the defect this codebase keeps producing.
//!
//! A voice id belongs to the engine that named it. "af_heart" means nothing to
//! `say`, so the choice is stored per engine (`settings::AudioSettings::voice_for`)
//! and changing engine resolves to a voice the new engine has rather than
//! carrying a dangling id across.
//!
//! # The Mac's voice is Siri, or nothing
//!
//! Juno only ever speaks in a Siri voice or in one of its providers' voices.
//! On the Mac that means one row, "Your Mac's voice", spoken by `say` with no
//! `-v`: the System Voice chosen in System Settings > Accessibility > Read &
//! Speak. That is the only route to a Siri voice. `say -v` reaches only the
//! MacinTalk and Vocalizer voices (Samantha, Ava and the rest), and
//! AVFoundation lists no Siri voices to a third-party app, so Juno names no
//! Mac voice at all. A named voice stored by an older build resolves to the
//! one row.
//!
//! When the System Voice is positively read as a non-Siri voice, the Mac's
//! voice is not used anywhere: not as the engine, not as a fallback, not as
//! a stand-in (see [`mac_voice_is_siri_or_unknown`]). A reading that fails is
//! trusted, because `say` with no `-v` speaks whatever the System Voice is and
//! Apple's default on current macOS is a Siri voice.

// The audition states, aliased so each emit reads as one line.
use crate::constants::events::juno_voice::{
    AUDITION as AUDITION_EVENT, DONE as EVENT_DONE, ENGINE_READY as ENGINE_READY_EVENT,
    FAILED as EVENT_FAILED, PREPARING as EVENT_PREPARING, SPEAKING as EVENT_SPEAKING,
};
use crate::state::AppState;
use serde::Serialize;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex as StdMutex;
use std::time::{Duration, Instant, SystemTime};
use tauri::{AppHandle, Emitter, State};
use tracing::{error, info, warn};

/// Which audition is current.
///
/// Tapping a second voice stops the first, and `say` exits non-zero on the
/// signal that stopped it. That is the interaction working, not a failure, so
/// a sample that has been taken over reports nothing: without this, browsing
/// voices quickly would put an error on screen for every one you interrupted.
static AUDITION: AtomicU64 = AtomicU64::new(0);

/// The sentence a voice says when it is selected. Short on purpose: an
/// audition that runs long gets talked over by the next tap.
pub const VOICE_SAMPLE_TEXT: &str = "Hi, I'm Juno. This is how I sound.";

/// Entry id for "whichever voice the Mac is set to".
pub const SYSTEM_DEFAULT_ID: &str = "system_default";
/// The stored engine that means silence.
pub const OFF_PROVIDER: &str = "off";

/// The Kokoro voice the model itself falls back to, and the one embedding the
/// downloader always fetches. See [`installed_kokoro_voices`].
pub const KOKORO_DEFAULT_VOICE: &str = "af_heart";
/// Supertonic ships two voices and names them in its own API.
pub const SUPERTONIC_DEFAULT_VOICE: &str = "M1";

// ---------------------------------------------------------------------------
// The Mac's voice
// ---------------------------------------------------------------------------

/// One row an engine offers, before it becomes a picker row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderVoice {
    /// What gets stored, and what the engine is handed.
    pub id: String,
    /// What the row is called.
    pub name: String,
    /// One line underneath it.
    pub descriptor: String,
}

/// What an engine's voices are, as far as Juno can tell from here.
///
/// `Elsewhere` is not an error and not an empty list: it is the honest answer
/// for an engine whose voices are chosen somewhere Juno cannot see. Showing
/// rows in that case would be offering choices the engine cannot honour.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderVoices {
    Known(Vec<ProviderVoice>),
    Elsewhere(&'static str),
}

/// Everything the resolver needs to know about this machine.
///
/// Gathered once per command rather than per voice: reading the System Voice
/// runs a process and listing Kokoro's voices reads a directory.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct VoiceInventory {
    /// The Kokoro voice embeddings on disk, by id.
    pub kokoro: Vec<String>,
    /// Why Kokoro could not get ready, when its last load failed.
    pub kokoro_problem: Option<String>,
    /// What the Mac's own voice is, as far as its settings say.
    pub system_voice: SystemVoice,
}

/// The voice `say` uses when it is given no `-v`: the one chosen under System
/// Settings > Accessibility > Read & Speak.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SystemVoice {
    /// The settings name a voice, and it is not a Siri voice.
    Other,
    /// The chosen voice is a Siri voice.
    Siri,
    /// Nothing could be read, or nothing is recorded. `say` then speaks
    /// Apple's default, which is a Siri voice on current macOS.
    #[default]
    Unset,
}

/// The gate: may Juno speak with the Mac's own voice at all?
///
/// Yes for a Siri voice, and yes when the reading found nothing (a failed
/// read is trusted: `say` speaks the System Voice and Juno cannot know
/// better). No only when the settings positively name a voice that is not
/// Siri. Then the Mac's voice is never used: not as the engine, not as a
/// fallback and not as a stand-in while Kokoro downloads. Pure.
pub fn mac_voice_is_siri_or_unknown(voice: SystemVoice) -> bool {
    voice != SystemVoice::Other
}

/// Read the System Voice choice. Pure, so it is testable without a Mac.
///
/// The id looks like `com.apple.siri.natural.Simone`,
/// `com.apple.speech.synthesis.voice.custom.siri.*` or
/// `com.apple.voice.premium.en-US.Zoe`; the name is the display name.
pub fn classify_system_voice(id: Option<&str>, name: Option<&str>) -> SystemVoice {
    let id = id.map(str::trim).filter(|v| !v.is_empty());
    let name = name.map(str::trim).filter(|v| !v.is_empty());
    if id.is_none() && name.is_none() {
        return SystemVoice::Unset;
    }
    let says_siri = |v: Option<&str>| v.is_some_and(|v| v.to_ascii_lowercase().contains("siri"));
    if says_siri(id) || says_siri(name) {
        SystemVoice::Siri
    } else {
        SystemVoice::Other
    }
}

/// What the Mac row's link says. It opens Read & Speak, where the voice is set.
pub const READ_AND_SPEAK_LINK: &str = "Open Read & Speak";

/// System Settings > Accessibility > Read & Speak, most specific first. The
/// anchor is a best guess for macOS 26; the second opens Accessibility.
pub const READ_AND_SPEAK_URLS: [&str; 2] = [
    "x-apple.systempreferences:com.apple.Accessibility-Settings.extension?SpokenContent",
    "x-apple.systempreferences:com.apple.Accessibility-Settings.extension",
];

/// The one row the Mac offers: its own voice, which is a Siri voice when the
/// person picked one in Read & Speak.
fn system_voice_row(system_voice: SystemVoice) -> ProviderVoice {
    let descriptor = if mac_voice_is_siri_or_unknown(system_voice) {
        "Uses the Siri voice you pick in Read & Speak."
    } else {
        "Pick a Siri voice in Read & Speak and Juno will use it."
    };
    ProviderVoice {
        id: SYSTEM_DEFAULT_ID.to_string(),
        name: "Your Mac's voice".to_string(),
        descriptor: descriptor.to_string(),
    }
}

/// The `(language, voice id)` for the first of `languages` that has a
/// selection in the `SpokenContentDefaultVoiceSelectionsByLanguage` value,
/// given as JSON (`plutil -extract ... json`). The value alternates a
/// language code and a dict:
/// `["en", {"boundLanguage": "en", "voiceId": "com.apple..."}]`. Pure.
pub fn parse_voice_selections(json: &str, languages: &[String]) -> Option<(String, String)> {
    let value: serde_json::Value = serde_json::from_str(json).ok()?;
    let items = value.as_array()?;
    let mut by_language: Vec<(String, String)> = Vec::new();
    for pair in items.chunks(2) {
        let [lang, selection] = pair else { continue };
        let (Some(lang), Some(id)) = (
            lang.as_str(),
            selection.get("voiceId").and_then(|v| v.as_str()),
        ) else {
            continue;
        };
        if !id.trim().is_empty() {
            by_language.push((lang.to_string(), id.trim().to_string()));
        }
    }
    languages.iter().find_map(|wanted| {
        by_language
            .iter()
            .find(|(lang, _)| lang.eq_ignore_ascii_case(wanted))
            .cloned()
    })
}

/// The System Voice as read from the Mac's settings.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SpokenContent {
    pub voice_id: Option<String>,
    pub voice_name: Option<String>,
}

/// The System Voice from both places it can live. The macOS 26 selection wins;
/// the old domain is what older macOS (and a Mac never touched) still has. Pure.
pub fn spoken_content_from(
    selection: Option<(String, String)>,
    old_id: Option<String>,
    old_name: Option<String>,
) -> SpokenContent {
    match selection {
        Some((_, id)) => SpokenContent {
            voice_id: Some(id),
            voice_name: None,
        },
        None => SpokenContent {
            voice_id: old_id,
            voice_name: old_name,
        },
    }
}

/// Where the System Voice reading came from, for the log.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpokenSource {
    /// `defaults export`, answered by cfprefsd.
    Preferences,
    /// The preferences file, read directly (needs Full Disk Access).
    File,
    /// The pre-macOS 26 `com.apple.speech.voice.prefs` domain.
    OldDomain,
    /// Nothing answered.
    Nothing,
}

/// Where the System Voice choice is written, newest first.
pub fn spoken_prefs_files() -> Vec<PathBuf> {
    let Some(home) = std::env::var_os("HOME") else {
        return Vec::new();
    };
    let prefs = PathBuf::from(home).join("Library/Preferences");
    vec![
        prefs.join("com.apple.Accessibility.plist"),
        prefs.join("com.apple.speech.voice.prefs.plist"),
    ]
}

/// The key the macOS 26 System Voice lives under.
const SELECTIONS_KEY: &str = "SpokenContentDefaultVoiceSelectionsByLanguage";

/// Read the System Voice. Blocking; runs `defaults` and `plutil`, all
/// read-only.
///
/// cfprefsd first: `com.apple.Accessibility.plist` is protected, so an app
/// without Full Disk Access usually cannot open the file, while asking the
/// preferences daemon for the domain can still answer. The file is the second
/// try, and the old domain the third.
#[cfg(target_os = "macos")]
pub fn read_spoken_content_blocking() -> (SpokenContent, SpokenSource) {
    use std::io::Write;
    use std::process::{Command, Stdio};

    fn finish(output: std::process::Output) -> Option<String> {
        output
            .status
            .success()
            .then(|| String::from_utf8_lossy(&output.stdout).trim().to_string())
            .filter(|v| !v.is_empty())
    }
    fn run(cmd: &str, args: &[&str]) -> Option<String> {
        finish(Command::new(cmd).args(args).output().ok()?)
    }
    fn extract(file: &std::path::Path, key: &str) -> Option<String> {
        run(
            "plutil",
            &["-extract", key, "json", "-o", "-", &file.to_string_lossy()],
        )
    }
    fn through_prefs(domain: &str, key: &str) -> Option<String> {
        let plist = Command::new("defaults")
            .args(["export", domain, "-"])
            .stderr(Stdio::null())
            .output()
            .ok()
            .filter(|output| output.status.success())?
            .stdout;
        let mut child = Command::new("plutil")
            .args(["-extract", key, "json", "-o", "-", "-"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .ok()?;
        if let Some(mut stdin) = child.stdin.take() {
            // plutil reads all of its input before it writes anything, so
            // writing first and then waiting cannot deadlock.
            let _ = stdin.write_all(&plist);
        }
        finish(child.wait_with_output().ok()?)
    }

    let mut languages: Vec<String> = Vec::new();
    if let Some(home) = std::env::var_os("HOME") {
        let global = PathBuf::from(home).join("Library/Preferences/.GlobalPreferences.plist");
        if let Some(Ok(list)) =
            extract(&global, "AppleLanguages").map(|j| serde_json::from_str::<Vec<String>>(&j))
        {
            languages.extend(
                list.iter()
                    .filter_map(|l| l.split('-').next())
                    .map(str::to_string),
            );
        }
    }
    languages.push("en".to_string());

    let selected = |json: Option<String>| json.and_then(|j| parse_voice_selections(&j, &languages));
    if let Some(selection) = selected(through_prefs("com.apple.Accessibility", SELECTIONS_KEY)) {
        return (
            spoken_content_from(Some(selection), None, None),
            SpokenSource::Preferences,
        );
    }
    let from_file = spoken_prefs_files()
        .first()
        .and_then(|file| extract(file, SELECTIONS_KEY));
    if let Some(selection) = selected(from_file) {
        return (
            spoken_content_from(Some(selection), None, None),
            SpokenSource::File,
        );
    }
    let old = |key: &str| run("defaults", &["read", "com.apple.speech.voice.prefs", key]);
    let content = spoken_content_from(None, old("SelectedVoiceID"), old("SelectedVoiceName"));
    let source = if content.voice_id.is_some() || content.voice_name.is_some() {
        SpokenSource::OldDomain
    } else {
        SpokenSource::Nothing
    };
    (content, source)
}

/// How long a reading of the System Voice is trusted without looking again.
pub const MAC_VOICE_FRESH_FOR: Duration = Duration::from_secs(10);

/// Whether a reading of the System Voice must be redone before it is used.
/// The person can change it at any time, so a reading is stale when the
/// preferences file has been written since (its modification time moved), or
/// when it is simply old. Pure.
pub fn reading_is_stale(
    age: Duration,
    mtime_when_read: Option<SystemTime>,
    mtime_now: Option<SystemTime>,
) -> bool {
    age >= MAC_VOICE_FRESH_FOR || mtime_when_read != mtime_now
}

/// Which voice will actually speak, and whether that is the one asked for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoiceResolution {
    /// The voice in force. `None` means the engine's own default.
    pub voice: Option<String>,
    /// True when something was stored that this engine cannot honour.
    pub substituted: bool,
}

// ---------------------------------------------------------------------------
// The other engines' voices
// ---------------------------------------------------------------------------

/// "af_heart" is the voice called Heart.
fn kokoro_voice_name(id: &str) -> String {
    let stem = id.split_once('_').map(|(_, rest)| rest).unwrap_or(id);
    let mut characters = stem.chars();
    match characters.next() {
        Some(first) => first.to_uppercase().collect::<String>() + characters.as_str(),
        None => id.to_string(),
    }
}

/// Kokoro names its voices `<language><gender>_<name>`, so the accent and the
/// gender are both in the id and neither is a guess about how it sounds.
fn kokoro_voice_descriptor(id: &str) -> String {
    let mut prefix = id.chars();
    let language = match prefix.next() {
        Some('a') => "American English",
        Some('b') => "British English",
        Some('e') => "Spanish",
        Some('f') => "French",
        Some('h') => "Hindi",
        Some('i') => "Italian",
        Some('j') => "Japanese",
        Some('p') => "Brazilian Portuguese",
        Some('z') => "Mandarin",
        _ => "",
    };
    let gender = match prefix.next() {
        Some('f') => "female",
        Some('m') => "male",
        _ => "",
    };
    match (language.is_empty(), gender.is_empty()) {
        (false, false) => format!("{language}, {gender}."),
        (false, true) => format!("{language}."),
        _ => "A Kokoro voice.".to_string(),
    }
}

/// Kokoro's rows: one per embedding on disk, the model's own default first.
pub fn kokoro_voices(embeddings: &[String]) -> Vec<ProviderVoice> {
    let mut ids: Vec<String> = embeddings.to_vec();
    ids.sort();
    ids.dedup();
    // The model's own default goes to the top. It is the one embedding the
    // downloader always fetches, so it is the one that is always there. The
    // sort is stable, so the rest stay alphabetical.
    ids.sort_by_key(|id| id.as_str() != KOKORO_DEFAULT_VOICE);
    ids.into_iter()
        .map(|id| ProviderVoice {
            name: kokoro_voice_name(&id),
            descriptor: kokoro_voice_descriptor(&id),
            id,
        })
        .collect()
}

/// Supertonic's two voices, named the way its API names them.
pub fn supertonic_voices() -> Vec<ProviderVoice> {
    vec![
        ProviderVoice {
            id: "M1".to_string(),
            name: "M1".to_string(),
            descriptor: "Male.".to_string(),
        },
        ProviderVoice {
            id: "F1".to_string(),
            name: "F1".to_string(),
            descriptor: "Female.".to_string(),
        },
    ]
}

// ---------------------------------------------------------------------------
// The engine, and its voices
// ---------------------------------------------------------------------------

/// The engine's name as a person would say it.
pub fn engine_label(engine: &str) -> &'static str {
    match engine.to_ascii_lowercase().as_str() {
        "system" => "Your Mac",
        "kokoro" => "Kokoro",
        "elevenlabs" => "ElevenLabs",
        "replicate" => "Replicate",
        "chatterbox" => "Chatterbox",
        "supertonic" => "Supertonic",
        OFF_PROVIDER => "Nothing",
        _ => "This engine",
    }
}

/// The engines Juno can speak with, in the order the picker offers them.
///
/// Silence is not one of them. It is the first row of the voice list, so it
/// is not offered twice.
pub const ENGINES: &[&str] = &[
    "system",
    "kokoro",
    "elevenlabs",
    "replicate",
    "chatterbox",
    "supertonic",
];

/// What an engine needs before it can make a sound.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct EngineNeeds {
    /// An API key from an account somewhere.
    pub account_key: bool,
    /// The network, every time it speaks.
    pub network: bool,
    /// A download before its first word.
    pub download: bool,
    /// A server somebody has to start.
    pub server: bool,
}

impl EngineNeeds {
    pub fn nothing(self) -> bool {
        self == Self::default()
    }
}

/// What each engine needs. The Mac's own voice needs none of it, which is
/// why every fallback chain ends there whenever it is a Siri voice.
pub fn engine_needs(engine: &str) -> EngineNeeds {
    match engine.to_ascii_lowercase().as_str() {
        "system" | OFF_PROVIDER => EngineNeeds::default(),
        "kokoro" => EngineNeeds {
            download: true,
            ..EngineNeeds::default()
        },
        "supertonic" => EngineNeeds {
            server: true,
            ..EngineNeeds::default()
        },
        _ => EngineNeeds {
            account_key: true,
            network: true,
            ..EngineNeeds::default()
        },
    }
}

/// One engine the picker offers.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct EngineOption {
    pub id: String,
    pub name: String,
}

pub fn engine_options() -> Vec<EngineOption> {
    ENGINES
        .iter()
        .map(|id| EngineOption {
            id: id.to_string(),
            name: engine_label(id).to_string(),
        })
        .collect()
}

/// The engine whose voices the pane lists.
///
/// Silence is the engine switched off, but the voices are still the ones that
/// come back when speech does: the engine that was speaking before, or the
/// default when nothing was.
pub fn engine_in_force(provider: &str, before_silent: Option<&str>) -> String {
    if !is_silent(provider) {
        return provider.to_string();
    }
    before_silent
        .map(str::trim)
        .filter(|engine| ENGINES.iter().any(|e| e.eq_ignore_ascii_case(engine)))
        .unwrap_or(crate::constants::settings::defaults::TTS_PROVIDER)
        .to_ascii_lowercase()
}

/// Whether this stored engine means Juno does not speak.
pub fn is_silent(provider: &str) -> bool {
    provider.trim().is_empty() || provider.eq_ignore_ascii_case(OFF_PROVIDER)
}

/// [`engine_in_force`] for what is stored.
fn stored_engine(audio: &crate::settings::AudioSettings) -> String {
    engine_in_force(&audio.tts_provider, audio.engine_before_silent.as_deref())
}

/// What turning speech off or on writes: the engine in force, and the one
/// remembered for the way back. Pure, so the round trip is testable.
pub fn silence_change(
    silent: bool,
    provider: &str,
    before_silent: Option<&str>,
) -> (String, Option<String>) {
    if silent {
        let remembered = if is_silent(provider) {
            before_silent.map(str::to_string)
        } else {
            Some(provider.to_string())
        };
        (OFF_PROVIDER.to_string(), remembered)
    } else {
        (engine_in_force(provider, before_silent), None)
    }
}

/// What this engine offers.
pub fn provider_voices(engine: &str, inventory: &VoiceInventory) -> ProviderVoices {
    match engine.to_ascii_lowercase().as_str() {
        // One row, never a named voice: see the module docs.
        "system" => ProviderVoices::Known(vec![system_voice_row(inventory.system_voice)]),
        "kokoro" => ProviderVoices::Known(kokoro_voices(&inventory.kokoro)),
        "supertonic" => ProviderVoices::Known(supertonic_voices()),
        "elevenlabs" => ProviderVoices::Elsewhere(
            "ElevenLabs speaks with the voice set on your ElevenLabs account, not here.",
        ),
        "replicate" => {
            ProviderVoices::Elsewhere("Replicate speaks with the one voice its model ships with.")
        }
        "chatterbox" => ProviderVoices::Elsewhere(
            "Chatterbox copies the reference audio you give it under Providers.",
        ),
        _ => ProviderVoices::Elsewhere("Juno does not know which voices this engine has."),
    }
}

/// This engine's default voice.
pub fn default_voice(engine: &str, inventory: &VoiceInventory) -> Option<String> {
    match engine.to_ascii_lowercase().as_str() {
        "system" => Some(SYSTEM_DEFAULT_ID.to_string()),
        "kokoro" => {
            let rows = kokoro_voices(&inventory.kokoro);
            rows.iter()
                .find(|row| row.id == KOKORO_DEFAULT_VOICE)
                .or_else(|| rows.first())
                .map(|row| row.id.clone())
        }
        "supertonic" => Some(SUPERTONIC_DEFAULT_VOICE.to_string()),
        _ => None,
    }
}

/// Which voice this engine will speak in, given what was stored for it.
///
/// A stored id this engine does not have resolves to the engine's default
/// rather than being passed through: handing Kokoro an embedding that is not
/// on disk is silence.
pub fn resolve_voice(
    engine: &str,
    inventory: &VoiceInventory,
    stored: Option<&str>,
) -> VoiceResolution {
    // The Mac has one voice, its own. A named voice an older build stored
    // ("Samantha") is not a choice that went missing, it is a voice Juno no
    // longer speaks in, so it resolves quietly to the one row.
    if engine.eq_ignore_ascii_case("system") {
        return VoiceResolution {
            voice: Some(SYSTEM_DEFAULT_ID.to_string()),
            substituted: false,
        };
    }

    let stored = stored.map(str::trim).filter(|voice| !voice.is_empty());
    match provider_voices(engine, inventory) {
        ProviderVoices::Known(rows) => {
            // Nothing to check against. Juno cannot call the stored choice
            // wrong when it cannot see a single one of the engine's voices,
            // and replacing it with nothing would throw away a choice that is
            // probably still good once the engine has its files.
            if rows.is_empty() {
                return VoiceResolution {
                    voice: stored.map(str::to_string),
                    substituted: false,
                };
            }
            if let Some(stored) = stored {
                if rows.iter().any(|row| row.id == stored) {
                    return VoiceResolution {
                        voice: Some(stored.to_string()),
                        substituted: false,
                    };
                }
            }
            let fallback = default_voice(engine, inventory);
            VoiceResolution {
                substituted: stored.is_some() && stored != fallback.as_deref(),
                voice: fallback,
            }
        }
        // Not Juno's choice to make, so the stored value is passed through
        // untouched rather than second-guessed.
        ProviderVoices::Elsewhere(_) => VoiceResolution {
            voice: stored.map(str::to_string),
            substituted: false,
        },
    }
}

/// The Kokoro voice that will actually load.
///
/// Called on the speaking path, where there is no settings window open to have
/// resolved anything: `any_tts` loads `voices/<id>.pt` straight off disk and
/// never fetches a missing one, so an id with no file is not a slow voice, it
/// is an error and then silence.
pub fn resolve_kokoro_voice(stored: Option<&str>) -> String {
    let inventory = VoiceInventory {
        kokoro: installed_kokoro_voices(),
        ..VoiceInventory::default()
    };
    resolve_voice("kokoro", &inventory, stored)
        .voice
        .unwrap_or_else(|| KOKORO_DEFAULT_VOICE.to_string())
}

// ---------------------------------------------------------------------------
// What the pane draws
// ---------------------------------------------------------------------------

/// What the picker draws, one row each.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct JunoVoiceOption {
    /// `system_default`, or an id the active engine knows.
    pub id: String,
    /// `system` or `voice`. The row draws itself from this.
    pub kind: String,
    pub name: String,
    pub descriptor: String,
    /// True for the row that is in force right now.
    pub selected: bool,
    /// Whether selecting this row makes a sound. Silence does not audition.
    pub speaks: bool,
    /// A link under the row, when the voice is set somewhere else: the Mac
    /// row opens Read & Speak (`open_read_and_speak`). The words to show.
    pub link: Option<String>,
}

/// The whole answer the Audio pane renders: the rows, the engine they belong
/// to, and anything that has to be said rather than shown.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct JunoVoiceList {
    /// The stored engine. `off` means Juno is silent.
    pub provider: String,
    /// Juno does not speak. The rows still show the voice that speaks when
    /// it does again.
    pub silent: bool,
    /// The engine these rows belong to.
    pub engine: String,
    /// That engine's name as a person would say it.
    pub engine_label: String,
    pub options: Vec<JunoVoiceOption>,
    /// One sentence when there is something to say instead of rows: an engine
    /// whose voices are chosen elsewhere, an engine with nothing installed
    /// yet, or a stored choice that could not be honoured.
    pub note: Option<String>,
    /// The engines the advanced picker offers, so the pane holds no list of
    /// engine names of its own.
    pub engines: Vec<EngineOption>,
    /// The speed row: present only when the engine in force can change speed
    /// and Juno is not silent, so the pane never shows a control that does
    /// nothing. Filled in by [`JunoVoiceList::with_rate`].
    pub speed: Option<crate::tts::rate::VoiceSpeed>,
}

impl JunoVoiceList {
    /// Attach the speed row for the stored rate.
    pub fn with_rate(mut self, stored_rate: f64) -> Self {
        self.speed = if self.silent {
            None
        } else {
            crate::tts::rate::speed_for(&self.engine, stored_rate)
        };
        self
    }
}

/// Build the rows.
pub fn voice_list(
    provider: &str,
    engine: &str,
    inventory: &VoiceInventory,
    resolution: &VoiceResolution,
) -> JunoVoiceList {
    let silent = is_silent(provider);
    let engine = engine.to_string();

    let mut options = Vec::new();

    let mut note: Option<String> = None;

    match provider_voices(&engine, inventory) {
        ProviderVoices::Known(rows) if !rows.is_empty() => {
            for row in rows {
                let is_mac_row = row.id == SYSTEM_DEFAULT_ID;
                options.push(JunoVoiceOption {
                    selected: resolution.voice.as_deref() == Some(row.id.as_str()),
                    kind: if is_mac_row { "system" } else { "voice" }.to_string(),
                    // A Mac voice that is not Siri is never spoken, so its
                    // row plays nothing; the link is how to fix it.
                    speaks: !is_mac_row || mac_voice_is_siri_or_unknown(inventory.system_voice),
                    link: is_mac_row.then(|| READ_AND_SPEAK_LINK.to_string()),
                    id: row.id,
                    name: row.name,
                    descriptor: row.descriptor,
                });
            }
        }
        // Kokoro downloads its model and voices as it loads, which starts the
        // moment it is chosen. Until that lands there is nothing to list, and
        // if it failed the reason is the useful thing to show.
        ProviderVoices::Known(_) => {
            note = Some(match &inventory.kokoro_problem {
                Some(problem) => format!(
                    "{} could not get ready: {}.",
                    engine_label(&engine),
                    problem.trim().trim_end_matches('.')
                ),
                None => format!(
                    "{} is getting its voices ready. They show up here when it is done.",
                    engine_label(&engine)
                ),
            });
        }
        ProviderVoices::Elsewhere(reason) => note = Some(reason.to_string()),
    }

    // Said once, and only when there are rows to be wrong about: the list
    // lights the voice that will actually speak, and this is why it moved.
    if note.is_none() && resolution.substituted {
        note = Some(format!(
            "The voice that was chosen is not on this Mac any more, so {} is speaking instead.",
            resolution.voice.as_deref().unwrap_or("its default voice")
        ));
    }

    JunoVoiceList {
        provider: provider.to_string(),
        silent,
        engine_label: engine_label(&engine).to_string(),
        engines: engine_options(),
        engine,
        options,
        note,
        speed: None,
    }
}

// ---------------------------------------------------------------------------
// Reading this machine
// ---------------------------------------------------------------------------

/// The last reading of the System Voice: when, the preferences file's
/// modification time then, and what it said.
static MAC_VOICE: StdMutex<Option<(Instant, Option<SystemTime>, SystemVoice)>> =
    StdMutex::new(None);

/// Where the last reading came from, so a change is logged once.
#[cfg(target_os = "macos")]
static MAC_VOICE_SOURCE: StdMutex<Option<(SpokenSource, Option<String>)>> = StdMutex::new(None);

/// When the System Voice preferences were last written: the newest of the
/// Accessibility plist (macOS 26) and the old Speech one. A `stat`, which
/// works even where reading the file does not.
fn spoken_prefs_mtime() -> Option<SystemTime> {
    spoken_prefs_files()
        .iter()
        .filter_map(|path| std::fs::metadata(path).ok()?.modified().ok())
        .max()
}

/// Forget the reading, so the next use reads the System Voice again.
pub fn forget_mac_voice() {
    if let Ok(mut slot) = MAC_VOICE.lock() {
        *slot = None;
    }
}

/// Read the System Voice now, and log where the answer came from when that
/// changes. Only the voice id is logged.
#[cfg(target_os = "macos")]
async fn read_system_voice_now() -> SystemVoice {
    let read = tokio::task::spawn_blocking(read_spoken_content_blocking).await;
    let Ok((spoken, source)) = read else {
        return SystemVoice::Unset;
    };
    let kind = classify_system_voice(spoken.voice_id.as_deref(), spoken.voice_name.as_deref());
    let seen = (source, spoken.voice_id.clone());
    let changed = MAC_VOICE_SOURCE
        .lock()
        .map(|mut last| {
            let changed = last.as_ref() != Some(&seen);
            *last = Some(seen);
            changed
        })
        .unwrap_or(false);
    if changed {
        match source {
            SpokenSource::Nothing => info!(
                "[Voices] Could not read the System Voice; trusting say with no -v ({kind:?})"
            ),
            _ => info!(
                "[Voices] System Voice read through {source:?}: {} ({kind:?})",
                spoken.voice_id.as_deref().unwrap_or("no id")
            ),
        }
    }
    kind
}

#[cfg(not(target_os = "macos"))]
async fn read_system_voice_now() -> SystemVoice {
    SystemVoice::Unset
}

/// The System Voice, from a recent reading when nothing has changed since.
async fn read_system_voice() -> SystemVoice {
    let mtime = spoken_prefs_mtime();
    let cached = MAC_VOICE.lock().ok().and_then(|slot| *slot);
    if let Some((at, seen, voice)) = cached {
        if !reading_is_stale(at.elapsed(), seen, mtime) {
            return voice;
        }
    }
    let voice = read_system_voice_now().await;
    if let Ok(mut slot) = MAC_VOICE.lock() {
        *slot = Some((Instant::now(), mtime, voice));
    }
    voice
}

/// Whether the Mac's own voice may speak right now. See
/// [`mac_voice_is_siri_or_unknown`].
pub async fn mac_voice_allowed() -> bool {
    mac_voice_is_siri_or_unknown(read_system_voice().await)
}

/// Where `any_tts` keeps the Kokoro voice embeddings.
///
/// It writes the Hugging Face cache layout and resolves the root from the same
/// environment variables, so this resolves it the same way. Mirroring a path
/// convention is not pretty; the alternative is loading the model to ask it,
/// which is an 82MB parse to draw a list.
fn kokoro_voices_dir() -> Option<PathBuf> {
    let root = if let Some(cache) = std::env::var_os("HUGGINGFACE_HUB_CACHE") {
        PathBuf::from(cache)
    } else if let Some(home) = std::env::var_os("HF_HOME") {
        PathBuf::from(home).join("hub")
    } else if let Some(cache) = std::env::var_os("XDG_CACHE_HOME") {
        PathBuf::from(cache).join("huggingface").join("hub")
    } else {
        PathBuf::from(std::env::var_os("HOME")?)
            .join(".cache")
            .join("huggingface")
            .join("hub")
    };

    Some(
        root.join("models--hexgrad--Kokoro-82M")
            .join("snapshots")
            .join("main")
            .join("voices"),
    )
}

/// Whether Kokoro's weights are on disk, so loading it is a parse and not a
/// download. Same cache layout as [`kokoro_voices_dir`].
pub fn kokoro_model_on_disk() -> bool {
    kokoro_voices_dir()
        .and_then(|voices| voices.parent().map(|dir| dir.join("kokoro-v1_0.pth")))
        .is_some_and(|weights| weights.is_file())
}

/// The Kokoro voices on disk, by id.
///
/// Empty is a real answer: the model and its voices download on the first
/// thing Kokoro ever says, so before that there is nothing to list and the
/// pane says so rather than offering names that would fail.
pub fn installed_kokoro_voices() -> Vec<String> {
    let Some(dir) = kokoro_voices_dir() else {
        return Vec::new();
    };
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };

    let mut ids = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("pt") {
            continue;
        }
        if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
            ids.push(stem.to_string());
        }
    }
    ids.sort();
    ids
}

/// How old a reading of this machine may be.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Freshness {
    /// Read it now.
    Now,
    /// A recent reading will do.
    Recent,
}

/// Everything this engine needs known about this machine, and nothing else:
/// the System Voice costs a process and Kokoro's voices a directory read.
pub async fn inventory_for(engine: &str, freshness: Freshness) -> VoiceInventory {
    let is_mac = engine.eq_ignore_ascii_case("system");
    let is_kokoro = engine.eq_ignore_ascii_case("kokoro");
    // Reading fresh is how the pane learns the System Voice changed; the
    // voice that speaks learns it from the same moment.
    if freshness == Freshness::Now && is_mac {
        forget_mac_voice();
    }
    VoiceInventory {
        kokoro: if is_kokoro {
            installed_kokoro_voices()
        } else {
            Vec::new()
        },
        kokoro_problem: if is_kokoro {
            crate::tts::kokoro::load_problem()
        } else {
            None
        },
        system_voice: if is_mac {
            read_system_voice().await
        } else {
            SystemVoice::Unset
        },
    }
}

/// Hand a voice to the running app.
///
/// The one place an engine's voice reaches `AppState`, so no call site can put
/// a voice under an engine that does not own it.
pub fn push_voice_to_state(
    state: &AppState,
    engine: &str,
    voice: Option<&str>,
) -> Result<(), String> {
    match engine.to_ascii_lowercase().as_str() {
        // The Mac's own voice is `say` with no `-v`, so no name is ever kept.
        "system" => state.set_system_voice(None),
        "kokoro" => state.set_kokoro_voice(voice.unwrap_or(KOKORO_DEFAULT_VOICE).to_string()),
        "supertonic" => {
            state.set_supertonic_voice(voice.unwrap_or(SUPERTONIC_DEFAULT_VOICE).to_string())
        }
        _ => Ok(()),
    }
}

// ---------------------------------------------------------------------------
// The audition
// ---------------------------------------------------------------------------

/// What the pane is told while a sample is playing.
#[derive(Debug, Clone, Serialize)]
struct AuditionReport<'a> {
    /// The row this is about.
    voice: &'a str,
    engine: &'a str,
    /// `preparing`, `speaking`, `done` or `failed`.
    state: &'a str,
    /// Only on `failed`, and always a sentence.
    message: Option<String>,
}

fn report_audition(
    app: &AppHandle,
    voice: &str,
    engine: &str,
    state: &str,
    message: Option<String>,
) {
    let report = AuditionReport {
        voice,
        engine,
        state,
        message,
    };
    if let Err(e) = app.emit(AUDITION_EVENT, report) {
        warn!("[Voices] Could not report the audition: {e}");
    }
}

/// Play the sample, and say what is happening while it does.
///
/// Spawned rather than awaited. `set_juno_voice` used to speak before it
/// returned, so the pane had nothing from Rust until the sample finished and
/// drew an optimistic selection in the meantime, which is what snapped back
/// when the real answer arrived. Now the choice is answered immediately and
/// the sound reports itself.
///
/// `preparing` is emitted before anything slow starts. `say` takes a second or
/// two to start and Kokoro loads an 82MB model on its first sample, and
/// several silent seconds with nothing on screen is the thing that reads as
/// broken.
///
/// `voice` names the row for the pane only. What actually speaks is the voice
/// stored for the engine, which both callers have just resolved.
fn audition(app_handle: &AppHandle, state: &AppState, engine: &str, voice: &str) {
    let generation = AUDITION.fetch_add(1, Ordering::SeqCst) + 1;
    crate::tts::stop_speech();
    crate::tts::reset_tts_stop_flag();

    let app = app_handle.clone();
    let app_state = state.clone();
    let engine = engine.to_string();
    let voice = voice.to_string();

    report_audition(&app, &voice, &engine, EVENT_PREPARING, None);

    tauri::async_runtime::spawn(async move {
        // `say` streams as it synthesises, so the Mac's voice is already
        // speaking. Every other engine renders a whole clip first.
        if engine.eq_ignore_ascii_case("system") {
            // A Mac voice that is not Siri is never spoken, not even as a
            // sample. Nothing is wrong to report: the row already says how
            // to fix it.
            if !mac_voice_allowed().await {
                info!("[Voices] The System Voice is not a Siri voice; no sample");
                report_audition(&app, &voice, &engine, EVENT_DONE, None);
                return;
            }
            report_audition(&app, &voice, &engine, EVENT_SPEAKING, None);
        }

        let sample = VOICE_SAMPLE_TEXT.to_string();
        let rendered = crate::tts::render_sample(sample, &engine, app_state).await;

        let outcome = match rendered {
            // The engine spoke it as it rendered it.
            Ok(None) => Ok(()),
            Ok(Some(audio)) => {
                if AUDITION.load(Ordering::SeqCst) != generation {
                    return;
                }
                report_audition(&app, &voice, &engine, EVENT_SPEAKING, None);
                crate::tts::play_sample(&audio).await
            }
            Err(e) => Err(e),
        };

        if AUDITION.load(Ordering::SeqCst) != generation {
            // A later tap took over. Being cut off is what was asked for, so
            // it is not reported.
            return;
        }

        match outcome {
            Ok(()) => report_audition(&app, &voice, &engine, EVENT_DONE, None),
            Err(e) => {
                warn!("[Voices] The sample did not play: {e}");
                let message = audition_failure(&engine, &e);
                report_audition(&app, &voice, &engine, EVENT_FAILED, Some(message));
            }
        }
    });
}

/// What to tell the person when the sample did not play.
fn audition_failure(engine: &str, error: &str) -> String {
    format!(
        "{} could not speak the sample: {}",
        engine_label(engine),
        error.trim()
    )
}

// ---------------------------------------------------------------------------
// Changing the engine and the voice
// ---------------------------------------------------------------------------

/// Serialises every change to the engine and its voice.
///
/// Each command reads the audio settings, awaits a listing of this machine,
/// then writes the whole struct back. Two of those interleaved is a lost
/// update: the pane opening (which resolves and can write) racing a tap on a
/// voice, or two quick taps, could each read the old choice and the slower
/// one would write it back over the newer. That is a selection snapping back
/// with nothing in the UI to blame. A tokio mutex, because it is held across
/// those awaits; nothing that holds it waits on anything that takes it.
static VOICE_CHANGE: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// Hold [`VOICE_CHANGE`] for another command that rewrites the audio
/// settings an engine reads, so it cannot interleave with a voice change.
pub async fn lock_voice_change() -> tokio::sync::MutexGuard<'static, ()> {
    VOICE_CHANGE.lock().await
}

fn settings_manager(
    app_handle: &AppHandle,
) -> Result<crate::settings::manager::SettingsManager, String> {
    crate::settings::manager::SettingsManager::new(app_handle.clone())
        .map_err(|e| format!("Failed to create settings manager: {e}"))
}

/// Keep the local model matched to the engine in force: warm Kokoro when it
/// is speaking, free it when it is not. Called only after a change has been
/// written, so it acts on the engine that is actually in force. When a load
/// finishes the pane is told, because Kokoro's voices are on disk only once
/// it has.
pub fn sync_engine_model(app_handle: &AppHandle, provider: &str) {
    crate::tts::kokoro::remember_app(app_handle);
    // The greeting rendered ahead follows the engine and voice in force.
    crate::greeting::refresh_cache(app_handle);
    let app = app_handle.clone();
    crate::tts::kokoro::sync_with_engine(provider, move || {
        if let Err(e) = app.emit(ENGINE_READY_EVENT, "kokoro") {
            warn!("[Voices] Could not report that Kokoro is ready: {e}");
        }
    });
}

/// Start the engine in force getting ready, first thing at launch.
///
/// On a new Mac Kokoro has its model to download before it can say a word.
/// Starting before anything else, rather than after the voice lists have been
/// read, gives the download the whole of onboarding to land in.
pub async fn warm_engine_at_launch(app_handle: &AppHandle) {
    let Ok(manager) = settings_manager(app_handle) else {
        return;
    };
    match manager.get_audio_settings().await {
        Ok(audio) => sync_engine_model(app_handle, &audio.tts_provider),
        Err(e) => warn!("[Voices] Could not read the engine to warm it: {e}"),
    }
}

/// Resolve, and make the resolution true.
///
/// A list that lights one voice while the engine speaks in another is the
/// defect this file exists to prevent, so resolving is a write: when the
/// stored choice cannot be honoured, the voice that will actually speak is
/// what gets stored and what the running app is told. Without this the pane
/// would light the voice it resolved while `say` went on using the stale one
/// in `AppState`. Callers hold [`VOICE_CHANGE`].
async fn resolve_and_list(
    manager: &crate::settings::manager::SettingsManager,
    state: &AppState,
    audio: &mut crate::settings::AudioSettings,
    freshness: Freshness,
) -> Result<JunoVoiceList, String> {
    let engine = stored_engine(audio);
    let inventory = inventory_for(&engine, freshness).await;
    let stored = audio.voice_for(&engine).map(str::to_string);
    let resolution = resolve_voice(&engine, &inventory, stored.as_deref());

    if resolution.voice != stored {
        if resolution.substituted {
            warn!(
                "[Voices] {} cannot speak as {:?}; using {:?} instead",
                engine, stored, resolution.voice
            );
        }
        audio.set_voice_for(&engine, resolution.voice.clone());
        manager
            .set_audio_settings(audio)
            .await
            .map_err(|e| format!("Failed to save Juno's voice: {e}"))?;
    }
    push_voice_to_state(state, &engine, resolution.voice.as_deref())?;
    state.set_voice_rate(audio.voice_rate)?;

    Ok(
        voice_list(&audio.tts_provider, &engine, &inventory, &resolution)
            .with_rate(audio.voice_rate),
    )
}

/// Put the stored engine and voice in force: at startup, and after a reset.
///
/// A stored choice is only the default if it is actually in force. A reset
/// writes the defaults to disk, and without this the running app would keep
/// speaking in the old voice until the next launch, so it runs this too. It
/// is also where a Mac voice an older build named is retired for good.
pub async fn apply_stored_voice(app_handle: &AppHandle, state: &AppState) -> Result<(), String> {
    let _change = VOICE_CHANGE.lock().await;
    let manager = settings_manager(app_handle)?;
    let mut audio = manager
        .get_audio_settings()
        .await
        .map_err(|e| format!("Failed to get audio settings: {e}"))?;
    if !audio.tts_provider.trim().is_empty() {
        state.set_tts_provider(audio.tts_provider.clone())?;
    }
    let list = resolve_and_list(&manager, state, &mut audio, Freshness::Now).await?;
    info!(
        "[Voices] {} speaks as {}",
        list.engine,
        list.options
            .iter()
            .find(|o| o.selected)
            .map(|o| o.id.as_str())
            .unwrap_or("its own default")
    );
    sync_engine_model(app_handle, &audio.tts_provider);
    Ok(())
}

/// Change the engine, and answer with that engine's voices.
///
/// The voice is resolved against the new engine before anything is written,
/// so the new engine is never left holding an id another engine named, and
/// the answer is the list the pane draws: the engine picker and the rows
/// cannot disagree because they come back in one reply.
pub async fn switch_engine(
    provider: &str,
    app_handle: &AppHandle,
    state: &AppState,
) -> Result<JunoVoiceList, String> {
    let provider = provider.trim().to_ascii_lowercase();
    if provider != OFF_PROVIDER && !ENGINES.contains(&provider.as_str()) {
        return Err(format!("Juno cannot speak with {provider}."));
    }

    let _change = VOICE_CHANGE.lock().await;
    let manager = settings_manager(app_handle)?;
    let mut audio = manager
        .get_audio_settings()
        .await
        .map_err(|e| format!("Failed to get audio settings: {e}"))?;

    // Silence is its own switch. Changing the engine while it is on changes
    // what speaks when it is turned off, and keeps Juno quiet until then.
    if is_silent(&provider) {
        let (stored, before) = silence_change(
            true,
            &audio.tts_provider,
            audio.engine_before_silent.as_deref(),
        );
        audio.tts_provider = stored;
        audio.engine_before_silent = before;
    } else if is_silent(&audio.tts_provider) {
        audio.engine_before_silent = Some(provider.clone());
    } else {
        audio.tts_provider = provider.clone();
    }
    let provider = audio.tts_provider.clone();
    let engine = stored_engine(&audio);
    let inventory = inventory_for(&engine, Freshness::Recent).await;
    let stored = audio.voice_for(&engine).map(str::to_string);
    let resolution = resolve_voice(&engine, &inventory, stored.as_deref());
    audio.set_voice_for(&engine, resolution.voice.clone());

    manager
        .set_audio_settings(&audio)
        .await
        .map_err(|e| format!("Failed to save audio settings: {e}"))?;
    state.set_tts_provider(provider.clone())?;
    push_voice_to_state(state, &engine, resolution.voice.as_deref())?;
    sync_engine_model(app_handle, &provider);

    info!(
        "[Voices] Engine is {} speaking as {}",
        engine,
        resolution.voice.as_deref().unwrap_or("its own default")
    );
    Ok(voice_list(&provider, &engine, &inventory, &resolution).with_rate(audio.voice_rate))
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

/// The rows the Audio pane draws.
#[tauri::command]
pub async fn get_juno_voices(
    app_handle: AppHandle,
    state: State<'_, AppState>,
) -> Result<JunoVoiceList, String> {
    let _change = VOICE_CHANGE.lock().await;
    let manager = settings_manager(&app_handle)?;
    let mut audio = manager
        .get_audio_settings()
        .await
        .map_err(|e| format!("Failed to get audio settings: {e}"))?;
    resolve_and_list(&manager, state.inner(), &mut audio, Freshness::Now).await
}

/// Pick Juno's voice, and hear it.
///
/// Returns the list Rust decided on, so the pane never has to guess: there is
/// no optimistic selection and therefore nothing to revert.
///
/// Picking a voice does not change the engine. It only turns sound back on.
#[tauri::command]
pub async fn set_juno_voice(
    id: String,
    app_handle: AppHandle,
    state: State<'_, AppState>,
) -> Result<JunoVoiceList, String> {
    let _change = VOICE_CHANGE.lock().await;
    let manager = settings_manager(&app_handle)?;
    let mut audio = manager
        .get_audio_settings()
        .await
        .map_err(|e| format!("Failed to get audio settings: {e}"))?;

    let engine = stored_engine(&audio);
    // The pane was drawn from a fresh reading moments ago, so a recent one is
    // enough to check the id against.
    let mut inventory = inventory_for(&engine, Freshness::Recent).await;

    let offered = |inventory: &VoiceInventory| match provider_voices(&engine, inventory) {
        ProviderVoices::Known(rows) => rows.iter().any(|row| row.id == id),
        ProviderVoices::Elsewhere(_) => false,
    };
    if !offered(&inventory) {
        // Something new since the last reading.
        inventory = inventory_for(&engine, Freshness::Now).await;
    }
    if !offered(&inventory) {
        return Err(format!(
            "{} has no voice called {}.",
            engine_label(&engine),
            id
        ));
    }
    let chosen = Some(id.clone());

    // Picking a voice while Juno is silent picks what speaks once speech is
    // back on. It does not turn speech on: that is its own switch.
    let silent = is_silent(&audio.tts_provider);
    let provider_changed = !silent && !audio.tts_provider.eq_ignore_ascii_case(&engine);
    if silent {
        audio.engine_before_silent = Some(engine.clone());
    } else {
        audio.tts_provider = engine.clone();
    }
    audio.set_voice_for(&engine, chosen.clone());
    manager
        .set_audio_settings(&audio)
        .await
        .map_err(|e| format!("Failed to save Juno's voice: {e}"))?;

    state.set_tts_provider(audio.tts_provider.clone())?;
    push_voice_to_state(state.inner(), &engine, chosen.as_deref())?;
    if provider_changed {
        sync_engine_model(&app_handle, &engine);
    }

    info!(
        "[Voices] {} speaks as {}",
        engine,
        chosen.as_deref().unwrap_or("the Mac's own voice")
    );

    let resolution = VoiceResolution {
        voice: chosen,
        substituted: false,
    };
    let list = voice_list(&audio.tts_provider, &engine, &inventory, &resolution)
        .with_rate(audio.voice_rate);
    crate::greeting::refresh_cache(&app_handle);
    audition(&app_handle, state.inner(), &engine, &id);
    Ok(list)
}

/// Turn Juno's speech off or back on.
///
/// Independent of the voice: off remembers the engine that was speaking, and
/// on returns to it (or to the default, when nothing was). Answers with the
/// list so the pane draws what Rust decided.
#[tauri::command]
pub async fn set_juno_silent(
    silent: bool,
    app_handle: AppHandle,
    state: State<'_, AppState>,
) -> Result<JunoVoiceList, String> {
    let _change = VOICE_CHANGE.lock().await;
    let manager = settings_manager(&app_handle)?;
    let mut audio = manager
        .get_audio_settings()
        .await
        .map_err(|e| format!("Failed to get audio settings: {e}"))?;

    let (provider, before) = silence_change(
        silent,
        &audio.tts_provider,
        audio.engine_before_silent.as_deref(),
    );
    audio.tts_provider = provider.clone();
    audio.engine_before_silent = before;
    manager
        .set_audio_settings(&audio)
        .await
        .map_err(|e| format!("Failed to save Juno's voice: {e}"))?;
    state.set_tts_provider(provider.clone())?;
    if silent {
        crate::tts::stop_speech();
    }
    sync_engine_model(&app_handle, &provider);
    info!(
        "[Voices] Juno {}",
        if silent { "is silent" } else { "speaks again" }
    );

    let list = resolve_and_list(&manager, state.inner(), &mut audio, Freshness::Recent).await?;
    crate::greeting::refresh_cache(&app_handle);
    Ok(list)
}

/// Set how fast Juno speaks, and hear it.
///
/// Stored once for every engine; an engine with no speed control ignores it
/// and the pane does not offer it. Answers with the list so the slider
/// settles on what Rust decided, and plays the sample at the new pace.
#[tauri::command]
pub async fn set_juno_voice_rate(
    rate: f64,
    app_handle: AppHandle,
    state: State<'_, AppState>,
) -> Result<JunoVoiceList, String> {
    if !rate.is_finite() {
        return Err("The speed has to be a number.".to_string());
    }
    let rate = crate::tts::rate::sanitize(rate);

    let _change = VOICE_CHANGE.lock().await;
    let manager = settings_manager(&app_handle)?;
    let mut audio = manager
        .get_audio_settings()
        .await
        .map_err(|e| format!("Failed to get audio settings: {e}"))?;

    audio.voice_rate = rate;
    manager
        .set_audio_settings(&audio)
        .await
        .map_err(|e| format!("Failed to save Juno's speed: {e}"))?;
    state.set_voice_rate(rate)?;
    info!("[Voices] Juno speaks at {rate:.2}x");

    let engine = stored_engine(&audio);
    let inventory = inventory_for(&engine, Freshness::Recent).await;
    let stored = audio.voice_for(&engine).map(str::to_string);
    let resolution = resolve_voice(&engine, &inventory, stored.as_deref());
    let list = voice_list(&audio.tts_provider, &engine, &inventory, &resolution).with_rate(rate);

    // Hearing it is the point of moving the slider. Only when the engine in
    // force can change speed: otherwise the sample would sound the same.
    if list.speed.is_some() {
        let row = resolution
            .voice
            .unwrap_or_else(|| SYSTEM_DEFAULT_ID.to_string());
        crate::greeting::refresh_cache(&app_handle);
        audition(&app_handle, state.inner(), &engine, &row);
    }
    Ok(list)
}

/// Say the sample again in the voice already chosen.
#[tauri::command]
pub async fn preview_juno_voice(
    app_handle: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let (engine, row) = {
        let _change = VOICE_CHANGE.lock().await;
        let manager = settings_manager(&app_handle)?;
        let audio = manager
            .get_audio_settings()
            .await
            .map_err(|e| format!("Failed to get audio settings: {e}"))?;

        if is_silent(&audio.tts_provider) {
            // Silence is a choice. Replaying it would contradict it.
            return Ok(());
        }

        let engine = stored_engine(&audio);
        let inventory = inventory_for(&engine, Freshness::Recent).await;
        let stored = audio.voice_for(&engine).map(str::to_string);
        let resolution = resolve_voice(&engine, &inventory, stored.as_deref());
        let row = resolution
            .voice
            .unwrap_or_else(|| SYSTEM_DEFAULT_ID.to_string());
        (engine, row)
    };

    audition(&app_handle, state.inner(), &engine, &row);
    Ok(())
}

/// Open System Settings > Accessibility > Read & Speak, where the Mac's voice
/// is chosen. The link under the "Your Mac's voice" row.
///
/// Tries the Read & Speak anchor, then Accessibility itself.
#[tauri::command]
pub async fn open_read_and_speak() -> Result<(), String> {
    for url in READ_AND_SPEAK_URLS {
        match std::process::Command::new("open").arg(url).status() {
            Ok(status) if status.success() => return Ok(()),
            Ok(status) => warn!("[Voices] `open {}` exited with {}", url, status),
            Err(e) => error!("[Voices] `open {}` failed: {}", url, e),
        }
    }
    Err("Could not open System Settings".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kokoro_mac() -> VoiceInventory {
        VoiceInventory {
            kokoro: vec![
                "af_heart".to_string(),
                "am_michael".to_string(),
                "bf_emma".to_string(),
            ],
            ..VoiceInventory::default()
        }
    }

    fn mac(voice: SystemVoice) -> VoiceInventory {
        VoiceInventory {
            system_voice: voice,
            ..VoiceInventory::default()
        }
    }

    fn list_for(provider: &str, inventory: &VoiceInventory, stored: Option<&str>) -> JunoVoiceList {
        let engine = engine_in_force(provider, None);
        let resolution = resolve_voice(&engine, inventory, stored);
        voice_list(provider, &engine, inventory, &resolution)
    }

    // -- the gate ----------------------------------------------------------

    /// Juno speaks in a Siri voice or a provider's voice, never another Mac
    /// voice. A failed reading is trusted: `say` speaks the System Voice.
    #[test]
    fn only_a_siri_or_unread_system_voice_may_speak() {
        assert!(mac_voice_is_siri_or_unknown(SystemVoice::Siri));
        assert!(mac_voice_is_siri_or_unknown(SystemVoice::Unset));
        assert!(!mac_voice_is_siri_or_unknown(SystemVoice::Other));
    }

    /// Knowing nothing is not knowing it is someone else.
    #[test]
    fn the_default_reading_is_unset() {
        assert_eq!(SystemVoice::default(), SystemVoice::Unset);
        assert_eq!(VoiceInventory::default().system_voice, SystemVoice::Unset);
    }

    // -- the Mac's one row -------------------------------------------------

    /// The Mac offers exactly one row, its own voice, whatever was stored.
    #[test]
    fn the_mac_offers_exactly_one_row() {
        for voice in [SystemVoice::Siri, SystemVoice::Unset, SystemVoice::Other] {
            for stored in [
                None,
                Some("Samantha"),
                Some("Ava (Premium)"),
                Some(SYSTEM_DEFAULT_ID),
            ] {
                let list = list_for("system", &mac(voice), stored);
                assert_eq!(list.options.len(), 1, "{voice:?} {stored:?}");
                let row = &list.options[0];
                assert_eq!(row.id, SYSTEM_DEFAULT_ID);
                assert_eq!(row.kind, "system");
                assert_eq!(row.name, "Your Mac's voice");
                assert!(row.selected);
                assert_eq!(row.link.as_deref(), Some(READ_AND_SPEAK_LINK));
                assert!(list.note.is_none(), "{:?}", list.note);
            }
        }
    }

    /// The defect: Simone set in Read & Speak, Juno speaking as Samantha.
    /// A named Mac voice from an older build is not a voice Juno speaks in
    /// any more; it resolves quietly to the Mac's own.
    #[test]
    fn a_stored_samantha_resolves_to_the_macs_own_voice() {
        for voice in [SystemVoice::Siri, SystemVoice::Unset, SystemVoice::Other] {
            let resolution = resolve_voice("system", &mac(voice), Some("Samantha"));
            assert_eq!(resolution.voice.as_deref(), Some(SYSTEM_DEFAULT_ID));
            assert!(!resolution.substituted);
        }
        assert_eq!(
            default_voice("system", &mac(SystemVoice::Unset)).as_deref(),
            Some(SYSTEM_DEFAULT_ID)
        );
    }

    /// The row says how it works, and how to fix it when the Mac's voice is
    /// not a Siri voice. A row that will not speak does not offer to.
    #[test]
    fn the_mac_row_says_where_its_voice_is_set() {
        let siri = list_for("system", &mac(SystemVoice::Siri), None);
        assert_eq!(
            siri.options[0].descriptor,
            "Uses the Siri voice you pick in Read & Speak."
        );
        assert!(siri.options[0].speaks);

        let other = list_for("system", &mac(SystemVoice::Other), None);
        assert_eq!(
            other.options[0].descriptor,
            "Pick a Siri voice in Read & Speak and Juno will use it."
        );
        assert!(!other.options[0].speaks);
        for row in [&siri.options[0], &other.options[0]] {
            assert!(!row.descriptor.contains('—'));
        }
    }

    /// Only the Mac row links anywhere.
    #[test]
    fn only_the_mac_row_has_a_link() {
        let kokoro = list_for("kokoro", &kokoro_mac(), None);
        assert!(kokoro.options.iter().all(|o| o.link.is_none()));
    }

    /// The most specific pane first, then Accessibility itself.
    #[test]
    fn read_and_speak_opens_the_accessibility_pane() {
        assert!(READ_AND_SPEAK_URLS[0].ends_with("?SpokenContent"));
        for url in READ_AND_SPEAK_URLS {
            assert!(url.starts_with(
                "x-apple.systempreferences:com.apple.Accessibility-Settings.extension"
            ));
        }
    }

    /// The Mac's own voice reaches `say` as no `-v`, for the greeting and
    /// every reply alike, whatever an older build stored.
    #[test]
    fn the_greeting_and_a_reply_speak_the_same_voice() {
        let state = AppState::new(None);
        let greeting = state.get_system_voice().unwrap();
        let resolution = resolve_voice("system", &mac(SystemVoice::Unset), Some("Samantha"));
        push_voice_to_state(&state, "system", resolution.voice.as_deref()).unwrap();
        push_voice_to_state(&state, "system", Some("Samantha")).unwrap();
        let reply = state.get_system_voice().unwrap();
        assert_eq!(greeting, None);
        assert_eq!(reply, None);
        assert_eq!(crate::tts::system::say_arguments("hi", None), vec!["hi"]);
    }

    // -- reading the System Voice ------------------------------------------

    const NEW_SELECTIONS: &str = r#"["en",{"boundLanguage":"en","_type":"Speech.VoiceSelection","voiceId":"com.apple.siri.natural.Simone","_version":0}]"#;

    fn langs(list: &[&str]) -> Vec<String> {
        list.iter().map(|l| l.to_string()).collect()
    }

    #[test]
    fn the_system_voice_choice_is_read_from_its_id_or_name() {
        assert_eq!(classify_system_voice(None, None), SystemVoice::Unset);
        assert_eq!(
            classify_system_voice(Some(" "), Some("")),
            SystemVoice::Unset
        );
        assert_eq!(
            classify_system_voice(Some("com.apple.siri.natural.Simone"), None),
            SystemVoice::Siri
        );
        assert_eq!(
            classify_system_voice(
                Some("com.apple.speech.synthesis.voice.custom.siri.nicky"),
                None
            ),
            SystemVoice::Siri
        );
        assert_eq!(
            classify_system_voice(Some("com.apple.voice.compact.en-US.Samantha"), None),
            SystemVoice::Other
        );
        assert_eq!(
            classify_system_voice(
                Some("com.apple.voice.premium.en-US.Ava"),
                Some("Ava (Premium)")
            ),
            SystemVoice::Other
        );
    }

    #[test]
    fn the_macos_26_voice_selection_is_read_by_language() {
        assert_eq!(
            parse_voice_selections(NEW_SELECTIONS, &langs(&["en"])),
            Some((
                "en".to_string(),
                "com.apple.siri.natural.Simone".to_string()
            ))
        );
        let two = r#"["en",{"voiceId":"a.en"},"fr",{"voiceId":"a.fr"}]"#;
        let fr = parse_voice_selections(two, &langs(&["fr", "en"]));
        assert_eq!(fr.map(|(_, id)| id).as_deref(), Some("a.fr"));
        let en = parse_voice_selections(two, &langs(&["de", "en"]));
        assert_eq!(en.map(|(_, id)| id).as_deref(), Some("a.en"));
        assert_eq!(parse_voice_selections(two, &langs(&["de"])), None);
        assert_eq!(parse_voice_selections("not json", &langs(&["en"])), None);
        assert_eq!(parse_voice_selections("[]", &langs(&["en"])), None);
    }

    #[test]
    fn the_new_key_wins_and_the_old_domain_is_the_fallback() {
        let selection = parse_voice_selections(NEW_SELECTIONS, &langs(&["en"]));
        let new = spoken_content_from(
            selection,
            Some("old.id".to_string()),
            Some("Old".to_string()),
        );
        assert_eq!(
            new.voice_id.as_deref(),
            Some("com.apple.siri.natural.Simone")
        );
        assert_eq!(new.voice_name, None);
        assert_eq!(
            classify_system_voice(new.voice_id.as_deref(), new.voice_name.as_deref()),
            SystemVoice::Siri
        );

        let old = spoken_content_from(
            None,
            Some("com.apple.voice.premium.en-US.Ava".to_string()),
            Some("Ava".to_string()),
        );
        assert_eq!(
            old.voice_id.as_deref(),
            Some("com.apple.voice.premium.en-US.Ava")
        );
        assert_eq!(old.voice_name.as_deref(), Some("Ava"));

        let nothing = spoken_content_from(None, None, None);
        assert_eq!(
            classify_system_voice(nothing.voice_id.as_deref(), nothing.voice_name.as_deref()),
            SystemVoice::Unset
        );
    }

    #[test]
    fn a_changed_system_voice_is_read_again() {
        let t0 = SystemTime::UNIX_EPOCH + Duration::from_secs(1000);
        let t1 = t0 + Duration::from_secs(2);
        let young = Duration::from_secs(1);
        // Nothing moved and the reading is young: reuse it.
        assert!(!reading_is_stale(young, Some(t0), Some(t0)));
        assert!(!reading_is_stale(young, None, None));
        // The preferences file was written since: read again, however young.
        assert!(reading_is_stale(young, Some(t0), Some(t1)));
        assert!(reading_is_stale(young, None, Some(t1)));
        assert!(reading_is_stale(young, Some(t0), None));
        // An old reading is read again even if the file looks unchanged.
        assert!(reading_is_stale(MAC_VOICE_FRESH_FOR, Some(t0), Some(t0)));
    }

    // -- the list is a function of the engine ------------------------------

    #[test]
    fn the_list_belongs_to_the_engine_that_is_speaking() {
        let kokoro = list_for("kokoro", &kokoro_mac(), None);
        assert_eq!(kokoro.engine, "kokoro");
        assert_eq!(kokoro.engine_label, "Kokoro");
        let ids: Vec<&str> = kokoro.options.iter().map(|o| o.id.as_str()).collect();
        assert!(ids.contains(&"af_heart"), "{ids:?}");
        assert!(!ids.contains(&SYSTEM_DEFAULT_ID), "{ids:?}");

        let mac = list_for("system", &kokoro_mac(), None);
        let ids: Vec<&str> = mac.options.iter().map(|o| o.id.as_str()).collect();
        assert_eq!(ids, vec![SYSTEM_DEFAULT_ID]);
    }

    /// An engine whose voices Juno cannot enumerate says so. Rows it could not
    /// honour would be worse than no rows.
    #[test]
    fn an_engine_juno_cannot_enumerate_reports_instead_of_guessing() {
        for engine in ["elevenlabs", "replicate", "chatterbox"] {
            let list = list_for(engine, &VoiceInventory::default(), None);
            assert_eq!(list.engine, engine);
            assert!(
                list.options.is_empty(),
                "{engine} should offer nothing it cannot honour"
            );
            let note = list.note.unwrap_or_default();
            assert!(!note.is_empty(), "{engine} must say why there are no rows");
            assert!(!note.contains('—'), "{note}");
        }
    }

    /// Kokoro before it has ever spoken: the model and its voices are not
    /// downloaded, so there is nothing to offer and the pane says so.
    #[test]
    fn an_engine_with_nothing_installed_says_so() {
        let list = list_for("kokoro", &VoiceInventory::default(), None);
        assert!(list.options.is_empty());
        assert!(list.note.unwrap_or_default().contains("Kokoro"));
    }

    // -- switching engines -------------------------------------------------

    /// Switching engine must leave a voice the new engine actually has, not
    /// the one the old engine named.
    #[test]
    fn switching_engine_yields_a_voice_that_engine_has() {
        let inventory = kokoro_mac();
        let kokoro = resolve_voice("kokoro", &inventory, Some(SYSTEM_DEFAULT_ID));
        assert_eq!(kokoro.voice.as_deref(), Some(KOKORO_DEFAULT_VOICE));
        assert!(kokoro.substituted, "the pane has to be able to say why");

        let back = resolve_voice("system", &inventory, Some("af_heart"));
        assert_eq!(back.voice.as_deref(), Some(SYSTEM_DEFAULT_ID));
    }

    /// A stored id the engine does not know resolves to that engine's default,
    /// never to nothing at all.
    #[test]
    fn an_unknown_id_resolves_to_the_default_rather_than_to_silence() {
        for engine in ["system", "kokoro", "supertonic"] {
            let resolution = resolve_voice(engine, &kokoro_mac(), Some("Nobody At All"));
            assert!(
                resolution.voice.is_some(),
                "{engine} resolved to silence: {resolution:?}"
            );
            assert_eq!(resolution.voice, default_voice(engine, &kokoro_mac()));
        }
        assert!(resolve_voice("kokoro", &kokoro_mac(), Some("Nobody")).substituted);
    }

    /// Resolving twice must not keep moving: the resolution is what gets
    /// stored, and a stored resolution has to survive the next read.
    #[test]
    fn resolving_is_idempotent() {
        let inventory = kokoro_mac();
        for engine in ["system", "kokoro", "supertonic"] {
            let once = resolve_voice(engine, &inventory, Some("Nobody At All"));
            let twice = resolve_voice(engine, &inventory, once.voice.as_deref());
            assert_eq!(once.voice, twice.voice, "{engine}");
            assert!(!twice.substituted, "{engine} kept reporting a substitution");
        }
    }

    /// Kokoro's default is the one embedding the downloader always fetches.
    #[test]
    fn kokoro_defaults_to_the_voice_the_model_ships_with() {
        assert_eq!(
            default_voice("kokoro", &kokoro_mac()),
            Some(KOKORO_DEFAULT_VOICE.to_string())
        );
        assert_eq!(
            kokoro_voices(&kokoro_mac().kokoro)[0].id,
            KOKORO_DEFAULT_VOICE,
            "the default is the row at the top"
        );
        // af_bella was the stored default for months and its embedding is not
        // one of the files the downloader fetches.
        let resolution = resolve_voice("kokoro", &kokoro_mac(), Some("af_bella"));
        assert_eq!(resolution.voice.as_deref(), Some(KOKORO_DEFAULT_VOICE));
    }

    #[test]
    fn a_kokoro_id_reads_as_a_name_and_an_accent() {
        let rows = kokoro_voices(&kokoro_mac().kokoro);
        let michael = rows.iter().find(|row| row.id == "am_michael").unwrap();
        assert_eq!(michael.name, "Michael");
        assert_eq!(michael.descriptor, "American English, male.");
        let emma = rows.iter().find(|row| row.id == "bf_emma").unwrap();
        assert_eq!(emma.descriptor, "British English, female.");
    }

    // -- silence -----------------------------------------------------------

    /// Silence is a switch, not a row. While it is on the list still shows
    /// the voice that speaks when it is off, with nothing to replay.
    #[test]
    fn silence_is_a_switch_and_the_rows_stay_the_engines() {
        let list = list_for(OFF_PROVIDER, &kokoro_mac(), None).with_rate(1.0);
        assert!(list.silent);
        assert_eq!(list.engine, "kokoro");
        assert!(list.options.iter().all(|o| o.kind != "silent"));
        assert!(list.speed.is_none(), "no speed row while silent");
        assert!(!list_for("kokoro", &kokoro_mac(), None).silent);
    }

    /// Off remembers the engine, and on returns to it.
    #[test]
    fn turning_speech_back_on_returns_to_the_engine_that_was_speaking() {
        let (off, remembered) = silence_change(true, "elevenlabs", None);
        assert_eq!(off, OFF_PROVIDER);
        assert_eq!(remembered.as_deref(), Some("elevenlabs"));
        assert_eq!(engine_in_force(&off, remembered.as_deref()), "elevenlabs");

        // Off twice keeps the first engine, not "off".
        let (_, again) = silence_change(true, &off, remembered.as_deref());
        assert_eq!(again.as_deref(), Some("elevenlabs"));

        let (on, cleared) = silence_change(false, &off, again.as_deref());
        assert_eq!(on, "elevenlabs");
        assert_eq!(cleared, None);
    }

    /// Silent from an older build, with nothing remembered, comes back on the
    /// default engine. Turning on what is already on changes nothing.
    #[test]
    fn speech_back_on_with_nothing_remembered_is_the_default() {
        let default = crate::constants::settings::defaults::TTS_PROVIDER;
        assert_eq!(silence_change(false, OFF_PROVIDER, None).0, default);
        assert_eq!(silence_change(false, "", None).0, default);
        assert_eq!(
            silence_change(false, OFF_PROVIDER, Some("nonsense")).0,
            default
        );
        assert_eq!(silence_change(false, "system", None).0, "system");
        assert_eq!(engine_in_force("kokoro", Some("system")), "kokoro");
    }

    #[test]
    fn exactly_one_row_is_in_force() {
        for provider in ["system", "kokoro", OFF_PROVIDER] {
            for stored in [None, Some("Daniel"), Some("af_heart"), Some("Nobody")] {
                let list = list_for(provider, &kokoro_mac(), stored);
                assert_eq!(
                    list.options.iter().filter(|o| o.selected).count(),
                    1,
                    "{provider} with {stored:?}: {:?}",
                    list.options
                );
            }
        }
    }

    // -- the words ---------------------------------------------------------

    #[test]
    fn the_sample_is_short_enough_to_interrupt() {
        assert!(VOICE_SAMPLE_TEXT.len() < 60, "{VOICE_SAMPLE_TEXT}");
        assert!(!VOICE_SAMPLE_TEXT.contains('—'));
    }

    #[test]
    fn a_failure_reads_as_a_sentence_naming_the_engine() {
        let message = audition_failure("kokoro", "Failed to load Kokoro-82M model: no network");
        assert!(message.starts_with("Kokoro could not speak the sample:"));
        assert!(!message.contains('—'));
    }

    // -- the default engine ------------------------------------------------

    /// The default is Kokoro: local, no key, no network once it is on disk.
    /// The one thing it needs is the first download. See `tts::fallback_chain`
    /// for what speaks until it lands.
    #[test]
    fn the_default_engine_is_kokoro_and_needs_only_a_download() {
        let default = crate::constants::settings::defaults::TTS_PROVIDER;
        assert_eq!(engine_in_force(default, None), "kokoro");
        assert!(ENGINES.contains(&default));
        let needs = engine_needs(default);
        assert!(needs.download);
        assert!(!needs.account_key && !needs.network && !needs.server);
    }

    /// The Mac's voice is the one engine that needs nothing at all. Every
    /// cloud engine needs an account.
    #[test]
    fn only_the_mac_voice_needs_nothing() {
        assert!(engine_needs("system").nothing());
        for engine in ENGINES.iter().filter(|e| **e != "system") {
            assert!(!engine_needs(engine).nothing(), "{engine}");
        }
        for cloud in ["elevenlabs", "replicate", "chatterbox"] {
            assert!(engine_needs(cloud).account_key, "{cloud}");
        }
    }

    /// The same default on an Intel Mac. Nothing about it is gated by
    /// architecture, and this pins that it stays so on the x86_64 build.
    #[cfg(target_arch = "x86_64")]
    #[test]
    fn an_intel_mac_gets_the_same_default_engine() {
        assert_eq!(crate::constants::settings::defaults::TTS_PROVIDER, "kokoro");
    }

    // -- each engine lists its own voices ----------------------------------

    /// Every engine's rows are that engine's voices and nobody else's, and the
    /// voice it resolves to is one of its own rows.
    #[test]
    fn every_engine_lists_only_its_own_voices() {
        let inventory = kokoro_mac();
        for engine in ENGINES {
            let list = list_for(engine, &inventory, None);
            assert_eq!(list.engine, *engine);
            for row in &list.options {
                let belongs = match *engine {
                    "system" => row.id == SYSTEM_DEFAULT_ID,
                    "kokoro" => inventory.kokoro.contains(&row.id),
                    "supertonic" => ["M1", "F1"].contains(&row.id.as_str()),
                    _ => false,
                };
                assert!(belongs, "{engine} offered {}", row.id);
            }
            let resolved = resolve_voice(engine, &inventory, None).voice;
            if let Some(voice) = resolved {
                assert!(
                    list.options.iter().any(|row| row.id == voice),
                    "{engine} resolved to {voice}, which it does not list"
                );
            }
        }
    }

    /// Kokoro loads as soon as it is chosen. Before its voices land the pane
    /// says it is getting ready; if the load failed, it says why instead of
    /// waiting forever.
    #[test]
    fn kokoro_says_why_it_has_no_voices_yet() {
        let loading = list_for("kokoro", &VoiceInventory::default(), None);
        assert!(loading
            .note
            .unwrap_or_default()
            .contains("getting its voices ready"));

        let failed = VoiceInventory {
            kokoro_problem: Some("no network.".to_string()),
            ..VoiceInventory::default()
        };
        let note = list_for("kokoro", &failed, None).note.unwrap_or_default();
        assert!(note.contains("could not get ready: no network."), "{note}");
        assert!(!note.contains(".."), "{note}");
    }

    /// The engine picker comes from here, and silence is not in it: silence
    /// is the first row of the voice list.
    #[test]
    fn the_engine_picker_is_every_engine_and_not_silence() {
        let list = list_for("kokoro", &kokoro_mac(), None);
        let ids: Vec<&str> = list.engines.iter().map(|e| e.id.as_str()).collect();
        assert_eq!(ids, ENGINES.to_vec());
        assert!(!ids.contains(&OFF_PROVIDER));
        for engine in &list.engines {
            assert_ne!(engine.name, "This engine", "{} has no name", engine.id);
        }
    }

    // -- speed -------------------------------------------------------------

    #[test]
    fn the_speed_row_shows_for_an_engine_that_can_change_speed() {
        let list = list_for("system", &VoiceInventory::default(), None).with_rate(1.5);
        let speed = list.speed.expect("the Mac's voice can change speed");
        assert_eq!(speed.value, 1.5);
        assert_eq!((speed.min, speed.max), (0.75, 2.0));
    }

    #[test]
    fn the_speed_row_is_absent_when_the_engine_has_no_speed_control() {
        for provider in ["replicate", "chatterbox"] {
            let list = list_for(provider, &VoiceInventory::default(), None).with_rate(1.5);
            assert!(list.speed.is_none(), "{provider} has no speed control");
        }
    }

    #[test]
    fn silence_has_no_speed_row() {
        let list = list_for("off", &VoiceInventory::default(), None).with_rate(1.5);
        assert!(list.speed.is_none());
    }

    #[test]
    fn a_new_list_has_no_speed_until_one_is_attached() {
        assert!(list_for("system", &VoiceInventory::default(), None)
            .speed
            .is_none());
    }
}
