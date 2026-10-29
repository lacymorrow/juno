# Island: one surface with five faces (spec)

**Status:** Built on `feat/island-one-surface`; tests green; evidence from the preview route. Draft PR, waiting on the hardware pass.
**DRI:** Frontend Engineer. Lacy is DRI for the hardware pass (tap, hold, dictate, drag, real answers) and the QA gate.
**Issue:** LAC-4240 (dictation into the follow-up field landed nowhere).

## The ask

Lacy, 2026-10-15: "make the island basically just like the pill except it's got a much better animation. combine the floating bar [and] floating chat into the same component so it might flip between showing the idle bar or the text input or transcript ... or a text response or a custom component that it generates."

The first Island (2026-09-29, `feat/island-appearance`) had six postures (capsule, hover, ear, line, status, card) and resized its window for each. Two things were wrong with it. The window protocol (grow the window, then spring; spring, then shrink the window) chopped every morph, because AppKit and WebKit land a resize on different frames. And the card owned its own follow-up field, which the island unmounted the moment any voice state began, so a dictation aimed at that field had nowhere to land (LAC-4240).

Lacy, 2026-10-15 (second pass): reconcile the Island with the Pill on pasted images, the wake phrase toggle, the connection dot, the agent roster and tooltips, and give it the Pill's full conversation as a fifth face of the island itself, not a separate pane.

## The person and the moment

Someone at their desk who chose Island. They press their key or click the capsule, say or type a thing, and read the answer in the same small object, then type the next line right under it. It goes away on its own when they stop looking.

## Ten seconds

A dark capsule sits at the top of the screen with one breathing dot. They hold their key: the capsule opens into a text box that says "Listening…", level bars where the mic button was, and their words appear in the box as they speak. They let go: "Transcribing…", "Sending…", "Working…". The answer streams in above the box, their question as one dim line on top. If the answer is a component (a task list, the weather), the component fills the island edge to edge and the prose folds into one caption line. A thin ring drains around the close control. They type a follow-up into the same box, or move away and the island settles back to the capsule. Nothing jumps: one shape, one spring, the box never moves relative to the edge it is anchored to.

## The five faces

| Face | Shape | What is in it |
|---|---|---|
| idle | 92 x 28 capsule, radius 14. Under the pointer 104 x 32 with Talk, Open chat and (when a wake phrase is set) the wake phrase toggle; each control past two widens it by 26 (118 for three). A click on the capsule itself opens the composer, so typing has no button of its own | the dot |
| compose | 360 wide; 36 tall, growing with the text to four lines (90). The small bar (248 x 36) while nobody is writing, see below | the composer only |
| answer | 360 wide; composer plus content, up to 400 tall, then the body scrolls | question line (dot, your words dimmed, speaker toggle when there is both text and speech, linger ring), the answer body, the composer as the follow-up field |
| component | 360 wide, same height rule | caption line (the prose folded to one line, click to expand), the component edge to edge, the composer |
| chat | 60% of the display both ways, clamped to 480..960 wide and 400..820 tall and to the room the well leaves; radius 22 | header (the dot, one collapse control), the whole conversation with history (`ChatContainerV2`, components through `MixedContentRenderer`), the composer |

