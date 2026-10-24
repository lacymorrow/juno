import { describe, expect, it, vi } from "vitest";

const toastError = vi.fn();
vi.mock("sonner", () => ({ toast: { error: toastError, info: vi.fn() } }));

import { applyDictationForceReset, dictationStateLabel } from "@/lib/dictationState";

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
