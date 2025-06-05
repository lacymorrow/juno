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
import { invoke } from "@tauri-apps/api/core";
import {
  AlertCircle,
  Brain,
  CheckCircle,
  Mic,
  MonitorSpeaker,
  Network,
  RefreshCw,
  Save,
  Settings as SettingsIcon,
  Shield,
  Terminal,
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
  const [spacebarClipboardEnabled, setSpacebarClipboardEnabled] =
    useState<boolean>(true);

  // Sound Settings
  const [soundEnabled, setSoundEnabled] = useState<boolean>(true);

  // Tool Configuration Settings
  const [toolConfigurations, setToolConfigurations] = useState<
    Record<string, ToolCategory>
  >({});
  const [toolConfigLoading, setToolConfigLoading] = useState<boolean>(false);

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
        "get_spacebar_clipboard_enabled"
      );
      setSpacebarClipboardEnabled(currentClipboardEnabled);

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
    } catch (error) {
      console.error("Failed to load settings:", error);
      toast.error("Failed to load settings");
    } finally {
      setIsLoading(false);
    }
  };

  const loadPermissionsStatus = async () => {
    try {
      setPermissionsLoading(true);
      const result = await invoke<{
        accessibility: { granted: boolean; required: boolean };
        screenRecording: { granted: boolean; required: boolean };
        microphone: { granted: boolean; required: boolean };
        allGranted: boolean;
        appName: string;
      }>("check_permissions_status");
      setPermissionsState(result);
    } catch (error) {
      console.error("Failed to load permissions status:", error);
    } finally {
      setPermissionsLoading(false);
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

  const handleSpacebarClipboardChange = async (enabled: boolean) => {
    try {
      await invoke("set_spacebar_clipboard_enabled", { enabled });
      setSpacebarClipboardEnabled(enabled);
      toast.success(
        `Spacebar dictation clipboard saving ${
          enabled ? "enabled" : "disabled"
        }`
      );
    } catch (error) {
      console.error("Failed to set spacebar clipboard setting:", error);
      toast.error("Failed to set spacebar clipboard setting");
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
              agent dictation or hold Spacebar for direct voice typing.
            </p>
          </div>

          <div className="space-y-2">
            <Label htmlFor="spacebar-clipboard">
              Spacebar Dictation Clipboard
            </Label>
            <div className="flex items-center gap-3">
              <Button
                variant={spacebarClipboardEnabled ? "default" : "outline"}
                size="sm"
                onClick={() =>
                  handleSpacebarClipboardChange(!spacebarClipboardEnabled)
                }
                className="min-w-[80px]"
              >
                {spacebarClipboardEnabled ? "Enabled" : "Disabled"}
              </Button>
              <span className="text-sm text-muted-foreground">
                Save transcribed text to clipboard when using spacebar dictation
              </span>
            </div>
            <p className="text-sm text-muted-foreground">
              When enabled, text transcribed via spacebar dictation (hold
              Spacebar) will be saved to the system clipboard in addition to
              being typed directly.
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

      {/* Keyboard Shortcuts Info */}
      <Card>
        <CardHeader>
          <CardTitle>Keyboard Shortcuts</CardTitle>
          <CardDescription>
            Essential keyboard shortcuts for using Juno
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          {/* Voice Input Shortcuts */}
          <div className="space-y-3">
            <h4 className="text-sm font-medium text-muted-foreground uppercase tracking-wide">
              Voice Input
            </h4>
            <div className="space-y-2">
              <div className="flex justify-between items-start">
                <div>
                  <span className="font-medium">AI Agent Dictation</span>
                  <p className="text-xs text-muted-foreground">
                    Send voice commands to AI agent
                  </p>
                </div>
                <kbd className="px-2 py-1 bg-muted rounded text-sm">Alt+D</kbd>
              </div>
              <div className="flex justify-between items-start">
                <div>
                  <span className="font-medium">Direct Voice Typing</span>
                  <p className="text-xs text-muted-foreground">
                    Hold to type speech directly (no AI processing)
                  </p>
                </div>
                <kbd className="px-2 py-1 bg-muted rounded text-sm">Space</kbd>
              </div>
            </div>
          </div>

          {/* General Shortcuts */}
          <div className="space-y-3">
            <h4 className="text-sm font-medium text-muted-foreground uppercase tracking-wide">
              General
            </h4>
            <div className="space-y-2">
              <div className="flex justify-between items-center">
                <span>Stop Current Task</span>
                <kbd className="px-2 py-1 bg-muted rounded text-sm">Escape</kbd>
              </div>
              <div className="flex justify-between items-center">
                <span>Settings</span>
                <kbd className="px-2 py-1 bg-muted rounded text-sm">Cmd+,</kbd>
              </div>
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
