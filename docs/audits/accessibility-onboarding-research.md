# Accessibility Onboarding Research

> Competitive analysis of guided permission onboarding flows in Codex Computer Use, Clicky.so, and Juno's current implementation.
> Date: 2026-05-14 | Issue: LAC-1400

---

## Executive Summary

Both Codex (via Sky acquisition) and Clicky have invested heavily in making the macOS permission gauntlet feel guided rather than abandoned. The core insight: **don't just tell the user to go to System Settings — show them, animate the transition, and poll for completion so the UI updates live.** Juno's current 6-step wizard is functional but static; adopting these patterns would meaningfully reduce onboarding drop-off.

---

## 1. Codex Computer Use (Sky)

Source: reverse-engineering of `SkyComputerUseService` binary, MacStories review, OpenAI developer docs.

### Permission Flow Architecture

- **Dedicated permissions window** — separate from chat, purpose-built for onboarding
- **Row registry** — each permission rendered as a trackable row with live status
- **Animated drag-to-settings transitions** — the app visually guides the user toward the correct System Settings pane rather than just opening a URL
- **Accessory windows** — helper windows that bridge the gap between the app and System Settings during each permission step
- **Progress state tracking** — monitors completion across permission steps

### Permission Order

1. **Screen Recording** — so the agent can see apps
2. **Accessibility** — so the agent can click, type, and navigate

This order is deliberate: Screen Recording is simpler to grant (just a checkbox) and immediately enables the agent to see, which provides value even before Accessibility is granted.

### Post-Onboarding Per-App Approval

During active tasks, Codex asks in-chat before controlling each specific app. Users can "Always allow" trusted apps. This is a separate layer from system permissions.

### Virtual Cursor (Operational, Not Onboarding)

- Wiggles while the model thinks
- Takes playful non-linear paths between actions
- Color derived from system wallpaper (dynamic theming)
- Virtual — doesn't steal the real cursor; user can keep working
- Multiple tasks can run with independent virtual cursors

### Key Takeaway

The onboarding is NOT in the chat window (contrary to initial claim). It's a polished standalone UI with animated transitions. The chat is used later for per-app approval. MacStories called it "the best I've ever seen in a third-party Mac app."

---

## 2. Clicky.so

Source: full codebase analysis of `github.com/farzaa/clicky` (Swift/SwiftUI, native macOS).

### Permission Flow Architecture

- **In-panel flow** — everything lives in a single floating `NSPanel` that drops down from the menu bar status item (320px wide)
- **No separate onboarding window** — the panel's content conditionally renders based on state
- **Permission polling at 1.5s intervals** — `refreshAllPermissions()` on a repeating timer, UI updates live as the user toggles permissions in System Settings
- **Smart system dialog deduplication** — tracks whether the native system prompt has been shown this launch; first tap triggers the dialog, subsequent taps go straight to System Settings

### Permission Order

Displayed in panel order:
1. **Microphone** — `AVCaptureDevice.requestAccess`
2. **Accessibility** — `AXIsProcessTrustedWithOptions`
3. **Screen Recording** — `CGRequestScreenCaptureAccess`
4. **Screen Content** — only appears after Screen Recording is granted (progressive disclosure)

All four required for `allPermissionsGranted`. No enforced order for first three, but Screen Content has a soft dependency on Screen Recording.

### State Machine (4 States)

1. **Permissions not granted** → intro text + grant buttons
2. **Permissions granted, no email** → email capture
3. **Email submitted, not onboarded** → "Start" button
4. **Onboarded** → hotkey instructions + model picker

### Animated Cursor Buddy (The Signature Feature)

The "cursor buddy" is a full-screen transparent overlay (`NSPanel` at `.screenSaver` level, `ignoresMouseEvents = true`) with:

