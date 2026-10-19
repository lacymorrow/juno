import { useLayoutEffect, type ReactElement } from "react";
import { act, render, type RenderResult } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { COMMANDS } from "@/lib/constants.generated";
import type { Rect } from "@/lib/steadyFrame";

/**
 * The Island's window is most of the display (`islandMaxFor`: the chat face
 * plus its room). Everything outside the drawn shape is transparent, and the
 * only thing that stops that transparent acreage swallowing every click on
 * the desktop is the hit regions this hook reports. So these tests are about
 * one question: does Rust always learn what is drawn?
 */

const { invoke, monitors, windowPos } = vi.hoisted(() => ({
  invoke: vi.fn((..._args: unknown[]): Promise<unknown> => Promise.resolve(null)),
  monitors: { list: [] as unknown[], throws: false },
  windowPos: { x: 0, y: 0 },
}));

vi.mock("@tauri-apps/api/core", () => ({ invoke: (...args: unknown[]) => invoke(...args) }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async () => () => {}),
  emit: vi.fn(async () => {}),
}));
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    label: "floating-bar",
    outerPosition: async () => ({ x: windowPos.x, y: windowPos.y }),
    outerSize: async () => ({ width: 200, height: 100 }),
    scaleFactor: async () => 1,
    setPosition: vi.fn(async () => {}),
    setSize: vi.fn(async () => {}),
  }),
  availableMonitors: async () => {
    if (monitors.throws) throw new Error("no window server");
    return monitors.list;
  },
}));

import { useIslandFrame } from "../useIslandFrame";
import { IDLE_SHAPE, SHADOW_PAD, islandMaxFor, islandRestFrame } from "../islandModel";
import { resetSteady, steadySwapsSettled, type Size } from "@/lib/steadyFrame";
import { resetBarSnapState } from "@/hooks/useBarSnapWells";

/** The capsule as IslandBar reports it at rest: the shape, plus its shadow room. */
const IDLE_BLOCK = {
  width: IDLE_SHAPE.width + 2 * SHADOW_PAD,
  height: IDLE_SHAPE.height + 2 * SHADOW_PAD,
};

/** IslandBar reports from a layout effect, before the frame is painted. */
function Harness({ hit, rest = null }: { hit: Rect; rest?: Size | null }) {
  const frame = useIslandFrame();
  const { report } = frame;
  useLayoutEffect(() => {
    report(IDLE_BLOCK, hit, rest);
  }, [report, hit, rest]);
  return null;
}

/** Every `set_bar_hit_regions` call, in order. */
const regionCalls = () =>
  invoke.mock.calls
    .filter((c) => c[0] === COMMANDS.BAR_SET_BAR_HIT_REGIONS)
    .map((c) => (c[1] as { regions: Rect[] | null }).regions);

/** Every frame put on the window, in order. */
const frameCalls = () =>
  invoke.mock.calls
    .filter((c) => c[0] === COMMANDS.BAR_SET_BAR_FRAME)
    .map((c) => c[1] as { x: number; y: number; width: number; height: number });

const A_MONITOR = {
  position: { x: 0, y: 0 },
  size: { width: 1440, height: 900 },
  scaleFactor: 1,
};

beforeEach(() => {
  invoke.mockClear();
  resetSteady();
  resetBarSnapState();
  monitors.list = [A_MONITOR];
  monitors.throws = false;
  windowPos.x = 0;
  windowPos.y = 0;
});

afterEach(() => {
  vi.clearAllMocks();
});

describe("useIslandFrame reports what the island draws", () => {
  const capsule: Rect = { x: 100, y: 24, width: IDLE_SHAPE.width, height: IDLE_SHAPE.height };

  it("tells Rust the drawn shape once the window is placed", async () => {
    await act(async () => {
      render(<Harness hit={capsule} />);
    });
    expect(regionCalls().at(-1)).toEqual([capsule]);
  });

  it("still reports when placement fails, so the window is not left swallowing clicks", async () => {
    // No displays answer: `availableMonitors` is empty, which is the launch
    // path's early return. The window is still on screen at its full size.
    monitors.list = [];
    await act(async () => {
      render(<Harness hit={capsule} />);
    });
    expect(regionCalls().at(-1)).toEqual([capsule]);
  });

  it("still reports when the window server throws during placement", async () => {
    monitors.throws = true;
    await act(async () => {
      render(<Harness hit={capsule} />);
    });
    expect(regionCalls().at(-1)).toEqual([capsule]);
  });
});

