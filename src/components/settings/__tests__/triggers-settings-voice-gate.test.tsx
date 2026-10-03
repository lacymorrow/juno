import { render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const invoke = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({ invoke: (...a: unknown[]) => invoke(...a) }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
  emit: vi.fn(),
}));
vi.mock("sonner", () => ({ toast: { error: vi.fn(), success: vi.fn() } }));

import { COMMANDS } from "@/lib/constants.generated";
import TriggersSettings from "@/components/settings/sections/TriggersSettings";
import { AdvancedSettingsProvider } from "@/components/settings/AdvancedSettingsContext";
import type { SettingsSectionProps } from "@/components/settings/types";

const STORED = [
  {
    id: "row-hold",
    gesture: "hold",
    target: "agent",
    binding: { kind: "keyboard", shortcut: "Fn" },
    phrase: null,
    require_hey_prefix: false,
    enabled: true,
  },
  {
    id: "row-say",
    gesture: "say",
    target: "agent",
    binding: null,
    phrase: "juno",
    require_hey_prefix: false,
    enabled: true,
  },
];

function mountBackend(advanced: boolean) {
  invoke.mockImplementation((command: string) => {
    switch (command) {
      case COMMANDS.SETTINGS_GET_ADVANCED_SETTINGS_ENABLED:
        return Promise.resolve(advanced);
      case COMMANDS.TRIGGERS_GET_TRIGGERS:
        return Promise.resolve(STORED);
      case COMMANDS.TRIGGERS_GET_TRIGGER_ISSUES:
        return Promise.resolve([]);
      default:
        return Promise.resolve(null);
    }
  });
}

const settings = { alwaysListeningActive: false } as unknown as SettingsSectionProps["settings"];

const renderScreen = () =>
  render(
    <AdvancedSettingsProvider>
      <TriggersSettings settings={settings} />
    </AdvancedSettingsProvider>,
  );

describe("wake phrases are experimental and live behind advanced settings", () => {
  beforeEach(() => invoke.mockReset());

  it("does not draw a saved Say row, or mention voice, while advanced is off", async () => {
    mountBackend(false);
    renderScreen();

    await screen.findByLabelText("Enable Hold to talk to Juno");
    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith(COMMANDS.SETTINGS_GET_ADVANCED_SETTINGS_ENABLED),
    );
    expect(screen.queryByLabelText("Enable Say to talk to Juno")).toBeNull();
    expect(screen.queryByLabelText("Wake phrase")).toBeNull();
    expect(screen.queryByText(/your voice/)).toBeNull();
  });

  it("shows the Say row with an experimental note once advanced is on", async () => {
    mountBackend(true);
    renderScreen();

    expect(await screen.findByLabelText("Enable Say to talk to Juno")).toBeTruthy();
    expect(screen.getByText(/Experimental\./)).toBeTruthy();
  });
});
