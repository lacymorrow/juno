//! Juno's voice: the voices the engine that is speaking actually has.
//!
//! The list is a function of the active engine, computed here. That is the
//! whole correction to the first version of this file, which offered a curated
//! list of macOS voices no matter which engine was doing the talking. Choose
//! Kokoro and the pane went on showing Samantha and Daniel, which Kokoro has
//! never heard of, and picking one quietly switched the engine back to the
//! Mac. A control that names a behaviour it is not wired to is the defect this
//! codebase keeps producing; the fix is that nothing about voices is decided
//! in the UI. Rust owns which voices exist, which one is active, whether it is
//! valid for the current engine and what the default is. React draws that.
//!
//! A voice id belongs to the engine that named it. "Samantha" means nothing to
//! Kokoro and "af_heart" means nothing to `say`, so the choice is stored per
//! engine (`settings::AudioSettings::voice_for`) and changing engine resolves
//! to a voice the new engine has rather than carrying a dangling id across.
//!
//! Curated, not dumped, for the Mac. `say -v '?'` lists more than a hundred
//! voices, most of them other languages and novelty voices ("Bad News", "Pipe
//! Organ"), and a hundred rows is not a choice. The catalog below is the short
//! list, and each entry offers the best version of itself this Mac has: the
//! Premium and Enhanced downloads replace the compact one in place, with no
//! code change, because the quality is in the name `say` reports.
//!
//! On the good voices: the voices that sound like Siri are not reachable from
//! here. `say` and `NSSpeechSynthesizer` see the MacinTalk and Vocalizer
//! voices; the Siri voices are a separate, entitled synthesiser
//! (`SiriTTS.framework`), and `AVSpeechSynthesizer` will not give them to an
//! app that is not Apple's. The best Juno can do is pick the best voice that
//! is installed, prefer Premium and Enhanced over compact when they are there,
//! and say where the better ones come from when they are not.

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
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, State};
use tracing::{info, warn};

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

/// Entry id for "do not speak out loud".
pub const SILENT_ID: &str = "silent";
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
// The macOS catalog
// ---------------------------------------------------------------------------

/// How good a macOS voice is, as `say` reports it in the name.
///
/// Apple ships one compact version of each voice and offers an Enhanced and a
/// Premium download; a downloaded one appears as a separate entry, "Samantha
/// (Enhanced)". Ordered worst to best on purpose: the derived `Ord` is what
/// picks the best installed version of a voice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum VoiceQuality {
    Compact,
    Enhanced,
    Premium,
}

/// Which version of a voice a `say` name is.
pub fn voice_quality(name: &str) -> VoiceQuality {
    let lower = name.to_ascii_lowercase();
    if lower.contains("(premium)") {
        VoiceQuality::Premium
    } else if lower.contains("(enhanced)") {
        VoiceQuality::Enhanced
    } else {
        VoiceQuality::Compact
    }
}

/// The voice behind a `say` name: "Ava (Premium)" is Ava.
pub fn voice_base_name(name: &str) -> &str {
    match name.find('(') {
        Some(paren) => name[..paren].trim_end(),
        None => name.trim(),
    }
}

/// One curated macOS voice, by the name it has with no quality suffix.
pub struct CatalogVoice {
    /// The base `say -v` name. The stored value is the full name of whichever
    /// version is installed, which may carry a suffix.
    pub name: &'static str,
    /// The locale `say` is expected to report for it. An installed voice whose
    /// locale disagrees is dropped, so the descriptor cannot lie about the
    /// accent.
    pub locale_prefix: &'static str,
    /// One line. What can be checked, never a tone nobody here has heard.
    pub descriptor: &'static str,
}

/// The short list, best first.
///
/// The order is the default chain: with nothing stored, Juno speaks as the
/// highest-quality entry on this Mac, and this is the tiebreak when two
/// entries are the same quality. American English leads because Juno's own
/// sample sentence and prompts are American English.
///
/// Ava, Zoe, Allison, Tom, Serena, Oliver and Lee are the modern Vocalizer
/// voices, which Apple offers as Enhanced and Premium downloads. Samantha,
/// Daniel, Karen and Moira are the compact ones every Mac has; they are last
/// because they are the voices somebody means by "robotic".
pub const SYSTEM_VOICE_CATALOG: &[CatalogVoice] = &[
    CatalogVoice {
        name: "Ava",
        locale_prefix: "en_US",
        descriptor: "American English.",
    },
    CatalogVoice {
        name: "Zoe",
        locale_prefix: "en_US",
        descriptor: "American English.",
    },
    CatalogVoice {
        name: "Allison",
        locale_prefix: "en_US",
        descriptor: "American English.",
    },
    CatalogVoice {
        name: "Tom",
        locale_prefix: "en_US",
        descriptor: "American English.",
    },
    CatalogVoice {
        name: "Serena",
        locale_prefix: "en_GB",
        descriptor: "British English.",
    },
    CatalogVoice {
        name: "Oliver",
        locale_prefix: "en_GB",
        descriptor: "British English.",
    },
    CatalogVoice {
        name: "Lee",
        locale_prefix: "en_AU",
        descriptor: "Australian English.",
    },
    CatalogVoice {
        name: "Samantha",
        locale_prefix: "en_US",
        descriptor: "American English. On every Mac.",
    },
    CatalogVoice {
        name: "Daniel",
        locale_prefix: "en_GB",
        descriptor: "British English.",
    },
    CatalogVoice {
        name: "Karen",
        locale_prefix: "en_AU",
        descriptor: "Australian English.",
    },
    CatalogVoice {
        name: "Moira",
        locale_prefix: "en_IE",
        descriptor: "Irish English.",
    },
];

