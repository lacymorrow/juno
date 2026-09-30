# Trigger gestures: hold, double tap, double tap and hold

**Status:** planned 2026-09-29. Phase 1 is the next PR on `feat/trigger-double-tap`. Phase 2 follows as its own PR.
**DRI:** Backend Engineer (LAC). Lacy approves.

## The rule

No key both holds and single-taps to activate. A short tap on a hold key is almost always a fumbled hold, and a quick stop-tap on a toggle key looks just like the first half of a double tap.

The pairs allowed on one key:

| Primary | May pair with | Never |
|---|---|---|
| Hold | double tap (always on, derived) or double tap and hold (phase 2) | single tap |
| Press (tap to start, tap to stop) | double tap and hold (phase 2) | double tap |

A hold key has one second gesture, never both. When phase 2 gives it a double tap and hold, that replaces its double tap.

## Why this reverses part of slice 3

Slice 3 (PR #611, `b00ddb5c`) made a short tap on a hold key keep the session running hands-free. Hands-free is the Press trigger's behavior reached from a different key, and it was built as a second implementation: `hands_free` and `swallow_next_release` in `dictation_monitor.rs`, and `set_bar_voice_active(true)` on a tap in `agent_monitor.rs`. The research it cites (`docs/design/settings-ux-reference.md:12`) says Wispr Flow reaches hands-free by double-tapping the push-to-talk key. Slice 3 picked single tap, which is the pairing the rule above forbids.

## First ten seconds (phase 1)

Someone holds Fn, talks, lets go: words appear. Same as today. They double-tap Fn: the start cue plays, the bar shows listening, and it keeps listening with their hands off the keyboard. They press Fn once more and the words land. A quick accidental tap on Fn does nothing.

## Phase 1: remove single-tap hands-free, add derived double tap

### Remove (restore pre-#611 behavior on a short hold)

- `dictation_monitor.rs`: drop `hands_free`, `swallow_next_release`, `HoldRelease::HandsFree`, `end_hands_free`, and the watchdog exemptions. A release under `HOLD_DURATION_MS` cancels again (`TRANSCRIPTION_CANCEL`, sets `last_cancellation_time`). Keep the `HoldRelease` enum with `Committed` / `Cancelled` / `Nothing`; it reads better than the old tuple.
- `agent_monitor.rs`: `end_hold` records the cancellation and bumps the generation again; the `else if agent_started` release branch emits `agent::CANCEL` again instead of `set_bar_voice_active(true)`.
- Replace the five slice-3 hands-free tests with tests for the restored cancel and the new recognizer.
- Keep from slice 3: the 400 ms threshold, key caps, the recorder, sentence labels.

### Add: a double tap on a hold key runs the Press code path

A small per-trigger gesture recognizer in front of the monitors, in `events/shortcuts.rs` (every source reaches `fire_trigger_edge`: global shortcuts, `platform/modifier_key_monitor.rs` for Fn, `platform/mouse_button_monitor.rs`). It keeps only gesture bookkeeping: the time of the last short release, and a "swallow the next release" flag. There is no session state and no hands-free mode.

For a Hold trigger:

1. First press and release go through the hold path unchanged. The mic opens at 15 ms, as it does now. A release under 400 ms cancels, and the recognizer notes the time.
2. A second press within `DOUBLE_TAP_WINDOW_MS` (start at 300, a constant in `constants/agent.rs` `monitor_sessions`) of that release is a double tap. On its **down edge** the recognizer calls the same function the Press trigger calls, and swallows the matching release:
   - dictation: `handle_dictation_tap_mode(app)`
   - agent: `agent_monitor::on_agent_input_released_with_mode(app, AgentTriggerMode::Tap)`
   Firing on the down edge keeps it instant and means a slightly long second press still counts.
3. Stopping: the next press of the same key while that session runs has to reach the Press stop, not start a hold on top of it.
   - Agent: already true. `bar_voice_active()` in `fire_trigger_edge` ends the session on release.
   - Dictation: in the dictation branch, a press that arrives while `AppState::is_dictation_active()` is true and the monitor is not tracking a hold goes to `handle_dictation_tap_mode` (which stops), and its release is swallowed. That check is derived from state that already exists. It likely also fixes a live bug (confirm first): pressing the hold key while a Press session runs appears to start a second hold on top of it, because the monitor does not know about Press sessions.

Press triggers get no double tap.

### Risk to verify

The first tap's cancel and the double tap's start land within about 300 ms on the same voice controller. `COOLDOWN_AFTER_CANCEL_MS` (150) only guards `start_hold`, and the Press path doesn't go through `start_hold`, so the cooldown won't block it. What still needs checking is whether `TRANSCRIPTION_CANCEL` finishes before `TRANSCRIPTION_START` arrives. The generation counter in `agent_monitor.rs` exists for exactly this race; dictation needs the same check. Add a test that runs cancel and then start back to back, and do one manual walk on a CI build (`juno-build feat/trigger-double-tap`).

### Copy

- `TriggersSettings.tsx` `METHOD_HINT.push_to_talk`: "Hold the key while you speak, let go to finish. Double-tap it to keep listening until you press it again."
- `docs/features/unified-triggers.md`: replace the slice-3 tap paragraph.
- `docs/plans/settings-human-forward.md` slice 3: one line pointing here.

### Validation (phase 1)

`triggers::validate` and `combo_conflict` stay one-binding-per-trigger in phase 1. A key shared by a Hold row and a Press row would make the double tap and the Press trigger fight over it, so the existing refusal is correct.

## Phase 2: double tap and hold as a secondary target

The original ask: hold to dictate, double tap and hold to talk to Juno.

- Model: a Hold or Press trigger gets an optional `secondary: Option<TriggerTarget>`. Don't add a new `TriggerMethod`: it isn't a row of its own, it's a second gesture on an existing key. When `secondary` is set on a Hold row, it replaces that row's derived double tap.
- Recognizer: a second press inside the window starts a **hold** for the secondary target on its down edge (`on_agent_input_pressed` / `on_dictation_input_pressed`), and its release ends that hold normally. On a Press row, the window starts from the tap that **started** a session, never from the stop tap, so stopping quickly can't trigger it.
- Validation: `secondary` must differ from the row's own target. A secondary target counts as a binding for conflict checks, so no other row can also claim that key for that target.
- UI: one optional line under the row, "Double-tap and hold to talk to Juno", with a target picker. It's off by default.

## Considered and cut

- Double tap as a separate setting or row. It's derived, like Wispr's, so there's nothing to configure.
- A new `DoubleTapHold` method or row. It's a property of a key, not a trigger.
- Delaying the first tap's cue to hide the cancel-then-start. Every press would pay that latency, just to tidy up the double tap.
- Holding the first tap's audio open to hand it to the double tap. That rebuilds hands-free state, which is the thing this plan removes.
- Making the window configurable. Start with 300 ms and only revisit if people report misses.

## Demo test (phase 1 PR)

1. Ten seconds: above.
2. Removed: single-tap hands-free (state, tests, hint text). Cut: listed above.
3. One primary action per row: hold. Double tap is secondary, and the hint mentions it.
4. Defaults: every hold key gets double tap with no setup.
5. States: an accidental tap cancels silently after the start cue. The Fn and mouse sources go through the same recognizer.
6. Instant: the double tap fires on the second down edge, and the cue plays on that edge.
7. Seams: all three input sources, both targets.
8. Stage test: hold, double tap, and press to stop, all on Fn, with no settings visit.
9. Evidence: CI green, recognizer and cancel/start unit tests, and a screen recording of the walk on a `juno-build` install.
10. DRI: Backend Engineer.
