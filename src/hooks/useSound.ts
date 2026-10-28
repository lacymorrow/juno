import { invoke } from '@tauri-apps/api/core';
import { useCallback } from 'react';
import { COMMANDS } from '@/lib/constants.generated';

// Juno has three sounds and Rust plays all of them (src-tauri/src/commands/sound.rs).
// The page asks for one: a typed message that did not go through.
export function useSound() {
	const playError = useCallback(async (): Promise<void> => {
		try {
			await invoke(COMMANDS.SOUND_PLAY_ERROR_SOUND);
		} catch (error) {
			console.error('Failed to play error sound:', error);
		}
	}, []);

	return { playError };
}