- **Welcome sequence**: cursor fades in (2s), streams "hey! i'm clicky" character-by-character (30ms/char), then plays an onboarding video attached to the cursor with spring physics
- **Demo interaction at 40s**: takes a screenshot, sends to Claude to find something interesting on screen, then the blue triangle **flies to that element along a quadratic Bezier arc** with:
  - 60fps frame animation
  - Triangle rotation following curve tangent
  - Scale pulse (1.3x at midpoint)
  - Glow intensification during flight
  - Smoothstep (Hermite) easing
  - Duration proportional to distance (0.6s–1.4s)
- **Speech bubbles**: streamed character-by-character with scale-bounce entrance
- **User cursor tracking**: movement >100px during demo cancels the animation

### Overlay Architecture

Per-monitor `NSPanel` windows:
- Full-screen, transparent, borderless
- `ignoresMouseEvents = true` (complete click-through)
- `level = .screenSaver` (always on top)
- `canBecomeKey = false` (never steals focus)
- `hidesOnDeactivate = false` (persists when app loses focus)

### Click-Outside Protection

During onboarding, the click-outside-to-dismiss handler skips dismissal when:
- Permissions aren't all granted AND
- A system dialog has focus (`!NSApp.isActive`)

This prevents the panel from closing when the user clicks a macOS permission dialog.

### Key Takeaway

Clicky's onboarding is minimal for permissions (standard macOS dialogs + polling) but elaborate for the *experience* — the animated cursor buddy IS the tutorial. The cursor demonstrates what the app can do while you watch.

---

## 3. Juno (Current State)

### Permission Flow Architecture

- **Separate onboarding window** — 440x700, non-resizable, centered
- **6-step wizard**: Welcome → Magic Keys → Escape → AI Provider → Permissions → Ready
- **Permission cards** with status indicators for 4 permissions
- **2-second polling** while System Settings is open
- **Window focus re-check** on return to app

### Permission Order

Displayed in cards:
1. **Accessibility** (required) — `computer_use_ai_sdk::check_accessibility_permissions()`
2. **Screen Recording** (required) — `computer_use_ai_sdk::check_screen_recording_permission()`
3. **Microphone** (optional) — `tauri_plugin_voice_transcription::mic_permissions`
4. **Input Monitoring** (optional) — Native TCC query

Only Accessibility + Screen Recording required to proceed (`areRequiredPermissionsGranted()`).

### Existing Floating Windows

- `desktop-cursor-overlay` — 1x1 transparent alwaysOnTop window for agent cursor
- `floating-bar` — persistent status display during agent operation
- `floating-panel` — agent interaction panel

### What's Missing vs. Competitors

| Gap | Codex | Clicky | Juno |
|-----|-------|--------|------|
| Animated transition to System Settings | Yes (drag animation) | No | No |
| Live permission polling | Yes | Yes (1.5s) | Partial (2s, only while Settings open) |
| Progressive permission disclosure | Unknown | Yes (Screen Content after Screen Recording) | No |
| System dialog deduplication | Unknown | Yes (first-launch tracking) | No |
| Click-outside protection during dialogs | Unknown | Yes | No |
| Animated cursor walkthrough | No (separate from onboarding) | Yes (blue triangle buddy) | No |
| In-app experience demo during onboarding | Unknown | Yes (40s demo interaction) | No |
| Replay onboarding | Unknown | Yes ("Watch Again" button) | Yes (`restart_onboarding()`) |

---

## 4. Recommendations for Juno

### Tier 1: Quick Wins (Low Effort, High Impact)

1. **Faster permission polling** — Drop from 2s to 1s intervals, poll continuously (not just while Settings is open). Users currently return to the app and wait confused.

2. **System dialog deduplication** — Track whether the native dialog has been shown per permission per launch. First click → system dialog. Subsequent clicks → open System Settings directly.

3. **Click-outside protection** — Don't dismiss any onboarding UI while a system permission dialog is in focus.

