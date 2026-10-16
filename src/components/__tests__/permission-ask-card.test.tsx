import { render, screen, fireEvent } from "@testing-library/react";
import { describe, it, expect, vi } from "vitest";
import { PermissionAskCard } from "@/components/ui/agent-cards";
import { availableComponents } from "@/components/ui/jsx-message-renderer";

describe("PermissionAskCard", () => {
  it("is on the renderer whitelist", () => {
    expect(availableComponents.PermissionAskCard).toBe(PermissionAskCard);
  });

  it("shows the backend's one line and one Continue button", () => {
    render(<PermissionAskCard line="I need Reminders for that." onContinue={() => {}} />);
    expect(screen.getByText("I need Reminders for that.")).toBeTruthy();
    const buttons = screen.getAllByRole("button");
    expect(buttons).toHaveLength(1);
    expect(buttons[0].textContent).toBe("Continue");
  });

  it("Continue fires once, then waits", () => {
    const onContinue = vi.fn();
    render(<PermissionAskCard line="I need Reminders for that." onContinue={onContinue} />);
    const button = screen.getByRole("button", { name: "Continue" });
    fireEvent.click(button);
    fireEvent.click(button);
    expect(onContinue).toHaveBeenCalledTimes(1);
    expect(screen.getByTestId("permission-ask-card").getAttribute("data-state")).toBe("continuing");
  });

  it("draws no button when there is nothing to call", () => {
    render(<PermissionAskCard line="I need Contacts for that." />);
    expect(screen.queryByRole("button")).toBeNull();
  });

  it("uses system blue and no gradient or glow", () => {
    render(<PermissionAskCard line="x" onContinue={() => {}} />);
    const cls = screen.getByRole("button").className;
    expect(cls).toContain("bg-primary");
    expect(cls).not.toMatch(/gradient|shadow|glow|purple/);
  });
});
