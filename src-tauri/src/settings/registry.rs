//! # The settings the agent can see, find and change
//!
//! One typed table of the settings Juno's own agent may reach through the
//! `settings` tool (`agent::tools::settings_tool`): where each one lives in the
//! Settings window, what values it takes, whether it hides behind "Show
//! advanced settings", and whether it is **protected**.
//!
//! ## Why a table and not a string
//!
//! A key is a [`SettingKey`] variant. The agent names a setting by its `id`
//! string, and [`SettingKey::from_id`] is the only way from that string to a
//! key: an id nothing declares is an error that lists the real ones, never a
//! silent no-op. Every match on a key in this module and in the tool is
//! exhaustive, so a new variant without a reader, a writer and a protection
//! decision does not compile. [`SettingKey::ALL`] is produced by the same macro
//! that declares the enum, so it cannot drift from it.
//!
//! ## Protected settings
//!
//! Juno is permissive by default; the exceptions are sending and spending. An
//! agent that could change its own permission mode, its approval gates, its
//! tools, its keys or its MCP servers could be talked out of every guardrail
//! by text on a web page or on screen. So a protected setting can be listed,
//! read (secrets only as set or not set), opened and highlighted, and never
//! changed by the agent. The person changes it.
//!
//! The refusal is not a check the tool remembers to make. [`authorize_set`] is
//! the only constructor of [`AuthorizedChange`], and the tool's writer takes
//! nothing else, so there is no path from the agent to a write that skips it.
//! The tests below pin the other half: every setting that writes a guardrail
//! store field is protected, so a new one cannot slip in unmarked.

use serde_json::{json, Value};

/// A pane of the Settings window. `id` is the sidebar id in
/// `src/components/settings/ModularSettingsWindow.tsx` (`settingsCategories`);
/// a test reads that file and fails if the two disagree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Pane {
    General,
    Triggers,
    Audio,
    Providers,
    Models,
    Notifications,
    Tools,
    Automations,
    Network,
    Security,
    Advanced,
}

impl Pane {
    pub const ALL: [Pane; 11] = [
        Pane::General,
        Pane::Triggers,
        Pane::Audio,
        Pane::Providers,
        Pane::Models,
        Pane::Notifications,
        Pane::Tools,
        Pane::Automations,
        Pane::Network,
        Pane::Security,
        Pane::Advanced,
    ];

    /// The sidebar id the Settings window selects.
    pub fn id(self) -> &'static str {
        match self {
            Pane::General => "general",
            Pane::Triggers => "triggers",
            Pane::Audio => "voice",
            Pane::Providers => "ai",
            Pane::Models => "models",
            Pane::Notifications => "notifications",
            Pane::Tools => "tools",
            Pane::Automations => "automations",
            Pane::Network => "network",
            Pane::Security => "security",
            Pane::Advanced => "advanced",
        }
    }

    /// The name the sidebar shows.
    pub fn name(self) -> &'static str {
        match self {
            Pane::General => "General",
            Pane::Triggers => "Triggers",
            Pane::Audio => "Audio",
            Pane::Providers => "Providers",
            Pane::Models => "Models",
            Pane::Notifications => "Notifications",
            Pane::Tools => "Tools",
            Pane::Automations => "Automations",
            Pane::Network => "Network",
            Pane::Security => "Security & Privacy",
            Pane::Advanced => "Advanced",
        }
    }

    /// Hidden until "Show advanced settings" is on (`advanced: true` in
    /// `settingsCategories`).
    pub fn advanced(self) -> bool {
        matches!(
            self,
            Pane::Tools | Pane::Automations | Pane::Network | Pane::Advanced
        )
    }

    /// A pane by sidebar id or by visible name, ignoring case. "voice" and
    /// "audio" both reach the Audio pane, "ai" and "providers" the Providers
    /// pane, "security" and "security & privacy" the same one.
    pub fn from_name(name: &str) -> Option<Pane> {
        let wanted = name.trim().to_ascii_lowercase();
        Pane::ALL.into_iter().find(|pane| {
            pane.id() == wanted
                || pane.name().to_ascii_lowercase() == wanted
                || (*pane == Pane::Security && wanted == "privacy")
        })
    }
}

