# CLAUDE.md - Frontend

This file provides guidance to Claude Code when working with the React/TypeScript frontend in this repository.

## CRITICAL: The Frontend is a Display Layer ONLY

The TypeScript frontend renders backend state and sends user interactions to Rust via `invoke()`. It contains **zero business logic**.

### What the frontend DOES
- Renders chat messages, components, and UI elements
- Listens for Tauri events and updates the display
- Sends user actions (clicks, text input, toggles) to the backend via `invoke()`
- Manages purely visual state (modals, scroll position, animations, hover effects)

### What the frontend NEVER does
- Record audio or access the microphone (`getUserMedia` is banned)
- Play audio or use `Web Audio API` / `AudioContext` (TTS is in Rust)
- Register keyboard shortcuts (global hotkeys are in Rust)
- Open WebSocket connections (`@tauri-apps/plugin-websocket` import causes build failure)
- Execute shell commands or file operations
- Make HTTP requests to AI providers
- Store persistent data in `localStorage` (use Tauri Store via backend)

### Third-party library rule
Web-oriented libraries (e.g., ElevenLabs React SDK) are only used for their **rendering/layout** components. Any component that accesses browser APIs (`getUserMedia`, `AudioContext`, `WebSocket`, `navigator.mediaDevices`) is off-limits. The native Rust backend handles all I/O.

### The pattern
```
Backend emits event → Frontend renders
User clicks button → Frontend calls invoke() → Backend handles it
```

The backend must be able to function without any frontend (headless/CLI mode).

---

## Frontend Overview

React/TypeScript frontend for Juno - a Tauri v2 desktop application with AI-powered automation capabilities. Built with modern React patterns, shadcn/ui components, and comprehensive Tauri integration.

## Development Commands

```bash
bun install                    # Install dependencies
bun run dev                    # Vite development server
bun run build                  # Build for production
bun run preview                # Preview production build
npm test                       # Run tests (Vitest)
npm run test:watch             # Watch mode testing
```

## Architecture

### Component Structure

```
src/
├── App.tsx                    # Main application component
├── main.tsx                   # React entry point
├── components/                # Reusable components
│   ├── FloatingBar.tsx        # Main floating interface
│   ├── Settings.tsx           # Settings panel
│   ├── VoiceStatusIndicator.tsx # Voice mode indicator
│   ├── ui/                    # shadcn/ui components
│   └── __tests__/             # Component tests
├── hooks/                     # Custom React hooks
├── lib/                       # Utilities and services
├── contexts/                  # React contexts
├── types/                     # TypeScript definitions
└── styles/                    # CSS and styling
```

### Key Components