/**
 * LAC-4832. Rust routes mouse events away from the transparent part of the
 * window, but macOS still lists the window at its full frame, so a
 * display-sized Island is a display-sized rectangle to a screen-share picker
 * and no app behind it can be picked. The window must therefore be the capsule
 * while the capsule is all that is drawn.
 */
describe("useIslandFrame rests in a window the size of the capsule", () => {
  const capsule: Rect = { x: 100, y: 24, width: IDLE_SHAPE.width, height: IDLE_SHAPE.height };
  const rest = islandRestFrame(0);
  const full = islandMaxFor(A_MONITOR);

  it("places the resting frame at launch, never the display-sized one", async () => {
    await act(async () => {
      render(<Harness hit={capsule} rest={rest} />);
    });
    const placed = frameCalls();
    expect(placed.length).toBeGreaterThan(0);
    // Not one frame on the way to resting may be the full window: a blink
    // would be cosmetic, but a display-sized frame on screen is the bug.
    for (const f of placed) {
      expect(f.width).toBe(rest.width);
      expect(f.height).toBe(rest.height);
    }
  });

  it("is a small fraction of the display it rests on", async () => {
    await act(async () => {
      render(<Harness hit={capsule} rest={rest} />);
    });
    const f = frameCalls().at(-1);
    expect(f).toBeDefined();
    const share = ((f?.width ?? 0) * (f?.height ?? 0)) / (A_MONITOR.size.width * A_MONITOR.size.height);
    expect(share).toBeLessThan(0.02);
    // And the regression it guards: the window it would otherwise have been.
    const was = (full.width * full.height) / (A_MONITOR.size.width * A_MONITOR.size.height);
    expect(was).toBeGreaterThan(0.5);
  });

  it("stays inside the full window's frame, so the capsule keeps its place", async () => {
    await act(async () => {
      render(<Harness hit={capsule} rest={rest} />);
    });
    const f = frameCalls().at(-1);
    expect(f).toBeDefined();
    // Top centre of a 1440 x 900 display: the resting window sits at the top
    // of the full window's rect and centred on the same column.
    expect((f?.x ?? 0) + (f?.width ?? 0) / 2).toBeCloseTo(A_MONITOR.size.width / 2, 0);
    expect(f?.y).toBeGreaterThanOrEqual(0);
    expect((f?.y ?? 0) + (f?.height ?? 0)).toBeLessThanOrEqual(A_MONITOR.size.height);
  });

  /** Re-report, then let the hidden two-frame swap finish. */
  const settle = async (view: RenderResult, node: ReactElement) => {
    await act(async () => {
      view.rerender(node);
    });
    await act(async () => {
      await steadySwapsSettled("floating-bar");
    });
  };

  it("grows to the full window when a face needs it, and shrinks back after", async () => {
    const view = await act(async () => render(<Harness hit={capsule} rest={rest} />));
    expect(frameCalls().at(-1)?.width).toBe(rest.width);

    // The island leaves rest: the composer, listening, an answer, the chat face.
    await settle(view, <Harness hit={capsule} rest={null} />);
    expect(frameCalls().at(-1)).toMatchObject({ width: full.width, height: full.height });

    // And back to rest when the turn is over.
    await settle(view, <Harness hit={capsule} rest={rest} />);
    expect(frameCalls().at(-1)).toMatchObject({ width: rest.width, height: rest.height });
  });

  it("keeps reporting hit regions across a frame change", async () => {
    const view = await act(async () => render(<Harness hit={capsule} rest={rest} />));
    await settle(view, <Harness hit={capsule} rest={null} />);
    // Whatever the frame, Rust still knows what is drawn: the transparent
    // acreage of the grown window must not start swallowing clicks.
    expect(regionCalls().at(-1)).toEqual([capsule]);
  });
});
