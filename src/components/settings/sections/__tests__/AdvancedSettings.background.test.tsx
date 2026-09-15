import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
  emit: vi.fn(),
}));

const toastFn = vi.fn();
vi.mock("sonner", () => {
  const toast = (...args: unknown[]) => toastFn(...args);
  toast.success = vi.fn();
  toast.error = vi.fn();
  return { toast };
});

import { invoke } from "@tauri-apps/api/core";
import { toast } from "sonner";

import AdvancedSettings from "../AdvancedSettings";
import { AdvancedSettingsProvider } from "../../AdvancedSettingsContext";
import type { SettingsSectionProps } from "../../types";
import { COMMANDS } from "@/lib/constants.generated";

const invokeMock = vi.mocked(invoke);

/** The section only reads a couple of flags off the settings context. */
const settingsStub = {
  performanceMonitoringEnabled: false,
  handlePerformanceMonitoringChange: vi.fn(),
  loadAllSettings: vi.fn(),
} as unknown as SettingsSectionProps["settings"];

const AGENT_SETTINGS = {
  background_mode: true,
  mouse_control: "ask",
  dock_icon_visible: true,
};

function mockBackend(overrides: Record<string, unknown> = {}) {
  invokeMock.mockImplementation(async (command: string) => {
    if (command === COMMANDS.SETTINGS_GET_AGENT_SETTINGS) {
      return { ...AGENT_SETTINGS, ...overrides };
    }
    return undefined;
  });
}

async function mount() {
  render(
    <AdvancedSettingsProvider>
      <AdvancedSettings settings={settingsStub} />
    </AdvancedSettingsProvider>,
  );
  // The section loads its three settings on mount.
  await waitFor(() =>
    expect(screen.getByLabelText("Work in the background")).toBeEnabled(),
  );
}

const click = async (element: HTMLElement) => {
  await act(async () => {
    fireEvent.click(element);
    await Promise.resolve();
  });
};

beforeEach(() => {
  invokeMock.mockReset();
  toastFn.mockReset();
  vi.mocked(toast.error).mockReset();
  mockBackend();
});

describe("Background settings", () => {
  it("shows what the backend says, not a guess", async () => {
    mockBackend({ background_mode: false, mouse_control: "always", dock_icon_visible: false });
    await mount();

    expect(screen.getByLabelText("Work in the background")).not.toBeChecked();
    expect(screen.getByLabelText("Show in Dock")).not.toBeChecked();
    expect(screen.getByRole("radio", { name: "Always allow" })).toHaveAttribute(
      "aria-checked",
      "true",
    );
  });

  it("locks the controls and says so when the settings will not load", async () => {
    invokeMock.mockRejectedValue(new Error("no backend"));
    render(
      <AdvancedSettingsProvider>
        <AdvancedSettings settings={settingsStub} />
      </AdvancedSettingsProvider>,
    );

    await waitFor(() =>
      expect(screen.getByLabelText("Work in the background")).toBeDisabled(),
    );
    expect(
      screen.getByText(/These settings could not be loaded/),
    ).toBeInTheDocument();
  });

  it("saves the background mode toggle", async () => {
    await mount();
    await click(screen.getByLabelText("Work in the background"));

    expect(invokeMock).toHaveBeenCalledWith(
      COMMANDS.INPUT_CONTROL_SET_BACKGROUND_MODE,
      { enabled: false },
    );
  });

  it("offers exactly two mouse control choices, asking by default", async () => {
    await mount();

    const options = screen.getAllByRole("radio");
    expect(options.map((o) => o.textContent)).toEqual([
      "Ask each time",
      "Always allow",
    ]);
    expect(options[0]).toHaveAttribute("aria-checked", "true");

    await click(options[1]);
    expect(invokeMock).toHaveBeenCalledWith(
      COMMANDS.INPUT_CONTROL_SET_MOUSE_CONTROL,
      { mode: "always" },
    );
  });

  it("puts the whole way back into the Show in Dock description", async () => {
    await mount();

    const description = screen.getByText(/lives only in the menu bar/);
    expect(description).toHaveTextContent("click the Juno icon in the menu bar");
    expect(description).toHaveTextContent("Applications folder");
  });

  it("says where Juno went the moment the Dock icon is turned off", async () => {
    await mount();
    await click(screen.getByLabelText("Show in Dock"));

    expect(invokeMock).toHaveBeenCalledWith(
      COMMANDS.INPUT_CONTROL_SET_DOCK_ICON_VISIBLE,
      { visible: false },
    );
    expect(toastFn).toHaveBeenCalledWith(
      "Juno is now in the menu bar only",
      expect.objectContaining({
        description: expect.stringContaining("menu bar"),
      }),
    );
  });

  it("stays quiet when the Dock icon comes back", async () => {
    mockBackend({ dock_icon_visible: false });
    await mount();
    await click(screen.getByLabelText("Show in Dock"));

    expect(invokeMock).toHaveBeenCalledWith(
      COMMANDS.INPUT_CONTROL_SET_DOCK_ICON_VISIBLE,
      { visible: true },
    );
    expect(toastFn).not.toHaveBeenCalled();
  });

  it("puts the switch back when the backend refuses", async () => {
    invokeMock.mockImplementation(async (command: string) => {
      if (command === COMMANDS.SETTINGS_GET_AGENT_SETTINGS) return AGENT_SETTINGS;
      throw new Error("not registered yet");
    });
    await mount();

    await click(screen.getByLabelText("Show in Dock"));

    await waitFor(() =>
      expect(screen.getByLabelText("Show in Dock")).toBeChecked(),
    );
    expect(toast.error).toHaveBeenCalledWith("Could not change that setting");
  });
});
