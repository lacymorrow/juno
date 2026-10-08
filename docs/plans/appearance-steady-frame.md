# The steady frame: bar appearances that never jump

Status: the Pill is done (branch `fix/pill-steady-frame`). Every other appearance still resizes its window per state and still jumps. This file explains why, what the Pill does now, and how to move each remaining look across, one PR per look.

## The symptom

Every bar appearance visibly jumped when it changed size: idle to listening, listening to answer, the pane opening. The jump went a different way depending on the well the bar was docked in. Docked right, it lurched left and back. Docked low, it lurched up and back. You could see it in the appearance chooser previews too.

## Root cause

A look that resizes its window per state asks two processes to agree on one display frame, and they don't.

1. The bar asks for a new frame (`useWindowSize` -> `set_bar_frame` -> one `NSWindow setFrame:`). AppKit applies it, and the WindowServer composites the window at its new origin and size on the next frame.
2. WebKit lays the page out again for the new viewport in its own process and commits the result a frame or two later.

In between, the old page is drawn inside the new window, pinned to the window's top-left. Anything pinned to the right edge (a right-hand well), the bottom edge (a bottom-half well) or the centre (a centre column) is drawn in the wrong place for those frames and then snaps back. Nothing pinned to the top-left moves. So the direction of the jump depended on the well.

No anchoring maths can fix this. `anchoredTop`, `anchoredLeft`, the `from` anchor, the two-phase grow/shrink and the atomic `setFrame` all made the window frame itself correct. The seam sits between AppKit and WebKit, and the only way through it is to never resize the window.

The preview had a second, simpler version of the same problem: `AppearancePreview` centred the fake window in the tile, so every resize re-centred it and the pill moved by half the size change.

## The pattern

Notch and island apps (NotchNook, Boring Notch, Alcove) and Wispr Flow's pill all solve this the same way:

- **One window per well, sized once for the largest state the look reaches.** It never resizes when the state changes.
- **The shape grows and shrinks in CSS inside it**, pinned to the docked edge: absolute offsets from the window's edges put the shape's docked corner on the well, and flex alignment keeps it flush with that edge while width and height animate.
- **The transparent rest of the window lets clicks through.** The page reports the rectangles it is drawing. Rust polls the cursor, flips `ignoresMouseEvents` as it crosses them, and emits enter and leave for them.

### Geometry (`src/lib/steadyFrame.ts`)

- A look declares a `SteadySpec`: `rest` (the resting footprint, which wells are computed for), `max` (the window size) and `stage` (what the preview frames).
- `steadyLayout(well, monitor, spec)` returns the window origin and the **anchor rect**, which is the resting footprint inside the window. The window extends away from the docked edges: rightward from a left well, leftward from a right well, evenly from a centre column, downward from a top-half well and upward from a bottom-half one. It is then clamped into the display's inset area. The clamp is absorbed by the anchor offset, never by the anchor itself, so **the resting shape is always exactly on the well**, the same place it sat before this change.
- `contentRect` / `contentPlacement` / `dockedEdges` are the rule the render follows and the tests pin.
- `roomForContent` tells a look how tall it may grow from the anchor. The Pill caps its pane to it, so a window clamped on a short display never clips.

### Click-through (`src-tauri/src/platform/bar_hit_test.rs`)

- Command `set_bar_hit_regions(regions | null)`: the page reports logical rects relative to the window's top-left. `null` means the mounted look is not steady. Click-through goes off and the native tracking area owns hover again.
- One poll thread at 16ms, its own OS thread rather than a tokio worker, because the window reads (`is_visible`, `outer_position`, `scale_factor`) each wait on the main thread. The cursor and the button come from Quartz (`CGEventGetLocation`, `CGEventSourceButtonState`), which answer from any thread. On a crossing it calls `set_ignore_cursor_events` and emits `mouse-entered-window` / `mouse-left-window`.
- A press that began on the content holds the window until release (`hit_state`), so a drag or a text selection never flips the bar to click-through mid-gesture.
- After every write the poll re-checks that a look still has regions. If the look unmounted between the read and the write, it writes `false` once, so a stale `true` can never leave the bar click-through for good.
- While regions are set, the whole-window tracking area in `platform/macos.rs` stays quiet for the bar (`bar_hit_test::owns_hover`). Mouse-moved forwarding still runs, because it only fires over content.
- Pure parts (`cursor_in_regions`, `hover_transition`) have Rust unit tests. The poller runs nothing until a steady look registers.

### Wells, drag, display hop (`src/hooks/useBarSnapWells.ts`)

