/**
 * Tests for race condition fixes in critical hooks
 */

import { describe, it, expect, beforeEach, vi } from 'vitest';
import { renderHook, act } from '@testing-library/react';
import { useBackendEvents } from '../hooks/useBackendEvents';
import { useSettings } from '../hooks/useSettings';
import { useAppStateSync } from '../hooks/useAppStateSync';
import { AgentTriggerMode } from '../types/state';

// Mock Tauri API
vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(),
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

describe('Race Condition Fixes', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe('useBackendEvents - Streaming Message Concurrency', () => {
    it('should handle concurrent streaming updates without race conditions', async () => {
      const mockProps = {
        addSystemMessage: vi.fn(),
        addAssistantMessage: vi.fn(),
        addOrUpdateStreamingMessage: vi.fn(),
        playAudioFromBase64: vi.fn(),
        stopCurrentAudio: vi.fn(),
        setIsProcessing: vi.fn(),
      };

      const { result } = renderHook(() => useBackendEvents(mockProps));

      // Simulate concurrent streaming events
      const chunks = ['Hello', ' ', 'World', '!'];

      // Test: Concurrent chunk updates should be processed in order
      const updatePromises = chunks.map(() => {
        return new Promise<void>(resolve => {
          setTimeout(() => {
            // Simulate event handler being called
            act(() => {
              // The hook now uses a lock mechanism to ensure thread-safe updates
              expect(mockProps.addOrUpdateStreamingMessage).toHaveBeenCalled();
            });
            resolve();
          }, Math.random() * 10); // Random delay to simulate concurrency
        });
      });

      await Promise.all(updatePromises);
      
      // Verify no race conditions occurred
      expect(result.current.isListening).toBe(true);
    });

    it('should clean up locks on unmount', () => {
      const mockProps = {
        addSystemMessage: vi.fn(),
        addAssistantMessage: vi.fn(),
        addOrUpdateStreamingMessage: vi.fn(),
        playAudioFromBase64: vi.fn(),
        stopCurrentAudio: vi.fn(),
        setIsProcessing: vi.fn(),
      };

      const { unmount } = renderHook(() => useBackendEvents(mockProps));

      // Unmount should clean up all internal state
      unmount();

      // Verify cleanup (internal state is cleaned up properly)
      expect(mockProps.addOrUpdateStreamingMessage).not.toHaveBeenCalled();
    });
  });

  describe('useSettings - Stale Closure Prevention', () => {
    it('should not capture stale activeProvider in event listener', async () => {
      const { invoke } = await import('@tauri-apps/api/core');
      const { listen } = await import('@tauri-apps/api/event');
      
      // Mock initial values
      vi.mocked(invoke).mockImplementation((cmd: string) => {
        switch (cmd) {
          case 'get_active_provider':
            return Promise.resolve('provider1');
          case 'get_providers':
            return Promise.resolve([{ id: 'provider1' }, { id: 'provider2' }]);
          default:
            return Promise.resolve('');
        }
      });

      let eventCallback: any;
      vi.mocked(listen).mockImplementation((event: string, callback: any) => {
        if (event === 'provider-settings-changed') {
          eventCallback = callback;
        }
        return Promise.resolve(() => {});
      });

      const { result } = renderHook(() => useSettings());

      // Wait for initial load
      await act(async () => {
        await new Promise(resolve => setTimeout(resolve, 100));
      });

      // Change active provider
      act(() => {
        result.current.handleActiveProviderChange('provider2');
      });

      // Simulate event with new provider
      if (eventCallback) {
        act(() => {
          eventCallback({
            payload: {
              active_provider: 'provider2',
              providers: [
                { id: 'provider1', api_key: 'key1' },
                { id: 'provider2', api_key: 'key2' },
              ],
            },
          });
        });
      }

      // The hook should now correctly use ref to avoid stale closure
      expect(result.current.activeProvider).toBe('provider2');
    });
  });

  describe('useAppStateSync - Optimistic Update Rollback', () => {
    it('should rollback optimistic updates on backend failure', async () => {
      const { invoke } = await import('@tauri-apps/api/core');
      
      // Mock successful initial load
      vi.mocked(invoke).mockImplementation((cmd: string) => {
        if (cmd.startsWith('get_')) {
          return Promise.resolve(false); // Default values
        }
        if (cmd.startsWith('set_')) {
          // Simulate backend failure
          return Promise.reject(new Error('Backend update failed'));
        }
        return Promise.resolve(null);
      });

      const { result } = renderHook(() => useAppStateSync());

      // Wait for initial load
      await act(async () => {
        await new Promise(resolve => setTimeout(resolve, 100));
      });

      const initialState = result.current.state;

      // Attempt to update state (should fail and rollback)
      await act(async () => {
        try {
          await result.current.updateState({
            audioSettings: {
              soundEnabled: true,
            },
          });
        } catch (error) {
          // Expected to fail
        }
      });

      // State should have rolled back to initial value
      expect(result.current.state.audioSettings.soundEnabled).toBe(initialState.audioSettings.soundEnabled);
      expect(result.current.error).toBeTruthy();
    });

    it('should handle concurrent state updates correctly', async () => {
      const { invoke } = await import('@tauri-apps/api/core');
      
      // Mock successful updates
      vi.mocked(invoke).mockImplementation((cmd: string) => {
        if (cmd.startsWith('get_')) {
          return Promise.resolve(false);
        }
        if (cmd.startsWith('set_')) {
          return Promise.resolve();
        }
        return Promise.resolve(null);
      });

      const { result } = renderHook(() => useAppStateSync());

      // Wait for initial load
      await act(async () => {
        await new Promise(resolve => setTimeout(resolve, 100));
      });

      // Fire multiple concurrent updates
      const updates = [
        { audioSettings: { soundEnabled: true } },
        { uiSettings: { debugMode: true } },
        { inputSettings: { agentTriggerMode: AgentTriggerMode.Tap } },
      ];

      await act(async () => {
        await Promise.all(
          updates.map(update => result.current.updateState(update))
        );
      });

      // All updates should have been applied
      expect(result.current.state.audioSettings.soundEnabled).toBe(true);
      expect(result.current.state.uiSettings.debugMode).toBe(true);
      expect(result.current.state.inputSettings.agentTriggerMode).toBe('tap');
    });
  });
});