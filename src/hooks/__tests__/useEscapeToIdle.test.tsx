import { act, fireEvent, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { UI } from "@/lib/constants.generated";
import { ESCAPE_REARM_MS } from "@/lib/barEscape";
import { useEscapeToIdle, type EscapeToIdleOptions } from "@/hooks/useEscapeToIdle";

const invoke = vi.hoisted(() =>
  vi.fn((_command: string, _args?: Record<string, unknown>) => Promise.resolve()),
);
vi.mock("@tauri-apps/api/core", () => ({ invoke }));

/** The backend events the hook subscribes to, so a test can fire one. */
const handlers = vi.hoisted(() => new Map<string, (payload: unknown) => void>());
vi.mock("@/hooks/useEventListener", () => ({
  useEventListener: (event: string, handler: (payload: unknown) => void) => {
    handlers.set(event, handler);
  },
}));

const DISMISS = "bar-dismiss-pane";
const PANE_OPEN = "set_bar_pane_open";

function render(over: Partial<EscapeToIdleOptions> = {}) {
  const collapse = vi.fn();
  const report = vi.fn();
  const initial: EscapeToIdleOptions = {
    barState: UI.BAR_STATES_DEFAULT,
    working: false,
    overlayOpen: false,
    composerOpen: false,
    popupOpen: false,
    collapse,
    report,
    ...over,
  };
  const view = renderHook((props: EscapeToIdleOptions) => useEscapeToIdle(props), {
    initialProps: initial,
  });
  return { ...view, collapse, report, initial };
}

const pressEscape = () => fireEvent.keyDown(document, { key: "Escape" });
const backendDismiss = () => act(() => void handlers.get(DISMISS)?.(null));
const paneCalls = (): boolean[] =>
  invoke.mock.calls
    .filter(([command]) => command === PANE_OPEN)
    .map(([, args]) => Boolean(args?.open));

beforeEach(() => {
  invoke.mockClear();
  handlers.clear();
});

describe("useEscapeToIdle: claiming the stop key", () => {
  it("claims it while the bar is expanded and lets go when it settles", () => {
    const { rerender, initial } = render({ overlayOpen: true });
    expect(paneCalls()).toEqual([true]);

    rerender({ ...initial, overlayOpen: false });
    expect(paneCalls()).toEqual([true, false]);
  });

  it("claims nothing for a bar at rest with nothing open", () => {
    render();
    expect(paneCalls()).toEqual([false]);
  });

  it("claims it for a bar state Rust is busy in, with no layer of its own open", () => {
    // Before this, only the Pill ever armed the monitor, and only while its
    // pane was open. Every other look ignored a press outside its own window.
    render({ barState: UI.BAR_STATES_SPEAKING });
    expect(paneCalls()).toEqual([true]);
  });

  it("re-asserts the claim so the backend's stale sweep cannot disarm it", () => {
    vi.useFakeTimers();
    try {
      render({ overlayOpen: true });
      expect(paneCalls()).toEqual([true]);
      act(() => {
        vi.advanceTimersByTime(ESCAPE_REARM_MS + 1);
      });
      expect(paneCalls()).toEqual([true, true]);
    } finally {
      vi.useRealTimers();
    }
  });

  it("lets go when the appearance is swapped out mid-answer", () => {
    const { unmount } = render({ overlayOpen: true });
    invoke.mockClear();
    unmount();
    expect(paneCalls()).toEqual([false]);
  });
});

describe("useEscapeToIdle: one press", () => {
  it("closes the look's own layer without asking Rust to stop anything", () => {
    const { collapse, report } = render({ overlayOpen: true });
    pressEscape();
    expect(collapse).toHaveBeenCalledTimes(1);
    expect(report).not.toHaveBeenCalled();
  });

  it("reports a state only Rust can end, and puts the layer away in the same press", () => {
    const { collapse, report } = render({
      barState: UI.BAR_STATES_AGENT_RESPONDING,
      working: true,
      overlayOpen: true,
    });
    pressEscape();
    expect(collapse).toHaveBeenCalledTimes(1);
    expect(report).toHaveBeenCalledTimes(1);
  });

  it("reports speaking, which every appearance used to swallow", () => {
    const { report } = render({ barState: UI.BAR_STATES_SPEAKING });
    pressEscape();
    expect(report).toHaveBeenCalledTimes(1);
  });

  it("does nothing at rest with nothing open", () => {
    const { collapse, report } = render();
    pressEscape();
    expect(collapse).not.toHaveBeenCalled();
    expect(report).not.toHaveBeenCalled();
  });

  it("leaves the key to a popup that owns it", () => {
    const { collapse, report } = render({ overlayOpen: true, popupOpen: true });
    pressEscape();
    expect(collapse).not.toHaveBeenCalled();
    expect(report).not.toHaveBeenCalled();
  });
});

describe("useEscapeToIdle: the two routes", () => {
  it("acts on a press Rust saw outside Juno's own windows", () => {
    const { collapse } = render({ overlayOpen: true });
    backendDismiss();
    expect(collapse).toHaveBeenCalledTimes(1);
  });

  it("acts once when both routes see the same press", () => {
    // The bar is focused, so the DOM sees the keystroke and Rust's monitor
    // reports the same one a moment later. One press, one collapse.
    const { collapse } = render({ overlayOpen: true, composerOpen: true });
    pressEscape();
    backendDismiss();
    expect(collapse).toHaveBeenCalledTimes(1);
  });

  it("acts twice on two real presses, however fast they come", () => {
    const { collapse } = render({ overlayOpen: true });
    pressEscape();
    pressEscape();
    expect(collapse).toHaveBeenCalledTimes(2);
  });

  it("reads the state as it is when the key arrives, not when it was wired", () => {
    const { rerender, initial, collapse, report } = render();
    pressEscape();
    expect(collapse).not.toHaveBeenCalled();

    rerender({ ...initial, barState: UI.BAR_STATES_ERROR, overlayOpen: true });
    pressEscape();
    expect(collapse).toHaveBeenCalledTimes(1);
    expect(report).toHaveBeenCalledTimes(1);
  });
});
