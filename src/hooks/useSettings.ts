import { useState, useEffect, useCallback } from "react";
import { toast } from "sonner";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { KeyboardShortcuts } from "@/types/keyboard";
import { AUDIO } from "@/lib/constants";
import { useInvoke } from "@/hooks/useInvoke";
import type {
	ProviderInfo,
	ProviderSettings,
	ToolCategory,
	MCPServerConfig,
	MCPServerStatus,
	MCPToolInfo,
	PermissionsState,
} from "@/types/settings";

// Types are now imported from shared types

// Global cache to prevent duplicate API calls during startup
interface SettingsCache {
	ttsProvider?: string;
	dictationClipboardEnabled?: boolean;
	soundEnabled?: boolean;
	toolConfigurations?: Record<string, ToolCategory>;
	providers?: ProviderInfo[];
	activeProvider?: string;
	agentMode?: string;
	agentTriggerMode?: string;
	alwaysListeningActive?: boolean;
	alwaysListeningSensitivity?: number;
	alwaysListeningWakeWords?: string[];
	performanceMonitoringEnabled?: boolean;
	permissionsState?: PermissionsState;
	keyboardShortcuts?: KeyboardShortcuts;
	mcpServers?: MCPServerConfig[];
	mcpServerStatuses?: Record<string, MCPServerStatus>;
	lastUpdated?: number;
}

// Cache with 30-second TTL to prevent excessive API calls
const CACHE_TTL = 30000; // 30 seconds
let settingsCache: SettingsCache = {};
const ongoingRequests = new Map<string, Promise<any>>();

// Helper to check if cache is valid
const isCacheValid = (cacheKey: keyof SettingsCache): boolean => {
	const lastUpdated = settingsCache.lastUpdated || 0;
	return Date.now() - lastUpdated < CACHE_TTL && settingsCache[cacheKey] !== undefined;
};

// Helper to get cached value or make API call
const getCachedOrFetch = async <T>(
	cacheKey: keyof SettingsCache,
	apiCall: () => Promise<T>
): Promise<T> => {
	// Return cached value if valid
	if (isCacheValid(cacheKey)) {
		return settingsCache[cacheKey] as T;
	}

	// Check if request is already in progress
	if (ongoingRequests.has(cacheKey)) {
		return ongoingRequests.get(cacheKey) as Promise<T>;
	}

	// Start new request
	const request = apiCall().then((result) => {
		(settingsCache as any)[cacheKey] = result;
		settingsCache.lastUpdated = Date.now();
		ongoingRequests.delete(cacheKey);
		return result;
	}).catch((error) => {
		ongoingRequests.delete(cacheKey);
		throw error;
	});

	ongoingRequests.set(cacheKey, request);
	return request;
};

// Helper to invalidate cache when settings change
const invalidateCache = (cacheKey?: keyof SettingsCache) => {
	if (cacheKey) {
		delete settingsCache[cacheKey];
	} else {
		settingsCache = {};
	}
};

