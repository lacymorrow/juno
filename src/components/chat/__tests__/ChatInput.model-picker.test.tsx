import { fireEvent, render, screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { COMMANDS } from "@/lib/constants.generated";
import { TooltipProvider } from "@/components/ui/tooltip";
import { SettingsProvider } from "@/contexts/SettingsContext";
import type { ProviderInfo } from "@/types/settings";
import { ChatInput } from "../ChatInput";

/**
 * Which models the chat input offers (LAC-9116).
 *
 * Mounted the way the app mounts it (main.tsx): the real `TooltipProvider` and
 * `SettingsProvider`, and no `AdvancedSettingsProvider`. Only the Tauri backend is faked, so the
 * model list and the selection arrive through the real `useSettings` loader.
 */

// cmdk calls scrollIntoView, which jsdom does not implement.
Element.prototype.scrollIntoView = vi.fn();

// The mock factory is hoisted above the imports, so it cannot read COMMANDS.
// The first test pins these literals against the generated constants.
const backend = vi.hoisted(() => ({
  selected: "",
  providers: [] as unknown[],
  GET_PROVIDERS: "get_providers",
  GET_ACTIVE: "get_active_provider",
  GET_SETTINGS: "get_provider_settings",
  GET_WAKE_WORDS: "get_always_listening_wake_words",
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn((command: string) => {
    switch (command) {
      case backend.GET_PROVIDERS:
        return Promise.resolve(backend.providers);
      case backend.GET_ACTIVE:
        return Promise.resolve("anthropic");
      case backend.GET_SETTINGS:
        return Promise.resolve({ id: "anthropic", model: backend.selected });
      // The loader joins this one, and stops before the selection without it.
      case backend.GET_WAKE_WORDS:
        return Promise.resolve([]);
      default:
        return Promise.resolve(undefined);
    }
  }),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
  emit: vi.fn(),
}));

vi.mock("sonner", () => ({
  toast: { error: vi.fn(), success: vi.fn(), warning: vi.fn(), info: vi.fn() },
}));

const model = (id: string, name: string, is_legacy: boolean) => ({
  id,
  name,
  is_legacy,
  supports_computer_use: true,
  is_recommended: false,
});

const OPUS_5_5 = model("claude-opus-5-5", "Claude Opus 5.5", false);
const SONNET_5_5 = model("claude-sonnet-5-5", "Claude Sonnet 5.5", false);
const OPUS_4_8 = model("claude-opus-4-8", "Claude Opus 4.8", true);
const OPUS_4_6 = model("claude-opus-4-6", "Claude Opus 4.6", true);

const provider = (models: ProviderInfo["model_info"]): ProviderInfo => ({
  id: "anthropic",
  name: "Anthropic Claude",
  description: "",
  models: models.map((m) => m.id),
  default_model: OPUS_5_5.id,
  model_info: models,
  is_available: true,
  is_default: true,
  computer_use_supported: true,
  needs_sign_in: false,
  has_saved_key: false,
});

/** Open the picker with `selected` chosen, once the real loader has it. */
const openPicker = async (selected: { id: string; name: string }) => {
  backend.selected = selected.id;
  render(
    <TooltipProvider>
      <SettingsProvider>
        <ChatInput
          query=""
          isProcessing={false}
          canSubmit
          onQueryChange={vi.fn()}
          onSubmit={vi.fn()}
          onStop={vi.fn()}
        />
      </SettingsProvider>
    </TooltipProvider>
  );
  const trigger = await screen.findByRole("button", {
    name: new RegExp(selected.name.replace(/\./g, "\\.")),
  });
  await vi.waitFor(() => expect(trigger).toBeEnabled());
  fireEvent.click(trigger);
  return within(screen.getByRole("listbox"));
};

describe("ChatInput model picker, mounted the way the app mounts it", () => {
  beforeEach(() => {
    backend.providers = [
      provider([OPUS_5_5, SONNET_5_5, OPUS_4_8, OPUS_4_6]),
    ];
  });

  it("pins the command names this file's hoisted mock matches on", () => {
    expect(backend.GET_PROVIDERS).toBe(COMMANDS.PROVIDERS_GET_PROVIDERS);
    expect(backend.GET_ACTIVE).toBe(COMMANDS.PROVIDERS_GET_ACTIVE_PROVIDER);
    expect(backend.GET_SETTINGS).toBe(
      COMMANDS.PROVIDERS_GET_PROVIDER_SETTINGS
    );
    expect(backend.GET_WAKE_WORDS).toBe(
      COMMANDS.ALWAYS_LISTENING_GET_ALWAYS_LISTENING_WAKE_WORDS
    );
  });

  it("does not offer a legacy model", async () => {
    const list = await openPicker(SONNET_5_5);
    expect(list.queryByText(OPUS_4_8.name)).not.toBeInTheDocument();
  });

  it("offers a current model", async () => {
    const list = await openPicker(OPUS_5_5);
    expect(list.getByText(SONNET_5_5.name)).toBeInTheDocument();
  });

  it("keeps the selected model listed even when it is legacy", async () => {
    const list = await openPicker(OPUS_4_6);
    expect(list.getByText(OPUS_4_6.name)).toBeInTheDocument();
  });

  it("never hides Opus 5.5, the default model", async () => {
    // Selected model is a different one, so the selected-model exception
    // cannot be what keeps Opus 5.5 on the list.
    const list = await openPicker(SONNET_5_5);
    expect(list.getByText(OPUS_5_5.name)).toBeInTheDocument();
  });
});
