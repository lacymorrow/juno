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
//! agent that could change its own permission mode or its ask-before-send gate
//! could be talked out of both by text on a web page or on screen, and one
//! that could change its provider, model, key or system prompt could spend the
//! person's money or leave Juno unable to answer. So a protected setting can be
//! listed, read (secrets only as set or not set), opened and highlighted, and
//! never changed by the agent. The person changes it.
//!
//! The owner chose (2026-10-08) to let the agent change mouse control, account
//! MCP connectors, tool categories and MCP servers. Those stay safe by
//! construction elsewhere: a server the agent adds is saved unapproved, so its
//! command never runs until the person approves it, exactly as when the person
//! adds one in the window.
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
    Tools,
    Automations,
    Network,
    Security,
    Advanced,
}

impl Pane {
    pub const ALL: [Pane; 10] = [
        Pane::General,
        Pane::Triggers,
        Pane::Audio,
        Pane::Providers,
        Pane::Models,
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
            Pane::Triggers => "Shortcuts",
            Pane::Audio => "Audio",
            Pane::Providers => "Providers",
            Pane::Models => "Models",
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
    /// pane, "security" and "security & privacy" the same one. Notifications
    /// lived in a pane of their own once and now sit in General, so the old
    /// name still finds them. The Triggers pane is shown as Shortcuts;
    /// "triggers" and "keyboard shortcuts" reach it.
    pub fn from_name(name: &str) -> Option<Pane> {
        let wanted = name.trim().to_ascii_lowercase();
        Pane::ALL.into_iter().find(|pane| {
            pane.id() == wanted
                || pane.name().to_ascii_lowercase() == wanted
                || (*pane == Pane::Security && wanted == "privacy")
                || (*pane == Pane::General && wanted == "notifications")
                || (*pane == Pane::Triggers && wanted == "keyboard shortcuts")
        })
    }
}

/// Lowercase letters and digits only, so "Speaking-Speed", "speaking speed"
/// and "speaking_speed" are one word.
fn squash(name: &str) -> String {
    name.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

/// Where a name from a link or the agent leads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Resolved {
    Setting(SettingKey),
    Pane(Pane),
}

/// Resolve leniently: an exact setting id, then a setting's last id segment or
/// alias ("voice" is `audio.voice`), then a pane ("security"). `None` when
/// nothing matches; callers open General.
pub fn resolve_target(name: &str) -> Option<Resolved> {
    SettingKey::resolve_lenient(name)
        .map(Resolved::Setting)
        .or_else(|| Pane::from_name(name).map(Resolved::Pane))
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
    /// Turn one tool category on or off: `{ "category": "Browser", "enabled": false }`.
    CategoryToggle,
    /// Add or remove one MCP server (see [`McpChange`]).
    McpServerChange,
}

/// Tool categories as `ToolConfigManager::parse_tool_category` spells them. A
/// test parses each one, so this list cannot name a category that does not
/// exist.
pub const TOOL_CATEGORIES: &[&str] = &[
    "AnthropicComputerUse",
    "Desktop",
    "Browser",
    "Timer",
    "Basic",
    "MCP",
];

/// One MCP server change, checked before anything is written.
#[derive(Debug, Clone, PartialEq)]
pub enum McpChange {
    /// The window's "Add server" shape: one server, keyed by its name.
    Add {
        name: String,
        command: String,
        args: Vec<String>,
        env: std::collections::HashMap<String, String>,
        description: Option<String>,
    },
    /// A server by name or id.
    Remove(String),
}

impl ValueKind {
    fn describe(self) -> Value {
        match self {
            ValueKind::CategoryToggle => json!({
                "type": "object",
                "shape": { "category": TOOL_CATEGORIES, "enabled": "boolean" }
            }),
            ValueKind::McpServerChange => json!({
                "type": "object",
                "add": { "add": { "<server name>": { "command": "npx", "args": ["..."], "env": {} } } },
                "remove": { "remove": "<server name or id>" }
            }),
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
    /// Offered (listed, readable, settable, highlightable) only while debug
    /// mode is on, because it is known to be broken for most voices and
    /// providers. The Settings window hides the row on the same condition.
    pub debug_only: bool,
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
    MouseControl,
    AccountConnectors,
    ToolCategories,
    McpServers,
    // Protected from here down.
    PermissionMode,
    AskBeforeSend,
    ActiveProvider,
    Model,
    ApiKey,
    SystemPrompt,
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

    /// Other words a person or a link uses for this setting, beyond its id and
    /// the id's last segment. Compared after [`squash`].
    pub fn aliases(self) -> &'static [&'static str] {
        match self {
            SettingKey::JunoVoice => &["juno voice", "mac voice", "system voice"],
            SettingKey::SpeakingSpeed => &["speed", "rate", "speech rate"],
            SettingKey::InputDevice => &["mic", "input"],
            SettingKey::OutputDevice => &["output", "speakers"],
            SettingKey::PlaySounds => &["sounds"],
            _ => &[],
        }
    }

    /// A setting by exact id, then by an id's last segment or an alias.
    /// `None` when nothing matches, or when a last segment names two settings.
    pub fn resolve_lenient(name: &str) -> Option<SettingKey> {
        if let Ok(key) = SettingKey::from_id(name) {
            return Some(key);
        }
        let wanted = squash(name);
        if wanted.is_empty() {
            return None;
        }
        let only = |pred: &dyn Fn(SettingKey) -> bool| -> Option<SettingKey> {
            let mut found = SettingKey::ALL.iter().copied().filter(|k| pred(*k));
            match (found.next(), found.next()) {
                (Some(one), None) => Some(one),
                _ => None,
            }
        };
        let last = |k: SettingKey| {
            k.spec()
                .id
                .rsplit('.')
                .next()
                .is_some_and(|seg| squash(seg) == wanted)
        };
        let alias = |k: SettingKey| k.aliases().iter().any(|a| squash(a) == wanted);
        only(&last).or_else(|| only(&alias))
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
            debug_only: false,
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
            K::SpeakingSpeed => SettingSpec {
                debug_only: true,
                ..row(
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
                )
            },
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
            K::MouseControl => row(
                self,
                "advanced.mouse_control",
                "Mouse control",
                Pane::Advanced,
                "mouse-control",
                V::Choice(&["ask", "always"]),
                "agent.mouse_control",
            ),
            K::AccountConnectors => SettingSpec {
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
            K::ToolCategories => row(
                self,
                "tools.tool_categories",
                "Tool categories",
                Pane::Tools,
                "tool-categories",
                V::CategoryToggle,
                "tools.category_enabled",
            ),
            K::McpServers => row(
                self,
                "network.mcp_servers",
                "MCP servers",
                Pane::Network,
                "mcp-json-config",
                V::McpServerChange,
                "tools.mcp_servers",
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
    /// A tool category (canonical spelling from [`TOOL_CATEGORIES`]) on or off.
    Category {
        category: &'static str,
        enabled: bool,
    },
    /// A validated MCP server change.
    Mcp(McpChange),
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
        ValueKind::CategoryToggle => check_category(raw),
        ValueKind::McpServerChange => check_mcp_change(raw).map(NewValue::Mcp),
    }
}

/// The agent may send an object, or the same object as a JSON string.
/// Malformed JSON is refused here, before anything is written.
fn as_object(raw: &Value) -> Result<serde_json::Map<String, Value>, String> {
    let value = match raw {
        Value::String(text) => serde_json::from_str::<Value>(text)
            .map_err(|e| format!("That is not valid JSON ({e}). Nothing was changed."))?,
        other => other.clone(),
    };
    match value {
        Value::Object(map) => Ok(map),
        _ => Err("Expected a JSON object. Nothing was changed.".to_string()),
    }
}

fn check_category(raw: &Value) -> Result<NewValue, String> {
    let map = as_object(raw)?;
    let wanted = map
        .get("category")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or("");
    // "Browser", "browser" and "Browser Tools" all name the Browser category.
    let squashed: String = wanted
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect::<String>()
        .to_ascii_lowercase();
    let category = TOOL_CATEGORIES
        .iter()
        .copied()
        .find(|c| {
            let c = c.to_ascii_lowercase();
            squashed == c || squashed == format!("{c}tools")
        })
        .ok_or_else(|| {
            format!(
                "Unknown tool category \"{wanted}\". Categories: {}.",
                TOOL_CATEGORIES.join(", ")
            )
        })?;
    let enabled = map
        .get("enabled")
        .and_then(parse_bool)
        .ok_or("Tool categories need \"enabled\": true or false.")?;
    Ok(NewValue::Category { category, enabled })
}

/// Read an add or a remove. An add takes the same shape the window's "Add
/// server" box takes (`{ "<name>": { "command", "args", "env" } }`) under
/// `"add"`; a remove names one server under `"remove"`.
pub fn check_mcp_change(raw: &Value) -> Result<McpChange, String> {
    let map = as_object(raw)?;
    if let Some(target) = map.get("remove") {
        let target = target
            .as_str()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or("\"remove\" takes a server name or id.")?;
        return Ok(McpChange::Remove(target.to_string()));
    }
    let add = match map.get("add") {
        Some(Value::Object(add)) => add,
        Some(Value::String(text)) => {
            return check_mcp_change(&json!({ "add": as_object(&json!(text))? }));
        }
        _ => {
            return Err(
                "Pass {\"add\": {\"<name>\": {\"command\": ..., \"args\": [...]}}} or \
                 {\"remove\": \"<name>\"}. Nothing was changed."
                    .to_string(),
            )
        }
    };
    let mut servers = add.iter();
    let (name, config) = match (servers.next(), servers.next()) {
        (Some(only), None) => only,
        _ => return Err("Add one server at a time, keyed by its name.".to_string()),
    };
    let name = name.trim();
    if name.is_empty() {
        return Err("The server needs a name.".to_string());
    }
    let config = config
        .as_object()
        .ok_or("The server's configuration must be an object.")?;
    let command = config
        .get("command")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or("The server needs a \"command\".")?
        .to_string();
    let args = match config.get("args") {
        None | Some(Value::Null) => Vec::new(),
        Some(Value::Array(items)) => items
            .iter()
            .map(|a| a.as_str().map(str::to_string))
            .collect::<Option<Vec<String>>>()
            .ok_or("\"args\" must be a list of strings.")?,
        Some(_) => return Err("\"args\" must be a list of strings.".to_string()),
    };
    let env = match config.get("env") {
        None | Some(Value::Null) => std::collections::HashMap::new(),
        Some(Value::Object(vars)) => vars
            .iter()
            .map(|(k, v)| v.as_str().map(|v| (k.clone(), v.to_string())))
            .collect::<Option<std::collections::HashMap<String, String>>>()
            .ok_or("\"env\" values must be strings.")?,
        Some(_) => return Err("\"env\" must be an object of strings.".to_string()),
    };
    let description = config
        .get("description")
        .and_then(Value::as_str)
        .map(str::to_string);
    Ok(McpChange::Add {
        name: name.to_string(),
        command,
        args,
        env,
        description,
    })
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
    /// whether it asks before it sends, where it sends and who bills for it,
    /// what it is told to be, and which individual tools it has. Anything that
    /// writes one of these is protected.
    ///
    /// This is the list to extend when a new guardrail setting appears. The
    /// tests below fail if any spec writes one of these without being
    /// protected, and if any protected spec writes something not listed here.
    ///
    /// Not listed, by the owner's decision of 2026-10-08: `agent.mouse_control`,
    /// `tools.category_enabled` and `tools.mcp_servers` (a server the agent
    /// adds is saved unapproved and never runs until the person approves it).
    const GUARDRAIL_FIELDS: &[&str] = &[
        "agent.permission_mode",
        "cli_ask_before_send_enabled",
        "providers.",
        "tools.tools",
        "cloud.",
        "updates.",
    ];

    /// Fields under a guardrail prefix that are deliberately not guardrails.
    /// Each one is an owner decision, not a convenience.
    const CARVED_OUT: &[&str] = &[
        // Which of the person's own claude.ai connectors load (2026-10-08).
        // Sending through them still asks while "Ask before Juno sends" is on.
        "providers.load_account_mcp",
    ];

    fn writes_a_guardrail(spec: &SettingSpec) -> bool {
        if CARVED_OUT.contains(&spec.writes) {
            return false;
        }
        GUARDRAIL_FIELDS.iter().any(|field| {
            spec.writes == *field || (field.ends_with('.') && spec.writes.starts_with(field))
        })
    }

    /// The other direction of the link: protection is never decoration. Each
    /// protected spec names a guardrail field, so the guard and the field it
    /// guards cannot drift apart.
    #[test]
    fn every_protected_setting_writes_a_guardrail() {
        for spec in all_specs().filter(|s| s.protected) {
            assert!(
                writes_a_guardrail(&spec),
                "{} is protected but writes {}, which GUARDRAIL_FIELDS does not name",
                spec.id,
                spec.writes
            );
        }
        // And every guardrail field is still pinned by some protected setting
        // or is out of the agent's reach entirely (no spec writes it).
        for field in GUARDRAIL_FIELDS {
            for spec in all_specs() {
                let hits = spec.writes == *field
                    || (field.ends_with('.') && spec.writes.starts_with(field));
                if hits && !CARVED_OUT.contains(&spec.writes) {
                    assert!(spec.protected, "{} writes guardrail {field}", spec.id);
                }
            }
        }
    }

    #[test]
    fn the_carve_outs_are_real_settings() {
        for field in CARVED_OUT {
            assert!(
                all_specs().any(|s| s.writes == *field && !s.protected),
                "{field} is carved out but no settable spec writes it; drop it"
            );
        }
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

    /// The Security pane is where permissions and approvals live. Nothing in
    /// it is the agent's to change.
    #[test]
    fn nothing_in_security_is_settable() {
        for spec in all_specs() {
            if spec.pane == Pane::Security {
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
            protected.len() >= 6,
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
        let expected: HashSet<&str> = [
            "security.permission_mode",
            "security.ask_before_send",
            "providers.provider",
            "providers.model",
            "providers.api_key",
            "providers.system_prompt",
        ]
        .into_iter()
        .collect();
        assert_eq!(protected, expected);
        // Settable by the owner's decision of 2026-10-08.
        for id in [
            "advanced.mouse_control",
            "providers.account_connectors",
            "tools.tool_categories",
            "network.mcp_servers",
        ] {
            assert!(!protected.contains(id), "{id} was opened to the agent");
        }
    }

    #[test]
    fn every_tool_category_parses_where_the_window_sends_it() {
        for category in TOOL_CATEGORIES {
            assert!(
                crate::agent::tools::tool_config::ToolConfigManager::parse_tool_category(category)
                    .is_ok(),
                "{category}"
            );
        }
    }

    #[test]
    fn tool_categories_are_checked() {
        assert_eq!(
            authorize_set(
                SettingKey::ToolCategories,
                &json!({ "category": "browser tools", "enabled": false })
            )
            .map(|c| c.value().clone()),
            Ok(NewValue::Category {
                category: "Browser",
                enabled: false
            })
        );
        assert!(authorize_set(
            SettingKey::ToolCategories,
            &json!({ "category": "Lasers", "enabled": true })
        )
        .is_err());
        assert!(authorize_set(SettingKey::ToolCategories, &json!({ "category": "MCP" })).is_err());
    }

    #[test]
    fn an_mcp_add_is_read_in_the_windows_shape() {
        let change = check_mcp_change(&json!({
            "add": { "firecrawl": { "command": "npx", "args": ["-y", "firecrawl-mcp"],
                                    "env": { "FIRECRAWL_API_KEY": "k" } } }
        }))
        .expect("valid add");
        match change {
            McpChange::Add {
                name,
                command,
                args,
                env,
                ..
            } => {
                assert_eq!(name, "firecrawl");
                assert_eq!(command, "npx");
                assert_eq!(args, vec!["-y", "firecrawl-mcp"]);
                assert_eq!(env.get("FIRECRAWL_API_KEY").map(String::as_str), Some("k"));
            }
            other => panic!("not an add: {other:?}"),
        }
        // The same object as a JSON string.
        assert!(
            check_mcp_change(&json!(r#"{"add": {"x": {"command": "uvx", "args": []}}}"#)).is_ok()
        );
        assert_eq!(
            check_mcp_change(&json!({ "remove": "firecrawl" })),
            Ok(McpChange::Remove("firecrawl".into()))
        );
    }

    /// A malformed config is refused before anything is written: the writer
    /// only ever sees a checked `McpChange`.
    #[test]
    fn a_malformed_mcp_config_is_refused() {
        for bad in [
            json!("{\"add\": {\"x\": {\"command\": "),
            json!({ "add": { "x": { "args": ["a"] } } }),
            json!({ "add": { "x": { "command": "npx", "args": "not a list" } } }),
            json!({ "add": { "x": { "command": "npx", "env": { "K": 1 } } } }),
            json!({ "add": { "a": { "command": "npx" }, "b": { "command": "npx" } } }),
            json!({ "remove": "" }),
            json!([1, 2]),
            json!({}),
        ] {
            assert!(
                authorize_set(SettingKey::McpServers, &bad).is_err(),
                "{bad} should be refused"
            );
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
    fn lenient_resolution_goes_id_then_segment_or_alias_then_pane() {
        // Exact id.
        assert_eq!(
            resolve_target("audio.voice"),
            Some(Resolved::Setting(SettingKey::JunoVoice))
        );
        // Last segment: "voice" is the setting, not the Voice pane.
        assert_eq!(
            resolve_target("voice"),
            Some(Resolved::Setting(SettingKey::JunoVoice))
        );
        assert_eq!(
            resolve_target(" Speaking-Speed "),
            Some(Resolved::Setting(SettingKey::SpeakingSpeed))
        );
        // Alias.
        assert_eq!(
            resolve_target("mic"),
            Some(Resolved::Setting(SettingKey::InputDevice))
        );
        // A pane when no setting answers to the name.
        assert_eq!(
            resolve_target("security"),
            Some(Resolved::Pane(Pane::Security))
        );
        assert_eq!(
            resolve_target("providers"),
            Some(Resolved::Pane(Pane::Providers))
        );
        // Nothing.
        assert_eq!(resolve_target("nope"), None);
        assert_eq!(resolve_target(""), None);
    }

    #[test]
    fn no_last_segment_or_alias_is_ambiguous_or_shadows_an_id() {
        for key in SettingKey::ALL {
            let spec = key.spec();
            assert_eq!(
                SettingKey::resolve_lenient(spec.id),
                Some(*key),
                "{} must resolve to itself",
                spec.id
            );
            let last = spec.id.rsplit('.').next().unwrap_or("");
            assert_eq!(
                SettingKey::resolve_lenient(last),
                Some(*key),
                "last segment {last} of {} is ambiguous",
                spec.id
            );
            for alias in key.aliases() {
                assert_eq!(
                    SettingKey::resolve_lenient(alias),
                    Some(*key),
                    "alias {alias} of {} collides",
                    spec.id
                );
            }
        }
    }

    #[test]
    fn speaking_speed_is_debug_only_and_nothing_else_is() {
        for key in SettingKey::ALL {
            assert_eq!(
                key.spec().debug_only,
                *key == SettingKey::SpeakingSpeed,
                "{}",
                key.spec().id
            );
        }
    }

    #[test]
    fn panes_resolve_by_id_or_name() {
        assert_eq!(Pane::from_name("voice"), Some(Pane::Audio));
        assert_eq!(Pane::from_name("Audio"), Some(Pane::Audio));
        assert_eq!(Pane::from_name("providers"), Some(Pane::Providers));
        assert_eq!(Pane::from_name("Security & Privacy"), Some(Pane::Security));
        // The retired Notifications pane's name lands where the row moved.
        assert_eq!(Pane::from_name("Notifications"), Some(Pane::General));
        // Shown as Shortcuts; the old name and the id still find it.
        assert_eq!(Pane::from_name("Shortcuts"), Some(Pane::Triggers));
        assert_eq!(Pane::from_name("triggers"), Some(Pane::Triggers));
        assert_eq!(Pane::from_name("Keyboard Shortcuts"), Some(Pane::Triggers));
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
