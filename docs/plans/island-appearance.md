# Island: one surface with four faces (spec)

**Status:** Built on `feat/island-one-surface`; tests green; evidence from the preview route. Draft PR, waiting on the hardware pass.
**DRI:** Frontend Engineer. Lacy is DRI for the hardware pass (tap, hold, dictate, drag, real answers) and the QA gate.
**Issue:** LAC-4240 (dictation into the follow-up field landed nowhere).

## The ask

Lacy, 2026-10-15: "make the island basically just like the pill except it's got a much better animation. combine the floating bar [and] floating chat into the same component so it might flip between showing the idle bar or the text input or transcript ... or a text response or a custom component that it generates."

The first Island (2026-09-29, `feat/island-appearance`) had six postures (capsule, hover, ear, line, status, card) and resized its window for each. Two things were wrong with it. The window protocol (grow the window, then spring; spring, then shrink the window) chopped every morph, because AppKit and WebKit land a resize on different frames. And the card owned its own follow-up field, which the island unmounted the moment any voice state began, so a dictation aimed at that field had nowhere to land (LAC-4240).

## The person and the moment

Someone at their desk who chose Island. They press their key or click the capsule, say or type a thing, and read the answer in the same small object, then type the next line right under it. It goes away on its own when they stop looking.

## Ten seconds

A dark capsule sits at the top of the screen with one breathing dot. They hold their key: the capsule opens into a text box that says "Listening…", level bars where the mic button was, and their words appear in the box as they speak. They let go: "Transcribing…", "Sending…", "Working…". The answer streams in above the box, their question as one dim line on top. If the answer is a component (a task list, the weather), the component fills the island edge to edge and the prose folds into one caption line. A thin ring drains around the close control. They type a follow-up into the same box, or move away and the island settles back to the capsule. Nothing jumps: one shape, one spring, the box never moves relative to the edge it is anchored to.

## The four faces

| Face | Shape | What is in it |
|---|---|---|
| idle | 92 x 28 capsule, radius 14. Under the pointer 132 x 32 with Talk, Type and (once there is a turn) Show last answer | the dot |
| compose | 360 wide; 36 tall, growing with the text to four lines (90). The small bar (248 x 36) while nobody is writing, see below | the composer only |
| answer | 360 wide; composer plus content, up to 400 tall, then the body scrolls | question line (dot, your words dimmed, speaker toggle when there is both text and speech, linger ring), the answer body, the composer as the follow-up field |
| component | 360 wide, same height rule | caption line (the prose folded to one line, click to expand), the component edge to edge, the composer |

