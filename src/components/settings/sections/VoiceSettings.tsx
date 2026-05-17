import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
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

  return (
    <div className="space-y-6">
      <Card>
        <CardHeader>
          <CardTitle>Text-to-Speech</CardTitle>
          <CardDescription>Configure voice output settings</CardDescription>
        </CardHeader>
        <CardContent>
          <div className="space-y-2">
            <Label htmlFor="tts-provider">TTS Provider</Label>
            <Select
              value={settings.ttsProvider}
              onValueChange={settings.handleTtsProviderChange}
            >
              <SelectTrigger>
                <SelectValue placeholder="Select TTS provider" />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="off">Off</SelectItem>
                <SelectItem value="system">System</SelectItem>
                <SelectItem value="kokoro">Kokoro (Local)</SelectItem>
                <SelectItem value="elevenlabs">ElevenLabs</SelectItem>
                <SelectItem value="replicate">Replicate</SelectItem>
                <SelectItem value="chatterbox">Chatterbox (Cloud)</SelectItem>
              </SelectContent>
            </Select>
          </div>

          {settings.ttsProvider === "chatterbox" && (
            <div className="mt-4 space-y-4 rounded-md border p-4">
              <p className="text-xs text-muted-foreground">
                Chatterbox is MIT-licensed and runs on Replicate (~$0.006/sec).
                Requires a Replicate API key.
              </p>

              <div className="space-y-2">
                <Label htmlFor="chatterbox-ref-audio">
                  Reference Audio URL (optional)
                </Label>
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
                <p className="text-xs text-muted-foreground">
                  5–10s WAV/MP3 URL for voice cloning. Leave blank for default voice.
                </p>
              </div>

              <div className="space-y-2">
                <Label htmlFor="chatterbox-exaggeration">
                  Emotion Exaggeration:{" "}
                  {chatterboxExag.toFixed(2)}
                </Label>
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
                  className="w-full h-2 bg-gray-200 rounded-lg appearance-none cursor-pointer"
                />
                <p className="text-xs text-muted-foreground">
                  0 = neutral, 1 = natural, 2 = very expressive
                </p>
              </div>

              <div className="flex items-center justify-between">
                <div>
                  <Label htmlFor="chatterbox-hd">Use Chatterbox HD</Label>
                  <p className="text-xs text-gray-500">
                    Higher quality, slightly slower (resemble-ai/chatterbox-hd)
                  </p>
                </div>
                <Switch
                  id="chatterbox-hd"
                  checked={chatterboxHd}
                  onCheckedChange={(checked) => {
                    setChatterboxHd(checked);
                    handleChatterboxSettingsChange(chatterboxRefUrl, chatterboxExag, checked);
                  }}
                />
              </div>
            </div>
          )}
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Speech-to-Text Model</CardTitle>
          <CardDescription>
            Choose the Whisper model used for transcription. Larger models are
            more accurate but require a one-time download.
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          {whisperDownloading && (
            <div className="space-y-2 rounded-md border p-3 bg-muted/40">
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
          )}

          <div className="space-y-2">
            <Label htmlFor="whisper-model">Active Model</Label>
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
            {selectedModel && (
              <p className="text-xs text-muted-foreground">
                {selectedModel.downloaded
                  ? `Active — ${selectedModel.size_mb} MB`
                  : `Not downloaded yet — ${selectedModel.size_mb} MB`}
              </p>
            )}
          </div>

          <div className="space-y-2">
            <Label>Download a Model</Label>
            <div className="grid gap-2">
              {whisperModels
                .filter((m) => !m.downloaded)
                .map((model) => (
                  <div
                    key={model.id}
                    className="flex items-center justify-between rounded-md border px-3 py-2"
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
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Dictation Settings</CardTitle>
          <CardDescription>
            Configure voice input and transcription
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="flex items-center justify-between">
            <div>
              <Label htmlFor="dictation-clipboard">
                Enable Clipboard Integration
              </Label>
              <p className="text-xs text-gray-500">
                Automatically copy dictated text to clipboard
              </p>
            </div>
            <Switch
              id="dictation-clipboard"
              checked={settings.dictationClipboardEnabled}
              onCheckedChange={settings.handleDictationClipboardChange}
            />
          </div>

          <div className="space-y-2">
            <Label htmlFor="dictation-trigger-mode">Trigger Mode</Label>
            <Select
              value={settings.dictationTriggerMode}
              onValueChange={settings.handleDictationTriggerModeChange}
            >
              <SelectTrigger>
                <SelectValue placeholder="Select trigger mode" />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="tap">Tap to Toggle</SelectItem>
                <SelectItem value="hold">Hold to Activate (Default)</SelectItem>
              </SelectContent>
            </Select>
            <p className="text-xs text-gray-500">
              <strong>Tap to Toggle:</strong> Press and release to toggle
              dictation mode on/off.
              <br />
              <strong>Hold to Activate:</strong> Hold key to activate dictation,
              release to stop (traditional dictation behavior).
            </p>
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Always Listening</CardTitle>
          <CardDescription>
            Configure wake word detection for hands-free activation
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="flex items-center justify-between">
            <div>
              <Label htmlFor="always-listening">Enable Always Listening</Label>
              <p className="text-xs text-gray-500">
                Listen for wake words to activate Juno
              </p>
            </div>
            <Switch
              id="always-listening"
              checked={settings.alwaysListeningActive}
              onCheckedChange={settings.handleAlwaysListeningToggle}
            />
          </div>

          {settings.alwaysListeningActive && (
            <>
              <div className="space-y-2">
                <Label htmlFor="sensitivity">
                  Sensitivity:{" "}
                  {(settings.alwaysListeningSensitivity * 100).toFixed(0)}%
                </Label>
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
                  className="w-full h-2 bg-gray-200 rounded-lg appearance-none cursor-pointer"
                />
              </div>

              <div className="space-y-2">
                <Label htmlFor="wake-words">Wake Words</Label>
                <div className="flex gap-2">
                  <Input
                    id="wake-words"
                    value={settings.wakeWordsInput}
                    onChange={(e) => settings.setWakeWordsInput(e.target.value)}
                    placeholder="hey juno, computer"
                    className="flex-1"
                  />
                  <Button onClick={settings.handleWakeWordsChange} size="sm">
                    <Save className="w-4 h-4" />
                  </Button>
                </div>
                <p className="text-xs text-gray-500">
                  Separate multiple wake words with commas
                </p>
              </div>
            </>
          )}
        </CardContent>
      </Card>
    </div>
  );
}
