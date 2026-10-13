import { describe, expect, it } from "vitest";
import {
  CURL_REST,
  FALLBACK_MENU_BAR,
  NOTCH_REST_WIDTH,
  EDGE_REST_DEPTH,
  SIDE_REST_LENGTH,
  curlCornerFor,
  menuBarHeight,
  nearestNotchWell,
  notchEdgeFor,
  notchFormFor,
  notchWells,
} from "../notchWells";
import type { MonitorRect } from "../snapWells";
import { contentRect, dockedEdges, steadyLayout } from "../steadyFrame";
import { glideTarget } from "@/hooks/useBarSnapWells";
import { NOTCH_STEADY_SPEC, notchShape, curlUnderClip, FOLD, SHOULDER } from "@/components/bar/notch/notchModel";
import { ISLAND_SIZES } from "@/components/bar/island/islandModel";

/** A 14" MacBook Pro: 1512x982 points, a 38 point menu bar under the notch. */
const LAPTOP: MonitorRect = {
  position: { x: 0, y: 0 },
  size: { width: 1512, height: 982 },
  workArea: { position: { x: 0, y: 38 }, size: { width: 1512, height: 944 } },
};

/** A second display to the right, 24 point menu bar, no notch. */
const EXTERNAL: MonitorRect = {
  position: { x: 1512, y: -200 },
  size: { width: 2560, height: 1440 },
  workArea: { position: { x: 1512, y: -176 }, size: { width: 2560, height: 1416 } },
};

