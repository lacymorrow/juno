//! # What "Reset all settings" means
//!
//! Two things, and the second one is the one that was missing.
//!
//! First, the compiled-in defaults are written over the settings file. Those
//! defaults are the same on every machine, because they are baked into the
//! binary, so they cannot know what this particular machine has.
//!
//! Second, and this is the part a factory reset owes the person: **nothing a
//! reset selects may be something that is not available here.** A reset is
//! supposed to return Juno to a working state. Returning it to a state that
//! names an AI provider with no credentials is worse than leaving it alone,
//! because the person now has a Juno that cannot answer and no setting of
//! theirs left to blame.
//!
//! The provider case is the one that makes the app unusable rather than merely
//! odd, and the rule for it already exists:
//! [`crate::agent::providers::default_selection`] decides which provider
//! should run given what is installed, and
//! [`crate::agent::providers::startup_default`] does the detection around it.
//! Startup has always run that. Reset did not, so a reset landed on the
//! compiled-in default provider (Anthropic) with the API key it needs freshly
//! deleted, and stayed there until the next launch.
//!
//! So reset runs the same rule startup runs. It does not re-implement it, and
//! it does not keep its own idea of which provider is a good default: there is
//! one owner of that question and this is a caller of it.
//!
//! ## Why the flag matters
//!
//! The rule only helps while
//! [`crate::settings::ProviderSettings::provider_chosen_by_user`] is false,
//! because a choice a person made is theirs to keep. A factory reset un-makes
//! every choice they ever made, so that flag has to come back false, and
//! [`crate::settings::ProviderSettings::default`] sets it so. If it ever
//! stopped doing that, reset would clear the credentials and the rule would
//! then politely decline to help, which is the exact shape of a Juno that
//! resets itself into a dead end.
//! `defaults_leave_the_provider_choice_unmade` below holds that down.

//!
//! ## The third thing: the running app
//!
//! Writing the file is not the same as resetting the app. Several things read
//! a setting once, at launch, and keep their own copy: the registered
//! shortcuts and trigger list, the tool configuration, the bar's config, the
//! Dock and menu-bar presence, the login item, the cursor follower. A reset
//! that only wrote the file changed none of them, so the old shortcuts kept
//! working, the Settings window kept showing what was just reset, and the next
//! launch silently changed everything at once. [`refresh_running_app`] hands
//! each of them the reset values, the same way startup does.

use tauri::{AppHandle, Emitter, Manager};
use tracing::warn;

use crate::agent::providers::{config as provider_config, startup_default};
use crate::settings::manager::SettingsManager;
use crate::settings::AppSettings;

/// Reset every setting to a default that works on this machine.
///
/// Returns the provider id that is active afterwards.
pub async fn reset_to_available_defaults(
    settings_manager: &SettingsManager,
) -> Result<String, String> {
    let active = apply_reset(settings_manager, &AppSettings::default()).await?;

    // A few switches live in the store beside the sections, not in them, so
    // the write above did not touch them. A reset that left "Ask before Juno
    // sends" off, for one, is not a reset.
    if let Err(e) = settings_manager.clear_standalone_flags().await {
        warn!("[Settings] Reset could not clear the standalone flags: {e}");
    }

    Ok(active)
}

/// What the running app was showing before a reset, for the parts that only
/// need touching when the reset changes them.
pub struct Before {
    dock_icon_visible: bool,
}

impl Before {
    /// Read it. Call before the reset writes anything.
    pub async fn capture(settings_manager: &SettingsManager) -> Self {
        let agent = settings_manager
            .get_agent_settings()
            .await
            .unwrap_or_default();
        Self {
            dock_icon_visible: agent.dock_icon_visible,
        }
    }
}

