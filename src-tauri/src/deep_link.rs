//! # `juno://` links
//!
//! A link takes the person somewhere in Juno. It never changes anything.
//! Any web page, message or app can fire a `juno://` URL at this Mac, so the
//! grammar has no way to say "set": [`JunoLink`] has no value in it, and the
//! only code that runs for a link is navigation. A test pins both.
//!
//! ```text
//! juno://settings                           open Settings
//! juno://settings/<pane>                    open it on a pane (voice, ai, security ...)
//! juno://settings/<pane>?highlight=<id>     the pane that owns <id>, scrolled to it and flashed
//! juno://settings?highlight=<id>            the same, without naming the pane
//! ```
//!
//! Pane and setting ids come from [`crate::settings::registry`]. A highlight
//! resolves leniently: an exact setting id, then a setting id's last segment
//! or alias (`voice` is `audio.voice`), then a pane id, otherwise General.
//! An unknown id opens Settings on General, silently. Extra path segments and unknown
//! query keys (`?set=`, `?value=`) are ignored. Case does not matter.
//!
//! A link runs through the same functions the `settings` tool's `open` and
//! `highlight` actions use (`settings_tool::open_pane`, `settings_tool::highlight`),
//! including turning "Show advanced settings" on when the target needs it.
//!
//! ## Where links arrive
//!
//! - Cold launch: macOS starts Juno with the URL. `tauri-plugin-deep-link`
//!   keeps it for [`register`], which handles it once the app has settled.
//! - Running app: the plugin's `on_open_url` callback.
//! - A link clicked in a chat reply: the window forwards the href to the
//!   `open_juno_link` command, so it never reaches the browser.
//!
//! New routes are new [`JunoLink`] variants plus an arm in [`parse`] and [`handle`].

use percent_encoding::{percent_decode_str, utf8_percent_encode, NON_ALPHANUMERIC};
use tauri::AppHandle;
use tracing::{info, warn};

use crate::settings::registry::{resolve_target, Pane, Resolved, SettingKey};

const SCHEME: &str = "juno";

/// Longer than any real link; anything past it is not one.
const MAX_LINK_LEN: usize = 2048;

/// How long a link that launched the app waits, so the bar and the settings
/// store are up before a window is asked for.
const COLD_LAUNCH_SETTLE_MS: u64 = 1500;

/// Where a link goes. Navigation only: there is deliberately no value here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JunoLink {
    Settings(SettingsTarget),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsTarget {
    /// The window, on whatever pane it shows.
    Window,
    /// The window on one pane.
    Pane(Pane),
    /// The pane that owns the setting, scrolled to its row and flashed.
    Setting(SettingKey),
}

/// The link that opens Settings at `pane`, or at the pane that owns `key`
/// with the row highlighted. The inverse of [`parse`].
pub fn settings_link(pane: Option<Pane>, key: Option<SettingKey>) -> String {
    match (key, pane) {
        (Some(key), _) => {
            let spec = key.spec();
            format!(
                "{SCHEME}://settings/{}?highlight={}",
                spec.pane.id(),
                utf8_percent_encode(spec.id, NON_ALPHANUMERIC_KEEP_ID)
            )
        }
        (None, Some(pane)) => format!("{SCHEME}://settings/{}", pane.id()),
        (None, None) => format!("{SCHEME}://settings"),
    }
}

/// Setting ids are `group.name` with dots, dashes and underscores; leave those
/// readable and encode everything else.
const NON_ALPHANUMERIC_KEEP_ID: &percent_encoding::AsciiSet =
    &NON_ALPHANUMERIC.remove(b'.').remove(b'-').remove(b'_');

fn decode(part: &str) -> Option<String> {
    percent_decode_str(part)
        .decode_utf8()
        .ok()
        .map(|s| s.into_owned())
}

