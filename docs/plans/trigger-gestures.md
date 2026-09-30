# Triggers as sentences

**Status:** planned 2026-09-29, replacing the first draft of this file (derived double tap, secondary target). Two PRs:
- Backend: gesture model, row IDs, migration, key-sharing rules, gesture recognizer. LAC-4070, Backend Engineer.
- Screen: the sentence editor. Separate ticket, Frontend Engineer, starts once the backend PR merges.

Lacy approves both.

## What the person sees

```
Hold               [🌐]      to  [talk to Juno ▾]     Let go to send
Double-tap & hold  [🌐]      to  [dictate ▾]          Let go to finish
Tap                [Space]   to  [dictate ▾]          Tap again to finish
Say   [hey] [juno]           to  [dictate ▾]
+ Add trigger
```

Every word you can change is a control. The gesture and the target are small menus, the key is the recorder we already have (`ShortcutRecorder`, `KeyCaps`), and the voice phrase keeps its hey toggle and text field. The grey line on the right is generated from the gesture, so the row explains how it ends and the old paragraph hints go away.

A note on Wispr Flow: it lists actions ("Push to talk", "Hands-free", "Command Mode") and gives each one a key, and its double tap is built in (`docs/design/settings-ux-reference.md:7`). This screen is a sentence builder instead. Juno has two targets and five gestures, so an action list would be mostly empty rows. A sentence list only shows what the person uses.

## Gestures

| Gesture | Starts | Ends |
|---|---|---|
| Hold | key down | release |
| Tap | release of a short press | next press of the same key |
| Double tap | second press, see resolution below | next press of the same key |
| Double tap and hold | second press, see resolution below | release |
| Say | wake phrase | end of speech, as today |

This covers what exists today: `push_to_talk` becomes Hold, `toggle` becomes Tap, `voice` becomes Say.

## Which gestures can share a key

**Tap can't share a key with Hold or Double tap.** A short tap on a hold key is usually a fumbled hold, and a quick stop-tap on a tap key looks just like the first half of a double tap. Any other mix is allowed, and each gesture appears at most once per key.

| On one key | Allowed |
|---|---|
| Hold + Double tap | yes |
| Hold + Double tap and hold | yes |
| Double tap + Double tap and hold | yes (Lacy, 2026-09-29) |
| Hold + Double tap + Double tap and hold | yes |
| Tap + Double tap and hold | yes |
| Hold + Tap | no |
| Tap + Double tap | no |

`triggers::validate` and `combo_conflict` enforce this table and nothing looser. The screen greys out a gesture that can't be picked and says why ("🌐 already has *Hold to talk to Juno*"), so a save never fails after the fact.

## Resolving a press (the recognizer)

There's one recognizer per bound key, sitting in `events/shortcuts.rs` in front of `fire_trigger_edge`. Every input source reaches it: global shortcuts, `platform/modifier_key_monitor.rs` for Fn, and `platform/mouse_button_monitor.rs`. It keeps only gesture bookkeeping (last short release, which second-press branch is pending, and whether to swallow the next release). It holds no session state and has no hands-free mode.

Constants live in `constants/agent.rs` `monitor_sessions`: `HOLD_DURATION_MS` 400 (exists), `DOUBLE_TAP_WINDOW_MS` 300, `SECOND_PRESS_HOLD_MS` 250.

