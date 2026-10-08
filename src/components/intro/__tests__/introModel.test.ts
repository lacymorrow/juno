import { describe, expect, it } from "vitest";
import { FRAG, LOOK, progressAt, uniformsFor, type IntroPlan } from "../introModel";

const topCentre: IntroPlan = {
  duration_ms: 2600,
  bar_at_ms: 1000,
  pill_x: 280,
  pill_y: 40,
  pill_w: 56,
  pill_h: 16,
  pill_radius: 8,
  inward_x: 0,
  inward_y: 1,
};
const viewport = { width: 560, height: 360 };

describe("uniformsFor", () => {
  it("flips the plan from top-left y-down points into bottom-left y-up device pixels", () => {
    const u = uniformsFor(topCentre, viewport, 2);
    expect(u.uRes).toEqual([1120, 720]);
    expect(u.uPill).toEqual([560, (360 - 40) * 2]);
    // "Down" in the plan is "-y" in the shader.
    expect(u.uInward).toEqual([0, -1]);
  });

  it("trusts only the centre of what the backend measured: the Pill reports its hit footprint, which is wider than the shape", () => {
    const wide = { ...topCentre, pill_w: 88, pill_h: 76, pill_radius: 38 };
    expect(uniformsFor(wide, viewport, 2)).toEqual(uniformsFor(topCentre, viewport, 2));
  });

  it("carries both tones at once, so the cloud reads over a dark desktop and a light one", () => {
    const u = uniformsFor(topCentre, viewport, 1);
    expect(u.uDark).toEqual([...LOOK.dark]);
    expect(u.uLight).toEqual([...LOOK.light]);
    // Near black and near white, not two greys: each must show on its own
    // against the other's background.
    expect(Math.max(...LOOK.dark)).toBeLessThan(0.15);
    expect(Math.min(...LOOK.light)).toBeGreaterThan(0.9);
    expect(u.uLightness).toBe(LOOK.lightness);
    expect(u.uRelief).toBe(LOOK.relief);
  });

  it("leaves a mid-screen pill with no direction, so the cloud is round", () => {
    const centred = { ...topCentre, inward_x: 0, inward_y: 0 };
    expect(uniformsFor(centred, viewport, 1).uInward).toEqual([0, -0]);
  });

  it("starts at the beginning of the sequence", () => {
    const u = uniformsFor(topCentre, viewport, 1);
    expect(u.uT).toBe(0);
    expect(u.uTime).toBe(0);
  });
});

describe("progressAt", () => {
  it("runs 0..1 over the backend's duration and clamps at both ends", () => {
    expect(progressAt(1000, 1000, 2600)).toBe(0);
    expect(progressAt(2300, 1000, 2600)).toBeCloseTo(0.5);
    expect(progressAt(9000, 1000, 2600)).toBe(1);
    expect(progressAt(500, 1000, 2600)).toBe(0);
  });

  it("treats a zero-length sequence as already over", () => {
    expect(progressAt(1000, 1000, 0)).toBe(1);
  });
});

describe("the shader", () => {
  it("reads every uniform the model produces, so a renamed tweakable cannot go quietly unused", () => {
    const u = uniformsFor(topCentre, viewport, 1);
    for (const name of Object.keys(u)) {
      expect(FRAG).toMatch(new RegExp(`uniform\\s+\\w+\\s+${name};`));
    }
  });

  it("draws no rim around the pill: the smoke is the whole picture", () => {
    expect(FRAG).not.toMatch(/rim|ring|halo/i);
  });

  it("leaves no hole under the bar: nothing in the shader knows the bar's shape", () => {
    expect(FRAG).not.toMatch(/sdRound|inside|hollow/);
    expect(FRAG).not.toMatch(/uHalf|uRadius/);
  });
});
