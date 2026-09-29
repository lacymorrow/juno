# Settings, triggers and pickers: what the best do

Reference for anyone touching Juno's Settings, onboarding, hotkey recorder or the bar appearance picker. Compiled 2026-09-29 from primary docs and research. Pair with the skills listed at the end. No em dashes, sentence case, outcome before mechanism.

## 1. How Wispr Flow, Superwhisper and Raycast assign triggers

**Wispr Flow**
- Actions are listed by outcome, not by key: Push to talk, Hands-free, Command Mode, Paste last transcript, Copy last transcript, Press Enter, Cancel. Mode names describe the gesture: "Push to talk (hold to dictate)", "Hands-free (press to start/stop)". https://docs.wisprflow.ai/articles/2612050838-supported-unsupported-keyboard-hotkey-shortcuts
- Recorder is press-to-record. Chords save on key release, mouse buttons on press. Confirmation reads "Shortcut saved". Up to four bindings per action via "+ Add another". Empty rows say "Click to add a shortcut". "Reset to default" says it has no undo.
- Validation rejects bare letters and numbers, platform-reserved combos, more than three keys, and combos with no modifier. Caps Lock and Escape are the only single-key exceptions.
- Defaults on Mac: Push to talk = Fn, Hands-free = Fn+Space. If no Apple Fn key is detected, onboarding switches to Ctrl+Opt variants.
- Double-tap is derived, not a setting: double-tapping the push-to-talk key toggles hands-free, and the editor shows a live hint built from the user's own key. https://wisprflow.ai/whats-new and https://docs.wisprflow.ai/articles/6391241694-use-flow-hands-free
- Hands-free is explained as a use case first: "Dictate without holding a key or button when writing longer messages or working with your hands occupied."
- No per-app hotkeys. Per-context tone lives on "Flow Styles". https://docs.wisprflow.ai/articles/2368263928-how-to-setup-flow-styles

**Superwhisper**
- Every shortcut row carries one behavior sentence. Toggle: "Press to start/stop recording." Push-to-Talk: "Press-and-hold to record, release to stop. A quick tap acts as toggle." Mouse: "quick click toggles, press-and-hold acts as push-to-talk." https://superwhisper.com/docs/get-started/settings-shortcuts
- Modes are the unit of configuration: model, prompt, language, and "Activate for apps". https://superwhisper.com/docs/modes/modes
- Users demanded direct per-mode shortcuts because cycling "requires conscious cognitive effort that interrupts thought chains". Shipped in v2.12. https://feedback.superwhisper.com/board/p/custom-key-shortcuts-for-different-modes-one-step-process
- Weakness to avoid: the active mode is only visible in the large recording window.

**Raycast**
- v0.51 shipped a visual recorder: keys render as key caps, a 1.5 s progress bar auto-saves a valid chord, Enter saves immediately, Backspace clears, an X removes. https://manual.raycast.com/command-aliases-and-hotkeys
- Conflicts: "Already used by Raycast" hard-blocks the launcher key. A command conflict shows the other command's name and icon in red and offers "Save the shortcut anyway. The conflicting command loses its hotkey."
- Recorder supports single-tap Fn, binds left and right modifiers separately, handles international keyboards. https://manual.raycast.com/settings
- Hyper Key shown with the ✦ glyph; Settings > Keyboard runs a diagnostic naming conflicts (other apps, Karabiner, Caps Lock mapping). https://manual.raycast.com/hyper-key

**Alfred** is the counterexample: "Alfred won't prevent you from configuring conflicting hotkeys ... It is your responsibility." https://www.alfredapp.com/help/workflows/triggers/hotkey/

## 2. Recorder and trigger-picker patterns that converged

- Record, never type. Every good app captures on keydown and saves on release.
- Conflict detection is table stakes. sindresorhus/KeyboardShortcuts warns when a chord is used by the system or the app's own menu. https://github.com/sindresorhus/KeyboardShortcuts
- Same library on defaults: "Users find it annoying when random apps steal their existing keyboard shortcuts. Instead, show a welcome screen on the first app launch that lets the user set the shortcut."
- Symbols over words on macOS: ⌘ ⌃ ⌥ ⇧ ⇪ and 🌐 for Fn. https://support.apple.com/en-us/102650
- Fn has no public hotkey API. Apple marked the modifier "reserved for system applications". Apps that bind it use event taps, which is why they need Input Monitoring or Accessibility. https://mjtsai.com/blog/2023/05/02/fn-key-reserved-for-system-applications/
- Apple's own Fn and Dictation UX is written as the gesture: "Press 🌐 key to", "Press Right Option Twice", "Press Either Command Twice", plus "Customize". https://support.apple.com/guide/mac-help/use-keyboard-function-keys-mchlp2596/mac
- The gesture vocabulary that converged: hold (push-to-talk), press (toggle), double-tap (hands-free), and "a quick tap acts as toggle" as the forgiving fallback on the hold key.
- Per-app behavior belongs on a mode or profile ("Activate for apps"), never on a hotkey.

