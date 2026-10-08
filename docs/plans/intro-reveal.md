# Intro reveal: how Juno first appears (spec)

**Status:** Shipped in #737 (setup end only). Every launch since the follow-up PR. Rust tests for the geometry, vitest for the uniform math, CI for fmt, clippy and cargo test.
**DRI:** Lacy for the hardware pass (first run on a real desk, dark window underneath, each well). Frontend Engineer for the look.
**Prototype:** https://claude.ai/artifact/ECo3zSUEx6MkLFkae2LmbT (the smoke on a mock desktop; three backgrounds, Reduce Motion, well bounds).

## The ask

Lacy, 2026-10-07: the pill just appears on the screen, black on black over dark windows, and you do not even see it pop up. You just hear it say "Hi, I'm Juno." Wanted: a really cool smoke animation as Juno fades in and says her introduction. Then: the smoke must never show the rectangular edge of its window, and it should adapt to whichever gravity well the bar is docked in.

## The ten seconds

Juno launches (or setup ends). Where the bar is about to be, smoke gathers out of nothing, light and dark in the same cloud, each billow lit from one side. The pill appears inside it. The smoke thins and breaks up. "Hi, I'm Juno. To talk to me, hold the globe key." The eye is already on her.

## What was removed

- The Pavel Dobryakov fluid simulation and its wrappers (`webgl-fluid`, `WebGL-Fluid-Enhanced`): 1,600 lines built around mouse splats, bloom and sunrays, neon dye. Cannot be choreographed; the look is what Juno's design rules call AI slop.
- Any tint on the smoke. It is grey, two greys: light and dark together, so it reads over any desktop without a theme check.
- The lit rim around the pill. On the real 56x16 pill it read as a solid white ring, jarring and out of place (Lacy, 2026-10-08). The smoke is the whole picture.
- The hole the smoke left for the bar. The shader hollowed out the rectangle the backend measured, and that rectangle is the Pill's hit footprint, 88x76 with its margin, not the 56x16 shape: an empty rounded box around the pill "like an invisible force field" (Lacy, 2026-10-08). Now nothing in the shader knows the bar's shape. The smoke is densest at the centre, the bar is drawn over it, and the last wisp goes at the centre.
- A declared window in `tauri.conf.json`. A declared window is built on every launch; this one is wanted once, so it is built for the occasion and closed.
- Nine per-well variants. The shape of the cloud is one parameter, the inward vector.
- A crossfade of the bar itself. The bar is a separate window and is shown with a plain `show()` as before; the smoke is dense around it at that moment, which is what hides the cut.
- Any word to the person when the reveal cannot run. The bar appears the plain way.

## How it works

```
the bar asks to be shown (show_bar_when_ready), or the macOS fallback
timer fires, or onboarding closes (restore_after_onboarding)
  └─ intro::show_bar_with_reveal
       ├─ bar already visible, or a reveal in flight: do nothing
       ├─ wait <= 400 ms for a steady look to report its drawn rectangles
            ├─ measure: bar window origin (points), the look's drawn rectangles
            │  (bar_hit_test::current_regions, the pill at rest), the display's
            │  work area
            ├─ place(): the intro window, 560x360, pushed from the pill toward
            │  the open screen, clamped onto the display; the pill's position
            │  inside it; the inward vector
            ├─ build the intro window hidden, click-through, always on top
            └─ spawn:
                 wait for intro_ready (<= 1.5 s)      the page asked for the plan
                 show the intro window                 t = 0
                 sleep BAR_AT_MS (1.0 s)
                 bar.show()                            greeting fires (it waits for this)
                 sleep DURATION_MS - BAR_AT_MS (1.6 s)
                 close the intro window
```

