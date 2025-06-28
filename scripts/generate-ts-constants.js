#!/usr/bin/env node
/**
 * Generate TypeScript constants from Rust constants
 *
 * This script parses ALL Rust constants modules and generates a comprehensive TypeScript
 * constants file, eliminating duplication and ensuring consistency.
 */

import fs from 'fs';
import path from 'path';

const RUST_CONSTANTS_DIR = 'src-tauri/src/constants';
const TS_OUTPUT_FILE = 'src/lib/constants.generated.ts';

/**
 * Parse all Rust constant definitions
 */
function parseRustConstants() {
    const constants = {
        events: {},
        timeouts: {},
        ports: {},
        api: {},
        app: {},
        ui: {},
        audio: {},
        files: {},
        permissions: {},
        errors: {},
        commands: {},
        memory: {},
        agent: {},
    };

    // Parse each constants module
    try {
        // Parse events module
        const eventsFile = fs.readFileSync(path.join(RUST_CONSTANTS_DIR, 'events.rs'), 'utf8');
        constants.events = parseEventConstants(eventsFile);

        // Parse timeouts module
        const timeoutsFile = fs.readFileSync(path.join(RUST_CONSTANTS_DIR, 'timeouts.rs'), 'utf8');
        constants.timeouts = parseSimpleConstants(timeoutsFile);

        // Parse ports module
        const portsFile = fs.readFileSync(path.join(RUST_CONSTANTS_DIR, 'ports.rs'), 'utf8');
        constants.ports = parseSimpleConstants(portsFile);

        // Parse API module
        const apiFile = fs.readFileSync(path.join(RUST_CONSTANTS_DIR, 'api.rs'), 'utf8');
        constants.api = parseModuleConstants(apiFile);

        // Parse app module
        const appFile = fs.readFileSync(path.join(RUST_CONSTANTS_DIR, 'app.rs'), 'utf8');
        constants.app = parseSimpleConstants(appFile);

        // Parse UI module
        const uiFile = fs.readFileSync(path.join(RUST_CONSTANTS_DIR, 'ui.rs'), 'utf8');
        constants.ui = parseModuleConstants(uiFile);

        // Parse audio module
        const audioFile = fs.readFileSync(path.join(RUST_CONSTANTS_DIR, 'audio.rs'), 'utf8');
        constants.audio = parseModuleConstants(audioFile);

        // Parse files module
        const filesFile = fs.readFileSync(path.join(RUST_CONSTANTS_DIR, 'files.rs'), 'utf8');
        constants.files = parseModuleConstants(filesFile);

        // Parse permissions module
        const permissionsFile = fs.readFileSync(path.join(RUST_CONSTANTS_DIR, 'permissions.rs'), 'utf8');
        constants.permissions = parseModuleConstants(permissionsFile);

        // Parse errors module
        const errorsFile = fs.readFileSync(path.join(RUST_CONSTANTS_DIR, 'errors.rs'), 'utf8');
        constants.errors = parseModuleConstants(errorsFile);

        // Parse commands module
        const commandsFile = fs.readFileSync(path.join(RUST_CONSTANTS_DIR, 'commands.rs'), 'utf8');
        constants.commands = parseModuleConstants(commandsFile);

        // Parse memory module
        const memoryFile = fs.readFileSync(path.join(RUST_CONSTANTS_DIR, 'memory.rs'), 'utf8');
        constants.memory = parseModuleConstants(memoryFile);

        // Parse agent module (contains computer actions and tool names)
        const agentFile = fs.readFileSync(path.join(RUST_CONSTANTS_DIR, 'agent.rs'), 'utf8');
        constants.agent = parseModuleConstants(agentFile);

    } catch (error) {
        console.warn(`Warning: Could not parse some constants files: ${error.message}`);
    }

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

function parseSimpleConstants(rustCode) {
    const constants = {};

    // Parse simple constants: pub const NAME: type = value;
    // Updated regex to handle underscores in numeric literals (e.g., 30_000)
    const constRegex = /pub const (\w+): (?:&str|u\d+|i\d+|f\d+|usize|bool|&\[&str\]) = (?:"([^"]+)"|(\d+(?:_\d+)*(?:\.\d+(?:_\d+)*)?)|(\w+)|&\[(.*?)\])/g;

    let match;
    while ((match = constRegex.exec(rustCode)) !== null) {
        const [, name, stringValue, numericValue, boolValue, arrayValue] = match;

        if (arrayValue !== undefined) {
            // Parse array of strings: &["hey juno", "computer"]
            const arrayElements = arrayValue.match(/"([^"]+)"/g);
            if (arrayElements) {
                constants[name] = arrayElements.map(el => el.slice(1, -1)); // Remove quotes
            }
        } else if (numericValue !== undefined) {
            // Remove underscores from numeric literals before parsing
            const cleanNumeric = numericValue.replace(/_/g, '');
            constants[name] = parseFloat(cleanNumeric);
        } else {
            constants[name] = stringValue || boolValue;
        }
    }

    return constants;
}