- A steady look registers its spec in the steady store (`registerSteady`). Wells for it are computed for `spec.rest`, never for the window.
- Drag: drag the shape, not the stage. At drag start the window shrinks to exactly what the look is drawing (`dragLayout(layout, footprint)`; the look records its footprint with `setSteadyFootprint`), at the shape's own screen position, behind the same two-frame hide a drop uses. Then the OS drags it (`startDragging()`), as before #719. macOS keeps a floating window's top edge below the menu bar on every frame change, the OS drag included, and with the 574 point steady window that stopped a pill docked low halfway up the screen. With the window the size of the pill, that same rule holds the pill's own top just under the menu bar, which is where the top wells are. The window also changes display with the pill, because it is the pill. The drop overlay (one window per display) gets the resting footprint and the grab offset re-expressed from the anchor rect, so the ring and the landing agree. All of it is in global desktop points (`src/lib/desktopPoints.ts`).
- The page sees no mouse events during the OS drag, so a release it misses is caught by a release watch (`bar_pointer_held`, every 150ms while a drag is armed), and the overlay re-arms its 8s safety hide for as long as the button is held.
- The drop overlay shares the bar's floating level, and showing it orders it in front; it then asks Rust to order the bar back above it (`bar_order_above_snap_wells` -> `bar_stacking::raise_over_overlay`, only while the bar is floating).
- Settle: the small drag window glides, drawing unchanged, until the anchor sits on the nearest well. Then `swapSteadyLayout` grows it back to the steady layout for that well behind a two-frame hide, and fades the shape back in 150ms (instantly under Reduce Motion). The well itself never moves in that swap.
- Swaps on one window are serialized: `swapSteadyLayout` waits for the previous one, so a display hop in flight and a drag starting can no longer interleave and unhide each other mid-swap. `steadySwapsSettled(label)` resolves when every swap asked for has finished; the settle waits on it.
- Insets come from each display's work area (`Monitor.workArea`, which is `NSScreen.visibleFrame`), via `screenInsets` in `snapWells.ts`: the top row sits 12 points under that display's real menu bar (24 points, 37 to 38 on a notched MacBook, none when it auto-hides), and every other edge keeps 16 points beyond the Dock. A display with no work area (the harness) gets the old fixed 36/16.
- Display hop: the same swap, onto the same slot on the new display.

### Never

These come from the #728 crash and the drag audit (`docs/audits/bar-drag-audit-2026-10-07.md`, PR #730). Read them before touching the bar window natively.

- **Never `object_setClass` a tao or wry object.** tao resolves `super` from `[self superclass]` at runtime (`TaoWindow send_event`, `drawRect:`), so any subclass swapped in under it recurses or crashes on the first event. The hierarchy must stay exactly what tao built.
- **Never subclass a class whose name starts with `NSKVONotifying_`.** WebKit observes the host window, so the bar is already a KVO subclass; that class's `-class` lies on purpose and a subclass Foundation did not make reads nil (the #728 `EXC_BAD_ACCESS at 0x8`).
- **The menu bar constraint applies to borderless windows.** Since 10.9 `constrainFrameRect:toScreen:` keeps any window at a level from `NSNormalWindowLevel` up to (not including) `NSMainMenuWindowLevel` from intersecting the menu bar, on the OS drag, `setFrame:` and `setFrameTopLeftPoint:` alike. Being borderless does not exempt the bar (level 3). Size the window to what must reach the top instead of fighting the constraint.
- If a method on a tao window ever must change, add it to tao's class with `class_addMethod` after checking the selector is not already implemented, and name the super class statically (`class!(NSWindow)`), never via `[self superclass]`.

### The one trade-off

A drop into a well on the other side, or a display hop, cannot move the frame and the drawing in the same display frame either. That swap is hidden rather than shown. It happens only on a drop or a hop, never on a state change. The alternative was to keep the webview fixed and move it natively inside the window in the same AppKit transaction as the frame change, which would be seamless. It means repositioning WKWebView inside Tauri's content view, which wry may fight on resize. That is a possible follow-up, not a prerequisite.

### Alternatives considered and cut

- **A full-screen overlay window per display** (pill offset is just its screen position, no swap ever). Cut: the OS window drag would drag the whole overlay, so cross-display drag would need a rewrite of the shared drag, and a display-sized transparent WKWebView costs backing-store memory for nothing.
- **Better anchoring of per-state resizes.** Cut: this is what the last five PRs did, and it cannot cross the AppKit/WebKit seam.
- **Hiding the resize behind an alpha dip on every state change.** Cut: it turns a jump into a blink, many times per turn.

## What changed for the Pill

- `FloatingBar` registers `PILL_STEADY_SPEC`: rest 88x76 (the compact pill plus pad), max 452x574 (full pill with the composer at its limit, roster and pane, plus one pixel so a centre well sits on whole pixels), stage 452x76.
- The launch placement sets the steady frame once. The resize controller (`issuedRef`, `applied`, `growsFrom`, `resizeConfigFor`, `SHRINK_DELAY_MS`) is gone. Every state draws straight from the current frame.
- The column is absolutely positioned at the docked edges and reports its footprint (`pillFootprint`, which is exactly the window the Pill used to resize to) to the hit test. The hover margin is the same 16px pad as before.
- The leave check (`cursorInsideContent`) tests the cursor against the drawn footprint, not the window.
- The pane gives up height only where a clamped window leaves too little room (a middle well on a short display). There it gives up exactly what the composer or roster is using, so its far edge holds still.
- The expand control ("Open chat") is now in every state while the chat is closed (voice, status, working, driving, error), ahead of every stop control. The voice and status pills widen by one button for it, inside the steady frame.
- `useWindowSize`'s no-op cache carries the steady epoch, so the next look to mount after the Pill never skips its first resize.
- Preview: the harness answers `get_bar_position`. The Pill preview docks in a centre well, and the preview frames the steady `stage` strip instead of the window, so the pill grows from the middle of the tile and never re-centres.

