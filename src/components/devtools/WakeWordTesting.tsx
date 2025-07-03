import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Separator } from "@/components/ui/separator";
import type { LoadingStates } from "@/types/devtools";
import { EVENTS } from "@/lib/constants.generated";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import {
  AudioLines,
  Brain,
  Bug,
  Info,
  Mic,
  MicOff,
  Pause,
  Play,
  Plus,
  Settings,
  Trash2,
  Volume2,
} from "lucide-react";
import React, { useEffect, useState } from "react";
import { toast } from "sonner";

interface WakeWordStatus {
  isActive: boolean;
  sensitivity: number;
  wakeWords: string[];
  lastActivation?: string;
  activationCount: number;
}

interface AlwaysListeningEvent {
  type:
    | "started"
    | "stopped"
    | "wake_word_detected"
    | "monitoring"
    | "activated"
    | "error"
    | "transcription_debug"
    | "audio_level";
  payload?: any;
}

interface TranscriptionDebugInfo {
  transcription: string;
  confidence: number;
  audio_length_ms: number;
  volume_level: number;
  wake_word_match: boolean;
  fuzzy_matches: string[];
}

// Custom styles for the range input
const sliderStyle = {
  WebkitAppearance: "none" as const,
  appearance: "none" as const,
  height: "8px",
  borderRadius: "4px",
  background:
    "linear-gradient(to right, #3b82f6 0%, #e5e7eb 50%, #e5e7eb 100%)",
  outline: "none",
  opacity: 0.7,
  transition: "opacity 0.2s",
  cursor: "pointer",
};

interface WakeWordTestingProps {
  loadingStates: LoadingStates;
  setLoadingStates: React.Dispatch<React.SetStateAction<LoadingStates>>;
}

