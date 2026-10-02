import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Switch } from "@/components/ui/switch";
import { Save } from "lucide-react";
import { useState } from "react";
import { SettingsSectionProps } from "../types";
import { SettingsGroup, SettingsRow } from "../ui";

/**
 * The settings only one engine has: Chatterbox's reference audio and
 * Supertonic's server.
 *
 * Which engine speaks is chosen under Audio, right above the voices it
 * offers, because the two are one decision: changing the engine changes the
 * list. It used to be chosen here, a pane away from the voices it changed,
 * with its own copy of the engine names. This group is only what an engine
 * needs configured, and it appears only while that engine is the one in
 * force. Every other engine has nothing to configure, so nothing is drawn.
 */
export default function TtsEngineGroup({ settings }: SettingsSectionProps) {
  const {
    chatterboxReferenceAudioUrl,
    chatterboxExaggeration,
    chatterboxUseHd,
    handleChatterboxSettingsChange,
    supertonicServerUrl,
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
  const [stSpeed, setStSpeed] = useState<number>(supertonicSpeed ?? 1.05);

  const saveSupertonicSettings = () => {
    handleSupertonicSettingsChange(stServerUrl, stSpeed);
  };

  if (settings.ttsProvider === "chatterbox") {
    return (
      <SettingsGroup
        title="Chatterbox"
        advanced
        footer="Runs on Replicate with your Replicate API key, about $0.006 a second."
      >
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
              <Button size="sm" onClick={saveChatterboxSettings} variant="outline" aria-label="Save reference audio">
                <Save className="h-3 w-3" />
              </Button>
            </div>
          }
        />

        <SettingsRow
          htmlFor="chatterbox-exaggeration"
          label={`Emotion: ${chatterboxExag.toFixed(2)}`}
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
          label="Higher quality"
          description="Slightly slower."
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
      </SettingsGroup>
    );
  }

  if (settings.ttsProvider === "supertonic") {
    return (
      <SettingsGroup
        title="Supertonic"
        advanced
        footer="Supertonic runs as a server on this Mac. Start it with: supertonic serve"
      >
        <SettingsRow
          htmlFor="supertonic-server-url"
          label="Server URL"
          below={
            <div className="flex gap-2">
              <Input
                id="supertonic-server-url"
                value={stServerUrl}
                onChange={(e) => setStServerUrl(e.target.value)}
                placeholder="http://localhost:8000"
                className="flex-1"
              />
              <Button size="sm" onClick={saveSupertonicSettings} variant="outline" aria-label="Save server URL">
                <Save className="h-3 w-3" />
              </Button>
            </div>
          }
        />

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
      </SettingsGroup>
    );
  }

  return null;
}
