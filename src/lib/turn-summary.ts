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

/** One capture the agent made during a turn. */
export type TurnScreenshot = {
	/**
	 * The PNG as Rust captured it, base64. Absent when the call was a screenshot
	 * by name but no image came back: a denied screen-recording permission, a
	 * failed capture, or an image pruned from visual context. The disclosure says
	 * so rather than drawing a broken image.
	 */
	base64?: string;
	/** Which tool took it. Alt text only. */
	toolName?: string;
};

/**
 * Where this turn begins: the nearest user message before the reply.
 *
 * Tool rows are inserted ahead of the open assistant bubble, so everything the
 * turn did sits in `(start, index)`. Returns -1 for the opening messages of a
 * conversation, which can arrive with no question in front of them.
 */
function turnStart(conversation: ChatMessage[], index: number): number {
	let start = index - 1;
	while (start >= 0 && conversation[start].role !== "user") start--;
	return start;
}

/** A row that stands for one tool call, whichever half of it the UI holds. */
function isToolRow(msg: ChatMessage): boolean {
	return msg.role === "tool_call_request" || msg.role === "tool_call_result";
}

/**
 * Every capture the assistant message at `index` made getting to its answer.
 *
 * This is the list the chat shows and the number the turn summary counts, so
 * the two can never disagree. It deliberately does not wait for the stream to
 * close: a computer-use turn holds the bubble open for a minute, and a person
 * watching it work wants the captures as they land, the same reason the spoken
 * panel stopped gating on `isStreaming`.
 *
 * Both tool roles count. A result normally folds into the request that produced
 * it, but a result whose request was lost stands alone, and its image is still
 * something the agent saw.
 */
export function turnScreenshots(
	conversation: ChatMessage[],
	index: number
): TurnScreenshot[] {
	const reply = conversation[index];
	if (!reply || reply.role !== "assistant") return [];

	const start = turnStart(conversation, index);
	const shots: TurnScreenshot[] = [];
	for (let i = start + 1; i < index; i++) {
		const msg = conversation[i];
		if (!isToolRow(msg) || !isScreenshot(msg)) continue;
		shots.push({ base64: msg.screenshot_base64, toolName: msg.tool_name });
	}
	return shots;
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

	const start = turnStart(conversation, index);

	let actions = 0;
	for (let i = start + 1; i < index; i++) {
		if (isToolRow(conversation[i])) actions++;
	}
	if (actions === 0) return null;

	// Derived, not recounted: the summary's number is the length of the list the
	// disclosure draws, so "4 screenshots" and four images are the same fact.
	const screenshots = turnScreenshots(conversation, index).length;

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
