//! What a shell command is allowed to do to the screen while the person is
//! using it.
//!
//! # Why this exists
//!
//! Background mode's promise is that the person keeps their hands on the
//! keyboard while Juno works. Every path Juno owns honours it: the pointer is
//! never warped, keystrokes are posted to a process rather than to whatever is
//! frontmost, and `open_application` leaves a running app where it is.
//!
//! The shell is the hole. On the Claude CLI provider the model has Claude
//! Code's own `Bash` tool, which runs outside Juno's tool layer entirely, and
//! `open -a Mail` brings Mail to the front from inside it. Lacy hit this on
//! 2026-10-19 trying to attach two PDFs to a mail message while typing in
//! Notes: "every time it interacted with mail it would take focus away from
//! what i was doing in notes and we would conflict with each other."
//!
//! So a shell command is read before it runs, and judged against the same
//! promise the rest of background mode keeps.
//!
//! # Rewrite where there is a background form, refuse where there is not
//!
//! `open` has `-g`, which does the same work without bringing the app forward,
//! so an `open` is rewritten rather than refused. The person still gets their
//! compose window; it just does not land on top of what they are typing in.
//!
//! AppleScript's `activate` has no such form. Bringing an app to the front is
//! the whole of what it does, so it is refused, with a sentence the model can
//! act on: do the work in the app where it is.
//!
//! Everything else runs untouched. This is a narrow reading of two commands,
//! not a shell policy: `ls`, `grep` and `git` have nothing to do with the
//! screen and are never looked at twice.
//!
//! # What it does not catch
//!
//! - A foregrounding command built at runtime (`xargs open -a Mail`, `eval
//!   "$cmd"`, a shell script that calls `open` inside itself). The text here is
//!   all there is to read.
//! - The Claude CLI with no Juno MCP server to serve the permission prompt
//!   (the headless and test path). There is no hook there, and
//!   [`crate::agent::providers::cli_approval::permission_args`] says so.
//!
//! Both are honest gaps rather than silent ones. Nothing here guesses.

use crate::agent::app_observation;

/// What to do with one shell command while background mode is on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShellPolicy {
    /// Nothing in it touches the front. Run it as written.
    Run,
    /// Run this instead: the same work, without taking the screen. `note` is
    /// the one sentence the model is told, so it does not go looking for the
    /// window it expected to be frontmost.
    Rewritten { command: String, note: String },
    /// Do not run it. The text is written for the model to act on.
    Refused(String),
}

/// Read one shell command the way background mode reads it.
///
/// Pure, so the whole policy is tested without a Mac in the loop.
pub fn policy_for(command: &str) -> ShellPolicy {
    // An `activate` is foregrounding and nothing else, so there is nothing to
    // rewrite it into. Checked first: a command carrying both forms is the
    // more serious of the two.
    if let Some(app) = app_observation::applescript_activation(command) {
        return ShellPolicy::Refused(refusal(&app));
    }

    match rewrite_opens(command) {
        Some(rewritten) => ShellPolicy::Rewritten {
            command: rewritten,
            note: OPENED_IN_BACKGROUND.to_string(),
        },
        None => ShellPolicy::Run,
    }
}

/// What the model is told when an `open` was rewritten.
///
/// It names the consequence (the app did not come forward) rather than the
/// flag, because the next thing the model does is decide whether to tell the
/// person the window is on their screen. It is not.
const OPENED_IN_BACKGROUND: &str = "Background mode: this `open` ran with `-g`, so the app did \
     not come to the front and the person's current app still has their \
     keyboard. Anything that opened is behind what they are working in. Say \
     that, and do not bring it forward.";

/// What the model is told when an activation was refused.
fn refusal(app: &str) -> String {
    format!(
        "Background mode: Juno did not run that, because activating {app} would take the \
         screen from the person while they are typing. Act on {app} where it is: drive it \
         with its own scripting (`tell application \"{app}\" to ...` without `activate`), or \
         use the `computer` tool, which reads and presses controls in windows that are not \
         in front. If the person has to look at {app} themselves, say so and let them \
         switch to it."
    )
}

/// Add `-g` to every `open` in the command that does not already have it.
///
/// `None` when there is nothing to change, so an untouched command is never
/// re-serialized and the common case costs one scan.
fn rewrite_opens(command: &str) -> Option<String> {
    let insertions = opens_that_take_the_front(command);
    if insertions.is_empty() {
        return None;
    }
    let mut out = String::with_capacity(command.len() + insertions.len() * 3);
    let mut last = 0;
    for at in insertions {
        out.push_str(&command[last..at]);
        out.push_str(" -g");
        last = at;
    }
    out.push_str(&command[last..]);
    Some(out)
}

