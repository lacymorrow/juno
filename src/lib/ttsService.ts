import { invoke } from "@tauri-apps/api/core";

export type TTSMode = 'local' | 'api';

// Helper type for the logging function
type LogFn = (message: string, level?: string) => void;

/**
 * Speaks text using the browser's Web Speech API.
 */
const speakLocal = (text: string, logFn: LogFn): Promise<void> => {
	return new Promise((resolve, reject) => {
		if ('speechSynthesis' in window) {
			try {
				const utterance = new SpeechSynthesisUtterance(text);
				utterance.onend = () => {
					logFn("Local speech finished.", "info");
					resolve();
				};
				utterance.onerror = (event) => {
					logFn(`Local speech error: ${event.error}`, "error");
					reject(new Error(`Speech synthesis error: ${event.error}`));
				};
				// utterance.onstart = () => logFn("Local speech started.", "info");

				window.speechSynthesis.speak(utterance);
				logFn(`Attempting local speech: "${text}"`, "info");

			} catch (error) {
				logFn(`Error initializing local speech: ${error}`, "error");
				reject(error);
			}
		} else {
			const errorMsg = "Web Speech API not supported in this browser/WebView.";
			logFn(errorMsg, "error");
			// Optionally provide user feedback via alert in the main component
			reject(new Error(errorMsg));
		}
	});
};

/**
 * Speaks text using the Replicate API via the Tauri backend.
 */
const speakApi = async (text: string, logFn: LogFn, invokeFn: typeof invoke): Promise<void> => {
	logFn(`Attempting API speech: "${text}"`, "info");
	try {
		// Ensure invokeFn is correctly typed if needed, though 'invoke' from tauri usually works
		const audioUrl: string = await invokeFn("invoke_replicate_tts", { text });

		if (audioUrl) {
			logFn(`Received audio URL from backend: ${audioUrl}`, "info");
			const audio = new Audio(audioUrl);

			return new Promise((resolve, reject) => {
				audio.onended = () => {
					logFn("API audio playback finished.", "info");
					resolve();
				};
				audio.onerror = (_err) => {
					// The event itself might not be very informative, log the element's error
					const errorDetails = audio.error ? `${audio.error.code}: ${audio.error.message}` : 'Unknown audio playback error';
					logFn(`Error playing API audio: ${errorDetails}`, "error");
					reject(new Error(`Failed to play API audio: ${errorDetails}`));
				};
				audio.play()
					.then(() => logFn("API audio playback started.", "info"))
					.catch(err => { // Catch potential initial play error
						logFn(`Initial API audio play() error: ${err}`, "error");
						reject(err);
					});
			});

		} else {
			logFn("Backend returned empty audio URL.", "error");
			throw new Error("Backend returned empty audio URL.");
		}
	} catch (error) {
		logFn(`Error invoking or handling Replicate TTS backend: ${error}`, "error");
		// Re-throw the error so the caller can handle it (e.g., show alert)
		throw error;
	}
};

/**
 * Synthesizes speech using the specified mode (local or API).
 * Handles online check for API mode.
 */
export const synthesizeSpeech = async (
	text: string,
	mode: TTSMode,
	logFn: LogFn,
	invokeFn: typeof invoke // Pass invoke function explicitly
): Promise<void> => {

	if (!text) {
		logFn("Synthesize speech called with empty text.", "warn");
		return;
	}

	if (mode === 'api') {
		if (!navigator.onLine) {
			const errorMsg = "Offline. Cannot use API TTS.";
			logFn(errorMsg, "warn");
			// Alert is handled by the caller (App.tsx) now based on thrown error
			// alert(errorMsg); // Remove direct alert from service
			throw new Error(errorMsg); // Reject the promise
		}
		// Let speakApi handle the invocation and playback
		await speakApi(text, logFn, invokeFn);
	} else {
		// Use local Web Speech API
		await speakLocal(text, logFn);
	}
};
