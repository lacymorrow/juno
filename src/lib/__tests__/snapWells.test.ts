import { describe, expect, it } from "vitest";
import {
  axisStops,
  computeWells,
  nearestWell,
  wellForSlot,
  easeOutCubic,
  SLOT,
  type MonitorRect,
} from "../snapWells";

const oneMonitor: MonitorRect[] = [
  { position: { x: 0, y: 0 }, size: { width: 1000, height: 800 }, scaleFactor: 1 },
];

describe("computeWells", () => {
  it("lays out eight wells per monitor (3×3 minus the centre)", () => {
    const wells = computeWells(oneMonitor, { windowWidth: 100, windowHeight: 60 });
    expect(wells).toHaveLength(8);
    expect(wells.some((w) => w.fx === 0.5 && w.fy === 0.5)).toBe(false);
  });

  it("optionally includes the centre well", () => {
    const wells = computeWells(oneMonitor, {
      windowWidth: 100,
      windowHeight: 60,
      includeCenter: true,
    });
    expect(wells).toHaveLength(9);
    expect(wells.some((w) => w.fx === 0.5 && w.fy === 0.5)).toBe(true);
  });

  it("insets corners by the margin (top-left) and window size (bottom-right)", () => {
    const wells = computeWells(oneMonitor, {
      windowWidth: 100,
      windowHeight: 60,
      margin: 16,
      topInset: 36,
    });
    const tl = wellForSlot(SLOT.topLeft, 0, wells)!;
    const br = wellForSlot(SLOT.bottomRight, 0, wells)!;
    expect(tl).toMatchObject({ x: 16, y: 36 });
    // right edge: 1000 - 16 - 100 = 884 ; bottom edge: 800 - 16 - 60 = 724
    expect(br).toMatchObject({ x: 884, y: 724 });
  });

  it("centres the middle wells within the inset area", () => {
    const wells = computeWells(oneMonitor, { windowWidth: 100, windowHeight: 60 });
    const topCenter = wellForSlot({ fx: 0.5, fy: 0 }, 0, wells)!;
    // left=16, right-w=884 → centre x = (16+884)/2 = 450
    expect(topCenter.x).toBe(450);
    expect(topCenter.y).toBe(36);
  });

  it("scales margins and the window footprint by each monitor's scaleFactor", () => {
    const retina: MonitorRect[] = [
      { position: { x: 0, y: 0 }, size: { width: 2000, height: 1600 }, scaleFactor: 2 },
    ];
    const wells = computeWells(retina, { windowWidth: 100, windowHeight: 60, margin: 16 });
    const tl = wellForSlot(SLOT.topLeft, 0, wells)!;
    expect(tl.x).toBe(32); // 16 logical × 2
    expect(tl).toMatchObject({ width: 200, height: 120 }); // logical × 2
    // right edge: 2000 - 32 - 200 = 1768 ; bottom: 1600 - 32 - 120 = 1448
    expect(wellForSlot(SLOT.bottomRight, 0, wells)).toMatchObject({ x: 1768, y: 1448 });
  });

  it("gives a wide display five stops across and a laptop three", () => {
    expect(axisStops(1440)).toEqual([0, 0.5, 1]);
    expect(axisStops(1920)).toEqual([0, 0.5, 1]);
    expect(axisStops(2560)).toEqual([0, 0.25, 0.5, 0.75, 1]);
    const ultrawide: MonitorRect[] = [
      { position: { x: 0, y: 0 }, size: { width: 2560, height: 1080 }, scaleFactor: 1 },
    ];
    const wells = computeWells(ultrawide, { windowWidth: 100, windowHeight: 60 });
    expect(wells).toHaveLength(5 * 3 - 1);
    // A 2× 27" reports 5120 physical, 2560 logical: five stops too.
    const big: MonitorRect[] = [
      { position: { x: 0, y: 0 }, size: { width: 5120, height: 2880 }, scaleFactor: 2 },
    ];
    expect(computeWells(big, { windowWidth: 100, windowHeight: 60 })).toHaveLength(15 - 1);
    // Portrait: five stops down, three across.
    const portrait: MonitorRect[] = [
      { position: { x: 0, y: 0 }, size: { width: 1080, height: 2560 }, scaleFactor: 1 },
    ];
    expect(computeWells(portrait, { windowWidth: 100, windowHeight: 60 })).toHaveLength(15 - 1);
  });

  it("offsets wells onto a second monitor's coordinate space", () => {
    const two: MonitorRect[] = [
      ...oneMonitor,
      { position: { x: 1000, y: 0 }, size: { width: 800, height: 600 }, scaleFactor: 1 },
    ];
    const wells = computeWells(two, { windowWidth: 100, windowHeight: 60, margin: 16 });
    expect(wells).toHaveLength(16);
    const second = wells.filter((w) => w.monitorIndex === 1);
    expect(second.every((w) => w.x >= 1000 + 16)).toBe(true);
  });

  it("collapses an axis when the window is wider than the inset area", () => {
    const wells = computeWells(oneMonitor, { windowWidth: 5000, windowHeight: 60, margin: 16 });
    // Every column falls back to the left inset when the window can't fit.
    expect(new Set(wells.map((w) => w.x))).toEqual(new Set([16]));
  });
});

