//! "quit" as an utterance: close Juno itself.
//!
//! One matcher for every input path. Typed bar and chat, push-to-talk and
//! dictation-to-agent all reach [`super::try_handle_local_intent`] through
//! `submit_query`; the always-listening stop path calls [`is_quit_juno`]
//! directly. All of them quit through
//! [`crate::menu::tray_menu::quit_app`], the function the tray menu's Quit
//! row runs, so cleanup cannot drift between the menu and the words.
//!
//! # Rules
//!
//! The whole utterance has to be the command. Courtesy at the edges is
//! tolerated ("hey Juno, quit please"), nothing else is:
//!
//! - bare "quit" or "exit" quits;
//! - a verb that names Juno quits: "quit Juno", "close Juno", "goodbye Juno",
//!   "shut down Juno", "stop Juno";
//! - "quit Safari" is not ours (`apps` quits Safari);
//! - bare "stop" or "cancel" never quits (see `stop`);
//! - a sentence that merely contains the word ("how do I quit vim") never
//!   quits.

use tauri::AppHandle;

/// Words that may open a command without changing it.
const LEADING_COURTESY: &[&str] = &[
    "hey", "hi", "hello", "ok", "okay", "yo", "juno", "please", "can", "could", "would", "will",
    "you", "just", "kindly",
];

/// Words that may close a command without changing it.
const TRAILING_COURTESY: &[&str] = &["please", "thanks", "thank", "you", "now"];

/// Whole commands (after courtesy is trimmed) that quit Juno.
const QUIT_PHRASES: &[&str] = &[
    "quit",
    "exit",
    "quit juno",
    "exit juno",
    "close juno",
    "goodbye juno",
    "bye juno",
    "stop juno",
    "shut down juno",
    "shutdown juno",
];

/// Is this whole utterance a request to quit Juno? Pure.
pub fn is_quit_juno(text: &str) -> bool {
    let lower = text.to_lowercase();
    let mut words: Vec<&str> = lower
        .split(|c: char| !(c.is_alphanumeric() || c == '\''))
        .filter(|w| !w.is_empty())
        .collect();
    while words.first().is_some_and(|w| LEADING_COURTESY.contains(w)) {
        words.remove(0);
    }
    while words.last().is_some_and(|w| TRAILING_COURTESY.contains(w)) {
        words.pop();
    }
    QUIT_PHRASES.contains(&words.join(" ").as_str())
}

/// Quit the app if the query is a quit command. Returns `true` when consumed.
pub(super) fn try_quit(app_handle: &AppHandle, query: &str) -> bool {
    if !is_quit_juno(query) {
        return false;
    }
    log::info!("Quit command received ('{}'), exiting Juno", query);
    crate::menu::tray_menu::quit_app(app_handle);
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quit_phrases_quit() {
        for q in [
            "quit",
            "Quit.",
            "exit",
            "quit Juno",
            "Quit Juno!",
            "close Juno",
            "goodbye Juno",
            "shut down Juno",
            "shutdown juno",
            "stop Juno",
            "hey Juno, quit please",
            "Juno, quit",
            "could you please quit",
        ] {
            assert!(is_quit_juno(q), "{q:?} should quit");
        }
    }

    #[test]
    fn other_apps_do_not_quit_juno() {
        for q in [
            "quit Safari",
            "exit vim",
            "close Safari",
            "quit the Spotify app",
        ] {
            assert!(!is_quit_juno(q), "{q:?} must not quit Juno");
        }
    }

    #[test]
    fn bare_stop_and_cancel_never_quit() {
        for q in [
            "stop",
            "cancel",
            "stop it",
            "never mind",
            "close",
            "goodbye",
            "bye",
        ] {
            assert!(!is_quit_juno(q), "{q:?} must not quit");
        }
    }

    #[test]
    fn sentences_containing_the_word_do_not_quit() {
        for q in [
            "how do I quit vim",
            "quit smoking tips",
            "exit the building",
            "please quit and then open Safari",
            "what does quit do",
            "can you explain how to exit fullscreen",
            "",
            "juno",
        ] {
            assert!(!is_quit_juno(q), "{q:?} must not quit");
        }
    }
}