/// Make the running app agree with the settings that were just reset.
///
/// Best effort, piece by piece: one that cannot be applied is logged and the
/// rest still are, because the file is already reset and a half-applied reset
/// is better than one that stopped at the first problem.
pub async fn refresh_running_app(app: &AppHandle, before: &Before) {
    use crate::constants::settings::events;

    let manager = match SettingsManager::new(app.clone()) {
        Ok(manager) => manager,
        Err(e) => {
            warn!("[Settings] Reset could not open settings to refresh the app: {e}");
            return;
        }
    };
    let all = match manager.get_all_settings().await {
        Ok(all) => all,
        Err(e) => {
            warn!("[Settings] Reset could not read settings to refresh the app: {e}");
            return;
        }
    };

    // Where Juno shows up. The Dock policy is only touched when it changes: it
    // deactivates the app, and AppKit orders windows out when that happens.
    if all.agent.dock_icon_visible != before.dock_icon_visible {
        crate::commands::dock_icon::apply_dock_icon_policy(app, all.agent.dock_icon_visible);
    }
    crate::menu::tray_menu::set_tray_icon_visible_now(all.agent.show_tray_icon).await;

    #[cfg(target_os = "macos")]
    crate::platform::cursor_follow::set_enabled(all.floating_bar.follow_cursor_display);

    // The login item is the system's, not Juno's, so the store was not the
    // only place it was kept.
    {
        use tauri_plugin_autostart::ManagerExt;
        let launcher = app.autolaunch();
        match launcher.is_enabled() {
            Ok(on) if on != all.autostart_enabled => {
                let changed = if all.autostart_enabled {
                    launcher.enable()
                } else {
                    launcher.disable()
                };
                if let Err(e) = changed {
                    warn!("[Settings] Reset could not change the login item: {e}");
                }
            }
            Ok(_) => {}
            Err(e) => warn!("[Settings] Reset could not read the login item: {e}"),
        }
    }

    if let Some(state) = app.try_state::<crate::state::AppState>() {
        // The cues read the mute switch from memory.
        let _ = state.set_sound_enabled(all.audio.sound_enabled);

        if let Err(e) =
            crate::agent::tools::tool_config::load_tool_config_from_centralized_settings(
                &manager, &state,
            )
            .await
        {
            warn!("[Settings] Reset could not reload the tool configuration: {e}");
        }
    }

    // The bar draws from its own copy of the config.
    crate::commands::ui_commands::reload_bar_config(app).await;

    // Shortcuts, trigger list and wake phrases.
    crate::commands::triggers::reload_from_store(app).await;

    // Open windows follow. The switches say what they now are, and the
    // Settings window redraws every pane from Rust, as it does when the agent
    // changes a setting.
    if let Err(e) = app.emit(
        events::ADVANCED_SETTINGS_CHANGED,
        all.advanced_settings_enabled,
    ) {
        warn!("[Settings] Reset could not announce the advanced switch: {e}");
    }
    if let Err(e) = app.emit(events::BETA_FEATURES_CHANGED, all.beta_features_enabled) {
        warn!("[Settings] Reset could not announce the beta switch: {e}");
    }
    let redraw = crate::agent::tools::settings_tool::Navigation {
        pane: None,
        row: None,
        reload: true,
    };
    if let Err(e) = app.emit(events::SETTINGS_NAVIGATE, &redraw) {
        warn!("[Settings] Reset could not reach the Settings window: {e}");
    }
}

