# Orb: one canvas that tells you what Juno is doing (spec)

**Status:** Built on `feat/orb-appearance`. Every posture captured from the preview route; two clips recorded; unit tests green; CI on the PR.
**DRI:** Frontend Engineer. Lacy is DRI for the hardware pass (hold the key, speak, drag, a real answer, a real GPU).

## The ask

Lacy, 2026-10-01: "we lost the coolest appearance type, the orb (react-orb)", and then: "the react orb is canvas, we wanted a full appearance created using it with every state thought-out. it's one of the most beautiful looks, and we use it on the website. it should be well thought out."

So this is not a file restore. The shader is the starting material; the deliverable is a whole appearance built on it, where every state Rust can send has a deliberate expression in the canvas itself.

## What happened to it

PR #632, "Halo rebuilt as a ring that measures the turn", took the `react_orb`
value for the ring and deleted `src/components/ui/react-orb.tsx` with it. The
look had nowhere left to live: the value that named it belongs to the Halo, and
the name "Orb" was on the ElevenLabs orb that PR #630 had rebuilt.

This branch gives the shader its own value, `shader_orb`, and takes the name
"Orb" back for it. The ElevenLabs orb keeps its value and its look and takes
PR #630's own word for itself, "Presence"; its spec moved to
`docs/plans/presence-appearance.md`. Halo is untouched. Eight appearances now,
not seven, and nobody's stored setting moves them to a different look:
`src/components/bar/__tests__/appearanceMigration.test.tsx` is that promise
written down.

The first commit on this branch restores the deleted file byte for byte, so the
rewrite that follows it is readable as a diff rather than as a new file.

## Jobs Standard critique of the starting material (the bar as of `42ba2e86^`)

```
For: someone who chose Orb because they want one beautiful object on the desk
     instead of a bar, and want to know what Juno is doing without reading.
Verdict: REDO
Remove:
- The 10px "Ready" / "Processing..." / "Transcribing..." label under the orb.
  A status word is an admission that the canvas failed to say it.
- The six-bucket hue map in bar-state-mapper (listening 140, thinking 40,
  talking 90, error 180, stopping 20, idle 0): pastel buckets, no meaning.
- `forceHoverState` pinned true, so the voice warp ran in every state at once.
- The rAF loop, which ran at 60fps forever, including on an idle desk.
Fails:
- 1: ten seconds is a 200x200 window holding a violet blob and the word
  "Ready" -> small and dim at rest, cool and swelling the instant you speak.
- 4: the WebGL context was torn down and rebuilt on every hue or intensity
  change, and intensity followed the audio level -> several context rebuilds
  per second while you talk. Build it once, drive it from a ref.
- 4: no approval, no error text, no way to read an answer. An agent asking
  for permission had nowhere to ask -> one panel, four contents.
- 4: rotation only advanced while the pointer was inside the orb, so the
  look was driven by the mouse rather than by the work.
- 7: not a keynote slide. It is a screensaver with a caption.
Ship as: one canvas, every Rust state expressed in it, and a panel that opens
  only for an approval, an error, the composer, or a click to read.
Evidence needed: a PNG per state, a clip of a turn, vitest, tsc, CI.
```

Everything in that critique is addressed below except one thing I kept: the
orb is still a decoration-grade WebGL shader, and if the context cannot be
created the canvas stays empty. That is caught and the rest of the appearance
keeps working.

## Research (what was worth taking)

