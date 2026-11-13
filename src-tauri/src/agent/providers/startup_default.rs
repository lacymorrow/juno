//! # Applying the default-provider rule at launch
//!
//! [`super::default_selection`] holds the rule. This holds the I/O around it:
//! read what is configured, ask the CLI about itself, write the answer back.
//!
//! It runs on a spawned task and nothing waits for it. Launch must not be held
//! up by a subprocess — `claude auth status` is fast, but "fast" on someone
//! else's machine is not a promise Juno gets to make, and a window that opens
//! late is a worse bug than a provider that settles a moment after it opens.
//!
//! When it does change something it goes through the ordinary settings save,
//! which emits `provider_settings_changed`, so a Settings window already open
//! updates the same way it would if a person had changed it there.

use tracing::{info, warn};

use super::claude_cli;
use super::codex_cli;
use super::config::ProviderConfig;
use super::copilot;
use super::default_selection::{decide, CliCandidate, Decision, SelectionState};
use super::types::Provider;
use crate::settings::manager::SettingsManager;

/// Environment variables that count as credentials for a provider.
///
/// Mirrors what each brain actually falls back to when settings hold no key,
/// so "has a credential" here means the same thing it will mean at query time.
fn env_key_for(provider: &Provider) -> Option<&'static str> {
    match provider {
        Provider::Anthropic => Some("ANTHROPIC_API_KEY"),
        Provider::OpenAI => Some("OPENAI_API_KEY"),
        // Rig borrows OpenAI's key, which `secrets::provider_key` already reflects.
        Provider::Rig => Some("OPENAI_API_KEY"),
        Provider::Gemini => Some("GEMINI_API_KEY"),
        // The CLIs have no key by design; their credential is their login.
        // Copilot's is a GitHub sign-in, which is checked as a candidate below
        // rather than counted as a key here.
        Provider::ClaudeCli | Provider::CodexCli | Provider::Copilot => None,
    }
}

/// Whether `config`'s active provider could actually run right now.
pub(crate) fn active_has_credential(config: &ProviderConfig) -> bool {
    has_credential(&config.active_provider, crate::secrets::provider_key)
}

/// [`active_has_credential`] with the saved-key lookup passed in, so tests
/// can say which keys are saved without a credential store.
fn has_credential(active_provider: &str, saved_key: impl Fn(&Provider) -> Option<String>) -> bool {
    // A demo build carries a compiled-in Anthropic key, so it is never short
    // of credentials and must not be moved off the provider that key is for.
    if crate::demo::is_demo_build() {
        return true;
    }

    let Some(provider) = Provider::from_str(active_provider) else {
        // An unrecognised provider id cannot run, so it has nothing to protect.
        return false;
    };

    saved_key(&provider).is_some_and(|key| !key.is_empty())
        || env_key_for(&provider)
            .and_then(|name| std::env::var(name).ok())
            .is_some_and(|value| !value.is_empty())
}

/// The key providers Juno will move to on the strength of a key alone, in
/// order of preference. Rig is left out: it borrows OpenAI's key, so OpenAI
/// is the provider that key was got for.
const KEY_PROVIDERS: [Provider; 3] = [Provider::Anthropic, Provider::OpenAI, Provider::Gemini];

/// The first key provider, other than the active one, that has a key now.
fn keyed_provider(
    active_provider: &str,
    saved_key: impl Fn(&Provider) -> Option<String>,
) -> Option<&'static str> {
    KEY_PROVIDERS
        .iter()
        .map(Provider::id)
        .find(|id| *id != active_provider && has_credential(id, &saved_key))
}

