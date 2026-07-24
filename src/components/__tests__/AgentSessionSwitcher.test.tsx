import { render, screen, fireEvent } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { AgentSessionSwitcher } from "../AgentSessionSwitcher";
import type { AgentSessionInfo } from "@/hooks/useAgentSessions";

const makeSession = (overrides: Partial<AgentSessionInfo> = {}): AgentSessionInfo => ({
	id: "session-1",
	agent_name: "orchestrator",
	display_color: "#22c55e",
	status: "running",
	current_action: "Clicking Submit button",
	started_at_ms: 1000,
	last_activity_ms: 2000,
	focused: false,
	...overrides,
});

describe("AgentSessionSwitcher", () => {
	it("renders nothing when no sessions are live", () => {
		const { container } = render(
			<AgentSessionSwitcher sessions={[]} onFocus={vi.fn()} onCancel={vi.fn()} />,
		);
		expect(container).toBeEmptyDOMElement();
	});

	it("shows each session with its name and current action", () => {
		const sessions = [
			makeSession(),
			makeSession({
				id: "session-2",
				agent_name: "browser",
				status: "needs_input",
				current_action: null,
			}),
		];
		render(<AgentSessionSwitcher sessions={sessions} onFocus={vi.fn()} onCancel={vi.fn()} />);

		expect(screen.getByText("orchestrator")).toBeInTheDocument();
		expect(screen.getByText("Clicking Submit button")).toBeInTheDocument();
		expect(screen.getByText("browser")).toBeInTheDocument();
		expect(screen.getByText("Needs input")).toBeInTheDocument();
	});

	it("focuses a session when its pill is clicked", () => {
		const onFocus = vi.fn();
		render(
			<AgentSessionSwitcher sessions={[makeSession()]} onFocus={onFocus} onCancel={vi.fn()} />,
		);
		fireEvent.click(screen.getByRole("tab"));
		expect(onFocus).toHaveBeenCalledWith("session-1");
	});

	it("cancels a session via its stop button", () => {
		const onCancel = vi.fn();
		render(
			<AgentSessionSwitcher sessions={[makeSession()]} onFocus={vi.fn()} onCancel={onCancel} />,
		);
		fireEvent.click(screen.getByRole("button", { name: "Stop orchestrator" }));
		expect(onCancel).toHaveBeenCalledWith("session-1");
	});

	it("hides the stop button for sessions already terminal or cancelling", () => {
		render(
			<AgentSessionSwitcher
				sessions={[makeSession({ status: "cancelling", current_action: null })]}
				onFocus={vi.fn()}
				onCancel={vi.fn()}
			/>,
		);
		expect(screen.queryByRole("button", { name: "Stop orchestrator" })).not.toBeInTheDocument();
		expect(screen.getByText("Stopping")).toBeInTheDocument();
	});
});
