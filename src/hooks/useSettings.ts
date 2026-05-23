import { useState, useEffect, useCallback } from "react";
import { toast } from "sonner";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { KeyboardShortcuts } from "@/types/keyboard";
import { AUDIO } from "@/lib/constants.generated";
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
interface CachedValue<T> {
	value: T;
	timestamp: number;
}

interface WhisperModelInfo {
	id: string;
	filename: string;
	display_name: string;
	size_mb: number;
	downloaded: boolean;
	is_default: boolean;
}

interface WhisperDownloadProgress {
	model_id: string;
	bytes_downloaded: number;
	total_bytes: number;
	percent: number;
}

interface SettingsCache {
	ttsProvider?: CachedValue<string>;
	dictationClipboardEnabled?: CachedValue<boolean>;
	dictationTriggerMode?: CachedValue<string>;
	soundEnabled?: CachedValue<boolean>;
	toolConfigurations?: CachedValue<Record<string, ToolCategory>>;
	providers?: CachedValue<ProviderInfo[]>;
	activeProvider?: CachedValue<string>;
	agentMode?: CachedValue<string>;
	agentTriggerMode?: CachedValue<string>;
	alwaysListeningActive?: CachedValue<boolean>;
	alwaysListeningSensitivity?: CachedValue<number>;
	alwaysListeningWakeWords?: CachedValue<string[]>;
	performanceMonitoringEnabled?: CachedValue<boolean>;
	permissionsState?: CachedValue<PermissionsState>;
	keyboardShortcuts?: CachedValue<KeyboardShortcuts>;
	mcpServers?: CachedValue<MCPServerConfig[]>;
	mcpServerStatuses?: CachedValue<Record<string, MCPServerStatus>>;
	whisperModels?: CachedValue<WhisperModelInfo[]>;
	currentWhisperModel?: CachedValue<string>;
}

// Cache with 30-second TTL to prevent excessive API calls
const CACHE_TTL = 30000; // 30 seconds
let settingsCache: SettingsCache = {};
const ongoingRequests = new Map<string, Promise<any>>();

// Helper to check if cache is valid
const isCacheValid = (cacheKey: keyof SettingsCache): boolean => {
	const cachedItem = settingsCache[cacheKey];
	if (!cachedItem) return false;
	return Date.now() - cachedItem.timestamp < CACHE_TTL;
};

