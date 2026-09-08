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

use serde::{Deserialize, Serialize};

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

/// The physical input bound to a key/mouse method.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Binding {
    /// A keyboard combo string parseable by [`crate::shortcuts::parse_shortcut_string`],
    /// e.g. `"Option+Space"`.
    Keyboard { shortcut: String },
    /// A mouse button by AppKit `buttonNumber` (0 = left, 1 = right, 2 = middle,
    /// 3+ = extra side buttons). Watched by the passive input monitor, not the
    /// global-shortcut plugin.
    Mouse { button: u16 },
}

impl Binding {
    /// Human label for logs and conflict messages.
    pub fn describe(&self) -> String {
        match self {
            Binding::Keyboard { shortcut } => shortcut.clone(),
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
    let keyboard_combo = |t: &Trigger, fallback: &str| match &t.binding {
        Some(Binding::Keyboard { shortcut }) => shortcut.clone(),
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

/// Validate a trigger list before it is persisted. Returns a human-readable
/// error the UI can show inline on the offending row.
///
/// Rules:
/// - No two enabled key/mouse triggers may share the same binding.
/// - A key/mouse trigger that is enabled must actually have a binding.
/// - An enabled voice trigger must have a non-blank phrase.
///
/// `reserved` are binding descriptions already owned by non-trigger shortcuts
/// (stop, open-settings, voice-activation); an enabled keyboard trigger may not
/// collide with one.
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
