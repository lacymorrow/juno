# CLAUDE.md - Frontend

This file provides guidance to Claude Code when working with the React/TypeScript frontend in this repository.

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
- **FloatingBar.tsx**: Primary floating interface for user interaction
- **Settings.tsx**: Comprehensive settings management
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

// Simple — no dependency arrays needed (ref pattern keeps handler current)
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
      else safeCleanupEventListener(fn); // Resolved after unmount — clean up immediately
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

**BANNED pattern** (race condition — cleanup runs before promise resolves):
```typescript
// DO NOT USE — listener leaks if component unmounts before listen() resolves
const unlisten = listen('event', handler);
return () => { unlisten.then((fn) => fn()); };
```

### Common Tauri Commands

- `submit_query` - Submit chat message to AI agent
- `get_settings` / `update_settings` - Settings management
- `start_dictation` / `stop_dictation` - Voice control
- `capture_screenshot` - Screenshot functionality
- `get_permissions_status` - Check system permissions

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
// PREFERRED: Use useEventListener hook — handles cleanup automatically
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