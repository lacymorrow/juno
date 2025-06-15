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
import { Textarea } from "@/components/ui/textarea";
import { KeyboardShortcuts } from "@/types/keyboard";
import { invoke } from "@tauri-apps/api/core";
import {
  AlertCircle,
  Brain,
  CheckCircle,
  ExternalLink,
  Keyboard,
  Mic,
  MonitorSpeaker,
  Network,
  RefreshCw,
  RotateCcw,
  Save,
  Server,
  Settings as SettingsIcon,
  Shield,
  Square,
  Terminal,
  Edit3,
  AlertTriangle,
} from "lucide-react";
import React, { useEffect, useState, useCallback } from "react";
import { toast } from "sonner";
import { cn } from "@/lib/utils";

interface ProviderInfo {
  id: string;
  name: string;
  description: string;
  models: string[];
  default_model: string;
  computer_use_supported: boolean;
  model_info?: {
    id: string;
    name: string;
    supports_computer_use: boolean;
    is_recommended: boolean;
  }[];
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

interface ShortcutInputProps {
  label: string;
  description: string;
  value: string;
  shortcutName: string;
  isSystemManaged?: boolean;
  onSave: (shortcutName: string, value: string) => Promise<void>;
  isLoading: boolean;
}

const ShortcutInput: React.FC<ShortcutInputProps> = ({
  label,
  description,
  value,
  shortcutName,
  isSystemManaged = false,
  onSave,
  isLoading,
}) => {
  const [isEditing, setIsEditing] = useState(false);
  const [isCapturing, setIsCapturing] = useState(false);
  const [currentValue, setCurrentValue] = useState(value);
  const [validationMessage, setValidationMessage] = useState<string>("");
  const [validationError, setValidationError] = useState<string>("");
  const [pressedKeys, setPressedKeys] = useState<string[]>([]);
  const [captureTimeout, setCaptureTimeout] = useState<NodeJS.Timeout | null>(
    null
  );

  // Update current value when prop changes
  useEffect(() => {
    setCurrentValue(value);
    setValidationMessage("");
    setValidationError("");
  }, [value]);

  // Cleanup timeout on unmount
  useEffect(() => {
    return () => {
      if (captureTimeout) {
        clearTimeout(captureTimeout);
      }
    };
  }, [captureTimeout]);

  // Validation function with debouncing
  const validateShortcut = useCallback(
    async (shortcutValue: string) => {
      if (!shortcutValue.trim()) {
        setValidationMessage("Enter a shortcut combination");
        setValidationError("");
        return;
      }

      try {
        const result = await invoke<string>("validate_keyboard_shortcut", {
          shortcutValue: shortcutValue,
          shortcutName: shortcutName,
        });
        setValidationMessage(result);
        setValidationError("");
      } catch (error) {
        setValidationError(
          typeof error === "string" ? error : "Invalid shortcut"
        );
        setValidationMessage("");
      }
    },
    [shortcutName]
  );

  // Debounced validation
  useEffect(() => {
    if (isEditing) {
      const timeoutId = setTimeout(() => {
        validateShortcut(currentValue);
      }, 300);
      return () => clearTimeout(timeoutId);
    }
  }, [currentValue, isEditing, validateShortcut]);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (!isCapturing) return;

      e.preventDefault();
      e.stopPropagation();

      // Clear any existing timeout
      if (captureTimeout) {
        clearTimeout(captureTimeout);
      }

      const modifiers: string[] = [];

      // Detect modifiers with platform-aware naming
      if (e.ctrlKey || e.metaKey) {
        if (e.metaKey) {
          modifiers.push("Cmd");
        } else {
          modifiers.push("Ctrl");
        }
      }
      if (e.altKey) {
        // Use Option on macOS for better UX
        modifiers.push(
          navigator.platform.toLowerCase().includes("mac") ? "Option" : "Alt"
        );
      }
      if (e.shiftKey) modifiers.push("Shift");

      let key = "";

      // Enhanced key detection with better special key handling
      switch (e.code) {
        case "Space":
          key = "Space";
          break;
        case "Escape":
          key = "Escape";
          break;
        case "Enter":
          key = "Enter";
          break;
        case "Tab":
          key = "Tab";
          break;
        case "Backspace":
          key = "Backspace";
          break;
        case "Delete":
          key = "Delete";
          break;
        case "Home":
          key = "Home";
          break;
        case "End":
          key = "End";
          break;
        case "PageUp":
          key = "PageUp";
          break;
        case "PageDown":
          key = "PageDown";
          break;
        case "Insert":
          key = "Insert";
          break;
        case "ArrowUp":
          key = "Up";
          break;
        case "ArrowDown":
          key = "Down";
          break;
        case "ArrowLeft":
          key = "Left";
          break;
        case "ArrowRight":
          key = "Right";
          break;
        case "PrintScreen":
          key = "PrintScreen";
          break;
        case "ScrollLock":
          key = "ScrollLock";
          break;
        case "Pause":
          key = "Pause";
          break;
        case "CapsLock":
          key = "CapsLock";
          break;
        case "NumLock":
          key = "NumLock";
          break;
        default:
          // Function keys (including extended range)
          if (e.code.startsWith("F") && e.code.length <= 3) {
            key = e.code;
          }
          // Number keys
          else if (e.code.startsWith("Digit")) {
            key = e.code.replace("Digit", "");
          }
          // Letter keys
          else if (e.code.startsWith("Key")) {
            key = e.code.replace("Key", "");
          }
          // Numpad keys
          else if (e.code.startsWith("Numpad")) {
            key = "Numpad" + e.code.replace("Numpad", "");
          }
          // Punctuation and special characters
          else if (
            e.key.length === 1 &&
            !e.ctrlKey &&
            !e.metaKey &&
            !e.altKey
          ) {
            key = e.key;
          }
          // Fallback to the key name for other special keys
          else if (
            e.key &&
            e.key !== "Control" &&
            e.key !== "Alt" &&
            e.key !== "Shift" &&
            e.key !== "Meta"
          ) {
            key = e.key;
          }
          break;
      }

      if (key) {
        const allKeys = [...modifiers, key];
        setPressedKeys(allKeys);

        const shortcutString = allKeys.join("+");
        setCurrentValue(shortcutString);

        // Auto-finish capture with longer delay for complex combinations
        const delay = modifiers.length >= 2 ? 800 : 500;
        const newTimeout = setTimeout(() => {
          setIsCapturing(false);
          setPressedKeys([]);
          setCaptureTimeout(null);
        }, delay);
        setCaptureTimeout(newTimeout);
      }
    },
    [isCapturing, captureTimeout]
  );

  const handleStartCapture = () => {
    setIsCapturing(true);
    setPressedKeys([]);
    setCurrentValue("");
    setValidationMessage("Press the key combination you want to use...");
    setValidationError("");

    // Auto-cancel capture after 10 seconds to prevent UI getting stuck
    const cancelTimeout = setTimeout(() => {
      setIsCapturing(false);
      setPressedKeys([]);
      setValidationMessage("");
      setCaptureTimeout(null);
    }, 10000);
    setCaptureTimeout(cancelTimeout);
  };

  const handleStopCapture = () => {
    if (captureTimeout) {
      clearTimeout(captureTimeout);
      setCaptureTimeout(null);
    }
    setIsCapturing(false);
    setPressedKeys([]);
  };

  const handleSave = async () => {
    if (validationError) {
      toast.error(validationError);
      return;
    }

    try {
      await onSave(shortcutName, currentValue);
      setIsEditing(false);
      setIsCapturing(false);
      setPressedKeys([]);
      if (captureTimeout) {
        clearTimeout(captureTimeout);
        setCaptureTimeout(null);
      }
      toast.success("Shortcut updated successfully");
    } catch (error) {
      console.error("Failed to save shortcut:", error);
      toast.error("Failed to save shortcut");
    }
  };

  const handleCancel = () => {
    setCurrentValue(value);
    setIsEditing(false);
    setIsCapturing(false);
    setPressedKeys([]);
    setValidationMessage("");
    setValidationError("");
    if (captureTimeout) {
      clearTimeout(captureTimeout);
      setCaptureTimeout(null);
    }
  };

  // Get platform-appropriate example
  const getExampleShortcut = () => {
    const isMac = navigator.platform.toLowerCase().includes("mac");
    if (shortcutName === "agent_mode_toggle") {
      return isMac ? "Option+D" : "Alt+D";
    } else if (shortcutName === "dictation_input") {
      return isMac ? "Option+Space" : "Alt+Space";
    }
    return isMac ? "Cmd+K" : "Ctrl+K";
  };

  if (isSystemManaged) {
    return (
      <div className="space-y-2">
        <Label>{label}</Label>
        <div className="flex items-center justify-between p-2 rounded border">
          <div className="flex items-center gap-3">
            <kbd className="px-2 py-1 bg-muted rounded text-sm min-w-[80px] text-center">
              {value}
            </kbd>
            <span className="text-sm text-muted-foreground">{description}</span>
          </div>
          <Badge variant="secondary" className="text-xs">
            System
          </Badge>
        </div>
      </div>
    );
  }

  return (
    <div className="space-y-2">
      <Label htmlFor={`shortcut-${shortcutName}`}>{label}</Label>
      <div className="space-y-2">
        {isEditing ? (
          <div className="space-y-3 p-3 border rounded-lg bg-muted/30">
            {/* Key capture area with enhanced feedback */}
            <div className="space-y-2">
              <div className="flex items-center gap-2">
                <Label className="text-sm font-medium">
                  Shortcut combination:
                </Label>
                {isCapturing && (
                  <div className="flex items-center gap-2">
                    <Badge
                      variant="outline"
                      className="text-xs animate-pulse bg-blue-50 border-blue-200"
                    >
                      🎯 Listening...
                    </Badge>
                    <button
                      onClick={handleStopCapture}
                      className="text-xs text-muted-foreground hover:text-foreground"
                    >
                      Stop
                    </button>
                  </div>
                )}
              </div>
              <div
                className={cn(
                  "min-h-[50px] p-3 border-2 border-dashed rounded-lg flex items-center gap-2 cursor-pointer transition-all duration-200",
                  isCapturing
                    ? "border-blue-500 bg-blue-50 dark:bg-blue-950/20 shadow-sm"
                    : "border-muted-foreground/30 hover:border-muted-foreground/50 hover:bg-muted/50"
                )}
                onClick={!isCapturing ? handleStartCapture : undefined}
                onKeyDown={handleKeyDown}
                tabIndex={0}
                role="button"
                aria-label={
                  isCapturing
                    ? "Press keys to capture shortcut"
                    : "Click to start capturing shortcut"
                }
              >
                {pressedKeys.length > 0 ? (
                  <div className="flex items-center gap-1 flex-wrap">
                    {pressedKeys.map((key, index) => (
                      <span key={index} className="flex items-center gap-1">
                        <kbd className="px-2 py-1 bg-background border rounded text-sm font-mono shadow-sm">
                          {key}
                        </kbd>
                        {index < pressedKeys.length - 1 && (
                          <span className="text-muted-foreground font-medium">
                            +
                          </span>
                        )}
                      </span>
                    ))}
                  </div>
                ) : currentValue ? (
                  <kbd className="px-2 py-1 bg-background border rounded text-sm font-mono shadow-sm">
                    {currentValue}
                  </kbd>
                ) : (
                  <div className="flex items-center gap-2 text-muted-foreground">
                    <Keyboard className="h-4 w-4" />
                    <span className="text-sm">
                      {isCapturing
                        ? "Press the keys you want to use..."
                        : `Click here to capture shortcut (e.g., ${getExampleShortcut()})`}
                    </span>
                  </div>
                )}
              </div>
            </div>

            {/* Manual input option with better guidance */}
            <div className="space-y-2">
              <Label className="text-sm">Or type manually:</Label>
              <Input
                value={currentValue}
                onChange={(e) => setCurrentValue(e.target.value)}
                placeholder={`e.g., ${getExampleShortcut()}, Ctrl+Shift+F1`}
                className="font-mono text-sm"
                disabled={isCapturing}
              />
              <div className="text-xs text-muted-foreground">
                Tip: Use modifiers like Alt, Ctrl, Cmd, Shift combined with
                letters or function keys
              </div>
            </div>

            {/* Enhanced validation feedback */}
            {(validationMessage || validationError) && (
              <div
                className={cn(
                  "flex items-start gap-2 text-sm p-3 rounded-md border",
                  validationError
                    ? "text-red-700 bg-red-50 border-red-200 dark:bg-red-950/20 dark:border-red-800 dark:text-red-400"
                    : "text-green-700 bg-green-50 border-green-200 dark:bg-green-950/20 dark:border-green-800 dark:text-green-400"
                )}
              >
                <div className="flex-shrink-0 mt-0.5">
                  {validationError ? (
                    <AlertTriangle className="h-4 w-4" />
                  ) : (
                    <CheckCircle className="h-4 w-4" />
                  )}
                </div>
                <div className="flex-1">
                  <span>{validationError || validationMessage}</span>
                  {validationError && validationError.includes("conflicts") && (
                    <div className="mt-1 text-xs opacity-75">
                      Consider using a different key combination to avoid
                      conflicts.
                    </div>
                  )}
                </div>
              </div>
            )}

            {/* Action buttons */}
            <div className="flex items-center gap-2 pt-2 border-t">
              <Button
                size="sm"
                onClick={handleSave}
                disabled={
                  isLoading ||
                  !!validationError ||
                  !currentValue.trim() ||
                  isCapturing
                }
                className="flex items-center gap-2"
              >
                <Save className="h-4 w-4" />
                Save
              </Button>
              <Button
                size="sm"
                variant="outline"
                onClick={handleCancel}
                disabled={isLoading}
              >
                Cancel
              </Button>
              <Button
                size="sm"
                variant="outline"
                onClick={isCapturing ? handleStopCapture : handleStartCapture}
                disabled={isLoading}
                className="ml-auto"
              >
                <Keyboard className="h-4 w-4 mr-1" />
                {isCapturing ? "Stop Capture" : "Capture Keys"}
              </Button>
            </div>
          </div>
        ) : (
          <div className="flex items-center justify-between p-2 rounded border hover:bg-muted/50 transition-colors">
            <div className="flex items-center gap-3">
              <kbd className="px-2 py-1 bg-muted rounded text-sm min-w-[80px] text-center font-mono">
                {value || "Not set"}
              </kbd>
              <span className="text-sm text-muted-foreground">
                {description}
              </span>
            </div>
            <Button
              size="sm"
              variant="outline"
              onClick={() => setIsEditing(true)}
              disabled={isLoading}
              className="flex items-center gap-1"
            >
              <Edit3 className="h-4 w-4" />
              Edit
            </Button>
          </div>
        )}
      </div>
    </div>
  );
};

