import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { MixedContentRenderer } from "@/components/ui/mixed-content-renderer";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
const mockedInvoke = vi.mocked(invoke);

const REMINDER = `<Form title="New reminder" submit="Create reminder" intro="Create this reminder:">
  <Field label="What"><Input name="what" /></Field>
  <Field label="Due date"><Input name="due_date" /></Field>
  <Field label="Alert me"><Switch name="alert" defaultChecked /></Field>
  <Field label="Notes"><Textarea name="notes" /></Field>
</Form>`;

describe("Form", () => {
  beforeEach(() => {
    mockedInvoke.mockReset();
    mockedInvoke.mockResolvedValue(undefined);
  });

  it("sends the values as the person's next message through dispatch_query", async () => {
    const { container } = render(<MixedContentRenderer content={REMINDER} />);
    fireEvent.change(container.querySelector('input[name="what"]')!, { target: { value: "Call Maya" } });
    fireEvent.change(container.querySelector('input[name="due_date"]')!, { target: { value: "Tomorrow 9 AM" } });
    fireEvent.click(screen.getByRole("button", { name: "Create reminder" }));

    await waitFor(() => expect(mockedInvoke).toHaveBeenCalledTimes(1));
    expect(mockedInvoke).toHaveBeenCalledWith("dispatch_query", {
      query: "Create this reminder:\nWhat: Call Maya\nDue date: Tomorrow 9 AM\nAlert: Yes",
    });
    // Answered: the form locks and says so, instead of inviting a second send.
    await screen.findByRole("button", { name: "Sent" });
    expect(screen.getByRole("button", { name: "Sent" })).toBeDisabled();
  });

  it("has exactly one primary button", () => {
    render(<MixedContentRenderer content={REMINDER} />);
    expect(screen.getAllByRole("button", { name: /create reminder/i })).toHaveLength(1);
  });

  it("says so in words, and logs the raw error, when the send fails", async () => {
    mockedInvoke.mockRejectedValue(new Error("ipc exploded"));
    const log = vi.spyOn(console, "error").mockImplementation(() => {});
    render(<MixedContentRenderer content={REMINDER} />);
    fireEvent.click(screen.getByRole("button", { name: "Create reminder" }));

    await screen.findByText("Could not send that. Try again.");
    expect(screen.queryByText(/ipc exploded/)).toBeNull();
    expect(log).toHaveBeenCalled();
    expect(screen.getByRole("button", { name: "Create reminder" })).not.toBeDisabled();
    log.mockRestore();
  });

  it("draws a control outside a Form as nothing: no dead controls", () => {
    const { container } = render(<MixedContentRenderer content={'Here:\n\n<Input name="x" placeholder="Nope" />'} />);
    expect(container.querySelector("input")).toBeNull();
  });

  it("draws nothing for a Form with no fields", () => {
    const { container } = render(<MixedContentRenderer content={'<Form title="Empty" submit="Send"></Form>'} />);
    expect(container.querySelector("form")).toBeNull();
  });
});
