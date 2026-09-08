import { describe, it, expect } from "vitest";
import type { MonitorRect } from "../snapWells";
import { unionLogicalBounds, wellToOverlayRect } from "../snapWellsOverlay";

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

describe("wellToOverlayRect", () => {
  const monitors = [mon(0, 0, 1920, 1080, 1)];

  it("maps a well straight through at scale 1 with a zero origin", () => {
    const rect = wellToOverlayRect(
      { x: 16, y: 36, monitorIndex: 0 },
      monitors,
      { x: 0, y: 0 },
      { width: 88, height: 66 },
    );
    expect(rect).toEqual({ x: 16, y: 36, w: 88, h: 66 });
  });

  it("divides physical coords + bar size by the well monitor's scale factor", () => {
    const retina = [mon(0, 0, 2560, 1600, 2)];
    const rect = wellToOverlayRect(
      { x: 200, y: 100, monitorIndex: 0 },
      retina,
      { x: 0, y: 0 },
      { width: 88, height: 66 },
    );
    expect(rect).toEqual({ x: 100, y: 50, w: 44, h: 33 });
  });

  it("subtracts the logical union origin so a secondary display lines up", () => {
    // Secondary 1x display sitting at physical x = -1920 (logical -1920).
    const twoMon = [mon(0, 0, 2560, 1600, 2), mon(-1920, 0, 1920, 1080, 1)];
    const origin = { x: -1920, y: 0 };
    const rect = wellToOverlayRect(
      { x: -1904, y: 36, monitorIndex: 1 },
      twoMon,
      origin,
      { width: 88, height: 66 },
    );
    // -1904 logical - (-1920) origin = 16 into the overlay.
    expect(rect).toEqual({ x: 16, y: 36, w: 88, h: 66 });
  });
});
