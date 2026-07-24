import { render, screen, fireEvent } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { AgentRosterStrip } from "../AgentRosterStrip";
import type { AgentSessionInfo } from "@/hooks/useAgentSessions";

let nextId = 0;
const makeSession = (overrides: Partial<AgentSessionInfo> = {}): AgentSessionInfo => ({
	id: `session-${nextId++}`,
	agent_name: "orchestrator",
	color_slot: 0,
	display_color: "#3B82F6",
	status: "running",
	current_action: "Clicking Submit button",
	started_at_ms: 1000,
	last_activity_ms: 2000,
	focused: false,
	...overrides,
});

const makeSessions = (count: number) =>
	Array.from({ length: count }, (_, index) =>
		makeSession({ id: `session-${index}`, color_slot: index % 8 }),
	);

describe("AgentRosterStrip", () => {
	it("renders nothing with fewer than two sessions", () => {
		const { container } = render(
			<AgentRosterStrip sessions={[makeSession()]} onFocus={vi.fn()} />,
		);
		expect(container).toBeEmptyDOMElement();
	});

	it("renders one dot per session when two or more run", () => {
		render(<AgentRosterStrip sessions={makeSessions(3)} onFocus={vi.fn()} />);
		expect(screen.getAllByRole("tab")).toHaveLength(3);
	});

	it("focuses a session when its dot is clicked", () => {
		const onFocus = vi.fn();
		const sessions = makeSessions(2);
		render(<AgentRosterStrip sessions={sessions} onFocus={onFocus} />);
		fireEvent.click(screen.getByTestId(`roster-dot-${sessions[1].id}`));
		expect(onFocus).toHaveBeenCalledWith(sessions[1].id);
	});

	it("marks the focused session's dot as selected", () => {
		const sessions = [makeSession({ id: "a", focused: true }), makeSession({ id: "b" })];
		render(<AgentRosterStrip sessions={sessions} onFocus={vi.fn()} />);
		expect(screen.getByTestId("roster-dot-a")).toHaveAttribute("aria-selected", "true");
		expect(screen.getByTestId("roster-dot-b")).toHaveAttribute("aria-selected", "false");
	});

	it("collapses overflow beyond eight dots into a +N label", () => {
		render(<AgentRosterStrip sessions={makeSessions(10)} onFocus={vi.fn()} />);
		expect(screen.getAllByRole("tab")).toHaveLength(8);
		expect(screen.getByText("+2")).toBeInTheDocument();
	});

	it("applies notification animations for terminal and needs-input states", () => {
		const sessions = [
			makeSession({ id: "done", status: "finished" }),
			makeSession({ id: "err", status: "failed" }),
			makeSession({ id: "ask", status: "needs_input" }),
		];
		render(<AgentRosterStrip sessions={sessions} onFocus={vi.fn()} />);
		expect(screen.getByTestId("roster-dot-done").className).toContain(
			"animate-agent-completion-pulse",
		);
		expect(screen.getByTestId("roster-dot-err").className).toContain("animate-agent-error-shake");
		expect(screen.getByTestId("roster-dot-ask").className).toContain(
			"animate-agent-needs-input-blink",
		);
	});
});
