import { useSettings } from "@/hooks/useSettings";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
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
  Settings,
  Shield,
  Square,
  Terminal,
  Lock,
  Zap,
  Info,
} from "lucide-react";
import React, { useEffect, useState } from "react";
import { toast } from "sonner";

// Import UI components
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
import { Separator } from "@/components/ui/separator";

interface SettingsCategory {
  id: string;
  name: string;
  icon: React.ReactNode;
  description: string;
}

const settingsCategories: SettingsCategory[] = [
  {
    id: "general",
    name: "General",
    icon: <Settings className="w-8 h-8" />,
    description: "Basic app settings and preferences",
  },
  {
    id: "voice",
    name: "Voice & Audio",
    icon: <Mic className="w-8 h-8" />,
    description: "Voice transcription and audio settings",
  },
  {
    id: "ai",
    name: "AI Provider",
    icon: <Brain className="w-8 h-8" />,
    description: "Configure AI models and providers",
  },
  {
    id: "network",
    name: "Network",
    icon: <Network className="w-8 h-8" />,
    description: "MCP servers and network configuration",
  },
  {
    id: "security",
    name: "Security & Privacy",
    icon: <Shield className="w-8 h-8" />,
    description: "Permissions and security settings",
  },
  {
    id: "shortcuts",
    name: "Keyboard Shortcuts",
    icon: <Keyboard className="w-8 h-8" />,
    description: "Customize keyboard shortcuts",
  },
  {
    id: "tools",
    name: "Tools",
    icon: <MonitorSpeaker className="w-8 h-8" />,
    description: "Configure available tools and features",
  },
  {
    id: "advanced",
    name: "Advanced",
    icon: <Terminal className="w-8 h-8" />,
    description: "Advanced settings and developer options",
  },
];

export default function SettingsWindow() {
  const [selectedCategory, setSelectedCategory] = useState("general");
  const settings = useSettings();
  const window = getCurrentWindow();

  useEffect(() => {
    // Set up the window properly for macOS
    const setupWindow = async () => {
      try {
        await window.setTitle("Juno Settings");
        // On macOS, we can set additional styling
        if (window.label === "settings") {
          console.log("Settings window initialized");
        }
      } catch (error) {
        console.error("Failed to setup settings window:", error);
      }
    };

    setupWindow();
  }, [window]);

  const handleCloseWindow = async () => {
    try {
      await invoke("close_settings_window");
    } catch (error) {
      console.error("Failed to close settings window:", error);
    }
  };

  return (
    <div className="flex h-screen bg-gray-50">
      {/* Sidebar with categories - macOS style */}
      <div className="w-64 bg-white border-r border-gray-200 flex flex-col">
        <div className="p-6 border-b border-gray-200">
          <h1 className="text-xl font-semibold text-gray-900">Settings</h1>
        </div>

        <div className="flex-1 overflow-y-auto p-4">
          <div className="space-y-1">
            {settingsCategories.map((category) => (
              <button
                key={category.id}
                onClick={() => setSelectedCategory(category.id)}
                className={`w-full flex items-center gap-3 px-3 py-3 rounded-lg text-left transition-colors ${
                  selectedCategory === category.id
                    ? "bg-blue-100 text-blue-700"
                    : "text-gray-700 hover:bg-gray-100"
                }`}
              >
                <div
                  className={`${
                    selectedCategory === category.id
                      ? "text-blue-600"
                      : "text-gray-500"
                  }`}
                >
                  {category.icon}
                </div>
                <div className="min-w-0 flex-1">
                  <div className="font-medium text-sm">{category.name}</div>
                  <div className="text-xs text-gray-500 mt-0.5 leading-tight">
                    {category.description}
                  </div>
                </div>
              </button>
            ))}
          </div>
        </div>

        {/* Footer with close button */}
        <div className="p-4 border-t border-gray-200">
          <button
            onClick={handleCloseWindow}
            className="w-full px-4 py-2 bg-gray-100 hover:bg-gray-200 rounded-lg text-sm font-medium text-gray-700 transition-colors"
          >
            Close Settings
          </button>
        </div>
      </div>

      {/* Main content area */}
      <div className="flex-1 flex flex-col">
        {/* Title bar area - transparent to work with macOS titlebar */}
        <div className="h-12 flex items-center justify-between px-6 bg-transparent">
          <div className="flex items-center gap-3">
            <div className="text-gray-500">
              {settingsCategories.find((c) => c.id === selectedCategory)?.icon}
            </div>
            <h2 className="text-lg font-semibold text-gray-900">
              {settingsCategories.find((c) => c.id === selectedCategory)?.name}
            </h2>
          </div>
        </div>

        {/* Settings content */}
        <div className="flex-1 overflow-y-auto p-6">
          <SettingsContent category={selectedCategory} settings={settings} />
        </div>
      </div>
    </div>
  );
}

function SettingsContent({
  category,
  settings,
}: {
  category: string;
  settings: ReturnType<typeof useSettings>;
}) {
  const renderCategoryContent = () => {
    switch (category) {
      case "general":
        return <GeneralSettings settings={settings} />;
      case "voice":
        return <VoiceSettings settings={settings} />;
      case "ai":
        return <AIProviderSettings settings={settings} />;
      case "network":
        return <NetworkSettings settings={settings} />;
      case "security":
        return <SecuritySettings settings={settings} />;
      case "shortcuts":
        return <ShortcutsSettings settings={settings} />;
      case "tools":
        return <ToolsSettings settings={settings} />;
      case "advanced":
        return <AdvancedSettings settings={settings} />;
      default:
        return <GeneralSettings settings={settings} />;
    }
  };

  return <div className="max-w-2xl">{renderCategoryContent()}</div>;
}