/// Byte offsets just past each `open` that starts a command and has no `-g`.
///
/// Reads the text the way a shell does for the one question it has to answer:
/// which words are in command position, and which are inside quotes. That is
/// enough to leave `echo "open -a Mail"` and `grep open file` alone, which a
/// plain substring search would not.
fn opens_that_take_the_front(command: &str) -> Vec<usize> {
    let bytes = command.as_bytes();
    let mut found = Vec::new();
    let mut i = 0;
    // The next word would be the first word of a command.
    let mut at_command_start = true;

    while i < bytes.len() {
        match bytes[i] {
            b'\'' => {
                // Single quotes have no escapes: everything to the next one is
                // literal. An unterminated quote runs to the end, which is what
                // the shell would do before failing.
                i += 1;
                while i < bytes.len() && bytes[i] != b'\'' {
                    i += 1;
                }
                i += 1;
                at_command_start = false;
            }
            b'"' => {
                i += 1;
                while i < bytes.len() && bytes[i] != b'"' {
                    i += if bytes[i] == b'\\' { 2 } else { 1 };
                }
                i += 1;
                at_command_start = false;
            }
            // Everything that ends one command and starts another.
            b';' | b'&' | b'|' | b'(' | b'{' | b'\n' | b'`' => {
                at_command_start = true;
                i += 1;
            }
            b' ' | b'\t' | b'\r' => i += 1,
            _ => {
                let start = i;
                while i < bytes.len() && !is_word_break(bytes[i]) {
                    i += 1;
                }
                if at_command_start
                    && &command[start..i] == "open"
                    && !already_background(command, i)
                {
                    found.push(i);
                }
                // Only the first word of a command is a command name.
                at_command_start = false;
            }
        }
    }
    found
}

fn is_word_break(b: u8) -> bool {
    matches!(
        b,
        b' ' | b'\t'
            | b'\r'
            | b'\n'
            | b';'
            | b'&'
            | b'|'
            | b'('
            | b')'
            | b'{'
            | b'}'
            | b'`'
            | b'\''
            | b'"'
    )
}

