/// <reference types="vitest" />
import '@testing-library/jest-dom/vitest';
import { vi } from 'vitest';

// jsdom implements none of the audio APIs. These stubs are environment
// scaffolding, not an endorsement of any particular path: Juno prefers Rust for
// audio, but the browser APIs stay available, so anything that reaches for one
// has somewhere to land instead of throwing "not implemented".
Object.defineProperty(window, 'speechSynthesis', {
	value: {
		speak: vi.fn(),
		cancel: vi.fn(),
		pause: vi.fn(),
		resume: vi.fn(),
		getVoices: vi.fn().mockReturnValue([]),
	},
	writable: true,
});

global.Audio = class {
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

Object.defineProperty(navigator, 'onLine', {
	writable: true,
	value: true,
});

// jsdom has no ResizeObserver; use-stick-to-bottom (the chat Conversation
// container) requires one. A no-op stub is enough for layout-free tests.
if (typeof globalThis.ResizeObserver === 'undefined') {
	globalThis.ResizeObserver = class {
		observe() {}
		unobserve() {}
		disconnect() {}
	} as unknown as typeof ResizeObserver;
}
