import { render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(() => Promise.resolve(null)),
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
  emit: vi.fn(),
}));
// The shared picker has its own tests; this file is about the Copilot card.
vi.mock("../sections/AssistantModelPicker", () => ({
  default: () => null,
}));

import AIProviderSettings from "../sections/AIProviderSettings";
import { providerUnavailableReason } from "@/lib/provider-status";
import type { ProviderInfo } from "@/types/settings";

const copilot = (overrides: Partial<ProviderInfo> = {}): ProviderInfo => ({
  id: "copilot_cli",
  name: "Copilot CLI",
  description: "",
  models: ["auto"],
  model_info: [],
  default_model: "auto",
  is_available: true,
  is_default: true,
  computer_use_supported: true,
  needs_sign_in: false,
  ...overrides,
});

// Only the fields the section reads; the rest of the hook's surface is unused here.
const settingsFor = (provider: ProviderInfo) =>
  ({
    activeProvider: "copilot_cli",
    providers: [provider],
    providerSettings: { load_account_mcp: true },
    formData: { apiKey: "", maxTokens: "", temperature: "", systemPrompt: "" },
    setFormData: vi.fn(),
    handleSaveProviderSettings: vi.fn(),
    handleLoadAccountMcpChange: vi.fn(),
  }) as never;

describe("Copilot provider card", () => {
  beforeEach(() => vi.clearAllMocks());

  it("says no key is needed when Copilot is signed in", () => {
    render(<AIProviderSettings settings={settingsFor(copilot())} />);
    expect(screen.getByText("No API key needed")).toBeTruthy();
    expect(
      screen.getByText(/Juno is using your GitHub Copilot plan through Copilot CLI/),
    ).toBeTruthy();
    expect(
      screen.getByText("Nothing to enter. Juno talks to Copilot on this Mac."),
    ).toBeTruthy();
  });

  it("tells a signed-out person the one command to run", () => {
    render(
      <AIProviderSettings
        settings={settingsFor(copilot({ is_available: false, needs_sign_in: true }))}
      />,
    );
    expect(screen.getByText("Copilot CLI is not signed in")).toBeTruthy();
    expect(screen.getByText("copilot login")).toBeTruthy();
  });

  it("never shows an API key field for Copilot", () => {
    render(<AIProviderSettings settings={settingsFor(copilot())} />);
    expect(screen.queryByText("API Keys")).toBeNull();
  });
});

describe("providerUnavailableReason for Copilot", () => {
  it("names Copilot on the signed-out badge", () => {
    expect(
      providerUnavailableReason(copilot({ is_available: false, needs_sign_in: true })),
    ).toBe("Sign in to Copilot");
  });

  it("renders nothing when Copilot is ready", () => {
    expect(providerUnavailableReason(copilot())).toBeNull();
  });
});
