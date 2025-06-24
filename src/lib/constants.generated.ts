// Generated file - do not edit manually
// This file is auto-generated from Rust constants
// Run 'npm run generate-constants' to update

export const EVENTS = {
  AGENT_EVENT: 'agent-event',
  AGENT_PROCESSING_COMPLETE: 'agent-processing-complete',
  AGENT_PROCESSING_ERROR: 'agent-processing-error',
  AGENT_STATE_CHANGED: 'agent-state-changed',
  AGENT_TOOL_CALL: 'agent-tool-call',
  AGENT_THOUGHT_PROCESS: 'agent-thought-process',
  AGENT_STOPPING: 'agent-stopping',
  AGENT_STATUS_UPDATE: 'agent-status-update',
  AGENT_ACTIVE: 'agent-active',
  AGENT_ERROR: 'agent-error',
  AGENT_TRANSCRIPTION_START: 'agent-transcription-start',
  AGENT_TRANSCRIPTION_STOP: 'agent-transcription-stop',
  AGENT_CANCEL: 'agent-cancel',
  AGENT_COMMITTED: 'agent-committed',
  AGENT_FORCE_STOP: 'agent-force-stop',
  AGENT_FORCE_CLEANUP: 'agent-force-cleanup',
  STREAMING_TEXT_STREAM: 'agent-text-stream',
  STREAMING_STREAM_START: 'agent-stream-start',
  STREAMING_STREAM_END: 'agent-stream-end',
  DICTATION_STARTED: 'app-dictation-started',
  DICTATION_FINISHED: 'app-dictation-finished',
  DICTATION_PARTIAL_RESULT: 'app-dictation-partial-result',
  DICTATION_ERROR: 'app-dictation-error',
  DICTATION_STATE_CHANGED: 'dictation-state-changed',
  DICTATION_ACTIVE: 'dictation-active',
  DICTATION_CANCELLED: 'dictation-cancelled',
  DICTATION_TRANSCRIPTION_START: 'dictation-transcription-start',
  DICTATION_TRANSCRIPTION_STOP: 'dictation-transcription-stop',
  DICTATION_COMMITTED: 'dictation-committed',
  DICTATION_STOP: 'dictation-stop',
  DICTATION_TRANSCRIPTION_CANCEL: 'dictation-transcription-cancel',
  DICTATION_TRANSCRIPTION_FORCE_STOP: 'dictation-transcription-force-stop',
  DICTATION_TRANSCRIPTION_FORCE_CLEANUP: 'dictation-transcription-force-cleanup',
  UI_BAR_STATE_CHANGED: 'bar-state-changed',
  UI_REQUEST_AUDIO_PLAYBACK_TEST: 'request-audio-playback-test',
  UI_KEY_PRESS_VISUALIZATION: 'key-press-visualization',
  UI_CLICK_VISUALIZATION: 'click-visualization',
  UI_UI_CURSOR_HIGHLIGHT_START: 'ui-cursor-highlight-start',
  UI_UI_CURSOR_HIGHLIGHT_MOVE: 'ui-cursor-highlight-move',
  UI_UI_CURSOR_HIGHLIGHT_STOP: 'ui-cursor-highlight-stop',
  MENU_SETTINGS_REQUESTED: 'settings-requested',
  MENU_DEVTOOLS_REQUESTED: 'devtools-requested',
  MENU_PERMISSIONS_REQUESTED: 'permissions-requested',
  MENU_FEEDBACK_REQUESTED: 'feedback-requested',
  MENU_HELP_REQUESTED: 'help-requested',
  MENU_NEW_CHAT_REQUESTED: 'new-chat-requested',
  MENU_CLEAR_HISTORY_REQUESTED: 'clear-history-requested',
  MENU_IMPORT_CHAT_REQUESTED: 'import-chat-requested',
  MENU_EXPORT_CHAT_REQUESTED: 'export-chat-requested',
  MENU_TOGGLE_FLOATING_BAR_REQUESTED: 'toggle-floating-bar-requested',
  MENU_TOGGLE_DEV_PANEL_REQUESTED: 'toggle-dev-panel-requested',
  MENU_TOGGLE_FULLSCREEN_REQUESTED: 'toggle-fullscreen-requested',
  MENU_MINIMIZE_WINDOW_REQUESTED: 'minimize-window-requested',
  MENU_ZOOM_WINDOW_REQUESTED: 'zoom-window-requested',
  MENU_UPDATE_CHECK_REQUESTED: 'update-check-requested',
  MENU_ABOUT_REQUESTED: 'about-requested',
  TTS_AUDIO_READY: 'tts-audio-ready',
  TTS_STOP_REQUESTED: 'tts-stop-requested',
  ALWAYS_LISTENING_MODE_CHANGED: 'always-listening-mode-changed',
  ALWAYS_LISTENING_WAKE_WORD_DETECTED: 'always-listening:wake-word-detected',
  ALWAYS_LISTENING_TOGGLE_DICTATION_REQUEST: 'toggle-dictation-request',
  PERMISSIONS_CHANGED: 'permissions-changed',
  PERMISSIONS_RESTART_REQUIRED: 'permissions-restart-required',
  DEV_TOOL_NOTIFICATION: 'dev-tool-notification',
  MESSAGES_USER_MESSAGE_SUBMITTED: 'user-message-submitted',
} as const;

