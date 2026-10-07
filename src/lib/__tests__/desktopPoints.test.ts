import { describe, expect, it } from "vitest";
import {
  cursorInPoints,
  monitorsInPoints,
  primaryScale,
  windowOriginInPoints,
} from "../desktopPoints";

// A 1x display left of a 2x Retina primary, as Tauri (tao) reports them:
// each monitor's points times its own scale factor.
const desk = [
  { position: { x: -1920, y: 0 }, size: { width: 1920, height: 1080 }, scaleFactor: 1 },
  { position: { x: 0, y: 0 }, size: { width: 3024, height: 1964 }, scaleFactor: 2 },
];

describe("desktop points", () => {
  it("divides each monitor by its own scale factor", () => {
    expect(monitorsInPoints(desk)).toEqual([
      { position: { x: -1920, y: 0 }, size: { width: 1920, height: 1080 }, scaleFactor: 1 },
      { position: { x: 0, y: 0 }, size: { width: 1512, height: 982 }, scaleFactor: 2 },
    ]);
  });

  it("divides the cursor by the primary display's factor, whichever display it is on", () => {
    expect(primaryScale(desk)).toBe(2);
    // The cursor 100 points into the 1x display is reported at -3640 (x2).
    expect(cursorInPoints({ x: -3640, y: 200 }, desk)).toEqual({ x: -1820, y: 100 });
  });

  it("divides a window's position by the factor of the display it is on", () => {
    expect(windowOriginInPoints({ x: -1820, y: 100 }, 1)).toEqual({ x: -1820, y: 100 });
    expect(windowOriginInPoints({ x: 200, y: 100 }, 2)).toEqual({ x: 100, y: 50 });
  });
});
