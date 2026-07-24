import { useState } from "react";
import { cn } from "@/lib/utils";
import type { AgentSessionInfo, AgentSessionStatus } from "@/hooks/useAgentSessions";

interface AgentRosterStripProps {
	sessions: AgentSessionInfo[];
	onFocus: (sessionId: string) => void;
	className?: string;
}

/** Max dots shown before collapsing into a "+N" overflow label (palette size). */
const MAX_VISIBLE_DOTS = 8;

const STATUS_LABELS: Record<AgentSessionStatus, string> = {
	starting: "Starting",
	running: "Working",
	needs_input: "Needs input",
	cancelling: "Stopping",
	cancelled: "Cancelled",
	finished: "Done",
	failed: "Failed",
};

/** Mini status overlay color for the bottom-right badge on each dot. */
const STATUS_OVERLAY_CLASS: Record<AgentSessionStatus, string> = {
	starting: "bg-gray-400",
	running: "bg-yellow-400 animate-pulse",
	needs_input: "bg-white",
	cancelling: "bg-gray-400",
	cancelled: "bg-gray-400",
	finished: "bg-green-400",
	failed: "bg-red-400",
};

/** Ring/animation treatment for notification states (LAC-2830 spec section 6). */
function dotStateClass(session: AgentSessionInfo): string {
	switch (session.status) {
		case "finished":
			return "ring-2 ring-green-400 animate-agent-completion-pulse";
		case "failed":
			return "ring-2 ring-red-400 animate-agent-error-shake";
		case "needs_input":
			return "animate-agent-needs-input-blink";
		default:
			return "";
	}
}

/**
 * Compact roster of parallel agent sessions (LAC-2830 spec section 3).
 *
 * One colored dot per live session, shown beneath the floating bar whenever
 * two or more agents run. The dot color is the agent's identity color; the
 * small overlay badge communicates status. Clicking a dot focuses that
 * session — background sessions keep running. Renders nothing with fewer
 * than two sessions so the single-agent experience is unchanged.
 */
export function AgentRosterStrip({ sessions, onFocus, className }: AgentRosterStripProps) {
	const [hoveredId, setHoveredId] = useState<string | null>(null);

	if (sessions.length < 2) return null;

	const visible = sessions.slice(0, MAX_VISIBLE_DOTS);
	const overflow = sessions.length - visible.length;

	return (
		<div
			className={cn(
				"flex items-center gap-2 rounded-full border border-white/10 bg-black/80 px-2 py-1 backdrop-blur-md",
				className,
			)}
			role="tablist"
			aria-label="Running agents"
		>
			{visible.map((session) => {
				const label = `Switch to ${session.agent_name} — ${STATUS_LABELS[session.status]}`;
				const tooltip =
					session.status === "running" && session.current_action
						? `${session.agent_name} — ${session.current_action}`
						: `${session.agent_name} — ${STATUS_LABELS[session.status]}`;
				return (
					<div key={session.id} className="relative">
						{hoveredId === session.id && (
							<div
								className="pointer-events-none absolute bottom-full left-1/2 z-10 mb-1 -translate-x-1/2 whitespace-nowrap rounded-md bg-black/90 px-2 py-1 text-[11px] text-white"
								role="tooltip"
							>
								{tooltip}
							</div>
						)}
						<button
							type="button"
							role="tab"
							aria-selected={session.focused}
							aria-label={label}
							tabIndex={0}
							onClick={() => onFocus(session.id)}
							onMouseEnter={() => setHoveredId(session.id)}
							onMouseLeave={() => setHoveredId(null)}
							data-testid={`roster-dot-${session.id}`}
							className={cn(
								"relative block rounded-full transition-transform animate-agent-dot-appear",
								session.focused
									? "h-3.5 w-3.5 border-2 border-white/80"
									: "h-3 w-3 border border-transparent hover:scale-110",
								dotStateClass(session),
							)}
							style={
								{
									backgroundColor: session.display_color,
									"--agent-color": session.display_color,
								} as React.CSSProperties
							}
						>
							<span
								className={cn(
									"absolute -bottom-0.5 -right-0.5 h-1.5 w-1.5 rounded-full",
									STATUS_OVERLAY_CLASS[session.status],
								)}
								aria-hidden="true"
							/>
							{session.status === "needs_input" && (
								<span
									className="absolute -right-1 -top-1 flex h-2 w-2 items-center justify-center rounded-full bg-white text-[8px] font-bold leading-none text-black"
									aria-hidden="true"
								>
									!
								</span>
							)}
						</button>
					</div>
				);
			})}
			{overflow > 0 && (
				<span className="text-[10px] text-white/60" aria-label={`${overflow} more agents`}>
					+{overflow}
				</span>
			)}
		</div>
	);
}
