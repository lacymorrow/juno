//! # Triggers: one gesture, one target, one row
//!
//! One list describes every way the user can summon Juno. A [`Trigger`] pairs a
//! [`Gesture`] (how it fires) with a [`TriggerTarget`] (what it does), so the
//! two axes that used to live in three different settings screens (the key
//! combo, the tap/hold mode, and the always-listening wake words) collapse into
//! a single object that reads as a sentence: *Hold the globe key to talk to
//! Juno*.
//!
//! Every gesture is a trigger in its own right: a row with its own key and its
//! own target, which the person can see and delete. The set is unbounded: a row
//! is its `id` and nothing else, so two rows may share a gesture, a target, or
//! both, as long as they sit on different keys. One key means one thing, and
//! [`validate`] enforces that and nothing looser.
//!
//! `triggers` is the source of truth for the settings UI and for shortcut
//! registration. The legacy `KeyboardShortcuts` / `AgentSettings.trigger_mode` /
//! `AudioSettings` wake-word fields are *derived* from it (see
//! [`derive_legacy`]) so existing runtime consumers keep working without a
//! codebase-wide rewrite.

use serde::{Deserialize, Deserializer, Serialize};

/// How a trigger fires.
///
/// Three independent gestures. The wire names of all three keep the words the
/// old model used as aliases (`push_to_talk`, `toggle`, `voice`), because a
/// store written before gestures existed has to keep working.
///
/// There used to be two more, `double_tap` and `double_tap_hold`. They were
/// removed because they did not work well. A store that still names one reads
/// as [`Gesture::Hold`] (see [`StoredGesture`]), and [`load_stored`] settles
/// what to do with the row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Gesture {
    /// Down edge starts it, release ends it.
    Hold,
    /// The release of a press starts it, the next press of the same key ends it.
    Tap,
    /// A wake phrase starts it, the end of speech ends it. No key.
    Say,
}

/// Every spelling of a gesture a stored or incoming row may use.
///
/// The two double gestures are read here and nowhere else. Nothing writes
/// them: [`Gesture`] serializes as hold, tap or say only.
#[derive(Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum StoredGesture {
    #[serde(alias = "push_to_talk")]
    Hold,
    #[serde(alias = "toggle")]
    Tap,
    DoubleTap,
    DoubleTapHold,
    #[serde(alias = "voice")]
    Say,
}

impl StoredGesture {
    fn is_retired(self) -> bool {
        matches!(self, Self::DoubleTap | Self::DoubleTapHold)
    }
}

impl From<StoredGesture> for Gesture {
    fn from(g: StoredGesture) -> Self {
        match g {
            // A retired double gesture is a hold on the same key. Whether that
            // is allowed to stand is [`load_stored`]'s question, not serde's.
            StoredGesture::Hold | StoredGesture::DoubleTap | StoredGesture::DoubleTapHold => {
                Gesture::Hold
            }
            StoredGesture::Tap => Gesture::Tap,
            StoredGesture::Say => Gesture::Say,
        }
    }
}

impl<'de> Deserialize<'de> for Gesture {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(StoredGesture::deserialize(deserializer)?.into())
    }
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
    pub const ALL: [Gesture; 3] = [Gesture::Hold, Gesture::Tap, Gesture::Say];

    /// The serde name. Both sides of the IPC boundary spell it this way.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Hold => "hold",
            Self::Tap => "tap",
            Self::Say => "say",
        }
    }

    /// The first word of the row's sentence.
    pub fn label(self) -> &'static str {
        match self {
            Self::Hold => "Hold",
            Self::Tap => "Tap",
            Self::Say => "Say",
        }
    }

    /// How this gesture ends, in the person's words.
    ///
    /// Generated from the gesture, so the line beside a row can only ever
    /// describe what that row does.
    pub fn ending(self) -> &'static str {
        match self {
            Self::Hold => "Let go to finish.",
            Self::Tap => "Press the key again to finish.",
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

/// A modifier-only binding: one bare modifier, or a chord of two or more held
/// together, which produces no ordinary key event, only modifier flag changes.
///
/// It is a set, not a list of named cases, because the person can hold any
/// combination: Fn, Fn and Control, Control and Option, Option and Command,
/// Fn and Shift. The set is what a binding means, so two spellings of the same
/// set ("Control+Fn", "fn+ctrl") are one key, and Fn is a different key from
/// Fn and Control.
///
/// Left and right are not told apart, with one exception the monitor can see:
/// the right-hand Option key on its own ([`ModifierKey::RIGHT_OPTION`]).
///
/// What may stand alone is deliberately short. Fn and Right Option are keys
/// nothing else uses on their own; Control is the hold for a keyboard with no
/// Fn key. A lone Shift, Command or left Option is part of every second
/// shortcut, so it is not offered. Any set of two or more is.
///
/// Caps Lock is deliberately absent. Checked on hardware: it emits one event
/// per press and nothing on release, because it is a hardware toggle, so a
/// push-to-talk bound to it would hold the microphone open until the next
/// press. Remapping it with `hidutil` to a spare function key is the honest
/// route, and that already works as an ordinary keyboard binding.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ModifierKey(u8);

const FN_BIT: u8 = 1;
const CONTROL_BIT: u8 = 1 << 1;
const OPTION_BIT: u8 = 1 << 2;
const SHIFT_BIT: u8 = 1 << 3;
const COMMAND_BIT: u8 = 1 << 4;
/// Only ever set together with [`OPTION_BIT`]: the right-hand Option key is an
/// Option key.
const RIGHT_BIT: u8 = 1 << 5;

/// Each modifier in the order a chord is written, with how it is written and
/// how it is shown.
const PARTS: [(u8, &str); 5] = [
    (FN_BIT, "Fn"),
    (CONTROL_BIT, "Control"),
    (OPTION_BIT, "Option"),
    (SHIFT_BIT, "Shift"),
    (COMMAND_BIT, "Command"),
];

impl ModifierKey {
    /// The globe key. Reports key code 63 with bit 1 << 23 while held.
    pub const FN: Self = Self(FN_BIT);
    /// Control on its own, for a keyboard with no Fn key.
    pub const CONTROL: Self = Self(CONTROL_BIT);
    /// Option, either side. Only ever part of a chord.
    pub const OPTION: Self = Self(OPTION_BIT);
    /// Shift, either side. Only ever part of a chord.
    pub const SHIFT: Self = Self(SHIFT_BIT);
    /// Command, either side. Only ever part of a chord.
    pub const COMMAND: Self = Self(COMMAND_BIT);
    /// The right-hand Option key on its own, key code 61, told apart from the
    /// left one.
    pub const RIGHT_OPTION: Self = Self(OPTION_BIT | RIGHT_BIT);
    /// The globe key and Control, the default dictation chord.
    pub const FN_CONTROL: Self = Self(FN_BIT | CONTROL_BIT);

    /// No modifier at all.
    pub const NONE: Self = Self(0);

    /// Both sets together.
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Every modifier in `other` is in this set too.
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// The same set with left and right no longer told apart.
    pub const fn sideless(self) -> Self {
        Self(self.0 & !RIGHT_BIT)
    }

    /// How many modifiers are in the set, left and right counted as one.
    pub fn count(self) -> u32 {
        self.sideless().0.count_ones()
    }

    /// Whether the set includes the globe key.
    pub const fn has_fn(self) -> bool {
        self.0 & FN_BIT != 0
    }

    /// Whether this set can be a binding: Fn, Control or Right Option alone,
    /// or any two or more modifiers. The right-hand Option key is only told
    /// apart on its own; inside a chord either Option counts.
    pub fn is_bindable(self) -> bool {
        if self == Self::FN || self == Self::CONTROL || self == Self::RIGHT_OPTION {
            return true;
        }
        self.0 & RIGHT_BIT == 0 && self.count() >= 2
    }

    /// What the settings window calls it.
    pub fn label(self) -> String {
        if self == Self::FN {
            return "Fn (globe)".to_string();
        }
        if self == Self::RIGHT_OPTION {
            return "Right Option".to_string();
        }
        self.parts().join(" + ")
    }

    /// The shortcut string this key is recorded as.
    ///
    /// Fn is a key on the keyboard, so it is written down the way every other
    /// key is written down. The fact that it has to be watched differently is
    /// a detail of the watching, not of the binding.
    pub fn shortcut(self) -> String {
        if self == Self::RIGHT_OPTION {
            return "RightOption".to_string();
        }
        self.parts().join("+")
    }

    fn parts(self) -> Vec<&'static str> {
        PARTS
            .iter()
            .filter(|(bit, _)| self.0 & *bit != 0)
            .map(|(_, name)| *name)
            .collect()
    }

    /// Keys that are also ordinary shortcut ingredients (Control+C, Option+key)
    /// and so can only ever be held, never tapped: a tap would fire on every
    /// Control-click. A chord may be tapped, because the monitor only counts a
    /// tap whose keys came up with nothing else pressed in between.
    pub fn hold_only(self) -> bool {
        self == Self::CONTROL || self == Self::RIGHT_OPTION
    }
}

