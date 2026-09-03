import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Progress } from "@/components/ui/progress";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import { Download, Save } from "lucide-react";
import { useState } from "react";
import { SettingsSectionProps } from "../types";
import { SettingsGroup, SettingsRow } from "../SettingsPrimitives";

export default function VoiceSettings({ settings }: SettingsSectionProps) {
  const {
    whisperModels,
    currentWhisperModel,
    whisperDownloading,
    whisperDownloadProgress,
    handleWhisperModelChange,
    handleWhisperModelDownload,
    chatterboxReferenceAudioUrl,
    chatterboxExaggeration,
    chatterboxUseHd,
    handleChatterboxSettingsChange,
    supertonicServerUrl,
    supertonicVoice,
    supertonicSpeed,
    handleSupertonicSettingsChange,
  } = settings;

  const selectedModel = whisperModels.find((m) => m.id === currentWhisperModel);
  const downloadingModel = whisperModels.find((m) => m.id === whisperDownloading);

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
    <>
      <SettingsGroup
        title="Text-to-Speech"
        description="Configure voice output settings"
      >
        <SettingsRow
          htmlFor="tts-provider"
          label="TTS provider"
          description="Where spoken responses are generated"
          control={
            <Select
              value={settings.ttsProvider}
              onValueChange={settings.handleTtsProviderChange}
            >
              <SelectTrigger id="tts-provider" className="w-44">
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
          }
        />
      </SettingsGroup>

      {settings.ttsProvider === "chatterbox" && (
        <SettingsGroup
          title="Chatterbox"
          description="MIT-licensed and runs on Replicate (~$0.006/sec). Requires a Replicate API key."
        >
          <SettingsRow
            htmlFor="chatterbox-ref-audio"
            label="Reference audio URL"
            description="5–10s WAV/MP3 URL for voice cloning. Leave blank for default voice."
          >
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
          </SettingsRow>

          <SettingsRow
            advanced
            htmlFor="chatterbox-exaggeration"
            label="Emotion exaggeration"
            description="0 = neutral, 1 = natural, 2 = very expressive"
          >
            <div className="space-y-2">
              <span className="tabular-nums text-sm text-muted-foreground">
                {chatterboxExag.toFixed(2)}
              </span>
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
            </div>
          </SettingsRow>

          <SettingsRow
            advanced
            htmlFor="chatterbox-hd"
            label="Use Chatterbox HD"
            description="Higher quality, slightly slower (resemble-ai/chatterbox-hd)"
            control={
              <Switch
                id="chatterbox-hd"
                checked={chatterboxHd}
                onCheckedChange={(checked) => {
                  setChatterboxHd(checked);
                  handleChatterboxSettingsChange(chatterboxRefUrl, chatterboxExag, checked);
                }}
              />
            }
          />
        </SettingsGroup>
      )}

      {settings.ttsProvider === "supertonic" && (
        <SettingsGroup
          title="Supertonic"
          description="MIT-licensed on-device TTS. 31 languages, 167x real-time on Apple Silicon. Requires: pip install supertonic && supertonic serve"
        >
          <SettingsRow
            htmlFor="supertonic-server-url"
            label="Server URL"
            description="URL of the local Supertonic server (supertonic serve)."
          >
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
          </SettingsRow>

          <SettingsRow
            htmlFor="supertonic-voice"
            label="Voice"
            control={
              <Select
                value={stVoice}
                onValueChange={(v) => {
                  setStVoice(v);
                  handleSupertonicSettingsChange(stServerUrl, v, stSpeed);
                }}
              >
                <SelectTrigger id="supertonic-voice" className="w-44">
                  <SelectValue placeholder="Select voice" />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="M1">M1 (Male)</SelectItem>
                  <SelectItem value="F1">F1 (Female)</SelectItem>
                </SelectContent>
              </Select>
            }
          />

          <SettingsRow
            advanced
            htmlFor="supertonic-speed"
            label="Speed"
            description="0.5 = slow, 1.05 = default, 2.0 = fast"
          >
            <div className="space-y-2">
              <span className="tabular-nums text-sm text-muted-foreground">
                {stSpeed.toFixed(2)}x
              </span>
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
            </div>
          </SettingsRow>
        </SettingsGroup>
      )}

      <SettingsGroup
        title="Speech-to-Text Model"
        description="Choose the Whisper model used for transcription. Larger models are more accurate but require a one-time download."
      >
        {whisperDownloading && (
          <SettingsRow label="Downloading">
            <div className="space-y-2 rounded-md border border-border bg-muted/40 p-3">
              <p className="text-sm font-medium">
                Downloading {downloadingModel?.display_name ?? whisperDownloading}…
              </p>
              {whisperDownloadProgress ? (
                <>
                  <Progress value={whisperDownloadProgress.percent} className="h-2" />
                  <p className="text-xs text-muted-foreground">
                    {(whisperDownloadProgress.bytes_downloaded / 1024 / 1024).toFixed(0)} MB
                    {whisperDownloadProgress.total_bytes > 0
                      ? ` / ${(whisperDownloadProgress.total_bytes / 1024 / 1024).toFixed(0)} MB`
                      : ""}{" "}
                    — {whisperDownloadProgress.percent.toFixed(0)}%
                  </p>
                </>
              ) : (
                <Progress className="h-2 animate-pulse" />
              )}
            </div>
          </SettingsRow>
        )}

        <SettingsRow
          htmlFor="whisper-model"
          label="Active model"
          description={
            selectedModel
              ? selectedModel.downloaded
                ? `Active — ${selectedModel.size_mb} MB`
                : `Not downloaded yet — ${selectedModel.size_mb} MB`
              : undefined
          }
        >
          <Select
            value={currentWhisperModel}
            onValueChange={(id) => {
              const model = whisperModels.find((m) => m.id === id);
              if (model?.downloaded) {
                handleWhisperModelChange(id);
              }
            }}
            disabled={!!whisperDownloading}
          >
            <SelectTrigger id="whisper-model">
              <SelectValue placeholder="Select model" />
            </SelectTrigger>
            <SelectContent>
              {whisperModels.map((model) => (
                <SelectItem
                  key={model.id}
                  value={model.id}
                  disabled={!model.downloaded}
                >
                  {model.display_name}
                  {model.is_default ? " (Recommended)" : ""}
                  {!model.downloaded ? " — not downloaded" : ""}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </SettingsRow>

        <SettingsRow advanced label="Download a model">
          <div className="grid gap-2">
            {whisperModels
              .filter((m) => !m.downloaded)
              .map((model) => (
                <div
                  key={model.id}
                  className="flex items-center justify-between rounded-md border border-border px-3 py-2"
                >
                  <div>
                    <p className="text-sm font-medium">{model.display_name}</p>
                    <p className="text-xs text-muted-foreground">
                      {model.size_mb} MB
                    </p>
                  </div>
                  <Button
                    size="sm"
                    variant="outline"
                    onClick={() => handleWhisperModelDownload(model.id)}
                    disabled={!!whisperDownloading}
                  >
                    <Download className="mr-1 h-3 w-3" />
                    Download
                  </Button>
                </div>
              ))}
            {whisperModels.filter((m) => !m.downloaded).length === 0 && (
              <p className="text-xs text-muted-foreground">
                All available models are downloaded.
              </p>
            )}
          </div>
        </SettingsRow>
      </SettingsGroup>

      <SettingsGroup
        title="Dictation"
        description="Configure voice input and transcription"
      >
        <SettingsRow
          htmlFor="dictation-clipboard"
          label="Enable clipboard integration"
          description="Automatically copy dictated text to clipboard"
          control={
            <Switch
              id="dictation-clipboard"
              checked={settings.dictationClipboardEnabled}
              onCheckedChange={settings.handleDictationClipboardChange}
            />
          }
        />

        <SettingsRow
          advanced
          htmlFor="dictation-trigger-mode"
          label="Trigger mode"
          description={
            <>
              <strong>Tap to Toggle:</strong> Press and release to toggle
              dictation mode on/off.
              <br />
              <strong>Hold to Activate:</strong> Hold key to activate dictation,
              release to stop (traditional dictation behavior).
            </>
          }
          control={
            <Select
              value={settings.dictationTriggerMode}
              onValueChange={settings.handleDictationTriggerModeChange}
            >
              <SelectTrigger id="dictation-trigger-mode" className="w-44">
                <SelectValue placeholder="Select trigger mode" />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="tap">Tap to Toggle</SelectItem>
                <SelectItem value="hold">Hold to Activate (Default)</SelectItem>
              </SelectContent>
            </Select>
          }
        />
      </SettingsGroup>

      <SettingsGroup
        title="Always Listening"
        description="Configure wake word detection for hands-free activation"
      >
        <SettingsRow
          htmlFor="always-listening"
          label="Enable always listening"
          description="Listen for wake words to activate Juno"
          control={
            <Switch
              id="always-listening"
              checked={settings.alwaysListeningActive}
              onCheckedChange={settings.handleAlwaysListeningToggle}
            />
          }
        />

        {settings.alwaysListeningActive && (
          <>
            <SettingsRow
              advanced
              htmlFor="sensitivity"
              label="Sensitivity"
              description={`${(settings.alwaysListeningSensitivity * 100).toFixed(0)}%`}
            >
              <input
                type="range"
                id="sensitivity"
                min="0"
                max="1"
                step="0.1"
                value={settings.alwaysListeningSensitivity}
                onChange={(e) =>
                  settings.handleSensitivityChange(parseFloat(e.target.value))
                }
                className="h-2 w-full cursor-pointer appearance-none rounded-lg bg-muted"
              />
            </SettingsRow>

            <SettingsRow
              advanced
              htmlFor="wake-words"
              label="Wake words"
              description="Separate multiple wake words with commas"
            >
              <div className="flex gap-2">
                <Input
                  id="wake-words"
                  value={settings.wakeWordsInput}
                  onChange={(e) => settings.setWakeWordsInput(e.target.value)}
                  placeholder="hey juno, computer"
                  className="flex-1"
                />
                <Button onClick={settings.handleWakeWordsChange} size="sm">
                  <Save className="h-4 w-4" />
                </Button>
              </div>
            </SettingsRow>
          </>
        )}
      </SettingsGroup>
    </>
  );
}
