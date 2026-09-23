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
use super::config::ProviderConfig;
use super::default_selection::{decide, Decision, SelectionState};
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
        // Rig borrows OpenAI's key, which `resolve_provider` already reflects.
        Provider::Rig => Some("OPENAI_API_KEY"),
        Provider::Gemini => Some("GEMINI_API_KEY"),
        // The CLI has no key by design; its credential is its login.
        Provider::ClaudeCli => None,
    }
}

/// Whether `config`'s active provider could actually run right now.
fn active_has_credential(config: &ProviderConfig) -> bool {
    // A demo build carries a compiled-in Anthropic key, so it is never short
    // of credentials and must not be moved off the provider that key is for.
    if crate::demo::is_demo_build() {
        return true;
    }

    let Some(provider) = Provider::from_str(&config.active_provider) else {
        // An unrecognised provider id cannot run, so it has nothing to protect.
        return false;
    };

    let from_settings = config
        .resolve_provider(provider.clone())
        .and_then(|p| p.api_key)
        .is_some_and(|key| !key.is_empty());

    from_settings
        || env_key_for(&provider)
            .and_then(|name| std::env::var(name).ok())
            .is_some_and(|value| !value.is_empty())
}

/// Work out which provider should be active and write it down if it changed.
///
/// Returns the provider id that is active afterwards, which is only used by
/// tests and logging — callers fire this and forget it.
pub async fn apply(settings_manager: &SettingsManager) -> Result<String, String> {
    let mut config = ProviderConfig::load_from_centralized_settings(settings_manager)
        .await
        .map_err(|e| format!("Could not read provider settings: {e}"))?;

    // Cheapest question first: with no `claude` binary there is nothing to ask
    // and, in the common case of someone who has never installed Claude Code,
    // no subprocess is spawned at all.
    let installed = claude_cli::is_claude_cli_available();
    let status = if installed {
        claude_cli::cli_status().await
    } else {
        claude_cli::CliStatus::default()
    };

    let state = SelectionState {
        active_provider: config.active_provider.clone(),
        chosen_by_user: config.provider_chosen_by_user,
        active_has_credential: active_has_credential(&config),
        cli_installed: status.installed,
        // Proof, not the benefit of the doubt: this decision moves
        // somebody onto a provider they never asked for.
        cli_signed_in: status.is_signed_in(),
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

            match status.email {
                Some(email) if provider_id == Provider::ClaudeCli.id() => info!(
                    "[Providers] No API key configured and the Claude CLI is signed in as {email} — using it"
                ),
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

    /// Default settings with `active` selected, optionally carrying a key.
    fn config_with(active: &str, api_key: Option<&str>) -> ProviderConfig {
        let mut config = ProviderConfig {
            active_provider: active.to_string(),
            ..ProviderConfig::default()
        };
        if let Some(key) = api_key {
            if let Some(entry) = config.providers.iter_mut().find(|p| p.id == active) {
                entry.api_key = Some(key.to_string());
            }
        }
        config
    }

    /// True when the developer running the tests has this key exported, in
    /// which case the "no credential" assertions cannot mean anything.
    fn env_key_is_set(name: &str) -> bool {
        std::env::var(name).is_ok_and(|v| !v.is_empty())
    }

    #[test]
    fn a_saved_key_counts_as_a_credential() {
        assert!(active_has_credential(&config_with(
            "anthropic",
            Some("sk-ant-example")
        )));
    }

    #[test]
    fn an_empty_key_does_not_count() {
        // An empty string is what a cleared field leaves behind. Reading it as
        // a credential would keep someone on a provider that cannot run.
        if env_key_is_set("ANTHROPIC_API_KEY") || crate::demo::is_demo_build() {
            return;
        }
        assert!(!active_has_credential(&config_with("anthropic", Some(""))));
    }

    #[test]
    fn no_key_anywhere_is_no_credential() {
        if env_key_is_set("ANTHROPIC_API_KEY") || crate::demo::is_demo_build() {
            return;
        }
        assert!(!active_has_credential(&config_with("anthropic", None)));
    }

    #[test]
    fn rig_borrows_the_openai_key() {
        // `resolve_provider` already does this at query time. The rule has to
        // agree, or Juno would move someone off a Rig setup that works.
        let mut config = config_with("rig", None);
        if let Some(entry) = config.providers.iter_mut().find(|p| p.id == "openai") {
            entry.api_key = Some("sk-proj-example".to_string());
        }
        assert!(active_has_credential(&config));
    }

    #[test]
    fn an_unknown_provider_id_has_nothing_worth_protecting() {
        if crate::demo::is_demo_build() {
            return;
        }
        assert!(!active_has_credential(&config_with("not-a-provider", None)));
    }

    #[test]
    fn the_cli_entry_never_looks_for_a_key() {
        // Its credential is the login, so an absent key must not read as
        // "unusable" and bounce someone straight back off it.
        assert_eq!(env_key_for(&Provider::ClaudeCli), None);
    }

    #[test]
    fn both_providers_the_rule_can_name_have_settings_entries() {
        // `apply` refuses to switch to a provider with no entry, so a rule
        // that named a missing one would silently do nothing.
        let config = ProviderConfig::default();
        let ids: Vec<&str> = config.providers.iter().map(|p| p.id.as_str()).collect();
        assert!(ids.contains(&Provider::ClaudeCli.id()));
        assert!(ids.contains(&super::super::config::DEFAULT_PROVIDER.id()));
    }
}
