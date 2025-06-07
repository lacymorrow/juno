import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Textarea } from "@/components/ui/textarea";
import { KeyboardShortcuts } from "@/types/keyboard";
import { invoke } from "@tauri-apps/api/core";
import {
  AlertCircle,
  Brain,
  CheckCircle,
  Keyboard,
  Mic,
  MonitorSpeaker,
  Network,
  RefreshCw,
  RotateCcw,
  Save,
  Settings as SettingsIcon,
  Shield,
  Terminal,
  Plus,
  Play,
  Square,
  Trash2,
  Edit,
  Server,
  ExternalLink,
} from "lucide-react";
import React, { useEffect, useState } from "react";
import { toast } from "sonner";

interface ProviderInfo {
  id: string;
  name: string;
  description: string;
  models: string[];
  default_model: string;
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

// MCP Server interfaces
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

interface SettingsProps {
  onNavigateToDevTools?: () => void;
  onNavigateToChat?: () => void;
  onNavigateToPermissions?: () => void;
}

const Settings: React.FC<SettingsProps> = ({
  onNavigateToDevTools,
  onNavigateToChat,
  onNavigateToPermissions,
}) => {
  // TTS Settings
  const [ttsProvider, setTtsProvider] = useState<string>("off");

  // AI Provider Settings
  const [providers, setProviders] = useState<ProviderInfo[]>([]);
  const [activeProvider, setActiveProvider] = useState<string>("");
  const [providerSettings, setProviderSettings] =
    useState<ProviderSettings | null>(null);
  const [isLoading, setIsLoading] = useState<boolean>(false);

  // Agent Mode Settings
  const [agentMode, setAgentMode] = useState<string>("multi");

  // Dictation Settings
  const [dictationClipboardEnabled, setDictationClipboardEnabled] =
    useState<boolean>(true);

  // Sound Settings
  const [soundEnabled, setSoundEnabled] = useState<boolean>(true);

  // Tool Configuration Settings
  const [toolConfigurations, setToolConfigurations] = useState<
    Record<string, ToolCategory>
  >({});
  const [toolConfigLoading, setToolConfigLoading] = useState<boolean>(false);

  // MCP Server Settings
  const [mcpServers, setMcpServers] = useState<MCPServerConfig[]>([]);
  const [mcpServerStatuses, setMcpServerStatuses] = useState<Record<string, MCPServerStatus>>({});
  const [mcpTools, setMcpTools] = useState<MCPToolInfo[]>([]);
  const [mcpLoading, setMcpLoading] = useState<boolean>(false);
  const [showAddMcpDialog, setShowAddMcpDialog] = useState<boolean>(false);
  const [editingMcpServer, setEditingMcpServer] = useState<MCPServerConfig | null>(null);
  const [mcpJsonMode, setMcpJsonMode] = useState<boolean>(false);
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

  // Form state for MCP server
  const [mcpFormData, setMcpFormData] = useState<{
    name: string;
    description: string;
    command: string;
    args: string;
    workingDirectory: string;
    environmentVariables: string;
    timeoutSeconds: string;
    maxRetries: string;
    autoStart: boolean;
  }>({
    name: "",
    description: "",
    command: "",
    args: "",
    workingDirectory: "",
    environmentVariables: "",
    timeoutSeconds: "30",
    maxRetries: "3",
    autoStart: true,
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
  const [keyboardShortcuts, setKeyboardShortcuts] = useState<KeyboardShortcuts>(
    {
      agent_mode_toggle: "",
      dictation_input: "",
      stop_current_task: "",
      open_settings: "",
    }
  );
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
      const currentTtsProvider = await invoke<string>(
        "get_tts_provider_command"
      );
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
      const currentClipboardEnabled = await invoke<boolean>(
        "get_dictation_clipboard_enabled"
      );
      setDictationClipboardEnabled(currentClipboardEnabled);

      // Load sound settings
      const currentSoundEnabled = await invoke<boolean>("get_sound_enabled");
      setSoundEnabled(currentSoundEnabled);

      if (currentActiveProvider) {
        const settings = await invoke<ProviderSettings>(
          "get_provider_settings",
          {
            providerId: currentActiveProvider,
          }
        );
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

  const loadMcpServers = async () => {
    setMcpLoading(true);
    try {
      // Load MCP server configurations
      const servers = await invoke<MCPServerConfig[]>("get_mcp_servers");
      setMcpServers(servers);

      // Load MCP server statuses
      const statuses = await invoke<Record<string, MCPServerStatus>>("get_mcp_server_statuses");
      setMcpServerStatuses(statuses);

      // Load MCP tools
      const tools = await invoke<MCPToolInfo[]>("get_mcp_tools");
      setMcpTools(tools);
    } catch (error) {
      console.error("Error loading MCP servers:", error);
      toast.error("Failed to load MCP servers");
    } finally {
      setMcpLoading(false);
    }
  };

  const resetMcpForm = () => {
    setMcpFormData({
      name: "",
      description: "",
      command: "",
      args: "",
      workingDirectory: "",
      environmentVariables: "",
      timeoutSeconds: "30",
      maxRetries: "3",
      autoStart: true,
    });
    setEditingMcpServer(null);
    setMcpJsonMode(false);
    setMcpJsonData("");
  };

  const handleAddMcpServer = () => {
    resetMcpForm();
    // Initialize with Cursor/Claude compatible JSON template
    const defaultConfig = {
      "my-server": {
        "command": "npx",
        "args": ["@modelcontextprotocol/server-filesystem", "/path/to/directory"],
        "env": {
          "API_KEY": "your-api-key-here"
        }
      }
    };
    setMcpJsonData(JSON.stringify(defaultConfig, null, 2));
    setShowAddMcpDialog(true);
  };

  const handleEditMcpServer = (server: MCPServerConfig) => {
    setMcpFormData({
      name: server.name,
      description: server.description || "",
      command: server.command,
      args: server.args.join(" "),
      workingDirectory: server.working_directory || "",
      environmentVariables: JSON.stringify(server.environment_variables, null, 2),
      timeoutSeconds: server.timeout_seconds.toString(),
      maxRetries: server.max_retries.toString(),
      autoStart: server.auto_start,
    });
    setEditingMcpServer(server);

    // Convert to Cursor/Claude compatible format for JSON editor
    const simplifiedConfig = {
      [server.name]: {
        command: server.command,
        args: server.args,
        ...(Object.keys(server.environment_variables).length > 0 && { env: server.environment_variables })
      }
    };
    setMcpJsonData(JSON.stringify(simplifiedConfig, null, 2));
    setShowAddMcpDialog(true);
  };

  const convertFormToJson = () => {
    try {
      let environmentVariables = {};
      if (mcpFormData.environmentVariables.trim()) {
        environmentVariables = JSON.parse(mcpFormData.environmentVariables);
      }

      // Convert to Cursor/Claude compatible format
      const serverName = mcpFormData.name.trim() || "my-server";
      const args = mcpFormData.args.trim().split(/\s+/).filter(arg => arg.length > 0);

      const simplifiedConfig = {
        [serverName]: {
          command: mcpFormData.command.trim(),
          args: args,
          ...(Object.keys(environmentVariables).length > 0 && { env: environmentVariables })
        }
      };

      setMcpJsonData(JSON.stringify(simplifiedConfig, null, 2));
    } catch (e) {
      toast.error("Invalid form data for JSON conversion");
    }
  };

  const convertJsonToForm = () => {
    try {
      const simplifiedConfig = JSON.parse(mcpJsonData);

      // Handle both Cursor/Claude format and legacy format
      let serverName = "";
      let command = "";
      let args: string[] = [];
      let env = {};

      if (simplifiedConfig.name && simplifiedConfig.command) {
        // Legacy format (full MCPServerConfig)
        serverName = simplifiedConfig.name;
        command = simplifiedConfig.command;
        args = Array.isArray(simplifiedConfig.args) ? simplifiedConfig.args : [];
        env = simplifiedConfig.environment_variables || {};
      } else {
        // Cursor/Claude format - get first server
        const serverNames = Object.keys(simplifiedConfig);
        if (serverNames.length > 0) {
          serverName = serverNames[0];
          const serverConfig = simplifiedConfig[serverName];
          command = serverConfig.command || "";
          args = Array.isArray(serverConfig.args) ? serverConfig.args : [];
          env = serverConfig.env || {};
        }
      }

      setMcpFormData({
        name: serverName,
        description: simplifiedConfig.description || "",
        command: command,
        args: args.join(" "),
        workingDirectory: simplifiedConfig.working_directory || "",
        environmentVariables: JSON.stringify(env, null, 2),
        timeoutSeconds: simplifiedConfig.timeout_seconds?.toString() || "30",
        maxRetries: simplifiedConfig.max_retries?.toString() || "3",
        autoStart: simplifiedConfig.auto_start ?? true,
      });
    } catch (e) {
      toast.error("Invalid JSON format");
    }
  };

  const handleSaveMcpServer = async () => {
    try {
      let config: MCPServerConfig;

      if (mcpJsonMode) {
        // Parse and validate JSON mode data
        try {
          const jsonData = JSON.parse(mcpJsonData);

          // Handle both Cursor/Claude format and legacy format
          if (jsonData.name && jsonData.command) {
            // Legacy format (full MCPServerConfig)
            config = jsonData;

            // Validate required fields
            if (!config.name?.trim() || !config.command?.trim()) {
              toast.error("Name and command are required");
              return;
            }

            // Ensure proper types and defaults
            config.args = Array.isArray(config.args) ? config.args : [];
            config.environment_variables = config.environment_variables || {};
            config.enabled = config.enabled ?? true;
            config.auto_start = config.auto_start ?? true;
            config.timeout_seconds = config.timeout_seconds || 30;
            config.max_retries = config.max_retries || 3;
            config.id = config.id || editingMcpServer?.id || `mcp-${Date.now()}`;
          } else {
            // Cursor/Claude format - convert to full format
            const serverNames = Object.keys(jsonData);
            if (serverNames.length === 0) {
              toast.error("No server configuration found in JSON");
              return;
            }

            if (serverNames.length > 1) {
              toast.error("Multiple servers detected. Please edit one server at a time.");
              return;
            }

            const serverName = serverNames[0];
            const serverConfig = jsonData[serverName];

            if (!serverName.trim() || !serverConfig.command?.trim()) {
              toast.error("Server name and command are required");
              return;
            }

            config = {
              id: editingMcpServer?.id || `mcp-${Date.now()}`,
              name: serverName.trim(),
              description: undefined,
              command: serverConfig.command,
              args: Array.isArray(serverConfig.args) ? serverConfig.args : [],
              working_directory: undefined,
              environment_variables: serverConfig.env || {},
              enabled: true,
              auto_start: true,
              timeout_seconds: 30,
              max_retries: 3,
            };
          }
        } catch (e) {
          toast.error("Invalid JSON format");
          return;
        }
      } else {
        // Use form mode data
        if (!mcpFormData.name.trim() || !mcpFormData.command.trim()) {
          toast.error("Name and command are required");
          return;
        }

        let environmentVariables = {};
        if (mcpFormData.environmentVariables.trim()) {
          try {
            environmentVariables = JSON.parse(mcpFormData.environmentVariables);
          } catch (e) {
            toast.error("Invalid JSON in environment variables");
            return;
          }
        }

        config = {
          id: editingMcpServer?.id || `mcp-${Date.now()}`,
          name: mcpFormData.name.trim(),
          description: mcpFormData.description.trim() || undefined,
          command: mcpFormData.command.trim(),
          args: mcpFormData.args.trim().split(/\s+/).filter(arg => arg.length > 0),
          working_directory: mcpFormData.workingDirectory.trim() || undefined,
          environment_variables: environmentVariables,
          enabled: true,
          auto_start: mcpFormData.autoStart,
          timeout_seconds: parseInt(mcpFormData.timeoutSeconds) || 30,
          max_retries: parseInt(mcpFormData.maxRetries) || 3,
        };
      }

      if (editingMcpServer) {
        await invoke("update_mcp_server", { config });
        toast.success("MCP server updated successfully");
      } else {
        await invoke("add_mcp_server", { config });
        toast.success("MCP server added successfully");
      }

      setShowAddMcpDialog(false);
      resetMcpForm();
      await loadMcpServers();
    } catch (error) {
      console.error("Error saving MCP server:", error);
      toast.error("Failed to save MCP server");
    }
  };

  const handleDeleteMcpServer = async (serverId: string) => {
    if (!confirm("Are you sure you want to delete this MCP server?")) {
      return;
    }

    try {
      await invoke("remove_mcp_server", { serverId });
      toast.success("MCP server deleted successfully");
      await loadMcpServers();
    } catch (error) {
      console.error("Error deleting MCP server:", error);
      toast.error("Failed to delete MCP server");
    }
  };

  const handleToggleMcpServer = async (serverId: string, enabled: boolean) => {
    try {
      await invoke("set_mcp_server_enabled", { serverId, enabled });
      toast.success(`MCP server ${enabled ? "enabled" : "disabled"} successfully`);
      await loadMcpServers();
    } catch (error) {
      console.error("Error toggling MCP server:", error);
      toast.error("Failed to toggle MCP server");
    }
  };

  const handleStartMcpServer = async (serverId: string) => {
    try {
      await invoke("start_mcp_server", { serverId });
      toast.success("MCP server started successfully");
      await loadMcpServers();
    } catch (error) {
      console.error("Error starting MCP server:", error);
      toast.error("Failed to start MCP server");
    }
  };

  const handleStopMcpServer = async (serverId: string) => {
    try {
      await invoke("stop_mcp_server", { serverId });
      toast.success("MCP server stopped successfully");
      await loadMcpServers();
    } catch (error) {
      console.error("Error stopping MCP server:", error);
      toast.error("Failed to stop MCP server");
    }
  };

  const handleTestMcpServer = async () => {
    if (!mcpFormData.name.trim() || !mcpFormData.command.trim()) {
      toast.error("Name and command are required for testing");
      return;
    }

    try {
      let environmentVariables = {};
      if (mcpFormData.environmentVariables.trim()) {
        try {
          environmentVariables = JSON.parse(mcpFormData.environmentVariables);
        } catch (e) {
          toast.error("Invalid JSON in environment variables");
          return;
        }
      }

      const config: MCPServerConfig = {
        id: `test-${Date.now()}`,
        name: mcpFormData.name.trim(),
        description: mcpFormData.description.trim() || undefined,
        command: mcpFormData.command.trim(),
        args: mcpFormData.args.trim().split(/\s+/).filter(arg => arg.length > 0),
        working_directory: mcpFormData.workingDirectory.trim() || undefined,
        environment_variables: environmentVariables,
        enabled: true,
        auto_start: false,
        timeout_seconds: parseInt(mcpFormData.timeoutSeconds) || 30,
        max_retries: parseInt(mcpFormData.maxRetries) || 3,
      };

      const tools = await invoke<string[]>("test_mcp_server_connection", { config });
      toast.success(`Connection successful! Found ${tools.length} tools: ${tools.join(", ")}`);
    } catch (error) {
      console.error("Error testing MCP server:", error);
      toast.error(`Connection failed: ${error}`);
    }
  };

  const getMcpServerStatusBadge = (status: MCPServerStatus) => {
    if (status.Connected !== undefined) {
      return <Badge variant="default" className="bg-green-500">Connected</Badge>;
    } else if (status.Connecting !== undefined) {
      return <Badge variant="secondary">Connecting</Badge>;
    } else if (status.Error !== undefined) {
      return <Badge variant="destructive">Error</Badge>;
    } else if (status.Timeout !== undefined) {
      return <Badge variant="destructive">Timeout</Badge>;
    } else {
      return <Badge variant="outline">Disconnected</Badge>;
    }
  };

  const getMcpServerStatusIcon = (status: MCPServerStatus) => {
    if (status.Connected !== undefined) {
      return <CheckCircle className="h-4 w-4 text-green-500" />;
    } else if (status.Connecting !== undefined) {
      return <RefreshCw className="h-4 w-4 text-blue-500 animate-spin" />;
    } else if (status.Error !== undefined || status.Timeout !== undefined) {
      return <AlertCircle className="h-4 w-4 text-red-500" />;
    } else {
      return <Square className="h-4 w-4 text-gray-400" />;
    }
  };

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
      if (
        formData.maxTokens &&
        formData.maxTokens !== providerSettings?.max_tokens?.toString()
      ) {
        await invoke("update_provider_max_tokens", {
          providerId: activeProvider,
          maxTokens: parseInt(formData.maxTokens),
        });
      }

      // Update temperature
      if (
        formData.temperature &&
        formData.temperature !== providerSettings?.temperature?.toString()
      ) {
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

      // Reload settings to reflect changes
      const updatedSettings = await invoke<ProviderSettings>(
        "get_provider_settings",
        {
          providerId: activeProvider,
        }
      );
      setProviderSettings(updatedSettings);
    } catch (error) {
      console.error("Failed to save provider settings:", error);
      toast.error("Failed to save provider settings");
    }
  };

  const handleDictationClipboardChange = async (enabled: boolean) => {
    try {
      await invoke("set_dictation_clipboard_enabled", { enabled });
      setDictationClipboardEnabled(enabled);
      toast.success(
        `Dictation Mode clipboard saving ${
          enabled ? "enabled" : "disabled"
        } - ${enabled ? "✓" : "✗"} Transcriptions will ${
          enabled ? "" : "not "
        }be saved to clipboard`
      );
    } catch (error) {
      console.error("Failed to set dictation clipboard setting:", error);
      toast.error("Failed to set dictation clipboard setting");
    }
  };

  const handleSoundEnabledChange = async (enabled: boolean) => {
    try {
      await invoke("set_sound_enabled", { enabled });
      setSoundEnabled(enabled);
      toast.success(`Sound effects ${enabled ? "enabled" : "disabled"}`);
    } catch (error) {
      console.error("Failed to set sound setting:", error);
      toast.error("Failed to set sound setting");
    }
  };

  const handleAgentModeChange = async (newMode: string) => {
    try {
      await invoke("set_agent_mode", { mode: newMode });
      setAgentMode(newMode);
      toast.success(
        `Agent mode set to: ${
          newMode === "single" ? "Single Agent" : "Multi-Agent"
        }`
      );
    } catch (error) {
      console.error("Failed to set agent mode:", error);
      toast.error("Failed to set agent mode");
    }
  };

  const loadToolConfigurations = async () => {
    try {
      setToolConfigLoading(true);
      const configurations = await invoke<Record<string, ToolCategory>>(
        "get_tool_configurations"
      );
      setToolConfigurations(configurations);
    } catch (error) {
      console.error("Failed to load tool configurations:", error);
      toast.error("Failed to load tool configurations");
    } finally {
      setToolConfigLoading(false);
    }
  };

  const handleToggleCategory = async (
    categoryName: string,
    enabled: boolean
  ) => {
    try {
      setToolConfigLoading(true);
      await invoke("set_tool_category_enabled", {
        category: categoryName,
        enabled,
      });

      // Update local state
      setToolConfigurations((prev) => ({
        ...prev,
        [categoryName]: {
          ...prev[categoryName],
          enabled,
        },
      }));

      toast.success(
        `${categoryName} category ${enabled ? "enabled" : "disabled"}`
      );
    } catch (error) {
      console.error("Failed to toggle category:", error);
      toast.error("Failed to update category setting");
    } finally {
      setToolConfigLoading(false);
    }
  };

  const handleToggleTool = async (toolName: string, enabled: boolean) => {
    try {
      await invoke("set_tool_enabled", {
        toolName,
        enabled,
      });

      // Update local state
      setToolConfigurations((prev) => {
        const updated = { ...prev };
        Object.keys(updated).forEach((categoryName) => {
          const category = updated[categoryName];
          if (category.tools) {
            category.tools = category.tools.map((tool) =>
              tool.name === toolName ? { ...tool, enabled } : tool
            );
          }
        });
        return updated;
      });

      toast.success(`${toolName} tool ${enabled ? "enabled" : "disabled"}`);
    } catch (error) {
      console.error("Failed to toggle tool:", error);
      toast.error("Failed to update tool setting");
    }
  };

  const handleResetToolConfiguration = async () => {
    try {
      setToolConfigLoading(true);
      await invoke("reset_tool_configuration");
      await loadToolConfigurations();
      toast.success("Tool configuration reset to defaults");
    } catch (error) {
      console.error("Failed to reset tool configuration:", error);
      toast.error("Failed to reset tool configuration");
    } finally {
      setToolConfigLoading(false);
    }
  };

  // Keyboard Shortcuts handlers
  const handleShortcutChange = async (shortcutName: string, value: string) => {
    try {
      setShortcutsLoading(true);
      await invoke("set_keyboard_shortcut", {
        shortcutName,
        shortcutValue: value,
      });

      setKeyboardShortcuts((prev) => ({
        ...prev,
        [shortcutName]: value,
      }));

      toast.success(`Updated ${getShortcutDisplayName(shortcutName)} shortcut`);
    } catch (error) {
      console.error("Failed to update shortcut:", error);
      toast.error(`Failed to update shortcut: ${error}`);
    } finally {
      setShortcutsLoading(false);
      setEditingShortcut(null);
    }
  };

  const handleResetShortcuts = async () => {
    try {
      setShortcutsLoading(true);
      await invoke("reset_keyboard_shortcuts");

      const shortcuts = await invoke<KeyboardShortcuts>(
        "get_keyboard_shortcuts"
      );
      setKeyboardShortcuts(shortcuts);

      toast.success("Keyboard shortcuts reset to defaults");
    } catch (error) {
      console.error("Failed to reset shortcuts:", error);
      toast.error("Failed to reset shortcuts");
    } finally {
      setShortcutsLoading(false);
    }
  };

  const getShortcutDisplayName = (shortcutName: string): string => {
    const names: Record<string, string> = {
      agent_mode_toggle: "Agent Mode Toggle",
      dictation_input: "Dictation Input",
      stop_current_task: "Stop Current Task",
      open_settings: "Open Settings",
    };
    return names[shortcutName] || shortcutName;
  };

  const getShortcutDescription = (shortcutName: string): string => {
    const descriptions: Record<string, string> = {
      agent_mode_toggle: "Send voice commands to AI agent",
      dictation_input: "Direct voice typing (hold to activate)",
      stop_current_task: "Stop any running AI task",
      open_settings: "Open the settings menu",
    };
    return descriptions[shortcutName] || "";
  };

  const currentProvider = providers.find((p) => p.id === activeProvider);

  const getPermissionIcon = (granted: boolean, required: boolean) => {
    if (granted) {
      return <CheckCircle className="h-4 w-4 text-green-500" />;
    } else if (required) {
      return <AlertCircle className="h-4 w-4 text-red-500" />;
    } else {
      return <AlertCircle className="h-4 w-4 text-gray-400" />;
    }
  };

  const getPermissionBadge = (granted: boolean, required: boolean) => {
    if (granted) {
      return (
        <Badge variant="outline" className="text-green-600 border-green-200">
          Granted
        </Badge>
      );
    } else if (required) {
      return <Badge variant="destructive">Required</Badge>;
    } else {
      return <Badge variant="secondary">Optional</Badge>;
    }
  };

  return (
    <div className="space-y-6 p-6 max-w-4xl mx-auto">
      <div className="flex items-center gap-2 mb-6">
        <SettingsIcon size={24} />
        <h1 className="text-2xl font-bold">Settings</h1>
      </div>

      {/* Voice & Audio Settings */}
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Mic size={20} />
            Voice & Audio
          </CardTitle>
          <CardDescription>
            Configure voice recognition and text-to-speech settings
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="space-y-2">
            <Label htmlFor="tts-provider">Text-to-Speech Provider</Label>
            <Select value={ttsProvider} onValueChange={handleTtsProviderChange}>
              <SelectTrigger>
                <SelectValue placeholder="Select TTS provider" />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="off">Off</SelectItem>
                <SelectItem value="system">System</SelectItem>
                <SelectItem value="elevenlabs">ElevenLabs</SelectItem>
                <SelectItem value="replicate">Replicate</SelectItem>
              </SelectContent>
            </Select>
            <p className="text-sm text-muted-foreground">
              Choose how AI responses should be spoken aloud. Use Alt+D for AI
              agent dictation or hold the dictation key for direct voice typing.
            </p>
          </div>

          <div className="space-y-2">
            <Label htmlFor="dictation-clipboard">
              Dictation Mode Clipboard
            </Label>
            <div className="flex items-center gap-3">
              <Button
                variant={dictationClipboardEnabled ? "default" : "outline"}
                size="sm"
                onClick={() =>
                  handleDictationClipboardChange(!dictationClipboardEnabled)
                }
                className="min-w-[80px]"
              >
                {dictationClipboardEnabled ? "Enabled" : "Disabled"}
              </Button>
              <span className="text-sm text-muted-foreground">
                Save transcribed text to clipboard when using Dictation Mode
              </span>
            </div>
            <p className="text-sm text-muted-foreground">
              When enabled, text transcribed via Dictation Mode (hold dictation
              key) will be saved to the system clipboard in addition to being
              typed directly.
            </p>
          </div>

          <div className="space-y-2">
            <Label htmlFor="sound-enabled">Sound Effects</Label>
            <div className="flex items-center gap-3">
              <Button
                variant={soundEnabled ? "default" : "outline"}
                size="sm"
                onClick={() => handleSoundEnabledChange(!soundEnabled)}
                className="min-w-[80px]"
              >
                {soundEnabled ? "Enabled" : "Disabled"}
              </Button>
              <span className="text-sm text-muted-foreground">
                Play sound effects for notifications and feedback
              </span>
            </div>
            <p className="text-sm text-muted-foreground">
              When enabled, the app will play sound effects for various
              notifications, successes, errors, and other feedback events.
            </p>
          </div>
        </CardContent>
      </Card>

      {/* Agent Architecture Settings */}
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Network size={20} />
            Agent Architecture
          </CardTitle>
          <CardDescription>
            Choose between single-agent and multi-agent execution modes
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="space-y-2">
            <Label htmlFor="agent-mode">Agent Mode</Label>
            <Select value={agentMode} onValueChange={handleAgentModeChange}>
              <SelectTrigger>
                <SelectValue placeholder="Select agent mode" />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="single">Single Agent</SelectItem>
                <SelectItem value="multi">Multi-Agent</SelectItem>
              </SelectContent>
            </Select>
            <div className="text-sm text-muted-foreground space-y-1">
              <p>
                <strong>Single Agent:</strong> Direct execution with all tools
                available to one agent. Faster and simpler.
              </p>
              <p>
                <strong>Multi-Agent:</strong> Specialized agents with
                orchestration. More robust for complex tasks.
              </p>
            </div>
          </div>
        </CardContent>
      </Card>

      {/* AI Provider Settings */}
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Brain size={20} />
            AI Provider
          </CardTitle>
          <CardDescription>
            Configure which AI model provider to use and its settings
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="space-y-2">
            <Label htmlFor="ai-provider">Active Provider</Label>
            <Select
              value={activeProvider}
              onValueChange={handleActiveProviderChange}
            >
              <SelectTrigger>
                <SelectValue placeholder="Select AI provider" />
              </SelectTrigger>
              <SelectContent>
                {providers.map((provider) => (
                  <SelectItem key={provider.id} value={provider.id}>
                    {provider.name}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
            {currentProvider && (
              <p className="text-sm text-muted-foreground">
                {currentProvider.description}
              </p>
            )}
          </div>

          {activeProvider && (
            <div className="space-y-4 pt-4 border-t">
              <h3 className="text-lg font-medium">Provider Configuration</h3>

              <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                <div className="space-y-2">
                  <Label htmlFor="api-key">API Key</Label>
                  <Input
                    id="api-key"
                    type="password"
                    value={formData.apiKey}
                    onChange={(e) =>
                      setFormData((prev) => ({
                        ...prev,
                        apiKey: e.target.value,
                      }))
                    }
                    placeholder="Enter API key..."
                  />
                </div>

                <div className="space-y-2">
                  <Label htmlFor="model">Model</Label>
                  <Select
                    value={formData.model}
                    onValueChange={(value) =>
                      setFormData((prev) => ({ ...prev, model: value }))
                    }
                  >
                    <SelectTrigger>
                      <SelectValue placeholder="Select model" />
                    </SelectTrigger>
                    <SelectContent>
                      {currentProvider?.models?.map((model) => (
                        <SelectItem key={model} value={model}>
                          {model}
                        </SelectItem>
                      )) || (
                        <SelectItem value="" disabled>
                          No models available
                        </SelectItem>
                      )}
                    </SelectContent>
                  </Select>
                </div>

                <div className="space-y-2">
                  <Label htmlFor="max-tokens">Max Tokens</Label>
                  <Input
                    id="max-tokens"
                    type="number"
                    value={formData.maxTokens}
                    onChange={(e) =>
                      setFormData((prev) => ({
                        ...prev,
                        maxTokens: e.target.value,
                      }))
                    }
                    placeholder="e.g., 4096"
                  />
                </div>

                <div className="space-y-2">
                  <Label htmlFor="temperature">Temperature</Label>
                  <Input
                    id="temperature"
                    type="number"
                    step="0.1"
                    min="0"
                    max="2"
                    value={formData.temperature}
                    onChange={(e) =>
                      setFormData((prev) => ({
                        ...prev,
                        temperature: e.target.value,
                      }))
                    }
                    placeholder="e.g., 0.7"
                  />
                </div>
              </div>

              <div className="space-y-2">
                <Label htmlFor="system-prompt">System Prompt</Label>
                <textarea
                  id="system-prompt"
                  className="w-full min-h-[100px] p-3 border rounded-md resize-y"
                  value={formData.systemPrompt}
                  onChange={(e) =>
                    setFormData((prev) => ({
                      ...prev,
                      systemPrompt: e.target.value,
                    }))
                  }
                  placeholder="Enter custom system prompt..."
                />
                <p className="text-sm text-muted-foreground">
                  Optional: Customize the AI's behavior with a custom system
                  prompt.
                </p>
              </div>

              <Button
                onClick={handleSaveProviderSettings}
                className="flex items-center gap-2"
                disabled={isLoading}
              >
                <Save size={16} />
                Save Provider Settings
              </Button>
            </div>
          )}
        </CardContent>
      </Card>

      {/* Tool Configuration Settings */}
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Terminal size={20} />
            Tool Configuration
          </CardTitle>
          <CardDescription>
            Enable or disable specific tools and tool categories for the AI
            agent
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          {toolConfigLoading ? (
            <div className="flex items-center gap-2 text-muted-foreground">
              <RefreshCw className="h-4 w-4 animate-spin" />
              Loading tool configurations...
            </div>
          ) : (
            <>
              {Object.entries(toolConfigurations).length === 0 ? (
                <div className="text-center p-4 text-muted-foreground">
                  <Terminal className="h-8 w-8 mx-auto mb-2 opacity-50" />
                  <p>Tool configuration will be available soon</p>
                  <p className="text-sm">
                    The system is being prepared for tool management
                  </p>
                </div>
              ) : (
                <div className="space-y-4">
                  {Object.entries(toolConfigurations).map(
                    ([categoryName, category]) => (
                      <div key={categoryName} className="border rounded-lg p-4">
                        <div className="flex items-center justify-between mb-3">
                          <div>
                            <h4 className="font-medium">{categoryName}</h4>
                            <p className="text-sm text-muted-foreground">
                              {category.description}
                            </p>
                          </div>
                          <Button
                            variant="ghost"
                            size="sm"
                            onClick={() =>
                              handleToggleCategory(
                                categoryName,
                                !category.enabled
                              )
                            }
                            className="flex items-center gap-2"
                          >
                            {category.enabled ? (
                              <>
                                <div className="w-4 h-4 bg-green-500 rounded-full"></div>
                                Enabled
                              </>
                            ) : (
                              <>
                                <div className="w-4 h-4 bg-gray-400 rounded-full"></div>
                                Disabled
                              </>
                            )}
                          </Button>
                        </div>

                        {category.tools && category.tools.length > 0 && (
                          <div className="space-y-2">
                            <div className="text-xs font-medium text-muted-foreground uppercase tracking-wide">
                              Tools (
                              {category.tools.filter((t) => t.enabled).length}/
                              {category.tools.length})
                            </div>
                            <div className="grid gap-2">
                              {category.tools.map((tool) => (
                                <div
                                  key={tool.name}
                                  className="flex items-center justify-between py-2 px-3 bg-gray-50 rounded"
                                >
                                  <div className="flex-1">
                                    <div className="flex items-center gap-2">
                                      <span className="text-sm font-medium">
                                        {tool.name}
                                      </span>
                                      {tool.required && (
                                        <span className="text-xs bg-blue-100 text-blue-800 px-2 py-1 rounded">
                                          Required
                                        </span>
                                      )}
                                    </div>
                                    {tool.description && (
                                      <p className="text-xs text-muted-foreground mt-1">
                                        {tool.description}
                                      </p>
                                    )}
                                  </div>
                                  <Button
                                    variant="ghost"
                                    size="sm"
                                    onClick={() =>
                                      handleToggleTool(tool.name, !tool.enabled)
                                    }
                                    disabled={tool.required && tool.enabled}
                                    className="ml-2"
                                  >
                                    {tool.enabled ? (
                                      <div className="w-3 h-3 bg-green-500 rounded-full"></div>
                                    ) : (
                                      <div className="w-3 h-3 bg-gray-400 rounded-full"></div>
                                    )}
                                  </Button>
                                </div>
                              ))}
                            </div>
                          </div>
                        )}
                      </div>
                    )
                  )}

                  <div className="flex gap-2 pt-4">
                    <Button
                      variant="outline"
                      onClick={handleResetToolConfiguration}
                      className="flex items-center gap-2"
                    >
                      <RefreshCw size={16} />
                      Reset to Defaults
                    </Button>
                    <Button
                      variant="outline"
                      onClick={loadToolConfigurations}
                      className="flex items-center gap-2"
                    >
                      <RefreshCw size={16} />
                      Refresh
                    </Button>
                  </div>
                </div>
              )}
            </>
          )}
        </CardContent>
      </Card>

      {/* Voice & Keyboard Shortcuts */}
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Keyboard size={20} />
            Voice & Keyboard Shortcuts
          </CardTitle>
          <CardDescription>
            Configure shortcuts for voice dictation and AI agent interaction
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-6">
          {/* Customizable Keyboard Shortcuts */}
          {shortcutsLoading ? (
            <div className="flex items-center gap-2 text-muted-foreground">
              <RefreshCw className="h-4 w-4 animate-spin" />
              <span>Updating shortcuts...</span>
            </div>
          ) : (
            <>
              <div className="space-y-4">
                <h4 className="text-sm font-medium text-muted-foreground uppercase tracking-wide border-b pb-2">
                  Customizable Shortcuts
                </h4>
                {Object.entries(keyboardShortcuts)
                  .filter(([key]) => key !== "open_settings") // Don't allow changing settings shortcut
                  .map(([key, value]) => (
                    <div key={key} className="space-y-2">
                      <Label htmlFor={`shortcut-${key}`}>
                        {getShortcutDisplayName(key)}
                      </Label>
                      <div className="flex items-center gap-3">
                        {editingShortcut === key ? (
                          <div className="flex items-center gap-2 flex-1">
                            <Input
                              id={`shortcut-${key}`}
                              value={value}
                              onChange={(e) =>
                                setKeyboardShortcuts((prev) => ({
                                  ...prev,
                                  [key]: e.target.value,
                                }))
                              }
                              placeholder="Enter shortcut (e.g., Alt+D)"
                              className="flex-1"
                              onKeyDown={(e) => {
                                if (e.key === "Enter") {
                                  handleShortcutChange(key, value);
                                } else if (e.key === "Escape") {
                                  setEditingShortcut(null);
                                  // Reset to original value
                                  const original =
                                    keyboardShortcuts[
                                      key as keyof KeyboardShortcuts
                                    ];
                                  setKeyboardShortcuts((prev) => ({
                                    ...prev,
                                    [key]: original,
                                  }));
                                }
                              }}
                              onBlur={() => {
                                handleShortcutChange(key, value);
                              }}
                              autoFocus
                            />
                            <Button
                              size="sm"
                              onClick={() => handleShortcutChange(key, value)}
                              disabled={shortcutsLoading}
                            >
                              <Save className="h-4 w-4" />
                            </Button>
                            <Button
                              size="sm"
                              variant="outline"
                              onClick={() => {
                                setEditingShortcut(null);
                                // Reset to original value
                                const original =
                                  keyboardShortcuts[
                                    key as keyof KeyboardShortcuts
                                  ];
                                setKeyboardShortcuts((prev) => ({
                                  ...prev,
                                  [key]: original,
                                }));
                              }}
                            >
                              Cancel
                            </Button>
                          </div>
                        ) : (
                          <div className="flex items-center justify-between flex-1">
                            <div className="flex items-center gap-3">
                              <kbd className="px-2 py-1 bg-muted rounded text-sm min-w-[80px] text-center">
                                {value || "Not set"}
                              </kbd>
                              <span className="text-sm text-muted-foreground">
                                {getShortcutDescription(key)}
                              </span>
                            </div>
                            <Button
                              size="sm"
                              variant="outline"
                              onClick={() => setEditingShortcut(key)}
                            >
                              Edit
                            </Button>
                          </div>
                        )}
                      </div>
                    </div>
                  ))}
              </div>

              <div className="flex gap-2 pt-4 border-t">
                <Button
                  variant="outline"
                  size="sm"
                  onClick={handleResetShortcuts}
                  disabled={shortcutsLoading}
                >
                  <RotateCcw className="h-4 w-4 mr-1" />
                  Reset to Defaults
                </Button>
              </div>
            </>
          )}

          {/* Fixed System Shortcuts */}
          <div className="space-y-4">
            <h4 className="text-sm font-medium text-muted-foreground uppercase tracking-wide border-b pb-2">
              System Shortcuts
            </h4>
            <div className="grid gap-3">
              <div className="flex items-center justify-between p-2 rounded border">
                <span className="text-sm">Cancel Current Operation</span>
                <kbd className="px-2 py-1 bg-muted rounded text-sm font-mono">
                  Esc
                </kbd>
              </div>
              <div className="flex items-center justify-between p-2 rounded border">
                <span className="text-sm">Open Settings</span>
                <kbd className="px-2 py-1 bg-muted rounded text-sm font-mono">
                  {keyboardShortcuts.open_settings || "⌘+,"}
                </kbd>
              </div>
            </div>
          </div>

          {/* Voice Settings */}
          <div className="space-y-4">
            <h4 className="text-sm font-medium text-muted-foreground uppercase tracking-wide border-b pb-2">
              Voice Settings
            </h4>
            <div className="space-y-3">
              <div className="flex items-center justify-between">
                <div>
                  <span className="text-sm font-medium">
                    Enable Voice Feedback
                  </span>
                  <p className="text-xs text-muted-foreground">
                    Play audio responses from AI agent
                  </p>
                </div>
                <Switch
                  checked={soundEnabled}
                  onCheckedChange={handleSoundEnabledChange}
                  id="voice-feedback"
                />
              </div>

              <div className="flex items-center justify-between">
                <div>
                  <span className="text-sm font-medium">
                    Dictation to Clipboard
                  </span>
                  <p className="text-xs text-muted-foreground">
                    Copy dictated text to clipboard automatically
                  </p>
                </div>
                <Switch
                  checked={dictationClipboardEnabled}
                  onCheckedChange={handleDictationClipboardChange}
                  id="dictation-clipboard"
                />
              </div>
            </div>
          </div>

          {/* Usage Tips */}
          <div className="bg-muted/50 p-4 rounded-lg">
            <h5 className="text-sm font-medium mb-2">💡 Voice Usage Tips</h5>
            <ul className="text-xs text-muted-foreground space-y-1 list-disc list-inside">
              <li>
                Configure your shortcuts above for dictation and AI agent access
              </li>
              <li>
                The floating bar shows real-time voice status and transcription
              </li>
              <li>
                Press Escape anytime to cancel voice input or stop the agent
              </li>
              <li>Voice feedback can be toggled in TTS settings above</li>
            </ul>
            <div className="text-xs text-muted-foreground space-y-1 mt-3 pt-2 border-t">
              <p>
                <strong>Tips:</strong> Use modifier keys like Alt, Cmd, Ctrl,
                Shift combined with letters (e.g., Alt+D, Cmd+Space).
              </p>
              <p>Changes are applied immediately and saved automatically.</p>
            </div>
          </div>
        </CardContent>
      </Card>

      {/* MCP Server Settings */}
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Server size={20} />
            MCP Servers
          </CardTitle>
          <CardDescription>
            Manage Model Context Protocol servers for extended functionality
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-2">
              <h3 className="text-lg font-medium">Connected Servers</h3>
              {mcpLoading && <RefreshCw className="h-4 w-4 animate-spin" />}
            </div>
            <div className="flex gap-2">
              <Button variant="outline" size="sm" onClick={loadMcpServers} disabled={mcpLoading}>
                <RefreshCw className="h-4 w-4 mr-1" />
                Refresh
              </Button>
              <Button onClick={handleAddMcpServer} size="sm">
                <Plus className="h-4 w-4 mr-1" />
                Add Server
              </Button>
            </div>
          </div>

          {mcpServers.length === 0 ? (
            <div className="text-center py-8 border-2 border-dashed border-muted rounded-lg">
              <Server className="h-12 w-12 mx-auto text-muted-foreground mb-4" />
              <h3 className="text-lg font-medium mb-2">No MCP Servers</h3>
              <p className="text-muted-foreground mb-4">
                Add MCP servers to extend the AI agent with additional tools and capabilities.
              </p>
              <Button onClick={handleAddMcpServer}>
                <Plus className="h-4 w-4 mr-1" />
                Add Your First Server
              </Button>
            </div>
          ) : (
            <div className="space-y-3">
              {mcpServers.map((server) => {
                const status = mcpServerStatuses[server.id] || { Disconnected: null };
                const isConnected = status.Connected !== undefined;
                const isConnecting = status.Connecting !== undefined;
                const hasError = status.Error !== undefined;
                const serverTools = mcpTools.filter(tool => tool.server_id === server.id);

                return (
                  <div key={server.id} className="border rounded-lg p-4 space-y-3">
                    <div className="flex items-center justify-between">
                      <div className="flex items-center gap-3">
                        {getMcpServerStatusIcon(status)}
                        <div>
                          <h4 className="font-medium">{server.name}</h4>
                          {server.description && (
                            <p className="text-sm text-muted-foreground">{server.description}</p>
                          )}
                        </div>
                      </div>
                      <div className="flex items-center gap-2">
                        {getMcpServerStatusBadge(status)}
                        <Switch
                          checked={server.enabled}
                          onCheckedChange={(enabled) => handleToggleMcpServer(server.id, enabled)}
                        />
                      </div>
                    </div>

                    <div className="grid grid-cols-2 gap-2 text-sm">
                      <div>
                        <span className="text-muted-foreground">Command:</span>
                        <code className="ml-2 px-1 py-0.5 bg-muted rounded text-xs">
                          {server.command} {server.args.join(" ")}
                        </code>
                      </div>
                      <div>
                        <span className="text-muted-foreground">Tools:</span>
                        <span className="ml-2">{serverTools.length} available</span>
                      </div>
                    </div>

                    {hasError && (
                      <div className="p-2 bg-red-50 border border-red-200 rounded text-sm text-red-600">
                        <strong>Error:</strong> {status.Error}
                      </div>
                    )}

                    {serverTools.length > 0 && (
                      <div className="space-y-2">
                        <p className="text-sm font-medium">Available Tools:</p>
                        <div className="flex flex-wrap gap-1">
                          {serverTools.slice(0, 5).map((tool) => (
                            <Badge key={tool.tool_definition.name} variant="secondary" className="text-xs">
                              {tool.tool_definition.name.replace(`${server.name}_`, "")}
                            </Badge>
                          ))}
                          {serverTools.length > 5 && (
                            <Badge variant="outline" className="text-xs">
                              +{serverTools.length - 5} more
                            </Badge>
                          )}
                        </div>
                      </div>
                    )}

                    <div className="flex gap-2 pt-2 border-t">
                      {server.enabled && !isConnected && !isConnecting && (
                        <Button
                          variant="outline"
                          size="sm"
                          onClick={() => handleStartMcpServer(server.id)}
                        >
                          <Play className="h-4 w-4 mr-1" />
                          Start
                        </Button>
                      )}
                      {server.enabled && (isConnected || isConnecting) && (
                        <Button
                          variant="outline"
                          size="sm"
                          onClick={() => handleStopMcpServer(server.id)}
                        >
                          <Square className="h-4 w-4 mr-1" />
                          Stop
                        </Button>
                      )}
                      <Button
                        variant="outline"
                        size="sm"
                        onClick={() => handleEditMcpServer(server)}
                      >
                        <Edit className="h-4 w-4 mr-1" />
                        Edit
                      </Button>
                      <Button
                        variant="outline"
                        size="sm"
                        onClick={() => handleDeleteMcpServer(server.id)}
                        className="text-red-600 hover:text-red-700"
                      >
                        <Trash2 className="h-4 w-4 mr-1" />
                        Delete
                      </Button>
                    </div>
                  </div>
                );
              })}
            </div>
          )}

          <div className="pt-4 border-t text-sm text-muted-foreground">
            <p className="mb-2">
              <strong>Popular MCP Servers:</strong>
            </p>
            <ul className="space-y-1 text-xs">
              <li>• <strong>File System:</strong> npx @modelcontextprotocol/server-filesystem /path/to/directory</li>
              <li>• <strong>SQLite:</strong> npx @modelcontextprotocol/server-sqlite /path/to/database.db</li>
              <li>• <strong>Git:</strong> npx @modelcontextprotocol/server-git /path/to/repo</li>
              <li>• <strong>Brave Search:</strong> npx @modelcontextprotocol/server-brave-search</li>
            </ul>
            <div className="mt-2 pt-2 border-t">
              <a
                href="https://github.com/modelcontextprotocol/servers"
                target="_blank"
                rel="noopener noreferrer"
                className="inline-flex items-center gap-1 text-blue-600 hover:text-blue-700"
              >
                <ExternalLink className="h-3 w-3" />
                Browse more servers on GitHub
              </a>
            </div>
          </div>
        </CardContent>
      </Card>

      {/* MCP Server Add/Edit Dialog */}
      <Dialog open={showAddMcpDialog} onOpenChange={setShowAddMcpDialog}>
        <DialogContent className="max-w-2xl max-h-[80vh] overflow-y-auto">
          <DialogHeader>
            <DialogTitle>
              {editingMcpServer ? "Edit MCP Server" : "Add MCP Server"}
            </DialogTitle>
            <DialogDescription>
              Configure a Model Context Protocol server to extend the AI agent with additional tools.
            </DialogDescription>
          </DialogHeader>

          <div className="space-y-4">
            <div className="grid grid-cols-2 gap-4">
              <div className="space-y-2">
                <Label htmlFor="mcp-name">Name *</Label>
                <Input
                  id="mcp-name"
                  value={mcpFormData.name}
                  onChange={(e) => setMcpFormData(prev => ({ ...prev, name: e.target.value }))}
                  placeholder="e.g., File System Tools"
                />
              </div>
              <div className="space-y-2">
                <Label htmlFor="mcp-command">Command *</Label>
                <Input
                  id="mcp-command"
                  value={mcpFormData.command}
                  onChange={(e) => setMcpFormData(prev => ({ ...prev, command: e.target.value }))}
                  placeholder="e.g., npx or /usr/local/bin/mcp-server"
                />
              </div>
            </div>

            <div className="space-y-2">
              <Label htmlFor="mcp-description">Description</Label>
              <Input
                id="mcp-description"
                value={mcpFormData.description}
                onChange={(e) => setMcpFormData(prev => ({ ...prev, description: e.target.value }))}
                placeholder="Brief description of what this server provides"
              />
            </div>

            {/* Editor Mode Toggle */}
            <div className="flex items-center justify-between p-3 bg-muted/50 rounded-lg">
              <div className="flex items-center gap-2">
                <Label htmlFor="mcp-json-mode">Configuration Mode</Label>
                <Badge variant={mcpJsonMode ? "default" : "secondary"}>
                  {mcpJsonMode ? "JSON Editor" : "Form Editor"}
                </Badge>
              </div>
              <div className="flex items-center gap-2">
                {!mcpJsonMode && (
                  <Button
                    type="button"
                    variant="outline"
                    size="sm"
                    onClick={convertFormToJson}
                    title="Convert form data to JSON"
                  >
                    → JSON
                  </Button>
                )}
                {mcpJsonMode && (
                  <Button
                    type="button"
                    variant="outline"
                    size="sm"
                    onClick={convertJsonToForm}
                    title="Convert JSON to form"
                  >
                    → Form
                  </Button>
                )}
                <Switch
                  id="mcp-json-mode"
                  checked={mcpJsonMode}
                  onCheckedChange={setMcpJsonMode}
                />
              </div>
            </div>

            {mcpJsonMode ? (
              // JSON Editor Mode
              <div className="space-y-2">
                <Label htmlFor="mcp-json">Server Configuration (JSON)</Label>
                <Textarea
                  id="mcp-json"
                  value={mcpJsonData}
                  onChange={(e) => setMcpJsonData(e.target.value)}
                  placeholder='{\n  "my-server": {\n    "command": "npx",\n    "args": ["@modelcontextprotocol/server-filesystem", "/path"],\n    "env": {\n      "API_KEY": "your-api-key-here"\n    }\n  }\n}'
                  className="h-64 font-mono text-sm"
                />
                <p className="text-xs text-muted-foreground">
                  Edit server configuration using Cursor/Claude compatible format. Server name as key, required fields: command, args
                </p>
              </div>
            ) : (
              // Form Editor Mode (existing form fields)
              <>
                <div className="space-y-2">
                  <Label htmlFor="mcp-args">Arguments</Label>
              <Input
                id="mcp-args"
                value={mcpFormData.args}
                onChange={(e) => setMcpFormData(prev => ({ ...prev, args: e.target.value }))}
                placeholder="e.g., @modelcontextprotocol/server-filesystem /path/to/directory"
              />
              <p className="text-xs text-muted-foreground">
                Space-separated arguments for the command
              </p>
            </div>

            <div className="space-y-2">
              <Label htmlFor="mcp-working-dir">Working Directory</Label>
              <Input
                id="mcp-working-dir"
                value={mcpFormData.workingDirectory}
                onChange={(e) => setMcpFormData(prev => ({ ...prev, workingDirectory: e.target.value }))}
                placeholder="/path/to/working/directory (optional)"
              />
            </div>

            <div className="space-y-2">
              <Label htmlFor="mcp-env-vars">Environment Variables</Label>
              <Textarea
                id="mcp-env-vars"
                value={mcpFormData.environmentVariables}
                onChange={(e) => setMcpFormData(prev => ({ ...prev, environmentVariables: e.target.value }))}
                placeholder='{"API_KEY": "your-key", "CONFIG_PATH": "/path/to/config"}'
                className="h-20"
              />
              <p className="text-xs text-muted-foreground">
                JSON object of environment variables (optional)
              </p>
            </div>

            <div className="grid grid-cols-2 gap-4">
              <div className="space-y-2">
                <Label htmlFor="mcp-timeout">Timeout (seconds)</Label>
                <Input
                  id="mcp-timeout"
                  type="number"
                  value={mcpFormData.timeoutSeconds}
                  onChange={(e) => setMcpFormData(prev => ({ ...prev, timeoutSeconds: e.target.value }))}
                  min="1"
                  max="300"
                />
              </div>
              <div className="space-y-2">
                <Label htmlFor="mcp-retries">Max Retries</Label>
                <Input
                  id="mcp-retries"
                  type="number"
                  value={mcpFormData.maxRetries}
                  onChange={(e) => setMcpFormData(prev => ({ ...prev, maxRetries: e.target.value }))}
                  min="0"
                  max="10"
                />
              </div>
            </div>

            <div className="flex items-center space-x-2">
              <Switch
                id="mcp-auto-start"
                checked={mcpFormData.autoStart}
                onCheckedChange={(checked) => setMcpFormData(prev => ({ ...prev, autoStart: checked }))}
              />
              <Label htmlFor="mcp-auto-start">Auto-start when app launches</Label>
            </div>
              </>
            )}
          </div>

          <DialogFooter className="gap-2">
            <Button variant="outline" onClick={handleTestMcpServer}>
              Test Connection
            </Button>
            <Button variant="outline" onClick={() => setShowAddMcpDialog(false)}>
              Cancel
            </Button>
            <Button onClick={handleSaveMcpServer}>
              {editingMcpServer ? "Update" : "Add"} Server
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* Developer Tools */}
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Terminal size={20} />
            Developer Tools
          </CardTitle>
          <CardDescription>
            Access developer tools and application windows
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
            <Button
              variant="outline"
              className="h-20 flex flex-col items-center justify-center gap-2"
              onClick={onNavigateToDevTools}
            >
              <Terminal size={24} />
              <div className="text-center">
                <div className="font-medium">Developer Tools</div>
                <div className="text-xs text-muted-foreground">
                  Debug and testing tools
                </div>
              </div>
            </Button>

            <Button
              variant="outline"
              className="h-20 flex flex-col items-center justify-center gap-2"
              onClick={onNavigateToChat}
            >
              <MonitorSpeaker size={24} />
              <div className="text-center">
                <div className="font-medium">Main Chat</div>
                <div className="text-xs text-muted-foreground">
                  Return to main interface
                </div>
              </div>
            </Button>
          </div>

          <p className="text-sm text-muted-foreground">
            You can also access Developer Tools from the system tray menu or use
            the toggle button in the main interface.
          </p>
        </CardContent>
      </Card>

      {/* Permissions Settings */}
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Shield size={20} />
            macOS Permissions
          </CardTitle>
          <CardDescription>
            Manage system permissions required for AI computer use features
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          {permissionsLoading ? (
            <div className="flex items-center gap-2 text-muted-foreground">
              <RefreshCw className="h-4 w-4 animate-spin" />
              <span>Checking permissions...</span>
            </div>
          ) : permissionsState ? (
            <div className="space-y-3">
              {/* Overall Status */}
              <div className="flex items-center justify-between p-3 bg-muted/50 rounded-lg">
                <div className="flex items-center gap-2">
                  {permissionsState.allGranted ? (
                    <CheckCircle className="h-5 w-5 text-green-500" />
                  ) : (
                    <AlertCircle className="h-5 w-5 text-red-500" />
                  )}
                  <span className="font-medium">
                    {permissionsState.allGranted
                      ? "All permissions granted"
                      : "Some permissions missing"}
                  </span>
                </div>
                <Badge
                  variant={
                    permissionsState.allGranted ? "default" : "destructive"
                  }
                >
                  {permissionsState.allGranted ? "Ready" : "Needs Setup"}
                </Badge>
              </div>

              {/* Individual Permissions */}
              <div className="grid gap-2">
                {/* Accessibility */}
                <div className="flex items-center justify-between p-2 border rounded">
                  <div className="flex items-center gap-2">
                    {getPermissionIcon(
                      permissionsState.accessibility.granted,
                      permissionsState.accessibility.required
                    )}
                    <span className="text-sm">Accessibility</span>
                  </div>
                  {getPermissionBadge(
                    permissionsState.accessibility.granted,
                    permissionsState.accessibility.required
                  )}
                </div>

                {/* Screen Recording */}
                <div className="flex items-center justify-between p-2 border rounded">
                  <div className="flex items-center gap-2">
                    {getPermissionIcon(
                      permissionsState.screenRecording.granted,
                      permissionsState.screenRecording.required
                    )}
                    <span className="text-sm">Screen Recording</span>
                  </div>
                  {getPermissionBadge(
                    permissionsState.screenRecording.granted,
                    permissionsState.screenRecording.required
                  )}
                </div>

                {/* Microphone */}
                <div className="flex items-center justify-between p-2 border rounded">
                  <div className="flex items-center gap-2">
                    {getPermissionIcon(
                      permissionsState.microphone.granted,
                      permissionsState.microphone.required
                    )}
                    <span className="text-sm">Microphone</span>
                  </div>
                  {getPermissionBadge(
                    permissionsState.microphone.granted,
                    permissionsState.microphone.required
                  )}
                </div>
              </div>

              {/* Action Buttons */}
              <div className="flex gap-2 pt-2">
                <Button
                  variant="outline"
                  size="sm"
                  onClick={loadPermissionsStatus}
                  disabled={permissionsLoading}
                >
                  <RefreshCw className="h-4 w-4 mr-1" />
                  Refresh
                </Button>
                <Button
                  onClick={onNavigateToPermissions}
                  size="sm"
                  variant={permissionsState.allGranted ? "outline" : "default"}
                >
                  <Shield className="h-4 w-4 mr-1" />
                  {permissionsState.allGranted
                    ? "View Details"
                    : "Setup Permissions"}
                </Button>
              </div>
            </div>
          ) : (
            <div className="text-center py-4 text-muted-foreground">
              <p>Unable to check permissions status</p>
              <Button
                variant="outline"
                size="sm"
                onClick={loadPermissionsStatus}
                className="mt-2"
              >
                <RefreshCw className="h-4 w-4 mr-1" />
                Try Again
              </Button>
            </div>
          )}
        </CardContent>
      </Card>
    </div>
  );
};

export default Settings;
