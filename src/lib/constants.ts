// Frontend Constants for Juno AI Computer Use Agent
// Centralized location for all magic numbers and strings used in the React/TypeScript frontend

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

export const EVENTS = {
  // Tauri events that the frontend listens to
  AGENT_EVENT: 'agent-event',
  APP_DICTATION_STARTED: 'app-dictation-started',
  APP_DICTATION_FINISHED: 'app-dictation-finished',
  APP_DICTATION_PARTIAL_RESULT: 'app-dictation-partial-result',
  APP_DICTATION_ERROR: 'app-dictation-error',

  // Agent events
  AGENT_PROCESSING_COMPLETE: 'agent-processing-complete',
  AGENT_PROCESSING_ERROR: 'agent-processing-error',
  AGENT_STATE_CHANGED: 'agent-state-changed',
  AGENT_TOOL_CALL: 'agent-tool-call',
  AGENT_THOUGHT_PROCESS: 'agent-thought-process',
  AGENT_STOPPING: 'agent-stopping',
  AGENT_STATUS_UPDATE: 'agent-status-update',

  // Streaming events
  AGENT_TEXT_STREAM: 'agent-text-stream',
  AGENT_STREAM_START: 'agent-stream-start',
  AGENT_STREAM_END: 'agent-stream-end',

  // User Message Events
  USER_MESSAGE_SUBMITTED: 'user-message-submitted',

  // UI events
  BAR_STATE_CHANGED: 'bar-state-changed',
  DICTATION_STATE_CHANGED: 'dictation-state-changed',
  REQUEST_AUDIO_PLAYBACK_TEST: 'request-audio-playback-test',

  // Menu and settings events
  SETTINGS_REQUESTED: 'settings-requested',
  DEVTOOLS_REQUESTED: 'devtools-requested',
  PERMISSIONS_REQUESTED: 'permissions-requested',
  FEEDBACK_REQUESTED: 'feedback-requested',
  HELP_REQUESTED: 'help-requested',
  NEW_CHAT_REQUESTED: 'new-chat-requested',
  CLEAR_HISTORY_REQUESTED: 'clear-history-requested',
  IMPORT_CHAT_REQUESTED: 'import-chat-requested',
  EXPORT_CHAT_REQUESTED: 'export-chat-requested',
  TOGGLE_FLOATING_BAR_REQUESTED: 'toggle-floating-bar-requested',
  TOGGLE_DEV_PANEL_REQUESTED: 'toggle-dev-panel-requested',
  TOGGLE_FULLSCREEN_REQUESTED: 'toggle-fullscreen-requested',
  MINIMIZE_WINDOW_REQUESTED: 'minimize-window-requested',
  ZOOM_WINDOW_REQUESTED: 'zoom-window-requested',
  UPDATE_CHECK_REQUESTED: 'update-check-requested',
} as const;

