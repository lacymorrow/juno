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

/** The message out of a `voice-transcription:error` payload (`{ type, message }`). */
export function voiceErrorMessage(payload: unknown): string {
	if (typeof payload === "string") return payload;
	if (payload && typeof payload === "object" && "message" in payload) {
		const { message } = payload as { message?: unknown };
		if (typeof message === "string") return message;
	}
	return "Transcription failed";
}

/** The level (0 to 1) out of a `voice-transcription:audio-level` payload (`{ level }`). */
export function audioLevelFrom(payload: unknown): number {
	if (payload && typeof payload === "object" && "level" in payload) {
		const { level } = payload as { level?: unknown };
		if (typeof level === "number" && Number.isFinite(level)) return Math.min(Math.max(level, 0), 1);
	}
	return 0;
}
