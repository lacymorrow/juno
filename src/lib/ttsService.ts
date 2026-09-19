import { invoke } from "@tauri-apps/api/core";

// Helper type for the logging function
type LogFn = (message: string, level?: string) => void;

/**
 * Stops any currently playing TTS.
 *
 * Speech playback lives in Rust: the TTS module spawns `afplay` as a child
 * process and `stop_tts` SIGTERMs it. That is the only speech path today, so
 * this function has no browser audio element or utterance of its own to cancel.
 * If something here ever does play audio in the browser, it owns stopping it.
 */
export const stopTTS = async (logFn?: LogFn): Promise<void> => {
	logFn = logFn || ((msg, level) => console.log(`[TTS-${level || 'info'}] ${msg}`));

	try {
		await invoke("stop_tts");
		logFn("Backend TTS stop command sent", "info");
	} catch (error) {
		logFn(`Error stopping backend TTS: ${error}`, "error");
	}
};
