//! # Triggers: one gesture, one target, one row
//!
//! One list describes every way the user can summon Juno. A [`Trigger`] pairs a
//! [`Gesture`] (how it fires) with a [`TriggerTarget`] (what it does), so the
//! two axes that used to live in three different settings screens (the key
//! combo, the tap/hold mode, and the always-listening wake words) collapse into
//! a single object that reads as a sentence: *Hold the globe key to talk to
//! Juno*.
//!
//! Every gesture is a trigger in its own right. A double tap is not something
//! bolted onto a hold key as a second way to reach the same thing; it is
//! another row, with its own key and its own target, which the person can see
//! and delete. The set is therefore unbounded: a row is its `id` and nothing
//! else, so two rows may share a gesture, a target, or both.
//!
//! Which gestures may live on one key is the table in [`can_share_key`], and
//! [`validate`] enforces that table and nothing looser.
//!
//! `triggers` is the source of truth for the settings UI and for shortcut
//! registration. The legacy `KeyboardShortcuts` / `AgentSettings.trigger_mode` /
//! `AudioSettings` wake-word fields are *derived* from it (see
//! [`derive_legacy`]) so existing runtime consumers keep working without a
//! codebase-wide rewrite.

use serde::{Deserialize, Deserializer, Serialize};

/// How a trigger fires.
///
/// Five independent gestures. The wire names of three of them keep the words
/// the old model used as aliases (`push_to_talk`, `toggle`, `voice`), because a
/// store written before gestures existed has to keep working.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Gesture {
    /// Down edge starts it, release ends it.
    #[serde(alias = "push_to_talk")]
    Hold,
    /// The release of a press starts it, the next press of the same key ends it.
    #[serde(alias = "toggle")]
    Tap,
    /// A second press inside
    /// [`DOUBLE_TAP_WINDOW_MS`](crate::constants::monitor_sessions::DOUBLE_TAP_WINDOW_MS)
    /// starts it, the next press of the same key ends it.
    DoubleTap,
    /// A second press still held at
    /// [`SECOND_PRESS_HOLD_MS`](crate::constants::monitor_sessions::SECOND_PRESS_HOLD_MS)
    /// starts it, release ends it.
    DoubleTapHold,
    /// A wake phrase starts it, the end of speech ends it. No key.
    #[serde(alias = "voice")]
    Say,
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

impl Gesture {
    /// Every gesture, in the order the settings menu offers them.
    pub const ALL: [Gesture; 5] = [
        Gesture::Hold,
        Gesture::Tap,
        Gesture::DoubleTap,
        Gesture::DoubleTapHold,
        Gesture::Say,
    ];

    /// The serde name. Both sides of the IPC boundary spell it this way.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Hold => "hold",
            Self::Tap => "tap",
            Self::DoubleTap => "double_tap",
            Self::DoubleTapHold => "double_tap_hold",
            Self::Say => "say",
        }
    }

    /// The first word of the row's sentence.
    pub fn label(self) -> &'static str {
        match self {
            Self::Hold => "Hold",
            Self::Tap => "Tap",
            Self::DoubleTap => "Double-tap",
            Self::DoubleTapHold => "Double-tap and hold",
            Self::Say => "Say",
        }
    }

    /// How this gesture ends, in the person's words.
    ///
    /// Generated from the gesture, so the line beside a row can only ever
    /// describe what that row does. The paragraph it replaced described a
    /// second gesture reaching the same target, which is the behaviour this
    /// model deleted.
    pub fn ending(self) -> &'static str {
        match self {
            Self::Hold | Self::DoubleTapHold => "Let go to finish.",
            Self::Tap | Self::DoubleTap => "Press the key again to finish.",
            Self::Say => "Juno starts listening when it hears the phrase.",
        }
    }

    /// Does this gesture need a key or mouse button recorded against it?
    pub fn needs_binding(self) -> bool {
        self != Self::Say
    }
}

impl TriggerTarget {
    /// Every target, in the order the settings menu offers them.
    pub const ALL: [TriggerTarget; 2] = [TriggerTarget::Agent, TriggerTarget::Dictation];

    /// The serde name. See [`Gesture::as_str`].
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Agent => "agent",
            Self::Dictation => "dictation",
        }
    }

    /// How the row names itself on screen, as the outcome the gesture has.
    pub fn label(self) -> &'static str {
        match self {
            Self::Agent => "to talk to Juno",
            Self::Dictation => "to dictate",
        }
    }
}

/// Can these two gestures live on the same key?
///
/// The table, which [`validate`] and [`combo_conflict`] enforce and nothing
/// looser:
///
/// | On one key | Allowed |
/// |---|---|
/// | Hold + Double tap | yes |
/// | Hold + Double tap and hold | yes |
/// | Double tap + Double tap and hold | yes |
/// | Hold + Double tap + Double tap and hold | yes |
/// | Tap + Double tap and hold | yes |
/// | Hold + Tap | no |
/// | Tap + Double tap | no |
///
/// Tap cannot share a key with Hold or Double tap. A short tap on a hold key is
/// usually a fumbled hold, and a quick stop-tap on a tap key looks exactly like
/// the first half of a double tap. Every other mix is allowed, and each gesture
/// appears at most once per key, including a gesture pointing at the other
/// target: one key cannot mean two things on the same edge.
pub fn can_share_key(a: Gesture, b: Gesture) -> bool {
    use Gesture::*;
    // Say has no key to share.
    if a == Say || b == Say {
        return false;
    }
    // Each gesture appears at most once per key.
    if a == b {
        return false;
    }
    !matches!(
        (a, b),
        (Tap, Hold) | (Hold, Tap) | (Tap, DoubleTap) | (DoubleTap, Tap)
    )
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

/// The physical input bound to a key/mouse gesture.
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

    /// The identity of the physical input, for grouping gestures by key.
    ///
    /// Derived from [`Binding::describe`] so "the same key" means the same
    /// thing to the recognizer, to the registration path and to a conflict
    /// message. `Fn` and `globe` are one key; so are `Option+Space` and
    /// `option+space`.
    pub fn signature(&self) -> String {
        self.describe().to_lowercase()
    }
}

fn default_true() -> bool {
    true
}

/// A single activation trigger: one gesture, one key, one target.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Trigger {
    /// Stable row identity, generated when a stored row has none (see
    /// [`ensure_ids`]).
    ///
    /// A row is this id and nothing else. The old model's identity was
    /// `(method, target)`, which is exactly why a double tap had to be bolted
    /// onto a hold instead of being a row: there was no room for a second row
    /// with the same pair.
    #[serde(default)]
    pub id: String,
    /// The gesture. `method` is the name the old model used, kept as an alias
    /// so a store written before this loads.
    #[serde(alias = "method")]
    pub gesture: Gesture,
    pub target: TriggerTarget,
    /// Binding for every gesture but [`Gesture::Say`], which has no key.
    #[serde(default)]
    pub binding: Option<Binding>,
    /// Wake phrase for [`Gesture::Say`] (e.g. `"juno"`, `"transcribe"`).
    /// `None` for key/mouse gestures. Stored lowercase without the "hey"
    /// prefix.
    #[serde(default)]
    pub phrase: Option<String>,
    /// Say only: require the phrase to be prefixed with "hey" (the grayed
    /// toggle in the UI). Ignored for other gestures.
    #[serde(default)]
    pub require_hey_prefix: bool,
    /// A disabled trigger is kept in the list but not registered.
    #[serde(default = "default_true")]
    pub enabled: bool,
}

impl Trigger {
    /// How this row reads on screen, e.g. `"Hold to dictate"`. Conflict
    /// messages quote it, so a refusal names the row in the same words the
    /// window does.
    pub fn label(&self) -> String {
        format!("{} {}", self.gesture.label(), self.target.label())
    }

