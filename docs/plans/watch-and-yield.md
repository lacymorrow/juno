# Watch and yield: Juno notices what the person does

**Status:** Plan only. Nothing built.
**DRI:** The Juno lead session owns the build. Every question is decided below; Lacy can overturn any of them.
**Builds on:** `docs/plans/ambient-awareness.md` (PR #659, open), which designs the wake-up path for chess, and PR #670 (merged), which made the existing screen and file monitors honest, bounded and stoppable. Read #659 first. This plan does not repeat it; it adds the half #659 left as a follow-up and changes one thing about how its watch decides to wake.

## The one-line problem

Juno cannot tell when the person does something, so it cannot take its turn when the person finishes theirs, and it cannot get out of the way when the person takes over.

## The ask

Lacy, 2026-10-07:

> "I'd really like Juno to be able to play chess. It's still not following up and monitoring what the user does and reacting based on it. [...] It could react when the user does stuff so that it doesn't get mixed up. Like if it's trying to control a program and the program keeps getting closed or changed. It can see that the user is using it and relinquish control or stop doing what it's doing. [...] It needs to feel natural, like Jarvis. [Juno does] its own thing, [knows] when to watch, [knows] when not to watch."

That is two behaviours, and they are the same missing sense:

1. **Yield.** While Juno is working, the person touches the mouse or keyboard, closes the app, or brings something else to the front. Juno stops acting on a world that changed under it and hands control back. Today it keeps clicking where the app used to be, and if the person closes the app, Juno reopens it and the two of them fight.
2. **Wake.** The person makes a chess move and lets go. Juno notices, takes its turn, and goes back to waiting. Today nothing happens until someone says "your move".

Both rest on one signal: **what the person just did, told apart from what Juno just did.**

And one rule governs both: **the person never manages it.** There is no watch mode, no setting, no "start watching" or "stop watching", no "take control" button. Juno decides when to watch, what to watch, when to stop, and when to step back, the way it already decides when to take a screenshot. That is what makes it feel like Jarvis rather than a tool with a watch feature.

## The ten seconds

**Yield.** Juno is filling a form in TextEdit. The person grabs the mouse and clicks somewhere. Juno's glow fades from the cursor inside a quarter of a second, no further click lands, and Juno says, once and quietly: "All yours." The bar shows the task as paused, not failed. "Keep going" picks up from the same conversation, after a fresh look at the screen.

**Wake.** The person says "let's play chess". Juno opens a board, plays white, and says "your move". Nobody told it to watch; the bar just reads Watching. The person drags a pawn, lets go, and about a second later Juno moves. Nobody speaks between moves. While the person's hand is on the mouse Juno never moves a piece, even if it has decided on one.

## What already exists

| Piece | Where | State |
|---|---|---|
| Tagging of Juno's own key events | `SYNTHESIZED_EVENT_MARKER` in `mcp-server-os-level/src/lib.rs:78`, set in `text_insertion.rs:65`, read by `src/platform/synthetic_events.rs` | **Partial.** Only text insertion tags its events. Clicks, moves, scrolls and key presses posted in `mcp-server-os-level/src/platforms/macos/engine.rs` (`:588`, `:680`, `:1620`) are untagged, so to any monitor they look exactly like the person. |
| Passive global NSEvent monitors that ignore tagged events | `src/platform/stop_key_monitor.rs`, `src/platform/modifier_key_monitor.rs:498` | Built. The shape to copy, including installing the global monitor only once Accessibility is granted. |
| Per-session cancel | `AgentSession::cancel` and `cancel_receiver` (`src/agents/session.rs:203`, `:208`), `cancel_focused` (`:376`) | Built. Yield ends a run through this, never through the global `signal_cancel` (`src/state.rs:1076`). |
| Background input and its tier | `src/input_control/mod.rs:64` `background_mode_enabled`, `with_input_tier` (`src/agent/tools/anthropic_computer_use.rs:1019`), target pid and window id in results since #721 | Built. Tells yield which conflicts matter (see the rules below). |
| A refusal shape that stops the rest of a batch | `create_anthropic_error_response`, used by `agent::app_observation` to decline clicking where an app used to be | Built. Yield returns the same shape. |
| App presence after a launch | `src/agent/app_observation.rs` | Built, but it is an `osascript` call at about 0.8 s, too slow to check before every click. |
| Cursor glow and ghost cursor | `src/cursor_overlay.rs` (#722) | Built. The visible half of handing control back. |
| Wake path, budget, Escape participation for monitors | `set_screen_monitor` / `set_file_monitor` in `src/agent/tools/timer_tools.rs`, fixed by #670 | Built and honest. `set_screen_monitor` now diffs a real region against a real threshold. |
| DOM-digest page watch for chess | `docs/plans/ambient-awareness.md` | Designed, not built. |

## The design

### 0. Knows when to watch, knows when not to

**When Juno watches.** At the end of any turn whose next step belongs to someone else, the model ends by watching for it instead of finishing or asking to be told. One sentence in the system prompt (`src/agent/prompts/templates.rs`) carries the rule: "When your task continues only after something you don't control happens, such as the person's move, a download finishing, or a reply arriving in a window you are using, watch for it and stop. Never ask the person to tell you when." The watch tools are #659's `watch_page` and #670's `set_screen_monitor`. Chess is an example, not a special case: no chess code anywhere.

**What it watches.** Only the window the task is about. Never the whole screen, never another app, never anything no task needs. A Juno with nothing to do observes nothing: the sense below is installed only while a run or a watch holds it.

**When it stops, without being told.**

| What happened | Juno |
|---|---|
| The woken turn sees the task is done: checkmate, the file arrived | Finishes normally, says so once |
| The person closes the watched window or quits its app | Ends the watch silently. Closing it was the answer. |
| No input in the watched window for 30 minutes while the person works elsewhere or is away | Lets the watch lapse silently |
| #659's ceiling: two hours or its wake count | Lapses silently. A backstop nobody should reach. |
| Escape, or "stop" | Stops, as today |

Talking to Juno about something else does not end a watch. The watch belongs to its own session and ends when its own reason does.

**What it never does.** Ask "should I keep watching?", announce that it is watching, or watch something no task asked about. The only sign is the bar's quiet `watching` state.

### 1. One sense: `platform/user_activity.rs`

A single passive global NSEvent monitor, in the `stop_key_monitor` shape, for left and right mouse down, key down, scroll wheel, and mouse moved. It drops every event carrying Juno's marker and publishes the rest on a `tokio::sync::broadcast` channel:

```rust
pub struct UserActivity {
    pub kind: ActivityKind,      // Click, Key, Scroll, Pointer
    pub at: Instant,
    pub frontmost_pid: i32,      // whose window the person was acting in
}
```

Next to it, an `NSWorkspace` observer for `didActivateApplication` and `didTerminateApplication`, publishing `AppFocusChanged { pid }` and `AppQuit { pid }` on the same channel. Both are push, free while nothing happens, and need no permission Juno does not already hold.

**It is installed only while something listens:** a run that holds an input lease, or an armed watch. A ref-counted ledger, the way the stop key already works. A Juno sitting idle observes nothing.

**Prerequisite, and the reason this is not a one-day change: every event Juno posts must carry the marker.** Set `kCGEventSourceUserData` to `SYNTHESIZED_EVENT_MARKER` at each `post` in `engine.rs`, and on the events sent through `SLEventPostToPid` in `interaction.rs`. Pin it with a test that greps the posting sites the way `anthropic_computer_use.rs:4360` pins `with_input_tier`, because one untagged path means Juno yields to itself on every click. This is the dead-control pattern from memory: the safeguard has to be linked to the thing it names, by a test, or it decays.

Pointer motion is the only noisy kind. A move counts as the person only when the pointer is more than 12 points from where Juno last put it, or Juno has not moved it at all this run. Tremor and trackpad drift stay under that; a deliberate grab does not.

### 2. Yield, during a run

A run that will send input takes an **`InputLease`** from the sense when it starts and drops it when it ends. The lease subscribes to the channel and flips to revoked when a rule below fires. The check sits in one place: the input dispatch in `anthropic_computer_use.rs` (around `:977`, where background mode is already read), before the cooldown and before the arbiter. A revoked lease means the action is not sent, the rest of the batch gets the refusal shape, and the run ends through its session's own cancel with reason `Yielded`.

**Which conflicts count depends on how Juno is driving:**

| Juno is driving | The person does this | Juno |
|---|---|---|
| The real cursor (foreground) | Clicks, types, scrolls, or moves the pointer away | Yields. There is one pointer and the person wins it. |
| Background, a target window | Clicks or types while the target app is frontmost | Yields. They are in the same window. |
| Background, a target window | Works in any other app | Keeps going. That is what background mode is for. |
| Either | Quits the target app, or closes the target window | Yields, and never reopens it in this run. |
| Either | Escape | Stops, as today. Escape is a stop, not a yield. |

**Yield is not a failure, and it reads differently from a stop.** The run ends with "All yours.", spoken once and quietly, and the bar returns to its resting state without an error. The person caused the yield and needs no explanation; a silent pause would look like a hang. The conversation is kept (`AdvancedMemoryManager` is shared across turns), so "keep going" continues the same task. The first thing the resumed turn does is take a fresh screenshot, which the system prompt tells it after a yield, because everything it knew about the screen is stale.

**No automatic resume for an ordinary task.** Juno cannot know the person is finished: they may be halfway through something of their own. Resuming is one spoken phrase away and that is the right cost. A watch (below) is the one place Juno resumes on its own, because there the person's turn ending is the whole point.

### 3. Do not act on a stale screen

The second half of "so it doesn't get mixed up". A click is planned against a screenshot, and the world can change between the screenshot and the click without the person touching anything Juno is using: a dialog appears, an app quits, a window moves.

When Juno takes a screenshot, record a small **world stamp**: frontmost pid, the target window id, and that window's frame. Before each coordinate action, compare the stamp to now using `CGWindowListCopyWindowInfo` for the one window, which costs about a millisecond, not the 0.8 s `osascript` path. If they differ, do not click. Return a result the model can act on:

> "The screen changed since your last screenshot: Safari is now in front, and the TextEdit window you were using has closed. Take a new screenshot before acting."

**Never fight the person twice.** If the target app quits or its window closes a second time in one run after Juno brought it back, that is the person saying no. The second time is a yield, not a re-plan.

AX-path actions (the ones that press a control through accessibility rather than at coordinates) skip the stamp check. They address an element, and if the element is gone the AX call fails honestly on its own.

### 4. Wake, for chess

#659's design stands: a `watch_page` tool that polls a JavaScript digest on a page Juno drives, re-arms after each wake, is bounded by wake count and deadline, holds a session row, and stops on Escape. This plan changes one rule and adds one fallback.

**The change: the person's hand gates the wake.** A watch subscribes to the same sense. While the person is clicking, dragging or typing in the watched window, the watch does not wake, even if the digest changed: the move may be half made, and a woken turn that acts now is the fight that yield exists to prevent. The watch wakes when the digest has changed **and** the person has been still for 800 ms. If the opponent is online rather than at this Mac, there is no hand on the mouse and the digest change alone wakes, as #659 specified. The rule is one line in the poll loop and it removes the worst failure the chess case has.

**The fallback: a board Juno cannot read through the DOM.** #659 notes that the person's own Chrome is probably unreachable over CDP unless remote debugging is on, so a game they already have open may be invisible to `watch_page`. For that case the watch uses the region monitor #670 made honest: `set_screen_monitor` on the board's rectangle, with a threshold of one square's worth of pixels, and with the same hand gate. A tick is a pixel diff in Rust, never a model call. The model is called only on a wake, which is once per move. Check during the build whether Chrome exposes the board through accessibility once `AXManualAccessibility` is set on it; if it does, that is a cheaper, exact digest and it replaces the pixel diff.

**A woken turn also takes an input lease.** If the person grabs the mouse while Juno is mid-move, Juno yields, and the watch stays armed and waits for the person to go still again. Yield ends a turn; it does not end a watch. Escape ends both, in the two-press order #659 specifies.

## What the person sees

- **The cursor glow** (#722) is the lease made visible. It appears when Juno takes the pointer and fades the moment Juno yields. No new UI.
- **One spoken line per yield**, "All yours.", and only one. If the person grabs the mouse three times in a minute they hear it once; after that the bar alone shows it.
- **The bar** shows `watching` while a watch is armed and nothing is running. That state is #659's open question 1 and it is asked again below.
- **No dialog, no notification, no toast** for a yield. The person caused it and already knows.

## Slices

Each one ships alone and is useful alone.

**Slice 1, yield.** Tag every posted event, the sense (input monitor plus workspace observer), the input lease, the dispatch check, the world stamp, the never-fight-twice rule, and the yielded ending with its spoken line. It touches no chess code and fixes the fighting today, for every task. Files: `mcp-server-os-level/src/platforms/macos/engine.rs`, `interaction.rs`, new `src/platform/user_activity.rs`, `src/platform/mod.rs`, `src/agent/tools/anthropic_computer_use.rs`, `src/anthropic.rs` (the run ending), `src/agent/prompts/templates.rs` (the fresh-screenshot sentence after a yield), `src/lib.rs` (shutdown).

**Slice 2, wake.** #659's slice 1, the page watch, with the hand gate added to its poll loop and the region fallback, plus the bar's `watching` state. Demo: a game against Juno, four moves by hand, no spoken prompt between them.

## Evidence before each slice is called done

- Slice 1: a recording of Juno filling a TextEdit form, the person grabbing the mouse mid-form, the glow fading, no further keystroke landing, the line spoken, then "keep going" finishing the form. A second recording of the person quitting the target app twice and Juno stopping rather than reopening it. Committed under `docs/changelog/media/<PR>/`.
- Slice 1 tests: the tag is set at every posting site (source-pinned); the motion threshold; the yield table above as a pure function from (driving mode, activity, target) to decision; the world-stamp comparison; never-fight-twice; a yield cancels only its own session.
- Slice 2: the chess recording, and tests for the hand gate (digest change during activity does not wake; the same change after 800 ms still does).
- Hardware check that only Lacy can run: Juno must never yield to itself. Run a long foreground task with the person's hands off the machine and confirm zero yields in the log.

## Removed, and decided against

- **Polling `CGEventSourceSecondsSinceLastEventType` for idle time.** It cannot tell Juno's events from the person's, so it measures nothing useful while Juno is driving. The tagged monitor can.
- **A CGEventTap that intercepts the person's input.** A passive monitor sees enough. A tap that can swallow events is a much bigger trust surface for no gain here.
- **Automatic resume after the person goes idle, for ordinary tasks.** Explained above. Kept only for watches.
- **A "Take control" button.** The mouse is the button. The person should never have to find a control to get their own computer back.
- **A watch setting, a watch mode, a "watch this" or "stop watching" command, and any prompt asking whether to keep watching.** Juno decides; see section 0. The person's only controls are the ones they already have: their hands, their voice, Escape.
- **A yield line that explains how to resume.** "Keep going" works because it is what a person says anyway; telling them so every time is noise.
- **Asking the model whether to yield.** A model call costs seconds, and the person's hand is already on the mouse. The decision is a table in Rust.
- **Watching by screenshot plus a model call per tick.** As in #659: a tick never costs a model call.

## Decisions

Recorded so nobody re-asks them. Lacy can overturn any.

1. **Background mode: typing anywhere does not pause Juno; only input in the window Juno is using does.** Pausing on any keystroke would make background mode useless, which defeats #547 and #721.
2. **A yield is spoken, once and quietly: "All yours."** The person's eyes are on their own work, and a silent pause looks like a hang.
3. **The bar gets a `watching` state** (#659 question 1). A bar that looks idle while Juno waits for your move is wrong.
4. **Juno arms and ends watches on its own** (section 0). No setting, no command.
5. **Slice 1 ships first.** It fixes a defect every task has today; chess is a feature on top of it.
