import React, { useCallback, useEffect, useRef, useState } from "react";
import { Plus, User, Users } from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import type { ChatStatus } from "ai";
import {
  PromptInput,
  PromptInputTextarea,
  PromptInputFooter,
  PromptInputTools,
  PromptInputButton,
  PromptInputSubmit,
  PromptInputSelect,
  PromptInputSelectTrigger,
  PromptInputSelectContent,
  PromptInputSelectItem,
  PromptInputSelectValue,
  type PromptInputMessage,
} from "@/components/ai-elements/prompt-input";
import { useSettings } from "@/hooks/useSettings";
import { useEventListener } from "@/hooks/useEventListener";
import { EVENTS, COMMANDS } from "@/lib/constants.generated";

interface ModelInfo {
  id: string;
  name: string;
  supports_computer_use: boolean;
  is_recommended: boolean;
}

interface ChatInputProps {
  query: string;
  isProcessing: boolean;
  canSubmit: boolean;
  onQueryChange: (value: string) => void;
  onSubmit: (text: string) => void;
  onStop: () => void;
  onNewChat: () => void;
}

export const ChatInput = React.memo(function ChatInput({
  query,
  isProcessing,
  canSubmit,
  onQueryChange,
  onSubmit,
  onStop,
  onNewChat,
}: ChatInputProps) {
  const settings = useSettings();
  const [availableModels, setAvailableModels] = useState<ModelInfo[]>([]);
  const [loadingModels, setLoadingModels] = useState(false);
  const loadRequestRef = useRef(0);

  const chatStatus: ChatStatus = isProcessing ? "streaming" : "ready";

  // Load models for the active provider
  const loadModelsForProvider = useCallback(async (providerId: string) => {
    const requestId = ++loadRequestRef.current;
    setLoadingModels(true);
    try {
      const models = await invoke<ModelInfo[]>(
        COMMANDS.PROVIDERS_GET_PROVIDER_MODELS,
        { providerId }
      );
      if (requestId !== loadRequestRef.current) return;
      setAvailableModels(models);
    } catch (error) {
      if (requestId !== loadRequestRef.current) return;
      console.error("ChatInput: Failed to load models:", error);
      setAvailableModels([]);
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

  // Refresh models on provider settings changes
  useEventListener(EVENTS.SYSTEM_PROVIDER_SETTINGS_CHANGED, () => {
    if (settings.activeProvider) {
      loadModelsForProvider(settings.activeProvider);
    }
  });

  const currentModelId =
    settings.formData.model || settings.providerSettings?.model || "";

  const handleModelChange = useCallback(
    async (value: string) => {
      try {
        const isValid = await invoke<boolean>(
          COMMANDS.PROVIDERS_VALIDATE_PROVIDER_MODEL,
          { providerId: settings.activeProvider, modelId: value }
        );
        if (!isValid) return;

        settings.setFormData((prev) => ({ ...prev, model: value }));

        await invoke(COMMANDS.PROVIDERS_UPDATE_PROVIDER_MODEL, {
          providerId: settings.activeProvider,
          model: value,
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

  // Agent mode toggle (single ↔ multi)
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

  const selectedModel = availableModels.find((m) => m.id === currentModelId);
  const modelDisplayName = selectedModel
    ? selectedModel.name.replace(/Claude\s*/i, "")
    : loadingModels
      ? "Loading..."
      : "Select model";

  return (
    <PromptInput onSubmit={handleSubmit}>
      <PromptInputTextarea
        value={query}
        onChange={(e) => onQueryChange(e.target.value)}
        placeholder={
          isProcessing ? "Processing..." : "What would you like to know?"
        }
        disabled={isProcessing || !canSubmit}
      />
      <PromptInputFooter>
        <PromptInputTools>
          <PromptInputButton
            tooltip="New Chat"
            onClick={onNewChat}
            disabled={isProcessing}
          >
            <Plus className="size-4" />
            <span className="text-xs">New</span>
          </PromptInputButton>
          <PromptInputButton
            tooltip={`Agent Mode: ${settings.agentMode === "multi" ? "Multi" : "Single"} (click to toggle)`}
            onClick={handleAgentModeToggle}
            disabled={isProcessing || settings.isLoading}
          >
            {settings.agentMode === "multi" ? (
              <Users className="size-4" />
            ) : (
              <User className="size-4" />
            )}
            <span className="text-xs">
              {settings.agentMode === "multi" ? "Multi" : "Single"}
            </span>
          </PromptInputButton>
        </PromptInputTools>

        {/* Model selector */}
        {availableModels.length > 0 && (
          <PromptInputSelect
            value={currentModelId}
            onValueChange={handleModelChange}
          >
            <PromptInputSelectTrigger className="h-7 w-auto max-w-[180px] text-xs">
              <PromptInputSelectValue placeholder="Select model">
                {modelDisplayName}
              </PromptInputSelectValue>
            </PromptInputSelectTrigger>
            <PromptInputSelectContent>
              {availableModels.map((model) => (
                <PromptInputSelectItem key={model.id} value={model.id}>
                  <span className="mr-1">
                    {model.supports_computer_use ? "\uD83D\uDDA5\uFE0F" : "\uD83D\uDCAC"}
                  </span>
                  {model.name}
                  {model.is_recommended && (
                    <span className="ml-1 text-xs text-green-600">
                      Recommended
                    </span>
                  )}
                </PromptInputSelectItem>
              ))}
            </PromptInputSelectContent>
          </PromptInputSelect>
        )}

        <PromptInputSubmit
          disabled={!isProcessing && (!canSubmit || !query.trim())}
          status={chatStatus}
          onStop={onStop}
        />
      </PromptInputFooter>
    </PromptInput>
  );
});