- **The React Bits orb** (https://reactbits.dev/backgrounds/orb). The shader this keeps: three base colours (violet, cyan, a deep blue core) rotated together by one `hue`, a simplex-noise rim, two radial lights, and one bright point orbiting inside. Lit at the rim, dark at the centre, so it reads as a luminous O rather than a ball. Rotating the triad together is what keeps it one object instead of a repaint.
- **Presence (`docs/plans/presence-appearance.md`, PR #630).** The same brief answered for a different shader. What was worth taking wholesale: a pure model beside the renderer, a drive ref so the canvas never re-renders, a frame loop that sleeps, a stage sized once, and the rule that the orb's centre sits at the same point in every window. What was deliberately not taken: subtitles.
- **Island (`docs/plans/island-appearance.md`, PR #618).** Grow the window first and show second; hide first and shrink second. Same two-phase resize here.
- **Siri on macOS 26 and 27.** The orb is the whole interface and no part of it says "listening". https://support.apple.com/guide/mac-help/use-siri-mchl6b029310/mac
- **WCAG 2.2.1 Timing Adjustable.** The unread answer's clock pauses while the pointer is over the orb or anything inside has focus, and Escape ends it at once. https://www.w3.org/WAI/WCAG21/Understanding/timing-adjustable.html

## The person and the moment

Someone who wants one beautiful object on the desk and nothing else. They hold their key, say a thing, let it go, and expect to know what is happening without reading a word. They read only when they choose to.

## Ten seconds

A small grey ring of light, dim, breathing slowly, its inside at a near stop. They hold the key: it cools toward blue, grows, and its rim ripples with their voice. They let go: it goes green for a beat while their words become text, then takes its own violet and cyan back and starts turning, with a pulse that gets quicker every time a tool finishes. Juno speaks, and the rim ripples to the speech. It blooms green once and settles. On the desk it is now a small green ember: there is an answer. One click and a dark sheet unfolds under it with what Juno heard and what it said. Click again and the sheet folds away. Nothing was read that they did not ask to read.

## What it does not do, and why

The Orb and Presence run the same shader brief on different shaders, so the one
thing that separates them has to be real. It is this: **Presence narrates and
the Orb does not.**

- No status word, in any state.
- No transcript. The orb ripples with your voice, so you know it hears you.
- No answer as subtitles. The answer is spoken; reading it is a click.

That costs something, and the cost is named: if Juno mishears you, nothing on
screen says so until the answer arrives. The recovery is that the sheet carries
the question as well as the answer, so one click shows what Juno heard. A
person who wants to watch their words land should choose Presence.

## Geometry

The canvas is a 160px stage, sized once; the orb scales itself inside it with a uniform rather than by resizing the canvas. Its centre is at `(width / 2, 80)` in every posture. Whatever is under it decides the window:

| Posture | Under the orb | Window |
|---|---|---|
| orb | nothing | 160 x 160 |
| words | one line (30 tall): the error, or the composer | 360 x 212 |
| approval | the question and Allow / Don't (66 tall) | 360 x 248 |
| sheet | the turn (56..320 tall, in steps of 8) | 360 x 238..502 |

The panel is Presence's, imported rather than copied: the same pill, the same
approval card, the same sheet, the same width. Only the canvas and the policy
are the Orb's own, which is also why the two looks line up where they overlap.

## Every state

The orb column is what the canvas does. The words column is what the panel shows, which is almost always nothing.

| Rust bar state | Orb | Words | Leaves when |
|---|---|---|---|
| default | base hue, 35% chroma, 55% bright, 52% of the stage, inside at 0.08, CSS breath, loop asleep | none (green ember instead while an answer is unread) | any state change, click |
| shrinking | same as default | none | Rust: Default after 300ms |
| dictation_ready | green ember, 50% chroma, 60% bright | none | dictation starts |
| always_listening | cool ember, 45% chroma, 40% bright, 50%: dimmer than rest, so a listening desk does not glow all day | none | wake word, or off |
| expanding | base hue, 80% chroma, 90% bright, 64%, inside at 0.35 | composer, disabled | Rust: Input after 300ms |
| input | same | composer, focused | submit, blur, Escape |
| listening | cool hue (350), full chroma, 76% plus up to 18% with your voice, rim ripple 0.22 plus up to 0.5 | none | mic closes |
| dictating | green hue (110), otherwise as listening: those words are yours | none | dictation ends |
| transcribing | green, turning, pulse: still your words, but Juno has them | none | Rust: Submitting or Default |
| submitting | base hue, turning at 0.45 rad/s, pulse every 1.4s | none | agent starts |
| loading | the same, quicker with every finished step (inside +0.3, spin +0.12, pulse period / 1.2 per step, counted to four) | none | first stream chunk |
| agent_responding | the same | none | stream ends |
| speaking | base hue, 76%, rim ripples with Juno's speech level | none | TTS ends |
| finishing | green bloom to 84%, once | none | 300ms (Rust: Default) |
| success | green bloom, once | none | 300ms |
| error | one red object (the three colours collapse), one inward flinch of 12% over 560ms, then holds | the error itself, in red | 3s (Rust: Default) or Escape |
| stopping | base hue dimming, 40% chroma, 70% bright, 66%, slow turn | none | Rust: Default |

Local conditions, layered on top:

| Condition | Effect |
|---|---|
| a tool waits on Allow or Don't | everything stops: the inside is frozen, no spin, no pulse, no ripple. A canvas that has stopped moving is the clearest way to say the next move is yours. The panel asks "Juno wants to <description>" with Allow and Don't |
| Juno is driving the cursor | it turns steadily, no pulse |
| Juno asks for the cursor | the sheet opens with the notice in it |
| an answer arrives and nobody has read it | at rest the orb takes a green ember, 50% chroma and a little larger, for 12 seconds. That ember is what makes the click worth making |
| the answer carries a component | the sheet opens by itself: a component cannot be spoken, so an orb that stayed shut would have swallowed it |
| you click the orb | with something to read, the sheet; with nothing to read, Rust opens the composer. One idea, two shapes: give me the words |
| you speak again | the sheet closes and the ember lets go at once |
| audio level while listening or dictating | scale adds up to 0.18, rim ripple up to 0.5, the inside up to 0.3 |
| audio level while Juno speaks | ripple up to 0.45, scale only up to 0.06: speech disturbs the surface rather than inflating it |
| Reduce Motion on | the CSS breath is off and the panel fades without rising. Colour and scale still ease, because they are state, not decoration |

## The canvas

Eight uniforms, each of them something a person can see.

| uniform | what it is | at rest | at full voice |
|---|---|---|---|
| `uTime` | the inside's own clock, advanced here at the look's flow rate | 0.08x | 1.0x |
| `uRot` | rigid rotation of the whole orb | 0 | 0.1 to 0.57 rad/s |
| `uHue` | degrees applied to all three base colours | 0 | 350 or 110 |
| `uSat` | chroma, 0 grey to 1 full | 0.35 | 1 |
| `uMono` | collapses the three colours onto one red object | 0 | 0 (error only) |
| `uScale` | how much of the canvas the orb fills | 0.52 | up to 0.94 |
| `uRipple` | surface disturbance | 0 | up to 0.72 |
| `uBright` | how present it is | 0.55 | 1 |

Three decisions inside that table are the design:

- **`uTime` is advanced here, not read from the frame clock.** That is what makes `flow: 0` a freeze rather than a slow drift, and a freeze is what an approval needed.
- **Thinking is motion, not a colour.** Working keeps the orb's own violet and cyan and says the work through turning, a quickening pulse and a racing inside. Only three hues exist: the orb's own, cool for the open microphone, green for your words landing and for done. Each one is a message that must not be missed, which is the opposite of the six pastel buckets this replaced.
- **`uMono` exists for exactly one state.** A failure must not read as a mood, so the triad collapses into one red object. A test asserts no other state ever sets it.

Everything else in the shader is the original's: the same simplex rim, the same two lights, the same orbiting point, the same `extractAlpha`. The ripple is literally the original's hover warp, read from state instead of from the pointer.

## Performance

- **The loop sleeps.** It stops once the look allows it (resting or frozen), the eased values have settled, no impulse or pulse is running, and 2.5 seconds have passed. The grace period matters: cutting straight to a still frame reads as a stall, coming to rest reads as rest. Any `bar-state-update` wakes it.
  The sleep decision deliberately does not read the `frameloop` prop. It did at first, and that was a deadlock: the bar only sets `frameloop` to `"demand"` in response to `onSettled`, which only fires when the loop sleeps, so an idle desk would have drawn a frame every 16ms forever. `OrbShaderCanvas.test.tsx` is that bug written down as a test. It cannot be shown in the preview route, which re-sends a held frame every 400ms (`AppearancePreview.tsx`), so a still or a clip can never show this orb asleep.
- **The resting breath is CSS**, a transform animation on the wrapper, so the orb still reads as alive on an idle desk with the WebGL loop asleep. It is paused, not removed, in active states, so it never snaps.
- **The context is built once.** The bar writes a ref, the loop reads it. No prop change rebuilds anything, which is the bug that made the old bar rebuild its context several times a second while you spoke.
- **The chunk is small.** `shader-orb-bar` builds to 61 kB, against 906 kB for Presence, because `ogl` is a few hundred lines and Three.js is not. `ogl@^1.0.11` never left package.json, so nothing was installed for this.

## Removed (and considered, then cut)

- The status label, and `getStatusLabel` for this look. It stays for Halo and Avatar.
- The whole of `mapToReactOrbConfig` as the Orb's source of truth. It is still in `bar-state-mapper.ts` and now has no caller; deleting it is a separate change, since that file is shared.
- Subtitles, a transcript line, and a running tool's name under the orb. Considered, and cut: that is Presence, and two looks that behave the same are one look with a settings toggle.
- Hover to open the sheet. Presence has it; here a click is the only way in, so passing the pointer over the orb never changes the window.
- A tick ring, a progress arc, or anything drawn around the canvas. The pulse cadence is the progress, and a ring is Halo.
- A second accent colour for "thinking". Cut on purpose: see above.
- Changing the shader's violet and cyan to system blue. The no-purple rule is about chrome (gradients and glows on text, borders and cards), not about rendered artwork, and this artwork is the thing being restored. `uHue` is there if a future tuning is wanted.

## Evidence

- `docs/frontend/screenshots/shader-orb/`: one PNG per state from `/__bar-preview?appearance=shader_orb&state=<state>` (default, always-listening, dictation-ready, input, listening, dictating, transcribing, loading, agent-responding, speaking, finishing, error, stopping), plus `approval.png` and `card.png` from the preview demos.
- `turn-loop.mp4`, ten seconds of the picker's loop (resting, listening, dictating, done) and `full-turn.mp4`, eleven seconds of one turn ending in a component answer that opens the sheet by itself. Both recorded with `scripts/bench-record.sh shader_orb`.
- Unit tests: `shaderOrbModel.test.ts` (36: every state, the audio, the sleep rule, the words policy, the postures, the window), `OrbShaderCanvas.test.tsx` (6: the real loop with ogl stubbed, so the sleep, the wake, the frozen clock, the hue easing taking the short way round, and a missing WebGL context are all exercised rather than mocked away) and `ShaderOrbBar.test.tsx` (16: rest and sleep, silence while you speak, silence while Juno works, the frozen approval and Allow, red with the error, the green ember and the click, the sheet opening by itself, the clock, speaking again, history, the composer, the click with nothing to read, Escape, driving, the two-phase resize, and one test that walks twelve states asserting the bar renders no text at all).
- `appearanceMigration.test.tsx` (14): every stored value lands on the component it landed on before, and the catalog has no duplicate value or name.
- `tsc` clean, `vitest` 57 files and 737 tests green, `bun run build` clean, `rustfmt --check` clean on `constants/ui.rs`.
- Not verified here: the real window on hardware, a real GPU's frame cost, the WebGL sleep under a real compositor, drag, OS focus, and the Rust side of every interaction. The preview runs the real components on the fake Tauri layer. Rust was not compiled locally; CI does that.
- Not captured: a still of the green unread ember on its own. The preview has no demo that reaches it with the sheet shut; it is covered by `ShaderOrbBar.test.tsx` and by the model test, and it is visible in `full-turn.mp4` only with the sheet open.

## Follow-ups (not in this branch)

- `mapToReactOrbConfig` and `ReactOrbConfig` in `bar-state-mapper.ts` now have no caller. Delete them with the next change to that file.
- A still that holds the unread green ember needs a `demo=held` in the preview route. Worth adding when the next appearance PR touches that file.
- Eight appearances is a lot for one picker. If one should go, that is a product decision and not this branch's to make.
- The preview's `state=speaking` frame carries no live audio level, so the held speaking still shows the resting ripple. The same gap Presence noted.
- `docs/frontend/screenshots/orb/` still holds Presence's stills, because that is where its spec points. Rename the directory to `presence/` when something else touches it; this branch left it alone rather than rewriting a shipped spec's evidence paths.
