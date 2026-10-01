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

/**
 * Which engine speaks for Juno, and the settings only that engine has.
 *
 * This lives in Providers, next to the AI provider, because it is the same
 * kind of decision: which service does the work. Nobody outside this file
 * needs the words "text to speech" to use Juno; the familiar version of this
 * setting is "Juno's voice", under Audio, which is where the person goes.
 * Advanced, and that is deliberate: the engine is plumbing.
 */
export default function TtsEngineGroup({ settings }: SettingsSectionProps) {
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

  const [chatterboxRefUrl, setChatterboxRefUrl] = useState<string>(chatterboxReferenceAudioUrl ?? "");
  const [chatterboxExag, setChatterboxExag] = useState<number>(chatterboxExaggeration ?? 0.5);
  const [chatterboxHd, setChatterboxHd] = useState<boolean>(chatterboxUseHd ?? false);

  const saveChatterboxSettings = () => {
    handleChatterboxSettingsChange(chatterboxRefUrl, chatterboxExag, chatterboxHd);
  };

  const [stServerUrl, setStServerUrl] = useState<string>(supertonicServerUrl ?? "http://localhost:8000");
  const [stVoice, setStVoice] = useState<string>(supertonicVoice ?? "M1");
  const [stSpeed, setStSpeed] = useState<number>(supertonicSpeed ?? 1.05);

  const saveSupertonicSettings = () => {
    handleSupertonicSettingsChange(stServerUrl, stVoice, stSpeed);
  };

  return (
    <SettingsGroup
      title="Voice engine"
      advanced
      footer="Your Mac's own voice needs no account, works offline and starts speaking immediately. Pick a Mac voice under Audio."
    >
      <SettingsRow
        htmlFor="tts-provider"
        label="Speaks with"
        description="Which service turns Juno's answers into sound."
      >
        <Select value={settings.ttsProvider} onValueChange={settings.handleTtsProviderChange}>
          <SelectTrigger id="tts-provider" className="w-[190px]">
            <SelectValue placeholder="Select a voice engine" />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="off">Off</SelectItem>
            <SelectItem value="system">Your Mac</SelectItem>
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
          <SettingsRow description="Chatterbox is MIT-licensed and runs on Replicate (about $0.006 a second). Needs a Replicate API key." />

          <SettingsRow
            htmlFor="chatterbox-ref-audio"
            label="Reference audio URL (optional)"
            description="A 5 to 10 second WAV or MP3 to clone. Leave blank for the default voice."
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
            htmlFor="chatterbox-exaggeration"
            label={`Emotion exaggeration: ${chatterboxExag.toFixed(2)}`}
            description="0 is neutral, 1 is natural, 2 is very expressive."
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
                className="h-2 w-full cursor-pointer appearance-none rounded-lg bg-muted"
              />
            }
          />

          <SettingsRow
            htmlFor="chatterbox-hd"
            label="Use Chatterbox HD"
            description="Higher quality, slightly slower."
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
          <SettingsRow description="Supertonic is an MIT-licensed on-device engine: 31 languages, 167x real time on Apple silicon. Needs pip install supertonic && supertonic serve." />

          <SettingsRow
            htmlFor="supertonic-server-url"
            label="Server URL"
            description="Where the local Supertonic server is listening."
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
                <SelectValue placeholder="Select a voice" />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="M1">M1 (male)</SelectItem>
                <SelectItem value="F1">F1 (female)</SelectItem>
              </SelectContent>
            </Select>
          </SettingsRow>

          <SettingsRow
            htmlFor="supertonic-speed"
            label={`Speed: ${stSpeed.toFixed(2)}x`}
            description="0.5 is slow, 1.05 is the default, 2.0 is fast."
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
                className="h-2 w-full cursor-pointer appearance-none rounded-lg bg-muted"
              />
            }
          />
        </>
      )}
    </SettingsGroup>
  );
}