export function useSettings() {
	const { invokeCommand } = useInvoke();
	// TTS Settings
	const [ttsProvider, setTtsProvider] = useState<string>("system");

	// AI Provider Settings
	const [providers, setProviders] = useState<ProviderInfo[]>([]);
	const [activeProvider, setActiveProvider] = useState<string>("");
	const [providerSettings, setProviderSettings] = useState<ProviderSettings | null>(null);
	const [isLoading, setIsLoading] = useState<boolean>(false);

	// Agent Mode Settings
	const [agentMode, setAgentMode] = useState<string>("multi");

	// Agent Trigger Mode Settings
	const [agentTriggerMode, setAgentTriggerMode] = useState<string>("tap");

	// Dictation Settings
	const [dictationClipboardEnabled, setDictationClipboardEnabled] = useState<boolean>(true);

	// Sound Settings
	const [soundEnabled, setSoundEnabled] = useState<boolean>(true);

	// Performance Monitoring Settings
	const [performanceMonitoringEnabled, setPerformanceMonitoringEnabled] = useState<boolean>(true);

	// Always Listening Settings
	const [alwaysListeningActive, setAlwaysListeningActive] = useState<boolean>(false);
	const [alwaysListeningSensitivity, setAlwaysListeningSensitivity] = useState<number>(0.5);
	const [alwaysListeningWakeWords, setAlwaysListeningWakeWords] = useState<string[]>([...AUDIO.DEFAULT_WAKE_WORDS]);
	const [wakeWordsInput, setWakeWordsInput] = useState<string>("");

	// Tool Configuration Settings
	const [toolConfigurations, setToolConfigurations] = useState<Record<string, ToolCategory>>({});
	const [toolConfigLoading, setToolConfigLoading] = useState<boolean>(false);

	// MCP Server Settings
	const [mcpServers, setMcpServers] = useState<MCPServerConfig[]>([]);
	const [mcpServerStatuses, setMcpServerStatuses] = useState<Record<string, MCPServerStatus>>({});
	const [mcpTools, setMcpTools] = useState<MCPToolInfo[]>([]);
	const [mcpLoading, setMcpLoading] = useState<boolean>(false);
	const [mcpJsonData, setMcpJsonData] = useState<string>("");

	// Form state for provider settings
	const [formData, setFormData] = useState<{
		apiKey: string;
		model: string;
		maxTokens: string;
		temperature: string;
		systemPrompt: string;
	}>({
		apiKey: "",
		model: "",
		maxTokens: "",
		temperature: "",
		systemPrompt: "",
	});

	// Permissions state
	const [permissionsState, setPermissionsState] = useState<PermissionsState | null>(null);
	const [permissionsLoading, setPermissionsLoading] = useState<boolean>(false);

	// Keyboard Shortcuts state
	const [keyboardShortcuts, setKeyboardShortcuts] = useState<KeyboardShortcuts>({
		agent_mode_toggle: "",
		dictation_input: "",
		stop_current_task: "",
		open_settings: "",
	});
	const [shortcutsLoading, setShortcutsLoading] = useState<boolean>(false);
	const [editingShortcut, setEditingShortcut] = useState<string | null>(null);

	// Load initial settings
	useEffect(() => {
		loadAllSettings();
	}, []);

	// Listen for MCP state updates from backend
	useEffect(() => {
		let unlisten: (() => void) | undefined;

		const setupMcpListener = async () => {
			unlisten = await listen<{
				servers: MCPServerConfig[];
				statuses: Record<string, MCPServerStatus>;
				tools: MCPToolInfo[];
			}>("mcp_state_updated", (event) => {
				console.log("Received MCP state update:", event.payload);
				setMcpServers(event.payload.servers);
				setMcpServerStatuses(event.payload.statuses);
				setMcpTools(event.payload.tools);
			});
		};

		setupMcpListener();
		return () => unlisten?.();
	}, []);

	const loadAllSettings = useCallback(async () => {
		setIsLoading(true);
		try {
			// Load all settings with caching to prevent duplicate API calls during startup
			const [
				currentTtsProvider,
				availableProviders,
				currentActiveProvider,
				currentAgentMode,
				currentAgentTriggerMode,
				currentClipboardEnabled,
				currentSoundEnabled,
				currentPerformanceMonitoringEnabled,
				alwaysListeningStatus,
				sensitivity,
				wakeWords
			] = await Promise.all([
				getCachedOrFetch('ttsProvider', () => invokeCommand<string>("get_tts_provider_command")),
				getCachedOrFetch('providers', () => invokeCommand<ProviderInfo[]>("get_providers")),
				getCachedOrFetch('activeProvider', () => invokeCommand<string>("get_active_provider")),
				getCachedOrFetch('agentMode', () => invokeCommand<string>("get_agent_mode")),
				getCachedOrFetch('agentTriggerMode', () => invokeCommand<string>("get_agent_trigger_mode")),
				getCachedOrFetch('dictationClipboardEnabled', () => invokeCommand<boolean>("get_dictation_clipboard_enabled")),
				getCachedOrFetch('soundEnabled', () => invokeCommand<boolean>("get_sound_enabled")),
				getCachedOrFetch('performanceMonitoringEnabled', () => invokeCommand<boolean>("get_performance_monitoring")),
				getCachedOrFetch('alwaysListeningActive', () => invokeCommand<boolean>("get_always_listening_status")),
				getCachedOrFetch('alwaysListeningSensitivity', () => invokeCommand<number>("get_always_listening_sensitivity")),
				getCachedOrFetch('alwaysListeningWakeWords', () => invokeCommand<string[]>("get_always_listening_wake_words"))
			]);

			// Set all state values
			setTtsProvider(currentTtsProvider);
			setProviders(availableProviders);
			setActiveProvider(currentActiveProvider);
			setAgentMode(currentAgentMode);
			setAgentTriggerMode(currentAgentTriggerMode);
			setDictationClipboardEnabled(currentClipboardEnabled);
			setSoundEnabled(currentSoundEnabled);
			setPerformanceMonitoringEnabled(currentPerformanceMonitoringEnabled);
			setAlwaysListeningActive(alwaysListeningStatus);
			setAlwaysListeningSensitivity(sensitivity);
			setAlwaysListeningWakeWords(wakeWords);
			setWakeWordsInput(wakeWords.join(", "));

			if (currentActiveProvider) {
				const settings = await invokeCommand<ProviderSettings>("get_provider_settings", {
					providerId: currentActiveProvider,
				});
				setProviderSettings(settings);
				setFormData({
					apiKey: settings.api_key || "",
					model: settings.model || "",
					maxTokens: settings.max_tokens?.toString() || "",
					temperature: settings.temperature?.toString() || "",
					systemPrompt: settings.system_prompt || "",
				});
			}

			// Load permissions status with caching
			await loadPermissionsStatus();

			// Load tool configurations with caching
			await loadToolConfigurations();

			// Load keyboard shortcuts with caching
			await loadKeyboardShortcuts();

			// Load MCP server configurations with caching
			await loadMcpServers();

			console.log("All settings loaded successfully with caching");
		} catch (error) {
			console.error("Error loading settings:", error);
			toast.error("Failed to load some settings");
		} finally {
			setIsLoading(false);
		}
	}, [invokeCommand]);

	const loadPermissionsStatus = useCallback(async () => {
		setPermissionsLoading(true);
		try {
			const permissions = await getCachedOrFetch('permissionsState', () =>
				invokeCommand<PermissionsState>("check_permissions_status")
			);
			setPermissionsState(permissions);
		} catch (error) {
			console.error("Error loading permissions status:", error);
			setPermissionsState(null);
		} finally {
			setPermissionsLoading(false);
		}
	}, [invokeCommand]);

	const loadKeyboardShortcuts = useCallback(async () => {
		setShortcutsLoading(true);
		try {
			const shortcuts = await getCachedOrFetch('keyboardShortcuts', () =>
				invokeCommand<KeyboardShortcuts>("get_keyboard_shortcuts")
			);
			setKeyboardShortcuts(shortcuts);
		} catch (error) {
			console.error("Error loading keyboard shortcuts:", error);
			toast.error("Failed to load keyboard shortcuts");
		} finally {
			setShortcutsLoading(false);
		}
	}, [invokeCommand]);

	const loadToolConfigurations = useCallback(async () => {
		setToolConfigLoading(true);
		try {
			const configs = await getCachedOrFetch('toolConfigurations', () =>
				invokeCommand<Record<string, ToolCategory>>("get_tool_configurations")
			);
			setToolConfigurations(configs);
		} catch (error) {
			console.error("Error loading tool configurations:", error);
			toast.error("Failed to load tool configurations");
		} finally {
			setToolConfigLoading(false);
		}
	}, [invokeCommand]);

	const loadMcpServers = useCallback(async () => {
		setMcpLoading(true);
		try {
			const servers = await invokeCommand<MCPServerConfig[]>("get_mcp_servers");
			setMcpServers(servers);

			const statuses = await invokeCommand<Record<string, MCPServerStatus>>("get_mcp_server_statuses");
			setMcpServerStatuses(statuses);

			const tools = await invokeCommand<MCPToolInfo[]>("get_mcp_tools");
			setMcpTools(tools);
		} catch (error) {
			console.error("Error loading MCP servers:", error);
			toast.error(`Failed to load MCP servers: ${error}`);
		} finally {
			setMcpLoading(false);
		}
	}, [invokeCommand]);

	// Handler functions
	const handleTtsProviderChange = useCallback(async (newProvider: string) => {
		await invokeCommand(
			"set_tts_provider_command",
			{ provider: newProvider },
			{
				showSuccessToast: true,
				successMessage: `TTS provider set to: ${newProvider === "off"
					? "Off"
					: newProvider.charAt(0).toUpperCase() + newProvider.slice(1)
					}`,
				errorMessage: "Failed to set TTS provider"
			}
		);
		invalidateCache('ttsProvider');
		setTtsProvider(newProvider);
	}, [invokeCommand]);

	const handleActiveProviderChange = useCallback(async (providerId: string) => {
		await invokeCommand("set_active_provider", { providerId });
		setActiveProvider(providerId);

		const settings = await invokeCommand<ProviderSettings>("get_provider_settings", {
			providerId,
		});
		setProviderSettings(settings);
		setFormData({
			apiKey: settings.api_key || "",
			model: settings.model || "",
			maxTokens: settings.max_tokens?.toString() || "",
			temperature: settings.temperature?.toString() || "",
			systemPrompt: settings.system_prompt || "",
		});

		toast.success(`Active AI provider set to: ${providerId}`);
	}, [invokeCommand]);

	const handleSaveProviderSettings = async () => {
		if (!activeProvider) {
			toast.error("No provider selected");
			return;
		}

		try {
			// Update API key
			if (formData.apiKey !== providerSettings?.api_key) {
				await invoke("update_provider_api_key", {
					providerId: activeProvider,
					apiKey: formData.apiKey,
				});
			}

			// Update model
			if (formData.model !== providerSettings?.model) {
				await invoke("update_provider_model", {
					providerId: activeProvider,
					model: formData.model,
				});
			}

			// Update max tokens
			if (formData.maxTokens && formData.maxTokens !== providerSettings?.max_tokens?.toString()) {
				await invoke("update_provider_max_tokens", {
					providerId: activeProvider,
					maxTokens: parseInt(formData.maxTokens),
				});
			}

			// Update temperature
			if (formData.temperature && formData.temperature !== providerSettings?.temperature?.toString()) {
				await invoke("update_provider_temperature", {
					providerId: activeProvider,
					temperature: parseFloat(formData.temperature),
				});
			}

			// Update system prompt
			if (formData.systemPrompt !== providerSettings?.system_prompt) {
				await invoke("update_provider_system_prompt", {
					providerId: activeProvider,
					systemPrompt: formData.systemPrompt,
				});
			}

			toast.success("Provider settings saved successfully");
			await loadAllSettings(); // Reload to get updated settings
		} catch (error) {
			console.error("Failed to save provider settings:", error);
			toast.error("Failed to save provider settings");
		}
	};

	const handleSoundEnabledChange = useCallback(async (enabled: boolean) => {
		await invokeCommand(
			"set_sound_enabled",
			{ enabled },
			{
				showSuccessToast: true,
				successMessage: `Sound ${enabled ? "enabled" : "disabled"}`,
				errorMessage: "Failed to update sound setting"
			}
		);
		setSoundEnabled(enabled);
	}, [invokeCommand]);

	const handlePerformanceMonitoringChange = useCallback(async (enabled: boolean) => {
		await invokeCommand(
			"set_performance_monitoring",
			{ enabled },
			{
				showSuccessToast: true,
				successMessage: `Performance monitoring ${enabled ? "enabled" : "disabled"}`,
				errorMessage: "Failed to update performance monitoring setting"
			}
		);
		setPerformanceMonitoringEnabled(enabled);
	}, [invokeCommand]);

	const handleAgentModeChange = useCallback(async (newMode: string) => {
		await invokeCommand(
			"set_agent_mode",
			{ mode: newMode },
			{
				showSuccessToast: true,
				successMessage: `Agent mode set to: ${newMode}`,
				errorMessage: "Failed to set agent mode"
			}
		);
		setAgentMode(newMode);
	}, [invokeCommand]);

	const handleAgentTriggerModeChange = async (newMode: string) => {
		try {
			await invoke("set_agent_trigger_mode", { mode: newMode });
			setAgentTriggerMode(newMode);
			toast.success(`Agent trigger mode set to: ${newMode === "tap" ? "Tap to Toggle" : "Hold to Activate"}`);
		} catch (error) {
			console.error("Failed to set agent trigger mode:", error);
			toast.error("Failed to set agent trigger mode");
		}
	};

	const handleDictationClipboardChange = async (enabled: boolean) => {
		try {
			await invoke("set_dictation_clipboard_enabled", { enabled });
			invalidateCache('dictationClipboardEnabled');
			setDictationClipboardEnabled(enabled);
			toast.success(`Dictation clipboard ${enabled ? "enabled" : "disabled"}`);
		} catch (error) {
			console.error("Failed to set dictation clipboard:", error);
			toast.error("Failed to update dictation setting");
		}
	};

	const handleAlwaysListeningToggle = async () => {
		try {
			const newState = await invoke<boolean>("toggle_always_listening_mode");
			setAlwaysListeningActive(newState);
			toast.success(`Always listening ${newState ? "enabled" : "disabled"}`);
		} catch (error) {
			console.error("Failed to toggle always listening:", error);
			toast.error("Failed to toggle always listening");
		}
	};

	const handleSensitivityChange = async (sensitivity: number) => {
		try {
			await invoke("set_always_listening_sensitivity", { sensitivity });
			setAlwaysListeningSensitivity(sensitivity);
		} catch (error) {
			console.error("Failed to set sensitivity:", error);
			toast.error("Failed to set sensitivity");
		}
	};

	const handleWakeWordsChange = async () => {
		try {
			const wakeWords = wakeWordsInput
				.split(",")
				.map((word) => word.trim())
				.filter((word) => word.length > 0);
			await invoke("set_always_listening_wake_words", { wakeWords });
			setAlwaysListeningWakeWords(wakeWords);
			toast.success("Wake words updated successfully");
		} catch (error) {
			console.error("Failed to set wake words:", error);
			toast.error("Failed to set wake words");
		}
	};

	return {
		// State
		ttsProvider,
		providers,
		activeProvider,
		providerSettings,
		isLoading,
		agentMode,
		agentTriggerMode,
		dictationClipboardEnabled,
		soundEnabled,
		performanceMonitoringEnabled,
		alwaysListeningActive,
		alwaysListeningSensitivity,
		alwaysListeningWakeWords,
		wakeWordsInput,
		setWakeWordsInput,
		toolConfigurations,
		toolConfigLoading,
		mcpServers,
		mcpServerStatuses,
		mcpTools,
		mcpLoading,
		mcpJsonData,
		setMcpJsonData,
		formData,
		setFormData,
		permissionsState,
		permissionsLoading,
		keyboardShortcuts,
		shortcutsLoading,
		editingShortcut,
		setEditingShortcut,

		// Actions
		loadAllSettings,
		handleTtsProviderChange,
		handleActiveProviderChange,
		handleSaveProviderSettings,
		handleSoundEnabledChange,
		handlePerformanceMonitoringChange,
		handleAgentModeChange,
		handleAgentTriggerModeChange,
		handleDictationClipboardChange,
		handleAlwaysListeningToggle,
		handleSensitivityChange,
		handleWakeWordsChange,
		loadPermissionsStatus,
		loadKeyboardShortcuts,
		loadToolConfigurations,
		loadMcpServers,
	};
}
