# Halo: a ring that measures the turn (spec)

**Status:** Built on `worktree-agent-aea847210136db5af` (draft PR, rebased onto v0.8.28); every posture captured from the preview route; unit tests green; CI on the draft PR.
**DRI:** Frontend Engineer. Lacy is DRI for the hardware pass (tap, hold, drag, real answers, a real tool run).

## The ask

Lacy, 2026-09-30: every appearance gets a from-scratch pass after Island. Halo (`react_orb`) is a soft ring with almost no state handling and no way to show a response. Redo it so it feels like its own app: the ring is an instrument.

## Jobs Standard critique of Halo as of main `58f94df2`

```
For: someone who chose Halo because they want one quiet instrument on the desk, not a chat window.
Verdict: REDO
Remove:
- The ogl WebGL orb (react-orb.tsx), its hue rotation and the hover-intensity quantiser.
- The 10px status caption at the bottom ("Ready", "Agent Working...", "Dictation Ready").
- The 200x200 square window: a transparent square that big steals clicks around a small object.
- Two vocabularies for state (hue AND label). The ring alone tells it.
- "Beside the ring" for tool descriptions: text beside a centred ring moves the ring when the window widens. Words go below; the ring is the anchor.
Fails:
- 1: the first ten seconds are a purple-blue blob that does nothing -> a thin white ring that fills blue the instant you speak.
- 4: no answer, no approval, no error text -> inside the ring when it fits whole, a sheet below when it does not; "Allow?" in the ring with Allow and Don't below; a red gap where it failed.
- 4: seven ring motions would be icon soup in motion form -> four verbs only: fill, travel, pulse, close. Ticks and the gap are marks on the same ring.
- 7: a placement that flips from inside to below mid-stream would jump -> streaming text is below or not yet; "inside" is decided once the answer is complete.
Ship as: the state table below, complete. No chat history, no roster, no wake-phrase button, no settings shortcuts.
Evidence needed: a still per posture from the preview route, one clip of a full turn, tsc, vitest, CI green.
```

## Research (what was worth taking)

- **Apple, Activity rings and the Watch gauges (HIG: Gauges; watchOS Activity).** A ring is read as a gauge before anything else: a partial stroke means "this much of the whole". Fill from 12 o'clock clockwise, one colour per meaning, and never animate the fill faster than the thing it measures. Halo's fill follows the mic level with a 120ms linear transition so it reads as a meter, not a spinner.
  https://developer.apple.com/design/human-interface-guidelines/gauges
- **Apple, indeterminate progress.** A travelling arc says "working, unknown length". Pausing it says "waiting on something specific". Halo pauses the arc on the tick the running tool will earn, so the pause has a place.
  https://developer.apple.com/design/human-interface-guidelines/progress-indicators
- **Clock faces and hour marks.** Twelve fixed positions are enough for a turn: a fixed tick per finished tool call accumulates around the ring like hours, so a long task literally fills the dial. Past twelve, the ring is full of marks and that is the message.
- **WCAG 2.2.1 Timing Adjustable.** A timer that dismisses content must be visible, must pause on hover and focus, and must be stoppable. Halo's timer is the ring itself draining; hover pauses it; Escape or a click ends it.
- **"Jarvis" HUD rings.** Surveyed a dozen. Rotating decoration, concentric scan rings, cyan on black. None of it survives the stage test and all of it is banned by Juno's flat, system-blue rule. One idea survives and is Halo's spine: the ring shows the state of the work, not a picture of thinking.

## The person and the moment

Someone at their desk who chose Halo. They press their key, say a thing, let go. They expect one small instrument to react at once, tell them how far along Juno is, show the answer, and go quiet without being told to.

## Ten seconds

A thin ring, 96px, on a black disc, sits where they left it. They hold the key: the ring fills blue with their voice, their words appear on a sheet under it. They let go: the ring reads full in green for a moment while the sentence is written down, then a white arc starts travelling around it. Juno checks the calendar: the arc pauses at 12 o'clock with "Checking your calendar" under the ring; when it finishes, a tick is left at 12 and the arc travels on. A second tool earns a tick at 1 o'clock. The answer is "42 min": it appears inside the ring, the ring closes green once, and then drains slowly as a clock. They read it. The ring runs out and rests thin again. Nothing was clicked.

## Geometry

| Part | Size |
|---|---|
| Disc (black backing) | 96 diameter |
| Ring | radius 44, stroke 2 at rest, 3 when it measures |
| Narrow window (ring alone) | 136 x 136 (disc plus 20 of margin for the shadow and the speaking pulses) |
| Sheet | 320 wide, radius 18, top at the disc's centre, content starts 10 below the disc's edge |
| Wide window (ring plus sheet) | 360 x (20 + 48 + sheet height + 20); sheet content 0..300, then it scrolls |

The ring's centre is at (window width / 2, 68) in both windows. The window grows around the ring horizontally (centre-stable resize) and downward from it. The ring never moves when the sheet opens or folds. That is the whole answer to the jumping lesson for this look.

