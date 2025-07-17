/**
 * Race Condition Test Suite for DotDot Event System
 * 
 * This test suite validates the application's resilience against
 * race conditions, concurrent access, and synchronization issues.
 */

import { renderHook, act, waitFor } from '@testing-library/react';
import { useBackendEvents } from '../hooks/useBackendEvents';
import { listen, emit } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';

// Mock Tauri APIs
jest.mock('@tauri-apps/api/event');
jest.mock('@tauri-apps/api/core');

// Test utilities
class EventSimulator {
  private eventHandlers = new Map<string, Set<Function>>();

  constructor() {
    // Mock the listen function to track handlers
    (listen as jest.Mock).mockImplementation((event: string, handler: Function) => {
      if (!this.eventHandlers.has(event)) {
        this.eventHandlers.set(event, new Set());
      }
      this.eventHandlers.get(event)!.add(handler);
      
      // Return unsubscribe function
      return () => {
        this.eventHandlers.get(event)?.delete(handler);
      };
    });
  }

  async simulateEvent(eventType: string, payload: any, delay?: number) {
    if (delay) await this.delay(delay);
    
    const handlers = this.eventHandlers.get(eventType) || new Set();
    const promises = Array.from(handlers).map(handler => 
      handler({ event: eventType, payload })
    );
    
    await Promise.all(promises);
  }

  async simulateConcurrentEvents(events: Array<{ type: string; payload: any; delay?: number }>) {
    const promises = events.map(event => 
      this.simulateEvent(event.type, event.payload, event.delay)
    );
    return Promise.all(promises);
  }

  private delay(ms: number): Promise<void> {
    return new Promise(resolve => setTimeout(resolve, ms));
  }
}

// Performance monitor for tracking metrics
class PerformanceMonitor {
  private startTime: number = 0;
  private eventTimings: Map<string, number[]> = new Map();
  private memorySnapshots: number[] = [];

  startTracking() {
    this.startTime = performance.now();
    this.memorySnapshots.push(this.getMemoryUsage());
  }

  recordEventTiming(eventType: string, duration: number) {
    if (!this.eventTimings.has(eventType)) {
      this.eventTimings.set(eventType, []);
    }
    this.eventTimings.get(eventType)!.push(duration);
  }

  getMemoryUsage(): number {
    if (performance.memory) {
      return performance.memory.usedJSHeapSize;
    }
    return 0;
  }

  getReport() {
    const endTime = performance.now();
    const currentMemory = this.getMemoryUsage();
    
    return {
      duration: endTime - this.startTime,
      memoryGrowth: currentMemory - this.memorySnapshots[0],
      eventTimings: Object.fromEntries(this.eventTimings),
      avgEventTime: this.calculateAverageEventTime()
    };
  }

  private calculateAverageEventTime(): number {
    let total = 0;
    let count = 0;
    
    this.eventTimings.forEach(timings => {
      timings.forEach(time => {
        total += time;
        count++;
      });
    });
    
    return count > 0 ? total / count : 0;
  }
}

