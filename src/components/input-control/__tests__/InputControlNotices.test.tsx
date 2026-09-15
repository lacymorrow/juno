import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const listeners = new Map<string, Array<(payload: unknown) => void>>();

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async (event: string, handler: (e: { payload: unknown }) => void) => {
    const wrapped = (payload: unknown) => handler({ payload });
    const forEvent = listeners.get(event) ?? [];
    forEvent.push(wrapped);
    listeners.set(event, forEvent);
    return () => {
      listeners.set(
        event,
        (listeners.get(event) ?? []).filter((fn) => fn !== wrapped),
      );
    };
  }),
  emit: vi.fn(),
}));

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/plugin-process", () => ({ exit: vi.fn() }));

import { invoke } from "@tauri-apps/api/core";
import { exit } from "@tauri-apps/plugin-process";

import { InputControlNotices } from "../InputControlNotices";
import { COMMANDS, EVENTS } from "@/lib/constants.generated";

const invokeMock = vi.mocked(invoke);
const exitMock = vi.mocked(exit);

/** Push a backend event at the component the way Tauri would. */
async function fire(event: string, payload?: unknown) {
  await act(async () => {
    for (const handler of listeners.get(event) ?? []) handler(payload);
    // Let the listener's own awaits settle.
    await Promise.resolve();
  });
}

/** Click a button and let the handler's promises settle. */
async function click(element: HTMLElement) {
  await act(async () => {
    fireEvent.click(element);
    await Promise.resolve();
  });
}

/** Render, then wait for the async `listen()` calls to be in place. */
async function mount() {
  render(<InputControlNotices />);
  await waitFor(() => expect(listeners.get(EVENTS.INPUT_CONTROL_REQUEST)?.length).toBe(1));
}

const REQUEST = {
  request_id: "req-1",
  tool: "computer",
  reason: "drag the clip onto the timeline",
  target_app: "Final Cut Pro",
};

beforeEach(() => {
  listeners.clear();
  invokeMock.mockReset();
  exitMock.mockReset();
  invokeMock.mockResolvedValue(undefined);
});

describe("InputControlNotices", () => {
  it("shows nothing until the backend has something to say", async () => {
    const { container } = render(<InputControlNotices />);
    expect(container).toBeEmptyDOMElement();
  });

  it("names what Juno wants and which app it is in", async () => {
    await mount();
    await fire(EVENTS.INPUT_CONTROL_REQUEST, REQUEST);

    expect(screen.getByTestId("input-control-prompt")).toBeInTheDocument();
    expect(screen.getByText("Juno needs your mouse")).toBeInTheDocument();
    expect(
      screen.getByText(/drag the clip onto the timeline in Final Cut Pro/),
    ).toBeInTheDocument();
  });

  it.each([
    ["Allow once", "once"],
    ["Always allow", "always"],
    ["Not now", "deny"],
  ])("sends %s as %s", async (label, decision) => {
    await mount();
    await fire(EVENTS.INPUT_CONTROL_REQUEST, REQUEST);

    await click(screen.getByRole("button", { name: label }));

    expect(invokeMock).toHaveBeenCalledWith(
      COMMANDS.INPUT_CONTROL_RESPOND_TO_REQUEST,
      { requestId: "req-1", decision },
    );
    await waitFor(() =>
      expect(screen.queryByTestId("input-control-prompt")).not.toBeInTheDocument(),
    );
  });

  it("keeps the prompt up and says so when the answer does not land", async () => {
    invokeMock.mockRejectedValueOnce(new Error("no backend"));
    await mount();
    await fire(EVENTS.INPUT_CONTROL_REQUEST, REQUEST);

    await click(screen.getByRole("button", { name: "Allow once" }));

    expect(await screen.findByText("Juno did not get that. Try again.")).toBeInTheDocument();
    expect(screen.getByTestId("input-control-prompt")).toBeInTheDocument();
  });

  it("clears the prompt once Juno actually has the mouse", async () => {
    await mount();
    await fire(EVENTS.INPUT_CONTROL_REQUEST, REQUEST);
    await fire(EVENTS.INPUT_CONTROL_STATE, { active: true, target_app: "Final Cut Pro" });

    expect(screen.queryByTestId("input-control-prompt")).not.toBeInTheDocument();
  });

  it("never takes focus by itself", async () => {
    await mount();
    await fire(EVENTS.INPUT_CONTROL_REQUEST, REQUEST);
    expect(document.activeElement).toBe(document.body);
  });

  it("puts every action in the keyboard's reach, in reading order", async () => {
    await mount();
    await fire(EVENTS.INPUT_CONTROL_REQUEST, REQUEST);

    const buttons = screen.getAllByRole("button");
    expect(buttons.map((b) => b.textContent)).toEqual([
      "Allow once",
      "Always allow",
      "Not now",
    ]);
    // Native buttons with no negative tabindex: each one is tab-reachable.
    for (const button of buttons) {
      expect(button.tagName).toBe("BUTTON");
      expect(button).not.toHaveAttribute("tabindex", "-1");
      button.focus();
      expect(button).toHaveFocus();
    }
  });
});