The composer is not part of any face. It is one textarea mounted for the life of the island, anchored to the far edge of the shape (bottom when the island grows down from a top well, top when it grows up from a low well, from `barDock`'s `growUp`). In idle it is transparent and takes no pointer, but it is still the same node, so it keeps its draft and its caret through every state.

### The chat face

The Pill opens `BarChatPane` under the pill. The Island does not get a pane: the same black shape grows into the conversation. It is sized for the display it is on (`chatSizeFor`: 60% of the display both ways, clamped, never more than the window can be given there) and pinned to the well's docked edges like every other face, so it grows away from the docked edge: down from a top well, up from a low one, out from a side. A well in the middle of a side has only the room between it and the window's edge, so there the chat is shorter (`roomForContent`).

- **Way in:** the expand control in the answer or component header ("Show the conversation"), the answer reached from the hover capsule's Open chat (one size up: idle, then the small chat, then the full chat), or Show Chat in the bar's right-click menu (`bar-toggle-pane`); before any turn both open the composer instead.
- **Way out:** Escape steps down one rung (`islandEscapeStep`): to the answer or component it grew from, or to the composer (caret in it) when there is no answer. The header's one control does the same thing. Show Chat again settles to the capsule.
- **Through a turn:** chat stays up through every Rust state. A new spoken or typed turn streams into the conversation; it never turns the chat back into the composer the way it does an answer.
- **No ring.** Chat stays until it is put down.
- **The composer** is the same node as in every other face, anchored at the far edge, with the hairline the answer face has.
- Motion is the same critically damped spring for the shape and the same crossfade for the content.

What was removed from the Pill's pane on the way in (the `insanely-great` pass, verdict CUT, 2026-10-15):

- The pane's header row of New chat, Open in window and Settings. Chat is a rung on the ladder, not a window; the big chat is the next rung up (Rust's `next`), and Settings has its own way in.
- The "esc to close" / "working" status words. The dot and the placeholder already say what is happening.
- A second close control. The one collapse control steps down a rung, the same as Escape.
- The question line, the speaker toggle and the linger ring of the answer face: the conversation shows the question, and the full message renders its own spoken text and actions.
- An empty chat. The hover capsule's Open chat is always offered, as the Pill's is, but it climbs one size at a time (Lacy, 2026-10-19: "Expand button on the small idle bar should open the small chat, not the full chat. Expand button on the small chat should open the full chat"). Before any turn it opens the composer, where a chat starts.

### Parity with the Pill

