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
import { computeWells, screenInsets, type MonitorRect } from "../snapWells";
import { monitorsInPoints, type TauriMonitorLike } from "../desktopPoints";
import {
  anchorScreenOrigin,
  contentPlacement,
  contentRect,
  dragLayout,
  dockedEdges,
  getSteady,
  registerSteady,
  resetSteady,
  roomForContent,
  sameDrawing,
  setSteadyLayout,
  steadyLayout,
  steadySwapsSettled,
  swapSteadyLayout,
  toScreen,
  type SteadyLayout,
} from "../steadyFrame";

// Desks the bar actually meets, as Tauri reports them (each monitor's points
// times its own scale): a Retina laptop, an external display left of it and
// one above it (negative global coordinates), a scaled 1.5x display, an
// ultrawide with five stops, a short display that forces the clamp, and a 2x
// laptop with a 1x display beside it. The geometry runs on them converted to
// global points, exactly as the bar does.
const TAURI_DESKS: Record<string, TauriMonitorLike[]> = {
  retinaLaptop: [{ position: { x: 0, y: 0 }, size: { width: 2880, height: 1800 }, scaleFactor: 2 }],
  dualLeftAndAbove: [
    { position: { x: 0, y: 0 }, size: { width: 1440, height: 900 }, scaleFactor: 1 },
    { position: { x: -2560, y: -300 }, size: { width: 2560, height: 1440 }, scaleFactor: 1 },
    { position: { x: 0, y: -1080 }, size: { width: 1920, height: 1080 }, scaleFactor: 1 },
  ],
  scaled: [{ position: { x: 0, y: 0 }, size: { width: 2880, height: 1620 }, scaleFactor: 1.5 }],
  ultrawide: [{ position: { x: 1440, y: 0 }, size: { width: 3440, height: 1440 }, scaleFactor: 1 }],
  short: [{ position: { x: 0, y: 0 }, size: { width: 1280, height: 720 }, scaleFactor: 1 }],
  mixedDensity: [
    { position: { x: 0, y: 0 }, size: { width: 3024, height: 1964 }, scaleFactor: 2 },
    { position: { x: 1512, y: 0 }, size: { width: 1920, height: 1080 }, scaleFactor: 1 },
  ],
  // With real work areas (NSScreen.visibleFrame, scaled like the rest): a
  // notched MacBook (38 point menu bar) beside a 1x display with a 24 point
  // menu bar and the Dock at the bottom; a display whose menu bar auto-hides
  // with the Dock on the left.
  notchAndDock: [
    {
      position: { x: 0, y: 0 },
      size: { width: 3024, height: 1964 },
      scaleFactor: 2,
      workArea: { position: { x: 0, y: 76 }, size: { width: 3024, height: 1888 } },
    },
    {
      position: { x: 1512, y: 0 },
      size: { width: 1920, height: 1080 },
      scaleFactor: 1,
      workArea: { position: { x: 1512, y: 24 }, size: { width: 1920, height: 986 } },
    },
  ],
  hiddenMenuBar: [
    {
      position: { x: 0, y: 0 },
      size: { width: 1440, height: 900 },
      scaleFactor: 1,
      workArea: { position: { x: 64, y: 0 }, size: { width: 1376, height: 900 } },
    },
  ],
};
const DESKS: Record<string, MonitorRect[]> = Object.fromEntries(
  Object.entries(TAURI_DESKS).map(([k, v]) => [k, monitorsInPoints(v)]),
);

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
      const w = toScreen(layout, { x: 0, y: 0, ...layout.size });
      const inset = screenInsets(mon);
      expect(w.x, name).toBeGreaterThanOrEqual(mon.position.x + inset.left);
      expect(w.y, name).toBeGreaterThanOrEqual(mon.position.y + inset.top);
      expect(w.x + w.width, name).toBeLessThanOrEqual(mon.position.x + mon.size.width - inset.right);
      expect(w.y + w.height, name).toBeLessThanOrEqual(
        mon.position.y + mon.size.height - inset.bottom,
      );
    }
  });

  it("keeps the window under each display's real menu bar and clear of the Dock", () => {
    const [notch, external] = DESKS.notchAndDock;
    for (const { name, layout, mon } of layouts.filter((l) => l.name.startsWith("notchAndDock"))) {
      const w = toScreen(layout, { x: 0, y: 0, ...layout.size });
      // 38 point notch menu bar, 24 point one on the external, plus the gap.
      const menuBar = mon === notch ? 38 : 24;
      expect(w.y, name).toBeGreaterThanOrEqual(mon.position.y + menuBar + 12);
      if (mon === external) {
        // The external's Dock takes the bottom 70 points.
        expect(w.y + w.height, name).toBeLessThanOrEqual(1080 - 70 - 16);
      }
    }
    for (const { name, layout, well } of layouts.filter((l) => l.name.startsWith("hiddenMenuBar"))) {
      // No menu bar: the top row sits at the ordinary margin; the Dock on the
      // left pushes the left column in.
      if (layout.slot.fy === 0) expect(well.y, name).toBe(16);
      if (layout.slot.fx === 0) expect(well.x, name).toBe(64 + 16);
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

describe("the drag layout: drag the shape, not the stage", () => {
  const layouts = allLayouts();
  const frames = allFrames();

  it("is exactly the footprint, where it is on screen, for every well and every state", () => {
    for (const { name, layout } of layouts) {
      for (const frame of frames) {
        const fp = footprint(layout, frame);
        const drag = dragLayout(layout, fp);
        const where = `${name} ${JSON.stringify(frame)}`;
        const drawn = toScreen(layout, contentRect(layout, fp));
        // The window is the drawn shape, at the shape's own screen position.
        expect(drag.origin, where).toEqual({ x: drawn.x, y: drawn.y });
        expect(drag.size, where).toEqual({ width: fp.width, height: fp.height });
        // So the shape fills it: the hit region is the whole window.
        expect(contentRect(drag, fp), where).toEqual({ x: 0, y: 0, ...drag.size });
        // Nothing inside the shape moves: same docked edges, same column, same
        // growth, the resting footprint (the grab and the wells' reference) in
        // the same place on screen, and the pane keeps its height.
        expect(dockedEdges(drag, fp), where).toEqual(dockedEdges(layout, fp));
        expect(drag.anchorX, where).toBe(layout.anchorX);
        expect(drag.growUp, where).toBe(layout.growUp);
        expect(anchorScreenOrigin(drag), where).toEqual(anchorScreenOrigin(layout));
        if (frame.paneOpen) {
          expect(pillPaneHeight(drag, frame), where).toBe(pillPaneHeight(layout, frame));
        }
        expect(roomForContent(drag), where).toBe(fp.height);
      }
    }
  });

  it("is a different drawing from every docked layout, so the drop swaps it back hidden", () => {
    for (const { name, layout } of layouts) {
      const fp = footprint(layout, frames[0]);
      expect(sameDrawing(dragLayout(layout, fp), layout), name).toBe(false);
    }
  });

  it("lets a pill docked low reach the top row: the pill's top is the window's top", () => {
    // The OS keeps the dragged window's top edge under the menu bar. The
    // steady window put a low-docked pill ~500 points below its own top, so
    // the pill stopped halfway up the screen. The drag window starts at the
    // pill, so the OS stops the pill at the menu bar, above the top row.
    for (const { name, layout, mon } of layouts.filter((l) => l.layout.growUp)) {
      expect(layout.anchor.y, name).toBeGreaterThan(0);
      const drag = dragLayout(layout, PILL_STEADY_SPEC.rest);
      expect(drag.anchor.y, name).toBe(0);
      const menuBarBottom = mon.workArea ? mon.workArea.position.y : mon.position.y + 24;
      expect(menuBarBottom + drag.anchor.y, name).toBeLessThanOrEqual(
        mon.position.y + screenInsets(mon).top,
      );
    }
  });
});

describe("swaps on one window never interleave", () => {
  afterEach(() => resetSteady());
  const mon: MonitorRect = { position: { x: 0, y: 0 }, size: { width: 1440, height: 900 }, scaleFactor: 1 };
  const wells = computeWells([mon], {
    windowWidth: PILL_STEADY_SPEC.rest.width,
    windowHeight: PILL_STEADY_SPEC.rest.height,
    includeCenter: true,
  });
  const at = (fx: number, fy: number) =>
    steadyLayout(wells.find((w) => w.fx === fx && w.fy === fy)!, mon, PILL_STEADY_SPEC);

  it("runs a second swap only after the first, which keeps the shape hidden throughout", async () => {
    registerSteady("bar", PILL_STEADY_SPEC);
    setSteadyLayout("bar", at(1, 0));
    const log: string[] = [];
    let release: () => void = () => {};
    const gate = new Promise<void>((r) => (release = r));
    const first = swapSteadyLayout("bar", at(0, 1), async () => {
      log.push(`first applies, hidden=${getSteady("bar")!.hidden}`);
      await gate;
      log.push("first applied");
    });
    const second = swapSteadyLayout("bar", at(1, 0), async () => {
      log.push(`second applies, hidden=${getSteady("bar")!.hidden}`);
    });
    await new Promise((r) => setTimeout(r, 80));
    // The first is parked mid-swap; the second has not started.
    expect(log).toEqual(["first applies, hidden=true"]);
    release();
    await Promise.all([first, second]);
    expect(log).toEqual([
      "first applies, hidden=true",
      "first applied",
      "second applies, hidden=true",
    ]);
    expect(getSteady("bar")!.layout).toEqual(at(1, 0));
    expect(getSteady("bar")!.hidden).toBe(false);
  });

  it("carries on after a failed swap, and says when everything has settled", async () => {
    registerSteady("bar", PILL_STEADY_SPEC);
    setSteadyLayout("bar", at(1, 0));
    const failed = swapSteadyLayout("bar", at(0, 1), async () => {
      throw new Error("window gone");
    });
    const next = swapSteadyLayout("bar", at(0, 1), async () => {});
    await expect(failed).rejects.toThrow("window gone");
    await steadySwapsSettled("bar");
    await next;
    expect(getSteady("bar")!.layout).toEqual(at(0, 1));
  });

  it("decides 'same drawing' against the layout in force when its turn comes", async () => {
    registerSteady("bar", PILL_STEADY_SPEC);
    setSteadyLayout("bar", at(1, 0));
    const hidden: boolean[] = [];
    void swapSteadyLayout("bar", at(0, 1), async () => {});
    // Asked while (1,0) was in force, but runs after (0,1) landed: same
    // drawing as (0,1), so no hide.
    await swapSteadyLayout("bar", at(0, 1), async () => {
      hidden.push(getSteady("bar")!.hidden);
    });
    expect(hidden).toEqual([false]);
  });
});
