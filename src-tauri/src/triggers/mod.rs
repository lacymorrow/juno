//! # Triggers: unified activation model
//!
//! One list describes every way the user can summon Juno. A [`Trigger`] pairs a
//! *method* (how it fires) with a *target* (what it does), so the two axes that
//! used to live in three different settings screens — the key combo, the
//! tap/hold mode, and the always-listening wake words — collapse into a single
//! object.
//!
//! The set is bounded: a trigger is unique by `(method, target)`, giving at most
//! six (3 methods x 2 targets). The UI lets the user add one of each remaining
//! combination.
//!
//! `triggers` is the source of truth for the settings UI and for shortcut
//! registration. The legacy `KeyboardShortcuts` / `AgentSettings.trigger_mode` /
//! `AudioSettings` wake-word fields are *derived* from it (see
//! [`Trigger::apply_to_legacy`]) so existing runtime consumers keep working
//! without a codebase-wide rewrite.

use serde::{Deserialize, Deserializer, Serialize};

/// How a trigger fires.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TriggerMethod {
    /// Hold the binding to activate, release to stop.
    PushToTalk,
    /// Press-and-release the binding to toggle on, again to toggle off.
    Toggle,
    /// Speak a wake phrase to activate. No key/mouse binding.
    Voice,
}

/// What a trigger activates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TriggerTarget {
    /// The agent (Juno acts on screen).
    Agent,
    /// Dictation / transcription (speech to text).
    Dictation,
}

impl TriggerMethod {
    /// The serde name. The settings window builds a row's identity out of this
    /// and the target, so both sides must spell it the same way.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::PushToTalk => "push_to_talk",
            Self::Toggle => "toggle",
            Self::Voice => "voice",
        }
    }

    /// How the row names itself on screen.
    pub fn label(self) -> &'static str {
        match self {
            Self::PushToTalk => "Push to talk",
            Self::Toggle => "Toggle",
            Self::Voice => "Voice",
        }
    }
}

impl TriggerTarget {
    /// The serde name. See [`TriggerMethod::as_str`].
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Agent => "agent",
            Self::Dictation => "dictation",
        }
    }

    /// How the row names itself on screen.
    pub fn label(self) -> &'static str {
        match self {
            Self::Agent => "Agent",
            Self::Dictation => "Dictation",
        }
    }
}

/// A key that produces no ordinary key event, only a modifier flag change.
///
/// Caps Lock is deliberately absent. Checked on hardware: it emits one event
/// per press and nothing on release, because it is a hardware toggle, so a
/// push-to-talk bound to it would hold the microphone open until the next
/// press. Remapping it with `hidutil` to a spare function key is the honest
/// route, and that already works as an ordinary keyboard binding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModifierKey {
    /// The globe key. Reports key code 63 with bit 1 << 23 while held.
    Fn,
}

impl ModifierKey {
    /// Every key setup listens for while asking someone to press theirs.
    pub const ALL: [ModifierKey; 1] = [ModifierKey::Fn];

    /// The macOS virtual key code reported on `NSEventTypeFlagsChanged`.
    pub fn key_code(self) -> u16 {
        match self {
            Self::Fn => 63,
        }
    }

    /// The `NSEventModifierFlag` bit that is set while the key is held.
    pub fn flag_bit(self) -> usize {
        match self {
            Self::Fn => 1 << 23,
        }
    }

    /// What the settings window calls it.
    pub fn label(self) -> &'static str {
        match self {
            Self::Fn => "Fn (globe)",
        }
    }

    /// The shortcut string this key is recorded as.
    ///
    /// Fn is a key on the keyboard, so it is written down the way every other
    /// key is written down. The fact that it has to be watched differently is
    /// a detail of the watching, not of the binding.
    pub fn shortcut(self) -> &'static str {
        match self {
            Self::Fn => "Fn",
        }
    }

    /// Every spelling of this key a shortcut string may use. Apple has called
    /// the same physical key both Fn and Globe, and a hand-edited store or an
    /// older build may carry either.
    fn aliases(self) -> &'static [&'static str] {
        match self {
            Self::Fn => &["fn", "globe"],
        }
    }
}

/// The bare modifier key a shortcut string names, if it names one.
///
/// This is the single place that decides which watcher a key goes to. A bare
/// modifier produces no ordinary key event, so the global-shortcut plugin
/// cannot register it and [`crate::platform::modifier_key_monitor`] takes it
/// instead. That is a fact about how the key is watched, which is why it lives
/// in the registration path and not in the shape of a binding.
pub fn bare_modifier(shortcut: &str) -> Option<ModifierKey> {
    let shortcut = shortcut.trim();
    ModifierKey::ALL.into_iter().find(|key| {
        key.aliases()
            .iter()
            .any(|a| a.eq_ignore_ascii_case(shortcut))
    })
}

/// The physical input bound to a key/mouse method.
///
/// Two kinds, because from where the person sits there are two: a key and a
/// mouse button. Fn used to be a third, and it should not have been.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Binding {
    /// A keyboard shortcut string. Usually a combo parseable by
    /// [`crate::shortcuts::parse_shortcut_string`], e.g. `"Option+Space"`; it
    /// may also be a bare modifier such as `"Fn"`, which that parser does not
    /// know and [`bare_modifier`] does.
    Keyboard { shortcut: String },
    /// A mouse button by AppKit `buttonNumber` (0 = left, 1 = right, 2 = middle,
    /// 3+ = extra side buttons). Watched by the passive input monitor, not the
    /// global-shortcut plugin.
    Mouse { button: u16 },
}

/// The shapes a stored or incoming binding may arrive in.
///
/// `modifier` is the retired third kind. It is accepted here and converted on
/// read so an Fn trigger already on disk survives the change, and so does a
/// window that has not reloaded since. Nothing writes it: [`Binding`]
/// serializes as keyboard or mouse only, so there is one live representation.
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum StoredBinding {
    Keyboard { shortcut: String },
    Mouse { button: u16 },
    Modifier { key: ModifierKey },
}

