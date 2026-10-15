/**
 * The coaching card renders what Rust tells it and decides nothing.
 *
 * The placement, the side, the timing and the dismissal all have their tests in
 * `src-tauri/src/permission_coach.rs`. These cover the half that is the page:
 * that it says what it was told, points the way it was told, turns into its own
 * receipt, and survives mounting after the event that carried its state.
 */

import { render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { PermissionCoach } from "@/components/PermissionCoach";
import { EVENTS } from "@/lib/constants.generated";

const invoke = vi.fn();
const handlers = new Map<string, (payload: unknown) => void>();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invoke(...args),
}));

vi.mock("@/hooks/useEventListener", () => ({
  useEventListener: (event: string, handler: (payload: unknown) => void) => {
    handlers.set(event, handler);
  },
}));

/** Push an event the way the backend would. */
function emit(payload: unknown) {
  const handler = handlers.get(EVENTS.PERMISSIONS_COACH);
  if (!handler) throw new Error("the card is not listening for its event");
  handler(payload);
}

const POINTING = {
  permission: "accessibility",
  headline: "Find Juno in the list and turn its switch on",
  arrow: "right",
  granted: false,
  receipt: "Accessibility is on",
};

describe("PermissionCoach", () => {
  beforeEach(() => {
    handlers.clear();
    invoke.mockReset();
    invoke.mockResolvedValue(null);
  });

  it("shows nothing until Rust says there is something to point at", async () => {
    render(<PermissionCoach />);
    await waitFor(() => expect(invoke).toHaveBeenCalled());
    expect(screen.queryByTestId("permission-coach")).toBeNull();
  });

  it("says the one line it was given, and nothing else", async () => {
    render(<PermissionCoach />);
    emit(POINTING);
    await waitFor(() => expect(screen.getByTestId("permission-coach")).toBeInTheDocument());
    expect(screen.getByText(POINTING.headline)).toBeInTheDocument();
    // One line of text on the card, and only one. Nothing explaining what
    // Accessibility is, and no second sentence softening the first.
    const lines = screen.getByTestId("permission-coach").querySelectorAll("p");
    expect(lines).toHaveLength(1);
    expect(lines[0].textContent).toBe(POINTING.headline);
  });

  it("points the way Rust says, and swaps side when Rust says so", async () => {
    render(<PermissionCoach />);
    emit(POINTING);
    await waitFor(() => expect(screen.getByTestId("permission-coach-arrow")).toBeInTheDocument());

    const card = screen.getByTestId("permission-coach");
    // Card first, arrow last: the arrow touches the window on the right.
    expect(card.className).toContain("flex-row");
    expect(card.className).not.toContain("flex-row-reverse");

    emit({ ...POINTING, arrow: "left" });
    await waitFor(() => expect(card.className).toContain("flex-row-reverse"));
  });

  it("becomes its own receipt rather than vanishing", async () => {
    render(<PermissionCoach />);
    emit(POINTING);
    await waitFor(() => expect(screen.getByTestId("permission-coach")).toBeInTheDocument());

    emit({ ...POINTING, granted: true });
    await waitFor(() => expect(screen.getByText(POINTING.receipt)).toBeInTheDocument());
    // Nothing left to point at.
    expect(screen.queryByTestId("permission-coach-arrow")).toBeNull();
  });

  it("asks for the current state when it mounts, for the launch-race case", async () => {
    invoke.mockResolvedValue(POINTING);
    render(<PermissionCoach />);
    await waitFor(() => expect(screen.getByText(POINTING.headline)).toBeInTheDocument());
  });

  it("does not let a late first read undo an event that already arrived", async () => {
    // The invoke resolves after the event, carrying the state as it was before
    // the grant. Applying it would put the card back to pointing at a switch
    // that is already on.
    let settle: (value: unknown) => void = () => {};
    invoke.mockReturnValue(new Promise((resolve) => (settle = resolve)));

    render(<PermissionCoach />);
    emit({ ...POINTING, granted: true });
    await waitFor(() => expect(screen.getByText(POINTING.receipt)).toBeInTheDocument());

    settle(POINTING);
    await waitFor(() => expect(screen.getByText(POINTING.receipt)).toBeInTheDocument());
    expect(screen.queryByText(POINTING.headline)).toBeNull();
  });

  it("falls back to a sane direction if the arrow is one it does not know", async () => {
    render(<PermissionCoach />);
    emit({ ...POINTING, arrow: "sideways" });
    await waitFor(() => expect(screen.getByTestId("permission-coach")).toBeInTheDocument());
    expect(screen.getByTestId("permission-coach-arrow")).toBeInTheDocument();
    expect(screen.getByTestId("permission-coach").className).toContain("flex-row");
  });
});
