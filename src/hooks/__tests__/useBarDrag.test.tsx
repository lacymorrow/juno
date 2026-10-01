/**
 * The one drag gesture every bar appearance shares: drag from anywhere, land
 * in a gravity well.
 *
 * The five assertions in "the shared bar drag gesture" below were moved here
 * verbatim from `components/__tests__/FloatingBar.test.tsx`, where they tested
 * the Pill's private copy of the gesture. The gesture is not the Pill's any
 * more, so neither are its tests. The only edits are the ones the harness
 * forces: the mic button is a plain button with a spy instead of the Pill's
 * `agent_voice` invoke, and the press-was-a-click test does not wait for a
 * launch placement the harness does not perform.
 */

import { act, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { useBarDrag } from "../useDragWindow";
import { resetBarSnapState } from "../useBarSnapWells";
import { resetDockSlots } from "@/lib/barDock";

// ── Tauri stand-ins ──────────────────────────────────────────────────
// Deliberately the same fake desktop FloatingBar.test uses: one 1000×800
// display at the origin, a 164×66 window parked at 100,100.

const { invoke, emit } = vi.hoisted(() => ({
  invoke: vi.fn((..._args: unknown[]): Promise<unknown> => Promise.resolve()),
  emit: vi.fn(async () => {}),
}));
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invoke(...args),
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async () => () => {}),
  emit: (...args: unknown[]) => emit(...(args as [])),
}));

const startDragging = vi.hoisted(() => vi.fn(() => Promise.resolve()));
const windowSetPosition = vi.hoisted(() => vi.fn(() => Promise.resolve()));
const outerPos = vi.hoisted(() => ({ x: 100, y: 100 }));
const outerSize = vi.hoisted(() => ({ width: 164, height: 66 }));
const monitors = vi.hoisted(() => ({
  value: [{ position: { x: 0, y: 0 }, size: { width: 1000, height: 800 }, scaleFactor: 1 }],
}));
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    label: "floating-bar",
    startDragging,
    setPosition: windowSetPosition,
    outerPosition: async () => ({ ...outerPos }),
    outerSize: async () => ({ ...outerSize }),
    scaleFactor: async () => 1,
  }),
  availableMonitors: async () => monitors.value,
  cursorPosition: async () => ({ x: 0, y: 0 }),
  PhysicalPosition: class {
    x: number;
    y: number;
    constructor(x: number, y: number) {
      this.x = x;
      this.y = y;
    }
  },
}));

// ── Harness ──────────────────────────────────────────────────────────

const onMicClick = vi.fn();

function Harness() {
  const { dragProps, swallowClickAfterDrag } = useBarDrag();
  return (
    <div data-testid="bar-root" {...dragProps} onClickCapture={swallowClickAfterDrag}>
      <button onClick={onMicClick}>Talk to Juno</button>
      <input placeholder="Ask Juno" />
    </div>
  );
}

const renderBar = async () => {
  const utils = render(<Harness />);
  await act(async () => {});
  return utils;
};

beforeEach(() => {
  resetBarSnapState();
  resetDockSlots();
  invoke.mockClear();
  emit.mockClear();
  startDragging.mockClear();
  windowSetPosition.mockClear();
  onMicClick.mockClear();
  outerPos.x = 100;
  outerPos.y = 100;
  outerSize.width = 164;
  outerSize.height = 66;
});

// A settle glides the window over up to SNAP_MAX_MS of requestAnimationFrame
// ticks that nothing awaits. Let any in-flight glide finish before the next
// test, so one test's animation cannot write positions into another's.
afterEach(async () => {
  await act(async () => {
    await new Promise((r) => setTimeout(r, 400));
  });
});

