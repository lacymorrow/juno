// Audio utility functions

export function base64ToBlob(base64: string, contentType = "audio/mpeg"): Blob {
  const byteCharacters = atob(base64);
  const byteNumbers = new Array(byteCharacters.length);
  for (let i = 0; i < byteCharacters.length; i++) {
    byteNumbers[i] = byteCharacters.charCodeAt(i);
  }
  const byteArray = new Uint8Array(byteNumbers);
  return new Blob([byteArray], { type: contentType });
}

export const playAudioFromBase64 = (
  base64Audio: string,
  setCurrentAudio: (audio: HTMLAudioElement | null) => void,
  currentAudio: HTMLAudioElement | null
) => {
  try {
    // Stop any currently playing audio
    if (currentAudio) {
      console.log("🔇 Stopping currently playing audio");
      currentAudio.pause();
      currentAudio.currentTime = 0;
    }

    // Convert base64 to blob and play
    const audioBlob = base64ToBlob(base64Audio);
    const audioUrl = URL.createObjectURL(audioBlob);
    const audio = new Audio(audioUrl);

    // Set audio properties
    audio.volume = 0.8;
    audio.preload = "auto";

    // Set up event listeners
    audio.addEventListener("loadstart", () => {
      console.log("🎵 Audio loading started");
    });

    audio.addEventListener("canplaythrough", () => {
      console.log("🎵 Audio can play through");
    });

    audio.addEventListener("play", () => {
      console.log("🎵 Audio playback started");
    });

    audio.addEventListener("ended", () => {
      console.log("🎵 Audio playback ended");
      setCurrentAudio(null);
      URL.revokeObjectURL(audioUrl); // Clean up object URL
    });

    audio.addEventListener("error", (e) => {
      console.error("❌ Audio playback error:", e);
      setCurrentAudio(null);
      URL.revokeObjectURL(audioUrl); // Clean up object URL
    });

    // Store reference and play
    setCurrentAudio(audio);
    console.log("🎵 Starting audio playback");
    audio.play().catch((error) => {
      console.error("❌ Failed to play audio:", error);
      setCurrentAudio(null);
      URL.revokeObjectURL(audioUrl); // Clean up object URL
    });
  } catch (error) {
    console.error("❌ Error processing audio:", error);
  }
};