export const WINDOW_LABELS = {
  MAIN: 'main',
  FLOATING_BAR: 'floating-bar',
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

export const HTTP_STATUS = {
  OK: 200,
  CREATED: 201,
  BAD_REQUEST: 400,
  UNAUTHORIZED: 401,
  FORBIDDEN: 403,
  NOT_FOUND: 404,
  INTERNAL_SERVER_ERROR: 500,
} as const;

export const CSS_CLASSES = {
  // Common utility classes
  HIDDEN: 'hidden',
  VISIBLE: 'visible',
  LOADING: 'loading',
  ERROR: 'error',
  SUCCESS: 'success',

  // Animation classes
  FADE_IN: 'fade-in',
  FADE_OUT: 'fade-out',
  SLIDE_IN: 'slide-in',
  SLIDE_OUT: 'slide-out',
} as const;

export const ERROR_MESSAGES = {
  // Generic error messages
  UNKNOWN_ERROR: 'An unknown error occurred',
  NETWORK_ERROR: 'Network connection error',
  TIMEOUT_ERROR: 'Request timed out',
  PERMISSION_DENIED: 'Permission denied',

  // App-specific errors
  VOICE_UNAVAILABLE: 'Voice transcription unavailable',
  AGENT_BUSY: 'Agent is currently processing another request',
  INVALID_COMMAND: 'Invalid command or parameters',
  CLOUD_DISCONNECTED: 'Cloud service disconnected',
} as const;

export const SUCCESS_MESSAGES = {
  SAVE_SUCCESS: 'Successfully saved',
  UPLOAD_SUCCESS: 'Upload completed',
  CONNECTION_SUCCESS: 'Connected successfully',
  SYNC_SUCCESS: 'Synchronized successfully',
} as const;

export const LOCAL_STORAGE_KEYS = {
  USER_PREFERENCES: 'juno_user_preferences',
  CHAT_HISTORY: 'juno_chat_history',
  CLOUD_CONFIG: 'juno_cloud_config',
  VOICE_SETTINGS: 'juno_voice_settings',
  THEME_PREFERENCE: 'juno_theme',
  WINDOW_STATE: 'juno_window_state',
} as const;

export const REGEX_PATTERNS = {
  EMAIL: /^[^\s@]+@[^\s@]+\.[^\s@]+$/,
  URL: /^https?:\/\/.+/,
  JSON: /^[\s]*[{\[]/,
  WHITESPACE_ONLY: /^\s*$/,

  // App-specific patterns
  WAKE_WORD: /^[a-zA-Z\s]{2,20}$/,
  COMMAND_PREFIX: /^[\/!@#]/,
} as const;

export const LIMITS = {
  // Input limitations
  MAX_MESSAGE_LENGTH: 10000,
  MAX_FILENAME_LENGTH: 255,
  MAX_CHAT_HISTORY_ITEMS: 1000,
  MAX_WAKE_WORDS: 10,

  // UI limitations
  MAX_RECENT_FILES: 20,
  MAX_SEARCH_RESULTS: 100,
  MIN_WINDOW_WIDTH: 320,
  MIN_WINDOW_HEIGHT: 240,
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
export type EventName = typeof EVENTS[keyof typeof EVENTS];
export type WindowLabel = typeof WINDOW_LABELS[keyof typeof WINDOW_LABELS];
export type ApiEndpoint = typeof API_ENDPOINTS[keyof typeof API_ENDPOINTS];
export type FileExtension = typeof FILE_EXTENSIONS[keyof typeof FILE_EXTENSIONS];
export type HttpStatus = typeof HTTP_STATUS[keyof typeof HTTP_STATUS];
export type LocalStorageKey = typeof LOCAL_STORAGE_KEYS[keyof typeof LOCAL_STORAGE_KEYS];
export type PermissionType = typeof PERMISSION_TYPES[keyof typeof PERMISSION_TYPES];
export type ChromeDebugPort = typeof CHROME_DEBUG.PRIMARY_PORT | typeof CHROME_DEBUG.ALT_PORT_1 | typeof CHROME_DEBUG.ALT_PORT_2;

// Validation helpers
export const validateEmail = (email: string): boolean => REGEX_PATTERNS.EMAIL.test(email);
export const validateUrl = (url: string): boolean => REGEX_PATTERNS.URL.test(url);
export const validateWakeWord = (word: string): boolean => REGEX_PATTERNS.WAKE_WORD.test(word);

// Utility functions for common operations
export const formatTimeout = (ms: number): string => {
  if (ms < 1000) return `${ms}ms`;
  if (ms < 60000) return `${(ms / 1000).toFixed(1)}s`;
  return `${(ms / 60000).toFixed(1)}m`;
};

export const isValidHttpStatus = (status: number): boolean => {
  return Object.values(HTTP_STATUS).includes(status as any);
};

export const getFileExtension = (filename: string): string => {
  const lastDot = filename.lastIndexOf('.');
  return lastDot === -1 ? '' : filename.substring(lastDot);
};

// Development mode helpers
export const isDevelopment = (): boolean => {
  return import.meta.env.MODE === 'development';
};

export const getDevServerUrl = (): string => {
  return `${API_ENDPOINTS.LOCALHOST_BASE}:${PORTS.VITE_DEV_PORT}`;
};

// Default configurations
export const DEFAULT_CONFIG = {
  theme: 'system',
  language: 'en',
  autoSave: true,
  soundEnabled: true,
  voiceSensitivity: AUDIO.DEFAULT_SENSITIVITY,
  wakeWords: [...AUDIO.DEFAULT_WAKE_WORDS],
  cloudEnabled: false,
  debugMode: false,
} as const;

export type DefaultConfig = typeof DEFAULT_CONFIG;
