# Human-forward Settings (spec)

**Status:** Slice 1 merged (PR #604, v0.8.6). Slice 2 merged (PR #606). Slice 3 is PR #611.
**DRI:** Frontend Engineer for slices 1 and 2. Founding Engineer for slice 3 (Rust).
**Reference:** `docs/design/settings-ux-reference.md` (research, principles, skills).

## The ask

Lacy, 2026-09-29: stop shipping "a toggle for every setting". Phrase settings as what the person experiences, not what the code does. Replace the bar appearance dropdown with a picker you can watch, the way you audition a Siri or ChatGPT voice. Assign triggers the way Wispr Flow does. Decide, per setting, whether help text sits beside the label, under the group, or behind an icon (answer: never behind an icon for anything that matters).

## Jobs Standard critique of Settings as of main `62b33d4b`

```
For: someone in their first week with Juno, pressing ⌘, to make dictation feel right.
Verdict: CUT
Remove:
- The "Bar appearance" Select and its seven implementation names ("ElevenLabs Orb", "React Orb", "Persona (AI Avatar)").
- The "Bar appearance updated" toast. The bar changing on screen is the feedback.
- The Appearance group's `advanced` gate. How the bar looks is the first thing a person wants to change.
- "Dictation Settings" as a group title and "Configure how dictation delivers text" as a footer. Both restate the controls.
- The em dash in the Triggers empty state.
- The purple Brain tile on AI Provider (banned by the no-AI-slop rule). Not in this slice; filed as a one-line follow-up.
Fails:
- 1: "Bar appearance" Select -> a live picker; the person sees each look listening, dictating and finishing before choosing.
- 2: 21 switches across the window -> every visible row rephrased as an outcome so a person can tell which ones matter; the count itself drops in slice 2.
- 4: appearance change gives a toast, not a visible result in the settings window -> the preview stage reacts, and the real bar changes at the same moment.
- 7: "React Orb" and "ElevenLabs Orb" on a keynote slide -> evocative names with a one-line descriptor.
- 10: no screenshot on any settings PR since #512 -> this one ships with the preview route captured for every appearance.
Ship as: slice 1 below.
Evidence needed: screenshots of `/__bar-preview` for all seven appearances, tsc and vitest output.
```

## Slice 1 (this branch): appearance picker + outcome-first copy

**Ten seconds.** General pane. Under "Appearance" there is a wide stage showing your bar. It sits idle, then listens, then types a sentence, then finishes, then rests. Beneath it: a name and one line. Arrows on either side. Press → and both the stage and your real bar switch to the next look. That is the whole interaction.

**Mechanism.** The stage is an iframe on the unlinked `/__bar-preview?appearance=<value>` route. That route installs the same fake Tauri layer the bar harness uses, so the real bar components render with no backend and no window side effects leaking into the settings window. A short looping script emits the same `bar-state-update` events Rust would. Reduced Motion holds one frame instead of looping.

**Primary action per screen.** Pick a look. Selection applies on browse (instant, reversible, no toast).

**States.** Loading: the name shows in the stage until the frame paints, then the preview fades in over 150 ms. Error: if the frame never paints, the stage shows "Preview unavailable" and the name and descriptor still let you choose. Empty: not applicable. Offline: not applicable, the preview is local.

**Copy (exact strings).**

| Where | Was | Now |
|---|---|---|
| General > Startup | Launch at login / Automatically start Juno when you log in to your computer | Open at login / Juno is in the menu bar as soon as you log in. |
| General > Sound | Sound effects / Play sounds for notifications and feedback | Play sounds / A soft cue when dictation starts and stops. |
| General > Appearance (group) | footer "Bar windows switch styles immediately when changed." | footer "Your bar changes as you browse. Every look goes through the same moments: resting, listening, dictating, done." |
| General > Appearance | Bar appearance / Which bar UI style to use in bar windows / Select | picker (name + descriptor under the stage) |
| General > Appearance | Follow cursor across displays / Keep the bar on whichever display your cursor is on, so you never hunt for it | Follow me across displays / The bar moves to whichever display your cursor is on. |
| General > Appearance | Show glowing border / Wrap the floating bar in a glowing border that lights up while Juno is listening or working | Glow while listening / The edge lights up while Juno hears you or works. |
| Voice > Dictation (group) | Dictation Settings / Configure how dictation delivers text | After you finish speaking / Juno puts the words where your cursor is. |
| Voice > Dictation | Text Insertion / Clipboard Paste, Clipboard-Free | How the words get typed / Paste them (most compatible), Type them (clipboard untouched) |
| Voice > Dictation | Copy to Clipboard / Leave the transcript on the clipboard after inserting | Keep a copy on the clipboard / Paste the last dictation again anywhere with ⌘V. |
| Voice > Dictation | Live transcription / Show words as you speak, in Juno's bar. Display-only — provisional text is never typed into the app; the final result is. | Show words as I speak / Provisional text appears in the bar. Only the final result is typed. |
| Triggers (labels) | Push-to-talk, Toggle, Voice / Agent, Dictation | Hold, Press, Say / to talk to Juno, to dictate |
| Triggers (hints) | Hold the binding while you speak or type. | Hold the key while you speak, let go to finish. |
| Triggers (footer) | Each way to summon Juno is one trigger. Add one of each method for the agent and for dictation. | Each row is one way to summon Juno. Mix keys, a mouse button and your voice however you like. |
| Triggers (empty) | A trigger is how you summon Juno — a hotkey, a mouse button, or your voice. | A trigger is how you summon Juno: a key, a mouse button, or your voice. |
| Triggers (recorder) | Press the key combination that summons this trigger. | Press the keys you want. They save when you let go. |

**Appearance names.** Names are what the person sees, values stay as they are in Rust.

| Value | Name | Descriptor |
|---|---|---|
| floating | Pill | A small pill that opens when you speak. The default. |
| app | Bar | A wide, warm bar that says what Juno is doing. |
| voice_ai | Studio | A light bar with its own waveform and transcript. |
| dynamic | Island | Grows and shrinks like the Dynamic Island. |
| orb | Orb | A blue orb that blooms as you speak. |
| react_orb | Halo | A soft ring that ripples as you speak. |
| persona | Avatar | A round avatar that shifts while Juno listens and speaks. |

Descriptors were checked against the captured previews (`docs/frontend/screenshots/settings-appearance-picker/`). "Bar" does not know the dictation states and shows "Ready" throughout; that is the component, not the preview.

**Found while building the preview, fixed in the same branch.**
- `BarHost` painted the default Pill before the bar config arrived, so every non-default window flashed the wrong look for a frame and ran the Pill's window-sizing effects against a component about to be replaced. It now renders nothing until the config is in.
- The Studio look (`voice_ai`) has thrown "Element type is invalid" in dev since Vite's pre-bundler started wrapping react-fast-marquee's CommonJS default as `{ default: Component }`. The import is unwrapped in `voice-ai-bar.tsx`.


## Slice 2: fewer switches, better defaults (frontend only)

**Status:** merged as PR #606.

- "Glow while listening" row removed. Only the Pill reads the setting, and it defaults on; the Rust setting stays for the harness and the bar config.
- Toggle toasts removed (sound, agent mode, performance monitoring, follow cursor). The switch flipping is the feedback.
- Advanced pane: "Show in Dock" and "Show system tray icon" become one control, "Show Juno in: Menu bar / Dock / Both". The state where Juno is nowhere is no longer expressible. The new place is turned on before the old one is turned off.
- AI Provider: the Save button is gone. The key, max tokens, temperature and system prompt save when the field loses focus (Enter also commits the key). The group footer reads "Saved." for a moment. The Claude CLI box is neutral chrome, not blue-on-blue.
- AI Provider tile: chip glyph on a light blue tile instead of the Brain on purple.
- Tools: category and per-tool switches are checkboxes (HIG: a dense list of options is a checkbox list). The Enable all / Disable all row stays.
- Settings search only returns rows the person can reach: rows flagged advanced, or in advanced sections, match only while the toggle is on.

Considered and cut: changing the theme's `--primary` to system blue (touches every window, not a settings change); the Notifications pane (one switch already).

## Slice 3: triggers the Wispr way (Rust + frontend)

**Status:** PR #611.

Backend (`dictation_monitor.rs`, `agent_monitor.rs`, `constants/agent.rs`, `triggers/mod.rs`):
- Hands-free on a hold key comes from a **double tap**, not a single tap. See [`docs/plans/trigger-gestures.md`](trigger-gestures.md) for the rule and phase 1. A short tap on a hold key cancels the session it opened; the gesture recognizer in `events/shortcuts.rs` promotes a second press inside `DOUBLE_TAP_WINDOW_MS` (300 ms) into the Press code path on its down edge and swallows the matching release. The single-tap hands-free from slice 3 was reverted in phase 1.
- The hold threshold is 400 ms (was 300). Under it is a tap.
- Row labels are sentences: "Hold to dictate", "Press to talk to Juno", "Say a phrase to dictate". Conflict messages quote them.

Frontend (`KeyCaps.tsx`, `ShortcutRecorder.tsx`, `TriggersSettings.tsx`):
- Bindings render as key caps with Apple glyphs in ⌃⌥⇧⌘ order. Fn is 🌐.
- The recorder records. It starts listening when it opens, shows the caps pressed while they are held, and saves when the main key is let go. Enter saves a chord already showing, Backspace clears it, Escape leaves the binding as it was. "Shortcut saved" appears inline for a moment. The old editor's manual text field, Save, Cancel and Capture buttons are gone.
- Conflicts show the backend's sentence in the row, naming the other trigger.
- "Reset to default" appears on the two rows that have a factory binding.
- The hold hint says what a tap does.

Considered and cut, each a follow-up if wanted: the onboarding tryout (a separate screen, its own slice), side-specific bare modifiers such as Right Option (needs the event tap work in the FluidVoice teardown, B2 and B3), keyboard detection to pre-fill a chord when no Globe key exists (needs IOKit), and "Use it anyway" on a conflict (the backend refuses duplicates, and letting one row silently take another's key is the trap Raycast warns about).

## What was considered and cut

- A separate "Advanced dictation timing" group. The thresholds above become defaults, not settings.
- Per-app hotkeys. Per-app behavior belongs on a mode, and Juno has no modes yet.
- A theme picker for the settings window itself. It follows the system.
- Storybook or a second preview mechanism. The harness already renders the real components; the preview route reuses it.