/// Read a URL. `None` means it is not a Juno link Juno knows (another scheme,
/// another route), and nothing happens. Never fails loudly.
pub fn parse(url: &str) -> Option<JunoLink> {
    let url = url.trim();
    if url.len() > MAX_LINK_LEN {
        return None;
    }
    let (scheme, rest) = url.split_once(':')?;
    if !scheme.eq_ignore_ascii_case(SCHEME) {
        return None;
    }
    let rest = rest.strip_prefix("//")?;
    let rest = rest.split('#').next().unwrap_or("");
    let (path, query) = rest.split_once('?').unwrap_or((rest, ""));

    let mut segments = path.split('/').filter(|s| !s.is_empty());
    let route = decode(segments.next()?)?.to_ascii_lowercase();
    match route.as_str() {
        "settings" => Some(JunoLink::Settings(parse_settings(segments.next(), query))),
        _ => None,
    }
}

fn parse_settings(pane: Option<&str>, query: &str) -> SettingsTarget {
    let general = SettingsTarget::Pane(Pane::General);

    // Only `highlight` means anything. `set`, `value` and the rest are not read.
    let highlight = query
        .split('&')
        .map(|pair| pair.split_once('=').unwrap_or((pair, "")))
        .find(|(name, _)| decode(name).is_some_and(|n| n.eq_ignore_ascii_case("highlight")))
        .map(|(_, value)| value)
        .filter(|value| !value.is_empty());

    if let Some(value) = highlight {
        // Lenient: an exact id, then an id's last segment or alias ("voice" is
        // `audio.voice`), then a pane, otherwise General. Still navigation only.
        return match decode(value).and_then(|name| resolve_target(&name)) {
            Some(Resolved::Setting(key)) => SettingsTarget::Setting(key),
            Some(Resolved::Pane(pane)) => SettingsTarget::Pane(pane),
            None => general,
        };
    }
    match pane {
        Some(name) => match decode(name).and_then(|n| Pane::from_name(&n)) {
            Some(pane) => SettingsTarget::Pane(pane),
            None => general,
        },
        None => SettingsTarget::Window,
    }
}

/// Go where the link says. Navigation through the settings tool's own paths.
pub async fn handle(app: &AppHandle, link: JunoLink) {
    use crate::agent::tools::settings_tool;
    let JunoLink::Settings(target) = link;
    let outcome = match target {
        SettingsTarget::Window => settings_tool::open_pane(app, None).await,
        SettingsTarget::Pane(pane) => settings_tool::open_pane(app, Some(pane)).await,
        SettingsTarget::Setting(key) => settings_tool::highlight(app, key).await,
    };
    if let Err(e) = outcome {
        // Nothing to tell the person: a link that cannot be followed does nothing.
        warn!("[DeepLink] Could not follow {link:?}: {e}");
    }
}

/// Parse and follow one URL, off the caller's thread.
pub fn handle_url(app: &AppHandle, url: &str, settle_ms: u64) {
    let Some(link) = parse(url) else {
        return;
    };
    info!("[DeepLink] {link:?}");
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        if settle_ms > 0 {
            tokio::time::sleep(std::time::Duration::from_millis(settle_ms)).await;
        }
        handle(&app, link).await;
    });
}