/// What a setting takes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ValueKind {
    /// On or off.
    Bool,
    /// One of a fixed list.
    Choice(&'static [&'static str]),
    /// One of a list only the running app knows (installed voices, connected
    /// devices). `get` returns the current list.
    LiveChoice,
    /// A number in a closed range.
    Number { min: f64, max: f64 },
    /// A keyboard shortcut such as "Option+Space" or "Fn+Space".
    Shortcut,
    /// Free text.
    Text,
}

impl ValueKind {
    fn describe(self) -> Value {
        match self {
            ValueKind::Bool => json!({ "type": "boolean" }),
            ValueKind::Choice(values) => json!({ "type": "choice", "values": values }),
            ValueKind::LiveChoice => {
                json!({ "type": "choice", "values": "call get for the current list" })
            }
            ValueKind::Number { min, max } => json!({ "type": "number", "min": min, "max": max }),
            ValueKind::Shortcut => json!({ "type": "shortcut", "example": "Option+Space" }),
            ValueKind::Text => json!({ "type": "text" }),
        }
    }
}

/// One row of the table.
#[derive(Debug, Clone, Copy)]
pub struct SettingSpec {
    pub key: SettingKey,
    /// What the agent calls it: `pane.setting`, lowercase.
    pub id: &'static str,
    /// The row's label in the window.
    pub label: &'static str,
    pub pane: Pane,
    /// The row's anchor, rendered as `settings-row-<row>` by `SettingsRow`.
    pub row: &'static str,
    pub kind: ValueKind,
    /// The row itself is behind "Show advanced settings" (its pane may be too).
    pub advanced: bool,
    /// The agent may show this setting and must not change it.
    pub protected: bool,
    /// The store field the UI's write path changes, as `section.field`. Read by
    /// the tests that keep guardrails protected, so it must be the truth.
    pub writes: &'static str,
}

impl SettingSpec {
    /// Behind the advanced toggle, by row or by pane.
    pub fn needs_advanced(&self) -> bool {
        self.advanced || self.pane.advanced()
    }

    /// The listing the agent reads.
    pub fn describe(&self) -> Value {
        json!({
            "key": self.id,
            "label": self.label,
            "pane": self.pane.name(),
            "value": self.kind.describe(),
            "advanced": self.needs_advanced(),
            "protected": self.protected,
        })
    }
}

macro_rules! setting_keys {
    ($($variant:ident),+ $(,)?) => {
        /// Every setting the agent can reach. Add a variant and the compiler
        /// asks for its spec, its reader and its writer.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum SettingKey {
            $($variant),+
        }

        impl SettingKey {
            /// Every key, by construction from the same list as the enum.
            pub const ALL: &'static [SettingKey] = &[$(SettingKey::$variant),+];
        }
    };
}

setting_keys! {
    JunoVoice,
    SpeakingSpeed,
    InputDevice,
    OutputDevice,
    PlaySounds,
    OpenAtLogin,
    CursorColor,
    AgentShortcut,
    DictationShortcut,
    BackgroundMode,
    PersistentSession,
    SmoothMouseMovement,
    // Protected from here down.
    PermissionMode,
    AskBeforeSend,
    MouseControl,
    ActiveProvider,
    Model,
    ApiKey,
    SystemPrompt,
    AccountConnectors,
    ToolCategories,
    McpServers,
}

impl SettingKey {
    /// The key the agent named, or an error that lists the real ones.
    pub fn from_id(id: &str) -> Result<SettingKey, String> {
        let wanted = id.trim().to_ascii_lowercase();
        SettingKey::ALL
            .iter()
            .copied()
            .find(|key| key.spec().id == wanted)
            .ok_or_else(|| {
                let known: Vec<&str> = SettingKey::ALL.iter().map(|k| k.spec().id).collect();
                format!(
                    "There is no setting called \"{}\". Known settings: {}.",
                    id.trim(),
                    known.join(", ")
                )
            })
    }