/// A voice `say` says it has.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledVoice {
    pub name: String,
    pub locale: String,
}

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
/// Gathered once per command rather than per voice: listing the Mac's voices
/// spawns a process and listing Kokoro's reads a directory.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct VoiceInventory {
    /// What `say -v '?'` reports.
    pub macos: Vec<InstalledVoice>,
    /// The Kokoro voice embeddings on disk, by id.
    pub kokoro: Vec<String>,
    /// Why Kokoro could not get ready, when its last load failed.
    pub kokoro_problem: Option<String>,
}

/// Which voice will actually speak, and whether that is the one asked for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoiceResolution {
    /// The voice in force. `None` means the engine's own default, which for
    /// the Mac is whatever voice System Settings is set to.
    pub voice: Option<String>,
    /// True when something was stored that this engine cannot honour.
    pub substituted: bool,
}

// ---------------------------------------------------------------------------
// Parsing and enumeration
// ---------------------------------------------------------------------------

/// Read `say -v '?'`.
///
/// One line per voice: a name (which can contain spaces and a quality suffix
/// in brackets), the locale, then `#` and the voice's own sample sentence.
/// Pure, so the parsing is testable without running anything.
pub fn parse_say_voice_list(stdout: &str) -> Vec<InstalledVoice> {
    let mut voices = Vec::new();
    for line in stdout.lines() {
        let before_sample = line.split('#').next().unwrap_or("").trim();
        if before_sample.is_empty() {
            continue;
        }
        // The locale is the last column; everything before it is the name, and
        // the name is where the spaces are ("Bad News", "Ava (Premium)").
        let Some((name, locale)) = before_sample.rsplit_once(char::is_whitespace) else {
            continue;
        };
        let name = name.trim();
        let locale = locale.trim();
        if name.is_empty() || locale.is_empty() {
            continue;
        }
        voices.push(InstalledVoice {
            name: name.to_string(),
            locale: locale.to_string(),
        });
    }
    voices
}

/// The best installed version of one catalog entry.
///
/// An entry whose installed locale disagrees with the catalog is dropped: the
/// descriptor names an accent, and offering a voice under the wrong accent is
/// worse than offering one voice fewer.
fn best_installed<'a>(
    entry: &CatalogVoice,
    installed: &'a [InstalledVoice],
) -> Option<&'a InstalledVoice> {
    installed
        .iter()
        .filter(|voice| {
            voice_base_name(&voice.name) == entry.name
                && voice.locale.starts_with(entry.locale_prefix)
        })
        .max_by_key(|voice| voice_quality(&voice.name))
}

fn is_english(locale: &str) -> bool {
    locale.starts_with("en_") || locale.starts_with("en-")
}

/// The accent, from the locale `say` reports. Used for a downloaded voice the
/// catalog does not name, so its row still says something checkable.
fn accent_for_locale(locale: &str) -> &'static str {
    match locale.get(3..5).unwrap_or("") {
        "US" => "American English.",
        "GB" => "British English.",
        "AU" => "Australian English.",
        "IE" => "Irish English.",
        "IN" => "Indian English.",
        "ZA" => "South African English.",
        "NZ" => "New Zealand English.",
        _ => "English.",
    }
}

/// What a row says about itself. The quality is worth one word when it is
/// better than the version every Mac already has.
fn with_quality(descriptor: &str, quality: VoiceQuality) -> String {
    match quality {
        VoiceQuality::Premium => format!("{descriptor} Premium."),
        VoiceQuality::Enhanced => format!("{descriptor} Enhanced."),
        VoiceQuality::Compact => descriptor.to_string(),
    }
}

/// How a Mac voice ranks. Higher is better; see [`MacCandidate::rank`].
type MacRank<'a> = (
    VoiceQuality,
    bool,
    bool,
    std::cmp::Reverse<usize>,
    std::cmp::Reverse<&'a str>,
);

/// One Mac voice worth offering, with what it takes to rank it.
struct MacCandidate<'a> {
    voice: &'a InstalledVoice,
    /// Its place in [`SYSTEM_VOICE_CATALOG`], when the catalog names it.
    catalog_rank: Option<usize>,
    descriptor: &'static str,
}

impl<'a> MacCandidate<'a> {
    /// Quality first, always: a downloaded voice is the point of downloading
    /// it, and the compact ones are what somebody means by "robotic". Then a
    /// voice the catalog vouches for, then American English (Juno's prompts
    /// and sample are American English), then catalog order, then the name,
    /// so the ranking is total and never flickers between two reads.
    fn rank(&self) -> MacRank<'a> {
        (
            voice_quality(&self.voice.name),
            self.catalog_rank.is_some(),
            self.voice.locale.starts_with("en_US"),
            std::cmp::Reverse(self.catalog_rank.unwrap_or(usize::MAX)),
            std::cmp::Reverse(self.voice.name.as_str()),
        )
    }
}