describe("nearestWell", () => {
  const wells = computeWells(oneMonitor, { windowWidth: 100, windowHeight: 60 });

  it("returns the closest well to a point", () => {
    const near = nearestWell({ x: 5, y: 5 }, wells)!;
    expect(near).toMatchObject(SLOT.topLeft);
    const nearBR = nearestWell({ x: 999, y: 799 }, wells)!;
    expect(nearBR).toMatchObject(SLOT.bottomRight);
  });

  it("returns null when there are no wells", () => {
    expect(nearestWell({ x: 0, y: 0 }, [])).toBeNull();
  });
});

describe("wellForSlot", () => {
  const laptop: MonitorRect = {
    position: { x: 0, y: 0 },
    size: { width: 1440, height: 900 },
    scaleFactor: 1,
  };
  const ultrawide: MonitorRect = {
    position: { x: 1440, y: 0 },
    size: { width: 2560, height: 1080 },
    scaleFactor: 1,
  };
  const wells = computeWells([laptop, ultrawide], {
    windowWidth: 100,
    windowHeight: 60,
    includeCenter: true,
  });

  it("finds the same corner on another display, with that display's padding", () => {
    const bl = wellForSlot(SLOT.bottomLeft, 1, wells)!;
    expect(bl).toMatchObject({ monitorIndex: 1, fx: 0, fy: 1, x: 1440 + 16, y: 1080 - 16 - 60 });
    const c = wellForSlot(SLOT.center, 1, wells)!;
    expect(c).toMatchObject({ monitorIndex: 1, fx: 0.5, fy: 0.5 });
  });

  it("maps an ultrawide quarter stop onto the laptop's nearest stop, centre on a tie", () => {
    // 0.25 is equidistant from 0 and 0.5 on a three-stop axis: keep it in view.
    expect(wellForSlot({ fx: 0.25, fy: 0 }, 0, wells)).toMatchObject({ fx: 0.5, fy: 0 });
    expect(wellForSlot({ fx: 0.75, fy: 1 }, 0, wells)).toMatchObject({ fx: 0.5, fy: 1 });
    expect(wellForSlot({ fx: 1, fy: 0 }, 0, wells)).toMatchObject({ fx: 1, fy: 0 });
  });

  it("returns null for a monitor that has no wells", () => {
    expect(wellForSlot(SLOT.center, 5, wells)).toBeNull();
  });
});

describe("easeOutCubic", () => {
  it("runs 0→1 and decelerates", () => {
    expect(easeOutCubic(0)).toBe(0);
    expect(easeOutCubic(1)).toBe(1);
    expect(easeOutCubic(0.5)).toBeGreaterThan(0.5); // past halfway by the midpoint
  });
});
