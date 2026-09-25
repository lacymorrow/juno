import type { ChatMessage } from "@/types/chat";

/** What one assistant turn cost, in the units a person actually asks about. */
export type TurnSummary = {
	/** Tool calls the model made in this turn. */
	actions: number;
	/** How many of those were screenshots. */
	screenshots: number;
	/** Wall time from the question to the finished answer, in ms. */
	durationMs?: number;
};

/**
 * A screenshot is any call that came back with an image, plus the calls whose
 * name says so even when the image was dropped.
 *
 * Both halves are load-bearing. `screenshot_base64` alone misses a capture that
 * failed or was pruned from visual context, and the name alone misses the
 * `computer` tool, where the action lives in the arguments rather than the tool
 * name. Counting either way over-counts nothing: a row is one call.
 */
function isScreenshot(msg: ChatMessage): boolean {
	if (msg.screenshot_base64) return true;
	const name = msg.tool_name?.toLowerCase() ?? "";
	if (name.includes("screenshot")) return true;
	const action = msg.tool_args?.action;
	return typeof action === "string" && action.toLowerCase().includes("screenshot");
}

/**
 * Count what the assistant message at `index` spent getting to its answer.
 *
 * Returns `null` when the turn used no tools, which is how a plain chat reply
 * renders nothing instead of "0 actions". The window is the run of messages
 * between the previous user message and this reply — tool calls are inserted
 * ahead of the open assistant bubble, so they sit in exactly that span.
 */
export function summarizeTurn(
	conversation: ChatMessage[],
	index: number
): TurnSummary | null {
	const reply = conversation[index];
	if (!reply || reply.role !== "assistant" || reply.isStreaming) return null;

	let start = index - 1;
	while (start >= 0 && conversation[start].role !== "user") start--;

	let actions = 0;
	let screenshots = 0;
	for (let i = start + 1; i < index; i++) {
		const msg = conversation[i];
		if (msg.role !== "tool_call_request") continue;
		actions++;
		if (isScreenshot(msg)) screenshots++;
	}
	if (actions === 0) return null;

	const question = start >= 0 ? conversation[start] : undefined;
	const durationMs =
		question?.timestamp && reply.timestamp
			? Math.max(0, reply.timestamp - question.timestamp)
			: undefined;

	return { actions, screenshots, durationMs };
}

/** `12 actions · 4 screenshots · 38s` — the one line a person reads. */
export function formatTurnSummary(summary: TurnSummary): string {
	const parts = [
		`${summary.actions} ${summary.actions === 1 ? "action" : "actions"}`,
	];
	if (summary.screenshots > 0) {
		parts.push(
			`${summary.screenshots} ${summary.screenshots === 1 ? "screenshot" : "screenshots"}`
		);
	}
	if (summary.durationMs !== undefined) {
		const seconds = summary.durationMs / 1000;
		parts.push(seconds < 60 ? `${seconds.toFixed(1)}s` : `${Math.round(seconds / 60)}m`);
	}
	return parts.join(" · ");
}
