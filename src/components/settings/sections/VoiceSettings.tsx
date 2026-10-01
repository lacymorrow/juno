import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import { useEffect } from "react";
import { SettingsSectionProps } from "../types";
import { SettingsGroup, SettingsRow } from "../ui";
import { VoicePicker } from "../VoicePicker";

// The insertion-mode row re-explains itself: its subtitle is the selected
// option's own one-line description.
const INSERTION_MODE_DESCRIPTIONS: Record<string, string> = {
  paste: "Pastes with Cmd+V. Most compatible.",
  clipboard_free: "Types the transcript directly. Never touches your clipboard.",
};

/**
 * "Follow the system" as a Select value. Radix needs a string, and the backend
 * wants null, so the two are translated at the edge rather than storing a
 * sentinel that would later look like a device name.
 */
const FOLLOW_SYSTEM = "__system__";

/** Engine names as a person would say them, for the one line that names one. */
const TTS_ENGINE_NAMES: Record<string, string> = {
  elevenlabs: "ElevenLabs",
  kokoro: "Kokoro",
  replicate: "Replicate",
  chatterbox: "Chatterbox",
  supertonic: "Supertonic",
};

/**
 * Audio: which microphone Juno hears you on, which speaker it answers from,
 * and which voice it answers in.
 *
 * The voice is here and the engine is under Providers on purpose. The engine
 * is plumbing; the voice is a preference, and it is the only one of the three
 * you can check by ear, which is why picking it plays it.
 */
