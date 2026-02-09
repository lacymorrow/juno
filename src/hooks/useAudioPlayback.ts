import { useCallback, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { setCurrentAudioElement, stopTTS } from "@/lib/ttsService";

export function useAudioPlayback() {
    // Use ref instead of state to avoid stale closures and unnecessary re-renders.
    // Audio state doesn't drive UI rendering — it's an imperative resource.
    const audioRef = useRef<HTMLAudioElement | null>(null);

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

    // Clean up an audio element and revoke its blob URL
    const cleanupAudio = useCallback((audio: HTMLAudioElement | null) => {
        if (!audio) return;
        audio.pause();
        audio.currentTime = 0;
        if (audio.src && audio.src.startsWith("blob:")) {
            URL.revokeObjectURL(audio.src);
        }
        audio.src = "";
    }, []);

    // Helper function to play audio from base64 data
    const playAudioFromBase64 = useCallback((base64Audio: string) => {
        // Stop any currently playing audio
        cleanupAudio(audioRef.current);
        audioRef.current = null;
        setCurrentAudioElement(null);

        try {
            const audioBlob = base64ToBlob(base64Audio);
            const audioUrl = URL.createObjectURL(audioBlob);
            const newAudio = new Audio(audioUrl);
            audioRef.current = newAudio;
            setCurrentAudioElement(newAudio);

            newAudio.play();

            newAudio.onended = () => {
                URL.revokeObjectURL(audioUrl);
                // Only clear if this is still the current audio
                if (audioRef.current === newAudio) {
                    audioRef.current = null;
                    setCurrentAudioElement(null);
                }

                invoke("handle_tts_completion").catch((error) => {
                    console.error("Failed to notify backend of TTS completion:", error);
                });
            };
            newAudio.onerror = (e) => {
                console.error("Audio playback error:", e);
                URL.revokeObjectURL(audioUrl);
                if (audioRef.current === newAudio) {
                    audioRef.current = null;
                    setCurrentAudioElement(null);
                }
            };
        } catch (error) {
            console.error("Error processing or playing audio:", error);
            audioRef.current = null;
            setCurrentAudioElement(null);
        }
    }, [base64ToBlob, cleanupAudio]);

    // Stop current audio playback
    const stopCurrentAudio = useCallback(() => {
        if (audioRef.current) {
            console.log("Stopping current audio element");
            cleanupAudio(audioRef.current);
            audioRef.current = null;
            setCurrentAudioElement(null);
        }
    }, [cleanupAudio]);

    // Comprehensive stop all audio operations
    const stopAllAudio = useCallback(async () => {
        try {
            stopCurrentAudio();

            await stopTTS((msg, level) =>
                console.log(`[Audio Stop-${level || "info"}] ${msg}`)
            );

            await invoke("stop_all_operations");
        } catch (error) {
            console.error("Error stopping all audio operations:", error);
        }
    }, [stopCurrentAudio]);

    // Cleanup on unmount
    useEffect(() => {
        return () => {
            if (audioRef.current) {
                audioRef.current.pause();
                if (audioRef.current.src && audioRef.current.src.startsWith("blob:")) {
                    URL.revokeObjectURL(audioRef.current.src);
                }
                audioRef.current = null;
                setCurrentAudioElement(null);
            }
        };
    }, []);

    return {
        // State
        currentAudio: audioRef.current,

        // Actions
        playAudioFromBase64,
        stopCurrentAudio,
        stopAllAudio,

        // Utilities
        base64ToBlob,
    };
}
