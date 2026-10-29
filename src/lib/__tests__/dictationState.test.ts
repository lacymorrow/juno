import { describe, expect, it, vi } from "vitest";

const toastError = vi.fn();
vi.mock("sonner", () => ({ toast: { error: toastError, info: vi.fn() } }));

import {
	applyDictationForceReset,
	audioLevelFrom,
	dictationStateLabel,
	voiceErrorMessage,
} from "@/lib/dictationState";

describe("dictationStateLabel", () => {
	it("reads the unit variants Rust sends", () => {
		expect(dictationStateLabel("Idle")).toBe("idle");
		expect(dictationStateLabel("Stopping")).toBe("stopping");
	});

	it("reads the variants that carry data", () => {
		expect(dictationStateLabel({ Active: { started_at: 1 } })).toBe("active");
		expect(dictationStateLabel({ Error: { message: "x" } })).toBe("error");
	});

	it("falls back to idle for anything it cannot read", () => {
		expect(dictationStateLabel(null)).toBe("idle");
		expect(dictationStateLabel({})).toBe("idle");
	});
});

describe("applyDictationForceReset", () => {
	it("puts the display to rest without telling the person about it", () => {
		const setDictationState = vi.fn();
		const setIsDictationActive = vi.fn();
		const log = vi.spyOn(console, "log").mockImplementation(() => {});

		applyDictationForceReset("Force stop dictation", { setDictationState, setIsDictationActive });

		expect(setDictationState).toHaveBeenCalledWith("idle");
		expect(setIsDictationActive).toHaveBeenCalledWith(false);
		expect(toastError).not.toHaveBeenCalled();
		expect(log).toHaveBeenCalled();
		log.mockRestore();
	});
});

describe("voiceErrorMessage", () => {
	it("reads the message out of the plugin's payload", () => {
		expect(voiceErrorMessage({ type: "transcription_failed", message: "Final transcription failed: x" })).toBe(
			"Final transcription failed: x",
		);
	});

	it("never stores the payload object as the message", () => {
		expect(voiceErrorMessage({ type: "transcription_failed" })).toBe("Transcription failed");
		expect(voiceErrorMessage("plain")).toBe("plain");
	});
});

describe("audioLevelFrom", () => {
	it("reads the plugin's { level } payload", () => {
		expect(audioLevelFrom({ level: 0.42 })).toBe(0.42);
	});

	it("is zero for anything else", () => {
		expect(audioLevelFrom({})).toBe(0);
		expect(audioLevelFrom({ level: Number.NaN })).toBe(0);
		expect(audioLevelFrom(0.5)).toBe(0);
		expect(audioLevelFrom(null)).toBe(0);
	});
});
