import { afterEach, describe, expect, it } from "vitest";

import {
  BAR_COMPOSER_MAX_PX,
  BAR_LAYOUTS,
  BAR_PILL_BUTTON_PX,
  PILL_MIN_PANE_PX,
  PILL_STEADY_SPEC,
  pillFootprint,
  pillPaneHeight,
  type BarLayout,
  type PillFrame,
} from "@/components/FloatingBar";
import { computeWells, type MonitorRect } from "../snapWells";
import {
  anchorScreenOrigin,
  contentPlacement,
  contentRect,
  dockedEdges,
  getSteady,
  registerSteady,
  resetSteady,
  sameDrawing,
  setSteadyLayout,
  steadyLayout,
  swapSteadyLayout,
  toScreen,
  type SteadyLayout,
} from "../steadyFrame";

// Desks the bar actually meets: a Retina laptop, an external display left of
// it and one above it (negative global coordinates), a scaled 1.5x display,
// an ultrawide with five stops, and a short display that forces the clamp.
const DESKS: Record<string, MonitorRect[]> = {
  retinaLaptop: [{ position: { x: 0, y: 0 }, size: { width: 2880, height: 1800 }, scaleFactor: 2 }],
  dualLeftAndAbove: [
    { position: { x: 0, y: 0 }, size: { width: 1440, height: 900 }, scaleFactor: 1 },
    { position: { x: -2560, y: -300 }, size: { width: 2560, height: 1440 }, scaleFactor: 1 },
    { position: { x: 0, y: -1080 }, size: { width: 1920, height: 1080 }, scaleFactor: 1 },
  ],
  scaled: [{ position: { x: 0, y: 0 }, size: { width: 2880, height: 1620 }, scaleFactor: 1.5 }],
  ultrawide: [{ position: { x: 1440, y: 0 }, size: { width: 3440, height: 1440 }, scaleFactor: 1 }],
  short: [{ position: { x: 0, y: 0 }, size: { width: 1280, height: 720 }, scaleFactor: 1 }],
};

/** Every frame the pill can draw: each layout, with and without each extra. */
function allFrames(): PillFrame[] {
  const out: PillFrame[] = [];
  for (const layout of Object.keys(BAR_LAYOUTS) as BarLayout[]) {
    for (const paneOpen of [false, true]) {
      for (const rosterVisible of [false, true]) {
        for (const composerGrowth of [0, 36, BAR_COMPOSER_MAX_PX]) {
          // The extra control slot exists on the narrower pills only; the
          // full pill already has the room.
          for (const extraWidth of layout === "full" ? [0] : [0, BAR_PILL_BUTTON_PX]) {
            out.push({ layout, paneOpen, rosterVisible, composerGrowth, extraWidth });
          }
        }
      }
    }
  }
  return out;
}

/** Every well on every desk, with its steady layout. */
interface Case {
  name: string;
  layout: SteadyLayout;
  well: { x: number; y: number };
  mon: MonitorRect;
}

function allLayouts(): Case[] {
  const out: Case[] = [];
  for (const [desk, monitors] of Object.entries(DESKS)) {
    const wells = computeWells(monitors, {
      windowWidth: PILL_STEADY_SPEC.rest.width,
      windowHeight: PILL_STEADY_SPEC.rest.height,
      includeCenter: true,
    });
    for (const well of wells) {
      const layout = steadyLayout(well, monitors[well.monitorIndex], PILL_STEADY_SPEC);
      out.push({
        name: `${desk} m${well.monitorIndex} (${well.fx},${well.fy})`,
        layout,
        well,
        mon: monitors[well.monitorIndex],
      });
    }
  }
  return out;
}

const footprint = (layout: SteadyLayout, frame: PillFrame) =>
  pillFootprint({ ...frame, paneHeight: pillPaneHeight(layout, frame) });