const Settings: React.FC<SettingsProps> = ({
  onNavigateToDevTools,
  onNavigateToChat,
  onNavigateToPermissions,
}) => {
  // TTS Settings
  const [ttsProvider, setTtsProvider] = useState<string>("system");

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

  // Always Listening Settings
  const [alwaysListeningActive, setAlwaysListeningActive] =
    useState<boolean>(false);
  const [alwaysListeningSensitivity, setAlwaysListeningSensitivity] =
    useState<number>(0.5);
  const [alwaysListeningWakeWords, setAlwaysListeningWakeWords] = useState<
    string[]
  >(["hey juno", "computer"]);
  const [wakeWordsInput, setWakeWordsInput] = useState<string>("");

  // Tool Configuration Settings
  const [toolConfigurations, setToolConfigurations] = useState<
    Record<string, ToolCategory>
  >({});
  const [toolConfigLoading, setToolConfigLoading] = useState<boolean>(false);

  // MCP Server Settings
  const [mcpServers, setMcpServers] = useState<MCPServerConfig[]>([]);
  const [mcpServerStatuses, setMcpServerStatuses] = useState<
    Record<string, MCPServerStatus>
  >({});
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
  const [keyboardShortcuts, setKeyboardShortcuts] = useState<KeyboardShortcuts>(
    {
      agent_mode_toggle: "",
      dictation_input: "",
      stop_current_task: "",
      open_settings: "",
    }
  );
  const [shortcutsLoading, setShortcutsLoading] = useState<boolean>(false);

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

      // Load always listening settings
      const alwaysListeningStatus = await invoke<boolean>(
        "get_always_listening_status"
      );
      setAlwaysListeningActive(alwaysListeningStatus);

      const sensitivity = await invoke<number>(
        "get_always_listening_sensitivity"
      );
      setAlwaysListeningSensitivity(sensitivity);

      const wakeWords = await invoke<string[]>(
        "get_always_listening_wake_words"
      );
      setAlwaysListeningWakeWords(wakeWords);
      setWakeWordsInput(wakeWords.join(", "));

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
      const shortcuts = await invoke<KeyboardShortcuts>(
        "get_keyboard_shortcuts"
      );
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
      console.log("Loading MCP servers...");

      // Load MCP server configurations
      console.log("Fetching MCP server configurations...");
      const servers = await invoke<MCPServerConfig[]>("get_mcp_servers");
      console.log("MCP servers loaded:", servers);
      setMcpServers(servers);

      // Load MCP server statuses
      console.log("Fetching MCP server statuses...");
      const statuses = await invoke<Record<string, MCPServerStatus>>(
        "get_mcp_server_statuses"
      );
      console.log("MCP server statuses loaded:", statuses);
      setMcpServerStatuses(statuses);

      // Load MCP tools
      console.log("Fetching MCP tools...");
      const tools = await invoke<MCPToolInfo[]>("get_mcp_tools");
      console.log("MCP tools loaded:", tools);
      setMcpTools(tools);

      console.log("MCP loading completed successfully");
    } catch (error) {
      console.error("Error loading MCP servers:", error);
      console.error("Error details:", JSON.stringify(error, null, 2));
      toast.error(`Failed to load MCP servers: ${error}`);
    } finally {
      setMcpLoading(false);
    }
  };

  const handleSaveMcpServer = async () => {
    try {
      const parsedEnvVars = JSON.parse(mcpJsonData);
      const newServer = {
        id: `mcp-${Date.now()}`,
        name: parsedEnvVars.name || "Unnamed Server",
        description: parsedEnvVars.description || "",
        command: parsedEnvVars.command || "",
        args: parsedEnvVars.args || [],
        working_directory: parsedEnvVars.working_directory || "",
        environment_variables: parsedEnvVars.environment_variables || {},
        enabled: true,
        auto_start: parsedEnvVars.auto_start || false,
        timeout_seconds: parsedEnvVars.timeout_seconds || 30,
        max_retries: parsedEnvVars.max_retries || 3,
      };

      await invoke("add_mcp_server", { config: newServer });
      toast.success("MCP server added successfully");
      setMcpJsonData("");
      await loadMcpServers();
    } catch (error) {
      console.error("Error adding MCP server:", error);
      if (error instanceof SyntaxError) {
        toast.error("Invalid JSON format");
      } else {
        toast.error("Failed to add MCP server");
      }
    }
  };

  const getMcpServerStatusBadge = (status: MCPServerStatus) => {
    // Handle undefined/null status
    if (!status) {
      return <Badge variant="outline">Disconnected</Badge>;
    }

    if (status.Connected !== undefined) {
      return (
        <Badge variant="default" className="bg-green-500">
          Connected
        </Badge>
      );
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
    // Handle undefined/null status
    if (!status) {
      return <Square className="h-4 w-4 text-gray-400" />;
    }

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

  const handleAlwaysListeningToggle = async (enabled: boolean) => {
    try {
      if (enabled) {
        await invoke("start_always_listening_mode");
      } else {
        await invoke("stop_always_listening_mode");
      }
      setAlwaysListeningActive(enabled);
      toast.success(`Always listening ${enabled ? "enabled" : "disabled"}`);
    } catch (error) {
      console.error("Failed to toggle always listening:", error);
      toast.error("Failed to toggle always listening");
    }
  };

  const handleSensitivityChange = async (sensitivity: number) => {
    try {
      await invoke("set_always_listening_sensitivity", { sensitivity });
      setAlwaysListeningSensitivity(sensitivity);
      toast.success("Sensitivity updated successfully");
    } catch (error) {
      console.error("Failed to update sensitivity:", error);
      toast.error("Failed to update sensitivity");
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
      console.error("Failed to update wake words:", error);
      toast.error("Failed to update wake words");
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
        <h1 className="text-2xl font-bold">OLD Settings</h1>
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

          <div className="space-y-4 pt-4 border-t">
            <div className="space-y-2">
              <Label htmlFor="always-listening">Always Listening Mode</Label>
              <div className="flex items-center gap-3">
                <Button
                  variant={alwaysListeningActive ? "default" : "outline"}
                  size="sm"
                  onClick={() =>
                    handleAlwaysListeningToggle(!alwaysListeningActive)
                  }
                  className="min-w-[80px]"
                >
                  {alwaysListeningActive ? "Active" : "Inactive"}
                </Button>
                <span className="text-sm text-muted-foreground">
                  Continuously monitor for wake words
                </span>
              </div>
              <p className="text-sm text-muted-foreground">
                When enabled, Juno will continuously listen for wake words like
                "hey juno" or "computer" to activate the AI assistant.
              </p>
            </div>

            {alwaysListeningActive && (
              <>
                <div className="space-y-2">
                  <Label htmlFor="sensitivity">
                    Sensitivity ({alwaysListeningSensitivity.toFixed(1)})
                  </Label>
                  <div className="flex items-center gap-3">
                    <input
                      type="range"
                      min="0.1"
                      max="2.0"
                      step="0.1"
                      value={alwaysListeningSensitivity}
                      onChange={(e) =>
                        handleSensitivityChange(parseFloat(e.target.value))
                      }
                      className="flex-1"
                    />
                    <span className="text-sm text-muted-foreground min-w-[100px]">
                      {alwaysListeningSensitivity < 0.5
                        ? "Very Sensitive"
                        : alwaysListeningSensitivity < 1.0
                        ? "Normal"
                        : alwaysListeningSensitivity < 1.5
                        ? "Less Sensitive"
                        : "Very Low"}
                    </span>
                  </div>
                  <p className="text-sm text-muted-foreground">
                    Lower values make the system more sensitive to quiet sounds.
                    Higher values require louder speech.
                  </p>
                </div>

                <div className="space-y-2">
                  <Label htmlFor="wake-words">Wake Words</Label>
                  <div className="flex items-center gap-2">
                    <Input
                      id="wake-words"
                      value={wakeWordsInput}
                      onChange={(e) => setWakeWordsInput(e.target.value)}
                      placeholder="hey juno, computer"
                      className="flex-1"
                    />
                    <Button
                      size="sm"
                      onClick={handleWakeWordsChange}
                      variant="outline"
                    >
                      <Save size={16} />
                    </Button>
                  </div>
                  <p className="text-sm text-muted-foreground">
                    Comma-separated list of phrases that will activate the
                    assistant. Current: {alwaysListeningWakeWords.join(", ")}
                  </p>
                </div>
              </>
            )}
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
                    <div className="flex items-center gap-2">
                      <span>{provider.name}</span>
                      {provider.computer_use_supported && (
                        <Badge
                          variant="secondary"
                          className="text-xs bg-blue-100 text-blue-800"
                        >
                          Computer Use
                        </Badge>
                      )}
                    </div>
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
            {currentProvider && (
              <div className="space-y-2">
                <p className="text-sm text-muted-foreground">
                  {currentProvider.description}
                </p>
                {currentProvider.computer_use_supported && (
                  <div className="flex items-center gap-2 text-sm">
                    <CheckCircle className="h-4 w-4 text-green-600" />
                    <span className="text-green-700">
                      Computer use capabilities available
                    </span>
                  </div>
                )}
              </div>
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
                  <Label htmlFor="model">
                    Model
                    {currentProvider?.computer_use_supported && (
                      <span className="text-xs text-muted-foreground ml-2">
                        (🖥️ = Computer Use)
                      </span>
                    )}
                  </Label>
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
                      {currentProvider?.model_info ? (
                        <>
                          {/* Computer Use Models */}
                          {currentProvider.model_info.filter(
                            (model) => model.supports_computer_use
                          ).length > 0 && (
                            <>
                              <div className="px-2 py-1 text-xs font-medium text-muted-foreground bg-blue-50 border-b">
                                Computer Use Models
                              </div>
                              {currentProvider.model_info
                                .filter((model) => model.supports_computer_use)
                                .map((model) => (
                                  <SelectItem key={model.id} value={model.id}>
                                    <div className="flex items-center gap-2">
                                      <span>🖥️</span>
                                      <span>{model.name}</span>
                                      {model.is_recommended && (
                                        <Badge
                                          variant="outline"
                                          className="text-xs bg-green-50 text-green-700 border-green-200"
                                        >
                                          Recommended
                                        </Badge>
                                      )}
                                    </div>
                                  </SelectItem>
                                ))}
                            </>
                          )}

                          {/* General Chat Models */}
                          {currentProvider.model_info.filter(
                            (model) => !model.supports_computer_use
                          ).length > 0 && (
                            <>
                              <div className="px-2 py-1 text-xs font-medium text-muted-foreground bg-gray-50 border-b">
                                General Chat Models
                              </div>
                              {currentProvider.model_info
                                .filter((model) => !model.supports_computer_use)
                                .map((model) => (
                                  <SelectItem key={model.id} value={model.id}>
                                    <div className="flex items-center gap-2">
                                      <span>💬</span>
                                      <span>{model.name}</span>
                                      {model.is_recommended && (
                                        <Badge
                                          variant="outline"
                                          className="text-xs bg-green-50 text-green-700 border-green-200"
                                        >
                                          Recommended
                                        </Badge>
                                      )}
                                    </div>
                                  </SelectItem>
                                ))}
                            </>
                          )}
                        </>
                      ) : (
                        // Fallback to old model list format
                        currentProvider?.models?.map((model) => (
                          <SelectItem key={model} value={model}>
                            {model}
                          </SelectItem>
                        )) || (
                          <SelectItem value="" disabled>
                            No models available
                          </SelectItem>
                        )
                      )}
                    </SelectContent>
                  </Select>
                  {formData.model && currentProvider?.model_info && (
                    <div className="text-xs text-muted-foreground">
                      {(() => {
                        const selectedModel = currentProvider.model_info.find(
                          (m) => m.id === formData.model
                        );
                        if (selectedModel?.supports_computer_use) {
                          return "✅ This model supports computer use automation";
                        } else {
                          return "⚠️ This model is for general chat only";
                        }
                      })()}
                    </div>
                  )}
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
                    <ShortcutInput
                      key={key}
                      label={getShortcutDisplayName(key)}
                      description={getShortcutDescription(key)}
                      value={value}
                      shortcutName={key}
                      isSystemManaged={key === "open_settings"}
                      onSave={handleShortcutChange}
                      isLoading={shortcutsLoading}
                    />
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
            <div className="space-y-3">
              <ShortcutInput
                label="Cancel Current Operation"
                description="Stop any running AI task or operation"
                value="Escape"
                shortcutName="stop_current_task"
                isSystemManaged={true}
                onSave={handleShortcutChange}
                isLoading={shortcutsLoading}
              />
              <ShortcutInput
                label="Open Settings"
                description="Open the settings menu"
                value={keyboardShortcuts.open_settings || "⌘+,"}
                shortcutName="open_settings"
                isSystemManaged={true}
                onSave={handleShortcutChange}
                isLoading={shortcutsLoading}
              />
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

      {/* MCP Server Settings - Simplified JSON Only */}
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Server size={20} />
            MCP Servers
          </CardTitle>
          <CardDescription>
            Configure Model Context Protocol servers using JSON format
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="space-y-2">
            <Label htmlFor="mcp-json-config">Server Configuration (JSON)</Label>
            <Textarea
              id="mcp-json-config"
              value={mcpJsonData}
              onChange={(e) => setMcpJsonData(e.target.value)}
              placeholder={`{
  "filesystem": {
    "command": "npx",
    "args": ["@modelcontextprotocol/server-filesystem", "/Users/username/Documents"],
    "env": {}
  },
  "everything": {
    "command": "npx",
    "args": ["@modelcontextprotocol/server-everything"],
    "env": {}
  }
}`}
              className="h-64 font-mono text-sm"
            />
            <div className="text-xs text-muted-foreground space-y-1">
              <p>
                • Server name is the JSON key (e.g., "filesystem", "everything")
              </p>
              <p>
                • Required field: <code>command</code>
              </p>
              <p>
                • Optional: <code>args</code> (array) and <code>env</code>{" "}
                (object)
              </p>
            </div>
          </div>

          <div className="flex gap-2">
            <Button
              onClick={handleSaveMcpServer}
              disabled={isLoading}
              className="flex items-center gap-2"
            >
              <Save size={16} />
              Save Configuration
            </Button>
            <Button
              variant="outline"
              onClick={loadMcpServers}
              disabled={mcpLoading}
              className="flex items-center gap-2"
            >
              <RefreshCw
                className={`h-4 w-4 ${mcpLoading ? "animate-spin" : ""}`}
              />
              Refresh
            </Button>
          </div>

          {mcpServers.length > 0 && (
            <div className="space-y-2 pt-4 border-t">
              <h3 className="text-sm font-medium">Active Servers:</h3>
              <div className="grid gap-2">
                {mcpServers.map((server) => {
                  const status = mcpServerStatuses[server.id] || {
                    Disconnected: null,
                  };
                  const hasError = status.Error !== undefined;
                  const serverTools = mcpTools.filter(
                    (tool) => tool.server_id === server.id
                  );

                  return (
                    <div
                      key={server.id}
                      className="flex items-center justify-between p-2 border rounded"
                    >
                      <div className="flex items-center gap-2">
                        {getMcpServerStatusIcon(status)}
                        <span className="font-medium text-sm">
                          {server.name}
                        </span>
                        <Badge variant="outline" className="text-xs">
                          {serverTools.length} tools
                        </Badge>
                      </div>
                      <div className="flex items-center gap-2">
                        {getMcpServerStatusBadge(status)}
                        {hasError && (
                          <Button
                            variant="ghost"
                            size="sm"
                            onClick={() => alert(`Error: ${status.Error}`)}
                            className="text-red-600"
                          >
                            <AlertCircle className="h-4 w-4" />
                          </Button>
                        )}
                      </div>
                    </div>
                  );
                })}
              </div>
            </div>
          )}

          <div className="pt-4 border-t text-sm text-muted-foreground">
            <div className="space-y-2">
              <p className="font-medium">Common MCP Servers:</p>
              <div className="space-y-1 text-xs font-mono bg-muted/50 p-3 rounded">
                <div>
                  <strong>File System:</strong> npx
                  @modelcontextprotocol/server-filesystem /path
                </div>
                <div>
                  <strong>Everything Server:</strong> npx
                  @modelcontextprotocol/server-everything
                </div>
                <div>
                  <strong>Memory:</strong> npx
                  @modelcontextprotocol/server-memory
                </div>
                <div>
                  <strong>Sequential Thinking:</strong> npx
                  @modelcontextprotocol/server-sequential-thinking
                </div>
              </div>
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
