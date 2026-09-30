# Bar: the narrator, a timeline of the turn (spec)

**Status:** Built on `feat/bar-narrator`; every posture captured from the preview route in light and dark; unit and component tests green; CI on the draft PR.
**DRI:** Frontend Engineer. Lacy is DRI for the hardware pass (real window, drag, focus, a real answer with real tools).

## The ask

Lacy, 2026-09-30: every appearance should feel like a different app, with its own way to show state, progress and the answer, and no jumping when the window resizes itself. Bar today is a wide pill with generic labels ("Processing your request...") and icon soup. Redo it from scratch as the narrator: wide, and it uses the width. The turn reads left to right as a timeline.

## Jobs Standard critique of Bar as of main `58f94df2`

```
For: someone who chose Bar because they want to watch Juno work, step by step, in one strip on their screen.
Verdict: REDO
Remove:
- Six lucide icons that stand for state (Mic, Zap, Volume2, MessageCircle, Keyboard, Send).
- Eleven status strings that describe the app to a developer ("Preparing input field...", "Converting speech to text...", "Task completed successfully!").
- Orange, yellow, emerald, cyan, gradient backgrounds per voice mode.
- The five-bar audio meter.
- A 60px "default" pill that grows to 280px on every state change (a width jump each time, before the content has moved).
- A window that is never sized: the bar lays out at whatever size the last look left.
- No answer at all. The old Bar never showed what Juno said.
Fails:
- 1: the first ten seconds are a lightning bolt in a black pill -> a strip that says "Ask Juno" where you will type, and your words running along it the moment you speak.
- 2: eleven labels, six icons, five colours for one dot's worth of information -> one dot, one rail, sentence-case words.
- 4: no error design beyond "Error: <message>" in red text on a red gradient -> the rail turns red at the bead that failed, and the message sits after it.
- 6: the answer arrived in a different surface (or nowhere) -> the answer's first sentence ends the line; the rest slides down from the strip.
- 7: stage test: a black pill with a bolt in it, next to Pill -> nothing to put on a slide.
Ship as: the state table below, complete, in light and dark. No chat history, no roster, no settings shortcuts.
Evidence needed: a still of every posture in both themes, one clip of a full turn, tsc, vitest, CI green.
```

## Research (what was worth taking)

- **Flight status strips and departure boards.** One row per flight, read left to right: origin, gate, status. The row never changes size; only the words and the colour of one field change. The Bar is one such row for the current turn. https://en.wikipedia.org/wiki/Split-flap_display
- **Safari's address-bar progress.** A thin blue line under the text that fills as the page loads and finishes at the right edge. Nobody needs it explained. The Bar's rail is that line, under the timeline. https://developer.apple.com/design/human-interface-guidelines/progress-indicators
- **Broadcast tickers and macOS Live Text dictation.** Newest words stay visible at the right; the start scrolls off the left. The Bar keeps the same right-to-left overflow trick Island uses for the live transcript: no motion, the line just fills. https://developer.apple.com/design/human-interface-guidelines/live-activities
- **Timelines in CI dashboards (GitHub Actions job graph, Vercel deployment steps).** A step is a bead on a line; the running one pulses; a failed one turns red and the line stops there. The Bar borrows the bead and the stop, not the graph.
- **WCAG 2.2.1 Timing Adjustable.** The auto-dismiss must be visible, must pause on hover and focus, and must be stoppable. The draining rail is the timer; hover pauses it; Escape or a click on the line stops it. https://www.w3.org/WAI/WCAG21/Understanding/timing-adjustable.html
- **macOS system colours.** Light and dark surfaces at system grey levels, system blue `#0A84FF` as the one accent, green for the person's own speech, red for failure. No tints, no gradients.

## The person and the moment

Someone at their desk who chose Bar. They want to see what Juno is doing, not just that it is doing something. They ask, they watch the steps go by, they read one line, and they open the rest only when there is more.

## Ten seconds