describe("the shared bar drag gesture", () => {
  it("drags the window from anywhere once the mouse moves, and swallows the click that follows", async () => {
    await renderBar();
    const mic = screen.getByRole("button", { name: "Talk to Juno" });

    fireEvent.mouseDown(mic, { button: 0, clientX: 10, clientY: 10 });
    fireEvent.mouseMove(mic, { clientX: 12, clientY: 11 }); // under the threshold
    expect(startDragging).not.toHaveBeenCalled();
    fireEvent.mouseMove(mic, { clientX: 30, clientY: 20 });
    expect(startDragging).toHaveBeenCalledTimes(1);

    fireEvent.mouseUp(mic);
    fireEvent.click(mic);
    await act(async () => {});
    expect(onMicClick).not.toHaveBeenCalled();
  });

  it("settles into the nearest well after a drag, gliding to it", async () => {
    await renderBar();
    const mic = screen.getByRole("button", { name: "Talk to Juno" });

    // Drag hands off to the OS window drag.
    fireEvent.mouseDown(mic, { button: 0, clientX: 10, clientY: 10 });
    fireEvent.mouseMove(mic, { clientX: 30, clientY: 20 });
    expect(startDragging).toHaveBeenCalledTimes(1);

    // The OS drops the window near the bottom-right; release settles it.
    outerPos.x = 850;
    outerPos.y = 700;
    fireEvent.mouseUp(window);

    // Let the async settle + the rAF glide run to completion.
    await act(async () => {
      await new Promise((r) => setTimeout(r, 500));
    });

    expect(windowSetPosition).toHaveBeenCalled();
    const last = (windowSetPosition.mock.calls.at(-1) as unknown[])[0] as {
      x: number;
      y: number;
    };
    // Nearest well to (850,700) on a 1000×800 monitor, window 164×66:
    // right column x = 1000 − 16 − 164 = 820 ; bottom row y = 800 − 16 − 66 = 718.
    expect(last).toMatchObject({ x: 820, y: 718 });
  });

  it("does not settle when the press was a click, not a drag", async () => {
    await renderBar();
    const mic = screen.getByRole("button", { name: "Talk to Juno" });

    fireEvent.mouseDown(mic, { button: 0, clientX: 10, clientY: 10 });
    fireEvent.mouseUp(window);
    await act(async () => {
      await new Promise((r) => setTimeout(r, 50));
    });

    expect(startDragging).not.toHaveBeenCalled();
    expect(windowSetPosition).not.toHaveBeenCalled();
  });

  it("treats a press and release without movement as the click it is", async () => {
    await renderBar();
    const mic = screen.getByRole("button", { name: "Talk to Juno" });

    fireEvent.mouseDown(mic, { button: 0, clientX: 10, clientY: 10 });
    fireEvent.mouseMove(mic, { clientX: 11, clientY: 10 });
    fireEvent.mouseUp(mic);
    fireEvent.click(mic);
    await act(async () => {});

    expect(startDragging).not.toHaveBeenCalled();
    expect(onMicClick).toHaveBeenCalledTimes(1);
  });

  it("never drags from the text input, so text can be selected", async () => {
    await renderBar();
    const input = screen.getByPlaceholderText("Ask Juno");

    fireEvent.mouseDown(input, { button: 0, clientX: 10, clientY: 10 });
    fireEvent.mouseMove(input, { clientX: 60, clientY: 10 });

    expect(startDragging).not.toHaveBeenCalled();
  });
});

describe("the drop indicator's show payload", () => {
  it("carries the bar's logical size and where inside it the drag was grabbed", async () => {
    // A wide look grabbed near its right-hand end: the offset is what the
    // overlay needs to predict the window's top-left from the cursor.
    outerSize.width = 900;
    outerSize.height = 66;
    await renderBar();
    const root = screen.getByTestId("bar-root");

    fireEvent.mouseDown(root, { button: 0, clientX: 850, clientY: 33 });
    fireEvent.mouseMove(root, { clientX: 880, clientY: 40 });
    await act(async () => {});

    expect(emit).toHaveBeenCalledWith("snap-wells-show", {
      windowWidth: 900,
      windowHeight: 66,
      grabOffsetX: 850,
      grabOffsetY: 33,
    });
  });

  it("hides the indicator on release, whatever the window size", async () => {
    await renderBar();
    const root = screen.getByTestId("bar-root");

    fireEvent.mouseDown(root, { button: 0, clientX: 10, clientY: 10 });
    fireEvent.mouseMove(root, { clientX: 40, clientY: 10 });
    await act(async () => {});
    fireEvent.mouseUp(window);
    await act(async () => {});

    expect(emit).toHaveBeenCalledWith("snap-wells-hide");
  });
});

describe("a look wider than the Pill", () => {
  it("settles by its own measured footprint, with no per-look data", async () => {
    // A Studio-width strip: the wells are inset by 900, not by 164, so the
    // right-hand column is a different place than it is for the Pill.
    outerSize.width = 900;
    outerSize.height = 66;
    await renderBar();
    const root = screen.getByTestId("bar-root");

    fireEvent.mouseDown(root, { button: 0, clientX: 450, clientY: 33 });
    fireEvent.mouseMove(root, { clientX: 500, clientY: 33 });
    outerPos.x = 80;
    outerPos.y = 700;
    fireEvent.mouseUp(window);
    await act(async () => {
      await new Promise((r) => setTimeout(r, 500));
    });

    const last = (windowSetPosition.mock.calls.at(-1) as unknown[])[0] as {
      x: number;
      y: number;
    };
    // 1000×800 display, window 900×66, margin 16, top inset 36:
    // spanX = (1000−16) − 900 − 16 = 68, so the columns are 16 / 50 / 84.
    // (80,700) is nearest the right column and the bottom row: 84 / 718.
    expect(last).toMatchObject({ x: 84, y: 718 });
  });
});