- **App.tsx**: Main window with modal system (help, feedback, import/export)
- **FloatingBar.tsx**: Primary floating interface, the default bar appearance, Wispr-Flow-shaped. Idle: a tiny 56×16 pill in an 88×48 window. Hover (native `mouse-entered-window` from the tracking area in `platform/macos.rs`, plus DOM hover): the pill animates to 132×34 and reveals a mic button (`invoke("agent_voice", {action:"start"})` → the same `agent-transcription-start` pipeline as the hotkey) and a type button (opens the input focused after `getCurrentWindow().setFocus()`). A drag anywhere on the pill, buttons included, moves the window once the mouse travels `DRAG_THRESHOLD_PX`; the click that follows a drag is swallowed. On release the window glides into the nearest **well**, one of the tidy anchor points around every display (corners + edge-midpoints, a 3×3 grid minus centre per monitor, `src/lib/snapWells.ts`); the drag is free, the wells decide where it lands. That gesture is **not the Pill's own**: it is `useBarDrag` (see Window Dragging below), shared by every appearance, so the Pill holds no drag or snap code of its own. Layouts (`pickLayout` / `BAR_LAYOUTS`): compact, hover, voice (220×34, listening/transcribing/dictating/always-listening, with a Stop-listening control), full (419×44: input open, backend input states, working/error/speaking, or the chat pane). The `StatusDot` renders in the compact idle pill and in the status-bearing layouts (voice/full) but **not** in hover; compact and hover both centre their single child, so growing from compact to hover cross-fades the dot out and the buttons in at the same centre instead of teleporting the dot from centre to the left edge. Once a query occurs `bar/BarChatPane.tsx` opens beneath the pill inside the same window: the main window's `ChatContainerV2` under a scoped `.dark` class, fed by `hooks/useBarConversation.ts` (`useConversation` + `useBackendEvents` with `skipServerCheck` and no audio, because the main window owns TTS playback). Follow-ups go through the pill input; Escape is **not the Pill's own** either, it is `useEscapeToIdle` (see The shared Escape below), so one press puts the input, the pane and whatever Rust is doing away together. **The Pill never resizes its window per state** (the steady frame, `src/lib/steadyFrame.ts`): one transparent window, sized for the largest footprint (full pill + grown composer + roster + pane), placed once per well, with the resting footprint exactly on the well. Every state change is a CSS transition of the column pinned to the well's docked edges (`contentPlacement`). The page reports the drawn footprint through `set_bar_hit_regions` and Rust (`platform/bar_hit_test.rs`) flips click-through as the cursor crosses it and emits enter/leave for it. A drop or display hop into a well that grows the other way swaps frame and drawing behind a two-frame hide (`swapSteadyLayout`). Root cause, pattern and the checklist for the other appearances: `docs/plans/appearance-steady-frame.md`. The bar window has `acceptFirstMouse` (tauri.conf.json) so the first click on an unfocused bar reaches the page; the window is never told about focus by OS focus changes, only by the input itself, so a mic click that activates the window cannot turn into the backend's expand-to-input transition.
- **settings/ModularSettingsWindow.tsx**: the Settings window (`/settings` route), styled after macOS System Settings. A compact left sidebar of coloured icon-tile rows with a search field (filters `settingsCategories` by name + `keywords`), a large content title, the `-apple-system`/SF font, and OS light/dark following via `useSystemTheme` (scopes a `.dark` class on the settings root). Sections live in `settings/sections/*` and are built from the primitives in `settings/ui.tsx`: `SettingsGroup` (a rounded inset card with an optional title above and footer below; `advanced` hides the whole group unless the advanced toggle is on) and `SettingsRow` (one list row: `label`+`description` left, `children` control right, `below` for full-width controls like sliders/textareas; `advanced` gates a single row, `destructive` reddens the label). Rows in a group are auto-separated by hairline dividers. The advanced-settings toggle lives in the sidebar footer; `AdvancedSettingsContext` persists it in the backend. Follow the primitives (not raw `Card`s) when adding settings, and keep neutral chrome on theme tokens so it reads in both themes.
- **VoiceStatusIndicator.tsx**: Real-time voice mode status display
- **ui/**: Complete shadcn/ui component library integration

### State Management

- **Tauri Store**: Persistent settings via `@tauri-apps/plugin-store`
- **React State**: Local component state with hooks
- **Contexts**: `VoiceContext` for voice-related state
- **Event System**: Tauri events for backend communication

## Tauri Integration

### API Communication

```typescript
// Tauri command invocation
import { invoke } from '@tauri-apps/api/core';

const result = await invoke('command_name', { param: value });
```

### Event Handling

**Use `useEventListener` hook** for all Tauri event listeners in React components. It handles the async listen/cleanup race condition and uses a ref to always call the latest handler:

```typescript
import { useEventListener } from '@/hooks/useEventListener';

// Simple, no dependency arrays needed (ref pattern keeps handler current)
useEventListener<{ chunk: string }>('event_name', (payload) => {
  // payload is event.payload, already unwrapped
});
```

**Manual pattern** (only when `useEventListener` won't work, e.g. conditional listeners):
```typescript
useEffect(() => {
  let unlisten: (() => void) | undefined;
  let mounted = true;

  const setup = async () => {
    try {
      const fn = await listen('event', (e) => {
        if (!mounted) return;
        handler(e);
      });
      if (mounted) unlisten = fn;
      else safeCleanupEventListener(fn); // Resolved after unmount, so clean up immediately
    } catch (err) {
      console.error('Failed to setup listener:', err);
    }
  };
  setup();

  return () => {
    mounted = false;
    safeCleanupEventListener(unlisten);
  };
}, []);
```

**BANNED pattern** (race condition: cleanup runs before the promise resolves):
```typescript
// DO NOT USE. Listener leaks if component unmounts before listen() resolves
const unlisten = listen('event', handler);
return () => { unlisten.then((fn) => fn()); };
```

### Common Tauri Commands

- `media_get_state` / `media_control` - Live player state for the agent-rendered `NowPlayingCard` (Spotify and Apple Music via AppleScript; the app is addressed as `Music`, shown as "Apple Music"). Music has no artwork URL, so the backend exports the track's artwork bytes once into `$APPCACHE/media-artwork/` and returns an `asset://` URL (asset protocol scope + `img-src` CSP in `tauri.conf.json`). Components that show live state (play/pause, progress) must read it from a live source like this; a `QueryButton` is a one-shot request and is never a play/pause toggle.
- Bare playback commands ("pause Spotify", "what's playing?") never reach the model: `src-tauri/src/agent/local_intents.rs` answers them in `submit_query` with one AppleScript call and the pre-built `NowPlayingCard`, emitting the same stream events as an agent run. Extend the grammar there, not in the prompt.
- `<Why>…</Why>` in an agent response is method rationale. `mixed-content-renderer.tsx` lifts it out before JSX parsing and renders it through `WhyBlock` (collapsed by default); a legacy `**Why X instead of Y:**` paragraph is collapsed the same way.
- `dispatch_query` - Submit a user query from any UI surface (chat input, example prompt, agent-rendered `QueryButton`). Fire-and-forget: the backend emits `user-message-submitted` (append the message, set processing) and runs the agent. Never call `submit_query` from the frontend.
- `submit_query` - Backend entry point that runs the agent; used by the `agent-query-ready` listener, CLI, cloud, and scheduler
- `get_settings` / `update_settings` - Settings management
- `start_dictation` / `stop_dictation` - Voice control
- `capture_screenshot` - Screenshot functionality
- `get_permissions_status` - Check system permissions

### Window Dragging

All floating windows use **programmatic dragging** via `useDragWindow` hooks. Do NOT use `data-tauri-drag-region`, which is unreliable on macOS with `transparent: true` + `decorations: false` windows.

**Two hooks** in `src/hooks/useDragWindow.ts`:

```typescript
// EVERY bar appearance (Pill, Bar, Studio, Island, Orb, Halo, Avatar):
import { useBarDrag } from "@/hooks/useDragWindow";

const { dragProps, swallowClickAfterDrag } = useBarDrag({ displayFollowPaused: busy });
return (
  <div
    {...dragProps}
    onClickCapture={(e) => {
      if (swallowClickAfterDrag(e)) return;
      /* the look's own click handling */
    }}
    className="cursor-grab active:cursor-grabbing"
  >…</div>
);
```

```typescript
// The floating panels, whose chrome always means "drag":
import { useDragWindow } from "@/hooks/useDragWindow";