A thin wide strip sits where they left it, white or near-black to match the system, with a dot at its left and "Ask Juno" dimmed where the words will go. They hold their key: the dot turns blue, their words run along the strip in green as they speak, and a green rail under the words breathes with their voice. They let go: their words settle at the left in ink, a bead appears after them with "Opening weather.com", and a blue rail under the strip fills up to that bead. The bead pulses while the tool runs. A second bead stops the rail: "Juno wants to read the forecast on this page", with Allow and Don't right there on the line. They click Allow. The rail moves on. A third bead, and the answer's first sentence types in: "Yes, bring a coat." Because there is more, a sheet slides down from the strip's bottom edge with the whole answer and a weather card. The rail is full. After twelve seconds it drains from the right; when it is empty the sheet slides back up and the strip keeps the turn on it, dimmed, until the next one.

## Always wide

The Bar never changes width. Decided from the ten seconds, and from the jumping lesson:

- The Bar is a fixture, like a status line. A strip that shrank to a nub at rest would be a Pill with a different coat. What earns the resting width is that the line keeps the last turn on it, dimmed: the narrator remembers.
- The timeline reads from the left edge. `set_bar_frame` keeps the window's horizontal centre stable, so anything left-anchored moves by half of any width change. With no width change there is nothing to move.
- The window only ever changes height, downward, anchored at its top. The strip sits at the top, so it never moves when the sheet opens or closes.

## Postures

The line has four postures. The sheet is not a posture: it is a layer below the strip that `rest`, `compose` and `turn` can have open.

| Posture | Rust states | On the line |
|---|---|---|
| rest | default, dictation_ready, shrinking, always_listening | the dot, and the last turn dimmed; "Ask Juno" when there is none; "Listening for “hey Juno”" when always listening with no turn |
| compose | expanding, input | the question slot is a text field ("Ask Juno", or "Follow up" when a turn is on the line); beads and the answer stay to its right |
| listen | listening, dictating | your words, green, newest at the right; "Listening" or "Dictating" until the first word |
| turn | transcribing, submitting, loading, agent_responding, finishing, success, speaking, error, stopping, and Juno driving the cursor | the timeline: question, beads, the live segment |

Sizes: strip 520 x 36, radius 10; window 552 x 68 (16px of shadow room on every side); sheet 520 wide, 56 to 320 tall in 8px steps, tucked 10px under the strip so the two read as one object.

## Every state

The "line" column is what sits after the dot, left to right. The "rail" is the 2px line along the strip's bottom edge.

| Rust bar state | Posture | Dot | Line | Rail | Leaves when |
|---|---|---|---|---|---|
| default | rest | ink 40%, breathes 4s | last turn, dimmed, or "Ask Juno" | empty (draining if an answer just finished) | click (Rust: Expanding), key, any state |
| dictation_ready | rest | green 70%, still | as default | empty | dictation starts |
| shrinking | rest | ink 25% | as default | empty | Rust: Default after 300ms |
| always_listening | rest | blue 45%, slow breath | last turn dimmed, or "Listening for “hey Juno”" | empty | wake word (Rust: Listening) or off |
| expanding | compose | ink 70%, still | text field, disabled | empty | Rust: Input after 300ms |
| input | compose | ink 70%, still | text field, focused | empty | submit, blur with empty text (Rust: Shrinking), Escape |
| listening | listen | blue, breathes | "Listening", then the live transcript in green | green meter, your voice level | mic closes (Rust: Transcribing) |
| dictating | listen | green, breathes | live transcript, provisional text green italic | green meter | dictation ends |
| transcribing | turn | green, breathes | the final transcript, dimmed | blue sliver | Rust: Submitting or Default |
| submitting | turn | blue, orbits | question, bead, "Sending" | blue, to the bead | agent starts |
| loading | turn | blue, orbits | question, a bead per step; the running step's words; "Thinking" when there is no step yet | blue, to the live bead | first stream chunk (Rust: AgentResponding) |
| agent_responding | turn | blue, orbits | question, beads, answer's first sentence (live) | blue, past the answer bead as the text arrives | stream ends |
| finishing | turn | green flash 600ms | question, beads, answer; "Done" when there is none | full | 300ms (Rust: Default) |
| success | turn | green flash | as finishing | full | 300ms |
| speaking | turn | blue, ripples | the answer line; a spoken-only answer is the line itself, dimmed | full (the drain waits) | TTS ends |
| error | turn | red, one shake | the failed step's bead turns red and carries the message; with no failed step, a red bead with the message | red, stops at the failed bead | 3s (Rust: Default) or Escape; the red step stays on the line until the next turn |
| stopping | turn | ink 50%, orbits slower | "Stopping" | empty | Rust: Default |

