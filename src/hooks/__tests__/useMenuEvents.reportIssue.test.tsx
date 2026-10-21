import { renderHook, waitFor } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { useMenuEvents } from "@/hooks/useMenuEvents";
import { COMMANDS, EVENTS } from "@/lib/constants.generated";

const invoke = vi.fn(() => Promise.resolve("https://github.com/lacymorrow/juno/issues/new?..."));
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invoke(...(args as [])),
}));

// Capture every handler the hook registers, keyed by event name, so a test can
// fire a menu event with the payload the Rust menu actually sends.
const handlers = new Map<string, (event: { payload: unknown }) => void>();
vi.mock("@tauri-apps/api/event", () => ({
  listen: (event: string, handler: (e: { payload: unknown }) => void) => {
    handlers.set(event, handler);
    return Promise.resolve(() => {});
  },
}));

function renderMenuEvents() {
  const setActiveModal = vi.fn();
  const setFeedbackData = vi.fn();
  const addSystemMessage = vi.fn();
  renderHook(() =>
    useMenuEvents({
      setCurrentView: vi.fn(),
      setIsDevPanelOpen: vi.fn(),
      setActiveModal,
      setFeedbackData,
      startNewChat: vi.fn(),
      addSystemMessage,
      handleUpdateCheck: vi.fn(() => Promise.resolve()),
    }),
  );
  return { setActiveModal, setFeedbackData, addSystemMessage };
}

describe("useMenuEvents: Report Issue goes to the bug form", () => {
  beforeEach(() => {
    handlers.clear();
    invoke.mockClear();
  });

  // The regression: `addListener` used to drop the event payload, so both menu
  // items opened the modal as "General Feedback" and a bug report was mailed
  // instead of filed (LAC-5069).
  it("opens the GitHub bug form and no modal for the Report Issue payload", async () => {
    const { setActiveModal, setFeedbackData } = renderMenuEvents();
    await waitFor(() => expect(handlers.has(EVENTS.MENU_SHOW_FEEDBACK)).toBe(true));

    handlers.get(EVENTS.MENU_SHOW_FEEDBACK)?.({ payload: "issue" });

    expect(invoke).toHaveBeenCalledWith(COMMANDS.ISSUE_REPORT_OPEN, { kind: "bug" });
    expect(setActiveModal).not.toHaveBeenCalled();
    expect(setFeedbackData).not.toHaveBeenCalled();
  });

  it("still opens the feedback modal for the Send Feedback payload", async () => {
    const { setActiveModal } = renderMenuEvents();
    await waitFor(() => expect(handlers.has(EVENTS.MENU_SHOW_FEEDBACK)).toBe(true));

    handlers.get(EVENTS.MENU_SHOW_FEEDBACK)?.({ payload: "feedback" });

    expect(setActiveModal).toHaveBeenCalledWith("feedback");
    expect(invoke).not.toHaveBeenCalled();
  });

  it("says so in the chat when the form cannot be opened", async () => {
    // Built inside the implementation, not passed in: a pre-rejected promise
    // is unhandled until the hook attaches its catch, which vitest reports.
    invoke.mockImplementationOnce(() => Promise.reject(new Error("no browser")));
    const { addSystemMessage } = renderMenuEvents();
    await waitFor(() => expect(handlers.has(EVENTS.MENU_SHOW_FEEDBACK)).toBe(true));

    handlers.get(EVENTS.MENU_SHOW_FEEDBACK)?.({ payload: "issue" });

    await waitFor(() =>
      expect(addSystemMessage).toHaveBeenCalledWith(
        expect.stringContaining("Could not open the bug report form"),
      ),
    );
  });
});
