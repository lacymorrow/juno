import { describe, expect, it } from "vitest";
import {
  ARROW_SHAPE,
  INITIAL_OVERLAY_STATE,
  cursorBox,
  lookFor,
  overlayReducer,
  shapeFor,
  type AgentCursorUpdate,
  type CursorShape,
} from "../agentCursor";

const IBEAM: CursorShape = {
  image: "data:image/png;base64,AA==",
  hotspot_x: 4,
  hotspot_y: 9,
  width: 9,
  height: 18,
};

function update(over: Partial<AgentCursorUpdate> = {}): AgentCursorUpdate {
  return {
    agent_id: "s1",
    x: 500,
    y: 300,
    state: "idle",
    color: "#0A84FF",
    foreground: false,
    origin_x: 0,
    origin_y: 0,
    ...over,
  };
}

describe("agent cursor geometry", () => {
  it("puts the hotspot exactly on the point", () => {
    const box = cursorBox(500, 300, 0, 0, IBEAM);
    expect(box.left + box.hotspotX).toBe(500);
    expect(box.top + box.hotspotY).toBe(300);
    expect(box.width).toBe(9);
    expect(box.height).toBe(18);
  });

  it("draws in window coordinates on a display left of and above the primary", () => {
    // Overlay covers an external display at (-2560, -458): a point near its
    // top left lands near the window's top left, not off screen.
    const box = cursorBox(-2550, -450, -2560, -458, ARROW_SHAPE);
    expect(box.left + box.hotspotX).toBe(10);
    expect(box.top + box.hotspotY).toBe(8);
  });

  it("glows in the real cursor's shape only when Juno holds the real cursor", () => {
    expect(lookFor({ foreground: true })).toBe("glow");
    expect(lookFor({ foreground: false })).toBe("ghost");
    expect(lookFor({})).toBe("ghost");
    expect(shapeFor("glow", IBEAM)).toBe(IBEAM);
    expect(shapeFor("ghost", IBEAM)).toBe(ARROW_SHAPE);
    // Before the first shape arrives, the arrow stands in.
    expect(shapeFor("glow", null)).toBe(ARROW_SHAPE);
    expect(shapeFor("glow", { ...IBEAM, width: 0 })).toBe(ARROW_SHAPE);
  });
});

describe("overlay state", () => {
  it("shows a cursor on its first update without gliding in from nowhere", () => {
    const s = overlayReducer(INITIAL_OVERLAY_STATE, { type: "update", update: update() });
    expect(s.cursors).toHaveLength(1);
    expect(s.cursors[0]).toMatchObject({ id: "s1", visible: true, look: "ghost", instant: true });
  });

  it("glides a visible ghost but never the glow", () => {
    let s = overlayReducer(INITIAL_OVERLAY_STATE, { type: "update", update: update() });
    s = overlayReducer(s, { type: "update", update: update({ x: 700 }) });
    expect(s.cursors[0].instant).toBe(false);

    let g = overlayReducer(INITIAL_OVERLAY_STATE, {
      type: "update",
      update: update({ foreground: true }),
    });
    g = overlayReducer(g, { type: "update", update: update({ foreground: true, x: 700 }) });
    expect(g.cursors[0].instant).toBe(true);
  });

  it("does not glide across a display change", () => {
    let s = overlayReducer(INITIAL_OVERLAY_STATE, { type: "update", update: update() });
    s = overlayReducer(s, {
      type: "update",
      update: update({ x: -100, origin_x: -1920, origin_y: 0 }),
    });
    expect(s.cursors[0].instant).toBe(true);
    expect(s.originX).toBe(-1920);
  });

  it("counts clicks so each one pulses", () => {
    let s = overlayReducer(INITIAL_OVERLAY_STATE, { type: "update", update: update() });
    expect(s.cursors[0].pulse).toBe(0);
    s = overlayReducer(s, { type: "update", update: update({ state: "clicking" }) });
    s = overlayReducer(s, { type: "update", update: update({ state: "moving" }) });
    s = overlayReducer(s, { type: "update", update: update({ state: "clicking" }) });
    expect(s.cursors[0].pulse).toBe(2);
  });

  it("fades, then removes, one cursor and leaves the others", () => {
    let s = overlayReducer(INITIAL_OVERLAY_STATE, { type: "update", update: update() });
    s = overlayReducer(s, {
      type: "update",
      update: update({ agent_id: "s2", color: "#10B981" }),
    });
    s = overlayReducer(s, { type: "fade", id: "s1" });
    expect(s.cursors.find((c) => c.id === "s1")?.visible).toBe(false);
    expect(s.cursors.find((c) => c.id === "s2")?.visible).toBe(true);
    s = overlayReducer(s, { type: "remove", id: "s1" });
    expect(s.cursors.map((c) => c.id)).toEqual(["s2"]);
  });
});
