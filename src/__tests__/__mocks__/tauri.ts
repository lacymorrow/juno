// Mock Tauri API for tests
import { vi } from 'vitest';

export const mockInvoke = vi.fn();
export const mockListen = vi.fn(() => Promise.resolve(() => {}));

// Mock window.__TAURI__ for tests
if (typeof window !== 'undefined') {
  (window as any).__TAURI__ = {
    event: {
      listen: mockListen,
    },
    core: {
      invoke: mockInvoke,
    },
  };
}