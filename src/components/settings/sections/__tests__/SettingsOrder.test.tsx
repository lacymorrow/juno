import { render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
  emit: vi.fn(),
}));
vi.mock("sonner", () => ({ toast: { success: vi.fn(), error: vi.fn() } }));
vi.mock("@/lib/permissions-service", () => ({
  getPermissionsStatus: vi.fn(async () => ({ all_granted: true })),
  invalidatePermissionsCache: vi.fn(),
}));
vi.mock("../../UpdatesGroup", () => ({
  UpdatesGroup: () => <h3>Updates</h3>,
}));
vi.mock("../../AppearancePicker", () => ({
  AppearancePicker: () => <div />,
}));
vi.mock("../../useBarAppearance", () => ({
  useBarAppearance: () => ({ value: "avatar", saving: false, change: vi.fn() }),
}));

import { invoke } from "@tauri-apps/api/core";
import GeneralSettings from "../GeneralSettings";
import VoiceSettings from "../VoiceSettings";
import SecuritySettings from "../SecuritySettings";
import { AgentModeGroup } from "../../AgentModeGroup";
import { AdvancedSettingsProvider } from "../../AdvancedSettingsContext";
import { COMMANDS } from "@/lib/constants.generated";
import { settingsRowIndex } from "../../ModularSettingsWindow";
import type { SettingsSectionProps } from "../../types";

function order(text: string, names: string[]): string[] {
  return names
    .map((name) => ({ name, at: text.indexOf(name) }))
    .filter((entry) => entry.at >= 0)
    .sort((a, b) => a.at - b.at)
    .map((entry) => entry.name);
}

function backend(advanced: boolean) {
  vi.mocked(invoke).mockImplementation(async (command: string) => {
    if (command === COMMANDS.SETTINGS_GET_ADVANCED_SETTINGS_ENABLED) return advanced;
    if (command === COMMANDS.SETTINGS_GET_CLI_ASK_BEFORE_SEND_ENABLED) return true;
    if (command === COMMANDS.TOOLS_GET_PERMISSION_MODE) return "ask_when_risky";
    return undefined;
  });
}

const voiceSettings = {
  audioDevices: null,
  junoVoices: null,
  voiceAudition: null,
  captureFailure: null,
  loadAudioDevices: vi.fn(async () => {}),
  loadJunoVoices: vi.fn(async () => {}),
  handleAudioInputDeviceChange: vi.fn(),
  handleAudioOutputDeviceChange: vi.fn(),
  handleJunoVoiceChange: vi.fn(),
  handlePreviewJunoVoice: vi.fn(),
  handleTtsProviderChange: vi.fn(),
  dismissCaptureFailure: vi.fn(),
  ttsProvider: "system",
  dictationInsertionMode: "paste",
  dictationClipboardEnabled: false,
  livePartialTranscription: false,
  soundEnabled: true,
} as unknown as SettingsSectionProps["settings"];

describe("settings order and labels", () => {
  it("General: Appearance, then Startup, Notifications, then Updates", async () => {
    backend(false);
    const { container } = render(
      <AdvancedSettingsProvider>
        <GeneralSettings settings={{} as SettingsSectionProps["settings"]} />
      </AdvancedSettingsProvider>,
    );
    await screen.findByText("Open at login");
    expect(
      order(container.textContent ?? "", ["Appearance", "Startup", "Notifications", "Updates"]),
    ).toEqual(["Appearance", "Startup", "Notifications", "Updates"]);
  });

  it("Audio: Don't speak, then Sound, Microphone, Speaker; Juno's voice only with Advanced", async () => {
    backend(false);
    const { container } = render(
      <AdvancedSettingsProvider>
        <VoiceSettings settings={voiceSettings} />
      </AdvancedSettingsProvider>,
    );
    await screen.findByText("Play sounds");
    const text = container.textContent ?? "";
    expect(text).not.toContain("Juno's voice");
    expect(order(text, ["Don't speak", "Sound", "Microphone", "Speaker"])).toEqual([
      "Don't speak",
      "Sound",
      "Microphone",
      "Speaker",
    ]);
    expect(text).not.toContain("Whatever my Mac");
  });

  it("Agent mode lists Single first, with no Recommended tag", () => {
    const { container } = render(
      <AgentModeGroup
        settings={
          {
            agentMode: "single",
            handleAgentModeChange: vi.fn(),
          } as unknown as SettingsSectionProps["settings"]
        }
      />,
    );
    expect(container.textContent).toContain("Single Agent");
    expect(container.textContent).not.toContain("Recommended");
  });

  it("Approvals show on Security & Privacy only with advanced on", async () => {
    backend(false);
    const off = render(
      <AdvancedSettingsProvider>
        <SecuritySettings />
      </AdvancedSettingsProvider>,
    );
    await screen.findByText("When Juno needs permission");
    expect(screen.queryByText("Ask before Juno sends")).toBeNull();
    off.unmount();

    backend(true);
    render(
      <AdvancedSettingsProvider>
        <SecuritySettings />
      </AdvancedSettingsProvider>,
    );
    await waitFor(() => expect(screen.getByText("Ask before Juno sends")).toBeTruthy());
  });

  it("search points the approvals row at Security & Privacy, advanced", () => {
    const entry = settingsRowIndex.find((r) => r.rowId === "cli-ask-before-send");
    expect(entry?.sectionId).toBe("security");
    expect(entry?.advanced).toBe(true);
  });
});
