# Studio: a recording studio for your voice (spec)

**Status:** Built on `worktree-agent-a494fa46f7291f5ae`; every posture captured from the preview route; unit and component tests green; CI on the draft PR.
**DRI:** Frontend Engineer. Lacy is DRI for the hardware pass (real mic level, real speech, drag, focus).

## The ask

Lacy, 2026-09-30: every appearance can feel like an entirely different app. Studio had a waveform and a transcript but the states were half-built. Redo it from scratch as its own app: a recording studio. The waveform is the hero and it is real. The transcript is a teleprompter. Progress is a tape counter, not a spinner. The answer arrives as a script. Light palette, system blue for Juno, system green for the person's own speech, red only for failure. Find the studio's own idle cue for always listening (not a red light).

## Jobs Standard critique of Studio as of main `58f94df2`

```
For: someone who chose Studio because they want to see their own voice on the desk, light and plain; they press the key and speak.
Verdict: CUT
Remove:
- The level meter. One metaphor for progress: the tape counter (mm:ss.f, tabular numbers) runs while Juno works and holds when it stops.
- The marquee, the lucide icon soup (Mic, Brain, Loader2, Send, Copy...), the canvas AudioVisualizer with its transition bar, the sample responses and dev panel, the four response phases, the 160x40 idle bar.
- Copy and share buttons, chat history, roster, wake-phrase toggle, settings shortcuts. The Studio shows the current take.
- A second composer. One composer, which is the teleprompter box itself while Rust is in its input state.
- Red anywhere but failure. Always listening is "Mic open": the baseline turns blue and drifts, no light.
Fails:
- 1: idle today is a bar with a mic icon and a label -> a deck: a thin flat hairline waveform in a light capsule, nothing else. The first thing that moves is your own voice.
- 4: progress was a spinner and a marquee -> the counter starts at 00:00.0 the instant you let go and keeps counting while the tool runs; it stops when the answer is complete, so the number is honest.
- 4: response faded in after the box grew -> grow the window first, spring the shape, crossfade the layer, all from a centred anchor (Island's #620 lesson).
- 6: spoken text shown twice (ticker and prose) -> the Juno line in the script is what was said aloud; the visible text and components are its notes beneath, once.
Ship as: five postures (deck, take, type, work, script), a real waveform driven by audioLevel and by spokenText cadence, a two-line teleprompter, a tape counter, a script with You/Juno lines, italic stage directions for tools and approvals with Allow and Don't. Every Rust state in a table with a unit test.
Evidence needed: a PNG per posture from the preview route, one MP4 of a full turn, tsc and vitest green, CI green on the draft PR.
```

## Research (what was worth taking)

- **Apple Voice Memos (iOS 17 and macOS Sonoma).** The waveform is a history of the level, newest at the right, drawn as thin bars mirrored around a centre line. Nothing pulses; the line only moves when you make sound. That honesty is the whole reason a waveform reads as "real". Studio's strip is the same idea: a ring buffer of the last 48 levels, newest on the right.
  https://support.apple.com/guide/voice-memos/welcome/mac
- **Teleprompters (PromptSmart, Teleprompter Premium, BIGVU).** The line being read sits at a fixed reading position; earlier lines scroll away above it, upcoming lines are lighter. Nothing jumps sideways. Studio anchors the newest line at the bottom of a two-line box and lets older lines rise out of it, with a fade at the top. Provisional words are lighter, the way an upcoming line is.
  https://promptsmart.com/ and https://bigvu.tv/teleprompter
- **Tape counters (Nagra, Studer, Otari).** A tape counter tells you where you are and how long it has been running; it never predicts. That is the right shape for "Juno is working": elapsed time in `mm:ss.f`, tabular figures, and it stops when the work stops. No estimate, no spinner. When a script lingers before closing, the same tape runs out: a hairline under the header drains from right to left, the reel emptying. One metaphor, two moments.
- **Screenplay format (Fountain, Final Draft).** Speaker names in small caps, dialogue beneath, stage directions in italics between lines. A reader can tell who spoke and what happened without a single icon. Studio's script uses exactly that: "You" and "Juno" lines, notes beneath the Juno line, italic directions for a running tool and for a request to allow something.
  https://fountain.io/syntax/
