# Presence: an orb that tells you everything by how it moves (spec)

**Renamed 2026-10-01.** This look was called "Orb" when it shipped; the name
moved to the restored React Bits shader (`docs/plans/orb-appearance.md`) and
this one took PR #630's own word for it, "a presence". The stored value is
still `orb` and the look itself did not change. The file was
`docs/plans/orb-appearance.md`.

**Status:** Built on `feat/orb-presence` (PR #630); every posture captured from the preview route; unit tests green; CI on the draft PR.
**DRI:** Frontend Engineer. Lacy is DRI for the hardware pass (hold the key, speak, drag, real answers, a real GPU).

## The ask

Lacy, 2026-09-30: every appearance gets a from-scratch pass, and each one can feel like an entirely different app. The Orb is the only object, and everything is told through its behaviour: rest, listening, dictating, thinking, speaking, error. Responses are spoken first; the visible text appears as captions beneath the orb one line at a time like subtitles, the full answer available on hover or click as a sheet that unfolds under it. Approvals: the orb holds still and the caption becomes the question with Allow and Don't. Drive the Three.js material with state rather than adding chrome around it. Performance matters: the loop pauses when idle, the canvas is sized once, the chunk stays lazy. The window is no bigger than the orb plus captions plus shadow, and the orb must not jump when captions appear.

## Jobs Standard critique of Orb as of main `58f94df2`

```
For: someone who chose Orb because they want Juno to be a presence on the desk, not a bar.
Verdict: REDO
Remove:
- The 200x200 always-on window with a 10px "Ready" / "Processing..." label under the orb.
- The six-family palette (periwinkle, gold, teal, slate, green, red) in bar-state-mapper.
- The frame loop that runs at 60fps while nothing moves.
- The agentState indirection (null / thinking / listening / talking): four buckets for sixteen states.
- Any "Listening" or "Thinking" caption. The colour and the motion are the words.
Fails:
- 1: the first ten seconds are a pastel blob that never changes -> small, dim, breathing; blue and swelling the instant you speak, your words under it.
- 4: no way to see an answer, an approval or an error -> one caption slot and one sheet.
- 4: the WebGL loop burns at idle -> loop on demand, CSS breath at rest.
- 6: the orb would jump if anything above it changed -> the orb's centre is at the same place in every window.
- 7: a pinwheel of hard sectors on stage -> soft lobes, a rim of light, one wash at the centre.
Ship as: one orb, one caption slot, one sheet; every Rust state a row; no roster, no history, no settings.
Evidence needed: a PNG of every posture, one clip of a full turn, tsc, vitest, CI.
```

## Research (what was worth taking)

- **Apple, Siri on macOS 26 and 27.** The Siri orb is the whole interface: it sits in a corner, its lobes swell with your voice, and the transcript and the answer appear as text beneath it, not inside a window with chrome. Nothing about Siri says "listening" in words. https://support.apple.com/guide/mac-help/use-siri-mchl6b029310/mac
- **Subtitles.** One line, white on a dark pill, cut hard between lines rather than crossfaded, newest words visible. The BBC's subtitle guidelines put the reading rate at roughly 160 to 180 words per minute and one sentence per line; a spoken answer at that pace fits a line at a time. https://www.bbc.co.uk/accessibility/forproducts/guides/subtitles/
- **ElevenLabs Conversational AI orb (the shader this keeps).** Seven polar lobes, two noisy rings, a four-stop colour ramp. What it drives well from state: the two ramp colours, the ring swell (`uInputVolume`), the flow turbulence (`uOutputVolume`), the flow's rotation (`uAnimation`) and the opacity. What it did badly at high contrast: the lobes meet at the centre as hard sectors. https://ui.elevenlabs.io/docs/components/orb
- **react-three-fiber on demand rendering.** `frameloop="demand"` stops the loop; `invalidate()` or switching back to `"always"` resumes it. Switching the prop at runtime reconfigures the root, so the loop can sleep whenever the bar says the orb is at rest. https://r3f.docs.pmnd.rs/advanced/scaling-performance#on-demand-rendering
- **WCAG 2.2.1 Timing Adjustable.** A finished answer lingers ten seconds, pauses while the pointer is over it or anything inside has focus, and Escape ends it at once. https://www.w3.org/WAI/WCAG21/Understanding/timing-adjustable.html

## The person and the moment

Someone at their desk who chose Orb. They press their key, say a thing, let go. They expect one object to react at once, say the answer, show it if it must, and get out of the way.

## Ten seconds

A small grey orb, dim, breathing slowly. They hold the key: it takes system blue and swells with their voice; their words appear beneath it, one line, newest words visible. They let go: it goes green for a beat while the words become text, then deep blue and turning while Juno works; the line under it is what they asked, dimmed; if a tool runs, its description replaces the line, and the orb turns a little faster each time a step completes. The first spoken sentence lands in the line as Juno says it; the orb ripples with the speech. The line advances sentence by sentence. If the answer carries a component, a dark sheet unfolds under the orb with the whole answer in it. The orb blooms green, once, and settles. The line stays ten seconds, then fades; the window shrinks back to the orb. Nothing was clicked.

## Geometry

The canvas is a 120px stage, sized once and never resized; the orb scales its own mesh. The orb's centre is at `(width / 2, 60)` in every posture, so a centre-stable, top-anchored resize never moves it. Whatever is under the orb decides the window:

| Posture | Under the orb | Window |
|---|---|---|
| orb | nothing | 120 x 120 |
| caption | one line (30 tall, up to 320 wide) or the composer | 360 x 170 |
| approval | the question and Allow / Don't (66 tall) | 360 x 206 |
| sheet | the whole answer (56..320 tall, in steps of 8) | 360 x 196..460 |

The window has two widths. Caption and composer share a height, and the sheet replaces the line rather than stacking under it, so nothing lurches when the person hovers or a tool asks.

## Every state

The orb column is what the orb does; the caption column is the one line under it. Sentence case, never a status word.

| Rust bar state | Orb | Caption | Leaves when |
|---|---|---|---|
| default | grey, 50% of the stage, 70% opacity, CSS breath 4s, loop asleep | none (a lingering answer keeps its last line for 10s) | any state change, click (Rust: Expanding) |
| shrinking | same as default | none | Rust: Default after 300ms |
| dictation_ready | green, rest size, breath | none | dictation starts |
| always_listening | blue at 55%, rest size, breath | none | wake word (Rust: Listening) or off |
| expanding | grey, 62%, full opacity, breath | composer, disabled | Rust: Input after 300ms |
| input | grey, 62%, full opacity, breath | composer, focused; "return" appears once there is text | submit, blur with empty text (Rust: Shrinking), Escape |
| listening | system blue, 78% plus up to 20% with the voice, rings swell | your words as they arrive, provisional ones italic; nothing until the first word | mic closes (Rust: Transcribing) |
| dictating | system green, swells the same way | your words | dictation ends |
| transcribing | green, 70%, turning | your words, dimmed | Rust: Submitting or Default |
| submitting | deep blue, 70%, turning, 1.2s pulse | what you asked, dimmed | agent starts |
| loading | deep blue, turning faster with each finished step | what you asked, dimmed; a running tool's description replaces it | first stream chunk (Rust: AgentResponding) |
| agent_responding | deep blue, turning | the latest spoken sentence; else the latest finished visible sentence; else the question | stream ends |
| speaking | blue, 78%, ripples with the speech level | the sentence being spoken | TTS ends |
| finishing | green bloom, once | the answer's last line | 300ms (Rust: Default) |
| success | green bloom, once | the answer's last line, or nothing | 300ms |
| error | red, one flinch (recoils 12% and returns in 480ms), then holds red | the error, in red | 3s (Rust: Default) or Escape |
| stopping | grey at 80%, turning slowly | none | Rust: Default |

Local conditions layered on top:

| Condition | Effect |
|---|---|
| a tool waits on Allow or Don't | the orb holds still (breath paused, no spin, no pulse); the caption is "Juno wants to <description>" with Allow and Don't; the linger clock does not run |
| Juno is driving the cursor (input-control state) | deep blue, steady turn; the caption is "Juno is using the mouse in <app>" |
| Juno asks for the cursor (input-control request) | the sheet opens with the notice in it |
| the answer carries a component | the sheet opens by itself and shows the whole answer; the line is not shown twice |
| the answer is text only | the line shows it sentence by sentence; hover 150ms or click unfolds the sheet; the sheet folds 400ms after the pointer leaves unless clicked open |
| the answer is spoken only | the line is the whole answer; there is no sheet |
| a finished answer | its last line and any open sheet linger 10s, paused while the pointer is over the orb or anything inside has focus; Escape settles at once |
| the person speaks again | everything under the orb clears at once; the orb is free |
| Rust in `input` while an answer lingers | the composer takes the slot; the answer's line returns when the composer closes |
| audio level while listening or dictating | scale adds up to 20% of the stage; `uInputVolume` follows the level so the rings rise |
| audio level while speaking | `uOutputVolume` follows the level so the flow churns; scale adds up to 8% |
| Reduce Motion on | the caption fades without rising; the sheet's height eases in 150ms; the CSS breath is off; the shader still eases colour and scale (it is state, not decoration) |

## The orb

What state drives, every frame, from a ref the bar writes into (no React re-render of the canvas):

- **tint**: the two ramp colours. Rest grey `#6E6E73 / #AEAEB2`; system blue `#0A84FF / #8EC5FF` for listening and speaking; deep blue `#0066D6 / #5FA8FF` for working; system green `#30D158 / #A6EFB8`; system red `#FF453A / #FFB3AE`. Eased at 8% per frame.
- **scale**: the mesh scale, 0.5 at rest, 0.62 awake, 0.7 working, 0.78 for voice, plus the swell. Eased at 16% per frame.
- **swell** (`uInputVolume`): the audio level while listening or dictating; the rings rise and the lobes pull in.
- **turbulence** (`uOutputVolume`): 0.12 at rest, 0.45 working, 0.35 plus 0.65 times the level while speaking.
- **spin**: how fast the flow turns (`uAnimation`): 0 at rest so the loop can sleep, 0.35 for voice, 0.8 plus 0.25 per finished step while working (capped at four steps), 0.6 speaking.
- **pulse**: a 3% swell and back, period 1.2s while working, shorter as steps complete. The cadence of the pulse is the progress.
- **impulse**: one flinch (recoil) for an error, one bloom for done. Plays once per entry into the state.
- **opacity**: 0.7 at rest, 0.55 waiting for the wake word, 0.8 stopping, 1 otherwise.

Shader changes from the ElevenLabs original: lobe softness 0.6 to 0.9; lobes fade to 30% toward the rim so the edge reads as light rather than the end of a slice; one soft wash at the centre so seven sectors never meet at a point. The rings, the ramp and the flow are as they were.

## Performance

- **The loop sleeps.** `frameloop` is `"always"` while anything moves and `"demand"` once the orb's look allows sleep (breathe or still) and its colours, scale, swell, turbulence and opacity have settled (`settled()` in the model). Any `bar-state-update` and any change of look wakes it. The resting breath is a CSS transform animation on the canvas wrapper; it is paused, not removed, in active states, so it never snaps. Verified on the bench: `data-loop="demand"` four seconds after mount with no events.
- **The canvas is sized once.** 120 x 120 CSS px, `dpr` 1 to 2, never remounted: the same `OrbCanvas` element stays mounted through every posture, and nothing keys it. Size is the mesh scale.
- **The chunk stays lazy.** `BarHost` still lazy-imports `elevenlabs-orb-bar`; the model, the caption pieces and the tests import nothing from Three.js. `OrbCanvas.tsx` is the only file that does, and it is imported only by the bar.

## Motion

- Caption in: the window grows first (`resizeWindowIfChanged`, one `set_bar_frame`), then the slot fades and rises 4px in 180ms. Caption out: the slot fades in 120ms, then the window shrinks 350ms later. Text changes inside the slot cut, like subtitles.
- Sheet height follows its content through a ResizeObserver, clamped 56..320 in steps of 8, under one spring (stiffness 380, damping 34).
- The orb never animates its position. Its stage is absolutely placed at the top centre of the window.

## Removed (and considered, then cut)

- The status label under the orb, and `getStatusLabel` for this look.
- `mapToOrbState` and `mapToElevenLabsOrbColors` (dead once the orb reads state itself). `getStatusLabel` stays for Halo and Avatar.
- `src/components/ui/elevenlabs-orb.tsx`, moved to `src/components/bar/orb/OrbCanvas.tsx` and rewritten to be driven from a ref.
- The `agentState` prop and the "auto" volume mode (synthetic wobble when there is no audio). The orb follows real levels or holds.
- Skill autocomplete in the composer. The orb is a presence first; the main window has it.
- Chat history and a follow-up field. The orb shows the current turn; a follow-up is a new press of the key, or a click on the resting orb.
- A "Listening" or "Listening for hey Juno" caption. Considered for the wake-word state; cut because the dim blue breath says it and the words would be chrome.
- A tick ring around the orb for tool calls. Considered; cut because a ring is chrome and the pulse cadence already carries the progress.

## Evidence

- `docs/frontend/screenshots/orb/`: one PNG per posture from `/__bar-preview?appearance=orb&state=<state>` and the demos (`default`, `dictation_ready`, `always_listening`, `input`, `listening`, `dictating`, `transcribing`, `loading`, `agent_responding`, `speaking`, `finishing`, `error`, `stopping`, `approval`, `sheet`), plus `spoken-turn.mp4`, one full spoken turn from `/__bar-preview?appearance=orb&demo=spoken`.
- Unit tests: `orbModel.test.ts` (the look, the targets, the caption, the sentences, the window, the turn; every state Rust can send), `ElevenLabsOrbBar.test.tsx` (rest and sleep, listening with words, green dictation, hide then shrink, working with a tool, the answer sentence by sentence and the linger, the sheet on hover and by itself for a component, history does not linger, speaking again clears, approval, error, Escape, the composer, click, driving).
- `tsc` clean; `vitest` 42 files, 482 tests green after the rebase onto #629 (2026-09-30). The clip and the hero still are also at `docs/changelog/media/630/`.
- Not verified here: the real window on hardware, a real GPU's frame cost, the WebGL sleep under a real compositor, drag, OS focus, the Rust side of every interaction. The bench runs the same components on the fake Tauri layer.

## Follow-ups (not in this branch)

- Grow upward when the orb is docked in the bottom half of a display. The window grows downward today and is clamped to the monitor.
- The preview's `state=speaking` frame carries no `spokenText`, so the held speaking still has no caption; the spoken demo covers it. Adding it to `heldFrame` touches every look's still, so it waits for the six appearance PRs to land.
- The task card in the sheet still carries a purple icon tint (`agent-cards/index.tsx`). Same follow-up as Island.
- `scripts/bench-record.sh` has no `spoken` mode; the clip here was recorded with the same steps from a scratch script. Add the mode once the appearance PRs land.
- The preview's `VoiceContext` logs "Cannot read properties of undefined (reading 'transformCallback')" on some loads, before the harness installs the mock. Not the orb's; noted.
