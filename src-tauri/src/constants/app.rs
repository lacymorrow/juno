//! # Application Constants
//!
//! Core application identity and configuration constants.

pub const APP_NAME: &str = "Juno";
pub const BUNDLE_IDENTIFIER: &str = "com.juno.desktop";
pub const PRODUCT_NAME: &str = "Juno";
/// What a demo build calls itself on screen. The bundle id and the app name on
/// disk stay the same as a normal Juno so the real app installs over it.
pub const DEMO_PRODUCT_NAME: &str = "Juno Demo";
pub const ENTITLEMENTS_FILE: &str = "juno.entitlements";
pub const CONFIG_DIR_NAME: &str = ".juno";
pub const SCREENSHOT_PREFIX: &str = "juno_screenshot_";
pub const DEVICE_NAME_PREFIX: &str = "Juno-";

// Cloud service identifiers (moved from frontend constants.ts)
pub const CLOUD_WS_GLOBAL_VAR: &str = "__JUNO_CLOUD_WS";

#[cfg(test)]
mod tests {
    use super::*;

    /// The constant and the bundle macOS actually sees must agree. Settings,
    /// history and permissions all live under the bundle id.
    #[test]
    fn bundle_identifier_matches_tauri_conf() {
        let conf: serde_json::Value =
            serde_json::from_str(include_str!("../../tauri.conf.json")).unwrap_or_default();
        assert_eq!(conf["identifier"].as_str(), Some(BUNDLE_IDENTIFIER));
    }

    /// Demo builds share the normal bundle id so installing the real Juno over
    /// a demo keeps everything. The build script must not override it.
    #[test]
    fn build_script_does_not_override_identifier() {
        let script = include_str!("../../../scripts/tauri-build.sh");
        assert!(!script.contains("\"identifier\""));
        assert!(!script.contains("\"productName\""));
    }
}
