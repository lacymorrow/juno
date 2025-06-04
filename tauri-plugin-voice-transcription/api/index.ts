import { invoke } from '@tauri-apps/api/core';
import { listen, type Event, type UnlistenFn } from '@tauri-apps/api/event';

// Re-export the UnlistenFn type
export type { UnlistenFn };

/**
 * Partial transcription result event payload
 */
export interface PartialTranscriptionResult {
    text: string;
}

/**
 * Final transcription result event payload
 */
export interface FinalTranscriptionResult {
    text: string;
}

/**
 * Voice transcription events
 */
export const VoiceTranscriptionEvents = {
    DICTATION_STARTED: 'voice-transcription:dictation-started',
    DICTATION_STOPPED: 'voice-transcription:dictation-stopped',
    PARTIAL_RESULT: 'voice-transcription:partial-result',
    FINAL_RESULT: 'voice-transcription:final-result',
} as const;

/**
 * Start voice dictation
 * @returns Promise that resolves when dictation starts
 */
export async function startDictation(): Promise<void> {
    return invoke('plugin:voice-transcription|start_dictation');
}

/**
 * Stop voice dictation
 * @returns Promise that resolves to true if dictation was actively stopped
 */
export async function stopDictation(): Promise<boolean> {
    return invoke('plugin:voice-transcription|stop_dictation');
}

/**
 * Toggle voice dictation on/off
 * @returns Promise that resolves to true if dictation is now active
 */
export async function toggleDictation(): Promise<boolean> {
    return invoke('plugin:voice-transcription|toggle_dictation');
}

/**
 * Get current dictation status
 * @returns Promise that resolves to true if currently dictating
 */
export async function getDictationStatus(): Promise<boolean> {
    return invoke('plugin:voice-transcription|get_dictation_status');
}

/**
 * Transcribe an audio file
 * @param filePath - Path to the audio file to transcribe
 * @returns Promise that resolves to the transcribed text
 */
export async function transcribeFile(filePath: string): Promise<string> {
    return invoke('plugin:voice-transcription|transcribe_file', { filePath });
}

/**
 * Set the Whisper model path
 * @param modelPath - Path to the Whisper model file
 * @returns Promise that resolves when the model is loaded
 */
export async function setModelPath(modelPath: string): Promise<void> {
    return invoke('plugin:voice-transcription|set_model_path', { modelPath });
}

/**
 * Get the current Whisper model path
 * @returns Promise that resolves to the current model path
 */
export async function getModelPath(): Promise<string> {
    return invoke('plugin:voice-transcription|get_model_path');
}

/**
 * Listen for dictation started events
 * @param handler - Event handler
 * @returns Promise that resolves to an unlisten function
 */
export async function onDictationStarted(
    handler: (event: Event<void>) => void
): Promise<UnlistenFn> {
    return listen(VoiceTranscriptionEvents.DICTATION_STARTED, handler);
}

/**
 * Listen for dictation stopped events
 * @param handler - Event handler
 * @returns Promise that resolves to an unlisten function
 */
export async function onDictationStopped(
    handler: (event: Event<void>) => void
): Promise<UnlistenFn> {
    return listen(VoiceTranscriptionEvents.DICTATION_STOPPED, handler);
}

/**
 * Listen for partial transcription results
 * @param handler - Event handler
 * @returns Promise that resolves to an unlisten function
 */
export async function onPartialResult(
    handler: (event: Event<PartialTranscriptionResult>) => void
): Promise<UnlistenFn> {
    return listen(VoiceTranscriptionEvents.PARTIAL_RESULT, handler);
}

/**
 * Listen for final transcription results
 * @param handler - Event handler
 * @returns Promise that resolves to an unlisten function
 */
export async function onFinalResult(
    handler: (event: Event<FinalTranscriptionResult>) => void
): Promise<UnlistenFn> {
    return listen(VoiceTranscriptionEvents.FINAL_RESULT, handler);
}
