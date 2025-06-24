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
		if (typeof window !== 'undefined') {
			// Tauri v2 API access pattern
			if ((window as any).__TAURI_INTERNALS__?.invoke) {
				// Use Tauri's backend to check debug mode
				const tauriInvoke = (window as any).__TAURI_INTERNALS__.invoke;
				const result = await tauriInvoke('get_debug_mode');
				return result;
			}
		}
		throw new Error('No Tauri environment detected');
	} catch (error) {
		console.warn('Failed to check async development mode:', error);
		return false;
	}
};