## Tests that pin it

- `src/lib/__tests__/steadyFrame.test.ts`: for every well on five desks (Retina laptop, displays left of and above the main one with negative coordinates, 1.5x scaled, ultrawide with five stops, a short 1280x720), and every frame the Pill can draw (each layout, pane, roster, composer growth, extra control), the docked edge's screen coordinate is identical. The resting footprint is exactly on the well, nothing draws past the window, the window stays inside the display's inset area, the CSS placement uses the pinned edges, and the swap hides only when the drawing changes.
- `src/components/__tests__/FloatingBar.test.tsx`: one `set_bar_frame` at launch and none through a whole turn, the docked corner of the reported footprint is constant across a turn, a bottom-well drop swaps once and opens the pane upward, hit regions are cleared on unmount, and the expand control shows up while working with the chat closed.
- `src-tauri/src/platform/bar_hit_test.rs`: logical and physical units, Retina, negative coordinates, half-open edges, enter and leave only on a change.

## Checklist: moving another appearance onto the steady frame

One PR per look. For each:

1. **Find its largest footprint.** List every state the look draws and the size of each, including panes, cards, captions and subtitles. `max` is the largest width and the largest height. Keep `max.width - rest.width` even.
2. **Pick `rest`.** The footprint at idle, padding included. Wells are computed for it. Check that the idle look lands where it does today: the test `anchorScreenOrigin(layout) == well` is the guard.
3. **Register the spec** in a `useLayoutEffect` (`registerSteady(label, SPEC)`). On cleanup, unregister and `invoke(BAR_SET_BAR_HIT_REGIONS, { regions: null })`.
4. **Place once.** In the look's launch placement, use `steadyWells` + `steadyLayout` + `applySteadyFrame` + `setSteadyLayout` + `setDockSlot`, as `FloatingBar` does. Remove every `useWindowSize` / `resizeWindowIfChanged` call from the look.
5. **Render from the layout.** The root is the full transparent window. One absolutely positioned column uses `contentPlacement(layout)` for its offsets, aligns its children by `layout.anchorX` and stacks by `layout.growUp`. Hide it with `opacity-0` while `getSteady(label).hidden`. Animate size with CSS transitions (ease-out, no springs, `motion-reduce:transition-none`).
6. **Report what is drawn.** Compute the footprint per state as a pure function and send `contentRect(layout, footprint)` to `set_bar_hit_regions` whenever it changes. Use several rects if the look draws disjoint shapes (an orb plus a separate caption).
7. **Verify leaves against the footprint**, not the window (copy `cursorInsideContent`).
8. **Cap any tall part to `roomForContent(layout)`** if the look has one.
9. **Tests:** add the look to a geometry test like `steadyFrame.test.ts` (every well x every state, docked edge identical, nothing past the window). In the look's component test, assert one `set_bar_frame` at mount and none across a state cycle.
10. **Preview:** give the spec a `stage`. The preview frames it automatically once the look registers.
11. **Hardware:** dock in every well (corners, edge midpoints, centre, a second display), cycle idle, listen, think, respond and chat, and watch the docked edge.

### Remaining appearances

| Look | Setting value | Component | Resizes via | Notes |
|---|---|---|---|---|
| Island | `dynamic` | `src/components/bar/island/IslandBar.tsx` | `useWindowSize` | Grows to hold the answer card; `max` is the card's expanded size. Closest to the Pill, so do it next. |
| Avatar | `persona` | `src/components/bar/persona-bar.tsx` | `useWindowSize` | Decides its own growth direction from where the head faces (passes `anchorX`/`growUp`). Map that to the slot, or give it its own `anchorX` in the layout. |
| Orb | `shader_orb` | `src/components/bar/shader-orb-bar.tsx` | `useWindowSize` | WebGL canvas: keep the canvas a fixed size inside the frame. Words appear only when Juno needs you, which may be a separate rect. |
| Presence | `orb` | `src/components/bar/elevenlabs-orb-bar.tsx` | `useWindowSize` | Subtitles beneath the orb. Two rects, orb and subtitles. |
| Halo | `react_orb` | `src/components/bar/react-orb-bar.tsx`, `halo/HaloBar.tsx` | `useWindowSize` | Ring plus step ring; check the approval card's size for `max`. |
| Bar | `app` (hidden) | `src/components/bar/app-bar.tsx` | `useWindowSize` | Wide strip, hidden from the picker. Lowest priority. |
| Studio | `voice_ai` (hidden) | `src/components/bar/voice-ai-bar.tsx` | `useWindowSize` | Hidden. Teleprompter and script panes make `max` large; check that it fits a 1280x720 display. |

When the last look moves across, delete the per-state anchoring in `useWindowSize` (`anchoredTop`, `anchoredLeft`, `from`, `clampToMonitor`, `withDockDefaults`) and `set_bar_frame`'s delta maths. Nothing will need them.
