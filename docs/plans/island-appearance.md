# Island: one window that grows to hold the answer (spec)

**Status:** Built on `feat/island-appearance`; every posture captured from the preview route; unit tests green; CI (fmt, clippy, cargo test) on the draft PR.
**DRI:** Frontend Engineer. Lacy is DRI for the hardware pass (tap, hold, drag, real answers).
**Companion fixes in the same branch:** appearance switch no longer reloads the bar window; settings toasts no longer stack; orb and avatar chunks preload; the picker keeps its neighbours warm.

## The ask

Lacy, 2026-09-29: every appearance but Pill is broken or wildly incomplete. Start with Island because it is closest to Pill. Island must not open a chat pane. It is one dynamic, constantly changing floating window with a beautiful resize animation. Spoken text sits behind a button. When a component is shown, a circular countdown runs; if nobody interacts, the island settles back to a small idle bar, which itself needs to shrink a lot. Think every state through from scratch: how words display, how components show, everything.

## Jobs Standard critique of Island as of main `a0d16c26`

```
For: someone who chose Island because they want Juno to be one small object on their desk, not a pill plus a pane.
Verdict: REDO
Remove:
- The generic `dynamic-island.tsx` primitive (14 aspect-ratio presets, a 691px MIN_WIDTH, clip paths to squircles that do not exist, hover shadow, focus-within background).
- The 150x44 idle island. Idle should be a mark, not a bar.
- The fixed 419px-wide window in every state: a 92px capsule inside a 419px transparent window steals clicks from whatever sits beside it.
- Response text that fades in 350ms AFTER the island has already grown (a blank box, then words).
- The pinned response that never leaves until someone finds the 8px close button.
- Two different follow-up inputs (one for input mode, one for pinned mode).
- The lowercase-everything labels ("listening", "done", "finishing"). Juno's other surfaces use sentence case.
Fails:
- 1: the first ten seconds are a big black lozenge that does nothing until you click it -> a capsule the size of the iPhone's island, a dot that breathes, words the instant you speak.
- 4: growing then fading content in -> content and size move together, one spring, content blurs in as the island opens.
- 4: no way to know an answer will go away -> the ring on the dismiss control is the timer; hover stops it.
- 6: "spoken text" arrives twice (TTS ticker in the bar and prose in the card) -> the spoken channel lives behind one speaker button.
- 7: island next to Pill on a slide: Pill wins on every state -> redo below.
Ship as: the state table below, complete. No chat history, no roster, no wake-phrase button, no settings shortcuts in the island.
Evidence needed: a screenshot of every posture from the preview route, tsc, vitest, CI green.
```

## Research (what was worth taking)

- **Apple, Live Activities / Dynamic Island (HIG, WWDC23 "Design dynamic Live Activities").** Compact is 36pt tall; expanded stays under 160pt. The expanded view is an enlarged version of the compact one, so layouts must expand predictably: the same elements keep their places. No background imagery; the island is a canvas of foreground elements. Interactive controls confuse people when they are not obviously controls.
  https://developer.apple.com/design/human-interface-guidelines/live-activities and https://developer.apple.com/videos/play/wwdc2023/10194/
- **Sentient OS "Notch Magic", Boring Notch, NotchNook.** State transitions are content-level; the shape morphs with a spring while the content crossfades. Sentient never resizes the window during a morph because a notch must stay glued to the bezel. Juno's island floats, so it keeps Pill's proven protocol instead: grow the window first, then spring the island; spring first, then shrink the window.
  https://github.com/Sentient-OS-Labs/sentient-os and https://theboring.name/
- **Raycast Quick AI, ChatGPT companion window, macOS 27 Siri pill.** The answer arrives in the same small window you asked from, and you keep typing to follow up. Siri's pill floats beside your work and expands into an answer card. That is the whole model: one surface, ask and answer in place.
  https://manual.raycast.com/ai/quick-ai and https://www.macrumors.com/2026/07/20/secret-siri-interface-macos-27-beta-enable/
- **Auto-dismiss with a visible timer (WCAG 2.2.1 Timing Adjustable).** The countdown must be visible, must pause on hover and focus, and the person must be able to stop it. The shrinking ring is the timer; hover pauses it; any interaction cancels it until the pointer leaves.
- **"Jarvis" HUDs.** Surveyed a dozen (GitHub and Blink clones). What they share is decoration: rings, tick marks, scan lines, orbiting dots, cyan on black. None of it survives the stage test and all of it is banned by Juno's flat, system-blue rule. Two ideas do survive and both are already in Island's DNA: the response comes to you where you are, and status is shown by motion rather than by words or icons.

## The person and the moment

Someone at their desk who chose Island. They press their key, say a thing, let go. They expect one small object to react at once, show the answer, and get out of the way without being told to.

## Ten seconds

