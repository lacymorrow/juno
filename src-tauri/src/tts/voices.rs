//! Juno's voice: a short list of macOS voices, each one spoken on selection.
//!
//! The model is the bar appearance picker (`src/components/bar/appearanceCatalog.ts`):
//! a few curated entries, each with a name and one honest line, and you can
//! see what you are choosing before you keep it. The equivalent of "you can
//! see it" for a voice is "you can hear it", so selecting an entry speaks a
//! sample in that voice. A voice list you cannot hear is a list of names.
//!
//! Curated, not dumped. `say -v '?'` lists more than a hundred voices, most of
//! them other languages and novelty voices ("Bad News", "Pipe Organ"), and a
//! hundred rows is not a choice. The catalog below is the short list; entries
//! that are not installed on this Mac are dropped rather than offered.
//!
//! Descriptors say what can be checked. Nobody here has heard these voices, so
//! none of them claims a tone: each one names the accent, which is the useful
//! difference between them and is confirmed against the locale `say` reports.
//! The sample is what tells you how it sounds.

use crate::state::AppState;
use serde::Serialize;
use std::sync::atomic::{AtomicU64, Ordering};
use tauri::{AppHandle, State};
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

/// One curated macOS voice.
pub struct CatalogVoice {
    /// The `say -v` name, which is also the stored value.
    pub name: &'static str,
    /// The locale `say` is expected to report for it. An installed voice whose
    /// locale disagrees is dropped, so the descriptor cannot lie about the
    /// accent.
    pub locale_prefix: &'static str,
    /// One line. What can be checked, never a tone nobody here has heard.
    pub descriptor: &'static str,
}

