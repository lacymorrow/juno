# Capturing Juno

One capture does three jobs: it is the evidence on the pull request, it is what
Lacy watches to approve the feature, and it is the picture in the changelog and
the docs. So it is shot once, at the standard the public pages need, and never
re-shot for a second audience.

This is the standard for that capture. It covers screenshots and short silent
clips of the product. Narrated demos are a different thing and stay with the
demo rig's director (`~/repo/marketing-videos/tools/juno-demo-rig/`).

## Who, when, from what

- **QA captures, at the QA stage.** An engineer can attach a rough screenshot
  while building, but the QA capture is the one that counts and the one that
  is committed.
- **From the CI build of the PR's head commit**, installed with
  `scripts/juno-build.sh <branch>`. Never from a dev build: it has a different
  bundle identifier and different permissions, so it can look and behave
  differently.
- **On the recording Mac (graphite), on its main display only.** It has the
  staged desktop and the privacy checks. Nobody's working desktop is ever
  recorded.

A feature a person can see does not pass QA without its capture. Backend, CI,
docs and dependency changes need none.

## What to shoot

Only what this PR changed. Never an unchanged screen.

| The change is | Shoot |
|---|---|
| A new or changed screen, panel or row | One still of it, in light and in dark |
| Something that happens (a gesture, a flow, a state change) | One clip of it happening, plus one still of the result |
| An empty state or an error a person can reach | One still of each |

Two to four files is normal. More than six means the PR is doing too much or
the capture is wandering.

## The stage

- The rig's staged desktop: its plain wallpaper, the staged folder
  (`~/Desktop/@`), nothing else open. Only apps on the rig's allow-list may be
  on screen.
- **Nothing private, ever.** No real conversations, client names, email
  addresses, file names from a real folder, keys or tokens. An account name
  that has to be visible is a fixture or a public handle. If something private
  is in frame, re-shoot. Do not blur or crop it out.
- Juno at its defaults: default appearance, default window sizes, default
  accent. Change a setting only when the setting is the feature.
- English, 100% text size, no accessibility zoom.

## Stills

- PNG, at the display's native 2x.
- The window alone, without its shadow, when the change lives in one window.
  When it lives on the desktop (the bar, the Island, the cursor), a region with
  even margin around the thing, the same margin on every side.
- The state a person would land on. No hover highlight, text cursor or tooltip
  unless that is the feature.
- Light and dark, same framing, same state.

## Clips

- MP4 (H.264), 30 fps, no audio unless the feature is sound. Under 20 seconds
  and under 5 MB.
- **One action per clip.** If it needs a second action, it is a second clip.
- **Frame 0 shows the feature at rest.** A paused clip is its own thumbnail,
  so it never opens on a blank, white or half-drawn frame. Start recording
  after the window has painted.
- Hold the result for one second before stopping, so the last frame reads.
- Pointer visible, moving at the speed of a hand. No corrections, no typos, no
  waiting on a spinner longer than the feature really takes.
- Same framing rules as stills: the window, or an even-margin region.

## Names and place

`docs/changelog/media/<PR number>/`, lower-case, hyphens, what it shows then
its state:

```
providers-advanced-off.png
providers-advanced-off-dark.png
shortcut-sentence-change-gesture.mp4
shortcut-sentence-empty.png
```

The changelog on junebug.ai and the social posts read from this folder, so a
name has to make sense to someone who never saw the PR.

## Where it goes

1. **The PR description**: every file embedded, stills as a light and dark
   pair.
2. **Lacy's approval card**: the same files attached, clip first. The card is
   answered by watching, so the clip has to show the whole claim.
3. **The docs**: if a page already describes this feature (a use-case page, a
   docs page, the README), replace its picture or add this one in the same
   change, and say which page on the card. If no page does, write
   "Docs: no page" on the card. Do not invent a page to hold a picture.

## Before it passes

- Frame 0 of every clip is the feature, not a blank.
- Every still exists in light and dark with matching framing.
- Nothing private in any frame. Check the whole frame, not the middle.
- Sizes and names match this page.
- The capture shows what the PR says it does. If the build does something
  else, that is a defect, not a capture problem.
