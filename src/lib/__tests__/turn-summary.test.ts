import { describe, expect, it } from "vitest";
import { formatTurnSummary, summarizeTurn } from "@/lib/turn-summary";
import type { ChatMessage } from "@/types/chat";

const user = (t: number): ChatMessage => ({ role: "user", content: "go", timestamp: t });
const reply = (t: number): ChatMessage => ({ role: "assistant", content: "done", timestamp: t });
const tool = (name: string, extra: Partial<ChatMessage> = {}): ChatMessage => ({
	role: "tool_call_request",
	content: name,
	tool_name: name,
	...extra,
});

describe("summarizeTurn", () => {
	it("returns null for a reply that used no tools", () => {
		// A plain chat answer renders nothing rather than "0 actions".
		expect(summarizeTurn([user(0), reply(1000)], 1)).toBeNull();
	});

	it("returns null while the reply is still streaming", () => {
		const open: ChatMessage = { ...reply(1000), isStreaming: true };
		expect(summarizeTurn([user(0), tool("computer"), open], 2)).toBeNull();
	});

	it("counts only the tool calls inside this turn", () => {
		// The earlier turn's calls must not leak into the later count.
		const convo = [
			user(0), tool("computer"), tool("computer"), reply(100),
			user(200), tool("computer"), reply(300),
		];
		expect(summarizeTurn(convo, 6)?.actions).toBe(1);
		expect(summarizeTurn(convo, 3)?.actions).toBe(2);
	});

	it("counts a screenshot by its image, its name, or its action argument", () => {
		// Three shapes reach the same conclusion: a captured image, a tool named
		// for it, and the `computer` tool carrying the action in its arguments.
		const convo = [
			user(0),
			tool("computer", { screenshot_base64: "iVBOR" }),
			tool("capture_screenshot"),
			tool("computer", { tool_args: { action: "screenshot" } }),
			tool("computer", { tool_args: { action: "left_click" } }),
			reply(500),
		];
		const s = summarizeTurn(convo, 5);
		expect(s?.actions).toBe(4);
		expect(s?.screenshots).toBe(3);
	});

	it("does not double-count a row that matches more than one screenshot signal", () => {
		const convo = [
			user(0),
			tool("capture_screenshot", { screenshot_base64: "iVBOR", tool_args: { action: "screenshot" } }),
			reply(100),
		];
		expect(summarizeTurn(convo, 2)?.screenshots).toBe(1);
	});

	it("measures the turn from the question to the answer", () => {
		expect(summarizeTurn([user(1000), tool("computer"), reply(4500)], 2)?.durationMs).toBe(3500);
	});

	it("omits duration when either timestamp is missing", () => {
		const noStamp: ChatMessage = { role: "assistant", content: "done" };
		expect(summarizeTurn([user(0), tool("computer"), noStamp], 2)?.durationMs).toBeUndefined();
	});

	it("survives a turn with no preceding user message", () => {
		// The very first messages can arrive without a question in front of them.
		expect(summarizeTurn([tool("computer"), reply(100)], 1)?.actions).toBe(1);
	});
});

describe("formatTurnSummary", () => {
	it("reads as a sentence fragment a person can scan", () => {
		expect(formatTurnSummary({ actions: 12, screenshots: 4, durationMs: 38_000 }))
			.toBe("12 actions · 4 screenshots · 38.0s");
	});

	it("drops the screenshot clause when there were none", () => {
		expect(formatTurnSummary({ actions: 3, screenshots: 0, durationMs: 1200 }))
			.toBe("3 actions · 1.2s");
	});

	it("singularises", () => {
		expect(formatTurnSummary({ actions: 1, screenshots: 1, durationMs: 900 }))
			.toBe("1 action · 1 screenshot · 0.9s");
	});

	it("switches to minutes past a minute", () => {
		expect(formatTurnSummary({ actions: 40, screenshots: 9, durationMs: 185_000 }))
			.toBe("40 actions · 9 screenshots · 3m");
	});
});