    pub fn spec(self) -> SettingSpec {
        use SettingKey as K;
        use ValueKind as V;
        let row = |key: SettingKey,
                   id: &'static str,
                   label: &'static str,
                   pane: Pane,
                   anchor: &'static str,
                   kind: ValueKind,
                   writes: &'static str| SettingSpec {
            key,
            id,
            label,
            pane,
            row: anchor,
            kind,
            advanced: false,
            protected: false,
            writes,
        };
        match self {
            K::JunoVoice => row(
                self,
                "audio.voice",
                "Juno's voice",
                Pane::Audio,
                "juno-voice",
                V::LiveChoice,
                "audio.system_voice",
            ),
            K::SpeakingSpeed => row(
                self,
                "audio.speaking_speed",
                "Speaking speed",
                Pane::Audio,
                "voice-speed",
                V::Number {
                    min: crate::tts::rate::MIN_RATE,
                    max: crate::tts::rate::MAX_RATE,
                },
                "audio.voice_rate",
            ),
            K::InputDevice => row(
                self,
                "audio.microphone",
                "Listen through",
                Pane::Audio,
                "audio-input-device",
                V::LiveChoice,
                "audio.input_device",
            ),
            K::OutputDevice => row(
                self,
                "audio.speaker",
                "Speak through",
                Pane::Audio,
                "audio-output-device",
                V::LiveChoice,
                "audio.output_device",
            ),
            K::PlaySounds => row(
                self,
                "audio.play_sounds",
                "Play sounds",
                Pane::Audio,
                "sound-enabled",
                V::Bool,
                "audio.sound_enabled",
            ),
            K::OpenAtLogin => row(
                self,
                "general.open_at_login",
                "Open at login",
                Pane::General,
                "auto-launch",
                V::Bool,
                "autostart_enabled",
            ),
            K::CursorColor => row(
                self,
                "general.cursor_color",
                "Cursor color",
                Pane::General,
                "cursor-color",
                V::Choice(crate::constants::ui::agent_cursor_colors::ALL),
                "floating_bar.agent_cursor_color",
            ),
            K::AgentShortcut => row(
                self,
                "triggers.agent_shortcut",
                "Shortcut that summons Juno",
                Pane::Triggers,
                "trigger-agent",
                V::Shortcut,
                "triggers",
            ),
            K::DictationShortcut => row(
                self,
                "triggers.dictation_shortcut",
                "Shortcut for dictation",
                Pane::Triggers,
                "trigger-dictation",
                V::Shortcut,
                "triggers",
            ),
            K::BackgroundMode => row(
                self,
                "advanced.work_in_background",
                "Work in the background",
                Pane::Advanced,
                "background-mode",
                V::Bool,
                "agent.background_mode",
            ),
            K::PersistentSession => row(
                self,
                "advanced.persistent_claude_session",
                "Persistent Claude session",
                Pane::Advanced,
                "cli-persistent-session",
                V::Bool,
                "cli_persistent_session_enabled",
            ),
            K::SmoothMouseMovement => row(
                self,
                "tools.smooth_mouse_movement",
                "Smooth mouse movement",
                Pane::Tools,
                "smooth-mouse-movement",
                V::Bool,
                "tools.smooth_mouse_movement",
            ),

            // Protected: the agent may show these and never change them.
            K::PermissionMode => SettingSpec {
                protected: true,
                ..row(
                    self,
                    "security.permission_mode",
                    "When Juno needs permission",
                    Pane::Security,
                    "permission-mode",
                    V::Choice(&["ask_first", "ask_when_risky", "dont_ask"]),
                    "agent.permission_mode",
                )
            },
            K::AskBeforeSend => SettingSpec {
                protected: true,
                advanced: true,
                ..row(
                    self,
                    "security.ask_before_send",
                    "Ask before Juno sends",
                    Pane::Security,
                    "cli-ask-before-send",
                    V::Bool,
                    "cli_ask_before_send_enabled",
                )
            },
            K::MouseControl => SettingSpec {
                protected: true,
                ..row(
                    self,
                    "advanced.mouse_control",
                    "Mouse control",
                    Pane::Advanced,
                    "mouse-control",
                    V::Choice(&["ask", "always"]),
                    "agent.mouse_control",
                )
            },
            K::ActiveProvider => SettingSpec {
                protected: true,
                ..row(
                    self,
                    "providers.provider",
                    "Active Provider",
                    Pane::Providers,
                    "ai-provider",
                    V::Text,
                    "providers.active_provider",
                )
            },
            K::Model => SettingSpec {
                protected: true,
                ..row(
                    self,
                    "providers.model",
                    "Model",
                    Pane::Providers,
                    "ai-model",
                    V::Text,
                    "providers.model",
                )
            },
            K::ApiKey => SettingSpec {
                protected: true,
                ..row(
                    self,
                    "providers.api_key",
                    "API key",
                    Pane::Providers,
                    "api-key",
                    V::Text,
                    "providers.api_key",
                )
            },
            K::SystemPrompt => SettingSpec {
                protected: true,
                advanced: true,
                ..row(
                    self,
                    "providers.system_prompt",
                    "System Prompt",
                    Pane::Providers,
                    "system-prompt",
                    V::Text,
                    "providers.system_prompt",
                )
            },
            K::AccountConnectors => SettingSpec {
                protected: true,
                advanced: true,
                ..row(
                    self,
                    "providers.account_connectors",
                    "Load account MCP connectors",
                    Pane::Providers,
                    "load-account-mcp",
                    V::Bool,
                    "providers.load_account_mcp",
                )
            },
            K::ToolCategories => SettingSpec {
                protected: true,
                ..row(
                    self,
                    "tools.tool_categories",
                    "Tool categories",
                    Pane::Tools,
                    "tool-categories",
                    V::Text,
                    "tools.category_enabled",
                )
            },
            K::McpServers => SettingSpec {
                protected: true,
                ..row(
                    self,
                    "network.mcp_servers",
                    "MCP servers",
                    Pane::Network,
                    "mcp-json-config",
                    V::Text,
                    "tools.mcp_servers",
                )
            },
        }
    }
}