const onDragMouseDown = useDragWindow();
return <div onMouseDown={onDragMouseDown} className="cursor-grab active:cursor-grabbing">...</div>;
```

**Rules:**
- `useBarDrag()` is the **only** drag a bar appearance uses, and it is what gives that look the gravity wells: drag past `DRAG_THRESHOLD_PX` (4px) starts the drag (`startBarDrag`) and shows the drop indicator, release glides the window into the nearest well and records the slot in the dock store (`src/lib/barDock.ts`). A look that calls it gets wells with no further wiring; a look that does not, does not snap. The snap itself is gated on the window label, so the hook is safe anywhere.
- **A steady look (the Pill) drags as a window the size of its shape.** At drag start `startBarDrag` shrinks the window to the footprint the look is drawing (`dragLayout`, behind the usual two-frame hide), then calls `startDragging()`. macOS keeps a floating window's top below the menu bar during the OS drag; with the window being the pill, that holds the pill just under the menu bar, which is where the top wells are. The drag window is put down by the cursor, not where the shape was at the press: `set_bar_frame` with `grabX`/`grabY` (the press re-measured from the shape, `grabInDragWindow`) reads the cursor in the same main-thread call as `setFrame:` and puts that spot under it. The OS drag keeps whatever offset the window has when it starts, so a fast flick (cursor already past the threshold and the hidden frames) used to leave the bar trailing the cursor. A look that is not steady is re-placed the same way at its own size before `startDragging()`. On release the small window glides into the well and `swapSteadyLayout` grows it back. A release the page never hears is caught by the release watch (`bar_pointer_held`). Never drive the drag from Rust or touch the window's class to lift the constraint (see "Never" in `docs/plans/appearance-steady-frame.md`).
- **All bar geometry is in global desktop points** (`src/lib/desktopPoints.ts`, `platform/desktop_points.rs`). Tauri's "physical" numbers use three different scales (monitors: own factor; cursor: primary display's; window: its display's), which disagree on a mixed-density desk. Convert at the edge; never compare raw Tauri positions. `set_bar_frame` and the stored bar position take points.
- **One snap-well overlay per display** (`snap-wells-overlay`, then `snap-wells-overlay-N`, built by `ensure_snap_wells_overlays`). With "Displays have separate Spaces" a window is drawn on one display only, so one overlay spanning every monitor cannot work.
- A bar drag starts **anywhere except text entry** (`input, textarea, [contenteditable], [data-no-drag]`). Buttons are draggable-through: the movement threshold means a press and release on one is still that button's click.
- `useDragWindow()` calls `startDragging()` immediately on mousedown, so it excludes every control (`button, input, textarea, select, a, [role="button"], [contenteditable], [data-no-drag]`). Panels only.
- Always add `cursor-grab active:cursor-grabbing` for visual affordance.
- NEVER add `data-tauri-drag-region` to any element. This attribute is banned.
- The Tauri capability `core:window:allow-start-dragging` must be present (already in `floating.json`).

### The shared Escape

Escape is the universal cancel key, fixed and not configurable (`constants/settings.rs`). One press returns the bar to the tiny idle bar from whatever it is showing, and that behaviour is defined **once**:

- `src/lib/barEscape.ts` is the decision, pure: `BarEscapeState` (the bar state, `working`, `overlayOpen`, `composerOpen`, `popupOpen` — all required, so a look that forgets one does not build) in, `{ closeLocally, report }` out. React closes the layers React opened; everything else is reported to Rust, which decides what stopping means. Never re-derive "is Rust busy" from the bar state in a component: guessing at that is what broke Escape in every appearance at once.
- `src/hooks/useEscapeToIdle.ts` is the wiring, and it is the only Escape handler a bar appearance has. It claims the stop key through `set_bar_pane_open` while the bar is expanded, so Rust's passive `NSEvent` monitor observes a press wherever the person is typing; it answers `bar-dismiss-pane` (what Rust sends back when it found nothing to stop); and it keeps a `document` keydown for the focused bar. Presses from the two routes are coalesced so one keystroke collapses one layer.
- Do not hand-roll `e.key === "Escape"` in an appearance, and do not arm the monitor or listen for the dismissal yourself. `src/components/bar/__tests__/escapeContract.test.ts` fails the build if you do, and fails if an appearance is added to the catalog without being wired.

**The wells are shared, not the Pill's.** `src/lib/snapWells.ts` is the pure geometry (wells per display, nearest, same-slot-on-another-display), `src/lib/barDock.ts` holds the dock slot plus what it decides (`dockAnchorX`, `dockGrowsUp`, `distinctWells`, `predictedWindowOrigin`), `src/hooks/useBarSnapWells.ts` performs the snap and the cursor-display re-home, and `useWindowSize` defaults `anchorX`/`growUp` from the dock slot so every look's resize is anchored on the edges it is docked against. Wells are computed from the window's **measured logical outer size** every time, so there is no per-look data: a wide look's columns simply converge as its width approaches the inset area.

## Component Patterns

### Modal System

```typescript
// Modal state management
const [isHelpOpen, setIsHelpOpen] = useState(false);
const [isFeedbackOpen, setIsFeedbackOpen] = useState(false);