- **Auto-dismiss with a visible timer (WCAG 2.2.1 Timing Adjustable).** The countdown must be visible, must pause on hover and focus, and the person must be able to stop it. The draining tape is the timer; hover pauses it; any interaction cancels it until the pointer leaves.
  https://www.w3.org/WAI/WCAG22/Understanding/timing-adjustable.html
- **Studio talkback and "mic open" lights.** A studio's red light means recording, which Juno's rules forbid outside failure. What a studio does have is an open channel: the fader up, the meter alive at a whisper. Studio's always-listening cue is that: the baseline goes blue and drifts by a pixel, and the label says "Mic open". No light.

## The person and the moment

Someone at their desk who chose Studio. They like seeing their own words as they say them and want a light surface that matches their apps. They hold the key, speak, let go, and expect the answer to arrive as something they can read back like a script.

## Ten seconds

A small light capsule with a flat grey hairline through it rests where they left it. They hold the key: the capsule widens into a take. The hairline becomes a waveform that moves with their voice, green, newest at the right. Beneath it their words roll up a two-line teleprompter, the last word or two lighter until the engine commits them. They let go: the waveform settles flat, the words go solid, and a tape counter starts at 00:00.0 beside them. The counter runs while Juno works. If Juno runs a tool, an italic stage direction says so. The take grows downward into a script: "You", their sentence; "Juno", what Juno says aloud, the waveform in the header moving blue with the speech; the notes (text, a component) beneath. When the counter stops, a hairline tape under the header runs out over twelve seconds and the script settles back to the capsule. Nothing was clicked.

## Postures

Every Rust bar state, every stream event and every local condition maps to one of five postures. The posture decides the size and which layer is on stage. The waveform, the words and the counter are decided per state below.

| Posture | Studio size | Radius | Window (studio + 24px shadow each side) | On stage |
|---|---|---|---|---|
| deck | 132 x 30 | 15 | 180 x 78 | hairline |
| take | 320 x 66 | 16 | 368 x 114 | waveform strip + two-line teleprompter |
| type | 320 x 40 | 16 | 368 x 88 | hairline + composer |
| work | 320 x 66 | 16 | 368 x 114 | waveform strip + your words (dim) + tape counter |
| script | 360 x 120..340 | 18 | 408 x 168..388 | header (waveform + counter + close) + script + follow-up |

Take and work share a size on purpose: the studio must not lurch the instant someone stops talking. Type is shorter because a 66px box around one line of text reads as empty.

Posture is chosen in this order: script if a script is open, work if Juno is driving the cursor, type if Rust is in an input state, take for listening, dictating, transcribing and always listening, work for every working, speaking, error, success, finishing and stopping state, deck otherwise.

## Every state

"Wave" is what the strip draws. "Line" is the text beneath it (the teleprompter in the take, the single line in work). The counter column says whether the tape counter is running, held, or absent.

| Rust bar state | Posture | Wave | Line | Counter | Leaves when |
|---|---|---|---|---|---|
| default | deck | flat grey hairline | none | none | any state change, click (Rust: Expanding), hover darkens the hairline only |
| dictation_ready | deck | flat green hairline, still | none | none | dictation starts |
| shrinking | deck | flat grey hairline, 40% | none | none | Rust: Default after 300ms |
| expanding | type | flat grey hairline | composer, disabled | none | Rust: Input after 300ms |
| input | type | flat grey hairline | composer, focused; "return" appears once there is text | none | submit, blur with empty text (Rust: Shrinking), Escape |
| listening | take | green, follows audioLevel | "Listening" until the first partial arrives, then the teleprompter | none | mic closes |
| dictating | take | green, follows audioLevel | "Dictating" until the first partial, then the teleprompter | none | dictation ends |
| transcribing | take | green, follows audioLevel; settles flat once the level is 0 | the teleprompter, provisional words lighter, final words solid | none | Rust: Submitting, Dictating or Default |
| always_listening | take | blue baseline, drifts by a pixel | "Mic open" | none | wake word (Rust: Listening) or off |
| submitting | work | flat green, settling | what you said, dim | starts at 00:00.0 | agent starts |
| loading | work | flat grey | what you said, dim; a running tool replaces it as an italic stage direction | running | first stream chunk (Rust: AgentResponding) |
| agent_responding | script | in the header: blue while Juno speaks, grey flat otherwise | the script | running | stream ends |
| speaking | script if open, else work | blue, synthesised from spokenText cadence | the sentence being spoken, blue, when no script | running if work continues, else held | TTS ends |
| finishing | script if open, else work | flat green, one settle | "Done" when no script | held | 300ms (Rust: Default) |
| success | script if open, else work | flat green | "Done" when no script | held | 300ms |
| error | script if open, else work | flat red hairline, one shake | the error text, red | held | 3s (Rust: Default) or Escape |
| stopping | work | flat grey, 50% | "Stopping" | held | Rust: Default |

