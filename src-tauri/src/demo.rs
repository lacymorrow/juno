//! Demo builds: a golden copy that carries its own Anthropic key.
//!
//! A demo build is for handing Juno to someone who should be able to open it
//! and talk to it without signing up for an AI provider first. The key is
//! baked in at compile time from `JUNO_DEMO_ANTHROPIC_KEY`, which is set only
//! by `scripts/tauri-build.sh --demo`. A normal build sets nothing, so
//! `option_env!` resolves to `None` and not a byte of key material is in the
//! binary.
//!
//! What this is not: a secret. Anyone holding the app can read the key out of
//! it with `strings`. The protections that matter live in the Anthropic
//! console, not here: give the demo its own workspace with a spend cap, use a
//! separate key per cohort so a leak can be traced and revoked, and hand the
//! build out through a private link rather than the release page. When the key
//! is revoked the app keeps working: the person is told the demo ended and can
//! add their own key in Settings.

/// The key compiled into this build, when it is a demo build.
pub fn api_key() -> Option<&'static str> {
    option_env!("JUNO_DEMO_ANTHROPIC_KEY").filter(|k| !k.is_empty())
}

/// Which batch this build was made for, e.g. "sept-investors". Shows in
/// Settings so a build in the wild can be traced back to its key.
pub fn cohort() -> Option<&'static str> {
    option_env!("JUNO_DEMO_COHORT").filter(|c| !c.is_empty())
}

pub fn is_demo_build() -> bool {
    api_key().is_some()
}

/// What Settings shows about this build. Serialised to the frontend.
#[derive(Debug, Clone, serde::Serialize, PartialEq, Eq)]
pub struct DemoInfo {
    pub is_demo: bool,
    pub cohort: Option<String>,
}

pub fn info() -> DemoInfo {
    DemoInfo {
        is_demo: is_demo_build(),
        cohort: cohort().map(str::to_string),
    }
}

/// What this build is, for the UI to show. Cheap; reads compile-time strings.
#[tauri::command]
pub fn get_demo_info() -> DemoInfo {
    info()
}

/// Where an Anthropic key comes from, in order. The person's own key always
/// wins: a demo build they later add a key to stops spending the demo budget.
pub fn resolve_api_key(
    from_settings: Option<String>,
    from_env: Option<String>,
    from_demo: Option<&'static str>,
) -> Option<String> {
    from_settings
        .filter(|k| !k.is_empty())
        .or_else(|| from_env.filter(|k| !k.is_empty()))
        .or_else(|| from_demo.map(str::to_string))
}

#[cfg(test)]
mod tests {
    use super::*;

    const DEMO: Option<&'static str> = Some("sk-ant-demo");

    #[test]
    fn the_persons_own_key_beats_the_demo_key() {
        assert_eq!(
            resolve_api_key(Some("mine".into()), None, DEMO),
            Some("mine".to_string())
        );
        assert_eq!(
            resolve_api_key(None, Some("from-env".into()), DEMO),
            Some("from-env".to_string())
        );
    }

    #[test]
    fn the_demo_key_is_the_last_resort() {
        assert_eq!(
            resolve_api_key(None, None, DEMO),
            Some("sk-ant-demo".into())
        );
    }

    #[test]
    fn an_empty_key_is_not_a_key() {
        assert_eq!(
            resolve_api_key(Some(String::new()), None, DEMO),
            Some("sk-ant-demo".into())
        );
        assert_eq!(
            resolve_api_key(Some(String::new()), Some(String::new()), None),
            None
        );
    }

    #[test]
    fn a_normal_build_carries_nothing() {
        // This test runs in the normal build, where the env var is unset.
        assert_eq!(api_key(), None);
        assert!(!is_demo_build());
        assert_eq!(
            info(),
            DemoInfo {
                is_demo: false,
                cohort: None
            }
        );
    }
}
