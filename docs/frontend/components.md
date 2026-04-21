# Core Components

## Bar System

Juno's floating bar is rendered inside a dedicated Tauri window (`floating-bar`) and comes in multiple appearance variants, selected via `BarHost`. Each variant subscribes to backend `BAR_STATE_UPDATE` events and sends interactions via `invoke("ui_handle_interaction", ...)`.

### `BarHost.tsx`
Routes to the active bar appearance based on backend config (`ui_get_bar_config`). Listens for `BAR_CONFIG_CHANGED` events to hot-swap variants.

### `dynamic-bar.tsx` (Dynamic Island variant)
The primary bar variant. Uses a framer-motion spring animation (`DynamicIsland` component) to smoothly morph between size presets.

**Window resize strategy — fixed width, variable height:**
- The Tauri window width is constant (`371 + 48px shadow padding = 419px`). The island's visual width animates via CSS spring, not window resize. This avoids horizontal repositioning (which causes flicker because macOS can't atomically set position + size).
- Only height varies per state. The window is top-anchored (macOS default: top edge fixed, bottom grows/shrinks). The island is positioned at the top of its container so height changes happen below it — no visual jump.
- The `useWindowSize` hook's `centerStableResize` adjusts X position to keep the horizontal center stable, but with fixed width this is a no-op.
- macOS window shadow is disabled (`shadow: false` in `tauri.conf.json`) to prevent shadow bounds recalculation flicker during height transitions.

**Resize timing:**
- **Growing** (idle → input): Resize window first (make room), then animate island into the new space.
- **Shrinking** (input → idle): Animate island first (350ms spring settle), then shrink the window. This prevents the island from being clipped during animation.

**Key fix — `DynamicIsland.setSize` guard:**
The original Dynamic Island component blocked returning to a previous size (checked `previousSize !== newSize`). This prevented idle → expanded → idle cycling. Fixed to only check `newSize !== state.size`.

**OS-level focus/blur:**
Listens to Tauri's `onFocusChanged` window event (Cmd+Tab, clicking another app) and sends focus/blur interactions to the backend. This is separate from the `<input>` element's DOM focus events which only fire within the webview.

### `FloatingBar.tsx`
The original bar variant. Uses manual CSS sizing (`width`/`height` style props) instead of the Dynamic Island spring animation. Follows the same backend event/interaction pattern.

### `floating-bar.tsx`
Dynamic Island variant of the floating bar (alternative to `dynamic-bar.tsx`). Same backend integration, different visual treatment.

### `voice-ai-bar.tsx` / `voice-ai-bar-dark.tsx`
Voice-focused bar variants with audio level visualization.

### `useWindowSize` hook
Manages window resizing with center-stable horizontal positioning:
- Compares physical pixel coordinates (`outerPosition`/`outerSize`) to compute dx
- Adjusts X position by `-dx/2` so the window center stays fixed
- Fires `setPosition` and `setSize` via `Promise.all` to minimize the gap between operations
- Caches last-applied size per window label to skip redundant resizes

### `DynamicIsland` component (`ui/dynamic-island.tsx`)
Framer-motion spring animation container. Size presets define width, aspect ratio, and border radius. The `setSize` callback triggers spring transitions between presets. Key presets used by the bar: `default` (150×44, idle pill), `compact` (235×44), `long` (371×84, input), `medium` (371×210, agent response).

---

### `FloatingBar.tsx` (legacy)
The signature UI of Juno.
- **Design**: compact, pill-shaped window.
- **Props**: None (Self-contained, subscribed to `useAppState` and `VoiceContext`).
- **Variants**:
  - `Input`: Standard text input.
  - `Thinking`: Animated pulse loader.
  - `Voice`: Waveform visualizer.

### `PermissionsManager.tsx`
Handles the intricate macOS permission flow.
- **Detection**: Uses `setInterval` to poll backend permission checks.
- **UX**:
  1. Shows 'X' status.
  2. User clicks "Grant".
  3. App opens System Settings deep link.
  4. App waits for focus regain to re-check.

### `ChatContainer.tsx` & `ChatInput.tsx`
- **Virtualization**: Uses simple DOM node limiting (`useConversation` logic) rather than complex virtualization libraries (React Window) for simplicity, as chat sessions are standardly short.
- **Markdown**: Renders LLM output using `react-markdown` with custom code block highlighters.