impl Serialize for ModifierKey {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.shortcut())
    }
}

impl<'de> Deserialize<'de> for ModifierKey {
    /// The retired `modifier` binding kind wrote `fn`, `fn_control`,
    /// `control` or `right_option`. Those, and any shortcut string a set is
    /// written as, read back.
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        let spelled = match raw.as_str() {
            "fn_control" => "Fn+Control",
            "right_option" => "RightOption",
            other => other,
        };
        bare_modifier(spelled)
            .ok_or_else(|| serde::de::Error::custom(format!("not a modifier key: {raw}")))
    }
}

/// Which modifier a word of a shortcut string names, if it names one.
///
/// Apple has called the same physical key both Fn and Globe, and a hand-edited
/// store or an older build may carry either.
fn modifier_word(word: &str) -> Option<ModifierKey> {
    Some(match word {
        "fn" | "globe" => ModifierKey::FN,
        "control" | "ctrl" => ModifierKey::CONTROL,
        "option" | "opt" | "alt" => ModifierKey::OPTION,
        "shift" => ModifierKey::SHIFT,
        "command" | "cmd" | "meta" | "super" => ModifierKey::COMMAND,
        "rightoption" | "right option" | "right_option" | "rightalt" | "right alt" | "roption" => {
            ModifierKey::RIGHT_OPTION
        }
        _ => return None,
    })
}

/// The bare modifier, or chord of bare modifiers, a shortcut string names, if
/// it names one.
///
/// This is the single place that decides which watcher a key goes to. A
/// modifier-only binding produces no ordinary key event, so the global-shortcut
/// plugin cannot register it and [`crate::platform::modifier_key_monitor`]
/// takes it instead. That is a fact about how the key is watched, which is why
/// it lives in the registration path and not in the shape of a binding.
///
/// `Fn`, `globe`, `Control`, `RightOption`, and any two or more modifiers
/// (`Fn+Control`, `Control+Option`, `Shift+Command`, any order, any case) are
/// bare. `Fn+F5` and `Control+Space` are ordinary combos that merely mention a
/// modifier, and the plugin keeps them. A lone Shift, Command or Option is not
/// a binding at all (see [`ModifierKey::is_bindable`]).
pub fn bare_modifier(shortcut: &str) -> Option<ModifierKey> {
    let words: Vec<String> = shortcut
        .split('+')
        .map(|p| p.trim().to_ascii_lowercase())
        .collect();
    let mut set = ModifierKey::NONE;
    for word in &words {
        let key = modifier_word(word)?;
        // The same modifier twice is a typo, not a chord.
        if set.sideless().contains(key.sideless()) {
            return None;
        }
        set = set.union(key);
    }
    // Right Option is only told apart on its own. Inside a chord it is Option.
    if words.len() > 1 {
        set = set.sideless();
    }
    set.is_bindable().then_some(set)
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
                shortcut: key.shortcut(),
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
                .map(|key| key.label())
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
    /// `(method, target)`, which left no room for a second row with the same
    /// pair.
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
    /// Juno moved this row off its globe-key default because no connected
    /// keyboard has a globe key, and will move it back when one appears. Set
    /// only by [`settle_for_keyboard`]; any binding the person records clears
    /// it (see [`clear_fallback_on_rebind`]), so a key somebody chose is never
    /// touched.
    #[serde(default, skip_serializing_if = "is_false")]
    pub keyboard_fallback: bool,
}

fn is_false(b: &bool) -> bool {
    !*b
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
/// gets an answer per gesture, each with its own target.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct KeyGestures {
    pub hold: Option<TriggerTarget>,
    pub tap: Option<TriggerTarget>,
}

impl KeyGestures {
    pub fn is_empty(&self) -> bool {
        self.hold.is_none() && self.tap.is_none()
    }

    /// Every target this key can reach, for the visual-feedback events the
    /// onboarding screen listens to.
    pub fn targets(&self) -> Vec<TriggerTarget> {
        let mut out = Vec::new();
        for target in [self.hold, self.tap].into_iter().flatten() {
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

/// The phrases the engine may actually arm: [`voice_phrases_for`], unless
/// voice triggers are switched off for this build.
///
/// Wake phrases are experimental for launch. They only arm while advanced
/// settings are on, which is also the only place a Say row is shown. A Say row
/// saved before that, or left behind when advanced settings were switched
/// off, stays in the list untouched but never opens the microphone. Every
/// start of the engine goes through this, so there is no path that arms the
/// microphone around it.
pub fn armed_voice_phrases(triggers: &[Trigger], voice_allowed: bool) -> Vec<String> {
    if !voice_allowed {
        return Vec::new();
    }
    voice_phrases_for(triggers)
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
        keyboard_fallback: false,
    }
}

/// A fresh row identity.
pub fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

/// The shortcut string the globe key is recorded as.
pub const GLOBE_SHORTCUT: &str = "Fn";

/// The shortcut string dictation is recorded as by default: the globe key and
/// Control held together.
pub const DICTATION_SHORTCUT: &str = "Fn+Control";

/// What the agent hold is recorded as on a keyboard with no Fn key.
pub const NO_FN_AGENT_SHORTCUT: &str = "RightOption";

/// What the dictation hold is recorded as on a keyboard with no Fn key.
pub const NO_FN_DICTATION_SHORTCUT: &str = "Control";

/// The default trigger set for a fresh install.
///
/// Two rows, because two is what a new install needs to be usable and anything
/// more is setup nobody asked for:
/// - Hold the globe key to talk to Juno. One key, under the thumb, nothing to
///   chord.
/// - Hold the globe key and Control to dictate. The same thumb, one finger
///   more, and it cannot be mistaken for the plain hold.
///
/// Both are Hold, so neither key carries a second meaning on a single tap.
///
/// On a Mac with no globe key connected, [`settle_for_keyboard`] moves both to
/// keys every keyboard has, and back again when one is plugged in.
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
                shortcut: DICTATION_SHORTCUT.to_string(),
            }),
        ),
    ]
}