**A tap or double-tap session is running:** the next press of that key stops it on the down edge, and its release is swallowed. This beats every gesture below. (Agent: `bar_voice_active()` already does this. Dictation: a press while `is_dictation_active()` is true and the monitor isn't tracking a hold goes to `handle_dictation_tap_mode`.)

**First press:**
- If the key has Hold, it goes through the hold path unchanged (mic at 15 ms). A release at 400 ms or later commits. A shorter release cancels, as it did before #611, and opens the window.
- If the key has Tap, a tap starts on release (the Press code path, called directly). The window opens only if the key also has Double tap and hold.
- If the key has only double gestures, the first press does nothing and a short release opens the window.

**Second press, inside the window:**
- **Double tap only:** fires the Tap code path for its target on the down edge and swallows the release.
- **Double tap and hold only:** starts a hold for its target on the down edge. Its release ends it.
- **Both:** undecided at first. A release before `SECOND_PRESS_HOLD_MS` is a double tap, and the Tap path starts on that release. Still held at `SECOND_PRESS_HOLD_MS` means double tap and hold: the hold starts and the start cue plays at that moment. This is the one place that waits. It only applies when both gestures share a key, and the cue tells the person when to talk. Buffering the audio from the down edge to avoid the wait is a follow-up, and only worth doing if people report clipped words.
- **After a Tap (Tap + Double tap and hold):** the tap session is running. Still held at `SECOND_PRESS_HOLD_MS` means the tap session is cancelled and the double-tap-and-hold hold starts. A release before that is an ordinary stop. The window only opens from the tap that **started** a session, so stopping after a real sentence can never trigger it.

"The Tap code path" means exactly what the Tap trigger calls today: `handle_dictation_tap_mode(app)` for dictation, and `agent_monitor::on_agent_input_released_with_mode(app, AgentTriggerMode::Tap)` for the agent.

## Backend PR (LAC-4070)

1. **Remove single-tap hands-free** from slice 3 (#611, `b00ddb5c`): `hands_free`, `swallow_next_release`, `HoldRelease::HandsFree`, `end_hands_free` and the watchdog exemptions in `dictation_monitor.rs`, and the tap branch's `set_bar_voice_active(true)` in `agent_monitor.rs`. A short hold cancels again (`TRANSCRIPTION_CANCEL` / `agent::CANCEL`, cooldown, generation bump). Keep `HoldRelease` as `Committed` / `Cancelled` / `Nothing`. Keep the 400 ms threshold, key caps, recorder and labels.
2. **Model** (`triggers/mod.rs`): add `id: String` (a UUID, generated when missing), and replace `method` with `gesture: Gesture { Hold, Tap, DoubleTap, DoubleTapHold, Say }`, keeping `#[serde(alias = "method")]` plus a value map so old stores load. `Trigger::key_str` stops being identity; every one of its ~19 users moves to `id`. `derive_legacy` projects Hold and Tap as before and skips the double gestures.
3. **Migration**, run once. Every existing Hold row also gets a Double tap row on the same key and target, so nobody loses the hands-free they had from slice 3. They reach it by double tap instead of a single tap.
4. **Defaults for new installs:** Hold ⌥Space to dictate, Double-tap ⌥Space to dictate, Tap ⌥D to talk to Juno.
5. **Validation** per the sharing table. Say rows must have unique phrases.
6. **Registration:** a key used by several rows is registered once. `dispatch_activation_triggers` hands the edge to that key's recognizer instead of looping over matching rows.
7. **Tests** (CI only, never cargo locally): the sharing table, the migration (old `method` store in, rows with IDs and added Double tap rows out), every recognizer branch above driven by fake timestamps (the `held_for_ms` pattern), and cancel-then-start back to back on one voice controller. That last one is the known risk: the first tap's cancel and the double tap's start land about 300 ms apart.
8. **Docs:** `docs/features/unified-triggers.md` and a one-line pointer in `docs/plans/settings-human-forward.md` slice 3.

## Screen PR (second ticket)

`src/components/settings/sections/TriggersSettings.tsx` is mostly rewritten; `TriggerRow` becomes `TriggerSentence`.
- **Controls:** gesture menu, key recorder, target menu, the generated "how it ends" line, enable switch, and delete on hover. Menu items that can't be picked are greyed out with the reason from `combo_conflict`.
- **Voice rows:** `Say [hey] [phrase] to [target]`, with the existing hey toggle and phrase field inline.
- **"+ Add trigger"** is the one primary action. It adds the next sensible sentence (a gesture and target that fit the rules) with the recorder open.
- **States:**
  - No triggers: "Nothing summons Juno yet" and the add button.
  - A key macOS refuses: the row shows the backend's sentence.
  - Globe key: `GlobeKeyNote` attaches only to rows using 🌐.
  - Voice rows without microphone permission: a one-line fix link.
- **Accessibility:** each sentence has an accessible name built from its words, the menus work from the keyboard, and the recorder keeps its Escape/Enter/Backspace behavior.
- **Evidence:** screenshots of the default list, the empty state, a greyed-out gesture with its reason, and the add flow, plus a recording of hold, double tap and double-tap-and-hold on one key in a `juno-build` install. Save them under `docs/frontend/screenshots/trigger-sentences/`.

## Considered and cut

- Double tap as a built-in, invisible gesture (the first draft of this file). A visible row is honest, and one that comes pre-filled and can be deleted gives the same convenience.
- A `secondary` target field. Double tap and hold is just another row.
- Single tap on a hold key. It's the rule.
- New targets (paste last transcript, cancel, command mode). The target menu makes them cheap to add later, but none ship here.
- A configurable window. Start at 300 / 250 ms and revisit only if people report misses.
- Delaying the first press's cue to hide the cancel-then-start. Every press would pay that latency.

## Demo test

1. Ten seconds: the list reads as sentences, and changing "dictate" to "talk to Juno" is one click.
2. Removed: single-tap hands-free, per-row paragraph hints, method:target identity. Cut: listed above.
3. One primary action: "+ Add trigger".
4. Defaults: three rows cover hold, hands-free and the agent with no setup.
5. States: empty, refused key, globe key, missing microphone permission.
6. Instant: every gesture except the shared-key double-tap-and-hold starts on an edge. That one waits 250 ms, and the cue marks it.
7. Seams: keyboard, Fn and mouse all go through one recognizer, for both targets.
8. Stage test: hold, double tap and double-tap-and-hold on 🌐 in one take.
9. Evidence: CI green, tests, screenshots, recording.
10. DRI: Backend Engineer (LAC-4070), Frontend Engineer (screen).
