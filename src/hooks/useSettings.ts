import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { toast } from "sonner";
import { KeyboardShortcuts } from "@/types/keyboard";
import { AUDIO } from "@/lib/constants";

interface ProviderInfo {
  id: string;
  name: string;
  description: string;
  models: string[];
  default_model: string;
  model_info: {
    id: string;
    name: string;
    supports_computer_use: boolean;
    is_recommended: boolean;
  }[];
  is_available: boolean;
  is_default: boolean;
  computer_use_supported: boolean;
}

interface ProviderSettings {
  api_key: string;
  model: string;
  max_tokens?: number;
  temperature?: number;
  system_prompt?: string;
}

interface ToolConfig {
  name: string;
  category: string;
  enabled: boolean;
  description?: string;
  required: boolean;
}

interface ToolCategory {
  name: string;
  description: string;
  enabled: boolean;
  tools: ToolConfig[];
}

interface MCPServerConfig {
  id: string;
  name: string;
  description?: string;
  command: string;
  args: string[];
  working_directory?: string;
  environment_variables: Record<string, string>;
  enabled: boolean;
  auto_start: boolean;
  timeout_seconds: number;
  max_retries: number;
}

interface MCPServerStatus {
  Disconnected?: null;
  Connecting?: null;
  Connected?: null;
  Error?: string;
  Timeout?: null;
}

interface MCPToolInfo {
  server_id: string;
  server_name: string;
  tool_definition: {
    name: string;
    description: string;
    input_schema: any;
  };
  enabled: boolean;
}

