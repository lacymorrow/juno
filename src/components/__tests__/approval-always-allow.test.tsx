import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(async () => true) }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
  emit: vi.fn(),
}));

import { invoke } from "@tauri-apps/api/core";

import { ChatMessageComponent } from "../ChatMessageV2";
import { COMMANDS } from "@/lib/constants.generated";
import type { ChatMessage } from "@/types/chat";

const invokeMock = vi.mocked(invoke);

/**
 * The third answer on the approval prompt.
 *
 * Lacy: "there's no 'allow all' or 'dont ask again'". It lives in #644's
 * top-level `ApprovalPrompt`, phrased as an answer rather than a setting: it
 * begins with the same word as Allow, states its own scope, and sits quietly
 * under the two primary answers so Allow stays the one primary action.
 *
 * Scope is one tool, one conversation, memory only. Rust decides whether it is
 * offered at all by sending `always_allow_label` or leaving it out. These tests
 * pin both halves: it appears with the label and grants, and it is absent for
 * an action no grant can cover.
 */
const STANDING_YES = /Allow, and stop asking about terminal commands/;

function approvalRow(overrides: Partial<ChatMessage> = {}): ChatMessage {
  return {
    role: "tool_call_request",
    content: "Run this in the terminal: npm install",
    tool_name: "bash",
    tool_id: "batch-1",
    approval_state: "pending",
    always_allow_label: "terminal commands",
    ...overrides,
  } as ChatMessage;
}

function row(msg: ChatMessage, onApprovalUpdate: () => void) {
  return (
    <ChatMessageComponent
      msg={msg}
      index={0}
      copiedMessageId={null}
      onCopyResponse={vi.fn()}
      onShareResponse={vi.fn()}
      onApprovalUpdate={onApprovalUpdate}
    />
  );
}

function renderRow(msg: ChatMessage, onApprovalUpdate = vi.fn()) {
  const { rerender } = render(row(msg, onApprovalUpdate));
  return {
    onApprovalUpdate,
    /** Re-render the same instance, the way a conversation update does. */
    settle: (next: ChatMessage) => rerender(row(next, onApprovalUpdate)),
  };
}

describe("approval prompt: the standing yes", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    invokeMock.mockResolvedValue(true as never);
  });

  it("sits with the other two answers, with nothing opened first", () => {
    renderRow(approvalRow());

    // Production draws the question at the top level, so all three answers are
    // reachable without opening a disclosure.
    expect(screen.getByText("Juno needs your OK")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Allow" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Don't allow" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: STANDING_YES })).toBeInTheDocument();
  });

  it("reads as an answer that names what it covers, not as a setting", () => {
    renderRow(approvalRow());

    const standing = screen.getByRole("button", { name: STANDING_YES });
    // It says what it covers and for how long, so agreeing to it is informed.
    expect(standing).toHaveTextContent("terminal commands");
    expect(standing).toHaveTextContent("for this conversation");
    // The raw tool name is an implementation detail and was the whole "not
    // very friendly" complaint.
    expect(standing).not.toHaveTextContent("bash");
    // It is not a checkbox or a toggle. A question has answers.
    expect(screen.queryByRole("checkbox")).not.toBeInTheDocument();
    expect(screen.queryByRole("switch")).not.toBeInTheDocument();
  });

  it("grants the tool for this conversation and settles the row", async () => {
    const { onApprovalUpdate } = renderRow(approvalRow());

    fireEvent.click(screen.getByRole("button", { name: STANDING_YES }));

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith(
        COMMANDS.TOOLS_ALLOW_TOOL_FOR_CONVERSATION,
        { toolId: "batch-1" },
      ),
    );
    expect(onApprovalUpdate).toHaveBeenCalledWith("batch-1", "approved");
  });

  it("says what the standing yes covers once it is given", async () => {
    // A grant a person cannot see they gave is the thing this was careful not
    // to build, so the settled line names what it covers and for how long.
    const { settle } = renderRow(approvalRow());

    fireEvent.click(screen.getByRole("button", { name: STANDING_YES }));
    await waitFor(() => expect(invokeMock).toHaveBeenCalled());
    settle(approvalRow({ approval_state: "approved" }));

    expect(
      screen.getByText(
        "Allowed. Juno will not ask about terminal commands again in this conversation.",
      ),
    ).toBeInTheDocument();
  });

  it("does not claim a standing yes when the person only pressed Allow", async () => {
    const { settle } = renderRow(approvalRow());

    fireEvent.click(screen.getByRole("button", { name: "Allow" }));
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith(
        COMMANDS.TOOLS_APPROVE_TOOL_EXECUTION,
        { toolId: "batch-1" },
      ),
    );
    settle(approvalRow({ approval_state: "approved" }));

    expect(
      screen.getByText("Allowed: Run this in the terminal: npm install"),
    ).toBeInTheDocument();
    expect(screen.queryByText(/will not ask about/)).not.toBeInTheDocument();
  });

  it("is not offered when Rust sends no label, which is how the floor is held", () => {
    // Rust omits `always_allow_label` for anything irreversible. No mode and no
    // standing grant waives that, so an answer promising otherwise would be the
    // next control that does not do what it says.
    renderRow(approvalRow({ always_allow_label: null }));

    expect(screen.queryByRole("button", { name: STANDING_YES })).not.toBeInTheDocument();
    // The ordinary answers are still there.
    expect(screen.getByRole("button", { name: "Allow" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Don't allow" })).toBeInTheDocument();
  });

  it("offers nothing to press once the question is settled", () => {
    renderRow(approvalRow({ approval_state: "denied" }));

    expect(screen.queryByRole("button", { name: STANDING_YES })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Allow" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Don't allow" })).not.toBeInTheDocument();
  });

  it("stays out of the development tool card, which #644 left alone", () => {
    render(
      <ChatMessageComponent
        msg={approvalRow()}
        index={0}
        copiedMessageId={null}
        onCopyResponse={vi.fn()}
        onShareResponse={vi.fn()}
        onApprovalUpdate={vi.fn()}
        showToolDetails
      />,
    );

    // The dev card is a disclosure, so its buttons live one click in.
    fireEvent.click(screen.getByRole("button", { name: /Awaiting Approval/ }));
    expect(screen.getByRole("button", { name: "Approve" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Deny" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: STANDING_YES })).not.toBeInTheDocument();
  });
});
