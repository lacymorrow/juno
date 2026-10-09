import { beforeEach, describe, expect, it, vi } from "vitest";

// One 1440x900 display at 1x. The bar window is 300x200 logical.
const win = {
  label: "floating-bar",
  position: { x: 700, y: 400 },
  outerPosition: vi.fn(async () => ({ ...win.position })),
  outerSize: vi.fn(async () => ({ width: 300, height: 200 })),
  scaleFactor: vi.fn(async () => 1),
  setPosition: vi.fn(async () => {}),
  startDragging: vi.fn(async () => {}),
};
let cursor: { x: number; y: number } | null = { x: 0, y: 0 };

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => win,
  availableMonitors: async () => [
    { position: { x: 0, y: 0 }, size: { width: 1440, height: 900 }, scaleFactor: 1 },
  ],
  cursorPosition: async () => {
    if (!cursor) throw new Error("no cursor");
    return cursor;
  },
  LogicalPosition: class {
    constructor(
      public x: number,
      public y: number,
    ) {}
  },
  PhysicalPosition: class {
    constructor(
      public x: number,
      public y: number,
    ) {}
  },
}));
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(async () => {}) }));
vi.mock("@tauri-apps/api/event", () => ({ emit: vi.fn(async () => {}), listen: vi.fn() }));

import { armBarSnap, resetBarSnapState, settleBarSnap } from "../useBarSnapWells";
import { getDockSlot } from "@/lib/barDock";

describe("settleBarSnap", () => {
  beforeEach(() => {
    resetBarSnapState();
    win.setPosition.mockClear();
  });

  it("picks the well from the cursor, not from where the OS held the window", async () => {
    // Grabbed 20px into the window and released with the cursor at the top of
    // the screen. macOS keeps a window's top edge below the menu bar, so a
    // tall steady window is stopped far lower; here it sits mid-screen.
    await armBarSnap({ x: 150, y: 20 });
    win.position = { x: 570, y: 330 };
    cursor = { x: 720, y: 22 };
    await settleBarSnap();
    expect(getDockSlot("floating-bar")?.fy).toBe(0);
  });

  it("falls back to the window position when the cursor cannot be read", async () => {
    await armBarSnap({ x: 150, y: 20 });
    win.position = { x: 570, y: 680 };
    cursor = null;
    await settleBarSnap();
    expect(getDockSlot("floating-bar")?.fy).toBe(1);
  });
});

import { invoke } from "@tauri-apps/api/core";
import { displayFollowTarget, isBarDragging, startBarDrag, steadyWells } from "../useBarSnapWells";
import {
  anchorScreenOrigin,
  contentRect,
  dragLayout,
  getSteady,
  registerSteady,
  resetSteady,
  setSteadyFootprint,
  setSteadyLayout,
  steadyLayout,
  type SteadyLayout,
} from "@/lib/steadyFrame";
import { monitorsInPoints, originUnderCursor } from "@/lib/desktopPoints";
import { wellForSlot } from "@/lib/snapWells";

const SPEC = { rest: { width: 88, height: 76 }, max: { width: 452, height: 574 }, stage: { width: 452, height: 76 } };
const ONE = [{ position: { x: 0, y: 0 }, size: { width: 1440, height: 900 }, scaleFactor: 1 }];
const tick = (ms = 120) => new Promise((r) => setTimeout(r, ms));

/** The frames Rust was asked to apply, in order, as plain rects. */
function framesApplied() {
  return vi
    .mocked(invoke)
    .mock.calls.filter((c) => c[0] === "set_bar_frame")
    .map((c) => c[1] as { x: number; y: number; width: number; height: number });
}

