//! # Which provider runs when nobody has said
//!
//! Someone paying for Claude Max already has everything Juno needs. Asking
//! them for an API key is asking them to buy the same thing twice, and the
//! bill arrives monthly. So when the `claude` binary is installed and signed
//! in, and nothing else is configured, Juno runs on it.
//!
//! The whole rule is one sentence: **Juno picks the Claude CLI when the person
//! has never picked a provider themselves, the provider that would otherwise
//! run has no credentials, and the CLI is installed and signed in.**
//!
//! Each clause is load-bearing:
//!
//! - *never picked themselves* — a choice made in Settings or in setup is a
//!   choice, and Juno does not overrule it. [`crate::settings::ProviderSettings::provider_chosen_by_user`]
//!   records that, and every path a human can pick through sets it.
//! - *no credentials* — someone who pasted an Anthropic key wants that key
//!   used. Free is not better than what they asked for.
//! - *installed and signed in* — an installed-but-logged-out CLI cannot answer,
//!   and switching to it would trade an honest "add a key" for a confusing
//!   "run `claude login`" they never asked to see.
//!
//! The same function walks it back. If Juno made the choice and the CLI later
//! disappears, Juno un-makes it rather than leaving the person on a provider
//! whose binary is gone. If *they* chose it, it stays chosen and they get the
//! real error, because that is their decision to revisit.
//!
//! Nothing here does I/O. Detection happens at the call site and arrives as
//! plain booleans, which is what makes the rule readable and testable.

use super::types::Provider;

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
    /// Whether the `claude` binary exists.
    pub cli_installed: bool,
    /// Whether `claude auth status` says it is signed in.
    pub cli_signed_in: bool,
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
    let on_cli = state.active_provider == Provider::ClaudeCli.id();

    // A choice the person made is theirs to keep, working or not.
    if state.chosen_by_user {
        return Decision::KeepCurrent;
    }

    // Juno put them on the CLI and the CLI is no longer usable. Walk it back to
    // the ordinary default so they land on "add an API key", which they can act
    // on, rather than "claude: not found", which they cannot.
    if on_cli && !(state.cli_installed && state.cli_signed_in) {
        return Decision::SwitchTo(super::config::DEFAULT_PROVIDER.id());
    }

    if on_cli {
        return Decision::KeepCurrent;
    }

    // The case this module exists for.
    if !state.active_has_credential && state.cli_installed && state.cli_signed_in {
        return Decision::SwitchTo(Provider::ClaudeCli.id());
    }

    Decision::KeepCurrent
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fresh install: nothing configured, nothing chosen.
    fn fresh() -> SelectionState {
        SelectionState {
            active_provider: super::super::config::DEFAULT_PROVIDER.id().to_string(),
            chosen_by_user: false,
            active_has_credential: false,
            cli_installed: false,
            cli_signed_in: false,
        }
    }

    #[test]
    fn a_signed_in_cli_is_the_default_when_nothing_else_is_set_up() {
        let state = SelectionState {
            cli_installed: true,
            cli_signed_in: true,
            ..fresh()
        };
        assert_eq!(decide(&state), Decision::SwitchTo("claude_cli"));
    }

    #[test]
    fn an_api_key_the_person_already_has_wins_over_free() {
        // Paying for the API is a decision. Juno does not second-guess it just
        // because a subscription would be cheaper.
        let state = SelectionState {
            active_has_credential: true,
            cli_installed: true,
            cli_signed_in: true,
            ..fresh()
        };
        assert_eq!(decide(&state), Decision::KeepCurrent);
    }

    #[test]
    fn an_explicit_choice_is_never_overruled() {
        let state = SelectionState {
            chosen_by_user: true,
            cli_installed: true,
            cli_signed_in: true,
            ..fresh()
        };
        assert_eq!(decide(&state), Decision::KeepCurrent);
    }

    #[test]
    fn installed_but_logged_out_is_not_good_enough() {
        // "Run `claude login`" is a worse dead end than "add an API key",
        // because they never asked to be on the CLI in the first place.
        let state = SelectionState {
            cli_installed: true,
            cli_signed_in: false,
            ..fresh()
        };
        assert_eq!(decide(&state), Decision::KeepCurrent);
    }

    #[test]
    fn no_cli_at_all_changes_nothing() {
        assert_eq!(decide(&fresh()), Decision::KeepCurrent);
    }

    #[test]
    fn a_cli_juno_chose_is_given_up_when_it_disappears() {
        let state = SelectionState {
            active_provider: "claude_cli".to_string(),
            chosen_by_user: false,
            active_has_credential: false,
            cli_installed: false,
            cli_signed_in: false,
        };
        assert_eq!(
            decide(&state),
            Decision::SwitchTo(super::super::config::DEFAULT_PROVIDER.id())
        );
    }

    #[test]
    fn a_cli_juno_chose_is_given_up_when_it_signs_out() {
        let state = SelectionState {
            active_provider: "claude_cli".to_string(),
            chosen_by_user: false,
            active_has_credential: false,
            cli_installed: true,
            cli_signed_in: false,
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
            cli_installed: false,
            cli_signed_in: false,
        };
        assert_eq!(decide(&state), Decision::KeepCurrent);
    }

    #[test]
    fn a_working_cli_juno_chose_is_left_alone() {
        let state = SelectionState {
            active_provider: "claude_cli".to_string(),
            chosen_by_user: false,
            active_has_credential: false,
            cli_installed: true,
            cli_signed_in: true,
        };
        assert_eq!(decide(&state), Decision::KeepCurrent);
    }

    #[test]
    fn the_decision_is_stable_when_applied_twice() {
        // Startup runs this every launch; a rule that flapped would rewrite
        // settings and fire a settings-changed event on every boot.
        let mut state = SelectionState {
            cli_installed: true,
            cli_signed_in: true,
            ..fresh()
        };
        let Decision::SwitchTo(first) = decide(&state) else {
            panic!("expected a switch on the first run");
        };
        state.active_provider = first.to_string();
        assert_eq!(decide(&state), Decision::KeepCurrent);
    }
}