    pub fn is_voice(&self) -> bool {
        self.gesture == Gesture::Say
    }

    /// The physical input this row is bound to, if any. See
    /// [`Binding::signature`].
    pub fn key_signature(&self) -> Option<String> {
        self.binding.as_ref().map(|b| b.signature())
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

/// Which gestures are bound to one key, and what each one does.
///
/// The recognizer's whole view of a key. It asks "what can this key mean" and
/// gets an answer per gesture, which is what makes each gesture an independent
/// trigger with its own target rather than a mode of one trigger.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct KeyGestures {
    pub hold: Option<TriggerTarget>,
    pub tap: Option<TriggerTarget>,
    pub double_tap: Option<TriggerTarget>,
    pub double_tap_hold: Option<TriggerTarget>,
}

impl KeyGestures {
    pub fn is_empty(&self) -> bool {
        self.hold.is_none()
            && self.tap.is_none()
            && self.double_tap.is_none()
            && self.double_tap_hold.is_none()
    }

    /// Does any gesture on this key wait for a second press?
    ///
    /// The recognizer only opens its double-tap window when the answer is yes,
    /// so a key with a single gesture has no hidden second meaning.
    pub fn has_double(&self) -> bool {
        self.double_tap.is_some() || self.double_tap_hold.is_some()
    }

    /// Every target this key can reach, for the visual-feedback events the
    /// onboarding screen listens to.
    pub fn targets(&self) -> Vec<TriggerTarget> {
        let mut out = Vec::new();
        for target in [self.hold, self.tap, self.double_tap, self.double_tap_hold]
            .into_iter()
            .flatten()
        {
            if !out.contains(&target) {
                out.push(target);
            }
        }
        out
    }
}

/// The gestures bound to one physical input, from the enabled rows.
pub fn gestures_on_key(triggers: &[Trigger], signature: &str) -> KeyGestures {
    let mut out = KeyGestures::default();
    for t in triggers.iter().filter(|t| t.enabled) {
        if t.key_signature().as_deref() != Some(signature) {
            continue;
        }
        let slot = match t.gesture {
            Gesture::Hold => &mut out.hold,
            Gesture::Tap => &mut out.tap,
            Gesture::DoubleTap => &mut out.double_tap,
            Gesture::DoubleTapHold => &mut out.double_tap_hold,
            // Say never has a binding, so it can never land here.
            Gesture::Say => continue,
        };
        // First row wins, which is the same rule the validator enforces: a
        // second row claiming the same gesture on the same key is refused on
        // save, so this only matters for a hand-edited store.
        if slot.is_none() {
            *slot = Some(t.target);
        }
    }
    out
}

/// Every distinct physical input the enabled rows bind, each once.
///
/// A key used by several rows is registered once; the recognizer resolves
/// which of its gestures fired.
pub fn bound_keys(triggers: &[Trigger]) -> Vec<Binding> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for t in triggers.iter().filter(|t| t.enabled) {
        let Some(binding) = t.binding.as_ref() else {
            continue;
        };
        if seen.insert(binding.signature()) {
            out.push(binding.clone());
        }
    }
    out
}

/// Every phrase the always-listening engine should be listening for.
///
/// This is the whole rule for whether voice is on: if it yields nothing, the
/// engine is stopped; otherwise it runs with these as its wake words. Say is a
/// gesture like Hold, so the answer comes from the trigger list and nowhere
/// else.
pub fn voice_phrases_for(triggers: &[Trigger]) -> Vec<String> {
    triggers
        .iter()
        .filter(|t| t.enabled && t.is_voice())
        .flat_map(|t| t.voice_phrases())
        .collect()
}

/// Build one row, with a fresh id.
pub fn trigger(gesture: Gesture, target: TriggerTarget, binding: Option<Binding>) -> Trigger {
    Trigger {
        id: new_id(),
        gesture,
        target,
        binding,
        phrase: None,
        require_hey_prefix: false,
        enabled: true,
    }
}

/// A fresh row identity.
pub fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

/// The shortcut string the globe key is recorded as.
pub const GLOBE_SHORTCUT: &str = "Fn";

/// The default trigger set for a fresh install.
///
/// Two rows, because two is what a new install needs to be usable and anything
/// more is setup nobody asked for:
/// - Hold the globe key to talk to Juno. One key, under the thumb, nothing to
///   chord.
/// - Hold Option+Space to dictate.
///
/// Both are Hold, so neither key carries a second meaning on a single tap.
pub fn default_triggers() -> Vec<Trigger> {
    vec![
        trigger(
            Gesture::Hold,
            TriggerTarget::Agent,
            Some(Binding::Keyboard {
                shortcut: GLOBE_SHORTCUT.to_string(),
            }),
        ),
        trigger(
            Gesture::Hold,
            TriggerTarget::Dictation,
            Some(Binding::Keyboard {
                shortcut: "Option+Space".to_string(),
            }),
        ),
    ]
}

/// Give every row a unique, non-blank id, leaving the ones it already has.
///
/// A blank id means a row the UI has just created; a repeated id means a
/// hand-edited or duplicated store. Both get a fresh one, because the whole
/// model rests on a row being identifiable.
pub fn ensure_ids(triggers: &mut [Trigger]) {
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    for t in triggers.iter_mut() {
        let id = t.id.trim().to_string();
        if id.is_empty() || !seen.insert(id.clone()) {
            let fresh = new_id();
            seen.insert(fresh.clone());
            t.id = fresh;
        } else {
            t.id = id;
        }
    }
}

/// Bring a stored trigger list onto the gesture model, once.
///
/// A list in which no row has an id was written before gestures existed, which
/// is also the build that shipped the derived double tap: every hold key
/// silently answered a second press by starting a hands-free session on the
/// same target. That behaviour is gone, so the reach it gave people is handed
/// back as what it should always have been, a row of its own: every bound Hold
/// row gets a Double tap row beside it, same key, same target, which they can
/// now see, rebind, or delete.
///
/// Nothing is dropped and nothing is rebound. The `method` field is read as
/// `gesture` by serde alias, so a Hold stays a Hold on the key it was on.
pub fn migrate_to_gestures(stored: Vec<Trigger>) -> Vec<Trigger> {
    let from_the_old_model = !stored.is_empty() && stored.iter().all(|t| t.id.trim().is_empty());
    let mut out = stored;
    if from_the_old_model {
        let companions: Vec<Trigger> = out
            .iter()
            .filter(|t| t.gesture == Gesture::Hold && t.binding.is_some())
            .map(|t| Trigger {
                id: String::new(),
                gesture: Gesture::DoubleTap,
                ..t.clone()
            })
            .collect();
        out.extend(companions);
    }
    ensure_ids(&mut out);
    out
}

/// Map a legacy trigger-mode string (`"hold"` / `"tap"`) to a gesture.
fn gesture_from_mode(mode: &str) -> Gesture {
    if mode.eq_ignore_ascii_case("hold") {
        Gesture::Hold
    } else {
        Gesture::Tap
    }
}

