import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { AgendaCard } from "@/components/ui/agent-cards";
import { availableComponents } from "@/components/ui/jsx-message-renderer";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

const mockedInvoke = vi.mocked(invoke);

const REMINDERS_URL =
  "x-apple.systempreferences:com.apple.preference.security?Privacy_Reminders";

describe("AgendaCard", () => {
  beforeEach(() => {
    mockedInvoke.mockReset();
    mockedInvoke.mockResolvedValue(undefined);
  });

  it("is on the renderer whitelist", () => {
    expect(availableComponents.AgendaCard).toBe(AgendaCard);
  });

  it("draws events and reminders with the same shape", () => {
    render(
      <AgendaCard
        items={[
          { title: "Dentist", when: "Tomorrow 3:00 PM", where: "Main St", kind: "event" },
          { title: "Call Katie", when: "Tomorrow 9:00 AM", kind: "reminder" },
          { title: "Pay rent", when: "Yesterday", kind: "reminder", done: true },
        ]}
      />,
    );

    expect(screen.getByText("Dentist")).toBeTruthy();
    expect(screen.getByText("Tomorrow 3:00 PM")).toBeTruthy();
    expect(screen.getByText("Main St")).toBeTruthy();
    expect(screen.getByText("Call Katie")).toBeTruthy();
    expect(screen.getByText("Tomorrow 9:00 AM")).toBeTruthy();

    // Done reads as done: struck through and no longer accented.
    const done = screen.getByText("Pay rent");
    expect(done.className).toContain("line-through");
    expect(screen.getByText("Yesterday").className).not.toContain("0a84ff");
    expect(screen.getByTestId("agenda-card").getAttribute("data-state")).toBe("items");
  });

  it("says so in one line when there is nothing", () => {
    render(<AgendaCard items={[]} empty="Nothing on your calendar for that time." />);
    expect(screen.getByText("Nothing on your calendar for that time.")).toBeTruthy();
    expect(screen.getByTestId("agenda-card").getAttribute("data-state")).toBe("empty");
  });

  it("survives items that never arrived", () => {
    render(<AgendaCard />);
    expect(screen.getByText("Nothing to show.")).toBeTruthy();
  });

  it("when access is off, offers one button that opens the right settings row", async () => {
    render(<AgendaCard needsAccess="Reminders" settingsUrl={REMINDERS_URL} />);

    expect(screen.getByText("Juno needs your OK to use Reminders.")).toBeTruthy();
    const buttons = screen.getAllByRole("button");
    expect(buttons).toHaveLength(1);

    fireEvent.click(buttons[0]);
    await waitFor(() =>
      expect(mockedInvoke).toHaveBeenCalledWith("open_url", { url: REMINDERS_URL }),
    );
  });

  it("never follows a link that is not System Settings", () => {
    render(<AgendaCard needsAccess="Reminders" settingsUrl="https://example.com" />);
    expect(screen.queryAllByRole("button")).toHaveLength(0);
    expect(mockedInvoke).not.toHaveBeenCalled();
  });

  it("names no framework or internal term", () => {
    render(<AgendaCard needsAccess="Calendar" settingsUrl={REMINDERS_URL} />);
    const text = screen.getByTestId("agenda-card").textContent ?? "";
    for (const banned of ["EventKit", "TCC", "NS", "plist"]) {
      expect(text).not.toContain(banned);
    }
  });
});
