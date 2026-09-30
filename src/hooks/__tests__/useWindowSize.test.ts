import { describe, expect, it } from "vitest";
import { anchoredLeft, anchoredTop } from "../useWindowSize";

// `anchoredTop` decides the new physical top edge for a resize that must keep
// one point at the same screen position, whether the window grows down
// (default) or up (docked in the bottom half). The point's distance from the
// top is `anchor` when growing down and `physH - anchor` when growing up; the
// invariant under test is `newTop + fNew === prevTop + fOld`.
//
// The bar passes the point explicitly for BOTH frames (`from` in the config),
// so the baseline is always the live frame plus a known offset. Nothing is
// remembered between resizes, so a glide into a well, a display hop or the
// launch restore cannot leave a stale baseline behind.

const fromTop = (physH: number, anchor: number, growUp: boolean, scale: number) =>
  growUp ? physH - Math.round(anchor * scale) : Math.round(anchor * scale);

describe("anchoredTop", () => {
  it("keeps the top put when growing downward with an unchanged anchor", () => {
    // compact -> hover: only width changes, the band's top edge stays 16 in.
    const prev = { physH: 76, anchor: 16, growUp: false };
    expect(anchoredTop(200, prev, { physH: 76, anchor: 16, growUp: false }, 1)).toBe(200);
  });

  it("keeps the top put when the pane opens downward: all the new room is below", () => {
    // The band's near edge is 16 in before and after, so the top does not move.
    const prev = { physH: 76, anchor: 16, growUp: false };
    expect(anchoredTop(200, prev, { physH: 444, anchor: 16, growUp: false }, 1)).toBe(200);
  });

  it("moves the top up by exactly the added height when growing upward", () => {
    // Docked at the bottom: the pane opens above, the bottom edge stays put.
    const prev = { physH: 76, anchor: 16, growUp: true };
    const next = { physH: 444, anchor: 16, growUp: true };
    const newTop = anchoredTop(700, prev, next, 1);
    expect(newTop).toBe(700 - (444 - 76));
    const pillOld = 700 + fromTop(prev.physH, prev.anchor, prev.growUp, 1);
    const pillNew = newTop + fromTop(next.physH, next.anchor, next.growUp, 1);
    expect(pillNew).toBe(pillOld);
  });

  it("moves the top back down when an upward-grown window shrinks", () => {
    const prev = { physH: 444, anchor: 16, growUp: true };
    const next = { physH: 76, anchor: 16, growUp: true };
    expect(anchoredTop(332, prev, next, 1)).toBe(332 + (444 - 76));
  });

  it("swaps the pane's side without moving the pill when the direction flips", () => {
    // The pane is open below (444 tall, band top at 16) and the bar is dropped
    // in a bottom well: the pane must go above. The pinned point is the band's
    // bottom edge, 60 from the top in the old frame and 16 from the bottom in
    // the new one.
    const prev = { physH: 444, anchor: 16 + 44, growUp: false };
    const next = { physH: 444, anchor: 16, growUp: true };
    const newTop = anchoredTop(200, prev, next, 1);
    expect(newTop + 444 - 16).toBe(200 + 60);
  });

  it("does not move a symmetric window when the direction flips", () => {
    // Pane closed: the window is band + pad each side, so the band's bottom
    // edge is at the same place measured from either end.
    const prev = { physH: 76, anchor: 16 + 44, growUp: false };
    const next = { physH: 76, anchor: 16, growUp: true };
    expect(anchoredTop(200, prev, next, 1)).toBe(200);
  });

  it("honours the display scale factor", () => {
    const prev = { physH: 152, anchor: 16, growUp: true };
    const next = { physH: 888, anchor: 16, growUp: true };
    const newTop = anchoredTop(1400, prev, next, 2);
    const pillOld = 1400 + fromTop(prev.physH, prev.anchor, prev.growUp, 2);
    const pillNew = newTop + fromTop(next.physH, next.anchor, next.growUp, 2);
    expect(pillNew).toBe(pillOld);
  });

  it("falls back to the top edge when there is no anchor at all", () => {
    expect(anchoredTop(150, undefined, { physH: 444, anchor: 16, growUp: true }, 1)).toBe(150);
    expect(anchoredTop(150, { physH: 76, anchor: 16, growUp: false }, { physH: 444 }, 1)).toBe(
      150,
    );
  });
});

describe("anchoredLeft", () => {
  it("keeps the centre by default", () => {
    expect(anchoredLeft(1000, 88, 180)).toBe(954);
    expect(anchoredLeft(954, 180, 88)).toBe(1000);
  });

  it("keeps the left edge for a bar docked on the left", () => {
    expect(anchoredLeft(16, 88, 451, "start")).toBe(16);
  });

  it("keeps the right edge for a bar docked on the right, so it never runs off the screen", () => {
    // Right well on a 1440-wide display: the right edge is at 1424 before and after.
    expect(anchoredLeft(1336, 88, 180, "end")).toBe(1244);
    expect(anchoredLeft(1244, 180, 88, "end")).toBe(1336);
  });

  it("does not move for a same-width resize", () => {
    expect(anchoredLeft(1336, 88, 88, "end")).toBe(1336);
  });
});