/// Every Mac voice worth offering, best first.
///
/// The catalog, each entry at the best version installed, plus any other
/// English voice this Mac has as an Enhanced or Premium download. Those are
/// exactly the voices somebody went to System Settings to get, so they are
/// offered whatever they are called. Compact voices outside the catalog are
/// not: that is where the novelty voices live ("Bad News", "Bubbles").
fn mac_candidates(installed: &[InstalledVoice]) -> Vec<MacCandidate<'_>> {
    let mut candidates: Vec<MacCandidate<'_>> = SYSTEM_VOICE_CATALOG
        .iter()
        .enumerate()
        .filter_map(|(rank, entry)| {
            Some(MacCandidate {
                voice: best_installed(entry, installed)?,
                catalog_rank: Some(rank),
                descriptor: entry.descriptor,
            })
        })
        .collect();

    for voice in installed {
        let base = voice_base_name(&voice.name);
        let quality = voice_quality(&voice.name);
        let in_catalog = SYSTEM_VOICE_CATALOG
            .iter()
            .any(|entry| entry.name == base && voice.locale.starts_with(entry.locale_prefix));
        if in_catalog || quality == VoiceQuality::Compact || !is_english(&voice.locale) {
            continue;
        }
        // One row per voice: keep the best version of it.
        match candidates.iter_mut().find(|c| {
            c.catalog_rank.is_none()
                && voice_base_name(&c.voice.name) == base
                && c.voice.locale == voice.locale
        }) {
            Some(existing) if voice_quality(&existing.voice.name) >= quality => {}
            Some(existing) => existing.voice = voice,
            None => candidates.push(MacCandidate {
                voice,
                catalog_rank: None,
                descriptor: accent_for_locale(&voice.locale),
            }),
        }
    }

    candidates.sort_by_key(|candidate| std::cmp::Reverse(candidate.rank()));
    candidates
}

/// The voices this Mac has worth offering, best first, so the default is the
/// row at the top.
pub fn macos_voices(installed: &[InstalledVoice]) -> Vec<ProviderVoice> {
    mac_candidates(installed)
        .into_iter()
        .map(|candidate| ProviderVoice {
            id: candidate.voice.name.clone(),
            name: voice_base_name(&candidate.voice.name).to_string(),
            descriptor: with_quality(candidate.descriptor, voice_quality(&candidate.voice.name)),
        })
        .collect()
}

/// The default macOS voice: the top of the ranking.
///
/// 1. A Premium voice, catalog first, then any other English one.
/// 2. An Enhanced voice, same order.
/// 3. A compact catalog voice. On a Mac with nothing downloaded this is
///    Samantha, because Samantha is what a Mac ships with.
/// 4. `None`, which runs `say` with no `-v` at all and so uses whichever
///    voice the Mac is set to. A missing voice must not produce silence, and
///    the Mac's own choice is a real voice.
///
/// Chosen by rank, never by name, so a voice Apple ships next year is the
/// default the day somebody downloads it.
pub fn best_macos_voice(installed: &[InstalledVoice]) -> Option<String> {
    mac_candidates(installed)
        .first()
        .map(|candidate| candidate.voice.name.clone())
}

/// True when this Mac has no English voice better than the compact ones. The
/// pane offers the control that gets a better one.
pub fn only_compact_voices(installed: &[InstalledVoice]) -> bool {
    !installed.iter().any(|voice| {
        is_english(&voice.locale) && voice_quality(&voice.name) > VoiceQuality::Compact
    })
}

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

/// What each engine needs. This is the whole argument for the default: the
/// Mac's own voice needs none of it and starts speaking as `say` starts
/// synthesising, on every Mac Juno runs on.
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
/// Silence is the engine switched off, so there is no engine to list. The
/// Mac's own voice stands in, because it is the engine that coming back out of
/// silence needs no account, no key and no download for, and picking one of
/// its voices is how a person turns Juno's voice back on without going near
/// the advanced pane.
pub fn listed_engine(provider: &str) -> &str {
    if provider.trim().is_empty() || provider.eq_ignore_ascii_case(OFF_PROVIDER) {
        "system"
    } else {
        provider
    }
}