describe("the Notch's wells", () => {
  const wells = notchWells([LAPTOP, EXTERNAL]);

  it("are every stop on the display's edges: the corners and the edge middles", () => {
    const laptop = wells
      .filter((w) => w.monitorIndex === 0)
      .map((w) => `${w.fx},${w.fy}`)
      .sort();
    expect(laptop).toEqual(["0,0", "0,0.5", "0,1", "0.5,0", "0.5,1", "1,0", "1,0.5", "1,1"]);
    // A wide display has five stops along its long edges, still none inside.
    const wide = wells.filter((w) => w.monitorIndex === 1);
    expect(wide).toHaveLength(12);
    expect(wide.some((w) => w.fx === 0.25 && w.fy === 0)).toBe(true);
    expect(wide.some((w) => w.fx === 0.25 && w.fy === 0.5)).toBe(false);
  });

  it("never offers the centre of the screen", () => {
    for (const w of wells) {
      const inside = w.fx > 0 && w.fx < 1 && w.fy > 0 && w.fy < 1;
      expect(inside, `${w.fx},${w.fy}`).toBe(false);
    }
  });

  it("puts a notch on each edge middle, sized for that edge", () => {
    const at = (fx: number, fy: number) =>
      wells.find((w) => w.monitorIndex === 0 && w.fx === fx && w.fy === fy)!;
    expect(at(0.5, 1)).toMatchObject({ y: 982 - EDGE_REST_DEPTH, width: NOTCH_REST_WIDTH, height: EDGE_REST_DEPTH });
    expect(at(0, 0.5)).toMatchObject({ x: 0, width: EDGE_REST_DEPTH, height: SIDE_REST_LENGTH });
    expect(at(1, 0.5)).toMatchObject({ x: 1512 - EDGE_REST_DEPTH });
    expect(at(0, 0.5).y + SIDE_REST_LENGTH / 2).toBe(491);
  });

  it("are flush with each display's edges, menu bar included", () => {
    const at = (fx: number, fy: number) =>
      wells.find((w) => w.monitorIndex === 0 && w.fx === fx && w.fy === fy)!;
    const [notch, tl, tr, bl, br] = [at(0.5, 0), at(0, 0), at(1, 0), at(0, 1), at(1, 1)];
    expect(notch).toMatchObject({ y: 0, width: NOTCH_REST_WIDTH, height: 38 });
    expect(notch.x + notch.width / 2).toBe(756);
    expect(tl).toMatchObject({ x: 0, y: 0, width: CURL_REST, height: CURL_REST });
    expect(tr).toMatchObject({ x: 1512 - CURL_REST, y: 0 });
    expect(bl).toMatchObject({ x: 0, y: 982 - CURL_REST });
    expect(br).toMatchObject({ x: 1512 - CURL_REST, y: 982 - CURL_REST });
    const external = wells.find((w) => w.monitorIndex === 1 && w.fx === 0.5)!;
    expect(external).toMatchObject({ y: -200, height: 24 });
  });

  it("reads the menu bar from the work area, with a fallback", () => {
    expect(menuBarHeight(LAPTOP)).toBe(38);
    expect(menuBarHeight({ position: { x: 0, y: 0 }, size: { width: 800, height: 600 } })).toBe(
      FALLBACK_MENU_BAR,
    );
  });

  it("picks the nearest well by centres, so a wide notch dragged to a corner lands there", () => {
    // A notch-sized shape dragged so its right end sits at the top-right corner.
    const origin = { x: 1512 - NOTCH_REST_WIDTH, y: 0 };
    const target = nearestNotchWell(origin, { width: NOTCH_REST_WIDTH, height: 38 }, wells);
    expect(target).toMatchObject({ fx: 1, fy: 0, monitorIndex: 0 });
    // Dragged anywhere near the middle of the screen, it does not stay there:
    // the centre is not a well.
    const middle = nearestNotchWell({ x: 700, y: 400 }, { width: 44, height: 44 }, wells);
    expect([middle?.fx, middle?.fy]).not.toEqual([0.5, 0.5]);
    // Near the left edge's middle, it becomes a notch on that edge.
    const left = nearestNotchWell({ x: 10, y: 470 }, { width: 44, height: 44 }, wells);
    expect(left).toMatchObject({ fx: 0, fy: 0.5 });
  });

  it("names its form and corner", () => {
    expect(notchFormFor({ fx: 0.5, fy: 0 })).toBe("notch");
    expect(notchFormFor({ fx: 1, fy: 1 })).toBe("curl");
    expect(notchFormFor({ fx: 0.5, fy: 1 })).toBe("notch");
    expect(notchFormFor({ fx: 1, fy: 0.5 })).toBe("notch");
    expect(notchEdgeFor({ fx: 0.5, fy: 1 })).toBe("bottom");
    expect(notchEdgeFor({ fx: 0, fy: 0.5 })).toBe("left");
    expect(notchEdgeFor({ fx: 1, fy: 0.5 })).toBe("right");
    expect(notchEdgeFor({ fx: 0.25, fy: 0 })).toBe("top");
    expect(notchFormFor(null)).toBe("notch");
    expect(curlCornerFor({ fx: 0, fy: 1 })).toBe("bottom-left");
    expect(curlCornerFor({ fx: 1, fy: 0 })).toBe("top-right");
  });
});

