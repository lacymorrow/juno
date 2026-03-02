import React, { useCallback, useMemo, useState } from "react";
import { Check, User, Users } from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import type { ChatStatus } from "ai";
import {
  PromptInput,
  PromptInputTextarea,
  PromptInputFooter,
  PromptInputTools,
  PromptInputButton,
  PromptInputSubmit,
  type PromptInputMessage,
} from "@/components/ai-elements/prompt-input";
import {
  ModelSelector,
  ModelSelectorTrigger,
  ModelSelectorContent,
  ModelSelectorInput,
  ModelSelectorList,
  ModelSelectorGroup,
  ModelSelectorItem,
  ModelSelectorEmpty,
  ModelSelectorLogo,
  ModelSelectorName,
} from "@/components/ai-elements/model-selector";
import { useSettings } from "@/hooks/useSettings";
import { COMMANDS } from "@/lib/constants.generated";

interface ChatInputProps {
  query: string;
  isProcessing: boolean;
  canSubmit: boolean;
  onQueryChange: (value: string) => void;
  onSubmit: (text: string) => void;
  onStop: () => void;
}

export const ChatInput = React.memo(function ChatInput({
  query,
  isProcessing,
  canSubmit,
  onQueryChange,
  onSubmit,
  onStop,
}: ChatInputProps) {
  const settings = useSettings();
  const [modelSelectorOpen, setModelSelectorOpen] = useState(false);

  const chatStatus: ChatStatus = isProcessing ? "streaming" : "ready";

  const currentModelId =
    settings.formData.model || settings.providerSettings?.model || "";

  // Build a sorted provider list: active provider first, then others with models
  const sortedProviders = useMemo(() => {
    const withModels = settings.providers.filter(
      (p) => p.model_info && p.model_info.length > 0
    );
    const active = withModels.filter((p) => p.id === settings.activeProvider);
    const rest = withModels.filter((p) => p.id !== settings.activeProvider);
    return [...active, ...rest];
  }, [settings.providers, settings.activeProvider]);

  // Find the currently selected model across all providers
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
        console.error("ChatInput: Failed to change model:", error);
        if (settings.providerSettings?.model) {
          settings.setFormData((prev) => ({
            ...prev,
            model: settings.providerSettings?.model || "",
          }));
        }
      }
    },
    [settings]
  );

  // Agent mode toggle (single <-> multi)
  const handleAgentModeToggle = useCallback(async () => {
    const newMode = settings.agentMode === "single" ? "multi" : "single";
    try {
      await settings.handleAgentModeChange(newMode);
    } catch (error) {
      console.error("ChatInput: Failed to toggle agent mode:", error);
    }
  }, [settings]);

  const handleSubmit = useCallback(
    (message: PromptInputMessage) => {
      const text = message.text.trim();
      if (!text) return;
      onSubmit(text);
    },
    [onSubmit]
  );

  const hasModels = sortedProviders.length > 0;

  return (
    <PromptInput onSubmit={handleSubmit}>
      <PromptInputTextarea
        value={query}
        onChange={(e) => onQueryChange(e.target.value)}
        placeholder={
          isProcessing ? "Processing..." : "Ask anything..."
        }
        disabled={isProcessing || !canSubmit}
      />
      <PromptInputFooter>
        <PromptInputTools>
          <PromptInputButton
            tooltip={`Agent Mode: ${settings.agentMode === "multi" ? "Multi" : "Single"} (click to toggle)`}
            onClick={handleAgentModeToggle}
            disabled={isProcessing || settings.isLoading}
          >
            {settings.agentMode === "multi" ? (
              <Users className="size-3.5" />
            ) : (
              <User className="size-3.5" />
            )}
            <span className="text-[11px] text-muted-foreground/70">
              {settings.agentMode === "multi" ? "Multi" : "Single"}
            </span>
          </PromptInputButton>

        {/* Model selector — shows all providers' models */}
        {hasModels && (
          <ModelSelector open={modelSelectorOpen} onOpenChange={setModelSelectorOpen}>
            <ModelSelectorTrigger asChild>
              <PromptInputButton tooltip="Change model" disabled={isProcessing || settings.isLoading}>
                {selectedModel && (
                  <ModelSelectorLogo provider={selectedModel.providerId} />
                )}
                <span className="max-w-[140px] truncate text-[11px] text-muted-foreground/70">{modelDisplayName}</span>
              </PromptInputButton>
            </ModelSelectorTrigger>
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
                            — setup required
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
                          onSelect={() => handleModelSelect(provider.id, model.id)}
                        >
                          <ModelSelectorLogo provider={provider.id} />
                          <ModelSelectorName>{model.name}</ModelSelectorName>
                          {model.is_recommended && (
                            <span className="text-xs text-green-600">Recommended</span>
                          )}
                          {model.supports_computer_use && (
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
        )}
        </PromptInputTools>

        <PromptInputSubmit
          disabled={!isProcessing && (!canSubmit || !query.trim())}
          status={chatStatus}
          onStop={onStop}
          className="rounded-full"
        />
      </PromptInputFooter>
    </PromptInput>
  );
});