A dark capsule the size of a pill's cap sits where they left it. A dot inside breathes slowly. They hold the key: the capsule widens, the dot turns blue, and their words appear inside as they speak, newest words always visible. They let go: the dot orbits, their sentence dims. The island grows downward into a card and the answer types in. If the answer is a component (weather, a file list, a timer) the component is the whole card. A thin ring around the close control drains over twelve seconds. They read, they move the mouse away, the ring runs out, the card shrinks back into the capsule. Nothing was clicked.

## Postures

Every Rust bar state, every stream event and every local condition maps to one of five postures. The posture decides size, corner radius and which layer is on stage. The dot and the words inside are decided per state, below.

| Posture | Island size | Radius | Window (island + 24px shadow each side) | On stage |
|---|---|---|---|---|
| capsule | 92 x 28 | 14 | 140 x 76 | dot |
| ear | 300 x 32 | 16 | 348 x 80 | dot + live words |
| line | 340 x 36 | 18 | 388 x 84 | dot + composer + "return" |
| status | 300 x 32 | 16 | 348 x 80 | dot + one line |
| card | 360 x 96..320 | 20 | 408 x 144..368 | header (dot + your words + ring) + body + follow-up |

Ear and status share a size on purpose, the same lesson Pill learned: the island must not lurch wider the instant someone stops talking.

Posture is chosen in this order: card if a card is open, line if Rust is in an input state, ear for listening, dictating and always listening, status for every working, speaking, error, success, finishing and stopping state, capsule otherwise.

## Every state

The "words" column is the single line inside the island. Newest words stay visible: the line is right-aligned inside an overflow-hidden box with a fade on its left edge, so a long dictation scrolls like a ticker without motion.

| Rust bar state | Posture | Dot | Words | Leaves when |
|---|---|---|---|---|
| default | capsule | white 45%, breathes 4s | none | any state change, click (Rust: Expanding). Under the pointer it widens to the hover posture (132x32): talk, type, and "Show last answer" once there is a turn, the way back to a closed card. The tray Show/Hide Chat toggles the same card. After Escape or a dismissed card it stays a capsule until the pointer leaves and returns |
| dictation_ready | capsule | green 60%, still | none | dictation starts |
| shrinking | capsule | white 25% | none | Rust: Default after 300ms |
| expanding | line | white 70%, still | composer, disabled | Rust: Input after 300ms |
| input | line | white 70%, still | composer, focused; "return" appears once there is text | submit, blur with empty text (Rust: Shrinking), Escape |
| listening | ear | system blue, breathes, ring pulses | "Listening" until the first partial arrives, then the live transcript | mic closes (Rust: Transcribing) |
| dictating | ear | green, breathes | live transcript, provisional text dimmed | dictation ends |
| always_listening | ear | blue 40%, slow breath | "Listening for “hey Juno”" | wake word (Rust: Listening) or off |
| transcribing | status | green, breathes | the final transcript, dimmed | Rust: Submitting or Default |
| submitting | status | white, orbits | the words you said, dimmed | agent starts |
| loading | status | white, orbits | the words you said; a running tool's description replaces it while the tool runs | first stream chunk (Rust: AgentResponding) |
| agent_responding | card | white, orbits (in the card header) | your words in the header, the answer typing in the body | stream ends |
| finishing | card if open, else status | green flash 600ms | header unchanged, ring starts | 300ms (Rust: Default) |
| success | card if open, else status | green flash | "Done" when no card | 300ms |
| speaking | card if open, else status | white, ripples | the sentence being spoken, dimmed, when no card | TTS ends |
| error | card if open, else status | red, one shake | the error text, red | 3s (Rust: Default) or Escape |
| stopping | status | white 50%, orbits slower | "Stopping" | Rust: Default |

Local conditions layered on top:

| Condition | Effect |
|---|---|
| card open, Rust in `input` | the follow-up field at the bottom of the card is the composer; the island does not change posture |
| card open, Rust goes to listening or dictating | the card closes; the person is speaking again and the ear must be free |
| pending tool approval in the conversation | the card opens (if not open) and shows one row: "Juno wants to <description>" with Allow and Don't. The ring does not run while an approval is pending |
| Juno is driving the cursor (input-control state) | status posture, the driving label, no ring |
| response has only spoken text and no visible content | the card body shows the spoken sentence itself; the speaker button is not shown twice |
| response has visible content and spoken text | body shows the content; a small speaker button in the footer reveals the spoken text under it |
| response is a component | the component is the body, edge to edge inside the card padding; prose before and after it renders as usual around it |
| answer taller than 320 | the body scrolls; the ring pauses while the pointer is over the card |
| new turn starts while a card is open | body clears at the first chunk; header takes the new question; ring resets |
| window loses focus with the composer empty | Rust shrinks to Default; an open card stays open |
| Reduce Motion on | springs become 150ms ease-out; no blur on content; the ring still drains (it is the timer, not decoration) |

## The ring