Local conditions layered on top:

| Condition | Effect |
|---|---|
| script open, Rust in `input` | the follow-up field at the bottom of the script is the composer; the posture does not change |
| script open, Rust goes to listening or dictating | the script closes; the person is speaking again and the take must be free |
| pending tool approval in the conversation | the script opens (if not open) with one stage direction: "Juno asks to <description>" and Allow, Don't. The tape does not run out while an approval is pending |
| a tool is running | an italic stage direction between the lines: "Juno runs <description>" |
| Juno is driving the cursor (input-control state) | work posture, the driving label as a stage direction, no tape |
| reply has only spoken text and no visible content | the Juno line is the spoken sentence; there are no notes |
| reply has visible content and spoken text | the Juno line is the spoken sentence; the visible text and components are the notes beneath it |
| reply has visible content and no spoken text | the visible content is the Juno line itself |
| reply is a component | the component is a note, edge to edge inside the script padding; prose before and after renders around it |
| script taller than 340 | the body scrolls; the tape pauses while the pointer is over the studio |
| new turn starts while a script is open | the script clears at the first chunk; the counter resets to 00:00.0 |
| window loses focus with the composer empty | Rust shrinks to Default; an open script stays open |
| Reduce Motion on | the waveform is a static row of the current level; springs become 150ms ease-out; no blur on layers; the tape still drains (it is the timer, not decoration) |

## The waveform

- A ring buffer of the last 48 levels, sampled every 50ms, drawn as 2px bars mirrored around the centre line with 1px between them. Newest on the right. Bars shorter than the hairline draw as the hairline, so silence is a flat line and never nothing.
- Your speech: the level is `audioLevel` from the bar state, smoothed (attack fast, release slow) so a burst of consonants does not spike and a pause does not cut to zero. Green.
- Juno's speech: no level exists for TTS, so the level is synthesised from the sentence being spoken: syllables at four per second, a small dip between words, a deterministic ripple from the text so two sentences do not look identical. Blue. It runs while Rust is in `speaking` with `spokenText`, and stops flat when speech ends.
- Rest: grey hairline. Always listening: blue hairline drifting by a pixel, slowly.
- Failure: red hairline, one shake.
- Colours: system green `#30D158`, system blue `#0A84FF`, system red `#FF453A`, hairline `rgba(0,0,0,0.18)`.

## The teleprompter

- A box two lines tall. The newest words sit at the bottom; older words rise out of the top behind a fade. No horizontal motion, ever.
- Final words are solid (`rgba(0,0,0,0.85)`); provisional words are lighter (`rgba(0,0,0,0.4)`). The split is the longest final text Rust has sent so far: everything after it is provisional while the flag is set.
- Until the first partial arrives it says what is happening: "Listening", "Dictating", "Mic open".

## The tape

- The counter starts at 00:00.0 the moment the take ends (Rust: Submitting) and runs while Rust is working or the conversation is processing. It holds when both stop. It resets to 00:00.0 at the next take.
- `mm:ss.f`, tabular numbers, 11px, grey. It never estimates.
- When the script is complete (not streaming, Rust idle, no approval pending), a 1px tape under the header drains from right to left over 12 seconds. Pointer over the studio or focus inside it pauses the tape; typing in the follow-up or pressing a button cancels it until the pointer leaves, then it starts fresh. When it runs out, the script closes and the studio settles to the deck. Escape or the close mark do the same at once.

## The script

