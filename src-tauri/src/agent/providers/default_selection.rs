//! # Which provider runs when nobody has said
//!
//! Somebody who already pays for a plan has everything Juno needs on that
//! plan. Asking them for an API key is asking them to buy the same thing
//! twice. So when a CLI is installed and signed in and nothing else is
//! configured, Juno runs on it.
//!
//! The rule is one sentence: **Juno picks a signed-in CLI, in a fixed order
//! of preference, when the person has never picked a provider themselves and
//! the provider that would otherwise run has no credentials.**
//!
//! Each clause is load-bearing:
//!
//! - *never picked themselves* — a choice made in Settings or in setup is a
//!   choice, and Juno does not overrule it.
//!   [`crate::settings::ProviderSettings::provider_chosen_by_user`] records
//!   that, and every path a human can pick through sets it.
//! - *no credentials*: somebody who pasted an Anthropic key wants that key
//!   used. Free is not better than what they asked for.
//! - *installed and signed in*: an installed-but-logged-out CLI cannot
//!   answer, and switching to it would trade an honest "add a key" for a
//!   confusing "run `claude login`" they never asked to see.
//! - *fixed order*: ties go to the first CLI in the list. Today that is
//!   Claude first, Codex second. When one person has both signed in Juno
//!   runs on Claude, because the Claude provider has further-along tool
//!   integration and that is what their experience will depend on.
//!
//! The same function walks it back. If Juno made the choice and the chosen
//! CLI later disappears or signs out, Juno un-makes it rather than leaving
//! the person on a provider whose binary is gone. If *they* chose it, it
//! stays chosen and they get the real error, because that is their decision
//! to revisit.
//!
//! There is a hook here for trial credits once that exists: a third
//! [`CliCandidate`] entry whose `signed_in` is "Juno has credits to spend".
//! Until then the list is just the two CLIs.
//!
//! Nothing in this module does I/O. Detection happens at the call site and
//! arrives as plain booleans, which is what makes the rule readable and
//! testable.

/// One CLI the rule can pick from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliCandidate {
    /// The provider id this candidate stands for ([`Provider::id`]).
    pub provider_id: &'static str,
    /// The binary exists.
    pub installed: bool,
    /// The CLI reports a usable login, proof enough for Juno to switch
    /// somebody onto it who did not ask.
    pub signed_in: bool,
}

impl CliCandidate {
    /// Does this CLI meet the bar for Juno to switch onto it unprompted?
    fn usable(&self) -> bool {
        self.installed && self.signed_in
    }
}

/// What is true at the moment of the decision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectionState {
    /// The provider id currently recorded as active.
    pub active_provider: String,
    /// Whether a human has ever picked a provider on this machine.
    pub chosen_by_user: bool,
    /// Whether the active provider has a key it could actually run with,
    /// from settings or from the environment.
    pub active_has_credential: bool,
    /// Candidate CLIs, in preference order. The first usable one wins when
    /// the rule needs to switch; the first ``cli_id`` match decides whether
    /// the active provider is one of these CLIs.
    pub cli_candidates: Vec<CliCandidate>,
}

impl SelectionState {
    /// The candidate the current active provider is, if any.
    fn active_candidate(&self) -> Option<&CliCandidate> {
        self.cli_candidates
            .iter()
            .find(|c| c.provider_id == self.active_provider)
    }
}

/// What to do about it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    /// Leave the settings alone.
    KeepCurrent,
    /// Switch to this provider id, and record that Juno was the one who chose.
    SwitchTo(&'static str),
}

/// Apply the rule in the module docs.
pub fn decide(state: &SelectionState) -> Decision {
    // A choice the person made is theirs to keep, working or not.
    if state.chosen_by_user {
        return Decision::KeepCurrent;
    }

    // Juno put them on a CLI and the CLI is no longer usable. Walk it back to
    // the ordinary default so they land on "add an API key", which they can
    // act on, rather than a sign-in error they cannot.
    if let Some(active) = state.active_candidate() {
        if !active.usable() {
            return Decision::SwitchTo(super::config::DEFAULT_PROVIDER.id());
        }
        return Decision::KeepCurrent;
    }

    // The case this module exists for: nothing paid for, nothing picked, but
    // some CLI the person already has can answer.
    if !state.active_has_credential {
        if let Some(candidate) = state.cli_candidates.iter().find(|c| c.usable()) {
            return Decision::SwitchTo(candidate.provider_id);
        }
    }

    Decision::KeepCurrent
}

#[cfg(test)]
mod tests {
    use super::super::types::Provider;
    use super::*;

    fn claude_candidate(installed: bool, signed_in: bool) -> CliCandidate {
        CliCandidate {
            provider_id: Provider::ClaudeCli.id(),
            installed,
            signed_in,
        }
    }

    fn codex_candidate(installed: bool, signed_in: bool) -> CliCandidate {
        CliCandidate {
            provider_id: Provider::CodexCli.id(),
            installed,
            signed_in,
        }
    }