// Modal components with proper accessibility
<Dialog open={isHelpOpen} onOpenChange={setIsHelpOpen}>
  <DialogContent className="max-w-4xl max-h-[80vh] overflow-y-auto">
    {/* Modal content */}
  </DialogContent>
</Dialog>
```

### Voice Integration

```typescript
// Voice context usage
const { isListening, startListening, stopListening } = useVoiceContext();

// Voice status indicator
<VoiceStatusIndicator 
  isListening={isListening}
  mode={voiceMode}
  onToggle={handleVoiceToggle}
/>
```

### Settings Management

```typescript
// Settings hook pattern
const { settings, updateSettings, isLoading } = useSettings();

// Settings update
await updateSettings({
  provider: 'anthropic',
  voiceMode: 'agent'
});
```

## Testing Strategy

### Technology Stack

- **Vitest**: Test runner with TypeScript support
- **Testing Library**: React component testing
- **jsdom**: Browser environment simulation
- **MSW**: API mocking (if needed)

### Test Structure

```typescript
// Component test example
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { vi } from 'vitest';
import Component from './Component';

// Mock Tauri APIs
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn()
}));

describe('Component', () => {
  it('renders and handles interaction', async () => {
    render(<Component />);
    
    const button = screen.getByRole('button');
    fireEvent.click(button);
    
    await waitFor(() => {
      expect(screen.getByText('Expected Text')).toBeInTheDocument();
    });
  });
});
```

### Mock Patterns

```typescript
// Tauri API mocks in test setup
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn()
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
  emit: vi.fn()
}));

