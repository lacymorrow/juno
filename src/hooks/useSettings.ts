import { useState, useEffect, useCallback, useRef } from "react";
import { toast } from "sonner";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { KeyboardShortcuts } from "@/types/keyboard";
import { AUDIO, SETTINGS, COMMANDS, EVENTS } from "@/lib/constants.generated";
import { useEventListener } from "@/hooks/useEventListener";
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

/** One microphone or speaker, as `list_audio_devices` reports it. */
export interface AudioDeviceEntry {
	name: string;
	is_default: boolean;
}

/** Everything the Audio pane needs to draw both device pickers. */
export interface AudioDeviceChoices {
	inputs: AudioDeviceEntry[];
	outputs: AudioDeviceEntry[];
	chosen_input: string | null;
	chosen_output: string | null;
	/** What the next dictation will actually open. */
	effective_input: string | null;
	/** The chosen microphone, when it is not connected. */
	missing_input: string | null;
	/** The chosen speaker, when it is not connected. */
	missing_output: string | null;
}

/** One row of the "Juno's voice" picker. */
export interface JunoVoiceOption {
	id: string;
	/** "silent", "system" or "voice". */
	kind: string;
	name: string;
	descriptor: string;
	selected: boolean;
	/** False for silence, which has nothing to audition. */
	speaks: boolean;
}

/**
 * Everything the Audio pane draws for Juno's voice, as Rust decided it.
 *
 * The rows belong to the engine that is speaking: Rust enumerates what that
 * engine actually offers, resolves the stored choice against it, and says
 * which row is in force. Nothing here is computed in TypeScript, because a
 * list computed twice is a list that disagrees with itself.
 */
export interface JunoVoiceList {
	/** The stored engine. "off" means Juno is silent. */
	provider: string;
	/** The engine these rows belong to. */
	engine: string;
	/** That engine's name as a person would say it. */
	engine_label: string;
	options: JunoVoiceOption[];
	/** One sentence when there is something to say instead of rows. */
	note: string | null;
	/** The engines the advanced picker offers. Silence is not one: it is a row. */
	engines: { id: string; name: string }[];
}

/**
 * How the sample is getting on.
 *
 * Reported rather than awaited: choosing a voice answers immediately with the
 * list Rust decided on, and the sound that follows says what it is doing.
 * "preparing" arrives before anything slow starts, which is what stops a
 * local engine loading its model from looking like a dead pane.
 */
export interface VoiceAudition {
	voice: string;
	engine: string;
	state: "preparing" | "speaking" | "done" | "failed";
	message: string | null;
}

/**
 * Why the microphone never opened, in the words Rust chose.
 *
 * `listening` is always false: this event exists so that no surface goes on
 * claiming Juno is listening after the capture thread has gone.
 */
export interface CaptureFailure {
	code: string;
	message: string;
	device: string | null;
	listening: boolean;
}

// Global cache to prevent duplicate API calls during startup
interface CachedValue<T> {
	value: T;
	timestamp: number;
}

interface SettingsCache {
	ttsProvider?: CachedValue<string>;
	dictationClipboardEnabled?: CachedValue<boolean>;
	dictationInsertionMode?: CachedValue<string>;
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
	livePartialTranscription?: CachedValue<boolean>;
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
	const [agentMode, setAgentMode] = useState<string>("single");

	// Agent Trigger Mode Settings
	const [agentTriggerMode, setAgentTriggerMode] = useState<string>("tap");

	// Dictation Settings
	const [dictationClipboardEnabled, setDictationClipboardEnabled] = useState<boolean>(true);
	const [dictationInsertionMode, setDictationInsertionMode] = useState<string>("paste");
	const [dictationTriggerMode, setDictationTriggerMode] = useState<string>("hold"); // Default to existing hold behavior

	// Sound Settings
	const [soundEnabled, setSoundEnabled] = useState<boolean>(true);

	// Performance Monitoring Settings
	const [performanceMonitoringEnabled, setPerformanceMonitoringEnabled] = useState<boolean>(true);

	// Microphone and speaker choices. Rust enumerates, chooses and persists;
	// this only holds what it said.
	const [audioDevices, setAudioDevices] = useState<AudioDeviceChoices | null>(null);