/// Each target's globe-key default and the key it falls back to when no
/// connected keyboard has a globe key: Right Option to talk to Juno, Control
/// to dictate. Both are keys every keyboard has.
const KEYBOARD_FALLBACKS: [(TriggerTarget, &str, &str); 2] = [
    (TriggerTarget::Agent, GLOBE_SHORTCUT, NO_FN_AGENT_SHORTCUT),
    (
        TriggerTarget::Dictation,
        DICTATION_SHORTCUT,
        NO_FN_DICTATION_SHORTCUT,
    ),
];

fn keyboard_signature(shortcut: &str) -> String {
    Binding::Keyboard {
        shortcut: shortcut.to_string(),
    }
    .signature()
}

/// Put the default holds on keys the connected keyboards actually have.
///
/// `fn_present` is what [`crate::platform::fn_key_detection`] found: whether any
/// connected keyboard has a globe key. Nobody is asked; the hardware says.
///
/// - No globe key: a Hold row still on its globe-key default moves to the
///   fallback (Right Option to talk to Juno, Control to dictate) and is marked
///   [`Trigger::keyboard_fallback`].
/// - A globe key again: a marked row moves back to its default and the mark
///   goes.
///
/// A row on any other key is somebody's choice and is never touched, and a
/// move that would land on a key another enabled row already uses is not made.
/// Returns whether anything changed. Idempotent.
pub fn settle_for_keyboard(triggers: &mut [Trigger], fn_present: bool) -> bool {
    // Decide every move against the list as it stands, then make them.
    let moves: Vec<(usize, &'static str, bool)> = triggers
        .iter()
        .enumerate()
        .filter_map(|(i, row)| {
            if row.gesture != Gesture::Hold {
                return None;
            }
            let (_, globe, fallback) = KEYBOARD_FALLBACKS
                .iter()
                .find(|(target, _, _)| *target == row.target)?;
            let (from, to, mark) = match (fn_present, row.keyboard_fallback) {
                (true, true) => (*fallback, *globe, false),
                (false, false) => (*globe, *fallback, true),
                _ => return None,
            };
            if row.key_signature() != Some(keyboard_signature(from)) {
                return None;
            }
            let target_sig = keyboard_signature(to);
            let taken = triggers.iter().enumerate().any(|(j, other)| {
                j != i
                    && other.enabled
                    && other.key_signature().as_deref() == Some(target_sig.as_str())
            });
            (!taken).then_some((i, to, mark))
        })
        .collect();
    for (i, to, mark) in &moves {
        if let Some(row) = triggers.get_mut(*i) {
            row.binding = Some(Binding::Keyboard {
                shortcut: to.to_string(),
            });
            row.keyboard_fallback = *mark;
        }
    }
    !moves.is_empty()
}

/// A row whose binding the person has just changed is theirs now: it loses
/// the [`Trigger::keyboard_fallback`] mark, so [`settle_for_keyboard`] leaves
/// it alone from here on. Rows are matched by id; a row that is new to the
/// list cannot carry a mark Juno did not set.
pub fn clear_fallback_on_rebind(previous: &[Trigger], next: &mut [Trigger]) {
    for t in next.iter_mut().filter(|t| t.keyboard_fallback) {
        let unchanged = previous
            .iter()
            .find(|p| p.id == t.id)
            .is_some_and(|p| p.keyboard_fallback && p.binding == t.binding);
        if !unchanged {
            t.keyboard_fallback = false;
        }
    }
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

/// A trigger as it may sit on disk or arrive from a window that has not
/// reloaded: the same fields as [`Trigger`], but a gesture may still be one of
/// the two retired double gestures.
#[derive(Deserialize)]
struct StoredTrigger {
    #[serde(default)]
    id: String,
    #[serde(alias = "method")]
    gesture: StoredGesture,
    target: TriggerTarget,
    #[serde(default)]
    binding: Option<Binding>,
    #[serde(default)]
    phrase: Option<String>,
    #[serde(default)]
    require_hey_prefix: bool,
    #[serde(default = "default_true")]
    enabled: bool,
    #[serde(default)]
    keyboard_fallback: bool,
}

impl From<StoredTrigger> for Trigger {
    fn from(t: StoredTrigger) -> Self {
        Trigger {
            id: t.id,
            gesture: t.gesture.into(),
            target: t.target,
            binding: t.binding,
            phrase: t.phrase,
            require_hey_prefix: t.require_hey_prefix,
            enabled: t.enabled,
            keyboard_fallback: t.keyboard_fallback,
        }
    }
}

/// Read a stored trigger list and bring it onto the current model.
///
/// This is the single door a stored list comes through, and it runs on every
/// load. It is idempotent: what it returns writes back as a list it leaves
/// alone, so running it again, or never writing it back, changes nothing.
///
/// - `push_to_talk`, `toggle` and `voice` read as Hold, Tap and Say, on the
///   same key with the same target and switch.
/// - A row without an `id` is given one.
/// - A retired double gesture becomes a Hold on the same key. If that would
///   put two rows on one key, the double row gives way: a dictation row is
///   rebound to [`DICTATION_SHORTCUT`], and any other row is dropped rather
///   than double-bound. If even that key is taken, dictation is dropped too.
///
/// It never adds a row. Triggers are triggers; nobody is handed a gesture they
/// did not bind.
pub fn load_stored(value: &serde_json::Value) -> Result<Vec<Trigger>, serde_json::Error> {
    let stored: Vec<StoredTrigger> = serde_json::from_value(value.clone())?;
    Ok(migrate_stored(stored))
}

/// [`load_stored`] as a serde hook, for the settings structs that carry a
/// trigger list (an imported settings file goes through it too).
pub fn deserialize_stored<'de, D>(deserializer: D) -> Result<Vec<Trigger>, D::Error>
where
    D: Deserializer<'de>,
{
    Ok(migrate_stored(Vec::<StoredTrigger>::deserialize(
        deserializer,
    )?))
}

fn migrate_stored(stored: Vec<StoredTrigger>) -> Vec<Trigger> {
    let retired: Vec<bool> = stored.iter().map(|t| t.gesture.is_retired()).collect();
    let mut rows: Vec<Trigger> = stored.into_iter().map(Trigger::from).collect();
    ensure_ids(&mut rows);

    // Rows that were never a double gesture stand as they are. The retired
    // ones are then settled in list order, each against everything already
    // settled, so the result does not depend on which came first in the file.
    let mut settled: Vec<Option<Trigger>> = rows
        .iter()
        .zip(&retired)
        .map(|(row, is_retired)| (!is_retired).then(|| row.clone()))
        .collect();

    for (i, row) in rows.into_iter().enumerate() {
        if !retired[i] {
            continue;
        }
        let mut row = row;
        if !takes_a_taken_key(&row, &settled) {
            settled[i] = Some(row);
            continue;
        }
        if row.target == TriggerTarget::Dictation {
            row.binding = Some(Binding::Keyboard {
                shortcut: DICTATION_SHORTCUT.to_string(),
            });
            if !takes_a_taken_key(&row, &settled) {
                settled[i] = Some(row);
            }
        }
    }

    settled.into_iter().flatten().collect()
}

/// Would this enabled row sit on a key an enabled, already settled row owns?
fn takes_a_taken_key(row: &Trigger, settled: &[Option<Trigger>]) -> bool {
    if !row.enabled {
        return false;
    }
    let Some(signature) = row.key_signature() else {
        return false;
    };
    settled
        .iter()
        .flatten()
        .any(|o| o.enabled && o.key_signature().as_deref() == Some(signature.as_str()))
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
            keyboard_fallback: false,
        });
    }

    triggers
}

