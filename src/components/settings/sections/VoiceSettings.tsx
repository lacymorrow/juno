import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import { Save } from "lucide-react";
import { useState } from "react";
import { SettingsSectionProps } from "../types";
import { SettingsGroup, SettingsRow } from "../ui";

// The insertion-mode row re-explains itself: its subtitle is the selected
// option's own one-line description.
const INSERTION_MODE_DESCRIPTIONS: Record<string, string> = {
  paste: "Pastes with Cmd+V. Most compatible.",
  clipboard_free: "Types the transcript directly. Never touches your clipboard.",
};

export default function VoiceSettings({ settings }: SettingsSectionProps) {
  const {
    chatterboxReferenceAudioUrl,
    chatterboxExaggeration,
    chatterboxUseHd,
    handleChatterboxSettingsChange,
    supertonicServerUrl,
    supertonicVoice,
    supertonicSpeed,
    handleSupertonicSettingsChange,
  } = settings;

  // Local draft state for Chatterbox settings (save on blur/button)
  const [chatterboxRefUrl, setChatterboxRefUrl] = useState<string>(chatterboxReferenceAudioUrl ?? "");
  const [chatterboxExag, setChatterboxExag] = useState<number>(chatterboxExaggeration ?? 0.5);
  const [chatterboxHd, setChatterboxHd] = useState<boolean>(chatterboxUseHd ?? false);

  const saveChatterboxSettings = () => {
    handleChatterboxSettingsChange(chatterboxRefUrl, chatterboxExag, chatterboxHd);
  };

  // Local draft state for Supertonic settings
  const [stServerUrl, setStServerUrl] = useState<string>(supertonicServerUrl ?? "http://localhost:8000");
  const [stVoice, setStVoice] = useState<string>(supertonicVoice ?? "M1");
  const [stSpeed, setStSpeed] = useState<number>(supertonicSpeed ?? 1.05);

  const saveSupertonicSettings = () => {
    handleSupertonicSettingsChange(stServerUrl, stVoice, stSpeed);
  };


  return (
    <div className="space-y-6">
      <SettingsGroup title="Text-to-Speech" footer="Configure voice output settings">
        <SettingsRow htmlFor="tts-provider" label="TTS Provider">
          <Select
            value={settings.ttsProvider}
            onValueChange={settings.handleTtsProviderChange}
          >
            <SelectTrigger id="tts-provider" className="w-[190px]">
              <SelectValue placeholder="Select TTS provider" />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="off">Off</SelectItem>
              <SelectItem value="system">System</SelectItem>
              <SelectItem value="kokoro">Kokoro (Local)</SelectItem>
              <SelectItem value="elevenlabs">ElevenLabs</SelectItem>
              <SelectItem value="replicate">Replicate</SelectItem>
              <SelectItem value="chatterbox">Chatterbox (Cloud)</SelectItem>
              <SelectItem value="supertonic">Supertonic (Local)</SelectItem>
            </SelectContent>
          </Select>
        </SettingsRow>

        {settings.ttsProvider === "chatterbox" && (
          <>
            <SettingsRow description="Chatterbox is MIT-licensed and runs on Replicate (~$0.006/sec). Requires a Replicate API key." />

            <SettingsRow
              advanced
              htmlFor="chatterbox-ref-audio"
              label="Reference Audio URL (optional)"
              description="5–10s WAV/MP3 URL for voice cloning. Leave blank for default voice."
              below={
                <div className="flex gap-2">
                  <Input
                    id="chatterbox-ref-audio"
                    value={chatterboxRefUrl}
                    onChange={(e) => setChatterboxRefUrl(e.target.value)}
                    placeholder="https://example.com/voice-sample.wav"
                    className="flex-1"
                  />
                  <Button size="sm" onClick={saveChatterboxSettings} variant="outline">
                    <Save className="h-3 w-3" />
                  </Button>
                </div>
              }
            />

            <SettingsRow
              advanced
              htmlFor="chatterbox-exaggeration"
              label={`Emotion Exaggeration: ${chatterboxExag.toFixed(2)}`}
              description="0 = neutral, 1 = natural, 2 = very expressive"
              below={
                <input
                  type="range"
                  id="chatterbox-exaggeration"
                  min="0"
                  max="2"
                  step="0.05"
                  value={chatterboxExag}
                  onChange={(e) => setChatterboxExag(parseFloat(e.target.value))}
                  onMouseUp={saveChatterboxSettings}
                  onTouchEnd={saveChatterboxSettings}
                  className="w-full h-2 bg-muted rounded-lg appearance-none cursor-pointer"
                />
              }
            />

            <SettingsRow
              advanced
              htmlFor="chatterbox-hd"
              label="Use Chatterbox HD"
              description="Higher quality, slightly slower (resemble-ai/chatterbox-hd)"
            >
              <Switch
                id="chatterbox-hd"
                checked={chatterboxHd}
                onCheckedChange={(checked) => {
                  setChatterboxHd(checked);
                  handleChatterboxSettingsChange(chatterboxRefUrl, chatterboxExag, checked);
                }}
              />
            </SettingsRow>
          </>
        )}

        {settings.ttsProvider === "supertonic" && (
          <>
            <SettingsRow description="Supertonic is an MIT-licensed on-device TTS engine. 31 languages, 167x real-time on Apple Silicon. Requires: pip install supertonic && supertonic serve" />

            <SettingsRow
              advanced
              htmlFor="supertonic-server-url"
              label="Server URL"
              description="URL of the local Supertonic server (supertonic serve)."
              below={
                <div className="flex gap-2">
                  <Input
                    id="supertonic-server-url"
                    value={stServerUrl}
                    onChange={(e) => setStServerUrl(e.target.value)}
                    placeholder="http://localhost:8000"
                    className="flex-1"
                  />
                  <Button size="sm" onClick={saveSupertonicSettings} variant="outline">
                    <Save className="h-3 w-3" />
                  </Button>
                </div>
              }
            />

            <SettingsRow htmlFor="supertonic-voice" label="Voice">
              <Select
                value={stVoice}
                onValueChange={(v) => {
                  setStVoice(v);
                  handleSupertonicSettingsChange(stServerUrl, v, stSpeed);
                }}
              >
                <SelectTrigger id="supertonic-voice" className="w-[190px]">
                  <SelectValue placeholder="Select voice" />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="M1">M1 (Male)</SelectItem>
                  <SelectItem value="F1">F1 (Female)</SelectItem>
                </SelectContent>
              </Select>
            </SettingsRow>

            <SettingsRow
              advanced
              htmlFor="supertonic-speed"
              label={`Speed: ${stSpeed.toFixed(2)}x`}
              description="0.5 = slow, 1.05 = default, 2.0 = fast"
              below={
                <input
                  type="range"
                  id="supertonic-speed"
                  min="0.5"
                  max="2"
                  step="0.05"
                  value={stSpeed}
                  onChange={(e) => setStSpeed(parseFloat(e.target.value))}
                  onMouseUp={saveSupertonicSettings}
                  onTouchEnd={saveSupertonicSettings}
                  className="w-full h-2 bg-muted rounded-lg appearance-none cursor-pointer"
                />
              }
            />
          </>
        )}
      </SettingsGroup>

      <SettingsGroup
        title="Dictation Settings"
        advanced
        footer="Configure how dictation delivers text"
      >
        <SettingsRow
          htmlFor="dictation-insertion-mode"
          label="Text Insertion"
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
              <SelectItem value="paste">Clipboard Paste</SelectItem>
              <SelectItem value="clipboard_free">Clipboard-Free</SelectItem>
            </SelectContent>
          </Select>
        </SettingsRow>

        <SettingsRow
          htmlFor="dictation-clipboard"
          label="Copy to Clipboard"
          description="Leave the transcript on the clipboard after inserting"
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
          label="Live transcription"
          description="Show words as you speak, in Juno's bar. Display-only — provisional text is never typed into the app; the final result is."
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