describe("dragging the shape, not the stage", () => {
  let docked: SteadyLayout;

  beforeEach(() => {
    resetBarSnapState();
    resetSteady();
    vi.mocked(invoke).mockReset();
    vi.mocked(invoke).mockImplementation(async () => {});
    win.startDragging.mockClear();
    const wells = steadyWells(ONE, SPEC);
    docked = steadyLayout(wellForSlot({ fx: 0.5, fy: 1 }, 0, wells)!, ONE[0], SPEC);
    registerSteady("floating-bar", SPEC);
    setSteadyLayout("floating-bar", docked);
  });

  it("shrinks the window to the footprint where it is, then hands it to the OS drag once", async () => {
    // The hovered pill is wider than the resting one.
    const footprint = { width: 164, height: 76 };
    setSteadyFootprint("floating-bar", footprint);
    const order: string[] = [];
    vi.mocked(invoke).mockImplementation(async (cmd) => {
      if (cmd === "set_bar_frame") order.push("frame");
    });
    win.startDragging.mockImplementation(async () => {
      order.push("drag");
    });
    startBarDrag({ x: docked.anchor.x + 44, y: docked.anchor.y + 38 });
    await tick();
    const drag = dragLayout(docked, footprint);
    // Placed by the cursor: the spot pressed, re-measured from the shape.
    expect(framesApplied()).toEqual([
      {
        x: drag.origin.x,
        y: drag.origin.y,
        width: 164,
        height: 76,
        grabX: docked.anchor.x + 44 - (drag.origin.x - docked.origin.x),
        grabY: docked.anchor.y + 38 - (drag.origin.y - docked.origin.y),
        startDrag: true,
        trace: null,
      },
    ]);
    expect(win.startDragging).toHaveBeenCalledTimes(1);
    expect(order).toEqual(["frame", "drag"]);
    expect(getSteady("floating-bar")!.layout).toEqual(drag);
    // The pill has not moved on screen.
    expect(anchorScreenOrigin(drag)).toEqual(anchorScreenOrigin(docked));
    // A backend that does not say it started the drag (this mock says
    // nothing) gets the page's startDragging(), as before. No timer-driven
    // drag commands either way.
    const commands = vi.mocked(invoke).mock.calls.map((c) => c[0]);
    expect(commands.some((c) => String(c).startsWith("bar_drag"))).toBe(false);
  });

  it("does not start an OS drag once the button is already up (a flick)", async () => {
    startBarDrag({ x: docked.anchor.x + 44, y: docked.anchor.y + 38 });
    cursor = null;
    win.position = { ...docked.origin };
    await settleBarSnap();
    expect(win.startDragging).not.toHaveBeenCalled();
    // And the settle put the steady window back.
    expect(getSteady("floating-bar")!.layout!.size).toEqual(docked.size);
  });

  it("lands a pill docked low in the top row with the pill on the well, then grows back", async () => {
    startBarDrag({ x: docked.anchor.x + 44, y: docked.anchor.y + 38 });
    await tick();
    const drag = getSteady("floating-bar")!.layout!;
    expect(drag.size).toEqual(SPEC.rest);
    // The OS carried the pill-sized window to the top: its top is just under
    // the menu bar, the pill with it.
    cursor = { x: 720, y: 40 + 38 };
    win.position = { x: 720 - 44 - drag.anchor.x, y: 40 - drag.anchor.y };
    await settleBarSnap();
    expect(getDockSlot("floating-bar")).toEqual({ fx: 0.5, fy: 0 });
    const top = wellForSlot({ fx: 0.5, fy: 0 }, 0, steadyWells(ONE, SPEC))!;
    const landed = getSteady("floating-bar")!.layout!;
    expect(anchorScreenOrigin(landed)).toEqual({ x: top.x, y: top.y });
    expect(landed).toEqual(steadyLayout(top, ONE[0], SPEC));
    // The last frame applied is the steady one for the top well.
    const last = framesApplied().at(-1)!;
    expect(last).toEqual({ ...landed.origin, ...landed.size });
  });

  it("settles a drag whose mouseup never reached the page, once Rust says the button is up", async () => {
    let held = true;
    vi.mocked(invoke).mockImplementation(async (cmd) => {
      if (cmd === "bar_pointer_held") return held;
    });
    startBarDrag({ x: docked.anchor.x + 44, y: docked.anchor.y + 38 });
    await tick(200);
    expect(getDockSlot("floating-bar")).not.toEqual({ fx: 1, fy: 0.5 });
    const drag = getSteady("floating-bar")!.layout!;
    cursor = { x: 1400, y: 450 };
    win.position = { x: 1400 - 44 - drag.anchor.x, y: 450 - 38 - drag.anchor.y };
    held = false;
    await tick(600);
    expect(getDockSlot("floating-bar")).toEqual({ fx: 1, fy: 0.5 });
  });
});