export const TIMEOUTS = {
  MICRO_DELAY_MS: 10,
  MINIMAL_DELAY_MS: 20,
  SMALL_DELAY_MS: 50,
  SHORT_DELAY_MS: 100,
  MEDIUM_DELAY_MS: 150,
  ANIMATION_DELAY_MS: 300,
  STANDARD_DELAY_MS: 500,
  LONG_DELAY_MS: 800,
  VERY_LONG_DELAY_MS: 1000,
  EXTENDED_DELAY_MS: 2000,
  MAX_DELAY_MS: 3000,
  SOUND_DEBOUNCE_MS: 300,
  HEARTBEAT_INTERVAL_MS: 30000,
  CLOUD_CONNECTION_TIMEOUT_MS: 10000,
  CLOUD_RECONNECT_DELAY_MS: 5000,
  ANIMATION_FAST_MS: 150,
  ANIMATION_NORMAL_MS: 300,
  ANIMATION_SLOW_MS: 500,
  STANDARD_TIMEOUT_SECONDS: 10,
  BROWSER_TIMEOUT_SECONDS: 30,
  NETWORK_TIMEOUT_SECONDS: 30,
  HEARTBEAT_INTERVAL_SECONDS: 30,
  STATUS_UPDATE_INTERVAL_SECONDS: 30,
  ERROR_RECOVERY_MAX_RETRY_DELAY_SECONDS: 10,
  ERROR_RECOVERY_TIMEOUT_THRESHOLD_SECONDS: 30,
  ERROR_RECOVERY_WAIT_SHORT_SECONDS: 3,
  ERROR_RECOVERY_WAIT_LONG_SECONDS: 10,
  TESTING_HUMAN_AVERAGE_SECONDS: 480,
  TESTING_AGENT_AVERAGE_SECONDS: 120,
  TESTING_RESPONSE_TIME_LIMIT_SECONDS: 30,
  TESTING_QA_TIMEOUT_SECONDS: 60,
  VOICE_CACHE_VALIDITY_SECONDS: 30,
  VOICE_STATE_CHECK_SECONDS: 2,
  VOICE_WAKE_DETECTION_SECONDS: 60,
  CLOUD_HEARTBEAT_INTERVAL_SECONDS: 30,
  CLOUD_STATUS_INTERVAL_SECONDS: 30,
  CLOUD_WATCHDOG_INTERVAL_SECONDS: 60,
  CLOUD_MAX_RETRY_DELAY_SECONDS: 300,
  MCP_OPERATION_TIMEOUT_SECONDS: 5,
  MCP_GRACEFUL_SHUTDOWN_SECONDS: 3,
  MCP_MAX_BACKOFF_DELAY_SECONDS: 30,
  MCP_SERVER_STARTUP_TIMEOUT_SECONDS: 45,
  ORCHESTRATOR_PARALLEL_EXECUTION_TIMEOUT_SECONDS: 5,
  ORCHESTRATOR_MIN_TIMEOUT_SECONDS: 30,
  ORCHESTRATOR_MAX_TIMEOUT_SECONDS: 600,
  AGENT_STEP_DELAY_SECONDS: 1,
  BROWSER_CONNECTION_TIMEOUT_SECONDS: 8,
  BROWSER_PAGE_OPERATION_TIMEOUT_SECONDS: 2,
  BROWSER_CLICK_TIMEOUT_SECONDS: 1,
  DESKTOP_TYPING_TIMEOUT_SECONDS: 30,
  VISUAL_PROCESSING_TIMEOUT_SECONDS: 10,
  VISUAL_TEMPORAL_CONTEXT_SECONDS: 30,
  COLLABORATIVE_AI_KNOWLEDGE_RETRIEVAL_SECONDS: 30,
  COLLABORATIVE_AI_COORDINATION_SECONDS: 60,
  STANDARD_TIMEOUT_MS: 10000,
  BROWSER_TIMEOUT_MS: 30000,
  DICTATION_MONITOR_INTERVAL_MS: 50,
  AGENT_MONITOR_INTERVAL_MS: 100,
  TREE_SEARCH_INTERVAL_MS: 250,
  MOUSE_MICRO_DELAY_MS: 10,
  MOUSE_CLICK_DELAY_MS: 50,
  MOUSE_ACTION_DELAY_MS: 100,
  MOUSE_SEQUENCE_DELAY_MS: 300,
  DOUBLE_CLICK_DELAY_MS: 500,
  UI_FADE_DELAY_MS: 300,
  UI_SLIDE_DELAY_MS: 600,
  UI_NOTIFICATION_DISPLAY_MS: 3000,
  PERMISSION_CHECK_DELAY_MS: 1000,
  SCREEN_RECORDING_CHECK_DELAY_MS: 2000,
  SYSTEM_SETTINGS_OPERATION_TIMEOUT_MS: 3000,
  SYSTEM_SETTINGS_CHECK_TIMEOUT_MS: 5000,
  MCP_SERVER_STARTUP_DELAY_MS: 500,
  MCP_SERVER_RESTART_DELAY_MS: 1000,
  CLOUD_RETRY_BASE_DELAY_MS: 2000,
  CLOUD_HEARTBEAT_INTERVAL_MS: 30000,
  CLOUD_STATUS_INTERVAL_MS: 30000,
  TTS_PROCESSING_DELAY_MS: 1000,
  PARTIAL_BUFFER_DURATION_MS: 1500,
  FINAL_BUFFER_DURATION_MS: 5000,
  MIN_AUDIO_LENGTH_MS: 500,
  DEFAULT_NAVIGATION_TIMEOUT_MS: 30000,
  REPLICATE_TIMEOUT_SECONDS: 300,
  PERMISSION_CHECK_TIMEOUT_MS: 3000,
  AUDIO_DEVICE_DETECTION_TIMEOUT_MS: 3000,
  TOOL_EXECUTION_TIMEOUT_MS: 10000,
  MCP_INTEGRATION_TIMEOUT_MS: 30000,
  BROWSER_PAGE_LOAD_DELAY_MS: 1000,
  SHELL_COMMAND_DELAY_MS: 10,
} as const;

