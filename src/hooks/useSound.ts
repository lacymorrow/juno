import { invoke } from '@tauri-apps/api/core';
import { useCallback } from 'react';
import { SoundPlayResult, SoundSystem, SoundType } from '../types/sound';

// Sound management to prevent overlapping
let lastSoundTime = 0;
let lastSoundType: SoundType | null = null;
const SOUND_DEBOUNCE_MS = 300; // Minimum time between sounds

export function useSound(): SoundSystem {
	// Play a sound by type
	const playSound = useCallback(async (soundType: SoundType): Promise<SoundPlayResult> => {
		const now = Date.now();

		// Prevent rapid duplicate sounds
		if (lastSoundType === soundType && (now - lastSoundTime) < SOUND_DEBOUNCE_MS) {
			console.log(`[Sound] Debouncing duplicate sound: ${soundType}`);
			return { success: false, message: "Sound debounced to prevent overlap" };
		}

		try {
			const result = await invoke<SoundPlayResult>('play_sound_by_type', {
				soundType
			});
			lastSoundTime = now;
			lastSoundType = soundType;
			return result;
		} catch (error) {
			console.error('Failed to play sound:', error);
			return {
				success: false,
				message: `Failed to play sound: ${error}`,
			};
		}
	}, []);

	// Play a sound file by path
	const playSoundFile = useCallback(async (filePath: string): Promise<SoundPlayResult> => {
		try {
			return await invoke<SoundPlayResult>('play_sound_file', {
				filePath
			});
		} catch (error) {
			console.error('Failed to play sound file:', error);
			return {
				success: false,
				message: `Failed to play sound file: ${error}`,
			};
		}
	}, []);

	// Convenience function for notifications
	const playNotification = useCallback(async (): Promise<SoundPlayResult> => {
		return await playSound(SoundType.NotificationSimple01);
	}, [playSound]);

	// Convenience function for success sounds
	const playSuccess = useCallback(async (): Promise<SoundPlayResult> => {
		return await playSound(SoundType.HeroSimpleCelebration01);
	}, [playSound]);

	// Convenience function for error sounds
	const playError = useCallback(async (): Promise<SoundPlayResult> => {
		return await playSound(SoundType.AlertSimple);
	}, [playSound]);

	// Convenience function for alert sounds
	const playAlert = useCallback(async (): Promise<SoundPlayResult> => {
		return await playSound(SoundType.AlertHighIntensity);
	}, [playSound]);

	// Get list of available sounds
	const getAvailableSounds = useCallback(async (): Promise<SoundType[]> => {
		try {
			return await invoke<SoundType[]>('get_available_sounds');
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
	const { playSound } = useSound();

	const playVoiceStart = useCallback(async () => {
		return await playSound(SoundType.NotificationAmbient); // Gentle start sound
	}, [playSound]);

	const playVoiceEnd = useCallback(async () => {
		return await playSound(SoundType.NotificationDecorative01); // Pleasant end sound
	}, [playSound]);

	const playDictationStart = useCallback(async () => {
		return await playSound(SoundType.RingtoneMinimal); // Distinct dictation start
	}, [playSound]);

	const playDictationEnd = useCallback(async () => {
		return await playSound(SoundType.NotificationDecorative02); // Different end sound
	}, [playSound]);

	const playVoiceError = useCallback(async () => {
		return await playSound(SoundType.AlertSimple); // Error sound
	}, [playSound]);

	return {
		playVoiceStart,
		playVoiceEnd,
		playDictationStart,
		playDictationEnd,
		playVoiceError,
	};
}
