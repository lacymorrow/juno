// Re-export generated constants for convenience
export * from './constants.generated';

// Frontend-specific constants that don't need Rust equivalents

export const TIMEOUTS = {
    SOUND_DEBOUNCE_MS: 300,
    HEARTBEAT_INTERVAL_MS: 30000,
    CLOUD_CONNECTION_TIMEOUT_MS: 10000,
} as const;

export const UI = {
    MAX_CONVERSATION_DISPLAY: 1000,
    SCROLL_THRESHOLD: 100,
    ANIMATION_DURATION_MS: 200,
} as const;

// Computer Use API Action Constants
// These match the action names expected by the backend computer command
export const COMPUTER_ACTIONS = {
    SCREENSHOT: 'screenshot',
    LEFT_CLICK: 'click',
    RIGHT_CLICK: 'right_click',
    MIDDLE_CLICK: 'middle_click',
    DOUBLE_CLICK: 'double_click',
    TRIPLE_CLICK: 'triple_click',
    DRAG: 'drag',
    MOUSE_MOVE: 'move',
    SCROLL: 'scroll',
    TYPE: 'type',
    KEY: 'key',
    HOLD_KEY: 'hold_key',
    WAIT: 'wait',
    CURSOR_POSITION: 'cursor_position',
} as const;