export const PORTS = {
  VITE_DEV_PORT: 1420,
  VITE_HMR_PORT: 1421,
  MCP_SERVER_PORT: 8080,
  WEBSOCKET_TEST_PORT: 8080,
  CHROME_DEBUG_PORT_PRIMARY: 9222,
  CHROME_DEBUG_PORT_ALT1: 9223,
  CHROME_DEBUG_PORT_ALT2: 9224,
} as const;

export const API_ENDPOINTS = {
  ENDPOINTS_ANTHROPIC_API_URL: 'https://api.anthropic.com/v1/messages',
  ENDPOINTS_OPENAI_API_URL: 'https://api.openai.com/v1/chat/completions',
  ENDPOINTS_GEMINI_API_BASE: 'https://generativelanguage.googleapis.com/v1beta/models',
  ENDPOINTS_CLOUD_SERVER_URL: 'wss://juno-cloud-backend.fly.dev/ws',
  ENDPOINTS_GITHUB_URL: 'https://github.com/juno-ai',
  ENDPOINTS_LOCALHOST_BASE: 'http://localhost',
  ENDPOINTS_LOCALHOST_CHROME_DEBUG: 'http://localhost:9222',
  ENDPOINTS_LOCALHOST_MCP_SERVER: 'http://localhost:8080',
  ENDPOINTS_WEBSOCKET_LOCALHOST: 'ws://localhost:8080',
  ENDPOINTS_ELEVENLABS_TTS_BASE: 'https://api.elevenlabs.io/v1/text-to-speech',
  ENDPOINTS_REPLICATE_API_BASE: 'https://api.replicate.com',
  ENDPOINTS_JUNO_CLOUD_WEBSOCKET: 'wss://juno-cloud-backend.fly.dev/ws',
  ENDPOINTS_DEV_SERVER_BASE: 'http://localhost:1420',
  ENDPOINTS_HMR_WEBSOCKET: 'ws://localhost:1421',
  HTTP_HEADERS_CONTENT_TYPE: 'Content-Type',
  HTTP_HEADERS_X_API_KEY: 'x-api-key',
  HTTP_HEADERS_APPLICATION_JSON: 'application/json',
  HTTP_HEADERS_AUTHORIZATION: 'Authorization',
  HTTP_HEADERS_USER_AGENT: 'User-Agent',
  ANTHROPIC_CONTENT_TYPES_MESSAGE_START: 'message_start',
  ANTHROPIC_CONTENT_TYPES_CONTENT_BLOCK_START: 'content_block_start',
  ANTHROPIC_CONTENT_TYPES_CONTENT_BLOCK_DELTA: 'content_block_delta',
  ANTHROPIC_CONTENT_TYPES_CONTENT_BLOCK_STOP: 'content_block_stop',
  ANTHROPIC_CONTENT_TYPES_TEXT_DELTA: 'text_delta',
  ANTHROPIC_CONTENT_TYPES_INPUT_JSON_DELTA: 'input_json_delta',
  ANTHROPIC_CONTENT_TYPES_TOOL_USE: 'tool_use',
  ANTHROPIC_CONTENT_TYPES_TOOL_RESULT: 'tool_result',
  ANTHROPIC_CONTENT_TYPES_TEXT: 'text',
  PROVIDER_NAMES_ANTHROPIC: 'anthropic',
  PROVIDER_NAMES_OPENAI: 'openai',
  PROVIDER_NAMES_GEMINI: 'gemini',
  PROVIDER_NAMES_ELEVENLABS: 'elevenlabs',
  PROVIDER_NAMES_REPLICATE: 'replicate',
  PROVIDER_NAMES_SYSTEM: 'system',
  CLOUD_NETWORKING_MAX_CONNECTION_RETRIES: 10,
  CLOUD_NETWORKING_BASE_RETRY_DELAY_MS: 2000,
  CLOUD_NETWORKING_BACKOFF_MULTIPLIER: 2,
  CLOUD_NETWORKING_MAX_BACKOFF_EXPONENT: 5,
  CLOUD_NETWORKING_CONNECTION_CHECK_INTERVAL_MS: 5000,
  CLOUD_NETWORKING_WATCHDOG_TIMEOUT_MS: 60000,
  CLOUD_NETWORKING_MAX_RETRY_INTERVAL_MS: 300000,
  CLOUD_NETWORKING_HEARTBEAT_SEND_INTERVAL_MS: 30000,
  CLOUD_NETWORKING_STATUS_CHECK_INTERVAL_MS: 30000,
  CLOUD_NETWORKING_RECONNECTION_DELAY_MS: 5000,
  ANTHROPIC_API_URL: 'https://api.anthropic.com/v1/messages',
  OPENAI_API_URL: 'https://api.openai.com/v1/chat/completions',
  GEMINI_API_BASE: 'https://generativelanguage.googleapis.com/v1beta/models',
  CLOUD_SERVER_URL: 'wss://juno-cloud-backend.fly.dev/ws',
  GITHUB_URL: 'https://github.com/juno-ai',
  LOCALHOST_BASE: 'http://localhost',
  LOCALHOST_CHROME_DEBUG: 'http://localhost:9222',
  LOCALHOST_MCP_SERVER: 'http://localhost:8080',
  WEBSOCKET_LOCALHOST: 'ws://localhost:8080',
  ELEVENLABS_TTS_BASE: 'https://api.elevenlabs.io/v1/text-to-speech',
  REPLICATE_API_BASE: 'https://api.replicate.com',
  JUNO_CLOUD_WEBSOCKET: 'wss://juno-cloud-backend.fly.dev/ws',
  DEV_SERVER_BASE: 'http://localhost:1420',
  HMR_WEBSOCKET: 'ws://localhost:1421',
  CONTENT_TYPE: 'Content-Type',
  X_API_KEY: 'x-api-key',
  APPLICATION_JSON: 'application/json',
  AUTHORIZATION: 'Authorization',
  USER_AGENT: 'User-Agent',
  MESSAGE_START: 'message_start',
  CONTENT_BLOCK_START: 'content_block_start',
  CONTENT_BLOCK_DELTA: 'content_block_delta',
  CONTENT_BLOCK_STOP: 'content_block_stop',
  TEXT_DELTA: 'text_delta',
  INPUT_JSON_DELTA: 'input_json_delta',
  TOOL_USE: 'tool_use',
  TOOL_RESULT: 'tool_result',
  TEXT: 'text',
  ANTHROPIC: 'anthropic',
  OPENAI: 'openai',
  GEMINI: 'gemini',
  ELEVENLABS: 'elevenlabs',
  REPLICATE: 'replicate',
  SYSTEM: 'system',
  MAX_CONNECTION_RETRIES: 10,
  BASE_RETRY_DELAY_MS: 2000,
  BACKOFF_MULTIPLIER: 2,
  MAX_BACKOFF_EXPONENT: 5,
  CONNECTION_CHECK_INTERVAL_MS: 5000,
  WATCHDOG_TIMEOUT_MS: 60000,
  MAX_RETRY_INTERVAL_MS: 300000,
  HEARTBEAT_SEND_INTERVAL_MS: 30000,
  STATUS_CHECK_INTERVAL_MS: 30000,
  RECONNECTION_DELAY_MS: 5000,
} as const;

