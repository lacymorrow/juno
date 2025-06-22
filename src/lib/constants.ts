// Frontend Constants for Juno AI Computer Use Agent
// Frontend-only constants that are NOT generated from Rust
// For Rust-derived constants, see constants.generated.ts

// Re-export generated constants for convenience
export * from './constants.generated';

export const TIMEOUTS = {
	// UI Animation and Debounce
	SOUND_DEBOUNCE_MS: 300,
	HEARTBEAT_INTERVAL_MS: 30000,

	// Cloud connection timeouts
	CLOUD_CONNECTION_TIMEOUT_MS: 10000,
	CLOUD_RECONNECT_DELAY_MS: 5000,
} as const;

export const PORTS = {
	// Development server ports
	VITE_DEV_PORT: 1420,
	VITE_HMR_PORT: 1421,

	// Backend service ports
	MCP_SERVER_PORT: 8080,
	WEBSOCKET_TEST_PORT: 8080,
} as const;

export const UI = {
	// Responsive breakpoints
	MOBILE_BREAKPOINT: 768,

	// CSS values
	PERCENTAGE_MULTIPLIER: 100.0,

	// Animation durations (CSS compatible)
	ANIMATION_FAST_MS: 150,
	ANIMATION_NORMAL_MS: 300,
	ANIMATION_SLOW_MS: 500,
} as const;

export const APP_IDENTITY = {
	APP_NAME: 'Juno',
	PRODUCT_NAME: 'Juno',
	BUNDLE_IDENTIFIER: 'com.juno.app',

	// Cloud service identifiers
	CLOUD_WS_GLOBAL_VAR: '__JUNO_CLOUD_WS',
} as const;

export const WINDOW_LABELS = {
	MAIN: 'main',
	FLOATING_BAR: 'floating-bar',
	FLOATING_PANEL: 'floating-panel',
	ONBOARDING: 'onboarding',
	SETTINGS: 'settings',
} as const;

export const AUDIO = {
	// Audio processing constants
	WHISPER_SAMPLE_RATE: 16000,
	DEFAULT_SENSITIVITY: 0.5,

	// Wake word defaults
	DEFAULT_WAKE_WORDS: ['hey juno', 'computer'],

	// Audio processing constants (matching Rust backend)
	SINC_LENGTH: 256,
	OVERSAMPLING_FACTOR: 256,
	AUDIO_RECV_TIMEOUT_MS: 100,
} as const;

export const API_ENDPOINTS = {
	// AI Provider URLs
	ANTHROPIC_API_URL: 'https://api.anthropic.com/v1/messages',
	OPENAI_API_URL: 'https://api.openai.com/v1/chat/completions',
	GEMINI_API_BASE: 'https://generativelanguage.googleapis.com/v1beta/models',

	// External URLs
	GITHUB_URL: 'https://github.com/juno-ai',

	// Local development URLs
	LOCALHOST_BASE: 'http://localhost',
	get LOCALHOST_MCP_SERVER() { return `${this.LOCALHOST_BASE}:${PORTS.MCP_SERVER_PORT}` },
	get WEBSOCKET_LOCALHOST() { return `ws://localhost:${PORTS.WEBSOCKET_TEST_PORT}` },
} as const;

export const FILE_EXTENSIONS = {
	JSON: '.json',
	TYPESCRIPT: '.ts',
	JAVASCRIPT: '.js',
	MARKDOWN: '.md',
	TEXT: '.txt',
	CSV: '.csv',
} as const;

export const PERMISSION_TYPES = {
	ACCESSIBILITY: 'accessibility',
	SCREEN_RECORDING: 'screen_recording',
	MICROPHONE: 'microphone',
	INPUT_MONITORING: 'input_monitoring',
} as const;

export const CHROME_DEBUG = {
	PRIMARY_PORT: 9222,
	ALT_PORT_1: 9223,
	ALT_PORT_2: 9224,

	// Helper to get all debug URLs
	getAllUrls: () => [
		`${API_ENDPOINTS.LOCALHOST_BASE}:${CHROME_DEBUG.PRIMARY_PORT}`,
		`${API_ENDPOINTS.LOCALHOST_BASE}:${CHROME_DEBUG.ALT_PORT_1}`,
		`${API_ENDPOINTS.LOCALHOST_BASE}:${CHROME_DEBUG.ALT_PORT_2}`,
	],
} as const;

// Type helpers for better TypeScript support
export type WindowLabel = typeof WINDOW_LABELS[keyof typeof WINDOW_LABELS];
export type ApiEndpoint = typeof API_ENDPOINTS[keyof typeof API_ENDPOINTS];
export type FileExtension = typeof FILE_EXTENSIONS[keyof typeof FILE_EXTENSIONS];
export type PermissionType = typeof PERMISSION_TYPES[keyof typeof PERMISSION_TYPES];
export type ChromeDebugPort = typeof CHROME_DEBUG.PRIMARY_PORT | typeof CHROME_DEBUG.ALT_PORT_1 | typeof CHROME_DEBUG.ALT_PORT_2;

// Utility functions for common operations
export const formatTimeout = (ms: number): string => {
	if (ms < 1000) return `${ms}ms`;
	if (ms < 60000) return `${(ms / 1000).toFixed(1)}s`;
	return `${(ms / 60000).toFixed(1)}m`;
};

export const getFileExtension = (filename: string): string => {
	const lastDot = filename.lastIndexOf('.');
	return lastDot === -1 ? '' : filename.substring(lastDot);
};

// Development mode helpers
export const getDevServerUrl = (): string => {
	return `${API_ENDPOINTS.LOCALHOST_BASE}:${PORTS.VITE_DEV_PORT}`;
};
