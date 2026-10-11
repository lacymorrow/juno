import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
  emit: vi.fn(),
}));
vi.mock("sonner", () => ({ toast: { success: vi.fn(), error: vi.fn() } }));

import { invoke } from "@tauri-apps/api/core";
import VoiceSettings from "../VoiceSettings";
import { AdvancedSettingsProvider } from "../../AdvancedSettingsContext";
import type { SettingsSectionProps } from "../../types";
import type { JunoVoiceList } from "@/hooks/useSettings";
import { COMMANDS } from "@/lib/constants.generated";

const invokeMock = vi.mocked(invoke);

class ResizeObserverStub {
  observe() {}
  unobserve() {}
  disconnect() {}
}
vi.stubGlobal("ResizeObserver", ResizeObserverStub);

function list(overrides: Partial<JunoVoiceList> = {}): JunoVoiceList {
  return {
    provider: "system",
    silent: false,
    engine: "system",
    engine_label: "Your Mac",
    note: null,
    engines: [{ id: "system", name: "Your Mac" }],
    options: [
      {
        id: "Samantha",
        kind: "voice",
        name: "Samantha",
        descriptor: "American English.",
        selected: true,
        speaks: true,
      },
    ],
    speed: { value: 1, min: 0.75, max: 2 },
    ...overrides,
  };
}

function renderSection(
  voices: JunoVoiceList,
  handleJunoVoiceRateChange = vi.fn(async (_rate: number) => {}),
  debug = true,
) {
  invokeMock.mockImplementation(((command: string) =>
    Promise.resolve(
      command === "get_debug_mode"
        ? debug
        : command === COMMANDS.SETTINGS_GET_ADVANCED_SETTINGS_ENABLED,
    )) as typeof invoke);
  const settings = {
    audioDevices: null,
    junoVoices: voices,
    voiceAudition: null,
    captureFailure: null,
    loadAudioDevices: vi.fn(async () => {}),
    loadJunoVoices: vi.fn(async () => {}),
    handleAudioInputDeviceChange: vi.fn(async () => {}),
    handleAudioOutputDeviceChange: vi.fn(async () => {}),
    handleJunoVoiceChange: vi.fn(async () => {}),
    handlePreviewJunoVoice: vi.fn(async () => {}),
    handleJunoVoiceRateChange,
    handleTtsProviderChange: vi.fn(async () => {}),
    dismissCaptureFailure: vi.fn(),
    ttsProvider: voices.provider,
    dictationInsertionMode: "paste",
    dictationClipboardEnabled: false,
    livePartialTranscription: false,
    handleDictationInsertionModeChange: vi.fn(),
    handleDictationClipboardChange: vi.fn(),
    handleLivePartialTranscriptionChange: vi.fn(),
  } as unknown as SettingsSectionProps["settings"];
  render(
    <AdvancedSettingsProvider>
      <VoiceSettings settings={settings} />
    </AdvancedSettingsProvider>,
  );
  return { handleJunoVoiceRateChange, loadJunoVoices: settings.loadJunoVoices };
}

beforeEach(() => {
  invokeMock.mockReset();
});

describe("Audio: how fast Juno speaks", () => {
  it("shows the speed beside the voice, with the rate in force", async () => {
    renderSection(list({ speed: { value: 1.5, min: 0.75, max: 2 } }));
    expect(await screen.findByText("Speaking speed")).toBeInTheDocument();
    expect(screen.getByText("How fast Juno talks.")).toBeInTheDocument();
    expect(screen.getByText("1.5x")).toBeInTheDocument();
    expect(screen.getByRole("slider")).toHaveAttribute("aria-valuemin", "0.75");
    expect(screen.getByRole("slider")).toHaveAttribute("aria-valuemax", "2");
  });

  it("reads the voices again when the window regains focus", async () => {
    const { loadJunoVoices } = renderSection(list());
    await screen.findByText("Samantha");
    const before = vi.mocked(loadJunoVoices).mock.calls.length;
    fireEvent.focus(window);
    expect(vi.mocked(loadJunoVoices).mock.calls.length).toBe(before + 1);
  });

  it("hides the speed unless debug mode is on", async () => {
    renderSection(list(), undefined, false);
    await screen.findByText("Samantha");
    expect(screen.queryByText("Speaking speed")).not.toBeInTheDocument();
    expect(screen.queryByRole("slider")).not.toBeInTheDocument();
  });

  it("draws no speed control when the engine has none", async () => {
    renderSection(list({ speed: null }));
    await screen.findByText("Samantha");
    expect(screen.queryByText("Speaking speed")).not.toBeInTheDocument();
    expect(screen.queryByRole("slider")).not.toBeInTheDocument();
  });

  it("draws no speed control when the answer carries no speed at all", async () => {
    renderSection(list({ speed: undefined }));
    await screen.findByText("Samantha");
    expect(screen.queryByRole("slider")).not.toBeInTheDocument();
  });

  it("sends the new rate once, when the slider is moved", async () => {
    const { handleJunoVoiceRateChange } = renderSection(list());
    const slider = await screen.findByRole("slider");
    slider.focus();
    fireEvent.keyDown(slider, { key: "ArrowRight" });
    await waitFor(() => expect(handleJunoVoiceRateChange).toHaveBeenCalledTimes(1));
    expect(handleJunoVoiceRateChange).toHaveBeenCalledWith(1.05);
  });
});