/// Every spec, in table order.
pub fn all_specs() -> impl Iterator<Item = SettingSpec> {
    SettingKey::ALL.iter().map(|key| key.spec())
}

/// A change the agent is allowed to make, with its value already checked
/// against the setting's kind. Only [`authorize_set`] builds one.
#[derive(Debug, Clone, PartialEq)]
pub struct AuthorizedChange {
    key: SettingKey,
    value: NewValue,
}

impl AuthorizedChange {
    pub fn key(&self) -> SettingKey {
        self.key
    }

    pub fn value(&self) -> &NewValue {
        &self.value
    }
}

/// A checked value.
#[derive(Debug, Clone, PartialEq)]
pub enum NewValue {
    Bool(bool),
    /// For a fixed choice, the canonical spelling from the list. For a live
    /// choice, what the agent asked for; the writer matches it against the
    /// running app's list.
    Text(String),
    Number(f64),
}

/// Why a change was not made.
#[derive(Debug, Clone, PartialEq)]
pub enum Refusal {
    /// A protected setting. The person changes it.
    Protected {
        label: &'static str,
        pane: &'static str,
    },
    /// The value does not fit the setting.
    BadValue(String),
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Refusal::Protected { label, pane } => write!(
                f,
                "\"{label}\" is protected: Juno can show it but only the person can change it. \
                 Open it with action \"highlight\" and ask them to change it in Settings > {pane}."
            ),
            Refusal::BadValue(why) => f.write_str(why),
        }
    }
}

/// The one door from an agent request to a settings write.
///
/// Protection is checked first and for every value, so a protected setting is
/// refused even when the value is valid, malformed, or the same as now.
pub fn authorize_set(key: SettingKey, raw: &Value) -> Result<AuthorizedChange, Refusal> {
    let spec = key.spec();
    if spec.protected {
        return Err(Refusal::Protected {
            label: spec.label,
            pane: spec.pane.name(),
        });
    }
    let value = check_value(&spec, raw).map_err(Refusal::BadValue)?;
    Ok(AuthorizedChange { key, value })
}

