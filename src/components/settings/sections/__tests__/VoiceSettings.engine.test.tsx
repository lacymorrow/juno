import { render, screen, within } from "@testing-library/react";
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
import { COMMANDS } from "@/lib/constants.generated";
import type { SettingsSectionProps } from "../../types";
import type { JunoVoiceList } from "@/hooks/useSettings";

const invokeMock = vi.mocked(invoke);

const ENGINES = [
  { id: "system", name: "Your Mac" },
  { id: "kokoro", name: "Kokoro" },
  { id: "elevenlabs", name: "ElevenLabs" },
];

function kokoroList(): JunoVoiceList {
  return {
    provider: "kokoro",
    engine: "kokoro",
    engine_label: "Kokoro",
    note: null,
    better_voices_available: false,
    engines: ENGINES,
    options: [
      {
        id: "silent",
        kind: "silent",
        name: "Silent",
        descriptor: "Juno writes the answer and never says it out loud.",
        selected: false,
        speaks: false,
      },
      {
        id: "af_heart",
        kind: "voice",
        name: "Heart",
        descriptor: "American English, female.",
        selected: true,
        speaks: true,
      },
    ],
  };
}

function elsewhereList(): JunoVoiceList {
  return {
    ...kokoroList(),
    provider: "elevenlabs",
    engine: "elevenlabs",
    engine_label: "ElevenLabs",
    note: "ElevenLabs speaks with the voice set on your ElevenLabs account, not here.",
    options: kokoroList().options.slice(0, 1),
  };
}

function settingsStub(list: JunoVoiceList): SettingsSectionProps["settings"] {
  return {
    audioDevices: null,
    junoVoices: list,
    voiceAudition: null,
    captureFailure: null,
    loadAudioDevices: vi.fn(async () => {}),
    loadJunoVoices: vi.fn(async () => {}),
    handleAudioInputDeviceChange: vi.fn(async () => {}),
    handleAudioOutputDeviceChange: vi.fn(async () => {}),
    handleJunoVoiceChange: vi.fn(async () => {}),
    handlePreviewJunoVoice: vi.fn(async () => {}),
    handleTtsProviderChange: vi.fn(async () => {}),
    dismissCaptureFailure: vi.fn(),
    ttsProvider: list.provider,
    dictationInsertionMode: "paste",
    dictationClipboardEnabled: false,
    livePartialTranscription: false,
    handleDictationInsertionModeChange: vi.fn(),
    handleDictationClipboardChange: vi.fn(),
    handleLivePartialTranscriptionChange: vi.fn(),
  } as unknown as SettingsSectionProps["settings"];
}

function renderSection(list: JunoVoiceList, advanced: boolean) {
  invokeMock.mockImplementation(((command: string) =>
    Promise.resolve(
      command === COMMANDS.SETTINGS_GET_ADVANCED_SETTINGS_ENABLED ? advanced : undefined,
    )) as typeof invoke);
  return render(
    <AdvancedSettingsProvider>
      <VoiceSettings settings={settingsStub(list)} />
    </AdvancedSettingsProvider>,
  );
}

beforeEach(() => {
  invokeMock.mockReset();
});

describe("Juno's voice: one list, with its engine right above it", () => {
  it("keeps the engine out of sight until Advanced is on", async () => {
    renderSection(kokoroList(), false);
    await screen.findByText("Heart");
    expect(screen.queryByText("Engine")).not.toBeInTheDocument();
  });

  it("shows the engine the rows belong to, above them, with Advanced on", async () => {
    renderSection(kokoroList(), true);
    const label = await screen.findByText("Engine");
    const row = label.closest("[id]") as HTMLElement;
    expect(within(row).getByRole("combobox")).toHaveTextContent("Kokoro");
  });

  it("draws only the engine's own voices, never the Mac's", async () => {
    renderSection(kokoroList(), false);
    await screen.findByText("Heart");
    for (const macVoice of ["Samantha", "Daniel", "Karen", "Moira"]) {
      expect(screen.queryByText(macVoice)).not.toBeInTheDocument();
    }
  });

  it("does not invite a pick when the engine's voices are chosen elsewhere", async () => {
    renderSection(elsewhereList(), false);
    await screen.findByText(/voice set on your ElevenLabs account/);
    expect(screen.queryByText("Pick one and you will hear it.")).not.toBeInTheDocument();
  });

  it("invites a pick, once, when there are voices to pick", async () => {
    renderSection(kokoroList(), false);
    expect(await screen.findAllByText("Pick one and you will hear it.")).toHaveLength(1);
  });
});