/// Build the trigger list from Juno's legacy settings fields. Used once, when a
/// store predates the unified model, so an upgrading user keeps their setup.
///
/// Historical always-listening always routed to the agent, so a migrated voice
/// trigger targets [`TriggerTarget::Agent`]. All configured wake words collapse
/// into that single trigger's phrase; the user can refine wording in the new UI.
///
/// No Double tap companion is added here. A store this old never had the
/// derived double tap, so there is no behaviour to hand back.
pub fn migrate_from_legacy(
    agent_combo: &str,
    agent_mode: &str,
    dictation_combo: &str,
    dictation_mode: &str,
    always_listening_active: bool,
    wake_words: &[String],
) -> Vec<Trigger> {
    let mut triggers = vec![
        trigger(
            gesture_from_mode(agent_mode),
            TriggerTarget::Agent,
            Some(Binding::Keyboard {
                shortcut: agent_combo.to_string(),
            }),
        ),
        trigger(
            gesture_from_mode(dictation_mode),
            TriggerTarget::Dictation,
            Some(Binding::Keyboard {
                shortcut: dictation_combo.to_string(),
            }),
        ),
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
            id: new_id(),
            gesture: Gesture::Say,
            target: TriggerTarget::Agent,
            binding: None,
            phrase: Some(phrase),
            require_hey_prefix,
            enabled: true,
        });
    }

    triggers
}

