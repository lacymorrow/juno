import { cn } from "@/lib/utils";
import { X } from "lucide-react";
import type { AgentSessionInfo, AgentSessionStatus } from "@/hooks/useAgentSessions";

interface AgentSessionSwitcherProps {
	sessions: AgentSessionInfo[];
	onFocus: (sessionId: string) => void;
	onCancel: (sessionId: string) => void;
	className?: string;
}

const STATUS_LABELS: Record<AgentSessionStatus, string> = {
	starting: "Starting",
	running: "Working",
	needs_input: "Needs input",
	cancelling: "Stopping",
	finished: "Done",
	failed: "Failed",
};

const cancellableStatuses: AgentSessionStatus[] = ["starting", "running", "needs_input"];

/**
 * Horizontal strip of live agent sessions (LAC-1432 parallel agents).
 *
 * Each pill shows the session's cursor color, name, and current action.
 * Clicking a pill focuses that session (escape then cancels it, and its
 * cursor overlay highlights); the X cancels just that session while the
 * others keep running. Renders nothing when no sessions are live.
 */
export function AgentSessionSwitcher({
	sessions,
	onFocus,
	onCancel,
	className,
}: AgentSessionSwitcherProps) {
	if (sessions.length === 0) return null;

	return (
		<div
			className={cn("flex flex-wrap items-center gap-1.5 px-3 pb-1.5", className)}
			role="tablist"
			aria-label="Running agents"
		>
			{sessions.map((session) => {
				const detail =
					session.status === "running" && session.current_action
						? session.current_action
						: STATUS_LABELS[session.status];
				return (
					<div
						key={session.id}
						className={cn(
							"group flex items-center gap-1.5 rounded-full border px-2.5 py-1 text-xs transition-colors",
							session.focused
								? "border-foreground/30 bg-muted"
								: "border-border bg-background hover:bg-muted/50",
						)}
					>
						<button
							type="button"
							role="tab"
							aria-selected={session.focused}
							onClick={() => onFocus(session.id)}
							className="flex min-w-0 items-center gap-1.5"
							title={`Focus ${session.agent_name}`}
						>
							<span
								className="h-2 w-2 shrink-0 rounded-full"
								style={{ backgroundColor: session.display_color }}
								aria-hidden="true"
							/>
							<span className="font-medium">{session.agent_name}</span>
							<span className="max-w-48 truncate text-muted-foreground">{detail}</span>
						</button>
						{cancellableStatuses.includes(session.status) && (
							<button
								type="button"
								onClick={() => onCancel(session.id)}
								className="rounded-full p-0.5 text-muted-foreground opacity-60 transition-opacity hover:text-foreground group-hover:opacity-100"
								aria-label={`Stop ${session.agent_name}`}
							>
								<X className="h-3 w-3" />
							</button>
						)}
					</div>
				);
			})}
		</div>
	);
}
