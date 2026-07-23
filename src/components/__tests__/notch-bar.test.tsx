import { render, screen, waitFor, act, fireEvent } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { NotchBar } from "../bar/notch-bar";
import { UI } from "@/lib/constants.generated";
import type { NotchGeometry } from "@/types/bar-config";

// Mock Tauri APIs. `listen` captures handlers per event name so tests can
// push bar-state updates the way the backend would.
const listeners = new Map<string, (event: { payload: unknown }) => void>();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn((eventName: string, handler: (event: { payload: unknown }) => void) => {
    listeners.set(eventName, handler);
    return Promise.resolve(() => listeners.delete(eventName));
  }),
  emit: vi.fn(),
}));

import { invoke } from "@tauri-apps/api/core";
const mockInvoke = vi.mocked(invoke);

const NOTCH_GEOMETRY: NotchGeometry = {
  has_notch: true,
  notch_width: 210,
  notch_height: 34,
  menu_bar_height: 34,
  canvas_width: 490,
  canvas_height: 184,
};

const PILL_GEOMETRY: NotchGeometry = {
  has_notch: false,
  notch_width: 200,
  notch_height: 30,
  menu_bar_height: 30,
  canvas_width: 480,
  canvas_height: 180,
};

const barStatePayload = (barState: string, overrides: Record<string, unknown> = {}) => ({
  barState,
  inputValue: "",
  lastSubmittedValue: "",
  currentError: null,
  transcriptionText: "",
  spokenText: "",
  voiceMode: UI.VOICE_MODES_IDLE,
  audioLevel: 0,
  isAgentWorking: false,
  isDictationMode: false,
  isAlwaysListening: false,
  agentState: null,
  ...overrides,
});

const emitBarState = (barState: string, overrides: Record<string, unknown> = {}) => {
  const handler = listeners.get("bar-state-update");
  expect(handler).toBeDefined();
  act(() => {
    handler?.({ payload: barStatePayload(barState, overrides) });
  });
};

describe("NotchBar", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    listeners.clear();
  });

  it("sizes the idle silhouette to the reported notch cutout", async () => {
    mockInvoke.mockImplementation((cmd) =>
      cmd === "get_notch_geometry"
        ? Promise.resolve(NOTCH_GEOMETRY)
        : Promise.resolve(undefined),
    );

    render(<NotchBar />);

    await waitFor(() => {
      const shape = screen.getByTestId("notch-shape");
      expect(shape.style.width).toBe("210px");
      expect(shape.style.height).toBe("34px");
    });
  });

  it("renders a centered pill on notch-less displays", async () => {
    mockInvoke.mockImplementation((cmd) =>
      cmd === "get_notch_geometry"
        ? Promise.resolve(PILL_GEOMETRY)
        : Promise.resolve(undefined),
    );

    render(<NotchBar />);

    await waitFor(() => {
      const shape = screen.getByTestId("notch-shape");
      expect(shape.className).toContain("rounded-full");
      expect(shape.style.width).toBe("200px");
    });
  });

  it("expands via CSS sizing on agent activity and shows the state label", async () => {
    mockInvoke.mockImplementation((cmd) =>
      cmd === "get_notch_geometry"
        ? Promise.resolve(NOTCH_GEOMETRY)
        : Promise.resolve(undefined),
    );

    render(<NotchBar />);
    await waitFor(() => expect(listeners.has("bar-state-update")).toBe(true));

    emitBarState(UI.BAR_STATES_LOADING, { agentState: "working" });

    const shape = screen.getByTestId("notch-shape");
    // Wider and taller than the idle silhouette — but only through CSS,
    // the window canvas is never resized.
    expect(Number.parseInt(shape.style.width)).toBeGreaterThan(210);
    expect(Number.parseInt(shape.style.height)).toBeGreaterThan(34);
    expect(screen.getByTestId("notch-label").textContent).toBe("working");
  });

  it("shows an input field in input state and submits via interaction", async () => {
    mockInvoke.mockImplementation((cmd) =>
      cmd === "get_notch_geometry"
        ? Promise.resolve(NOTCH_GEOMETRY)
        : Promise.resolve(undefined),
    );

    render(<NotchBar />);
    await waitFor(() => expect(listeners.has("bar-state-update")).toBe(true));

    emitBarState(UI.BAR_STATES_INPUT);

    const input = screen.getByPlaceholderText("Ask Juno") as HTMLInputElement;
    act(() => {
      input.focus();
    });
    act(() => {
      fireEvent.change(input, { target: { value: "open safari" } });
      fireEvent.submit(input.closest("form") as HTMLFormElement);
    });

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith(
        "ui_handle_interaction",
        expect.objectContaining({
          elementId: "notch-bar",
          interaction: expect.objectContaining({
            interaction_type: UI.INTERACTION_TYPES_SUBMIT,
            data: { value: "open safari" },
          }),
        }),
      );
    });
  });

  it("requests activation on idle click", async () => {
    mockInvoke.mockImplementation((cmd) =>
      cmd === "get_notch_geometry"
        ? Promise.resolve(NOTCH_GEOMETRY)
        : Promise.resolve(undefined),
    );

    render(<NotchBar />);
    await waitFor(() => expect(listeners.has("bar-state-update")).toBe(true));

    fireEvent.click(screen.getByTestId("notch-shape"));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith(
        "ui_handle_interaction",
        expect.objectContaining({
          interaction: expect.objectContaining({
            interaction_type: UI.INTERACTION_TYPES_CLICK,
          }),
        }),
      );
    });
  });
});
