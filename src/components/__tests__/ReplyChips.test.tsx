import { act, fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { ReplyChips } from "../chat/ReplyChips";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(() => Promise.resolve()) }));

const handlers = vi.hoisted(() => ({ current: null as null | ((p: unknown) => void) }));
vi.mock("@/hooks/useEventListener", () => ({
  useEventListener: (_event: string, handler: (p: unknown) => void) => {
    handlers.current = handler;
  },
}));

const send = (payload: unknown) => act(() => handlers.current?.(payload));

describe("ReplyChips", () => {
  beforeEach(() => vi.mocked(invoke).mockClear());

  it("renders nothing until the backend offers chips", () => {
    render(<ReplyChips />);
    expect(screen.queryByTestId("reply-chips")).not.toBeInTheDocument();
  });

  it("shows the alternatives and a tap asks the backend to run it", () => {
    render(<ReplyChips />);
    send({
      chips: [
        { id: "alt-0", label: "Open Mac settings" },
        { id: "alt-1", label: "Open Juno settings" },
      ],
    });
    expect(screen.getAllByRole("button")).toHaveLength(2);
    fireEvent.click(screen.getByRole("button", { name: "Open Juno settings" }));
    expect(invoke).toHaveBeenCalledWith("run_reply_chip", { id: "alt-1" });
  });

  it("an empty list from the backend clears the row", () => {
    render(<ReplyChips />);
    send({ chips: [{ id: "alt-0", label: "Open Mac settings" }] });
    expect(screen.getByTestId("reply-chips")).toBeInTheDocument();
    send({ chips: [] });
    expect(screen.queryByTestId("reply-chips")).not.toBeInTheDocument();
  });
});