## Every state

The ring has four verbs (fill, travel, pulse, close) plus rest, drain (the clock) and gap (the break). The caption is the one line under the ring. Ticks are counted from the current turn's finished tool calls; the running tool holds the arc at the tick it will earn.

| Rust bar state | Ring | Caption (sheet) | Leaves when |
|---|---|---|---|
| default | rest, white 30%; drain (white 40%, level = clock) while a finished answer is on stage | none | any state change, click (Rust: Expanding), hover brightens the ring |
| dictation_ready | rest, green 55% | none | dictation starts |
| shrinking | rest, white 20% | none | Rust: Default after 300ms |
| expanding | rest, white 55% | composer, disabled | Rust: Input after 300ms |
| input | rest, white 55% | composer, focused; "return" appears once there is text | submit, blur with empty text (Rust: Shrinking), Escape |
| listening | fill, blue, level = mic | "Listening" until the first partial, then the live transcript | mic closes (Rust: Transcribing) |
| dictating | fill, green, level = mic | live transcript, provisional text dimmed | dictation ends |
| always_listening | rest, blue 45%, breathes over 6s | "Listening for “hey Juno”" | wake word (Rust: Listening) or off |
| transcribing | fill, green 60%, full | the final transcript, dimmed | Rust: Submitting or Default |
| submitting | travel, white | the words you said, dimmed | agent starts |
| loading | travel, white; held at the next tick while a tool runs or waits on Allow | the running tool's description, else your words dimmed; nothing while an approval is up | first stream chunk (Rust: AgentResponding) |
| agent_responding | travel, white, same holds | the running tool, else your words until the answer shows | stream ends |
| finishing | close, green, full, once (held 600ms whatever comes next) | none: the close is the word | 300ms (Rust: Default), then the clock |
| success | close, green | none | 300ms |
| speaking | pulse: three soft rings expanding out of the ring | the sentence being spoken, dimmed, only when no answer is on stage | TTS ends |
| error | gap: the ring white 60% with a red break at the mark the next tool would have earned, the break breathing | the error text, red | 3s (Rust: Default) or Escape |
| stopping | travel at half speed, white 50% | "Stopping" | Rust: Default |

Local conditions layered on top:

| Condition | Effect |
|---|---|
| a tool call finishes (success or failure) | a tick at the next hour mark, with a 200ms scale-in; twelve at most |
| a tool is running | the arc glides forward to that tool's tick and holds; the caption is the tool's description |
| pending tool approval | "Allow?" inside the ring; the sheet shows "Juno wants to <description>" with Allow and Don't; the arc holds at the tick; the clock does not run |
| Juno is driving the cursor (input-control state) | travel in blue; the caption is the driving label ("using the mouse in Safari") |
| Juno asks for the cursor (input-control request) | the sheet opens for the notice (InputControlNotices); it closes when nothing is left under the ring |
| answer complete and short (no component, no line breaks, at most 18 plain characters) | inside the ring: 26px for up to four characters, 18px up to eight, 13px otherwise, two lines at most |
| answer streaming and short | not shown yet: the arc travels, the caption keeps your words. Inside is decided once, at the end, so it can never jump to the sheet mid-stream |
| answer long, or a list, or a component | the sheet unfolds below with `MixedContentRenderer`; the ring keeps travelling until Rust finishes |
| response has only spoken text and no visible content | the spoken sentence is the answer: inside when short, on the sheet in italics when long |
| response has visible content and spoken text, on the sheet | a small speaker button under the content reveals the spoken text; never shown twice |
| answer complete, Rust idle, no approval | the clock runs: the ring drains white over 12s; when it runs out the answer leaves and the sheet folds |
| pointer over the ring or focus inside it | the clock holds; pressing a button or typing cancels it until the pointer leaves, then a fresh clock starts |
| Escape | while Juno works: stop (Rust). With an answer on stage: dismiss. With the composer open: close it (Rust) |
| Rust goes to listening or dictating while an answer is up | the answer leaves; the ring is the person's meter again |
| new turn starts | ticks reset with the turn; the previous answer is gone at the first frame of the new one |
| an answer already in the conversation when the ring mounts (history) | not shown |
| Reduce Motion on | no travel rotation (the arc sits at its tick or at 12), no pulses, no breath, no tick scale-in; the sheet eases in 150ms; the clock still drains (it is the timer) |

## Motion

