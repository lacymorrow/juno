import '@testing-library/jest-dom';
import { cleanup } from '@testing-library/react';
import { afterEach, beforeEach, vi } from 'vitest';

// Clean up after each test
afterEach(() => {
    cleanup();
    vi.clearAllMocks();
});

// Mock Tauri API
beforeEach(() => {
    // Mock tauri invoke
    global.window.__TAURI__ = {
        core: {
            invoke: vi.fn().mockResolvedValue({}),
        },
        event: {
            listen: vi.fn().mockResolvedValue(() => { }),
            emit: vi.fn().mockResolvedValue(undefined),
        },
        app: {
            getName: vi.fn().mockResolvedValue('Juno'),
            getVersion: vi.fn().mockResolvedValue('0.2.3'),
        },
        window: {
            getCurrent: vi.fn().mockReturnValue({
                minimize: vi.fn().mockResolvedValue(undefined),
                maximize: vi.fn().mockResolvedValue(undefined),
                unmaximize: vi.fn().mockResolvedValue(undefined),
                close: vi.fn().mockResolvedValue(undefined),
                setFullscreen: vi.fn().mockResolvedValue(undefined),
                isMaximized: vi.fn().mockResolvedValue(false),
                isFullscreen: vi.fn().mockResolvedValue(false),
                onResized: vi.fn().mockResolvedValue(() => { }),
                onMoved: vi.fn().mockResolvedValue(() => { }),
            }),
        },
        globalShortcut: {
            register: vi.fn().mockResolvedValue(undefined),
            unregister: vi.fn().mockResolvedValue(undefined),
        },
        notification: {
            sendNotification: vi.fn().mockResolvedValue(undefined),
            isPermissionGranted: vi.fn().mockResolvedValue(true),
            requestPermission: vi.fn().mockResolvedValue('granted'),
        },
        process: {
            exit: vi.fn().mockResolvedValue(undefined),
        },
        store: {
            set: vi.fn().mockResolvedValue(undefined),
            get: vi.fn().mockResolvedValue(null),
            delete: vi.fn().mockResolvedValue(undefined),
            clear: vi.fn().mockResolvedValue(undefined),
            keys: vi.fn().mockResolvedValue([]),
            values: vi.fn().mockResolvedValue([]),
            entries: vi.fn().mockResolvedValue([]),
            length: vi.fn().mockResolvedValue(0),
            has: vi.fn().mockResolvedValue(false),
            onChange: vi.fn().mockResolvedValue(() => { }),
        },
    };

    // Mock tauri commands
    global.window.__TAURI__.core.invoke = vi.fn().mockImplementation((command: string, args?: any) => {
        // Default mock responses for common commands
        const mockResponses: Record<string, any> = {
            'agent:submit_query': {
                response: 'Mock agent response',
                tool_calls: [],
                conversation_id: 'test-conv-id',
                tokens_used: 100,
            },
            'app:get_state': {
                isListening: false,
                isAgentExecuting: false,
                currentMode: 'idle',
                lastVoiceActivity: null,
            },
            'app:toggle_listening': { isListening: true },
            'app:stop_listening': { isListening: false },
            'app:set_mode': { mode: args?.mode || 'dictation' },
            'permissions:check_accessibility': true,
            'permissions:check_screen_recording': true,
            'permissions:request_accessibility': true,
            'permissions:request_screen_recording': true,
            'window:minimize': undefined,
            'window:maximize': undefined,
            'window:close': undefined,
            'dev:get_logs': [],
            'dev:clear_logs': undefined,
            'settings:get_all': {
                theme: 'dark',
                voiceSettings: {
                    enabled: true,
                    wakeWordEnabled: false,
                    language: 'en-US',
                },
                agentSettings: {
                    model: 'claude-3-5-sonnet-20241022',
                    maxTokens: 4000,
                    temperature: 0.7,
                },
            },
            'settings:update': undefined,
            'chat:export': JSON.stringify([
                { role: 'user', content: 'Test message', timestamp: Date.now() },
                { role: 'assistant', content: 'Test response', timestamp: Date.now() + 1000 },
            ]),
            'chat:import': { success: true, messagesImported: 2 },
            'chat:clear': undefined,
            'feedback:submit': { success: true },
            'updates:check': { available: false, version: '0.2.3' },
            'updates:install': undefined,
        };

        const response = mockResponses[command];
        if (response !== undefined) {
            return Promise.resolve(response);
        }

        // Default fallback
        console.warn(`Unmocked Tauri command: ${command}`);
        return Promise.resolve({});
    });
});

// Mock fetch for external API calls
global.fetch = vi.fn().mockImplementation((url: string | URL | Request, init?: RequestInit) => {
    const urlString = typeof url === 'string' ? url : url.toString();

    // Mock GitHub API for feedback
    if (urlString.includes('api.github.com')) {
        return Promise.resolve({
            ok: true,
            status: 201,
            json: async () => ({
                id: 123,
                number: 456,
                html_url: 'https://github.com/test/repo/issues/456',
                title: 'Test Issue',
                state: 'open',
            }),
        } as Response);
    }

    // Mock Anthropic API
    if (urlString.includes('anthropic.com')) {
        return Promise.resolve({
            ok: true,
            status: 200,
            json: async () => ({
                content: [{ text: 'Mock AI response' }],
                usage: { input_tokens: 50, output_tokens: 20 },
            }),
        } as Response);
    }

    // Default mock response
    return Promise.resolve({
        ok: true,
        status: 200,
        json: async () => ({}),
        text: async () => '',
    } as Response);
});

// Mock localStorage
const localStorageMock = {
    getItem: vi.fn().mockImplementation((key: string) => {
        const store: Record<string, string> = {
            'juno-theme': 'dark',
            'juno-onboarding-completed': 'true',
        };
        return store[key] || null;
    }),
    setItem: vi.fn(),
    removeItem: vi.fn(),
    clear: vi.fn(),
    length: 0,
    key: vi.fn(),
};

