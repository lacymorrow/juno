import { beforeEach, describe, expect, it, vi } from "vitest";

// One 1440x900 display at 1x. The bar window is 300x200 logical.
const win = {
  label: "floating-bar",
  position: { x: 700, y: 400 },
  outerPosition: vi.fn(async () => ({ ...win.position })),
  outerSize: vi.fn(async () => ({ width: 300, height: 200 })),
  scaleFactor: vi.fn(async () => 1),
  setPosition: vi.fn(async () => {}),
  startDragging: vi.fn(async () => {}),
};
let cursor: { x: number; y: number } | null = { x: 0, y: 0 };

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => win,
  availableMonitors: async () => [
    { position: { x: 0, y: 0 }, size: { width: 1440, height: 900 }, scaleFactor: 1 },
  ],
  cursorPosition: async () => {
    if (!cursor) throw new Error("no cursor");
    return cursor;
  },
  LogicalPosition: class {
    constructor(
      public x: number,
      public y: number,
    ) {}
  },
  PhysicalPosition: class {
    constructor(
      public x: number,
      public y: number,
    ) {}
  },
}));
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(async () => {}) }));
vi.mock("@tauri-apps/api/event", () => ({ emit: vi.fn(async () => {}), listen: vi.fn() }));

import { armBarSnap, resetBarSnapState, settleBarSnap } from "../useBarSnapWells";
import { getDockSlot } from "@/lib/barDock";

describe("settleBarSnap", () => {
  beforeEach(() => {
    resetBarSnapState();
    win.setPosition.mockClear();
  });

  it("picks the well from the cursor, not from where the OS held the window", async () => {
    // Grabbed 20px into the window and released with the cursor at the top of
    // the screen. macOS keeps a window's top edge below the menu bar, so a
    // tall steady window is stopped far lower; here it sits mid-screen.
    await armBarSnap({ x: 150, y: 20 });
    win.position = { x: 570, y: 330 };
    cursor = { x: 720, y: 22 };
    await settleBarSnap();
    expect(getDockSlot("floating-bar")?.fy).toBe(0);
  });

  it("falls back to the window position when the cursor cannot be read", async () => {
    await armBarSnap({ x: 150, y: 20 });
    win.position = { x: 570, y: 680 };
    cursor = null;
    await settleBarSnap();
    expect(getDockSlot("floating-bar")?.fy).toBe(1);
  });
});

import { invoke } from "@tauri-apps/api/core";
import { displayFollowTarget, startBarDrag, steadyWells } from "../useBarSnapWells";
import {
  anchorScreenOrigin,
  contentRect,
  dragLayout,
  getSteady,
  registerSteady,
  resetSteady,
  setSteadyFootprint,
  setSteadyLayout,
  steadyLayout,
  type SteadyLayout,
} from "@/lib/steadyFrame";
import { monitorsInPoints } from "@/lib/desktopPoints";
import { wellForSlot } from "@/lib/snapWells";

const SPEC = { rest: { width: 88, height: 76 }, max: { width: 452, height: 574 }, stage: { width: 452, height: 76 } };
const ONE = [{ position: { x: 0, y: 0 }, size: { width: 1440, height: 900 }, scaleFactor: 1 }];
const tick = (ms = 120) => new Promise((r) => setTimeout(r, ms));

/** The frames Rust was asked to apply, in order, as plain rects. */
function framesApplied() {
  return vi
    .mocked(invoke)
    .mock.calls.filter((c) => c[0] === "set_bar_frame")
    .map((c) => c[1] as { x: number; y: number; width: number; height: number });
}

