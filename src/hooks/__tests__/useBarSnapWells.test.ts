import { beforeEach, describe, expect, it, vi } from "vitest";

// One 1440x900 display at 1x. The bar window is 300x200 logical.
const win = {
  label: "floating-bar",
  position: { x: 700, y: 400 },
  outerPosition: vi.fn(async () => ({ ...win.position })),
  outerSize: vi.fn(async () => ({ width: 300, height: 200 })),
  scaleFactor: vi.fn(async () => 1),
  setPosition: vi.fn(async () => {}),
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