- Fill: `stroke-dashoffset` from 12 o'clock clockwise, 120ms linear so it tracks the mic without lag or jitter. Colour 200ms.
- Travel: one `motion` value for the rotation, 1.4s per lap, linear, forever. A hold animates the rotation forward (never backward) to the tick over 400ms ease-out, so the arc glides to its place instead of snapping there. Stopping runs at 2.8s per lap.
- Close: the gauge offset goes to 0 over 400ms ease-out in green. Held 600ms locally because Rust's Finishing lasts only 300ms.
- Drain: the same gauge, white 40%, offset following the clock at 60ms linear.
- Pulse: three copies of the ring scale 1 to 1.42 and fade over 1.8s, 600ms apart.
- Gap: a dash pattern with an 8% hole centred on the failed mark, plus a red piece 70% of the hole, breathing over 1.6s.
- Ticks: a 200ms scale-in from their middle. The rotation lives on a wrapping `<g>` so the mark's own scale cannot override it. (Found on the bench: `transform-box: fill-box` on an element that also carries a `rotate(a cx cy)` attribute moves the rotation centre to the SVG corner. The red piece was drawn at 7 o'clock until that was removed.)
- Sheet: height on one spring (stiffness 380, damping 34), overflow hidden, so it slides out from under the disc and folds back under it. The disc is drawn after the sheet so it always sits on top.
- Window protocol, unchanged from Pill and Island: growing, resize the window then spring the sheet; shrinking, spring then resize 350ms later. Both through `set_bar_frame` so position and size land in one frame.

## Removed (and considered, then cut)

- `react-orb.tsx` (the ogl WebGL shader) and everything that fed it: `mapToReactOrbConfig` is no longer imported by Halo. The `ogl` dependency is now unused and can leave `package.json` in a follow-up (lockfile change, kept out of this branch). **Superseded 2026-10-01 (#649): `ogl` is in use again by the Orb appearance. Do not remove it.**
- The status label and its "..." vocabulary.
- The square window. The window is now the disc plus margin, or the disc plus sheet.
- Tool descriptions beside the ring. Below, so the ring stays put.
- A distinct "speaking" colour. The pulse is the verb; the ring stays white.
- A chat pane, history, the roster, wake-phrase and settings buttons.
- A close button. The ring is the timer and Escape is the close; a control would be a second way to do one thing.
- Showing a short answer inside while it streams. Cut for the jump it would cause; the cost is a short wait on the shortest answers.

## Decision: SVG, not WebGL

The whole look is one SVG: two circles for the disc, one for the track, one for the gauge, a `<g>` that rotates for the arc, lines for the ticks, two dashed circles for the gap, three circles for the pulses. No canvas, no shader, no context re-initialisation on every prop change (the old orb rebuilt its WebGL context whenever hue or intensity changed, which is why the level was quantised). `BarHost` still lazy-imports `react-orb-bar.tsx`, since that file is off limits to this pass, so the chunk stays but it is a few kilobytes of SVG instead of a shader and `ogl`.

## Evidence

- `docs/frontend/screenshots/halo/`: one PNG per posture from `/__bar-preview?appearance=react_orb&state=<state>` (default, always-listening, dictation-ready, listening, dictating, transcribing, input, submitting, speaking, finishing, stopping) and from the Halo demos (`demo=ring`: working with a held arc and a tick, then the answer inside with the clock; `demo=card`: a long answer on the sheet; `demo=allow`: the approval; `demo=break`: two ticks and the red gap), plus `ring-demo.mp4`, an eight-second clip of one full turn made with `scripts/bench-record.sh react_orb demo=ring 8` (the script gained a `demo=<name>` mode for it).
- Unit tests: `haloModel.test.ts` (ring verb for every bar state, ticks, gap, caption for every state, placement inside/below/none and the no-flip rule, type size, window geometry, the clock rule, the turn reducer) and `HaloBar.test.tsx` (fill with level, dictation colour, grow-first and shrink-after protocol, ticks and the held arc, inside only once complete, sheet for a long answer then close, drain and settle, hover holds the clock and Escape dismisses, history not shown, voice clears the answer, approval in the ring with Allow reaching the backend, the red gap after a tool, pulse keeps the answer, spoken-only reply, spoken channel behind one button, Escape stops work, composer types and submits, idle click asks Rust, driving label).
- `tsc` clean; `vitest` 46 files, 581 tests green (2026-09-30, after the rebase onto main at #630).
- Not verified here: the real window on hardware. The bench runs the same components on the fake Tauri layer, so resize timing, drag, OS focus, the Rust side of every interaction, a real tool run (ticks from real `tool_call_result` events) and TTS are for the hardware pass (Lacy, DRI).

## Follow-ups (not in this branch)

- Drop `ogl` from `package.json` once nothing else needs it (a lockfile change).
- `BarHost` could import Halo eagerly now that it is SVG; the host file was off limits to this pass.
- Grow upward when the ring is docked in the bottom half of a display. The sheet opens downward and the window is clamped to the monitor, so nothing is lost, but a low ring will be nudged up when the sheet opens.
- The demos in `AppearancePreview.tsx` re-send their current frame every 400ms because a lazy-loaded bar subscribes late; the same is true of `demo=card` for the orbs and avatar, which still fire once.
- A component that is still streaming renders as empty space until its closing tag arrives. Same in Island and the pane; belongs to the JSX renderer.
