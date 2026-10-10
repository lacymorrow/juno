# Bar drag audit: the stall, the crash, and the one direction

Date: 2026-10-07. Read-only audit of `origin/main` at v0.8.116 (#729 reverting #728 is in CI). DRI for the fix: whoever picks up the follow-up issue; this file is the spec.

## The one sentence

Keep the steady frame for state changes, and drag the shape, not the stage: at drag start the window becomes the pill's own footprint and the OS drags it, as it did before #719. `bar_drag.rs` and every form of `constrainFrameRect` hacking are deleted.

Why this and not a lift: the stall is AppKit doing exactly what it documents, the crash came from fighting tao's class hierarchy, and the pre-#719 pill-sized window reached every well with zero custom drag code. The only thing the steady frame has to protect is a state change, and no state changes during a drag.

## 1. Why the window stops at the menu bar

### The facts, with sources

1. **The bar's style mask and level.** tao builds a `decorations: false` window as `NSWindowStyleMaskBorderless | Resizable | Miniaturizable`, then strips `Resizable` because `tauri.conf.json` says `resizable: false` (`~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tao-0.35.3/src/platform_impl/macos/window.rs` lines 216 to 232). `alwaysOnTop: true` sets `NSFloatingWindowLevel` (3), and Juno re-applies 3 as the resting level (`src-tauri/src/bar_stacking.rs`, `BarStacking::Floating`). So: borderless, not resizable, level 3.

2. **AppKit constrains that window on every frame change.** The macOS 10.9 AppKit release notes, verbatim:

   > Prior to 10.9, the NSWindow method `-[NSWindow constrainFrameRect:toScreen:]` was invoked only for windows with NSTitledWindowMask set in their styleMask. In 10.9, this method is invoked for all windows. [...] windows are now constrained to not intersect the menu bar on their containing space. This restriction was already in place for titled windows, but it has been extended to borderless windows whose level is at least NSNormalWindowLevel but less than NSMainMenuWindowLevel. This behavior is implemented in `-[NSWindow constrainFrameRect:toScreen:]`. You may override that method in an NSWindow subclass to adjust or prevent this constraining.

   Level 3 sits inside `[NSNormalWindowLevel (0), NSMainMenuWindowLevel (24))`, so the bar is in the constrained band. Being borderless does not exempt it (that was the pre-10.9 rule, and it is what #727's module comment assumed).

3. **Every path Juno uses goes through that constraint.** The OS drag (`performWindowDragWithEvent:`, which is what `startDragging()` calls, tao `window.rs` line 957), `setFrameTopLeftPoint:` (#727's `tick`, and also tao's `set_outer_position`, so the glide in `animateWindowTo` too), and `setFrame:display:` (`set_bar_frame_atomic`). The constraint keeps the **window's** top edge below the menu bar. The steady window is 574 pt tall with the pill at its bottom when docked low, so the pill stops 574 pt minus the pill height below the menu bar: "about halfway up the screen" on a 900 pt display. That is the whole stall.

4. **#727 was built on a wrong premise and never observed working.** Its comment says "AppKit only constrains titled windows". The `[BarDrag] driving the bar drag` log line was only added in #728, so there is no evidence the driven drag ran in the #727 session (`~/Library/Logs/Juno/juno-2026-10-07.log` lines 90171 to 90344 contain nothing from the drag). Either way the outcome is the same: a level 3 window cannot be carried above the menu bar line by any `setFrame` variant.

### The three real ways to lift it, ranked

| Option | Works? | Cost | Verdict |
|---|---|---|---|
| A. Do not need it: drag a window that is the shape | Yes (pre-#719 did it) | A hidden layout swap at drag start, which #727 already pays | **Recommended** |
| B. Raise the bar to `NSMainMenuWindowLevel + 1` (25) for the drag only, via `bar_stacking` | Yes, per the 10.9 note (the band stops at 24); Boring Notch runs its panel at `.mainMenu + 3` | Bar draws over the menu bar and any system alert while the button is down; must be a `BarSituation` fact, not a bare `setLevel:` | Fallback if A is rejected |
| C. `class_addMethod(TaoWindow, constrainFrameRect:toScreen:)`, gated by a per-window flag, forwarding to `NSWindow` via `objc_msgSendSuper` with the superclass named statically | Yes, this is Electron's `enableLargerThanScreen` shape without the subclass | Runtime surgery on a third-party class; one more thing that can break on a tao bump | Not worth it when A exists |
| D. `objc2::define_class!` subclass | No: tao instantiates `TaoWindow` itself; Tauri gives no hook to pick the class | Fork tao | Cut |
| E. Pure borderless style mask | No: already borderless, and 10.9 made the constraint style-independent | None | Cut |

Electron's actual override, for the record (`shell/browser/ui/cocoa/electron_ns_window.mm`): it calls `[super constrainFrameRect:toScreen:]` first and only returns the raw rect when `enableLargerThanScreen` is set and the window has no frame. It is an override on a class Electron itself allocates, which is why it is safe there and was not here.

## 2. Why #728 crashed, so it is never repeated

Both reports (`~/Library/Logs/DiagnosticReports/juno-2026-10-07-203818.ips` and `-203824.ips`) are identical: `EXC_BAD_ACCESS KERN_INVALID_ADDRESS at 0x8`, thread 0, in `-[NSObject superclass] + 24`, called from an unsymbolicated juno frame that `-[NSApplication(NSEventRouting) sendEvent:]` called, which was itself called from a juno frame under `-[NSApplication _handleEvent:]`. Registers at the fault: `x0 = 0`, `x1 = @selector(_isKVOA)`, `x2 = NSKVOIsAutonotifying`.

Read with tao's source, that stack is:

- `_handleEvent:` -> **`TaoApp sendEvent:`** (tao `app.rs` line 23) -> `NSApplication sendEvent:` -> the window's `sendEvent:` -> **`TaoWindow send_event`** (tao `window.rs` line 423) -> `util::superclass(this)`.
- `util::superclass` is `msg_send![this, superclass]` (tao `util/mod.rs` line 108): tao finds the class to call `super` on **dynamically from the instance**, then does `msg_send![super(this, superclass), sendEvent:]`. `view.rs` `drawRect:` does the same.

Two things go wrong once the bar's isa is swapped to a runtime subclass, and the report shows the second:

1. **Plain subclass (`JunoUnconstrainedBar_TaoWindow`)**: `[this superclass]` now returns `TaoWindow`, so `objc_msgSendSuper` starting at `TaoWindow` finds `TaoWindow`'s own `sendEvent:` again. Unbounded recursion on the first event; stack overflow. This alone makes any isa-swizzle of a tao object fatal.

2. **What actually happened: the window was already a KVO subclass.** WebKit's `WKWindowVisibilityObserver` observes the window's `contentLayoutRect` and `titlebarAppearsTransparent` (`Source/WebKit/UIProcess/mac/WebViewImpl.mm`, `startObserving:`), so once the webview is in it the bar is an `NSKVONotifying_TaoWindow`. #728 subclassed *that*. KVO's notifying class overrides `-class` to return the original class by looking its own class up in Foundation's table; for a class Foundation did not make, that lookup returns nil. `-[NSObject superclass]` is `[self class]->superclass`, so it read offset 8 of nil: address `0x8`, with the KVO selectors still in `x1`/`x2`. First `sendEvent:` after the swizzle, segfault.

Rules that fall out of it:

- Never `object_setClass` a tao/wry object. tao resolves `super` from `[self superclass]` at runtime, so the hierarchy must stay exactly what tao built, KVO layer included.
- Never subclass a class whose name starts with `NSKVONotifying_`; Foundation owns that layer and its `-class` lies for a reason.
- If a method on a tao window ever must change, add it to tao's class (`class_addMethod` on `TaoWindow`, after checking it does not already implement the selector), and name the super class statically (`class!(NSWindow)`), never via `[self superclass]`.

## 3. Step back: is one huge window the right design?

Yes for state changes, no for the drag, and those are separable.

- **What the steady frame fixed is real.** Resizing the window per state asks AppKit and WebKit to agree on one frame across two processes; the jump is the frame or two where the old page is drawn in the new window. The plan file (`docs/plans/appearance-steady-frame.md`) documents five anchoring PRs that could not cross that seam. A fixed window with CSS growth inside it is also the standard overlay recipe for webview apps: Electron's documented click-through-with-forwarding pattern (`setIgnoreMouseEvents(true, { forward: true })` plus a hover region) is exactly what `bar_hit_test.rs` reimplements in Rust.
- **Native apps avoid the problem differently and it does not transfer.** Raycast and Little Arc resize native `NSPanel`s; AppKit lays out and resizes in one transaction, so there is no seam. Notch apps (Boring Notch, DynamicNotchKit) use a fixed, borderless, non-activating `NSPanel` at `.mainMenu + 3` and animate inside it, and they do not drag at all. Nothing comparable drags a window far larger than its visible shape.
- **The drag is the one moment the steady frame buys nothing.** No state changes while the button is down. The pre-#719 pill-sized window dragged into every well on every display with the OS drag and no Rust timer, because the window *was* the pill and the menu bar constraint then means "the pill's top stays below the menu bar", which is the correct behaviour and exactly where the top wells are (`topInset: 36`).
- **#727 already pays the swap.** `startBarDrag` calls `swapSteadyLayout(win.label, dragLayout(layout))`; `dragLayout` moves the anchor, so `sameDrawing` is false and the pill is hidden for two frames at drag start today. Shrinking the window in that same swap costs nothing extra the person can see.

So the direction: **the drag layout is the footprint**. Window size = the current footprint, anchor at (0, 0), OS drag, settle glides the small window, then the existing hidden swap restores the steady layout for the landing well. The cross-display concern that made #727 centre the shape in the big window disappears, because the window is the shape.

## 4. Other defects found

Ordered by consequence.

1. **`bar_hit_test.rs` can leave the bar permanently click-through.** The poll clones `REGIONS`, then calls `set_ignore_cursor_events(!inside)`. If `set_bar_hit_regions(None)` lands between those two steps (the Pill unmounting when the look changes), the command's `set_ignore_cursor_events(false)` runs first and the tick's `true` wins; the next tick sees `None` and `continue`s forever. Fix: after the write, re-check `ACTIVE`; if it dropped, write `false` once.
2. **`topInset` is a constant 36 but the menu bar is not.** It is 24 pt on a notch-less display, 37 to 38 pt on notch MacBooks, 0 when the menu bar auto-hides. Because the constraint from section 1 also applies to `set_bar_frame_atomic`, a top-well layout that asks for y = 36 on a 38 pt menu bar is silently applied at 38 and the anchor rect no longer sits on the well by 2 pt. Tauri 2.11 exposes `Monitor.workArea` (`node_modules/@tauri-apps/api/window.d.ts` line 60, `tauri::Monitor::work_area`), which is `NSScreen.visibleFrame`. Derive both `computeWells`' `topInset`/`margin` and `steadyLayout`'s from it.
3. **The drop overlay and the bar share level 3.** `SnapWellsOverlay.show()` orders the overlay front within the floating level after the bar is already there, so the 30 percent dim can sit on top of the dragged pill. Unverified on hardware; worth one look. If confirmed, order the bar above the overlays in Rust when the drag starts (`orderWindow:NSWindowAbove relativeTo:`), through `bar_stacking`.
4. **`AUTO_HIDE_MS = 8000` hides the overlay mid-drag.** A drag held longer than eight seconds loses its wells and `overlayShown` stays true on the bar side, so nothing re-shows them. Re-arm from the highlight poll while the button is down, or drop the timer and rely on `bar-drag-ended` plus the window `mouseup`.
5. **Every 16 ms tick blocks a tokio worker on four main-thread round trips.** `is_visible`, `cursor_position`, `primary_monitor`, `outer_position` and `scale_factor` each dispatch to the main thread and wait. It works, and `cursor_follow` has the same shape, but during a drag it competes with the 8 ms drag ticks and the glide. With the OS drag back, the 8 ms timer goes away and this stops mattering; if the poll is ever hot again, read `NSEvent.mouseLocation` on the main thread in one dispatch.
6. **Two `swapSteadyLayout`s can interleave.** `useBarDisplayFollow` checks `barSnapBusy()`, but a hop already in flight is not blocked by a drag that starts during it, and the `hidden` flag is a single boolean: the hop's `finally` unhides while the drag swap is mid-flight. One frame of the wrong layout. Fix: a module-level swap promise that the next swap awaits.
7. **`dragStarting` is settled but never cleared on the OS-drag fallback**, and `drivenDrag` is only cleared in `settleBarSnap`. Harmless today; both go away with the plan below.
8. **`desktop_points` is sound.** The three tao scales (monitor by its own factor, cursor by the primary's, window by its own display's) are undone correctly, with tests on both sides. Rounding to whole points at 1.5x can put a well one point off; acceptable.

## 5. Fix plan, file by file

One PR. The Pill is the only steady look, so nothing else moves.

### Rust

- **`src-tauri/src/platform/bar_drag.rs`**: delete. The driven drag, the 8 ms timer, `GENERATION`, `GRAB`, `is_dragging`, the `bar-drag-ended` event.
- **`src-tauri/src/commands/bar_position.rs`**: remove `bar_drag_follow` and `bar_drag_stop`; remove their `generate_handler!` entries in `src-tauri/src/lib.rs` (lines 801 to 802) and the constants in `src-tauri/src/constants/commands.rs` (`DRAG_FOLLOW`, `DRAG_STOP`) and `constants/events.rs` (`DRAG_ENDED`). `set_bar_frame` stays; it is the one call that resizes the window at the swap.
- **`src-tauri/src/platform/bar_hit_test.rs`**: drop the `bar_drag::is_dragging()` term. Fix defect 1: after `set_ignore_cursor_events(!inside)`, `if !ACTIVE.load() { let _ = window.set_ignore_cursor_events(false); last_inside = None; }`. During an OS drag the window is the footprint, so the page reports one region equal to the whole window and the cursor is inside it by construction.
- **`src-tauri/src/platform/mod.rs`**: remove the module line.

### TypeScript

- **`src/lib/steadyFrame.ts`**: `dragLayout(layout, footprint: Size)` returns `{ size: footprint, anchor: { x: 0, y: 0, ...footprint }, anchorX: "center", growUp: false, origin: screen position of the footprint }`, so the window is exactly what is drawn. `contentPlacement`, `contentRect`, `roomForContent` already work for any `size`. Add a test: for every well and every state footprint, `dragLayout` origin equals `toScreen(layout, contentRect(layout, footprint))`.
- **`src/hooks/useBarSnapWells.ts`**:
  - `startBarDrag`: for a steady look, `await swapSteadyLayout(label, dragLayout(layout, footprint), applySteadyFrame)` (hidden swap, as today), then `win.startDragging()`. tao synthesises the mouse-down when the current event is a drag (`window.rs` lines 930 to 957), so the two-frame delay is fine; pre-#719 started the OS drag from the same `mousemove` threshold. Drop `drivenDrag`, `dragStarting`, the `BAR_DRAG_FOLLOW` and `BAR_DRAG_STOP` invokes.
  - `armBarSnap`: `footprintGrab` is `grabOffset` minus the footprint's top-left inside the *pre-swap* layout; after the swap it is just the grab inside the window. Payload `windowWidth/Height` stay `spec.rest` so the wells and the ring are unchanged.
  - `settleSteady`: unchanged in shape. `animateWindowTo` glides the small window, `swapSteadyLayout(next, applySteadyFrame)` grows it back hidden.
  - Remove the `BAR_DRAG_ENDED` listener in `src/hooks/useDragWindow.ts`; the window `mouseup` (capture) and the click swallow were enough before #727.
- **`src/components/FloatingBar.tsx`**: expose the current footprint to `startBarDrag` (it is already computed for `set_bar_hit_regions`); while `steady.layout.size` equals the footprint, report the whole window as the hit region.
- **`src/lib/snapWells.ts` and `steadyFrame.ts`** (defect 2): take `topInset`/`margin` from `Monitor.workArea` where callers have monitors; keep the constants as the fallback for the harness.
- **Tests**: `src/hooks/__tests__/useBarSnapWells.test.ts` loses the driven-drag cases and gains: drag start swaps to the footprint window and calls `startDragging` once; a drop from a bottom well into a top well lands with the anchor on the well; `src/lib/__tests__/steadyFrame.test.ts` gets the `dragLayout` identity above.

### Docs

- **`docs/plans/appearance-steady-frame.md`**, "Wells, drag, display hop": replace the Rust-drag paragraph with the footprint-window drag, and add the two rules from section 2 under a "Never" heading.

### Hardware check (the demo test)

Dock low on the laptop, drag to every top well; dock on the external, drag to the laptop and back; drag with the pane open; hold a drag for ten seconds (defect 4); switch looks mid-session and confirm the bar still takes clicks (defect 1). Record it and put it in `docs/changelog/media/<PR>/`.

## What was considered and cut

- Lifting the constraint (options B and C) keeps a second drag implementation alive for a benefit the OS drag already gives.
- A full-screen overlay window per display (the plan file's cut alternative) still pays the straddling and memory costs for nothing the drag needs.
- Going back to the pre-#719 per-state resize reopens the jump the steady frame closed.

## Sources

- macOS 10.9 AppKit release notes, "constrainFrameRect:toScreen: now invoked for borderless windows" and "NSWindows constrained to not intersect the menu bar": https://gist.github.com/zwaldowski/8710fddc8b0b39d2c152
- tao 0.35.3 macOS backend: `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/tao-0.35.3/src/platform_impl/macos/{window.rs,app.rs,view.rs,util/mod.rs}`
- Electron `constrainFrameRect:toScreen:` override: https://github.com/electron/electron/blob/main/shell/browser/ui/cocoa/electron_ns_window.mm
- WebKit's KVO on the host window: https://github.com/WebKit/WebKit/blob/main/Source/WebKit/UIProcess/mac/WebViewImpl.mm (`WKWindowVisibilityObserver startObserving:`)
- Crash reports: `~/Library/Logs/DiagnosticReports/juno-2026-10-07-203818.ips`, `juno-2026-10-07-203824.ips` (arm64 slice UUID F532B81B matches `/Applications/Juno.app` 0.8.116; the binary is stripped, so the juno frames were read against tao's source and the register state)
- App log: `~/Library/Logs/Juno/juno-2026-10-07.log` lines 90171 to 90655
