import { render, screen } from "@testing-library/react";
import { describe, it, expect, vi } from "vitest";
import { ModalSystem, type FeedbackData } from "@/components/ModalSystem";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({
	listen: vi.fn(() => Promise.resolve(() => {})),
	emit: vi.fn(),
}));

function renderFeedbackModal(type: FeedbackData["type"]) {
	render(
		<ModalSystem
			activeModal="feedback"
			onClose={vi.fn()}
			feedbackData={{ type, title: "", description: "", priority: "medium" }}
			onFeedbackDataChange={vi.fn()}
			conversation={[]}
			isExporting={false}
			isImporting={false}
			keyboardShortcuts={null}
			onUpdateConversation={vi.fn()}
			onAddSystemMessage={vi.fn()}
		/>,
	);
}

// The regression: the feedback modal used to write Priority and Contact into
// the GitHub issue body. Once a bug or a feature goes to the issue form, the
// form has no field for either, so both controls would keep collecting a value
// that nothing reads. A control whose value is discarded is worse than no
// control (LAC-5069).
describe("the feedback modal asks only for what it sends", () => {
	it("does not ask a bug report for a priority", () => {
		renderFeedbackModal("issue");
		expect(screen.queryByLabelText("Priority")).not.toBeInTheDocument();
		expect(screen.queryByText("Priority")).not.toBeInTheDocument();
	});

	it("does not ask a feature request for a priority", () => {
		renderFeedbackModal("feature");
		expect(screen.queryByText("Priority")).not.toBeInTheDocument();
	});

	it("still asks general feedback for a priority, which the email carries", () => {
		renderFeedbackModal("general");
		expect(screen.getByText("Priority")).toBeInTheDocument();
	});

	it("never asks for an email address, because nothing sends one", () => {
		for (const type of ["issue", "feature", "general"] as const) {
			const { unmount } = render(
				<ModalSystem
					activeModal="feedback"
					onClose={vi.fn()}
					feedbackData={{ type, title: "", description: "", priority: "medium" }}
					onFeedbackDataChange={vi.fn()}
					conversation={[]}
					isExporting={false}
					isImporting={false}
					keyboardShortcuts={null}
					onUpdateConversation={vi.fn()}
					onAddSystemMessage={vi.fn()}
				/>,
			);
			expect(document.querySelector('input[type="email"]')).toBeNull();
			unmount();
		}
	});
});