## 3. Apple HIG for macOS Settings

- "People expect apps and games to just work, but they also appreciate having ways to customize the experience." Task-specific options belong next to the task, not in Settings. https://developer.apple.com/design/human-interface-guidelines/settings
- macOS settings are modeless and apply immediately. No Cancel, Save, Done or Apply. Close button only. https://zenn.dev/usagimaru/articles/b2a328775124ef?locale=en
- Ventura+ shape: sidebar of panes, grouped inset forms. In SwiftUI that is `Form { Section(header:footer:) }` with `.formStyle(.grouped)`. The Section footer is where Apple puts the one explanatory sentence. https://developer.apple.com/documentation/swiftui/formstyle/grouped
- "In general, don't replace a checkbox with a switch." Checkbox labels are sentence case and as specific as possible. https://developer.apple.com/design/human-interface-guidelines/toggles
- Label grammar in System Settings: verb first, outcome stated, sentence case. "Show in Menu Bar", "Show When Active", "Show menu bar background". https://support.apple.com/guide/mac-help/change-control-center-settings-mchlad96d366/mac
- ⌘, opens Settings for the front app. The menu item is "Settings…" since macOS 13.

## 4. Where help text goes

- NN/g: "Users shouldn't need to find a tooltip in order to complete their task." Tooltips are hard to discover and do not exist on touch. https://www.nngroup.com/articles/tooltip-guidelines/
- NN/g on (i) icons: constraints, rules and comparisons between options belong at the primary level, not behind an icon. https://www.nngroup.com/articles/info-tips-bad/
- Never put a label or help text in a placeholder. https://www.nngroup.com/articles/form-design-placeholders/
- Progressive disclosure: show a few important options, offer the rest on request, two levels max, and label the second level so it sets expectations. https://www.nngroup.com/articles/progressive-disclosure/
- The good products phrase rows as outcomes: "Press-and-hold to record, release to stop", "Paste last transcript", "Press Right Option Twice".
- Apple writing guidance: sentence case, be specific, say what the control affects. https://developer.apple.com/design/human-interface-guidelines/writing

## 5. Experience pickers instead of dropdowns

- Siri: voices are dots; tapping auditions the voice with a longer sentence; pace and expressivity sliders re-demo on change. https://www.macstories.net/stories/ios-and-ipados-27-review/5/
- ChatGPT: evocative names with a two-word descriptor ("Cove: Composed and direct", "Breeze: Animated and earnest"), preview before confirm. The web dropdown is the weaker version. https://help.openai.com/en/articles/20001274-chatgpt-voice
- Apple Watch Face Gallery: the face at the top changes as you customize, one "Add to Watch" action. https://support.apple.com/guide/watch/change-apple-watch-faces-apda6559ad78/watchos
- macOS Wallpaper: large live preview on top, thumbnails below, applies immediately, per-item options next to the name. https://support.apple.com/guide/mac-help/change-your-desktop-picture-mchlp3013/mac
- Arc: a color grid you drag a dot around and the whole window recolors live. Linear: a swatch row for Light / Dark / System / Custom. https://linear.app/docs/account-preferences
- What makes them work: preview reacts on selection, not on save; selection is instant and reversible; names are evocative with a short descriptor; it is a gallery or carousel, not a menu.

## 6. Micro-interactions

- Saffer: trigger, rules, feedback, loops and modes. Feedback is how the system communicates the rules. "The details are the differentiators." https://archive.uie.com/brainsparks/2014/08/01/dan-saffer-big-considerations-from-microinteractions/
- Apple HIG Motion: add motion purposefully, aim for brevity and precision in feedback, make motion optional. https://developer.apple.com/design/human-interface-guidelines/motion
- Kowalski: UI under 300 ms; buttons 100 to 160 ms, popovers 125 to 200 ms, dropdowns 150 to 250 ms; ease-out at 200 ms feels faster than ease-in at 200 ms; springs only for drag, interruptible gestures or "alive" elements, bounce 0.1 to 0.3; reduced motion means fewer and gentler animations, not zero. https://emilkowal.ski/ui/7-practical-animation-tips
- Freiberg: under 200 ms to feel immediate; scale dialogs from ~0.8, never 0; frequent low-novelty actions get no extra animation; theme switches do not transition; feedback sits next to its trigger. https://interfaces.rauno.me/
- Immediate apply is the macOS convention; the control's state equals the stored state at all times.

