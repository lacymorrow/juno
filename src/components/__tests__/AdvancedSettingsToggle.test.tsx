import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";

// Rendering the full settings window in jsdom is slow (Radix + many sections),
// and several tests re-render it end to end. Give the file more headroom than
// the 5s default so a loaded CI worker doesn't flake on the heavier cases.
vi.setConfig({ testTimeout: 20000 });

import ModularSettingsWindow, {
  settingsCategories,
  settingsRowIndex,
  visibleCategories,
  searchCategories,
  searchRows,
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
    theme: vi.fn(() => Promise.resolve("light")),
    onThemeChanged: vi.fn(() => Promise.resolve(() => {})),
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
    NotificationSettings: stub("notifications"),
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
  "Notifications",
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
      "notifications",
      "security",
      "shortcuts",
    ]);
  });

  it("returns every section in advanced mode", () => {
    expect(visibleCategories(true)).toEqual(settingsCategories);
  });
});

describe("searchCategories", () => {
  const all = settingsCategories;

  it("returns everything for an empty or whitespace query", () => {
    expect(searchCategories(all, "")).toEqual(all);
    expect(searchCategories(all, "   ")).toEqual(all);
  });

  it("matches on the visible name", () => {
    expect(searchCategories(all, "network").map((c) => c.id)).toEqual(["network"]);
  });

  it("matches on hidden keywords, not just the name", () => {
    // "microphone" appears only in keywords, for Voice and Security.
    const ids = searchCategories(all, "microphone").map((c) => c.id);
    expect(ids).toContain("voice");
    expect(ids).toContain("security");
    expect(ids).not.toContain("network");
  });

  it("requires every term to match (AND)", () => {
    expect(searchCategories(all, "mcp servers").map((c) => c.id)).toEqual(["network"]);
    expect(searchCategories(all, "mcp zzzz")).toEqual([]);
  });

  it("is case-insensitive", () => {
    expect(searchCategories(all, "KEYBOARD").map((c) => c.id)).toEqual(["shortcuts"]);
  });
});

describe("searchRows", () => {
  it("returns nothing for an empty or whitespace query", () => {
    expect(searchRows("")).toEqual([]);
    expect(searchRows("   ")).toEqual([]);
  });

  it("matches a row on its label or keywords and reports its section", () => {
    const hits = searchRows("temperature");
    expect(hits.map((r) => r.rowId)).toContain("temperature");
    expect(hits.every((r) => r.sectionId === "ai")).toBe(true);
  });

  it("requires every term to match (AND)", () => {
    expect(searchRows("reset factory").map((r) => r.rowId)).toEqual([
      "reset-all-settings",
    ]);
    expect(searchRows("reset zzzz")).toEqual([]);
  });

  it("is case-insensitive", () => {
    expect(searchRows("WHISPER").map((r) => r.rowId)).toContain("whisper-model");
  });

  it("indexes every row against a real section id", () => {
    const sectionIds = new Set(settingsCategories.map((c) => c.id));
    for (const row of settingsRowIndex) {
      expect(sectionIds.has(row.sectionId)).toBe(true);
    }
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

    expect(screen.getByText("Launch at login")).toBeInTheDocument();
    expect(screen.getByText("Sound effects")).toBeInTheDocument();
    for (const hidden of [
      "Bar appearance",
      "Agent mode",
      "Trigger mode",
      "Enable Companion Mode",
      "Enable big cursor",
      "Restart onboarding",
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
      expect(screen.getByText("Bar appearance")).toBeInTheDocument()
    );
    expect(screen.getByText("Launch at login")).toBeInTheDocument();
    for (const shown of [
      "Agent mode",
      "Trigger mode",
      "Enable Companion Mode",
      "Enable big cursor",
      "Restart onboarding",
    ]) {
      // Card titles and field labels can repeat the same text.
      expect(screen.getAllByText(shown).length).toBeGreaterThan(0);
    }
  });
});

describe("ModularSettingsWindow search", () => {
  it("filters the sidebar to matching sections and clears back to all", async () => {
    mockBackend(true);
    render(<ModularSettingsWindow />);
    await waitFor(() => expect(sidebarButton("Network")).toBeInTheDocument());

    const search = screen.getByLabelText("Search settings");
    fireEvent.change(search, { target: { value: "network" } });

    await waitFor(() => expect(sidebarButton("General")).not.toBeInTheDocument());
    expect(sidebarButton("Network")).toBeInTheDocument();

    fireEvent.change(search, { target: { value: "" } });
    await waitFor(() => expect(sidebarButton("General")).toBeInTheDocument());
  });

  it("shows an empty state when nothing matches", async () => {
    mockBackend(true);
    render(<ModularSettingsWindow />);
    await waitFor(() => expect(sidebarButton("General")).toBeInTheDocument());

    fireEvent.change(screen.getByLabelText("Search settings"), {
      target: { value: "zzzznomatch" },
    });

    await waitFor(() =>
      expect(screen.getByText("No settings found")).toBeInTheDocument()
    );
  });

  it("deep-links a row keyword to its section and keeps that section listed", async () => {
    mockBackend(true);
    render(<ModularSettingsWindow />);
    await waitFor(() => expect(sidebarButton("General")).toBeInTheDocument());

    // "temperature" only lives in a row index entry for the AI Provider section,
    // so a plain section search would miss it — deep-linking must select AI.
    fireEvent.change(screen.getByLabelText("Search settings"), {
      target: { value: "temperature" },
    });

    await waitFor(() =>
      expect(screen.getByTestId("section-ai")).toBeInTheDocument()
    );
    expect(sidebarButton("AI Provider")).toBeInTheDocument();
    expect(sidebarButton("General")).not.toBeInTheDocument();
  });

  it("deep-links a row in an advanced section", async () => {
    mockBackend(true);
    render(<ModularSettingsWindow />);
    await waitFor(() => expect(sidebarButton("Advanced")).toBeInTheDocument());

    fireEvent.change(screen.getByLabelText("Search settings"), {
      target: { value: "reset factory" },
    });

    await waitFor(() =>
      expect(screen.getByTestId("section-advanced")).toBeInTheDocument()
    );
    expect(sidebarButton("Advanced")).toBeInTheDocument();
  });
});
