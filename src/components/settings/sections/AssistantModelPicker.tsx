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
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Check } from "lucide-react";
import { SettingsSectionProps } from "../types";
import { SettingsGroup, SettingsRow } from "../ui";
import { useAdvancedSettings } from "../AdvancedSettingsContext";
import { useCallback, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { COMMANDS } from "@/lib/constants.generated";

interface AssistantModelPickerProps {
  settings: SettingsSectionProps["settings"];
  /** Gate the whole group behind the advanced-settings toggle. */
  advanced?: boolean;
}

/**
 * Provider + model picker for the assistant/computer-use agent. Extracted from
 * AIProviderSettings so the same control can appear in the unified Models pane
 * without duplicating its validate/persist behavior — both surfaces read and
 * write the single active provider+model through the shared settings context,
 * so they stay in sync.
 */
export default function AssistantModelPicker({
  settings,
  advanced = false,
}: AssistantModelPickerProps) {
  const [modelSelectorOpen, setModelSelectorOpen] = useState(false);

  const currentProvider = settings.providers?.find(
    (p) => p.id === settings.activeProvider
  );

  const { advanced: showAdvanced } = useAdvancedSettings();

  const currentModelId =
    settings.formData.model || settings.providerSettings?.model || "";

  // Build sorted provider list: active provider first, then others with models.
  // A provider whose models are all hidden drops out rather than rendering an
  // empty group.
  const sortedProviders = useMemo(() => {
    const withModels = settings.providers.filter(
      (p) =>
        p.model_info &&
        p.model_info.some(
          (m) => showAdvanced || !m.is_legacy || m.id === currentModelId
        )
    );
    const active = withModels.filter((p) => p.id === settings.activeProvider);
    const rest = withModels.filter((p) => p.id !== settings.activeProvider);
    return [...active, ...rest];
  }, [
    settings.providers,
    settings.activeProvider,
    showAdvanced,
    currentModelId,
  ]);

  // Juno offers the current generation and nothing else, the way an Apple
  // product does. Models that only drive the computer through an older tool
  // version stay out of the list unless you ask for them — with one exception:
  // whatever is selected right now is always shown, so the list can never hide
  // what Juno is actually running, and you can always switch off it.
  const visibleModels = useCallback(
    (models: typeof settings.providers[number]["model_info"]) =>
      models.filter(
        (model) => showAdvanced || !model.is_legacy || model.id === currentModelId
      ),
    [showAdvanced, currentModelId]
  );

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
        console.error("AssistantModelPicker: Failed to change model:", error);
      }
    },
    [settings]
  );

  return (
    <SettingsGroup
      title="Provider Selection"
      advanced={advanced}
      footer="Choose your AI provider and model"
    >
      <SettingsRow
        htmlFor="ai-provider"
        label="Active Provider"
        below={
          currentProvider && (
            // Every provider Juno ships can drive the computer, so a green
            // "capabilities available" line under each one was decoration.
            // The exception is marked in the list instead.
            <p className="text-sm text-muted-foreground">
              {currentProvider.description}
            </p>
          )
        }
      >
        <Select
          value={settings.activeProvider}
          onValueChange={settings.handleActiveProviderChange}
        >
          <SelectTrigger id="ai-provider" className="w-[220px]">
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
                  {/* Every provider Juno ships has computer-use models, so a
                      "Computer Use" badge on each row said nothing. Only the
                      exception is worth marking. */}
                  {provider.is_available && !provider.computer_use_supported && (
                    <Badge variant="secondary" className="text-xs text-muted-foreground">
                      Chat only
                    </Badge>
                  )}
                </div>
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </SettingsRow>

      {/* Model selector — same component as chat input */}
      <SettingsRow
        label="Model"
        below={
          <div className="space-y-2">
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
                      {visibleModels(provider.model_info).map((model) => {
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
                              <span className="text-xs text-muted-foreground">Recommended</span>
                            )}
                            {/* Absence is the signal: computer use is what Juno
                                is for, so only a model that cannot do it is
                                marked. There is no longer a "Juno cannot drive
                                this yet" case — Juno sends both the legacy
                                computer tools and the current toolset, so a
                                model that is unmarked here can be driven. */}
                            {provider.is_available && !model.supports_computer_use && (
                              <span className="text-xs text-muted-foreground">Chat only</span>
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
            {selectedModel && !selectedModel.model.supports_computer_use && (
              <div className="text-xs text-muted-foreground">
                {`${selectedModel.model.name} can answer questions, but it cannot control the computer. Pick another model for that.`}
              </div>
            )}
          </div>
        }
      />
    </SettingsGroup>
  );
}