function GeneralSettings({
  settings,
}: {
  settings: ReturnType<typeof useSettings>;
}) {
  return (
    <div className="space-y-6">
      <div>
        <h3 className="text-lg font-medium text-gray-900 mb-4">
          General Settings
        </h3>

        <Card>
          <CardHeader>
            <CardTitle>Sound Effects</CardTitle>
            <CardDescription>
              Configure audio feedback and notifications
            </CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="flex items-center justify-between">
              <div>
                <Label htmlFor="sound-enabled" className="text-sm font-medium">
                  Enable Sound Effects
                </Label>
                <p className="text-xs text-gray-500">
                  Play sounds for notifications and feedback
                </p>
              </div>
              <Switch
                id="sound-enabled"
                checked={settings.soundEnabled}
                onCheckedChange={settings.handleSoundEnabledChange}
              />
            </div>
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle>Agent Mode</CardTitle>
            <CardDescription>
              Choose how Juno handles tasks and AI interactions
            </CardDescription>
          </CardHeader>
          <CardContent>
            <div className="space-y-2">
              <Label htmlFor="agent-mode">Agent Mode</Label>
              <Select
                value={settings.agentMode}
                onValueChange={settings.handleAgentModeChange}
              >
                <SelectTrigger>
                  <SelectValue placeholder="Select agent mode" />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="multi">
                    Multi-Agent (Recommended)
                  </SelectItem>
                  <SelectItem value="single">Single Agent</SelectItem>
                </SelectContent>
              </Select>
              <p className="text-xs text-gray-500">
                Multi-agent mode uses specialized agents for different tasks,
                while single agent mode uses one agent for everything.
              </p>
            </div>
          </CardContent>
        </Card>
      </div>
    </div>
  );
}

function VoiceSettings({
  settings,
}: {
  settings: ReturnType<typeof useSettings>;
}) {
  return (
    <div className="space-y-6">
      <div>
        <h3 className="text-lg font-medium text-gray-900 mb-4">
          Voice & Audio
        </h3>

        <Card>
          <CardHeader>
            <CardTitle>Text-to-Speech</CardTitle>
            <CardDescription>Configure voice output settings</CardDescription>
          </CardHeader>
          <CardContent>
            <div className="space-y-2">
              <Label htmlFor="tts-provider">TTS Provider</Label>
              <Select
                value={settings.ttsProvider}
                onValueChange={settings.handleTtsProviderChange}
              >
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
            </div>
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle>Dictation Settings</CardTitle>
            <CardDescription>
              Configure voice input and transcription
            </CardDescription>
          </CardHeader>
          <CardContent>
            <div className="flex items-center justify-between">
              <div>
                <Label htmlFor="dictation-clipboard">
                  Enable Clipboard Integration
                </Label>
                <p className="text-xs text-gray-500">
                  Automatically copy dictated text to clipboard
                </p>
              </div>
              <Switch
                id="dictation-clipboard"
                checked={settings.dictationClipboardEnabled}
                onCheckedChange={settings.handleDictationClipboardChange}
              />
            </div>
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle>Always Listening</CardTitle>
            <CardDescription>
              Configure wake word detection for hands-free activation
            </CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="flex items-center justify-between">
              <div>
                <Label htmlFor="always-listening">
                  Enable Always Listening
                </Label>
                <p className="text-xs text-gray-500">
                  Listen for wake words to activate Juno
                </p>
              </div>
              <Switch
                id="always-listening"
                checked={settings.alwaysListeningActive}
                onCheckedChange={settings.handleAlwaysListeningToggle}
              />
            </div>

            {settings.alwaysListeningActive && (
              <>
                <div className="space-y-2">
                  <Label htmlFor="sensitivity">
                    Sensitivity:{" "}
                    {(settings.alwaysListeningSensitivity * 100).toFixed(0)}%
                  </Label>
                  <input
                    type="range"
                    id="sensitivity"
                    min="0"
                    max="1"
                    step="0.1"
                    value={settings.alwaysListeningSensitivity}
                    onChange={(e) =>
                      settings.handleSensitivityChange(
                        parseFloat(e.target.value)
                      )
                    }
                    className="w-full h-2 bg-gray-200 rounded-lg appearance-none cursor-pointer"
                  />
                </div>

                <div className="space-y-2">
                  <Label htmlFor="wake-words">Wake Words</Label>
                  <div className="flex gap-2">
                    <Input
                      id="wake-words"
                      value={settings.wakeWordsInput}
                      onChange={(e) =>
                        settings.setWakeWordsInput(e.target.value)
                      }
                      placeholder="hey juno, computer"
                      className="flex-1"
                    />
                    <Button onClick={settings.handleWakeWordsChange} size="sm">
                      <Save className="w-4 h-4" />
                    </Button>
                  </div>
                  <p className="text-xs text-gray-500">
                    Separate multiple wake words with commas
                  </p>
                </div>
              </>
            )}
          </CardContent>
        </Card>
      </div>
    </div>
  );
}

