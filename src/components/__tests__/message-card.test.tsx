import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { MessageCard } from "@/components/ui/agent-cards";
import { availableComponents } from "@/components/ui/jsx-message-renderer";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

const mockedInvoke = vi.mocked(invoke);

const FULL_DISK_URL =
  "x-apple.systempreferences:com.apple.preference.security?Privacy_AllFiles";

describe("MessageCard", () => {
  beforeEach(() => {
    mockedInvoke.mockReset();
    mockedInvoke.mockResolvedValue(undefined);
  });

  it("is on the renderer whitelist", () => {
    expect(availableComponents.MessageCard).toBe(MessageCard);
  });

  it("draft: shows recipient, body, the phrase, and one Send button", () => {
    const onSend = vi.fn();
    render(
      <MessageCard
        state="draft"
        kind="text"
        to="Doug Keesler"
        address="+17045550100"
        body="I'm running ten minutes late"
        onSend={onSend}
      />,
    );
    expect(screen.getByText("Doug Keesler")).toBeTruthy();
    expect(screen.getByText("+17045550100")).toBeTruthy();
    expect(screen.getByText("I'm running ten minutes late")).toBeTruthy();
    expect(screen.getByText(/send it/)).toBeTruthy();
    expect(screen.getByTestId("message-card").getAttribute("data-state")).toBe("draft");

    const buttons = screen.getAllByRole("button");
    expect(buttons).toHaveLength(1);
    expect(buttons[0].textContent).toBe("Send");
    fireEvent.click(buttons[0]);
    fireEvent.click(buttons[0]);
    expect(onSend).toHaveBeenCalledTimes(1);
  });

  it("an email shows its subject", () => {
    render(
      <MessageCard state="draft" kind="email" to="Katie" address="k@example.com" subject="Thursday" body="See you at nine." onSend={() => {}} />,
    );
    expect(screen.getByText("Thursday")).toBeTruthy();
    expect(screen.getByText("See you at nine.")).toBeTruthy();
  });

  it("sending: no button, says it is going", () => {
    render(<MessageCard state="sending" to="Doug" body="On my way" onSend={() => {}} />);
    expect(screen.queryAllByRole("button")).toHaveLength(0);
    expect(screen.getByText("Sending")).toBeTruthy();
    expect(screen.getByTestId("message-card").getAttribute("data-state")).toBe("sending");
  });

  it("sent: no button, says Sent", () => {
    render(<MessageCard state="sent" to="Doug" body="On my way" />);
    expect(screen.queryAllByRole("button")).toHaveLength(0);
    expect(screen.getByText("Sent")).toBeTruthy();
    expect(screen.getByTestId("message-card").getAttribute("data-state")).toBe("sent");
  });

  it("a tag from a tool with no handler draws no Send button", () => {
    render(<MessageCard state="draft" to="Doug" body="hi" />);
    expect(screen.queryAllByRole("button")).toHaveLength(0);
  });

  it("an unknown state reads as a draft", () => {
    render(<MessageCard state="weird" to="Doug" body="hi" />);
    expect(screen.getByTestId("message-card").getAttribute("data-state")).toBe("draft");
  });

  it("when access is off, one sentence and one button to the right settings row", async () => {
    render(
      <MessageCard
        needsAccess="Full Disk Access"
        reason="I need Full Disk Access to read your messages."
        settingsUrl={FULL_DISK_URL}
      />,
    );
    expect(screen.getByText("I need Full Disk Access to read your messages.")).toBeTruthy();
    const buttons = screen.getAllByRole("button");
    expect(buttons).toHaveLength(1);
    fireEvent.click(buttons[0]);
    await waitFor(() =>
      expect(mockedInvoke).toHaveBeenCalledWith("open_url", { url: FULL_DISK_URL }),
    );
  });

  it("never follows a link that is not System Settings", () => {
    render(<MessageCard needsAccess="Messages" settingsUrl="https://example.com" />);
    expect(screen.queryAllByRole("button")).toHaveLength(0);
  });

  it("names no framework or internal term", () => {
    const { container } = render(
      <MessageCard needsAccess="Full Disk Access" reason="I need Full Disk Access to read your messages." settingsUrl={FULL_DISK_URL} />,
    );
    for (const banned of ["TCC", "plist", "chat.db", "AppleScript", "framework"]) {
      expect(container.textContent).not.toContain(banned);
    }
  });
});