Local conditions layered on top:

| Condition | Effect |
|---|---|
| a step waits on Allow / Don't (`approval_state: "pending"`) | the bead is live, its segment reads "Juno wants to <description>" and carries Allow (blue) and Don't (quiet) inline; the rail stops at that bead; the drain does not run; the segment gets at least 62% of the line and the question yields to 28%, so the request and its two buttons always fit |
| more than one step | older steps collapse to bare beads (hover shows their words); the newest keeps its words; a done step keeps its words until something newer arrives, so the line never goes silent mid-turn |
| a step failed but Juno carried on | its bead stays red, collapsed; the line continues |
| the answer is one sentence | the line is the answer; the sheet stays closed; clicking the line opens it by hand |
| the answer is longer than one sentence, or holds a component | the sheet slides down from the strip's bottom edge, sized to content, as the answer streams |
| answer has only spoken text | the line shows the spoken sentence, dimmed; no sheet, no speaker button |
| answer has visible content and spoken text | the sheet's footer has one speaker button that reveals the spoken text under the body |
| answer is a component | the component is the sheet's body; prose before and after renders around it |
| answer taller than 320 | the sheet body scrolls; scrolling counts as engagement and holds the drain |
| Juno asks for the physical cursor (`input-control-request`) | the sheet opens with `InputControlNotices` at its top |
| Juno is driving the cursor (`input-control-state` active) | the live bead reads "using the mouse in <app>", the dot orbits blue, no rail progress past it |
| Rust goes to `input` while a turn is on the line (clicking the sheet focuses the window) | the question slot becomes the text field with "Follow up"; beads and the answer stay to its right; submitting starts a new turn |
| new turn starts while the sheet is open | the line takes the new question at its first event; the sheet closes at the first listening state, or clears and re-opens at the first chunk of the new answer |
| the person speaks again | the sheet closes; the line is theirs |
| window loses focus with the composer empty | Rust shrinks to Default; the sheet stays open |
| Reduce Motion on | the sheet slides in 150ms ease-out, the rail moves without easing, the dot and beads do not animate; the drain still runs (it is the timer, not decoration) |
| answer already in the conversation when the Bar mounts (history) | the line shows it dimmed; the sheet does not open |

## The drain

- Starts when the answer is complete: the message is no longer streaming, Rust is no longer working or speaking, and no approval is pending.
- Lasts 12 seconds and is the rail itself: blue, emptying from the right edge toward the dot.
- Pauses while the pointer is over the Bar, while anything inside it has focus, and while the sheet is being scrolled. Resumes from where it stopped when the pointer leaves.
- Cancelled outright by pressing a button or typing; starts fresh once the pointer leaves.
- When it runs out: the sheet slides up and the line dims. Escape does the same at once. Clicking the answer line toggles the sheet by hand.

## Motion

- The sheet: one spring for its slide (stiffness 380, damping 36), clipped by a container that starts at the strip's bottom edge, so it visibly comes out from under the strip. Its height follows content through a ResizeObserver in 8px steps.
- The rail: width eases 260ms on its way to a bead, 80ms linear as a meter, 60ms linear while draining. Colour crossfades 200ms.
- The dot and beads: CSS keyframes only (breathe, orbit, ripple, shake, flash, and a bead pulse). No springs on text.
- Words: opacity and colour crossfade 200ms; text itself never moves.
- Window protocol: growing, `resizeWindowIfChanged` first, then the sheet slides; shrinking, the sheet slides, then the window shrinks 350ms later. Both through `set_bar_frame`, so position and size land in one frame.

## The jumping audit

Every transition, both directions, judged against the lesson that the window grows before content and shrinks after, so anything anchored to a window edge moves by half the change.