/// What this engine offers.
pub fn provider_voices(engine: &str, inventory: &VoiceInventory) -> ProviderVoices {
    match engine.to_ascii_lowercase().as_str() {
        "system" => ProviderVoices::Known(macos_voices(&inventory.macos)),
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
        "system" => best_macos_voice(&inventory.macos),
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
/// rather than being passed through: handing `say` a voice that has been
/// uninstalled, or Kokoro an embedding that is not on disk, is silence.
pub fn resolve_voice(
    engine: &str,
    inventory: &VoiceInventory,
    stored: Option<&str>,
) -> VoiceResolution {
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
        macos: Vec::new(),
        kokoro: installed_kokoro_voices(),
        kokoro_problem: None,
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
    /// `silent`, `system_default`, or an id the active engine knows.
    pub id: String,
    /// `silent`, `system`, or `voice`. The row draws itself from this.
    pub kind: String,
    pub name: String,
    pub descriptor: String,
    /// True for the row that is in force right now.
    pub selected: bool,
    /// Whether selecting this row makes a sound. Silence does not audition.
    pub speaks: bool,
}

/// The whole answer the Audio pane renders: the rows, the engine they belong
/// to, and anything that has to be said rather than shown.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct JunoVoiceList {
    /// The stored engine. `off` means Juno is silent.
    pub provider: String,
    /// The engine these rows belong to.
    pub engine: String,
    /// That engine's name as a person would say it.
    pub engine_label: String,
    pub options: Vec<JunoVoiceOption>,
    /// One sentence when there is something to say instead of rows: an engine
    /// whose voices are chosen elsewhere, an engine with nothing installed
    /// yet, or a stored choice that could not be honoured.
    pub note: Option<String>,
    /// True when the Mac is speaking and every voice it has is the compact
    /// version. The better ones are a download, and the pane says where.
    pub better_voices_available: bool,
    /// The engines the advanced picker offers, so the pane holds no list of
    /// engine names of its own.
    pub engines: Vec<EngineOption>,
}

/// Build the rows.
pub fn voice_list(
    provider: &str,
    inventory: &VoiceInventory,
    resolution: &VoiceResolution,
) -> JunoVoiceList {
    let silent = provider.eq_ignore_ascii_case(OFF_PROVIDER);
    let engine = listed_engine(provider).to_string();
    let is_mac = engine.eq_ignore_ascii_case("system");

    let mut options = vec![JunoVoiceOption {
        id: SILENT_ID.to_string(),
        kind: SILENT_ID.to_string(),
        name: "Silent".to_string(),
        descriptor: "Juno writes the answer and never says it out loud.".to_string(),
        selected: silent,
        speaks: false,
    }];

    let mut note: Option<String> = None;

    match provider_voices(&engine, inventory) {
        ProviderVoices::Known(rows) if !rows.is_empty() => {
            for row in rows {
                options.push(JunoVoiceOption {
                    selected: !silent && resolution.voice.as_deref() == Some(row.id.as_str()),
                    id: row.id,
                    kind: "voice".to_string(),
                    name: row.name,
                    descriptor: row.descriptor,
                    speaks: true,
                });
            }
        }
        // Nothing curated is installed. One honest row beats an empty list:
        // the Mac still has a voice, Juno just cannot name it.
        ProviderVoices::Known(_) if is_mac => {
            options.push(JunoVoiceOption {
                id: SYSTEM_DEFAULT_ID.to_string(),
                kind: "system".to_string(),
                name: "Your Mac's voice".to_string(),
                descriptor: "Whichever voice this Mac is set to use.".to_string(),
                selected: !silent,
                speaks: true,
            });
        }
        // Kokoro downloads its model and voices as it loads, which starts the
        // moment it is chosen. Until that lands there is nothing to list, and
        // if it failed the reason is the useful thing to show.
        ProviderVoices::Known(_) => {
            note = Some(match &inventory.kokoro_problem {
                Some(problem) => format!(
                    "{} could not get ready: {}. Your Mac's voice speaks until it can.",
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
            resolution
                .voice
                .as_deref()
                .unwrap_or("your Mac's own voice")
        ));
    }

    JunoVoiceList {
        provider: provider.to_string(),
        engine_label: engine_label(&engine).to_string(),
        better_voices_available: is_mac && only_compact_voices(&inventory.macos),
        engines: engine_options(),
        engine,
        options,
        note,
    }
}

// ---------------------------------------------------------------------------
// Reading this machine
// ---------------------------------------------------------------------------

/// Ask `say` which voices this Mac has.
///
/// `say -v '?'` prints the list and speaks nothing.
#[cfg(target_os = "macos")]
async fn installed_macos_voices() -> Vec<InstalledVoice> {
    let output = tokio::task::spawn_blocking(|| {
        std::process::Command::new("say")
            .arg("-v")
            .arg("?")
            .output()
    })
    .await;

    match output {
        Ok(Ok(output)) if output.status.success() => {
            parse_say_voice_list(&String::from_utf8_lossy(&output.stdout))
        }
        Ok(Ok(output)) => {
            warn!("[Voices] say -v '?' exited with {}", output.status);
            Vec::new()
        }
        Ok(Err(e)) => {
            warn!("[Voices] Could not list the Mac's voices: {e}");
            Vec::new()
        }
        Err(e) => {
            warn!("[Voices] Listing the Mac's voices did not finish: {e}");
            Vec::new()
        }
    }
}

#[cfg(not(target_os = "macos"))]
async fn installed_macos_voices() -> Vec<InstalledVoice> {
    Vec::new()
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

/// The Mac's voices as last read, and when.
///
/// Listing them spawns `say`, and that spawn used to sit between a tap on a
/// voice and the sample starting. A tap only needs to check the id it was
/// handed against a list the pane was drawn from moments ago, so it reads
/// this; drawing the pane always reads afresh, so a voice downloaded while
/// Juno is running shows up the next time the pane opens.
static MAC_VOICES: StdMutex<Option<(Instant, Vec<InstalledVoice>)>> = StdMutex::new(None);
const MAC_VOICES_FRESH_FOR: Duration = Duration::from_secs(60);

/// How old a reading of this machine may be.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Freshness {
    /// Read it now.
    Now,
    /// A reading from the last minute will do.
    Recent,
}

async fn macos_inventory(freshness: Freshness) -> Vec<InstalledVoice> {
    if freshness == Freshness::Recent {
        let cached = MAC_VOICES
            .lock()
            .ok()
            .and_then(|guard| guard.clone())
            .filter(|(read_at, _)| read_at.elapsed() < MAC_VOICES_FRESH_FOR);
        if let Some((_, voices)) = cached {
            return voices;
        }
    }
    let voices = installed_macos_voices().await;
    if !voices.is_empty() {
        if let Ok(mut guard) = MAC_VOICES.lock() {
            *guard = Some((Instant::now(), voices.clone()));
        }
    }
    voices
}

/// Everything this engine needs known about this machine, and nothing else:
/// the Mac's list costs a process and Kokoro's costs a directory read.
pub async fn inventory_for(engine: &str, freshness: Freshness) -> VoiceInventory {
    VoiceInventory {
        macos: if engine.eq_ignore_ascii_case("system") {
            macos_inventory(freshness).await
        } else {
            Vec::new()
        },
        kokoro: if engine.eq_ignore_ascii_case("kokoro") {
            installed_kokoro_voices()
        } else {
            Vec::new()
        },
        kokoro_problem: if engine.eq_ignore_ascii_case("kokoro") {
            crate::tts::kokoro::load_problem()
        } else {
            None
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
        "system" => state.set_system_voice(voice.map(str::to_string)),
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
/// `preparing` is emitted before anything slow starts. The Mac's voice is
/// speaking within a tenth of a second, but Kokoro loads an 82MB model on its
/// first sample, and several silent seconds with nothing on screen is the
/// thing that reads as broken.
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
    let app = app_handle.clone();
    crate::tts::kokoro::sync_with_engine(provider, move || {
        if let Err(e) = app.emit(ENGINE_READY_EVENT, "kokoro") {
            warn!("[Voices] Could not report that Kokoro is ready: {e}");
        }
    });
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
    let engine = listed_engine(&audio.tts_provider).to_string();
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

    Ok(voice_list(&audio.tts_provider, &inventory, &resolution))
}

/// Put the stored engine and voice in force: at startup, and after a reset.
///
/// The best voice this Mac has is only the default if it is actually in
/// force. With nothing stored, `say` used to run on whichever voice System
/// Settings happens to be set to, which on a Mac upgraded from an old one is a
/// voice from 2005. A reset wrote the defaults to disk and left the running
/// app speaking in the old voice until the next launch, so it runs this too.
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

    audio.tts_provider = provider.clone();
    let engine = listed_engine(&provider).to_string();
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
        provider,
        resolution.voice.as_deref().unwrap_or("its own default")
    );
    Ok(voice_list(&provider, &inventory, &resolution))
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

    let engine = listed_engine(&audio.tts_provider).to_string();
    // The pane was drawn from a fresh reading moments ago, so a recent one is
    // enough to check the id against, and it keeps a process spawn out of
    // the gap between the tap and the sound.
    let mut inventory = inventory_for(&engine, Freshness::Recent).await;

    if id == SILENT_ID {
        // Silence is the engine switched off. Every engine keeps the voice
        // chosen for it, so coming back out of silence returns to it.
        audio.tts_provider = OFF_PROVIDER.to_string();
        manager
            .set_audio_settings(&audio)
            .await
            .map_err(|e| format!("Failed to save Juno's voice: {e}"))?;
        state.set_tts_provider(OFF_PROVIDER.to_string())?;
        crate::tts::stop_speech();
        sync_engine_model(&app_handle, OFF_PROVIDER);
        info!("[Voices] Juno is silent");

        let stored = audio.voice_for(&engine).map(str::to_string);
        let resolution = resolve_voice(&engine, &inventory, stored.as_deref());
        return Ok(voice_list(&audio.tts_provider, &inventory, &resolution));
    }

    let offered = |inventory: &VoiceInventory| match provider_voices(&engine, inventory) {
        ProviderVoices::Known(rows) => rows.iter().any(|row| row.id == id),
        ProviderVoices::Elsewhere(_) => false,
    };
    let chosen = if id == SYSTEM_DEFAULT_ID && engine.eq_ignore_ascii_case("system") {
        None
    } else {
        if !offered(&inventory) {
            // A voice installed since the last reading.
            inventory = inventory_for(&engine, Freshness::Now).await;
        }
        if !offered(&inventory) {
            return Err(format!(
                "{} has no voice called {}.",
                engine_label(&engine),
                id
            ));
        }
        Some(id.clone())
    };

    let provider_changed = !audio.tts_provider.eq_ignore_ascii_case(&engine);
    audio.tts_provider = engine.clone();
    audio.set_voice_for(&engine, chosen.clone());
    manager
        .set_audio_settings(&audio)
        .await
        .map_err(|e| format!("Failed to save Juno's voice: {e}"))?;

    state.set_tts_provider(engine.clone())?;
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
    let list = voice_list(&audio.tts_provider, &inventory, &resolution);
    audition(&app_handle, state.inner(), &engine, &id);
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

        if audio.tts_provider.eq_ignore_ascii_case(OFF_PROVIDER) {
            // Silence is a choice. Replaying it would contradict it.
            return Ok(());
        }

        let engine = listed_engine(&audio.tts_provider).to_string();
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Real `say -v '?'` output, including the shapes that break naive
    /// parsing: a name with a space, a quality suffix in brackets, and a
    /// sample sentence containing a '#'.
    const SAY_OUTPUT: &str = "\
Albert              en_US    # Hello, my name is Albert.
Bad News            en_US    # The light you see at the end of the tunnel is the headlamp of a train.
Daniel              en_GB    # Hello, my name is Daniel. I am a British-English voice.
Fred                en_US    # Hello, my name is Fred. Issue #1 is me.
Karen               en_AU    # Hello, my name is Karen. I am an Australian-English voice.
Moira               en_IE    # Hello, my name is Moira. I am an Irish-English voice.
Samantha            en_US    # Hello, my name is Samantha. I am an American-English voice.
Thomas              fr_FR    # Bonjour, je m'appelle Thomas.
";

    /// The same Mac after somebody downloaded the good voices.
    const SAY_OUTPUT_WITH_DOWNLOADS: &str = "\
Ava (Premium)       en_US    # Hello, my name is Ava.
Daniel              en_GB    # Hello, my name is Daniel.
Daniel (Enhanced)   en_GB    # Hello, my name is Daniel.
Samantha            en_US    # Hello, my name is Samantha.
Samantha (Enhanced) en_US    # Hello, my name is Samantha.
";

    fn stock_mac() -> VoiceInventory {
        VoiceInventory {
            macos: parse_say_voice_list(SAY_OUTPUT),
            kokoro: Vec::new(),
            kokoro_problem: None,
        }
    }

    fn mac_with_downloads() -> VoiceInventory {
        VoiceInventory {
            macos: parse_say_voice_list(SAY_OUTPUT_WITH_DOWNLOADS),
            kokoro: Vec::new(),
            kokoro_problem: None,
        }
    }

    fn kokoro_mac() -> VoiceInventory {
        VoiceInventory {
            macos: parse_say_voice_list(SAY_OUTPUT),
            kokoro: vec![
                "af_heart".to_string(),
                "am_michael".to_string(),
                "bf_emma".to_string(),
            ],
            kokoro_problem: None,
        }
    }

    fn list_for(provider: &str, inventory: &VoiceInventory, stored: Option<&str>) -> JunoVoiceList {
        let engine = listed_engine(provider);
        let resolution = resolve_voice(engine, inventory, stored);
        voice_list(provider, inventory, &resolution)
    }

    // -- parsing -----------------------------------------------------------

    #[test]
    fn a_name_with_a_space_survives_parsing() {
        let voices = parse_say_voice_list(SAY_OUTPUT);
        assert!(voices.iter().any(|v| v.name == "Bad News"));
    }

    #[test]
    fn a_quality_suffix_survives_parsing_and_is_read() {
        let voices = parse_say_voice_list(SAY_OUTPUT_WITH_DOWNLOADS);
        assert!(voices.iter().any(|v| v.name == "Samantha (Enhanced)"));
        assert_eq!(voice_quality("Samantha (Enhanced)"), VoiceQuality::Enhanced);
        assert_eq!(voice_quality("Ava (Premium)"), VoiceQuality::Premium);
        assert_eq!(voice_quality("Samantha"), VoiceQuality::Compact);
        assert_eq!(voice_base_name("Ava (Premium)"), "Ava");
        assert_eq!(voice_base_name("Samantha"), "Samantha");
    }

    #[test]
    fn every_parsed_voice_has_a_name_and_a_locale() {
        let voices = parse_say_voice_list(SAY_OUTPUT);
        assert_eq!(voices.len(), 8, "{voices:?}");
        for voice in voices {
            assert!(!voice.name.is_empty());
            assert!(voice.locale.contains('_'), "{:?}", voice.locale);
        }
    }

    // -- the list is a function of the engine ------------------------------

    /// The defect this file was rewritten for: Kokoro was offered macOS voices.
    #[test]
    fn the_list_belongs_to_the_engine_that_is_speaking() {
        let kokoro = list_for("kokoro", &kokoro_mac(), None);
        assert_eq!(kokoro.engine, "kokoro");
        assert_eq!(kokoro.engine_label, "Kokoro");
        let ids: Vec<&str> = kokoro.options.iter().map(|o| o.id.as_str()).collect();
        assert!(ids.contains(&"af_heart"), "{ids:?}");
        for mac_voice in ["Samantha", "Daniel", "Karen", "Moira"] {
            assert!(
                !ids.contains(&mac_voice),
                "{mac_voice} is not a voice Kokoro has"
            );
        }

        let mac = list_for("system", &kokoro_mac(), None);
        let ids: Vec<&str> = mac.options.iter().map(|o| o.id.as_str()).collect();
        assert!(ids.contains(&"Samantha"), "{ids:?}");
        assert!(!ids.contains(&"af_heart"), "{ids:?}");
    }

    /// A hundred voices is not a choice, and a novelty voice is not a choice
    /// either.
    #[test]
    fn the_mac_list_curates_rather_than_dumping() {
        let list = list_for("system", &stock_mac(), None);
        assert!(
            list.options.len() <= SYSTEM_VOICE_CATALOG.len() + 1,
            "{} rows",
            list.options.len()
        );
        for noise in ["Bad News", "Fred", "Albert", "Thomas"] {
            assert!(
                !list.options.iter().any(|o| o.name == noise),
                "{noise} should not be offered"
            );
        }
    }

    /// An engine whose voices Juno cannot enumerate says so. Rows it could not
    /// honour would be worse than no rows.
    #[test]
    fn an_engine_juno_cannot_enumerate_reports_instead_of_guessing() {
        for engine in ["elevenlabs", "replicate", "chatterbox"] {
            let list = list_for(engine, &VoiceInventory::default(), None);
            assert_eq!(list.engine, engine);
            assert_eq!(
                list.options.len(),
                1,
                "{engine} should offer silence and nothing it cannot honour"
            );
            assert_eq!(list.options[0].id, SILENT_ID);
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
        assert_eq!(list.options.len(), 1);
        assert!(list.note.unwrap_or_default().contains("Kokoro"));
    }

    // -- switching engines -------------------------------------------------

    /// The pin for the bug: switching engine must leave a voice the new engine
    /// actually has, not the one the old engine named.
    #[test]
    fn switching_engine_yields_a_voice_that_engine_has() {
        let inventory = kokoro_mac();

        // On the Mac, with a Mac voice stored.
        let mac = resolve_voice("system", &inventory, Some("Daniel"));
        assert_eq!(mac.voice.as_deref(), Some("Daniel"));

        // Switch to Kokoro. The Mac's voice means nothing here.
        let kokoro = resolve_voice("kokoro", &inventory, Some("Daniel"));
        assert_eq!(kokoro.voice.as_deref(), Some(KOKORO_DEFAULT_VOICE));
        assert!(kokoro.substituted, "the pane has to be able to say why");

        // And back again.
        let back = resolve_voice("system", &inventory, Some("af_heart"));
        assert!(back.substituted);
        assert_eq!(
            back.voice,
            best_macos_voice(&inventory.macos),
            "a Kokoro id must not survive into `say`"
        );
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
            assert!(resolution.substituted, "{engine}");
            assert_eq!(resolution.voice, default_voice(engine, &kokoro_mac()));
        }
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

    // -- the default chain -------------------------------------------------

    /// Rung 3: a stock Mac has only compact voices, and Samantha is the one
    /// every Mac has.
    #[test]
    fn a_stock_mac_defaults_to_the_voice_every_mac_has() {
        assert_eq!(
            best_macos_voice(&stock_mac().macos),
            Some("Samantha".to_string())
        );
        assert!(
            only_compact_voices(&stock_mac().macos),
            "the pane should offer to go and get a better one"
        );
    }

    /// Rungs 1 and 2: a downloaded voice is the point of downloading it, so
    /// quality wins over catalog position, and Premium wins over Enhanced.
    #[test]
    fn a_downloaded_voice_wins_over_a_compact_one() {
        let inventory = mac_with_downloads();
        assert_eq!(
            best_macos_voice(&inventory.macos),
            Some("Ava (Premium)".to_string())
        );
        assert!(!only_compact_voices(&inventory.macos));

        // With no Premium installed, the best Enhanced one wins, even though
        // the compact Samantha sits higher in the catalog than Daniel.
        let enhanced_only: Vec<InstalledVoice> = inventory
            .macos
            .iter()
            .filter(|voice| voice_quality(&voice.name) != VoiceQuality::Premium)
            .cloned()
            .collect();
        assert_eq!(
            voice_quality(&best_macos_voice(&enhanced_only).unwrap_or_default()),
            VoiceQuality::Enhanced
        );
    }

    /// Rung 4: a Mac with none of the catalog still speaks. `None` runs `say`
    /// with no `-v`, which is the voice the Mac is set to, not silence.
    #[test]
    fn a_mac_with_no_catalog_voice_falls_back_to_the_macs_own() {
        let bare = VoiceInventory {
            macos: parse_say_voice_list("Zarvox              en_US    # Hello.\n"),
            kokoro: Vec::new(),
            kokoro_problem: None,
        };
        assert_eq!(best_macos_voice(&bare.macos), None);

        let list = list_for("system", &bare, Some("Samantha"));
        assert_eq!(list.options.len(), 2, "silence plus the Mac's own voice");
        assert_eq!(list.options[1].id, SYSTEM_DEFAULT_ID);
        assert!(list.options[1].selected);
        assert!(list.options[1].speaks, "the Mac's own voice is not silence");
    }

    /// A row offers the best version of itself, so a download is in force
    /// without anybody choosing again.
    #[test]
    fn a_row_offers_the_best_version_of_itself() {
        let rows = macos_voices(&mac_with_downloads().macos);
        let samantha = rows
            .iter()
            .find(|row| row.name == "Samantha")
            .expect("Samantha is installed");
        assert_eq!(samantha.id, "Samantha (Enhanced)");
        assert!(samantha.descriptor.ends_with("Enhanced."));
        assert_eq!(rows.iter().filter(|row| row.name == "Samantha").count(), 1);
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

    #[test]
    fn silence_is_a_row_and_it_does_not_speak() {
        let list = list_for(OFF_PROVIDER, &stock_mac(), None);
        let silent = &list.options[0];
        assert_eq!(silent.id, SILENT_ID);
        assert!(
            silent.selected,
            "silence is in force when the engine is off"
        );
        assert!(!silent.speaks, "selecting silence must not make a sound");
        assert_eq!(
            list.options.iter().filter(|o| o.selected).count(),
            1,
            "exactly one row is in force"
        );
    }

    /// Silence has no engine, so the rows are the Mac's: picking one is how
    /// somebody turns Juno's voice back on without the advanced pane.
    #[test]
    fn silence_still_offers_a_way_back() {
        assert_eq!(listed_engine(OFF_PROVIDER), "system");
        assert_eq!(listed_engine(""), "system");
        assert_eq!(listed_engine("kokoro"), "kokoro");

        let list = list_for(OFF_PROVIDER, &stock_mac(), None);
        assert!(list.options.iter().any(|o| o.id == "Samantha" && o.speaks));
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

    /// A voice that was uninstalled must not leave the list showing nothing in
    /// force while a different voice does the talking.
    #[test]
    fn an_uninstalled_choice_lights_the_voice_that_will_actually_speak() {
        let list = list_for("system", &stock_mac(), Some("Fiona"));
        let lit = list
            .options
            .iter()
            .find(|o| o.selected)
            .map(|o| o.id.clone())
            .unwrap_or_default();
        assert_eq!(
            lit,
            best_macos_voice(&stock_mac().macos).unwrap_or_default()
        );
        assert!(
            list.note.unwrap_or_default().contains("not on this Mac"),
            "the pane has to say why the selection moved"
        );
    }

    // -- the words ---------------------------------------------------------

    #[test]
    fn every_descriptor_is_one_plain_line() {
        for entry in SYSTEM_VOICE_CATALOG {
            assert!(!entry.descriptor.is_empty(), "{} has no line", entry.name);
            assert!(
                entry.descriptor.ends_with('.'),
                "{} is not a sentence",
                entry.name
            );
            assert!(
                !entry.descriptor.contains('—'),
                "{} uses an em dash",
                entry.name
            );
            assert!(
                !entry.descriptor.contains('\n'),
                "{} is more than one line",
                entry.name
            );
        }
    }

    /// Every catalog entry names an English accent, because the descriptor is
    /// the only thing separating two rows that both just say a name.
    #[test]
    fn every_catalog_entry_is_an_english_voice() {
        for entry in SYSTEM_VOICE_CATALOG {
            assert!(
                entry.locale_prefix.starts_with("en_"),
                "{} is {}",
                entry.name,
                entry.locale_prefix
            );
        }
    }

    /// A voice installed under a different accent than the catalog claims is
    /// dropped, because the descriptor names the accent.
    #[test]
    fn a_locale_mismatch_drops_the_entry() {
        let wrong = vec![InstalledVoice {
            name: "Daniel".to_string(),
            locale: "en_US".to_string(),
        }];
        assert!(macos_voices(&wrong).is_empty());
    }

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

    /// The default is the engine that needs nothing: no key, no network, no
    /// download, no server. Anything else is silence, or a wait, on a fresh
    /// install.
    #[test]
    fn the_default_engine_needs_nothing() {
        let default = crate::constants::settings::defaults::TTS_PROVIDER;
        assert!(engine_needs(default).nothing(), "{default} needs something");
        assert!(ENGINES.contains(&default));
        assert_eq!(listed_engine(default), "system");
    }

    /// Kokoro sounds good but downloads 82MB and renders a whole clip before
    /// the first sound, and every cloud engine needs an account. None of them
    /// can be the default for that reason, and each says why.
    #[test]
    fn every_other_engine_needs_something_and_so_is_not_the_default() {
        for engine in ENGINES.iter().filter(|e| **e != "system") {
            assert!(!engine_needs(engine).nothing(), "{engine}");
            assert_ne!(*engine, crate::constants::settings::defaults::TTS_PROVIDER);
        }
        assert!(engine_needs("kokoro").download);
        for cloud in ["elevenlabs", "replicate", "chatterbox"] {
            assert!(engine_needs(cloud).account_key, "{cloud}");
        }
    }

    /// The same default on an Intel Mac. Nothing about it is gated by
    /// architecture, and this pins that it stays so on the x86_64 build.
    #[cfg(target_arch = "x86_64")]
    #[test]
    fn an_intel_mac_gets_the_same_default_engine() {
        assert_eq!(crate::constants::settings::defaults::TTS_PROVIDER, "system");
        assert!(engine_needs(crate::constants::settings::defaults::TTS_PROVIDER).nothing());
    }

    // -- ranking by quality, not by name -----------------------------------

    /// A downloaded voice the catalog has never heard of is still the best
    /// voice on the Mac, and the default, because it is ranked by quality.
    /// The novelty voices stay out.
    #[test]
    fn a_downloaded_voice_the_catalog_does_not_name_wins_on_quality() {
        let installed = parse_say_voice_list(
            "\
Eddy (English (US)) en_US    # Hello! My name is Eddy.
Evan (Enhanced)     en_US    # Hello! My name is Evan.
Samantha            en_US    # Hello! My name is Samantha.
Bubbles             en_US    # Hello! My name is Bubbles.
",
        );
        assert_eq!(
            best_macos_voice(&installed),
            Some("Evan (Enhanced)".to_string())
        );
        let rows = macos_voices(&installed);
        assert_eq!(rows[0].id, "Evan (Enhanced)", "the default is the top row");
        assert_eq!(rows[0].name, "Evan");
        assert_eq!(rows[0].descriptor, "American English. Enhanced.");
        let names: Vec<&str> = rows.iter().map(|r| r.name.as_str()).collect();
        assert!(!names.contains(&"Eddy"), "{names:?}");
        assert!(!names.contains(&"Bubbles"), "{names:?}");
        assert!(!only_compact_voices(&installed));
    }

    /// With nothing stored (a fresh install, or a reset) the Mac speaks in the
    /// best voice installed, and the list is ordered so that is the top row.
    #[test]
    fn nothing_stored_resolves_to_the_best_voice_installed() {
        let inventory = mac_with_downloads();
        let resolution = resolve_voice("system", &inventory, None);
        assert_eq!(resolution.voice.as_deref(), Some("Ava (Premium)"));
        assert!(!resolution.substituted);
        let rows = macos_voices(&inventory.macos);
        assert_eq!(rows[0].id, "Ava (Premium)");
        let qualities: Vec<VoiceQuality> = rows.iter().map(|r| voice_quality(&r.id)).collect();
        let mut sorted = qualities.clone();
        sorted.sort();
        sorted.reverse();
        assert_eq!(qualities, sorted, "best first");
    }

    // -- each engine lists its own voices ----------------------------------

    /// Every engine's rows are that engine's voices and nobody else's, and the
    /// voice it resolves to is one of its own rows.
    #[test]
    fn every_engine_lists_only_its_own_voices() {
        let inventory = kokoro_mac();
        let mac_ids: Vec<String> = inventory.macos.iter().map(|v| v.name.clone()).collect();
        for engine in ENGINES {
            let list = list_for(engine, &inventory, None);
            assert_eq!(list.engine, *engine);
            let rows: Vec<&JunoVoiceOption> =
                list.options.iter().filter(|o| o.kind == "voice").collect();
            for row in &rows {
                let belongs = match *engine {
                    "system" => mac_ids.contains(&row.id),
                    "kokoro" => inventory.kokoro.contains(&row.id),
                    "supertonic" => ["M1", "F1"].contains(&row.id.as_str()),
                    _ => false,
                };
                assert!(belongs, "{engine} offered {}", row.id);
            }
            let resolved = resolve_voice(engine, &inventory, None).voice;
            if let Some(voice) = resolved {
                assert!(
                    rows.iter().any(|row| row.id == voice),
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
}
