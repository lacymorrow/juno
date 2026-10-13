import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
  emit: vi.fn(),
}));
vi.mock("sonner", () => ({ toast: { success: vi.fn(), error: vi.fn() } }));
vi.mock("../../AppearancePicker", () => ({ AppearancePicker: () => null }));
vi.mock("../../useBarAppearance", () => ({
  useBarAppearance: () => ({ value: "pill", saving: false, change: vi.fn() }),
}));
vi.mock("../../UpdatesGroup", () => ({ UpdatesGroup: () => null }));
vi.mock("../../NotificationsGroup", () => ({ NotificationsGroup: () => null }));
vi.mock("../../CursorColorPicker", () => ({ CursorColorPicker: () => null }));

import { invoke } from "@tauri-apps/api/core";
import GeneralSettings from "../GeneralSettings";
import type { SettingsSectionProps } from "../../types";
import { COMMANDS } from "@/lib/constants.generated";

const invokeMock = vi.mocked(invoke);

function renderGeneral(beta: boolean) {
  invokeMock.mockImplementation(((command: string) =>
    Promise.resolve(
      command === COMMANDS.SETTINGS_GET_BETA_FEATURES ? beta : undefined,
    )) as typeof invoke);
  render(
    <GeneralSettings
      settings={{} as unknown as SettingsSectionProps["settings"]}
    />,
  );
}

beforeEach(() => {
  invokeMock.mockReset();
});

describe("General: Beta features", () => {
  it("is off by default and explains itself", async () => {
    renderGeneral(false);
    const toggle = await screen.findByRole("switch", { name: /beta features/i });
    expect(toggle).toHaveAttribute("aria-checked", "false");
    expect(
      screen.getByText("Try features that are still being finished."),
    ).toBeInTheDocument();
  });

  it("shows the saved value", async () => {
    renderGeneral(true);
    const toggle = await screen.findByRole("switch", { name: /beta features/i });
    await waitFor(() => expect(toggle).toHaveAttribute("aria-checked", "true"));
  });

  it("saves the choice when flipped", async () => {
    renderGeneral(false);
    fireEvent.click(await screen.findByRole("switch", { name: /beta features/i }));
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith(COMMANDS.SETTINGS_SET_BETA_FEATURES, {
        enabled: true,
      }),
    );
  });
});