## 7. Onboarding for a menu-bar dictation app

- Wispr (Mac): sign in, microphone, Accessibility ("so Flow can insert dictated text"), shortcut choice, then mandatory practice demos where text appears live in a demo window. Each demo can be skipped. "Quitting mid-setup is safe." https://docs.wisprflow.ai/articles/3152211871-setup-guide
- Superwhisper: Microphone "to record your voice", Accessibility "to paste text into other apps", then "The defaults work well, and you can change everything later." First test: click a text field, press shortcut, speak, press again. https://superwhisper.com/docs/get-started/quickstart
- Raycast: first step is the launcher hotkey with a one-click "Replace Spotlight"; skipped steps come back via a Show Onboarding command. https://manual.raycast.com/quickstart
- Permission etiquette: one checkable sentence on what it is for, ask when the feature is first used, name which permission, open the exact System Settings pane. https://hop.tools/blog/why-mac-apps-ask-for-accessibility/
- Defect to avoid: a Continue that promises a hotkey the app cannot deliver until permission lands. Make step copy depend on real capability and poll so the hotkey activates without a restart. https://github.com/joaodavidsilva/brightboi/issues/32

## Principles distilled (apply directly)

1. Name each trigger by gesture and outcome in one sentence: "Hold to dictate, release to insert." Not "PTT mode."
2. Record chords, never type them. Save on release, show "Shortcut saved", Enter saves early, Backspace clears.
3. Render shortcuts as key caps with Apple symbols, never spelled words.
4. Detect conflicts live against the system, your own bindings and the app menu. Name the owner. Allow overwrite of your own, hard-block system ones.
5. Treat double-tap and quick-tap as derived behaviors of the hold key and show a live hint built from the user's own key.
6. Never document Fn as a default without a fallback; detect the keyboard and pre-fill a chord when there is no Globe key.
7. Every hotkey has Reset to default, and it says there is no undo.
8. Per-app behavior goes on a mode or profile, not on a hotkey.
9. Settings apply immediately. No Save, Apply, OK or Cancel anywhere.
10. Sidebar of panes, grouped inset forms, sentence case, verb-first labels.
11. The explanation goes in the group footer, one sentence, outcome not mechanism.
12. Never hide a constraint or anything task-critical behind an (i) tooltip.
13. Progressive disclosure: two levels max, and label the second level.
14. Prefer checkboxes to switches in dense forms; a switch only for a section-level on/off.
15. Replace a dropdown with a gallery whenever the option can be experienced: preview animates on selection, instant and reversible, evocative name plus a short descriptor.
16. Motion: 150 to 250 ms, ease-out for entrances, springs only for drag, no transitions on theme switch, respect Reduce Motion by keeping opacity and dropping movement.
17. Feedback next to its trigger. No toast across the screen for a toggle.
18. Onboarding order: hotkey, mic, Accessibility and Input Monitoring with one-line "what for", then a skippable "press your key and speak" demo with live text.
19. Onboarding copy tracks real capability. Never promise the hotkey before its permission is granted.
20. Say "You can change all of this later" once, then prove it: Settings reachable from the menu bar and ⌘,.

## Skills that encode this (installed 2026-09-29, user-level `~/.claude/skills/`)

| Skill | Use it for | Source |
|---|---|---|
| `impeccable` (v4.4: critique, polish, clarify, onboard, delight, harden, distill, animate) | design review, microcopy, first-run flows, empty and error states | pbakaus/impeccable |
| `apple-hig` | what belongs in Settings vs inline, shortcut and modifier rules, macOS conventions | justinwetch/HIGAgentSkills |
| `native-feel-cross-platform-desktop` | making a Tauri app stop feeling like a web page: settings window, Escape, focus rings, no page transitions | yetone/native-feel-skill |
| `animate`, `review-animations` | the "should this animate at all" gate, curves and durations, reduced motion | emilkowalski/skills |
| `make-interfaces-feel-better` | radius, optical alignment, press scale, icon stroke, tabular numbers | jakubkrehel/make-interfaces-feel-better |
| `ux-writing` | button, error, empty state, confirmation, tooltip and label rules; strips AI tells | humbleteam/ux-writing |

Gap not covered by any published skill: the shortcut recorder widget itself (capture, conflict naming, glyph order ⌃⌥⇧⌘, reset to default, "press keys to record" empty state). The principles above are the house rule until a skill is written.

Gaps in this research: Apple's HIG pages are JavaScript-rendered, so HIG wording comes from search excerpts and the usagimaru analysis. No primary screenshot of the ChatGPT desktop voice carousel was retrievable.
