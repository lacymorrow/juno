import { act, render } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { EVENTS } from "@/lib/constants.generated";
import { FADE_OUT_MS, FOREGROUND_LINGER_MS } from "@/lib/agentCursor";

// Capture every handler the overlay registers, so a test can play the part of
// the Rust backend and emit events at it.
const handlers = new Map<string, (payload: unknown) => void>();
vi.mock("@/hooks/useEventListener", () => ({
  useEventListener: (event: string, handler: (payload: unknown) => void) => {
    handlers.set(event, handler);
  },
}));

import { DesktopCursorOverlay } from "../DesktopCursorOverlay";

function emit(event: string, payload: unknown) {
  const handler = handlers.get(event);
  if (!handler) throw new Error(`overlay does not listen for ${event}`);
  act(() => handler(payload));
}

function cursorEl(container: HTMLElement, id: string): HTMLElement | null {
  return container.querySelector(`[data-agent-cursor="${id}"]`);
}

function translate(el: HTMLElement): [number, number] {
  const m = /translate3d\((-?[\d.]+)px, (-?[\d.]+)px/.exec(el.style.transform);
  if (!m) throw new Error(`no translate in ${el.style.transform}`);
  return [Number(m[1]), Number(m[2])];
}

describe("DesktopCursorOverlay", () => {
  beforeEach(() => {
    handlers.clear();
    vi.useFakeTimers();
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  it("listens for the events Rust sends", () => {
    render(<DesktopCursorOverlay />);
    for (const event of [
      EVENTS.UI_AGENT_CURSOR_UPDATE,
      EVENTS.UI_AGENT_CURSOR_REMOVE,
      EVENTS.UI_AGENT_CURSOR_SHAPE,
    ]) {
      expect(handlers.has(event), event).toBe(true);
    }
  });

  it("an agent action makes the overlay visible at the right place", () => {
    const { container } = render(<DesktopCursorOverlay />);
    expect(cursorEl(container, "s1")).toBeNull();

    // Background action on a display to the left of the primary.
    emit(EVENTS.UI_AGENT_CURSOR_UPDATE, {
      agent_id: "s1",
      x: -1500,
      y: 400,
      state: "clicking",
      color: "#0A84FF",
      foreground: false,
      origin_x: -1920,
      origin_y: 0,
    });

    const el = cursorEl(container, "s1");
    expect(el).not.toBeNull();
    expect(el!.style.opacity).toBe("1");
    expect(el!.dataset.look).toBe("ghost");
    // Ghost arrow hotspot is (3, 2): the tip sits on (-1500 + 1920, 400).
    expect(translate(el!)).toEqual([420 - 3, 400 - 2]);
    // A ghost arrow is drawn, tinted glow behind it in the chosen color.
    expect(el!.querySelector("img")).not.toBeNull();
    const tint = el!.querySelector<HTMLElement>(".juno-glow-tint");
    expect(tint?.style.backgroundColor).toBe("rgb(10, 132, 255)");
    // The click pulses.
    expect(el!.querySelector(".juno-glow--pulse")).not.toBeNull();
  });

  it("glows behind the real cursor in its own shape when Juno moves it", () => {
    const { container } = render(<DesktopCursorOverlay />);
    emit(EVENTS.UI_AGENT_CURSOR_SHAPE, {
      image: "data:image/png;base64,AA==",
      hotspot_x: 4,
      hotspot_y: 9,
      width: 9,
      height: 18,
    });
    emit(EVENTS.UI_AGENT_CURSOR_UPDATE, {
      agent_id: "s1",
      x: 200,
      y: 100,
      state: "moving",
      color: "#30D158",
      foreground: true,
      origin_x: 0,
      origin_y: 0,
    });

    const el = cursorEl(container, "s1")!;
    expect(el.dataset.look).toBe("glow");
    // No second arrow over the real one.
    expect(el.querySelector("img")).toBeNull();
    expect(translate(el)).toEqual([200 - 4, 100 - 9]);
    expect(el.style.width).toBe("9px");
    // Same frame as the move: no glide on the glow.
    expect(el.style.transition).not.toContain("transform");
    const tint = el.querySelector<HTMLElement>(".juno-glow-tint")!;
    expect(tint.style.maskImage || tint.style.webkitMaskImage).toContain("data:image/png");

    // The person takes their cursor back; the glow does not stay behind.
    act(() => {
      vi.advanceTimersByTime(FOREGROUND_LINGER_MS + 1);
    });
    expect(cursorEl(container, "s1")!.style.opacity).toBe("0");
  });

  it("keeps the ghost until Juno lets go, then fades it out", () => {
    const { container } = render(<DesktopCursorOverlay />);
    emit(EVENTS.UI_AGENT_CURSOR_UPDATE, {
      agent_id: "s1",
      x: 10,
      y: 10,
      state: "idle",
      color: "#0A84FF",
      foreground: false,
      origin_x: 0,
      origin_y: 0,
    });
    act(() => {
      vi.advanceTimersByTime(FOREGROUND_LINGER_MS * 4);
    });
    expect(cursorEl(container, "s1")!.style.opacity).toBe("1");

    emit(EVENTS.UI_AGENT_CURSOR_REMOVE, { agent_id: "s1" });
    expect(cursorEl(container, "s1")!.style.opacity).toBe("0");
    act(() => {
      vi.advanceTimersByTime(FADE_OUT_MS + 1);
    });
    expect(cursorEl(container, "s1")).toBeNull();
  });
});