    /// A fresh install: nothing configured, nothing chosen, no CLIs usable.
    fn fresh() -> SelectionState {
        SelectionState {
            active_provider: super::super::config::DEFAULT_PROVIDER.id().to_string(),
            chosen_by_user: false,
            active_has_credential: false,
            cli_candidates: vec![
                claude_candidate(false, false),
                codex_candidate(false, false),
            ],
        }
    }

    #[test]
    fn a_signed_in_cli_is_the_default_when_nothing_else_is_set_up() {
        let state = SelectionState {
            cli_candidates: vec![claude_candidate(true, true), codex_candidate(false, false)],
            ..fresh()
        };
        assert_eq!(decide(&state), Decision::SwitchTo("claude_cli"));
    }

    #[test]
    fn claude_wins_a_tie_with_codex() {
        // Both signed in. Claude is first in the list, so Claude is chosen.
        let state = SelectionState {
            cli_candidates: vec![claude_candidate(true, true), codex_candidate(true, true)],
            ..fresh()
        };
        assert_eq!(decide(&state), Decision::SwitchTo("claude_cli"));
    }

    #[test]
    fn codex_wins_when_claude_is_absent() {
        let state = SelectionState {
            cli_candidates: vec![claude_candidate(false, false), codex_candidate(true, true)],
            ..fresh()
        };
        assert_eq!(decide(&state), Decision::SwitchTo("codex_cli"));
    }

    #[test]
    fn an_api_key_the_person_already_has_wins_over_free() {
        // Paying for the API is a decision. Juno does not second-guess it just
        // because a subscription would be cheaper.
        let state = SelectionState {
            active_has_credential: true,
            cli_candidates: vec![claude_candidate(true, true), codex_candidate(true, true)],
            ..fresh()
        };
        assert_eq!(decide(&state), Decision::KeepCurrent);
    }

    #[test]
    fn an_explicit_choice_is_never_overruled() {
        let state = SelectionState {
            chosen_by_user: true,
            cli_candidates: vec![claude_candidate(true, true), codex_candidate(true, true)],
            ..fresh()
        };
        assert_eq!(decide(&state), Decision::KeepCurrent);
    }

    #[test]
    fn installed_but_logged_out_is_not_good_enough() {
        let state = SelectionState {
            cli_candidates: vec![claude_candidate(true, false), codex_candidate(true, false)],
            ..fresh()
        };
        assert_eq!(decide(&state), Decision::KeepCurrent);
    }

    #[test]
    fn no_cli_at_all_changes_nothing() {
        assert_eq!(decide(&fresh()), Decision::KeepCurrent);
    }

    #[test]
    fn a_claude_cli_juno_chose_is_given_up_when_it_disappears() {
        let state = SelectionState {
            active_provider: "claude_cli".to_string(),
            chosen_by_user: false,
            active_has_credential: false,
            cli_candidates: vec![
                claude_candidate(false, false),
                codex_candidate(false, false),
            ],
        };
        assert_eq!(
            decide(&state),
            Decision::SwitchTo(super::super::config::DEFAULT_PROVIDER.id())
        );
    }

    #[test]
    fn a_codex_cli_juno_chose_is_given_up_when_it_signs_out() {
        let state = SelectionState {
            active_provider: "codex_cli".to_string(),
            chosen_by_user: false,
            active_has_credential: false,
            cli_candidates: vec![claude_candidate(false, false), codex_candidate(true, false)],
        };
        assert_eq!(
            decide(&state),
            Decision::SwitchTo(super::super::config::DEFAULT_PROVIDER.id())
        );
    }

    #[test]
    fn a_cli_the_person_chose_is_theirs_even_when_it_breaks() {
        // They picked it, so they get the real error and can fix or change it.
        let state = SelectionState {
            active_provider: "claude_cli".to_string(),
            chosen_by_user: true,
            active_has_credential: false,
            cli_candidates: vec![
                claude_candidate(false, false),
                codex_candidate(false, false),
            ],
        };
        assert_eq!(decide(&state), Decision::KeepCurrent);
    }

    #[test]
    fn a_working_cli_juno_chose_is_left_alone() {
        let state = SelectionState {
            active_provider: "codex_cli".to_string(),
            chosen_by_user: false,
            active_has_credential: false,
            cli_candidates: vec![claude_candidate(false, false), codex_candidate(true, true)],
        };
        assert_eq!(decide(&state), Decision::KeepCurrent);
    }

    #[test]
    fn the_decision_is_stable_when_applied_twice() {
        // Startup runs this every launch; a rule that flapped would rewrite
        // settings and fire a settings-changed event on every boot.
        let mut state = SelectionState {
            cli_candidates: vec![claude_candidate(true, true), codex_candidate(false, false)],
            ..fresh()
        };
        let Decision::SwitchTo(first) = decide(&state) else {
            panic!("expected a switch on the first run");
        };
        state.active_provider = first.to_string();
        assert_eq!(decide(&state), Decision::KeepCurrent);
    }
}