/// Write `settings` as a reset, then make sure what they select is available.
///
/// Takes the settings rather than building them, because the headless
/// `config reset --section` writes an `AppSettings` that is default in one
/// section and current in the rest. That is still a reset and owes the person
/// the same guarantee, so it comes through here instead of calling
/// `save_all_settings` for itself, which is how the UI reset and the CLI reset
/// came to disagree about what a reset means.
///
/// Returns the provider id that is active afterwards.
pub async fn apply_reset(
    settings_manager: &SettingsManager,
    settings: &AppSettings,
) -> Result<String, String> {
    settings_manager.save_all_settings(settings).await?;

    // The write above went straight to the store, so the cached provider
    // configuration still holds the settings that were just thrown away,
    // including the API key whose absence is the whole input to the rule
    // below. Ask the rule about the machine as it is now.
    provider_config::invalidate_config_cache();

    match startup_default::apply(settings_manager).await {
        Ok(active) => Ok(active),
        Err(e) => {
            // The reset itself happened and the settings are written. Failing
            // now would tell the person their reset did not work when it did.
            // Say so in the log and hand back what was written.
            warn!("[Settings] Reset could not reconcile the provider choice: {e}");
            Ok(settings.providers.active_provider.clone())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::providers::default_selection::{
        decide, CliCandidate, Decision, SelectionState,
    };
    use crate::agent::providers::types::Provider;

    /// What the rule is handed immediately after a reset, read out of the real
    /// defaults rather than restated here, so that a change to the defaults
    /// shows up as a failure in these tests instead of passing quietly.
    ///
    /// This mirrors the construction in [`startup_default::apply`], which is
    /// the production path; everything except the per-CLI booleans comes from
    /// `AppSettings::default()`.
    fn state_after_reset(cli_installed: bool, cli_signed_in: bool) -> SelectionState {
        let providers = AppSettings::default().providers;
        let active_has_credential = providers
            .providers
            .iter()
            .find(|p| p.id == providers.active_provider)
            .and_then(|p| p.api_key.as_deref())
            .is_some_and(|key| !key.is_empty());

        // These tests predate Codex and read Claude as "the CLI". The second
        // candidate stays absent so the rule's answer is unchanged in cases
        // where Claude alone is signed in.
        SelectionState {
            active_provider: providers.active_provider,
            chosen_by_user: providers.provider_chosen_by_user,
            active_has_credential,
            cli_candidates: vec![
                CliCandidate {
                    provider_id: Provider::ClaudeCli.id(),
                    installed: cli_installed,
                    signed_in: cli_signed_in,
                },
                CliCandidate {
                    provider_id: Provider::CodexCli.id(),
                    installed: false,
                    signed_in: false,
                },
            ],
        }
    }

    #[test]
    fn a_reset_with_no_api_key_and_a_signed_in_cli_lands_on_the_cli() {
        // Lacy's machine: no Anthropic key, Claude CLI signed in. Reset used to
        // leave him on Anthropic, which cannot answer.
        assert_eq!(
            decide(&state_after_reset(true, true)),
            Decision::SwitchTo(Provider::ClaudeCli.id())
        );
    }

    #[test]
    fn defaults_leave_the_provider_choice_unmade() {
        // A reset un-makes every choice the person made, so the rule is
        // allowed to choose for them again. With this true, reset would delete
        // the credentials and then decline to fix the provider.
        assert!(!AppSettings::default().providers.provider_chosen_by_user);
    }

    #[test]
    fn a_reset_with_no_cli_leaves_the_ordinary_default_in_place() {
        // Nothing better is available, so there is nothing to switch to and
        // the person gets "add an API key", which they can act on.
        assert_eq!(
            decide(&state_after_reset(false, false)),
            Decision::KeepCurrent
        );
    }

    #[test]
    fn a_reset_does_not_move_anyone_onto_a_logged_out_cli() {
        // Installed is not enough. "Run `claude login`" is a dead end they
        // never asked to see.
        assert_eq!(
            decide(&state_after_reset(true, false)),
            Decision::KeepCurrent
        );
    }

    #[test]
    fn every_provider_a_reset_writes_is_one_the_app_recognises() {
        // A default that names a provider id nothing can parse would be a
        // value the app rejects at query time, and `startup_default::apply`
        // refuses to switch to a provider with no entry, so a default the
        // rule could name but not find would fail silently.
        let providers = AppSettings::default().providers;

        for entry in &providers.providers {
            assert!(
                Provider::from_str(&entry.id).is_some(),
                "default provider entry '{}' is not a provider the app knows",
                entry.id
            );
        }

        assert!(
            providers
                .providers
                .iter()
                .any(|p| p.id == providers.active_provider),
            "reset selects '{}', which has no entry to configure",
            providers.active_provider
        );
        assert!(
            providers
                .providers
                .iter()
                .any(|p| p.id == Provider::ClaudeCli.id()),
            "the rule can switch a reset onto the Claude CLI, so it needs an entry"
        );
        assert!(
            providers
                .providers
                .iter()
                .any(|p| p.id == Provider::CodexCli.id()),
            "the rule can switch a reset onto the Codex CLI, so it needs an entry"
        );
    }

    #[test]
    fn every_model_a_reset_writes_is_that_provider_s_own_default() {
        // A model id is the other value in this section that makes the app
        // unusable rather than odd: a request naming a model the provider has
        // retired fails on every message. Each provider owns its default
        // model, so the settings defaults must not carry a second opinion.
        for entry in &AppSettings::default().providers.providers {
            let provider = Provider::from_str(&entry.id).expect("checked by the test above");
            assert_eq!(
                entry.model.as_deref(),
                Some(provider.default_model()),
                "default model for '{}' does not match the provider's own default",
                entry.id
            );
        }
    }
}
