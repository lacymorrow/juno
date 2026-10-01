import { describe, expect, it, beforeEach } from "vitest";
import {
  computeWells,
  nearestWell,
  SLOT,
  type MonitorRect,
  type Well,
} from "../snapWells";
import {
  distinctWells,
  dockAnchorX,
  dockGrowsUp,
  getDockSlot,
  monitorIndexAt,
  predictedWindowOrigin,
  resetDockSlots,
  setDockSlot,
  subscribeDockSlot,
} from "../barDock";

// One ordinary 1000×800 display at the origin. `computeWells` insets by 16 on
// the left/right/bottom and 36 at the top (to clear the menu bar), and by the
// window's own footprint, so every number below is derived, not chosen.
const display: MonitorRect[] = [
  { position: { x: 0, y: 0 }, size: { width: 1000, height: 800 }, scaleFactor: 1 },
];

/** The Pill-ish narrow look, and a Studio/Bar-ish wide strip. */
const NARROW = { windowWidth: 88, windowHeight: 66 };
const WIDE = { windowWidth: 900, windowHeight: 66 };

const xs = (wells: Well[]) => [...new Set(wells.map((w) => w.x))].sort((a, b) => a - b);
const ys = (wells: Well[]) => [...new Set(wells.map((w) => w.y))].sort((a, b) => a - b);

describe("dockAnchorX / dockGrowsUp", () => {
  it("anchors a left well's left edge, a right well's right edge, and a middle well's centre", () => {
    expect(dockAnchorX(SLOT.topLeft)).toBe("start");
    expect(dockAnchorX(SLOT.bottomRight)).toBe("end");
    expect(dockAnchorX(SLOT.center)).toBe("center");
    expect(dockAnchorX({ fx: 0.25, fy: 0 })).toBe("center");
    expect(dockAnchorX(null)).toBe("center");
  });

  it("grows upward from the bottom half of the display, including the midline", () => {
    expect(dockGrowsUp(SLOT.bottomLeft)).toBe(true);
    expect(dockGrowsUp(SLOT.center)).toBe(true);
    expect(dockGrowsUp(SLOT.topRight)).toBe(false);
    expect(dockGrowsUp(null)).toBe(false);
  });
});

describe("one geometry rule, every width", () => {
  it("gives a narrow look nine distinct wells on a 3×3 display", () => {
    const wells = computeWells(display, { ...NARROW, includeCenter: true });
    expect(wells).toHaveLength(9);
    // spanX = (1000−16) − 88 − 16 = 880 → 16 / 456 / 896.
    expect(xs(wells)).toEqual([16, 456, 896]);
    // spanY = (800−16) − 66 − 36 = 682 → 36 / 377 / 718.
    expect(ys(wells)).toEqual([36, 377, 718]);
    expect(distinctWells(wells)).toHaveLength(9);
  });

  it("converges a wide look's columns without a single branch", () => {
    const wells = computeWells(display, { ...WIDE, includeCenter: true });
    expect(wells).toHaveLength(9);
    // spanX = (1000−16) − 900 − 16 = 68 → 16 / 50 / 84. Still three columns,
    // just 34px apart instead of 440: the same formula, a bigger footprint.
    expect(xs(wells)).toEqual([16, 50, 84]);
    expect(ys(wells)).toEqual([36, 377, 718]);
  });

  it("collapses to one well per landing spot when a look cannot fit at all", () => {
    const wells = computeWells(display, {
      windowWidth: 5000,
      windowHeight: 66,
      includeCenter: true,
    });
    // Every column falls back to the left inset, so nine wells are three
    // places. The overlay must draw three rings, not nine stacked into three.
    expect(xs(wells)).toEqual([16]);
    const distinct = distinctWells(wells);
    expect(distinct).toHaveLength(3);
    expect(ys(distinct)).toEqual([36, 377, 718]);
  });

  it("keeps wells from different displays apart even at the same offset", () => {
    const two: MonitorRect[] = [
      ...display,
      { position: { x: 1000, y: 0 }, size: { width: 1000, height: 800 }, scaleFactor: 1 },
    ];
    const wells = computeWells(two, {
      windowWidth: 5000,
      windowHeight: 66,
      includeCenter: true,
    });
    // Three per display: coincidence is per monitor, never across them.
    expect(distinctWells(wells)).toHaveLength(6);
  });

  it("leaves a list with nothing coincident exactly as it was", () => {
    const wells = computeWells(display, { ...NARROW, includeCenter: true });
    expect(distinctWells(wells)).toEqual(wells);
  });
});