fn check_value(spec: &SettingSpec, raw: &Value) -> Result<NewValue, String> {
    let text = || {
        raw.as_str()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .ok_or_else(|| format!("\"{}\" takes a text value.", spec.label))
    };
    match spec.kind {
        ValueKind::Bool => parse_bool(raw)
            .map(NewValue::Bool)
            .ok_or_else(|| format!("\"{}\" is on or off: pass true or false.", spec.label)),
        ValueKind::Choice(values) => {
            let wanted = text()?;
            values
                .iter()
                .find(|v| v.eq_ignore_ascii_case(&wanted))
                .map(|v| NewValue::Text((*v).to_string()))
                .ok_or_else(|| format!("\"{}\" can be one of: {}.", spec.label, values.join(", ")))
        }
        ValueKind::LiveChoice | ValueKind::Shortcut | ValueKind::Text => text().map(NewValue::Text),
        ValueKind::Number { min, max } => {
            let number = raw
                .as_f64()
                .or_else(|| raw.as_str().and_then(|s| s.trim().parse::<f64>().ok()))
                .filter(|n| n.is_finite())
                .ok_or_else(|| format!("\"{}\" takes a number.", spec.label))?;
            if number < min || number > max {
                return Err(format!("\"{}\" goes from {min} to {max}.", spec.label));
            }
            Ok(NewValue::Number(number))
        }
    }
}