- Starts when the answer is complete: the message is no longer streaming, Rust is no longer working and no approval is pending.
- Lasts 12 seconds. Long enough to read three lines; anything longer is meant to be read on hover.
- Draws as a 22px circle around the close control in the card header. The stroke drains clockwise. Its colour is white at 35%: it is a timer, not an alert.
- Pauses while the pointer is over the island, while anything inside it has focus, and while the body is being scrolled. Resumes from where it stopped when the pointer leaves and nothing has focus.
- Cancelled outright by typing in the follow-up field or pressing a button. Starts fresh once the pointer leaves.
- When it runs out: the card closes and the island springs back to the capsule. Escape does the same at once. Clicking the control does the same.

## Motion

- One spring for width, height and radius: stiffness 380, damping 34, mass 1. Settles in roughly 320ms without overshoot on the corners.
- Content layers crossfade: entering layer opacity 0 to 1, blur 6px to 0, scale 0.98 to 1 over 180ms, 60ms after the spring begins. Leaving layer opacity 1 to 0, blur to 6px over 120ms. Both layers are absolutely positioned so nothing reflows.
- The dot is drawn at one fixed home (16px from the left edge, vertically centred) in every posture except the capsule, where it is centred. Growing from the capsule reveals words to the dot's right; the dot itself slides the short distance from centre to home under the same spring.
- Window protocol, unchanged from Pill and the old Island: growing, resize the window then spring; shrinking, spring then resize 350ms later. Both through `set_bar_frame` so position and size land in one frame.
- The card's height follows its content through a ResizeObserver, clamped to 96..320, so a streaming answer grows the card word by word under the same spring.

## Removed (and considered, then cut)

- The `dynamic-island.tsx` primitive and `dynamic-bar.tsx`. Replaced by `IslandShell` (one motion.div) and `IslandBar`.
- The fixed-width window. The window is now the island plus shadow, in every posture.
- The 20-character streaming threshold before content shows.
- The content fade-in delay after the spring.
- Two follow-up inputs. There is one composer, in the line posture, and one follow-up field, in the card.
- Chat history in the island. The island shows the current turn. History lives in the main window; a follow-up continues the same conversation there too.
- The roster strip, wake-phrase button, "open in window" and settings buttons that Pill's pane carries. Considered "open in window" for long answers; cut for now because the body scrolls and the main window already has the conversation. Revisit if people ask.
- Lowercase status words.

## Companion fixes (same branch)

1. **Appearance switch reloads the bar window.** `ui_set_bar_config` navigated the bar window to a per-appearance route on every change. Every route renders `BarHost`, which already swaps the component when `floating-bar-config-changed` arrives, so the navigation only caused a full page load: a blank window, a second `useSettings` load in the bar window and its "Failed to load some settings" toast, and the lazy orb and avatar chunks fetched from cold. The navigation is removed.
2. **Settings toasts stack.** `SettingsProvider` wrapped every route, so each bar page and each preview frame ran the whole settings loader on mount. The provider now wraps only the windows that read settings (`/`, `/settings`). The loader's error toast also carries a fixed id so a repeat can never stack.
3. **Orb and avatar start blank.** `BarHost` preloads the lazy chunks after the first paint of any bar, so a switch lands on warm code. The picker mounts the previous and next looks hidden beside the current one, so stepping is instant, and the preview reports "ready" one frame after the bar has painted so a WebGL canvas has drawn before the frame fades in.

## Evidence

- `docs/frontend/screenshots/island/`: one PNG per posture from `/__bar-preview?appearance=dynamic&state=<state>` and `&demo=card` (default, listening, dictating, input, submitting, error, always_listening, card-streaming, card), plus `card-demo.mp4`, a nine-second clip of one full turn made with `scripts/bench-record.sh dynamic card`. Any appearance can be recorded the same way; the clip is what a release post should carry.
- Unit tests: `islandModel.test.ts` (posture, size, dot and words for every bar state; the ring rule; the current-turn reducer), `IslandBar.test.tsx` (state to posture, window protocol, card opens on a new answer and not on history, ring counts only when idle, pauses on hover, cancels on a press and restarts on leave, Escape closes or stops, approvals reach the backend, the spoken channel stays behind its button), `AppearancePicker.test.tsx` (neighbours mounted hidden, warm neighbour shows at once).
- `tsc` clean; `vitest` 34 files, 342 tests green (2026-09-29).
- Not verified here: the real window on hardware. The bench runs the same components on the fake Tauri layer, so resize timing, drag, focus and the Rust side of every interaction are for the hardware pass (Lacy, DRI). CI compiles the Rust change.

## Follow-ups (not in this branch)

- The recording caught a real one, now fixed in the same branch: the island was pinned to the window's left edge, so the capsule jumped left when the window grew first. It is centred now and grows around itself.
- Grow upward when the island is docked in the bottom half of a display, the way Pill's pane does. The island grows downward today and the window is clamped to the monitor, so nothing is lost, but a low island will jump up when the card opens.
- The agent cards (`agent-cards/index.tsx`) still carry a purple icon tint. Out of scope here; it shows in the card body for task lists.
- A component that is still streaming renders as empty space until its closing tag arrives. Same in the pane; belongs to the JSX renderer.
- Pill, Bar, Studio, Orb, Halo and Avatar: each needs the same from-scratch pass. Island is the template.