export const APP_IDENTITY = {
  APP_NAME: 'Juno',
  BUNDLE_IDENTIFIER: 'com.juno.app',
  PRODUCT_NAME: 'Juno',
  ENTITLEMENTS_FILE: 'juno.entitlements',
  CONFIG_DIR_NAME: '.juno',
  SCREENSHOT_PREFIX: 'juno_screenshot_',
  DEVICE_NAME_PREFIX: 'Juno-',
  CLOUD_WS_GLOBAL_VAR: '__JUNO_CLOUD_WS',
} as const;

export const UI = {
  WINDOW_LABELS_MAIN: 'main',
  WINDOW_LABELS_FLOATING_BAR: 'floating-bar',
  WINDOW_LABELS_FLOATING_PANEL: 'floating-panel',
  WINDOW_LABELS_ONBOARDING: 'onboarding',
  WINDOW_LABELS_SETTINGS: 'settings',
  MAIN: 'main',
  FLOATING_BAR: 'floating-bar',
  FLOATING_PANEL: 'floating-panel',
  ONBOARDING: 'onboarding',
  SETTINGS: 'settings',
  MOBILE_BREAKPOINT: 768,
  PERCENTAGE_MULTIPLIER: 100,
  SCROLL_WHEEL_EVENT_LINE_SCROLL: 120,
  DOUBLE_CLICK_INTERVAL_MS: 50,
  MAX_TREE_SEARCH_DEPTH: 100,
} as const;