	// Juno's voice: the rows the active engine has, and which one is in force.
	// Null until Rust has answered, which is a different state from "no voices".
	const [junoVoices, setJunoVoices] = useState<JunoVoiceList | null>(null);
	const [voiceAudition, setVoiceAudition] = useState<VoiceAudition | null>(null);

	// The last capture failure, in the words Rust chose. Null when the
	// microphone is fine. Shown in the Audio pane, which is where the
	// microphone is chosen.
	const [captureFailure, setCaptureFailure] = useState<CaptureFailure | null>(null);

	// Always Listening Settings
	const [alwaysListeningActive, setAlwaysListeningActive] = useState<boolean>(false);
	const [alwaysListeningSensitivity, setAlwaysListeningSensitivity] = useState<number>(0.5);
	const [alwaysListeningWakeWords, setAlwaysListeningWakeWords] = useState<string[]>([...AUDIO.DEFAULT_WAKE_WORDS]);
	const [wakeWordsInput, setWakeWordsInput] = useState<string>("");


	// STT engine (dictation model) settings
	const [livePartialTranscription, setLivePartialTranscriptionState] = useState<boolean>(false);

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
				}>(EVENTS.SYSTEM_MCP_STATE_UPDATED, (event) => {
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
				}>(EVENTS.SYSTEM_PROVIDER_SETTINGS_CHANGED, async (event) => {
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
						const freshProviders = await invokeCommand<ProviderInfo[]>(COMMANDS.PROVIDERS_GET_PROVIDERS);
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

	// Each window (Settings, chat pane) runs its own copy of this hook, so a
	// change made in one never reached the other. The backend emits the full
	// settings on every save; follow the agent mode from there.
	useEventListener<{ agent?: { execution_mode?: string } }>(
		SETTINGS.EVENTS_SETTINGS_CHANGED,
		(payload) => {
			// This event carries the whole settings file, so everything the
			// cache above is holding is now out of date. Dropping all of it,
			// rather than the one key this handler goes on to read, is what
			// keeps a pane added later honest: it follows its own section
			// event and gets fresh values when it asks for them.
			invalidateCache();
			const mode = payload?.agent?.execution_mode;
			if (!mode) return;
			setAgentMode(mode);
		},
	);

	/**
	 * Read every setting back out of Rust and redraw from it.
	 *
	 * The cache below exists to collapse the burst of identical reads several
	 * windows make while they mount, and it holds each value for 30 seconds.
	 * An explicit load is not part of that burst: it is somebody asking what
	 * the settings are *now*, so it starts by throwing the cache away.
	 *
	 * Without that, "Reset all settings" wrote the defaults, reloaded, and was
	 * served the pre-reset values straight back out of the cache, so every
	 * pane redrew exactly what it already showed while the toast said the
	 * reset had worked. `ongoingRequests` still dedupes genuinely concurrent
	 * calls, which is the part of the cache that was earning its keep.
	 */
	const loadAllSettings = useCallback(async () => {
		setIsLoading(true);
		invalidateCache();
		try {
			// Load all settings with caching to prevent duplicate API calls during startup
			const [
				availableProviders,
				currentActiveProvider,
				currentAgentMode,
				currentAgentTriggerMode,
				currentClipboardEnabled,
				currentInsertionMode,
				currentDictationTriggerMode,
				currentSoundEnabled,
				currentPerformanceMonitoringEnabled,
				alwaysListeningStatus,
				sensitivity,
				wakeWords
			] = await Promise.all([
				getCachedOrFetch('providers', () => invokeCommand<ProviderInfo[]>(COMMANDS.PROVIDERS_GET_PROVIDERS)),
				getCachedOrFetch('activeProvider', () => invokeCommand<string>(COMMANDS.PROVIDERS_GET_ACTIVE_PROVIDER)),
				getCachedOrFetch('agentMode', () => invokeCommand<string>(COMMANDS.AGENT_GET_AGENT_MODE)),
				getCachedOrFetch('agentTriggerMode', () => invokeCommand<string>(COMMANDS.AGENT_GET_AGENT_TRIGGER_MODE)),
				getCachedOrFetch('dictationClipboardEnabled', () => invokeCommand<boolean>(COMMANDS.DICTATION_GET_DICTATION_CLIPBOARD_ENABLED)),
				getCachedOrFetch('dictationInsertionMode', () => invokeCommand<string>(COMMANDS.DICTATION_GET_DICTATION_INSERTION_MODE)),
				getCachedOrFetch('dictationTriggerMode', () => invokeCommand<string>(COMMANDS.DICTATION_GET_DICTATION_TRIGGER_MODE)),
				getCachedOrFetch('soundEnabled', () => invokeCommand<boolean>(COMMANDS.SOUND_GET_SOUND_ENABLED)),
				getCachedOrFetch('performanceMonitoringEnabled', () => invokeCommand<boolean>(COMMANDS.CORE_GET_PERFORMANCE_MONITORING)),
				getCachedOrFetch('alwaysListeningActive', () => invokeCommand<boolean>(COMMANDS.ALWAYS_LISTENING_GET_ALWAYS_LISTENING_STATUS)),
				getCachedOrFetch('alwaysListeningSensitivity', () => invokeCommand<number>(COMMANDS.ALWAYS_LISTENING_GET_ALWAYS_LISTENING_SENSITIVITY)),
				getCachedOrFetch('alwaysListeningWakeWords', () => invokeCommand<string[]>(COMMANDS.ALWAYS_LISTENING_GET_ALWAYS_LISTENING_WAKE_WORDS))
			]);

			// Set all state values
			// The engine comes with its voices, from one command, so the engine
			// picker and the voice rows are never drawn from two reads taken
			// at different moments.
			void loadJunoVoices();
			setProviders(availableProviders);
			setActiveProvider(currentActiveProvider);
			setAgentMode(currentAgentMode);
			setAgentTriggerMode(currentAgentTriggerMode);
			setDictationClipboardEnabled(currentClipboardEnabled);
			setDictationInsertionMode(currentInsertionMode);
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
				}>(COMMANDS.TTS_GET_CHATTERBOX_SETTINGS);
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
				}>(COMMANDS.TTS_GET_SUPERTONIC_SETTINGS);
				setSupertonicServerUrl(stSettings.server_url);
				setSupertonicVoice(stSettings.voice);
				setSupertonicSpeed(stSettings.speed);
			} catch (error) {
				console.warn("Failed to load Supertonic settings:", error);
			}

			if (currentActiveProvider) {
				const settings = await invokeCommand<ProviderSettings>(COMMANDS.PROVIDERS_GET_PROVIDER_SETTINGS, {
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

			// Load the live-partial dictation display mode
			await loadLivePartialSetting();

			// Load tool configurations with caching
			await loadToolConfigurations();

			// Load keyboard shortcuts with caching
			await loadKeyboardShortcuts();

			// Load MCP server configurations with caching
			await loadMcpServers();

			console.log("All settings loaded successfully with caching");
		} catch (error) {
			console.error("Error loading settings:", error);
			// One id: a repeat load (StrictMode, a remount) replaces the toast
			// instead of stacking another copy of it.
			toast.error("Some settings could not be loaded", { id: "settings-load" });
		} finally {
			setIsLoading(false);
		}
	}, [invokeCommand]);

	const loadPermissionsStatus = useCallback(async () => {
		setPermissionsLoading(true);
		try {
			const permissions = await getCachedOrFetch('permissionsState', () =>
				invokeCommand<PermissionsState>(COMMANDS.PERMISSIONS_CHECK_PERMISSIONS_STATUS)
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
				invokeCommand<KeyboardShortcuts>(COMMANDS.SHORTCUTS_GET_KEYBOARD_SHORTCUTS)
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
				}>>(COMMANDS.TOOLS_GET_TOOL_CONFIGURATIONS);

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
			const servers = await invokeCommand<MCPServerConfig[]>(COMMANDS.MCP_GET_MCP_SERVERS);
			setMcpServers(servers);

			const statuses = await invokeCommand<Record<string, MCPServerStatus>>(COMMANDS.MCP_GET_MCP_SERVER_STATUSES);
			setMcpServerStatuses(statuses);

			const tools = await invokeCommand<MCPToolInfo[]>(COMMANDS.MCP_GET_MCP_TOOLS);
			setMcpTools(tools);
		} catch (error) {
			console.error("Error loading MCP servers:", error);
			toast.error(`Failed to load MCP servers: ${error}`);
		} finally {
			setMcpLoading(false);
		}
	}, [invokeCommand]);

	// Re-read the device lists. Cheap and read-only (listing never opens a
	// device), and deliberately uncached: a microphone can be unplugged while
	// the settings window is open.
	const loadAudioDevices = useCallback(async () => {
		try {
			setAudioDevices(
				await invoke<AudioDeviceChoices>(COMMANDS.AUDIO_LIST_AUDIO_DEVICES),
			);
		} catch (error) {
			console.error("Failed to list audio devices:", error);
		}
	}, []);

	// Every answer about Juno's voice is numbered, and only the newest one is
	// drawn. Opening the pane, picking a voice and changing the engine each
	// come back with the whole list; a slow read that was asked first and
	// answered last used to overwrite a newer pick, which is a selection
	// snapping back while Rust held the right answer all along.
	const voiceRequest = useRef(0);
	const applyVoiceList = useCallback(
		async (request: () => Promise<JunoVoiceList>) => {
			const ticket = ++voiceRequest.current;
			const list = await request();
			if (ticket !== voiceRequest.current) return;
			setJunoVoices(list);
			// The list carries the engine it belongs to, so the engine picker
			// and the voice rows cannot disagree about which engine is in force.
			setTtsProvider(list.provider);
		},
		[],
	);

	const loadJunoVoices = useCallback(async () => {
		try {
			await applyVoiceList(() => invoke<JunoVoiceList>(COMMANDS.AUDIO_GET_JUNO_VOICES));
		} catch (error) {
			console.error("Failed to load Juno's voices:", error);
		}
	}, [applyVoiceList]);

	const handleAudioInputDeviceChange = useCallback(
		async (name: string | null) => {
			// Optimistic, because the pickers are the one place a wrong answer
			// is obvious: the list reloads from Rust straight after.
			setAudioDevices((prev) => (prev ? { ...prev, chosen_input: name } : prev));
			setCaptureFailure(null);
			try {
				await invoke(COMMANDS.AUDIO_SET_AUDIO_INPUT_DEVICE, { name });
			} catch (error) {
				console.error("Failed to set the microphone:", error);
				toast.error(String(error));
			}
			await loadAudioDevices();
		},
		[loadAudioDevices],
	);

	const handleAudioOutputDeviceChange = useCallback(
		async (name: string | null) => {
			setAudioDevices((prev) => (prev ? { ...prev, chosen_output: name } : prev));
			try {
				await invoke(COMMANDS.AUDIO_SET_AUDIO_OUTPUT_DEVICE, { name });
			} catch (error) {
				console.error("Failed to set the speaker:", error);
				toast.error(String(error));
			}
			await loadAudioDevices();
		},
		[loadAudioDevices],
	);

	/**
	 * Pick Juno's voice. Selecting is still the audition, but the two halves
	 * are no longer one await.
	 *
	 * There is no optimistic selection here on purpose. The first version
	 * marked the tapped row as chosen, then awaited a command that did not
	 * return until the sample had finished speaking, and only then replaced
	 * the list with Rust's answer. Any id Rust resolved differently snapped
	 * back several seconds later, which is what "it jumps back to the old
	 * selection" was. Now the command writes, resolves and returns the list
	 * straight away, and the sound reports itself on an event.
	 */
	const handleJunoVoiceChange = useCallback(async (id: string) => {
		try {
			await applyVoiceList(() =>
				invoke<JunoVoiceList>(COMMANDS.AUDIO_SET_JUNO_VOICE, { id }),
			);
		} catch (error) {
			console.error("Failed to set Juno's voice:", error);
			toast.error(String(error));
		}
	}, [applyVoiceList]);

	/** Hear the voice already chosen again. */
	const handlePreviewJunoVoice = useCallback(async () => {
		try {
			await invoke(COMMANDS.AUDIO_PREVIEW_JUNO_VOICE);
		} catch (error) {
			console.error("Failed to play the voice sample:", error);
			toast.error(String(error));
		}
	}, []);

	// A local engine finished loading in the background. Its voices are on
	// disk now, or the list can say why not.
	useEventListener<string>(EVENTS.JUNO_VOICE_ENGINE_READY, () => {
		void loadJunoVoices();
	});

	// What the sample is doing. Rust owns this, so every window that draws
	// the picker shows the same thing at the same time.
	useEventListener<VoiceAudition>(EVENTS.JUNO_VOICE_AUDITION, (payload) => {
		if (!payload) return;
		setVoiceAudition(payload.state === EVENTS.JUNO_VOICE_DONE ? null : payload);
	});

	// The microphone never opened. Rust has already switched listening off in
	// the store and in its own state; this is what stops the window saying
	// otherwise, and puts the reason where the microphone is chosen.
	useEventListener<CaptureFailure>(EVENTS.VOICE_CAPTURE_FAILED, (payload) => {
		if (!payload) return;
		setCaptureFailure(payload);
		setAlwaysListeningActive(false);
		invalidateCache("alwaysListeningActive");
		void loadAudioDevices();
	});

	// A chosen microphone was gone and another one stood in. Not a failure:
	// dictation is working, on a different device than the one on the label.
	useEventListener<{ requested?: string; used?: string }>(
		EVENTS.VOICE_CAPTURE_DEVICE_SUBSTITUTED,
		() => {
			void loadAudioDevices();
		},
	);

	// Handler functions
	/**
	 * Change the engine. Rust answers with the new engine's voices, resolved
	 * and in force, and that answer is what gets drawn. No optimistic value
	 * and no toast: the picker showing the new engine, with that engine's
	 * voices underneath it, is the confirmation.
	 */
	const handleTtsProviderChange = useCallback(async (newProvider: string) => {
		try {
			await applyVoiceList(() =>
				invoke<JunoVoiceList>(COMMANDS.TTS_SET_TTS_PROVIDER, { provider: newProvider }),
			);
		} catch (error) {
			console.error("Failed to change Juno's voice engine:", error);
			toast.error(String(error));
		}
	}, [applyVoiceList]);

	const handleChatterboxSettingsChange = useCallback(async (
		referenceAudioUrl: string,
		exaggeration: number,
		useHd: boolean,
	) => {
		await invokeCommand(
			COMMANDS.TTS_SET_CHATTERBOX_SETTINGS,
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
		speed: number,
	) => {
		// No voice: it is chosen in the voice list, and sending the one this
		// pane last saw would write an old choice back over a newer one.
		await invokeCommand(
			COMMANDS.TTS_SET_SUPERTONIC_SETTINGS,
			{ serverUrl, voice: null, speed },
			{
				showSuccessToast: true,
				successMessage: "Supertonic settings saved",
				errorMessage: "Failed to save Supertonic settings",
			}
		);
		setSupertonicServerUrl(serverUrl);
		setSupertonicSpeed(speed);
	}, [invokeCommand]);

	const handleActiveProviderChange = useCallback(async (providerId: string) => {
		try {
			console.log(`Switching active provider to: ${providerId}`);
			await invokeCommand(COMMANDS.PROVIDERS_SET_ACTIVE_PROVIDER, { providerId });
			setActiveProvider(providerId);

			// Invalidate cache for fresh data
			invalidateCache('activeProvider');
			invalidateCache('providers');

			// Load settings specifically for the new provider
			console.log(`Loading settings for provider: ${providerId}`);
			const settings = await invokeCommand<ProviderSettings>(COMMANDS.PROVIDERS_GET_PROVIDER_SETTINGS, {
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

	// Saves whatever differs from the stored provider settings. Called when a
	// field loses focus, so there is no Save button; the pane shows "Saved"
	// inline on `true`. Nothing to save also returns true.
	const handleSaveProviderSettings = async (): Promise<boolean> => {
		if (!activeProvider) {
			toast.error("No provider selected");
			return false;
		}

		try {
			console.log("Saving provider settings changes...");

			// Update API key
			if (formData.apiKey !== providerSettings?.api_key) {
				await invoke(COMMANDS.PROVIDERS_UPDATE_PROVIDER_API_KEY, {
					providerId: activeProvider,
					apiKey: formData.apiKey,
				});
			}

			// Update model
			if (formData.model !== providerSettings?.model) {
				await invoke(COMMANDS.PROVIDERS_UPDATE_PROVIDER_MODEL, {
					providerId: activeProvider,
					model: formData.model,
				});
			}

			// Update max tokens
			if (formData.maxTokens && formData.maxTokens !== providerSettings?.max_tokens?.toString()) {
				await invoke(COMMANDS.PROVIDERS_UPDATE_PROVIDER_MAX_TOKENS, {
					providerId: activeProvider,
					maxTokens: parseInt(formData.maxTokens),
				});
			}

			// Update temperature
			if (formData.temperature && formData.temperature !== providerSettings?.temperature?.toString()) {
				await invoke(COMMANDS.PROVIDERS_UPDATE_PROVIDER_TEMPERATURE, {
					providerId: activeProvider,
					temperature: parseFloat(formData.temperature),
				});
			}

			// Update system prompt
			if (formData.systemPrompt !== providerSettings?.system_prompt) {
				await invoke(COMMANDS.PROVIDERS_UPDATE_PROVIDER_SYSTEM_PROMPT, {
					providerId: activeProvider,
					systemPrompt: formData.systemPrompt,
				});
			}

			// Only reload the specific provider settings instead of all settings
			console.log("Reloading specific provider settings...");
			const updatedSettings = await invokeCommand<ProviderSettings>(COMMANDS.PROVIDERS_GET_PROVIDER_SETTINGS, {
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

			console.log("Provider settings saved and reloaded successfully");
			return true;
		} catch (error) {
			console.error("Failed to save provider settings:", error);
			toast.error("Failed to save provider settings");
			return false;
		}
	};

	// Applies immediately, like a System Settings switch — not part of the
	// Save-button formData flow. Claude CLI provider only (LAC-4056).
	const handleLoadAccountMcpChange = useCallback(async (enabled: boolean) => {
		if (!activeProvider) return;
		await invokeCommand(
			COMMANDS.PROVIDERS_UPDATE_PROVIDER_LOAD_ACCOUNT_MCP,
			{ providerId: activeProvider, loadAccountMcp: enabled },
			{
				showSuccessToast: true,
				successMessage: `Account MCP connectors ${enabled ? "enabled" : "disabled"}`,
				errorMessage: "Failed to update MCP connector setting"
			}
		);
		setProviderSettings((prev) =>
			prev ? { ...prev, load_account_mcp: enabled } : prev
		);
	}, [activeProvider, invokeCommand]);

	const handleSoundEnabledChange = useCallback(async (enabled: boolean) => {
		await invokeCommand(
			COMMANDS.SOUND_SET_SOUND_ENABLED,
			{ enabled },
			{
				showSuccessToast: false,
				errorMessage: "Failed to update sound setting"
			}
		);
		setSoundEnabled(enabled);
	}, [invokeCommand]);

	const handlePerformanceMonitoringChange = useCallback(async (enabled: boolean) => {
		await invokeCommand(
			COMMANDS.CORE_SET_PERFORMANCE_MONITORING,
			{ enabled },
			{
				showSuccessToast: false,
				errorMessage: "Failed to update performance monitoring setting"
			}
		);
		setPerformanceMonitoringEnabled(enabled);
	}, [invokeCommand]);

	const handleAgentModeChange = useCallback(async (newMode: string) => {
		await invokeCommand(
			COMMANDS.AGENT_SET_AGENT_MODE,
			{ mode: newMode },
			{
				showSuccessToast: false,
				errorMessage: "Failed to set agent mode"
			}
		);
		invalidateCache("agentMode");
		setAgentMode(newMode);
	}, [invokeCommand]);

	const handleAgentTriggerModeChange = async (newMode: string) => {
		try {
			await invoke(COMMANDS.AGENT_SET_AGENT_TRIGGER_MODE, { mode: newMode });
			setAgentTriggerMode(newMode);
			toast.success(`Agent trigger mode set to: ${newMode === "tap" ? "Tap to Toggle" : "Hold to Activate"}`);
		} catch (error) {
			console.error("Failed to set agent trigger mode:", error);
			toast.error("Failed to set agent trigger mode");
		}
	};

	const handleDictationClipboardChange = async (enabled: boolean) => {
		try {
			await invoke(COMMANDS.DICTATION_SET_DICTATION_CLIPBOARD_ENABLED, { enabled });
			invalidateCache('dictationClipboardEnabled');
			setDictationClipboardEnabled(enabled);
			toast.success(`Copy to clipboard ${enabled ? "enabled" : "disabled"}`);
		} catch (error) {
			console.error("Failed to set dictation clipboard:", error);
			toast.error("Failed to update dictation setting");
		}
	};

	const handleDictationInsertionModeChange = async (mode: string) => {
		try {
			await invoke(COMMANDS.DICTATION_SET_DICTATION_INSERTION_MODE, { mode });
			invalidateCache('dictationInsertionMode');
			setDictationInsertionMode(mode);
			toast.success(mode === "clipboard_free" ? "Clipboard-free insertion enabled" : "Clipboard paste insertion enabled");
		} catch (error) {
			console.error("Failed to set dictation insertion mode:", error);
			toast.error("Failed to update dictation setting");
		}
	};

	const handleDictationTriggerModeChange = async (newMode: string) => {
		try {
			await invoke(COMMANDS.DICTATION_SET_DICTATION_TRIGGER_MODE, { mode: newMode });
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
			const newState = await invoke<boolean>(COMMANDS.ALWAYS_LISTENING_TOGGLE_ALWAYS_LISTENING_MODE);
			setAlwaysListeningActive(newState);
			toast.success(`Always listening ${newState ? "enabled" : "disabled"}`);
		} catch (error) {
			console.error("Failed to toggle always listening:", error);
			toast.error("Failed to toggle always listening");
		}
	};

	const handleSensitivityChange = async (sensitivity: number) => {
		try {
			await invoke(COMMANDS.ALWAYS_LISTENING_SET_ALWAYS_LISTENING_SENSITIVITY, { sensitivity });
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
			await invoke(COMMANDS.ALWAYS_LISTENING_SET_ALWAYS_LISTENING_WAKE_WORDS, { wakeWords });
			setAlwaysListeningWakeWords(wakeWords);
			toast.success("Wake words updated successfully");
		} catch (error) {
			console.error("Failed to set wake words:", error);
			toast.error("Failed to set wake words");
		}
	};

	const loadLivePartialSetting = useCallback(async () => {
		try {
			const live = await getCachedOrFetch("livePartialTranscription", () =>
				invokeCommand<boolean>(COMMANDS.STT_MODELS_GET_LIVE_PARTIAL_TRANSCRIPTION)
			);
			setLivePartialTranscriptionState(live);
		} catch (error) {
			console.error("Error loading live transcription setting:", error);
		}
	}, [invokeCommand]);

	const handleLivePartialTranscriptionChange = useCallback(async (enabled: boolean) => {
		// Optimistic: reflect the toggle immediately, revert on failure.
		setLivePartialTranscriptionState(enabled);
		try {
			await invokeCommand(COMMANDS.STT_MODELS_SET_LIVE_PARTIAL_TRANSCRIPTION, { enabled });
			invalidateCache("livePartialTranscription");
		} catch (error) {
			console.error("Failed to set live partial transcription:", error);
			setLivePartialTranscriptionState(!enabled);
			toast.error("Failed to update live transcription");
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
		dictationInsertionMode,
		dictationTriggerMode,
		soundEnabled,
		performanceMonitoringEnabled,
		audioDevices,
		junoVoices,
		voiceAudition,
		captureFailure,
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

		// Dictation display mode (the models themselves live in useSttModels)
		livePartialTranscription,

		// Actions
		loadAllSettings,
		loadAudioDevices,
		loadJunoVoices,
		handleAudioInputDeviceChange,
		handleAudioOutputDeviceChange,
		handleJunoVoiceChange,
		handlePreviewJunoVoice,
		dismissCaptureFailure: () => setCaptureFailure(null),
		handleTtsProviderChange,
		handleChatterboxSettingsChange,
		handleSupertonicSettingsChange,
		handleActiveProviderChange,
		handleSaveProviderSettings,
		handleLoadAccountMcpChange,
		handleSoundEnabledChange,
		handlePerformanceMonitoringChange,
		handleAgentModeChange,
		handleAgentTriggerModeChange,
		handleDictationClipboardChange,
		handleDictationInsertionModeChange,
		handleDictationTriggerModeChange,
		handleAlwaysListeningToggle,
		handleSensitivityChange,
		handleWakeWordsChange,
		loadPermissionsStatus,
		loadKeyboardShortcuts,
		loadToolConfigurations,
		loadLivePartialSetting,
		handleLivePartialTranscriptionChange,
		setToolConfigurations,
		invalidateToolConfigCache,
		loadMcpServers,
		debugSettings,
	};
}
