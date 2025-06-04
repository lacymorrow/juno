import { invoke } from '@tauri-apps/api/core';
import { useCallback } from 'react';
import { SoundPlayResult, SoundSystem, SoundType } from '../types/sound';

export function useSound(): SoundSystem {
    // Play a sound by type
    const playSound = useCallback(async (soundType: SoundType): Promise<SoundPlayResult> => {
        try {
            const result = await invoke<SoundPlayResult>('play_sound_by_type', {
                soundType
            });
            return result;
        } catch (error) {
            console.error('Failed to play sound:', error);
            return {
                success: false,
                message: `Failed to play sound: ${error}`,
                file_path: undefined,
            };
        }
    }, []);

    // Play a sound file by path
    const playSoundFile = useCallback(async (filePath: string): Promise<SoundPlayResult> => {
        try {
            const result = await invoke<SoundPlayResult>('play_sound_file', {
                filePath
            });
            return result;
        } catch (error) {
            console.error('Failed to play sound file:', error);
            return {
                success: false,
                message: `Failed to play sound file: ${error}`,
                file_path: filePath,
            };
        }
    }, []);

    // Convenience function for notifications
    const playNotification = useCallback(async (): Promise<SoundPlayResult> => {
        try {
            const result = await invoke<SoundPlayResult>('play_notification_sound');
            return result;
        } catch (error) {
            console.error('Failed to play notification sound:', error);
            return {
                success: false,
                message: `Failed to play notification sound: ${error}`,
            };
        }
    }, []);

    // Convenience function for success sounds
    const playSuccess = useCallback(async (): Promise<SoundPlayResult> => {
        try {
            const result = await invoke<SoundPlayResult>('play_success_sound');
            return result;
        } catch (error) {
            console.error('Failed to play success sound:', error);
            return {
                success: false,
                message: `Failed to play success sound: ${error}`,
            };
        }
    }, []);

    // Convenience function for error sounds
    const playError = useCallback(async (): Promise<SoundPlayResult> => {
        try {
            const result = await invoke<SoundPlayResult>('play_error_sound');
            return result;
        } catch (error) {
            console.error('Failed to play error sound:', error);
            return {
                success: false,
                message: `Failed to play error sound: ${error}`,
            };
        }
    }, []);

    // Convenience function for alert sounds
    const playAlert = useCallback(async (): Promise<SoundPlayResult> => {
        try {
            const result = await invoke<SoundPlayResult>('play_alert_sound');
            return result;
        } catch (error) {
            console.error('Failed to play alert sound:', error);
            return {
                success: false,
                message: `Failed to play alert sound: ${error}`,
            };
        }
    }, []);

    // Get list of available sounds
    const getAvailableSounds = useCallback(async (): Promise<SoundType[]> => {
        try {
            const result = await invoke<SoundType[]>('get_available_sounds');
            return result;
        } catch (error) {
            console.error('Failed to get available sounds:', error);
            return [];
        }
    }, []);

    return {
        playSound,
        playSoundFile,
        playNotification,
        playSuccess,
        playError,
        playAlert,
        getAvailableSounds,
    };
}

// Additional hooks for specific sound scenarios

export function useAgentSounds() {
    const { playSuccess, playError, playNotification, playAlert } = useSound();

    const playAgentStart = useCallback(async () => {
        return await playNotification();
    }, [playNotification]);

    const playAgentSuccess = useCallback(async () => {
        return await playSuccess();
    }, [playSuccess]);

    const playAgentError = useCallback(async () => {
        return await playError();
    }, [playError]);

    const playAgentAttention = useCallback(async () => {
        return await playAlert();
    }, [playAlert]);

    return {
        playAgentStart,
        playAgentSuccess,
        playAgentError,
        playAgentAttention,
    };
}

export function useVoiceSounds() {
    const { playSound, playNotification, playError } = useSound();

    const playVoiceStart = useCallback(async () => {
        // Use a gentle notification for voice start
        return await playSound(SoundType.NotificationAmbient);
    }, [playSound]);

    const playVoiceEnd = useCallback(async () => {
        // Use a simple notification for voice end
        return await playNotification();
    }, [playNotification]);

    const playVoiceError = useCallback(async () => {
        // Use error sound for voice/transcription errors
        return await playError();
    }, [playError]);

    const playDictationStart = useCallback(async () => {
        // Use a simple notification for dictation start
        return await playSound(SoundType.NotificationSimple01);
    }, [playSound]);

    const playDictationEnd = useCallback(async () => {
        // Use a slightly different sound for dictation end
        return await playSound(SoundType.NotificationSimple02);
    }, [playSound]);

    return {
        playVoiceStart,
        playVoiceEnd,
        playVoiceError,
        playDictationStart,
        playDictationEnd,
    };
}
