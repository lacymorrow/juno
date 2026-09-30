# Pill: every jump and flash on self-resize, with a ledger

**Status:** Fixed on `worktree-agent-a4ce369c316e4f574`; every transition traced frame by frame on the bench before and after; 380 frontend tests green; CI on the draft PR.
**DRI:** the Pill audit agent for the code and this ledger. Lacy is DRI for the hardware pass (real window timing, the glide into a well, a display hop, launch).
**Not a redesign.** The Pill is the default and its look is unchanged. This is the list of every moment the pill, its dot, its pane or its window moved when it should not have, what caused each, and what now prevents the whole class.

## The ask

Lacy, 2026-09-30: "That jumping behavior is pretty pervasive. We need to scan the entirety of the pill appearance transitions to fix all jumping flashes that happen when the window resizes itself."

## Jobs Standard critique of the Pill as of main `58f94df2`

```
For: someone who lives with the Pill all day; they hover it, type into it, talk to it, and it must never twitch.
Verdict: CUT (fix the classes of jumps, not the instances; nothing visual changes)
Remove:
- The 220ms shrink delay as a hiding place: it stays only as the wait for the pill's own animation, never to paper over a wrong anchor.
- Every value that describes the same frame twice (a cached anchor baseline, a growUp read back from the window, a `vcenter` flag set in an effect).
- Full's private pad and band (24 and 44 against 16 and 34 everywhere else).
Fails:
- 4 (design is how it works): the window is moved in one backend call while the pill moves in a CSS transition; any anchor that changes between layouts shows as a lurch.
- 4: the growth direction is decided after the resize that needs it, so the first pane at a bottom well opens the wrong way and the close teleports the bar.
- 6 (one object): the bar walks out of its well on every hover at an edge well; the monitor clamp and the centred growth disagree about where the bar lives.
- 5 (back of the fence): three comments in FloatingBar promise "the dot stays put" while the geometry moves it.
Ship as: one pad and band for every layout; both the window and the DOM pinned to the near edge and the docked column; direction from the well slot; window first on growth, pill first on shrink; explicit anchors, no cache; the launch placement gates the first resize.
Evidence needed: per-frame traces of every transition before and after, stills of the drift end states, one clip of a full turn, tests that pin the resize call sequence.
```

## How the audit was done

The bench at `/__bar-harness` renders the real `FloatingBar` on the fake Tauri layer with a simulated window. A probe (`__jtrace`) samples the simulated window, the pill, the status dot and the pane every animation frame after a trigger, in simulated-screen pixels. Two numbers tell the story of a transition: the largest single-frame step of the dot (a jump, as opposed to the ease-out glide whose first frame is about a quarter of the travel), and the dot's start and end positions (a drift when a round trip does not come back to the same place). Every transition below was traced at the top-right well (the default install position) and, where the direction matters, at the bottom-right well, before and after the fix.

## The ledger

Numbers are simulated-screen pixels on a 1440x900 display, scale 1. "Step" is the largest dot movement in one frame; "drift" is where the dot ends against where it started for a round trip.

