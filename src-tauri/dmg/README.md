# The Juno install window

What a stranger sees after double-clicking `Juno.dmg`: the Juno icon, an arrow,
the Applications folder. One action, nothing else in the window.

| File | Role |
|---|---|
| `background.svg` | Source of truth. Edit this. |
| `background@2x.png` | What the bundler reads. Generated — never hand-edit. |

Regenerate after editing the SVG:

```sh
scripts/build-dmg-background.sh     # needs Chrome; design-time only, never CI
scripts/verify-dmg.sh <built.dmg>   # asserts the result
```

## Geometry

`tauri.conf.json` holds the numbers but cannot hold comments, so they are
explained here. Change one and you must change the others to match; the SVG
repeats them in its header, and `verify-dmg.sh` fails if they drift.

| Setting | Value | Why |
|---|---|---|
| `windowSize` | 660 x 432 | **Outer** size, title bar included. |
| content area | 660 x 400 | 432 - 32. What the background fills. |
| `appPosition` | 180, 190 | Icon centre. 180pt from the left. |
| `applicationFolderPosition` | 480, 190 | Icon centre. 180pt from the right — symmetric. |
| `windowPosition` | 390, 220 | Fixed; see below. |
| icon size | 128pt | create-dmg's default. **Not configurable.** |

**The 32pt title bar is measured, not assumed.** On macOS 26 a disk-image
window with the toolbar hidden reports an outer frame of 660x432 and a content
area of 660x390 at `windowSize.height = 422` — a 32pt difference. So a 400pt
tall background needs `height = 432`. `verify-dmg.sh` asserts the background's
logical size equals `windowSize` minus that constant, so if a future macOS
changes the title bar you get a failed release instead of a strip of bare
window along the bottom edge.

**`windowPosition` is fixed, because "centred" cannot be expressed here.** The
position is baked into the disk image at build time, when the screen it will
open on is unknown. 390,220 is centred on a 1440x900 desktop and fits entirely
on the smallest Mac display still shipping (1280x800).

A position that does not fit is corrected rather than lost: Finder clamps the
window to the visible screen. On the 1024x768 virtual display in
`dmg-preview.yml` the window opens at `364, 220, 1024, 652` — shifted left by
26pt to stay on screen, intact. So the worst case of a fixed position is a
window that is not quite centred, never one with its edge cut off.

**Icon size cannot be set.** `DmgConfig` has no field for it and tauri-bundler
never passes `--icon-size`, so create-dmg's 128pt default applies. The ticket
asked for "larger icons"; 128pt is already the large end, and the only way to
change it would be patching the bundler. Dropped.

**The title is already hidden.** The bundler's AppleScript does
`set toolbar visible to false` and `set statusbar visible to false` on every
DMG. No configuration needed, and none exists.

## Two things that silently break this

Both are invisible when building locally, which is why the stock DMG shipped
for 200 releases.

**1. The layout is skipped on CI.** tauri-bundler appends `--skip-jenkins` to
`bundle_dmg.sh` whenever `CI=true`, and that flag skips the AppleScript that
applies the background and the icon positions. The build still succeeds and
still produces a `.dmg` — just a stock one. `release-tauri.yml` sets
`TAURI_BUNDLER_DMG_IGNORE_CI: "true"` to stop that.

**2. Gatekeeper rejects the disk image.** Tauri notarizes and staples the
`.app`, then wraps it in a `.dmg` it only *signs* — there is no notarize call
in the bundler's `dmg/mod.rs`. Gatekeeper judges the file the person actually
double-clicks, so a signed-but-unnotarized disk image is **rejected**, and the
notarized app inside is never reached. Every release up to v0.8.201 shipped
that way:

```
$ spctl -a -t open --context context:primary-signature Juno_0.8.200_universal.dmg
Juno_0.8.200_universal.dmg: rejected
source=Unnotarized Developer ID
```

`release-tauri.yml` now notarizes and staples the DMG after the build and
re-uploads it over the copy `tauri-action` attached.

## Design notes

**The arrow is Juno's thread.** The app icon is hand-felted and outlined in a
teal running stitch. The arrow is that thread carried on: it comes in from the
left edge, passes behind the Juno icon (Finder draws the icon over it), lifts
out in a shallow arc and is sewn across to Applications, finishing in a
stitched chevron. Each stitch casts a tight shadow and carries a thin sheen
line; without those two the dashes read as a generic dotted arrow. The thread
fades in from the left: direction of travel, stated without an animation we
cannot play. (LAC-7607, after Superwhisper's waveform-as-arrow. The idea is
borrowed, every asset is ours.)

**The field is light on purpose.** Finder draws disk-image icon labels in
black in *both* light and dark appearance, and that colour cannot be
configured; only label size and position live in the `.DS_Store`. A dark
background needs capsules painted behind each label to stay legible (this is
why Exodus's DMG art has two light pills in it). Superwhisper's dark DMG skips
the capsules, and its labels are close to invisible. A light field needs no
such device: the darkest point of the label band is `#CDCDED`, where black is
13.6:1. **Do not darken the lower half of this art without re-solving that
problem.**

**The palette is sampled from the app icon**, not invented: pink `#E8569B`
(top-left felt) and periwinkle `#7178E1` (top-right) as the two pools under the
icons, over a pastel diagonal of the same pair; thread `#2C7480`, the icon's
stitching deepened so it clears 3:1 on the field (3.5:1). A fine fractal-noise
fibre is multiplied in at 7% so the field reads as felt and not as a CSS
gradient.

**No words.** The volume is named "Juno", so Finder already puts that in the
title bar, and it labels both icons itself. An instruction line would need
translating and would say what the arrow already says.