| Transition | Width change | Height change | Anchor | Verdict |
|---|---|---|---|---|
| first paint after launch | the window the last look left -> 552 | -> 68 | strip hidden (opacity 0) until the first resize resolves | no clipped frame, no jump |
| rest -> listen -> turn -> rest | none | none | strip fixed at (16,16) | nothing can move; verified in the component test ("never asks for a new window size while the line changes") |
| sheet opens | none | +56..320, downward, top anchored | strip at the top | the strip does not move; the sheet slides into room that already exists |
| sheet grows as the answer streams | none | +8 per step, downward | strip at the top | same; the sheet's motion.div height animates inside the grown window |
| sheet closes | none | -h after 350ms | strip at the top | the sheet is gone before the window shrinks |
| hover | none | none | none | the drain pauses; nothing resizes |
| composer opens (focus) | none | none | none | the question slot swaps to a text field in place |
| skill suggestions | none | + list height below | strip at the top | grows first, list appears below the strip |
| Bar docked in the bottom half of a display | none | grows downward, clamped by `clampToMonitor` | | KNOWN: like Island, the window is nudged up when the sheet would run off the bottom. Follow-up: pass `growUp` when the strip is low on the display |
| preview route | none | the preview centres the frame, so the strip rises in the bench when the sheet opens | | bench only; the real window is top anchored |

## Removed (and considered, then cut)

- All six state icons and the Send button. The dot and the rail carry the state.
- The eleven developer-facing status strings. Sentence-case words a person would say: "Listening", "Sending", "Thinking", "Done", "Stopping", "Juno wants to ...".
- Per-voice-mode gradient backgrounds and the five-bar meter. One surface per theme; the rail is the meter.
- The 60px resting pill and the 280px active pill. One width, always.
- A compact resting width that grows while a turn is live. Considered; cut, because every width change is a jump risk for a left-reading line and because the rest state has a job (remember the last turn).
- A chat pane. The Bar shows the current turn; history lives in the main window.
- A close button on the sheet. Escape, the draining rail, and a click on the answer line close it; a fourth way would be clutter.
- Labels under beads. There is no room under a 36px strip; the newest step's words sit on the line and older steps collapse to beads with a tooltip.
- Speaker text on the strip. The spoken channel lives behind one button in the sheet, as on Island.
- A "Thinking" note while a done step still has its words on the line. The step's words are the narration.

## Evidence

- `docs/frontend/screenshots/bar/`: one PNG per posture from `/__bar-preview?appearance=app&state=<state>&theme=<light|dark>` and from `demo=steps` / `demo=fail` (rest, listening, dictating, input, loading with a step, approval, answer with the sheet, done and draining, error, always listening) in both themes, plus `turn-demo.mp4`, a clip of one full turn with Allow clicked, made with the bench and `demo=steps`.
- Unit tests: `narratorModel.test.ts` (posture, sizes, dot, the current turn, the answer line, the line for every state, the rail for every state, the drain rule), `AppBar.test.tsx` (first paint sizing, the listening line, no resize while the line changes, beads and collapsing, approvals to the backend, one-line answer stays on the strip, sheet grows the window first and shrinks after the slide, the drain and its pause, history does not open the sheet, speaking again closes it, the failed bead, the spoken channel, spoken-only, open by hand, Escape stops work, click opens the composer, typing and submit, driving).
- `tsc` clean; `vitest` full suite green (see PR).
- Preview additions (additive): `theme=light|dark`, `demo=steps`, `demo=fail`, and a `plugin:window|theme` case in the fake Tauri layer.
- Not verified here: the real window on hardware. The bench runs the same components on the fake Tauri layer, so resize timing, drag, OS focus and the Rust side of every interaction are for the hardware pass (Lacy, DRI). No Rust changed.

## Follow-ups (not in this branch)

- Grow upward when the strip is docked in the bottom half of a display (same follow-up as Island).
- The agent cards (`WeatherCard` and friends) still carry their own gradient tints inside the sheet. Out of scope here; they belong to the JSX renderer.
- Collapsed beads show their words only as a native tooltip. A small hover label in the Bar's own type would be nicer.
- The vite dev server does not pick up file changes inside a worktree under `.claude/` (no HMR); the bench needed a restart per edit. Worth a look in `vite.config.ts` (`server.watch`).