Object.defineProperty(window, 'localStorage', {
    value: localStorageMock,
});

// Mock matchMedia for responsive design tests
Object.defineProperty(window, 'matchMedia', {
    writable: true,
    value: vi.fn().mockImplementation(query => ({
        matches: false,
        media: query,
        onchange: null,
        addListener: vi.fn(), // Deprecated
        removeListener: vi.fn(), // Deprecated
        addEventListener: vi.fn(),
        removeEventListener: vi.fn(),
        dispatchEvent: vi.fn(),
    })),
});

// Mock ResizeObserver
global.ResizeObserver = vi.fn().mockImplementation(() => ({
    observe: vi.fn(),
    unobserve: vi.fn(),
    disconnect: vi.fn(),
}));

// Mock IntersectionObserver
global.IntersectionObserver = vi.fn().mockImplementation(() => ({
    observe: vi.fn(),
    unobserve: vi.fn(),
    disconnect: vi.fn(),
}));

// Custom testing utilities
export const testUtils = {
    // Wait for async operations to complete
    waitForAsyncOperations: () => new Promise(resolve => setTimeout(resolve, 0)),

    // Mock user interactions
    mockUserVoiceInput: (text: string) => {
        const event = new CustomEvent('voice-input', { detail: { text } });
        window.dispatchEvent(event);
    },

    // Mock agent responses
    mockAgentResponse: (response: string, toolCalls: any[] = []) => {
        const event = new CustomEvent('agent-response', {
            detail: { response, tool_calls: toolCalls }
        });
        window.dispatchEvent(event);
    },

    // Mock keyboard shortcuts
    mockKeyboardShortcut: (key: string, modifiers: string[] = []) => {
        const event = new KeyboardEvent('keydown', {
            key,
            ctrlKey: modifiers.includes('ctrl'),
            metaKey: modifiers.includes('cmd'),
            altKey: modifiers.includes('alt'),
            shiftKey: modifiers.includes('shift'),
        });
        document.dispatchEvent(event);
    },

    // Mock file operations
    mockFileOperation: (operation: string, success: boolean = true) => {
        const event = new CustomEvent('file-operation', {
            detail: { operation, success }
        });
        window.dispatchEvent(event);
    },

    // Create mock conversation data
    createMockConversation: (messageCount: number = 5) => {
        const messages = [];
        for (let i = 0; i < messageCount; i++) {
            messages.push(
                {
                    role: 'user',
                    content: `Test user message ${i + 1}`,
                    timestamp: Date.now() - (messageCount - i) * 60000,
                },
                {
                    role: 'assistant',
                    content: `Test assistant response ${i + 1}`,
                    timestamp: Date.now() - (messageCount - i) * 60000 + 30000,
                    tool_calls: i % 2 === 0 ? [
                        {
                            name: 'computer',
                            input: { action: 'screenshot' }
                        }
                    ] : undefined,
                }
            );
        }
        return messages;
    },

    // Mock error scenarios
    mockError: (command: string, error: string) => {
        const originalInvoke = global.window.__TAURI__.core.invoke;
        global.window.__TAURI__.core.invoke = vi.fn().mockImplementation((cmd: string, args?: any) => {
            if (cmd === command) {
                return Promise.reject(new Error(error));
            }
            return originalInvoke(cmd, args);
        });
    },

    // Reset all mocks
    resetMocks: () => {
        vi.clearAllMocks();
        localStorageMock.getItem.mockClear();
        localStorageMock.setItem.mockClear();
        localStorageMock.removeItem.mockClear();
        localStorageMock.clear.mockClear();
    }
};

// Export for use in tests
export { vi };

// Global test configuration
declare global {
    interface Window {
        __TAURI__: any;
    }
}

// Test environment detection
export const isTestEnvironment = process.env.NODE_ENV === 'test' || process.env.VITEST;

// Performance testing utilities
export const performanceUtils = {
    measureRenderTime: async (renderFunction: () => void): Promise<number> => {
        const start = performance.now();
        await renderFunction();
        return performance.now() - start;
    },

    measureMemoryUsage: (): number => {
        if ('memory' in performance) {
            return (performance as any).memory.usedJSHeapSize;
        }
        return 0;
    },

    waitForNextFrame: (): Promise<void> => {
        return new Promise(resolve => requestAnimationFrame(() => resolve()));
    },
};

// Accessibility testing utilities
export const a11yUtils = {
    checkFocusOrder: (container: HTMLElement): boolean => {
        const focusableElements = container.querySelectorAll(
            'button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])'
        );

        // Check if elements are in logical tab order
        let lastTabIndex = -Infinity;
        for (const element of focusableElements) {
            const tabIndex = parseInt((element as HTMLElement).tabIndex.toString()) || 0;
            if (tabIndex < lastTabIndex) {
                return false;
            }
            lastTabIndex = tabIndex;
        }

        return true;
    },

    checkAriaLabels: (container: HTMLElement): string[] => {
        const issues: string[] = [];
        const buttons = container.querySelectorAll('button');

        buttons.forEach((button, index) => {
            if (!button.textContent?.trim() && !button.getAttribute('aria-label')) {
                issues.push(`Button at index ${index} has no accessible name`);
            }
        });

        return issues;
    },

    checkColorContrast: (element: HTMLElement): boolean => {
        // Simplified contrast check - in real implementation would use proper color analysis
        const style = window.getComputedStyle(element);
        const color = style.color;
        const backgroundColor = style.backgroundColor;

        // Basic check for common accessibility issues
        return color !== backgroundColor && color !== 'transparent';
    },
};
