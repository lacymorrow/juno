import { useState, useCallback, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { setCurrentAudioElement, stopTTS } from "@/lib/ttsService";

export function useAudioPlayback() {
    const [currentAudio, setCurrentAudio] = useState<HTMLAudioElement | null>(null);

    // Helper function to convert base64 to Blob
    const base64ToBlob = useCallback((base64: string, contentType = "audio/mpeg"): Blob => {
        const byteCharacters = atob(base64);
        const byteNumbers = new Array(byteCharacters.length);
        for (let i = 0; i < byteCharacters.length; i++) {
            byteNumbers[i] = byteCharacters.charCodeAt(i);
        }
        const byteArray = new Uint8Array(byteNumbers);
        return new Blob([byteArray], { type: contentType });
    }, []);

    // Helper function to play audio from base64 data
    const playAudioFromBase64 = useCallback(async (base64Audio: string) => {
        // Stop any currently playing audio
        if (currentAudio) {
            currentAudio.pause();
            currentAudio.currentTime = 0;
            if (currentAudio.src && currentAudio.src.startsWith("blob:")) {
                URL.revokeObjectURL(currentAudio.src);
            }
            currentAudio.src = ""; // Clear the source
        }

        try {
            const audioBlob = base64ToBlob(base64Audio);
            const audioUrl = URL.createObjectURL(audioBlob);
            const newAudio = new Audio(audioUrl);
            setCurrentAudio(newAudio); // Store the new audio element
            setCurrentAudioElement(newAudio); // Sync with TTS service

            newAudio.play();

            newAudio.onended = () => {
                URL.revokeObjectURL(audioUrl); // Clean up object URL
                setCurrentAudio(null);
                setCurrentAudioElement(null); // Sync with TTS service

                // Notify backend that TTS has finished so it can play the success sound
                invoke("handle_tts_completion").catch((error) => {
                    console.error("Failed to notify backend of TTS completion:", error);
                });
            };
            newAudio.onerror = (e) => {
                console.error("Audio playback error:", e);
                URL.revokeObjectURL(audioUrl); // Clean up object URL
                setCurrentAudio(null);
                setCurrentAudioElement(null); // Sync with TTS service
            };
        } catch (error) {
            console.error("Error processing or playing audio:", error);
            setCurrentAudio(null);
            setCurrentAudioElement(null); // Sync with TTS service
        }
    }, [currentAudio, base64ToBlob]);

    // Stop current audio playback
    const stopCurrentAudio = useCallback(async () => {
        if (currentAudio) {
            console.log("Stopping current audio element");
            currentAudio.pause();
            currentAudio.currentTime = 0;
            if (currentAudio.src && currentAudio.src.startsWith("blob:")) {
                URL.revokeObjectURL(currentAudio.src);
            }
            currentAudio.src = "";
            setCurrentAudio(null);
            setCurrentAudioElement(null);
        }
    }, [currentAudio]);

    // Comprehensive stop all audio operations
    const stopAllAudio = useCallback(async () => {
        try {
            // Stop current audio element
            stopCurrentAudio();

            // Also call the TTS service stop function
            await stopTTS((msg, level) =>
                console.log(`[Audio Stop-${level || "info"}] ${msg}`)
            );

            // Call backend stop operations
            await invoke("stop_all_operations");
        } catch (error) {
            console.error("Error stopping all audio operations:", error);
        }
    }, [stopCurrentAudio]);

    // Cleanup effect for audio
    useEffect(() => {
        return () => {
            if (currentAudio) {
                currentAudio.pause();
                currentAudio.currentTime = 0; // Reset playback position
                if (currentAudio.src && currentAudio.src.startsWith("blob:")) {
                    URL.revokeObjectURL(currentAudio.src);
                }
                setCurrentAudio(null); // Clear the audio reference
                setCurrentAudioElement(null); // Sync with TTS service
            }
        };
    }, [currentAudio]);

    return {
        // State
        currentAudio,

        // Actions
        playAudioFromBase64,
        stopCurrentAudio,
        stopAllAudio,

        // Utilities
        base64ToBlob,
    };
}
