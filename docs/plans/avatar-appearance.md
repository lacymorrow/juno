# Avatar: a character you talk to (spec)

**Status:** Built on `feat/avatar-appearance`; every posture captured from the preview route; unit and component tests green; CI on the draft PR.
**DRI:** Frontend Engineer. Lacy is DRI for the hardware pass (a real turn, drag, dock low, dark and light wallpaper).
**Template:** `docs/plans/island-appearance.md` (PRs #618 and #620). Same structure, same window protocol.

## The ask

Lacy, 2026-09-30: every appearance gets a from-scratch pass and can feel like an entirely different app. Avatar's identity: a character, so state is expression and gesture, and conversation is speech bubbles. Rest is a calm face with a blink. Listening leans in with a blue cue and your words in a bubble on your side (green for dictation). Working is a thought bubble with a moving mark and the running tool named inside. Speaking moves the mouth with the spoken sentence in Juno's bubble. The answer is Juno's bubble, text and components, sized to content, spoken part first, notes beneath, spoken text collapsible. Approvals are a bubble with Allow and Don't. Error is a wince and a red-edged bubble. Finished is a nod. The avatar is the anchor: the window grows first, then the bubble animates in, and nothing pushes the head.

## Jobs Standard critique of Avatar as of main `58f94df2`

```
For: someone who chose Avatar because they want Juno to be a someone on their desk they talk to, not a bar.
Verdict: REDO
Remove:
- The 200px glass sphere as the whole UI: a paperweight; nothing on it says it heard you.
- The 10px caption under it ("Listening...", "Processing...", "Agent Working...", title case, trailing dots).
- bar-state-mapper's five-way mapping that sends `input` to "listening" and `error` to "idle".
- The remote-only Rive load with no fallback (offline = a blank 200px square).
Fails:
1: ten seconds are a sphere and a caption; an answer never appears -> bubbles.
4: error is "Error" in 10px; no answer, no approval, no tool, no offline state -> a row per state below.
6: the avatar cannot show the turn; you open the main window -> Juno's bubble carries text and components.
7: a glass ball with "Processing..." under it fails the stage test.
Ship as: head + your bubble + Juno's bubble, every Rust state in the table.
Evidence needed: a still per posture, one clip of a turn, tsc, vitest, CI.
```

## The Rive file, and what was decided

`src/components/ai-elements/persona.tsx` streams `obsidian-2.0.riv` from a Vercel blob at runtime (nothing is in the repo). Its strings give the whole contract: state machine `default`, boolean inputs `listening`, `thinking`, `speaking`, `asleep`, and a pointer-driven `hover`; animations `on_loop_1..3` (idle), `hover_in/out`, `listening_in/loop/out`, `thinking_loop_1..3`, `speaking_in/loop/out`, `asleep`, `wake`, `fade_in`; one view-model property, `color`, which the Persona sets to black in light mode and white in dark mode. The sphere takes that colour: dark glass on a light desktop, white glass on a dark one.

It has no eyes, no mouth, no neck, no blink. So:

- **The file drives what it has.** `listening`, `thinking` and `speaking` follow the state table below (the sphere's inner light moves differently in each). `asleep` is unused: no Rust state means it. `hover` is the file's own pointer reaction and is left alone.
- **The face is SVG on top.** Two eyes that blink every 4.4s at rest, look toward your bubble while you speak or type, look up and away while thinking, open wide when a tool needs you, squint on a failure. A mouth that is a line at rest and opens and closes in an uneven rhythm while Juno talks. Drawn in the opposite of the sphere's colour, so it reads in both themes.
- **Gestures are transforms on the head's box** (origin at the neck, 50% 88%): lean (rotate 4deg, toward you 2px, scale 1.04), attend (rotate 2deg), think (rotate -4deg), nod (640ms, two dips), wince (420ms to rotate -3deg scale 0.97, held while the error shows). Reduce Motion turns all of it off.
- **A flat disc sits under the canvas.** The head is there on the first paint, before the network has delivered the sphere, and stays there if it never does. The disc fades out when the sphere is ready. Offline is not a special state; it is the disc.

## Research (what was worth taking)

- **Comic speech conventions.** The tail points at the speaker; a thought has no tail and two circles lead to the thinker; one bubble per beat reads faster than a transcript. Taken whole. Juno's tail points at the head; yours points right, off the panel, at you; the thought's circles rise to the head. Colours stay monochrome with one hairline accent, so it is a comic's grammar without its palette.
- **Animal Crossing, Clippy, Siri's orb, Cozmo.** Characters that people forgave: the reaction comes first and is small (a lean, a blink), the words come second. Clippy failed because it interrupted; nothing here appears unless you spoke, typed, or Juno has something to answer with.
- **Apple HIG, Live Activities (36pt compact, 160pt expanded).** The head is 84pt, the panel 356pt, an answer bubble at most 320pt tall before it scrolls. https://developer.apple.com/design/human-interface-guidelines/live-activities
- **WCAG 2.2.1 Timing Adjustable.** A timed dismissal must be visible and pausable. Island's ring is reused as is: it drains on the answer bubble for 12 seconds, pauses on hover and focus, is cancelled by any press, and the close mark is always there.
- **Island (PRs #618, #620).** The window protocol, the linger, the approval row, the spoken channel behind one control, and `latestTurn` are taken directly. The jumping lesson too: the head is centred and anchored, never edge-pinned.

## The person and the moment

Someone at their desk who chose Avatar. They press their key, say a thing, let go. They want the character to react before the words are even there, to show the answer beside its own head, and to settle down on its own.

## Ten seconds

A small glass head sits where they left it, blinking now and then. They hold the key: it leans toward them, a blue light appears by its ear, and their words fill a bubble on their side as they speak. They let go: the bubble dims and shrinks to a short quote, the head tilts the other way, and a thought bubble rises from it with three dots pulsing and "Working" inside; when a tool runs, its name replaces the word. The first words of the answer arrive: the thought becomes a speech bubble under the head, the mouth moves, the spoken sentence sits at the top and the notes and components below. The head nods once. A ring on the bubble's corner drains for twelve seconds; the person reads, moves the mouse away, the bubble fades and the window shrinks back around the head. Nothing was clicked.

## Layout: the head is the anchor

| Posture | Window | Head | Stack |
|---|---|---|---|
| rest | 116 x 116 | centred, 16 from the top (or bottom) | none |
| open | 356 x (116 + 10 + stack) | same spot | your bubble on the right, Juno's centred under the head |

The window is centre-stable on x and anchored on y at the head's centre (`anchorY = 58`), through `useWindowSize("floating-bar")`. The panel width is one constant, so width only ever changes between rest and open, and the head sits at the centre both times: it never slides. Height follows the measured stack in 8px steps, so a streaming answer does not resize on every glyph; while streaming the window carries one extra step of headroom so the next line lands inside it.

Docked in the bottom half of the display (checked when the avatar opens from rest, the way Pill does), the stack rises above the head instead: `growUp`, the head 16 from the bottom, Juno's tail pointing down, the lean and the eyes toward you upward. At rest both layouts put the head at the same pixel, so the flip itself never moves it.

Bubbles: dark (`#1C1C1E`), hairline, radius 16 (22 for a thought), 13px text, width fit-to-content up to 300. Juno's bubble is centred so its tail is always under the head. Your bubble is right-aligned with the tail on its right edge. Once Juno's bubble is up, your line becomes a dimmed one-line quote at most 140 wide, so the tail's path is clear.

## Every state

| Rust bar state | Rive | Gesture | Cue | Your bubble | Juno's bubble |
|---|---|---|---|---|---|
| default | idle | calm, blinks | none | none (your question stays, dimmed, while an answer is up) | the answer, if one is up |
| dictation_ready | idle | calm | green, still, 60% | none | none |
| shrinking | idle | calm | none | none | none |
| expanding | idle | attend | none | composer, disabled | none |
| input | idle | attend (eyes on the composer) | none | composer, focused; "return" appears once there is text | the answer, if one is up |
| listening | listening | lean, blue | blue, breathes | "Listening" until the first partial, then the live transcript (provisional in italics), blue hairline | none |
| dictating | listening | lean | green, breathes | live transcript, green hairline and tone | none |
| always_listening | listening | calm | blue, slow, 45% | "Listening for “hey Juno”" | none |
| transcribing | thinking | think | green, breathes | the final transcript, dimmed | none |
| submitting | thinking | think (eyes up) | none | your question, dimmed | thought: "Thinking" |
| loading | thinking | think | none | your question, dimmed | thought: the running tool, else "Working" |
| agent_responding | thinking until content, then speaking | think, then talk | none | your question, dimmed | thought until the first chunk, then the answer |
| speaking | speaking | talk (mouth moves) | none | your question, dimmed | the answer if up, else the spoken sentence in a speech bubble |
| finishing | idle | nod | none | your question, dimmed | the answer, if up |
| success | idle | nod | none | your question, dimmed | the answer, if up |
| error | idle | wince (held) | none | your question, dimmed | red-edged bubble with the message, "Something went wrong" if none |
| stopping | thinking | think | none | your question, dimmed | thought: "Stopping" |

Local conditions layered on top:

| Condition | Effect |
|---|---|
| a tool waits on Allow or Don't | Juno holds up the question ("Can I open the page in Safari?") with Allow (blue) and Don't; eyes go wide; the linger does not run; a failure still wins over the question |
| Juno is driving the cursor (input-control state) | thought bubble "Juno is using the mouse in Safari"; Rive thinking; whatever Rust says |
| a cursor-control request arrives with no bubble up | a plain Juno bubble holds the notice; it hides itself once answered |
| answer up, Rust in `input` | the composer opens in your bubble above the answer; the answer stays; the linger pauses |
| answer up, Rust goes to listening or dictating | the answer leaves; you are speaking again |
| reply has spoken text and visible text | spoken sentence first, a hairline, the notes beneath; the speaker control folds the spoken part away |
| reply has only spoken text | the sentence is the bubble |
| reply is a component | the component renders inside the bubble at its own width, up to 300 |
| answer taller than 320 | the bubble body scrolls; scrolling pauses the linger |
| new turn starts while an answer is up | the bubble takes the new answer at its first chunk; the linger resets |
| the sphere has not arrived, or never does | the disc is the head; the face and every gesture work the same |
| Reduce Motion on | no blink, nod, wince, talk or cue motion; gestures still set their end pose; bubbles appear without the scale |
| history already there on mount | nothing opens; the head rests |

## The linger

Island's `useLinger` and `LingerRing`, unchanged: twelve seconds once the answer is complete, Rust is idle, no approval is pending and the composer is closed. Pauses while the pointer is over the avatar or anything inside has focus. Cancelled by a press or a scroll until the pointer leaves, then starts fresh. Runs out: the answer leaves, the window shrinks around the head. Escape does the same at once (or stops the work if Juno is working).

## Motion

- Head gestures: `transform` 260ms `cubic-bezier(.2,.8,.2,1)`; nod 640ms; wince 420ms and held.
- Eyes: look 220ms; blink 4.4s loop with an 80ms close; mouth 760ms uneven loop while talking.
- Bubbles: laid out hidden (opacity 0, scale 0.96, 4px toward the head) while the window grows, then 180ms opacity and 220ms transform once it has. Leaving: removed from the scene at once; the window shrinks 350ms later.
- Window protocol, same as Island and Pill: growing, resize then show; shrinking, hide then resize. Both through one `set_bar_frame` so position and size land in the same frame.

## Removed (and considered, then cut)

- The 200px sphere, the caption, and `mapToPersonaState`'s use in the bar (the mapper file is untouched; the function is now unused by the bar).
- A chat pane, history, roster, wake-phrase and settings buttons. The avatar shows the current turn; history lives in the main window.
- A separate follow-up field. Clicking the head at rest opens the composer in your bubble, above the answer if one is up.
- Both bubbles at full width at once. Considered a two-column layout so Juno's tail and your full question could coexist; cut, because a 356px panel cannot hold two 300px bubbles side by side and a tail that points at nothing is worse than a shorter quote.
- Eyes that follow the mouse pointer. Considered; cut as a distraction that says nothing about state.
- A speech bubble for `success` with no answer ("Done"). The nod is enough.
- Vendoring the `.riv` into the repo. The disc covers the offline case; the asset's licence is not ours to copy without checking. Follow-up.

## Evidence

- `docs/frontend/screenshots/avatar/`: one PNG per posture from `/__bar-preview?appearance=persona&state=<state>&pin=frame` (default, listening, dictating, input, thinking, error, answer, answer-above for the docked-low layout), plus `turn.mp4`, a clip of one full turn made with `scripts/bench-record.sh persona card` and `BENCH_QUERY="&pin=frame"`.
- Bench additions, all additive: `pin=frame` (the preview places the window by the x/y the bar asked for, unscaled, so an anchored look is seen holding still as on hardware) and `dock=low` (the fake window starts in the bottom half) on `/__bar-preview`; `BENCH_QUERY` and `BENCH_VIEWPORT` on `bench-record.sh`.
- Unit tests: `avatarModel.test.ts` (the head, your bubble and Juno's bubble for every bar state; every local condition; the window and its anchor; the linger rule), `PersonaBar.test.tsx` (rest, lean and cue, dictation tint, window protocol both ways, the anchor across postures, docked-low growth, thought with the running tool, the answer with spoken first, the speaker fold, nod and linger, hover pause and Escape, history on mount, speaking again, approval, wince, driving, Escape while working, the composer, settling to rest under focus). Rive is mocked as a div that reports the input it would set.
- `tsc` clean; `vitest` green (numbers in the PR).
- Not verified here: the real window on hardware. Resize timing, drag, OS focus, the Rust side of every interaction, the sphere's look on a real wallpaper in both themes, and the docked-low flip on a real display are for the hardware pass (Lacy, DRI).

## Follow-ups (not in this branch)

- Vite ignores `**/.claude/**` for watching, so a worktree bench never gets HMR; every edit needs a server restart. Worth a `VITE_WATCH_WORKTREE=1` escape in `vite.config.ts`.
- `latestTurn` and `answerKey` live in `island/islandModel.ts` and are imported here; they belong in a shared `bar/turn.ts`.
- Vendor the `.riv` (8KB) once its licence is confirmed, so the sphere does not depend on a third-party blob at launch.
- The Rive `asleep` input is unused. A "quiet hours" or "muted" state would be the place for it.
- Audio-driven mouth: `audioLevel` in the bar payload is the microphone, not Juno's voice. If Rust ever emits a playback level, the mouth should follow it instead of a rhythm.