export const AUDIO = {
  PROCESSING_SINC_LENGTH: 256,
  PROCESSING_OVERSAMPLING_FACTOR: 256,
  PROCESSING_AUDIO_RECV_TIMEOUT_MS: 100,
  WHISPER_SAMPLE_RATE: 16000,
  SOUND_DEBOUNCE_MS: 300,
  DEFAULT_SENSITIVITY: 0.5,
  AUDIO_RECV_TIMEOUT_MS: 100,
  DEFAULT_WAKE_WORDS: ['hey juno', 'computer'],
  SINC_LENGTH: 256,
  OVERSAMPLING_FACTOR: 256,
} as const;

export const FILE_EXTENSIONS = {
  EXTENSIONS_JSON: '.json',
  EXTENSIONS_RUST: '.rs',
  EXTENSIONS_TYPESCRIPT: '.ts',
  EXTENSIONS_JAVASCRIPT: '.js',
  EXTENSIONS_MARKDOWN: '.md',
  EXTENSIONS_LOGENSION: '.log',
  EXTENSIONS_TMPENSION: '.tmp',
  EXTENSIONS_CACHEENSION: '.cache',
  EXTENSIONS_BACKUPENSION: '.backup',
  EXTENSIONS_TEXT: '.txt',
  EXTENSIONS_CSV: '.csv',
  JSON: '.json',
  RUST: '.rs',
  TYPESCRIPT: '.ts',
  JAVASCRIPT: '.js',
  MARKDOWN: '.md',
  LOGENSION: '.log',
  TMPENSION: '.tmp',
  CACHEENSION: '.cache',
  BACKUPENSION: '.backup',
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
  PRIMARY: 9222,
  ALT1: 9223,
  ALT2: 9224,
} as const;

