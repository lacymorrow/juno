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
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import { Save } from "lucide-react";
import { SettingsSectionProps } from "../types";

export default function VoiceSettings({ settings }: SettingsSectionProps) {
  return (
    <div className="space-y-6">
      <div>
        <h3 className="text-lg font-medium text-gray-900 mb-4">
          Voice & Audio
        </h3>

        <Card>
          <CardHeader>
            <CardTitle>Text-to-Speech</CardTitle>
            <CardDescription>Configure voice output settings</CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
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
                  <SelectItem value="elevenlabs">ElevenLabs</SelectItem>
                  <SelectItem value="replicate">Replicate</SelectItem>
                </SelectContent>
              </Select>
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
          <CardContent>
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
                <Label htmlFor="always-listening">
                  Enable Always Listening
                </Label>
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
                      settings.handleSensitivityChange(
                        parseFloat(e.target.value)
                      )
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
                      onChange={(e) =>
                        settings.setWakeWordsInput(e.target.value)
                      }
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
    </div>
  );
}