/// Project the trigger list back onto the legacy fields so peripheral consumers
/// (tray labels, the always-listening controller, the session's recorded start
/// method) stay coherent. Best-effort and keyboard-only: a mouse or Say primary
/// trigger leaves the legacy keyboard string untouched, and the two double
/// gestures are skipped entirely because the legacy fields have no word for
/// them. Returns the derived pieces: `(agent_combo, agent_mode,
/// dictation_combo, dictation_mode, always_listening_active, wake_words)`.
#[allow(clippy::type_complexity)]
pub fn derive_legacy(
    triggers: &[Trigger],
    prev_agent_combo: &str,
    prev_dictation_combo: &str,
) -> (String, String, String, String, bool, Vec<String>) {
    // Only Hold and Tap project: the legacy pair of modes is "hold" and "tap",
    // and a double gesture is neither.
    let find_key = |target: TriggerTarget| {
        triggers
            .iter()
            .filter(|t| t.enabled && t.target == target)
            .find(|t| matches!(t.gesture, Gesture::Hold | Gesture::Tap))
    };

    let mode_str = |t: &Trigger| {
        if t.gesture == Gesture::Hold {
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

    let agent = find_key(TriggerTarget::Agent);
    let dictation = find_key(TriggerTarget::Dictation);

    let agent_combo = agent
        .map(|t| keyboard_combo(t, prev_agent_combo))
        .unwrap_or_else(|| prev_agent_combo.to_string());
    let agent_mode = agent.map(mode_str).unwrap_or_else(|| "hold".to_string());
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

/// The keyboard trigger worth naming as "this is how you summon Juno".
///
/// Onboarding and the spoken greeting both need one answer to "which key", and
/// neither may invent it: a screen that teaches Option+D while the person's
/// trigger is the globe key has taught them nothing. Hold first, because it is
/// the gesture that needs the least explaining; then the other gestures in the
/// order they are easiest to describe. Mouse and Say rows are skipped, because
/// a key cap cannot draw them.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TriggerHint {
    /// The shortcut string, which key caps are drawn from.
    pub shortcut: String,
    /// The gesture's own word, e.g. `"Hold"`.
    pub gesture: String,
    /// The whole row as a sentence, e.g. `"Hold to talk to Juno"`.
    pub sentence: String,
}

/// What onboarding should draw for each target. `None` where nothing keyboard
/// shaped is bound, so the screen can say so instead of naming a key that does
/// nothing.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TriggerHints {
    pub agent: Option<TriggerHint>,
    pub dictation: Option<TriggerHint>,
}

/// The order a hint prefers its gestures in.
const HINT_ORDER: [Gesture; 4] = [
    Gesture::Hold,
    Gesture::DoubleTapHold,
    Gesture::Tap,
    Gesture::DoubleTap,
];

/// The hint for one target. See [`TriggerHint`].
pub fn hint_for(triggers: &[Trigger], target: TriggerTarget) -> Option<TriggerHint> {
    for gesture in HINT_ORDER {
        let found = triggers
            .iter()
            .filter(|t| t.enabled && t.target == target && t.gesture == gesture)
            .find_map(|t| match t.binding.as_ref() {
                Some(Binding::Keyboard { shortcut }) if !shortcut.trim().is_empty() => {
                    Some((t, shortcut.clone()))
                }
                _ => None,
            });
        if let Some((t, shortcut)) = found {
            return Some(TriggerHint {
                shortcut,
                gesture: gesture.label().to_string(),
                sentence: t.label(),
            });
        }
    }
    None
}

/// Both hints at once, which is what onboarding reads.
pub fn hints(triggers: &[Trigger]) -> TriggerHints {
    TriggerHints {
        agent: hint_for(triggers, TriggerTarget::Agent),
        dictation: hint_for(triggers, TriggerTarget::Dictation),
    }
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
        if t.gesture.needs_binding() && t.enabled && t.binding.is_none() {
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
/// their key, drawn greyed out, doing nothing.
///
/// Only the None -> Some edge counts. Changing which key is bound on a row
/// somebody deliberately switched off leaves it off, because that is a change
/// of which key, not a decision to start using it. Rows are matched by `id`,
/// so a key recorded on one row never switches on a different one.
pub fn enable_newly_bound(previous: &[Trigger], next: &mut [Trigger]) {
    for t in next.iter_mut() {
        if t.is_voice() || t.enabled || t.binding.is_none() {
            continue;
        }
        // A row that did not exist a moment ago and arrives already bound was
        // bound by whatever created it, so it counts as newly bound too.
        let was_unbound = previous
            .iter()
            .find(|p| p.id == t.id)
            .map(|p| p.binding.is_none())
            .unwrap_or(true);
        if was_unbound {
            t.enabled = true;
        }
    }
}

/// One thing wrong with the trigger list, named against the row that caused it.
///
/// An issue is derived from the list every time it is asked for, never stored.
/// That is the whole fix for the error that outlived its cause: "Fn (globe) is
/// bound to more than one trigger" was raised once and kept, so it stayed on
/// screen long after the second trigger had been rebound. There is nowhere for
/// a stale one to live now.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TriggerIssue {
    /// The row the message belongs beside.
    pub trigger_id: String,
    pub message: String,
}

/// Everything wrong with this trigger list, in list order.
///
/// Rules:
/// - A Say row needs a non-blank phrase, and no two Say rows may listen for the
///   same phrase.
/// - Two gestures on one key must be allowed to share it ([`can_share_key`]).
/// - No enabled keyboard trigger may take a reserved combo.
///
/// A key/mouse trigger with no binding is allowed through and is switched off
/// by [`disable_unbound`] instead, so adding a row is not lost work.
///
/// `reserved` are binding descriptions already owned by the two fixed utility
/// shortcuts (Escape to stop, Cmd+Comma to open settings).
pub fn issues(triggers: &[Trigger], reserved: &[String]) -> Vec<TriggerIssue> {
    let mut out = Vec::new();
    // (key signature, gesture, row label) for every enabled bound row already
    // seen, so a refusal can name the row that got there first.
    let mut claimed: Vec<(String, Gesture, String)> = Vec::new();
    let mut phrases: std::collections::HashSet<String> = std::collections::HashSet::new();

    for t in triggers.iter().filter(|t| t.enabled) {
        if t.is_voice() {
            let phrase = t
                .phrase
                .as_ref()
                .map(|p| p.trim().to_lowercase())
                .unwrap_or_default();
            if phrase.is_empty() {
                out.push(TriggerIssue {
                    trigger_id: t.id.clone(),
                    message: "Voice triggers need a wake phrase.".to_string(),
                });
                continue;
            }
            if !phrases.insert(phrase.clone()) {
                out.push(TriggerIssue {
                    trigger_id: t.id.clone(),
                    message: format!("\"{phrase}\" is already a wake phrase."),
                });
            }
            continue;
        }

        // A freshly added trigger has no binding yet. That is allowed to
        // persist (the UI shows it as unconfigured); it simply is not
        // registered until a key or mouse button is recorded.
        let Some(binding) = t.binding.as_ref() else {
            continue;
        };
        let label = binding.describe();
        if reserved.iter().any(|r| r.eq_ignore_ascii_case(&label)) {
            out.push(TriggerIssue {
                trigger_id: t.id.clone(),
                message: format!("\"{label}\" is already used by another shortcut."),
            });
            continue;
        }
        let signature = binding.signature();
        if let Some((_, _, other)) = claimed
            .iter()
            .find(|(sig, gesture, _)| *sig == signature && !can_share_key(*gesture, t.gesture))
        {
            out.push(TriggerIssue {
                trigger_id: t.id.clone(),
                message: format!("\"{label}\" already has {other}."),
            });
            continue;
        }
        claimed.push((signature, t.gesture, t.label()));
    }

    out
}

/// Would binding `combo` to the row named `editing_id` collide with anything?
///
/// Returns the sentence the save would fail with, so the hint shown while
/// someone is still typing and the result of pressing Save cannot disagree.
/// Passing the row being edited is what keeps a row from reporting a conflict
/// with its own current binding, and the row's gesture is read from the list,
/// because whether a key is free depends on which gesture wants it: the globe
/// key can hold *and* double-tap-and-hold, and that is the headline case.
pub fn combo_conflict(
    triggers: &[Trigger],
    combo: &str,
    editing_id: Option<&str>,
    reserved: &[String],
) -> Option<String> {
    let binding = Binding::Keyboard {
        shortcut: combo.to_string(),
    };
    let label = binding.describe();

    if reserved.iter().any(|r| r.eq_ignore_ascii_case(&label)) {
        return Some(format!("\"{label}\" is already used by another shortcut."));
    }

    // Which gesture is asking. An id that names no row (a row the window has
    // not saved yet) gets the strict answer: anything already on that key
    // conflicts. Guessing generously here would let a save fail after the fact,
    // which is the one thing this function exists to prevent.
    let asking = editing_id
        .and_then(|id| triggers.iter().find(|t| t.id == id))
        .map(|t| t.gesture);

    let signature = binding.signature();
    triggers
        .iter()
        .filter(|t| t.enabled)
        .filter(|t| editing_id.is_none_or(|id| t.id != id))
        .filter(|t| t.key_signature().as_deref() == Some(signature.as_str()))
        .find(|t| match asking {
            Some(gesture) => !can_share_key(t.gesture, gesture),
            None => true,
        })
        .map(|t| format!("\"{label}\" already has {}.", t.label()))
}

/// Validate a trigger list before it is persisted. Returns the first issue as a
/// human-readable error the UI can show inline on the offending row.
pub fn validate(triggers: &[Trigger], reserved: &[String]) -> Result<(), String> {
    match issues(triggers, reserved).into_iter().next() {
        Some(issue) => Err(issue.message),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(gesture: Gesture, target: TriggerTarget, shortcut: &str) -> Trigger {
        trigger(
            gesture,
            target,
            Some(Binding::Keyboard {
                shortcut: shortcut.to_string(),
            }),
        )
    }

    fn voice(phrase: &str, target: TriggerTarget, enabled: bool) -> Trigger {
        Trigger {
            id: new_id(),
            gesture: Gesture::Say,
            target,
            binding: None,
            phrase: Some(phrase.to_string()),
            require_hey_prefix: false,
            enabled,
        }
    }

    /* ---------------------------------------------------------------- */
    /* The sharing table                                                */
    /* ---------------------------------------------------------------- */

    #[test]
    fn hold_and_double_tap_and_hold_share_one_key() {
        // The headline case. The globe key holds to talk to Juno and
        // double-taps-and-holds to dictate, and neither is derived from the
        // other.
        assert!(can_share_key(Gesture::Hold, Gesture::DoubleTapHold));
        assert!(can_share_key(Gesture::DoubleTapHold, Gesture::Hold));
    }

    #[test]
    fn the_sharing_table_is_exactly_the_plans_table() {
        use Gesture::*;
        // yes
        assert!(can_share_key(Hold, DoubleTap));
        assert!(can_share_key(Hold, DoubleTapHold));
        assert!(can_share_key(DoubleTap, DoubleTapHold));
        assert!(can_share_key(Tap, DoubleTapHold));
        // no
        assert!(!can_share_key(Hold, Tap));
        assert!(!can_share_key(Tap, Hold));
        assert!(!can_share_key(Tap, DoubleTap));
        assert!(!can_share_key(DoubleTap, Tap));
        // and no gesture twice on one key
        for g in Gesture::ALL {
            assert!(!can_share_key(g, g), "{g:?} twice on one key");
        }
        // Say has no key to share
        for g in Gesture::ALL {
            assert!(!can_share_key(Say, g));
            assert!(!can_share_key(g, Say));
        }
    }

    #[test]
    fn validate_accepts_hold_and_double_tap_and_hold_on_the_globe_key() {
        let ts = vec![
            row(Gesture::Hold, TriggerTarget::Agent, "Fn"),
            row(Gesture::DoubleTapHold, TriggerTarget::Dictation, "Fn"),
        ];
        assert!(validate(&ts, &[]).is_ok(), "{:?}", validate(&ts, &[]));
    }

    #[test]
    fn validate_accepts_all_three_double_capable_gestures_on_one_key() {
        let ts = vec![
            row(Gesture::Hold, TriggerTarget::Agent, "Fn"),
            row(Gesture::DoubleTap, TriggerTarget::Dictation, "Fn"),
            row(Gesture::DoubleTapHold, TriggerTarget::Dictation, "Fn"),
        ];
        assert!(validate(&ts, &[]).is_ok());
    }

    #[test]
    fn validate_refuses_hold_and_tap_on_one_key() {
        let ts = vec![
            row(Gesture::Hold, TriggerTarget::Agent, "Fn"),
            row(Gesture::Tap, TriggerTarget::Dictation, "Fn"),
        ];
        let err = validate(&ts, &[]).expect_err("hold and tap cannot share a key");
        assert!(err.contains("Fn (globe)"), "got: {err}");
        assert!(err.contains("Hold to talk to Juno"), "got: {err}");
    }

    #[test]
    fn validate_refuses_tap_and_double_tap_on_one_key() {
        let ts = vec![
            row(Gesture::Tap, TriggerTarget::Dictation, "Option+Space"),
            row(Gesture::DoubleTap, TriggerTarget::Agent, "Option+Space"),
        ];
        assert!(validate(&ts, &[]).is_err());
    }

    #[test]
    fn validate_refuses_the_same_gesture_twice_on_one_key() {
        let ts = vec![
            row(Gesture::Hold, TriggerTarget::Agent, "Option+Space"),
            row(Gesture::Hold, TriggerTarget::Dictation, "Option+Space"),
        ];
        assert!(validate(&ts, &[]).is_err());
    }

    #[test]
    fn one_gesture_on_two_different_keys_is_fine() {
        // Two Hold rows are legal now, because a row is its id: holding one key
        // talks to Juno and holding another dictates.
        let ts = default_triggers();
        assert_eq!(ts.len(), 2);
        assert!(ts.iter().all(|t| t.gesture == Gesture::Hold));
        assert!(validate(&ts, &[]).is_ok());
    }

    #[test]
    fn fn_and_a_combo_cannot_be_confused_for_a_duplicate() {
        let ts = vec![
            row(Gesture::Hold, TriggerTarget::Dictation, "Fn"),
            row(Gesture::Tap, TriggerTarget::Agent, "Option+D"),
        ];
        assert!(validate(&ts, &[]).is_ok());
    }

    #[test]
    fn the_two_spellings_of_the_globe_key_are_one_key() {
        let ts = vec![
            row(Gesture::Hold, TriggerTarget::Agent, "Fn"),
            row(Gesture::Tap, TriggerTarget::Dictation, "globe"),
        ];
        assert!(
            validate(&ts, &[]).is_err(),
            "Apple calls the same key both things, so this is Hold + Tap on one key"
        );
    }

    /* ---------------------------------------------------------------- */
    /* An error disappears when the conflict does                       */
    /* ---------------------------------------------------------------- */

    #[test]
    fn a_resolved_conflict_reports_nothing() {
        // The reported defect: "'Fn (globe)' is bound to more than one trigger"
        // was raised once and never cleared, so it sat on the screen after the
        // conflict had been fixed. Issues are derived from the list every time
        // they are asked for, so there is nowhere for a stale one to live.
        let mut ts = vec![
            row(Gesture::Hold, TriggerTarget::Agent, "Fn"),
            row(Gesture::Tap, TriggerTarget::Dictation, "Fn"),
        ];
        let before = issues(&ts, &[]);
        assert_eq!(before.len(), 1, "the conflict is reported once");
        assert_eq!(before[0].trigger_id, ts[1].id, "against the later row");

        // Rebind the second row off the globe key: the conflict is gone, so the
        // message must be gone with it.
        ts[1].binding = Some(Binding::Keyboard {
            shortcut: "Option+Space".to_string(),
        });
        assert!(
            issues(&ts, &[]).is_empty(),
            "a resolved conflict reports nothing"
        );
        assert!(validate(&ts, &[]).is_ok());
    }

    #[test]
    fn switching_the_second_row_off_also_resolves_it() {
        // The other way a person fixes it. A disabled row is never registered,
        // so it holds no key.
        let mut ts = vec![
            row(Gesture::Hold, TriggerTarget::Agent, "Fn"),
            row(Gesture::Tap, TriggerTarget::Dictation, "Fn"),
        ];
        assert_eq!(issues(&ts, &[]).len(), 1);
        ts[1].enabled = false;
        assert!(issues(&ts, &[]).is_empty());
    }

    #[test]
    fn deleting_the_second_row_also_resolves_it() {
        let mut ts = vec![
            row(Gesture::Hold, TriggerTarget::Agent, "Fn"),
            row(Gesture::Tap, TriggerTarget::Dictation, "Fn"),
        ];
        assert_eq!(issues(&ts, &[]).len(), 1);
        ts.pop();
        assert!(issues(&ts, &[]).is_empty());
    }

    #[test]
    fn an_issue_names_the_row_it_belongs_beside() {
        let ts = vec![
            row(Gesture::Hold, TriggerTarget::Agent, "Fn"),
            row(Gesture::Tap, TriggerTarget::Dictation, "Fn"),
        ];
        let found = issues(&ts, &[]);
        assert_eq!(found[0].trigger_id, ts[1].id);
        assert!(!found[0].trigger_id.is_empty());
    }

    /* ---------------------------------------------------------------- */
    /* Migration                                                        */
    /* ---------------------------------------------------------------- */

    #[test]
    fn a_push_to_talk_trigger_on_fn_becomes_a_hold_on_fn() {
        // The user in the report. Their store says `method: push_to_talk` on
        // the globe key; it has to come back as Hold on the globe key, with
        // nothing rebound and nothing dropped.
        let stored = r#"[{
            "method": "push_to_talk",
            "target": "dictation",
            "binding": { "kind": "keyboard", "shortcut": "Fn" },
            "phrase": null,
            "require_hey_prefix": false,
            "enabled": true
        }]"#;
        let read: Vec<Trigger> = serde_json::from_str(stored).expect("the old shape must load");
        assert_eq!(read[0].gesture, Gesture::Hold);
        assert!(read[0].id.is_empty(), "the old shape carries no id");

        let migrated = migrate_to_gestures(read);
        let hold = migrated
            .iter()
            .find(|t| t.gesture == Gesture::Hold)
            .expect("the hold survives");
        assert_eq!(hold.target, TriggerTarget::Dictation);
        assert_eq!(
            hold.binding,
            Some(Binding::Keyboard {
                shortcut: "Fn".to_string()
            }),
            "still the globe key"
        );
        assert!(hold.enabled, "and still switched on");
        assert!(!hold.id.is_empty(), "and now has a row identity");

        // The derived double tap shipped, so the reach it gave is handed back
        // as a row of its own rather than taken away.
        let double = migrated
            .iter()
            .find(|t| t.gesture == Gesture::DoubleTap)
            .expect("a double-tap row is added beside it");
        assert_eq!(double.target, TriggerTarget::Dictation);
        assert_eq!(double.binding, hold.binding, "on the same key");
        assert_ne!(double.id, hold.id, "two rows, two identities");

        // And the result is a list the validator accepts: Hold + Double tap
        // share a key by the table.
        assert!(validate(&migrated, &[]).is_ok());
        assert_eq!(migrated.len(), 2);
    }

    #[test]
    fn migration_maps_every_old_method_name() {
        let stored = r#"[
            { "method": "push_to_talk", "target": "dictation", "binding": { "kind": "keyboard", "shortcut": "Option+Space" }, "enabled": true },
            { "method": "toggle", "target": "agent", "binding": { "kind": "keyboard", "shortcut": "Option+D" }, "enabled": true },
            { "method": "voice", "target": "agent", "binding": null, "phrase": "juno", "require_hey_prefix": false, "enabled": true }
        ]"#;
        let read: Vec<Trigger> = serde_json::from_str(stored).expect("loads");
        assert_eq!(read[0].gesture, Gesture::Hold);
        assert_eq!(read[1].gesture, Gesture::Tap);
        assert_eq!(read[2].gesture, Gesture::Say);

        let migrated = migrate_to_gestures(read);
        // Three rows in, four out: only the Hold row earns a companion.
        assert_eq!(migrated.len(), 4);
        assert_eq!(
            migrated
                .iter()
                .filter(|t| t.gesture == Gesture::DoubleTap)
                .count(),
            1
        );
        // Nothing lost: every original row is still there, on its own key.
        assert!(migrated
            .iter()
            .any(|t| t.gesture == Gesture::Tap && t.target == TriggerTarget::Agent));
        assert!(migrated
            .iter()
            .any(|t| t.gesture == Gesture::Say && t.phrase.as_deref() == Some("juno")));
        assert!(validate(&migrated, &[]).is_ok());
    }

    #[test]
    fn migration_skips_an_unbound_hold() {
        // Nothing to double-tap on a row with no key.
        let stored = r#"[{ "method": "push_to_talk", "target": "dictation", "binding": null, "enabled": false }]"#;
        let read: Vec<Trigger> = serde_json::from_str(stored).expect("loads");
        let migrated = migrate_to_gestures(read);
        assert_eq!(migrated.len(), 1);
    }

    #[test]
    fn migration_runs_once() {
        // A list that already has ids was written by this model. Running the
        // migration again must not keep stacking double-tap rows onto it,
        // which is what an unmarked migration does on every launch.
        let first = migrate_to_gestures(vec![Trigger {
            id: String::new(),
            gesture: Gesture::Hold,
            target: TriggerTarget::Agent,
            binding: Some(Binding::Keyboard {
                shortcut: "Fn".to_string(),
            }),
            phrase: None,
            require_hey_prefix: false,
            enabled: true,
        }]);
        assert_eq!(first.len(), 2);
        let second = migrate_to_gestures(first.clone());
        assert_eq!(second.len(), 2, "the second pass adds nothing");
        assert_eq!(
            second.iter().map(|t| t.id.clone()).collect::<Vec<_>>(),
            first.iter().map(|t| t.id.clone()).collect::<Vec<_>>(),
            "and keeps the identities it gave out"
        );
    }

    #[test]
    fn ensure_ids_replaces_a_repeated_identity() {
        let mut ts = vec![
            row(Gesture::Hold, TriggerTarget::Agent, "Fn"),
            row(Gesture::DoubleTap, TriggerTarget::Agent, "Fn"),
        ];
        ts[1].id = ts[0].id.clone();
        ensure_ids(&mut ts);
        assert_ne!(ts[0].id, ts[1].id);
        assert!(!ts[0].id.is_empty() && !ts[1].id.is_empty());
    }

    #[test]
    fn a_settings_file_written_before_the_collapse_keeps_its_fn_trigger() {
        // The retired binding shape, exactly as it sits in a store on disk. If
        // this ever stops resolving, someone's globe key silently stops working
        // after an update.
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
    fn the_wire_names_are_what_the_settings_window_sends() {
        // The settings window spells a gesture with these strings. If the serde
        // name and `as_str` drift, a saved row comes back as a different
        // gesture, which is the quietest possible way to break a trigger.
        for g in Gesture::ALL {
            let written = serde_json::to_string(&g).expect("serialize");
            assert_eq!(written, format!("\"{}\"", g.as_str()), "{g:?}");
            let read: Gesture =
                serde_json::from_str(&written).expect("a gesture reads back as itself");
            assert_eq!(read, g);
        }
        assert_eq!(Gesture::DoubleTapHold.as_str(), "double_tap_hold");
    }

    #[test]
    fn a_gesture_row_round_trips_through_the_store() {
        let ts = vec![row(Gesture::DoubleTapHold, TriggerTarget::Dictation, "Fn")];
        let written = serde_json::to_string(&ts).expect("serialize");
        assert!(written.contains("double_tap_hold"), "got: {written}");
        assert!(!written.contains("\"method\""), "one live field name");
        let reread: Vec<Trigger> = serde_json::from_str(&written).expect("deserialize");
        assert_eq!(reread, ts);
    }

    /* ---------------------------------------------------------------- */
    /* Defaults                                                         */
    /* ---------------------------------------------------------------- */

    #[test]
    fn the_defaults_are_hold_globe_to_talk_and_option_space_to_dictate() {
        let ts = default_triggers();
        assert_eq!(
            ts.len(),
            2,
            "two rows, nothing a new install did not ask for"
        );

        let agent = ts
            .iter()
            .find(|t| t.target == TriggerTarget::Agent)
            .expect("something talks to Juno");
        assert_eq!(agent.gesture, Gesture::Hold);
        assert_eq!(
            agent.binding,
            Some(Binding::Keyboard {
                shortcut: "Fn".to_string()
            })
        );
        assert_eq!(agent.label(), "Hold to talk to Juno");

        let dictation = ts
            .iter()
            .find(|t| t.target == TriggerTarget::Dictation)
            .expect("something dictates");
        assert_eq!(dictation.gesture, Gesture::Hold);
        assert_eq!(
            dictation.binding,
            Some(Binding::Keyboard {
                shortcut: "Option+Space".to_string()
            })
        );

        assert!(validate(&ts, &["Escape".to_string()]).is_ok());
        assert!(ts.iter().all(|t| !t.id.is_empty()), "every row has an id");
    }

    #[test]
    fn no_default_fires_on_a_single_tap() {
        // The standing rule: a keyboard trigger never fires on a single tap of
        // a hold key. Both defaults are Hold, so neither key has a tap meaning
        // at all.
        for t in default_triggers() {
            assert_ne!(t.gesture, Gesture::Tap);
            assert_ne!(t.gesture, Gesture::DoubleTap);
        }
    }

    /* ---------------------------------------------------------------- */
    /* Onboarding reads the live registry                               */
    /* ---------------------------------------------------------------- */

    #[test]
    fn the_hint_names_the_key_that_is_actually_bound() {
        let ts = default_triggers();
        let agent = hint_for(&ts, TriggerTarget::Agent).expect("a hint for the agent");
        assert_eq!(agent.shortcut, "Fn");
        assert_eq!(agent.gesture, "Hold");
        assert_eq!(agent.sentence, "Hold to talk to Juno");
    }

    #[test]
    fn changing_a_trigger_changes_what_onboarding_shows() {
        // The reported defect: onboarding taught a hardcoded shortcut, so
        // changing the trigger moved the lie rather than fixing it.
        let mut ts = default_triggers();
        let before = hint_for(&ts, TriggerTarget::Agent).expect("hint");
        assert_eq!(before.shortcut, "Fn");

        let agent = ts
            .iter_mut()
            .find(|t| t.target == TriggerTarget::Agent)
            .unwrap();
        agent.gesture = Gesture::Tap;
        agent.binding = Some(Binding::Keyboard {
            shortcut: "Control+J".to_string(),
        });

        let after = hint_for(&ts, TriggerTarget::Agent).expect("hint");
        assert_eq!(after.shortcut, "Control+J");
        assert_eq!(after.gesture, "Tap");
        assert_eq!(after.sentence, "Tap to talk to Juno");
        assert_ne!(before, after);
    }

    #[test]
    fn a_switched_off_trigger_is_never_what_onboarding_shows() {
        let mut ts = default_triggers();
        for t in ts.iter_mut() {
            t.enabled = false;
        }
        assert_eq!(hint_for(&ts, TriggerTarget::Agent), None);
        assert_eq!(hints(&ts).dictation, None);
    }

    #[test]
    fn the_hint_skips_what_a_key_cap_cannot_draw() {
        // A mouse button and a wake phrase are real triggers and neither is a
        // key cap, so the screen says nothing rather than drawing a lie.
        let ts = vec![
            trigger(
                Gesture::Hold,
                TriggerTarget::Agent,
                Some(Binding::Mouse { button: 3 }),
            ),
            voice("juno", TriggerTarget::Dictation, true),
        ];
        assert_eq!(hint_for(&ts, TriggerTarget::Agent), None);
        assert_eq!(hint_for(&ts, TriggerTarget::Dictation), None);
    }

    #[test]
    fn the_hint_prefers_hold_over_the_other_gestures() {
        let ts = vec![
            row(Gesture::DoubleTap, TriggerTarget::Agent, "Option+D"),
            row(Gesture::Hold, TriggerTarget::Agent, "Fn"),
        ];
        assert_eq!(hint_for(&ts, TriggerTarget::Agent).unwrap().shortcut, "Fn");
    }

    /* ---------------------------------------------------------------- */
    /* Grouping gestures by key                                         */
    /* ---------------------------------------------------------------- */

    #[test]
    fn the_gestures_on_one_key_are_read_off_the_list() {
        let ts = vec![
            row(Gesture::Hold, TriggerTarget::Agent, "Fn"),
            row(Gesture::DoubleTapHold, TriggerTarget::Dictation, "globe"),
            row(Gesture::Tap, TriggerTarget::Dictation, "Option+Space"),
        ];
        let globe = gestures_on_key(&ts, "fn (globe)");
        assert_eq!(globe.hold, Some(TriggerTarget::Agent));
        assert_eq!(globe.double_tap_hold, Some(TriggerTarget::Dictation));
        assert_eq!(globe.tap, None);
        assert!(globe.has_double());
        assert_eq!(globe.targets().len(), 2);

        let space = gestures_on_key(&ts, "option+space");
        assert_eq!(space.tap, Some(TriggerTarget::Dictation));
        assert!(!space.has_double());
    }

    #[test]
    fn a_switched_off_row_binds_no_gesture() {
        let mut ts = vec![row(Gesture::Hold, TriggerTarget::Agent, "Fn")];
        ts[0].enabled = false;
        assert!(gestures_on_key(&ts, "fn (globe)").is_empty());
    }

    #[test]
    fn a_key_used_by_several_rows_is_registered_once() {
        let ts = vec![
            row(Gesture::Hold, TriggerTarget::Agent, "Fn"),
            row(Gesture::DoubleTap, TriggerTarget::Dictation, "Fn"),
            row(Gesture::DoubleTapHold, TriggerTarget::Dictation, "globe"),
            row(Gesture::Tap, TriggerTarget::Dictation, "Option+Space"),
        ];
        let keys = bound_keys(&ts);
        assert_eq!(
            keys.len(),
            2,
            "two physical keys, however many rows: {keys:?}"
        );
    }

    /* ---------------------------------------------------------------- */
    /* Conflict hints while someone is still typing                     */
    /* ---------------------------------------------------------------- */

    #[test]
    fn a_row_does_not_conflict_with_its_own_binding() {
        let ts = vec![row(Gesture::Hold, TriggerTarget::Dictation, "Option+Space")];
        let me = ts[0].id.clone();
        assert_eq!(
            combo_conflict(&ts, "Option+Space", Some(&me), &[]),
            None,
            "a row must be allowed to keep the combo it already has"
        );
    }

    #[test]
    fn a_double_tap_and_hold_row_may_take_the_hold_rows_key() {
        // The headline case, checked through the path the recorder uses while
        // the person is still choosing, so the hint and the save agree.
        let mut ts = vec![row(Gesture::Hold, TriggerTarget::Agent, "Fn")];
        let mut second = trigger(Gesture::DoubleTapHold, TriggerTarget::Dictation, None);
        let second_id = second.id.clone();
        second.enabled = false; // unbound rows persist switched off
        ts.push(second);
        assert_eq!(combo_conflict(&ts, "Fn", Some(&second_id), &[]), None);
    }

    #[test]
    fn a_tap_row_may_not_take_the_hold_rows_key() {
        let mut ts = vec![row(Gesture::Hold, TriggerTarget::Agent, "Fn")];
        let mut second = trigger(Gesture::Tap, TriggerTarget::Dictation, None);
        let second_id = second.id.clone();
        second.enabled = false;
        ts.push(second);
        let msg = combo_conflict(&ts, "Fn", Some(&second_id), &[])
            .expect("tap cannot share a key with hold");
        assert!(msg.contains("Hold to talk to Juno"), "got: {msg}");
    }

    #[test]
    fn an_unknown_row_gets_the_strict_answer() {
        // A row the window has not saved yet has no gesture on record. Saying
        // "free" here would let the save fail after the fact.
        let ts = vec![row(Gesture::Hold, TriggerTarget::Agent, "Fn")];
        assert!(combo_conflict(&ts, "Fn", Some("not-a-row"), &[]).is_some());
        assert!(combo_conflict(&ts, "Fn", None, &[]).is_some());
    }

    #[test]
    fn a_switched_off_row_does_not_hold_its_combo() {
        let mut ts = vec![row(Gesture::Hold, TriggerTarget::Agent, "Option+Space")];
        ts[0].enabled = false;
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
    fn validate_still_protects_the_fixed_utility_shortcuts() {
        let reserved = vec!["Escape".to_string(), "Cmd+Comma".to_string()];
        for combo in ["Escape", "Cmd+Comma"] {
            let ts = vec![row(Gesture::Hold, TriggerTarget::Agent, combo)];
            assert!(validate(&ts, &reserved).is_err(), "{combo}");
        }
    }

    #[test]
    fn validate_frees_the_retired_voice_activation_combo() {
        let ts = vec![row(Gesture::Tap, TriggerTarget::Agent, "Option+Shift+V")];
        let reserved = vec!["Escape".to_string(), "Cmd+Comma".to_string()];
        assert!(validate(&ts, &reserved).is_ok());
    }

    #[test]
    fn validate_allows_unconfigured_binding() {
        let ts = vec![trigger(Gesture::Hold, TriggerTarget::Agent, None)];
        assert!(validate(&ts, &[]).is_ok());
    }

    /* ---------------------------------------------------------------- */
    /* Say                                                              */
    /* ---------------------------------------------------------------- */

    #[test]
    fn an_enabled_say_trigger_is_what_turns_listening_on() {
        let t = vec![voice("juno", TriggerTarget::Agent, true)];
        assert_eq!(voice_phrases_for(&t), vec!["juno", "hey juno"]);
    }

    #[test]
    fn a_disabled_say_trigger_listens_for_nothing() {
        let t = vec![voice("juno", TriggerTarget::Agent, false)];
        assert!(voice_phrases_for(&t).is_empty());
    }

    #[test]
    fn key_triggers_do_not_turn_listening_on() {
        assert!(voice_phrases_for(&default_triggers()).is_empty());
    }

    #[test]
    fn say_can_target_dictation_as_well_as_the_agent() {
        let t = vec![
            voice("juno", TriggerTarget::Agent, true),
            voice("transcribe", TriggerTarget::Dictation, true),
        ];
        let phrases = voice_phrases_for(&t);
        assert!(phrases.contains(&"juno".to_string()));
        assert!(phrases.contains(&"transcribe".to_string()));
    }

    #[test]
    fn validate_rejects_blank_voice_phrase() {
        let ts = vec![voice("  ", TriggerTarget::Agent, true)];
        assert!(validate(&ts, &[]).is_err());
    }

    #[test]
    fn validate_rejects_two_say_rows_listening_for_the_same_phrase() {
        let ts = vec![
            voice("juno", TriggerTarget::Agent, true),
            voice("Juno", TriggerTarget::Dictation, true),
        ];
        let err = validate(&ts, &[]).expect_err("one phrase cannot mean two things");
        assert!(err.contains("juno"), "got: {err}");
    }

    #[test]
    fn voice_phrases_without_prefix_match_both_wordings() {
        let t = voice("Juno", TriggerTarget::Agent, true);
        assert_eq!(t.voice_phrases(), vec!["juno", "hey juno"]);
    }

    #[test]
    fn voice_phrases_with_prefix_require_hey() {
        let mut t = voice("Juno", TriggerTarget::Agent, true);
        t.require_hey_prefix = true;
        assert_eq!(t.voice_phrases(), vec!["hey juno"]);
    }

    #[test]
    fn voice_phrases_empty_when_blank() {
        let t = voice("   ", TriggerTarget::Dictation, true);
        assert!(t.voice_phrases().is_empty());
    }

    /* ---------------------------------------------------------------- */
    /* The switch tells the truth                                       */
    /* ---------------------------------------------------------------- */

    #[test]
    fn disable_unbound_turns_off_a_key_trigger_with_no_binding() {
        let mut ts = vec![
            trigger(Gesture::Hold, TriggerTarget::Dictation, None),
            row(Gesture::Tap, TriggerTarget::Agent, "Option+Space"),
        ];
        disable_unbound(&mut ts);
        assert!(!ts[0].enabled, "an unbound key trigger must not read as on");
        assert!(ts[1].enabled, "a bound trigger is left alone");
        assert_eq!(ts.len(), 2, "the unconfigured row still persists");
    }

    #[test]
    fn disable_unbound_leaves_a_say_trigger_alone() {
        let mut ts = vec![voice("juno", TriggerTarget::Agent, true)];
        disable_unbound(&mut ts);
        assert!(ts[0].enabled);
    }

    #[test]
    fn binding_a_key_to_an_unbound_row_switches_it_on() {
        let previous = vec![trigger(Gesture::Hold, TriggerTarget::Dictation, None)];
        let mut next = previous.clone();
        next[0].enabled = false;
        next[0].binding = Some(Binding::Keyboard {
            shortcut: "Fn".to_string(),
        });
        enable_newly_bound(&previous, &mut next);
        assert!(next[0].enabled);
    }

    #[test]
    fn rebinding_a_row_somebody_switched_off_leaves_it_off() {
        let mut previous = vec![row(Gesture::Hold, TriggerTarget::Dictation, "Option+Space")];
        previous[0].enabled = false;
        let mut next = previous.clone();
        next[0].binding = Some(Binding::Keyboard {
            shortcut: "Fn".to_string(),
        });
        enable_newly_bound(&previous, &mut next);
        assert!(!next[0].enabled);
    }

    #[test]
    fn rows_are_matched_by_id_not_by_gesture_and_target() {
        // Two Hold rows pointing at dictation are legal now. Matching on
        // (gesture, target) would let a key recorded on one switch on the
        // other, which is the identity bug the id replaces.
        let a = row(Gesture::Hold, TriggerTarget::Dictation, "Option+Space");
        let mut b = trigger(Gesture::Hold, TriggerTarget::Dictation, None);
        b.enabled = false;
        let previous = vec![a.clone(), b.clone()];

        let mut next = previous.clone();
        next[1].binding = Some(Binding::Keyboard {
            shortcut: "Control+J".to_string(),
        });
        enable_newly_bound(&previous, &mut next);
        assert!(next[1].enabled, "the row that got a key is on");
        assert_eq!(next[0].binding, a.binding, "the other row is untouched");
    }

    #[test]
    fn the_two_switch_rules_do_not_fight() {
        let previous = vec![
            trigger(Gesture::Hold, TriggerTarget::Dictation, None),
            trigger(Gesture::DoubleTapHold, TriggerTarget::Agent, None),
        ];
        let mut next = previous.clone();
        next[0].enabled = false;
        next[0].binding = Some(Binding::Keyboard {
            shortcut: "Fn".to_string(),
        });
        next[1].enabled = true;
        enable_newly_bound(&previous, &mut next);
        disable_unbound(&mut next);
        assert!(next[0].enabled, "the row that just got a key is on");
        assert!(!next[1].enabled, "the row with no key is off");
    }

    #[test]
    fn a_say_trigger_is_never_switched_on_by_this() {
        let previous = vec![voice("juno", TriggerTarget::Agent, false)];
        let mut next = previous.clone();
        enable_newly_bound(&previous, &mut next);
        assert!(!next[0].enabled);
    }

    /* ---------------------------------------------------------------- */
    /* Watchers and labels                                              */
    /* ---------------------------------------------------------------- */

    #[test]
    fn a_bare_modifier_goes_to_the_modifier_watcher() {
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
        assert_eq!(
            watcher_for(&Binding::Keyboard {
                shortcut: "Fn+F5".to_string()
            }),
            Watcher::GlobalShortcut,
            "a combo that merely mentions the word is not the bare key"
        );
        assert_eq!(
            watcher_for(&Binding::Mouse { button: 3 }),
            Watcher::MouseButton(3)
        );
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
    fn mouse_binding_describes_side_buttons_one_indexed() {
        assert_eq!(Binding::Mouse { button: 3 }.describe(), "Mouse Button 4");
        assert_eq!(Binding::Mouse { button: 2 }.describe(), "Middle Click");
    }

    #[test]
    fn every_gesture_says_how_it_ends() {
        for g in Gesture::ALL {
            let ending = g.ending();
            assert!(!ending.is_empty(), "{g:?}");
            // The copy this replaced taught a second gesture that reached the
            // same target. Nothing generated from one gesture may do that.
            assert!(
                !ending.to_lowercase().contains("double-tap"),
                "{g:?} describes another gesture: {ending}"
            );
        }
    }

    /* ---------------------------------------------------------------- */
    /* The legacy projection                                            */
    /* ---------------------------------------------------------------- */

    #[test]
    fn derive_legacy_projects_hold_and_tap_only() {
        let ts = vec![
            row(Gesture::Tap, TriggerTarget::Agent, "Option+D"),
            row(Gesture::Hold, TriggerTarget::Dictation, "Option+Space"),
            row(Gesture::DoubleTapHold, TriggerTarget::Dictation, "Fn"),
        ];
        let (a_combo, a_mode, d_combo, d_mode, al, words) = derive_legacy(&ts, "old+a", "old+d");
        assert_eq!(a_combo, "Option+D");
        assert_eq!(a_mode, "tap");
        assert_eq!(d_combo, "Option+Space", "the double gesture is skipped");
        assert_eq!(d_mode, "hold");
        assert!(!al);
        assert!(words.is_empty());
    }

    #[test]
    fn derive_legacy_keeps_previous_combo_for_an_fn_binding() {
        // The legacy fields are parsed as combos and drawn as key caps, and
        // "Fn" is neither. It falls through the same way a mouse button does.
        let ts = vec![row(Gesture::Hold, TriggerTarget::Dictation, "Fn")];
        let (_, _, d_combo, d_mode, ..) = derive_legacy(&ts, "kept+a", "kept+d");
        assert_eq!(d_combo, "kept+d");
        assert_eq!(d_mode, "hold");
    }

    #[test]
    fn derive_legacy_keeps_previous_combo_for_mouse_binding() {
        let ts = vec![trigger(
            Gesture::Hold,
            TriggerTarget::Agent,
            Some(Binding::Mouse { button: 3 }),
        )];
        let (a_combo, a_mode, ..) = derive_legacy(&ts, "kept+combo", "kept+d");
        assert_eq!(a_combo, "kept+combo");
        assert_eq!(a_mode, "hold");
    }

    #[test]
    fn derive_legacy_only_sees_double_gestures_as_nothing() {
        // A list of nothing but double gestures has no legacy spelling, so the
        // previous values survive rather than being overwritten with a guess.
        let ts = vec![row(Gesture::DoubleTap, TriggerTarget::Agent, "Fn")];
        let (a_combo, a_mode, ..) = derive_legacy(&ts, "kept+a", "kept+d");
        assert_eq!(a_combo, "kept+a");
        assert_eq!(a_mode, "hold");
    }

    #[test]
    fn migrate_maps_modes_to_gestures() {
        let ts = migrate_from_legacy("Option+D", "tap", "Option+Space", "hold", false, &[]);
        assert_eq!(ts.len(), 2);
        let agent = ts
            .iter()
            .find(|t| t.target == TriggerTarget::Agent)
            .unwrap();
        assert_eq!(agent.gesture, Gesture::Tap);
        let dictation = ts
            .iter()
            .find(|t| t.target == TriggerTarget::Dictation)
            .unwrap();
        assert_eq!(dictation.gesture, Gesture::Hold);
        assert_eq!(
            dictation.binding,
            Some(Binding::Keyboard {
                shortcut: "Option+Space".to_string()
            })
        );
        assert!(ts.iter().all(|t| !t.id.is_empty()));
    }

    #[test]
    fn migrate_reconstructs_say_trigger_when_always_listening() {
        let words = vec!["hey juno".to_string(), "hey computer".to_string()];
        let ts = migrate_from_legacy("Option+D", "tap", "Option+Space", "hold", true, &words);
        let voice = ts.iter().find(|t| t.is_voice()).expect("say trigger");
        assert_eq!(voice.target, TriggerTarget::Agent);
        assert_eq!(voice.phrase.as_deref(), Some("juno"));
        assert!(voice.require_hey_prefix, "every word had the prefix");
    }
}