/// Work out which provider should be active and write it down if it changed.
///
/// Returns the provider id that is active afterwards, which is only used by
/// tests and logging — callers fire this and forget it.
pub async fn apply(settings_manager: &SettingsManager) -> Result<String, String> {
    let mut config = ProviderConfig::load_from_centralized_settings(settings_manager)
        .await
        .map_err(|e| format!("Could not read provider settings: {e}"))?;

    // Cheapest question first: with no CLI binary there is nothing to ask
    // and, in the common case of someone who has installed neither, no
    // subprocess or filesystem read is spawned at all.
    let claude_installed = claude_cli::is_claude_cli_available();
    let claude_status = if claude_installed {
        claude_cli::cli_status().await
    } else {
        claude_cli::CliStatus::default()
    };

    let codex_installed = codex_cli::is_codex_cli_available();
    let codex_status = if codex_installed {
        codex_cli::cli_status().await
    } else {
        codex_cli::CliStatus::default()
    };

    // Copilot is a network round trip to GitHub, not a subprocess. Somebody
    // with a key or a signed-in CLI never needs it decided, but asking anyway
    // keeps Settings honest about it from the first render.
    let copilot_token = crate::secrets::provider_key(&Provider::Copilot);
    let copilot_status = copilot::status(copilot_token.as_deref()).await;
    // GitHub unreachable is not signed out: a provider Juno already chose is
    // kept while its token is still there, rather than abandoned on a blip.
    let copilot_signed_in = copilot_status.signed_in
        || (config.active_provider == Provider::Copilot.id()
            && copilot_status.token_found
            && copilot::last_known_sign_in() == claude_cli::SignIn::Unknown);

    // Preference order lives here: Claude, then Codex, then GitHub Copilot,
    // then (future) trial credits. The rule walks the list first-usable-wins, so this is the one
    // place to touch when a provider is added or re-ordered.
    let cli_candidates = vec![
        CliCandidate {
            provider_id: Provider::ClaudeCli.id(),
            installed: claude_status.installed,
            signed_in: claude_status.is_signed_in(),
        },
        CliCandidate {
            provider_id: Provider::CodexCli.id(),
            installed: codex_status.installed,
            signed_in: codex_status.is_signed_in(),
        },
        CliCandidate {
            provider_id: Provider::Copilot.id(),
            installed: copilot_status.token_found,
            signed_in: copilot_signed_in,
        },
    ];

    let state = SelectionState {
        active_provider: config.active_provider.clone(),
        chosen_by_user: config.provider_chosen_by_user,
        active_has_credential: active_has_credential(&config),
        keyed_provider: keyed_provider(&config.active_provider, crate::secrets::provider_key),
        cli_candidates,
    };

    match decide(&state) {
        Decision::KeepCurrent => Ok(config.active_provider),
        Decision::SwitchTo(provider_id) => {
            // Deliberately not `set_active_provider`: that records the choice
            // as the person's, and this one is Juno's. Leaving the flag alone
            // is what lets the rule walk itself back later.
            if !config.providers.iter().any(|p| p.id == provider_id) {
                warn!("[Providers] Cannot default to unknown provider '{provider_id}'");
                return Ok(config.active_provider);
            }
            config.active_provider = provider_id.to_string();

            config
                .save_to_centralized_settings(settings_manager)
                .await
                .map_err(|e| format!("Could not save the provider choice: {e}"))?;

            match provider_id {
                id if id == Provider::ClaudeCli.id() => match claude_status.email {
                    Some(email) => info!(
                        "[Providers] No API key configured and the Claude CLI is signed in as {email} — using it"
                    ),
                    None => info!("[Providers] Active provider is now '{provider_id}'"),
                },
                id if id == Provider::CodexCli.id() => match codex_status.email {
                    Some(email) => info!(
                        "[Providers] No API key configured and the Codex CLI is signed in with ChatGPT as {email}, using it"
                    ),
                    None => info!("[Providers] Active provider is now '{provider_id}'"),
                },
                id if id == Provider::Copilot.id() => match copilot_status.login {
                    Some(login) => info!(
                        "[Providers] No API key configured and GitHub Copilot is signed in as {login}, using it"
                    ),
                    None => info!("[Providers] Active provider is now '{provider_id}'"),
                },
                _ => info!("[Providers] Active provider is now '{provider_id}'"),
            }
            Ok(provider_id.to_string())
        }
    }
}