/// Listen for links, and follow the one that launched the app. Call from setup.
pub fn register(app: &AppHandle) {
    use tauri_plugin_deep_link::DeepLinkExt;

    let deep_link = app.deep_link();

    let running = app.clone();
    deep_link.on_open_url(move |event| {
        for url in event.urls() {
            handle_url(&running, url.as_str(), 0);
        }
    });

    // Cold launch: the URL that started the app.
    match deep_link.get_current() {
        Ok(Some(urls)) => {
            for url in urls {
                handle_url(app, url.as_str(), COLD_LAUNCH_SETTLE_MS);
            }
        }
        Ok(None) => {}
        Err(e) => warn!("[DeepLink] Could not read the launch URL: {e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::registry::all_specs;

    fn settings(url: &str) -> SettingsTarget {
        match parse(url) {
            Some(JunoLink::Settings(target)) => target,
            None => panic!("{url} did not parse"),
        }
    }

    const GENERAL: SettingsTarget = SettingsTarget::Pane(Pane::General);

    #[test]
    fn bare_settings_opens_the_window() {
        assert_eq!(settings("juno://settings"), SettingsTarget::Window);
        assert_eq!(settings("juno://settings/"), SettingsTarget::Window);
    }

    #[test]
    fn a_pane_by_sidebar_id() {
        assert_eq!(
            settings("juno://settings/voice"),
            SettingsTarget::Pane(Pane::Audio)
        );
        assert_eq!(
            settings("juno://settings/ai"),
            SettingsTarget::Pane(Pane::Providers)
        );
        assert_eq!(
            settings("juno://settings/advanced"),
            SettingsTarget::Pane(Pane::Advanced)
        );
    }

    #[test]
    fn a_pane_by_name_too() {
        assert_eq!(
            settings("juno://settings/audio"),
            SettingsTarget::Pane(Pane::Audio)
        );
    }

    #[test]
    fn highlight_names_the_pane_that_owns_the_setting() {
        let key = SettingKey::JunoVoice;
        let url = format!("juno://settings?highlight={}", key.spec().id);
        assert_eq!(settings(&url), SettingsTarget::Setting(key));
    }

    #[test]
    fn highlight_voice_lands_on_the_voice_setting_in_the_voice_pane() {
        let target = settings("juno://settings?highlight=voice");
        assert_eq!(target, SettingsTarget::Setting(SettingKey::JunoVoice));
        // The setting lives on the Voice pane (sidebar id "voice") and its row
        // is the one the window flashes.
        let spec = SettingKey::JunoVoice.spec();
        assert_eq!(spec.pane, Pane::Audio);
        assert_eq!(spec.pane.id(), "voice");
        assert_eq!(spec.row, "juno-voice");
    }

    #[test]
    fn highlight_resolves_leniently() {
        // Exact id, last segment, alias, in that order.
        assert_eq!(
            settings("juno://settings?highlight=audio.voice"),
            SettingsTarget::Setting(SettingKey::JunoVoice)
        );
        assert_eq!(
            settings("juno://settings?highlight=Speaking_Speed"),
            SettingsTarget::Setting(SettingKey::SpeakingSpeed)
        );
        assert_eq!(
            settings("juno://settings?highlight=mic"),
            SettingsTarget::Setting(SettingKey::InputDevice)
        );
        // A pane id when no setting answers to it.
        assert_eq!(
            settings("juno://settings?highlight=security"),
            SettingsTarget::Pane(Pane::Security)
        );
        assert_eq!(
            settings("juno://settings?highlight=providers"),
            SettingsTarget::Pane(Pane::Providers)
        );
    }

    #[test]
    fn highlight_wins_over_a_pane_that_disagrees() {
        let key = SettingKey::JunoVoice;
        let url = format!("juno://settings/security?highlight={}", key.spec().id);
        assert_eq!(settings(&url), SettingsTarget::Setting(key));
    }

    #[test]
    fn unknown_ids_open_general() {
        assert_eq!(settings("juno://settings/nope"), GENERAL);
        assert_eq!(settings("juno://settings?highlight=nope.nothing"), GENERAL);
        assert_eq!(settings("juno://settings/voice?highlight=nope"), GENERAL);
        assert_eq!(settings("juno://settings/%FF%FE"), GENERAL);
    }

    #[test]
    fn an_empty_highlight_is_no_highlight() {
        assert_eq!(
            settings("juno://settings/voice?highlight="),
            SettingsTarget::Pane(Pane::Audio)
        );
    }

    #[test]
    fn percent_encoding_is_decoded() {
        let key = SettingKey::JunoVoice;
        let id = key.spec().id.replace('.', "%2E");
        assert_eq!(
            settings(&format!("juno://settings?highlight={id}")),
            SettingsTarget::Setting(key)
        );
        assert_eq!(
            settings("juno://settings/%76oice"),
            SettingsTarget::Pane(Pane::Audio)
        );
        assert_eq!(
            settings("juno://settings/security%20%26%20privacy"),
            SettingsTarget::Pane(Pane::Security)
        );
        assert_eq!(
            settings("juno://%73ettings/voice"),
            SettingsTarget::Pane(Pane::Audio)
        );
    }

    #[test]
    fn extra_path_segments_are_ignored() {
        assert_eq!(
            settings("juno://settings/voice/extra/more"),
            SettingsTarget::Pane(Pane::Audio)
        );
    }

    #[test]
    fn a_fragment_is_ignored() {
        assert_eq!(
            settings("juno://settings/voice#anything"),
            SettingsTarget::Pane(Pane::Audio)
        );
    }

    #[test]
    fn case_does_not_matter() {
        assert_eq!(
            settings("JUNO://Settings/VOICE"),
            SettingsTarget::Pane(Pane::Audio)
        );
        let key = SettingKey::JunoVoice;
        let url = format!(
            "juno://settings?HighLight={}",
            key.spec().id.to_ascii_uppercase()
        );
        assert_eq!(settings(&url), SettingsTarget::Setting(key));
    }

    #[test]
    fn other_schemes_and_routes_do_nothing() {
        assert_eq!(parse("https://settings/voice"), None);
        assert_eq!(parse("juno://elsewhere"), None);
        assert_eq!(parse("juno:settings"), None);
        assert_eq!(parse("juno://"), None);
        assert_eq!(parse(""), None);
        assert_eq!(
            parse(&format!("juno://settings/{}", "a".repeat(3000))),
            None
        );
    }

    /// A link only navigates. `set` and `value` mean nothing in the grammar, so
    /// adding them changes nothing about where a link goes.
    #[test]
    fn a_link_cannot_ask_for_a_change() {
        let key = SettingKey::PlaySounds;
        let id = key.spec().id;
        let plain = format!("juno://settings/voice?highlight={id}");
        for extra in [
            "set=false",
            "value=false",
            "set=audio.play_sounds&value=true",
            "action=set",
            "key=audio.play_sounds&value=1",
        ] {
            assert_eq!(
                parse(&format!("{plain}&{extra}")),
                parse(&plain),
                "{extra} changed what the link does"
            );
            assert_eq!(
                parse(&format!("juno://settings/voice?{extra}")),
                parse("juno://settings/voice"),
                "{extra} changed what the link does"
            );
        }
        // A `set` route does not exist either.
        assert_eq!(parse("juno://set/audio.play_sounds?value=false"), None);
        assert_eq!(
            parse("juno://settings/set?value=false"),
            Some(JunoLink::Settings(GENERAL))
        );
    }

    /// The code that follows a link must never reach the write path: it opens
    /// and highlights through the settings tool and nothing else. Reads this
    /// file, so a call added later to `authorize_set`, the advanced switch or
    /// the tool's `set` fails here.
    #[test]
    fn following_a_link_never_touches_a_write_path() {
        let source = include_str!("deep_link.rs");
        let production = source.split("#[cfg(test)]").next().unwrap_or(source);
        for forbidden in [
            "authorize_set",
            "apply(",
            "set_advanced",
            "settings_tool::set",
            "AuthorizedChange",
            "NewValue",
            "SettingsManager",
        ] {
            assert!(
                !production.contains(forbidden),
                "deep_link.rs must only navigate, but mentions {forbidden}"
            );
        }
        for allowed in ["settings_tool::open_pane", "settings_tool::highlight"] {
            assert!(production.contains(allowed), "{allowed} is the one path");
        }
    }

    #[test]
    fn every_pane_round_trips() {
        for pane in Pane::ALL {
            let link = settings_link(Some(pane), None);
            assert_eq!(
                settings(&link),
                SettingsTarget::Pane(pane),
                "{link} did not come back as {pane:?}"
            );
        }
        assert_eq!(settings(&settings_link(None, None)), SettingsTarget::Window);
    }

    #[test]
    fn every_setting_round_trips() {
        for spec in all_specs() {
            let key = SettingKey::from_id(spec.id).expect("a registry id is a key");
            let link = settings_link(Some(spec.pane), Some(key));
            assert_eq!(
                settings(&link),
                SettingsTarget::Setting(key),
                "{link} did not come back as {}",
                spec.id
            );
            // Without the pane, the setting alone finds its way.
            assert_eq!(
                settings(&settings_link(None, Some(key))),
                SettingsTarget::Setting(key)
            );
        }
    }
}
