<<<<<<< HEAD
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { synthesizeSpeech } from './ttsService';
=======
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { synthesizeSpeech, TTSMode } from './ttsService';
>>>>>>> main

describe('ttsService', () => {
  // Mock functions
  const mockLogFn = vi.fn();
  const mockInvokeFn = vi.fn();
<<<<<<< HEAD

  // Mock Audio class methods
  const mockAudioPlay = vi.fn().mockResolvedValue(undefined);
  const originalAudio = global.Audio;

  beforeEach(() => {
    // Reset mocks before each test
    vi.clearAllMocks();

=======
  
  // Mock Audio class methods
  const mockAudioPlay = vi.fn().mockResolvedValue(undefined);
  const originalAudio = global.Audio;
  
  beforeEach(() => {
    // Reset mocks before each test
    vi.clearAllMocks();
    
>>>>>>> main
    // Mock Audio class
    global.Audio = vi.fn().mockImplementation((src) => ({
      src,
      play: mockAudioPlay,
      onended: null,
      onerror: null,
      error: null
    }));
<<<<<<< HEAD

=======
    
>>>>>>> main
    // Mock SpeechSynthesisUtterance
    global.SpeechSynthesisUtterance = vi.fn().mockImplementation((text) => ({
      text,
      onend: null,
      onerror: null
    }));
<<<<<<< HEAD

    // Set navigator.onLine to true by default
    Object.defineProperty(navigator, 'onLine', { value: true, writable: true });
  });

=======
    
    // Set navigator.onLine to true by default
    Object.defineProperty(navigator, 'onLine', { value: true, writable: true });
  });
  
>>>>>>> main
  afterEach(() => {
    // Restore original Audio class
    global.Audio = originalAudio;
  });
<<<<<<< HEAD

  describe('synthesizeSpeech', () => {
    it('should handle empty text', async () => {
      await synthesizeSpeech('', 'local', mockLogFn, mockInvokeFn);

=======
  
  describe('synthesizeSpeech', () => {
    it('should handle empty text', async () => {
      await synthesizeSpeech('', 'local', mockLogFn, mockInvokeFn);
      
>>>>>>> main
      expect(mockLogFn).toHaveBeenCalledWith('Synthesize speech called with empty text.', 'warn');
      expect(window.speechSynthesis.speak).not.toHaveBeenCalled();
      expect(mockInvokeFn).not.toHaveBeenCalled();
    });
<<<<<<< HEAD

=======
    
>>>>>>> main
    it('should use local speech synthesis when mode is local', async () => {
      // Create a promise that resolves when the utterance's onend is called
      let resolveUtteranceEnd: () => void;
      const utteranceEndPromise = new Promise<void>((resolve) => {
        resolveUtteranceEnd = resolve;
      });
<<<<<<< HEAD

=======
      
>>>>>>> main
      // Mock SpeechSynthesisUtterance to capture the onend callback
      global.SpeechSynthesisUtterance = vi.fn().mockImplementation((text) => {
        return {
          text,
          set onend(callback: () => void) {
            // Store the callback to call it later
            setTimeout(() => {
              callback();
              resolveUtteranceEnd();
            }, 10);
          },
          onerror: null
        };
      });
<<<<<<< HEAD

      // Call the function
      const promise = synthesizeSpeech('Hello world', 'local', mockLogFn, mockInvokeFn);

      // Wait for the utterance end event to be triggered
      await utteranceEndPromise;
      await promise;

=======
      
      // Call the function
      const promise = synthesizeSpeech('Hello world', 'local', mockLogFn, mockInvokeFn);
      
      // Wait for the utterance end event to be triggered
      await utteranceEndPromise;
      await promise;
      
>>>>>>> main
      // Verify the correct functions were called
      expect(window.speechSynthesis.speak).toHaveBeenCalled();
      expect(mockLogFn).toHaveBeenCalledWith('Attempting local speech: "Hello world"', 'info');
      expect(mockLogFn).toHaveBeenCalledWith('Local speech finished.', 'info');
      expect(mockInvokeFn).not.toHaveBeenCalled();
    });
<<<<<<< HEAD

    it('should throw an error when offline and mode is api', async () => {
      // Set navigator.onLine to false
      Object.defineProperty(navigator, 'onLine', { value: false });

      await expect(synthesizeSpeech('Hello world', 'api', mockLogFn, mockInvokeFn))
        .rejects.toThrow('Offline. Cannot use API TTS.');

      expect(mockLogFn).toHaveBeenCalledWith('Offline. Cannot use API TTS.', 'warn');
      expect(mockInvokeFn).not.toHaveBeenCalled();
    });

    it('should use API speech synthesis when mode is api and online', async () => {
      // Mock the invoke function to return an audio URL
      mockInvokeFn.mockResolvedValue('https://example.com/audio.mp3');

=======
    
    it('should throw an error when offline and mode is api', async () => {
      // Set navigator.onLine to false
      Object.defineProperty(navigator, 'onLine', { value: false });
      
      await expect(synthesizeSpeech('Hello world', 'api', mockLogFn, mockInvokeFn))
        .rejects.toThrow('Offline. Cannot use API TTS.');
      
      expect(mockLogFn).toHaveBeenCalledWith('Offline. Cannot use API TTS.', 'warn');
      expect(mockInvokeFn).not.toHaveBeenCalled();
    });
    
    it('should use API speech synthesis when mode is api and online', async () => {
      // Mock the invoke function to return an audio URL
      mockInvokeFn.mockResolvedValue('https://example.com/audio.mp3');
      
>>>>>>> main
      // Create a promise that resolves when the audio's onended is called
      let resolveAudioEnd: () => void;
      const audioEndPromise = new Promise<void>((resolve) => {
        resolveAudioEnd = resolve;
      });
<<<<<<< HEAD

=======
      
>>>>>>> main
      // Mock Audio to capture the onended callback
      global.Audio = vi.fn().mockImplementation((src) => {
        return {
          src,
          play: () => {
            return Promise.resolve();
          },
          set onended(callback: () => void) {
            // Store the callback to call it later
            setTimeout(() => {
              callback();
              resolveAudioEnd();
            }, 10);
          },
          onerror: null,
          error: null
        };
      });
<<<<<<< HEAD

      // Call the function
      const promise = synthesizeSpeech('Hello world', 'api', mockLogFn, mockInvokeFn);

      // Wait for the audio end event to be triggered
      await audioEndPromise;
      await promise;

=======
      
      // Call the function
      const promise = synthesizeSpeech('Hello world', 'api', mockLogFn, mockInvokeFn);
      
      // Wait for the audio end event to be triggered
      await audioEndPromise;
      await promise;
      
>>>>>>> main
      // Verify the correct functions were called
      expect(mockInvokeFn).toHaveBeenCalledWith('invoke_replicate_tts', { text: 'Hello world' });
      expect(mockLogFn).toHaveBeenCalledWith('Attempting API speech: "Hello world"', 'info');
      expect(mockLogFn).toHaveBeenCalledWith('Received audio URL from backend: https://example.com/audio.mp3', 'info');
      expect(mockLogFn).toHaveBeenCalledWith('API audio playback started.', 'info');
      expect(mockLogFn).toHaveBeenCalledWith('API audio playback finished.', 'info');
      expect(global.Audio).toHaveBeenCalledWith('https://example.com/audio.mp3');
    });
<<<<<<< HEAD

    it('should handle API errors', async () => {
      // Mock the invoke function to throw an error
      mockInvokeFn.mockRejectedValue(new Error('API error'));

      await expect(synthesizeSpeech('Hello world', 'api', mockLogFn, mockInvokeFn))
        .rejects.toThrow('API error');

      expect(mockLogFn).toHaveBeenCalledWith('Attempting API speech: "Hello world"', 'info');
      expect(mockLogFn).toHaveBeenCalledWith(expect.stringContaining('Error invoking or handling Replicate TTS backend'), 'error');
    });

    it('should handle empty audio URL from backend', async () => {
      // Mock the invoke function to return an empty string
      mockInvokeFn.mockResolvedValue('');

      await expect(synthesizeSpeech('Hello world', 'api', mockLogFn, mockInvokeFn))
        .rejects.toThrow('Backend returned empty audio URL.');

      expect(mockLogFn).toHaveBeenCalledWith('Attempting API speech: "Hello world"', 'info');
      expect(mockLogFn).toHaveBeenCalledWith('Backend returned empty audio URL.', 'error');
    });

    it('should handle audio playback errors', async () => {
      // Mock the invoke function to return an audio URL
      mockInvokeFn.mockResolvedValue('https://example.com/audio.mp3');

=======
    
    it('should handle API errors', async () => {
      // Mock the invoke function to throw an error
      mockInvokeFn.mockRejectedValue(new Error('API error'));
      
      await expect(synthesizeSpeech('Hello world', 'api', mockLogFn, mockInvokeFn))
        .rejects.toThrow('API error');
      
      expect(mockLogFn).toHaveBeenCalledWith('Attempting API speech: "Hello world"', 'info');
      expect(mockLogFn).toHaveBeenCalledWith(expect.stringContaining('Error invoking or handling Replicate TTS backend'), 'error');
    });
    
    it('should handle empty audio URL from backend', async () => {
      // Mock the invoke function to return an empty string
      mockInvokeFn.mockResolvedValue('');
      
      await expect(synthesizeSpeech('Hello world', 'api', mockLogFn, mockInvokeFn))
        .rejects.toThrow('Backend returned empty audio URL.');
      
      expect(mockLogFn).toHaveBeenCalledWith('Attempting API speech: "Hello world"', 'info');
      expect(mockLogFn).toHaveBeenCalledWith('Backend returned empty audio URL.', 'error');
    });
    
    it('should handle audio playback errors', async () => {
      // Mock the invoke function to return an audio URL
      mockInvokeFn.mockResolvedValue('https://example.com/audio.mp3');
      
>>>>>>> main
      // Mock Audio to simulate an error during playback
      global.Audio = vi.fn().mockImplementation((src) => {
        return {
          src,
          play: () => {
            return Promise.reject(new Error('Audio playback failed'));
          },
          onended: null,
          onerror: null,
          error: { code: 3, message: 'MEDIA_ERR_DECODE' }
        };
      });
<<<<<<< HEAD

      await expect(synthesizeSpeech('Hello world', 'api', mockLogFn, mockInvokeFn))
        .rejects.toThrow('Audio playback failed');

=======
      
      await expect(synthesizeSpeech('Hello world', 'api', mockLogFn, mockInvokeFn))
        .rejects.toThrow('Audio playback failed');
      
>>>>>>> main
      expect(mockInvokeFn).toHaveBeenCalledWith('invoke_replicate_tts', { text: 'Hello world' });
      expect(mockLogFn).toHaveBeenCalledWith('Attempting API speech: "Hello world"', 'info');
      expect(mockLogFn).toHaveBeenCalledWith('Received audio URL from backend: https://example.com/audio.mp3', 'info');
      expect(mockLogFn).toHaveBeenCalledWith(expect.stringContaining('Initial API audio play() error'), 'error');
    });
  });
<<<<<<< HEAD
});
=======
});
>>>>>>> main
