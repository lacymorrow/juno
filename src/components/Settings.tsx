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
import { Brain, Mic, Save, Settings as SettingsIcon } from "lucide-react";
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

const Settings: React.FC = () => {
  // TTS Settings
  const [ttsProvider, setTtsProvider] = useState<string>("off");

  // AI Provider Settings
  const [providers, setProviders] = useState<ProviderInfo[]>([]);
  const [activeProvider, setActiveProvider] = useState<string>("");
  const [providerSettings, setProviderSettings] =
    useState<ProviderSettings | null>(null);
  const [isLoading, setIsLoading] = useState<boolean>(false);

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
    } catch (error) {
      console.error("Failed to load settings:", error);
      toast.error("Failed to load settings");
    } finally {
      setIsLoading(false);
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

  const currentProvider = providers.find((p) => p.id === activeProvider);

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
              Choose how AI responses should be spoken aloud. Use Alt+D to
              toggle voice input.
            </p>
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

      {/* Keyboard Shortcuts Info */}
      <Card>
        <CardHeader>
          <CardTitle>Keyboard Shortcuts</CardTitle>
          <CardDescription>
            Essential keyboard shortcuts for using Juno
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-2">
          <div className="flex justify-between items-center">
            <span>Toggle Voice Input</span>
            <kbd className="px-2 py-1 bg-muted rounded text-sm">Alt+D</kbd>
          </div>
          <div className="flex justify-between items-center">
            <span>Stop Current Task</span>
            <kbd className="px-2 py-1 bg-muted rounded text-sm">Escape</kbd>
          </div>
          <div className="flex justify-between items-center">
            <span>Settings</span>
            <kbd className="px-2 py-1 bg-muted rounded text-sm">Cmd+,</kbd>
          </div>
        </CardContent>
      </Card>
    </div>
  );
};

export default Settings;
