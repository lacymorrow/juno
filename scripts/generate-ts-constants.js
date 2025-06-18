#!/usr/bin/env node
/**
 * Generate TypeScript constants from Rust constants
 *
 * This script parses the Rust constants modules and generates a TypeScript
 * constants file, eliminating duplication and ensuring consistency.
 */

import fs from 'fs';
import path from 'path';

const RUST_CONSTANTS_DIR = 'src-tauri/src/constants';
const TS_OUTPUT_FILE = 'src/lib/constants.generated.ts';

/**
 * Parse Rust constant definitions
 */
function parseRustConstants() {
    const constants = {
        events: {},
        timeouts: {},
        ports: {},
        api: {},
        app: {},
        ui: {},
    };

    // Parse events module
    const eventsFile = fs.readFileSync(path.join(RUST_CONSTANTS_DIR, 'events.rs'), 'utf8');
    constants.events = parseEventConstants(eventsFile);

    return constants;
}

function parseEventConstants(rustCode) {
    const events = {};

    // Parse nested modules like agent::EVENT
    const moduleRegex = /pub mod (\w+) \{([^}]+)\}/g;
    const constRegex = /pub const (\w+): &str = "([^"]+)"/g;

    let moduleMatch;
    while ((moduleMatch = moduleRegex.exec(rustCode)) !== null) {
        const [, moduleName, moduleContent] = moduleMatch;

        let constMatch;
        while ((constMatch = constRegex.exec(moduleContent)) !== null) {
            const [, constName, constValue] = constMatch;
            events[`${moduleName.toUpperCase()}_${constName}`] = constValue;
        }
    }

    return events;
}

/**
 * Generate TypeScript constants file
 */
function generateTypeScript(constants) {
    return `// Generated file - do not edit manually
// This file is auto-generated from Rust constants
// Run 'npm run generate-constants' to update

export const EVENTS = {
${Object.entries(constants.events)
    .map(([key, value]) => `  ${key}: '${value}',`)
    .join('\n')}
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
  EMAIL: /^[^\\s@]+@[^\\s@]+\\.[^\\s@]+$/,
  URL: /^https?:\\/\\/.+/,
  JSON: /^[\\s]*[{\\[]/,
  WHITESPACE_ONLY: /^\\s*$/,
  WAKE_WORD: /^[a-zA-Z\\s]{2,20}$/,
  COMMAND_PREFIX: /^[\\/!@#]/,
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
`;
}

/**
 * Main execution
 */
function main() {
    try {
        console.log('🔧 Generating TypeScript constants from Rust...');

        const constants = parseRustConstants();
        const typescript = generateTypeScript(constants);

        fs.writeFileSync(TS_OUTPUT_FILE, typescript);

        console.log(`✅ Generated ${TS_OUTPUT_FILE}`);
        console.log(`📊 Generated ${Object.keys(constants.events).length} event constants`);

    } catch (error) {
        console.error('❌ Error generating constants:', error);
        process.exit(1);
    }
}

if (import.meta.url === `file://${process.argv[1]}`) {
    main();
}