export function useSettings() {
  // TTS Settings
  const [ttsProvider, setTtsProvider] = useState<string>("system");

  // AI Provider Settings
  const [providers, setProviders] = useState<ProviderInfo[]>([]);
  const [activeProvider, setActiveProvider] = useState<string>("");
  const [providerSettings, setProviderSettings] = useState<ProviderSettings | null>(null);
  const [isLoading, setIsLoading] = useState<boolean>(false);

  // Agent Mode Settings
  const [agentMode, setAgentMode] = useState<string>("multi");

  // Dictation Settings
  const [dictationClipboardEnabled, setDictationClipboardEnabled] = useState<boolean>(true);

  // Sound Settings
  const [soundEnabled, setSoundEnabled] = useState<boolean>(true);

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
  const [permissionsState, setPermissionsState] = useState<{
    accessibility: { granted: boolean; required: boolean };
    screenRecording: { granted: boolean; required: boolean };
    microphone: { granted: boolean; required: boolean };
    allGranted: boolean;
    appName: string;
  } | null>(null);
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

  const loadAllSettings = async () => {
    setIsLoading(true);
    try {
      // Load TTS settings
      const currentTtsProvider = await invoke<string>("get_tts_provider_command");
      setTtsProvider(currentTtsProvider);

      // Load AI provider settings
      const availableProviders = await invoke<ProviderInfo[]>("get_providers");
      setProviders(availableProviders);

      const currentActiveProvider = await invoke<string>("get_active_provider");
      setActiveProvider(currentActiveProvider);

      // Load agent mode settings
      const currentAgentMode = await invoke<string>("get_agent_mode");
      setAgentMode(currentAgentMode);

      // Load dictation settings
      const currentClipboardEnabled = await invoke<boolean>("get_dictation_clipboard_enabled");
      setDictationClipboardEnabled(currentClipboardEnabled);

      // Load sound settings
      const currentSoundEnabled = await invoke<boolean>("get_sound_enabled");
      setSoundEnabled(currentSoundEnabled);

      // Load always listening settings
      const alwaysListeningStatus = await invoke<boolean>("get_always_listening_status");
      setAlwaysListeningActive(alwaysListeningStatus);

      const sensitivity = await invoke<number>("get_always_listening_sensitivity");
      setAlwaysListeningSensitivity(sensitivity);

      const wakeWords = await invoke<string[]>("get_always_listening_wake_words");
      setAlwaysListeningWakeWords(wakeWords);
      setWakeWordsInput(wakeWords.join(", "));

      if (currentActiveProvider) {
        const settings = await invoke<ProviderSettings>("get_provider_settings", {
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

      // Load permissions status
      await loadPermissionsStatus();

      // Load tool configurations
      await loadToolConfigurations();

      // Load keyboard shortcuts
      await loadKeyboardShortcuts();

      // Load MCP server configurations
      await loadMcpServers();
    } catch (error) {
      console.error("Error loading settings:", error);
      toast.error("Failed to load some settings");
    } finally {
      setIsLoading(false);
    }
  };

  const loadPermissionsStatus = async () => {
    setPermissionsLoading(true);
    try {
      const permissions = await invoke<{
        accessibility: { granted: boolean; required: boolean };
        screenRecording: { granted: boolean; required: boolean };
        microphone: { granted: boolean; required: boolean };
        allGranted: boolean;
        appName: string;
      }>("check_permissions_status");
      setPermissionsState(permissions);
    } catch (error) {
      console.error("Error loading permissions status:", error);
      setPermissionsState(null);
    } finally {
      setPermissionsLoading(false);
    }
  };

  const loadKeyboardShortcuts = async () => {
    setShortcutsLoading(true);
    try {
      const shortcuts = await invoke<KeyboardShortcuts>("get_keyboard_shortcuts");
      setKeyboardShortcuts(shortcuts);
    } catch (error) {
      console.error("Error loading keyboard shortcuts:", error);
      toast.error("Failed to load keyboard shortcuts");
    } finally {
      setShortcutsLoading(false);
    }
  };

  const loadToolConfigurations = async () => {
    setToolConfigLoading(true);
    try {
      const configs = await invoke<Record<string, ToolCategory>>("get_tool_configurations");
      setToolConfigurations(configs);
    } catch (error) {
      console.error("Error loading tool configurations:", error);
      toast.error("Failed to load tool configurations");
    } finally {
      setToolConfigLoading(false);
    }
  };

  const loadMcpServers = async () => {
    setMcpLoading(true);
    try {
      const servers = await invoke<MCPServerConfig[]>("get_mcp_servers");
      setMcpServers(servers);

      const statuses = await invoke<Record<string, MCPServerStatus>>("get_mcp_server_statuses");
      setMcpServerStatuses(statuses);

      const tools = await invoke<MCPToolInfo[]>("get_mcp_tools");
      setMcpTools(tools);
    } catch (error) {
      console.error("Error loading MCP servers:", error);
      toast.error(`Failed to load MCP servers: ${error}`);
    } finally {
      setMcpLoading(false);
    }
  };

  // Handler functions
  const handleTtsProviderChange = async (newProvider: string) => {
    try {
      await invoke("set_tts_provider_command", { provider: newProvider });
      setTtsProvider(newProvider);
      toast.success(
        `TTS provider set to: ${
          newProvider === "off"
            ? "Off"
            : newProvider.charAt(0).toUpperCase() + newProvider.slice(1)
        }`
      );
    } catch (error) {
      console.error("Failed to set TTS provider:", error);
      toast.error("Failed to set TTS provider");
    }
  };

  const handleActiveProviderChange = async (providerId: string) => {
    try {
      await invoke("set_active_provider", { providerId });
      setActiveProvider(providerId);

      const settings = await invoke<ProviderSettings>("get_provider_settings", {
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
    } catch (error) {
      console.error("Failed to set active provider:", error);
      toast.error("Failed to set active provider");
    }
  };

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

  const handleSoundEnabledChange = async (enabled: boolean) => {
    try {
      await invoke("set_sound_enabled", { enabled });
      setSoundEnabled(enabled);
      toast.success(`Sound ${enabled ? "enabled" : "disabled"}`);
    } catch (error) {
      console.error("Failed to set sound enabled:", error);
      toast.error("Failed to update sound setting");
    }
  };

  const handleAgentModeChange = async (newMode: string) => {
    try {
      await invoke("set_agent_mode", { mode: newMode });
      setAgentMode(newMode);
      toast.success(`Agent mode set to: ${newMode}`);
    } catch (error) {
      console.error("Failed to set agent mode:", error);
      toast.error("Failed to set agent mode");
    }
  };

  const handleDictationClipboardChange = async (enabled: boolean) => {
    try {
      await invoke("set_dictation_clipboard_enabled", { enabled });
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
    dictationClipboardEnabled,
    soundEnabled,
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
    handleAgentModeChange,
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