const WakeWordTesting: React.FC<WakeWordTestingProps> = ({
  loadingStates,
  setLoadingStates,
}) => {
  const [status, setStatus] = useState<WakeWordStatus>({
    isActive: false,
    sensitivity: 0.5,
    wakeWords: ["hey juno", "computer"],
    activationCount: 0,
  });

  const [newWakeWord, setNewWakeWord] = useState<string>("");
  const [testingMode, setTestingMode] = useState<boolean>(false);
  const [recentEvents, setRecentEvents] = useState<string[]>([]);
  const [volumeLevel, setVolumeLevel] = useState<number>(0);
  const [transcriptionDebugging, setTranscriptionDebugging] =
    useState<boolean>(false);
  const [recentTranscriptions, setRecentTranscriptions] = useState<
    TranscriptionDebugInfo[]
  >([]);
  const [audioLevelMonitoring, setAudioLevelMonitoring] =
    useState<boolean>(false);

  // Load initial status
  useEffect(() => {
    loadStatus();
    const cleanup = setupEventListeners();

    return () => {
      cleanup.then((cleanupFn) => cleanupFn && cleanupFn());
    };
  }, []);

  const setupEventListeners = async () => {
    try {
      // Listen for always listening events
      const unlistenAlwaysListening = await listen<AlwaysListeningEvent>(
        EVENTS.ALWAYS_LISTENING_EVENT,
        (event) => {
          const { type, payload } = event.payload;
          const timestamp = new Date().toLocaleTimeString();

          switch (type) {
            case "started":
              addEvent(`✅ Always listening started at ${timestamp}`);
              setStatus((prev) => ({ ...prev, isActive: true }));
              break;
            case "stopped":
              addEvent(`⏹️ Always listening stopped at ${timestamp}`);
              setStatus((prev) => ({ ...prev, isActive: false }));
              break;
            case "wake_word_detected":
              addEvent(
                `🎯 Wake word detected: "${
                  payload?.word || "unknown"
                }" at ${timestamp}`
              );
              setStatus((prev) => ({
                ...prev,
                lastActivation: timestamp,
                activationCount: prev.activationCount + 1,
              }));
              break;
            case "monitoring":
              addEvent(`👁️ Entered monitoring state at ${timestamp}`);
              break;
            case "activated":
              addEvent(`🔴 Voice activation detected at ${timestamp}`);
              break;
            case "error":
              addEvent(
                `❌ Error: ${
                  payload?.message || "Unknown error"
                } at ${timestamp}`
              );
              break;
            case "transcription_debug":
              if (transcriptionDebugging && payload) {
                const debugInfo: TranscriptionDebugInfo = payload;
                addEvent(
                  `🔍 Transcription: "${debugInfo.transcription}" (conf: ${
                    debugInfo.confidence?.toFixed(2) || "N/A"
                  }, len: ${debugInfo.audio_length_ms}ms, vol: ${(
                    debugInfo.volume_level * 100
                  ).toFixed(1)}%) at ${timestamp}`
                );
                setRecentTranscriptions((prev) => [
                  debugInfo,
                  ...prev.slice(0, 9),
                ]);
              }
              break;
            case "audio_level":
              if (audioLevelMonitoring && payload) {
                setVolumeLevel(payload.level || 0);
              }
              break;
          }
        }
      );

      // Listen for volume level updates (if available)
      const unlistenVolume = await listen<{ level: number }>(
        "always-listening-volume",
        (event) => {
          setVolumeLevel(event.payload.level);
        }
      );

      return () => {
        if (typeof unlistenAlwaysListening === "function") {
          unlistenAlwaysListening();
        }
        if (typeof unlistenVolume === "function") {
          unlistenVolume();
        }
      };
    } catch (error) {
      console.error("Failed to setup event listeners:", error);
      return () => {}; // Return a no-op function if setup fails
    }
  };

  const addEvent = (event: string) => {
    setRecentEvents((prev) => [event, ...prev.slice(0, 19)]); // Keep last 20 events
  };

  const loadStatus = async () => {
    try {
      const [isActive, sensitivity, wakeWords] = await Promise.all([
        invoke<boolean>("get_always_listening_status"),
        invoke<number>("get_always_listening_sensitivity"),
        invoke<string[]>("get_always_listening_wake_words"),
      ]);

      setStatus((prev) => ({
        ...prev,
        isActive,
        sensitivity,
        wakeWords,
      }));
    } catch (error) {
      console.error("Failed to load always listening status:", error);
      toast.error("Failed to load status");
    }
  };

  const handleStartStop = async () => {
    const action = status.isActive ? "stop" : "start";
    setLoadingStates((prev) => ({ ...prev, toggleAlwaysListening: true }));

    try {
      await invoke("toggle_always_listening_mode");
      await loadStatus(); // Refresh status after toggle
      addEvent(`${status.isActive ? "⏹️" : "▶️"} Always listening ${action}ed`);
      toast.success(`Always listening ${action}ed`);
    } catch (error) {
      toast.error(`Failed to ${action} always listening: ${error}`);
    } finally {
      setLoadingStates((prev) => ({ ...prev, toggleAlwaysListening: false }));
    }
  };

  const handleSensitivityChange = async (value: number[]) => {
    const newSensitivity = value[0];
    setLoadingStates((prev) => ({
      ...prev,
      setAlwaysListeningSensitivity: true,
    }));

    try {
      await invoke("set_always_listening_sensitivity", {
        sensitivity: newSensitivity,
      });
      setStatus((prev) => ({ ...prev, sensitivity: newSensitivity }));
      addEvent(`📊 Sensitivity updated to ${newSensitivity}`);
      toast.success(`Sensitivity set to ${newSensitivity}`);
    } catch (error) {
      toast.error(`Failed to update sensitivity: ${error}`);
    } finally {
      setLoadingStates((prev) => ({
        ...prev,
        setAlwaysListeningSensitivity: false,
      }));
    }
  };

  const handleAddWakeWord = async () => {
    if (!newWakeWord.trim()) {
      toast.error("Please enter a wake word.");
      return;
    }

    setLoadingStates((prev) => ({
      ...prev,
      setAlwaysListeningWakeWords: true,
    }));

    try {
      const updatedWakeWords = [...status.wakeWords, newWakeWord.trim()];
      await invoke("set_always_listening_wake_words", {
        wakeWords: updatedWakeWords,
      });
      setStatus((prev) => ({ ...prev, wakeWords: updatedWakeWords }));
      setNewWakeWord("");
      addEvent(`➕ Added wake word: "${newWakeWord.trim()}"`);
      toast.success(`Wake word "${newWakeWord.trim()}" added`);
    } catch (error) {
      toast.error(`Failed to add wake word: ${error}`);
    } finally {
      setLoadingStates((prev) => ({
        ...prev,
        setAlwaysListeningWakeWords: false,
      }));
    }
  };

  const handleRemoveWakeWord = async (word: string) => {
    setLoadingStates((prev) => ({
      ...prev,
      setAlwaysListeningWakeWords: true,
    }));

    try {
      const updatedWakeWords = status.wakeWords.filter((w) => w !== word);
      await invoke("set_always_listening_wake_words", {
        wakeWords: updatedWakeWords,
      });
      setStatus((prev) => ({ ...prev, wakeWords: updatedWakeWords }));
      addEvent(`➖ Removed wake word: "${word}"`);
      toast.success(`Wake word "${word}" removed`);
    } catch (error) {
      toast.error(`Failed to remove wake word: ${error}`);
    } finally {
      setLoadingStates((prev) => ({
        ...prev,
        setAlwaysListeningWakeWords: false,
      }));
    }
  };

  const handleTestMode = () => {
    setTestingMode(!testingMode);
    if (!testingMode) {
      addEvent("🧪 Test mode enabled - speak wake words to test detection");
    } else {
      addEvent("⏸️ Test mode disabled");
    }
  };

  const handleClearEvents = () => {
    setRecentEvents([]);
    setRecentTranscriptions([]);
  };

  const getStatusColor = () => {
    return status.isActive ? "bg-green-500" : "bg-gray-400";
  };

  const getStatusText = () => {
    return status.isActive ? "Active" : "Inactive";
  };

  const handleDebugStatus = async () => {
    setLoadingStates((prev) => ({ ...prev, debugAlwaysListening: true }));

    try {
      const debugInfo = await invoke("debug_always_listening_status");
      console.log("Always Listening Debug Info:", debugInfo);
      addEvent(`🔍 Debug info retrieved (check console)`);
      toast.success("Debug information logged to console");
    } catch (error) {
      toast.error(`Failed to get debug info: ${error}`);
    } finally {
      setLoadingStates((prev) => ({ ...prev, debugAlwaysListening: false }));
    }
  };

  const handleToggleTranscriptionDebugging = async () => {
    const newState = !transcriptionDebugging;
    setTranscriptionDebugging(newState);

    try {
      await invoke("set_transcription_debugging", { enabled: newState });
      addEvent(
        `🔍 Transcription debugging ${newState ? "enabled" : "disabled"}`
      );
      toast.success(
        `Transcription debugging ${newState ? "enabled" : "disabled"}`
      );
    } catch (error) {
      toast.error(`Failed to toggle transcription debugging: ${error}`);
      setTranscriptionDebugging(!newState); // Revert on error
    }
  };

  const handleToggleAudioLevelMonitoring = async () => {
    const newState = !audioLevelMonitoring;
    setAudioLevelMonitoring(newState);

    try {
      await invoke("set_audio_level_monitoring", { enabled: newState });
      addEvent(
        `📊 Audio level monitoring ${newState ? "enabled" : "disabled"}`
      );
      toast.success(
        `Audio level monitoring ${newState ? "enabled" : "disabled"}`
      );
    } catch (error) {
      toast.error(`Failed to toggle audio level monitoring: ${error}`);
      setAudioLevelMonitoring(!newState); // Revert on error
    }
  };

  const handleTestWhisperModel = async () => {
    setLoadingStates((prev) => ({ ...prev, debugAlwaysListening: true }));

    try {
      const testResult = await invoke("test_whisper_model");
      console.log("Whisper Model Test Result:", testResult);
      addEvent(`🧠 Whisper model test completed (check console)`);
      toast.success("Whisper model test completed - check console for details");
    } catch (error) {
      toast.error(`Whisper model test failed: ${error}`);
      addEvent(`❌ Whisper model test failed: ${error}`);
    } finally {
      setLoadingStates((prev) => ({ ...prev, debugAlwaysListening: false }));
    }
  };

  const handleForceTranscription = async () => {
    setLoadingStates((prev) => ({ ...prev, debugAlwaysListening: true }));

    try {
      const result = await invoke("force_transcription_test");
      console.log("Force Transcription Test Result:", result);
      addEvent(`🎤 Force transcription test completed (check console)`);
      toast.success(
        "Force transcription test completed - check console for details"
      );
    } catch (error) {
      toast.error(`Force transcription test failed: ${error}`);
      addEvent(`❌ Force transcription test failed: ${error}`);
    } finally {
      setLoadingStates((prev) => ({ ...prev, debugAlwaysListening: false }));
    }
  };

  return (
    <div className="space-y-6">
      {/* Status Overview */}
      <div className="flex items-center justify-between">
        <div className="flex items-center space-x-3">
          <div className={`w-3 h-3 rounded-full ${getStatusColor()}`} />
          <span className="font-medium">
            Always Listening: {getStatusText()}
          </span>
          <Badge variant={status.isActive ? "default" : "secondary"}>
            {status.activationCount} activations
          </Badge>
        </div>
        <Button
          onClick={handleStartStop}
          disabled={loadingStates.toggleAlwaysListening}
          variant={status.isActive ? "destructive" : "default"}
          size="sm"
        >
          {loadingStates.toggleAlwaysListening ? (
            "..."
          ) : status.isActive ? (
            <>
              <MicOff className="w-4 h-4 mr-2" />
              Stop
            </>
          ) : (
            <>
              <Mic className="w-4 h-4 mr-2" />
              Start
            </>
          )}
        </Button>
      </div>

      {status.lastActivation && (
        <div className="text-sm text-muted-foreground">
          Last activation: {status.lastActivation}
        </div>
      )}

      <Separator />

      {/* Enhanced Debugging Controls */}
      <div className="space-y-3">
        <Label className="flex items-center space-x-2">
          <Bug className="w-4 h-4" />
          <span>Advanced Debugging</span>
        </Label>

        <div className="grid grid-cols-2 gap-2">
          <Button
            onClick={handleToggleTranscriptionDebugging}
            variant={transcriptionDebugging ? "default" : "outline"}
            size="sm"
          >
            <Brain className="w-4 h-4 mr-2" />
            {transcriptionDebugging ? "Stop" : "Start"} Transcription Debug
          </Button>

          <Button
            onClick={handleToggleAudioLevelMonitoring}
            variant={audioLevelMonitoring ? "default" : "outline"}
            size="sm"
          >
            <AudioLines className="w-4 h-4 mr-2" />
            {audioLevelMonitoring ? "Stop" : "Start"} Audio Monitor
          </Button>

          <Button
            onClick={handleTestWhisperModel}
            disabled={loadingStates.debugAlwaysListening}
            variant="outline"
            size="sm"
          >
            <Brain className="w-4 h-4 mr-2" />
            Test Whisper Model
          </Button>

          <Button
            onClick={handleForceTranscription}
            disabled={loadingStates.debugAlwaysListening}
            variant="outline"
            size="sm"
          >
            <Mic className="w-4 h-4 mr-2" />
            Force Transcription
          </Button>
        </div>
      </div>

      <Separator />

      {/* Sensitivity Control */}
      <div className="space-y-3">
        <div className="flex items-center space-x-2">
          <Volume2 className="w-4 h-4" />
          <Label>Sensitivity: {status.sensitivity.toFixed(2)}</Label>
          <Info className="w-4 h-4 text-muted-foreground" />
        </div>
        <div className="px-2">
          <input
            type="range"
            value={status.sensitivity}
            onChange={(e) => handleSensitivityChange([Number(e.target.value)])}
            min={0.1}
            max={2.0}
            step={0.1}
            className="w-full h-2 bg-gray-200 rounded-lg appearance-none cursor-pointer dark:bg-gray-700 slider"
            style={sliderStyle}
          />
          <div className="flex justify-between text-xs text-muted-foreground mt-1">
            <span>Low (0.1)</span>
            <span>Default (0.5)</span>
            <span>High (2.0)</span>
          </div>
        </div>
        {volumeLevel > 0 && (
          <div className="text-sm text-muted-foreground">
            Current volume level: {(volumeLevel * 100).toFixed(1)}%
          </div>
        )}
      </div>

      <Separator />

      {/* Recent Transcriptions (when debugging enabled) */}
      {transcriptionDebugging && recentTranscriptions.length > 0 && (
        <>
          <div className="space-y-3">
            <Label>Recent Transcriptions</Label>
            <div className="bg-gray-50 dark:bg-gray-900 rounded-md p-3 max-h-32 overflow-y-auto">
              <div className="space-y-1">
                {recentTranscriptions.map((transcription, index) => (
                  <div key={index} className="text-xs font-mono space-y-1">
                    <div className="flex justify-between">
                      <span
                        className={
                          transcription.transcription
                            ? "text-green-600"
                            : "text-red-600"
                        }
                      >
                        "{transcription.transcription || "(empty)"}"
                      </span>
                      <span className="text-muted-foreground">
                        {transcription.confidence?.toFixed(2) || "N/A"}
                      </span>
                    </div>
                    <div className="text-muted-foreground">
                      Length: {transcription.audio_length_ms}ms, Vol:{" "}
                      {(transcription.volume_level * 100).toFixed(1)}%, Match:{" "}
                      {transcription.wake_word_match ? "✅" : "❌"}
                      {transcription.fuzzy_matches &&
                        transcription.fuzzy_matches.length > 0 && (
                          <span>
                            {" "}
                            (Fuzzy: {transcription.fuzzy_matches.join(", ")})
                          </span>
                        )}
                    </div>
                  </div>
                ))}
              </div>
            </div>
          </div>
          <Separator />
        </>
      )}

      {/* Wake Words Management */}
      <div className="space-y-3">
        <div className="flex items-center space-x-2">
          <Settings className="w-4 h-4" />
          <Label>Wake Words</Label>
        </div>

        <div className="flex flex-wrap gap-2">
          {status.wakeWords.map((word, index) => (
            <Badge
              key={index}
              variant="outline"
              className="flex items-center space-x-1"
            >
              <span>{word}</span>
              <button
                onClick={() => handleRemoveWakeWord(word)}
                className="ml-1 hover:text-red-500"
              >
                <Trash2 className="w-3 h-3" />
              </button>
            </Badge>
          ))}
        </div>

        <div className="flex items-center space-x-2">
          <Input
            placeholder="Add new wake word"
            value={newWakeWord}
            onChange={(e) => setNewWakeWord(e.target.value)}
            onKeyPress={(e) => e.key === "Enter" && handleAddWakeWord()}
          />
          <Button onClick={handleAddWakeWord} size="sm">
            <Plus className="w-4 h-4" />
          </Button>
        </div>
      </div>

      <Separator />

      {/* Testing Mode */}
      <div className="space-y-3">
        <div className="flex items-center justify-between">
          <Label>Testing Mode</Label>
          <Button
            onClick={handleTestMode}
            variant={testingMode ? "destructive" : "outline"}
            size="sm"
          >
            {testingMode ? (
              <>
                <Pause className="w-4 h-4 mr-2" />
                Stop Testing
              </>
            ) : (
              <>
                <Play className="w-4 h-4 mr-2" />
                Start Testing
              </>
            )}
          </Button>
        </div>

        {testingMode && (
          <div className="p-3 bg-blue-50 dark:bg-blue-950 rounded-md">
            <div className="flex items-center space-x-2 text-blue-700 dark:text-blue-300">
              <Info className="w-4 h-4" />
              <span className="text-sm">
                Test mode active. Speak your wake words and watch for detection
                events below.
              </span>
            </div>
          </div>
        )}
      </div>

      <Separator />

      {/* Event Log */}
      <div className="space-y-3">
        <div className="flex items-center justify-between">
          <Label>Recent Events</Label>
          <Button onClick={handleClearEvents} variant="ghost" size="sm">
            <Trash2 className="w-4 h-4 mr-2" />
            Clear
          </Button>
        </div>

        <div className="bg-gray-50 dark:bg-gray-900 rounded-md p-3 max-h-48 overflow-y-auto">
          {recentEvents.length === 0 ? (
            <div className="text-sm text-muted-foreground text-center">
              No events yet. Start always listening to see activity.
            </div>
          ) : (
            <div className="space-y-1">
              {recentEvents.map((event, index) => (
                <div key={index} className="text-sm font-mono">
                  {event}
                </div>
              ))}
            </div>
          )}
        </div>
      </div>

      {/* Debug Information */}
      <div className="space-y-3">
        <Label>Debug Information</Label>
        <div className="bg-gray-50 dark:bg-gray-900 rounded-md p-3 space-y-2 text-sm">
          <div>Status: {status.isActive ? "✅ Active" : "❌ Inactive"}</div>
          <div>Sensitivity: {status.sensitivity}</div>
          <div>Wake Words: {status.wakeWords.join(", ")}</div>
          <div>Total Activations: {status.activationCount}</div>
          <div>Test Mode: {testingMode ? "🧪 Enabled" : "⏸️ Disabled"}</div>
          <div>
            Transcription Debug:{" "}
            {transcriptionDebugging ? "🔍 Enabled" : "⏸️ Disabled"}
          </div>
          <div>
            Audio Monitor: {audioLevelMonitoring ? "📊 Enabled" : "⏸️ Disabled"}
          </div>
        </div>
      </div>

      <Separator />

      <div className="flex items-center justify-between">
        <Button
          onClick={handleDebugStatus}
          disabled={loadingStates.debugAlwaysListening}
          variant="outline"
          size="sm"
        >
          {loadingStates.debugAlwaysListening ? (
            "..."
          ) : (
            <Info className="h-4 w-4" />
          )}
          {loadingStates.debugAlwaysListening ? "Loading..." : "Debug Status"}
        </Button>
      </div>
    </div>
  );
};

export default WakeWordTesting;
