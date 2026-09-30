import { invoke } from '@tauri-apps/api/core';
import { useCallback } from 'react';
import { SoundPlayResult, SoundSystem, SoundType } from '../types/sound';
import { COMMANDS } from '@/lib/constants.generated';

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
			const result = await invoke<SoundPlayResult>(COMMANDS.SOUND_PLAY_SOUND_BY_TYPE, {
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
			return await invoke<SoundPlayResult>(COMMANDS.SOUND_PLAY_SOUND_FILE, {
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
		return await invoke<SoundPlayResult>(COMMANDS.SOUND_PLAY_NOTIFICATION_SOUND);
	}, []);

	const playSuccess = useCallback(async (): Promise<SoundPlayResult> => {
		return await invoke<SoundPlayResult>(COMMANDS.SOUND_PLAY_SUCCESS_SOUND);
	}, []);

	const playError = useCallback(async (): Promise<SoundPlayResult> => {
		return await invoke<SoundPlayResult>(COMMANDS.SOUND_PLAY_ERROR_SOUND);
	}, []);

	const playAlert = useCallback(async (): Promise<SoundPlayResult> => {
		return await invoke<SoundPlayResult>(COMMANDS.SOUND_PLAY_ALERT_SOUND);
	}, []);

	// Get list of available sounds
	const getAvailableSounds = useCallback(async (): Promise<SoundType[]> => {
		try {
			return await invoke<SoundType[]>(COMMANDS.SOUND_GET_AVAILABLE_SOUNDS);
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
		return await invoke<SoundPlayResult>(COMMANDS.SOUND_PLAY_AGENT_START_SOUND);
	}, []);

	const playAgentSuccess = useCallback(async (): Promise<SoundPlayResult> => {
		return await invoke<SoundPlayResult>(COMMANDS.SOUND_PLAY_AGENT_SUCCESS_SOUND);
	}, []);

	const playAgentError = useCallback(async (): Promise<SoundPlayResult> => {
		return await invoke<SoundPlayResult>(COMMANDS.SOUND_PLAY_AGENT_ERROR_SOUND);
	}, []);

	const playAgentAttention = useCallback(async (): Promise<SoundPlayResult> => {
		return await invoke<SoundPlayResult>(COMMANDS.SOUND_PLAY_AGENT_ATTENTION_SOUND);
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
		return await invoke<SoundPlayResult>(COMMANDS.SOUND_PLAY_VOICE_START_SOUND);
	}, []);

	const playVoiceEnd = useCallback(async (): Promise<SoundPlayResult> => {
		return await invoke<SoundPlayResult>(COMMANDS.SOUND_PLAY_VOICE_END_SOUND);
	}, []);

	const playDictationStart = useCallback(async (): Promise<SoundPlayResult> => {
		return await invoke<SoundPlayResult>(COMMANDS.SOUND_PLAY_DICTATION_START_SOUND);
	}, []);

	const playDictationEnd = useCallback(async (): Promise<SoundPlayResult> => {
		return await invoke<SoundPlayResult>(COMMANDS.SOUND_PLAY_DICTATION_END_SOUND);
	}, []);

	const playVoiceError = useCallback(async (): Promise<SoundPlayResult> => {
		return await invoke<SoundPlayResult>(COMMANDS.SOUND_PLAY_VOICE_ERROR_SOUND);
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
		return await invoke<SoundPlayResult>(COMMANDS.SOUND_PLAY_BOOT_SOUND);
	}, []);

	const playSystemReady = useCallback(async (): Promise<SoundPlayResult> => {
		return await invoke<SoundPlayResult>(COMMANDS.SOUND_PLAY_SYSTEM_READY_SOUND);
	}, []);

	const playConnectionSound = useCallback(async (): Promise<SoundPlayResult> => {
		return await invoke<SoundPlayResult>(COMMANDS.SOUND_PLAY_CONNECTION_SOUND);
	}, []);

	const playDisconnectionSound = useCallback(async (): Promise<SoundPlayResult> => {
		return await invoke<SoundPlayResult>(COMMANDS.SOUND_PLAY_DISCONNECTION_SOUND);
	}, []);

	return {
		playBootSound,
		playSystemReady,
		playConnectionSound,
		playDisconnectionSound,
	};
}