/// Does the `open` whose name ends at `from` already ask for the background?
///
/// Only this command's own arguments are read. The text is cut at the first
/// separator, because reading past it would let the `-g` in `open -a Mail &&
/// open -g -a Music` answer for the first `open` as well, and that one would
/// go on taking the screen.
///
/// A separator inside a quoted filename cuts early, which can only produce a
/// second `-g` on a command that already had one. `open` takes that happily;
/// the opposite mistake is the one that costs the person their screen.
fn already_background(command: &str, from: usize) -> bool {
    let rest = command[from..].split([';', '&', '|', ')', '}', '`']).next();
    for token in rest.unwrap_or_default().split_whitespace() {
        if token == "--background" {
            return true;
        }
        let Some(flags) = token.strip_prefix('-') else {
            continue;
        };
        if flags.starts_with('-') {
            continue;
        }
        for flag in flags.chars() {
            if flag == 'g' {
                return true;
            }
            // `open -aMail` is legal: these flags swallow the rest of the
            // token as their value, so no letter after one is a flag.
            if matches!(flag, 'a' | 'b' | 's' | 'i') {
                break;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rewritten(command: &str) -> Option<String> {
        match policy_for(command) {
            ShellPolicy::Rewritten { command, .. } => Some(command),
            _ => None,
        }
    }

    fn refused(command: &str) -> Option<String> {
        match policy_for(command) {
            ShellPolicy::Refused(reason) => Some(reason),
            _ => None,
        }
    }

    /// The two commands from the transcript, verbatim. Focus theft #1 and #2
    /// on 2026-10-19: both have to come back as background opens.
    #[test]
    fn the_two_commands_that_took_lacys_screen_are_rewritten() {
        assert_eq!(
            rewritten(
                r#"cd ~/Desktop/"Jenny's Folder" && open -a Mail "Headphones receipt.pdf" "Kitchen remodel quote.pdf""#
            )
            .as_deref(),
            Some(
                r#"cd ~/Desktop/"Jenny's Folder" && open -g -a Mail "Headphones receipt.pdf" "Kitchen remodel quote.pdf""#
            )
        );
        assert_eq!(
            rewritten("open -a Mail").as_deref(),
            Some("open -g -a Mail")
        );
    }

    #[test]
    fn every_shape_of_open_loses_the_front() {
        for (before, after) in [
            ("open -a Mail", "open -g -a Mail"),
            ("open -b com.apple.mail", "open -g -b com.apple.mail"),
            ("open ~/Desktop/quote.pdf", "open -g ~/Desktop/quote.pdf"),
            ("open https://example.com", "open -g https://example.com"),
            (
                "open -na '/Applications/Mail.app'",
                "open -g -na '/Applications/Mail.app'",
            ),
            ("open -R ~/Desktop", "open -g -R ~/Desktop"),
            // Every command in a chain is judged on its own.
            (
                "open -a Mail; open -a Notes",
                "open -g -a Mail; open -g -a Notes",
            ),
            (
                "ls && open -a Mail || true",
                "ls && open -g -a Mail || true",
            ),
            (
                "for f in *.pdf\nopen -a Preview $f",
                "for f in *.pdf\nopen -g -a Preview $f",
            ),
        ] {
            assert_eq!(rewritten(before).as_deref(), Some(after), "{before:?}");
        }
    }

    #[test]
    fn an_open_that_already_asked_for_the_background_is_left_alone() {
        for command in [
            "open -g -a Mail",
            "open -ga Mail",
            "open -gj -a Mail",
            "open --background -a Mail",
            "open -g ~/Desktop/quote.pdf",
        ] {
            assert_eq!(policy_for(command), ShellPolicy::Run, "{command:?}");
        }
        // One of two needing it is still a rewrite, and only that one.
        assert_eq!(
            rewritten("open -g -a Mail && open -a Notes").as_deref(),
            Some("open -g -a Mail && open -g -a Notes")
        );
    }

    /// Each `open` is answered on its own arguments. Reading past the
    /// separator let a later command's `-g` excuse an earlier one, so
    /// `open -a Mail` kept the front whenever anything downstream was already
    /// backgrounded. Both orders, so neither direction regresses.
    #[test]
    fn a_later_commands_background_flag_does_not_excuse_an_earlier_open() {
        assert_eq!(
            rewritten("open -a Mail && open -g -a Music").as_deref(),
            Some("open -g -a Mail && open -g -a Music")
        );
        assert_eq!(
            rewritten("open -a Mail; open -g -a Music").as_deref(),
            Some("open -g -a Mail; open -g -a Music")
        );
        // No spaces around the separator is the same command.
        assert_eq!(
            rewritten("open -a Mail&&open -g -a Music").as_deref(),
            Some("open -g -a Mail&&open -g -a Music")
        );
        assert_eq!(
            rewritten("open -a Mail | tee /tmp/log && open -g -a Music").as_deref(),
            Some("open -g -a Mail | tee /tmp/log && open -g -a Music")
        );
    }

    /// `open -aMail` packs the app name into the flag token. The `a` swallows
    /// the rest, so the `g` in "Pages" is a letter in a name, not a flag.
    #[test]
    fn a_value_packed_flag_does_not_fake_a_background_flag() {
        assert_eq!(
            rewritten("open -aPages ~/Desktop/notes.txt").as_deref(),
            Some("open -g -aPages ~/Desktop/notes.txt")
        );
        assert_eq!(
            rewritten("open -bcom.apple.Pages").as_deref(),
            Some("open -g -bcom.apple.Pages")
        );
    }

    /// The word has to be a command, not a mention of one. A substring search
    /// would rewrite all of these and break them.
    #[test]
    fn the_word_open_somewhere_else_is_not_a_command() {
        for command in [
            r#"echo "open -a Mail""#,
            "echo 'open -a Mail'",
            "grep open /etc/hosts",
            "which open",
            "ls ~/open",
            "git commit -m 'open -a Mail broke the demo'",
            "cat opener.sh",
        ] {
            assert_eq!(policy_for(command), ShellPolicy::Run, "{command:?}");
        }
    }

    /// AppleScript activation: no background form, so it does not run, and the
    /// model is told what to do instead rather than just "no".
    #[test]
    fn an_applescript_activate_is_refused_with_somewhere_to_go() {
        let reason = refused(r#"osascript -e 'tell application "Mail" to activate'"#)
            .expect("an activate must not run in background mode");
        assert!(reason.contains("Mail"));
        assert!(reason.contains("where it is"));
        assert!(reason.contains("computer"));
        // The refusal has to be actionable, not a scolding.
        assert!(!reason.contains("error"));

        assert!(refused(r#"osascript -e 'activate application "Slack"'"#).is_some());
        assert!(refused(
            r#"osascript -e 'tell application "Notes"
activate
end tell'"#
        )
        .is_some());
    }

    /// Driving an app that stays where it is was always allowed, and still is.
    /// This is the line the whole module sits on: background mode stops focus
    /// theft, not work.
    #[test]
    fn scripting_an_app_without_activating_it_runs() {
        for command in [
            r#"osascript -e 'tell application "Music" to play'"#,
            r#"osascript -e 'tell application "Mail" to get unread count of inbox'"#,
            "ls /Applications",
            "git status",
            "defaults write com.apple.finder AppleShowAllFiles true",
        ] {
            assert_eq!(policy_for(command), ShellPolicy::Run, "{command:?}");
        }
    }

    /// A command that both opens and activates is the worse of the two.
    #[test]
    fn an_activate_beats_a_rewrite_when_a_command_does_both() {
        assert!(
            refused(r#"open -a Mail && osascript -e 'tell application "Mail" to activate'"#)
                .is_some()
        );
    }

    #[test]
    fn the_note_tells_the_model_the_window_is_not_in_front() {
        let ShellPolicy::Rewritten { note, .. } = policy_for("open -a Mail") else {
            panic!("open -a must be rewritten");
        };
        assert!(note.contains("did not come to the front"));
        assert!(note.contains("do not bring it forward"));
    }
}