function AIProviderSettings({
  settings,
}: {
  settings: ReturnType<typeof useSettings>;
}) {
  return (
    <div className="space-y-6">
      <div>
        <h3 className="text-lg font-medium text-gray-900 mb-4">AI Provider</h3>

        <Card>
          <CardHeader>
            <CardTitle>Provider Selection</CardTitle>
            <CardDescription>Choose your AI provider and model</CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="space-y-2">
              <Label htmlFor="ai-provider">Active Provider</Label>
              <Select
                value={settings.activeProvider}
                onValueChange={settings.handleActiveProviderChange}
              >
                <SelectTrigger>
                  <SelectValue placeholder="Select AI provider" />
                </SelectTrigger>
                <SelectContent>
                  {settings.providers.map((provider) => (
                    <SelectItem key={provider.id} value={provider.id}>
                      <div className="flex items-center gap-2">
                        <span>{provider.name}</span>
                        {provider.computer_use_supported && (
                          <Badge variant="secondary" className="text-xs bg-blue-100 text-blue-800">
                            Computer Use
                          </Badge>
                        )}
                      </div>
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
              {settings.providers.find(p => p.id === settings.activeProvider) && (
                <div className="space-y-2">
                  <p className="text-sm text-muted-foreground">
                    {settings.providers.find(p => p.id === settings.activeProvider)?.description}
                  </p>
                  {settings.providers.find(p => p.id === settings.activeProvider)?.computer_use_supported && (
                    <div className="flex items-center gap-2 text-sm">
                      <CheckCircle className="h-4 w-4 text-green-600" />
                      <span className="text-green-700">Computer use capabilities available</span>
                    </div>
                  )}
                </div>
              )}
            </div>
          </CardContent>
        </Card>

        {settings.activeProvider && settings.providerSettings && (
          <Card>
            <CardHeader>
              <CardTitle>Provider Configuration</CardTitle>
              <CardDescription>
                Configure settings for {settings.activeProvider}
              </CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
              <div className="space-y-2">
                <Label htmlFor="api-key">API Key</Label>
                <Input
                  id="api-key"
                  type="password"
                  value={settings.formData.apiKey}
                  onChange={(e) =>
                    settings.setFormData((prev) => ({
                      ...prev,
                      apiKey: e.target.value,
                    }))
                  }
                  placeholder="Enter your API key"
                />
              </div>

              <div className="space-y-2">
                <Label htmlFor="model">
                  Model
                  {settings.providers.find(p => p.id === settings.activeProvider)?.computer_use_supported && (
                    <span className="text-xs text-gray-500 ml-2">
                      (🖥️ = Computer Use)
                    </span>
                  )}
                </Label>
                <Select
                  value={settings.formData.model}
                  onValueChange={(value) =>
                    settings.setFormData((prev) => ({ ...prev, model: value }))
                  }
                >
                  <SelectTrigger>
                    <SelectValue placeholder="Select model" />
                  </SelectTrigger>
                  <SelectContent>
                    {(() => {
                      const currentProvider = settings.providers.find(p => p.id === settings.activeProvider);
                      
                      if (currentProvider?.model_info) {
                        return (
                          <>
                            {/* Computer Use Models */}
                            {currentProvider.model_info.filter(model => model.supports_computer_use).length > 0 && (
                              <>
                                <div className="px-2 py-1 text-xs font-medium text-gray-500 bg-blue-50 border-b">
                                  Computer Use Models
                                </div>
                                {currentProvider.model_info
                                  .filter(model => model.supports_computer_use)
                                  .map((model) => (
                                    <SelectItem key={model.id} value={model.id}>
                                      <div className="flex items-center gap-2">
                                        <span>🖥️</span>
                                        <span>{model.name}</span>
                                        {model.is_recommended && (
                                          <Badge variant="outline" className="text-xs bg-green-50 text-green-700 border-green-200">
                                            Recommended
                                          </Badge>
                                        )}
                                      </div>
                                    </SelectItem>
                                  ))}
                              </>
                            )}
                            
                            {/* General Chat Models */}
                            {currentProvider.model_info.filter(model => !model.supports_computer_use).length > 0 && (
                              <>
                                <div className="px-2 py-1 text-xs font-medium text-gray-500 bg-gray-50 border-b">
                                  General Chat Models
                                </div>
                                {currentProvider.model_info
                                  .filter(model => !model.supports_computer_use)
                                  .map((model) => (
                                    <SelectItem key={model.id} value={model.id}>
                                      <div className="flex items-center gap-2">
                                        <span>💬</span>
                                        <span>{model.name}</span>
                                        {model.is_recommended && (
                                          <Badge variant="outline" className="text-xs bg-green-50 text-green-700 border-green-200">
                                            Recommended
                                          </Badge>
                                        )}
                                      </div>
                                    </SelectItem>
                                  ))}
                              </>
                            )}
                          </>
                        );
                      } else {
                        // Fallback to old format
                        return currentProvider?.models?.map((model) => (
                          <SelectItem key={model} value={model}>
                            {model}
                          </SelectItem>
                        ));
                      }
                    })()}
                  </SelectContent>
                </Select>
                {(() => {
                  const currentProvider = settings.providers.find(p => p.id === settings.activeProvider);
                  if (settings.formData.model && currentProvider?.model_info) {
                    const selectedModel = currentProvider.model_info.find(m => m.id === settings.formData.model);
                    if (selectedModel?.supports_computer_use) {
                      return (
                        <div className="text-xs text-gray-500">
                          ✅ This model supports computer use automation
                        </div>
                      );
                    } else if (selectedModel) {
                      return (
                        <div className="text-xs text-gray-500">
                          ⚠️ This model is for general chat only
                        </div>
                      );
                    }
                  }
                  return null;
                })()}
              </div>

              <div className="grid grid-cols-2 gap-4">
                <div className="space-y-2">
                  <Label htmlFor="max-tokens">Max Tokens</Label>
                  <Input
                    id="max-tokens"
                    type="number"
                    value={settings.formData.maxTokens}
                    onChange={(e) =>
                      settings.setFormData((prev) => ({
                        ...prev,
                        maxTokens: e.target.value,
                      }))
                    }
                    placeholder="e.g., 4000"
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
                    value={settings.formData.temperature}
                    onChange={(e) =>
                      settings.setFormData((prev) => ({
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
                <Textarea
                  id="system-prompt"
                  value={settings.formData.systemPrompt}
                  onChange={(e) =>
                    settings.setFormData((prev) => ({
                      ...prev,
                      systemPrompt: e.target.value,
                    }))
                  }
                  placeholder="Enter custom system prompt (optional)"
                  rows={4}
                />
              </div>

              <Button
                onClick={settings.handleSaveProviderSettings}
                className="w-full"
              >
                <Save className="w-4 h-4 mr-2" />
                Save Provider Settings
              </Button>
            </CardContent>
          </Card>
        )}
      </div>
    </div>
  );
}

function NetworkSettings({
  settings,
}: {
  settings: ReturnType<typeof useSettings>;
}) {
  const [newServerJson, setNewServerJson] = useState("");

  const getMcpServerStatusBadge = (status: any) => {
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

  const getMcpServerStatusIcon = (status: any) => {
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

  const handleAddMcpServer = async () => {
    try {
      const parsedServer = JSON.parse(newServerJson);
      const newServer = {
        id: `mcp-${Date.now()}`,
        name: parsedServer.name || "Unnamed Server",
        description: parsedServer.description || "",
        command: parsedServer.command || "",
        args: parsedServer.args || [],
        working_directory: parsedServer.working_directory || "",
        environment_variables: parsedServer.environment_variables || {},
        enabled: true,
        auto_start: parsedServer.auto_start || false,
        timeout_seconds: parsedServer.timeout_seconds || 30,
        max_retries: parsedServer.max_retries || 3,
      };

      await invoke("add_mcp_server", { server: newServer });
      toast.success("MCP server added successfully");
      setNewServerJson("");
      await settings.loadMcpServers();
    } catch (error) {
      console.error("Error adding MCP server:", error);
      if (error instanceof SyntaxError) {
        toast.error("Invalid JSON format");
      } else {
        toast.error("Failed to add MCP server");
      }
    }
  };

  return (
    <div className="space-y-6">
      <div>
        <h3 className="text-lg font-medium text-gray-900 mb-4">Network</h3>

        <Card>
          <CardHeader>
            <CardTitle>MCP Servers</CardTitle>
            <CardDescription>
              Manage Model Context Protocol (MCP) servers for external tool
              integration
            </CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            {settings.mcpLoading ? (
              <div className="flex items-center justify-center py-8">
                <RefreshCw className="h-6 w-6 animate-spin" />
                <span className="ml-2">Loading MCP servers...</span>
              </div>
            ) : (
              <div className="space-y-3">
                {settings.mcpServers.map((server) => (
                  <div
                    key={server.id}
                    className="flex items-center justify-between p-3 border rounded-lg"
                  >
                    <div className="flex items-center gap-3">
                      {getMcpServerStatusIcon(
                        settings.mcpServerStatuses[server.id]
                      )}
                      <div>
                        <div className="font-medium">{server.name}</div>
                        <div className="text-sm text-gray-500">
                          {server.description || server.command}
                        </div>
                      </div>
                    </div>
                    <div className="flex items-center gap-2">
                      {getMcpServerStatusBadge(
                        settings.mcpServerStatuses[server.id]
                      )}
                      <Switch
                        checked={server.enabled}
                        onCheckedChange={async (enabled) => {
                          try {
                            await invoke("toggle_mcp_server", {
                              serverId: server.id,
                              enabled,
                            });
                            await settings.loadMcpServers();
                          } catch (error) {
                            toast.error("Failed to toggle server");
                          }
                        }}
                      />
                    </div>
                  </div>
                ))}

                {settings.mcpServers.length === 0 && (
                  <div className="text-center py-8 text-gray-500">
                    No MCP servers configured
                  </div>
                )}
              </div>
            )}
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle>Add MCP Server</CardTitle>
            <CardDescription>
              Add a new MCP server configuration in JSON format
            </CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="space-y-2">
              <Label htmlFor="server-config">Server Configuration (JSON)</Label>
              <Textarea
                id="server-config"
                value={newServerJson}
                onChange={(e) => setNewServerJson(e.target.value)}
                placeholder={JSON.stringify(
                  {
                    name: "Example Server",
                    description: "Description of the server",
                    command: "python",
                    args: ["-m", "example_server"],
                    working_directory: "/path/to/server",
                    environment_variables: {},
                    auto_start: true,
                    timeout_seconds: 30,
                    max_retries: 3,
                  },
                  null,
                  2
                )}
                rows={8}
                className="font-mono text-sm"
              />
            </div>
            <Button
              onClick={handleAddMcpServer}
              disabled={!newServerJson.trim()}
            >
              <Server className="w-4 h-4 mr-2" />
              Add Server
            </Button>
          </CardContent>
        </Card>

        {settings.mcpTools.length > 0 && (
          <Card>
            <CardHeader>
              <CardTitle>Available MCP Tools</CardTitle>
              <CardDescription>
                Tools provided by connected MCP servers
              </CardDescription>
            </CardHeader>
            <CardContent>
              <div className="space-y-2">
                {settings.mcpTools.map((tool) => (
                  <div
                    key={`${tool.server_id}-${tool.tool_definition.name}`}
                    className="flex items-center justify-between p-2 border rounded"
                  >
                    <div>
                      <div className="font-medium">
                        {tool.tool_definition.name}
                      </div>
                      <div className="text-sm text-gray-500">
                        from {tool.server_name}
                      </div>
                      <div className="text-xs text-gray-400">
                        {tool.tool_definition.description}
                      </div>
                    </div>
                    <Switch
                      checked={tool.enabled}
                      onCheckedChange={async (enabled) => {
                        try {
                          await invoke("toggle_mcp_tool", {
                            serverId: tool.server_id,
                            toolName: tool.tool_definition.name,
                            enabled,
                          });
                          await settings.loadMcpServers();
                        } catch (error) {
                          toast.error("Failed to toggle tool");
                        }
                      }}
                    />
                  </div>
                ))}
              </div>
            </CardContent>
          </Card>
        )}
      </div>
    </div>
  );
}

function SecuritySettings({
  settings,
}: {
  settings: ReturnType<typeof useSettings>;
}) {
  const [isRequestingPermission, setIsRequestingPermission] = useState<string | null>(null);
  const [autoRedirectEnabled] = useState(true);

  // Enhanced permission icons with priority indicators
  const getPermissionPriorityIcon = (permissionType: string) => {
    switch (permissionType) {
      case "accessibility":
        return <Lock className="h-5 w-5 text-blue-600" />;
      case "screen_recording":
        return <MonitorSpeaker className="h-5 w-5 text-purple-600" />;
      case "microphone":
        return <Mic className="h-5 w-5 text-green-600" />;
      case "input_monitoring":
        return <Keyboard className="h-5 w-5 text-orange-600" />;
      default:
        return <Shield className="h-5 w-5 text-gray-600" />;
    }
  };

  const getPermissionIcon = (granted: boolean, required: boolean) => {
    if (granted) {
      return <CheckCircle className="h-5 w-5 text-green-500" />;
    } else if (required) {
      return <AlertCircle className="h-5 w-5 text-red-500" />;
    } else {
      return <AlertCircle className="h-5 w-5 text-yellow-500" />;
    }
  };

  const getPermissionBadge = (granted: boolean, required: boolean) => {
    if (granted) {
      return (
        <Badge variant="outline" className="text-green-600 border-green-200 bg-green-50">
          ✓ Granted
        </Badge>
      );
    } else if (required) {
      return (
        <Badge variant="destructive" className="bg-red-100 text-red-700 border-red-300">
          ⚠ Required
        </Badge>
      );
    } else {
      return (
        <Badge variant="secondary" className="bg-yellow-100 text-yellow-700 border-yellow-300">
          💡 Optional
        </Badge>
      );
    }
  };

  // Enhanced accessibility permission request with auto-redirect
  const requestAccessibilityPermissionEnhanced = async (withAutoRedirect = true) => {
    try {
      setIsRequestingPermission("accessibility");

      const granted = withAutoRedirect && autoRedirectEnabled
        ? await invoke<boolean>("request_accessibility_permission_with_auto_redirect", { autoOpenSettings: true })
        : await invoke<boolean>("request_accessibility_permission");

      if (granted) {
        await settings.loadPermissionsStatus();
      } else if (!withAutoRedirect) {
        await openSystemPreferences("accessibility");
      }
    } catch (err) {
      console.error("Error requesting accessibility permission:", err);
      toast.error("Failed to request accessibility permission");
    } finally {
      setIsRequestingPermission(null);
    }
  };

  // Enhanced system preferences opening
  const openSystemPreferencesEnhanced = async (preferencePane: string) => {
    try {
      await invoke("open_system_settings_enhanced", { permissionType: preferencePane });
    } catch (err) {
      console.error("Error opening enhanced System Settings:", err);
      toast.error("Failed to open System Settings");
    }
  };

  const openSystemPreferences = async (preferencePane: string) => {
    try {
      await invoke("open_system_preferences", { preferencePane });
    } catch (err) {
      console.error("Error opening System Preferences:", err);
      toast.error("Failed to open System Settings");
    }
  };

  // Request other permissions with enhanced system settings navigation
  const requestScreenRecordingPermission = async () => {
    try {
      setIsRequestingPermission("screen_recording");
      const granted = await invoke<boolean>("request_screen_recording_permission");

      if (granted) {
        await settings.loadPermissionsStatus();
      } else {
        setTimeout(async () => {
          await settings.loadPermissionsStatus();
        }, 2000);
      }
    } catch (err) {
      console.error("Error requesting screen recording permission:", err);
      toast.error("Failed to request screen recording permission");
    } finally {
      setIsRequestingPermission(null);
    }
  };

  const requestMicrophonePermission = async () => {
    try {
      setIsRequestingPermission("microphone");
      const granted = await invoke<boolean>("request_microphone_permission");

      if (granted) {
        await settings.loadPermissionsStatus();
      } else {
        setTimeout(async () => {
          await settings.loadPermissionsStatus();
        }, 2000);
      }
    } catch (err) {
      console.error("Error requesting microphone permission:", err);
      toast.error("Failed to request microphone permission");
    } finally {
      setIsRequestingPermission(null);
    }
  };

  // Enhanced permission card renderer
  const renderPermissionCard = (permission: any, permissionType: string, onRequest?: () => void, onRequestEnhanced?: () => void) => {
    const isRequired = permission.required;
    const cardClassName = permission.granted
      ? "transition-colors border-green-200 bg-green-50/30"
      : isRequired
      ? "transition-colors border-red-200 bg-red-50/30"
      : "transition-colors border-yellow-200 bg-yellow-50/30";

    return (
      <Card key={permissionType} className={cardClassName}>
        <CardHeader className="pb-4">
          <div className="flex items-start justify-between">
            <div className="flex items-start space-x-3">
              <div className="flex items-center space-x-2">
                {getPermissionPriorityIcon(permissionType)}
                {getPermissionIcon(permission.granted, permission.required)}
              </div>
              <div className="flex-1">
                <div className="flex items-center space-x-2 mb-1">
                  <CardTitle className="text-lg">
                    {permissionType.replace("_", " ").replace(/\b\w/g, (l) => l.toUpperCase())} Access
                  </CardTitle>
                  {getPermissionBadge(permission.granted, permission.required)}
                </div>
                <CardDescription className="text-sm">
                  {permission.description || `${permissionType.replace("_", " ")} permission is required for Juno to function properly`}
                </CardDescription>
                {isRequired && !permission.granted && (
                  <div className="flex items-center space-x-1 mt-2">
                    <AlertCircle className="h-4 w-4 text-red-500" />
                    <span className="text-sm font-medium text-red-700">
                      This permission is required for Juno to function properly
                    </span>
                  </div>
                )}
                {!isRequired && !permission.granted && (
                  <div className="flex items-center space-x-1 mt-2">
                    <Info className="h-4 w-4 text-yellow-600" />
                    <span className="text-sm text-yellow-700">
                      Optional - enhances functionality when granted
                    </span>
                  </div>
                )}
              </div>
            </div>
          </div>
        </CardHeader>
        <CardContent>
          {!permission.granted && (
            <div className="space-y-3">
              <div className={`p-3 rounded-md border ${isRequired ? "bg-red-50 border-red-200" : "bg-yellow-50 border-yellow-200"}`}>
                <p className={`text-sm ${isRequired ? "text-red-800" : "text-yellow-800"}`}>
                  <strong>{isRequired ? "Action Required:" : "Optional Setup:"}</strong>{" "}
                  {permission.instructions || `Click 'Grant Permission' to enable ${permissionType.replace("_", " ")} access.`}
                </p>
              </div>
              <div className="flex flex-wrap gap-2">
                {/* Enhanced auto-redirect button for accessibility */}
                {onRequestEnhanced && autoRedirectEnabled && permissionType === "accessibility" && (
                  <Button
                    onClick={onRequestEnhanced}
                    disabled={isRequestingPermission === permissionType}
                    size="sm"
                    className="bg-blue-600 hover:bg-blue-700"
                  >
                    {isRequestingPermission === permissionType ? (
                      <>
                        <RefreshCw className="h-4 w-4 mr-2 animate-spin" />
                        Opening Settings...
                      </>
                    ) : (
                      <>
                        <Zap className="w-4 h-4 mr-2" />
                        Auto-Grant Permission
                      </>
                    )}
                  </Button>
                )}

                {/* Standard request button */}
                {onRequest && (
                  <Button
                    onClick={onRequest}
                    disabled={isRequestingPermission === permissionType}
                    size="sm"
                    variant={autoRedirectEnabled && permissionType === "accessibility" ? "outline" : isRequired ? "default" : "secondary"}
                  >
                    {isRequestingPermission === permissionType ? (
                      <>
                        <RefreshCw className="h-4 w-4 mr-2 animate-spin" />
                        Requesting...
                      </>
                    ) : (
                      <>
                        {isRequired ? "Grant Required Permission" : "Grant Optional Permission"}
                      </>
                    )}
                  </Button>
                )}

                {/* Manual settings button */}
                <Button
                  variant="outline"
                  size="sm"
                  onClick={() =>
                    autoRedirectEnabled
                      ? openSystemPreferencesEnhanced(permissionType)
                      : openSystemPreferences(permissionType)
                  }
                >
                  <Settings className="h-4 w-4 mr-2" />
                  Open System Settings
                  <ExternalLink className="h-4 w-4 ml-2" />
                </Button>
              </div>

              {/* Auto-redirect feature notice */}
              {autoRedirectEnabled && permissionType === "accessibility" && (
                <div className="mt-2 p-2 bg-blue-50 rounded-md border border-blue-200">
                  <p className="text-xs text-blue-700">
                    <Zap className="h-3 w-3 inline mr-1" />
                    Auto-redirect enabled: System Settings will open automatically when needed
                  </p>
                </div>
              )}
            </div>
          )}
          {permission.granted && (
            <div className="flex items-center space-x-2 p-3 bg-green-50 rounded-md border border-green-200">
              <CheckCircle className="h-4 w-4 text-green-600" />
              <p className="text-sm text-green-800 font-medium">
                Permission granted - {permissionType.replace("_", " ")} access is working properly
              </p>
            </div>
          )}
        </CardContent>
      </Card>
    );
  };

  return (
    <div className="space-y-6">
      <div>
        <h3 className="text-lg font-medium text-gray-900 mb-4">
          Security & Privacy
        </h3>

        {/* Enhanced permissions header */}
        <Card className="mb-6">
          <CardHeader>
            <CardTitle className="flex items-center space-x-2">
              {settings.permissionsState?.allGranted ? (
                <CheckCircle className="h-5 w-5 text-green-500" />
              ) : (
                <Shield className="h-5 w-5 text-blue-500" />
              )}
              <span>macOS Permissions Setup</span>
            </CardTitle>
            <CardDescription>
              {settings.permissionsState?.allGranted
                ? "✅ All permissions configured! Juno is ready for full functionality."
                : "Configure the permissions below to enable Juno's AI computer use capabilities. Required permissions are marked with ⚠️."}
            </CardDescription>
          </CardHeader>
          {!settings.permissionsState?.allGranted && (
            <CardContent>
              <div className="flex flex-wrap gap-2">
                <Button
                  onClick={settings.loadPermissionsStatus}
                  disabled={settings.permissionsLoading}
                  size="sm"
                  className="bg-green-600 hover:bg-green-700"
                >
                  <RefreshCw className={`h-4 w-4 mr-2 ${settings.permissionsLoading ? "animate-spin" : ""}`} />
                  Refresh Status
                </Button>
              </div>
            </CardContent>
          )}
        </Card>

        {/* Loading State */}
        {settings.permissionsLoading && (
          <Card>
            <CardHeader>
              <CardTitle className="flex items-center space-x-2">
                <RefreshCw className="h-5 w-5 animate-spin" />
                <span>Checking Permissions</span>
              </CardTitle>
              <CardDescription>
                Verifying macOS permissions for Juno with enhanced features...
              </CardDescription>
            </CardHeader>
          </Card>
        )}

        {/* Enhanced Permission Cards */}
        {!settings.permissionsLoading && settings.permissionsState && (
          <div className="space-y-6">
            {/* Required Permissions Section */}
            <div className="space-y-4">
              <div className="flex items-center space-x-2">
                <AlertCircle className="h-5 w-5 text-red-500" />
                <h3 className="text-lg font-semibold text-red-800">Required Permissions</h3>
                <Badge variant="destructive" className="bg-red-100 text-red-700">
                  Essential for Core Functionality
                </Badge>
              </div>
              <p className="text-sm text-gray-600 mb-4">
                These permissions are essential for Juno's core AI computer use functionality. 
                Without them, Juno cannot automate tasks or interact with your desktop.
              </p>

              <div className="space-y-3">
                {/* Accessibility Permission */}
                {settings.permissionsState.accessibility.required &&
                  renderPermissionCard(
                    settings.permissionsState.accessibility,
                    "accessibility",
                    () => requestAccessibilityPermissionEnhanced(false),
                    autoRedirectEnabled ? () => requestAccessibilityPermissionEnhanced(true) : undefined
                  )}

                {/* Screen Recording Permission */}
                {settings.permissionsState.screenRecording.required &&
                  renderPermissionCard(
                    settings.permissionsState.screenRecording,
                    "screen_recording",
                    requestScreenRecordingPermission
                  )}
              </div>
            </div>

            {/* Optional Permissions Section */}
            {settings.permissionsState.microphone && (
              <>
                <Separator />
                <div className="space-y-4">
                  <div className="flex items-center space-x-2">
                    <Info className="h-5 w-5 text-yellow-600" />
                    <h3 className="text-lg font-semibold text-yellow-800">Optional Permissions</h3>
                    <Badge variant="secondary" className="bg-yellow-100 text-yellow-700">
                      Enhances Experience
                    </Badge>
                  </div>
                  <p className="text-sm text-gray-600 mb-4">
                    These permissions enhance Juno's functionality but are not required for basic operation. 
                    You can grant them now or enable them later.
                  </p>

                  <div className="space-y-3">
                    {/* Microphone Permission */}
                    {settings.permissionsState.microphone && !settings.permissionsState.microphone.required &&
                      renderPermissionCard(
                        settings.permissionsState.microphone,
                        "microphone",
                        requestMicrophonePermission
                      )}
                  </div>
                </div>
              </>
            )}

            {/* Success State */}
            {settings.permissionsState.allGranted && (
              <Card className="border-green-200 bg-green-50">
                <CardContent className="pt-6">
                  <div className="flex items-center space-x-3">
                    <CheckCircle className="h-6 w-6 text-green-600" />
                    <div>
                      <span className="text-green-800 font-semibold text-lg">🎉 Setup Complete!</span>
                      <p className="text-green-700 text-sm mt-1">
                        All permissions configured. Juno is ready for AI computer use.
                      </p>
                    </div>
                  </div>
                </CardContent>
              </Card>
            )}
          </div>
        )}

        {/* Error State */}
        {!settings.permissionsLoading && !settings.permissionsState && (
          <Card className="border-red-200 bg-red-50">
            <CardContent className="pt-6">
              <div className="flex items-center space-x-3">
                <AlertCircle className="h-6 w-6 text-red-600" />
                <div>
                  <span className="text-red-800 font-semibold">Failed to load permissions status</span>
                  <p className="text-red-700 text-sm mt-1">
                    Unable to check current permission status. Please try refreshing.
                  </p>
                </div>
              </div>
            </CardContent>
          </Card>
        )}
      </div>
    </div>
  );
}

function ShortcutsSettings({
  settings,
}: {
  settings: ReturnType<typeof useSettings>;
}) {
  const getShortcutDisplayName = (shortcutName: string): string => {
    const names: { [key: string]: string } = {
      agent_mode_toggle: "Toggle Agent Mode",
      dictation_input: "Start Dictation",
      stop_current_task: "Stop Current Task",
      open_settings: "Open Settings",
    };
    return names[shortcutName] || shortcutName;
  };

  const getShortcutDescription = (shortcutName: string): string => {
    const descriptions: { [key: string]: string } = {
      agent_mode_toggle: "Switch between agent and normal mode",
      dictation_input: "Activate voice input for dictation",
      stop_current_task: "Stop the current AI task or operation",
      open_settings: "Open the settings window",
    };
    return descriptions[shortcutName] || "";
  };

  const handleShortcutChange = async (shortcutName: string, value: string) => {
    try {
      await invoke("set_keyboard_shortcut", { shortcutName, shortcut: value });
      await settings.loadKeyboardShortcuts();
      toast.success("Keyboard shortcut updated");
    } catch (error) {
      console.error("Failed to set keyboard shortcut:", error);
      toast.error("Failed to update keyboard shortcut");
    }
  };

  const handleResetShortcuts = async () => {
    try {
      await invoke("reset_keyboard_shortcuts");
      await settings.loadKeyboardShortcuts();
      toast.success("Keyboard shortcuts reset to defaults");
    } catch (error) {
      console.error("Failed to reset keyboard shortcuts:", error);
      toast.error("Failed to reset keyboard shortcuts");
    }
  };

  return (
    <div className="space-y-6">
      <div>
        <h3 className="text-lg font-medium text-gray-900 mb-4">
          Keyboard Shortcuts
        </h3>

        <Card>
          <CardHeader>
            <CardTitle>Global Shortcuts</CardTitle>
            <CardDescription>
              Configure keyboard shortcuts that work system-wide
            </CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            {settings.shortcutsLoading ? (
              <div className="flex items-center justify-center py-8">
                <RefreshCw className="h-6 w-6 animate-spin" />
                <span className="ml-2">Loading shortcuts...</span>
              </div>
            ) : (
              <div className="space-y-3">
                {Object.entries(settings.keyboardShortcuts).map(
                  ([shortcutName, shortcutValue]) => (
                    <div
                      key={shortcutName}
                      className="flex items-center justify-between p-3 border rounded-lg"
                    >
                      <div>
                        <div className="font-medium">
                          {getShortcutDisplayName(shortcutName)}
                        </div>
                        <div className="text-sm text-gray-500">
                          {getShortcutDescription(shortcutName)}
                        </div>
                      </div>
                      <div className="flex items-center gap-2">
                        {settings.editingShortcut === shortcutName ? (
                          <Input
                            value={shortcutValue}
                            onChange={(e) =>
                              handleShortcutChange(shortcutName, e.target.value)
                            }
                            onBlur={() => settings.setEditingShortcut(null)}
                            onKeyDown={(e) => {
                              if (e.key === "Enter") {
                                settings.setEditingShortcut(null);
                              } else if (e.key === "Escape") {
                                settings.setEditingShortcut(null);
                                settings.loadKeyboardShortcuts();
                              }
                            }}
                            placeholder="e.g., Cmd+Shift+J"
                            className="w-40 text-sm"
                            autoFocus
                          />
                        ) : (
                          <Button
                            variant="outline"
                            size="sm"
                            onClick={() =>
                              settings.setEditingShortcut(shortcutName)
                            }
                            className="w-40 justify-start font-mono text-sm"
                          >
                            {shortcutValue || "Not set"}
                          </Button>
                        )}
                      </div>
                    </div>
                  )
                )}
              </div>
            )}

            <div className="pt-4 border-t">
              <Button
                onClick={handleResetShortcuts}
                variant="outline"
                disabled={settings.shortcutsLoading}
                className="w-full"
              >
                <RotateCcw className="w-4 h-4 mr-2" />
                Reset to Defaults
              </Button>
            </div>
          </CardContent>
        </Card>
      </div>
    </div>
  );
}

function ToolsSettings({
  settings,
}: {
  settings: ReturnType<typeof useSettings>;
}) {
  const handleToggleCategory = async (
    categoryName: string,
    enabled: boolean
  ) => {
    try {
      await invoke("set_tool_category_enabled", { categoryName, enabled });
      await settings.loadToolConfigurations();
      toast.success(
        `${categoryName} tools ${enabled ? "enabled" : "disabled"}`
      );
    } catch (error) {
      console.error("Failed to toggle tool category:", error);
      toast.error("Failed to toggle tool category");
    }
  };

  const handleToggleTool = async (toolName: string, enabled: boolean) => {
    try {
      await invoke("set_tool_enabled", { toolName, enabled });
      await settings.loadToolConfigurations();
      toast.success(`${toolName} ${enabled ? "enabled" : "disabled"}`);
    } catch (error) {
      console.error("Failed to toggle tool:", error);
      toast.error("Failed to toggle tool");
    }
  };

  const handleResetToolConfiguration = async () => {
    try {
      await invoke("reset_tool_configuration");
      await settings.loadToolConfigurations();
      toast.success("Tool configuration reset to defaults");
    } catch (error) {
      console.error("Failed to reset tool configuration:", error);
      toast.error("Failed to reset tool configuration");
    }
  };

  return (
    <div className="space-y-6">
      <div>
        <h3 className="text-lg font-medium text-gray-900 mb-4">Tools</h3>

        <Card>
          <CardHeader>
            <CardTitle>Tool Categories</CardTitle>
            <CardDescription>
              Enable or disable categories of tools available to the AI agent
            </CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            {settings.toolConfigLoading ? (
              <div className="flex items-center justify-center py-8">
                <RefreshCw className="h-6 w-6 animate-spin" />
                <span className="ml-2">Loading tool configurations...</span>
              </div>
            ) : (
              <div className="space-y-4">
                {Object.entries(settings.toolConfigurations).map(
                  ([categoryName, category]) => (
                    <div key={categoryName} className="border rounded-lg">
                      <div className="flex items-center justify-between p-4 border-b">
                        <div>
                          <div className="font-medium">{category.name}</div>
                          <div className="text-sm text-gray-500">
                            {category.description}
                          </div>
                        </div>
                        <Switch
                          checked={category.enabled}
                          onCheckedChange={(enabled) =>
                            handleToggleCategory(categoryName, enabled)
                          }
                        />
                      </div>

                      {category.enabled && (
                        <div className="p-4 space-y-2">
                          {category.tools.map((tool) => (
                            <div
                              key={tool.name}
                              className="flex items-center justify-between p-2 rounded bg-gray-50"
                            >
                              <div>
                                <div className="text-sm font-medium">
                                  {tool.name}
                                </div>
                                {tool.description && (
                                  <div className="text-xs text-gray-500">
                                    {tool.description}
                                  </div>
                                )}
                                {tool.required && (
                                  <Badge
                                    variant="secondary"
                                    className="mt-1 text-xs"
                                  >
                                    Required
                                  </Badge>
                                )}
                              </div>
                              <Switch
                                checked={tool.enabled}
                                disabled={tool.required}
                                onCheckedChange={(enabled) =>
                                  handleToggleTool(tool.name, enabled)
                                }
                              />
                            </div>
                          ))}
                        </div>
                      )}
                    </div>
                  )
                )}

                {Object.keys(settings.toolConfigurations).length === 0 && (
                  <div className="text-center py-8 text-gray-500">
                    No tool configurations available
                  </div>
                )}
              </div>
            )}

            <div className="pt-4 border-t">
              <Button
                onClick={handleResetToolConfiguration}
                variant="outline"
                disabled={settings.toolConfigLoading}
                className="w-full"
              >
                <RotateCcw className="w-4 h-4 mr-2" />
                Reset Tool Configuration
              </Button>
            </div>
          </CardContent>
        </Card>
      </div>
    </div>
  );
}

function AdvancedSettings({
  settings,
}: {
  settings: ReturnType<typeof useSettings>;
}) {
  return (
    <div className="space-y-6">
      <div>
        <h3 className="text-lg font-medium text-gray-900 mb-4">Advanced</h3>

        <Card>
          <CardHeader>
            <CardTitle>Developer Options</CardTitle>
            <CardDescription>
              Advanced settings for developers and power users
            </CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="flex items-center justify-between p-3 border rounded-lg">
              <div>
                <div className="font-medium">Debug Mode</div>
                <div className="text-sm text-gray-500">
                  Enable verbose logging and debug features
                </div>
              </div>
              <Switch
                onCheckedChange={async (enabled) => {
                  try {
                    await invoke("set_debug_mode", { enabled });
                    toast.success(
                      `Debug mode ${enabled ? "enabled" : "disabled"}`
                    );
                  } catch (error) {
                    toast.error("Failed to toggle debug mode");
                  }
                }}
              />
            </div>

            <div className="flex items-center justify-between p-3 border rounded-lg">
              <div>
                <div className="font-medium">Performance Monitoring</div>
                <div className="text-sm text-gray-500">
                  Monitor system resource usage
                </div>
              </div>
              <Switch
                onCheckedChange={async (enabled) => {
                  try {
                    await invoke("set_performance_monitoring", { enabled });
                    toast.success(
                      `Performance monitoring ${
                        enabled ? "enabled" : "disabled"
                      }`
                    );
                  } catch (error) {
                    toast.error("Failed to toggle performance monitoring");
                  }
                }}
              />
            </div>
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle>Reset Settings</CardTitle>
            <CardDescription>
              Reset all settings to their default values
            </CardDescription>
          </CardHeader>
          <CardContent>
            <Button
              variant="destructive"
              onClick={async () => {
                if (
                  confirm(
                    "Are you sure you want to reset all settings? This action cannot be undone."
                  )
                ) {
                  try {
                    await invoke("reset_all_settings");
                    await settings.loadAllSettings();
                    toast.success("All settings have been reset to defaults");
                  } catch (error) {
                    toast.error("Failed to reset settings");
                  }
                }
              }}
              className="w-full"
            >
              <RotateCcw className="w-4 h-4 mr-2" />
              Reset All Settings
            </Button>
          </CardContent>
        </Card>
      </div>
    </div>
  );
}
