import { AlertCircle, Bell, Check, Loader2, X } from "lucide-react";
import { cn } from "@/lib/utils";
import type { AgentSessionInfo, AgentSessionStatus } from "@/hooks/useAgentSessions";

interface AgentSessionRowsProps {
	sessions: AgentSessionInfo[];
	onFocus: (sessionId: string) => void;
	className?: string;
}

const STATUS_LABELS: Record<AgentSessionStatus, string> = {
	starting: "Starting",
	running: "Working",
	needs_input: "Needs input",
	cancelling: "Stopping",
	cancelled: "Cancelled",
	finished: "Done",
	failed: "Failed",
};

function StatusIcon({ status }: { status: AgentSessionStatus }) {
	switch (status) {
		case "starting":
		case "running":
		case "cancelling":
			return <Loader2 size={14} className="animate-spin text-yellow-500" aria-hidden="true" />;
		case "finished":
			return <Check size={14} className="text-green-500" aria-hidden="true" />;
		case "failed":
			return <AlertCircle size={14} className="text-red-500" aria-hidden="true" />;
		case "needs_input":
			return <Bell size={14} className="animate-pulse text-amber-500" aria-hidden="true" />;
		case "cancelled":
			return <X size={14} className="text-gray-400" aria-hidden="true" />;
	}
}

/**
 * Named agent rows for the floating panel's expanded mode
 * (LAC-2830 spec section 4).
 *
 * Each row shows the agent's identity color, name, current action, and a
 * status icon. Clicking a row focuses that session — same as clicking its
 * roster dot — without pausing any background session. Renders nothing
 * when no sessions are live.
 */
export function AgentSessionRows({ sessions, onFocus, className }: AgentSessionRowsProps) {
	if (sessions.length === 0) return null;

	const active = sessions.filter(
		(session) => session.status === "starting" || session.status === "running",
	).length;
	const done = sessions.filter((session) => session.status === "finished").length;

	return (
		<div className={cn("space-y-1", className)}>
			<div className="text-xs font-medium text-gray-600">
				Running Agents ({sessions.length})
			</div>
			<div className="max-h-[400px] space-y-0.5 overflow-y-auto" role="list">
				{sessions.map((session) => (
					<button
						key={session.id}
						type="button"
						role="listitem"
						onClick={() => onFocus(session.id)}
						data-testid={`agent-row-${session.id}`}
						className={cn(
							"flex w-full items-center gap-2 rounded-md px-1.5 py-1 text-left transition-colors hover:bg-black/5",
							session.focused && "bg-black/10",
						)}
						title={`Switch to ${session.agent_name}`}
					>
						<span
							className="h-2 w-2 shrink-0 rounded-full"
							style={{ backgroundColor: session.display_color }}
							aria-hidden="true"
						/>
						<span className="min-w-0 flex-1 truncate text-sm font-medium text-gray-800">
							{session.agent_name}
						</span>
						<span className="max-w-[140px] truncate text-xs text-gray-500">
							{session.status === "running" && session.current_action
								? session.current_action
								: STATUS_LABELS[session.status]}
						</span>
						<StatusIcon status={session.status} />
					</button>
				))}
			</div>
			<div className="border-t border-black/10 pt-1.5 text-[11px] text-gray-500">
				Total: {sessions.length} {sessions.length === 1 ? "agent" : "agents"} · {active} active ·{" "}
				{done} done
			</div>
		</div>
	);
}
