import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
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

function mockBackend(
  overrides: { smartRouting?: boolean; setFails?: boolean } = {},
) {
  invokeMock.mockImplementation(async (command: string) => {
    if (command === COMMANDS.SETTINGS_GET_AGENT_SETTINGS) {
      return { background_mode: true, mouse_control: "ask", dock_icon_visible: true };
    }
    if (command === COMMANDS.SETTINGS_GET_SMART_ROUTING_ENABLED) {
      return overrides.smartRouting ?? false;
    }
    if (command === COMMANDS.SETTINGS_SET_SMART_ROUTING_ENABLED && overrides.setFails) {
      throw new Error("not registered yet");
    }
    return undefined;
  });
}

const toggle = () => screen.getByLabelText(/Smart routing/);

async function mount() {
  render(
    <AdvancedSettingsProvider>
      <AdvancedSettings settings={settingsStub} />
    </AdvancedSettingsProvider>,
  );
  // Disabled until the mount-time GET resolves; wait for enabled.
  await waitFor(() => expect(toggle()).toBeEnabled());
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

async function openInfo(el: HTMLElement, text: RegExp) {
  const row = el.closest('[id^="settings-row-"]') as HTMLElement;
  await act(async () => {
    within(row).getByRole("button", { name: "More info" }).focus();
  });
  return (await screen.findAllByText(text))[0];
}

describe("Smart routing (beta) toggle", () => {
  it("is off unless the backend says otherwise", async () => {
    await mount();
    expect(toggle()).not.toBeChecked();
  });

  it("shows the backend's value when the flag is on", async () => {
    mockBackend({ smartRouting: true });
    await mount();
    await waitFor(() => expect(toggle()).toBeChecked());
  });

  it("sits in the Beta group and says what it does in one plain line", async () => {
    await mount();
    expect(screen.getByText("Beta")).toBeInTheDocument();
    const description = await openInfo(toggle(), /picks a model for each request/i);
    expect(description).toHaveTextContent(/use your computer/i);
    expect(description.textContent ?? "").not.toMatch(/\u2014/);
  });

  it("saves the flag through the backend", async () => {
    await mount();
    await click(toggle());
    expect(invokeMock).toHaveBeenCalledWith(
      COMMANDS.SETTINGS_SET_SMART_ROUTING_ENABLED,
      { enabled: true },
    );
  });

  it("puts the switch back when the backend refuses", async () => {
    mockBackend({ setFails: true });
    await mount();
    await click(toggle());
    await waitFor(() =>
      expect(toast.error).toHaveBeenCalledWith("Could not change that setting"),
    );
    expect(toggle()).not.toBeChecked();
  });
});
