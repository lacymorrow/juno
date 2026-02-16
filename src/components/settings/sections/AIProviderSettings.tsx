import {
  EnvironmentVariables,
  EnvironmentVariablesHeader,
  EnvironmentVariablesTitle,
  EnvironmentVariablesToggle,
  EnvironmentVariablesContent,
  EnvironmentVariable,
} from "@/components/ai-elements/environment-variables";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
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
import { Textarea } from "@/components/ui/textarea";
import { Save, CheckCircle, Brain, Cpu } from "lucide-react";
import { SettingsSectionProps } from "../types";
import { useEffect, useState, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";

interface ModelInfo {
  id: string;
  name: string;
  supports_computer_use: boolean;
  is_recommended: boolean;
}

export default function AIProviderSettings({ settings }: SettingsSectionProps) {
  const [availableModels, setAvailableModels] = useState<ModelInfo[]>([]);
  const [loadingModels, setLoadingModels] = useState(false);
  const loadRequestRef = useRef(0);

  const currentProvider = settings.providers?.find(
    (p) => p.id === settings.activeProvider
  );

  const loadModelsForProvider = useCallback(async (providerId: string) => {
    const requestId = ++loadRequestRef.current;
    setLoadingModels(true);
    try {
      const models = await invoke<ModelInfo[]>("get_provider_models", {
        providerId,
      });
      // Only apply results if this is still the latest request (prevents race condition)
      if (requestId === loadRequestRef.current) {
        setAvailableModels(models);
      }
    } catch (error) {
      console.error("Failed to load models for provider:", error);
      if (requestId === loadRequestRef.current) {
        setAvailableModels([]);
      }
    } finally {
      if (requestId === loadRequestRef.current) {
        setLoadingModels(false);
      }
    }
  }, []);

  // Load models when active provider changes
  useEffect(() => {
    if (settings.activeProvider && !settings.isLoading) {
      loadModelsForProvider(settings.activeProvider);
    }
  }, [settings.activeProvider, settings.isLoading, loadModelsForProvider]);

  const selectedModel = availableModels.find(
    (m) => m.id === settings.formData.model
  );

  return (
    <div className="space-y-6">
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
            <EnvironmentVariables>
              <EnvironmentVariablesHeader>
                <EnvironmentVariablesTitle>API Keys</EnvironmentVariablesTitle>
                <EnvironmentVariablesToggle />
              </EnvironmentVariablesHeader>
              <EnvironmentVariablesContent>
                <EnvironmentVariable
                  name={`${(settings.activeProvider ?? "").toUpperCase()}_API_KEY`}
                  value={settings.formData.apiKey}
                  onChange={(val) =>
                    settings.setFormData((prev) => ({
                      ...prev,
                      apiKey: val,
                    }))
                  }
                  required
                />
              </EnvironmentVariablesContent>
            </EnvironmentVariables>

            <div className="space-y-2">
              <Label htmlFor="model">
                Model
                {currentProvider?.computer_use_supported && (
                  <span className="text-xs text-muted-foreground ml-2">
                    (🖥️ = Computer Use)
                  </span>
                )}
              </Label>
              {loadingModels ? (
                <div className="flex items-center gap-2 p-2 border rounded">
                  <div className="w-4 h-4 bg-muted animate-pulse rounded" />
                  <span className="text-sm text-muted-foreground">
                    Loading models...
                  </span>
                </div>
              ) : (
                <Select
                  value={settings.formData.model}
                  onValueChange={async (value) => {
                    // Validate model before setting
                    try {
                      const isValid = await invoke<boolean>(
                        "validate_provider_model",
                        {
                          providerId: settings.activeProvider,
                          modelId: value,
                        }
                      );

                      if (isValid) {
                        settings.setFormData((prev) => ({
                          ...prev,
                          model: value,
                        }));
                      } else {
                        console.warn(
                          `Model ${value} is not valid for provider ${settings.activeProvider}`
                        );
                      }
                    } catch (error) {
                      console.error("Failed to validate model:", error);
                    }
                  }}
                >
                  <SelectTrigger>
                    <SelectValue placeholder="Select model" />
                  </SelectTrigger>
                  <SelectContent>
                    {/* Computer Use Models */}
                    {availableModels.filter(
                      (model) => model.supports_computer_use
                    ).length > 0 && (
                      <>
                        <div className="px-2 py-1 text-xs font-medium text-muted-foreground bg-blue-50 border-b">
                          <div className="flex items-center gap-1">
                            <Cpu size={12} />
                            Computer Use Models
                          </div>
                        </div>
                        {availableModels
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
                    {availableModels.filter(
                      (model) => !model.supports_computer_use
                    ).length > 0 && (
                      <>
                        <div className="px-2 py-1 text-xs font-medium text-muted-foreground bg-gray-50 border-b">
                          <div className="flex items-center gap-1">
                            <Brain size={12} />
                            General Chat Models
                          </div>
                        </div>
                        {availableModels
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
                  </SelectContent>
                </Select>
              )}
              {selectedModel && (
                <div className="text-xs text-muted-foreground">
                  {selectedModel.supports_computer_use ? (
                    <span className="text-green-700">
                      ✅ This model supports computer use automation
                    </span>
                  ) : (
                    <span className="text-amber-700">
                      ⚠️ This model is for general chat only
                    </span>
                  )}
                </div>
              )}
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
  );
}
