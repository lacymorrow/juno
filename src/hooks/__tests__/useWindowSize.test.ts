import { describe, expect, it } from "vitest";
import { anchoredTop } from "../useWindowSize";

// `anchoredTop` decides the new physical top edge for a resize that must keep
// the pill's vertical centre at the same screen position, whether the window
// grows down (default) or up (docked in the bottom half). The pill centre's
// distance from the top is `anchor` when growing down and `physH - anchor`
// when growing up; the invariant under test is `newTop + fNew === prevTop + fOld`.

const fromTop = (physH: number, anchor: number, growUp: boolean, scale: number) =>
  growUp ? physH - Math.round(anchor * scale) : Math.round(anchor * scale);

describe("anchoredTop", () => {
  it("keeps the top put when growing downward with an unchanged anchor", () => {
    // compact -> hover: only width changes, anchor 33 stays, top must not move.
    const prev = { physH: 66, anchor: 33, growUp: false };
    expect(anchoredTop(200, prev, { physH: 66, anchor: 33, growUp: false }, 1)).toBe(200);
  });

  it("keeps the pill centre fixed when the anchor changes downward", () => {
    // compact -> full+pane, downward: anchor 33 -> 46, pane grows below.
    const prev = { physH: 66, anchor: 33, growUp: false };
    const next = { physH: 460, anchor: 46, growUp: false };
    const newTop = anchoredTop(200, prev, next, 1);
    const pillOld = 200 + fromTop(prev.physH, prev.anchor, prev.growUp, 1);
    const pillNew = newTop + fromTop(next.physH, next.anchor, next.growUp, 1);
    expect(pillNew).toBe(pillOld);
    // Top-anchored: the top only shifts by the small anchor delta.
    expect(newTop).toBe(200 - (46 - 33));
  });

  it("moves the top up so the pill stays put when growing upward", () => {
    // Docked at the bottom: compact -> full+pane, growUp. The window gains
    // height entirely above the pill; the pill's screen Y is unchanged.
    const prev = { physH: 66, anchor: 33, growUp: true };
    const next = { physH: 460, anchor: 46, growUp: true };
    const newTop = anchoredTop(700, prev, next, 1);
    const pillOld = 700 + fromTop(prev.physH, prev.anchor, prev.growUp, 1);
    const pillNew = newTop + fromTop(next.physH, next.anchor, next.growUp, 1);
    expect(pillNew).toBe(pillOld);
    // The top moved strictly upward to make room for the pane above.
    expect(newTop).toBeLessThan(700);
  });

  it("moves the top back down when an upward-grown window shrinks", () => {
    const prev = { physH: 460, anchor: 46, growUp: true };
    const next = { physH: 66, anchor: 33, growUp: true };
    const newTop = anchoredTop(319, prev, next, 1);
    const pillOld = 319 + fromTop(prev.physH, prev.anchor, prev.growUp, 1);
    const pillNew = newTop + fromTop(next.physH, next.anchor, next.growUp, 1);
    expect(pillNew).toBe(pillOld);
    expect(newTop).toBeGreaterThan(319);
  });

  it("honours the display scale factor", () => {
    const prev = { physH: 132, anchor: 33, growUp: true };
    const next = { physH: 920, anchor: 46, growUp: true };
    const newTop = anchoredTop(1400, prev, next, 2);
    const pillOld = 1400 + fromTop(prev.physH, prev.anchor, prev.growUp, 2);
    const pillNew = newTop + fromTop(next.physH, next.anchor, next.growUp, 2);
    expect(pillNew).toBe(pillOld);
  });

  it("falls back to the top edge when there is no previous state", () => {
    expect(anchoredTop(150, undefined, { physH: 460, anchor: 46, growUp: true }, 1)).toBe(150);
  });

  // Regression: gravity-well drift. When the window is moved outside the resize
  // path (a well snap, a display hop, the launch restore), the real top changes
  // but the stored baseline does not. The next resize then corrects against a
  // position that no longer exists and the pill drifts. resetWindowAnchor clears
  // that baseline so the next resize is top-anchored from the real position;
  // here that is the difference between the drifted top and the correct one.
  it("would drift if a stale baseline survives an external move, and does not once cleared", () => {
    // A resize was recorded while the window sat at top=200 (pill band 46 down).
    const staleBaseline = { physH: 92, anchor: 46, growUp: false };
    // The bar then snapped to a well at top=900 without the cache being reset.
    const realTopAfterSnap = 900;
    // A same-shape resize now runs. With the stale baseline the correction is
    // zero here (same anchor), but a growUp resize exposes the drift:
    const next = { physH: 460, anchor: 46, growUp: true };
    const drifted = anchoredTop(realTopAfterSnap, staleBaseline, next, 1);
    const corrected = anchoredTop(realTopAfterSnap, undefined, next, 1);
    expect(corrected).toBe(realTopAfterSnap); // top-anchored from where it really is
    expect(drifted).not.toBe(corrected); // the stale baseline moved it away from that
  });
});
