import { describe, expect, it } from "vitest";
import { ISLAND_GLOW } from "../IslandShell";
import { SHADOW_PAD } from "../islandModel";

/** How far each box-shadow layer paints past the shape, per side. */
function reach(shadow: string) {
  return shadow.split(/,(?![^(]*\))/).map((layer) => {
    const [x = 0, y = 0, blur = 0, spread = 0] = (layer.match(/-?[\d.]+px|\b0\b(?![.\d])/g) ?? [])
      .map((n) => parseFloat(n));
    return {
      top: blur + spread - y,
      bottom: blur + spread + y,
      left: blur + spread - x,
      right: blur + spread + x,
    };
  });
}

describe("island shadow", () => {
  it("ends inside the window padding on every side, so the window edge never clips it", () => {
    for (const r of reach(ISLAND_GLOW)) {
      expect(Math.max(r.top, r.bottom, r.left, r.right)).toBeLessThanOrEqual(SHADOW_PAD);
    }
  });
});