describe("dragging the shape, not the stage", () => {
  let docked: SteadyLayout;

  beforeEach(() => {
    resetBarSnapState();
    resetSteady();
    vi.mocked(invoke).mockReset();
    vi.mocked(invoke).mockImplementation(async () => {});
    win.startDragging.mockClear();
    const wells = steadyWells(ONE, SPEC);
    docked = steadyLayout(wellForSlot({ fx: 0.5, fy: 1 }, 0, wells)!, ONE[0], SPEC);
    registerSteady("floating-bar", SPEC);
    setSteadyLayout("floating-bar", docked);
  });

  it("shrinks the window to the footprint where it is, then hands it to the OS drag once", async () => {
    // The hovered pill is wider than the resting one.
    const footprint = { width: 164, height: 76 };
    setSteadyFootprint("floating-bar", footprint);
    const order: string[] = [];
    vi.mocked(invoke).mockImplementation(async (cmd) => {
      if (cmd === "set_bar_frame") order.push("frame");
    });
    win.startDragging.mockImplementation(async () => {
      order.push("drag");
    });
    startBarDrag({ x: docked.anchor.x + 44, y: docked.anchor.y + 38 });
    await tick();
    const drag = dragLayout(docked, footprint);
    // Placed by the cursor: the spot pressed, re-measured from the shape.
    expect(framesApplied()).toEqual([
      {
        x: drag.origin.x,
        y: drag.origin.y,
        width: 164,
        height: 76,
        grabX: docked.anchor.x + 44 - (drag.origin.x - docked.origin.x),
        grabY: docked.anchor.y + 38 - (drag.origin.y - docked.origin.y),
      },
    ]);
    expect(win.startDragging).toHaveBeenCalledTimes(1);
    expect(order).toEqual(["frame", "drag"]);
    expect(getSteady("floating-bar")!.layout).toEqual(drag);
    // The pill has not moved on screen.
    expect(anchorScreenOrigin(drag)).toEqual(anchorScreenOrigin(docked));
    // No Rust-driven drag is involved any more.
    const commands = vi.mocked(invoke).mock.calls.map((c) => c[0]);
    expect(commands.some((c) => String(c).startsWith("bar_drag"))).toBe(false);
  });

  it("does not start an OS drag once the button is already up (a flick)", async () => {
    startBarDrag({ x: docked.anchor.x + 44, y: docked.anchor.y + 38 });
    cursor = null;
    win.position = { ...docked.origin };
    await settleBarSnap();
    expect(win.startDragging).not.toHaveBeenCalled();
    // And the settle put the steady window back.
    expect(getSteady("floating-bar")!.layout!.size).toEqual(docked.size);
  });

  it("lands a pill docked low in the top row with the pill on the well, then grows back", async () => {
    startBarDrag({ x: docked.anchor.x + 44, y: docked.anchor.y + 38 });
    await tick();
    const drag = getSteady("floating-bar")!.layout!;
    expect(drag.size).toEqual(SPEC.rest);
    // The OS carried the pill-sized window to the top: its top is just under
    // the menu bar, the pill with it.
    cursor = { x: 720, y: 40 + 38 };
    win.position = { x: 720 - 44 - drag.anchor.x, y: 40 - drag.anchor.y };
    await settleBarSnap();
    expect(getDockSlot("floating-bar")).toEqual({ fx: 0.5, fy: 0 });
    const top = wellForSlot({ fx: 0.5, fy: 0 }, 0, steadyWells(ONE, SPEC))!;
    const landed = getSteady("floating-bar")!.layout!;
    expect(anchorScreenOrigin(landed)).toEqual({ x: top.x, y: top.y });
    expect(landed).toEqual(steadyLayout(top, ONE[0], SPEC));
    // The last frame applied is the steady one for the top well.
    const last = framesApplied().at(-1)!;
    expect(last).toEqual({ ...landed.origin, ...landed.size });
  });

  it("settles a drag whose mouseup never reached the page, once Rust says the button is up", async () => {
    let held = true;
    vi.mocked(invoke).mockImplementation(async (cmd) => {
      if (cmd === "bar_pointer_held") return held;
    });
    startBarDrag({ x: docked.anchor.x + 44, y: docked.anchor.y + 38 });
    await tick(200);
    expect(getDockSlot("floating-bar")).not.toEqual({ fx: 1, fy: 0.5 });
    const drag = getSteady("floating-bar")!.layout!;
    cursor = { x: 1400, y: 450 };
    win.position = { x: 1400 - 44 - drag.anchor.x, y: 450 - 38 - drag.anchor.y };
    held = false;
    await tick(600);
    expect(getDockSlot("floating-bar")).toEqual({ fx: 1, fy: 0.5 });
  });
});

