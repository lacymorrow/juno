import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";

import ModularSettingsWindow, {
  settingsCategories,
  visibleCategories,
} from "../settings/ModularSettingsWindow";
import {
  AdvancedOnly,
  AdvancedSettingsProvider,
  GET_ADVANCED_SETTINGS_ENABLED,
  SET_ADVANCED_SETTINGS_ENABLED,
} from "../settings/AdvancedSettingsContext";
import { SettingsSection } from "../settings/SettingsSection";
import { SettingsField } from "../settings/SettingsField";
import GeneralSettings from "../settings/sections/GeneralSettings";

// Mock Tauri APIs the way the other component tests do
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
  emit: vi.fn(),
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    label: "settings",
    setTitle: vi.fn(() => Promise.resolve()),
  }),
}));

vi.mock("sonner", () => ({
  toast: { success: vi.fn(), error: vi.fn() },
}));

// The window only needs *a* settings object; the sections are stubbed below.
vi.mock("@/contexts/SettingsContext", () => ({
  useSettingsContext: () => ({}),
}));

// Stub every section so the window test exercises only the sidebar/visibility
// logic. ModularSettingsWindow imports sections from "./index".
vi.mock("../settings/index", () => {
  const stub = (id: string) => () => (
    <div data-testid={`section-${id}`}>{id} content</div>
  );
  return {
    GeneralSettings: stub("general"),
    VoiceSettings: stub("voice"),
    AIProviderSettings: stub("ai"),
    ToolsSettings: stub("tools"),
    AutomationsSettings: stub("automations"),
    NetworkSettings: stub("network"),
    SecuritySettings: stub("security"),
    ShortcutsSettings: stub("shortcuts"),
    AdvancedSettings: stub("advanced"),
  };
});

const invokeMock = vi.mocked(invoke);

/** Backend stub: the persisted toggle plus the defaults GeneralSettings loads. */
function mockBackend(advancedPersisted: boolean) {
  invokeMock.mockImplementation(async (cmd: string) => {
    switch (cmd) {
      case GET_ADVANCED_SETTINGS_ENABLED:
        return advancedPersisted;
      case SET_ADVANCED_SETTINGS_ENABLED:
        return undefined;
      case "is_autostart_enabled":
        return false;
      case "get_onboarding_info":
        return null;
      case "ui_get_bar_config":
        return { bar_appearance: "floating" };
      case "get_big_cursor_enabled":
        return true;
      case "get_big_cursor_scale":
        return 3;
      case "get_companion_mode":
        return false;
      case "get_system_cursor_size":
        return 1;
      default:
        return undefined;
    }
  });
}

const BASIC_SECTIONS = [
  "General",
  "Voice & Audio",
  "AI Provider",
  "Security & Privacy",
  "Keyboard Shortcuts",
];
const ADVANCED_SECTIONS = ["Tools", "Automations", "Network", "Advanced"];

const sidebarButton = (name: string) =>
  screen.queryByRole("button", { name: new RegExp(`^${name}\\b`) });

const toggle = () => screen.getByRole("switch", { name: /advanced settings/i });

beforeEach(() => {
  invokeMock.mockReset();
  // Radix Slider (Big Cursor card) asks for ResizeObserver, which jsdom lacks.
  if (!("ResizeObserver" in globalThis)) {
    (globalThis as any).ResizeObserver = class {
      observe() {}
      unobserve() {}
      disconnect() {}
    };
  }
});

describe("visibleCategories", () => {
  it("keeps only non-advanced sections in basic mode", () => {
    expect(visibleCategories(false).map((c) => c.id)).toEqual([
      "general",
      "voice",
      "ai",
      "security",
      "shortcuts",
    ]);
  });

  it("returns every section in advanced mode", () => {
    expect(visibleCategories(true)).toEqual(settingsCategories);
  });
});

