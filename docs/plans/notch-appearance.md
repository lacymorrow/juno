# Notch appearance

The Notch lives in the MacBook's notch. At rest it is the notch: a black shape flush with the top of the display, a little wider than the camera housing, with the status dot in its right ear. Drag it to any other edge stop (the bottom, a side, a quarter point on a wide display) and it is the same tab growing out of that edge. Drag it into a corner and it becomes that corner of the screen peeled back like a page, with Juno on the black underneath. The only place it cannot go is the middle of the screen.

Investigation that came before this: `notch-bar-appearance-option.md` (LAC-3030).

## First ten seconds

1. The person picks Notch in Settings. The bar leaves wherever it was and lands in the notch. On a notched MacBook almost nothing changes: the notch is a few points wider and a small dot sits beside the camera.
2. They hold the talk shortcut. The notch drops down below the menu bar into a line with the dot (it swells with their voice) and their words.
3. Juno answers. The notch grows into the Island's card, hung under the menu bar strip, with the linger ring. When the ring runs out it shrinks back into the notch.
4. They drag it to the bottom-right corner. The wells show every edge stop and corner, and nothing in the middle. On release the shape glides into the corner and becomes a small peeled-back corner: a black triangle with the dot, and the page's light underside folded over it.
5. The next answer opens the peel into a card anchored to the corner, its inner corner still folded.

## What it reuses

Every state, word, control and timer is the Island's (`island/islandModel.ts`, `docs/plans/island-appearance.md`). The Notch is `IslandBar` with `frame="notch"`; only the body and the window differ:

| | Island | Notch |
|---|---|---|
| Body | `IslandShell`, rounded rect | `notch/NotchShell`: the notch (shoulders, round bottom) or a curl |
| Window | resized per posture | steady frame (`lib/steadyFrame.ts`), never resized per state |
| Wells | the inset grid with the centre | the grid's perimeter, flush: every edge stop and corner, no centre (`lib/notchWells.ts`) |
| Level | floating (3) | over the menu bar (27) while resting; steps aside like every look |

## Shapes

| Posture | Top notch | Bottom / side notch | In a corner |
|---|---|---|---|
| Resting (capsule) | the menu bar strip, 236 wide; dot in the right ear | a 24pt tab (236 long on the bottom, 72 on a side); dot centred | a 44pt peeled corner; dot in the black triangle |
| Hover, line, ear, status | the Island's size, hung below the menu bar strip | the Island's size, growing straight out of the edge | the Island's size on the black, fold strip on the inner side |
| Card | the Island's card, below the strip | the card, out of the edge | the card, anchored to the corner, inner corner folded 22pt |

Every notch has the hardware's concave shoulders where it meets its edge and round far corners. A side notch at mid-height grows upward from its resting tab, as every look docked in the lower half does.

Top corners cover the menu bar strip first, as the notch does, so words never sit under the clock or the Apple menu. Bottom corners start at the screen edge.

## Wells

`notchWells` gives each display the grid's perimeter: the corners (44 x 44 curls) and every edge stop (three per edge on a laptop, five along a wide display's long edges), all flush with the display's edges. The top centre is sized to the display's menu bar (37 or 38 points under a notch, 24 elsewhere). Nothing inside the perimeter is a well, so the Notch cannot be dropped mid-screen; a release anywhere goes to the nearest well, judged by centres because the wells are different sizes. The drop indicator draws the same wells; the top row hangs from the menu bar as tabs, because the overlay window sits under the menu bar.

## Above the menu bar

AppKit keeps any window below `NSMainMenuWindowLevel` out of the menu bar on every frame change (`appearance-steady-frame.md`, "Never"). The Notch therefore asks Rust (`set_bar_over_menu_bar`) to rest at `NSMainMenuWindowLevel + 3`, the level NotchNook, Boring Notch and Sentient use. `bar_stacking::BarStacking::bar_level` lifts only the resting intent: when a system prompt is expected or a Juno window is in front, the bar goes to the normal level exactly as every other look does. The look reports `false` when it unmounts. The window's class is never touched.

## Removed, and considered and cut

- No centre well, and no inner stops of a five-stop grid. A notch has to grow out of an edge.
- No new settings. Notch width and fold size are constants, not preferences.
- No notch geometry from `NSScreen.auxiliaryTopLeftArea`. The shape is wider than every current notch, and the menu bar height comes from the work area the bar already reads. Follow-up if a future notch is wider than 236 points.
- No 3D page-curl rendering (WebGL, perspective transforms). Two clip-paths and a gradient read as a peeled page and cost nothing at idle.
- No separate component. The Notch is the Island's component, so every Island fix reaches it.

## States the person sees

- Empty: the resting notch or corner, dot only.
- Loading/working: the Island's status line with the orbiting dot.
- Error: the Island's red words and shake.
- Offline/provider down: the dot's red, as everywhere.
- No notch (external display, older Mac): the same shape reads as a small black tab at the top centre.

## Owed: the hardware pass

Not verifiable in the preview, and Lacy's call on a CI build (`juno-build feat/notch-appearance`):

- The notch sits flush at y = 0 and covers the camera housing on a 14" and a 16" MacBook Pro.
- Menu bar menus and Control Center open over it (level 27 is below pop-up menus at 101).
- A drag from the notch to each corner and edge and back; a display hop to an external monitor.
- A bottom notch over the Dock: level 27 draws above the Dock (20), so a centred Dock is partly covered while it is open.
- Hot corners still fire under a corner curl (they are cursor-position based, so they should).
- Switching from Notch to another look puts the bar back at the floating level.

DRI: Lacy (hardware pass and merge).
