import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useEventListener } from "@/hooks/useEventListener";
import { EVENTS } from "@/lib/constants.generated";

/** Mirrors `AgentSessionStatus` in src-tauri/src/agents/session.rs (snake_case serde). */
export type AgentSessionStatus =
	| "starting"
	| "running"
	| "needs_input"
	| "cancelling"
	| "cancelled"
	| "finished"
	| "failed";

/** Mirrors `AgentSessionInfo` in src-tauri/src/agents/session.rs. */
export interface AgentSessionInfo {
	id: string;
	agent_name: string;
	/** 0-7 index into the fixed identity palette (LAC-2830 spec section 2). */
	color_slot: number;
	display_color: string;
	status: AgentSessionStatus;
	current_action: string | null;
	started_at_ms: number;
	last_activity_ms: number;
	focused: boolean;
}

/**
 * Live view of the backend's parallel-agent session registry (LAC-1432).
 *
 * Loads the initial snapshot via `list_agent_sessions`, then stays in sync
 * through `agent-sessions-updated` events — the backend broadcasts the full
 * session list on every registry mutation, so no polling or diffing is needed.
 */
export function useAgentSessions() {
	const [sessions, setSessions] = useState<AgentSessionInfo[]>([]);

	useEffect(() => {
		let mounted = true;
		invoke<AgentSessionInfo[]>("list_agent_sessions")
			.then((snapshot) => {
				if (mounted) setSessions(snapshot);
			})
			.catch((error) => {
				console.error("Failed to load agent sessions:", error);
			});
		return () => {
			mounted = false;
		};
	}, []);

	useEventListener<AgentSessionInfo[]>(EVENTS.AGENT_SESSIONS_UPDATED, setSessions);

	const focusSession = useCallback(async (sessionId: string | null) => {
		try {
			await invoke("focus_agent_session", { sessionId });
		} catch (error) {
			console.error("Failed to focus agent session:", error);
		}
	}, []);

	const cancelSession = useCallback(async (sessionId: string) => {
		try {
			await invoke("cancel_agent_session", { sessionId });
		} catch (error) {
			console.error("Failed to cancel agent session:", error);
		}
	}, []);

	const focusedSession = sessions.find((session) => session.focused) ?? null;

	return { sessions, focusedSession, focusSession, cancelSession };
}