describe("ModularSettingsWindow advanced toggle", () => {
  it("hides the advanced sections by default", async () => {
    mockBackend(false);
    render(<ModularSettingsWindow />);

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith(GET_ADVANCED_SETTINGS_ENABLED)
    );
    await waitFor(() => expect(toggle()).not.toBeDisabled());

    for (const name of BASIC_SECTIONS) {
      expect(sidebarButton(name)).toBeInTheDocument();
    }
    for (const name of ADVANCED_SECTIONS) {
      expect(sidebarButton(name)).not.toBeInTheDocument();
    }
    expect(toggle()).toHaveAttribute("aria-checked", "false");
    expect(screen.getByTestId("section-general")).toBeInTheDocument();
  });

  it("reveals the advanced sections and persists the toggle through invoke", async () => {
    mockBackend(false);
    render(<ModularSettingsWindow />);
    await waitFor(() => expect(toggle()).not.toBeDisabled());

    fireEvent.click(toggle());

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith(SET_ADVANCED_SETTINGS_ENABLED, {
        enabled: true,
      })
    );
    for (const name of [...BASIC_SECTIONS, ...ADVANCED_SECTIONS]) {
      expect(sidebarButton(name)).toBeInTheDocument();
    }
    expect(toggle()).toHaveAttribute("aria-checked", "true");
  });

  it("restores a persisted 'on' state from the backend", async () => {
    mockBackend(true);
    render(<ModularSettingsWindow />);

    await waitFor(() =>
      expect(toggle()).toHaveAttribute("aria-checked", "true")
    );
    for (const name of ADVANCED_SECTIONS) {
      expect(sidebarButton(name)).toBeInTheDocument();
    }
  });

  it("navigates to the first visible section when the current one is hidden", async () => {
    mockBackend(true);
    render(<ModularSettingsWindow />);
    await waitFor(() => expect(sidebarButton("Tools")).toBeInTheDocument());

    fireEvent.click(sidebarButton("Tools")!);
    expect(screen.getByTestId("section-tools")).toBeInTheDocument();

    fireEvent.click(toggle());

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith(SET_ADVANCED_SETTINGS_ENABLED, {
        enabled: false,
      })
    );
    await waitFor(() =>
      expect(screen.getByTestId("section-general")).toBeInTheDocument()
    );
    expect(screen.queryByTestId("section-tools")).not.toBeInTheDocument();
    expect(sidebarButton("Tools")).not.toBeInTheDocument();
  });

  it("reverts the optimistic toggle when the backend rejects it", async () => {
    mockBackend(false);
    invokeMock.mockImplementation(async (cmd: string) => {
      if (cmd === GET_ADVANCED_SETTINGS_ENABLED) return false;
      if (cmd === SET_ADVANCED_SETTINGS_ENABLED) throw new Error("store down");
      return undefined;
    });
    render(<ModularSettingsWindow />);
    await waitFor(() => expect(toggle()).not.toBeDisabled());

    fireEvent.click(toggle());

    await waitFor(() =>
      expect(toggle()).toHaveAttribute("aria-checked", "false")
    );
    expect(sidebarButton("Tools")).not.toBeInTheDocument();
  });
});

describe("advanced markers on fields and sections", () => {
  function Fixture() {
    return (
      <AdvancedSettingsProvider>
        <SettingsSection title="Always shown">
          <SettingsField label="Basic field">
            <input aria-label="basic" />
          </SettingsField>
          <SettingsField label="Tuning field" advanced>
            <input aria-label="tuning" />
          </SettingsField>
          <AdvancedOnly>
            <p>wrapped block</p>
          </AdvancedOnly>
        </SettingsSection>
        <SettingsSection title="Power section" advanced>
          <p>power content</p>
        </SettingsSection>
      </AdvancedSettingsProvider>
    );
  }

  it("hides advanced fields, wrappers and sections in basic mode", async () => {
    mockBackend(false);
    render(<Fixture />);
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith(GET_ADVANCED_SETTINGS_ENABLED)
    );

    expect(screen.getByText("Always shown")).toBeInTheDocument();
    expect(screen.getByText("Basic field")).toBeInTheDocument();
    expect(screen.queryByText("Tuning field")).not.toBeInTheDocument();
    expect(screen.queryByText("wrapped block")).not.toBeInTheDocument();
    expect(screen.queryByText("Power section")).not.toBeInTheDocument();
  });

  it("shows everything once the persisted toggle is on", async () => {
    mockBackend(true);
    render(<Fixture />);

    await waitFor(() =>
      expect(screen.getByText("Power section")).toBeInTheDocument()
    );
    expect(screen.getByText("Tuning field")).toBeInTheDocument();
    expect(screen.getByText("wrapped block")).toBeInTheDocument();
  });

  it("shows everything when rendered outside a provider", () => {
    render(
      <SettingsSection title="Loose section" advanced>
        <SettingsField label="Loose field" advanced>
          <input aria-label="loose" />
        </SettingsField>
      </SettingsSection>
    );
    expect(screen.getByText("Loose section")).toBeInTheDocument();
    expect(screen.getByText("Loose field")).toBeInTheDocument();
  });
});

describe("GeneralSettings in basic mode", () => {
  const settingsStub = {
    soundEnabled: true,
    handleSoundEnabledChange: vi.fn(),
  } as any;

  it("keeps launch-at-login and sound effects, hides the power-user cards", async () => {
    mockBackend(false);
    render(
      <AdvancedSettingsProvider>
        <GeneralSettings settings={settingsStub} />
      </AdvancedSettingsProvider>
    );
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith(GET_ADVANCED_SETTINGS_ENABLED)
    );

    expect(screen.getByText("Launch at Login")).toBeInTheDocument();
    expect(screen.getByText("Enable Sound Effects")).toBeInTheDocument();
    for (const hidden of [
      "Bar Appearance",
      "Agent Mode",
      "Agent Trigger Mode",
      "Companion Mode",
      "Big Cursor",
      "Restart Onboarding Flow",
    ]) {
      expect(screen.queryByText(hidden)).not.toBeInTheDocument();
    }
  });

  it("shows the power-user cards with the toggle on", async () => {
    mockBackend(true);
    render(
      <AdvancedSettingsProvider>
        <GeneralSettings settings={settingsStub} />
      </AdvancedSettingsProvider>
    );

    await waitFor(() =>
      expect(screen.getByText("Bar Appearance")).toBeInTheDocument()
    );
    expect(screen.getByText("Launch at Login")).toBeInTheDocument();
    for (const shown of [
      "Agent Mode",
      "Agent Trigger Mode",
      "Companion Mode",
      "Big Cursor",
      "Restart Onboarding Flow",
    ]) {
      // Card titles and field labels can repeat the same text.
      expect(screen.getAllByText(shown).length).toBeGreaterThan(0);
    }
  });
});
