import { open } from "@tauri-apps/api/dialog";
import React from "react";
import {
  onDictationStarted,
  onDictationStopped,
  onFinalResult,
  onPartialResult,
  startDictation,
  stopDictation,
  transcribeFile,
  useVoiceTranscription,
} from "tauri-plugin-voice-transcription-api";

// Example 1: Using the hook
function DictationWithHook() {
  const {
    isListening,
    transcript,
    partialTranscript,
    startListening,
    stopListening,
    toggleListening,
  } = useVoiceTranscription();

  return (
    <div className="dictation-hook">
      <h2>Voice Dictation (Hook)</h2>
      <div className="status">
        Status: {isListening ? "🔴 Recording" : "⚫ Stopped"}
      </div>
      <div className="controls">
        <button onClick={startListening} disabled={isListening}>
          Start
        </button>
        <button onClick={stopListening} disabled={!isListening}>
          Stop
        </button>
        <button onClick={toggleListening}>Toggle</button>
      </div>
      <div className="transcripts">
        {partialTranscript && (
          <div className="partial">
            <strong>Partial:</strong> {partialTranscript}
          </div>
        )}
        {transcript && (
          <div className="final">
            <strong>Final:</strong> {transcript}
          </div>
        )}
      </div>
    </div>
  );
}

// Example 2: Using the API directly
function DictationWithAPI() {
  const [isListening, setIsListening] = React.useState(false);
  const [transcript, setTranscript] = React.useState("");
  const [partialTranscript, setPartialTranscript] = React.useState("");

  React.useEffect(() => {
    const unlisteners: Array<() => void> = [];

    // Set up event listeners
    (async () => {
      unlisteners.push(
        await onDictationStarted(() => {
          console.log("Dictation started");
          setIsListening(true);
        })
      );

      unlisteners.push(
        await onDictationStopped(() => {
          console.log("Dictation stopped");
          setIsListening(false);
        })
      );

      unlisteners.push(
        await onPartialResult((event) => {
          setPartialTranscript(event.payload.text);
        })
      );

      unlisteners.push(
        await onFinalResult((event) => {
          setTranscript(event.payload.text);
          setPartialTranscript("");
        })
      );
    })();

    return () => {
      unlisteners.forEach((fn) => fn());
    };
  }, []);

  const handleStart = async () => {
    try {
      await startDictation();
      setTranscript("");
      setPartialTranscript("");
    } catch (error) {
      console.error("Failed to start dictation:", error);
    }
  };

  const handleStop = async () => {
    try {
      await stopDictation();
    } catch (error) {
      console.error("Failed to stop dictation:", error);
    }
  };

  return (
    <div className="dictation-api">
      <h2>Voice Dictation (Direct API)</h2>
      <div className="status">
        Status: {isListening ? "🔴 Recording" : "⚫ Stopped"}
      </div>
      <div className="controls">
        <button onClick={handleStart} disabled={isListening}>
          Start Recording
        </button>
        <button onClick={handleStop} disabled={!isListening}>
          Stop Recording
        </button>
      </div>
      <div className="results">
        <div className="partial-result">
          <h3>Partial Result:</h3>
          <p>{partialTranscript || "Waiting for speech..."}</p>
        </div>
        <div className="final-result">
          <h3>Final Result:</h3>
          <p>{transcript || "No transcription yet"}</p>
        </div>
      </div>
    </div>
  );
}

// Example 3: File transcription
function FileTranscription() {
  const [transcribing, setTranscribing] = React.useState(false);
  const [result, setResult] = React.useState("");
  const [error, setError] = React.useState("");

  const handleSelectFile = async () => {
    try {
      const selected = await open({
        multiple: false,
        filters: [
          {
            name: "Audio",
            extensions: ["wav", "mp3", "m4a"],
          },
        ],
      });

      if (selected && typeof selected === "string") {
        setTranscribing(true);
        setError("");
        setResult("");

        const transcription = await transcribeFile(selected);
        setResult(transcription);
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : "Unknown error");
    } finally {
      setTranscribing(false);
    }
  };

  return (
    <div className="file-transcription">
      <h2>File Transcription</h2>
      <button onClick={handleSelectFile} disabled={transcribing}>
        {transcribing ? "Transcribing..." : "Select Audio File"}
      </button>
      {error && <div className="error">Error: {error}</div>}
      {result && (
        <div className="result">
          <h3>Transcription:</h3>
          <p>{result}</p>
        </div>
      )}
    </div>
  );
}

// Main App
export default function App() {
  return (
    <div className="app">
      <h1>Voice Transcription Plugin Demo</h1>
      <div className="examples">
        <DictationWithHook />
        <hr />
        <DictationWithAPI />
        <hr />
        <FileTranscription />
      </div>
    </div>
  );
}
