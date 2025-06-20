/**
 * React hook for the centralized, reactive settings system
 *
 * This hook provides access to all application settings through a single
 * interface, with automatic reactivity when settings change.
 */

import { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/tauri';
import { listen } from '@tauri-apps/api/event';

// Types
interface AppSettings {
    keyboard_shortcuts: KeyboardShortcuts;
    floating_bar: FloatingBarConfig;
    agent: AgentSettings;
    providers: ProviderConfig;
    cloud: CloudConfig;
    tools: ToolConfig;
    prompts: PromptConfig;
    audio: AudioSettings;
    ui: UISettings;
    onboarding: OnboardingState;
    performance: PerformanceSettings;
}

interface KeyboardShortcuts {
    agent_mode_toggle: string;
    dictation_input: string;
    stop_current_task: string;
    open_settings: string;
}

interface FloatingBarConfig {
    show_voice_indicator: boolean;
    enable_animations: boolean;
    auto_hide: boolean;
    auto_hide_delay: number;
    opacity: number;
}

interface AgentSettings {
    mode: string;
    trigger_mode: string;
    default_provider: string;
    max_execution_time: number;
    enable_memory: boolean;
    memory_retention_days: number;
}

interface ProviderConfig {
    active_provider: string;
    providers: ProviderInfo[];
    fallback_provider?: string;
}

interface ProviderInfo {
    id: string;
    name: string;
    api_key?: string;
    model: string;
    max_tokens: number;
    temperature: number;
    system_prompt?: string;
    enabled: boolean;
}

interface CloudConfig {
    enabled: boolean;
    server_url: string;
    device_id?: string;
    device_name: string;
    api_key?: string;
    auto_connect: boolean;
    reconnect_interval: number;
    heartbeat_interval: number;
    command_timeout: number;
    security_level: string;
}

interface ToolConfig {
    categories: Record<string, ToolCategory>;
    mcp_servers: MCPServerConfig[];
    enabled_tools: string[];
}

interface ToolCategory {
    name: string;
    enabled: boolean;
    tools: ToolInfo[];
}

interface ToolInfo {
    name: string;
    enabled: boolean;
    config: Record<string, any>;
}

interface MCPServerConfig {
    name: string;
    command: string;
    args: string[];
    env: Record<string, string>;
    enabled: boolean;
}

interface PromptConfig {
    global_variables: Record<string, string>;
    custom_prompts: Record<string, PromptTemplate>;
}

interface PromptTemplate {
    content: string;
    variables: string[];
    customizable: boolean;
}

interface AudioSettings {
    tts_provider: string;
    tts_voice?: string;
    tts_speed: number;
    dictation_clipboard_enabled: boolean;
    sound_enabled: boolean;
    always_listening_active: boolean;
    always_listening_sensitivity: number;
    always_listening_wake_words: string[];
}

interface UISettings {
    theme: string;
    accent_color: string;
    font_size: number;
    animation_speed: number;
    show_notifications: boolean;
    notification_duration: number;
}

interface OnboardingState {
    completed: boolean;
    completed_at?: string;
    skipped: boolean;
    current_step: number;
}

interface PerformanceSettings {
    monitoring_enabled: boolean;
    collection_interval: number;
    retention_days: number;
    detailed_logging: boolean;
}

interface SettingsUpdateEvent {
    section: string;
    key?: string;
    settings: AppSettings;
    timestamp: string;
}

interface SettingsManagerHook {
    // Current settings
    settings: AppSettings | null;
    loading: boolean;
    error: string | null;

    // Core operations
    initialize: () => Promise<void>;
    refresh: () => Promise<void>;

    // Get operations
    getSection: (path: string) => Promise<any>;

    // Update operations
    updateSection: (path: string, value: any) => Promise<void>;
    updateMultiple: (updates: Array<[string, any]>) => Promise<void>;

    // Reset operations
    resetSection: (path: string) => Promise<void>;
    resetAll: () => Promise<void>;

    // Convenience accessors
    keyboardShortcuts: KeyboardShortcuts | null;
    floatingBar: FloatingBarConfig | null;
    agent: AgentSettings | null;
    providers: ProviderConfig | null;
    cloud: CloudConfig | null;
    tools: ToolConfig | null;
    prompts: PromptConfig | null;
    audio: AudioSettings | null;
    ui: UISettings | null;
    onboarding: OnboardingState | null;
    performance: PerformanceSettings | null;

    // Convenience updaters
    updateKeyboardShortcuts: (shortcuts: Partial<KeyboardShortcuts>) => Promise<void>;
    updateFloatingBar: (config: Partial<FloatingBarConfig>) => Promise<void>;
    updateAgent: (config: Partial<AgentSettings>) => Promise<void>;
    updateProviders: (config: Partial<ProviderConfig>) => Promise<void>;
    updateCloud: (config: Partial<CloudConfig>) => Promise<void>;
    updateTools: (config: Partial<ToolConfig>) => Promise<void>;
    updatePrompts: (config: Partial<PromptConfig>) => Promise<void>;
    updateAudio: (config: Partial<AudioSettings>) => Promise<void>;
    updateUI: (config: Partial<UISettings>) => Promise<void>;
    updateOnboarding: (config: Partial<OnboardingState>) => Promise<void>;
    updatePerformance: (config: Partial<PerformanceSettings>) => Promise<void>;
}

export function useSettingsManager(): SettingsManagerHook {
    const [settings, setSettings] = useState<AppSettings | null>(null);
    const [loading, setLoading] = useState(true);
    const [error, setError] = useState<string | null>(null);

    // Initialize settings manager
    const initialize = useCallback(async () => {
        try {
            setLoading(true);
            setError(null);

            // Initialize the backend settings manager
            await invoke('settings_initialize');

            // Load all settings
            const allSettings = await invoke<AppSettings>('settings_get_all');
            setSettings(allSettings);

            console.log('✅ Settings manager initialized successfully');
        } catch (err) {
            const errorMessage = err instanceof Error ? err.message : String(err);
            setError(errorMessage);
            console.error('❌ Failed to initialize settings manager:', errorMessage);
        } finally {
            setLoading(false);
        }
    }, []);

    // Refresh settings from backend
    const refresh = useCallback(async () => {
        try {
            const allSettings = await invoke<AppSettings>('settings_get_all');
            setSettings(allSettings);
        } catch (err) {
            const errorMessage = err instanceof Error ? err.message : String(err);
            setError(errorMessage);
            console.error('❌ Failed to refresh settings:', errorMessage);
        }
    }, []);

    // Get specific settings section
    const getSection = useCallback(async (path: string) => {
        try {
            return await invoke('settings_get_section', { path });
        } catch (err) {
            const errorMessage = err instanceof Error ? err.message : String(err);
            setError(errorMessage);
            throw new Error(`Failed to get settings section '${path}': ${errorMessage}`);
        }
    }, []);

    // Update settings section
    const updateSection = useCallback(async (path: string, value: any) => {
        try {
            await invoke('settings_update_section', { path, value });
            // Settings will update automatically via event listener
        } catch (err) {
            const errorMessage = err instanceof Error ? err.message : String(err);
            setError(errorMessage);
            throw new Error(`Failed to update settings section '${path}': ${errorMessage}`);
        }
    }, []);

    // Update multiple sections
    const updateMultiple = useCallback(async (updates: Array<[string, any]>) => {
        try {
            await invoke('settings_update_multiple', { updates });
            // Settings will update automatically via event listener
        } catch (err) {
            const errorMessage = err instanceof Error ? err.message : String(err);
            setError(errorMessage);
            throw new Error(`Failed to update multiple settings: ${errorMessage}`);
        }
    }, []);

    // Reset section to default
    const resetSection = useCallback(async (path: string) => {
        try {
            await invoke('settings_reset_section', { path });
            // Settings will update automatically via event listener
        } catch (err) {
            const errorMessage = err instanceof Error ? err.message : String(err);
            setError(errorMessage);
            throw new Error(`Failed to reset settings section '${path}': ${errorMessage}`);
        }
    }, []);

    // Reset all settings
    const resetAll = useCallback(async () => {
        try {
            await invoke('settings_reset_all');
            // Settings will update automatically via event listener
        } catch (err) {
            const errorMessage = err instanceof Error ? err.message : String(err);
            setError(errorMessage);
            throw new Error(`Failed to reset all settings: ${errorMessage}`);
        }
    }, []);

    // Convenience updaters
    const updateKeyboardShortcuts = useCallback(async (shortcuts: Partial<KeyboardShortcuts>) => {
        const current = settings?.keyboard_shortcuts || {};
        await updateSection('keyboard_shortcuts', { ...current, ...shortcuts });
    }, [settings?.keyboard_shortcuts, updateSection]);

    const updateFloatingBar = useCallback(async (config: Partial<FloatingBarConfig>) => {
        const current = settings?.floating_bar || {};
        await updateSection('floating_bar', { ...current, ...config });
    }, [settings?.floating_bar, updateSection]);

    const updateAgent = useCallback(async (config: Partial<AgentSettings>) => {
        const current = settings?.agent || {};
        await updateSection('agent', { ...current, ...config });
    }, [settings?.agent, updateSection]);

    const updateProviders = useCallback(async (config: Partial<ProviderConfig>) => {
        const current = settings?.providers || {};
        await updateSection('providers', { ...current, ...config });
    }, [settings?.providers, updateSection]);

    const updateCloud = useCallback(async (config: Partial<CloudConfig>) => {
        const current = settings?.cloud || {};
        await updateSection('cloud', { ...current, ...config });
    }, [settings?.cloud, updateSection]);

    const updateTools = useCallback(async (config: Partial<ToolConfig>) => {
        const current = settings?.tools || {};
        await updateSection('tools', { ...current, ...config });
    }, [settings?.tools, updateSection]);

    const updatePrompts = useCallback(async (config: Partial<PromptConfig>) => {
        const current = settings?.prompts || {};
        await updateSection('prompts', { ...current, ...config });
    }, [settings?.prompts, updateSection]);

    const updateAudio = useCallback(async (config: Partial<AudioSettings>) => {
        const current = settings?.audio || {};
        await updateSection('audio', { ...current, ...config });
    }, [settings?.audio, updateSection]);

    const updateUI = useCallback(async (config: Partial<UISettings>) => {
        const current = settings?.ui || {};
        await updateSection('ui', { ...current, ...config });
    }, [settings?.ui, updateSection]);

    const updateOnboarding = useCallback(async (config: Partial<OnboardingState>) => {
        const current = settings?.onboarding || {};
        await updateSection('onboarding', { ...current, ...config });
    }, [settings?.onboarding, updateSection]);

    const updatePerformance = useCallback(async (config: Partial<PerformanceSettings>) => {
        const current = settings?.performance || {};
        await updateSection('performance', { ...current, ...config });
    }, [settings?.performance, updateSection]);

    // Set up event listener for reactive updates
    useEffect(() => {
        const unlisten = listen<SettingsUpdateEvent>('settings-updated', (event) => {
            const { settings: updatedSettings } = event.payload;
            setSettings(updatedSettings);
            console.log(`🔄 Settings updated: ${event.payload.section}`);
        });

        return () => {
            unlisten.then(fn => fn());
        };
    }, []);

    // Initialize on mount
    useEffect(() => {
        initialize();
    }, [initialize]);

    return {
        // Core state
        settings,
        loading,
        error,

        // Core operations
        initialize,
        refresh,
        getSection,
        updateSection,
        updateMultiple,
        resetSection,
        resetAll,

        // Convenience accessors
        keyboardShortcuts: settings?.keyboard_shortcuts || null,
        floatingBar: settings?.floating_bar || null,
        agent: settings?.agent || null,
        providers: settings?.providers || null,
        cloud: settings?.cloud || null,
        tools: settings?.tools || null,
        prompts: settings?.prompts || null,
        audio: settings?.audio || null,
        ui: settings?.ui || null,
        onboarding: settings?.onboarding || null,
        performance: settings?.performance || null,

        // Convenience updaters
        updateKeyboardShortcuts,
        updateFloatingBar,
        updateAgent,
        updateProviders,
        updateCloud,
        updateTools,
        updatePrompts,
        updateAudio,
        updateUI,
        updateOnboarding,
        updatePerformance,
    };
}

export default useSettingsManager;