/// Project the trigger list back onto the legacy fields so peripheral consumers
/// (tray labels, the always-listening controller, the session's recorded start
/// method) stay coherent. Best-effort and keyboard-only: a mouse or Say primary
/// trigger leaves the legacy keyboard string untouched. Returns the derived
/// pieces: `(agent_combo, agent_mode,
/// dictation_combo, dictation_mode, always_listening_active, wake_words)`.
#[allow(clippy::type_complexity)]
pub fn derive_legacy(
    triggers: &[Trigger],
    prev_agent_combo: &str,
    prev_dictation_combo: &str,
) -> (String, String, String, String, bool, Vec<String>) {
    // Only Hold and Tap project: the legacy pair of modes is "hold" and "tap",
    // and Say has no key.
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
    /// Whether a connected keyboard has a globe key, so a screen does not
    /// invite somebody to press a key they do not have.
    pub globe_key: bool,
}

/// The order a hint prefers its gestures in.
const HINT_ORDER: [Gesture; 2] = [Gesture::Hold, Gesture::Tap];

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
        globe_key: true,
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
/// - One key means one thing: two enabled rows may not share a key, whatever
///   their gestures or targets.
/// - No enabled keyboard trigger may take a reserved combo.
///
/// A key/mouse trigger with no binding is allowed through and is switched off
/// by [`disable_unbound`] instead, so adding a row is not lost work.
///
/// `reserved` are binding descriptions already owned by the two fixed utility
/// shortcuts (Escape to stop, Cmd+Comma to open settings).
pub fn issues(triggers: &[Trigger], reserved: &[String]) -> Vec<TriggerIssue> {
    let mut out = Vec::new();
    // (key signature, row label) for every enabled bound row already seen, so a
    // refusal can name the row that got there first.
    let mut claimed: Vec<(String, String)> = Vec::new();
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
                    message: "Voice shortcuts need a wake phrase.".to_string(),
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
        if t.gesture == Gesture::Tap {
            if let Watcher::ModifierKey(key) = watcher_for(binding) {
                if key.hold_only() {
                    out.push(TriggerIssue {
                        trigger_id: t.id.clone(),
                        message: format!("\"{label}\" can only be held, not tapped."),
                    });
                    continue;
                }
            }
        }
        let signature = binding.signature();
        if let Some((_, other)) = claimed.iter().find(|(sig, _)| *sig == signature) {
            out.push(TriggerIssue {
                trigger_id: t.id.clone(),
                message: format!("\"{label}\" already has {other}."),
            });
            continue;
        }
        claimed.push((signature, t.label()));
    }

    out
}