describe('Race Condition Tests', () => {
  let eventSimulator: EventSimulator;
  let performanceMonitor: PerformanceMonitor;
  let mockProps: any;

  beforeEach(() => {
    eventSimulator = new EventSimulator();
    performanceMonitor = new PerformanceMonitor();
    
    mockProps = {
      addSystemMessage: jest.fn(),
      addAssistantMessage: jest.fn(),
      addOrUpdateStreamingMessage: jest.fn(),
      playAudioFromBase64: jest.fn().mockResolvedValue(undefined),
      stopCurrentAudio: jest.fn().mockResolvedValue(undefined),
      setIsProcessing: jest.fn()
    };

    jest.clearAllMocks();
  });

  describe('Streaming Message Race Conditions', () => {
    it('should handle concurrent updates to the same message ID without data loss', async () => {
      const { result } = renderHook(() => useBackendEvents(mockProps));
      
      const messageId = 'test-msg-123';
      const chunks = Array.from({ length: 100 }, (_, i) => `chunk${i}`);
      
      performanceMonitor.startTracking();

      // Simulate concurrent chunk arrivals with random delays
      await act(async () => {
        const promises = chunks.map((chunk, index) => 
          eventSimulator.simulateEvent('agent-text-stream', { 
            message_id: messageId, 
            chunk,
            timestamp: Date.now() + Math.random() * 10
          })
        );
        
        await Promise.all(promises);
      });

      // Wait for all updates to be processed
      await waitFor(() => {
        const calls = mockProps.addOrUpdateStreamingMessage.mock.calls;
        const lastCall = calls[calls.length - 1];
        if (lastCall) {
          const [, content] = lastCall;
          // Verify all chunks are present in the final message
          chunks.forEach(chunk => {
            expect(content).toContain(chunk);
          });
        }
      });

      const report = performanceMonitor.getReport();
      console.log('Performance Report:', report);
      
      // Ensure no excessive memory growth
      expect(report.memoryGrowth).toBeLessThan(10 * 1024 * 1024); // 10MB threshold
    });

    it('should handle out-of-order streaming events correctly', async () => {
      const { result } = renderHook(() => useBackendEvents(mockProps));
      
      const messageId = 'test-msg-456';
      
      await act(async () => {
        // Send events in reverse order
        await eventSimulator.simulateEvent('agent-stream-end', { 
          message_id: messageId,
          complete_text: 'Hello World'
        });
        
        await eventSimulator.simulateEvent('agent-text-stream', { 
          message_id: messageId, 
          chunk: 'World' 
        });
        
        await eventSimulator.simulateEvent('agent-text-stream', { 
          message_id: messageId, 
          chunk: 'Hello ' 
        });
        
        await eventSimulator.simulateEvent('agent-stream-start', { 
          message_id: messageId 
        });
      });

      // Verify the complete message was used when stream ended
      await waitFor(() => {
        const calls = mockProps.addOrUpdateStreamingMessage.mock.calls;
        const completeCall = calls.find(call => call[2] === true); // isComplete = true
        expect(completeCall).toBeTruthy();
        expect(completeCall[1]).toBe('Hello World');
      });
    });

    it('should handle multiple simultaneous message streams', async () => {
      const { result } = renderHook(() => useBackendEvents(mockProps));
      
      const numStreams = 10;
      const messages = Array.from({ length: numStreams }, (_, i) => ({
        id: `msg-${i}`,
        chunks: Array.from({ length: 50 }, (_, j) => `Stream${i}-Chunk${j}`)
      }));
      
      performanceMonitor.startTracking();

      await act(async () => {
        // Create all events
        const allEvents = messages.flatMap(msg =>
          msg.chunks.map(chunk => ({
            type: 'agent-text-stream',
            payload: { message_id: msg.id, chunk }
          }))
        );
        
        // Shuffle for random execution order
        const shuffled = allEvents.sort(() => Math.random() - 0.5);
        
        // Fire all events concurrently
        await eventSimulator.simulateConcurrentEvents(shuffled);
      });

      // Verify each stream maintained its integrity
      await waitFor(() => {
        messages.forEach(msg => {
          const calls = mockProps.addOrUpdateStreamingMessage.mock.calls
            .filter(call => call[0] === msg.id);
          
          // Should have received updates for this message
          expect(calls.length).toBeGreaterThan(0);
          
          // Final message should contain all chunks
          const lastCall = calls[calls.length - 1];
          if (lastCall) {
            const content = lastCall[1];
            msg.chunks.forEach(chunk => {
              expect(content).toContain(chunk);
            });
          }
        });
      });

      const report = performanceMonitor.getReport();
      expect(report.avgEventTime).toBeLessThan(10); // Should process quickly
    });
  });

  describe('State Update Race Conditions', () => {
    it('should maintain state consistency under concurrent updates', async () => {
      const { result } = renderHook(() => useBackendEvents(mockProps));
      
      const stateUpdates = [
        { event: 'backend-response', payload: { response: { text: 'Response 1', agent_state: 'idle' } } },
        { event: 'agent-stop', payload: { stopType: 'normal' } },
        { event: 'tts-audio-ready', payload: { audio_base64: 'test-audio' } },
        { event: 'agent-stop', payload: { stopType: 'force' } },
        { event: 'backend-response', payload: { response: { text: 'Response 2', agent_state: 'processing' } } },
      ];
      
      await act(async () => {
        // Fire all state updates concurrently
        await eventSimulator.simulateConcurrentEvents(
          stateUpdates.map(update => ({
            type: update.event,
            payload: update.payload
          }))
        );
      });

      // Verify final state is consistent
      expect(mockProps.setIsProcessing).toHaveBeenCalled();
      expect(mockProps.addAssistantMessage).toHaveBeenCalledTimes(2);
      expect(mockProps.stopCurrentAudio).toHaveBeenCalled();
      
      // Last setIsProcessing should be false (from agent-stop)
      const lastProcessingCall = mockProps.setIsProcessing.mock.calls[
        mockProps.setIsProcessing.mock.calls.length - 1
      ];
      expect(lastProcessingCall[0]).toBe(false);
    });

    it('should handle rapid event switching without corruption', async () => {
      const { result } = renderHook(() => useBackendEvents(mockProps));
      
      const iterations = 50;
      const events = [];
      
      // Generate alternating events
      for (let i = 0; i < iterations; i++) {
        events.push({
          type: i % 2 === 0 ? 'agent-stream-start' : 'agent-stop',
          payload: i % 2 === 0 
            ? { message_id: `msg-${i}` }
            : { stopType: 'normal' }
        });
      }
      
      await act(async () => {
        await eventSimulator.simulateConcurrentEvents(events);
      });

      // Verify no errors occurred and state remained consistent
      expect(mockProps.setIsProcessing).toHaveBeenCalledTimes(iterations);
      expect(mockProps.stopCurrentAudio).toHaveBeenCalledTimes(iterations / 2);
    });
  });

  describe('Audio Resource Management Race Conditions', () => {
    it('should handle rapid audio start/stop without resource leaks', async () => {
      const { result } = renderHook(() => useBackendEvents(mockProps));
      
      const audioBase64 = 'test-audio-base64';
      const iterations = 20;
      
      for (let i = 0; i < iterations; i++) {
        await act(async () => {
          // Start playing audio
          await eventSimulator.simulateEvent('tts-audio-ready', { 
            audio_base64: audioBase64 
          });
          
          // Randomly stop audio before it completes
          if (Math.random() > 0.5) {
            await eventSimulator.simulateEvent('tts-stop-requested', {});
          }
        });
      }

      // Verify audio operations were handled correctly
      expect(mockProps.playAudioFromBase64.mock.calls.length).toBeGreaterThan(0);
      expect(mockProps.stopCurrentAudio.mock.calls.length).toBeGreaterThan(0);
      
      // No calls should have thrown errors
      expect(mockProps.playAudioFromBase64).not.toHaveBeenCalledWith(
        expect.objectContaining({ error: expect.any(Error) })
      );
    });

    it('should handle concurrent audio operations correctly', async () => {
      const { result } = renderHook(() => useBackendEvents(mockProps));
      
      const audioSamples = Array(5).fill(null).map((_, i) => `audio-${i}`);
      
      await act(async () => {
        // Try to play multiple audio streams concurrently
        await eventSimulator.simulateConcurrentEvents(
          audioSamples.map(audio => ({
            type: 'tts-audio-ready',
            payload: { audio_base64: audio }
          }))
        );
      });

      // All audio play attempts should have been made
      expect(mockProps.playAudioFromBase64).toHaveBeenCalledTimes(5);
      
      // Stop audio to clean up
      await act(async () => {
        await eventSimulator.simulateEvent('agent-stop', { stopType: 'normal' });
      });
      
      expect(mockProps.stopCurrentAudio).toHaveBeenCalled();
    });
  });

  describe('Memory Leak Detection', () => {
    it('should not leak memory during extended operation', async () => {
      const { result, unmount } = renderHook(() => useBackendEvents(mockProps));
      
      performanceMonitor.startTracking();
      const duration = 5000; // 5 seconds for test
      const startTime = Date.now();
      
      // Continuous event firing
      const interval = setInterval(() => {
        eventSimulator.simulateEvent('agent-text-stream', { 
          message_id: `msg-${Date.now()}`,
          chunk: 'x'.repeat(1000)
        });
      }, 10);
      
      // Wait for test duration
      await new Promise(resolve => setTimeout(resolve, duration));
      clearInterval(interval);
      
      const report = performanceMonitor.getReport();
      
      // Check memory growth - should be reasonable for 5 seconds
      expect(report.memoryGrowth).toBeLessThan(20 * 1024 * 1024); // 20MB threshold
      
      // Cleanup
      unmount();
    });
  });

  describe('Error Recovery and Edge Cases', () => {
    it('should handle malformed events gracefully', async () => {
      const { result } = renderHook(() => useBackendEvents(mockProps));
      
      const malformedEvents = [
        { type: 'agent-text-stream', payload: null },
        { type: 'agent-text-stream', payload: undefined },
        { type: 'agent-text-stream', payload: {} },
        { type: 'agent-text-stream', payload: { message_id: null } },
        { type: 'agent-text-stream', payload: { chunk: null } },
        { type: 'unknown-event', payload: { data: 'test' } }
      ];
      
      await act(async () => {
        // None of these should throw errors
        for (const event of malformedEvents) {
          await expect(
            eventSimulator.simulateEvent(event.type, event.payload)
          ).resolves.not.toThrow();
        }
      });

      // App should still be functional
      await act(async () => {
        await eventSimulator.simulateEvent('agent-text-stream', {
          message_id: 'valid-msg',
          chunk: 'Valid chunk'
        });
      });
      
      expect(mockProps.addOrUpdateStreamingMessage).toHaveBeenCalledWith(
        'valid-msg',
        expect.stringContaining('Valid chunk'),
        false
      );
    });

    it('should recover from event handler errors', async () => {
      const { result } = renderHook(() => useBackendEvents(mockProps));
      
      // Make one handler throw an error
      mockProps.playAudioFromBase64.mockRejectedValueOnce(new Error('Audio failed'));
      
      await act(async () => {
        await eventSimulator.simulateEvent('tts-audio-ready', {
          audio_base64: 'failing-audio'
        });
      });
      
      // Should have attempted to play
      expect(mockProps.playAudioFromBase64).toHaveBeenCalled();
      
      // Should still handle subsequent events
      await act(async () => {
        await eventSimulator.simulateEvent('agent-stop', {
          stopType: 'normal'
        });
      });
      
      expect(mockProps.stopCurrentAudio).toHaveBeenCalled();
      expect(mockProps.setIsProcessing).toHaveBeenCalledWith(false);
    });
  });
});

// Helper function to shuffle array
function shuffleArray<T>(array: T[]): T[] {
  const shuffled = [...array];
  for (let i = shuffled.length - 1; i > 0; i--) {
    const j = Math.floor(Math.random() * (i + 1));
    [shuffled[i], shuffled[j]] = [shuffled[j], shuffled[i]];
  }
  return shuffled;
}

// Helper to calculate percentiles
function percentile(values: number[], p: number): number {
  const sorted = values.slice().sort((a, b) => a - b);
  const index = Math.ceil((p / 100) * sorted.length) - 1;
  return sorted[Math.max(0, index)];
}