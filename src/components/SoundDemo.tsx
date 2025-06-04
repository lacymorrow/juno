import { useAgentSounds, useSound, useVoiceSounds } from "@/hooks/useSound";
import { SoundPlayResult, SoundType } from "@/types/sound";
import React, { useEffect, useState } from "react";

export const SoundDemo: React.FC = () => {
  const sound = useSound();
  const agentSounds = useAgentSounds();
  const voiceSounds = useVoiceSounds();

  const [lastResult, setLastResult] = useState<SoundPlayResult | null>(null);
  const [availableSounds, setAvailableSounds] = useState<SoundType[]>([]);
  const [isLoading, setIsLoading] = useState(false);

  // Load available sounds on component mount
  useEffect(() => {
    const loadSounds = async () => {
      const sounds = await sound.getAvailableSounds();
      setAvailableSounds(sounds);
    };
    loadSounds();
  }, [sound]);

  const handlePlaySound = async (soundType: SoundType) => {
    setIsLoading(true);
    try {
      const result = await sound.playSound(soundType);
      setLastResult(result);
    } finally {
      setIsLoading(false);
    }
  };

  const handlePlayConvenience = async (
    _actionName: string,
    playFunction: () => Promise<SoundPlayResult>
  ) => {
    setIsLoading(true);
    try {
      const result = await playFunction();
      setLastResult(result);
    } finally {
      setIsLoading(false);
    }
  };

  return (
    <div className="p-6 max-w-4xl mx-auto">
      <h1 className="text-3xl font-bold mb-6">Juno Sound System Demo</h1>

      {/* Status Display */}
      {lastResult && (
        <div
          className={`mb-6 p-4 rounded-lg ${
            lastResult.success
              ? "bg-green-100 border-green-500"
              : "bg-red-100 border-red-500"
          } border`}
        >
          <h3 className="font-semibold">Last Sound Result:</h3>
          <p className="text-sm">
            Status: {lastResult.success ? "Success" : "Failed"}
          </p>
          <p className="text-sm">Message: {lastResult.message}</p>
          {lastResult.file_path && (
            <p className="text-sm">File: {lastResult.file_path}</p>
          )}
        </div>
      )}

      {/* Convenience Sound Buttons */}
      <section className="mb-8">
        <h2 className="text-2xl font-semibold mb-4">Quick Actions</h2>
        <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
          <button
            onClick={() =>
              handlePlayConvenience("Notification", sound.playNotification)
            }
            disabled={isLoading}
            className="px-4 py-2 bg-blue-500 text-white rounded hover:bg-blue-600 disabled:opacity-50"
          >
            🔔 Notification
          </button>
          <button
            onClick={() => handlePlayConvenience("Success", sound.playSuccess)}
            disabled={isLoading}
            className="px-4 py-2 bg-green-500 text-white rounded hover:bg-green-600 disabled:opacity-50"
          >
            ✅ Success
          </button>
          <button
            onClick={() => handlePlayConvenience("Error", sound.playError)}
            disabled={isLoading}
            className="px-4 py-2 bg-red-500 text-white rounded hover:bg-red-600 disabled:opacity-50"
          >
            ❌ Error
          </button>
          <button
            onClick={() => handlePlayConvenience("Alert", sound.playAlert)}
            disabled={isLoading}
            className="px-4 py-2 bg-yellow-500 text-white rounded hover:bg-yellow-600 disabled:opacity-50"
          >
            ⚠️ Alert
          </button>
        </div>
      </section>

      {/* Agent Sounds */}
      <section className="mb-8">
        <h2 className="text-2xl font-semibold mb-4">AI Agent Sounds</h2>
        <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
          <button
            onClick={() =>
              handlePlayConvenience("Agent Start", agentSounds.playAgentStart)
            }
            disabled={isLoading}
            className="px-4 py-2 bg-indigo-500 text-white rounded hover:bg-indigo-600 disabled:opacity-50"
          >
            🤖 Agent Start
          </button>
          <button
            onClick={() =>
              handlePlayConvenience(
                "Agent Success",
                agentSounds.playAgentSuccess
              )
            }
            disabled={isLoading}
            className="px-4 py-2 bg-green-500 text-white rounded hover:bg-green-600 disabled:opacity-50"
          >
            🎉 Agent Success
          </button>
          <button
            onClick={() =>
              handlePlayConvenience("Agent Error", agentSounds.playAgentError)
            }
            disabled={isLoading}
            className="px-4 py-2 bg-red-500 text-white rounded hover:bg-red-600 disabled:opacity-50"
          >
            🚨 Agent Error
          </button>
          <button
            onClick={() =>
              handlePlayConvenience(
                "Agent Attention",
                agentSounds.playAgentAttention
              )
            }
            disabled={isLoading}
            className="px-4 py-2 bg-orange-500 text-white rounded hover:bg-orange-600 disabled:opacity-50"
          >
            🎯 Agent Attention
          </button>
        </div>
      </section>

      {/* Voice Sounds */}
      <section className="mb-8">
        <h2 className="text-2xl font-semibold mb-4">
          Voice & Dictation Sounds
        </h2>
        <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
          <button
            onClick={() =>
              handlePlayConvenience("Voice Start", voiceSounds.playVoiceStart)
            }
            disabled={isLoading}
            className="px-4 py-2 bg-purple-500 text-white rounded hover:bg-purple-600 disabled:opacity-50"
          >
            🎤 Voice Start
          </button>
          <button
            onClick={() =>
              handlePlayConvenience("Voice End", voiceSounds.playVoiceEnd)
            }
            disabled={isLoading}
            className="px-4 py-2 bg-purple-500 text-white rounded hover:bg-purple-600 disabled:opacity-50"
          >
            🔇 Voice End
          </button>
          <button
            onClick={() =>
              handlePlayConvenience(
                "Dictation Start",
                voiceSounds.playDictationStart
              )
            }
            disabled={isLoading}
            className="px-4 py-2 bg-cyan-500 text-white rounded hover:bg-cyan-600 disabled:opacity-50"
          >
            📝 Dictation Start
          </button>
          <button
            onClick={() =>
              handlePlayConvenience(
                "Dictation End",
                voiceSounds.playDictationEnd
              )
            }
            disabled={isLoading}
            className="px-4 py-2 bg-cyan-500 text-white rounded hover:bg-cyan-600 disabled:opacity-50"
          >
            📄 Dictation End
          </button>
        </div>
      </section>

      {/* All Available Sounds */}
      <section className="mb-8">
        <h2 className="text-2xl font-semibold mb-4">
          All Available Sounds ({availableSounds.length})
        </h2>
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-3">
          {availableSounds.map((soundType) => (
            <button
              key={soundType}
              onClick={() => handlePlaySound(soundType)}
              disabled={isLoading}
              className="px-3 py-2 bg-gray-500 text-white rounded hover:bg-gray-600 disabled:opacity-50 text-sm"
            >
              {soundType.replace(/([A-Z])/g, " $1").trim()}
            </button>
          ))}
        </div>
      </section>

      {/* Manual File Path */}
      <section>
        <h2 className="text-2xl font-semibold mb-4">Play Custom Sound File</h2>
        <div className="flex gap-2">
          <input
            type="text"
            placeholder="e.g., sounds/ogg/02 Alerts and Notifications/alert_simple.ogg"
            className="flex-1 px-3 py-2 border rounded"
            onKeyPress={async (e) => {
              if (e.key === "Enter") {
                const filePath = (e.target as HTMLInputElement).value;
                if (filePath) {
                  setIsLoading(true);
                  try {
                    const result = await sound.playSoundFile(filePath);
                    setLastResult(result);
                  } finally {
                    setIsLoading(false);
                  }
                }
              }
            }}
            disabled={isLoading}
          />
          <span className="text-sm text-gray-500 flex items-center">
            Press Enter to play
          </span>
        </div>
      </section>
    </div>
  );
};