describe("the Pill's steady frame", () => {
  const layouts = allLayouts();
  const frames = allFrames();

  it("covers every kind of well: corners, edges, centre, on every desk", () => {
    const slots = new Set(layouts.map((l) => `${l.layout.anchorX}/${l.layout.growUp}`));
    // Left, centre and right columns, each growing down and up.
    expect(slots.size).toBe(6);
    expect(layouts.length).toBeGreaterThan(40);
  });

  it("keeps the docked edge at the same screen pixel through every state, in every well", () => {
    for (const { name, layout } of layouts) {
      const rest = dockedEdges(layout, footprint(layout, frames[0]));
      for (const frame of frames) {
        const edge = dockedEdges(layout, footprint(layout, frame));
        expect(edge, `${name} ${JSON.stringify(frame)}`).toEqual(rest);
      }
    }
  });

  it("puts the resting pill exactly on its well, so the idle bar sits where it always has", () => {
    for (const { name, layout, well } of layouts) {
      expect(anchorScreenOrigin(layout), name).toEqual({ x: well.x, y: well.y });
    }
  });

  it("never draws past the window, in any state, in any well", () => {
    for (const { name, layout } of layouts) {
      for (const frame of frames) {
        const r = contentRect(layout, footprint(layout, frame));
        const where = `${name} ${JSON.stringify(frame)}`;
        expect(r.x, where).toBeGreaterThanOrEqual(0);
        expect(r.y, where).toBeGreaterThanOrEqual(0);
        expect(r.x + r.width, where).toBeLessThanOrEqual(layout.size.width);
        expect(r.y + r.height, where).toBeLessThanOrEqual(layout.size.height);
      }
    }
  });

  it("keeps the window on its display, inside the wells' inset area", () => {
    for (const { name, layout, mon } of layouts) {
      const sf = layout.scaleFactor;
      const w = toScreen(layout, { x: 0, y: 0, ...layout.size });
      expect(w.x, name).toBeGreaterThanOrEqual(mon.position.x + 16 * sf);
      expect(w.y, name).toBeGreaterThanOrEqual(mon.position.y + 36 * sf);
      expect(w.x + w.width, name).toBeLessThanOrEqual(mon.position.x + mon.size.width - 16 * sf);
      expect(w.y + w.height, name).toBeLessThanOrEqual(mon.position.y + mon.size.height - 16 * sf);
    }
  });

  it("gives the pane its full height in every well but a middle one on a short display", () => {
    for (const { name, layout } of layouts) {
      const h = pillPaneHeight(layout);
      if (name.startsWith("short") && layout.slot.fy === 0.5) {
        expect(h, name).toBeLessThan(360);
        expect(h, name).toBeGreaterThanOrEqual(PILL_MIN_PANE_PX);
      } else {
        expect(h, name).toBe(360);
      }
    }
  });

  it("lets a tight pane give up only the room the composer takes, so its far edge holds still", () => {
    for (const { name, layout } of layouts) {
      const base = { layout: "full" as const, paneOpen: true, rosterVisible: false, extraWidth: 0 };
      const one = contentRect(layout, footprint(layout, { ...base, composerGrowth: 0 }));
      const grown = contentRect(layout, footprint(layout, { ...base, composerGrowth: 36 }));
      const farEdge = (r: { y: number; height: number }) => (layout.growUp ? r.y : r.y + r.height);
      if (pillPaneHeight(layout) < 360) {
        expect(farEdge(grown), name).toBe(farEdge(one));
      }
    }
  });

  it("is never resized: the window size is the same in every well", () => {
    for (const { layout } of layouts) expect(layout.size).toEqual(PILL_STEADY_SPEC.max);
  });

  it("places content by the docked edges in CSS, the same edges the geometry pins", () => {
    for (const { name, layout } of layouts) {
      const p = contentPlacement(layout);
      if (layout.anchorX === "start") expect(p.left, name).toBe(layout.anchor.x);
      if (layout.anchorX === "end")
        expect(p.right, name).toBe(layout.size.width - layout.anchor.x - layout.anchor.width);
      if (layout.anchorX === "center") expect(p.translateX, name).toBe(true);
      if (layout.growUp)
        expect(p.bottom, name).toBe(layout.size.height - layout.anchor.y - layout.anchor.height);
      else expect(p.top, name).toBe(layout.anchor.y);
    }
  });
});

describe("swapSteadyLayout", () => {
  afterEach(() => resetSteady());
  const mon: MonitorRect = { position: { x: 0, y: 0 }, size: { width: 1440, height: 900 }, scaleFactor: 1 };
  const wells = computeWells([mon], {
    windowWidth: PILL_STEADY_SPEC.rest.width,
    windowHeight: PILL_STEADY_SPEC.rest.height,
    includeCenter: true,
  });
  const at = (fx: number, fy: number) =>
    steadyLayout(wells.find((w) => w.fx === fx && w.fy === fy)!, mon, PILL_STEADY_SPEC);

  it("moves the frame without hiding anything when the drawing is the same", async () => {
    registerSteady("bar", PILL_STEADY_SPEC);
    setSteadyLayout("bar", at(1, 0));
    const next = at(1, 0);
    expect(sameDrawing(at(1, 0), next)).toBe(true);
    const hiddenDuring: boolean[] = [];
    await swapSteadyLayout("bar", next, async () => {
      hiddenDuring.push(getSteady("bar")!.hidden);
    });
    expect(hiddenDuring).toEqual([false]);
    expect(getSteady("bar")!.layout).toEqual(next);
  });

  it("hides the drawing while a frame that grows the other way goes in, and shows it after", async () => {
    registerSteady("bar", PILL_STEADY_SPEC);
    setSteadyLayout("bar", at(1, 0));
    const next = at(0, 1);
    expect(sameDrawing(at(1, 0), next)).toBe(false);
    const hiddenDuring: boolean[] = [];
    await swapSteadyLayout("bar", next, async () => {
      hiddenDuring.push(getSteady("bar")!.hidden);
    });
    expect(hiddenDuring).toEqual([true]);
    expect(getSteady("bar")!.hidden).toBe(false);
    expect(getSteady("bar")!.layout).toEqual(next);
  });

  it("shows the drawing again even when the frame cannot be applied", async () => {
    registerSteady("bar", PILL_STEADY_SPEC);
    setSteadyLayout("bar", at(1, 0));
    await expect(
      swapSteadyLayout("bar", at(0, 1), async () => {
        throw new Error("window gone");
      }),
    ).rejects.toThrow("window gone");
    expect(getSteady("bar")!.hidden).toBe(false);
  });
});