// Plugin mocks
vi.mock('tauri-plugin-voice-transcription-api', () => ({
  startDictation: vi.fn(),
  stopDictation: vi.fn()
}));
```

## UI/UX Patterns

### Design System

- **shadcn/ui**: Complete component library
- **Tailwind CSS**: Utility-first styling
- **Lucide Icons**: Consistent iconography
- **Radix UI**: Accessible component primitives

### Responsive Design

```typescript
// Mobile-first responsive patterns
const isMobile = useMediaQuery('(max-width: 768px)');

// Conditional rendering
{isMobile ? <MobileComponent /> : <DesktopComponent />}
```

### Accessibility

- **ARIA Labels**: Proper labeling for screen readers
- **Keyboard Navigation**: Full keyboard support
- **Focus Management**: Proper focus trapping in modals
- **Semantic HTML**: Meaningful element structure

## Performance Considerations

### Event Debouncing

```typescript
// Debounce rapid events
const debouncedHandler = useMemo(
  () => debounce((value: string) => {
    // Handle value change
  }, 300),
  []
);

useEffect(() => {
  return () => debouncedHandler.cancel();
}, []);
```

### Component Optimization

```typescript
// Memoization for expensive operations
const expensiveValue = useMemo(() => {
  return computeExpensiveValue(props);
}, [props.dependency]);

// Callback memoization
const handleClick = useCallback((id: string) => {
  onItemClick(id);
}, [onItemClick]);
```

## Error Handling

### Error Boundaries

```typescript
// Error boundary for graceful error handling
class ErrorBoundary extends Component {
  componentDidCatch(error: Error, errorInfo: ErrorInfo) {
    console.error('Component error:', error, errorInfo);
  }
  
  render() {
    if (this.state.hasError) {
      return <ErrorFallback />;
    }
    return this.props.children;
  }
}
```

### Async Error Handling

```typescript
// Proper async error handling
const handleAsyncOperation = async () => {
  try {
    setLoading(true);
    const result = await invoke('command_name');
    setData(result);
  } catch (error) {
    console.error('Operation failed:', error);
    setError(error.message);
  } finally {
    setLoading(false);
  }
};
```

## Common Issues and Solutions

### Tauri Event Cleanup

```typescript
// PREFERRED: Use the useEventListener hook, which handles cleanup automatically
import { useEventListener } from '@/hooks/useEventListener';
useEventListener<PayloadType>('event-name', (payload) => { /* ... */ });

// For multiple listeners in one effect, use the mounted flag pattern
// (see Event Handling section above)
```

### Type Safety

```typescript
// Proper typing for Tauri commands
interface CommandResult {
  success: boolean;
  data?: any;
  error?: string;
}

const result = await invoke<CommandResult>('command_name');
```

### Development vs Production

```typescript
// Environment-specific behavior
const isDev = import.meta.env.DEV;

if (isDev) {
  // Development-only code
}
```

## Key Files Reference

- `src/App.tsx` - Main application component with modal system
- `src/components/FloatingBar.tsx` - Primary user interface
- `src/components/Settings.tsx` - Settings management
- `src/hooks/useSettings.ts` - Settings hook
- `src/lib/utils.ts` - Utility functions
- `src/contexts/VoiceContext.tsx` - Voice state management
- `src/types/` - TypeScript type definitions
- `vitest.config.ts` - Test configuration
- `tsconfig.json` - TypeScript configuration