fn parse_bool(raw: &Value) -> Option<bool> {
    if let Some(b) = raw.as_bool() {
        return Some(b);
    }
    match raw.as_str()?.trim().to_ascii_lowercase().as_str() {
        "true" | "on" | "yes" | "enabled" => Some(true),
        "false" | "off" | "no" | "disabled" => Some(false),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    /// Store fields that hold a guardrail: what Juno may do without asking,
    /// where it sends, what it spends with, which tools and servers it has, and
    /// what it is told to be. Anything that writes one of these is protected.
    ///
    /// This is the list to extend when a new guardrail setting appears. The
    /// tests below fail if any spec writes one of these without being
    /// protected.
    const GUARDRAIL_FIELDS: &[&str] = &[
        "agent.permission_mode",
        "agent.mouse_control",
        "cli_ask_before_send_enabled",
        "providers.",
        "tools.tools",
        "tools.category_enabled",
        "tools.mcp_servers",
        "cloud.",
        "updates.",
    ];

    fn writes_a_guardrail(spec: &SettingSpec) -> bool {
        GUARDRAIL_FIELDS.iter().any(|field| {
            spec.writes == *field || (field.ends_with('.') && spec.writes.starts_with(field))
        })
    }

    #[test]
    fn every_setting_that_writes_a_guardrail_is_protected() {
        for spec in all_specs() {
            if writes_a_guardrail(&spec) {
                assert!(
                    spec.protected,
                    "{} writes {}, a guardrail field, and is not protected. The agent \
                     could switch off its own safety with it.",
                    spec.id, spec.writes
                );
            }
        }
    }

    /// The Security and Network panes are where permissions, approvals and
    /// servers live. Nothing in them is the agent's to change.
    #[test]
    fn nothing_in_security_or_network_is_settable() {
        for spec in all_specs() {
            if matches!(spec.pane, Pane::Security | Pane::Network) {
                assert!(
                    spec.protected,
                    "{} sits in {} and is not protected",
                    spec.id,
                    spec.pane.name()
                );
            }
        }
    }

    /// The guard is linked to what it names: every protected key is refused by
    /// the only constructor of a change, for a valid value and an invalid one.
    #[test]
    fn the_guard_refuses_every_protected_setting_whatever_the_value() {
        let protected: Vec<SettingKey> = SettingKey::ALL
            .iter()
            .copied()
            .filter(|k| k.spec().protected)
            .collect();
        assert!(
            protected.len() >= 10,
            "the protected list shrank to {}",
            protected.len()
        );
        for key in protected {
            for value in [
                json!(true),
                json!(false),
                json!("dont_ask"),
                json!("always"),
                json!(1),
                json!(null),
            ] {
                match authorize_set(key, &value) {
                    Err(Refusal::Protected { .. }) => {}
                    other => panic!(
                        "{:?} with {value} was not refused as protected: {other:?}",
                        key
                    ),
                }
            }
        }
    }

    /// The protected list, by name, so removing a name is a visible decision.
    #[test]
    fn the_protected_list_is_the_one_in_the_pr() {
        let protected: HashSet<&str> = all_specs().filter(|s| s.protected).map(|s| s.id).collect();
        for id in [
            "security.permission_mode",
            "security.ask_before_send",
            "advanced.mouse_control",
            "providers.provider",
            "providers.model",
            "providers.api_key",
            "providers.system_prompt",
            "providers.account_connectors",
            "tools.tool_categories",
            "network.mcp_servers",
        ] {
            assert!(protected.contains(id), "{id} must stay protected");
        }
    }

    #[test]
    fn ids_and_rows_are_unique_and_well_formed() {
        let mut ids = HashSet::new();
        let mut rows = HashSet::new();
        for spec in all_specs() {
            assert!(ids.insert(spec.id), "duplicate id {}", spec.id);
            assert!(rows.insert(spec.row), "duplicate row {}", spec.row);
            assert_eq!(spec.id, spec.id.to_ascii_lowercase());
            assert!(spec.id.contains('.'), "{} is not pane.setting", spec.id);
            assert_eq!(spec.key.spec().id, spec.id);
        }
    }

    #[test]
    fn an_unknown_key_is_an_error_that_names_the_real_ones() {
        let err = SettingKey::from_id("audio.volume_of_doom").expect_err("unknown");
        assert!(err.contains("audio.voice"), "{err}");
        assert_eq!(
            SettingKey::from_id(" Audio.Voice "),
            Ok(SettingKey::JunoVoice)
        );
    }

    #[test]
    fn values_are_checked_against_the_kind() {
        assert_eq!(
            authorize_set(SettingKey::PlaySounds, &json!("off")).map(|c| c.value().clone()),
            Ok(NewValue::Bool(false))
        );
        assert!(authorize_set(SettingKey::PlaySounds, &json!("maybe")).is_err());
        assert_eq!(
            authorize_set(SettingKey::CursorColor, &json!("Pink")).map(|c| c.value().clone()),
            Ok(NewValue::Text("pink".into()))
        );
        assert!(authorize_set(SettingKey::CursorColor, &json!("plaid")).is_err());
        assert!(authorize_set(SettingKey::SpeakingSpeed, &json!(9.0)).is_err());
        assert_eq!(
            authorize_set(SettingKey::SpeakingSpeed, &json!("1.25")).map(|c| c.value().clone()),
            Ok(NewValue::Number(1.25))
        );
        assert!(authorize_set(SettingKey::AgentShortcut, &json!("  ")).is_err());
    }

    #[test]
    fn panes_resolve_by_id_or_name() {
        assert_eq!(Pane::from_name("voice"), Some(Pane::Audio));
        assert_eq!(Pane::from_name("Audio"), Some(Pane::Audio));
        assert_eq!(Pane::from_name("providers"), Some(Pane::Providers));
        assert_eq!(Pane::from_name("Security & Privacy"), Some(Pane::Security));
        assert_eq!(Pane::from_name("nowhere"), None);
    }

    /// The window and the table name the same panes and the same rows. Read
    /// from source, so renaming a row anchor without the table fails here.
    #[test]
    fn every_pane_and_row_exists_in_the_settings_window() {
        let window = include_str!("../../../src/components/settings/ModularSettingsWindow.tsx");
        for pane in Pane::ALL {
            assert!(
                window.contains(&format!("id: \"{}\"", pane.id())),
                "pane {} is not a sidebar id in ModularSettingsWindow.tsx",
                pane.id()
            );
        }
        let sections = [
            include_str!("../../../src/components/settings/sections/GeneralSettings.tsx"),
            include_str!("../../../src/components/settings/sections/VoiceSettings.tsx"),
            include_str!("../../../src/components/settings/sections/AIProviderSettings.tsx"),
            include_str!("../../../src/components/settings/sections/AssistantModelPicker.tsx"),
            include_str!("../../../src/components/settings/sections/SecuritySettings.tsx"),
            include_str!("../../../src/components/settings/sections/AdvancedSettings.tsx"),
            include_str!("../../../src/components/settings/sections/ToolsSettings.tsx"),
            include_str!("../../../src/components/settings/sections/NetworkSettings.tsx"),
            include_str!("../../../src/components/settings/sections/TriggersSettings.tsx"),
        ]
        .join("\n");
        for spec in all_specs() {
            assert!(
                sections.contains(&format!("\"{}\"", spec.row)),
                "row {} ({}) has no anchor in the settings sections",
                spec.row,
                spec.id
            );
        }
    }
}
