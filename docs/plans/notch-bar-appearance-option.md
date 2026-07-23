# Notch Bar — Investigation & Implementation Plan (LAC-3030)

**Status:** Investigation complete — recommended to implement.
**Scope:** A new appearance option that docks the floating bar where the MacBook notch is, extending the notch into a live AI status surface (reference: Sentient OS "Notch Magic").

## 1. Reference: how Sentient OS does it

Sentient OS is open source (Swift/SwiftUI) and documents its notch overlay in
[`Notch Magic.md`](https://github.com/Sentient-OS-Labs/sentient-os/blob/main/Sentient%20OS%20macOS/Documentation/Notch%20Magic/Notch%20Magic.md).
The recipe, distilled:

| Concern | Sentient OS approach |
|---|---|
| Window type | Non-activating `NSPanel`, borderless, transparent, no shadow |
| Level | `.mainMenu + 3` (above the menu bar) |
| Placement | **Fixed canvas** sized once to the largest state + slack (140pt H / 90pt V), pinned top-flush at the bezel. Never moved or resized during a morph — per-state window resizing makes the notch visibly detach from the bezel while animating. All state transitions are content-level (SwiftUI/CSS), not window-level. |
| Menu-bar overlap | `constrainFrameRect` overridden so the window may sit at the very top of the screen (macOS otherwise pushes windows below the menu bar) |
| Spaces | `collectionBehavior = [.canJoinAllSpaces, .stationary, .ignoresCycle, .fullScreenAuxiliary]`, **re-asserted on every reveal** — macOS silently drops `.canJoinAllSpaces` when a window is re-ordered |
| Notch geometry | `NSScreen.auxiliaryTopLeftArea` / `auxiliaryTopRightArea` → notch width/height; `+2pt` bottom cover because the reported height is slightly shallower than the real cutout. `baseWidth = max(notch.width, 200)`, `baseHeight = max(notch.height, 32)` |
| No-notch fallback | Renders a centered "pill" at the menu bar on notch-less displays — the feature degrades gracefully, it is not gated on hardware |
| Multi-display | Anchor logic: hotkey → main (menu-bar) display via `CGMainDisplayID` (not `NSScreen.main`); physical notch hover/click → built-in display. Re-placed on `didChangeScreenParameters` / `activeSpaceDidChange` / wake |
| Click-through | Toggle `ignoresMouseEvents` from a cursor-position poll: mouse events pass through everywhere except over the notch silhouette. (A `hitTest` override cannot do this — any non-transparent pixel captures the click first.) |
| Typing without focus steal | `makeKeyAndOrderFront` on the non-activating panel takes keystrokes without activating the app |
| Extra (optional) | SkyLight private API pins the panel into a window-server space so it doesn't slide during a 3-finger Spaces swipe (`.stationary` only covers Exposé). Best-effort with public fallback. |

Prior art with the same recipe: [Boring Notch](https://theboring.name/) (`OverlayPanelWindow.swift`, `extension+NSScreen.swift`), NotchNook, [AgentNotch](https://github.com/appgram/agentnotch).

## 2. Current state of Juno's floating bar

- **Window config** (`src-tauri/tauri.conf.json:46-59`): `floating-bar` is 419×92, borderless, transparent, always-on-top, no shadow, `focus: false`, **no x/y** — placement is OS default at creation.
- **NSWindow setup** (`src-tauri/src/platform/macos.rs:63-111`): `setLevel_(5)`, non-opaque, no shadow, and — conveniently — the **exact same collection behavior set Sentient uses** (`CanJoinAllSpaces | Stationary | IgnoresCycle | FullScreenAuxiliary`). Never focused programmatically (`activate_floating_bar_window` only calls `show()`).
- **Positioning:** the only programmatic positioning is `centerStableResize` (`src/hooks/useWindowSize.ts:22-39`), which keeps the horizontal center fixed while the bar expands/collapses. No persistence: `floating_bar_keys::POSITION`/`SIZE` (`src-tauri/src/constants/settings.rs:100-101`) are declared but never used.
- **Appearance system:** `bar_appearance` setting with 6 variants (`floating`, `app`, `voice_ai`, `dynamic`, `orb`, `persona`) — `src-tauri/src/constants/ui.rs:27-33`, switch-rendered by `src/components/bar/BarHost.tsx:71-95`, chosen in `GeneralSettings.tsx:288-320`.
- **Gaps:** zero notch/safe-area/NSScreen geometry code anywhere in the repo (repo-wide grep: no hits for `notch|safeArea|auxiliaryTop|visibleFrame`). No Rust code reads monitor geometry at all. The only monitor-geometry precedent is frontend (`DesktopCursorOverlay.tsx:521-575`, union-of-monitors via `availableMonitors`).
- **Useful precedents already in-tree:** dynamic objc class creation (`platform/macos.rs` mouse_tracking module, `NSTrackingArea` + delegate classes, emits `mouse-entered-window`/`mouse-left-window` — reusable for hover-to-expand), `macOSPrivateApi: true` already enabled, `cocoa`/`objc` crates already in `Cargo.toml`.

## 3. Feasibility in Tauri v2

Everything required is reachable from Rust via the existing `cocoa`/`objc` deps and `ns_window()`:

1. **Notch geometry** — `NSScreen.safeAreaInsets` (macOS 12+) and `auxiliaryTopLeftArea`/`auxiliaryTopRightArea` via `msg_send!`. Notch width = screen width − left aux width − right aux width; height = `safeAreaInsets.top` (with the +2pt cover fudge).
2. **Window level above menu bar** — `setLevel_(NSMainMenuWindowLevel + 3)` (= 27), same call pattern as the existing `setLevel_(5)`.
3. **Top-flush placement over the menu bar** — the real risk (see §5). Plain NSWindows are constrained below the menu bar by `constrainFrameRect:toScreen:`. Two mitigations:
   - [`tauri-nspanel`](https://github.com/ahkohd/tauri-nspanel) converts a Tauri window into an `NSPanel` subclass (panels are not constrained the same way, and it also unlocks `nonactivatingPanel` for focus-free typing later). This is the standard approach for Spotlight-style Tauri apps.
   - Or override `constrainFrameRect:` by dynamically subclassing/swizzling — the mouse_tracking module already creates dynamic objc classes, so there's an in-tree pattern.
4. **Fixed canvas** — size the window once (max expanded state + slack) and drive every visual state with CSS transitions inside the webview. This *bypasses* `centerStableResize` entirely, which also avoids fighting the resize path.
5. **Fallback + multi-display** — same as Sentient: no notch → centered pill under the menu bar; anchor to the built-in display's cutout, re-place on `didChangeScreenParameters` (Tauri exposes monitor-change via window events; worst case re-place on show).

## 4. Recommended shape of the feature

Add **`notch` as a 7th `bar_appearance` variant** rather than a separate position setting — it composes cleanly with the existing system:

- `bar_appearances::NOTCH` in `constants/ui.rs` + settings enum + `GeneralSettings` select item.
- New `setup_notch_bar_window` in `platform/macos.rs`: level 27, fixed canvas, top-flush centered on the notch (geometry from a new `get_notch_geometry()` helper), collection behaviors re-asserted on every show.
- New `NotchBar` component in `src/components/bar/`, rendered by `BarHost`: OLED-black shape flush to the top edge, notch-width when idle, expands downward/wider on hover or during agent activity (reuse the existing `mouse-entered-window` events for hover; reuse existing bar-state events for streaming status).
- Disable `useDragWindow`/`useWindowSize` resize logic in notch mode (position is computed, not dragged; window never resizes).
- Non-notch Macs and external displays get the centered-pill fallback — the option stays available everywhere.

**Phase 2 (optional, separate issue):** `tauri-nspanel` conversion for focus-free typing in the notch, and click-through outside the silhouette via an `ignoresMouseEvents` cursor poll (Rust-side timer, same as the existing tracking module).

## 5. Risks

| Risk | Severity | Mitigation |
|---|---|---|
| `constrainFrameRect` blocks top-flush placement of a plain NSWindow | **High — prototype first** | `tauri-nspanel`, or dynamic subclass override (in-tree precedent) |
| Window resize during expand animation detaches bar from bezel | Medium | Fixed canvas (Sentient's hard-won lesson §7a) — never resize per state |
| `.canJoinAllSpaces` silently dropped on re-order | Medium | Re-assert collection behavior on every show (Sentient lesson §3) |
| Notch metrics vary (aux-area height reads shallow) | Low | +2pt bottom cover |
| Screen-top clicks dying at boundary coords (`NSRect.contains` half-open, `Path.contains` excludes boundary) | Low (Phase 2) | Overhang hit rects by +2pt past the top edge |
| macOS < 12 lacks `safeAreaInsets` | Low | Fallback pill path handles it |

## 6. Effort

Medium. Roughly: Rust geometry + window setup ~150–250 lines in `platform/macos.rs` + one command; frontend `NotchBar` variant + BarHost/settings wiring; constrain-rect prototype is the only unknown (time-box: if a plain NSWindow can't sit top-flush, pull in `tauri-nspanel`). Estimate 2–4 engineer-days including QA on notch + notch-less hardware.

<!-- LAC-3035 guard verification: throwaway PR, close without merging -->