/// [`apply`], for a caller that has an `AppHandle` rather than a manager.
///
/// Awaited by startup state initialisation, which is itself already spawned,
/// so nothing on screen waits for the subprocess this runs.
pub async fn apply_for_app(app_handle: tauri::AppHandle) -> Result<String, String> {
    let settings_manager =
        SettingsManager::new(app_handle).map_err(|e| format!("Settings unavailable: {e}"))?;
    apply(&settings_manager).await
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A saved-key lookup that answers `key` for `saved_for` and nothing else.
    fn saved(saved_for: Provider, key: &str) -> impl Fn(&Provider) -> Option<String> {
        let key = key.to_string();
        move |p| (*p == saved_for).then(|| key.clone())
    }

    fn nothing_saved(_: &Provider) -> Option<String> {
        None
    }

    /// True when the developer running the tests has this key exported, in
    /// which case the "no credential" assertions cannot mean anything.
    fn env_key_is_set(name: &str) -> bool {
        std::env::var(name).is_ok_and(|v| !v.is_empty())
    }

    #[test]
    fn a_key_for_another_provider_is_found() {
        if env_key_is_set("ANTHROPIC_API_KEY") || env_key_is_set("GEMINI_API_KEY") {
            return;
        }
        assert_eq!(
            keyed_provider("anthropic", saved(Provider::OpenAI, "sk-example")),
            Some("openai")
        );
        // The active provider's own key is not "another provider's".
        if !env_key_is_set("OPENAI_API_KEY") {
            assert_eq!(
                keyed_provider("openai", saved(Provider::OpenAI, "sk-example")),
                None
            );
            assert_eq!(keyed_provider("anthropic", nothing_saved), None);
        }
    }

    #[test]
    fn a_saved_key_counts_as_a_credential() {
        assert!(has_credential(
            "anthropic",
            saved(Provider::Anthropic, "sk-ant-example")
        ));
        // Rig asks for its own key; `secrets::provider_key` answers with
        // OpenAI's when Rig has none, and that answer counts.
        assert!(has_credential(
            "rig",
            saved(Provider::Rig, "sk-proj-example")
        ));
    }

    #[test]
    fn an_empty_key_does_not_count() {
        if env_key_is_set("ANTHROPIC_API_KEY") || crate::demo::is_demo_build() {
            return;
        }
        assert!(!has_credential("anthropic", saved(Provider::Anthropic, "")));
    }

    #[test]
    fn no_key_anywhere_is_no_credential() {
        if env_key_is_set("ANTHROPIC_API_KEY") || crate::demo::is_demo_build() {
            return;
        }
        assert!(!has_credential("anthropic", nothing_saved));
        // Another provider's key is not this one's.
        assert!(!has_credential(
            "anthropic",
            saved(Provider::Gemini, "gemini-key")
        ));
    }

    #[test]
    fn an_unknown_provider_id_has_nothing_worth_protecting() {
        if crate::demo::is_demo_build() {
            return;
        }
        assert!(!has_credential("not-a-provider", nothing_saved));
    }

    #[test]
    fn the_cli_entries_never_look_for_a_key() {
        // Their credential is the login, so an absent key must not read as
        // "unusable" and bounce someone straight back off it.
        assert_eq!(env_key_for(&Provider::ClaudeCli), None);
        assert_eq!(env_key_for(&Provider::CodexCli), None);
        assert_eq!(env_key_for(&Provider::Copilot), None);
    }

    #[test]
    fn every_provider_the_rule_can_name_has_a_settings_entry() {
        // `apply` refuses to switch to a provider with no entry, so a rule
        // that named a missing one would silently do nothing.
        let config = ProviderConfig::default();
        let ids: Vec<&str> = config.providers.iter().map(|p| p.id.as_str()).collect();
        assert!(ids.contains(&Provider::ClaudeCli.id()));
        assert!(ids.contains(&Provider::CodexCli.id()));
        assert!(ids.contains(&Provider::Copilot.id()));
        assert!(ids.contains(&super::super::config::DEFAULT_PROVIDER.id()));
    }
}