describe("a fast drag keeps the spot pressed under the cursor", () => {
  // What Rust does with a grab: read the cursor at the moment the frame is
  // set and put the grab under it. The cursor here is in points (one 1x
  // display), and it is read when the command runs, not when it was asked for.
  function emulateRustPlacement(frames: { x: number; y: number }[]) {
    vi.mocked(invoke).mockImplementation(async (cmd, args) => {
      if (cmd !== "set_bar_frame") return undefined;
      const a = args as { x: number; y: number; grabX?: number; grabY?: number };
      const placed =
        a.grabX !== undefined && a.grabY !== undefined && cursor
          ? { x: cursor.x - a.grabX, y: cursor.y - a.grabY }
          : { x: a.x, y: a.y };
      frames.push(placed);
      return placed;
    });
  }

  beforeEach(() => {
    resetBarSnapState();
    resetSteady();
    vi.mocked(invoke).mockReset();
    win.startDragging.mockReset();
    win.startDragging.mockImplementation(async () => {});
  });

  const SLOTS = [
    { fx: 0, fy: 0 },
    { fx: 0.5, fy: 0 },
    { fx: 1, fy: 0 },
    { fx: 0, fy: 0.5 },
    { fx: 1, fy: 0.5 },
    { fx: 0, fy: 1 },
    { fx: 0.5, fy: 1 },
    { fx: 1, fy: 1 },
  ];
  // Resting, hovered, listening and the full pill.
  const FOOTPRINTS = [
    { width: 88, height: 76 },
    { width: 164, height: 76 },
    { width: 252, height: 66 },
    { width: 451, height: 76 },
  ];

  for (const slot of SLOTS) {
    for (const fp of FOOTPRINTS) {
      it(`well (${slot.fx},${slot.fy}), footprint ${fp.width}x${fp.height}: flicked 90pt before the drag window lands`, async () => {
        const docked = steadyLayout(wellForSlot(slot, 0, steadyWells(ONE, SPEC))!, ONE[0], SPEC);
        registerSteady("floating-bar", SPEC);
        setSteadyLayout("floating-bar", docked);
        setSteadyFootprint("floating-bar", fp);
        const frames: { x: number; y: number }[] = [];
        emulateRustPlacement(frames);
        const order: string[] = [];
        win.startDragging.mockImplementation(async () => {
          order.push(`drag after ${frames.length} frame(s)`);
        });

        // Pressed 10pt into the shape from its top-left.
        const drawn = contentRect(docked, fp);
        const press = { x: drawn.x + 10, y: drawn.y + 10 };
        const pressedOnScreen = { x: docked.origin.x + press.x, y: docked.origin.y + press.y };
        // The threshold is crossed 4pt later...
        cursor = { x: pressedOnScreen.x + 4, y: pressedOnScreen.y };
        startBarDrag(press);
        // ...and the hand keeps going while the window swaps behind its
        // hidden frames.
        cursor = { x: pressedOnScreen.x + 90, y: pressedOnScreen.y - 40 };
        // Until the OS drag starts, not a fixed delay: the swap waits on
        // animation frames, which a loaded runner can stretch past 120ms.
        await vi.waitFor(() => expect(order).toHaveLength(1), { timeout: 5000 });

        expect(frames).toHaveLength(1);
        expect(order).toEqual(["drag after 1 frame(s)"]);
        const drag = getSteady("floating-bar")!.layout!;
        // The window is where Rust put it, the spot pressed under the cursor
        // as the OS drag takes over: no offset to carry for the whole drag.
        expect(drag.origin).toEqual(frames[0]);
        const grabbed = { x: drag.origin.x + (press.x - drawn.x), y: drag.origin.y + (press.y - drawn.y) };
        expect(grabbed).toEqual(cursor);
        expect(drag.size).toEqual(fp);
        // The resting footprint inside it is where the drop indicator and the
        // settle predict it from the cursor, so the ring and the landing agree.
        expect(anchorScreenOrigin(drag)).toEqual({
          x: cursor!.x - (press.x - docked.anchor.x),
          y: cursor!.y - (press.y - docked.anchor.y),
        });
      });
    }
  }

  it("a slow start (no travel) does not move the shape at all", async () => {
    const docked = steadyLayout(wellForSlot({ fx: 1, fy: 1 }, 0, steadyWells(ONE, SPEC))!, ONE[0], SPEC);
    registerSteady("floating-bar", SPEC);
    setSteadyLayout("floating-bar", docked);
    const fp = { width: 164, height: 76 };
    setSteadyFootprint("floating-bar", fp);
    const frames: { x: number; y: number }[] = [];
    emulateRustPlacement(frames);
    const drawn = contentRect(docked, fp);
    const press = { x: drawn.x + 30, y: drawn.y + 20 };
    cursor = { x: docked.origin.x + press.x, y: docked.origin.y + press.y };
    startBarDrag(press);
    await vi.waitFor(() => expect(win.startDragging).toHaveBeenCalled(), { timeout: 5000 });
    expect(frames).toEqual([dragLayout(docked, fp).origin]);
  });

  it("a look that is not steady is put back under the cursor before the OS drag", async () => {
    // 300x200 window at (700, 400), pressed at (150, 20) inside it; the
    // cursor is 60pt further on when the frame is set.
    win.position = { x: 700, y: 400 };
    const frames: { x: number; y: number }[] = [];
    emulateRustPlacement(frames);
    const order: string[] = [];
    win.startDragging.mockImplementation(async () => {
      order.push(`drag after ${frames.length} frame(s)`);
    });
    cursor = { x: 850 + 60, y: 420 };
    startBarDrag({ x: 150, y: 20 });
    await tick();
    const call = framesApplied()[0] as Record<string, number>;
    expect(call).toMatchObject({ width: 300, height: 200, grabX: 150, grabY: 20 });
    expect(frames).toEqual([{ x: 760, y: 400 }]);
    expect(order).toEqual(["drag after 1 frame(s)"]);
  });
});

