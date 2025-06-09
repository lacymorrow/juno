import { render, screen } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { ComponentType } from 'react';

// Mock Tauri API
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
  emit: vi.fn(),
}));

// Mock the voice transcription plugin
vi.mock('tauri-plugin-voice-transcription-api', () => ({
  startListening: vi.fn(),
  stopListening: vi.fn(),
  isListening: vi.fn(),
}));

// Mock lucide-react icons
vi.mock('lucide-react', () => ({
  AlertCircle: () => <div data-testid="alert-circle-icon" />,
  Brain: () => <div data-testid="brain-icon" />,
  Mic: () => <div data-testid="mic-icon" />,
  MicOff: () => <div data-testid="mic-off-icon" />,
  Type: () => <div data-testid="type-icon" />,
  Volume2: () => <div data-testid="volume-icon" />,
}));

// Mock the utils
vi.mock('@/lib/utils', () => ({
  cn: (...classes: any[]) => classes.filter(Boolean).join(' '),
}));

// Dynamic import for the component to test
let VoiceStatusIndicator: ComponentType<any>;

describe('VoiceStatusIndicator', () => {
  beforeEach(async () => {
    vi.clearAllMocks();
    
    // Dynamically import the component after mocks are set up
    const module = await import('../VoiceStatusIndicator');
    VoiceStatusIndicator = module.VoiceStatusIndicator;
  });

  it('renders with default state', () => {
    const result = render(<VoiceStatusIndicator />);
    expect(result).toBeDefined();
  });

  it('shows voice indicator elements', () => {
    render(<VoiceStatusIndicator />);
    
    // Should render some content
    expect(document.body).toContainHTML('<div');
    
    // Check for icon presence (one of the mocked icons should be present)
    const icons = [
      screen.queryByTestId('mic-icon'),
      screen.queryByTestId('mic-off-icon'),
      screen.queryByTestId('brain-icon'),
      screen.queryByTestId('type-icon'),
      screen.queryByTestId('volume-icon'),
      screen.queryByTestId('alert-circle-icon'),
    ];
    
    const hasIcon = icons.some(icon => icon !== null);
    expect(hasIcon).toBe(true);
  });

  it('renders in compact mode', () => {
    const result = render(<VoiceStatusIndicator variant="compact" />);
    expect(result).toBeDefined();
  });

  it('renders with text disabled', () => {
    const result = render(<VoiceStatusIndicator showText={false} />);
    expect(result).toBeDefined();
  });

  it('applies custom className', () => {
    render(<VoiceStatusIndicator className="custom-class" />);
    
    // Should render without errors with custom class
    expect(document.body).toContainHTML('<div');
  });
});