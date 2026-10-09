import { act, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

// The agent's `settings` tool points the Settings window at a pane and a row
// (Rust: agent/tools/settings_tool.rs). These pin the window's half: the pane
// switches, the row is scrolled to and tinted, a change redraws the pane, and
// the advanced toggle follows Rust.

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
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
vi.mock("sonner", () => ({ toast: { success: vi.fn(), error: vi.fn() } }));

const loadAllSettings = vi.fn(async () => {});
vi.mock("@/contexts/SettingsContext", () => ({
  useSettingsContext: () => ({ loadAllSettings }),
}));

// Counts mounts, so a remount (the redraw after an agent change) is visible.
const voiceMounts = vi.fn();

vi.mock("../index", async () => {
  const { useEffect } = await import("react");
  const stub = (id: string) => () => <div data-testid={`section-${id}`}>{id}</div>;
  function VoiceStub() {
    useEffect(() => {
      voiceMounts();
    }, []);
    return (
      <div data-testid="section-voice">
        <div id="settings-row-juno-voice">Juno's voice</div>
      </div>
    );
  }
  function SecurityStub() {
    return (
      <div data-testid="section-security">
        <div id="settings-row-permission-mode">When Juno needs permission</div>
      </div>
    );
  }
  return {
    GeneralSettings: stub("general"),
    TriggersSettings: stub("triggers"),
    VoiceSettings: VoiceStub,
    AIProviderSettings: stub("ai"),
    ModelsSettings: stub("models"),
    NotificationSettings: stub("notifications"),
    ToolsSettings: stub("tools"),
    AutomationsSettings: stub("automations"),
    NetworkSettings: stub("network"),
    SecuritySettings: SecurityStub,
    AdvancedSettings: stub("advanced"),
  };
});

import ModularSettingsWindow from "../ModularSettingsWindow";
import { ROW_FLASH_CLASSES, ROW_FLASH_MS, flashRow, revealRow } from "../revealRow";
import { COMMANDS, SETTINGS } from "@/lib/constants.generated";

const invokeMock = vi.mocked(invoke);
const listenMock = vi.mocked(listen);

/** Handlers the window registered, by event name. */
function handlers() {
  const map = new Map<string, (event: { payload: unknown }) => void>();
  for (const [name, handler] of listenMock.mock.calls) {
    map.set(name as string, handler as (event: { payload: unknown }) => void);
  }
  return map;
}

function backend(pending: unknown, advanced = false) {
  invokeMock.mockImplementation(async (cmd: string) => {
    if (cmd === COMMANDS.SETTINGS_TAKE_PENDING_SETTINGS_NAVIGATION) return pending;
    if (cmd === COMMANDS.SETTINGS_GET_ADVANCED_SETTINGS_ENABLED) return advanced;
    return undefined;
  });
}

beforeEach(() => {
  invokeMock.mockReset();
  listenMock.mockClear();
  loadAllSettings.mockClear();
  voiceMounts.mockClear();
  Element.prototype.scrollIntoView = vi.fn();
  vi.stubGlobal("requestAnimationFrame", (cb: FrameRequestCallback) =>
    setTimeout(() => cb(0), 0) as unknown as number,
  );
  vi.stubGlobal("cancelAnimationFrame", (id: number) => clearTimeout(id));
});

afterEach(() => {
  vi.unstubAllGlobals();
  vi.useRealTimers();
});

describe("revealRow", () => {
  it("scrolls the row into view and tints it, then takes the tint away", () => {
    vi.useFakeTimers();
    const row = document.createElement("div");
    row.id = "settings-row-sample";
    document.body.appendChild(row);

    revealRow("sample");
    vi.advanceTimersByTime(20); // one frame
    expect(row.scrollIntoView).toHaveBeenCalledWith(
      expect.objectContaining({ block: "center" }),
    );
    for (const c of ROW_FLASH_CLASSES) expect(row.classList.contains(c)).toBe(true);

    vi.advanceTimersByTime(ROW_FLASH_MS);
    for (const c of ROW_FLASH_CLASSES) expect(row.classList.contains(c)).toBe(false);
    row.remove();
  });

  it("waits for a row that has not rendered yet, and gives up quietly", () => {
    vi.useFakeTimers();
    revealRow("late");
    const row = document.createElement("div");
    row.id = "settings-row-late";
    vi.advanceTimersByTime(50);
    document.body.appendChild(row);
    vi.advanceTimersByTime(50);
    expect(row.classList.contains(ROW_FLASH_CLASSES[0])).toBe(true);
    row.remove();

    // Never appears: no throw, nothing left running past the wait.
    revealRow("never", 100);
    vi.advanceTimersByTime(500);
  });

  it("is flat: a tint, never a ring or a glow", () => {
    const el = document.createElement("div");
    flashRow(el, 10);
    expect(el.className).not.toMatch(/ring|shadow|glow|animate/);
  });
});

describe("the agent points the Settings window", () => {
  it("opens on the pane it was opened for and points at the row", async () => {
    backend({ pane: "voice", row: "juno-voice", reload: false });
    render(<ModularSettingsWindow />);

    expect(await screen.findByTestId("section-voice")).toBeTruthy();
    expect(screen.getByRole("heading", { name: "Audio" })).toBeTruthy();
    const row = document.getElementById("settings-row-juno-voice");
    await waitFor(() =>
      expect(row?.classList.contains(ROW_FLASH_CLASSES[0])).toBe(true),
    );
    expect(row?.scrollIntoView).toHaveBeenCalled();
  });

  it("follows an open window's navigate event", async () => {
    backend(null);
    render(<ModularSettingsWindow />);
    expect(await screen.findByTestId("section-general")).toBeTruthy();
    await waitFor(() => expect(handlers().has(SETTINGS.EVENTS_SETTINGS_NAVIGATE)).toBe(true));

    act(() => {
      handlers().get(SETTINGS.EVENTS_SETTINGS_NAVIGATE)?.({
        payload: { pane: "security", row: "permission-mode", reload: false },
      });
    });
    expect(await screen.findByTestId("section-security")).toBeTruthy();
    await waitFor(() =>
      expect(
        document
          .getElementById("settings-row-permission-mode")
          ?.classList.contains(ROW_FLASH_CLASSES[0]),
      ).toBe(true),
    );
  });

  it("redraws from Rust when the agent changed a setting", async () => {
    backend({ pane: "voice", row: null, reload: false });
    render(<ModularSettingsWindow />);
    expect(await screen.findByTestId("section-voice")).toBeTruthy();
    expect(voiceMounts).toHaveBeenCalledTimes(1);

    act(() => {
      handlers().get(SETTINGS.EVENTS_SETTINGS_NAVIGATE)?.({
        payload: { pane: "voice", row: "juno-voice", reload: true },
      });
    });
    await waitFor(() => expect(voiceMounts).toHaveBeenCalledTimes(2));
    expect(loadAllSettings).toHaveBeenCalled();
  });

  it("waits for advanced settings before opening an advanced pane", async () => {
    backend({ pane: "advanced", row: null, reload: false }, false);
    render(<ModularSettingsWindow />);
    expect(await screen.findByTestId("section-general")).toBeTruthy();
    expect(screen.queryByTestId("section-advanced")).toBeNull();

    // Rust turns advanced settings on and says so.
    await waitFor(() =>
      expect(handlers().has(SETTINGS.EVENTS_ADVANCED_SETTINGS_CHANGED)).toBe(true),
    );
    act(() => {
      handlers().get(SETTINGS.EVENTS_ADVANCED_SETTINGS_CHANGED)?.({ payload: true });
    });
    expect(await screen.findByTestId("section-advanced")).toBeTruthy();
  });
});
