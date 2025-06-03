import { useCallback, useEffect, useState } from 'react';
import {
    getDictationStatus,
    onDictationStarted,
    onDictationStopped,
    onFinalResult,
    onPartialResult,
    startDictation,
    stopDictation,
    toggleDictation,
    type UnlistenFn,
} from './index';

/**
 * React hook for voice transcription
 * @example
 * ```tsx
 * const { isListening, transcript, startListening, stopListening } = useVoiceTranscription();
 * ```
 */
export function useVoiceTranscription() {
    const [isListening, setIsListening] = useState(false);
    const [transcript, setTranscript] = useState('');
    const [partialTranscript, setPartialTranscript] = useState('');

    useEffect(() => {
        const unlisteners: UnlistenFn[] = [];

        (async () => {
            // Check initial status
            const status = await getDictationStatus();
            setIsListening(status);

            // Listen for events
            unlisteners.push(await onDictationStarted(() => setIsListening(true)));
            unlisteners.push(await onDictationStopped(() => setIsListening(false)));
            unlisteners.push(await onPartialResult((event) => setPartialTranscript(event.payload.text)));
            unlisteners.push(await onFinalResult((event) => {
                setTranscript(event.payload.text);
                setPartialTranscript('');
            }));
        })();

        return () => {
            unlisteners.forEach(fn => fn());
        };
    }, []);

    const startListening = useCallback(async () => {
        try {
            await startDictation();
            setTranscript('');
            setPartialTranscript('');
        } catch (error) {
            console.error('Failed to start dictation:', error);
        }
    }, []);

    const stopListening = useCallback(async () => {
        try {
            await stopDictation();
        } catch (error) {
            console.error('Failed to stop dictation:', error);
        }
    }, []);

    const toggleListening = useCallback(async () => {
        try {
            const newState = await toggleDictation();
            if (newState) {
                setTranscript('');
                setPartialTranscript('');
            }
        } catch (error) {
            console.error('Failed to toggle dictation:', error);
        }
    }, []);

    return {
        isListening,
        transcript,
        partialTranscript,
        startListening,
        stopListening,
        toggleListening,
    };
}