function parseModuleConstants(rustCode) {
    const allConstants = {};

    // Parse nested modules with proper brace matching
    const moduleMatches = parseModulesWithBraceMatching(rustCode);

    moduleMatches.forEach(({ fullMatch, moduleName, moduleContent }) => {
        const moduleConstants = parseSimpleConstants(moduleContent);

        // Prefix module constants with module name
        Object.entries(moduleConstants).forEach(([key, value]) => {
            allConstants[`${moduleName.toUpperCase()}_${key}`] = value;
        });
    });

    // Parse top-level constants by removing module content first to avoid duplicates
    let codeWithoutModules = rustCode;
    moduleMatches.forEach(({ fullMatch }) => {
        codeWithoutModules = codeWithoutModules.replace(fullMatch, '');
    });

    const topLevelConstants = parseSimpleConstants(codeWithoutModules);
    Object.assign(allConstants, topLevelConstants);

    return allConstants;
}

/**
 * Parse Rust modules with proper brace matching to handle nested braces correctly
 */
function parseModulesWithBraceMatching(rustCode) {
    const modules = [];
    const moduleStartRegex = /pub mod (\w+) \{/g;
    let match;

    while ((match = moduleStartRegex.exec(rustCode)) !== null) {
        const moduleName = match[1];
        const moduleStartIndex = match.index;
        const contentStartIndex = match.index + match[0].length;

        // Find the matching closing brace by counting braces
        let braceCount = 1;
        let currentIndex = contentStartIndex;

        while (currentIndex < rustCode.length && braceCount > 0) {
            const char = rustCode[currentIndex];
            if (char === '{') {
                braceCount++;
            } else if (char === '}') {
                braceCount--;
            }
            currentIndex++;
        }

        if (braceCount === 0) {
            // Found the matching closing brace
            const moduleEndIndex = currentIndex;
            const fullMatch = rustCode.substring(moduleStartIndex, moduleEndIndex);
            const moduleContent = rustCode.substring(contentStartIndex, currentIndex - 1);

            modules.push({
                fullMatch,
                moduleName,
                moduleContent
            });

            // Update the regex lastIndex to continue searching after this module
            moduleStartRegex.lastIndex = moduleEndIndex;
        } else {
            // Unmatched braces - log warning and continue
            console.warn(`Warning: Unmatched braces in module ${moduleName}`);
            break;
        }
    }

    return modules;
}

/**
 * Helper function to format values consistently for TypeScript generation
 */
function formatValue(value) {
    if (Array.isArray(value)) {
        return `[${value.map(v => `'${v}'`).join(', ')}]`;
    } else if (typeof value === 'string') {
        return `'${value}'`;
    } else {
        return value;
    }
}

/**
 * Generate comprehensive TypeScript constants file
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

export const TIMEOUTS = {
${Object.entries(constants.timeouts)
    .map(([key, value]) => `  ${key}: ${value},`)
    .join('\n')}
} as const;

export const PORTS = {
${Object.entries(constants.ports)
    .map(([key, value]) => `  ${key}: ${value},`)
    .join('\n')}
} as const;

export const API_ENDPOINTS = {
${Object.entries(constants.api)
    .map(([key, value]) => `  ${key}: ${formatValue(value)},`)
    .join('\n')}
} as const;

export const APP_IDENTITY = {
${Object.entries(constants.app)
    .map(([key, value]) => `  ${key}: ${formatValue(value)},`)
    .join('\n')}
} as const;

export const UI = {
${Object.entries(constants.ui)
    .map(([key, value]) => `  ${key}: ${formatValue(value)},`)
    .join('\n')}
} as const;

export const AUDIO = {
${Object.entries(constants.audio)
    .map(([key, value]) => `  ${key}: ${formatValue(value)},`)
    .join('\n')}
} as const;

export const COMMANDS = {
${Object.entries(constants.commands)
    .map(([key, value]) => `  ${key}: ${formatValue(value)},`)
    .join('\n')}
} as const;

export const MEMORY = {
${Object.entries(constants.memory)
    .map(([key, value]) => `  ${key}: ${formatValue(value)},`)
    .join('\n')}
} as const;

export const AGENT = {
${Object.entries(constants.agent)
    .map(([key, value]) => `  ${key}: ${formatValue(value)},`)
    .join('\n')}
} as const;

export const COMPUTER_ACTIONS = {
${(() => {
    const computerActions = {};

    // Collect all computer action constants, avoiding duplicates
    Object.entries(constants.agent)
        .filter(([key]) =>
            key.startsWith('COMPUTER_ACTIONS_') ||
            key.startsWith('TOOL_NAMES_ACTION_') ||
            (key.startsWith('ACTION_') && !key.startsWith('TOOL_NAMES_'))
        )
        .forEach(([key, value]) => {
            let cleanKey = key;
            if (cleanKey.startsWith('COMPUTER_ACTIONS_')) {
                cleanKey = cleanKey.replace('COMPUTER_ACTIONS_', '');
            } else if (cleanKey.startsWith('TOOL_NAMES_ACTION_')) {
                cleanKey = cleanKey.replace('TOOL_NAMES_ACTION_', '');
            } else if (cleanKey.startsWith('ACTION_')) {
                cleanKey = cleanKey.replace('ACTION_', '');
            }

            // Only add if not already present, prioritizing ACTION_ prefixed constants
            if (!computerActions[cleanKey] || key.startsWith('ACTION_') || key.startsWith('TOOL_NAMES_ACTION_')) {
                computerActions[cleanKey] = value;
            }
        });

    return Object.entries(computerActions)
        .map(([key, value]) => `  ${key}: '${value}',`)
        .join('\n');
})()}
} as const;

export const TOOL_NAMES = {
${Object.entries(constants.agent)
    .filter(([key]) => key.startsWith('TOOL_NAMES_'))
    .map(([key, value]) => `  ${key.replace('TOOL_NAMES_', '')}: '${value}',`)
    .join('\n')}
} as const;

export const FILE_EXTENSIONS = {
${Object.entries(constants.files)
    .filter(([key]) => key.includes('EXT'))
    .map(([key, value]) => {
        // Handle both _EXT and _EXTENSION suffixes correctly
        let cleanKey = key;
        if (cleanKey.endsWith('_EXTENSION')) {
            cleanKey = cleanKey.replace('_EXTENSION', '_EXT');
        }
        cleanKey = cleanKey.replace('_EXT', '');
        return `  ${key}: '${value}',\n  ${cleanKey}: '${value}',`;
    })
    .join('\n')}
} as const;

export const PERMISSION_TYPES = {
${Object.entries(constants.permissions)
    .filter(([key]) => key.startsWith('TYPES_'))
    .map(([key, value]) => `  ${key.replace('TYPES_', '')}: '${value}',`)
    .join('\n')}
} as const;

export const CHROME_DEBUG = {
${Object.entries(constants.ports)
    .filter(([key]) => key.startsWith('CHROME_DEBUG_PORT_'))
    .map(([key, value]) => `  ${key.replace('CHROME_DEBUG_PORT_', '')}: ${value},`)
    .join('\n')}
} as const;

export const WINDOW_LABELS = {
${Object.entries(constants.ui)
    .filter(([key]) => key.startsWith('WINDOW_LABELS_'))
    .map(([key, value]) => `  ${key.replace('WINDOW_LABELS_', '')}: '${value}',`)
    .join('\n')}
} as const;

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

// Type helpers
export type EventName = typeof EVENTS[keyof typeof EVENTS];
export type WindowLabel = typeof WINDOW_LABELS[keyof typeof WINDOW_LABELS];
export type ApiEndpoint = typeof API_ENDPOINTS[keyof typeof API_ENDPOINTS];
export type FileExtension = typeof FILE_EXTENSIONS[keyof typeof FILE_EXTENSIONS];
export type PermissionType = typeof PERMISSION_TYPES[keyof typeof PERMISSION_TYPES];
export type ChromeDebugPort = typeof CHROME_DEBUG[keyof typeof CHROME_DEBUG];
export type CommandName = typeof COMMANDS[keyof typeof COMMANDS];
export type ComputerAction = typeof COMPUTER_ACTIONS[keyof typeof COMPUTER_ACTIONS];
export type ToolName = typeof TOOL_NAMES[keyof typeof TOOL_NAMES];
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
        console.log(`📊 Generated constants from ${Object.keys(constants).length} Rust modules`);
        console.log(`   - Events: ${Object.keys(constants.events).length}`);
        console.log(`   - Timeouts: ${Object.keys(constants.timeouts).length}`);
        console.log(`   - Ports: ${Object.keys(constants.ports).length}`);
        console.log(`   - API: ${Object.keys(constants.api).length}`);
        console.log(`   - App: ${Object.keys(constants.app).length}`);
        console.log(`   - UI: ${Object.keys(constants.ui).length}`);
        console.log(`   - Audio: ${Object.keys(constants.audio).length}`);
        console.log(`   - Files: ${Object.keys(constants.files).length}`);
        console.log(`   - Permissions: ${Object.keys(constants.permissions).length}`);
        console.log(`   - Agent: ${Object.keys(constants.agent).length}`);

    } catch (error) {
        console.error('❌ Error generating constants:', error);
        process.exit(1);
    }
}

if (import.meta.url === `file://${process.argv[1]}`) {
    main();
}