describe("the offer to stop asking", () => {
  const askMode = { mouse_control: "ask", mouse_control_prompt_dismissed: false };

  it("appears while Juno still asks every time", async () => {
    invokeMock.mockResolvedValue(askMode);
    await mount();
    await fire(EVENTS.INPUT_CONTROL_OFFER);

    expect(
      await screen.findByText("Let Juno use the mouse without asking?"),
    ).toBeInTheDocument();
  });

  it("stays quiet when Juno is already allowed to take the mouse", async () => {
    invokeMock.mockResolvedValue({
      mouse_control: "always",
      mouse_control_prompt_dismissed: false,
    });
    await mount();
    await fire(EVENTS.INPUT_CONTROL_OFFER);

    expect(screen.queryByTestId("mouse-control-offer")).not.toBeInTheDocument();
  });

  it("stays quiet after the person said not to ask again", async () => {
    invokeMock.mockResolvedValue({
      mouse_control: "ask",
      mouse_control_prompt_dismissed: true,
    });
    await mount();
    await fire(EVENTS.INPUT_CONTROL_OFFER);

    expect(screen.queryByTestId("mouse-control-offer")).not.toBeInTheDocument();
  });

  it("stays quiet when the settings cannot be read", async () => {
    invokeMock.mockRejectedValue(new Error("no backend"));
    await mount();
    await fire(EVENTS.INPUT_CONTROL_OFFER);

    expect(screen.queryByTestId("mouse-control-offer")).not.toBeInTheDocument();
  });

  it("never offers twice in one session", async () => {
    invokeMock.mockResolvedValue(askMode);
    await mount();

    await fire(EVENTS.INPUT_CONTROL_OFFER);
    await screen.findByTestId("mouse-control-offer");
    await click(screen.getByRole("button", { name: "No" }));
    expect(screen.queryByTestId("mouse-control-offer")).not.toBeInTheDocument();

    await fire(EVENTS.INPUT_CONTROL_OFFER);
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(screen.queryByTestId("mouse-control-offer")).not.toBeInTheDocument();
  });

  it("switches Juno to always on Yes", async () => {
    invokeMock.mockResolvedValue(askMode);
    await mount();
    await fire(EVENTS.INPUT_CONTROL_OFFER);
    await screen.findByTestId("mouse-control-offer");

    await click(screen.getByRole("button", { name: "Yes" }));

    expect(invokeMock).toHaveBeenCalledWith(
      COMMANDS.INPUT_CONTROL_SET_MOUSE_CONTROL,
      { mode: "always" },
    );
  });

  it("remembers the refusal on Don't ask again", async () => {
    invokeMock.mockResolvedValue(askMode);
    await mount();
    await fire(EVENTS.INPUT_CONTROL_OFFER);
    await screen.findByTestId("mouse-control-offer");

    await click(screen.getByRole("button", { name: "Don't ask again" }));

    expect(invokeMock).toHaveBeenCalledWith(COMMANDS.INPUT_CONTROL_DISMISS_PROMPT);
  });
});

describe("the reopen notice", () => {
  it("points at the menu bar on the first try", async () => {
    await mount();
    await fire(EVENTS.APP_LIFECYCLE_REOPEN_ATTEMPT, { count: 1 });

    const notice = screen.getByTestId("reopen-notice");
    expect(notice).toHaveAttribute("data-notice", "hint");
    expect(screen.getByText(/lives in the menu bar/)).toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "Show Juno in the Dock again" }),
    ).not.toBeInTheDocument();
  });

  it("offers the way out once the person is clearly stuck", async () => {
    await mount();
    await fire(EVENTS.APP_LIFECYCLE_REOPEN_ATTEMPT, { count: 3 });

    expect(screen.getByTestId("reopen-notice")).toHaveAttribute(
      "data-notice",
      "escalated",
    );
    expect(
      screen.getByRole("button", { name: "Show Juno in the Dock again" }),
    ).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Quit Juno" })).toBeInTheDocument();
  });

  it("puts the Dock icon back on request", async () => {
    await mount();
    await fire(EVENTS.APP_LIFECYCLE_REOPEN_ATTEMPT, { count: 4 });

    await click(screen.getByRole("button", { name: "Show Juno in the Dock again" }));

    expect(invokeMock).toHaveBeenCalledWith(
      COMMANDS.INPUT_CONTROL_SET_DOCK_ICON_VISIBLE,
      { visible: true },
    );
    await waitFor(() =>
      expect(screen.queryByTestId("reopen-notice")).not.toBeInTheDocument(),
    );
  });

  it("quits when that is what they want", async () => {
    await mount();
    await fire(EVENTS.APP_LIFECYCLE_REOPEN_ATTEMPT, { count: 3 });

    await click(screen.getByRole("button", { name: "Quit Juno" }));

    expect(exitMock).toHaveBeenCalledWith(0);
  });
});
