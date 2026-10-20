import { act, fireEvent, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { UI } from "@/lib/constants.generated";
import { ESCAPE_REARM_MS, LADDER_CHAIN_MS, PRESENTED_MS } from "@/lib/barEscape";
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
const LADDER = "bar-ladder";
const PANE_OPEN = "set_bar_pane_open";
const HIDE_BAR = "hide_floating_bar";
const OPEN_MAIN = "open_main_window";

function render(over: Partial<EscapeToIdleOptions> = {}) {
  const collapse = vi.fn();
  const report = vi.fn();
  const climb = vi.fn();
  const initial: EscapeToIdleOptions = {
    barState: UI.BAR_STATES_DEFAULT,
    working: false,
    overlayOpen: false,
    composerOpen: false,
    popupOpen: false,
    collapse,
    report,
    climb,
    ...over,
  };
  const view = renderHook((props: EscapeToIdleOptions) => useEscapeToIdle(props), {
    initialProps: initial,
  });
  return { ...view, collapse, report, climb, initial };
}

const pressEscape = () => fireEvent.keyDown(document, { key: "Escape" });
const backendDismiss = () => act(() => void handlers.get(DISMISS)?.(null));
const ladder = (to: string) => act(() => void handlers.get(LADDER)?.({ to }));
const called = (command: string) => invoke.mock.calls.filter(([c]) => c === command).length;
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

  it("takes the bar off the screen from rest when the press was typed into the bar", () => {
    const { collapse, report } = render();
    pressEscape();
    expect(collapse).not.toHaveBeenCalled();
    expect(report).not.toHaveBeenCalled();
    expect(called(HIDE_BAR)).toBe(1);
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

describe("useEscapeToIdle: the ladder down", () => {
  it("walks floating chat, idle bar, hidden: one rung per press", () => {
    const { rerender, initial, collapse } = render({ overlayOpen: true });
    backendDismiss();
    expect(collapse).toHaveBeenCalledTimes(1);
    expect(called(HIDE_BAR)).toBe(0);

    rerender({ ...initial, overlayOpen: false });
    backendDismiss();
    expect(called(HIDE_BAR)).toBe(1);
  });

  it("never hides the bar for an Escape typed in another app", () => {
    // At rest, nothing just happened: a press the monitor saw belongs to
    // whatever app the person is in.
    render();
    backendDismiss();
    expect(called(HIDE_BAR)).toBe(0);
  });

  it("lets the walk lapse", () => {
    vi.useFakeTimers();
    try {
      const { rerender, initial } = render({ overlayOpen: true });
      backendDismiss();
      rerender({ ...initial, overlayOpen: false });
      act(() => {
        vi.advanceTimersByTime(LADDER_CHAIN_MS + 1);
      });
      backendDismiss();
      expect(called(HIDE_BAR)).toBe(0);
    } finally {
      vi.useRealTimers();
    }
  });

  it("keeps the stop key claimed for the next rung, then lets go", () => {
    vi.useFakeTimers();
    try {
      const { rerender, initial } = render({ overlayOpen: true });
      backendDismiss();
      rerender({ ...initial, overlayOpen: false });
      expect(paneCalls().at(-1)).toBe(true);
      act(() => {
        vi.advanceTimersByTime(LADDER_CHAIN_MS + 1);
      });
      expect(paneCalls().at(-1)).toBe(false);
    } finally {
      vi.useRealTimers();
    }
  });

  it("does not hide the bar because the big chat took over", () => {
    const { rerender, initial, collapse } = render({ overlayOpen: true });
    backendDismiss();
    rerender({ ...initial, overlayOpen: false });
    act(() => void handlers.get(DISMISS)?.({ reason: "main_window" }));
    expect(collapse).toHaveBeenCalledTimes(1);
    expect(called(HIDE_BAR)).toBe(0);
  });
});

describe("useEscapeToIdle: the ladder up", () => {
  it("opens the text input when Juno comes back from hidden", () => {
    const { climb } = render();
    ladder("composer");
    expect(climb).toHaveBeenCalledWith("composer");
  });

  it("opens the floating chat from the idle bar or the text input", () => {
    const { climb } = render({ composerOpen: true });
    ladder("next");
    expect(climb).toHaveBeenCalledWith("floating");
    expect(called(OPEN_MAIN)).toBe(0);
  });

  it("opens the big chat from the floating chat", () => {
    const { climb } = render({ overlayOpen: true });
    ladder("next");
    expect(climb).not.toHaveBeenCalled();
    expect(called(OPEN_MAIN)).toBe(1);
  });

  it("opens the floating chat when Escape steps down from the big chat", () => {
    const { climb } = render();
    ladder("floating");
    expect(climb).toHaveBeenCalledWith("floating");
  });
});

describe("useEscapeToIdle: just after Juno appears by herself", () => {
  const presented = () => act(() => void handlers.get("bar-presented")?.(null));

  it("claims the stop key, and the first Escape puts her away though the bar was never focused", () => {
    render();
    presented();
    expect(paneCalls().at(-1)).toBe(true);
    backendDismiss();
    expect(called(HIDE_BAR)).toBe(1);
  });

  it("is over after one press", () => {
    render();
    presented();
    backendDismiss();
    invoke.mockClear();
    vi.useFakeTimers();
    try {
      act(() => {
        vi.advanceTimersByTime(LADDER_CHAIN_MS + 1);
      });
      backendDismiss();
      expect(called(HIDE_BAR)).toBe(0);
    } finally {
      vi.useRealTimers();
    }
  });

  it("lapses, and a press elsewhere ends it sooner", () => {
    vi.useFakeTimers();
    try {
      render();
      presented();
      act(() => {
        vi.advanceTimersByTime(PRESENTED_MS + 1);
      });
      expect(paneCalls().at(-1)).toBe(false);
      backendDismiss();
      expect(called(HIDE_BAR)).toBe(0);

      presented();
      act(() => void handlers.get("mouse-pressed-outside")?.(null));
      backendDismiss();
      expect(called(HIDE_BAR)).toBe(0);
    } finally {
      vi.useRealTimers();
    }
  });
});