export const WINDOW_LABELS = {
  MAIN: 'main',
  FLOATING_BAR: 'floating-bar',
  FLOATING_PANEL: 'floating-panel',
  ONBOARDING: 'onboarding',
  SETTINGS: 'settings',
} as const;

// Type helpers
export type EventName = typeof EVENTS[keyof typeof EVENTS];
export type WindowLabel = typeof WINDOW_LABELS[keyof typeof WINDOW_LABELS];
export type ApiEndpoint = typeof API_ENDPOINTS[keyof typeof API_ENDPOINTS];
export type FileExtension = typeof FILE_EXTENSIONS[keyof typeof FILE_EXTENSIONS];
export type PermissionType = typeof PERMISSION_TYPES[keyof typeof PERMISSION_TYPES];
export type ChromeDebugPort = typeof CHROME_DEBUG[keyof typeof CHROME_DEBUG];

// Frontend-specific constants (not duplicated from Rust)
export const CSS_CLASSES = {
  HIDDEN: 'hidden',
  VISIBLE: 'visible',
  LOADING: 'loading',
  ERROR: 'error',
  SUCCESS: 'success',
  FADE_IN: 'fade-in',
  FADE_OUT: 'fade-out',
  SLIDE_IN: 'slide-in',
  SLIDE_OUT: 'slide-out',
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
  WAKE_WORD: /^[a-zA-Z\s]{2,20}$/,
  COMMAND_PREFIX: /^[\/!@#]/,
} as const;

export const LIMITS = {
  MAX_MESSAGE_LENGTH: 10000,
  MAX_FILENAME_LENGTH: 255,
  MAX_CHAT_HISTORY_ITEMS: 1000,
  MAX_WAKE_WORDS: 10,
  MAX_RECENT_FILES: 20,
  MAX_SEARCH_RESULTS: 100,
  MIN_WINDOW_WIDTH: 320,
  MIN_WINDOW_HEIGHT: 240,
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

export const ERROR_MESSAGES = {
  UNKNOWN_ERROR: 'An unknown error occurred',
  NETWORK_ERROR: 'Network connection error',
  TIMEOUT_ERROR: 'Request timed out',
  PERMISSION_DENIED: 'Permission denied',
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

// Validation helpers
export const validateEmail = (email: string): boolean => REGEX_PATTERNS.EMAIL.test(email);
export const validateUrl = (url: string): boolean => REGEX_PATTERNS.URL.test(url);
export const validateWakeWord = (word: string): boolean => REGEX_PATTERNS.WAKE_WORD.test(word);

// Development mode helpers
export const isDevelopment = (): boolean => {
  try {
    return typeof window !== 'undefined' &&
           // @ts-ignore - Vite environment variable
           (window as any).import?.meta?.env?.MODE === 'development';
  } catch {
    return false;
  }
};

// Default configuration
export const DEFAULT_CONFIG = {
  theme: 'system',
  language: 'en',
  autoSave: true,
  soundEnabled: true,
  voiceSensitivity: 0.5,
  wakeWords: ['hey juno', 'computer'],
  cloudEnabled: false,
  debugMode: false,
} as const;

export type DefaultConfig = typeof DEFAULT_CONFIG;

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

// Chrome debug URL helpers
export const getChromeDebugUrls = (): string[] => [
  `${API_ENDPOINTS.LOCALHOST_BASE}:${CHROME_DEBUG.PRIMARY}`,
  `${API_ENDPOINTS.LOCALHOST_BASE}:${CHROME_DEBUG.ALT1}`,
  `${API_ENDPOINTS.LOCALHOST_BASE}:${CHROME_DEBUG.ALT2}`,
];