// Helper to get cached value or make API call
const getCachedOrFetch = async <T>(
	cacheKey: keyof SettingsCache,
	apiCall: () => Promise<T>
): Promise<T> => {
	// Return cached value if valid
	if (isCacheValid(cacheKey)) {
		return (settingsCache[cacheKey] as CachedValue<T>).value;
	}

	// Check if request is already in progress
	if (ongoingRequests.has(cacheKey)) {
		return ongoingRequests.get(cacheKey) as Promise<T>;
	}

	// Start new request
	const request = apiCall().then((result) => {
		(settingsCache as any)[cacheKey] = {
			value: result,
			timestamp: Date.now()
		};
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

	// Chatterbox TTS Settings
	const [chatterboxReferenceAudioUrl, setChatterboxReferenceAudioUrl] = useState<string>("");
	const [chatterboxExaggeration, setChatterboxExaggeration] = useState<number>(0.5);
	const [chatterboxUseHd, setChatterboxUseHd] = useState<boolean>(false);

	// Supertonic TTS Settings
	const [supertonicServerUrl, setSupertonicServerUrl] = useState<string>("http://localhost:8000");
	const [supertonicVoice, setSupertonicVoice] = useState<string>("M1");
	const [supertonicSpeed, setSupertonicSpeed] = useState<number>(1.05);

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
	const [dictationTriggerMode, setDictationTriggerMode] = useState<string>("hold"); // Default to existing hold behavior

	// Sound Settings
	const [soundEnabled, setSoundEnabled] = useState<boolean>(true);

	// Performance Monitoring Settings
	const [performanceMonitoringEnabled, setPerformanceMonitoringEnabled] = useState<boolean>(true);

	// Always Listening Settings
	const [alwaysListeningActive, setAlwaysListeningActive] = useState<boolean>(false);
	const [alwaysListeningSensitivity, setAlwaysListeningSensitivity] = useState<number>(0.5);
	const [alwaysListeningWakeWords, setAlwaysListeningWakeWords] = useState<string[]>([...AUDIO.DEFAULT_WAKE_WORDS]);
	const [wakeWordsInput, setWakeWordsInput] = useState<string>("");

	// Whisper Model Settings
	const [whisperModels, setWhisperModels] = useState<WhisperModelInfo[]>([]);
	const [currentWhisperModel, setCurrentWhisperModel] = useState<string>("large-v3-turbo");
	const [whisperDownloading, setWhisperDownloading] = useState<string | null>(null);
	const [whisperDownloadProgress, setWhisperDownloadProgress] = useState<WhisperDownloadProgress | null>(null);

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
		agent_mode: "",
		dictation_input: "",
		stop_current_task: "",
		open_settings: "",
		voice_activation: "",
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
		let mounted = true;

		const setupMcpListener = async () => {
			try {
				const fn = await listen<{
					servers: MCPServerConfig[];
					statuses: Record<string, MCPServerStatus>;
					tools: MCPToolInfo[];
				}>("mcp_state_updated", (event) => {
					if (!mounted) return;
					console.log("Received MCP state update:", event.payload);
					setMcpServers(event.payload.servers);
					setMcpServerStatuses(event.payload.statuses);
					setMcpTools(event.payload.tools);
				});
				if (mounted) {
					unlisten = fn;
				} else {
					fn();
				}
			} catch (error) {
				console.error("Failed to setup MCP listener:", error);
			}
		};

		setupMcpListener();
		return () => {
			mounted = false;
			unlisten?.();
		};
	}, []);

	// Listen for provider settings changes from backend
	useEffect(() => {
		let unlisten: (() => void) | undefined;
		let mounted = true;

		const setupProviderListener = async () => {
			try {
				const fn = await listen<{
					active_provider: string;
					providers: {
						id: string;
						api_key?: string;
						model?: string;
						max_tokens?: number;
						temperature?: number;
						system_prompt?: string;
					}[];
				}>("provider_settings_changed", async (event) => {
					if (!mounted) return;
					console.log("useSettings: Received provider settings update:", event.payload);
					const fullProviderSettings = event.payload;

					// Update active provider
					setActiveProvider(fullProviderSettings.active_provider);

					// Find the current provider's settings
					const currentProviderSettings = fullProviderSettings.providers.find(
						p => p.id === fullProviderSettings.active_provider
					);

					if (currentProviderSettings) {
						console.log("useSettings: Updating provider settings for:", fullProviderSettings.active_provider);

						// Update provider settings state
						setProviderSettings(currentProviderSettings);

						// Update form data to reflect the changes
						setFormData({
							apiKey: currentProviderSettings.api_key || "",
							model: currentProviderSettings.model || "",
							maxTokens: currentProviderSettings.max_tokens?.toString() || "",
							temperature: currentProviderSettings.temperature?.toString() || "",
							systemPrompt: currentProviderSettings.system_prompt || "",
						});
					} else {
						console.warn("useSettings: Could not find settings for active provider:", fullProviderSettings.active_provider);
					}

					// Invalidate cache to force fresh data on next request
					invalidateCache();

					// Re-fetch providers to update is_available after API key changes
					try {
						const freshProviders = await invokeCommand<ProviderInfo[]>("get_providers");
						if (mounted) {
							setProviders(freshProviders);
						}
					} catch (err) {
						console.warn("useSettings: Failed to refresh providers after settings change:", err);
					}
				});
				if (mounted) {
					unlisten = fn;
				} else {
					fn();
				}
			} catch (error) {
				console.error("Failed to setup provider listener:", error);
			}
		};

		setupProviderListener();
		return () => {
			mounted = false;
			unlisten?.();
		};
	}, []); // No deps needed — handler always gets latest state via event payload

	// Listen for whisper model download events
	useEffect(() => {
		let unlistenProgress: (() => void) | undefined;
		let unlistenComplete: (() => void) | undefined;
		let unlistenError: (() => void) | undefined;
		let mounted = true;

		const setup = async () => {
			try {
				const fnProgress = await listen<WhisperDownloadProgress>(
					"whisper-download-progress",
					(event) => {
						if (!mounted) return;
						setWhisperDownloadProgress(event.payload);
					}
				);
				const fnComplete = await listen<{ model_id: string }>(
					"whisper-download-complete",
					(event) => {
						if (!mounted) return;
						setWhisperDownloading(null);
						setWhisperDownloadProgress(null);
						setCurrentWhisperModel(event.payload.model_id);
						invalidateCache("whisperModels");
						invalidateCache("currentWhisperModel");
					}
				);
				const fnError = await listen<{ model_id: string; error: string }>(
					"whisper-download-error",
					(event) => {
						if (!mounted) return;
						console.error("Whisper download error:", event.payload.error);
						setWhisperDownloading(null);
						setWhisperDownloadProgress(null);
					}
				);
				if (mounted) {
					unlistenProgress = fnProgress;
					unlistenComplete = fnComplete;
					unlistenError = fnError;
				} else {
					fnProgress();
					fnComplete();
					fnError();
				}
			} catch (error) {
				console.error("Failed to setup whisper download listeners:", error);
			}
		};

		setup();
		return () => {
			mounted = false;
			unlistenProgress?.();
			unlistenComplete?.();
			unlistenError?.();
		};
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
				currentDictationTriggerMode,
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
				getCachedOrFetch('dictationTriggerMode', () => invokeCommand<string>("get_dictation_trigger_mode")),
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
			setDictationTriggerMode(currentDictationTriggerMode);
			setSoundEnabled(currentSoundEnabled);
			setPerformanceMonitoringEnabled(currentPerformanceMonitoringEnabled);
			setAlwaysListeningActive(alwaysListeningStatus);
			setAlwaysListeningSensitivity(sensitivity);
			setAlwaysListeningWakeWords(wakeWords);
			setWakeWordsInput(wakeWords.join(", "));

			// Load Chatterbox-specific settings
			try {
				const chatterboxSettings = await invokeCommand<{
					reference_audio_url: string | null;
					exaggeration: number;
					use_hd: boolean;
				}>("get_chatterbox_settings_command");
				setChatterboxReferenceAudioUrl(chatterboxSettings.reference_audio_url ?? "");
				setChatterboxExaggeration(chatterboxSettings.exaggeration);
				setChatterboxUseHd(chatterboxSettings.use_hd);
			} catch (error) {
				console.warn("Failed to load Chatterbox settings:", error);
			}

			// Load Supertonic-specific settings
			try {
				const stSettings = await invokeCommand<{
					server_url: string;
					voice: string;
					speed: number;
				}>("get_supertonic_settings_command");
				setSupertonicServerUrl(stSettings.server_url);
				setSupertonicVoice(stSettings.voice);
				setSupertonicSpeed(stSettings.speed);
			} catch (error) {
				console.warn("Failed to load Supertonic settings:", error);
			}

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

			// Load whisper model info
			await loadWhisperModels();

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
				invokeCommand<PermissionsState>("check_permissions_status_native")
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
			const configs = await getCachedOrFetch('toolConfigurations', async () => {
				console.log("🔄 Loading tool configurations from backend...");

				// Use the batch API endpoint to get all tool configurations in a single call
				const toolConfigsResponse = await invokeCommand<Record<string, {
					name: string;
					description: string;
					enabled: boolean;
					tools: Array<{
						name: string;
						category: string;
						enabled: boolean;
						description: string;
						required: boolean;
						server_id?: string;
					}>;
				}>>("get_tool_configurations");

				console.log(`📊 Loaded ${Object.keys(toolConfigsResponse).length} tool categories from backend`);

				// Transform the response to match our TypeScript ToolCategory interface
				const transformedConfigs: Record<string, ToolCategory> = {};

				for (const [categoryKey, categoryData] of Object.entries(toolConfigsResponse)) {
					// Transform tools to match ToolConfig interface
					const transformedTools = categoryData.tools.map(tool => ({
						name: tool.name,
						category: tool.category,
						enabled: tool.enabled,
						description: tool.description,
						required: tool.required,
					}));

					transformedConfigs[categoryKey] = {
						name: categoryData.name,
						description: categoryData.description,
						enabled: categoryData.enabled,
						tools: transformedTools,
					};
				}

				console.log(`✅ Transformed ${Object.keys(transformedConfigs).length} tool categories for frontend`);
				return transformedConfigs;
			});

			setToolConfigurations(configs);
		} catch (error) {
			console.error("Error loading tool configurations:", error);
			toast.error(`Failed to load tool configurations: ${error}`);

			// Set empty configurations on error to prevent UI issues
			setToolConfigurations({});
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

	const handleChatterboxSettingsChange = useCallback(async (
		referenceAudioUrl: string,
		exaggeration: number,
		useHd: boolean,
	) => {
		await invokeCommand(
			"set_chatterbox_settings_command",
			{
				referenceAudioUrl: referenceAudioUrl || null,
				exaggeration,
				useHd,
			},
			{
				showSuccessToast: true,
				successMessage: "Chatterbox settings saved",
				errorMessage: "Failed to save Chatterbox settings",
			}
		);
		setChatterboxReferenceAudioUrl(referenceAudioUrl);
		setChatterboxExaggeration(exaggeration);
		setChatterboxUseHd(useHd);
	}, [invokeCommand]);

	const handleSupertonicSettingsChange = useCallback(async (
		serverUrl: string,
		voice: string,
		speed: number,
	) => {
		await invokeCommand(
			"set_supertonic_settings_command",
			{ serverUrl, voice, speed },
			{
				showSuccessToast: true,
				successMessage: "Supertonic settings saved",
				errorMessage: "Failed to save Supertonic settings",
			}
		);
		setSupertonicServerUrl(serverUrl);
		setSupertonicVoice(voice);
		setSupertonicSpeed(speed);
	}, [invokeCommand]);

	const handleActiveProviderChange = useCallback(async (providerId: string) => {
		try {
			console.log(`Switching active provider to: ${providerId}`);
			await invokeCommand("set_active_provider", { providerId });
			setActiveProvider(providerId);

			// Invalidate cache for fresh data
			invalidateCache('activeProvider');
			invalidateCache('providers');

			// Load settings specifically for the new provider
			console.log(`Loading settings for provider: ${providerId}`);
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

			console.log(`Active AI provider set to: ${providerId}`, { settings });
			toast.success(`Active AI provider set to: ${providerId}`);
		} catch (error) {
			console.error("Failed to change active provider:", error);
			toast.error(`Failed to change provider: ${error}`);
		}
	}, [invokeCommand]);

	// Debug function to check current settings state
	const debugSettings = useCallback(() => {
		console.log("=== Settings Debug Info ===");
		console.log("Active Provider:", activeProvider);
		console.log("Available Providers:", providers);
		console.log("Provider Settings:", providerSettings);
		console.log("Form Data:", formData);
		console.log("Is Loading:", isLoading);
		console.log("========================");
	}, [activeProvider, providers, providerSettings, formData, isLoading]);

	const handleSaveProviderSettings = async () => {
		if (!activeProvider) {
			toast.error("No provider selected");
			return;
		}

		try {
			console.log("Saving provider settings changes...");

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

			// Only reload the specific provider settings instead of all settings
			console.log("Reloading specific provider settings...");
			const updatedSettings = await invokeCommand<ProviderSettings>("get_provider_settings", {
				providerId: activeProvider,
			});
			setProviderSettings(updatedSettings);

			// Update form data to reflect saved changes
			setFormData({
				apiKey: updatedSettings.api_key || "",
				model: updatedSettings.model || "",
				maxTokens: updatedSettings.max_tokens?.toString() || "",
				temperature: updatedSettings.temperature?.toString() || "",
				systemPrompt: updatedSettings.system_prompt || "",
			});

			toast.success("Provider settings saved successfully");
			console.log("Provider settings saved and reloaded successfully");
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

	const handleDictationTriggerModeChange = async (newMode: string) => {
		try {
			await invoke("set_dictation_trigger_mode", { mode: newMode });
			invalidateCache('dictationTriggerMode');
			setDictationTriggerMode(newMode);
			toast.success(`Dictation trigger mode set to: ${newMode === "tap" ? "Tap to Toggle" : "Hold to Activate"}`);
		} catch (error) {
			console.error("Failed to set dictation trigger mode:", error);
			toast.error("Failed to set dictation trigger mode");
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

	const loadWhisperModels = useCallback(async () => {
		try {
			const [models, current] = await Promise.all([
				getCachedOrFetch("whisperModels", () =>
					invokeCommand<WhisperModelInfo[]>("get_whisper_models")
				),
				getCachedOrFetch("currentWhisperModel", () =>
					invokeCommand<string>("get_current_whisper_model")
				),
			]);
			setWhisperModels(models);
			setCurrentWhisperModel(current);
		} catch (error) {
			console.error("Error loading whisper models:", error);
		}
	}, [invokeCommand]);

	const handleWhisperModelDownload = useCallback(async (modelId: string) => {
		try {
			setWhisperDownloading(modelId);
			setWhisperDownloadProgress(null);
			await invokeCommand("download_whisper_model", { modelId });
		} catch (error) {
			console.error("Failed to start whisper model download:", error);
			setWhisperDownloading(null);
			toast.error(`Failed to download model: ${error}`);
		}
	}, [invokeCommand]);

	const handleWhisperModelChange = useCallback(async (modelId: string) => {
		try {
			await invokeCommand(
				"set_whisper_model",
				{ modelId },
				{
					showSuccessToast: true,
					successMessage: "Whisper model switched",
					errorMessage: "Failed to switch model",
				}
			);
			setCurrentWhisperModel(modelId);
			invalidateCache("currentWhisperModel");
		} catch (error) {
			console.error("Failed to switch whisper model:", error);
		}
	}, [invokeCommand]);

	const invalidateToolConfigCache = useCallback(() => {
		invalidateCache('toolConfigurations');
	}, []);

	return {
		// State
		ttsProvider,
		chatterboxReferenceAudioUrl,
		chatterboxExaggeration,
		chatterboxUseHd,
		supertonicServerUrl,
		supertonicVoice,
		supertonicSpeed,
		providers,
		activeProvider,
		providerSettings,
		isLoading,
		agentMode,
		agentTriggerMode,
		dictationClipboardEnabled,
		dictationTriggerMode,
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

		// Whisper model
		whisperModels,
		currentWhisperModel,
		whisperDownloading,
		whisperDownloadProgress,

		// Actions
		loadAllSettings,
		handleTtsProviderChange,
		handleChatterboxSettingsChange,
		handleSupertonicSettingsChange,
		handleActiveProviderChange,
		handleSaveProviderSettings,
		handleSoundEnabledChange,
		handlePerformanceMonitoringChange,
		handleAgentModeChange,
		handleAgentTriggerModeChange,
		handleDictationClipboardChange,
		handleDictationTriggerModeChange,
		handleAlwaysListeningToggle,
		handleSensitivityChange,
		handleWakeWordsChange,
		loadPermissionsStatus,
		loadKeyboardShortcuts,
		loadToolConfigurations,
		loadWhisperModels,
		handleWhisperModelDownload,
		handleWhisperModelChange,
		setToolConfigurations,
		invalidateToolConfigCache,
		loadMcpServers,
		debugSettings,
	};
}
