import { beforeEach, describe, expect, it, vi } from "vitest";

// One 1440x900 display at 1x. The bar window is 300x200 logical.
const win = {
  label: "floating-bar",
  position: { x: 700, y: 400 },
  outerPosition: vi.fn(async () => ({ ...win.position })),
  outerSize: vi.fn(async () => ({ width: 300, height: 200 })),
  scaleFactor: vi.fn(async () => 1),
  setPosition: vi.fn(async () => {}),
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
  dragLayout,
  getSteady,
  registerSteady,
  resetSteady,
  setSteadyLayout,
  steadyLayout,
} from "@/lib/steadyFrame";
import { monitorsInPoints } from "@/lib/desktopPoints";
import { wellForSlot } from "@/lib/snapWells";

const SPEC = { rest: { width: 88, height: 76 }, max: { width: 452, height: 574 }, stage: { width: 452, height: 76 } };
const ONE = [{ position: { x: 0, y: 0 }, size: { width: 1440, height: 900 }, scaleFactor: 1 }];

describe("the driven drag of a steady look", () => {
  beforeEach(() => {
    resetBarSnapState();
    resetSteady();
    vi.mocked(invoke).mockClear();
  });

  it("hands the drag to Rust, re-laid out with the shape centred", async () => {
    const wells = steadyWells(ONE, SPEC);
    const bottom = wellForSlot({ fx: 0.5, fy: 1 }, 0, wells)!;
    const docked = steadyLayout(bottom, ONE[0], SPEC);
    registerSteady("floating-bar", SPEC);
    setSteadyLayout("floating-bar", docked);
    // Pressed in the middle of the resting pill, near the bottom of its window.
    const grab = { x: docked.anchor.x + 44, y: docked.anchor.y + 38 };
    startBarDrag(grab);
    await new Promise((r) => setTimeout(r, 120));
    const follows = vi
      .mocked(invoke)
      .mock.calls.filter((c) => c[0] === "bar_drag_follow")
      .map((c) => c[1]);
    expect(follows[0]).toEqual({ grabX: grab.x, grabY: grab.y });
    const drag = getSteady("floating-bar")!.layout!;
    expect(drag).toEqual(dragLayout(docked));
    // The re-grab keeps the same point of the pill under the cursor.
    expect(follows[1]).toEqual({ grabX: drag.anchor.x + 44, grabY: drag.anchor.y + 38 });
    // And the pill has not moved on screen in the re-layout.
    expect(anchorScreenOrigin(drag)).toEqual(anchorScreenOrigin(docked));
  });

  it("lands a pill docked low in the top row when carried there", async () => {
    const wells = steadyWells(ONE, SPEC);
    const bottom = wellForSlot({ fx: 0.5, fy: 1 }, 0, wells)!;
    const docked = steadyLayout(bottom, ONE[0], SPEC);
    registerSteady("floating-bar", SPEC);
    setSteadyLayout("floating-bar", dragLayout(docked));
    await armBarSnap({ x: docked.anchor.x + 44, y: docked.anchor.y + 38 });
    // Rust carried the window above the top of the screen: its top is well
    // above y = 0 while the pill is just under the menu bar.
    const drag = getSteady("floating-bar")!.layout!;
    cursor = { x: 720, y: 60 };
    win.position = { x: 720 - drag.anchor.x - 44, y: 60 - drag.anchor.y - 38 };
    expect(win.position.y).toBeLessThan(0);
    await settleBarSnap();
    expect(getDockSlot("floating-bar")).toEqual({ fx: 0.5, fy: 0 });
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