/// Would binding `combo` to the row named `editing_id` collide with anything?
///
/// Returns the sentence the save would fail with, so the hint shown while
/// someone is still typing and the result of pressing Save cannot disagree.
/// Passing the row being edited is what keeps a row from reporting a conflict
/// with its own current binding.
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

    let signature = binding.signature();
    triggers
        .iter()
        .filter(|t| t.enabled)
        .filter(|t| editing_id.is_none_or(|id| t.id != id))
        .find(|t| t.key_signature().as_deref() == Some(signature.as_str()))
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
            keyboard_fallback: false,
        }
    }

    /* ---------------------------------------------------------------- */
    /* One key, one thing                                               */
    /* ---------------------------------------------------------------- */

    #[test]
    fn a_chord_with_the_globe_key_is_a_different_key_from_the_globe_key() {
        // The owner's setup: holding Fn talks to Juno, holding Fn and Control
        // dictates. Two keys as far as a row is concerned.
        let ts = vec![
            row(Gesture::Hold, TriggerTarget::Agent, "Fn"),
            row(Gesture::Hold, TriggerTarget::Dictation, "Fn+Control"),
        ];
        assert!(validate(&ts, &[]).is_ok(), "{:?}", validate(&ts, &[]));
        assert_eq!(bound_keys(&ts).len(), 2);
    }

    #[test]
    fn the_chord_has_one_signature_however_it_is_spelled() {
        let sig = |s: &str| {
            Binding::Keyboard {
                shortcut: s.to_string(),
            }
            .signature()
        };
        assert_eq!(sig("Fn+Control"), sig("control+FN"));
        assert_eq!(sig("globe+ctrl"), sig("Fn+Control"));
        assert_ne!(sig("Fn+Control"), sig("Fn"));
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

    /// Load a stored list the way the settings store does.
    fn load(json: &str) -> Vec<Trigger> {
        load_stored(&serde_json::from_str(json).expect("valid json")).expect("loads")
    }

    fn stored_row(gesture: &str, target: &str, shortcut: &str) -> String {
        format!(
            r#"{{ "gesture": "{gesture}", "target": "{target}", "binding": {{ "kind": "keyboard", "shortcut": "{shortcut}" }}, "enabled": true }}"#
        )
    }

    fn list(rows: &[String]) -> String {
        format!("[{}]", rows.join(","))
    }

    fn shortcut_of(t: &Trigger) -> String {
        match &t.binding {
            Some(Binding::Keyboard { shortcut }) => shortcut.clone(),
            other => panic!("expected a keyboard binding, got {other:?}"),
        }
    }

    #[test]
    fn a_push_to_talk_trigger_on_fn_becomes_a_hold_on_fn() {
        // A store from before gestures. `method: push_to_talk` on the globe
        // key comes back as exactly one row: Hold on the globe key, same
        // target, same switch, and an id.
        let migrated = load(
            r#"[{
                "method": "push_to_talk",
                "target": "dictation",
                "binding": { "kind": "keyboard", "shortcut": "Fn" },
                "phrase": null,
                "require_hey_prefix": false,
                "enabled": true
            }]"#,
        );
        assert_eq!(migrated.len(), 1, "one row in, one row out: {migrated:?}");
        let hold = &migrated[0];
        assert_eq!(hold.gesture, Gesture::Hold);
        assert_eq!(hold.target, TriggerTarget::Dictation);
        assert_eq!(shortcut_of(hold), "Fn");
        assert!(hold.enabled);
        assert!(!hold.id.is_empty(), "and now has a row identity");
        assert!(validate(&migrated, &[]).is_ok());
    }

    #[test]
    fn migration_maps_every_old_method_name() {
        let migrated = load(
            r#"[
            { "method": "push_to_talk", "target": "dictation", "binding": { "kind": "keyboard", "shortcut": "Option+Space" }, "enabled": true },
            { "method": "toggle", "target": "agent", "binding": { "kind": "keyboard", "shortcut": "Option+D" }, "enabled": true },
            { "method": "voice", "target": "agent", "binding": null, "phrase": "juno", "require_hey_prefix": false, "enabled": true }
        ]"#,
        );
        assert_eq!(migrated.len(), 3);
        assert_eq!(migrated[0].gesture, Gesture::Hold);
        assert_eq!(migrated[1].gesture, Gesture::Tap);
        assert_eq!(migrated[2].gesture, Gesture::Say);
        assert_eq!(migrated[2].phrase.as_deref(), Some("juno"));
        assert!(validate(&migrated, &[]).is_ok());
    }

    #[test]
    fn migration_keeps_an_unbound_row_unbound() {
        let migrated = load(
            r#"[{ "method": "push_to_talk", "target": "dictation", "binding": null, "enabled": false }]"#,
        );
        assert_eq!(migrated.len(), 1);
        assert_eq!(migrated[0].binding, None);
        assert!(!migrated[0].enabled, "and still switched off");
    }

    /* The retired double gestures ------------------------------------ */

    #[test]
    fn the_owners_setup_keeps_its_agent_hold_and_dictation_moves_to_fn_control() {
        // [Hold Fn -> agent, DoubleTapHold Fn -> dictation]
        let migrated = load(&list(&[
            stored_row("hold", "agent", "Fn"),
            stored_row("double_tap_hold", "dictation", "Fn"),
        ]));
        assert_eq!(migrated.len(), 2, "{migrated:?}");
        assert_eq!(migrated[0].gesture, Gesture::Hold);
        assert_eq!(migrated[0].target, TriggerTarget::Agent);
        assert_eq!(shortcut_of(&migrated[0]), "Fn");
        assert_eq!(migrated[1].gesture, Gesture::Hold);
        assert_eq!(migrated[1].target, TriggerTarget::Dictation);
        assert_eq!(shortcut_of(&migrated[1]), "Fn+Control");
        assert!(migrated[1].enabled);
        assert!(validate(&migrated, &[]).is_ok());
    }

    #[test]
    fn the_collision_is_settled_the_same_way_whichever_row_comes_first() {
        let migrated = load(&list(&[
            stored_row("double_tap_hold", "dictation", "Fn"),
            stored_row("hold", "agent", "Fn"),
        ]));
        assert_eq!(migrated.len(), 2, "{migrated:?}");
        // The double row keeps its place in the list, on the new key.
        assert_eq!(migrated[0].target, TriggerTarget::Dictation);
        assert_eq!(shortcut_of(&migrated[0]), "Fn+Control");
        assert_eq!(migrated[1].target, TriggerTarget::Agent);
        assert_eq!(shortcut_of(&migrated[1]), "Fn");
    }

    #[test]
    fn a_double_row_with_a_free_key_simply_becomes_a_hold() {
        for gesture in ["double_tap", "double_tap_hold"] {
            let migrated = load(&list(&[
                stored_row("hold", "agent", "Fn"),
                stored_row(gesture, "dictation", "Option+Space"),
            ]));
            assert_eq!(migrated.len(), 2, "{gesture}");
            assert_eq!(migrated[1].gesture, Gesture::Hold, "{gesture}");
            assert_eq!(shortcut_of(&migrated[1]), "Option+Space", "{gesture}");
        }
    }

    #[test]
    fn a_colliding_agent_double_row_is_dropped_not_double_bound() {
        for gesture in ["double_tap", "double_tap_hold"] {
            let migrated = load(&list(&[
                stored_row("hold", "dictation", "Fn"),
                stored_row(gesture, "agent", "Fn"),
            ]));
            assert_eq!(migrated.len(), 1, "{gesture}: {migrated:?}");
            assert_eq!(migrated[0].target, TriggerTarget::Dictation);
            assert_eq!(shortcut_of(&migrated[0]), "Fn");
        }
    }

    #[test]
    fn a_double_tap_on_a_tap_key_gives_way_too() {
        // Tap + double tap and hold used to share a key; Hold + Tap cannot.
        let migrated = load(&list(&[
            stored_row("tap", "dictation", "Option+D"),
            stored_row("double_tap_hold", "agent", "Option+D"),
        ]));
        assert_eq!(migrated.len(), 1, "{migrated:?}");
        assert_eq!(migrated[0].gesture, Gesture::Tap);
    }

    #[test]
    fn dictation_is_dropped_if_even_the_default_key_is_taken() {
        let migrated = load(&list(&[
            stored_row("hold", "agent", "Fn"),
            stored_row("hold", "agent", "Fn+Control"),
            stored_row("double_tap", "dictation", "Fn"),
        ]));
        assert_eq!(migrated.len(), 2, "never double-bound: {migrated:?}");
        assert!(migrated.iter().all(|t| t.target == TriggerTarget::Agent));
        assert!(validate(&migrated, &[]).is_ok());
    }

    #[test]
    fn two_double_rows_on_one_key_leave_one_hold() {
        let migrated = load(&list(&[
            stored_row("double_tap", "agent", "Fn"),
            stored_row("double_tap_hold", "agent", "Fn"),
        ]));
        assert_eq!(migrated.len(), 1, "{migrated:?}");
        assert_eq!(migrated[0].gesture, Gesture::Hold);
    }

    #[test]
    fn a_switched_off_double_row_keeps_its_place_and_its_switch() {
        let migrated = load(
            r#"[
            { "gesture": "hold", "target": "agent", "binding": { "kind": "keyboard", "shortcut": "Fn" }, "enabled": true },
            { "gesture": "double_tap_hold", "target": "dictation", "binding": { "kind": "keyboard", "shortcut": "Fn" }, "enabled": false }
        ]"#,
        );
        assert_eq!(migrated.len(), 2);
        assert_eq!(migrated[1].gesture, Gesture::Hold);
        assert!(!migrated[1].enabled);
        assert_eq!(shortcut_of(&migrated[1]), "Fn", "off rows hold no key");
        assert!(validate(&migrated, &[]).is_ok());
    }

    #[test]
    fn migration_never_invents_a_row() {
        // The property, not the example: across every shape of old store, the
        // row count never goes up.
        let cases: Vec<String> = vec![
            "[]".to_string(),
            list(&[stored_row("hold", "dictation", "Fn")]),
            list(&[
                stored_row("hold", "agent", "Fn"),
                stored_row("double_tap_hold", "dictation", "Fn"),
                stored_row("double_tap", "agent", "Fn"),
                stored_row("tap", "agent", "Option+D"),
            ]),
        ];
        for json in cases {
            let before = serde_json::from_str::<Vec<serde_json::Value>>(&json)
                .unwrap()
                .len();
            let after = load(&json).len();
            assert!(after <= before, "{after} rows from {before}: {json}");
        }
    }

    #[test]
    fn migration_is_idempotent_through_a_save_and_reload() {
        let once = load(&list(&[
            stored_row("hold", "agent", "Fn"),
            stored_row("double_tap_hold", "dictation", "Fn"),
            stored_row("double_tap", "agent", "Option+D"),
            stored_row("tap", "dictation", "Option+Space"),
        ]));
        // What the store writes, read back through the same door.
        let written = serde_json::to_value(&once).expect("serialize");
        assert!(
            !written.to_string().contains("double_tap"),
            "nothing writes a retired gesture: {written}"
        );
        let twice = load_stored(&written).expect("reload");
        assert_eq!(twice, once, "the second pass changes nothing");
        let thrice = load_stored(&serde_json::to_value(&twice).unwrap()).unwrap();
        assert_eq!(thrice, once);
    }

    #[test]
    fn running_the_migration_on_an_unsaved_list_twice_gives_the_same_rows() {
        // The load is not written back until the next save, so every launch
        // reads the same old file. The rows must come out the same each time,
        // apart from the ids handed to rows that had none.
        let json = list(&[
            stored_row("hold", "agent", "Fn"),
            stored_row("double_tap_hold", "dictation", "Fn"),
        ]);
        let strip = |ts: Vec<Trigger>| -> Vec<(Gesture, TriggerTarget, Option<Binding>, bool)> {
            ts.into_iter()
                .map(|t| (t.gesture, t.target, t.binding, t.enabled))
                .collect()
        };
        assert_eq!(strip(load(&json)), strip(load(&json)));
    }

    #[test]
    fn a_deleted_row_stays_deleted_through_save_and_reload() {
        // The report: "I removed the double tap and it came back." A list the
        // person has pruned is written, read back and migrated, and nothing
        // reappears.
        let mut rows = load(&list(&[
            stored_row("hold", "agent", "Fn"),
            stored_row("hold", "dictation", "Fn+Control"),
            stored_row("tap", "agent", "Option+D"),
        ]));
        rows.retain(|t| t.gesture != Gesture::Tap);
        let reloaded =
            load_stored(&serde_json::to_value(&rows).expect("serialize")).expect("reload");
        assert_eq!(reloaded, rows);
        assert_eq!(reloaded.len(), 2);
    }

    #[test]
    fn an_unknown_gesture_is_still_refused() {
        let v: serde_json::Value =
            serde_json::from_str(&list(&[stored_row("triple_tap", "agent", "Fn")])).unwrap();
        assert!(load_stored(&v).is_err());
    }

    /* Serde back-compat ---------------------------------------------- */

    #[test]
    fn a_retired_gesture_name_still_reads_and_is_a_hold() {
        for name in ["double_tap", "double_tap_hold"] {
            let g: Gesture = serde_json::from_str(&format!("\"{name}\"")).expect("still readable");
            assert_eq!(g, Gesture::Hold, "{name}");
        }
        // And a row from a window that has not reloaded still deserializes.
        let t: Trigger = serde_json::from_str(
            r#"{ "id": "x", "gesture": "double_tap_hold", "target": "dictation",
                 "binding": { "kind": "keyboard", "shortcut": "Fn" } }"#,
        )
        .expect("a stale window's row still parses");
        assert_eq!(t.gesture, Gesture::Hold);
    }

    #[test]
    fn ensure_ids_replaces_a_repeated_identity() {
        let mut ts = vec![
            row(Gesture::Hold, TriggerTarget::Agent, "Fn"),
            row(Gesture::Tap, TriggerTarget::Agent, "Option+D"),
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
        assert_eq!(Gesture::ALL.len(), 3, "hold, tap, say and nothing else");
    }

    #[test]
    fn a_gesture_row_round_trips_through_the_store() {
        let ts = vec![row(Gesture::Hold, TriggerTarget::Dictation, "Fn+Control")];
        let written = serde_json::to_string(&ts).expect("serialize");
        assert!(written.contains("\"hold\""), "got: {written}");
        assert!(!written.contains("\"method\""), "one live field name");
        let reread: Vec<Trigger> = serde_json::from_str(&written).expect("deserialize");
        assert_eq!(reread, ts);
    }

    /* ---------------------------------------------------------------- */
    /* Defaults                                                         */
    /* ---------------------------------------------------------------- */

    #[test]
    fn the_defaults_are_hold_globe_to_talk_and_hold_globe_control_to_dictate() {
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
                shortcut: "Fn+Control".to_string()
            })
        );

        assert!(validate(&ts, &["Escape".to_string()]).is_ok());
        assert!(ts.iter().all(|t| !t.id.is_empty()), "every row has an id");
    }

    #[test]
    fn both_defaults_are_watched_by_the_modifier_monitor_and_are_two_keys() {
        let ts = default_triggers();
        let watchers: Vec<Watcher> = bound_keys(&ts).iter().map(watcher_for).collect();
        assert_eq!(
            watchers,
            vec![
                Watcher::ModifierKey(ModifierKey::FN),
                Watcher::ModifierKey(ModifierKey::FN_CONTROL)
            ]
        );
    }

    #[test]
    fn the_defaults_survive_a_trip_through_the_store_unchanged() {
        let ts = default_triggers();
        let reloaded = load_stored(&serde_json::to_value(&ts).unwrap()).unwrap();
        assert_eq!(reloaded, ts);
    }

    #[test]
    fn no_default_fires_on_a_single_tap() {
        // The standing rule: a keyboard trigger never fires on a single tap of
        // a hold key. Both defaults are Hold, so neither key has a tap meaning
        // at all.
        for t in default_triggers() {
            assert_eq!(t.gesture, Gesture::Hold);
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
            row(Gesture::Tap, TriggerTarget::Agent, "Option+D"),
            row(Gesture::Hold, TriggerTarget::Agent, "Fn"),
        ];
        assert_eq!(hint_for(&ts, TriggerTarget::Agent).unwrap().shortcut, "Fn");
    }

    #[test]
    fn the_hint_for_the_chord_is_the_chord() {
        let ts = default_triggers();
        let hint = hint_for(&ts, TriggerTarget::Dictation).expect("a hint");
        assert_eq!(hint.shortcut, "Fn+Control");
        assert_eq!(hint.sentence, "Hold to dictate");
    }

    /* ---------------------------------------------------------------- */
    /* Grouping gestures by key                                         */
    /* ---------------------------------------------------------------- */

    #[test]
    fn the_gestures_on_one_key_are_read_off_the_list() {
        let ts = vec![
            row(Gesture::Hold, TriggerTarget::Agent, "globe"),
            row(Gesture::Hold, TriggerTarget::Dictation, "Fn+Control"),
            row(Gesture::Tap, TriggerTarget::Dictation, "Option+Space"),
        ];
        let globe = gestures_on_key(&ts, "fn (globe)");
        assert_eq!(globe.hold, Some(TriggerTarget::Agent));
        assert_eq!(globe.tap, None);
        assert_eq!(globe.targets(), vec![TriggerTarget::Agent]);

        let chord = gestures_on_key(&ts, "fn + control");
        assert_eq!(chord.hold, Some(TriggerTarget::Dictation));

        let space = gestures_on_key(&ts, "option+space");
        assert_eq!(space.tap, Some(TriggerTarget::Dictation));
        assert_eq!(space.hold, None);
    }

    #[test]
    fn a_switched_off_row_binds_no_gesture() {
        let mut ts = vec![row(Gesture::Hold, TriggerTarget::Agent, "Fn")];
        ts[0].enabled = false;
        assert!(gestures_on_key(&ts, "fn (globe)").is_empty());
    }

    #[test]
    fn a_key_used_by_several_rows_is_registered_once() {
        // A hand-edited store can name one key twice; it is still one key.
        let ts = vec![
            row(Gesture::Hold, TriggerTarget::Agent, "Fn"),
            row(Gesture::Hold, TriggerTarget::Dictation, "globe"),
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
    fn a_row_may_not_take_another_rows_key() {
        let mut ts = vec![row(Gesture::Hold, TriggerTarget::Agent, "Fn")];
        let mut second = trigger(Gesture::Hold, TriggerTarget::Dictation, None);
        let second_id = second.id.clone();
        second.enabled = false; // unbound rows persist switched off
        ts.push(second);
        let msg =
            combo_conflict(&ts, "Fn", Some(&second_id), &[]).expect("one key means one thing");
        assert!(msg.contains("Hold to talk to Juno"), "got: {msg}");
        assert_eq!(
            combo_conflict(&ts, "Fn+Control", Some(&second_id), &[]),
            None,
            "the chord is a different key"
        );
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
    fn a_saved_say_trigger_arms_nothing_while_voice_is_gated() {
        // Somebody who switched a wake phrase on before it was gated must not
        // launch into an open microphone.
        let t = vec![voice("juno", TriggerTarget::Agent, true)];
        assert!(armed_voice_phrases(&t, false).is_empty());
        assert_eq!(armed_voice_phrases(&t, true), vec!["juno", "hey juno"]);
    }

    #[test]
    fn the_gate_never_arms_a_disabled_say_trigger() {
        let t = vec![voice("juno", TriggerTarget::Agent, false)];
        assert!(armed_voice_phrases(&t, true).is_empty());
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
            trigger(Gesture::Tap, TriggerTarget::Agent, None),
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
            Watcher::ModifierKey(ModifierKey::FN)
        );
        assert_eq!(
            watcher_for(&Binding::Keyboard {
                shortcut: "globe".to_string()
            }),
            Watcher::ModifierKey(ModifierKey::FN),
            "Apple calls the same key both things"
        );
    }

    #[test]
    fn the_globe_control_chord_goes_to_the_modifier_watcher_in_any_spelling() {
        for spelling in [
            "Fn+Control",
            "fn+control",
            "Control+Fn",
            "globe+ctrl",
            " Fn + Control ",
        ] {
            assert_eq!(
                watcher_for(&Binding::Keyboard {
                    shortcut: spelling.to_string()
                }),
                Watcher::ModifierKey(ModifierKey::FN_CONTROL),
                "{spelling}"
            );
        }
    }

    #[test]
    fn the_chord_names_itself_the_way_a_person_would() {
        let label = Binding::Keyboard {
            shortcut: "Fn+Control".to_string(),
        }
        .describe();
        assert_eq!(label, "Fn + Control");
        assert_eq!(ModifierKey::FN_CONTROL.shortcut(), "Fn+Control");
        assert_eq!(
            bare_modifier(&ModifierKey::FN_CONTROL.shortcut()),
            Some(ModifierKey::FN_CONTROL)
        );
        assert_eq!(
            bare_modifier(&ModifierKey::FN.shortcut()),
            Some(ModifierKey::FN)
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
        for combo in [
            "Fn+F5",
            "Option",
            "Shift",
            "Control+Space",
            "Fn+Control+Space",
            "Fn+",
        ] {
            assert_eq!(
                watcher_for(&Binding::Keyboard {
                    shortcut: combo.to_string()
                }),
                Watcher::GlobalShortcut,
                "{combo} is not a bare modifier chord"
            );
        }
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
            voice("juno", TriggerTarget::Agent, true),
        ];
        let (a_combo, a_mode, d_combo, d_mode, al, words) = derive_legacy(&ts, "old+a", "old+d");
        assert_eq!(a_combo, "Option+D");
        assert_eq!(a_mode, "tap");
        assert_eq!(d_combo, "Option+Space");
        assert_eq!(d_mode, "hold");
        assert!(al);
        assert_eq!(words, vec!["juno", "hey juno"]);
    }

    #[test]
    fn derive_legacy_keeps_previous_combo_for_the_chord() {
        let ts = vec![row(Gesture::Hold, TriggerTarget::Dictation, "Fn+Control")];
        let (_, _, d_combo, d_mode, ..) = derive_legacy(&ts, "kept+a", "kept+d");
        assert_eq!(
            d_combo, "kept+d",
            "a chord of bare modifiers is not a combo string"
        );
        assert_eq!(d_mode, "hold");
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

    /* ---------------------------------------------------------------- */
    /* Keyboards with no Fn key                                         */
    /* ---------------------------------------------------------------- */

    fn keyboard_shortcut(t: &Trigger) -> Option<&str> {
        match t.binding.as_ref() {
            Some(Binding::Keyboard { shortcut }) => Some(shortcut.as_str()),
            _ => None,
        }
    }

    #[test]
    fn right_option_and_control_are_bare_modifiers() {
        for (spelling, key) in [
            ("RightOption", ModifierKey::RIGHT_OPTION),
            ("right option", ModifierKey::RIGHT_OPTION),
            ("Control", ModifierKey::CONTROL),
            ("ctrl", ModifierKey::CONTROL),
        ] {
            assert_eq!(bare_modifier(spelling), Some(key), "{spelling}");
            assert_eq!(
                watcher_for(&Binding::Keyboard {
                    shortcut: spelling.to_string()
                }),
                Watcher::ModifierKey(key)
            );
        }
        // Their own shortcut strings read back as themselves.
        assert_eq!(
            bare_modifier(&ModifierKey::RIGHT_OPTION.shortcut()),
            Some(ModifierKey::RIGHT_OPTION)
        );
        assert_eq!(
            bare_modifier(&ModifierKey::CONTROL.shortcut()),
            Some(ModifierKey::CONTROL)
        );
        // Left Option is an ordinary modifier, not a bare hold.
        assert_eq!(bare_modifier("Option"), None);
        assert_eq!(bare_modifier("Control+Space"), None);
    }

    #[test]
    fn with_no_globe_key_the_defaults_fall_back_to_right_option_and_control() {
        let mut ts = default_triggers();
        let ids: Vec<String> = ts.iter().map(|t| t.id.clone()).collect();
        assert!(settle_for_keyboard(&mut ts, false));
        assert_eq!(ts.len(), 2, "rebinds the rows, adds none");
        assert_eq!(ts[0].id, ids[0]);
        assert_eq!(keyboard_shortcut(&ts[0]), Some("RightOption"));
        assert_eq!(keyboard_shortcut(&ts[1]), Some("Control"));
        assert!(ts.iter().all(|t| t.keyboard_fallback));
        assert!(validate(&ts, &["Escape".to_string()]).is_ok());
        let watchers: Vec<Watcher> = bound_keys(&ts).iter().map(watcher_for).collect();
        assert_eq!(
            watchers,
            vec![
                Watcher::ModifierKey(ModifierKey::RIGHT_OPTION),
                Watcher::ModifierKey(ModifierKey::CONTROL)
            ]
        );
        // Settling again changes nothing.
        let once = ts.clone();
        assert!(!settle_for_keyboard(&mut ts, false));
        assert_eq!(ts, once);
    }

    #[test]
    fn a_globe_key_appearing_puts_the_defaults_back() {
        let mut ts = default_triggers();
        settle_for_keyboard(&mut ts, false);
        assert!(settle_for_keyboard(&mut ts, true));
        assert_eq!(keyboard_shortcut(&ts[0]), Some("Fn"));
        assert_eq!(keyboard_shortcut(&ts[1]), Some("Fn+Control"));
        assert!(ts.iter().all(|t| !t.keyboard_fallback));
        assert!(!settle_for_keyboard(&mut ts, true), "nothing left to do");
    }

    #[test]
    fn a_globe_key_being_present_leaves_the_defaults_alone() {
        let mut ts = default_triggers();
        let before = ts.clone();
        assert!(!settle_for_keyboard(&mut ts, true));
        assert_eq!(ts, before);
    }

    #[test]
    fn customized_triggers_are_never_moved() {
        // Somebody chose Right Option and Control themselves. A globe key
        // turning up must not take them away.
        let mut chosen = vec![
            row(Gesture::Hold, TriggerTarget::Agent, "RightOption"),
            row(Gesture::Hold, TriggerTarget::Dictation, "Control"),
        ];
        let before = chosen.clone();
        assert!(!settle_for_keyboard(&mut chosen, true));
        assert_eq!(chosen, before);

        // And keys that are not a default stay where they are with no globe
        // key either.
        let mut other = vec![
            row(Gesture::Hold, TriggerTarget::Agent, "Option+Space"),
            row(Gesture::Tap, TriggerTarget::Dictation, "Fn"),
            row(Gesture::Hold, TriggerTarget::Dictation, "Control+Option"),
        ];
        let before = other.clone();
        assert!(!settle_for_keyboard(&mut other, false));
        assert_eq!(other, before);
    }

    #[test]
    fn rebinding_a_fallback_row_makes_it_the_persons() {
        let mut stored = default_triggers();
        settle_for_keyboard(&mut stored, false);
        let mut next = stored.clone();
        next[0].binding = Some(Binding::Keyboard {
            shortcut: "Control+Option".to_string(),
        });
        clear_fallback_on_rebind(&stored, &mut next);
        assert!(!next[0].keyboard_fallback, "the person chose this key");
        assert!(next[1].keyboard_fallback, "an untouched row keeps its mark");

        // Back to the person's own choice of Right Option: still theirs.
        next[0].binding = Some(Binding::Keyboard {
            shortcut: "RightOption".to_string(),
        });
        let mut later = next.clone();
        clear_fallback_on_rebind(&next, &mut later);
        settle_for_keyboard(&mut later, true);
        assert_eq!(keyboard_shortcut(&later[0]), Some("RightOption"));
        assert_eq!(keyboard_shortcut(&later[1]), Some("Fn+Control"));
    }

    #[test]
    fn the_fallback_never_lands_on_a_key_another_row_uses() {
        let mut ts = default_triggers();
        ts.push(row(Gesture::Hold, TriggerTarget::Agent, "Control"));
        settle_for_keyboard(&mut ts, false);
        assert_eq!(keyboard_shortcut(&ts[0]), Some("RightOption"));
        assert_eq!(
            keyboard_shortcut(&ts[1]),
            Some("Fn+Control"),
            "Control is taken, so dictation stays put"
        );
        assert!(!ts[1].keyboard_fallback);
        assert!(validate(&ts, &[]).is_ok());
    }

    #[test]
    fn the_fallback_mark_round_trips_and_is_absent_when_unset() {
        let mut ts = default_triggers();
        let plain = serde_json::to_value(&ts).expect("serializes");
        assert!(plain[0].get("keyboard_fallback").is_none());
        settle_for_keyboard(&mut ts, false);
        let json = serde_json::to_value(&ts).expect("serializes");
        assert_eq!(load_stored(&json).expect("loads"), ts);
    }

    /* ---------------------------------------------------------------- */
    /* Modifier chords                                                  */
    /* ---------------------------------------------------------------- */

    #[test]
    fn every_globe_chord_is_recordable_next_to_the_globe_key() {
        // The reported defect: with Hold Fn on one row, recording Fn+Control,
        // Fn+Option, Fn+Shift or Fn+Command on another was refused as
        // "Fn (globe) already has Hold to talk to Juno". A chord is its own key.
        let mut ts = vec![row(Gesture::Hold, TriggerTarget::Agent, "Fn")];
        for chord in ["Fn+Control", "Fn+Option", "Fn+Shift", "Fn+Command"] {
            assert_eq!(combo_conflict(&ts, chord, None, &[]), None, "{chord}");
            ts.push(row(Gesture::Hold, TriggerTarget::Dictation, chord));
        }
        assert!(validate(&ts, &[]).is_ok(), "{:?}", validate(&ts, &[]));
        assert_eq!(bound_keys(&ts).len(), 5);
    }

    #[test]
    fn the_identical_binding_is_still_refused() {
        let ts = vec![row(Gesture::Hold, TriggerTarget::Agent, "Fn+Option")];
        let err = combo_conflict(&ts, "Option+Fn", None, &[]).expect("same set, other order");
        assert!(err.contains("Fn + Option"), "{err}");
        assert!(err.contains("Hold to talk to Juno"), "{err}");
        let twice = vec![
            row(Gesture::Hold, TriggerTarget::Agent, "Control+Option"),
            row(Gesture::Hold, TriggerTarget::Dictation, "option+ctrl"),
        ];
        assert!(validate(&twice, &[]).is_err());
    }

    #[test]
    fn control_and_control_option_coexist() {
        let ts = vec![
            row(Gesture::Hold, TriggerTarget::Dictation, "Control"),
            row(Gesture::Hold, TriggerTarget::Agent, "Control+Option"),
            row(Gesture::Tap, TriggerTarget::Agent, "Option+Command"),
            row(Gesture::Hold, TriggerTarget::Dictation, "Control+Shift"),
        ];
        assert!(validate(&ts, &[]).is_ok(), "{:?}", validate(&ts, &[]));
        assert_eq!(bound_keys(&ts).len(), 4);
    }

    #[test]
    fn any_two_modifiers_are_a_chord_the_monitor_watches() {
        for (spelling, shortcut, label) in [
            ("Control+Option", "Control+Option", "Control + Option"),
            ("alt+ctrl", "Control+Option", "Control + Option"),
            ("Cmd+Shift", "Shift+Command", "Shift + Command"),
            ("Option+Command", "Option+Command", "Option + Command"),
            ("globe+shift", "Fn+Shift", "Fn + Shift"),
            (
                "Control+Option+Shift",
                "Control+Option+Shift",
                "Control + Option + Shift",
            ),
            ("RightOption+Control", "Control+Option", "Control + Option"),
        ] {
            let key = bare_modifier(spelling).unwrap_or_else(|| panic!("{spelling}"));
            assert_eq!(key.shortcut(), shortcut, "{spelling}");
            assert_eq!(key.label(), label, "{spelling}");
            assert_eq!(bare_modifier(&key.shortcut()), Some(key), "{spelling}");
            assert_eq!(
                watcher_for(&Binding::Keyboard {
                    shortcut: spelling.to_string()
                }),
                Watcher::ModifierKey(key)
            );
        }
        // A lone Shift, Command or Option is part of every other shortcut.
        for lone in ["Shift", "Command", "Option", "Control+Control", "Fn+Fn"] {
            assert_eq!(bare_modifier(lone), None, "{lone}");
        }
    }

    #[test]
    fn a_chord_may_be_tapped() {
        let ts = vec![row(Gesture::Tap, TriggerTarget::Agent, "Control+Option")];
        assert!(validate(&ts, &[]).is_ok());
    }

    #[test]
    fn the_retired_modifier_kind_still_reads_every_spelling() {
        for (stored, shortcut) in [
            ("fn", "Fn"),
            ("fn_control", "Fn+Control"),
            ("control", "Control"),
            ("right_option", "RightOption"),
        ] {
            let json = format!(r#"{{ "kind": "modifier", "key": "{stored}" }}"#);
            let b: Binding = serde_json::from_str(&json).expect("reads");
            assert_eq!(
                b,
                Binding::Keyboard {
                    shortcut: shortcut.to_string()
                }
            );
        }
    }

    #[test]
    fn hold_control_and_hold_fn_control_do_not_collide() {
        let ts = vec![
            row(Gesture::Hold, TriggerTarget::Agent, "Control"),
            row(Gesture::Hold, TriggerTarget::Dictation, "Fn+Control"),
            row(Gesture::Hold, TriggerTarget::Agent, "Fn"),
            row(Gesture::Hold, TriggerTarget::Dictation, "RightOption"),
        ];
        assert!(validate(&ts, &[]).is_ok());
        let sigs: std::collections::HashSet<String> =
            ts.iter().filter_map(|t| t.key_signature()).collect();
        assert_eq!(sigs.len(), 4, "four different keys");
    }

    #[test]
    fn right_option_does_not_collide_with_left_option_combos_or_a_second_use() {
        let ts = vec![
            row(Gesture::Hold, TriggerTarget::Agent, "RightOption"),
            row(Gesture::Hold, TriggerTarget::Dictation, "Option+Space"),
        ];
        assert!(validate(&ts, &[]).is_ok());

        // But the same key twice is still one key.
        let twice = vec![
            row(Gesture::Hold, TriggerTarget::Agent, "RightOption"),
            row(Gesture::Hold, TriggerTarget::Dictation, "right option"),
        ];
        assert!(validate(&twice, &[]).is_err());
        assert!(combo_conflict(&ts, "RightOption", None, &[]).is_some());
        assert!(combo_conflict(&ts, "Control", None, &[]).is_none());
    }

    #[test]
    fn control_and_right_option_can_only_be_held() {
        for shortcut in ["Control", "RightOption"] {
            let ts = vec![row(Gesture::Tap, TriggerTarget::Agent, shortcut)];
            let err = validate(&ts, &[]).unwrap_err();
            assert!(err.contains("can only be held"), "{shortcut}: {err}");
            let ok = vec![row(Gesture::Hold, TriggerTarget::Agent, shortcut)];
            assert!(validate(&ok, &[]).is_ok());
        }
        // Fn may still be tapped.
        let fn_tap = vec![row(Gesture::Tap, TriggerTarget::Agent, "Fn")];
        assert!(validate(&fn_tap, &[]).is_ok());
    }
}
