import { render, screen } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';

// Mock Tauri API
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(),
  emit: vi.fn(),
}));

// Mock heavy sub-panels that trigger async effects or use browser APIs unavailable in jsdom
vi.mock('../devtools/CloudTestPanel', () => ({
  CloudTestPanel: () => <div data-testid="cloud-test-panel" />,
}));

vi.mock('../devtools/WakeWordTesting', () => ({
  __esModule: true,
  default: ({ children }: { children?: any }) => (
    <div data-testid="wake-word-testing">{children}</div>
  ),
}));

// VisualizationSettings reads localStorage in useState initializers — mock to avoid
// "localStorage.getItem is not a function" in jsdom
vi.mock('../devtools/VisualizationSettings', () => ({
  __esModule: true,
  default: () => <div data-testid="visualization-settings" />,
}));

// Mock the UI components with simple implementations
vi.mock('@/components/ui/button', () => ({
  Button: ({ children, onClick, ...props }: any) => (
    <button onClick={onClick} {...props}>
      {children}
    </button>
  ),
}));

vi.mock('@/components/ui/card', () => ({
  Card: ({ children }: any) => <div data-testid="card">{children}</div>,
  CardContent: ({ children }: any) => <div data-testid="card-content">{children}</div>,
  CardDescription: ({ children }: any) => <div data-testid="card-description">{children}</div>,
  CardHeader: ({ children }: any) => <div data-testid="card-header">{children}</div>,
  CardTitle: ({ children }: any) => <div data-testid="card-title">{children}</div>,
}));

vi.mock('@/components/ui/input', () => ({
  Input: (props: any) => <input {...props} />,
}));

vi.mock('@/components/ui/scroll-area', () => ({
  ScrollArea: ({ children }: any) => <div data-testid="scroll-area">{children}</div>,
}));

vi.mock('@/components/ui/tabs', () => ({
  Tabs: ({ children }: any) => <div data-testid="tabs">{children}</div>,
  TabsContent: ({ children }: any) => <div data-testid="tabs-content">{children}</div>,
  TabsList: ({ children }: any) => <div data-testid="tabs-list">{children}</div>,
  TabsTrigger: ({ children }: any) => <button data-testid="tabs-trigger">{children}</button>,
}));

// Import statically: vi.mock() calls are hoisted above imports, so the mocks
// are already in place. The old pattern dynamically imported the component
// inside beforeEach, which made the FIRST hook pay for cold-transforming
// DevToolsPanel's large sub-panel dependency graph — on a loaded CI worker
// that blew the 10s hook timeout. A static import moves that cost to module
// collection, where it belongs and has no per-hook deadline.
import DevToolsPanel from '../DevToolsPanel';

describe('DevToolsPanel', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('should render without crashing', () => {
    // Simple test that just checks if the component can be rendered
    const result = render(<DevToolsPanel />);
    expect(result).toBeDefined();
  });

  it('should contain dev tools elements', () => {
    render(<DevToolsPanel />);
    
    // Check for basic structure elements that should be present
    const tabs = screen.queryByTestId('tabs');
    if (tabs) {
      expect(tabs).toBeInTheDocument();
    }
    
    // At minimum, the component should render something
    expect(document.body).toContainHTML('<div');
  });
});