The composer is not part of any face. It is one textarea mounted for the life of the island, anchored to the far edge of the shape (bottom when the island grows down from a top well, top when it grows up from a low well, from `barDock`'s `growUp`). In idle it is transparent and takes no pointer, but it is still the same node, so it keeps its draft and its caret through every state.

### The small bar

Lacy, 2026-10-15: "compose listening, compose working, and compose error all could be a small bar unless the user has turned on see dictated text as you speak."

The compose face is 248 wide, one line, whenever nobody is writing in it: no draft, no caret, Rust not in its input state, and (while a mic is open) "See dictated text as you speak" (`live_partial_transcription`) off. Listening, transcribing, sending, working, an error, a reply being spoken and Juno driving all show as the small bar with their placeholder ("Listening…", "Working…", the error text) and their controls. A Send with nothing to send is not drawn in it. With the setting on, a recording is the full 360 box so the words can be read as they arrive; work and errors stay small either way. A click in the box, or typing, makes it the full box: only the width moves, under the same spring. The setting is read at mount and again as each recording starts or ends (`get_live_partial_transcription`). With it off, the box says "Listening…" and never shows words mid-recording.

### The activity flame

The Island carries the Pill's canvas-ui flame border (`BarFlameBorder`) with the same language, from one shared module (`bar/barFlame.ts`: `flameForState`, `useShowGlowBorder`): green for your dictation, blue for listening, red for an error, the focused session's colour breathing while Juno works, dim blue breathing for the wake phrase. It follows the glow-border setting. The shape is `isolate` and no longer clips, so the flame sits at z-index -1 above the black face, behind the content, and licks outward; the content clips in an inner layer that inherits the radius.

## Every Rust bar state

"Answer open" means an answer or a tool waiting on Allow arrived in this turn and has not been put away. Placeholder text shows only when the box is empty; a typed draft always wins.

| Rust bar state | Face, nothing open | Face, answer open | Placeholder | Controls in the box |
|---|---|---|---|---|
| default | idle (compose if there is a draft, the caret, or a step back from a component) | answer / component | Ask Juno, or Follow up beside an answer | Talk, Send (disabled when empty), Close without sending in compose |
| dictation_ready | idle | answer / component | as default | as default |
| shrinking | idle | answer / component | as default | as default |
| always_listening (wake phrase armed: a resting state) | idle | answer / component | as default | as default |
| expanding | compose | answer / component | Ask Juno / Follow up | Talk, Send, Close without sending |
| input | compose, caret in the box | answer / component, caret in the follow-up field | Ask Juno / Follow up | Talk, Send, Close without sending |
| listening (spoken turn to Juno) | compose | compose: the answer turns back into the box, the draft kept | your words as they arrive, else Listening… | level bars, Send (sends what was said), Cancel without sending |
| listening or dictating with the dictation flag | compose | stays answer / component, so the field the dictation types into stays | as listening | as listening |
| dictating | compose | answer / component | as listening | as listening |
| transcribing, provisional (mic still open) | compose | answer / component (dictation) or compose (spoken) | your words, provisional | as listening |
| transcribing, final decode | compose | answer / component | the final words, else Transcribing… | Send (typed line), Stop Juno |
| submitting | compose | compose until the new answer arrives (the open one belongs to the last question) | Sending… | Send disabled, Stop Juno |
| loading | compose | as submitting | the running tool's description plus …, else Working… | Send disabled, Stop Juno |
| agent_responding | compose | answer / component, streaming | Working… | Send disabled, Stop Juno |
| finishing | compose | answer / component | Done, or Follow up beside an answer | Send disabled, Stop Juno (Rust still counts it as work) |
| success | compose | answer / component | Done / Follow up | Talk, Send, Close without sending (compose) |
| speaking | compose | answer / component | the sentence being spoken (no answer), Follow up (answer) | Talk, Send, Close without sending (compose) |
| error | compose | answer / component | the error text, in red | Talk, Send, Close without sending |
| stopping | compose | answer / component | Stopping… | Stop Juno |

Local conditions on top:

| Condition | Effect |
|---|---|
| Juno is driving the pointer | placeholder "Juno is using the mouse in <app>", the stop control is Stop Juno |
| a tool waits on Allow | the answer opens with one row: "Juno wants to <description>", Don't and Allow. The ring does not run |
| answer is spoken only | the body is the spoken sentence; no speaker toggle |
| answer has text and speech | the speaker toggle in the question line reveals the spoken text under the body |
| answer taller than 400 | the body scrolls inside the shape |
| a new answer while the island is up | the answer face opens; an answer already there at mount (history) does not |
| hover over idle | the hover controls; Show last answer brings a settled answer back |
| tray Show/Hide Chat | toggles the last answer |
| Reduce Motion | size changes and fades are a 150ms ease-out; no blur, no travel |

## Escape and the close controls

Escape is still the shared one (`useEscapeToIdle`, `barEscape.ts`, the ladder from #797). The island only says what one rung down is:

- component: X or Escape steps back to compose, draft and caret kept.
- answer: X or Escape settles to idle.
- compose at rest: Escape (or Close without sending) puts the box away and clears the draft. Rust is told the composer blurred so it leaves its input state; until it does the island stays idle rather than flashing the box back.
- compose while Juno works: Escape stops the work (Rust's call). The draft is kept.

## The ring

- Runs on the answer and component faces only, once the answer has finished streaming, Rust is not working and no tool waits on Allow.
- Never runs while the composer has text or the caret.
- Twelve seconds. Pauses while the pointer is over the island or anything in it has focus. A press on a control inside cancels it until the pointer leaves, then it starts over.
- On expiry the island settles to idle.

## Motion

- **The steady frame.** The Island now lives on the Pill's steady frame (`lib/steadyFrame.ts`, `useIslandFrame`). The window is sized once for the largest face (408 x 648: the 360 shape at its 400 cap, 24 of shadow room each side, 200 for the skill suggestions) and placed once per well. Every face change is animation inside it. The page reports the drawn shape through `set_bar_hit_regions`, so the transparent rest of the window passes clicks through, and through `setSteadyFootprint`, so a drag carries a window the size of the shape.
- **One shape.** One `motion.div` animates width, height and corner radius on a critically damped spring (stiffness 440, damping 42, mass 1: damping is exactly 2 * sqrt(stiffness * mass), so there is no overshoot) that settles in about 300ms. Height follows content through a ResizeObserver on the body.
- **Pinned edges.** The shape is pinned to the well's docked edges (`contentPlacement`): a top well grows down, a low well grows up, a left or right well grows away from its side. Wells are computed for the resting capsule, so idle sits exactly where it always did.
- **Content.** The face's content is one absolutely positioned layer that crossfades: in, opacity 0 to 1, blur 4px to 0, y 4px to 0, 180ms starting 60ms into the spring; out, opacity to 0 and blur to 4px over 120ms. The composer is outside that layer and only fades in and out of idle.
- **Reduce Motion:** 150ms ease-out for size and fades, no blur, no travel.
- Flat macOS look: black shape, hairline, system blue only on Allow and an armed Send, sentence case.

## Removed

- The ear, line, status and card postures, and the hover posture as its own posture (it is idle under the pointer now).
- The separate follow-up field in the card. There is one composer.
- The status-words line. The placeholder carries status.
- Per-posture window resizes: the grow-then-spring and spring-then-shrink protocol, `windowSize`, `SHRINK_DELAY_MS`, and the 8px card height steps.
- "The card closes on any voice state". A spoken turn turns the answer back into the composer with the draft kept; a dictation leaves the answer where it is.
- The "return" hint in the line, and the "Type instead" voice control (the box is already the text box).
- The dot in the composer. The placeholder and the level bars say what the dot used to.

Kept on purpose: no chat history in the island (current turn only), no roster, no settings or wake-phrase buttons.

## The Notch

The Notch was the Island's component with `frame="notch"`. It is now `notch/NotchBar.tsx` with the old posture model in `notch/notchPosture.ts`, unchanged in behaviour. It should get the same one-surface pass; that is a follow-up, not this branch.

## Evidence

Committed with the PR under `docs/changelog/media/<PR>/`: one PNG per face from `/__bar-preview?appearance=dynamic` (`state=default`, `state=listening`, `state=loading`, `state=error`, `demo=answer`, `demo=card`) and an MP4 of one full turn from `scripts/bench-record.sh dynamic demo=turn` (rest, speak, an answer with a component, two Escapes down the ladder to rest).

Tests: `islandModel.test.ts` (face for every state, shapes, placeholder, stop control, ring rule, the turn reducer), `IslandBar.test.tsx` (one composer node through every state, answer and component faces, Escape ladder, ring, approvals, hover, and Rust's real dictation key-up sequence replayed into the follow-up field), `notchPosture.test.ts`, `mixed-content-renderer.test.tsx` (`splitHeroContent`).

Not verified here: the real window on hardware. The bench runs the same components on the fake Tauri layer, so click-through, drag, focus, a real dictation paste and the Rust side of each interaction are for the hardware pass.