describe("predictedWindowOrigin", () => {
  it("is the cursor less the grab offset", () => {
    expect(predictedWindowOrigin({ x: 880, y: 60 }, { x: 850, y: 33 }, display)).toEqual({
      x: 30,
      y: 27,
    });
  });

  it("scales the grab offset by the display the cursor is on", () => {
    // The offset came from a DOM clientX/clientY, so it is logical px; the
    // cursor is physical. On a 2× display 850 logical is 1700 physical.
    const retina: MonitorRect[] = [
      { position: { x: 0, y: 0 }, size: { width: 2560, height: 1600 }, scaleFactor: 2 },
    ];
    expect(predictedWindowOrigin({ x: 1760, y: 120 }, { x: 850, y: 33 }, retina)).toEqual({
      x: 60,
      y: 54,
    });
  });

  it("falls back to the first display's scale when the cursor is between displays", () => {
    const gap: MonitorRect[] = [
      ...display,
      { position: { x: 1000, y: 0 }, size: { width: 800, height: 600 }, scaleFactor: 2 },
    ];
    // y = 700 is below the second display and right of the first: on neither.
    expect(predictedWindowOrigin({ x: 1200, y: 700 }, { x: 10, y: 10 }, gap)).toEqual({
      x: 1190,
      y: 690,
    });
  });

  it("is the cursor itself with no monitors at all", () => {
    expect(predictedWindowOrigin({ x: 5, y: 7 }, { x: 2, y: 3 }, [])).toEqual({ x: 3, y: 4 });
  });
});

describe("the drop indicator agrees with where a wide look lands", () => {
  // A 900×66 strip on the 1000×800 display, grabbed 850px in from its left
  // edge (near the right-hand end) and dragged so the cursor sits at 880,60.
  const wells = computeWells(display, { ...WIDE, includeCenter: true });
  const grabOffset = { x: 850, y: 33 };
  const cursor = { x: 880, y: 60 };

  /** What the settle does: the window's real top-left against well top-lefts. */
  const landing = (origin: { x: number; y: number }) => nearestWell(origin, wells)!;

  /** What the overlay USED to do: the bare cursor against well centres. */
  const oldHighlight = () => {
    let best: Well | null = null;
    let bestD = Infinity;
    for (const w of wells) {
      const dx = cursor.x - (w.x + w.width / 2);
      const dy = cursor.y - (w.y + w.height / 2);
      const d = dx * dx + dy * dy;
      if (d < bestD) {
        bestD = d;
        best = w;
      }
    }
    return best!;
  };

  it("predicts the well the bar will actually land in", () => {
    const origin = predictedWindowOrigin(cursor, grabOffset, display);
    expect(origin).toEqual({ x: 30, y: 27 });
    const predicted = nearestWell(origin, wells)!;
    // The window's top-left is at 30,27, so the nearest well is the left
    // column's top row: 16,36.
    expect({ x: predicted.x, y: predicted.y }).toEqual({ x: 16, y: 36 });
    expect(predicted).toBe(landing(origin));
  });

  it("fails against the cursor-centre rule it replaces", () => {
    // The old rule brightened the RIGHT column, because the cursor is 850px
    // to the right of the window's own top-left. The bar landed in the left
    // column. Half a window's width of disagreement, a whole column of error.
    const old = oldHighlight();
    expect({ x: old.x, y: old.y }).toEqual({ x: 84, y: 36 });
    const origin = predictedWindowOrigin(cursor, grabOffset, display);
    expect(old).not.toBe(landing(origin));
  });

  it("agrees for a narrow look too, where the old rule happened to be right", () => {
    const narrowWells = computeWells(display, { ...NARROW, includeCenter: true });
    // A Pill grabbed near its middle: the error the old rule carried was at
    // most half of 88px, far less than the 440px between columns.
    const origin = predictedWindowOrigin({ x: 500, y: 70 }, { x: 44, y: 33 }, display);
    const predicted = nearestWell(origin, narrowWells)!;
    expect({ x: predicted.x, y: predicted.y }).toEqual({ x: 456, y: 36 });
  });
});

describe("monitorIndexAt", () => {
  const two = [
    { position: { x: 0, y: 0 }, size: { width: 1000, height: 800 } },
    { position: { x: 1000, y: 0 }, size: { width: 800, height: 600 } },
  ];

  it("finds the display a point is on, exclusive of the far edge", () => {
    expect(monitorIndexAt(two, 0, 0)).toBe(0);
    expect(monitorIndexAt(two, 999, 799)).toBe(0);
    expect(monitorIndexAt(two, 1000, 0)).toBe(1);
  });

  it("is -1 in the gap between two displays of different heights", () => {
    expect(monitorIndexAt(two, 1200, 700)).toBe(-1);
  });
});

describe("the dock slot store", () => {
  beforeEach(() => resetDockSlots());

  it("is null until the window lands somewhere", () => {
    expect(getDockSlot("floating-bar")).toBeNull();
  });

  it("keeps a slot per window label", () => {
    setDockSlot("floating-bar", SLOT.bottomRight);
    expect(getDockSlot("floating-bar")).toEqual(SLOT.bottomRight);
    expect(getDockSlot("floating-panel")).toBeNull();
  });

  it("tells subscribers when the slot changes, and only then", () => {
    let woken = 0;
    const unsubscribe = subscribeDockSlot("floating-bar", () => {
      woken += 1;
    });
    setDockSlot("floating-bar", { fx: 1, fy: 1 });
    expect(woken).toBe(1);
    // The same place again: a re-settle into the well it is already in.
    setDockSlot("floating-bar", { fx: 1, fy: 1 });
    expect(woken).toBe(1);
    setDockSlot("floating-bar", { fx: 0, fy: 0 });
    expect(woken).toBe(2);
    unsubscribe();
    setDockSlot("floating-bar", { fx: 0.5, fy: 0.5 });
    expect(woken).toBe(2);
  });
});