| Transition | What jumped (before) | Cause | Fix | After |
|---|---|---|---|---|
| compact -> hover, top-right well | 30px leftward jump at frame 0, then a 46px glide; after leaving, the bar sits 30px left of its well | Centre-stable width growth ran the window past the screen edge; `clampToMonitor` shoved it back 30px in one frame, and the centre-stable shrink kept the shoved position | The window's docked edge is the anchor (`anchorX` from the well's column) and the pill is aligned to the same edge, so the growth goes inward and the clamp never fires | Step 24 (ease-out), right edge fixed at 1425, back to x=1337 exactly |
| hover -> compact | 46px glide ending 30px off the well | Same clamp, uncorrected on shrink | Same | Back to the well, 0 drift |
| compact -> full + pane, top-right | 173px jump at frame 0 and a 5px vertical twitch (dot y 70 -> 65 -> 70 over 200ms) | Clamp (173px overflow); full's pad snapping 16 -> 24 while the band animated 34 -> 44 under `justify-start` | One `BAR_PAD` and `BAR_BAND` for every layout; the band's near edge is the anchor in every frame | Step 96 at frame 2 (the first frame of a 363px ease-out), y constant, right edge fixed |
| pane close, top-right | Dot up 9.3px at frame 0, drifting to 13px, then back down 13px at frame 13 when the delayed shrink landed; ended 173px left of the well | The DOM switched to compact's pad and band (16 and 34) while the window held full's frame (24 and 44) for 220ms; the anchor itself moved 13px; `vcenter` flipped a render late | Constant pad and band; the vertical origin is `justify-start`/`justify-end` from the applied frame, no `vcenter` | y constant at 75 throughout, back to x=1337, 0 drift |
| pane open at a bottom well, first time after launch | Opened downward, clamped against the bottom; dot 852 -> 860 -> 855 | `growUp` was false at launch (never computed there) and recomputed asynchronously from the window's centre the first time the pane opened, a few frames after the resize that needed it | `growUp` (and `anchorX`) come from the well slot the bar is docked in, set at launch placement, on release after a drag and on a display hop, never read back from the window | Window top 809 -> 441 (upward), pill and dot y 847 throughout |
| pane close at a bottom well | The bar teleported 381px up the screen (dot 868 -> 487) and stayed there | The open resize was issued with `growUp: false`, the close with `growUp: true`, against a cached baseline recorded for the other direction | Explicit `from` anchors on every resize (the live frame plus a known offset), no cache; a direction flip is its own re-anchoring resize with the band's far edge as the pinned point | Back to 1337,809 exactly |
| composer growth, pane open (1 -> 3 lines) | Window jumped 350px down at frame 1 | Stale `growUp` baseline again (the bench had moved the bar; on hardware the same happens after any move with the pane open) | As above | Window top fixed at 37, dot moves 18px over 200ms (4.8px per frame), pane slides |
| composer shrink, pane open (3 -> 1) | Dot up 18px over 200ms, then back down 18px when the delayed shrink landed | The anchor was the pill's centre, which moves with the band; the DOM pinned the band's top | The anchor is the band's near edge, which does not move with the text | Dot moves 18px over 200ms, window top fixed |
| composer growth, no pane | Clean (centre-anchored, the pill opened around its middle) | | Now pinned at the top: the first line stays put and the pill grows downward, the same as with a pane | Dot 18px over 200ms, top fixed |
| compact -> listening / status / error, and back | Glide only, plus the clamp drift at edge wells (ended 102px off after listening at the top-right well) | Clamp | Docked-edge anchor | Step 54 to 64 at frame 2 to 4 (ease-out), 0 drift |
| listening -> transcribing -> working -> responding -> finishing | No movement | Voice and status share a width; no resize | | No movement |
| roster strip on / off | Same as pane open and close (full layout, extra height) | Same | Same | 0 drift, y constant |
| default -> input -> shrinking -> default; Escape; blur with empty text | Glide plus the clamp drift at edge wells | Clamp | Docked-edge anchor | 0 drift, y constant |
| error auto-clear, speaking, always listening | Same as status and voice | | | Clean |
| first paint after launch | The launch placement (3 backend round trips) and the first resize (5) raced: the resize read the placeholder position before the placement moved the window, then `set_bar_frame`d it back, after `show_bar_when_ready`; on a first launch with nothing saved the bar showed at the placeholder frame | Two writers of the same frame at mount | The resize controller waits for the placement (`placed`), which seeds the frame it starts from; no resize is issued at all after a successful placement | Not reproducible on the bench (the mock answers in order). Pinned by test: `set_bar_frame` once, `resizeWindowIfChanged` never, before the first interaction |
| large growth (compact -> full) on hardware | The pill animated 56 -> 419 wide inside an 88px window for the frames the backend took to apply the resize: clipped edges, then a pop | The DOM changed size in the same commit that asked for the room | Two-phase, as Island: on growth the window is resized first and the pill is drawn at the new frame once the backend has applied it; on shrink the pill animates first and the window follows after `SHRINK_DELAY_MS` | Not visible on the bench (the mock applies within a microtask). Pinned by test with a deferred resize promise |
| two resizes in flight | A second resize could read the frame before the first had landed and move the window from a position it no longer had | `useWindowSize` had no ordering | Resizes are serialised per window label | Structural; pinned by the hook's contract |
| Reduce Motion | The pill and band transitions ran anyway, and the shrink waited 220ms for an animation that should not exist | No `motion-reduce` guard on the transitions | `motion-reduce:transition-none` on the pill and band; the shrink delay is 0 under `useReducedMotion` | Bench with `prefers-reduced-motion: reduce`: transition `none`, every change lands in one frame, 0 drift, no second move |
| StrictMode (dev) double effects | Placement and show could run twice | | The launch effect's `cancelled` flag already covered it; pinned by test: one `set_bar_frame`, one `show_bar_when_ready` | |

## The one rule that removes the class

Every jump above was the window and the pill disagreeing for a frame. The window moves in one backend call; the pill moves in a CSS transition; React commits in between. There is no way to make the three atomic, so the geometry is arranged so that they never need to be:

