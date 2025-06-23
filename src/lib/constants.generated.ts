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
	UI_CURSOR_HIGHLIGHT_START: 'ui-cursor-highlight-start',
	UI_CURSOR_HIGHLIGHT_STOP: 'ui-cursor-highlight-stop',
	UI_CURSOR_HIGHLIGHT_MOVE: 'ui-cursor-highlight-move',
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

// Type helpers
export type EventName = typeof EVENTS[keyof typeof EVENTS];

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
	return import.meta.env.MODE === 'development';
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