| Control | Where | Behaviour | Source |
|---|---|---|---|
| Pasted pictures | the composer | A pasted image is kept as a data URL and sent with the text through `dispatch_query` (the Pill's path). A thumbnail row (32 px, one `x` each, "Remove image") sits above the text. A picture alone is a draft: the box is the full 360 width, not the small bar, and Send is armed | `hooks/usePastedImages.ts`, shared with the Pill |
| Wake phrase toggle | the hover capsule, last | Shown only when a phrase is configured. Ear while listening ("Stop listening for the wake phrase"), crossed ear while paused ("Listen for the wake phrase"). It pauses the engine, never the trigger; it follows Rust's `voice-trigger-listening` | `hooks/useWakePhrase.ts`, shared with the Pill |
| Connection dot | every face with a dot | System red and still when Rust says Juno cannot answer (`connectivity-changed` / `get_connectivity`, `pillStatus.dotTone`); an open mic still wins. Its label is Rust's reason ("Network unavailable") | `dotFor(state, driving, connectivity)`, `dotLabel` |
| Agent roster | beside the shape | From two agents: a second small black capsule (the capsule's height, 8 px away) beside the island, level with the docked edge, on the side away from a side-docked edge. Apple's split island: a second activity is a second small shape, so the island itself never moves for it. Its rect is reported as a second hit region. Hidden while the island is carried |
| Hover expand | the hover capsule, after Talk; the only way back to an answer from idle | Open chat (`Maximize2`), the Pill's hover control: opens the small chat (the last answer), or the composer before any turn. Always offered. The answer's own expand opens the full chat | `IslandBar.tsx` | `AgentRosterStrip`, `rosterWidthFor` |
| Chat header | the full chat face, before collapse | New chat (`Plus`: stops an answer still arriving, `new_conversation`, an empty chat with the caret in the box) and Settings (`Settings`: `open_settings_window`), the Pill pane's two controls (Lacy, 2026-10-19) | `IslandBar.tsx` | |
| Tooltips | the dot and every icon control (Talk, Open chat, New chat, Settings, wake phrase, expand, collapse, close, spoken aloud, the composer's mic, Send and stop, Remove image) | `PillTooltip`: Talk names its live shortcut as the Pill's does ("Hold ⌃⌥", `bar/useTalkShortcut.ts`, shared with the Pill); 500 ms, opens from the native forwarded hover (`mouse-moved-window`) as well as DOM hover, closed during a drag (`useBarDragging`), toward the side the island grows. `title` stays as the fallback and steps aside while the tooltip shows. Text buttons (Allow, Don't, the caption) carry no tooltip | `bar/PillTooltip.tsx` |

### The small bar

Lacy, 2026-10-15: "compose listening, compose working, and compose error all could be a small bar unless the user has turned on see dictated text as you speak."

The compose face is 248 wide, one line, whenever nobody is writing in it: no draft, no caret, Rust not in its input state, and (while a mic is open) "See dictated text as you speak" (`live_partial_transcription`) off. Listening, transcribing, sending, working, an error, a reply being spoken and Juno driving all show as the small bar with their placeholder ("Listening…", "Working…", the error text) and their controls. A Send with nothing to send is not drawn in it. With the setting on, a recording is the full 360 box so the words can be read as they arrive; work and errors stay small either way. A click in the box, or typing, makes it the full box: only the width moves, under the same spring. The setting is read at mount and again as each recording starts or ends (`get_live_partial_transcription`). With it off, the box says "Listening…" and never shows words mid-recording.

### The activity flame

The Island carries the Pill's canvas-ui flame border (`BarFlameBorder`) with the same language, from one shared module (`bar/barFlame.ts`: `flameForState`, `useShowGlowBorder`): green for your dictation, blue for listening, red for an error, the focused session's colour breathing while Juno works, dim blue breathing for the wake phrase. It follows the glow-border setting. The shape is `isolate` and no longer clips, so the flame sits at z-index -1 above the black face, behind the content, and licks outward; the content clips in an inner layer that inherits the radius.

## Every Rust bar state

"Answer open" means an answer or a tool waiting on Allow arrived in this turn and has not been put away. Placeholder text shows only when the box is empty; a typed draft always wins.

| Rust bar state | Face, nothing open | Face, answer open | Placeholder | Controls in the box |
|---|---|---|---|---|
| default | idle (compose if there is a draft, the caret, or a step back from a component; a draft put away with Escape stays idle) | answer / component | Ask Juno, or Follow up beside an answer | Talk, Send (disabled when empty), Close without sending in compose |
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
| hover over idle | the hover controls; Open chat opens the small chat (the last answer) or, before any turn, the composer; the small chat's expand opens the full chat |
| right-click Show/Hide Chat | opens the chat face (the composer before any turn); again, settles to the capsule |
| New Chat (tray, right-click, Cmd+N, the + in the chat header) | stops an answer still arriving, rotates Rust's conversation, clears the screen, caret in the box; the chat face stays the chat face. Skipped while the full-size window is up (it owns the conversation) |
| a spoken turn reaches its final decode, or anything is sent | the open answer is put away; the new one opens when it arrives (it used to come back between the mic closing and the new question landing) |
| a press outside the island (the shared ladder, `outsidePressStep` in `src/lib/barEscape.ts`) | one rung down per click, never stopping work: with a draft in the box the first click leaves it open, then the capsule, then hidden. Without one, the capsule, then hidden. Juno at work is never hidden, and clicks Juno makes while driving another app do not count. Focus loss alone only folds an empty composer |
| chat face open | stays through every bar state; Escape or its collapse control steps down to the answer, or to the composer with no answer |
| a picture pasted into the composer | thumbnail row above the text; the compose face is the full box; Send sends text and pictures through `dispatch_query` |
| wake phrase configured | the hover capsule gains the wake phrase toggle |
| Rust says Juno cannot answer | the dot is system red and still, its tooltip and label Rust's reason; an open mic is still blue |
| two or more agents running | the roster bubble appears beside the shape; the shape does not move |
| Reduce Motion | size changes and fades are a 150ms ease-out; no blur, no travel |

## Escape and the close controls

Escape is still the shared one (`useEscapeToIdle`, `barEscape.ts`, the ladder from #797). Every press is one rung, and repeated presses always reach the capsule. The island only says what one rung down is:

- Rust busy (a dictation, a spoken turn, a run, a reply being spoken, an error, `escapeIsRusts`): the press is Rust's and the island's faces do not move on it. Rust ends the work and rests (`bar_escape_to_idle`, or the stop-key monitor on its own), so the next press finds the island at rest. Before 2026-10-17 the chat face stepped down on the same press that stopped the work: two rungs in one.
- chat: Escape or its collapse control steps back to the answer or component, or to compose (caret in it) when there is no answer.
- component: X or Escape steps back to compose, draft and caret kept.
- answer: X or Escape settles to idle. The X closes even while the reply is being spoken (it used to do nothing until the voice finished); the speaking state then shows as the small bar.
- compose: Escape (or Close without sending) puts the box away and **keeps the draft** (Lacy, 2026-10-17: "if there is text in the text box and someone pushes escape we should still go down to the idle state and just keep that text there"). The island is the capsule at rest even with text in the box (`faceFor`'s `dismissed`). Rust's copy of the text is only a mirror, so it is emptied and then told the box blurred, in that order, and Rust leaves its input state. The draft is back, with the caret, when the composer opens again (a click on the capsule, the Dock) or is focused; focusing it hands Rust its copy again, so Cmd+Enter still sends it. A put-away draft does not hold an answer's ring.
- compose while Juno works: the first press stops the work (Rust's call), the second puts the box away. The draft is kept through both.

Ladder, every face, with or without a draft: chat, then the answer or component, then compose, then idle; dictating or listening, then (stopped) wherever the island was, then on down. Rust's stop-key monitor now ends an open dictation before Settings can take the press (`events/shortcuts.rs`): with Settings focused and something in it holding Escape, every press used to go to Settings and reach nothing, so a dictation could not be stopped with Escape at all (Juno log, 2026-10-17 20:04:59 to 20:05:03, ten presses).

## The ring

- Runs on the answer and component faces only, once the answer has finished streaming, Rust is not working and no tool waits on Allow.
- Never runs while the composer has the caret, or text that has not been put away.
- Twelve seconds. Pauses while the pointer is over the island or anything in it has focus. A press on a control inside cancels it until the pointer leaves, then it starts over.
- On expiry the island settles to idle.

## Motion

- **The steady frame.** The Island now lives on the Pill's steady frame (`lib/steadyFrame.ts`, `useIslandFrame`). The window is sized once per display for the largest face, now the chat face: the chat size, the roster's room on both sides (226), 24 of shadow room each side and 200 for the skill suggestions (1364 x 788 on a 1440 x 900 display). `SteadySpec.maxFor` takes the display, so a drop or a display hop onto a display of another size places a window of the right size behind the usual two-frame hide; the face reads its chat size back from the placed window (`chatSizeInWindow`). The window is placed once per well. Every face change is animation inside it. The page reports the drawn shape through `set_bar_hit_regions`, so the transparent rest of the window passes clicks through, and through `setSteadyFootprint`, so a drag carries a window the size of the shape.
- **The resting frame (LAC-4832).** The window above is 83% of the display's area (1364 x 788 on a 1440 x 900), and macOS lists a window by its frame however little of it is drawn or takes the mouse. So a screen-share picker offered that invisible rectangle instead of the app behind it, and Lacy could not pick any app behind the Island. Click-through cannot reach this (it routes mouse events, it does not leave the window list) and neither can `sharingType = .none` (it only blanks the content inside a capture). The window therefore holds the full face only while a full face is drawn: resting, it is the capsule, the hover capsule it becomes, the roster bubble and the shadow (`islandRestFrame`, 166 x 80 with no roster). The boundary is resting against not-resting, never a state inside a turn, and it is crossed by the same hidden two-frame swap a drag uses. Launch and the tray's re-centre place the resting frame directly, so neither blinks; what is left is ~70ms when the island leaves rest and again when it returns, paid where someone has just acted and never during the morph that follows, plus a second blink on a drop or a display hop while resting (the shared settle grows the window back before this shrinks it again). Full reasoning, measurements and the alternative that was cut: `docs/plans/appearance-steady-frame.md`, "The one exception".
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

Kept on purpose: no settings button.

Superseded on 2026-10-15: "no chat history in the island" held for the answer and component faces, which still show the current turn only. The chat face shows the whole conversation, because Lacy asked for the Pill's chat capability as the island itself rather than a pane; without it the Island had no way to read back further than one turn short of opening the main window. "No roster" and "no wake-phrase button" went with the same parity pass: the Island is a full replacement for the Pill, and an all-day microphone needs a pause within reach.

## The Notch

The Notch was the Island's component with `frame="notch"`. It is now `notch/NotchBar.tsx` with the old posture model in `notch/notchPosture.ts`, unchanged in behaviour. It should get the same one-surface pass; that is a follow-up, not this branch.

## Evidence

Committed with the PR under `docs/changelog/media/<PR>/`: one PNG per face from `/__bar-preview?appearance=dynamic` (`state=default`, `state=listening`, `state=loading`, `state=error`, `demo=answer`, `demo=card`) and an MP4 of one full turn from `scripts/bench-record.sh dynamic demo=turn` (rest, speak, an answer with a component, two Escapes down the ladder to rest).

Second pass, also under `docs/changelog/media/818/`: `hover-wake-toggle.png` (`demo=hover&tip=listen&wake=on`), `dot-offline-tooltip.png` (`demo=hover&tip=dot&net=offline`), `roster.png` (`state=default&agents=3`), `composer-pasted-image.png` (`demo=paste`), `chat-face.png` (`demo=chat&hold=chat&pin=frame`) and `answer-chat-answer.mp4` (`scripts/bench-record.sh dynamic demo=chat`: the answer, Show Chat into the conversation, Escape back to the answer).

Tests: `islandModel.test.ts` (face for every state, shapes, placeholder, stop control, ring rule, the turn reducer, the chat size per display and its read-back, chat in every well grows away from the docked edge inside the window, the escape rungs, pasted pictures as a draft, the hover width, the red dot, the roster, the resting frame against every display size and roster width), `useIslandFrame.test.tsx` (the hit regions on every placement path, the resting frame placed at launch, grown when a face needs it and shrunk back after), `IslandBar.test.tsx` (one composer node through every state and into chat, answer and component faces, Escape ladder including chat, ring, approvals, hover, Rust's real dictation key-up sequence replayed into the follow-up field, a picture pasted and sent alone, the wake toggle, the red dot with its label, the roster at two agents without moving the shape, a tooltip from the forwarded hover), `escapeContract.test.ts` (the Island's chat rung), `notchPosture.test.ts`, `mixed-content-renderer.test.tsx` (`splitHeroContent`).

Third pass (2026-10-17, `fix/island-escape-expand-drag`): Escape reaches the capsule from every face and state, one rung a press, with the draft kept; the hover capsule's Open chat; drag fixes (below). Media under `docs/changelog/media/831/`. Tests: the whole ladder against a stand-in Rust over both routes (the island's own keydown and Rust's monitor), with and without a draft, during a dictation, a spoken reply and a run; the kept draft coming back with Rust's copy; Open chat; the hover capsule held through a drag; the composer draggable by its edge; Rust's dictation-before-Settings order in `escapeContract.test.ts`.

Drag, from the 2026-10-17 `[Drag]` log and the code: the compose face (every state past rest, including the small Listening and Working bar) could not be picked up at all, because the whole composer was `data-no-drag`; now only the text is (`textarea` is in the shared no-drag selector) and the box's padding and controls drag like the Pill's. And the hover capsule could fold to the idle capsule mid-drag on a leave reported as the drag window shrank and moved, shrinking the shape inside a window sized for the hover capsule; it now keeps the look it had at the press until the drop. Seen in the log and not fixed here (shared drag code, every look): a drag that starts late (313 ms and 447 ms from the threshold to the drag window) misses a quick flick, so the window jumps under the cursor and then does not follow.

Not verified here: the real window on hardware, including the resting frame in a real screen-share picker, the larger chat-sized frame through a drag, a drop in every well and a display hop to a display of another size. The bench runs the same components on the fake Tauri layer, so click-through, drag, focus, a real dictation paste and the Rust side of each interaction are for the hardware pass.