1. **One pad and one band for every layout** (`BAR_PAD` 16, `BAR_BAND` 44). No layout change can move the anchor, so the near window edge never moves.
2. **Both the window and the DOM are pinned to the same edges**: the band's near edge (top, or bottom when the pane opens upward) and the docked column (left, centre or right, from the well). Growth only ever happens at the far edges, where the added room is transparent. The frame between the backend applying a resize and React committing the matching geometry shows nothing moving.
3. **The growth direction and the anchored column come from the well slot** (`dock`), set the moment the bar lands (launch placement, release after a drag, display hop). Nothing is read back from the window, so nothing can arrive late.
4. **Window first on growth, pill first on shrink** (`applied` frame vs `issuedRef`). Nothing is ever drawn past the window's edge.
5. **Every resize names its anchor in both frames** (`from`), so the baseline is the live frame plus a known offset. Nothing is cached between resizes, so a glide into a well, a display hop or the launch restore cannot leave a stale baseline.
6. **The launch placement gates the first resize.**

## What changed in `useWindowSize` (shared by every bar)

- `anchorY` now means "the offset of the pinned point from the near edge of the NEW frame". Unchanged for callers that pass nothing (top-anchored) or a constant.
- New `from?: { anchorY, growUp? }`: the same point in the frame the window has right now. Defaults to the new frame's own offsets, which is exactly what the old live-frame fallback did, so a caller that passes only `anchorY` behaves as before.
- New `anchorX?: "start" | "center" | "end"`: which horizontal edge stays put. Default `center`, as before.
- Resizes are serialised per window label; `resizeWindowIfChanged` resolves once the frame is applied.
- The size cache includes `growUp`, so a same-size resize that flips the direction goes through.
- Removed: `resetWindowAnchor` and the anchor cache (`lastAnchorByLabel`). Only the Pill ever used them. Island, the voice bar, the orbs and the avatar pass no `anchorY` and are unaffected.

## Behaviour that changed on purpose

- The compact window is 88x76, not 88x66: 5px more transparent margin above and below the 16px pill. The full window is 451x76, not 467x92.
- At a left or right well the pill grows away from the screen edge instead of around its centre. At a right well the dot therefore travels the whole width change (92px on hover, 363px into full) over the 200ms ease-out, where it used to travel half of it plus a clamp jump. The alternative, centred growth, is what walked the bar out of its well.
- Composer growth pins the first line: the pill grows downward (or upward at a bottom well) and the dot, centred in the pill, moves 9px per line over 200ms. It used to grow around its centre, moving the text instead of the dot.
- On growth the pill starts animating one backend round trip after the trigger, once the room exists.
- The bench's well buttons go through the bar's own drag-and-settle path, so the bar learns its well as it does on hardware.

## Evidence

Under `docs/frontend/screenshots/pill-resize/` (committed):

- `before-topright-after-one-hover.png` / `after-topright-after-one-hover.png`: the bar after one hover at the top-right well; before it sits 30px left of the well, after it is back in it.
- `before-bottomright-after-pane-open-close.png` / `after-bottomright-after-pane-open-close.png`: the bar after one pane open and close at the bottom-right well; before it has teleported to the middle of the screen, after it is back in the well.
- `posture-*.png`: default, listening, transcribing, agent_responding, speaking, error, input from the preview route.
- `card-demo.mp4` and `card-demo.png`: one full turn (question, streamed answer with a component, spoken text, finish) from the preview route.

The per-frame traces quoted in the ledger came from the probe on the bench; the top-right, bottom-right and composer runs are reproducible with `/__bar-harness` and the state buttons (hover, toggle chat pane, the wells, the input state plus typed text).

Tests: `src/components/__tests__/FloatingBar.test.tsx` ("FloatingBar resize sequence") pins the order and the anchors of the resize calls for launch, hover, a whole turn, a bottom well, a side swap with the pane open, composer growth, Reduce Motion and StrictMode; `src/hooks/__tests__/useWindowSize.test.ts` pins the anchor maths including the flip and the docked edge.

## Not verified here

The bench has no real window. It cannot show: the backend's actual resize latency (the frames a large growth used to be clipped for), the glide into a well on release and the `set_bar_position` persist, a display hop, the launch restore against a real saved position, the OS focus and the tracking-area leave under a resize, or Reduce Motion as macOS reports it (the bench emulates the media query). These need Lacy's hardware pass: hover and leave at each of the nine wells, open and close the pane at a top and a bottom well, type three lines with the pane open, drag with the pane open from the top half to the bottom half, and a cold launch with nothing saved.

## Follow-ups (not in this branch)

- Wells are computed for the window's footprint, so snapping with the pane open lands the tall window in the well and the pill somewhere else once the pane closes. Wells should be computed for the pill's footprint.
- At a right-hand well the dot rides the pill's left edge and travels the whole width change. Mirroring the pill's content order for right-docked wells (dot on the right) would keep the dot still. A design choice, not a jump; left as is because the look stays.
- The first mount of the flame border stalled the bench for about two frames (the first listening transition showed the pill mid-way through its ease-out on the first sampled frame). Worth a look on hardware; the canvas setup may be the cost.
