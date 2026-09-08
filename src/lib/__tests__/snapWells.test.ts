import { describe, expect, it } from "vitest";
import {
  computeWells,
  nearestWell,
  easeOutCubic,
  type MonitorRect,
} from "../snapWells";

const oneMonitor: MonitorRect[] = [
  { position: { x: 0, y: 0 }, size: { width: 1000, height: 800 }, scaleFactor: 1 },
];

describe("computeWells", () => {
  it("lays out eight wells per monitor (3×3 minus the centre)", () => {
    const wells = computeWells(oneMonitor, { windowWidth: 100, windowHeight: 60 });
    expect(wells).toHaveLength(8);
    expect(wells.some((w) => w.col === "center" && w.row === "middle")).toBe(false);
  });

  it("optionally includes the centre well", () => {
    const wells = computeWells(oneMonitor, {
      windowWidth: 100,
      windowHeight: 60,
      includeCenter: true,
    });
    expect(wells).toHaveLength(9);
    expect(wells.some((w) => w.col === "center" && w.row === "middle")).toBe(true);
  });

  it("insets corners by the margin (top-left) and window size (bottom-right)", () => {
    const wells = computeWells(oneMonitor, {
      windowWidth: 100,
      windowHeight: 60,
      margin: 16,
      topInset: 36,
    });
    const tl = wells.find((w) => w.col === "left" && w.row === "top")!;
    const br = wells.find((w) => w.col === "right" && w.row === "bottom")!;
    expect(tl).toMatchObject({ x: 16, y: 36 });
    // right edge: 1000 - 16 - 100 = 884 ; bottom edge: 800 - 16 - 60 = 724
    expect(br).toMatchObject({ x: 884, y: 724 });
  });

  it("centres the middle wells within the inset area", () => {
    const wells = computeWells(oneMonitor, { windowWidth: 100, windowHeight: 60 });
    const topCenter = wells.find((w) => w.col === "center" && w.row === "top")!;
    // left=16, right-w=884 → centre x = (16+884)/2 = 450
    expect(topCenter.x).toBe(450);
    expect(topCenter.y).toBe(36);
  });

  it("scales margins by each monitor's scaleFactor", () => {
    const retina: MonitorRect[] = [
      { position: { x: 0, y: 0 }, size: { width: 2000, height: 1600 }, scaleFactor: 2 },
    ];
    const wells = computeWells(retina, { windowWidth: 200, windowHeight: 120, margin: 16 });
    const tl = wells.find((w) => w.col === "left" && w.row === "top")!;
    expect(tl.x).toBe(32); // 16 logical × 2
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
    expect(near).toMatchObject({ col: "left", row: "top" });
    const nearBR = nearestWell({ x: 999, y: 799 }, wells)!;
    expect(nearBR).toMatchObject({ col: "right", row: "bottom" });
  });

  it("returns null when there are no wells", () => {
    expect(nearestWell({ x: 0, y: 0 }, [])).toBeNull();
  });
});

describe("easeOutCubic", () => {
  it("runs 0→1 and decelerates", () => {
    expect(easeOutCubic(0)).toBe(0);
    expect(easeOutCubic(1)).toBe(1);
    expect(easeOutCubic(0.5)).toBeGreaterThan(0.5); // past halfway by the midpoint
  });
});
