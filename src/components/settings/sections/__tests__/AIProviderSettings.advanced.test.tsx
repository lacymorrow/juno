import { render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import AIProviderSettings from "../AIProviderSettings";

Element.prototype.scrollIntoView = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(() => Promise.resolve({ is_demo: false, cohort: null })),
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
  emit: vi.fn(),
}));

const advanced = vi.hoisted(() => ({ on: false }));
vi.mock("../../AdvancedSettingsContext", () => ({
  useAdvancedSettings: () => ({ advanced: advanced.on }),
  AdvancedOnly: ({ children }: { children: React.ReactNode }) =>
    advanced.on ? children : null,
}));

const settingsFor = (activeProvider: string, hasSavedKey = false) =>
  ({
    activeProvider,
    providers: [
      {
        id: activeProvider,
        name: activeProvider,
        description: "",
        models: ["m"],
        default_model: "m",
        model_info: [],
        is_available: true,
        is_default: true,
        computer_use_supported: true,
        needs_sign_in: false,
        has_saved_key: hasSavedKey,
      },
    ],
    formData: { model: "m", apiKey: "", systemPrompt: "", maxTokens: 1024, temperature: 1 },
    providerSettings: { model: "m", load_account_mcp: true },
    isLoading: false,
    setFormData: vi.fn(),
    handleActiveProviderChange: vi.fn(),
    handleSaveProviderSettings: vi.fn(),
    handleClearProviderApiKey: vi.fn(),
    handleLoadAccountMcpChange: vi.fn(),
  }) as never;

describe("AIProviderSettings Advanced gating", () => {
  beforeEach(() => {
    advanced.on = false;
  });

  it("hides provider configuration (API key, CLI rows) with Advanced off", () => {
    render(<AIProviderSettings settings={settingsFor("anthropic")} />);
    expect(screen.queryByText("Provider Configuration")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("API key")).not.toBeInTheDocument();
  });

  it("hides the CLI rows with Advanced off", () => {
    render(<AIProviderSettings settings={settingsFor("claude_cli")} />);
    expect(screen.queryByText("Provider Configuration")).not.toBeInTheDocument();
    expect(screen.queryByText("Load account MCP connectors")).not.toBeInTheDocument();
    expect(screen.queryByText(/No API key needed/)).not.toBeInTheDocument();
  });

  it("shows the API key field with Advanced on", () => {
    advanced.on = true;
    render(<AIProviderSettings settings={settingsFor("anthropic")} />);
    expect(screen.getByText("Provider Configuration")).toBeInTheDocument();
    expect(screen.getByLabelText("API key")).toHaveValue("");
    expect(screen.queryByText(/Key saved securely/)).not.toBeInTheDocument();
  });

  it("says a key is saved without showing it, and offers to remove it", () => {
    advanced.on = true;
    render(<AIProviderSettings settings={settingsFor("anthropic", true)} />);
    const field = screen.getByLabelText("API key");
    expect(field).toHaveValue("");
    expect(field).toHaveAttribute("type", "password");
    expect(screen.getByText(/Key saved securely/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Remove key" })).toBeInTheDocument();
  });

  it("shows the CLI rows with Advanced on", () => {
    advanced.on = true;
    render(<AIProviderSettings settings={settingsFor("claude_cli")} />);
    expect(screen.getByText("Provider Configuration")).toBeInTheDocument();
    expect(screen.getByText("Load account MCP connectors")).toBeInTheDocument();
  });
});
