<<<<<<< HEAD
/// <reference types="vitest" />
import '@testing-library/jest-dom';
import { vi } from 'vitest';

// Mock the window.speechSynthesis API
Object.defineProperty(window, 'speechSynthesis', {
	value: {
		speak: vi.fn(),
		cancel: vi.fn(),
		pause: vi.fn(),
		resume: vi.fn(),
		getVoices: vi.fn().mockReturnValue([]),
	},
	writable: true,
=======
import '@testing-library/jest-dom';

// Mock the window.speechSynthesis API
Object.defineProperty(window, 'speechSynthesis', {
  value: {
    speak: vi.fn(),
    cancel: vi.fn(),
    pause: vi.fn(),
    resume: vi.fn(),
    getVoices: vi.fn().mockReturnValue([]),
  },
  writable: true,
>>>>>>> main
});

// Mock Audio class
global.Audio = class {
<<<<<<< HEAD
	src: string | undefined;
	onended: (() => void) | null = null;
	onerror: ((event: ErrorEvent) => void) | null = null;
	error: MediaError | null = null;

	constructor(src?: string) {
		this.src = src;
	}

	play() {
		return Promise.resolve();
	}
} as unknown as new (src?: string) => HTMLAudioElement;

// Mock navigator.onLine
Object.defineProperty(navigator, 'onLine', {
	writable: true,
	value: true,
});
=======
  src: string;
  onended: (() => void) | null = null;
  onerror: ((event: ErrorEvent) => void) | null = null;
  error: MediaError | null = null;

  constructor(src: string) {
    this.src = src;
  }

  play() {
    return Promise.resolve();
  }
};

// Mock navigator.onLine
Object.defineProperty(navigator, 'onLine', {
  writable: true,
  value: true,
});
>>>>>>> main