describe("the drag starts where the window is placed", () => {
  // Rust: read the cursor, set the frame with the grab under it and, with
  // startDrag and the button down, start the drag in that same call, anchored
  // at the cursor then. The offset the window has from the cursor at that
  // moment is what we record; the follow keeps it from there on (see the
  // "carries the window" tests below).
  let anchors: { x: number; y: number }[];
  let buttonDown: boolean;

  function emulateRust() {
    vi.mocked(invoke).mockImplementation(async (cmd, args) => {
      if (cmd !== "set_bar_frame") return undefined;
      const a = args as { x: number; y: number; grabX?: number; grabY?: number; startDrag?: boolean };
      const placed =
        a.grabX !== undefined && a.grabY !== undefined && cursor
          ? { x: cursor.x - a.grabX, y: cursor.y - a.grabY }
          : { x: a.x, y: a.y };
      const started = Boolean(a.startDrag) && buttonDown;
      if (started) anchors.push({ x: cursor!.x - placed.x, y: cursor!.y - placed.y });
      // The hand keeps going after the call returns: the reply to the page
      // and anything the page does next happen with the cursor elsewhere.
      if (a.grabX !== undefined && cursor) cursor = { x: cursor.x + 37, y: cursor.y - 21 };
      if (!a.startDrag) return placed;
      return { ...placed, drag: started ? "started" : "released" };
    });
  }

  beforeEach(() => {
    resetBarSnapState();
    resetSteady();
    vi.mocked(invoke).mockReset();
    win.startDragging.mockReset();
    win.startDragging.mockImplementation(async () => {
      // What the page-started drag used to anchor at: the cursor by then.
      const w = getSteady("floating-bar")?.layout?.origin ?? win.position;
      anchors.push({ x: cursor!.x - w.x, y: cursor!.y - w.y });
    });
    anchors = [];
    buttonDown = true;
    emulateRust();
  });

  it("anchors at the spot pressed even though the cursor moves on after the placement", async () => {
    const docked = steadyLayout(wellForSlot({ fx: 1, fy: 0 }, 0, steadyWells(ONE, SPEC))!, ONE[0], SPEC);
    registerSteady("floating-bar", SPEC);
    setSteadyLayout("floating-bar", docked);
    const fp = { width: 164, height: 76 };
    setSteadyFootprint("floating-bar", fp);
    const drawn = contentRect(docked, fp);
    const press = { x: drawn.x + 12, y: drawn.y + 9 };
    cursor = { x: docked.origin.x + press.x + 90, y: docked.origin.y + press.y + 40 };
    startBarDrag(press);
    await vi.waitFor(() => expect(anchors).toHaveLength(1), { timeout: 5000 });
    // The drag was anchored with the spot pressed exactly under the cursor...
    expect(anchors[0]).toEqual({ x: press.x - drawn.x, y: press.y - drawn.y });
    // ...and the page did not start a second, late one.
    await vi.waitFor(() => expect(getSteady("floating-bar")!.layout!.size).toEqual(fp), {
      timeout: 5000,
    });
    expect(win.startDragging).not.toHaveBeenCalled();
  });

  it("a look that is not steady also has its drag started by Rust", async () => {
    win.position = { x: 700, y: 400 };
    cursor = { x: 850 + 60, y: 420 };
    startBarDrag({ x: 150, y: 20 });
    await vi.waitFor(() => expect(anchors).toHaveLength(1), { timeout: 5000 });
    expect(anchors[0]).toEqual({ x: 150, y: 20 });
    await settleBarSnap();
    expect(win.startDragging).not.toHaveBeenCalled();
  });

  it("starts no drag at all when the button was already up (a flick)", async () => {
    buttonDown = false;
    const docked = steadyLayout(wellForSlot({ fx: 0, fy: 1 }, 0, steadyWells(ONE, SPEC))!, ONE[0], SPEC);
    registerSteady("floating-bar", SPEC);
    setSteadyLayout("floating-bar", docked);
    cursor = { x: 300, y: 700 };
    startBarDrag({ x: docked.anchor.x + 10, y: docked.anchor.y + 10 });
    await vi.waitFor(() => expect(framesApplied()).toHaveLength(1), { timeout: 5000 });
    await vi.waitFor(() => expect(getSteady("floating-bar")!.layout!.size).toEqual(SPEC.rest), {
      timeout: 5000,
    });
    expect(anchors).toEqual([]);
    expect(win.startDragging).not.toHaveBeenCalled();
  });

  it("is a drag in progress from the start until the snap has settled", async () => {
    const docked = steadyLayout(wellForSlot({ fx: 1, fy: 0 }, 0, steadyWells(ONE, SPEC))!, ONE[0], SPEC);
    registerSteady("floating-bar", SPEC);
    setSteadyLayout("floating-bar", docked);
    cursor = { x: 1300, y: 60 };
    expect(isBarDragging()).toBe(false);
    startBarDrag({ x: docked.anchor.x + 10, y: docked.anchor.y + 10 });
    expect(isBarDragging()).toBe(true);
    await vi.waitFor(() => expect(anchors).toHaveLength(1), { timeout: 5000 });
    const settling = settleBarSnap();
    expect(isBarDragging()).toBe(true);
    await settling;
    expect(isBarDragging()).toBe(false);
    // The release is reported once, for the [Drag] log.
    const released = vi.mocked(invoke).mock.calls.filter((c) => c[0] === "bar_drag_released");
    expect(released).toHaveLength(1);
  });
});

