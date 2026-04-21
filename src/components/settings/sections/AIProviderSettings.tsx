import {
  EnvironmentVariables,
  EnvironmentVariablesHeader,
  EnvironmentVariablesTitle,
  EnvironmentVariablesToggle,
  EnvironmentVariablesContent,
  EnvironmentVariable,
} from "@/components/ai-elements/environment-variables";
import {
  ModelSelector,
  ModelSelectorContent,
  ModelSelectorEmpty,
  ModelSelectorGroup,
  ModelSelectorInput,
  ModelSelectorItem,
  ModelSelectorList,
  ModelSelectorLogo,
  ModelSelectorName,
} from "@/components/ai-elements/model-selector";
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
import { Save, Check, CheckCircle } from "lucide-react";
import { SettingsSectionProps } from "../types";
import { useMemo, useState, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { COMMANDS } from "@/lib/constants.generated";

export default function AIProviderSettings({ settings }: SettingsSectionProps) {
  const [modelSelectorOpen, setModelSelectorOpen] = useState(false);

  const currentProvider = settings.providers?.find(
    (p) => p.id === settings.activeProvider
  );

  // Build sorted provider list: active provider first, then others with models
  const sortedProviders = useMemo(() => {
    const withModels = settings.providers.filter(
      (p) => p.model_info && p.model_info.length > 0
    );
    const active = withModels.filter((p) => p.id === settings.activeProvider);
    const rest = withModels.filter((p) => p.id !== settings.activeProvider);
    return [...active, ...rest];
  }, [settings.providers, settings.activeProvider]);

  // Find selected model across all providers
  const currentModelId =
    settings.formData.model || settings.providerSettings?.model || "";

  const selectedModel = useMemo(() => {
    for (const provider of settings.providers) {
      const found = provider.model_info?.find((m) => m.id === currentModelId);
      if (found) return { model: found, providerId: provider.id };
    }
    return null;
  }, [settings.providers, currentModelId]);

  const modelDisplayName = selectedModel
    ? selectedModel.model.name
    : settings.isLoading
      ? "Loading..."
      : "Select model";

  const handleModelSelect = useCallback(
    async (providerId: string, modelId: string) => {
      setModelSelectorOpen(false);
      try {
        // If switching providers, change the active provider first
        if (providerId !== settings.activeProvider) {
          await settings.handleActiveProviderChange(providerId);
        }

        // Validate and set the model
        const isValid = await invoke<boolean>(
          COMMANDS.PROVIDERS_VALIDATE_PROVIDER_MODEL,
          { providerId, modelId }
        );
        if (!isValid) return;

        settings.setFormData((prev) => ({ ...prev, model: modelId }));

        await invoke(COMMANDS.PROVIDERS_UPDATE_PROVIDER_MODEL, {
          providerId,
          model: modelId,
        });
      } catch (error) {
        console.error("AIProviderSettings: Failed to change model:", error);
      }
    },
    [settings]
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
                  <SelectItem
                    key={provider.id}
                    value={provider.id}
                    disabled={!provider.is_available}
                    className={!provider.is_available ? "opacity-50" : undefined}
                  >
                    <div className="flex items-center gap-2">
                      <span>{provider.name}</span>
                      {!provider.is_available && (
                        <Badge
                          variant="outline"
                          className="text-xs text-muted-foreground"
                        >
                          {provider.id === "claude_cli"
                            ? "CLI not found"
                            : "No API key"}
                        </Badge>
                      )}
                      {provider.is_available && provider.computer_use_supported && (
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

          {/* Model selector — same component as chat input */}
          <div className="space-y-2">
            <Label>Model</Label>
            <ModelSelector open={modelSelectorOpen} onOpenChange={setModelSelectorOpen}>
              <Button
                variant="outline"
                className="w-full justify-between font-normal"
                onClick={() => setModelSelectorOpen(true)}
                disabled={settings.isLoading}
              >
                <span className="flex items-center gap-2">
                  {selectedModel && (
                    <ModelSelectorLogo provider={selectedModel.providerId} />
                  )}
                  <span className="truncate">{modelDisplayName}</span>
                </span>
              </Button>
              <ModelSelectorContent>
                <ModelSelectorInput placeholder="Search models..." />
                <ModelSelectorList>
                  <ModelSelectorEmpty>No models found.</ModelSelectorEmpty>
                  {sortedProviders.map((provider) => (
                    <ModelSelectorGroup
                      key={provider.id}
                      heading={
                        <span className="flex items-center gap-1.5">
                          <ModelSelectorLogo provider={provider.id} className="size-3" />
                          {provider.name}
                          {!provider.is_available && (
                            <span className="text-[10px] text-muted-foreground/60">
                              —{" "}
                              {provider.id === "claude_cli"
                                ? "CLI not found"
                                : "No API key"}
                            </span>
                          )}
                        </span>
                      }
                      className={!provider.is_available ? "opacity-50" : undefined}
                    >
                      {provider.model_info.map((model) => {
                        const isActive =
                          model.id === currentModelId &&
                          provider.id === settings.activeProvider;
                        return (
                          <ModelSelectorItem
                            key={`${provider.id}:${model.id}`}
                            value={`${provider.id} ${model.id} ${model.name}`}
                            onSelect={() => {
                              if (!provider.is_available) return;
                              handleModelSelect(provider.id, model.id);
                            }}
                            disabled={!provider.is_available}
                            className={!provider.is_available ? "opacity-50 cursor-not-allowed" : undefined}
                          >
                            <ModelSelectorLogo provider={provider.id} />
                            <ModelSelectorName>{model.name}</ModelSelectorName>
                            {!provider.is_available && (
                              <span className="text-xs text-muted-foreground">
                                {provider.id === "claude_cli"
                                  ? "CLI not found"
                                  : "No API key"}
                              </span>
                            )}
                            {provider.is_available && model.is_recommended && (
                              <span className="text-xs text-green-600">Recommended</span>
                            )}
                            {provider.is_available && model.supports_computer_use && (
                              <span className="text-xs text-blue-600">Computer Use</span>
                            )}
                            {isActive && <Check className="size-4 text-primary" />}
                          </ModelSelectorItem>
                        );
                      })}
                    </ModelSelectorGroup>
                  ))}
                </ModelSelectorList>
              </ModelSelectorContent>
            </ModelSelector>
            {selectedModel && (
              <div className="text-xs text-muted-foreground">
                {selectedModel.model.supports_computer_use ? (
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
        </CardContent>
      </Card>

      {settings.activeProvider && settings.providerSettings && (
        <Card>
          <CardHeader>
            <CardTitle>Provider Configuration</CardTitle>
            <CardDescription>
              Configure settings for{" "}
              {settings.activeProvider === "claude_cli"
                ? "Claude CLI"
                : settings.activeProvider}
            </CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            {settings.activeProvider === "claude_cli" ? (
              <div className="rounded-md border border-blue-200 bg-blue-50 p-4 dark:border-blue-800 dark:bg-blue-950">
                <p className="text-sm font-medium text-blue-900 dark:text-blue-100">
                  No API key needed
                </p>
                <p className="mt-1 text-sm text-blue-700 dark:text-blue-300">
                  Claude CLI uses your existing authentication. Run{" "}
                  <code className="rounded bg-blue-100 px-1 py-0.5 text-xs dark:bg-blue-900">
                    claude login
                  </code>{" "}
                  in your terminal if not authenticated.
                </p>
              </div>
            ) : (
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
            )}

            {/* Max tokens / temperature — not applicable to Claude CLI (managed by the CLI) */}
            {settings.activeProvider !== "claude_cli" && (
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
            )}

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