export default function VoiceSettings({ settings }: SettingsSectionProps) {
  const {
    audioDevices,
    junoVoices,
    speakingVoiceId,
    captureFailure,
    loadAudioDevices,
    loadJunoVoices,
    handleAudioInputDeviceChange,
    handleAudioOutputDeviceChange,
    handleJunoVoiceChange,
    handlePreviewJunoVoice,
    dismissCaptureFailure,
  } = settings;

  useEffect(() => {
    void loadAudioDevices();
    void loadJunoVoices();
  }, [loadAudioDevices, loadJunoVoices]);

  const engineName = TTS_ENGINE_NAMES[settings.ttsProvider];
  const inputs = audioDevices?.inputs ?? [];
  const outputs = audioDevices?.outputs ?? [];

  // What the microphone row says underneath itself. In order of what the
  // person most needs to know: a choice that is not connected, then what is
  // actually being used.
  const microphoneNote = audioDevices?.missing_input
    ? `${audioDevices.missing_input} is not connected. Juno is using ${audioDevices.effective_input ?? "nothing"} instead.`
    : audioDevices?.effective_input
      ? `Juno hears you through ${audioDevices.effective_input}.`
      : "Juno cannot find a microphone.";

  const speakerNote = audioDevices?.missing_output
    ? `${audioDevices.missing_output} is not connected. Juno is using your Mac's output instead.`
    : undefined;

  return (
    <div className="space-y-6">
      <SettingsGroup title="Microphone" footer={microphoneNote}>
        {captureFailure && (
          <SettingsRow
            id="capture-failure"
            label="Juno is not listening"
            description={captureFailure.message}
          >
            <button
              type="button"
              onClick={dismissCaptureFailure}
              className="text-[12px] text-muted-foreground hover:text-foreground"
            >
              Dismiss
            </button>
          </SettingsRow>
        )}

        <SettingsRow htmlFor="audio-input-device" label="Listen through">
          <Select
            value={audioDevices?.chosen_input ?? FOLLOW_SYSTEM}
            onValueChange={(value) =>
              void handleAudioInputDeviceChange(value === FOLLOW_SYSTEM ? null : value)
            }
          >
            <SelectTrigger id="audio-input-device" className="w-[250px]">
              <SelectValue placeholder="Select a microphone" />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value={FOLLOW_SYSTEM}>Whatever my Mac is using</SelectItem>
              {inputs.map((device) => (
                <SelectItem key={device.name} value={device.name}>
                  {device.name}
                </SelectItem>
              ))}
              {/* A choice that has been unplugged stays selectable, so the
                  Select shows what was chosen rather than snapping to
                  something the person never picked. That it is not connected
                  is said once, underneath, not twice. */}
              {audioDevices?.missing_input && (
                <SelectItem value={audioDevices.missing_input}>
                  {audioDevices.missing_input}
                </SelectItem>
              )}
            </SelectContent>
          </Select>
        </SettingsRow>
      </SettingsGroup>

      <SettingsGroup
        title="Speaker"
        footer={
          speakerNote ??
          "Juno's own voice plays here. Cloud voices play through your Mac's output."
        }
      >
        <SettingsRow htmlFor="audio-output-device" label="Speak through">
          <Select
            value={audioDevices?.chosen_output ?? FOLLOW_SYSTEM}
            onValueChange={(value) =>
              void handleAudioOutputDeviceChange(value === FOLLOW_SYSTEM ? null : value)
            }
          >
            <SelectTrigger id="audio-output-device" className="w-[250px]">
              <SelectValue placeholder="Select a speaker" />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value={FOLLOW_SYSTEM}>Whatever my Mac is using</SelectItem>
              {outputs.map((device) => (
                <SelectItem key={device.name} value={device.name}>
                  {device.name}
                </SelectItem>
              ))}
              {audioDevices?.missing_output && (
                <SelectItem value={audioDevices.missing_output}>
                  {audioDevices.missing_output}
                </SelectItem>
              )}
            </SelectContent>
          </Select>
        </SettingsRow>
      </SettingsGroup>

      <SettingsGroup
        title="Juno's voice"
        footer="Pick one and you will hear it. Pick the one you are using to hear it again."
      >
        {engineName && (
          <SettingsRow
            description={`${engineName} is giving Juno her voice right now. Pick one of these to use a voice from your Mac instead.`}
          />
        )}
        <SettingsRow
          id="juno-voice"
          below={
            <VoicePicker
              options={junoVoices}
              onChange={(id) => void handleJunoVoiceChange(id)}
              onReplay={() => void handlePreviewJunoVoice()}
              speakingId={speakingVoiceId}
            />
          }
        />
      </SettingsGroup>

      <SettingsGroup
        title="After you finish speaking"
        advanced
        footer="Juno puts the words where your cursor is."
      >
        <SettingsRow
          htmlFor="dictation-insertion-mode"
          label="How the words get typed"
          description={
            INSERTION_MODE_DESCRIPTIONS[settings.dictationInsertionMode] ??
            INSERTION_MODE_DESCRIPTIONS.paste
          }
        >
          <Select
            value={settings.dictationInsertionMode}
            onValueChange={settings.handleDictationInsertionModeChange}
          >
            <SelectTrigger id="dictation-insertion-mode" className="w-[190px]">
              <SelectValue placeholder="Select insertion mode" />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="paste">Paste them</SelectItem>
              <SelectItem value="clipboard_free">Type them</SelectItem>
            </SelectContent>
          </Select>
        </SettingsRow>

        <SettingsRow
          htmlFor="dictation-clipboard"
          label="Keep a copy on the clipboard"
          description="Paste the last dictation again anywhere with ⌘V."
        >
          <Switch
            id="dictation-clipboard"
            checked={settings.dictationClipboardEnabled}
            onCheckedChange={settings.handleDictationClipboardChange}
          />
        </SettingsRow>

        <SettingsRow
          advanced
          htmlFor="live-partial-transcription"
          label="Show words as I speak"
          description="Provisional text appears in the bar. Only the final result is typed."
        >
          <Switch
            id="live-partial-transcription"
            checked={settings.livePartialTranscription}
            onCheckedChange={settings.handleLivePartialTranscriptionChange}
          />
        </SettingsRow>
      </SettingsGroup>
    </div>
  );
}
