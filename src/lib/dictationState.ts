/**
 * Dictation state as Rust's `DictationState` sends it, read for display.
 *
 * serde writes the enum as `"Idle"`, `"Stopping"` and so on for the unit
 * variants and as `{ "Active": { started_at } }` or `{ "Error": { message } }`
 * for the rest. The app used to compare it against `"active"` and `"idle"`,
 * which Rust never sends, so neither branch ever ran.
 */
export function dictationStateLabel(state: unknown): string {
	if (typeof state === "string") return state.toLowerCase();
	if (state && typeof state === "object") {
		const [variant] = Object.keys(state);
		if (variant) return variant.toLowerCase();
	}
	return "idle";
}

interface ForceResetTarget {
	setDictationState: (state: string) => void;
	setIsDictationActive: (active: boolean) => void;
}

/**
 * The backend reset dictation. Every coordinated stop (Escape, the Stop
 * button) does this whether or not anything was dictating, so it is routine,
 * not an error: put the display back to rest and log it. Nothing is shown to
 * the person.
 */
export function applyDictationForceReset(reason: unknown, target: ForceResetTarget): void {
	console.log("[Dictation State] Reset:", reason);
	target.setDictationState("idle");
	target.setIsDictationActive(false);
}