describe("the Notch's steady frame", () => {
  const wells = notchWells([LAPTOP]);

  it("puts the resting shape exactly on its well, over the menu bar", () => {
    for (const well of wells) {
      const layout = steadyLayout(well, LAPTOP, NOTCH_STEADY_SPEC);
      expect(layout.origin.x + layout.anchor.x).toBe(well.x);
      expect(layout.origin.y + layout.anchor.y).toBe(well.y);
      expect(layout.anchor.width).toBe(well.width);
      // Never below the menu bar's edge for a top well.
      if (well.fy === 0) expect(layout.origin.y).toBe(0);
      // The window stays on its display.
      expect(layout.origin.x).toBeGreaterThanOrEqual(0);
      expect(layout.origin.x + layout.size.width).toBeLessThanOrEqual(1512);
    }
  });

  it("glides a notch into a corner flush with that corner", () => {
    const tr = wells.find((w) => w.fx === 1 && w.fy === 0)!;
    expect(glideTarget(tr, { width: NOTCH_REST_WIDTH, height: 38 })).toEqual({
      x: 1512 - NOTCH_REST_WIDTH,
      y: 0,
    });
    const br = wells.find((w) => w.fx === 1 && w.fy === 1)!;
    expect(glideTarget(br, { width: 44, height: 44 })).toEqual({ x: br.x, y: br.y });
  });

  it("grows a side notch straight out of its edge, centred on its tab", () => {
    for (const fx of [0, 1]) {
      const well = wells.find((w) => w.fx === fx && w.fy === 0.5)!;
      const layout = steadyLayout(well, LAPTOP, NOTCH_STEADY_SPEC);
      expect(layout).toMatchObject({ centerY: true, growUp: false });
      const edge = fx === 0 ? "left" : "right";
      const mid = well.y + well.height / 2;
      const sizes = {
        capsule: ISLAND_SIZES.capsule,
        line: ISLAND_SIZES.line,
        status: ISLAND_SIZES.status,
        card: { width: 360, height: 320, radius: 20 },
      };
      for (const [posture, size] of Object.entries(sizes)) {
        const shape = notchShape("notch", posture as keyof typeof sizes, size, 38, "top-left", edge);
        const r = contentRect(layout, shape);
        // The middle never moves, and the shape stays on the screen's edge.
        expect(dockedEdges(layout, shape).y).toBe(mid);
        expect(layout.origin.x + r.x).toBe(fx === 0 ? 0 : 1512 - shape.width);
        expect(layout.origin.y + r.y).toBeGreaterThanOrEqual(0);
        expect(layout.origin.y + r.y + r.height).toBeLessThanOrEqual(982);
      }
      // A dragged shape glides in centred on the tab, not above or below it.
      expect(glideTarget(well, { width: 236, height: 38 }).y).toBe(Math.round(mid - 19));
    }
    // Every other well keeps its row.
    for (const w of wells.filter((w) => w.fy !== 0.5)) {
      expect(steadyLayout(w, LAPTOP, NOTCH_STEADY_SPEC).centerY).toBe(false);
    }
  });
});

describe("the Notch's shapes", () => {
  it("rests as the menu bar strip in the notch, and hangs the island below it", () => {
    const rest = notchShape("notch", "capsule", ISLAND_SIZES.capsule, 38);
    expect(rest).toMatchObject({ width: NOTCH_REST_WIDTH, height: 38 });
    const card = notchShape("notch", "card", { width: 360, height: 200, radius: 20 }, 38);
    expect(card.height).toBe(238);
    expect(card.width).toBe(360 + 2 * SHOULDER);
    expect(card.content).toMatchObject({ top: 38, height: 200 });
  });

  it("grows straight out of the bottom and the sides", () => {
    const bottom = notchShape("notch", "status", ISLAND_SIZES.status, 38, "top-left", "bottom");
    expect(bottom).toMatchObject({ edge: "bottom", height: ISLAND_SIZES.status.height });
    expect(bottom.content.top).toBe(0);
    const side = notchShape("notch", "card", { width: 360, height: 200, radius: 20 }, 38, "top-left", "left");
    expect(side).toMatchObject({ edge: "left", width: 360, height: 200 + 2 * SHOULDER });
    const restSide = notchShape("notch", "capsule", ISLAND_SIZES.capsule, 38, "top-left", "right");
    expect(restSide).toMatchObject({ width: EDGE_REST_DEPTH, height: SIDE_REST_LENGTH });
  });

  it("rests as a peeled corner, and opens into a card with its corner folded", () => {
    const rest = notchShape("curl", "capsule", ISLAND_SIZES.capsule, 38, "bottom-right");
    expect(rest).toMatchObject({ width: CURL_REST, height: CURL_REST, fold: CURL_REST });
    // Folded the whole way, the black is a triangle: the corner itself.
    expect(curlUnderClip(44, 44, 44)).toBe(
      "polygon(0px 0px, 44px 0px, 44px 0px, 0px 44px, 0px 44px)",
    );
    const top = notchShape("curl", "status", ISLAND_SIZES.status, 38, "top-left");
    expect(top.content.top).toBe(38);
    expect(top.height).toBe(38 + ISLAND_SIZES.status.height + FOLD);
    const bottom = notchShape("curl", "status", ISLAND_SIZES.status, 38, "bottom-left");
    expect(bottom.content.top).toBe(FOLD);
    expect(bottom.height).toBe(ISLAND_SIZES.status.height + FOLD);
  });
});