impl<'de> Deserialize<'de> for Binding {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(match StoredBinding::deserialize(deserializer)? {
            StoredBinding::Keyboard { shortcut } => Binding::Keyboard { shortcut },
            StoredBinding::Mouse { button } => Binding::Mouse { button },
            StoredBinding::Modifier { key } => Binding::Keyboard {
                shortcut: key.shortcut().to_string(),
            },
        })
    }
}

/// Which observer is able to see a given binding fire.
///
/// One binding kind for keys, and this decides who watches each one. Keeping
/// the decision in a function of the shortcut string is what stops "the plugin
/// cannot register Fn" from leaking back into the model or the settings window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Watcher {
    /// The tauri global-shortcut plugin, which handles ordinary combos.
    GlobalShortcut,
    /// `platform::modifier_key_monitor`, for keys that only ever arrive as a
    /// change of modifier flags.
    ModifierKey(ModifierKey),
    /// `platform::mouse_button_monitor`, by AppKit `buttonNumber`.
    MouseButton(u16),
}

/// Who watches this binding.
pub fn watcher_for(binding: &Binding) -> Watcher {
    match binding {
        Binding::Keyboard { shortcut } => match bare_modifier(shortcut) {
            Some(key) => Watcher::ModifierKey(key),
            None => Watcher::GlobalShortcut,
        },
        Binding::Mouse { button } => Watcher::MouseButton(*button),
    }
}

impl Binding {
    /// Human label for logs and conflict messages.
    pub fn describe(&self) -> String {
        match self {
            // A bare modifier gets its spoken name, because "Fn (globe)" is
            // what a person would recognise in "that is already in use".
            Binding::Keyboard { shortcut } => bare_modifier(shortcut)
                .map(|key| key.label().to_string())
                .unwrap_or_else(|| shortcut.clone()),
            Binding::Mouse { button } => match button {
                0 => "Left Click".to_string(),
                1 => "Right Click".to_string(),
                2 => "Middle Click".to_string(),
                n => format!("Mouse Button {}", n + 1),
            },
        }
    }
}

fn default_true() -> bool {
    true
}

/// A single activation trigger.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Trigger {
    pub method: TriggerMethod,
    pub target: TriggerTarget,
    /// Binding for `PushToTalk` / `Toggle`. Always `None` for `Voice`.
    #[serde(default)]
    pub binding: Option<Binding>,
    /// Wake phrase for `Voice` (e.g. `"juno"`, `"transcribe"`). `None` for
    /// key/mouse methods. Stored lowercase without the "hey" prefix.
    #[serde(default)]
    pub phrase: Option<String>,
    /// Voice only: require the phrase to be prefixed with "hey" (the grayed
    /// toggle in the UI). Ignored for non-voice methods.
    #[serde(default)]
    pub require_hey_prefix: bool,
    /// A disabled trigger is kept in the list but not registered.
    #[serde(default = "default_true")]
    pub enabled: bool,
}

impl Trigger {
    /// The uniqueness key. At most one trigger per `(method, target)` pair.
    pub fn key(&self) -> (TriggerMethod, TriggerTarget) {
        (self.method, self.target)
    }

    /// The same key as a string, spelled `"<method>:<target>"`.
    ///
    /// This is the name the settings window uses to say which row it is
    /// editing, so a check that runs while someone is typing can tell "this
    /// combo is already mine" from "this combo belongs to another row".
    pub fn key_str(&self) -> String {
        format!("{}:{}", self.method.as_str(), self.target.as_str())
    }

    /// How this row reads on screen, e.g. `"Push to talk to Dictation"`.
    pub fn label(&self) -> String {
        format!("{} to {}", self.method.label(), self.target.label())
    }

    pub fn is_voice(&self) -> bool {
        self.method == TriggerMethod::Voice
    }

    /// The phrases this voice trigger listens for, lowercased. A prefix-required
    /// trigger listens only for `"hey <phrase>"`; otherwise it listens for both
    /// the bare phrase and its "hey" form so either wording works.
    pub fn voice_phrases(&self) -> Vec<String> {
        let Some(phrase) = self.phrase.as_ref().map(|p| p.trim().to_lowercase()) else {
            return Vec::new();
        };
        if phrase.is_empty() {
            return Vec::new();
        }
        let hey = format!("hey {phrase}");
        if self.require_hey_prefix {
            vec![hey]
        } else {
            vec![phrase, hey]
        }
    }
}

/// Every phrase the always-listening engine should be listening for.
///
/// This is the whole rule for whether voice is on: if it yields nothing, the
/// engine is stopped; otherwise it runs with these as its wake words. Voice is
/// a trigger method like push-to-talk, so the answer comes from the trigger
/// list and nowhere else.
pub fn voice_phrases_for(triggers: &[Trigger]) -> Vec<String> {
    triggers
        .iter()
        .filter(|t| t.enabled && t.is_voice())
        .flat_map(|t| t.voice_phrases())
        .collect()
}

/// The default trigger set for a fresh install: mirrors Juno's historical
/// defaults so the app is never left with no way to be summoned.
/// - Toggle -> Agent on `Option+D` (was `agent_mode`, agent trigger `tap`)
/// - Push-to-talk -> Dictation on `Option+Space` (was `dictation_input`, dictation trigger `hold`)
pub fn default_triggers(agent_combo: &str, dictation_combo: &str) -> Vec<Trigger> {
    vec![
        Trigger {
            method: TriggerMethod::Toggle,
            target: TriggerTarget::Agent,
            binding: Some(Binding::Keyboard {
                shortcut: agent_combo.to_string(),
            }),
            phrase: None,
            require_hey_prefix: false,
            enabled: true,
        },
        Trigger {
            method: TriggerMethod::PushToTalk,
            target: TriggerTarget::Dictation,
            binding: Some(Binding::Keyboard {
                shortcut: dictation_combo.to_string(),
            }),
            phrase: None,
            require_hey_prefix: false,
            enabled: true,
        },
    ]
}

