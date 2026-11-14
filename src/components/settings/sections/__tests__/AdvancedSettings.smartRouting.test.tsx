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

/**
 * Beta features defaults to on here: smart routing is a beta row, so the rest
 * of its behaviour is only reachable with the switch on. The gate itself is
 * covered by passing `beta: false`.
 */
function mockBackend(
  overrides: { smartRouting?: boolean; setFails?: boolean; beta?: boolean } = {},
) {
  invokeMock.mockImplementation(async (command: string) => {
    if (command === COMMANDS.SETTINGS_GET_BETA_FEATURES) {
      return overrides.beta ?? true;
    }
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
const findToggle = () => screen.queryByLabelText(/Smart routing/);
const betaSwitch = () => screen.findByRole("switch", { name: /beta features/i });
const rowOf = (el: HTMLElement) =>
  el.closest('[id^="settings-row-"]') as HTMLElement;

function renderSection() {
  render(
    <AdvancedSettingsProvider>
      <AdvancedSettings settings={settingsStub} />
    </AdvancedSettingsProvider>,
  );
}

async function mount() {
  renderSection();
  // The row arrives with the beta answer, then enables with the flag's GET.
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
  const row = rowOf(el);
  await act(async () => {
    within(row).getByRole("button", { name: "More info" }).focus();
  });
  return (await screen.findAllByText(text))[0];
}

describe("Smart routing is behind the Beta features switch", () => {
  it("stays hidden while Beta features is off", async () => {
    mockBackend({ beta: false });
    renderSection();
    // The master switch is the group's only row until beta is on.
    await betaSwitch();
    expect(findToggle()).not.toBeInTheDocument();
  });

  it("stays hidden with Beta features off even when the flag is saved on", async () => {
    mockBackend({ beta: false, smartRouting: true });
    renderSection();
    await betaSwitch();
    expect(findToggle()).not.toBeInTheDocument();
  });

  it("is shown once Beta features is on", async () => {
    mockBackend({ beta: true });
    renderSection();
    expect(await screen.findByLabelText(/Smart routing/)).toBeInTheDocument();
  });

  it("appears as soon as the Beta features switch is turned on", async () => {
    mockBackend({ beta: false });
    renderSection();
    const master = await betaSwitch();
    expect(findToggle()).not.toBeInTheDocument();
    await click(master);
    await waitFor(() => expect(toggle()).toBeInTheDocument());
  });

  it("carries the Beta badge, so the switch says what it turned on", async () => {
    await mount();
    expect(within(rowOf(toggle())).getByText("Beta")).toBeInTheDocument();
  });

  it("sits in the Beta group, beside the switch that unlocks it", async () => {
    await mount();
    const group = rowOf(toggle()).closest("section");
    expect(group).not.toBeNull();
    // The group heading, not the row's badge: both read "Beta".
    expect(
      within(group as HTMLElement).getByRole("heading", { name: "Beta" }),
    ).toBeInTheDocument();
    expect(
      within(group as HTMLElement).getByRole("switch", { name: /beta features/i }),
    ).toBeInTheDocument();
  });
});

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

  it("says what it does in one plain line", async () => {
    await mount();
    const description = await openInfo(toggle(), /picks a model for each request/i);
    expect(description).toHaveTextContent(/use your computer/i);
    expect(description.textContent ?? "").not.toMatch(/—/);
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