4. **Permission order optimization** — Consider leading with Screen Recording (simpler to grant, immediate visual payoff) then Accessibility, matching Codex's order.

### Tier 2: Guided Flow (Medium Effort, High Impact)

5. **Animated transitions to System Settings** — When the user clicks "Grant", animate a visual cue (arrow, highlight, or mini-cursor) showing where to go in System Settings. Could use the existing `desktop-cursor-overlay` window infrastructure.

6. **Progressive disclosure** — Show permissions one at a time. After each is granted, reveal the next with a celebratory micro-animation. Reduces cognitive overwhelm.

7. **In-chat onboarding option** — Move permission steps into the main chat window as conversational prompts from the agent. "I need Screen Recording to see your screen. Click here and I'll walk you through it." This collapses onboarding and first-use into one flow.

### Tier 3: Signature Experience (High Effort, High Differentiation)

8. **Animated cursor walkthrough** — Use the existing `desktop-cursor-overlay` window to render an animated Juno cursor that:
   - Flies to the System Settings icon in the Dock
   - Navigates to Privacy & Security → the relevant pane
   - Highlights the checkbox the user needs to toggle
   - All using the Bezier arc animation pattern from Clicky

9. **Live demo during onboarding** — After permissions are granted, immediately demonstrate Juno's capabilities by having the agent perform a simple task on screen (open an app, move a window, etc.) using the same overlay cursor.

10. **Per-app approval layer** — After system permissions, add Codex-style in-chat approval when the agent first touches each application. Builds trust incrementally.

### Tier 4: Polish

11. **Virtual cursor theming** — Derive cursor color from the user's wallpaper or system accent color (Codex pattern).

12. **Cursor personality** — Add idle wiggle animation while the agent thinks, and non-linear movement paths during operation.

---

## 5. Implementation Notes

### Existing Infrastructure We Can Leverage

- **`desktop-cursor-overlay` window** — Already exists as a transparent always-on-top `NSPanel`. Currently 1x1; needs resizing to full-screen for walkthrough animations.
- **`floating-bar`** — Could display permission progress during onboarding instead of a separate window.
- **`mcp-server-os-level`** — Already has macOS accessibility APIs for element detection.
- **`computer_use_ai_sdk`** — Already has permission checking functions.
- **`native_permissions.rs`** — Has the request/check infrastructure; needs deduplication logic.

### Key Technical Challenges

1. **Cross-window animation** — Animating a cursor from the Juno onboarding window to System Settings requires either:
   - A full-screen transparent overlay (Clicky's approach)
   - AppleScript/AX automation to open and navigate System Settings programmatically
   
2. **System Settings navigation** — `x-apple.systempreferences:` URLs open the right pane but don't highlight the specific checkbox. Programmatic AX tree walking in System Settings is possible but fragile across macOS versions.

3. **Permission state detection** — Some permissions (Screen Recording on newer macOS versions) require app restart to take effect. The polling approach works for most but edge cases exist.

### Rough Effort Estimates

| Tier | Items | Estimate |
|------|-------|----------|
| Tier 1 | 4 quick wins | 1-2 days |
| Tier 2 | 3 guided flow items | 3-5 days |
| Tier 3 | 3 signature items | 1-2 weeks |
| Tier 4 | 2 polish items | 2-3 days |

---

## Sources

- Codex: [MacStories review](https://www.macstories.net/notes/openais-new-codex-app-has-the-best-computer-use-feature-ive-ever-tested/), [SkyComputerUseService binary analysis](https://github.com/vtomnet/codex-cua-tea/blob/main/SkyComputerUseService.md), [OpenAI Developer docs](https://developers.openai.com/codex/app/computer-use)
- Clicky: [GitHub repo](https://github.com/farzaa/clicky), full codebase analysis
- Juno: codebase analysis of `src/components/onboarding/`, `src-tauri/src/commands/permissions.rs`, `src-tauri/src/commands/onboarding.rs`
