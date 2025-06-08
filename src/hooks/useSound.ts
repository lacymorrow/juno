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

	// Convenience functions - now just call backend commands
	const playNotification = useCallback(async (): Promise<SoundPlayResult> => {
		return await invoke<SoundPlayResult>('play_notification_sound');
	}, []);

	const playSuccess = useCallback(async (): Promise<SoundPlayResult> => {
		return await invoke<SoundPlayResult>('play_success_sound');
	}, []);

	const playError = useCallback(async (): Promise<SoundPlayResult> => {
		return await invoke<SoundPlayResult>('play_error_sound');
	}, []);

	const playAlert = useCallback(async (): Promise<SoundPlayResult> => {
		return await invoke<SoundPlayResult>('play_alert_sound');
	}, []);

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

// Additional hooks for specific sound scenarios - now use backend commands

export function useAgentSounds() {
	const playAgentStart = useCallback(async (): Promise<SoundPlayResult> => {
		return await invoke<SoundPlayResult>('play_agent_start_sound');
	}, []);

	const playAgentSuccess = useCallback(async (): Promise<SoundPlayResult> => {
		return await invoke<SoundPlayResult>('play_agent_success_sound');
	}, []);

	const playAgentError = useCallback(async (): Promise<SoundPlayResult> => {
		return await invoke<SoundPlayResult>('play_agent_error_sound');
	}, []);

	const playAgentAttention = useCallback(async (): Promise<SoundPlayResult> => {
		return await invoke<SoundPlayResult>('play_agent_attention_sound');
	}, []);

	return {
		playAgentStart,
		playAgentSuccess,
		playAgentError,
		playAgentAttention,
	};
}

export function useVoiceSounds() {
	const playVoiceStart = useCallback(async (): Promise<SoundPlayResult> => {
		return await invoke<SoundPlayResult>('play_voice_start_sound');
	}, []);

	const playVoiceEnd = useCallback(async (): Promise<SoundPlayResult> => {
		return await invoke<SoundPlayResult>('play_voice_end_sound');
	}, []);

	const playDictationStart = useCallback(async (): Promise<SoundPlayResult> => {
		return await invoke<SoundPlayResult>('play_dictation_start_sound');
	}, []);

	const playDictationEnd = useCallback(async (): Promise<SoundPlayResult> => {
		return await invoke<SoundPlayResult>('play_dictation_end_sound');
	}, []);

	const playVoiceError = useCallback(async (): Promise<SoundPlayResult> => {
		return await invoke<SoundPlayResult>('play_voice_error_sound');
	}, []);

	return {
		playVoiceStart,
		playVoiceEnd,
		playDictationStart,
		playDictationEnd,
		playVoiceError,
	};
}

// Additional system sound hooks

export function useSystemSounds() {
	const playBootSound = useCallback(async (): Promise<SoundPlayResult> => {
		return await invoke<SoundPlayResult>('play_boot_sound');
	}, []);

	const playSystemReady = useCallback(async (): Promise<SoundPlayResult> => {
		return await invoke<SoundPlayResult>('play_system_ready_sound');
	}, []);

	const playConnectionSound = useCallback(async (): Promise<SoundPlayResult> => {
		return await invoke<SoundPlayResult>('play_connection_sound');
	}, []);

	const playDisconnectionSound = useCallback(async (): Promise<SoundPlayResult> => {
		return await invoke<SoundPlayResult>('play_disconnection_sound');
	}, []);

	return {
		playBootSound,
		playSystemReady,
		playConnectionSound,
		playDisconnectionSound,
	};
}