describe("Rust carries the window with the cursor for the whole drag", () => {
  // Rust as it now is (`platform/bar_drag_follow.rs`): `set_bar_frame` with
  // startDrag places the window with the grab under the cursor and, with the
  // button down, starts following; every mouse-dragged event after that puts
  // the window's top-left at the cursor minus that same grab, until the
  // button comes up or the page's settle stops it (`bar_drag_released`).
  let follow: { x: number; y: number } | null;
  let buttonDown: boolean;
  let log: string[];

  function emulateRust() {
    vi.mocked(invoke).mockImplementation(async (cmd, args) => {
      if (cmd === "bar_pointer_held") return buttonDown;
      if (cmd === "bar_drag_released") {
        log.push(follow ? "stopped by the page" : "released");
        follow = null;
        return undefined;
      }
      if (cmd !== "set_bar_frame") return undefined;
      const a = args as { x: number; y: number; grabX?: number; grabY?: number; startDrag?: boolean };
      const grab = a.grabX !== undefined && a.grabY !== undefined ? { x: a.grabX, y: a.grabY } : null;
      const placed = grab && cursor ? originUnderCursor(cursor, grab) : { x: a.x, y: a.y };
      win.position = { ...placed };
      if (!a.startDrag) return placed;
      if (!buttonDown || !grab) return { ...placed, drag: "released" };
      follow = grab;
      log.push("following");
      return { ...placed, drag: "started" };
    });
  }

  /** One mouse-dragged event: the hand is at `p`. */
  function dragTo(p: { x: number; y: number }) {
    cursor = p;
    if (follow && buttonDown) win.position = originUnderCursor(p, follow);
  }

  /** The button comes up; Rust's monitor sees it and stops. */
  function release() {
    buttonDown = false;
    follow = null;
  }

  /** How far the spot pressed is from the cursor right now. */
  function drift(grab: { x: number; y: number }) {
    return { x: cursor!.x - win.position.x - grab.x, y: cursor!.y - win.position.y - grab.y };
  }

  beforeEach(() => {
    resetBarSnapState();
    resetSteady();
    vi.mocked(invoke).mockReset();
    win.startDragging.mockReset();
    win.setPosition.mockReset();
    win.setPosition.mockImplementation(async () => {
      log.push("glide");
    });
    follow = null;
    buttonDown = true;
    log = [];
    emulateRust();
  });

  // Speeds in points per mouse-dragged event: a slow drag, a quick one, and
  // a flick far faster than a hand (a 120 Hz trackpad at 30,000 pt/s).
  for (const speed of [3, 40, 250]) {
    it(`keeps the spot pressed under the cursor at ${speed}pt per event, with zero drift after 600 events`, async () => {
      const docked = steadyLayout(wellForSlot({ fx: 1, fy: 1 }, 0, steadyWells(ONE, SPEC))!, ONE[0], SPEC);
      registerSteady("floating-bar", SPEC);
      setSteadyLayout("floating-bar", docked);
      const fp = { width: 164, height: 76 };
      setSteadyFootprint("floating-bar", fp);
      const drawn = contentRect(docked, fp);
      const press = { x: drawn.x + 30, y: drawn.y + 20 };
      const grab = { x: press.x - drawn.x, y: press.y - drawn.y };
      cursor = { x: docked.origin.x + press.x + 4, y: docked.origin.y + press.y };
      startBarDrag(press);
      // The hand keeps moving while the window swaps behind its hidden frames.
      dragTo({ x: cursor.x - speed, y: cursor.y - speed });
      await vi.waitFor(() => expect(log).toContain("following"), { timeout: 5000 });
      expect(drift(grab)).toEqual({ x: 0, y: 0 });

      // A zig-zag across the whole display and past its edges, every event
      // `speed` points from the last, checked after every single one.
      let worst = 0;
      let p = { ...cursor! };
      for (let i = 0; i < 600; i++) {
        const angle = i * 0.91;
        p = {
          x: Math.min(1700, Math.max(-200, p.x + speed * Math.cos(angle))),
          y: Math.min(1100, Math.max(-100, p.y + speed * Math.sin(angle * 1.3))),
        };
        dragTo(p);
        const d = drift(grab);
        worst = Math.max(worst, Math.abs(d.x), Math.abs(d.y));
      }
      expect(worst).toBe(0);
      expect(drift(grab)).toEqual({ x: 0, y: 0 });
      expect(win.startDragging).not.toHaveBeenCalled();

      // Released near the top-left: it lands in the top-left well, with the
      // resting footprint exactly on it.
      dragTo({ x: 140, y: 90 });
      release();
      await settleBarSnap();
      expect(getDockSlot("floating-bar")).toEqual({ fx: 0, fy: 0 });
      const well = wellForSlot({ fx: 0, fy: 0 }, 0, steadyWells(ONE, SPEC))!;
      expect(anchorScreenOrigin(getSteady("floating-bar")!.layout!)).toEqual({ x: well.x, y: well.y });
    });
  }

  it("stops following before the glide when the release was never seen", async () => {
    const docked = steadyLayout(wellForSlot({ fx: 0.5, fy: 1 }, 0, steadyWells(ONE, SPEC))!, ONE[0], SPEC);
    registerSteady("floating-bar", SPEC);
    setSteadyLayout("floating-bar", docked);
    cursor = { x: docked.origin.x + docked.anchor.x + 40, y: docked.origin.y + docked.anchor.y + 30 };
    startBarDrag({ x: docked.anchor.x + 40, y: docked.anchor.y + 30 });
    await vi.waitFor(() => expect(log).toContain("following"), { timeout: 5000 });
    for (let i = 0; i < 50; i++) dragTo({ x: cursor!.x + 23, y: cursor!.y - 15 });
    // The button comes up where no monitor hears it: Rust is still following
    // and the page heard no mouseup. The release watch notices.
    buttonDown = false;
    await vi.waitFor(() => expect(isBarDragging()).toBe(false), { timeout: 5000 });
    // The page stopped the follow, and only then did anything glide.
    expect(log[log.indexOf("following") + 1]).toBe("stopped by the page");
    const firstGlide = log.indexOf("glide");
    if (firstGlide >= 0) expect(firstGlide).toBeGreaterThan(log.indexOf("stopped by the page"));
    expect(follow).toBeNull();
    // And it landed in a well, not where the hand let go.
    expect(getDockSlot("floating-bar")).not.toBeNull();
    expect(getSteady("floating-bar")!.layout!.size).toEqual(docked.size);
  });

  it("a look that is not steady is carried the same way", async () => {
    win.position = { x: 700, y: 400 };
    cursor = { x: 850 + 60, y: 420 };
    startBarDrag({ x: 150, y: 20 });
    await vi.waitFor(() => expect(log).toContain("following"), { timeout: 5000 });
    for (let i = 0; i < 200; i++) {
      dragTo({ x: 910 - i * 4, y: 420 + ((i * 37) % 300) });
      expect(drift({ x: 150, y: 20 })).toEqual({ x: 0, y: 0 });
    }
    release();
    await settleBarSnap();
    expect(win.startDragging).not.toHaveBeenCalled();
    expect(log).toContain("released");
  });
});

