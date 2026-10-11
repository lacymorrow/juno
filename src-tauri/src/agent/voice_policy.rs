//! Whether Juno's answers are spoken, and what the system prompt asks for
//! because of it.
//!
//! The prompt tells the model to open every reply with a short `<TTS>` line
//! and to keep speech and text apart. When nobody will hear it (the voice is
//! set to Silent) all of that is wasted: the model writes
//! sentences nothing plays, and every turn carries a few thousand prompt tokens
//! of speech rules. So with voice off the instructions are left out of the
//! prompt entirely, for every provider, and the reply is plain text.
//!
//! The prompt is also part of the Claude CLI session's launch fingerprint
//! (`claude_cli_session::signature_parts`), so a live session started with the
//! speech rules is not reused after the voice goes off, and the other way
//! round. The warm spare remembers which kind of prompt it was primed with and
//! is not kept when that no longer matches (`claude_cli::prewarm_persistent_session`).

use super::prompts::PromptFragments;
use crate::state::AppState;
use tauri::{AppHandle, Manager};

/// Whether anything Juno says will be heard.
///
/// Off means the engine is `off`, which is Silent in the voice picker (or no
/// engine at all). `tts::enqueue` already refuses to speak then; this is the
/// same rule applied before the model is asked.
///
/// "Play sounds" is not part of it: that switch is the dictation cues, and the
/// Mac's own voice speaks through `say` whatever it is set to.
pub fn voice_output_on(provider: &str) -> bool {
    let provider = provider.trim();
    !provider.is_empty() && !provider.eq_ignore_ascii_case("off")
}

/// [`voice_output_on`] for the running app. Defaults to on when the state is
/// not there yet, so a launch race can only cost tokens, never silence.
pub fn voice_enabled_now(app: &AppHandle) -> bool {
    let Some(state) = app.try_state::<AppState>() else {
        return true;
    };
    match state.get_tts_provider() {
        Ok(provider) => voice_output_on(&provider),
        Err(_) => true,
    }
}

/// [`voice_output_on`] from what is saved. The prompt is built from this, so
/// it is right even before the running state has been filled in.
pub async fn voice_enabled_from_settings(
    manager: &crate::settings::manager::SettingsManager,
) -> bool {
    match manager.get_audio_settings().await {
        Ok(audio) => voice_output_on(&audio.tts_provider),
        Err(_) => true,
    }
}

/// The prompt as it should be sent: untouched when the voice is on, without
/// the speech instructions when it is off.
pub fn apply(prompt: String, voice_on: bool) -> String {
    if voice_on {
        prompt
    } else {
        without_voice_instructions(&prompt)
    }
}

/// Spans removed whole: from the start text up to the end text. An end that
/// finishes with a newline is removed with it; any other end is kept.
const SECTIONS: &[(&str, &str)] = &[
    ("<voice_guidelines>", "</voice_guidelines>\n"),
    ("**PREFILL STARTERS**", "**QUALITY GUIDELINES**"),
    ("**CRITICAL TTS RULE**:", "</orchestration_strategy>"),
];

/// Lines that carry a rule about speech in prose, edited rather than dropped so
/// the sentence around them survives.
const REWRITES: &[(&str, &str)] = &[
    ("TEXT + VOICE + COMPONENTS", "TEXT + COMPONENTS"),
    (
        "You have THREE simultaneous output channels",
        "You have several simultaneous output channels",
    ),
    (" through voice interaction", ""),
    (" Comes AFTER TTS.", ""),
    (
        "2. **Text** (markdown outside tags)",
        "1. **Text** (markdown outside tags)",
    ),
    (
        "3. **Components** (JSX/React)",
        "2. **Components** (JSX/React)",
    ),
    (
        "4. **Rationale** (`<Why>` tags)",
        "3. **Rationale** (`<Why>` tags)",
    ),
    (
        "finish with only `<TTS>` + `<NowPlayingCard>`",
        "finish with only the `<NowPlayingCard>`",
    ),
    (
        "proper tool usage, TTS formatting, and thinking patterns",
        "proper tool usage and thinking patterns",
    ),
    (
        "Speak first: the opening `<TTS>` line always comes BEFORE `<thinking>`, so the person hears you while you plan. Then use",
        "Use",
    ),
    ("WITHOUT duplicate TTS.", "without repeating it."),
    (
        "Provide appropriate TTS feedback",
        "Provide an appropriate response",
    ),
];