describe("a fast drag keeps the spot pressed under the cursor", () => {
  // What Rust does with a grab: read the cursor at the moment the frame is
  // set and put the grab under it. The cursor here is in points (one 1x
  // display), and it is read when the command runs, not when it was asked for.
  function emulateRustPlacement(frames: { x: number; y: number }[]) {
    vi.mocked(invoke).mockImplementation(async (cmd, args) => {
      if (cmd !== "set_bar_frame") return undefined;
      const a = args as { x: number; y: number; grabX?: number; grabY?: number };
      const placed =
        a.grabX !== undefined && a.grabY !== undefined && cursor
          ? { x: cursor.x - a.grabX, y: cursor.y - a.grabY }
          : { x: a.x, y: a.y };
      frames.push(placed);
      return placed;
    });
  }

  beforeEach(() => {
    resetBarSnapState();
    resetSteady();
    vi.mocked(invoke).mockReset();
    win.startDragging.mockReset();
    win.startDragging.mockImplementation(async () => {});
  });

  const SLOTS = [
    { fx: 0, fy: 0 },
    { fx: 0.5, fy: 0 },
    { fx: 1, fy: 0 },
    { fx: 0, fy: 0.5 },
    { fx: 1, fy: 0.5 },
    { fx: 0, fy: 1 },
    { fx: 0.5, fy: 1 },
    { fx: 1, fy: 1 },
  ];
  // Resting, hovered, listening and the full pill.
  const FOOTPRINTS = [
    { width: 88, height: 76 },
    { width: 164, height: 76 },
    { width: 252, height: 66 },
    { width: 451, height: 76 },
  ];

  for (const slot of SLOTS) {
    for (const fp of FOOTPRINTS) {
      it(`well (${slot.fx},${slot.fy}), footprint ${fp.width}x${fp.height}: flicked 90pt before the drag window lands`, async () => {
        const docked = steadyLayout(wellForSlot(slot, 0, steadyWells(ONE, SPEC))!, ONE[0], SPEC);
        registerSteady("floating-bar", SPEC);
        setSteadyLayout("floating-bar", docked);
        setSteadyFootprint("floating-bar", fp);
        const frames: { x: number; y: number }[] = [];
        emulateRustPlacement(frames);
        const order: string[] = [];
        win.startDragging.mockImplementation(async () => {
          order.push(`drag after ${frames.length} frame(s)`);
        });

        // Pressed 10pt into the shape from its top-left.
        const drawn = contentRect(docked, fp);
        const press = { x: drawn.x + 10, y: drawn.y + 10 };
        const pressedOnScreen = { x: docked.origin.x + press.x, y: docked.origin.y + press.y };
        // The threshold is crossed 4pt later...
        cursor = { x: pressedOnScreen.x + 4, y: pressedOnScreen.y };
        startBarDrag(press);
        // ...and the hand keeps going while the window swaps behind its
        // hidden frames.
        cursor = { x: pressedOnScreen.x + 90, y: pressedOnScreen.y - 40 };
        // Until the OS drag starts, not a fixed delay: the swap waits on
        // animation frames, which a loaded runner can stretch past 120ms.
        await vi.waitFor(() => expect(order).toHaveLength(1), { timeout: 5000 });

        expect(frames).toHaveLength(1);
        expect(order).toEqual(["drag after 1 frame(s)"]);
        const drag = getSteady("floating-bar")!.layout!;
        // The window is where Rust put it, the spot pressed under the cursor
        // as the OS drag takes over: no offset to carry for the whole drag.
        expect(drag.origin).toEqual(frames[0]);
        const grabbed = { x: drag.origin.x + (press.x - drawn.x), y: drag.origin.y + (press.y - drawn.y) };
        expect(grabbed).toEqual(cursor);
        expect(drag.size).toEqual(fp);
        // The resting footprint inside it is where the drop indicator and the
        // settle predict it from the cursor, so the ring and the landing agree.
        expect(anchorScreenOrigin(drag)).toEqual({
          x: cursor!.x - (press.x - docked.anchor.x),
          y: cursor!.y - (press.y - docked.anchor.y),
        });
      });
    }
  }

  it("a slow start (no travel) does not move the shape at all", async () => {
    const docked = steadyLayout(wellForSlot({ fx: 1, fy: 1 }, 0, steadyWells(ONE, SPEC))!, ONE[0], SPEC);
    registerSteady("floating-bar", SPEC);
    setSteadyLayout("floating-bar", docked);
    const fp = { width: 164, height: 76 };
    setSteadyFootprint("floating-bar", fp);
    const frames: { x: number; y: number }[] = [];
    emulateRustPlacement(frames);
    const drawn = contentRect(docked, fp);
    const press = { x: drawn.x + 30, y: drawn.y + 20 };
    cursor = { x: docked.origin.x + press.x, y: docked.origin.y + press.y };
    startBarDrag(press);
    await vi.waitFor(() => expect(win.startDragging).toHaveBeenCalled(), { timeout: 5000 });
    expect(frames).toEqual([dragLayout(docked, fp).origin]);
  });

  it("a look that is not steady is put back under the cursor before the OS drag", async () => {
    // 300x200 window at (700, 400), pressed at (150, 20) inside it; the
    // cursor is 60pt further on when the frame is set.
    win.position = { x: 700, y: 400 };
    const frames: { x: number; y: number }[] = [];
    emulateRustPlacement(frames);
    const order: string[] = [];
    win.startDragging.mockImplementation(async () => {
      order.push(`drag after ${frames.length} frame(s)`);
    });
    cursor = { x: 850 + 60, y: 420 };
    startBarDrag({ x: 150, y: 20 });
    await tick();
    const call = framesApplied()[0] as Record<string, number>;
    expect(call).toMatchObject({ width: 300, height: 200, grabX: 150, grabY: 20 });
    expect(frames).toEqual([{ x: 760, y: 400 }]);
    expect(order).toEqual(["drag after 1 frame(s)"]);
  });
});