/// Drop duplicates by `(method, target)`, keeping the first occurrence, so the
/// bounded-matrix invariant holds even if a hand-edited store violates it.
pub fn dedupe_by_key(triggers: Vec<Trigger>) -> Vec<Trigger> {
    let mut seen = std::collections::HashSet::new();
    triggers
        .into_iter()
        .filter(|t| seen.insert(t.key()))
        .collect()
}

/// Map a legacy trigger-mode string (`"hold"` / `"tap"`) to a method.
fn method_from_mode(mode: &str) -> TriggerMethod {
    if mode.eq_ignore_ascii_case("hold") {
        TriggerMethod::PushToTalk
    } else {
        TriggerMethod::Toggle
    }
}

/// Build the trigger list from Juno's legacy settings fields. Used once, when a
/// store predates the unified model, so an upgrading user keeps their setup.
///
/// Historical always-listening always routed to the agent, so a migrated voice
/// trigger targets [`TriggerTarget::Agent`]. All configured wake words collapse
/// into that single trigger's phrase (the bounded matrix allows only one
/// voice->agent); the user can refine wording in the new UI.
pub fn migrate_from_legacy(
    agent_combo: &str,
    agent_mode: &str,
    dictation_combo: &str,
    dictation_mode: &str,
    always_listening_active: bool,
    wake_words: &[String],
) -> Vec<Trigger> {
    let mut triggers = vec![
        Trigger {
            method: method_from_mode(agent_mode),
            target: TriggerTarget::Agent,
            binding: Some(Binding::Keyboard {
                shortcut: agent_combo.to_string(),
            }),
            phrase: None,
            require_hey_prefix: false,
            enabled: true,
        },
        Trigger {
            method: method_from_mode(dictation_mode),
            target: TriggerTarget::Dictation,
            binding: Some(Binding::Keyboard {
                shortcut: dictation_combo.to_string(),
            }),
            phrase: None,
            require_hey_prefix: false,
            enabled: true,
        },
    ];

    if always_listening_active {
        // Reconstruct a single voice phrase from the historical wake-word list:
        // strip any "hey " prefix, take the first meaningful word, and require
        // the prefix only if every configured word carried it.
        let cleaned: Vec<String> = wake_words
            .iter()
            .map(|w| w.trim().to_lowercase())
            .filter(|w| !w.is_empty())
            .collect();
        let phrase = cleaned
            .iter()
            .map(|w| w.strip_prefix("hey ").unwrap_or(w).to_string())
            .find(|w| !w.is_empty())
            .unwrap_or_else(|| "juno".to_string());
        let require_hey_prefix =
            !cleaned.is_empty() && cleaned.iter().all(|w| w.starts_with("hey "));
        triggers.push(Trigger {
            method: TriggerMethod::Voice,
            target: TriggerTarget::Agent,
            binding: None,
            phrase: Some(phrase),
            require_hey_prefix,
            enabled: true,
        });
    }

    dedupe_by_key(triggers)
}

/// Project the trigger list back onto the legacy fields so peripheral consumers
/// (onboarding hints, tray labels, the always-listening controller) stay
/// coherent. Best-effort and keyboard-only: a mouse/voice primary trigger
/// leaves the legacy keyboard string untouched. Returns the derived pieces:
/// `(agent_combo, agent_mode, dictation_combo, dictation_mode,
/// always_listening_active, wake_words)`.
#[allow(clippy::type_complexity)]
pub fn derive_legacy(
    triggers: &[Trigger],
    prev_agent_combo: &str,
    prev_dictation_combo: &str,
) -> (String, String, String, String, bool, Vec<String>) {
    let find = |method_is_key: bool, target: TriggerTarget| {
        triggers
            .iter()
            .filter(|t| t.enabled && t.target == target)
            .find(|t| {
                if method_is_key {
                    matches!(t.method, TriggerMethod::PushToTalk | TriggerMethod::Toggle)
                } else {
                    t.is_voice()
                }
            })
    };

    let mode_str = |t: &Trigger| {
        if t.method == TriggerMethod::PushToTalk {
            "hold".to_string()
        } else {
            "tap".to_string()
        }
    };
    // A bare modifier is a keyboard binding, but it is not a combo string: the
    // legacy fields are fed to `parse_shortcut_string`, which knows nothing of
    // Fn, and to key caps that draw one modifier plus one key. So it falls
    // through to the previous value, the same way a mouse button does.
    let keyboard_combo = |t: &Trigger, fallback: &str| match &t.binding {
        Some(Binding::Keyboard { shortcut }) if bare_modifier(shortcut).is_none() => {
            shortcut.clone()
        }
        _ => fallback.to_string(),
    };

    let agent = find(true, TriggerTarget::Agent);
    let dictation = find(true, TriggerTarget::Dictation);

    let agent_combo = agent
        .map(|t| keyboard_combo(t, prev_agent_combo))
        .unwrap_or_else(|| prev_agent_combo.to_string());
    let agent_mode = agent.map(mode_str).unwrap_or_else(|| "tap".to_string());
    let dictation_combo = dictation
        .map(|t| keyboard_combo(t, prev_dictation_combo))
        .unwrap_or_else(|| prev_dictation_combo.to_string());
    let dictation_mode = dictation
        .map(mode_str)
        .unwrap_or_else(|| "hold".to_string());

    let voice_triggers: Vec<&Trigger> = triggers
        .iter()
        .filter(|t| t.enabled && t.is_voice())
        .collect();
    let always_listening_active = !voice_triggers.is_empty();
    let mut wake_words: Vec<String> = voice_triggers
        .iter()
        .flat_map(|t| t.voice_phrases())
        .collect();
    wake_words.dedup();

    (
        agent_combo,
        agent_mode,
        dictation_combo,
        dictation_mode,
        always_listening_active,
        wake_words,
    )
}

