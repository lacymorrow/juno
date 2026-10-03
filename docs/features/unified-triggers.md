# Triggers: unified activation

Juno is summoned through **triggers**. A trigger pairs a **gesture** (how it
fires) with a **target** (what it does), and reads as a sentence: *Hold the
globe key to talk to Juno*. This replaced the older setup where activation was
spread across three settings screens (key combos, agent tap/hold, dictation
tap/hold plus wake words).

Shipped in PR #520. Gestures replaced the three-method model in the LAC-4070
rebuild; the design is `docs/plans/trigger-gestures.md`.

## The model

**Every gesture is a trigger in its own right**: a row with its own key and its
own target, which the person can see and delete. A row is its `id` and nothing
else, so two rows may share a gesture, a target, or both, as long as they sit on
different keys.

| Gesture | Starts | Ends |
| --- | --- | --- |
| Hold | key down | release |
| Tap | release of a press | next press of the same key |
| Say | wake phrase | end of speech |

There used to be two double-tap gestures. They were removed (2026-10-03)
because they did not work well; see Migration for what happens to a stored row.

A short press of a Hold key (under `HOLD_DURATION_MS`, 400 ms) cancels the
session it opened, because a short tap on a hold key is almost always a fumbled
hold. **No keyboard trigger fires on a single tap of a hold key.**

`Say` takes a wake phrase instead of a key, with an optional "hey" prefix
("juno" or "hey juno"; require the prefixed form with the toggle). Say phrases
must be unique across rows. Wake phrases only arm behind advanced settings.

**Target** decides what the activation drives: the agent, or dictation (speech
typed at the cursor). Voice routing is per phrase, so "juno" can wake the agent
while "transcribe" starts dictation.

A binding is either a **key combo** or a **mouse button**. The globe key is a
keyboard binding whose shortcut string is `"Fn"`, and the globe key held with
Control is `"Fn+Control"`. Neither produces an ordinary key event, so both need
a native observer rather than the global-shortcut plugin; that is a fact about
the watching, not about the binding.

## One key, one thing

Two enabled rows may not share a key, whatever their gestures or targets.
`triggers::validate` and `triggers::combo_conflict` enforce it, so the hint
shown while someone is still choosing a key and the result of saving cannot
disagree. `Fn` and `Fn+Control` are different keys.

## Defaults

A fresh install gets two rows, both Hold, so neither key carries a second
meaning on a single tap:

- Hold **Fn** (the globe key) to talk to Juno
- Hold **Fn+Control** to dictate

Nothing guesses whether the keyboard has a globe key. There is no reliable way
to know once a second keyboard is plugged in, and a press is the only proof the
key reaches Juno, so setup asks the person to press it (`set_trigger_capture`).

### The Fn + Control chord

`platform::modifier_key_monitor::ModifierState` reads both halves out of the
`flagsChanged` flag word: Fn is key code 63 with bit `1 << 23`, Control is key
code 59 or 62 with bit `1 << 18`. The chord is down while both bits are set and
up as soon as either clears. With Hold Fn and Hold Fn+Control both bound, the
chord takes the finger from the plain key: pressing Control while holding Fn
releases the plain hold (a short hold is cancelled like any brief press) and
starts the chord, and the plain key stays quiet until Fn itself comes up.

## The recognizer

One recognizer per bound key, in `events::shortcuts::recognizer`, in front of
the agent and dictation monitors. Every input source reaches it through
`fire_key_edge`: the global-shortcut plugin, `platform::modifier_key_monitor`
for the globe key and its chord, and `platform::mouse_button_monitor`. A key
used by several rows is registered **once**.

It holds no session state. Per key it remembers one bit: that a press was spent
stopping a tap session, so its release is swallowed. "Is a tap session running"
is read back out of the voice-session registry, which records how each session
was opened.

Resolution:

- **A tap session is running:** the next press of that key stops it on the down
  edge, and its release is swallowed.
- **Otherwise a Hold key** starts on the down edge and ends on the release; a
  **Tap key** acts on the release.

## Errors

`triggers::issues` derives every problem with a list each time it is asked,
against the row that caused it, and nothing is stored. That is the whole reason
a conflict message cannot outlive its cause: the settings screen drops any
standing refusal the moment a save is accepted, because a list the backend
accepted has no conflict in it.

## Where it lives

Rust owns all activation logic; the frontend is display-only.

| Concern | Location |
| ------- | -------- |
| Data model, migration, validation | `src-tauri/src/triggers/mod.rs` |
| Gesture recognizer + routing | `src-tauri/src/events/shortcuts.rs` |
| Read/write commands | `src-tauri/src/commands/triggers.rs` (`get_triggers`, `set_triggers`, `get_trigger_hints`) |
| Persistence + in-memory cache | `settings/` (a `triggers` store key) and `AppState` |
| Key registration | `src-tauri/src/commands/shortcuts.rs` |
| Globe-key observer | `src-tauri/src/platform/modifier_key_monitor.rs` |
| Mouse-button observer | `src-tauri/src/platform/mouse_button_monitor.rs` |
| Voice engine + routing | `tauri-plugin-voice-transcription/src/always_listening.rs`, `src-tauri/src/integration.rs` |
| Settings UI | `src/components/settings/sections/TriggersSettings.tsx` |
| Onboarding's key caps | `src/components/onboarding/Onboarding.tsx` (`summonDemo`) |

`triggers` is the source of truth. The legacy fields (`KeyboardShortcuts`,
`AgentSettings.trigger_mode`, the always-listening wake words) are **derived**
from it (`triggers::derive_legacy`), so existing runtime consumers keep working
without a rewrite. Only Hold and Tap project onto those fields.

## Who says which key is bound

Onboarding and the spoken greeting both teach a key, and neither may invent
one. `triggers::hint_for` answers "which key, and in what words" from the live
list; `get_trigger_hints` hands both targets' answers to onboarding, which
draws the caps from them. Onboarding used to fall back to a hardcoded
`Option+D`, so someone whose trigger was the globe key finished setup having
been taught a shortcut that did nothing.

## Migration

- **No stored triggers:** the list is synthesized from the legacy shortcut,
  tap/hold, and wake-word settings (`migrate_from_legacy`), so an upgrading
  user keeps their setup.
- **Stored triggers from before gestures, or naming a retired gesture:**
  `triggers::load_stored` is the one door a stored list comes through. It runs
  on every load, never adds a row, and is idempotent: what it returns writes
  back as a list it leaves alone. `push_to_talk` reads as Hold, `toggle` as Tap
  and `voice` as Say, each keeping its key, its target and its switch, and a
  row without an id is given one.

  A stored `double_tap` or `double_tap_hold` row becomes a Hold on the same key.
  If that would put two rows on one key, the double row gives way:

  | Stored double row | Key already held by another row | Result |
  | --- | --- | --- |
  | any target | no | Hold on the same key |
  | dictation | yes | Hold on `Fn+Control` (dropped if that is taken too) |
  | agent | yes | dropped, never double-bound |

  So `[Hold Fn -> agent, DoubleTapHold Fn -> dictation]` becomes
  `[Hold Fn -> agent, Hold Fn+Control -> dictation]`. Serde still reads the old
  names, and nothing writes them.

## Adding a gesture or target later

Add a variant to `Gesture` or `TriggerTarget` in `triggers/mod.rs`, give it a
label and an ending, handle it in the recognizer and
in `perform` (the compiler will point at the non-exhaustive matches), and list
it in `TriggersSettings.tsx`.
