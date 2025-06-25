import { invoke } from "@tauri-apps/api/core";

/**
 * Development mode detection utilities
 *
 * These functions are separate from constants to avoid being overwritten
 * when constants are regenerated from Rust.
 */

/**
 * Async development mode detection using Tauri backend
 */
export const isDevelopment = async (): Promise<boolean> => {
	try {
		const result = await invoke<boolean>('get_debug_mode');
		return result;
	} catch (error) {
		console.warn('Failed to check async development mode:', error);
		return false;
	}
};