/// A line that begins with this is replaced by the text beside it.
const RESPONSE_ORDER: (&str, &str) = (
    "**⚡ RESPONSE ORDER**",
    "**⚡ RESPONSE ORDER**: (tool calls) → Text → Components → `<Why>` last. Rationale waits until asked for.",
);

/// Lines dropped because they are about speaking and nothing else.
const DROPPED_LINES: &[&str] = &[
    "Voice-first",
    "voice-first",
    "spoken-first",
    "Voice and text should",
    "users hear, don't read",
];

/// Take the speech instructions out of a system prompt.
///
/// The speech format section goes whole. Examples that were a `<TTS>` line
/// become the plain reply they stand for, so the examples still show what a
/// good answer looks like; any other line that mentions `TTS` is a rule about
/// speaking and is dropped. Idempotent, so a prompt that already went through
/// it comes out the same.
pub fn without_voice_instructions(prompt: &str) -> String {
    let mut text = prompt.replace(PromptFragments::tts_speech_format(), "");

    for (start, end) in SECTIONS {
        let Some(from) = text.find(start) else {
            continue;
        };
        let Some(found) = text[from..].find(end) else {
            continue;
        };
        let at = from + found;
        let to = if end.ends_with('\n') {
            at + end.len()
        } else {
            at
        };
        text.replace_range(from..to, "");
    }

    for (from, to) in REWRITES {
        text = text.replace(from, to);
    }

    let mut out: Vec<String> = Vec::new();
    let mut in_table = false;
    let mut drop_voice_column = false;
    for line in text.split('\n') {
        let trimmed = line.trim();
        if trimmed.starts_with('|') {
            if !in_table {
                in_table = true;
                drop_voice_column = trimmed
                    .trim_matches('|')
                    .split('|')
                    .any(|cell| cell.trim() == "Voice");
            }
            if drop_voice_column {
                let mut cells: Vec<&str> = line.split('|').collect();
                if cells.len() > 4 {
                    cells.remove(3);
                }
                out.push(cells.join("|"));
            } else {
                out.push(line.to_string());
            }
            continue;
        }
        in_table = false;

        if line.starts_with(RESPONSE_ORDER.0) {
            out.push(RESPONSE_ORDER.1.to_string());
            continue;
        }
        if DROPPED_LINES.iter().any(|marker| line.contains(marker)) {
            continue;
        }
        if line.contains("TTS") {
            if let Some(plain) = unwrap_spoken_example(line) {
                out.push(plain);
            }
            continue;
        }
        out.push(line.to_string());
    }
    out.join("\n")
}

