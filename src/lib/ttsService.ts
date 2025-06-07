import { invoke } from "@tauri-apps/api/core";

export type TTSMode = 'local' | 'api';

// Helper type for the logging function
type LogFn = (message: string, level?: string) => void;

// Global variables to track current TTS state
let currentUtterance: SpeechSynthesisUtterance | null = null;
let currentAudio: HTMLAudioElement | null = null;

// Function to set the current audio element from outside (used by App.tsx)
export const setCurrentAudioElement = (audio: HTMLAudioElement | null): void => {
	currentAudio = audio;
};

// Function to get the current audio element
export const getCurrentAudioElement = (): HTMLAudioElement | null => {
	return currentAudio;
};

/**
 * Stops any currently playing TTS.
 */
export const stopTTS = async (logFn?: LogFn): Promise<void> => {
	logFn = logFn || ((msg, level) => console.log(`[TTS-${level || 'info'}] ${msg}`));

	try {
		// Stop backend TTS
		await invoke("stop_tts");
		logFn("Backend TTS stop command sent", "info");
	} catch (error) {
		logFn(`Error stopping backend TTS: ${error}`, "error");
	}

	// Stop local speech synthesis
	if (currentUtterance && 'speechSynthesis' in window) {
		window.speechSynthesis.cancel();
		currentUtterance = null;
		logFn("Local speech synthesis stopped", "info");
	}

	// Stop current audio playback
	if (currentAudio) {
		currentAudio.pause();
		currentAudio.currentTime = 0;
		if (currentAudio.src && currentAudio.src.startsWith("blob:")) {
			URL.revokeObjectURL(currentAudio.src);
		}
		currentAudio = null;
		logFn("Audio playback stopped", "info");
	}
};

/**
 * Speaks text using the browser's Web Speech API.
 */
const speakLocal = (text: string, logFn: LogFn): Promise<void> => {
	return new Promise((resolve, reject) => {
		if ('speechSynthesis' in window) {
			try {
				// Stop any existing speech
				if (currentUtterance) {
					window.speechSynthesis.cancel();
				}

				const utterance = new SpeechSynthesisUtterance(text);
				currentUtterance = utterance;

				utterance.onend = () => {
					logFn("Local speech finished.", "info");
					currentUtterance = null;
					resolve();
				};
				utterance.onerror = (event) => {
					logFn(`Local speech error: ${event.error}`, "error");
					currentUtterance = null;
					reject(new Error(`Speech synthesis error: ${event.error}`));
				};
				utterance.onstart = () => {
					logFn("Local speech started.", "info");
				};

				// Check if speech was cancelled before starting
				if (currentUtterance === utterance) {
					window.speechSynthesis.speak(utterance);
					logFn(`Attempting local speech: "${text}"`, "info");
				} else {
					// Speech was cancelled before it could start
					resolve();
				}

			} catch (error) {
				logFn(`Error initializing local speech: ${error}`, "error");
				currentUtterance = null;
				reject(error);
			}
		} else {
			const errorMsg = "Web Speech API not supported in this browser/WebView.";
			logFn(errorMsg, "error");
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
		// Stop any existing audio
		if (currentAudio) {
			currentAudio.pause();
			currentAudio.currentTime = 0;
			if (currentAudio.src && currentAudio.src.startsWith("blob:")) {
				URL.revokeObjectURL(currentAudio.src);
			}
		}

		const audioUrl: string = await invokeFn("invoke_replicate_tts", { text });

		if (audioUrl) {
			logFn(`Received audio URL from backend: ${audioUrl}`, "info");
			const audio = new Audio(audioUrl);
			currentAudio = audio;

			return new Promise((resolve, reject) => {
				if (!currentAudio) {
					// Audio was stopped before it could start
					resolve();
					return;
				}

				audio.onended = () => {
					logFn("API audio playback finished.", "info");
					if (currentAudio === audio) {
						currentAudio = null;
					}
					resolve();
				};
				audio.onerror = (_err) => {
					const errorDetails = audio.error ? `${audio.error.code}: ${audio.error.message}` : 'Unknown audio playback error';
					logFn(`Error playing API audio: ${errorDetails}`, "error");
					if (currentAudio === audio) {
						currentAudio = null;
					}
					reject(new Error(`Failed to play API audio: ${errorDetails}`));
				};

				// Check if audio was stopped before playing
				if (currentAudio === audio) {
					audio.play()
						.then(() => logFn("API audio playback started.", "info"))
						.catch(err => {
							logFn(`Initial API audio play() error: ${err}`, "error");
							if (currentAudio === audio) {
								currentAudio = null;
							}
							reject(err);
						});
				} else {
					// Audio was stopped before it could start
					resolve();
				}
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
			throw new Error(errorMsg);
		}
		// Let speakApi handle the invocation and playback
		await speakApi(text, logFn, invokeFn);
	} else {
		// Use local Web Speech API
		await speakLocal(text, logFn);
	}
};
