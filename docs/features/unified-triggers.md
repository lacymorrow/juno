# Triggers: unified activation

Juno is summoned through **triggers**. A trigger pairs a **gesture** (how it
fires) with a **target** (what it does), and reads as a sentence: *Hold the
globe key to talk to Juno*. This replaced the older setup where activation was
spread across three settings screens (key combos, agent tap/hold, dictation
tap/hold plus wake words).

Shipped in PR #520. Gestures replaced the three-method model in the LAC-4070
rebuild; the design is `docs/plans/trigger-gestures.md`.

## The model

**Every gesture is a trigger in its own right.** A double tap is not a second
way to reach a hold trigger; it is another row, with its own key and its own
target, which the person can see and delete. A row is its `id` and nothing
else, so two rows may share a gesture, a target, or both, as long as the
sharing table below allows their gestures on one key.

| Gesture | Starts | Ends |
| --- | --- | --- |
| Hold | key down | release |
| Tap | release of a press | next press of the same key |
| Double tap | second press inside `DOUBLE_TAP_WINDOW_MS` (300 ms) | next press of the same key |
| Double tap and hold | second press still held at `SECOND_PRESS_HOLD_MS` (250 ms) | release |
| Say | wake phrase | end of speech |

A short press of a Hold key (under `HOLD_DURATION_MS`, 400 ms) cancels the
session it opened, because a short tap on a hold key is almost always a fumbled
hold. **No keyboard trigger fires on a single tap of a hold key**, and a key
carrying only double gestures does nothing at all on a first press.

`Say` takes a wake phrase instead of a key, with an optional "hey" prefix
("juno" or "hey juno"; require the prefixed form with the toggle). Say phrases
must be unique across rows.

**Target** decides what the activation drives: the agent, or dictation (speech
typed at the cursor). Voice routing is per phrase, so "juno" can wake the agent
while "transcribe" starts dictation.

A binding is either a **key combo** or a **mouse button**. The globe key is a
keyboard binding whose shortcut string is `"Fn"`; that it needs a native
observer rather than the global-shortcut plugin is a fact about the watching,
not about the binding.

## Which gestures can share a key

| On one key | Allowed |
| --- | --- |
| Hold + Double tap | yes |
| Hold + Double tap and hold | yes |
| Double tap + Double tap and hold | yes |
| Hold + Double tap + Double tap and hold | yes |
| Tap + Double tap and hold | yes |
| Hold + Tap | no |
| Tap + Double tap | no |

Tap cannot share a key with Hold or Double tap: a short tap on a hold key is
usually a fumbled hold, and a quick stop-tap on a tap key looks exactly like
the first half of a double tap. Each gesture appears at most once per key,
including a gesture pointing at the other target, because one key cannot mean
two things on the same edge.

`triggers::can_share_key` is the table. `triggers::validate` and
`triggers::combo_conflict` enforce it and nothing looser, so the hint shown
while someone is still choosing a key and the result of saving cannot disagree.

**Hold + Double tap and hold on one key is the headline case:** hold the globe
key to talk to Juno, double-tap-and-hold the same key to dictate. Two rows, two
targets, neither derived from the other.

## Defaults

A fresh install gets two rows, both Hold, so neither key carries a second
meaning on a single tap:

- Hold **🌐 (Fn)** to talk to Juno
- Hold **⌥Space** to dictate

## The recognizer

One recognizer per bound key, in `events::shortcuts::recognizer`, in front of
the agent and dictation monitors. Every input source reaches it through
`fire_key_edge`: the global-shortcut plugin, `platform::modifier_key_monitor`
for the globe key, and `platform::mouse_button_monitor`. A key used by several
rows is registered **once**; the recognizer resolves which of its gestures a
given edge belongs to.

It holds no session state and has no hands-free mode. Per key it remembers only
gesture bookkeeping: when a short release opened the double-tap window, which
second-press branch is still undecided, whether a release has been claimed, and
whether a double-tap-and-hold is down. "Is a tap session running" is read back
out of the voice-session registry, which records how each session was opened.

Resolution:

- **A tap or double-tap session is running:** the next press of that key stops
  it on the down edge, and its release is swallowed. This beats every gesture
  below.
- **First press:** a Hold key goes through the hold path unchanged. A Tap key
  does nothing until its release. A key with only double gestures does nothing.
- **Second press, inside the window:** Double tap alone runs the Tap code path
  on the down edge and swallows the release. Double tap and hold alone starts
  its hold on the down edge. **Both** is the one place that waits: a release
  before `SECOND_PRESS_HOLD_MS` is the double tap, still held at that point is
  the double tap and hold, and the start cue plays when the hold begins, so the
  person is told when to talk. After a Tap that started a session, holding the
  second press cancels that session and starts the double-tap-and-hold instead.
- **The window** opens only from a short Hold release (a cancel) or from the
  Tap release that *started* a session, and only when some gesture on that key
  is waiting for a second press. So stopping after a real sentence can never be
  read as the first half of a double tap.

Every entry point takes `now`, so the state machine is a pure function of its
inputs and its tests drive it with synthetic timings rather than real keys.

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
| Data model, migration, validation, sharing table | `src-tauri/src/triggers/mod.rs` |
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
without a rewrite. Only Hold and Tap project onto those fields; the legacy pair
of modes has no word for a double gesture.

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
- **Stored triggers from before gestures:** `migrate_to_gestures` runs once, on
  a list in which no row has an id. `push_to_talk` reads as Hold, `toggle` as
  Tap and `voice` as Say (serde aliases), every row keeps its key, its target
  and its switch, and every bound Hold row gains a **Double tap** row beside it
  on the same key and target. That hands back the hands-free reach the derived
  double tap gave people, as the row it should always have been: visible,
  rebindable, deletable. The migration is self-marking, because after it every
  row has an id, so it cannot stack companions on every launch.

## Adding a gesture or target later

Add a variant to `Gesture` or `TriggerTarget` in `triggers/mod.rs`, give it a
label, an ending and a row in `can_share_key`, handle it in the recognizer and
in `perform` (the compiler will point at the non-exhaustive matches), and list
it in `TriggersSettings.tsx`.
