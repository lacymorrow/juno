import { describe, it, expect } from "vitest";
import type { MonitorRect } from "../snapWells";
import { computeWells } from "../snapWells";
import { monitorsInPoints } from "../desktopPoints";
import {
  overlayDisplayIndex,
  unionLogicalBounds,
  wellToOverlayRect,
  wellsForDisplay,
} from "../snapWellsOverlay";

const mon = (
  x: number,
  y: number,
  w: number,
  h: number,
  scaleFactor = 1,
): MonitorRect => ({ position: { x, y }, size: { width: w, height: h }, scaleFactor });

describe("unionLogicalBounds", () => {
  it("returns the monitor's logical rect for a single 1x display", () => {
    expect(unionLogicalBounds([mon(0, 0, 1920, 1080)])).toEqual({
      originX: 0,
      originY: 0,
      width: 1920,
      height: 1080,
    });
  });

  it("divides physical geometry by scale factor for a retina display", () => {
    // 2560x1600 physical at 2x → 1280x800 logical
    expect(unionLogicalBounds([mon(0, 0, 2560, 1600, 2)])).toEqual({
      originX: 0,
      originY: 0,
      width: 1280,
      height: 800,
    });
  });

  it("unions multiple monitors in logical space, including negative origins", () => {
    // Primary retina at origin, secondary 1x to its left.
    const b = unionLogicalBounds([
      mon(0, 0, 2560, 1600, 2), // logical 0..1280 x 0..800
      mon(-1920, 0, 1920, 1080, 1), // logical -1920..0 x 0..1080
    ]);
    expect(b.originX).toBe(-1920);
    expect(b.originY).toBe(0);
    expect(b.width).toBe(1920 + 1280);
    expect(b.height).toBe(1080);
  });

  it("is safe when given no monitors", () => {
    expect(unionLogicalBounds([])).toEqual({
      originX: 0,
      originY: 0,
      width: 0,
      height: 0,
    });
  });
});

describe("one overlay per display", () => {
  // A 2x laptop (1512x982 points) with a 1x 1920x1080 display to its left,
  // as Tauri reports them, converted to points the way the overlay does.
  const desk = monitorsInPoints([
    mon(0, 0, 3024, 1964, 2),
    mon(-1920, -100, 1920, 1080, 1),
  ]);
  const wells = computeWells(desk, { windowWidth: 88, windowHeight: 76, includeCenter: true });

  it("reads its display from its window label", () => {
    expect(overlayDisplayIndex("snap-wells-overlay")).toBe(0);
    expect(overlayDisplayIndex("snap-wells-overlay-1")).toBe(1);
    expect(overlayDisplayIndex("snap-wells-overlay-x")).toBe(-1);
    expect(overlayDisplayIndex("floating-bar")).toBe(-1);
  });

  it("draws only its own display's wells, all inside its own window", () => {
    for (const idx of [0, 1]) {
      const display = desk[idx];
      const own = wellsForDisplay(wells, idx);
      expect(own).toHaveLength(9);
      for (const w of own) {
        const r = wellToOverlayRect(w, display);
        expect(r.x).toBeGreaterThanOrEqual(0);
        expect(r.y).toBeGreaterThanOrEqual(0);
        expect(r.x + r.w).toBeLessThanOrEqual(display.size.width);
        expect(r.y + r.h).toBeLessThanOrEqual(display.size.height);
      }
    }
  });

  it("puts a well on the left display 16 points in from that display's edge", () => {
    const tl = wellsForDisplay(wells, 1).find((w) => w.fx === 0 && w.fy === 0)!;
    expect(wellToOverlayRect(tl, desk[1])).toEqual({ x: 16, y: 36, w: 88, h: 76 });
  });
});
