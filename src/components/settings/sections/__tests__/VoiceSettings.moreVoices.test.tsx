import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
  emit: vi.fn(),
}));
vi.mock("sonner", () => ({ toast: { success: vi.fn(), error: vi.fn() } }));

import { invoke } from "@tauri-apps/api/core";
import { toast } from "sonner";

import VoiceSettings from "../VoiceSettings";
import { AdvancedSettingsProvider } from "../../AdvancedSettingsContext";
import { COMMANDS } from "@/lib/constants.generated";
import type { SettingsSectionProps } from "../../types";
import type { JunoVoiceList } from "@/hooks/useSettings";

const invokeMock = vi.mocked(invoke);

/**
 * The voices Juno can speak with are the voices macOS has installed, and the
 * natural-sounding ones are downloads nobody has made yet. Juno cannot install
 * one; the person can, so the row that says so carries the control that does
 * it rather than a sentence about where to look.
 *
 * Whether that row appears is Rust's call, through `better_voices_available`.
 * Typed as `JunoVoiceList` on purpose: the rest of the stub is cast, and an
 * untyped list here once hid a wrong shape from `tsc` entirely.
 */
function voiceList(betterVoicesAvailable: boolean): JunoVoiceList {
  return {
    provider: "system",
    engine: "system",
    engine_label: "your Mac",
    options: [
      {
        id: "com.apple.voice.compact.en-US.Samantha",
        kind: "voice",
        name: "Samantha",
        descriptor: "American",
        selected: true,
        speaks: true,
      },
    ],
    note: null,
    better_voices_available: betterVoicesAvailable,
    engines: [{ id: "system", name: "Your Mac" }],
  };
}

function settingsStub(betterVoicesAvailable: boolean): SettingsSectionProps["settings"] {
  return {
    audioDevices: null,
    junoVoices: voiceList(betterVoicesAvailable),
    speakingVoiceId: null,
    captureFailure: null,
    loadAudioDevices: vi.fn(async () => {}),
    loadJunoVoices: vi.fn(async () => {}),
    handleAudioInputDeviceChange: vi.fn(async () => {}),
    handleAudioOutputDeviceChange: vi.fn(async () => {}),
    handleJunoVoiceChange: vi.fn(async () => {}),
    handlePreviewJunoVoice: vi.fn(async () => {}),
    dismissCaptureFailure: vi.fn(),
    ttsProvider: "system",
    dictationInsertionMode: "paste",
    dictationClipboardEnabled: false,
    livePartialTranscription: false,
    handleDictationInsertionModeChange: vi.fn(),
    handleDictationClipboardChange: vi.fn(),
    handleLivePartialTranscriptionChange: vi.fn(),
  } as unknown as SettingsSectionProps["settings"];
}

function renderSection(betterVoicesAvailable = true) {
  return render(
    <AdvancedSettingsProvider>
      <VoiceSettings settings={settingsStub(betterVoicesAvailable)} />
    </AdvancedSettingsProvider>,
  );
}

beforeEach(() => {
  invokeMock.mockReset();
  invokeMock.mockResolvedValue(undefined);
  vi.mocked(toast.error).mockClear();
});

describe("Juno's voice: a voice you do not have yet is a condition you can change", () => {
  it("offers one control that opens where voices are installed", async () => {
    renderSection();
    const button = await screen.findByRole("button", { name: "Add Voices" });
    fireEvent.click(button);
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith(COMMANDS.PERMISSIONS_OPEN_SYSTEM_PREFERENCES, {
        preferencePane: "spoken_content",
      }),
    );
  });

  it("is offered when every voice this Mac has is the compact one", async () => {
    renderSection(true);
    const button = await screen.findByRole("button", { name: "Add Voices" });
    expect(button).toBeEnabled();
  });

  it("is not offered when this Mac already has the good voices", async () => {
    renderSection(false);
    // Let the section settle before asserting an absence, or this passes
    // simply because nothing has rendered yet.
    await screen.findByText("Samantha");
    expect(screen.queryByRole("button", { name: "Add Voices" })).not.toBeInTheDocument();
  });

  it("says where to look by hand when the pane will not open", async () => {
    invokeMock.mockRejectedValue("nope");
    renderSection();
    fireEvent.click(await screen.findByRole("button", { name: "Add Voices" }));
    await waitFor(() =>
      expect(toast.error).toHaveBeenCalledWith(
        expect.stringContaining("Spoken Content"),
      ),
    );
  });

  it("never names the hardware anywhere on the screen", async () => {
    renderSection();
    await screen.findByRole("button", { name: "Add Voices" });
    for (const word of [/Apple Silicon/i, /\bIntel\b/, /arm64/, /x86_64/, /architecture/i]) {
      expect(screen.queryByText(word)).not.toBeInTheDocument();
    }
  });
});