describe("display follow on mixed desks", () => {
  // A 2x laptop with a 1x display right of, left of and above it, as Tauri
  // reports them; the cursor arrives from Rust in points.
  const laptop = { position: { x: 0, y: 0 }, size: { width: 3024, height: 1964 }, scaleFactor: 2 };
  const externals = {
    right: { position: { x: 1512, y: 0 }, size: { width: 1920, height: 1080 }, scaleFactor: 1 },
    left: { position: { x: -1920, y: 0 }, size: { width: 1920, height: 1080 }, scaleFactor: 1 },
    above: { position: { x: 0, y: -1080 }, size: { width: 1920, height: 1080 }, scaleFactor: 1 },
  };

  for (const [where, ext] of Object.entries(externals)) {
    it(`re-homes to the same slot on the display ${where}`, () => {
      const rects = monitorsInPoints([laptop, ext]);
      const wells = steadyWells(rects, SPEC);
      const cursor = {
        x: rects[1].position.x + rects[1].size.width / 2,
        y: rects[1].position.y + rects[1].size.height / 2,
      };
      const target = displayFollowTarget(rects, { fx: 1, fy: 0 }, cursor, 0, wells)!;
      expect(target.monitorIndex).toBe(1);
      expect(target).toMatchObject({ fx: 1, fy: 0 });
      // Top-right of the external, with its own padding, in points.
      expect(target.x).toBe(rects[1].position.x + rects[1].size.width - 16 - 88);
      expect(target.y).toBe(rects[1].position.y + 36);
      // And its steady window stays on that display.
      const layout = steadyLayout(target, rects[1], SPEC);
      expect(layout.origin.x).toBeGreaterThanOrEqual(rects[1].position.x);
    });
  }

  it("stays put when the cursor is on the bar's own display", () => {
    const rects = monitorsInPoints([laptop, externals.right]);
    expect(displayFollowTarget(rects, { fx: 1, fy: 0 }, { x: 100, y: 100 }, 0, steadyWells(rects, SPEC))).toBeNull();
  });
});
