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

const settingsStub = {
  performanceMonitoringEnabled: false,
  handlePerformanceMonitoringChange: vi.fn(),
  loadAllSettings: vi.fn(),
} as unknown as SettingsSectionProps["settings"];

function mockBackend(overrides: Record<string, unknown> = {}) {
  invokeMock.mockImplementation(async (command: string) => {
    if (command === COMMANDS.SETTINGS_GET_AGENT_SETTINGS) {
      return { background_mode: true, mouse_control: "ask", dock_icon_visible: true };
    }
    if (command === COMMANDS.SETTINGS_GET_CLI_PERSISTENT_SESSION_ENABLED) {
      return overrides.persistent ?? false;
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
  await waitFor(() =>
    expect(screen.getByLabelText(/Persistent Claude session/)).toBeInTheDocument(),
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

describe("Persistent Claude session (beta) toggle", () => {
  it("is off unless the backend says otherwise", async () => {
    await mount();
    expect(screen.getByLabelText(/Persistent Claude session/)).not.toBeChecked();
  });

  it("shows the backend's value when the flag is on", async () => {
    mockBackend({ persistent: true });
    await mount();
    await waitFor(() =>
      expect(screen.getByLabelText(/Persistent Claude session/)).toBeChecked(),
    );
  });

  it("tells the person it is beta and what the trade is", async () => {
    await mount();
    // "Beta" lives on the group heading, not repeated in the row subtext.
    expect(screen.getByText("Beta")).toBeInTheDocument();
    const description = screen.getByText(/keeps one Claude CLI process/i);
    expect(description).toHaveTextContent(/1\.6–3\.1s faster/);
    expect(description).toHaveTextContent(/stall|hang/i);
    expect(description).toHaveTextContent(/next conversation/i);
  });

  it("saves the flag through the backend", async () => {
    await mount();
    await click(screen.getByLabelText(/Persistent Claude session/));

    expect(invokeMock).toHaveBeenCalledWith(
      COMMANDS.SETTINGS_SET_CLI_PERSISTENT_SESSION_ENABLED,
      { enabled: true },
    );
  });

  it("puts the switch back when the backend refuses", async () => {
    invokeMock.mockImplementation(async (command: string) => {
      if (command === COMMANDS.SETTINGS_GET_AGENT_SETTINGS) {
        return { background_mode: true, mouse_control: "ask", dock_icon_visible: true };
      }
      if (command === COMMANDS.SETTINGS_GET_CLI_PERSISTENT_SESSION_ENABLED) {
        return false;
      }
      throw new Error("not registered yet");
    });
    await mount();

    await click(screen.getByLabelText(/Persistent Claude session/));

    await waitFor(() =>
      expect(screen.getByLabelText(/Persistent Claude session/)).not.toBeChecked(),
    );
    expect(toast.error).toHaveBeenCalledWith("Could not change that setting");
  });
});