describe("display follow on mixed desks", () => {
  // A 2x laptop with a 1x display right of, left of and above it, as Tauri
  // reports them; the cursor arrives from Rust in points.
  const laptop = { position: { x: 0, y: 0 }, size: { width: 3024, height: 1964 }, scaleFactor: 2 };
  const externals = {
    right: { position: { x: 1512, y: 0 }, size: { width: 1920, height: 1080 }, scaleFactor: 1 },
    left: { position: { x: -1920, y: 0 }, size: { width: 1920, height: 1080 }, scaleFactor: 1 },
    above: { position: { x: 0, y: -1080 }, size: { width: 1920, height: 1080 }, scaleFactor: 1 },
  };

  for (const [where, ext] of Object.entries(externals)) {
    it(`re-homes to the same slot on the display ${where}`, () => {
      const rects = monitorsInPoints([laptop, ext]);
      const wells = steadyWells(rects, SPEC);
      const cursor = {
        x: rects[1].position.x + rects[1].size.width / 2,
        y: rects[1].position.y + rects[1].size.height / 2,
      };
      const target = displayFollowTarget(rects, { fx: 1, fy: 0 }, cursor, 0, wells)!;
      expect(target.monitorIndex).toBe(1);
      expect(target).toMatchObject({ fx: 1, fy: 0 });
      // Top-right of the external, with its own padding, in points.
      expect(target.x).toBe(rects[1].position.x + rects[1].size.width - 16 - 88);
      expect(target.y).toBe(rects[1].position.y + 36);
      // And its steady window stays on that display.
      const layout = steadyLayout(target, rects[1], SPEC);
      expect(layout.origin.x).toBeGreaterThanOrEqual(rects[1].position.x);
    });
  }

  it("stays put when the cursor is on the bar's own display", () => {
    const rects = monitorsInPoints([laptop, externals.right]);
    expect(displayFollowTarget(rects, { fx: 1, fy: 0 }, { x: 100, y: 100 }, 0, steadyWells(rects, SPEC))).toBeNull();
  });
});