/// Turn off any key or mouse trigger that has no binding recorded yet.
///
/// A freshly added row is allowed to persist unconfigured, so the work of
/// adding it is not lost while someone goes to find the key they want. What is
/// not allowed is for that row to claim it is on. An enabled trigger with no
/// binding can never fire, so its switch was telling the person something that
/// could not become true: the row said "on" and the key did nothing, forever.
/// The row survives, the switch tells the truth, and binding a key is what
/// turns it on.
pub fn disable_unbound(triggers: &mut [Trigger]) {
    for t in triggers.iter_mut() {
        let needs_binding = matches!(t.method, TriggerMethod::PushToTalk | TriggerMethod::Toggle);
        if needs_binding && t.enabled && t.binding.is_none() {
            t.enabled = false;
        }
    }
}

/// Switch on a key or mouse trigger that has just had its first binding recorded.
///
/// This is the other half of [`disable_unbound`]'s contract. That function ends
/// "binding a key is what turns it on", and nothing carried it out: a row added
/// from the "+" menu persists unbound, so it is switched off, and recording a
/// key left it switched off. The person was then looking at a row that named
/// their key, drawn greyed out, doing nothing, with no hint that a second and
/// unrelated-looking gesture was still owed. Nobody binds a key in order to
/// leave it off.
///
/// It bit the globe key hardest. Fn cannot be tried out to see whether it took,
/// because a trigger that is off is never registered, so "it does not work" and
/// "it was never set" look identical from the outside.
///
/// Only the None -> Some edge counts. Changing which key is bound on a row
/// somebody deliberately switched off leaves it off, because that is a change
/// of which key, not a decision to start using it.
pub fn enable_newly_bound(previous: &[Trigger], next: &mut [Trigger]) {
    for t in next.iter_mut() {
        if t.is_voice() || t.enabled || t.binding.is_none() {
            continue;
        }
        // A row that did not exist a moment ago and arrives already bound was
        // bound by whatever created it, so it counts as newly bound too.
        let was_unbound = previous
            .iter()
            .find(|p| p.key() == t.key())
            .map(|p| p.binding.is_none())
            .unwrap_or(true);
        if was_unbound {
            t.enabled = true;
        }
    }
}

/// Validate a trigger list before it is persisted. Returns a human-readable
/// error the UI can show inline on the offending row.
///
/// Rules:
/// - No two enabled key/mouse triggers may share the same binding.
/// - An enabled voice trigger must have a non-blank phrase.
///
/// A key/mouse trigger with no binding is allowed through and is switched off
/// by [`disable_unbound`] instead, so adding a row is not lost work.
///
/// `reserved` are binding descriptions already owned by the two fixed utility
/// shortcuts (Escape to stop, Cmd+Comma to open settings); an enabled keyboard
/// trigger may not collide with one.
/// Would binding `combo` to the row named `editing_key` collide with anything?
///
/// Returns the sentence the save would fail with, so the hint shown while
/// someone is still typing and the result of pressing Save cannot disagree.
/// `editing_key` is a [`Trigger::key_str`]; passing the row being edited is
/// what keeps a row from reporting a conflict with its own current binding.
pub fn combo_conflict(
    triggers: &[Trigger],
    combo: &str,
    editing_key: Option<&str>,
    reserved: &[String],
) -> Option<String> {
    let label = Binding::Keyboard {
        shortcut: combo.to_string(),
    }
    .describe();

    if reserved.iter().any(|r| r.eq_ignore_ascii_case(&label)) {
        return Some(format!("\"{label}\" is already used by another shortcut."));
    }

    triggers
        .iter()
        .filter(|t| t.enabled)
        .filter(|t| editing_key.is_none_or(|k| t.key_str() != k))
        .find(|t| {
            t.binding
                .as_ref()
                .is_some_and(|b| b.describe().eq_ignore_ascii_case(&label))
        })
        .map(|t| format!("\"{label}\" is already bound to {}.", t.label()))
}

