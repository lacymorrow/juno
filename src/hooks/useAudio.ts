import { useState, useEffect } from "react";

export const useAudio = (addLog: (message: string, level?: string) => void) => {
  const [currentAudio, setCurrentAudio] = useState<HTMLAudioElement | null>(null);

  // Helper function to convert base64 to Blob
  const base64ToBlob = (base64: string, contentType = "audio/mpeg"): Blob => {
    const byteCharacters = atob(base64);
    const byteNumbers = new Array(byteCharacters.length);
    for (let i = 0; i < byteCharacters.length; i++) {
      byteNumbers[i] = byteCharacters.charCodeAt(i);
    }
    const byteArray = new Uint8Array(byteNumbers);
    return new Blob([byteArray], { type: contentType });
  };

  // Helper function to play audio from base64 data
  const playAudioFromBase64 = (base64Audio: string) => {
    // Stop any currently playing audio
    if (currentAudio) {
      currentAudio.pause();
      currentAudio.src = ""; // Release object URL implicitly via new assignment below
      addLog("Stopped previous audio playback.", "debug");
    }

    try {
      const audioBlob = base64ToBlob(base64Audio);
      const audioUrl = URL.createObjectURL(audioBlob);
      const newAudio = new Audio(audioUrl);
      setCurrentAudio(newAudio); // Store the new audio element

      newAudio.play();
      addLog("Starting audio playback.", "info");

      newAudio.onended = () => {
        addLog("Audio playback finished.", "info");
        URL.revokeObjectURL(audioUrl); // Clean up object URL
        setCurrentAudio(null);
      };
      newAudio.onerror = (e) => {
        console.error("Audio playback error:", e);
        addLog(`Audio playback error: ${e}`, "error");
        URL.revokeObjectURL(audioUrl); // Clean up object URL
        setCurrentAudio(null);
      };
    } catch (error) {
      console.error("Error processing or playing audio:", error);
      addLog(`Failed to process or play audio: ${error}`, "error");
      setCurrentAudio(null);
    }
  };

  // Cleanup effect for audio
  useEffect(() => {
    return () => {
      if (currentAudio) {
        currentAudio.pause();
        if (currentAudio.src && currentAudio.src.startsWith("blob:")) {
          URL.revokeObjectURL(currentAudio.src);
        }
        addLog("Cleaned up audio on component unmount.", "debug");
      }
    };
  }, [currentAudio, addLog]);

  return { playAudioFromBase64 };
};