The frontend (`/intro`, `src/components/intro/IntroReveal.tsx`) asks for the plan on mount, and the answer is its starting gun. It draws one fragment shader through `ogl` (the orb's renderer) for `duration_ms` and stops. It decides nothing.

If the page never asks (no WebGL, a failed load), the bar is shown at 1.5 s and the window closes. If there is no bar, no display, no regions: the bar is shown plainly. The log says why at debug level.

## Two tones

The smoke does not read the theme, because the theme is no guide to what is behind the bar: a light window sits on a dark desktop and the other way round. Mid-sized noise blotches decide which patches of the cloud are near white and which near black, so both are in every frame side by side, and the haze sampled a little toward the upper left lights each billow from that side (`relief`). Over a dark window the light patches carry the picture; over a light one the dark patches do. One shader, no `prefers-color-scheme`.

## The inward vector

The bar docks in a 3x3 grid of wells per display. The smoke must never meet the screen edge, and the cloud is bigger than the gap between the pill and that edge. So the plan carries one vector: from the pill toward the open screen, computed from where the pill sits on its display (`inward_for`). Edge midpoints give a pure axis, corners a diagonal, the centre nothing.

The shader uses it twice. Smoke is faded to nothing on the edge-facing side of the pill (`side`), and the window's own edge fades are short on that side and long elsewhere. The window is placed so the pill sits 70 points from the edge-facing side horizontally and 40 vertically. Nothing else changes between wells: same timing, same density, same colour, one shader.

## The pill's shape

The Pill is a steady look: one window far bigger than the shape it draws. The window's centre is no guide to the pill. The look already reports its footprint to Rust for click-through (`set_bar_hit_regions`), and at rest that is the pill plus the margin around it (88x76 for a 56x16 pill), so the reveal reads the same table (`drawn_bounds`) and trusts its centre, which is the pill's centre. The shader uses nothing but that centre. With nothing reported, a 56x16 pill centred in the bar window is assumed (`fallback_pill`).

## Tweakables

Two blocks, each a handful of numbers with a comment per line.

| Where | What |
|---|---|
| `src-tauri/src/intro.rs` | `DURATION_MS` (2600), `BAR_AT_MS` (1000), `WINDOW_WIDTH` / `WINDOW_HEIGHT` (560x360), `PILL_INSET_X` / `PILL_INSET_Y` (70 / 40), `DEAD_ZONE` (0.15), `READY_WAIT` (1.5 s) |
| `src/components/intro/introModel.ts` `LOOK` | `reach` (0.55, how far the cloud spreads), `density` (0.85), `churn` (1, speed), `dark` and `light` (the two tones, near black and near white), `lightness` (0.5, how much of the cloud is light), `relief` (0.6, how strongly billows are lit from the upper left) |

Dev Tools has a **Replay intro** button (`replay_intro`) that hides the bar and runs the reveal on it again, so a change can be looked at without relaunching.

## Reduce Motion

Nothing drawn. The bar still arrives at 1.0 s and the greeting follows it. The bar still shows at 1.0 s and the greeting still follows it.

## Demo test

1. Ten seconds: above.
2. Removed: above.
3. One action: none. It is a picture; it takes no clicks (the window is click-through).
4. Defaults: no setting. Reduce Motion is read from the system.
5. States: no WebGL (empty window, bar shows at 1.5 s), no regions (fallback pill), no display (plain show), already running (plain show). Nothing is said to the person in any of them.
6. Feedback: the smoke starts the frame the window is shown.
7. Seams: the bar's own `show()` is a hard cut, hidden inside the densest smoke. The greeting's existing wait for the bar means the voice lands after the pill, as before.
8. Stage test: pending Lacy's hardware pass. The prototype passed the edge check on three backgrounds.
9. Evidence: prototype link above; a recording from a real first run goes in `docs/changelog/media/<PR>/`.
10. DRI: above.

## Not done, on purpose

- No setting to turn it off. It runs on every launch, and the Reduce Motion fallback is the quiet version.
- No sound of its own. The greeting is the sound.
- No per-appearance variants. Island, Orb and the rest get the same reveal around whatever they draw at rest.
