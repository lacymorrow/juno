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
import { providerUnavailableReason } from "@/lib/provider-status";
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
import { useCallback, useMemo, useRef, useState } from "react";
import { toast } from "sonner";
import { invoke } from "@tauri-apps/api/core";
import { COMMANDS } from "@/lib/constants.generated";

interface AssistantModelPickerProps {
  settings: SettingsSectionProps["settings"];
  /** Gate the whole group behind the advanced-settings toggle. */
  advanced?: boolean;
}

/**
 * Provider + model picker for the assistant/computer-use agent. Extracted from
 * AIProviderSettings purely to keep that file readable; AIProviderSettings is
 * its only call site. A pick validates the model against the provider before
 * persisting it (PROVIDERS_VALIDATE_PROVIDER_MODEL, then
 * PROVIDERS_UPDATE_PROVIDER_MODEL).
 *
 * It is not the only writer of that pair, so do not read it as the single
 * source of truth: chat/ChatInput.tsx carries a near-identical
 * handleModelSelect for the chat model menu, and useSettings writes
 * update_provider_model on save when formData.model changed. Rust owns the
 * invariant, not this component.
 *
 * It did have a second home once: #573 mounted it in a "unified Models pane"
 * next to the dictation tiers, and #578 removed it from there as a duplicate
 * when that pane became one dictation-model list. Settings > Models is
 * ModelsSettings, the dictation catalog, and does not mount this picker.
 * Wiring it back in would re-create the duplicate #578 deleted.
 */
export default function AssistantModelPicker({
  settings,
  advanced = false,
}: AssistantModelPickerProps) {
  const [modelSelectorOpen, setModelSelectorOpen] = useState(false);

  const currentProvider = settings.providers?.find(
    (p) => p.id === settings.activeProvider
  );

  const currentModelId =
    settings.formData.model || settings.providerSettings?.model || "";

  // Every model a provider reports is listed here, deliberately, and this
  // picker does no filtering of its own.
  //
  // It is wrapped in a `SettingsGroup advanced`, and its only call site passes
  // `advanced` (AIProviderSettings.tsx), so it is mounted only while the
  // advanced-settings toggle is on — and someone who turned that on is asking
  // to see everything.
  //
  // It used to filter on legacy and capability flags, under a comment claiming
  // Juno "offers the current generation and nothing else, the way an Apple
  // product does". That filter read the SAME toggle that gates the group, so it
  // short-circuited on `showAdvanced` every time it ran and hid nothing, in
  // every build that ever shipped (LAC-8407). Do not re-add it here: a filter
  // keyed on the toggle that decides whether this component exists at all can
  // never fire. The surface where that rule would bite is the chat input's
  // model selector (components/chat/ChatInput.tsx), which every person sees and
  // which filters nothing today.
  //
  // A provider that reports no models drops out rather than rendering an empty
  // group.
  const sortedProviders = useMemo(() => {
    const withModels = settings.providers.filter(
      (p) => p.model_info && p.model_info.length > 0
    );
    const active = withModels.filter((p) => p.id === settings.activeProvider);
    const rest = withModels.filter((p) => p.id !== settings.activeProvider);
    return [...active, ...rest];
  }, [settings.providers, settings.activeProvider]);

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

  // Newest pick wins: a slower earlier pick that resolves late must not write
  // its model over a later one (LAC-8469).
  const latestPick = useRef(0);

  const handleModelSelect = useCallback(
    async (providerId: string, modelId: string) => {
      setModelSelectorOpen(false);
      const pick = ++latestPick.current;
      const previousProvider = settings.activeProvider;
      const switching = providerId !== previousProvider;
      let switched = false;
      try {
        // Validate before touching anything, so a rejected model never
        // leaves the provider half switched.
        const isValid = await invoke<boolean>(
          COMMANDS.PROVIDERS_VALIDATE_PROVIDER_MODEL,
          { providerId, modelId }
        );
        if (pick !== latestPick.current) return;
        if (!isValid) {
          toast.error("That model isn't available for this provider.");
          return;
        }

        if (switching) {
          await settings.handleActiveProviderChange(providerId);
          switched = true;
          if (pick !== latestPick.current) return;
        }

        await invoke(COMMANDS.PROVIDERS_UPDATE_PROVIDER_MODEL, {
          providerId,
          model: modelId,
        });
        if (pick !== latestPick.current) return;

        // Only show the new model once Rust has it. Setting it before the
        // persist left the picker showing a model that failed to save (LAC-8423).
        settings.setFormData((prev) => ({ ...prev, model: modelId }));
      } catch (error) {
        console.error("AssistantModelPicker: Failed to change model:", error);
        toast.error("Couldn't save the model. Nothing was changed.");
        // The provider switch already landed in Rust. Switch back, which also
        // reloads the old provider's model from the backend, so the picker
        // and Rust agree again.
        if (switched && pick === latestPick.current && previousProvider) {
          await settings.handleActiveProviderChange(previousProvider);
        }
      }
    },
    [settings]
  );

  return (
    <SettingsGroup
      title="Provider Selection"
      advanced={advanced}
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
                      {providerUnavailableReason(provider)}
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
        id="ai-model"
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
                              — {providerUnavailableReason(provider)}
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
                                {providerUnavailableReason(provider)}
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