/// The short list, in the order the picker walks through it.
pub const SYSTEM_VOICE_CATALOG: &[CatalogVoice] = &[
    CatalogVoice {
        name: "Samantha",
        locale_prefix: "en_US",
        descriptor: "American English. Juno's default.",
    },
    CatalogVoice {
        name: "Alex",
        locale_prefix: "en_US",
        descriptor: "American English. The voice Macs have shipped with since 2005.",
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

/// What the picker draws, one row each.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct JunoVoiceOption {
    /// `silent`, `system_default`, or a macOS voice name.
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

/// A voice `say` says it has.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledVoice {
    pub name: String,
    pub locale: String,
}

/// Read `say -v '?'`.
///
/// One line per voice: a name (which can contain spaces), the locale, then
/// `#` and the voice's own sample sentence. Pure, so the parsing is testable
/// without running anything.
pub fn parse_say_voice_list(stdout: &str) -> Vec<InstalledVoice> {
    let mut voices = Vec::new();
    for line in stdout.lines() {
        let before_sample = line.split('#').next().unwrap_or("").trim();
        if before_sample.is_empty() {
            continue;
        }
        // The locale is the last column; everything before it is the name, and
        // the name is where the spaces are ("Bad News", "Grandma (Enhanced)").
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

/// The catalog entries this Mac actually has, in catalog order.
///
/// An entry whose installed locale disagrees with the catalog is dropped: the
/// descriptor names an accent, and offering a voice under the wrong accent is
/// worse than offering one voice fewer.
pub fn curated_voices(installed: &[InstalledVoice]) -> Vec<&'static CatalogVoice> {
    SYSTEM_VOICE_CATALOG
        .iter()
        .filter(|entry| {
            installed.iter().any(|voice| {
                voice.name == entry.name && voice.locale.starts_with(entry.locale_prefix)
            })
        })
        .collect()
}

/// The rows the Audio pane shows, and which one is in force.
///
/// `provider` is the stored TTS provider: `off` means the person chose
/// silence. A provider other than the Mac's own voice still shows this list,
/// because picking a row here is how someone comes back to a Mac voice.
pub fn voice_options(
    installed: &[InstalledVoice],
    provider: &str,
    chosen_voice: Option<&str>,
) -> Vec<JunoVoiceOption> {
    let silent = provider.eq_ignore_ascii_case("off");
    let curated = curated_voices(installed);

    let mut options = vec![JunoVoiceOption {
        id: SILENT_ID.to_string(),
        kind: SILENT_ID.to_string(),
        name: "Silent".to_string(),
        descriptor: "Juno writes the answer and never says it out loud.".to_string(),
        selected: silent,
        speaks: false,
    }];

    // Nothing curated is installed. One honest row beats an empty list: the
    // Mac still has a voice, Juno just cannot name it.
    if curated.is_empty() {
        options.push(JunoVoiceOption {
            id: SYSTEM_DEFAULT_ID.to_string(),
            kind: "system".to_string(),
            name: "Your Mac's voice".to_string(),
            descriptor: "Whichever voice this Mac is set to use.".to_string(),
            selected: !silent,
            speaks: true,
        });
        return options;
    }

    let chosen = chosen_voice
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .filter(|v| curated.iter().any(|entry| entry.name == *v));
    // No stored voice, or one that is not installed any more: the first
    // curated entry is what will actually speak, so that is the row to light.
    let effective = chosen.unwrap_or(curated[0].name);

    for entry in curated {
        options.push(JunoVoiceOption {
            id: entry.name.to_string(),
            kind: "voice".to_string(),
            name: entry.name.to_string(),
            descriptor: entry.descriptor.to_string(),
            selected: !silent && entry.name == effective,
            speaks: true,
        });
    }
    options
}

/// The voice `say` should use: the stored one if it is installed, else the
/// first curated one, else none at all (which leaves `say` on the Mac's own).
pub fn effective_voice_name(installed: &[InstalledVoice], chosen: Option<&str>) -> Option<String> {
    let curated = curated_voices(installed);
    if let Some(chosen) = chosen.map(str::trim).filter(|v| !v.is_empty()) {
        if curated.iter().any(|entry| entry.name == chosen) {
            return Some(chosen.to_string());
        }
        warn!("[Voices] {chosen} is not installed; falling back to a voice that is");
    }
    curated.first().map(|entry| entry.name.to_string())
}

/// Ask `say` which voices this Mac has.
///
/// `say -v '?'` prints the list and speaks nothing.
#[cfg(target_os = "macos")]
async fn installed_voices() -> Vec<InstalledVoice> {
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
async fn installed_voices() -> Vec<InstalledVoice> {
    Vec::new()
}

/// The rows the Audio pane draws.
#[tauri::command]
pub async fn get_juno_voices(app_handle: AppHandle) -> Result<Vec<JunoVoiceOption>, String> {
    let settings_manager = crate::settings::manager::SettingsManager::new(app_handle)
        .map_err(|e| format!("Failed to create settings manager: {e}"))?;
    let audio = settings_manager
        .get_audio_settings()
        .await
        .map_err(|e| format!("Failed to get audio settings: {e}"))?;

    let installed = installed_voices().await;
    Ok(voice_options(
        &installed,
        &audio.tts_provider,
        audio.system_voice.as_deref(),
    ))
}

/// Pick Juno's voice, and hear it.
///
/// Selecting is the audition: the sample plays in the voice just chosen, so
/// the choice is made with the answer already in your ears rather than after
/// saving and backing out. `silent` is the one row that does not speak.
#[tauri::command]
pub async fn set_juno_voice(
    id: String,
    app_handle: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let settings_manager = crate::settings::manager::SettingsManager::new(app_handle.clone())
        .map_err(|e| format!("Failed to create settings manager: {e}"))?;
    let mut audio = settings_manager
        .get_audio_settings()
        .await
        .map_err(|e| format!("Failed to get audio settings: {e}"))?;

    let silent = id == SILENT_ID;
    let voice = match id.as_str() {
        SILENT_ID => audio.system_voice.clone(),
        SYSTEM_DEFAULT_ID => None,
        name => Some(name.to_string()),
    };

    audio.tts_provider = if silent {
        "off".to_string()
    } else {
        "system".to_string()
    };
    audio.system_voice = voice.clone();

    settings_manager
        .set_audio_settings(&audio)
        .await
        .map_err(|e| format!("Failed to save Juno's voice: {e}"))?;
    state.set_tts_provider(audio.tts_provider.clone())?;
    state.set_system_voice(voice)?;

    info!("[Voices] Juno's voice set to {id}");

    if silent {
        // Nothing to audition, and speaking here would contradict the choice.
        crate::tts::stop_speech();
        return Ok(());
    }

    speak_sample(&state).await
}

/// Say the sample again in the voice already chosen.
#[tauri::command]
pub async fn preview_juno_voice(state: State<'_, AppState>) -> Result<(), String> {
    speak_sample(&state).await
}

/// Speak the sample in the stored voice, through the stored speaker.
///
/// Anything already playing is stopped first: tapping a second voice should
/// interrupt the first, not queue behind it.
async fn speak_sample(state: &State<'_, AppState>) -> Result<(), String> {
    let generation = AUDITION.fetch_add(1, Ordering::SeqCst) + 1;
    crate::tts::stop_speech();
    crate::tts::reset_tts_stop_flag();

    let installed = installed_voices().await;
    let chosen = state.get_system_voice().unwrap_or_default();
    let voice = effective_voice_name(&installed, chosen.as_deref());
    let device = state.get_output_device().unwrap_or_default();

    let outcome =
        crate::tts::system::speak_directly(VOICE_SAMPLE_TEXT.to_string(), voice, device).await;

    if AUDITION.load(Ordering::SeqCst) != generation {
        // Another voice was tapped while this one was speaking. Being cut off
        // is what was asked for, so it is not reported.
        return Ok(());
    }

    outcome.map(|finish| info!("[Voices] Sample finished: {finish}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Real `say -v '?'` output, including the shapes that break naive
    /// parsing: a name with a space, and a sample sentence containing a '#'.
    const SAY_OUTPUT: &str = "\
Alex                en_US    # Most people recognize me by my voice.
Bad News            en_US    # The light you see at the end of the tunnel is the headlamp of a train.
Daniel              en_GB    # Hello, my name is Daniel. I am a British-English voice.
Karen               en_AU    # Hello, my name is Karen. I am an Australian-English voice.
Moira               en_IE    # Hello, my name is Moira. I am an Irish-English voice.
Samantha            en_US    # Hello, my name is Samantha. I am an American-English voice.
Thomas              fr_FR    # Bonjour, je m'appelle Thomas.
Grandma (Enhanced)  en_US    # Hello, my name is Grandma. Issue #1 is my favourite.
";

    fn installed() -> Vec<InstalledVoice> {
        parse_say_voice_list(SAY_OUTPUT)
    }

    #[test]
    fn a_name_with_a_space_survives_parsing() {
        let voices = installed();
        assert!(voices.iter().any(|v| v.name == "Bad News"));
        assert!(voices.iter().any(|v| v.name == "Grandma (Enhanced)"));
    }

    #[test]
    fn every_parsed_voice_has_a_name_and_a_locale() {
        let voices = installed();
        assert_eq!(voices.len(), 8, "{voices:?}");
        for voice in voices {
            assert!(!voice.name.is_empty());
            assert!(voice.locale.contains('_'), "{:?}", voice.locale);
        }
    }

    /// A hundred voices is not a choice. The picker shows the short list.
    #[test]
    fn the_picker_curates_rather_than_dumping() {
        let options = voice_options(&installed(), "system", None);
        assert!(
            options.len() <= SYSTEM_VOICE_CATALOG.len() + 1,
            "{} rows",
            options.len()
        );
        for noise in ["Bad News", "Thomas", "Grandma (Enhanced)"] {
            assert!(
                !options.iter().any(|o| o.name == noise),
                "{noise} should not be offered"
            );
        }
    }

    #[test]
    fn silence_is_a_row_and_it_does_not_speak() {
        let options = voice_options(&installed(), "off", None);
        let silent = &options[0];
        assert_eq!(silent.id, SILENT_ID);
        assert!(
            silent.selected,
            "silence is in force when the provider is off"
        );
        assert!(!silent.speaks, "selecting silence must not make a sound");
        assert_eq!(
            options.iter().filter(|o| o.selected).count(),
            1,
            "exactly one row is in force"
        );
    }

    #[test]
    fn exactly_one_voice_is_in_force() {
        for chosen in [None, Some("Daniel"), Some("Nobody")] {
            let options = voice_options(&installed(), "system", chosen);
            assert_eq!(
                options.iter().filter(|o| o.selected).count(),
                1,
                "chosen: {chosen:?}"
            );
        }
    }

    /// A voice that was uninstalled must not leave the list showing nothing in
    /// force while a different voice does the talking.
    #[test]
    fn an_uninstalled_choice_lights_the_voice_that_will_actually_speak() {
        let options = voice_options(&installed(), "system", Some("Fiona"));
        let lit = options
            .iter()
            .find(|o| o.selected)
            .map(|o| o.name.clone())
            .unwrap_or_default();
        assert_eq!(
            lit,
            effective_voice_name(&installed(), Some("Fiona")).unwrap_or_default()
        );
    }

    #[test]
    fn an_installed_choice_is_what_speaks() {
        assert_eq!(
            effective_voice_name(&installed(), Some("Karen")),
            Some("Karen".to_string())
        );
    }

    #[test]
    fn no_voices_at_all_still_offers_the_macs_own() {
        let options = voice_options(&[], "system", None);
        assert_eq!(options.len(), 2, "silence plus the Mac's own voice");
        assert_eq!(options[1].id, SYSTEM_DEFAULT_ID);
        assert_eq!(effective_voice_name(&[], Some("Samantha")), None);
    }

    /// A voice installed under a different accent than the catalog claims is
    /// dropped, because the descriptor names the accent.
    #[test]
    fn a_locale_mismatch_drops_the_entry() {
        let wrong = vec![InstalledVoice {
            name: "Daniel".to_string(),
            locale: "en_US".to_string(),
        }];
        assert!(curated_voices(&wrong).is_empty());
    }

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

    #[test]
    fn the_sample_is_short_enough_to_interrupt() {
        assert!(VOICE_SAMPLE_TEXT.len() < 60, "{VOICE_SAMPLE_TEXT}");
        assert!(!VOICE_SAMPLE_TEXT.contains('—'));
    }
}
