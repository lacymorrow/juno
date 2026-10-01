//! "stop" as a submitted query: the Escape key, in words.
//!
//! A person watching Juno do the wrong thing says "stop". Until now that
//! sentence went to the model, which meant it was queued behind the very run
//! it was trying to end, or read as a new instruction. The key they would
//! have pressed does the right thing immediately, so the word does too: a
//! submitted stop runs the same coordinated stop Escape runs, and nothing is
//! sent to the model.
//!
//! # Three rules keep this from stealing sentences
//!
//! 1. **Bare phrases only.** The whole utterance has to be a stop and nothing
//!    else. "stop the music" is a media command, "stop sharing my screen" is
//!    a task; both go where they already went. This follows the pattern the
//!    rest of this module already uses for "stop the timer".
//! 2. **Only while something is running.** With no run in flight, a bare
//!    "stop" is ambiguous and not ours to interpret, so it falls through
//!    unchanged, which today means `media` may pause the music.
//! 3. **Submit path only.** This is reached from `submit_query`, the one
//!    place a query becomes a run. Dictation does not pass through here:
//!    saying "stop" while dictating types the word, which is what Lacy
//!    asked for and what `dictation_delivery_never_consults_the_stop_intent`
//!    pins.
//!
//! Headless runs skip local intents entirely, so this is a UI-session
//! behaviour; a CLI run still stops the way a CLI run stops.

use tauri::{AppHandle, Manager};

use super::utterance;
use crate::state::AppState;

/// Whole utterances that mean "stop what you are doing", and nothing else.
///
/// Compared after [`utterance::normalize`], which lower-cases, strips
/// punctuation and trims courtesy, so "Stop!" and "stop, please" are both
/// "stop" by the time they get here.
const STOP_PHRASES: &[&str] = &[
    "stop",
    "stop it",
    "stop that",
    "cancel",
    "cancel it",
    "cancel that",
    "never mind",
    "nevermind",
    "halt",
    "abort",
];

/// Is this whole utterance a bare stop?
pub fn is_bare_stop(query: &str) -> bool {
    utterance::normalize(query).is_some_and(|u| STOP_PHRASES.contains(&u.as_str()))
}

/// Should this submitted query halt the session instead of reaching the model?
///
/// Pure, so both halves of the rule are tested without a running app.
pub fn halts_the_session(query: &str, session_running: bool) -> bool {
    session_running && is_bare_stop(query)
}

/// Halt the session if this query is a bare stop and something is running.
///
/// Returns `true` when the query was consumed, which tells
/// [`super::try_handle_local_intent`] not to run the agent.
pub(super) async fn try_halt(app_handle: &AppHandle, query: &str) -> bool {
    let running = app_handle
        .try_state::<AppState>()
        .is_some_and(|state| state.is_agent_executing());
    if !halts_the_session(query, running) {
        return false;
    }

    // The same coordinated stop Escape runs, by the same path, so the word
    // and the key cannot drift apart. It tears down TTS, the run, dictation
    // and always-listening, and announces `agent-active = false`, which is
    // what returns every surface from its working state. Nothing is added to
    // the conversation: Escape says nothing either, and the person can see
    // that it stopped.
    let coordinator = crate::commands::stop_coordinator::get_stop_coordinator();
    match coordinator
        .stop_all_operations(app_handle, "submitted_stop")
        .await
    {
        Ok(summary) => {
            log::info!("Submitted stop halted the session: {}", summary);
            true
        }
        Err(e) => {
            // Could not stop, so do not pretend to have. Falling through
            // sends the word to the model, which is where it used to go.
            log::warn!("Submitted stop could not halt the session: {}", e);
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bare_stop_halts_a_running_session() {
        for query in [
            "stop",
            "Stop.",
            "stop it",
            "Stop it!",
            "stop that",
            "cancel",
            "Cancel that",
            "never mind",
            "nevermind",
            "halt",
            "abort",
            "hey Juno, stop please",
        ] {
            assert!(
                halts_the_session(query, true),
                "{query:?} should halt the session"
            );
        }
    }

    #[test]
    fn a_stop_with_an_object_is_content_and_goes_to_the_model() {
        for query in [
            // Media owns this one, exactly as it owns "stop the timer".
            "stop the music",
            "stop the timer",
            "stop playing",
            "stop sharing my screen",
            "stop the build",
            "cancel my 3pm",
            "cancel the order",
            "stop and then open Safari",
            "why did you stop",
            "don't stop",
            "stop stop stop",
        ] {
            assert!(
                !halts_the_session(query, true),
                "{query:?} is content, not a stop signal"
            );
        }
    }

    /// Bare "stop" with nothing running is ambiguous. It may be aimed at the
    /// music, or at nothing at all, and guessing is not ours to do.
    #[test]
    fn a_bare_stop_with_nothing_running_passes_through() {
        for query in ["stop", "cancel", "never mind", "abort"] {
            assert!(is_bare_stop(query));
            assert!(!halts_the_session(query, false));
        }
    }

    /// "stop the music" still reaches the media domain, which is what makes
    /// rule 1 safe rather than merely narrow.
    #[test]
    fn media_still_owns_stop_the_music() {
        use crate::agent::local_intents::{parse_local_intent, LocalIntent};
        assert!(matches!(
            parse_local_intent("stop the music"),
            Some(LocalIntent::Media(_))
        ));
        assert!(matches!(
            parse_local_intent("stop the timer"),
            Some(LocalIntent::Timer(_))
        ));
    }

    /// Lacy was explicit: "stop" spoken during dictation must type the word,
    /// not halt the agent. Dictation delivers text through
    /// `commands::dictation::insert_dictation_text`, which must never consult
    /// this module. Asserted on the source, because the guarantee is "this
    /// code is not called from there" and no runtime check can show that.
    #[test]
    fn dictation_delivery_never_consults_the_stop_intent() {
        let dictation = include_str!("../../commands/dictation.rs");
        for forbidden in ["local_intents", "is_bare_stop", "halts_the_session"] {
            assert!(
                !dictation.contains(forbidden),
                "dictation must not consult the stop intent ({forbidden} found); saying \
                 \"stop\" while dictating types the word"
            );
        }

        // And the stop check has exactly one caller: the local-intent entry
        // point, which only `submit_query` calls.
        let entry = include_str!("mod.rs");
        assert_eq!(
            entry.matches(concat!("stop::", "try_halt")).count(),
            1,
            "the stop check must have exactly one call site, on the submit path"
        );
    }
}