- Header: the waveform strip (small), the counter, the close mark. The tape hairline under it.
- "You" line: the question.
- Stage directions in italics between lines: "Juno runs <tool>" while a tool runs, "Juno ran <tool>" once it has, "Juno asks to <description>" with Allow and Don't while an approval waits, and the driving label while Juno holds the cursor. `InputControlNotices` renders the cursor-control request itself.
- "Juno" line: what Juno says aloud, when there is spoken text; the visible content otherwise. Notes (visible text and components) sit beneath the line, indented, rendered by `MixedContentRenderer`.
- Footer: the follow-up composer, only while Rust is in its input state.

## Motion

- One spring for width and height: stiffness 380, damping 34, mass 1. Settles in about 320ms without overshoot.
- Content layers crossfade: entering layer opacity 0 to 1 and blur 4px to 0 over 180ms, 60ms after the spring begins. Leaving layer fades over 120ms. Both absolutely positioned so nothing reflows.
- Window protocol, unchanged from Pill and Island: growing, resize the window then spring; shrinking, spring then resize 350ms later. Both through `useWindowSize("floating-bar").resizeWindowIfChanged`, so position and size land in one frame.
- The studio is centred in its window and grows around itself. Nothing is anchored to a window edge, so the grow-first step cannot move anything on screen (the Island #620 lesson). The waveform strip sits at the top-left of the studio in every posture but the deck, where it is centred; the change is a crossfade between layers, never a slide, so the strip cannot be seen jumping.
- The script's height follows its content through a ResizeObserver, clamped to 120..340 in 8px steps, so a streaming answer grows word by word under the same spring and never resizes the window per glyph.

## Removed (and considered, then cut)

- The canvas `AudioVisualizer` (`audio-visualizer.tsx`) with its state transitions and progress bar. Replaced by an SVG strip that draws the level history. Deleted; nothing else imported it.
- `react-fast-marquee` and its CJS unwrap. The teleprompter scrolls up, never sideways. The package stays in `package.json` untouched (lockfile discipline); nothing imports it now.
- The `tauri.conf.json` import and `getBarLayoutWindowLabel`: the studio sized a `voice-bar` window that does not exist in the config. It sizes `floating-bar`, the window it lives in.
- The sample responses, the dev panel, the four response phases, the content-type switch (text, code, component, image, video), the copy button, the expand chevrons.
- A level meter next to the counter. Two metaphors for one moment is one too many.
- Chat history. The script is the current take; history lives in the main window.
- A red recording light for always listening. Red is for failure only.
- A word-by-word typewriter for the Juno line. Words arrive as Rust streams them; a second animation on top would be a lie about timing.

## Evidence

- `docs/frontend/screenshots/studio/`: one PNG per posture from `/__bar-preview?appearance=voice_ai&state=<state>` and `&demo=card` (default, dictation_ready, input, listening, dictating, transcribing, always_listening, submitting, loading, speaking, error, script-streaming, script), plus `script-demo.mp4`, a clip of one full turn made with `scripts/bench-record.sh voice_ai card`.
- Unit tests: `studioModel.test.ts` (posture, size, wave, line and counter for every bar state; the teleprompter split; the speech envelope; the tape rule; the script reducer). `VoiceAIBar.test.tsx` (state to posture, window protocol, script opens on a new answer and not on history, tape counts only when idle, pauses on hover, Escape closes or stops, approvals reach the backend, the spoken line and the notes).
- `tsc` clean; `vitest` green.
- Not verified here: the real window on hardware. The bench runs the same components on the fake Tauri layer, so resize timing, drag, OS focus, the real mic level in `audioLevel`, and real TTS timing against the synthesised envelope are for the hardware pass (Lacy, DRI).

## Follow-ups (not in this branch)

- `audioLevel` in the bar payload is initialised to 0 in `ui_commands.rs` and nothing in the Rust UI manager writes it; the pill reads the same field. If it is always 0 on hardware, the waveform will be flat while you speak, and the fix is a Rust one: feed the voice plugin's `audio-level` into the bar state. The studio also listens to nothing else on purpose, so one fix serves every appearance.
- Grow upward when the studio is docked in the bottom half of a display. The script grows downward today and the window is clamped to the monitor.
- A component that is still streaming renders as empty space until its closing tag arrives. Belongs to the JSX renderer.