pub fn validate(triggers: &[Trigger], reserved: &[String]) -> Result<(), String> {
    use std::collections::HashSet;
    let mut seen_bindings: HashSet<String> = HashSet::new();

    for t in triggers.iter().filter(|t| t.enabled) {
        match t.method {
            TriggerMethod::Voice => {
                if t.phrase
                    .as_ref()
                    .map(|p| p.trim().is_empty())
                    .unwrap_or(true)
                {
                    return Err("Voice triggers need a wake phrase.".to_string());
                }
            }
            TriggerMethod::PushToTalk | TriggerMethod::Toggle => {
                // A freshly added trigger has no binding yet. That is allowed to
                // persist (the UI shows it as unconfigured); it simply is not
                // registered until a key or mouse button is recorded. Only
                // validate a binding that actually exists.
                let Some(binding) = t.binding.as_ref() else {
                    continue;
                };
                let label = binding.describe();
                if reserved.iter().any(|r| r.eq_ignore_ascii_case(&label)) {
                    return Err(format!("\"{label}\" is already used by another shortcut."));
                }
                if !seen_bindings.insert(label.to_lowercase()) {
                    return Err(format!("\"{label}\" is bound to more than one trigger."));
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn voice(phrase: &str, target: TriggerTarget, enabled: bool) -> Trigger {
        Trigger {
            method: TriggerMethod::Voice,
            target,
            binding: None,
            phrase: Some(phrase.to_string()),
            require_hey_prefix: false,
            enabled,
        }
    }

    #[test]
    fn a_row_does_not_conflict_with_its_own_binding() {
        // The bug this covers: every trigger row reported "already assigned"
        // against the combo it was already holding, which disabled Save and
        // made both activation shortcuts impossible to rebind.
        let ts = vec![key_trigger(
            TriggerTarget::Dictation,
            Some(Binding::Keyboard {
                shortcut: "Option+Space".to_string(),
            }),
            true,
        )];
        let me = ts[0].key_str();
        assert_eq!(
            combo_conflict(&ts, "Option+Space", Some(&me), &[]),
            None,
            "a row must be allowed to keep the combo it already has"
        );
    }

    #[test]
    fn another_rows_binding_still_conflicts() {
        let ts = vec![key_trigger(
            TriggerTarget::Agent,
            Some(Binding::Keyboard {
                shortcut: "Option+Space".to_string(),
            }),
            true,
        )];
        let other = Trigger {
            method: TriggerMethod::Toggle,
            target: TriggerTarget::Dictation,
            binding: None,
            phrase: None,
            require_hey_prefix: false,
            enabled: true,
        }
        .key_str();
        let msg = combo_conflict(&ts, "Option+Space", Some(&other), &[])
            .expect("a combo held by another enabled row is taken");
        assert!(msg.contains("Push to talk to Agent"), "got: {msg}");
    }

    #[test]
    fn a_switched_off_row_does_not_hold_its_combo() {
        // A disabled trigger is not registered, so its combo is free. Saying
        // otherwise would strand a key behind a row that does nothing.
        let ts = vec![key_trigger(
            TriggerTarget::Agent,
            Some(Binding::Keyboard {
                shortcut: "Option+Space".to_string(),
            }),
            false,
        )];
        assert_eq!(combo_conflict(&ts, "Option+Space", None, &[]), None);
    }

    #[test]
    fn the_reserved_combos_are_never_available() {
        let reserved = vec!["Escape".to_string(), "Cmd+Comma".to_string()];
        assert!(combo_conflict(&[], "Escape", None, &reserved).is_some());
        assert!(combo_conflict(&[], "cmd+comma", None, &reserved).is_some());
        assert_eq!(combo_conflict(&[], "Option+Space", None, &reserved), None);
    }

    #[test]
    fn a_rows_key_string_matches_what_the_settings_window_sends() {
        // The settings window builds `trigger_${method}:${target}` from the
        // serde names. If these drift, the exclusion silently stops matching
        // and the self-conflict bug comes straight back.
        let t = key_trigger(TriggerTarget::Dictation, None, true);
        assert_eq!(t.key_str(), "push_to_talk:dictation");
    }

    #[test]
    fn an_enabled_voice_trigger_is_what_turns_listening_on() {
        let t = vec![voice("juno", TriggerTarget::Agent, true)];
        assert_eq!(voice_phrases_for(&t), vec!["juno", "hey juno"]);
    }

    #[test]
    fn a_disabled_voice_trigger_listens_for_nothing() {
        // Nothing to listen for means the engine is stopped, so the switch on
        // the row is the whole control.
        let t = vec![voice("juno", TriggerTarget::Agent, false)];
        assert!(voice_phrases_for(&t).is_empty());
    }

    #[test]
    fn key_triggers_do_not_turn_listening_on() {
        let t = default_triggers("Option+Space", "Option+D");
        assert!(voice_phrases_for(&t).is_empty());
    }

    #[test]
    fn voice_can_target_dictation_as_well_as_the_agent() {
        // Voice is a method, not a feature of one target.
        let t = vec![
            voice("juno", TriggerTarget::Agent, true),
            voice("transcribe", TriggerTarget::Dictation, true),
        ];
        let phrases = voice_phrases_for(&t);
        assert!(phrases.contains(&"juno".to_string()));
        assert!(phrases.contains(&"transcribe".to_string()));
    }

    #[test]
    fn validate_rejects_duplicate_bindings() {
        let mk = |target: TriggerTarget| Trigger {
            method: if target == TriggerTarget::Agent {
                TriggerMethod::Toggle
            } else {
                TriggerMethod::PushToTalk
            },
            target,
            binding: Some(Binding::Keyboard {
                shortcut: "Option+Space".to_string(),
            }),
            phrase: None,
            require_hey_prefix: false,
            enabled: true,
        };
        let ts = vec![mk(TriggerTarget::Agent), mk(TriggerTarget::Dictation)];
        assert!(validate(&ts, &[]).is_err());
    }

    #[test]
    fn disable_unbound_turns_off_a_key_trigger_with_no_binding() {
        let mut ts = vec![
            Trigger {
                method: TriggerMethod::PushToTalk,
                target: TriggerTarget::Dictation,
                binding: None,
                phrase: None,
                require_hey_prefix: false,
                enabled: true,
            },
            Trigger {
                method: TriggerMethod::Toggle,
                target: TriggerTarget::Agent,
                binding: Some(Binding::Keyboard {
                    shortcut: "Option+Space".to_string(),
                }),
                phrase: None,
                require_hey_prefix: false,
                enabled: true,
            },
        ];
        disable_unbound(&mut ts);
        assert!(!ts[0].enabled, "an unbound key trigger must not read as on");
        assert!(ts[1].enabled, "a bound trigger is left alone");
        assert_eq!(ts.len(), 2, "the unconfigured row still persists");
    }

    #[test]
    fn disable_unbound_leaves_a_voice_trigger_alone() {
        // Voice is armed by its phrase, not by a binding, so a null binding is
        // its normal resting state and says nothing about whether it can fire.
        let mut ts = vec![Trigger {
            method: TriggerMethod::Voice,
            target: TriggerTarget::Agent,
            binding: None,
            phrase: Some("juno".to_string()),
            require_hey_prefix: false,
            enabled: true,
        }];
        disable_unbound(&mut ts);
        assert!(ts[0].enabled);
    }

    #[test]
    fn validate_rejects_reserved_binding() {
        let ts = vec![Trigger {
            method: TriggerMethod::Toggle,
            target: TriggerTarget::Agent,
            binding: Some(Binding::Keyboard {
                shortcut: "Escape".to_string(),
            }),
            phrase: None,
            require_hey_prefix: false,
            enabled: true,
        }];
        assert!(validate(&ts, &["Escape".to_string()]).is_err());
    }

    #[test]
    fn validate_frees_the_retired_voice_activation_combo() {
        // Voice activation used to reserve Option+Shift+V. It was retired, so
        // the reserved list is Escape and Cmd+Comma only and a real trigger
        // can claim the combo it used to sit on.
        let ts = vec![Trigger {
            method: TriggerMethod::Toggle,
            target: TriggerTarget::Agent,
            binding: Some(Binding::Keyboard {
                shortcut: "Option+Shift+V".to_string(),
            }),
            phrase: None,
            require_hey_prefix: false,
            enabled: true,
        }];
        let reserved = vec!["Escape".to_string(), "Cmd+Comma".to_string()];
        assert!(validate(&ts, &reserved).is_ok());
    }

    #[test]
    fn validate_still_protects_the_fixed_utility_shortcuts() {
        let mk = |combo: &str| {
            vec![Trigger {
                method: TriggerMethod::Toggle,
                target: TriggerTarget::Agent,
                binding: Some(Binding::Keyboard {
                    shortcut: combo.to_string(),
                }),
                phrase: None,
                require_hey_prefix: false,
                enabled: true,
            }]
        };
        let reserved = vec!["Escape".to_string(), "Cmd+Comma".to_string()];
        assert!(validate(&mk("Escape"), &reserved).is_err());
        assert!(validate(&mk("Cmd+Comma"), &reserved).is_err());
    }

    #[test]
    fn validate_accepts_default_set() {
        let ts = default_triggers("Option+D", "Option+Space");
        assert!(validate(&ts, &["Escape".to_string()]).is_ok());
    }

    #[test]
    fn validate_allows_unconfigured_binding() {
        // A just-added trigger with no binding must be savable so the "+" flow
        // can persist it before the user records a key/mouse button.
        let ts = vec![Trigger {
            method: TriggerMethod::Toggle,
            target: TriggerTarget::Agent,
            binding: None,
            phrase: None,
            require_hey_prefix: false,
            enabled: true,
        }];
        assert!(validate(&ts, &[]).is_ok());
    }

    #[test]
    fn validate_rejects_blank_voice_phrase() {
        let ts = vec![Trigger {
            method: TriggerMethod::Voice,
            target: TriggerTarget::Agent,
            binding: None,
            phrase: Some("  ".to_string()),
            require_hey_prefix: false,
            enabled: true,
        }];
        assert!(validate(&ts, &[]).is_err());
    }

    #[test]
    fn voice_phrases_without_prefix_match_both_wordings() {
        let t = Trigger {
            method: TriggerMethod::Voice,
            target: TriggerTarget::Agent,
            binding: None,
            phrase: Some("Juno".to_string()),
            require_hey_prefix: false,
            enabled: true,
        };
        assert_eq!(t.voice_phrases(), vec!["juno", "hey juno"]);
    }

    #[test]
    fn voice_phrases_with_prefix_require_hey() {
        let t = Trigger {
            method: TriggerMethod::Voice,
            target: TriggerTarget::Agent,
            binding: None,
            phrase: Some("Juno".to_string()),
            require_hey_prefix: true,
            enabled: true,
        };
        assert_eq!(t.voice_phrases(), vec!["hey juno"]);
    }

    #[test]
    fn voice_phrases_empty_when_blank() {
        let t = Trigger {
            method: TriggerMethod::Voice,
            target: TriggerTarget::Dictation,
            binding: None,
            phrase: Some("   ".to_string()),
            require_hey_prefix: false,
            enabled: true,
        };
        assert!(t.voice_phrases().is_empty());
    }

    #[test]
    fn dedupe_keeps_first_per_method_target() {
        let mk = |target: TriggerTarget, combo: &str| Trigger {
            method: TriggerMethod::Toggle,
            target,
            binding: Some(Binding::Keyboard {
                shortcut: combo.to_string(),
            }),
            phrase: None,
            require_hey_prefix: false,
            enabled: true,
        };
        let deduped = dedupe_by_key(vec![
            mk(TriggerTarget::Agent, "A"),
            mk(TriggerTarget::Agent, "B"), // dup key, dropped
            mk(TriggerTarget::Dictation, "C"),
        ]);
        assert_eq!(deduped.len(), 2);
        assert_eq!(
            deduped[0].binding,
            Some(Binding::Keyboard {
                shortcut: "A".to_string()
            })
        );
    }

    #[test]
    fn default_set_has_agent_and_dictation() {
        let ts = default_triggers("Option+D", "Option+Space");
        assert_eq!(ts.len(), 2);
        assert_eq!(dedupe_by_key(ts.clone()).len(), 2);
        assert!(ts
            .iter()
            .any(|t| t.target == TriggerTarget::Agent && t.method == TriggerMethod::Toggle));
        assert!(
            ts.iter()
                .any(|t| t.target == TriggerTarget::Dictation
                    && t.method == TriggerMethod::PushToTalk)
        );
    }

    /* ---------------------------------------------------------------- */
    /* Recording a key is what turns a row on                           */
    /* ---------------------------------------------------------------- */

    fn key_trigger(target: TriggerTarget, binding: Option<Binding>, enabled: bool) -> Trigger {
        Trigger {
            method: TriggerMethod::PushToTalk,
            target,
            binding,
            phrase: None,
            require_hey_prefix: false,
            enabled,
        }
    }

    #[test]
    fn binding_a_key_to_an_unbound_row_switches_it_on() {
        // The reported defect. Adding a row leaves it off because it cannot
        // fire yet; choosing a key used to leave it off as well, so the row
        // named the key and did nothing.
        let previous = vec![key_trigger(TriggerTarget::Dictation, None, false)];
        let mut next = vec![key_trigger(
            TriggerTarget::Dictation,
            Some(Binding::Keyboard {
                shortcut: "Fn".to_string(),
            }),
            false,
        )];
        enable_newly_bound(&previous, &mut next);
        assert!(next[0].enabled);
    }

    #[test]
    fn rebinding_a_row_somebody_switched_off_leaves_it_off() {
        // Changing which key is bound is not a decision to start using it.
        let previous = vec![key_trigger(
            TriggerTarget::Dictation,
            Some(Binding::Keyboard {
                shortcut: "Option+Space".to_string(),
            }),
            false,
        )];
        let mut next = vec![key_trigger(
            TriggerTarget::Dictation,
            Some(Binding::Keyboard {
                shortcut: "Fn".to_string(),
            }),
            false,
        )];
        enable_newly_bound(&previous, &mut next);
        assert!(!next[0].enabled);
    }

    #[test]
    fn a_row_that_is_still_unbound_is_not_switched_on() {
        let previous = vec![key_trigger(TriggerTarget::Dictation, None, false)];
        let mut next = vec![key_trigger(TriggerTarget::Dictation, None, false)];
        enable_newly_bound(&previous, &mut next);
        assert!(!next[0].enabled);
    }

    #[test]
    fn the_two_switch_rules_do_not_fight() {
        // Run in the order set_triggers runs them: a newly bound row ends on,
        // an unbound one ends off, and neither undoes the other.
        let previous = vec![
            key_trigger(TriggerTarget::Dictation, None, false),
            key_trigger(TriggerTarget::Agent, None, false),
        ];
        let mut next = vec![
            key_trigger(
                TriggerTarget::Dictation,
                Some(Binding::Keyboard {
                    shortcut: "Fn".to_string(),
                }),
                false,
            ),
            key_trigger(TriggerTarget::Agent, None, true),
        ];
        enable_newly_bound(&previous, &mut next);
        disable_unbound(&mut next);
        assert!(next[0].enabled, "the row that just got a key is on");
        assert!(!next[1].enabled, "the row with no key is off");
    }

    #[test]
    fn a_voice_trigger_is_never_switched_on_by_this() {
        // Voice is armed by its phrase and has no binding to record.
        let previous = vec![voice("juno", TriggerTarget::Agent, false)];
        let mut next = vec![voice("juno", TriggerTarget::Agent, false)];
        enable_newly_bound(&previous, &mut next);
        assert!(!next[0].enabled);
    }

    /* ---------------------------------------------------------------- */
    /* Fn is a keyboard key                                             */
    /* ---------------------------------------------------------------- */

    #[test]
    fn a_settings_file_written_before_the_collapse_keeps_its_fn_trigger() {
        // The retired shape, exactly as it sits in a store on disk today. If
        // this ever stops resolving, someone's globe key silently stops
        // working after an update, which is the whole reason it is read.
        let stored = r#"[{
            "method": "push_to_talk",
            "target": "dictation",
            "binding": { "kind": "modifier", "key": "fn" },
            "phrase": null,
            "require_hey_prefix": false,
            "enabled": true
        }]"#;
        let triggers: Vec<Trigger> =
            serde_json::from_str(stored).expect("the old shape must still be readable");
        assert_eq!(
            triggers[0].binding,
            Some(Binding::Keyboard {
                shortcut: "Fn".to_string()
            }),
            "an old modifier binding becomes the keyboard binding it always was"
        );
        assert!(triggers[0].enabled, "and it is still switched on");
    }

    #[test]
    fn the_converted_binding_survives_the_save_that_follows() {
        // Reading is only half of it: the next save writes the new shape, and
        // that has to read back as the same key rather than as a stranger.
        let stored = r#"{ "kind": "modifier", "key": "fn" }"#;
        let read: Binding = serde_json::from_str(stored).expect("old shape");
        let written = serde_json::to_string(&read).expect("serialize");
        assert_eq!(written, r#"{"kind":"keyboard","shortcut":"Fn"}"#);
        let reread: Binding = serde_json::from_str(&written).expect("new shape");
        assert_eq!(read, reread);
    }

    #[test]
    fn nothing_writes_the_retired_shape_any_more() {
        // One live representation. A `modifier` binding can be read and never
        // produced, so the two shapes cannot drift apart in the store.
        let fn_binding = Binding::Keyboard {
            shortcut: "Fn".to_string(),
        };
        let written = serde_json::to_string(&fn_binding).expect("serialize");
        assert!(!written.contains("modifier"));
    }

    #[test]
    fn a_bare_modifier_goes_to_the_modifier_watcher() {
        // The plugin cannot register a key that only ever changes a modifier
        // flag, so the registration layer hands it to the flags-changed
        // monitor. This is the only place that distinction is allowed to live.
        assert_eq!(
            watcher_for(&Binding::Keyboard {
                shortcut: "Fn".to_string()
            }),
            Watcher::ModifierKey(ModifierKey::Fn)
        );
        assert_eq!(
            watcher_for(&Binding::Keyboard {
                shortcut: "globe".to_string()
            }),
            Watcher::ModifierKey(ModifierKey::Fn),
            "Apple calls the same key both things"
        );
    }

    #[test]
    fn an_ordinary_combo_still_goes_to_the_global_shortcut_plugin() {
        assert_eq!(
            watcher_for(&Binding::Keyboard {
                shortcut: "Option+Space".to_string()
            }),
            Watcher::GlobalShortcut
        );
        // A combo that merely mentions the word is not the bare key.
        assert_eq!(
            watcher_for(&Binding::Keyboard {
                shortcut: "Fn+F5".to_string()
            }),
            Watcher::GlobalShortcut
        );
        assert_eq!(
            watcher_for(&Binding::Mouse { button: 3 }),
            Watcher::MouseButton(3)
        );
    }

    #[test]
    fn an_fn_binding_is_valid_and_stays_switched_on() {
        // The reported defect: a trigger bound to Fn could not be enabled. It
        // is a binding like any other, so it passes validation and
        // disable_unbound leaves it alone.
        let mut ts = vec![Trigger {
            method: TriggerMethod::PushToTalk,
            target: TriggerTarget::Dictation,
            binding: Some(Binding::Keyboard {
                shortcut: "Fn".to_string(),
            }),
            phrase: None,
            require_hey_prefix: false,
            enabled: true,
        }];
        disable_unbound(&mut ts);
        assert!(ts[0].enabled);
        assert!(validate(&ts, &["Escape".to_string()]).is_ok());
    }

    #[test]
    fn fn_is_named_the_way_a_person_would_name_it_in_a_conflict() {
        let label = Binding::Keyboard {
            shortcut: "Fn".to_string(),
        }
        .describe();
        assert_eq!(label, "Fn (globe)");
    }

    #[test]
    fn fn_and_a_combo_cannot_be_confused_for_a_duplicate() {
        let ts = vec![
            Trigger {
                method: TriggerMethod::PushToTalk,
                target: TriggerTarget::Dictation,
                binding: Some(Binding::Keyboard {
                    shortcut: "Fn".to_string(),
                }),
                phrase: None,
                require_hey_prefix: false,
                enabled: true,
            },
            Trigger {
                method: TriggerMethod::Toggle,
                target: TriggerTarget::Agent,
                binding: Some(Binding::Keyboard {
                    shortcut: "Option+D".to_string(),
                }),
                phrase: None,
                require_hey_prefix: false,
                enabled: true,
            },
        ];
        assert!(validate(&ts, &[]).is_ok());
    }

    #[test]
    fn derive_legacy_keeps_previous_combo_for_an_fn_binding() {
        // The legacy fields are parsed as combos and drawn as key caps, and
        // "Fn" is neither. It falls through the same way a mouse button does.
        let ts = vec![Trigger {
            method: TriggerMethod::PushToTalk,
            target: TriggerTarget::Dictation,
            binding: Some(Binding::Keyboard {
                shortcut: "Fn".to_string(),
            }),
            phrase: None,
            require_hey_prefix: false,
            enabled: true,
        }];
        let (_, _, d_combo, d_mode, ..) = derive_legacy(&ts, "kept+a", "kept+d");
        assert_eq!(d_combo, "kept+d");
        assert_eq!(d_mode, "hold");
    }

    #[test]
    fn mouse_binding_describes_side_buttons_one_indexed() {
        assert_eq!(Binding::Mouse { button: 3 }.describe(), "Mouse Button 4");
        assert_eq!(Binding::Mouse { button: 2 }.describe(), "Middle Click");
    }

    #[test]
    fn migrate_maps_modes_to_methods() {
        let ts = migrate_from_legacy("Option+D", "tap", "Option+Space", "hold", false, &[]);
        assert_eq!(ts.len(), 2);
        let agent = ts
            .iter()
            .find(|t| t.target == TriggerTarget::Agent)
            .unwrap();
        assert_eq!(agent.method, TriggerMethod::Toggle); // tap -> toggle
        let dictation = ts
            .iter()
            .find(|t| t.target == TriggerTarget::Dictation)
            .unwrap();
        assert_eq!(dictation.method, TriggerMethod::PushToTalk); // hold -> ptt
        assert_eq!(
            dictation.binding,
            Some(Binding::Keyboard {
                shortcut: "Option+Space".to_string()
            })
        );
    }

    #[test]
    fn migrate_reconstructs_voice_trigger_when_always_listening() {
        let words = vec!["hey juno".to_string(), "hey computer".to_string()];
        let ts = migrate_from_legacy("Option+D", "tap", "Option+Space", "hold", true, &words);
        let voice = ts.iter().find(|t| t.is_voice()).expect("voice trigger");
        assert_eq!(voice.target, TriggerTarget::Agent);
        assert_eq!(voice.phrase.as_deref(), Some("juno"));
        assert!(voice.require_hey_prefix); // every word had the prefix
    }

    #[test]
    fn derive_legacy_roundtrips_keyboard_and_modes() {
        let ts = default_triggers("Option+D", "Option+Space");
        let (a_combo, a_mode, d_combo, d_mode, al, words) = derive_legacy(&ts, "old+a", "old+d");
        assert_eq!(a_combo, "Option+D");
        assert_eq!(a_mode, "tap"); // Toggle
        assert_eq!(d_combo, "Option+Space");
        assert_eq!(d_mode, "hold"); // PushToTalk
        assert!(!al);
        assert!(words.is_empty());
    }

    #[test]
    fn derive_legacy_keeps_previous_combo_for_mouse_binding() {
        let ts = vec![Trigger {
            method: TriggerMethod::PushToTalk,
            target: TriggerTarget::Agent,
            binding: Some(Binding::Mouse { button: 3 }),
            phrase: None,
            require_hey_prefix: false,
            enabled: true,
        }];
        let (a_combo, a_mode, ..) = derive_legacy(&ts, "kept+combo", "kept+d");
        assert_eq!(a_combo, "kept+combo"); // mouse can't be a keyboard string
        assert_eq!(a_mode, "hold");
    }
}
