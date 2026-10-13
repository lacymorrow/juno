import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
  emit: vi.fn(),
}));
vi.mock("sonner", () => {
  const toast = vi.fn() as unknown as { success: unknown; error: unknown };
  toast.success = vi.fn();
  toast.error = vi.fn();
  return { toast };
});

import { invoke } from "@tauri-apps/api/core";
import AdvancedSettings from "../AdvancedSettings";
import GeneralSettings from "../GeneralSettings";
import { AdvancedSettingsProvider } from "../../AdvancedSettingsContext";
import type { SettingsSectionProps } from "../../types";
import { COMMANDS } from "@/lib/constants.generated";

vi.mock("../../AppearancePicker", () => ({ AppearancePicker: () => null }));
vi.mock("../../useBarAppearance", () => ({
  useBarAppearance: () => ({ value: "pill", saving: false, change: vi.fn() }),
}));
vi.mock("../../UpdatesGroup", () => ({ UpdatesGroup: () => null }));
vi.mock("../../NotificationsGroup", () => ({ NotificationsGroup: () => null }));
vi.mock("../../CursorColorPicker", () => ({ CursorColorPicker: () => null }));

const invokeMock = vi.mocked(invoke);

const settingsStub = {
  performanceMonitoringEnabled: false,
  handlePerformanceMonitoringChange: vi.fn(),
  loadAllSettings: vi.fn(),
} as unknown as SettingsSectionProps["settings"];

function mockBackend(beta: boolean) {
  invokeMock.mockImplementation((async (command: string) => {
    if (command === COMMANDS.SETTINGS_GET_BETA_FEATURES) return beta;
    if (command === COMMANDS.SETTINGS_GET_AGENT_SETTINGS) {
      return { background_mode: true, mouse_control: "ask", dock_icon_visible: true };
    }
    return undefined;
  }) as typeof invoke);
}

function renderAdvanced() {
  render(
    <AdvancedSettingsProvider>
      <AdvancedSettings settings={settingsStub} />
    </AdvancedSettingsProvider>,
  );
}

beforeEach(() => {
  invokeMock.mockReset();
});

describe("Advanced: Beta features", () => {
  it("is off by default and says where to find what it turns on", async () => {
    mockBackend(false);
    renderAdvanced();
    const toggle = await screen.findByRole("switch", { name: /beta features/i });
    expect(toggle).toHaveAttribute("aria-checked", "false");
    expect(
      screen.getByText("Try features that are still being finished. Each one is marked Beta."),
    ).toBeInTheDocument();
  });

  it("shows the saved value", async () => {
    mockBackend(true);
    renderAdvanced();
    const toggle = await screen.findByRole("switch", { name: /beta features/i });
    await waitFor(() => expect(toggle).toHaveAttribute("aria-checked", "true"));
  });

  it("saves the choice when flipped", async () => {
    mockBackend(false);
    renderAdvanced();
    fireEvent.click(await screen.findByRole("switch", { name: /beta features/i }));
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith(COMMANDS.SETTINGS_SET_BETA_FEATURES, {
        enabled: true,
      }),
    );
  });

  it("is no longer in General", async () => {
    mockBackend(false);
    render(<GeneralSettings settings={settingsStub} />);
    await screen.findByText("Open at login");
    expect(screen.queryByRole("switch", { name: /beta features/i })).not.toBeInTheDocument();
  });
});