/// `Good response: <TTS>Sure.</TTS>` becomes `Good response: Sure.`; any line
/// that is not just one spoken example (a rule, a heading) yields `None`.
fn unwrap_spoken_example(line: &str) -> Option<String> {
    let open = line.find("<TTS>")?;
    let close = line.rfind("</TTS>")?;
    if close < open {
        return None;
    }
    let before = &line[..open];
    let after = &line[close + "</TTS>".len()..];
    let is_label = before.trim().is_empty() || before.trim_end().ends_with(':');
    if !after.trim().is_empty() || !is_label {
        return None;
    }
    Some(format!("{}{}", before, &line[open + "<TTS>".len()..close]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::prompts::{DefaultPrompts, PromptManager, PromptType};

    /// Words that only a speech instruction contains. "hear" and "speak" are
    /// not here on purpose: "what the user needs to hear" is about content.
    const BANNED: &[&str] = &[
        "tts",
        "spoken",
        "aloud",
        "voice",
        "speech",
        "speak first",
        "hears you",
        "hear you",
    ];

    fn every_template() -> Vec<(String, String)> {
        DefaultPrompts::get_all()
            .into_iter()
            .map(|(kind, template)| (format!("{kind:?}"), template.content))
            .collect()
    }

    #[test]
    fn the_default_prompts_ask_for_speech_when_voice_is_on() {
        for (name, content) in every_template() {
            if content.contains("TTS") {
                assert_eq!(
                    apply(content.clone(), true),
                    content,
                    "{name}: voice on must not change the prompt"
                );
            }
        }
        let default = DefaultPrompts::system_default().content;
        assert!(default.contains("<TTS>"), "the default prompt speaks");
        assert!(default.contains("TTS/SPEECH RESPONSE FORMAT"));
    }

    #[test]
    fn no_prompt_mentions_speech_when_voice_is_off() {
        let mut checked = 0;
        for (name, content) in every_template() {
            let stripped = apply(content, false).to_lowercase();
            for word in BANNED {
                assert!(
                    !stripped.contains(word),
                    "{name} still mentions {word:?} with voice off"
                );
            }
            checked += 1;
        }
        assert!(checked > 0, "no templates were checked");
    }

    #[test]
    fn the_prompt_is_much_smaller_without_speech() {
        let on = DefaultPrompts::system_default().content;
        let off = apply(on.clone(), false);
        assert!(off.len() < on.len());
        assert!(!off.contains("<TTS>"));
        // The rest of the prompt is still there.
        assert!(off.contains("You are Juno"));
        assert!(off.contains("TRI-MODAL RESPONSE FORMAT"));
        assert!(off.contains("<NowPlayingCard"));
    }

    #[test]
    fn examples_survive_as_plain_replies() {
        let off = apply(DefaultPrompts::system_default().content, false);
        assert!(off.contains("Good response: It's open. Now what?"));
    }

    #[test]
    fn the_voice_column_leaves_the_channel_table() {
        let off = apply(DefaultPrompts::system_default().content, false);
        assert!(off.contains("| Scenario | Text | Component |"));
        assert!(!off.contains("| Voice |"));
    }

    #[test]
    fn stripping_twice_changes_nothing() {
        for (name, content) in every_template() {
            let once = apply(content, false);
            assert_eq!(apply(once.clone(), false), once, "{name}");
        }
    }

    #[test]
    fn every_rewrite_still_matches_a_prompt() {
        // A rewrite whose text was edited away would let a speech rule through
        // as a dropped line or not at all; this fails when that happens.
        let all: String = every_template()
            .into_iter()
            .map(|(_, content)| content)
            .collect::<Vec<_>>()
            .join("\n");
        for (from, _) in REWRITES {
            assert!(all.contains(from), "no prompt contains {from:?}");
        }
        for (start, end) in SECTIONS {
            assert!(all.contains(start), "no prompt contains {start:?}");
            assert!(all.contains(end.trim_end()), "no prompt contains {end:?}");
        }
        assert!(all.contains(RESPONSE_ORDER.0));
    }

    #[test]
    fn a_manager_built_for_silence_hands_out_prompts_without_speech() {
        let on = PromptManager::new();
        let off = PromptManager::new().with_voice(false);
        let prompt_on = on.get_prompt(PromptType::SystemDefault, None).unwrap();
        let prompt_off = off.get_prompt(PromptType::SystemDefault, None).unwrap();
        assert!(prompt_on.contains("<TTS>"));
        assert!(!prompt_off.to_lowercase().contains("tts"));
        assert!(!off
            .get_default_system_prompt()
            .to_lowercase()
            .contains("tts"));
        assert!(!off
            .get_expert_prompt("browser_expert")
            .to_lowercase()
            .contains("tts"));
        assert!(!off
            .get_orchestrator_personality_prompt()
            .to_lowercase()
            .contains("tts"));
    }

    #[test]
    fn voice_is_off_for_silent_and_on_for_every_engine() {
        for engine in ["system", "kokoro", "ElevenLabs", "replicate", "supertonic"] {
            assert!(voice_output_on(engine), "{engine} speaks");
        }
        assert!(!voice_output_on("off"), "Silent");
        assert!(!voice_output_on("OFF"));
        assert!(!voice_output_on(" off "));
        assert!(!voice_output_on(""), "no engine is no voice");
    }
}
