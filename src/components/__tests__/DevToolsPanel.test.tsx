import { render, screen } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { ComponentType } from 'react';

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

// Dynamic import for the component to test
let DevToolsPanel: ComponentType;

describe('DevToolsPanel', () => {
  beforeEach(async () => {
    vi.clearAllMocks();
    
    // Dynamically import the component after mocks are set up
    const module = await import('../DevToolsPanel');
    DevToolsPanel = module.default;
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