import { describe, expect, it } from "vitest";
import { boxInCanvasPx } from "../FlameWrap";

const rect = (left: number, top: number, width: number, height: number) => ({
  left,
  top,
  width,
  height,
  right: left + width,
  bottom: top + height,
});

describe("boxInCanvasPx", () => {
  it("measures the box in canvas pixels when nothing is scaled", () => {
    // Canvas 400x40 at (100, 50); box fills it.
    expect(boxInCanvasPx(rect(100, 50, 400, 40), rect(100, 50, 400, 40), 400, 40)).toEqual({
      cx: 200,
      cy: 20,
      hx: 200,
      hy: 20,
    });
  });

  it("gives the same answer under an ancestor scale (the appearance picker)", () => {
    // The same layout drawn at scale(0.5) with the origin moved: on screen
    // everything is half size, but the canvas is still 400x40 in layout px.
    expect(boxInCanvasPx(rect(30, 10, 200, 20), rect(30, 10, 200, 20), 400, 40)).toEqual({
      cx: 200,
      cy: 20,
      hx: 200,
      hy: 20,
    });
  });

  it("keeps an inset box centred under scale", () => {
    // Canvas 400x80 layout, box 200x40 centred; drawn at scale(0.5).
    const out = rect(0, 0, 200, 40);
    const box = rect(50, 10, 100, 20);
    expect(boxInCanvasPx(out, box, 400, 80)).toEqual({ cx: 200, cy: 40, hx: 100, hy: 20 });
  });

  it("returns null before layout", () => {
    expect(boxInCanvasPx(rect(0, 0, 0, 0), rect(0, 0, 10, 10), 0, 0)).toBeNull();
  });
});
